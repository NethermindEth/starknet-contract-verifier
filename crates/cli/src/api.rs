use std::fmt::Display;
use std::fs::File;
use std::{str::FromStr, thread::sleep};

use anyhow::{anyhow, Error, Ok, Result};
use dyn_compiler::dyn_compiler::{SupportedCairoVersions, SupportedScarbVersions};
use reqwest::{
    blocking::{get, multipart, Client},
    StatusCode,
};

#[derive(Debug, Clone)]
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
    pub fn to_long_string(&self) -> String {
        let string_repr = match *self {
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
    Goerli,
    Goerli2,
    Integration,
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Goerli => write!(f, "goerli"),
            Network::Goerli2 => write!(f, "goerli2"),
            Network::Integration => write!(f, "integration"),
        }
    }
}

impl FromStr for Network {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "goerli" => Ok(Network::Goerli),
            "goerli2" => Ok(Network::Goerli2),
            "integration" => Ok(Network::Integration),
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

pub fn get_network_api(network: Network) -> String {
    let url = match network {
        Network::Mainnet => "https://voyager.online",
        Network::Goerli => "https://goerli.voyager.online",
        Network::Goerli2 => "https://goerli-2.voyager.online",
        Network::Integration => "https://integration.voyager.online",
    };

    url.into()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationJobDispatch {
    job_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationJob {
    job_id: String,
    status: VerifyJobStatus,
    class_hash: String,
    created_timestamp: String,
    updated_timestamp: String,
    address: String,
    contract_name: String,
    name: String,
    version: String,
    license: String,
}

#[derive(Debug)]
pub struct FileInfo {
    pub path: String,
    pub content: String,
}

pub fn does_contract_exist(network: Network, address: &str) -> Result<bool> {
    let url = get_network_api(network);
    let result = get(url + "/api/contract/" + address)?;
    match result.status() {
        StatusCode::OK => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        _ => Err(anyhow::anyhow!(
            "Unexpected status code: {}",
            result.status()
        )),
    }
}

pub fn does_class_exist(network: Network, class_hash: &str) -> Result<bool> {
    let url = get_network_api(network);
    let result = get(url + "/api/class/" + class_hash)?;
    match result.status() {
        StatusCode::OK => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        _ => Err(anyhow::anyhow!(
            "Unexpected status code: {}",
            result.status()
        )),
    }
}

pub fn dispatch_class_verification_job(
    network: Network,
    address: &str,
    cairo_version: SupportedCairoVersions,
    scarb_version: SupportedScarbVersions,
    license: &str,
    is_account: bool,
    name: &str,
    files: Vec<FileInfo>,
) -> Result<String> {
    // Construct form body
    let body = multipart::Form::new()
        .text("compiler-version", cairo_version.to_string())
        .text("scarb-version", scarb_version.to_string())
        .text("license", license.to_string())
        .text("account-contract", is_account.to_string())
        .text("name", name.to_string())
        .text("contract-name", address.to_string());

    let body = files.iter().fold(body, |body, file| {
        let part = multipart::Part::text(file.content.clone()).file_name(file.path.clone());
        body.part("file", part)
    });

    let url = get_network_api(network);
    let client = Client::new();

    let response = client
        .post(url + "/api/class/" + address + "/code")
        .multipart(body)
        .send()?;

    match response.status() {
        StatusCode::OK => (),
        StatusCode::NOT_FOUND => {
            return Err(anyhow!("Job not found"));
        }
        _ => {
            return Err(anyhow!("Unexpected status code: {}", response.status()));
        }
    }

    let data = response.json::<VerificationJobDispatch>().unwrap();

    Ok(data.job_id)
}

pub fn poll_verification_status(
    network: Network,
    job_id: &str,
    max_retries: u32,
) -> Result<VerificationJob> {
    // Get network api url
    let url = get_network_api(network);

    // Blocking loop that polls every 2 seconds
    static RETRY_INTERVAL: u64 = 2000; // Ms
    let mut retries: u32 = 0;

    let client = Client::new();

    // Retry every 500ms until we hit maxRetries
    while retries < max_retries {
        let result = client
            .get(url.clone() + "/api/class/job/" + job_id)
            .send()?;
        match result.status() {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => {
                return Err(anyhow!("Job not found"));
            }
            _ => {
                return Err(anyhow!("Unexpected status code: {}", result.status()));
            }
        }

        // Go through the possible status
        let data = result.json::<VerificationJob>().unwrap();
        match data.status {
            VerifyJobStatus::Success => return Ok(data),
            VerifyJobStatus::Fail => return Err(anyhow!("Failed to verify")),
            VerifyJobStatus::CompileFailed => return Err(anyhow!("Compilation failed")),
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
