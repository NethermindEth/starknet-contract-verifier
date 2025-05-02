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
        poll_verification_status, ApiClient, ApiClientError, ContractMetadataInfo, VerificationJob,
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

    #[error("{0}")]
    InvalidLicense(String),

    #[error(transparent)]
    NoPackageSelected(#[from] errors::NoPackageSelected),

    #[error("Couldn't find any packages. Is the manifest file valid?")]
    NoPackages,

    #[error(transparent)]
    Resolver(#[from] resolver::Error),

    #[error("Couldn't strip {prefix} from {path}")]
    StripPrefix {
        path: Utf8PathBuf,
        prefix: Utf8PathBuf,
    },
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
            let job_id = submit(&public, &private, args)?;
            println!("verification job id: {job_id}");
        }
        Commands::Status { job } => {
            let status = check(&public, job)?;
            println!("{status:?}");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn submit(public: &ApiClient, private: &ApiClient, args: &SubmitArgs) -> Result<String, CliError> {
    let metadata = args.path.metadata();

    let cairo_version = metadata.app_version_info.cairo.version.clone();
    let scarb_version = metadata.app_version_info.version.clone();
    // TODO: Check version support

    let mut workspace_packages = resolver::workspace_packages(metadata);

    let selected_package = if workspace_packages.len() > 1 {
        args.package
            .clone()
            .ok_or(CliError::from(errors::NoPackageSelected {
                suggestions: workspace_packages.iter().map(|p| p.id.clone()).collect(),
            }))
            .and_then(|package_name| {
                workspace_packages
                    .into_iter()
                    .find(|p| p.name == *package_name)
                    .ok_or(CliError::from(errors::MissingPackage::new(
                        package_name,
                        metadata,
                    )))
            })?
    } else {
        workspace_packages.pop().ok_or(CliError::NoPackages)?
    };

    let license = selected_package
        .manifest_metadata
        .license
        .map(|lic| args::license_value_parser(&lic))
        .transpose()
        .map_err(CliError::InvalidLicense)?;

    if license.is_none() {
        println!(
            "[WARNING] No license field defined in the Scarb.toml, defaults to All Rights Reserved"
        );
    }

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

    let project_meta = ContractMetadataInfo {
        cairo_version: cairo_version.clone(),
        scarb_version: scarb_version.clone(),
        project_dir_path: project_dir_path.into(),
        package_name: selected_package.name.clone(),
        name: args.contract.clone(),
        license,
    };

    println!(
        "Submitting contract: {} from {},",
        args.contract, selected_package.name
    );
    println!(
        "licensed with: {}.",
        license.map_or("No License (None)", |id| id.name)
    );
    println!("using cairo: {cairo_version} and scarb {scarb_version}");
    println!("These are the files that I'm about to transfer:");
    for path in files.values() {
        println!("{path}");
    }

    if args.execute {
        private
            .get_class(&args.hash)
            .map_err(CliError::from)
            .and_then(|does_exist| {
                if does_exist {
                    Ok(does_exist)
                } else {
                    Err(CliError::NotDeclared(args.hash.clone()))
                }
            })?;

        return public
            .verify_class(&args.hash, args.name.as_ref(), project_meta, &files)
            .map_err(CliError::from);
    }

    println!("Nothing to do, add `--execute` flag to actually submit contract");
    Err(CliError::DryRun)
}

fn check(public: &ApiClient, job_id: &str) -> Result<VerificationJob, CliError> {
    poll_verification_status(public, job_id).map_err(CliError::from)
}
