use super::types::VerifyJobStatus;
use crate::project::ProjectType;
use semver;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Error {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct VerificationJobDispatch {
    pub job_id: String,
}

#[derive(Debug, Deserialize)]
pub struct VerificationJob {
    pub job_id: String,
    pub status: VerifyJobStatus,
    pub status_description: Option<String>,
    pub message: Option<String>,
    pub error_category: Option<String>,
    pub class_hash: Option<String>,
    pub created_timestamp: Option<f64>,
    pub updated_timestamp: Option<f64>,
    pub address: Option<String>,
    pub contract_file: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub license: Option<String>,
    pub dojo_version: Option<String>,
    pub build_tool: Option<String>,
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

    pub fn dojo_version(&self) -> Option<&str> {
        self.dojo_version.as_deref()
    }

    pub fn build_tool(&self) -> Option<&str> {
        self.build_tool.as_deref()
    }

    pub const fn is_completed(&self) -> bool {
        matches!(
            self.status,
            VerifyJobStatus::Success | VerifyJobStatus::Fail | VerifyJobStatus::CompileFailed
        )
    }

    pub const fn has_failed(&self) -> bool {
        matches!(
            self.status,
            VerifyJobStatus::Fail | VerifyJobStatus::CompileFailed
        )
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
    pub build_tool: String,           // "scarb" or "sozo"
    pub dojo_version: Option<String>, // Dojo version for Dojo projects
}

impl ProjectMetadataInfo {
    pub fn new(
        cairo_version: semver::Version,
        scarb_version: semver::Version,
        project_dir_path: String,
        contract_file: String,
        package_name: String,
        project_type: ProjectType,
        dojo_version: Option<String>,
    ) -> Self {
        Self {
            cairo_version,
            scarb_version,
            project_dir_path,
            contract_file,
            package_name,
            build_tool: if project_type == ProjectType::Dojo {
                log::debug!("Setting build_tool to 'sozo' for Dojo project");
                "sozo".to_string()
            } else {
                log::debug!("Setting build_tool to 'scarb' for non-Dojo project: {project_type:?}");
                "scarb".to_string()
            },
            dojo_version,
        }
    }
}
