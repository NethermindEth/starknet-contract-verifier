mod api;
mod args;
mod class_hash;
mod resolver;
mod voyager;

use crate::{
    api::{poll_verification_status, ApiClient, ApiClientError, VerificationJob},
    args::{Args, Commands, SubmitArgs},
    resolver::ResolverError,
};
use api::ProjectMetadataInfo;
use clap::Parser;
use class_hash::ClassHash;
use scarb_metadata::{PackageId, PackageMetadata};
use std::{collections::HashMap, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Api(#[from] ApiClientError),

    #[error("Class hash {0} is not declared")]
    NotDeclared(ClassHash),

    #[error(
        "No contracts selected for verification. Add [tool.voyager] section to Scorb.toml file"
    )]
    NoTarget,

    #[error("Only single contract verification is supported")]
    MultipleContracts,

    #[error(transparent)]
    Resolver(#[from] ResolverError),
}

fn main() -> Result<(), CliError> {
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
    let sources = resolver::gather_sources(metadata)?;
    for file_info in &sources {
        print!("{file_info:?}");
    }

    let mut package_contracts: HashMap<PackageId, Vec<PathBuf>> =
        resolver::contract_paths(metadata)?;

    if package_contracts.is_empty() {
        return Err(CliError::NoTarget);
    }

    let contract: PathBuf;
    let package_meta: &PackageMetadata;
    let mut contracts_iter = package_contracts.drain();
    match contracts_iter.next() {
        None => {
            return Err(CliError::NoTarget);
        }
        Some((p_id, mut contracts)) => {
            package_meta = metadata
                .get_package(&p_id)
                .ok_or(CliError::Resolver(ResolverError::NoPackage(p_id)))?;
            match contracts.pop() {
                None => {
                    return Err(CliError::NoTarget);
                }
                Some(c) => {
                    contract = c;
                }
            };
            // if !contracts.is_empty() {
            //     return Err(CliError::MultipleContracts);
            // }
        }
    }

    // if let Some(_) = contracts_iter.next() {
    //     return Err(CliError::MultipleContracts);
    // }

    let contract_project_path = resolver::relative_package_path(metadata, package_meta)?;

    let project_meta = ProjectMetadataInfo {
        cairo_version: metadata.app_version_info.cairo.version.clone(),
        scarb_version: metadata.app_version_info.version.clone(),
        contract_file: contract_project_path
            .clone()
            .into_std_path_buf()
            .join(contract)
            .to_string_lossy()
            .to_string(),
        // this one is super weird
        project_dir_path: contract_project_path.to_string(),
    };

    println!("{project_meta:?}");
    // Check if the class exists on the network
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
            sources,
        )
        .map_err(CliError::from);
}
// TODO: do a first pass to find all the contracts in the project
// For now we keep using the hardcoded value in the scarb.toml file
// let (project_files, project_metadata) = resolver::resolve_scarb(
//     args.path.clone().into(),
//     local_cairo_version,
//     local_scarb_version,
// )
// .map_err(|err| anyhow!(err.to_string()))?;

// Check if the class exists on the network
// private.get_class(&args.hash).and_then(|does_exist| {
//     if !does_exist {
//         Err(ApiClientError::Other(anyhow::anyhow!(
//             "This class hash does not exist for the given network. Please try again."
//         )))
//     } else {
//         Ok(does_exist)
//     }
// })?;

// println!("gathered files");
// for file in &project_files {
//     println!("{file:?}");
// }

// FileInfo { name: "dependency/src/mod_a/file_a1.cairo", path: "/home/nat/work/nethermind/cairo/starknet-contract-verifier/examples/cairo_ds/voyager-verify/dependency/src/mod_a/file_a1.cairo" }

// public.verify_class(
//     args.hash.clone(),
//     args.license
//         .map_or("No License (None)".to_string(), |l| {
//             format!("{} ({})", l.full_name, l.name)
//         })
//         .as_ref(),
//     args.name.as_ref(),
//     project_metadata,
//     project_files,
// )

fn check(public: ApiClient, job_id: &String) -> Result<VerificationJob, CliError> {
    poll_verification_status(public, job_id).map_err(CliError::from)
}
