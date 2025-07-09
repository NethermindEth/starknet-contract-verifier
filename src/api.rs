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
    Processing = 5,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("[E004] Compilation failed: {0}")]
    CompilationFailure(String),

    #[error("[E005] Verification failed: {0}")]
    VerificationFailure(String),
}

impl VerificationError {
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::CompilationFailure(_) => "E004",
            Self::VerificationFailure(_) => "E005",
        }
    }

    pub fn suggestions(&self) -> Vec<&'static str> {
        match self {
            Self::CompilationFailure(msg) => {
                if msg.contains("payload too large") || msg.contains("Payload too large") {
                    vec![
                        "The project files exceed the maximum allowed size of 10MB",
                        "Try removing unnecessary files or large assets",
                        "Use --test-files flag only when necessary",
                        "Remove --lock-file flag if not needed",
                        "Check for large binary files or dependencies",
                    ]
                } else {
                    vec![
                        "Check your Cairo syntax for errors",
                        "Verify all dependencies are properly declared",
                        "Ensure your Cairo version is compatible",
                        "Run 'scarb build' locally to debug compilation issues",
                    ]
                }
            },
            Self::VerificationFailure(msg) => {
                if msg.contains("payload too large") || msg.contains("Payload too large") {
                    vec![
                        "The project files exceed the maximum allowed size of 10MB",
                        "Try removing unnecessary files or large assets",
                        "Use --test-files flag only when necessary",
                        "Remove --lock-file flag if not needed",
                        "Check for large binary files or dependencies",
                    ]
                } else {
                    vec![
                        "Check that your contract was compiled correctly",
                        "Verify the class hash matches your contract",
                        "Ensure you're using the correct network",
                        "Check that all contract dependencies are available",
                    ]
                }
            },
        }
    }
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
            Self::Processing => write!(f, "Processing"),
            Self::Unknown => write!(f, "Unknown"),
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
    #[error("[E006] Invalid base URL: {0}\n\nSuggestions:\n  • Provide a valid HTTP or HTTPS URL\n  • Example: https://api.example.com\n  • Ensure the URL includes the protocol (http:// or https://)")]
    CannotBeBase(Url),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("[E007] Verification job is still in progress\n\nSuggestions:\n  • Wait a moment before checking again\n  • Use --wait to automatically wait for completion\n  • Check the job status periodically")]
    InProgress,

    #[error(transparent)]
    Failure(#[from] errors::RequestFailure),

    #[error("[E008] Job '{0}' not found\n\nSuggestions:\n  • Check that the job ID is correct\n  • Verify the job was submitted successfully\n  • The job may have expired from the server\n  • Try submitting a new verification request")]
    JobNotFound(String),

    #[error(transparent)]
    Verify(#[from] VerificationError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("[E009] Invalid URL format: {0}\n\nSuggestions:\n  • Check the URL format is correct\n  • Ensure proper encoding of special characters\n  • Use absolute URLs with protocol (http:// or https://)")]
    UrlCannotBeBase(#[from] url::ParseError),
}

impl ApiClientError {
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::CannotBeBase(_) => "E006",
            Self::Reqwest(_) | Self::IoError(_) => "E999", // Network errors get generic code
            Self::InProgress => "E007",
            Self::Failure(f) => f.error_code().as_str(),
            Self::JobNotFound(_) => "E008",
            Self::Verify(v) => v.error_code(),
            Self::UrlCannotBeBase(_) => "E009",
        }
    }
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
            StatusCode::PAYLOAD_TOO_LARGE => {
                return Err(ApiClientError::from(RequestFailure::new(
                    url,
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Request payload too large. Maximum allowed size is 10MB.".to_string(),
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

        let response_text = response.text()?;
        log::debug!("Raw API Response: {}", response_text);
        
        let data: VerificationJob = serde_json::from_str(&response_text)
            .map_err(|e| {
                log::error!("Failed to parse JSON response: {}", e);
                log::error!("Response text: {}", response_text);
                ApiClientError::from(RequestFailure::new(
                    url.clone(),
                    StatusCode::OK,
                    format!("Failed to parse JSON response: {}", e)
                ))
            })?;
        
        // Debug logging to see the actual response
        log::debug!("Parsed API Response: job_id={}, status={:?}, status_description={:?}, message={:?}, error_category={:?}", 
                   data.job_id, data.status, data.status_description, data.message, data.error_category);
        
        match data.status {
            VerifyJobStatus::Success => Ok(Some(data)),
            VerifyJobStatus::Fail => {
                let error_message = data.message
                    .or_else(|| data.status_description.clone())
                    .unwrap_or_else(|| "unknown failure".to_owned());
                
                // Parse specific error types from the server response
                let parsed_error = if error_message.contains("Payload too large") || error_message.contains("payload too large") {
                    "Request payload too large. The project files exceed the maximum allowed size of 10MB. Try reducing file sizes or removing unnecessary files."
                } else {
                    &error_message
                };
                
                Err(ApiClientError::from(
                    VerificationError::VerificationFailure(parsed_error.to_owned()),
                ))
            },
            VerifyJobStatus::CompileFailed => {
                let error_message = data.message
                    .or_else(|| data.status_description.clone())
                    .unwrap_or_else(|| "unknown failure".to_owned());
                
                // Parse specific error types from the server response
                let parsed_error = if error_message.contains("Payload too large") || error_message.contains("payload too large") {
                    "Request payload too large. The project files exceed the maximum allowed size of 10MB. Try reducing file sizes or removing unnecessary files."
                } else if error_message.contains("Couldn't connect to cairo compilation service") {
                    "Cairo compilation service is currently unavailable. Please try again later."
                } else {
                    &error_message
                };
                
                Err(ApiClientError::from(VerificationError::CompilationFailure(
                    parsed_error.to_owned(),
                )))
            }
            VerifyJobStatus::Submitted
            | VerifyJobStatus::Compiled
            | VerifyJobStatus::Processing
            | VerifyJobStatus::Unknown => Ok(None),
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

#[derive(Debug, serde::Deserialize)]
pub struct VerificationJob {
    job_id: String,
    status: VerifyJobStatus,
    status_description: Option<String>,
    message: Option<String>,
    error_category: Option<String>,
    class_hash: Option<String>,
    created_timestamp: Option<f64>,
    updated_timestamp: Option<f64>,
    address: Option<String>,
    contract_file: Option<String>,
    name: Option<String>,
    version: Option<String>,
    license: Option<String>,
}

impl VerificationJob {
    pub const fn status(&self) -> &VerifyJobStatus {
        &self.status
    }

    pub fn class_hash(&self) -> &str {
        self.class_hash.as_deref().unwrap_or("unknown")
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn contract_file(&self) -> Option<&str> {
        self.contract_file.as_deref()
    }

    pub fn status_description(&self) -> Option<&str> {
        self.status_description.as_deref()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn error_category(&self) -> Option<&str> {
        self.error_category.as_deref()
    }

    pub const fn created_timestamp(&self) -> Option<f64> {
        self.created_timestamp
    }

    pub const fn updated_timestamp(&self) -> Option<f64> {
        self.updated_timestamp
    }

    pub fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn license(&self) -> Option<&str> {
        self.license.as_deref()
    }
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
