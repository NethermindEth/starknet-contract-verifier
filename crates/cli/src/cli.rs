mod api;
mod args;
mod class_hash;
mod errors;
mod resolver;
mod voyager;

use crate::{
    api::{poll_verification_status, ApiClient, ApiClientError, VerificationJob},
    args::{Args, Commands, SubmitArgs},
    resolver::ResolverError,
};
use api::{FileInfo, ProjectMetadataInfo};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use class_hash::ClassHash;
use itertools::Itertools;
use scarb_metadata::PackageMetadata;
use std::{collections::HashMap, path::PathBuf};
use thiserror::Error;
use voyager::VoyagerError;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Api(#[from] ApiClientError),

    #[error(transparent)]
    MissingPackage(#[from] errors::MissingPackage),

    #[error("Class hash {0} is not declared")]
    NotDeclared(ClassHash),

    #[error(
        "No contracts selected for verification. Add [tool.voyager] section to Scorb.toml file"
    )]
    NoTarget,

    #[error("Only single contract verification is supported. Select one with --contract argument")]
    MultipleContracts,

    // TODO: Display suggestions
    #[error(transparent)]
    MissingContract(#[from] errors::MissingContract),

    #[error(transparent)]
    Resolver(#[from] ResolverError),

    #[error("Couldn't strip {prefix} from {path}")]
    StripPrefix {
        path: Utf8PathBuf,
        prefix: Utf8PathBuf,
    },

    #[error(transparent)]
    Utf8(#[from] camino::FromPathBufError),

    #[error(transparent)]
    Voyager(#[from] VoyagerError),
}

fn main() -> anyhow::Result<()> {
    let Args {
        command: cmd,
        network_url: network,
        network: _,
    } = Args::parse();

    println!("public: {:?}", network.public);
    println!("private: {:?}", network.private);
    let public = ApiClient::new(network.public)?;
    let private = ApiClient::new(network.private)?;

    match &cmd {
        Commands::Submit(args) => {
            if args.license.is_none() {
                println!("[WARNING] No license provided, defaults to All Rights Reserved")
            }

            let job_id = submit(public, private, args)?;
            println!("verification job id: {}", job_id);
        }
        Commands::Status { job } => {
            let status = check(public, job)?;
            println!("{status:?}")
        }
    }
    Ok(())
}

fn submit(public: ApiClient, private: ApiClient, args: &SubmitArgs) -> Result<String, CliError> {
    let metadata = args.path.metadata();

    let mut packages: Vec<PackageMetadata> = vec![];
    resolver::gather_packages(metadata, &mut packages)?;
    let sources = packages
        .iter()
        .flat_map(resolver::package_sources)
        .collect_vec();

    let prefix = resolver::biggest_common_prefix(&sources, args.path.root_dir());
    let manifest = metadata
        .runtime_manifest
        .strip_prefix(&prefix)
        .map_err(|_| CliError::StripPrefix {
            path: metadata.runtime_manifest.clone(),
            prefix: prefix.clone(),
        })?;
    let files: HashMap<String, PathBuf> = HashMap::from([(
        manifest.to_string(),
        metadata.runtime_manifest.clone().into_std_path_buf(),
    )]);

    let tool_sections = voyager::tool_section(metadata)?;

    let contract_names: Vec<String> = tool_sections
        .iter()
        .flat_map(|(_id, v)| v.iter().map(|(name, _attrs)| name.clone()).collect_vec())
        .collect_vec();

    if let Some(to_submit) = args.contract.to_owned() {
        if !contract_names.contains(&to_submit) {
            return Err(CliError::from(errors::MissingContract::new(
                to_submit,
                contract_names,
            )));
        } else if contract_names.len() != 1 {
            return Err(CliError::MultipleContracts)
        }
    }

    let cairo_version = metadata.app_version_info.cairo.version.clone();
    let scarb_version = metadata.app_version_info.version.clone();

    for (package_id, tools) in &tool_sections {
        for (contract_name, voyager) in tools {
            // We should probably remove this and submit everything
            if Some(contract_name.clone()) != args.contract {
                continue;
            }

            let package_meta = metadata.get_package(package_id).ok_or(CliError::from(
                errors::MissingPackage::new(package_id, metadata),
            ))?;
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

            private
                .get_class(&args.hash)
                .map_err(CliError::from)
                .and_then(|does_exist| {
                    if !does_exist {
                        Err(CliError::NotDeclared(args.hash.clone()))
                    } else {
                        Ok(does_exist)
                    }
                })?;

            return public
                .verify_class(
                    args.hash.clone(),
                    args.license
                        .map_or("No License (None)".to_string(), |l| {
                            format!("{} ({})", l.full_name, l.name)
                        })
                        .as_ref(),
                    args.name.as_ref(),
                    project_meta,
                    files
                        .into_iter()
                        .map(|(name, path)| FileInfo { name, path })
                        .collect_vec(),
                )
                .map_err(CliError::from);
        }
    }

    Err(CliError::NoTarget)
}

fn check(public: ApiClient, job_id: &String) -> Result<VerificationJob, CliError> {
    poll_verification_status(public, job_id).map_err(CliError::from)
}
