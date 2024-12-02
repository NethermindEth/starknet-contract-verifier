use std::{fmt::Display, fs, path::PathBuf, time::Duration};

use anyhow::anyhow;
use backon::{BlockingRetryable, ExponentialBuilder};
use reqwest::{
    blocking::{self, multipart, Client},
    StatusCode,
};
use semver;
use thiserror::Error;
use url::Url;

use crate::class_hash::ClassHash;

#[derive(Debug, serde::Deserialize)]
pub enum VerifyJobStatus {
    Submitted,
    Compiled,
    CompileFailed,
    Fail,
    Success,
}

// TODO: Option blindness?
type JobStatus = Option<VerificationJob>;

impl From<u8> for VerifyJobStatus {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Submitted,
            1 => Self::Compiled,
            2 => Self::CompileFailed,
            3 => Self::Fail,
            4 => Self::Success,
            _ => panic!("Unknown status: {}", value),
        }
    }
}

impl Display for VerifyJobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyJobStatus::Submitted => write!(f, "Submitted"),
            VerifyJobStatus::Compiled => write!(f, "Compiled"),
            VerifyJobStatus::CompileFailed => write!(f, "CompileFailed"),
            VerifyJobStatus::Fail => write!(f, "Fail"),
            VerifyJobStatus::Success => write!(f, "Success"),
        }
    }
}

#[derive(Clone)]
pub struct ApiClient {
    base: Url,
    client: Client,
}

#[derive(Error, Debug)]
pub enum ApiClientError {
    #[error("{0} cannot be base, provide valid URL")]
    CannotBeBase(Url),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/**
 * Currently only GetJobStatus and VerifyClass are public available apis.
 * In the future, the get class api should be moved to using public apis too.
 * TODO: Change get class api to use public apis.
 */
// TODO: Perhaps make a client and make this execute calls
impl ApiClient {
    pub fn new(base: Url) -> Result<Self, ApiClientError> {
        // Test here so that we are sure path_segments_mut succeeds
        if base.cannot_be_a_base() {
            Err(ApiClientError::CannotBeBase(base))
        } else {
            Ok(Self {
                base,
                client: blocking::Client::new(),
            })
        }
    }

    pub fn get_class_url(&self, class_hash: &ClassHash) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("")
            .extend(&["api", "class", class_hash.as_ref()]);
        url
    }

    pub fn get_class(&self, class_hash: &ClassHash) -> Result<bool, ApiClientError> {
        let result = self
            .client
            .get(self.get_class_url(&class_hash))
            // shouldn't `?` be enough?
            .send()
            .map_err(ApiClientError::Reqwest)?;

        match result.status() {
            StatusCode::OK => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            _ => Err(ApiClientError::Other(anyhow::anyhow!(
                "Unexpected status code {} when trying to get class hash with error {}",
                result.status(),
                result.text()?
            ))),
        }
    }

    pub fn verify_class_url(&self, class_hash: ClassHash) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("")
            .extend(&["class-verify", class_hash.as_ref()]);
        url
    }

    pub fn verify_class(
        &self,
        class_hash: ClassHash,
        license: &str,
        name: &str,
        project_metadata: ProjectMetadataInfo,
        files: Vec<FileInfo>,
    ) -> Result<String, ApiClientError> {
        let mut body = multipart::Form::new()
            .percent_encode_noop()
            .text(
                "compiler_version",
                project_metadata.cairo_version.to_string(),
            )
            .text("scarb_version", project_metadata.scarb_version.to_string())
            .text("license", license.to_string())
            .text("name", name.to_string())
            .text("contract_file", project_metadata.contract_file)
            .text("project_dir_path", project_metadata.project_dir_path);

        for file in files.iter() {
            let file_content = fs::read_to_string(file.path.as_path())?;
            body = body.text(format!("files__{}", file.name.clone()), file_content);
        }

        let response = self
            .client
            .post(self.verify_class_url(class_hash))
            .multipart(body)
            // shouldn't `?` be enough?
            .send()
            .map_err(ApiClientError::Reqwest)?;

        match response.status() {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => {
                return Err(ApiClientError::Other(anyhow!("Job not found")));
            }
            StatusCode::BAD_REQUEST => {
                let err_response = response.json::<ApiError>()?;

                return Err(ApiClientError::Other(anyhow!(
                    "Failed to dispatch verification job with status 400: {}",
                    err_response.error
                )));
            }
            unknown_status_code => {
                return Err(ApiClientError::Other(anyhow!(
                    "Failed to dispatch verification job with status {}: {}",
                    unknown_status_code,
                    response.text()?
                )));
            }
        }

        Ok(response.json::<VerificationJobDispatch>()?.job_id)
    }

    pub fn get_job_status_url(&self, job_id: String) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("")
            .extend(&["class-verify", "job", job_id.as_ref()]);
        url
    }

    pub fn get_job_status(&self, job_id: String) -> Result<JobStatus, ApiClientError> {
        let response = self.client.get(self.get_job_status_url(job_id)).send()?;

        match response.status() {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => {
                return Err(ApiClientError::Other(anyhow!("Job not found")));
            }
            unknown_status_code => {
                return Err(ApiClientError::Other(anyhow!(
                    "Unexpected status code: {}, with error message: {}",
                    unknown_status_code,
                    response.text()?
                )));
            }
        }

        let data = response.json::<VerificationJob>()?;
        match VerifyJobStatus::from(data.status) {
            VerifyJobStatus::Success => return Ok(Some(data)),
            VerifyJobStatus::Fail => {
                return Err(ApiClientError::Other(anyhow!(
                    "Failed to verify: {:?}",
                    data.status_description
                        .unwrap_or("unknown failure".to_owned())
                )))
            }
            VerifyJobStatus::CompileFailed => {
                return Err(ApiClientError::Other(anyhow!(
                    "Compilation failed: {:?}",
                    data.status_description
                        .unwrap_or("unknown failure".to_owned())
                )))
            }
            _ => Ok(None),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct ApiError {
    error: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct VerificationJobDispatch {
    job_id: String,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct VerificationJob {
    job_id: String,
    status: u8,
    status_description: Option<String>,
    class_hash: String,
    created_timestamp: Option<f64>,
    updated_timestamp: Option<f64>,
    address: Option<String>,
    contract_file: Option<String>,
    name: Option<String>,
    version: Option<String>,
    license: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct FileInfo {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectMetadataInfo {
    pub cairo_version: semver::Version,
    pub scarb_version: semver::Version,
    pub project_dir_path: String,
    pub contract_file: String,
}

pub enum Status {
    InProgress,
    Finished(ApiClientError),
}

fn is_is_progress(status: &Status) -> bool {
    match status {
        Status::InProgress => true,
        Status::Finished(_) => false,
    }
}

pub fn poll_verification_status(
    api: ApiClient,
    job_id: &str,
) -> Result<VerificationJob, ApiClientError> {
    let fetch = || -> Result<VerificationJob, Status> {
        let result: Option<VerificationJob> = api
            .get_job_status(job_id.to_owned())
            .map_err(Status::Finished)?;

        result.ok_or(Status::InProgress)
    };

    // So verbose because it has problems with inference
    fetch
        .retry(
            ExponentialBuilder::default()
                .with_max_times(0)
                .with_min_delay(Duration::from_secs(2))
                .with_max_delay(Duration::from_secs(300)) // 5 mins
                .with_max_times(20),
        )
        .when(is_is_progress)
        .notify(|_, dur: Duration| {
            println!("Job: {} didn't finish, retrying in {:?}", job_id, dur);
        })
        .call()
        .map_err(|err| match err {
            Status::InProgress => {
                ApiClientError::Other(anyhow!("Verification job is still in progress"))
            }
            Status::Finished(e) => e,
        })
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::env;

//     #[test]
//     fn test_getting_default_voyager_endpoints() {
//         let selected_network = Network::Sepolia;
//         let actual_network_api = get_network_api(selected_network);

//         // Assert that the internal api is correct
//         assert_eq!(actual_network_api.0, "https://sepolia.voyager.online");
//         // Assert that the public api is correct``
//         assert_eq!(
//             actual_network_api.1,
//             "https://sepolia-api.voyager.online/beta"
//         );
//     }

//     #[test]
//     fn test_getting_custom_endpoints() {
//         let my_internal_api_url = "https://my-instance-internal-api.com";
//         let my_public_api_url = "https://my-instance-public-api.com";
//         // set env vars for this testing case
//         env::set_var("CUSTOM_INTERNAL_API_ENDPOINT_URL", my_internal_api_url);
//         env::set_var("CUSTOM_PUBLIC_API_ENDPOINT_URL", my_public_api_url);

//         let selected_network = Network::Custom;
//         let actual_network_api = get_network_api(selected_network);

//         // Assert that the internal api is correct
//         assert_eq!(actual_network_api.0, my_internal_api_url);
//         // Assert that the public api is correct``
//         assert_eq!(actual_network_api.1, my_public_api_url);
//     }
// }
