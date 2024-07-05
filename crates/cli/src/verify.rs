use std::{env::current_dir, str::FromStr};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{arg, Args};

use dyn_compiler::dyn_compiler::SupportedCairoVersions;

use crate::{
    api::{
        dispatch_class_verification_job, does_class_exist, poll_verification_status, FileInfo,
        Network, ProjectMetadataInfo,
    },
    license::LicenseType,
    resolver::get_dynamic_compiler,
};

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

    pub api_key: String,
}

#[derive(Args, Debug)]
pub struct VerifyFileArgs {
    #[arg(help = "File path")]
    path: Utf8PathBuf,
}

pub fn verify_project(
    args: VerifyProjectArgs,
    metadata: ProjectMetadataInfo,
    files: Vec<FileInfo>,
) -> Result<()> {
    // Check if the class exists on the network
    let network_enum = Network::from_str(args.network.as_str())?;
    match does_class_exist(network_enum.clone(), &args.hash) {
        Ok(true) => (),
        Ok(false) => return Err(anyhow::anyhow!("Class does not exist on the network")),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Error while checking if class exists: {}",
                e
            ))
        }
    }

    let dispatch_response = dispatch_class_verification_job(
        args.api_key.as_str(),
        network_enum.clone(),
        &args.hash,
        args.license.to_long_string().as_str(),
        args.is_account_contract.unwrap_or(false),
        &args.name,
        metadata,
        files,
    );

    let job_id = match dispatch_response {
        Ok(response) => response,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to dispatch verification job: {}",
                e
            ));
        }
    };

    // Retry for 5 minutes
    let poll_result = poll_verification_status(
        args.api_key.as_str(),
        network_enum,
        &job_id,
        args.max_retries.unwrap_or(180),
    );

    match poll_result {
        Ok(_response) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(
            "Error while polling verification status: {}",
            e
        )),
    }
}

pub fn _verify_file(args: VerifyFileArgs, cairo_version: SupportedCairoVersions) -> Result<()> {
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
