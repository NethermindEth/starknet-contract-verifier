use std::{
    env::current_dir,
    fs::{self, File},
    slice,
    str::FromStr,
};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{arg, builder::PossibleValue, Args, ValueEnum};
use walkdir::{DirEntry, WalkDir};

use dyn_compiler::dyn_compiler::{SupportedCairoVersions, SupportedScarbVersions};

use crate::{
    api::{dispatch_class_verification_job, does_class_exist, FileInfo, LicenseType, Network, poll_verification_status, ProjectMetadataInfo},
    resolver::get_dynamic_compiler,
};

impl ValueEnum for LicenseType {
    fn from_str(input: &str, ignore_case: bool) -> std::result::Result<Self, String> {
        match input {
            "NoLicense" => Ok(LicenseType::NoLicense),
            "Unlicense" => Ok(LicenseType::Unlicense),
            "MIT" => Ok(LicenseType::MIT),
            "GPLv2" => Ok(LicenseType::GPLv2),
            "GPLc3" => Ok(LicenseType::GPLv3),
            "LGPLv2_1" => Ok(LicenseType::LGPLv2_1),
            "LGPLv3" => Ok(LicenseType::LGPLv3),
            "BSD2Clause" => Ok(LicenseType::BSD2Clause),
            "BSD3Clause" => Ok(LicenseType::BSD3Clause),
            "MPL2" => Ok(LicenseType::MPL2),
            "OSL3" => Ok(LicenseType::OSL3),
            "Apache2" => Ok(LicenseType::Apache2),
            "AGPLv3" => Ok(LicenseType::AGPLv3),
            "BSL1_1" => Ok(LicenseType::BSL1_1),
            _ => Err(format!("Unknown license type: {}", input)),
        }
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        PossibleValue::new(self.to_string()).into()
    }

    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::NoLicense,
            Self::Unlicense,
            Self::MIT,
            Self::GPLv2,
            Self::GPLv3,
            Self::LGPLv2_1,
            Self::LGPLv3,
            Self::BSD2Clause,
            Self::BSD3Clause,
            Self::MPL2,
            Self::OSL3,
            Self::Apache2,
            Self::AGPLv3,
            Self::BSL1_1,
        ]
    }
}

#[derive(Args, Debug)]
pub struct VerifyProjectArgs {
    #[arg(
        help = "Network to verify against",
        default_value_t=String::from("mainnet"),
        required = true
    )]
    pub network: String,

    #[arg(help = "Class hash to verify", required = true)]
    pub hash: String,

    #[arg(help = "license type", required = true)]
    pub license: LicenseType,

    #[arg(help = "Name", required = true)]
    pub name: String,

    #[arg(help = "Source directory", required = true)]
    pub path: Utf8PathBuf,

    #[arg(long, help = "Is it an account contract?")]
    pub is_account_contract: Option<bool>,

    #[arg(long, help = "Max retries")]
    pub max_retries: Option<u32>,
}

#[derive(Args, Debug)]
pub struct VerifyFileArgs {
    #[arg(help = "File path")]
    path: Utf8PathBuf,
}

pub fn verify_project(
    args: VerifyProjectArgs,
    cairo_version: SupportedCairoVersions,
    scarb_version: SupportedScarbVersions,
) -> Result<()> {
    // Extract necessary files from the Scarb project for the verified contract
    let source_dir = if args.path.is_absolute() {
        args.path
    } else {
        let mut current_path = current_dir().unwrap();
        current_path.push(args.path);
        Utf8PathBuf::from_path_buf(current_path).unwrap()
    };

    let compiler = get_dynamic_compiler(cairo_version);

    let contract_paths = compiler.get_contracts_to_verify_path(&source_dir)?;

    // TODO: maybe support multiple contracts in one verification?
    if contract_paths.len() == 0 {
        return Err(anyhow::anyhow!("No contracts to verify"));
    }
    if contract_paths.len() > 1 {
        return Err(anyhow::anyhow!(
            "Only one contract can be verified at a time"
        ));
    }

    let network_enum = Network::from_str(args.network.as_str())?;

    match does_class_exist(network_enum.clone(), &args.hash) {
        Ok(true) => {}
        Ok(false) => {
            return Err(anyhow::anyhow!("Class does not exist on the network"));
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Error while checking if class exists: {}",
                e
            ));
        }
    }

    // Compiler and extract the necessary files
    compiler.compile_project(&source_dir)?;

    // Since we know that we extract the files into the `voyager-verify` directory,
    // we'll read the files from there.
    let extracted_files_dir = source_dir.join("voyager-verify");

    // Since we also know that the dir of main project to be verified will be the same name, extract relative path
    let project_dir_path = source_dir.strip_prefix(source_dir.parent().unwrap()).unwrap();

    // Read project directory
    let project_files = WalkDir::new(extracted_files_dir.as_path())
        .into_iter()
        .filter_map(|f| f.ok())
        .filter(|f| f.file_type().is_file())
        .filter(|f| {
            f.path().extension().unwrap() == "cairo" || 
                f.path().file_name().map(|f| f.to_string_lossy().to_owned()).unwrap_or("".into()).to_lowercase() == "scarb.toml"
        })
        .collect::<Vec<DirEntry>>();

    let project_files = project_files
        .iter()
        .map(|f| {
            let actual_path = f.path().to_owned();
            let file_name = actual_path
                .strip_prefix(&extracted_files_dir)
                .unwrap()
                .to_str()
                .to_owned()
                .unwrap()
                .to_string();
            FileInfo {
                name: file_name,
                path: actual_path
            }
        })
        .collect::<Vec<FileInfo>>();

    let dispatch_response = dispatch_class_verification_job(
        network_enum.clone(),
        &args.hash,
        args.license.to_long_string().as_str(),
        args.is_account_contract.unwrap_or(false),
        &args.name,
        ProjectMetadataInfo {
            cairo_version,
            scarb_version,
            project_dir_path: project_dir_path.as_str().to_owned()
        },
        project_files,
    );

    let job_id = match dispatch_response {
        Ok(response) => response,
        Err(e) => {
            return Err(anyhow::anyhow!("Error while dispatching verification job: {}", e));
        }
    };

    let poll_result = poll_verification_status(network_enum, &job_id, args.max_retries.unwrap_or(10));

    match poll_result {
        Ok(response) => {
            println!("Successfully verified!");
            return Ok(());
        },
        Err(e) => {
            return Err(anyhow::anyhow!("Error while polling verification status: {}", e));
        }
    }
}

pub fn verify_file(args: VerifyFileArgs, cairo_version: SupportedCairoVersions) -> Result<()> {
    let file_dir: Utf8PathBuf = match args.path.is_absolute() {
        true => args.path.clone(),
        false => {
            let mut current_path = current_dir().unwrap();
            current_path.push(args.path);
            Utf8PathBuf::from_path_buf(current_path).unwrap()
        }
    };

    let compiler = get_dynamic_compiler(cairo_version);
    compiler.compile_file(&file_dir)
}
