use std::fmt::Display;
use std::path::PathBuf;
use std::{str::FromStr, thread::sleep};
use strum_macros::EnumIter;

use anyhow::{anyhow, Error, Ok, Result};
use dyn_compiler::dyn_compiler::{SupportedCairoVersions, SupportedScarbVersions};
use reqwest::{
    blocking::{get, multipart, Client},
    StatusCode,
};

#[derive(Debug, Clone, EnumIter, Copy)]
pub enum LicenseType {
    NoLicense,
    Unlicense,
    MIT,
    GPLv2,
    GPLv3,
    LGPLv2_1,
    LGPLv3,
    BSD2Clause,
    BSD3Clause,
    MPL2,
    OSL3,
    Apache2,
    AGPLv3,
    BSL1_1,
}

impl LicenseType {
    pub fn to_long_string(self) -> String {
        let string_repr = match self {
            Self::NoLicense => "No License (None)",
            Self::Unlicense => "The Unlicense (Unlicense)",
            Self::MIT => "MIT License (MIT)",
            Self::GPLv2 => "GNU General Public License v2.0 (GNU GPLv2)",
            Self::GPLv3 => "GNU General Public License v3.0 (GNU GPLv3)",
            Self::LGPLv2_1 => "GNU Lesser General Public License v2.1 (GNU LGPLv2.1)",
            Self::LGPLv3 => "GNU Lesser General Public License v3.0 (GNU LGPLv3)",
            Self::BSD2Clause => "BSD 2-clause \"Simplified\" license (BSD-2-Clause)",
            Self::BSD3Clause => "BSD 3-clause \"New\" Or Revisited license (BSD-3-Clause)",
            Self::MPL2 => "Mozilla Public License 2.0 (MPL-2.0)",
            Self::OSL3 => "Open Software License 3.0 (OSL-3.0)",
            Self::Apache2 => "Apache 2.0 (Apache-2.0)",
            Self::AGPLv3 => "GNU Affero General Public License (GNU AGPLv3)",
            Self::BSL1_1 => "Business Source License (BSL 1.1)",
        };
        string_repr.to_owned()
    }
}

impl ToString for LicenseType {
    fn to_string(&self) -> String {
        let string_repr = match *self {
            Self::NoLicense => "NoLicense",
            Self::Unlicense => "Unlicense",
            Self::MIT => "MIT",
            Self::GPLv2 => "GPLv2",
            Self::GPLv3 => "GPLv3",
            Self::LGPLv2_1 => "LGPLv2_1",
            Self::LGPLv3 => "LGPLv3",
            Self::BSD2Clause => "BSD2Clause",
            Self::BSD3Clause => "BSD3Clause",
            Self::MPL2 => "MPL2",
            Self::OSL3 => "OSL3",
            Self::Apache2 => "Apache2",
            Self::AGPLv3 => "AGPLv3",
            Self::BSL1_1 => "BSL1_1",
        };
        string_repr.to_owned()
    }
}

#[derive(Debug, Clone)]
pub enum Network {
    Mainnet,
    Sepolia,
    Local,
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Sepolia => write!(f, "sepolia"),
            Network::Local => write!(f, "local"),
        }
    }
}

impl FromStr for Network {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "sepolia" => Ok(Network::Sepolia),
            "local" => Ok(Network::Local),
            _ => Err(anyhow!("Unknown network: {}", s)),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub enum VerifyJobStatus {
    Submitted,
    Compiled,
    CompileFailed,
    Fail,
    Success,
}

impl VerifyJobStatus {
    fn from_u8(status: u8) -> Self {
        match status {
            0 => Self::Submitted,
            1 => Self::Compiled,
            2 => Self::CompileFailed,
            3 => Self::Fail,
            4 => Self::Success,
            _ => panic!("Unknown status: {}", status),
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

/**
 * Currently only GetJobStatus and VerifyClass are public available apis.
 * In the future, the get class api should be moved to using public apis too.
 */
pub enum ApiEndpoints {
    GetClass,
    GetJobStatus,
    VerifyClass,
}

impl ApiEndpoints {
    fn as_str(&self) -> String {
        match self {
            ApiEndpoints::GetClass => "/api/class/{class_hash}".to_owned(),
            ApiEndpoints::GetJobStatus => "/class-verify/job/{job_id}".to_owned(),
            ApiEndpoints::VerifyClass => "/class-verify/{class_hash}".to_owned(),
        }
    }

    fn to_api_path(&self, param: String) -> String {
        match self {
            ApiEndpoints::GetClass => self.as_str().replace("{class_hash}", param.as_str()),
            ApiEndpoints::GetJobStatus => self.as_str().replace("{job_id}", param.as_str()),
            ApiEndpoints::VerifyClass => self.as_str().replace("{class_hash}", param.as_str()),
        }
    }
}

pub fn get_network_api(network: Network) -> (String, String) {
    let url = match network {
        Network::Mainnet => "https://voyager.online",
        Network::Sepolia => "https://sepolia.voyager.online",
        Network::Local => "http://localhost:8899",
    };

    let public_url = match network {
        Network::Mainnet => "https://api.voyager.online/beta",
        Network::Sepolia => "https://sepolia-api.voyager.online/beta",
        Network::Local => "http://localhost:30380",
    };

    (url.into(), public_url.into())
}

#[derive(Debug, serde::Deserialize)]
pub struct ApiError {
    error: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct VerificationJobDispatch {
    job_id: String,
}

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

#[derive(Debug)]
pub struct FileInfo {
    pub name: String,
    pub path: PathBuf,
}

pub fn does_class_exist(network: Network, class_hash: &str) -> Result<bool> {
    let (url, _) = get_network_api(network);
    let path_with_params = ApiEndpoints::GetClass.to_api_path(class_hash.to_owned());
    let result = get(url + path_with_params.as_str())?;
    match result.status() {
        StatusCode::OK => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        _ => Err(anyhow::anyhow!(
            "Unexpected status code {} when trying to get class hash with error {}",
            result.status(),
            result.text()?
        )),
    }
}

#[derive(Debug, Clone)]
pub struct ProjectMetadataInfo {
    pub cairo_version: SupportedCairoVersions,
    pub scarb_version: SupportedScarbVersions,
    pub project_dir_path: String,
    pub contract_file: String,
}

pub fn dispatch_class_verification_job(
    api_key: &str,
    network: Network,
    address: &str,
    license: &str,
    is_account: bool,
    name: &str,
    project_metadata: ProjectMetadataInfo,
    files: Vec<FileInfo>,
) -> Result<String> {
    // Construct form body
    let mut body = multipart::Form::new()
        .percent_encode_noop()
        .text(
            "compiler_version",
            project_metadata.cairo_version.to_string(),
        )
        .text("scarb_version", project_metadata.scarb_version.to_string())
        .text("license", license.to_string())
        .text("account_contract", is_account.to_string())
        .text("name", name.to_string())
        .text("contract_file", project_metadata.contract_file)
        .text("project_dir_path", project_metadata.project_dir_path);

    for file in files.iter() {
        let file_content = fs::read_to_string(file.path.as_path())?;
        body = body.text(format!("files__{}", file.name.clone()), file_content);
    }

    let (_, public_url) = get_network_api(network);
    let client = Client::new();

    let path_with_param = ApiEndpoints::VerifyClass.to_api_path(address.to_owned());

    let response = client
        .post(public_url + path_with_param.as_str())
        // .header("x-api-key", api_key)
        .multipart(body)
        .send()?;

    match response.status() {
        StatusCode::OK => (),
        StatusCode::NOT_FOUND => {
            return Err(anyhow!("Job not found"));
        }
        StatusCode::BAD_REQUEST => {
            let err_response = response.json::<ApiError>()?;

            return Err(anyhow!(
                "Failed to dispatch verification job with status 400: {}",
                err_response.error
            ));
        }
        unknown_status_code => {
            return Err(anyhow!(
                "Failed to dispatch verification job with status {}: {}",
                unknown_status_code,
                response.text()?
            ));
        }
    }

    let data = response.json::<VerificationJobDispatch>().unwrap();

    Ok(data.job_id)
}

pub fn poll_verification_status(
    api_key: &str,
    network: Network,
    job_id: &str,
    max_retries: u32,
) -> Result<VerificationJob> {
    // Get network api url
    let (_, public_url) = get_network_api(network);

    // Blocking loop that polls every 2 seconds
    static RETRY_INTERVAL: u64 = 2000; // Ms
    let mut retries: u32 = 0;
    let client = Client::new();

    let path_with_param = ApiEndpoints::GetJobStatus.to_api_path(job_id.to_owned());

    // Retry every 2000ms until we hit maxRetries
    while retries < max_retries {
        let result = client
            .get(public_url.clone() + path_with_param.as_str())
            // .header("x-api-key", api_key)
            .send()?;
        match result.status() {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => {
                return Err(anyhow!("Job not found"));
            }
            unknown_status_code => {
                return Err(anyhow!(
                    "Unexpected status code: {}, with error message: {}",
                    unknown_status_code,
                    result.text()?
                ));
            }
        }

        // Go through the possible status
        let data = result.json::<VerificationJob>()?;
        match VerifyJobStatus::from_u8(data.status) {
            VerifyJobStatus::Success => return Ok(data),
            VerifyJobStatus::Fail => {
                return Err(anyhow!(
                    "Failed to verify: {:?}",
                    data.status_description
                        .unwrap_or("unknown failure".to_owned())
                ))
            }
            VerifyJobStatus::CompileFailed => {
                return Err(anyhow!(
                    "Compilation failed: {:?}",
                    data.status_description
                        .unwrap_or("unknown failure".to_owned())
                ))
            }
            _ => (),
        }
        retries += 1;
        sleep(std::time::Duration::from_millis(RETRY_INTERVAL));
    }

    // If we hit maxRetries, throw an timeout error
    Err(anyhow!(
        "Timeout: Verification job took too long to complete"
    ))
}
