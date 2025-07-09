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
