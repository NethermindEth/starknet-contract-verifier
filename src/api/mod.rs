//! # API Client for Starknet Contract Verification
//!
//! This module provides a comprehensive API client for interacting with Starknet
//! contract verification services. It handles HTTP requests, response parsing,
//! and provides type-safe interfaces for all verification operations.
//!
//! ## Features
//!
//! - **HTTP Client**: Built on `reqwest` with automatic retries and error handling
//! - **Type Safety**: Strong typing for all requests and responses
//! - **Polling**: Automatic polling for long-running verification jobs
//! - **Error Handling**: Comprehensive error types with actionable suggestions
//! - **Multipart Uploads**: Support for uploading contract source files
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use verifier::api::{ApiClient, FileInfo, ProjectMetadataInfo};
//! use verifier::class_hash::ClassHash;
//! use url::Url;
//! use std::path::PathBuf;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create API client
//! let client = ApiClient::new(Url::parse("https://api.voyager.online/beta")?)?;
//!
//! // Check if a class exists
//! let class_hash = ClassHash::new("0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18")?;
//! let exists = client.get_class(&class_hash)?;
//!
//! // Get verification job status
//! let job_status = client.get_job_status("job-id")?;
//! # Ok(())
//! # }
//! ```

// Re-export the API module components
pub use self::{
    client::ApiClient,
    errors::{ApiClientError, VerificationError},
    models::{FileInfo, ProjectMetadataInfo, VerificationJob, VerificationJobDispatch},
    polling::poll_verification_status,
    types::{JobStatus, Status, VerifyJobStatus},
};

// Module declarations
mod client;
mod errors;
mod models;
mod polling;
mod types;
