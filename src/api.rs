use std::{fmt::Display, fs, path::PathBuf, time::Duration};

use backon::{BlockingRetryable, ExponentialBuilder};
use reqwest::{
    blocking::{self, multipart, Client},
    StatusCode,
};
use semver;
use spdx::LicenseId;
use thiserror::Error;
use url::Url;

use crate::{
    class_hash::ClassHash,
    errors::{self, RequestFailure},
};

#[derive(Debug, serde::Deserialize)]
pub enum VerifyJobStatus {
    Submitted,
    Compiled,
    CompileFailed,
    Fail,
    Success,
}

#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("Compilation failed: {0}")]
    CompilationFailure(String),

    #[error("Compilation failed: {0}")]
    VerificationFailure(String),
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

    #[error("Verification job is still in progress")]
    InProgress,

    #[error(transparent)]
    Failure(#[from] errors::RequestFailure),

    #[error("Job {0} not found")]
    JobNotFound(String),

    #[error(transparent)]
    Verify(#[from] VerificationError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
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
        let url = self.get_class_url(&class_hash);
        let result = self
            .client
            .get(url.clone())
            .send()
            .map_err(ApiClientError::from)?;

        match result.status() {
            StatusCode::OK => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            _ => Err(ApiClientError::from(RequestFailure::new(
                url,
                result.status(),
                result.text()?,
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
        license: Option<LicenseId>,
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
            // .text("license", license.to_string())
            .text("name", name.to_string())
            .text("contract_file", project_metadata.contract_file)
            .text("project_dir_path", project_metadata.project_dir_path);

        match license {
            Some(id) => body = body.text("license", id.name),
            _ => {}
        }

        for file in files.iter() {
            let file_content = fs::read_to_string(file.path.as_path())?;
            body = body.text(format!("files__{}", file.name.clone()), file_content);
        }

        let url = self.verify_class_url(class_hash);

        let response = self
            .client
            .post(url.clone())
            .multipart(body)
            // shouldn't `?` be enough?
            .send()
            .map_err(ApiClientError::Reqwest)?;

        match response.status() {
            StatusCode::OK => (),
            StatusCode::BAD_REQUEST => {
                return Err(ApiClientError::from(RequestFailure::new(
                    url,
                    StatusCode::BAD_REQUEST,
                    response.json::<ApiError>()?.error,
                )));
            }
            status_code => {
                return Err(ApiClientError::from(RequestFailure::new(
                    url,
                    status_code,
                    response.text()?,
                )));
            }
        }

        Ok(response.json::<VerificationJobDispatch>()?.job_id)
    }

    pub fn get_job_status_url(&self, job_id: impl AsRef<str>) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("")
            .extend(&["class-verify", "job", job_id.as_ref()]);
        url
    }

    pub fn get_job_status(
        &self,
        job_id: impl Into<String> + Clone,
    ) -> Result<JobStatus, ApiClientError> {
        let url = self.get_job_status_url(job_id.clone().into());
        let response = self.client.get(url.clone()).send()?;

        match response.status() {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => return Err(ApiClientError::JobNotFound(job_id.into())),
            status_code => {
                return Err(ApiClientError::from(RequestFailure::new(
                    url,
                    status_code,
                    response.text()?,
                )));
            }
        }

        let data = response.json::<VerificationJob>()?;
        match VerifyJobStatus::from(data.status) {
            VerifyJobStatus::Success => return Ok(Some(data)),
            VerifyJobStatus::Fail => {
                return Err(ApiClientError::from(
                    VerificationError::VerificationFailure(
                        data.status_description
                            .unwrap_or("unknown failure".to_owned()),
                    ),
                ))
            }
            VerifyJobStatus::CompileFailed => {
                return Err(ApiClientError::from(VerificationError::CompilationFailure(
                    data.status_description
                        .unwrap_or("unknown failure".to_owned()),
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
            Status::InProgress => ApiClientError::InProgress,
            Status::Finished(e) => e,
        })
}
