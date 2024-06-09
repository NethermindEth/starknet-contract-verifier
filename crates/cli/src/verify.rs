use std::{env::current_dir, fs, str::FromStr, time::Instant};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{arg, builder::PossibleValue, Args, ValueEnum};
use console::{style, Emoji};
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use voyager_resolver_cairo::compiler::scarb_utils::read_additional_scarb_manifest_metadata;
use walkdir::{DirEntry, WalkDir};

use dyn_compiler::dyn_compiler::{SupportedCairoVersions, SupportedScarbVersions};

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
    cairo_version: SupportedCairoVersions,
    scarb_version: SupportedScarbVersions,
) -> Result<()> {
    // Start a spinner for the verification process
    let started = Instant::now();
    let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈");

    println!(
        "{} {}Extracting files from the Scarb project...",
        style("[1/4]").bold().dim(),
        Emoji("📃  ", "")
    );
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
    if contract_paths.is_empty() {
        return Err(anyhow::anyhow!("No contracts to verify"));
    }
    if contract_paths.len() > 1 {
        return Err(anyhow::anyhow!(
            "Only one contract can be verified at a time"
        ));
    }

    println!(
        "{} {}Checking if the class is already declared...",
        style("[2/4]").bold().dim(),
        Emoji("🔍  ", "")
    );
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

    println!(
        "{} {}Resolving contract dependencies...",
        style("[3/4]").bold().dim(),
        Emoji("🔗  ", "")
    );
    let steps = 4;
    let pb = ProgressBar::new(steps);

    // Read the scarb metadata to get more information
    // TODO: switch this to using scarb-metadata
    let scarb_toml_content = fs::read_to_string(source_dir.join("Scarb.toml"))?;
    let extracted_scarb_toml_data =
        read_additional_scarb_manifest_metadata(scarb_toml_content.as_str())?;

    // Compiler and extract the necessary files
    compiler.compile_project(&source_dir)?;
    pb.inc(2);

    // Since we know that we extract the files into the `voyager-verify` directory,
    // we'll read the files from there.
    let extracted_files_dir = source_dir.join("voyager-verify");

    // The compiler compiles into the original scarb package name
    // As such we have to craft the correct path to the main package
    let project_dir_path = extracted_files_dir.join(extracted_scarb_toml_data.name.clone());
    let project_dir_path = project_dir_path
        .strip_prefix(extracted_files_dir.clone())
        .unwrap();

    // Read project directory
    let project_files = WalkDir::new(extracted_files_dir.as_path())
        .into_iter()
        .filter_map(|f| f.ok())
        .filter(|f| f.file_type().is_file())
        .filter(|f| {
            let file_path = f.path();

            let is_cairo_file = match file_path.extension() {
                Some(ext) => ext == "cairo",
                None => false,
            };
            let file_entry_name = file_path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or("".into());

            let is_supplementary_file = file_entry_name.to_lowercase() == "scarb.toml"
                || file_entry_name == extracted_scarb_toml_data.license_file
                || file_entry_name == extracted_scarb_toml_data.readme;

            is_cairo_file || is_supplementary_file
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
                path: actual_path,
            }
        })
        .collect::<Vec<FileInfo>>();
    pb.inc(1);
    pb.finish_and_clear();

    // We already know the contract file specified in Scarb.toml is relative to src/
    let contract_file = format!(
        "{}/src/{}",
        extracted_scarb_toml_data.name.clone(),
        contract_paths[0].as_str()
    );
    // let spinner = ProgressBar::new_spinner();
    // spinner.set_style(ProgressStyle::default_spinner());
    // spinner.set_style(ProgressStyle::with_template("{spinner:2.green/white} {msg} [{elapsed_precise}] ").unwrap());
    // spinner.set_message("dispatching verification job");

    // let spinner_clone = spinner.clone();
    // thread::spawn(move || {
    //     while !spinner_clone.is_finished() {
    //         spinner_clone.tick();
    //         thread::sleep(std::time::Duration::from_millis(100));
    //     }
    // });

    let dispatch_response = dispatch_class_verification_job(
        args.api_key.as_str(),
        network_enum.clone(),
        &args.hash,
        args.license.to_long_string().as_str(),
        args.is_account_contract.unwrap_or(false),
        &args.name,
        ProjectMetadataInfo {
            cairo_version,
            scarb_version,
            contract_file,
            project_dir_path: project_dir_path.as_str().to_owned(),
        },
        project_files,
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
        Ok(_response) => {
            println!(
                "{} Successfully verified in {}",
                Emoji("✅ ", ""),
                HumanDuration(started.elapsed())
            );
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!(
            "Error while polling verification status: {}",
            e
        )),
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
