mod args;
use crate::args::{Args, Commands, SubmitArgs};

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use colored::*;
use itertools::Itertools;
use log::{debug, info, warn};
use scarb_metadata::PackageMetadata;
use std::collections::HashMap;
use thiserror::Error;
use verifier::{
    api::{
        poll_verification_status, ApiClient, ApiClientError, FileInfo, ProjectMetadataInfo,
        VerificationJob,
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

    #[error("Submit dry run")]
    DryRun,

    #[error(
        "No contracts selected for verification. Add [tool.voyager] section to Scarb.toml file"
    )]
    NoTarget,

    #[error("Only single contract verification is supported. Select one with --contract argument")]
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
        Commands::Submit(args) => {
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
    args: &SubmitArgs,
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

    let tool_sections = voyager::tool_section(metadata)?;

    let contract_names: Vec<String> = tool_sections
        .iter()
        .flat_map(|(_id, v)| v.iter().map(|(name, _attrs)| name.clone()).collect_vec())
        .collect_vec();

    if let Some(to_submit) = args.contract.clone() {
        if !contract_names.contains(&to_submit) {
            return Err(CliError::from(errors::MissingContract::new(
                to_submit,
                contract_names,
            )));
        }
    } else if contract_names.len() != 1 {
        return Err(CliError::MultipleContracts);
    }

    let cairo_version = metadata.app_version_info.cairo.version.clone();
    let scarb_version = metadata.app_version_info.version.clone();

    for (package_id, tools) in &tool_sections {
        for (contract_name, voyager) in tools {
            // We should probably remove this and submit everything
            if let Some(to_submit) = args.contract.clone() {
                if &to_submit != contract_name {
                    continue;
                }
            }

            let package_meta = metadata
                .get_package(package_id)
                .ok_or_else(|| CliError::from(errors::MissingPackage::new(package_id, metadata)))?;
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

            let contract_dir = Utf8PathBuf::try_from(package_meta.root.join_os(&voyager.path))?;
            let contract_file =
                contract_dir
                    .strip_prefix(prefix.clone())
                    .map_err(|_| CliError::StripPrefix {
                        path: contract_dir.clone(),
                        prefix,
                    })?;
            let project_meta = ProjectMetadataInfo {
                cairo_version: cairo_version.clone(),
                scarb_version: scarb_version.clone(),
                contract_file: contract_file.to_string(),
                project_dir_path: project_dir_path.to_string(),
            };

            info!("Submitting contract: {contract_name} from {contract_file},");
            info!("under the name of: {contract_name}");

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
            info!("licensed with: {license_display}.");

            info!("using cairo: {cairo_version} and scarb {scarb_version}");
            info!("These are the files that I'm about to transfer:");
            for path in files.values() {
                info!("{path}");
            }

            if args.execute {
                return public
                    .verify_class(
                        &args.hash,
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

            info!("Nothing to do, add `--execute` flag to actually submit contract");
            return Err(CliError::DryRun);
        }
    }

    Err(CliError::NoTarget)
}

fn check(public: &ApiClient, job_id: &str) -> Result<VerificationJob, CliError> {
    poll_verification_status(public, job_id).map_err(CliError::from)
}
