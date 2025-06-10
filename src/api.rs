use std::{fmt::Display, fs, path::PathBuf, time::Duration};

use backon::{BlockingRetryable, ExponentialBuilder};
use reqwest::{
    blocking::{self, multipart, Client},
    StatusCode,
};
use semver;
use serde_repr::{Deserialize_repr, Serialize_repr};
use thiserror::Error;
use url::Url;

use crate::{
    class_hash::ClassHash,
    errors::{self, RequestFailure},
};

#[derive(Clone, Debug, Deserialize_repr, Eq, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum VerifyJobStatus {
    Submitted = 0,
    Compiled = 1,
    CompileFailed = 2,
    Fail = 3,
    Success = 4,
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

impl Display for VerifyJobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Submitted => write!(f, "Submitted"),
            Self::Compiled => write!(f, "Compiled"),
            Self::CompileFailed => write!(f, "CompileFailed"),
            Self::Fail => write!(f, "Fail"),
            Self::Success => write!(f, "Success"),
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

    #[error("URL cannot be a base: {0}")]
    UrlCannotBeBase(#[from] url::ParseError),
}

/**
 * Currently only `GetJobStatus` and `VerifyClass` are public available apis.
 * In the future, the get class api should be moved to using public apis too.
 * TODO: Change get class api to use public apis.
 */
impl ApiClient {
    /// # Errors
    ///
    /// Fails if provided `Url` cannot be a base. We rely on that
    /// invariant in other methods.
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

    /// # Errors
    ///
    /// Will return `Err` if the URL cannot be a base.
    pub fn get_class_url(&self, class_hash: &ClassHash) -> Result<Url, ApiClientError> {
        let mut url = self.base.clone();
        let url_clone = url.clone();
        url.path_segments_mut()
            .map_err(|_| ApiClientError::CannotBeBase(url_clone))?
            .extend(&["classes", class_hash.as_ref()]);
        Ok(url)
    }

    /// # Errors
    ///
    /// Returns `Err` if the required `class_hash` is not found or on
    /// network failure.
    pub fn get_class(&self, class_hash: &ClassHash) -> Result<bool, ApiClientError> {
        let url = self.get_class_url(class_hash)?;
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

    /// # Errors
    ///
    /// Will return `Err` if the URL cannot be a base.
    pub fn verify_class_url(&self, class_hash: &ClassHash) -> Result<Url, ApiClientError> {
        let mut url = self.base.clone();
        let url_clone = url.clone();
        url.path_segments_mut()
            .map_err(|_| ApiClientError::CannotBeBase(url_clone))?
            .extend(&["class-verify", class_hash.as_ref()]);
        Ok(url)
    }

    /// # Errors
    ///
    /// Will return `Err` on network request failure or if can't
    /// gather file contents for submission.
    pub fn verify_class(
        &self,
        class_hash: &ClassHash,
        license: Option<String>,
        name: &str,
        project_metadata: ProjectMetadataInfo,
        files: &[FileInfo],
    ) -> Result<String, ApiClientError> {
        let mut body = multipart::Form::new()
            .percent_encode_noop()
            .text(
                "compiler_version",
                project_metadata.cairo_version.to_string(),
            )
            .text("scarb_version", project_metadata.scarb_version.to_string())
            .text("package_name", project_metadata.package_name)
            .text("name", name.to_string())
            .text("contract_file", project_metadata.contract_file)
            .text("project_dir_path", project_metadata.project_dir_path);

        // Add license using raw SPDX identifier
        let license_value = if let Some(lic) = license {
            if lic == "MIT" {
                "MIT".to_string() // Ensure MIT is formatted correctly
            } else {
                lic
            }
        } else {
            "NONE".to_string()
        };

        body = body.text("license", license_value);

        // Send each file as a separate field with files[] prefix
        for file in files {
            let file_content = fs::read_to_string(file.path.as_path())?;
            body = body.text(format!("files[{}]", file.name), file_content);
        }

        let url = self.verify_class_url(class_hash)?;

        let response = self
            .client
            .post(url.clone())
            .multipart(body)
            .send()
            .map_err(ApiClientError::Reqwest)?;

        match response.status() {
            StatusCode::OK => (),
            StatusCode::BAD_REQUEST => {
                return Err(ApiClientError::from(RequestFailure::new(
                    url,
                    StatusCode::BAD_REQUEST,
                    response.json::<Error>()?.error,
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

    /// # Errors
    ///
    /// Will return `Err` if the URL cannot be a base.
    pub fn get_job_status_url(&self, job_id: impl AsRef<str>) -> Result<Url, ApiClientError> {
        let mut url = self.base.clone();
        let url_clone = url.clone();
        url.path_segments_mut()
            .map_err(|_| ApiClientError::CannotBeBase(url_clone))?
            .extend(&["class-verify", "job", job_id.as_ref()]);
        Ok(url)
    }

    /// # Errors
    ///
    /// Will return `Err` on network error or if the verification has
    /// failed.
    pub fn get_job_status(
        &self,
        job_id: impl Into<String> + Clone,
    ) -> Result<JobStatus, ApiClientError> {
        let url = self.get_job_status_url(job_id.clone().into())?;
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
        match data.status {
            VerifyJobStatus::Success => Ok(Some(data)),
            VerifyJobStatus::Fail => Err(ApiClientError::from(
                VerificationError::VerificationFailure(
                    data.status_description
                        .unwrap_or_else(|| "unknown failure".to_owned()),
                ),
            )),
            VerifyJobStatus::CompileFailed => {
                Err(ApiClientError::from(VerificationError::CompilationFailure(
                    data.status_description
                        .unwrap_or_else(|| "unknown failure".to_owned()),
                )))
            }
            _ => Ok(None),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct Error {
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
    status: VerifyJobStatus,
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
    pub package_name: String,
}

pub enum Status {
    InProgress,
    Finished(ApiClientError),
}

const fn is_is_progress(status: &Status) -> bool {
    match status {
        Status::InProgress => true,
        Status::Finished(_) => false,
    }
}

/// # Errors
///
/// Will return `Err` on network error or if the verification has
/// failed.
pub fn poll_verification_status(
    api: &ApiClient,
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
            println!("Job: {job_id} didn't finish, retrying in {dur:?}");
        })
        .call()
        .map_err(|err| match err {
            Status::InProgress => ApiClientError::InProgress,
            Status::Finished(e) => e,
        })
}
