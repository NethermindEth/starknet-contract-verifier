use thiserror::Error;
use url::Url;

use crate::errors::RequestFailure;

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
                let mut suggestions = vec![
                    "Check that all dependencies are properly declared in Scarb.toml",
                    "Verify that the contract syntax is correct",
                    "Ensure all imports are valid and accessible",
                    "Check for typos in function names and variable declarations",
                ];

                if msg.contains("not found") {
                    suggestions.push("Verify that all modules and dependencies are available");
                }

                if msg.contains("syntax") {
                    suggestions.push("Review the Cairo syntax documentation");
                }

                suggestions
            }
            Self::VerificationFailure(msg) => {
                let mut suggestions = vec![
                    "Ensure the compiled class hash matches the declared class hash",
                    "Verify that the source code corresponds to the deployed contract",
                    "Check that all dependencies are at the correct versions",
                    "Confirm that the contract was compiled with the same Cairo version",
                ];

                if msg.contains("hash") {
                    suggestions.push("Double-check the class hash value");
                }

                if msg.contains("version") {
                    suggestions.push("Verify Cairo compiler version compatibility");
                }

                suggestions
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum ApiClientError {
    #[error("[E006] Invalid base URL: {0}\n\nSuggestions:\n  • Provide a valid HTTP or HTTPS URL\n  • Example: https://api.example.com\n  • Ensure the URL includes the protocol (http:// or https://)")]
    CannotBeBase(Url),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("[E007] Verification job is still in progress\n\nSuggestions:\n  • Wait a moment before checking again\n  • Use --wait to automatically wait for completion\n  • Check the job status periodically")]
    InProgress,

    #[error(transparent)]
    Failure(#[from] RequestFailure),

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
