mod args;
use crate::args::{Args, Commands, VerifyArgs};

use camino::{Utf8Path, Utf8PathBuf};
use chrono::{DateTime, Utc};
use clap::Parser;
use colored::*;
use itertools::Itertools;
use log::{debug, info, warn};
use scarb_metadata::PackageMetadata;
use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};
use thiserror::Error;
use verifier::{
    api::{
        poll_verification_status, ApiClient, ApiClientError, FileInfo, ProjectMetadataInfo,
        VerificationJob, VerifyJobStatus,
    },
    class_hash::ClassHash,
    errors, resolver, voyager,
};

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Api(#[from] ApiClientError),

    #[error(transparent)]
    MissingPackage(#[from] errors::MissingPackage),

    #[error("Class hash {0} is not declared")]
    NotDeclared(ClassHash),

    #[error("Verification dry run")]
    DryRun,

    #[error("No contracts selected for verification. Use --contract-name argument")]
    NoTarget,

    #[error(
        "Only single contract verification is supported. Specify with --contract-name argument"
    )]
    MultipleContracts,

    // TODO: Display suggestions
    #[error(transparent)]
    MissingContract(#[from] errors::MissingContract),

    #[error(transparent)]
    Resolver(#[from] resolver::Error),

    #[error("Couldn't strip {prefix} from {path}")]
    StripPrefix {
        path: Utf8PathBuf,
        prefix: Utf8PathBuf,
    },

    #[error(transparent)]
    Utf8(#[from] camino::FromPathBufError),

    #[error(transparent)]
    Voyager(#[from] voyager::Error),
}

fn display_verification_job_id(job_id: &str) {
    println!();
    println!("verification job id: {}", job_id.green().bold());
    println!();
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let Args {
        command: cmd,
        network_url: network,
        network: _,
    } = Args::parse();
    let public = ApiClient::new(network.public)?;
    let private = ApiClient::new(network.private)?;

    match &cmd {
        Commands::Verify(args) => {
            // Check if we can directly access the license from the manifest
            let license_result =
                std::fs::read_to_string(args.path.manifest_path()).map(|toml_content| {
                    if let Some(license_line) = toml_content
                        .lines()
                        .find(|line| line.trim().starts_with("license"))
                    {
                        if let Some(license_value) = license_line.split('=').nth(1) {
                            let license = license_value.trim().trim_matches('"').trim_matches('\'');
                            debug!("Found license in Scarb.toml: {license}");
                            // Accept any license value found
                            return Some(license.to_string());
                        }
                    }
                    None
                });

            let found_license = license_result.unwrap_or(None);

            if args.license.is_none()
                && args.path.get_license().is_none()
                && found_license.is_none()
            {
                warn!(
                    "No license provided via CLI or in Scarb.toml, defaults to All Rights Reserved"
                );
            }

            let job_id = submit(&public, &private, args, found_license)?;
            display_verification_job_id(&job_id);
        }
        Commands::Status { job } => {
            let status = check(&public, job)?;
            info!("{status:?}");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn submit(
    public: &ApiClient,
    _private: &ApiClient,
    args: &VerifyArgs,
    direct_license: Option<String>,
) -> Result<String, CliError> {
    let metadata = args.path.metadata();

    let mut packages: Vec<PackageMetadata> = vec![];
    resolver::gather_packages(metadata, &mut packages)?;

    // Get raw license string directly if we found it
    let raw_license_str: Option<String> = direct_license;

    // Get license as LicenseId for display purposes
    let license = args.license.or_else(|| args.path.get_license());

    let mut sources: Vec<Utf8PathBuf> = vec![];
    for package in &packages {
        let mut package_sources = resolver::package_sources(package)?;
        sources.append(&mut package_sources);
    }

    let prefix = resolver::biggest_common_prefix(&sources, args.path.root_dir());
    let manifest_path = voyager::manifest_path(metadata);
    let manifest = manifest_path
        .strip_prefix(&prefix)
        .map_err(|_| CliError::StripPrefix {
            path: manifest_path.clone(),
            prefix: prefix.clone(),
        })?;

    let mut files: HashMap<String, Utf8PathBuf> = sources
        .iter()
        .map(|p| -> Result<(String, Utf8PathBuf), CliError> {
            let name = p.strip_prefix(&prefix).map_err(|_| CliError::StripPrefix {
                path: p.clone(),
                prefix: prefix.clone(),
            })?;
            Ok((name.to_string(), p.clone()))
        })
        .try_collect()?;
    files.insert(
        manifest.to_string(),
        voyager::manifest_path(metadata).clone(),
    );

    // Also ensure the workspace root Scarb.toml is included if we're in a workspace
    let workspace_manifest = &metadata.workspace.manifest_path;
    // Check if this is a workspace by comparing normalized paths and checking if workspace has multiple members
    let is_workspace = workspace_manifest != manifest_path && metadata.workspace.members.len() > 1;
    debug!("Workspace manifest: {}", workspace_manifest);
    debug!("Current manifest: {}", manifest_path);
    debug!("Is workspace project: {}", is_workspace);
    debug!("Workspace members: {}", metadata.workspace.members.len());

    if is_workspace {
        let workspace_manifest_rel =
            workspace_manifest
                .strip_prefix(&prefix)
                .map_err(|_| CliError::StripPrefix {
                    path: workspace_manifest.clone(),
                    prefix: prefix.clone(),
                })?;
        debug!("Including workspace root manifest: {}", workspace_manifest);
        files.insert(
            workspace_manifest_rel.to_string(),
            workspace_manifest.clone(),
        );
    }

    // Filter packages based on the --package argument if provided
    let filtered_packages: Vec<&PackageMetadata> = if let Some(package_id) = &args.package {
        packages.iter().filter(|p| p.name == *package_id).collect()
    } else {
        packages.iter().collect()
    };

    if filtered_packages.is_empty() {
        if let Some(package_id) = &args.package {
            let available_packages: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
            return Err(CliError::from(errors::MissingContract::new(
                package_id.clone(),
                available_packages,
            )));
        }
    }

    // We need either --package or --contract or both to be specified
    if args.package.is_none() {
        // For workspace projects, package is required
        if is_workspace {
            let available_packages: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
            return Err(CliError::from(errors::MissingContract::new(
                "Workspace project detected - use --package argument".to_string(),
                available_packages,
            )));
        }
    }

    let cairo_version = metadata.app_version_info.cairo.version.clone();
    let scarb_version = metadata.app_version_info.version.clone();

    // Process the first matching package (or the first one if no package specified)
    let package_meta = filtered_packages
        .first()
        .ok_or_else(|| CliError::NoTarget)?;

    // Use the provided contract name
    let contract_name = &args.contract_name;

    let project_dir_path = args
        .path
        .root_dir()
        .strip_prefix(&prefix)
        .map_err(|_| CliError::StripPrefix {
            path: args.path.root_dir().clone(),
            prefix: prefix.clone(),
        })
        // backend expects this for cwd
        .map(|p| {
            if p == Utf8Path::new("") {
                Utf8Path::new(".")
            } else {
                p
            }
        })?;

    // Find the main source file for the package (conventionally src/lib.cairo or src/main.cairo)
    let possible_main_paths = vec!["src/lib.cairo", "src/main.cairo"];

    let mut contract_file_path = None;

    for path in possible_main_paths {
        let full_path = package_meta.root.join(path);
        if full_path.exists() {
            contract_file_path = Some(full_path);
            break;
        }
    }

    // If we can't find a main file, use the first source file in the package
    if contract_file_path.is_none() {
        // Get all source files from this package
        let package_source_files = sources
            .iter()
            .filter(|path| path.starts_with(&package_meta.root))
            .find(|path| path.extension() == Some("cairo"))
            .cloned();

        contract_file_path = package_source_files;
    }

    let contract_file_path = contract_file_path.ok_or_else(|| CliError::NoTarget)?;

    let contract_file = contract_file_path
        .strip_prefix(prefix.clone())
        .map_err(|_| CliError::StripPrefix {
            path: contract_file_path.clone(),
            prefix,
        })?;

    let project_meta = ProjectMetadataInfo {
        cairo_version: cairo_version.clone(),
        scarb_version: scarb_version.clone(),
        contract_file: contract_file.to_string(),
        project_dir_path: project_dir_path.to_string(),
        package_name: package_meta.name.clone(),
    };

    info!("Verifying contract: {contract_name} from {contract_file}");

    // Format the license display
    let license_display = match &license {
        Some(id) => match id.name {
            // Map common license names to their SPDX identifiers
            "MIT License" => "MIT",
            "Apache License 2.0" => "Apache-2.0",
            "GNU General Public License v3.0 only" => "GPL-3.0-only",
            "BSD 3-Clause License" => "BSD-3-Clause",
            other => other,
        },
        None => {
            if let Some(ref direct) = raw_license_str {
                direct
            } else {
                "NONE"
            }
        }
    };
    info!("licensed with: {license_display}");

    info!("using cairo: {cairo_version} and scarb {scarb_version}");
    info!("These are the files that will be used for verification:");
    for path in files.values() {
        info!("{path}");
    }

    if args.execute {
        return public
            .verify_class(
                &args.class_hash,
                Some(license_display.to_string()),
                contract_name,
                project_meta,
                &files
                    .into_iter()
                    .map(|(name, path)| FileInfo {
                        name,
                        path: path.into_std_path_buf(),
                    })
                    .collect_vec(),
            )
            .map_err(CliError::from);
    }

    info!("Nothing to do, add `--execute` flag to actually verify the contract");
    Err(CliError::DryRun)
}

fn format_timestamp(timestamp: f64) -> String {
    let duration = Duration::from_secs_f64(timestamp);
    if let Some(datetime) = UNIX_EPOCH.checked_add(duration) {
        let datetime: DateTime<Utc> = datetime.into();
        datetime.to_rfc3339()
    } else {
        timestamp.to_string()
    }
}

fn check(public: &ApiClient, job_id: &str) -> Result<VerificationJob, CliError> {
    let status = poll_verification_status(public, job_id).map_err(CliError::from)?;

    match status.status() {
        VerifyJobStatus::Success => {
            println!("\n✅ Verification successful!");
            if let Some(name) = status.name() {
                println!("Contract name: {name}");
            }
            if let Some(file) = status.contract_file() {
                println!("Contract file: {file}");
            }
            if let Some(version) = status.version() {
                println!("Version: {version}");
            }
            if let Some(license) = status.license() {
                println!("License: {license}");
            }
            if let Some(address) = status.address() {
                println!("Contract address: {address}");
            }
            println!("Class hash: {}", status.class_hash());
            if let Some(created) = status.created_timestamp() {
                println!("Created: {}", format_timestamp(created));
            }
            if let Some(updated) = status.updated_timestamp() {
                println!("Last updated: {}", format_timestamp(updated));
            }
            println!("\nThe contract is now verified and visible on Voyager.");
            println!("You can view it by searching for the class hash above.");
        }
        VerifyJobStatus::Fail => {
            println!("\n❌ Verification failed!");
            if let Some(desc) = status.status_description() {
                println!("Reason: {desc}");
            }
            if let Some(created) = status.created_timestamp() {
                println!("Started: {}", format_timestamp(created));
            }
            if let Some(updated) = status.updated_timestamp() {
                println!("Failed: {}", format_timestamp(updated));
            }
        }
        VerifyJobStatus::CompileFailed => {
            println!("\n❌ Compilation failed!");
            if let Some(desc) = status.status_description() {
                println!("Reason: {desc}");
            }
            if let Some(created) = status.created_timestamp() {
                println!("Started: {}", format_timestamp(created));
            }
            if let Some(updated) = status.updated_timestamp() {
                println!("Failed: {}", format_timestamp(updated));
            }
        }
        VerifyJobStatus::Processing => {
            println!("\n⏳ Contract verification is being processed...");
            println!("Job ID: {}", status.job_id());
            println!("Status: Processing");
            if let Some(created) = status.created_timestamp() {
                println!("Started: {}", format_timestamp(created));
            }
            if let Some(updated) = status.updated_timestamp() {
                println!("Last updated: {}", format_timestamp(updated));
            }
            println!("\nUse the same command to check progress later.");
        }
        VerifyJobStatus::Submitted => {
            println!("\n⏳ Verification job submitted and waiting for processing...");
            println!("Job ID: {}", status.job_id());
            println!("Status: Submitted");
            if let Some(created) = status.created_timestamp() {
                println!("Submitted: {}", format_timestamp(created));
            }
            println!("\nUse the same command to check progress later.");
        }
        VerifyJobStatus::Compiled => {
            println!("\n⏳ Contract compiled successfully, verification in progress...");
            println!("Job ID: {}", status.job_id());
            println!("Status: Compiled");
            if let Some(created) = status.created_timestamp() {
                println!("Started: {}", format_timestamp(created));
            }
            if let Some(updated) = status.updated_timestamp() {
                println!("Last updated: {}", format_timestamp(updated));
            }
            println!("\nUse the same command to check progress later.");
        }
        _ => {
            println!("\n⏳ Verification in progress...");
            println!("Job ID: {}", status.job_id());
            println!("Status: {}", status.status());
            if let Some(created) = status.created_timestamp() {
                println!("Started: {}", format_timestamp(created));
            }
            if let Some(updated) = status.updated_timestamp() {
                println!("Last updated: {}", format_timestamp(updated));
            }
            println!("\nUse the same command to check progress later.");
        }
    }

    Ok(status)
}
