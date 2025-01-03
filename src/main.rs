mod args;
use crate::args::{Args, Commands, SubmitArgs};

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use itertools::Itertools;
use scarb_metadata::PackageMetadata;
use std::collections::HashMap;
use thiserror::Error;
use verifier::{
    api::{
        poll_verification_status, ApiClient, ApiClientError, FileInfo, ProjectMetadataInfo,
        VerificationJob,
    },
    class_hash::ClassHash,
    errors,
    resolver::{self, ResolverError},
    voyager::{self, VoyagerError},
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

    if let Some(to_submit) = args.contract.to_owned() {
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
            if let Some(to_submit) = args.contract.to_owned() {
                if &to_submit != contract_name {
                    continue;
                }
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

            // TODO: switch backend to use SPDX identifiers
            let license: String = match args.license {
                None => "No License (None)".to_string(),
                Some(id) => match id.name {
                    "Unlicense" => "The Unlicense (Unlicense)".to_string(),
                    "MIT" => "MIT License (MIT)".to_string(),
                    "GPL-2.0" => "GNU General Public License v2.0 (GNU GPLv2)".to_string(),
                    "GPL-3.0" => "GNU General Public License v3.0 (GNU GPLv3)".to_string(),
                    "LGPL-2.1" => {
                        "GNU Lesser General Public License v2.1 (GNU LGPLv2.1)".to_string()
                    }
                    "LGPL-3.0" => "GNU Lesser General Public License v3.0 (GNU LGPLv3)".to_string(),
                    "BSD-2-Clause" => {
                        r#"BSD 2-clause "Simplified" license (BSD-2-Clause)"#.to_string()
                    }
                    "BSD-3-Clause" => {
                        r#"BSD 3-clause "New" Or "Revisited license (BSD-3-Clause)"#.to_string()
                    }
                    "MPL-2.0" => "Mozilla Public License 2.0 (MPL-2.0)".to_string(),
                    "OSL-3.0" => "Open Software License 3.0 (OSL-3.0)".to_string(),
                    "Apache-2.0" => "Apache 2.0 (Apache-2.0)".to_string(),
                    "AGPL-3.0" => "GNU Affero General Public License (GNU AGPLv3)".to_string(),
                    "BUSL-1.1" => "Business Source License (BSL 1.1)".to_string(),
                    _ => format!("{} ({})", id.full_name, id.name),
                },
            };

            println!(
                "Submiting contract: {} from {},",
                contract_name, contract_file
            );
            println!("under the name of: {},", args.name);
            println!("licensed with: {}.", license);
            println!("using cairo: {} and scarb {}", cairo_version, scarb_version);
            println!("These are the files that I'm about to transfer:");
            for (_name, path) in &files {
                println!("{path}");
            }

            if args.execute {
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
                        license.as_str(),
                        args.name.as_ref(),
                        project_meta,
                        files
                            .into_iter()
                            .map(|(name, path)| FileInfo {
                                name,
                                path: path.into_std_path_buf(),
                            })
                            .collect_vec(),
                    )
                    .map_err(CliError::from);
            } else {
                println!("Nothing to do, add `--execute` flag to actually submit contract");
                return Err(CliError::DryRun);
            }
        }
    }

    Err(CliError::NoTarget)
}

fn check(public: ApiClient, job_id: &String) -> Result<VerificationJob, CliError> {
    poll_verification_status(public, job_id).map_err(CliError::from)
}
