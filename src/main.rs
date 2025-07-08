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
    errors, license, resolver, voyager,
};

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Api(#[from] ApiClientError),

    #[error(transparent)]
    MissingPackage(#[from] errors::MissingPackage),

    #[error("Class hash {0} is not declared")]
    NotDeclared(ClassHash),

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
            let license_info = license::resolve_license_info(
                args.license,
                args.path.get_license(),
                args.path.manifest_path(),
            );

            license::warn_if_no_license(&license_info);

            let job_id = submit(&public, &private, args, &license_info)?;
            if job_id != "dry-run" {
                display_verification_job_id(&job_id);
            }
        }
        Commands::Status { job } => {
            let status = check(&public, job)?;
            info!("{status:?}");
        }
    }
    Ok(())
}

fn submit(
    public: &ApiClient,
    _private: &ApiClient,
    args: &VerifyArgs,
    license_info: &license::LicenseInfo,
) -> Result<String, CliError> {
    let metadata = args.path.metadata();

    // Gather packages and sources
    let packages = gather_packages_and_validate(metadata, args)?;
    let sources = collect_source_files(metadata, &packages)?;

    // Prepare project structure
    let (file_infos, package_meta, contract_file, project_dir_path) =
        prepare_project_for_verification(args, metadata, &packages, sources)?;

    // Log verification info
    log_verification_info(args, metadata, &file_infos, &contract_file, license_info);

    // Execute verification if requested
    if args.execute {
        return execute_verification(
            public,
            args,
            file_infos,
            package_meta,
            contract_file,
            project_dir_path,
            license_info,
        );
    }

    info!("Nothing to do, add `--execute` flag to actually verify the contract");
    Ok("dry-run".to_string())
}

fn gather_packages_and_validate(
    metadata: &scarb_metadata::Metadata,
    args: &VerifyArgs,
) -> Result<Vec<PackageMetadata>, CliError> {
    let mut packages: Vec<PackageMetadata> = vec![];
    resolver::gather_packages(metadata, &mut packages)?;

    // Filter packages based on --package argument
    let filtered_packages: Vec<&PackageMetadata> = if let Some(package_id) = &args.package {
        packages.iter().filter(|p| p.name == *package_id).collect()
    } else {
        packages.iter().collect()
    };

    // Validate package selection
    if filtered_packages.is_empty() {
        if let Some(package_name) = &args.package {
            let available_packages: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
            return Err(CliError::from(errors::MissingContract::new(
                package_name.clone(),
                available_packages,
            )));
        }
    }

    // Check workspace requirements
    let workspace_manifest = &metadata.workspace.manifest_path;
    let manifest_path = voyager::manifest_path(metadata);
    let is_workspace = workspace_manifest != manifest_path && metadata.workspace.members.len() > 1;

    if args.package.is_none() && is_workspace {
        let available_packages: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
        return Err(CliError::from(errors::MissingContract::new(
            "Workspace project detected - use --package argument".to_string(),
            available_packages,
        )));
    }

    Ok(packages)
}

fn collect_source_files(
    _metadata: &scarb_metadata::Metadata,
    packages: &[PackageMetadata],
) -> Result<Vec<Utf8PathBuf>, CliError> {
    let mut sources: Vec<Utf8PathBuf> = vec![];
    for package in packages {
        let mut package_sources = resolver::package_sources(package)?;
        sources.append(&mut package_sources);
    }
    Ok(sources)
}

fn prepare_project_for_verification(
    args: &VerifyArgs,
    metadata: &scarb_metadata::Metadata,
    packages: &[PackageMetadata],
    sources: Vec<Utf8PathBuf>,
) -> Result<(Vec<FileInfo>, PackageMetadata, String, String), CliError> {
    let prefix = resolver::biggest_common_prefix(&sources, args.path.root_dir());

    // Build file map
    let files = build_file_map(&sources, &prefix, metadata, args)?;

    // Filter packages and get the target package
    let filtered_packages: Vec<&PackageMetadata> = if let Some(package_id) = &args.package {
        packages.iter().filter(|p| p.name == *package_id).collect()
    } else {
        packages.iter().collect()
    };

    let package_meta = filtered_packages
        .first()
        .ok_or_else(|| CliError::NoTarget)?;

    // Find contract file
    let contract_file_path = find_contract_file(package_meta, &sources)?;
    let contract_file =
        contract_file_path
            .strip_prefix(&prefix)
            .map_err(|_| CliError::StripPrefix {
                path: contract_file_path.clone(),
                prefix: prefix.clone(),
            })?;

    // Prepare project directory path
    let project_dir_path = prepare_project_dir_path(args, &prefix)?;

    // Convert to FileInfo
    let file_infos = convert_to_file_info(files);

    Ok((
        file_infos,
        (*package_meta).clone(),
        contract_file.to_string(),
        project_dir_path,
    ))
}

fn build_file_map(
    sources: &[Utf8PathBuf],
    prefix: &Utf8Path,
    metadata: &scarb_metadata::Metadata,
    args: &VerifyArgs,
) -> Result<HashMap<String, Utf8PathBuf>, CliError> {
    let mut files: HashMap<String, Utf8PathBuf> = sources
        .iter()
        .map(|p| -> Result<(String, Utf8PathBuf), CliError> {
            let name = p.strip_prefix(prefix).map_err(|_| CliError::StripPrefix {
                path: p.clone(),
                prefix: prefix.to_path_buf(),
            })?;
            Ok((name.to_string(), p.clone()))
        })
        .try_collect()?;

    // Add manifest files
    add_manifest_files(&mut files, metadata, prefix)?;

    // Add lock file if requested
    add_lock_file_if_requested(&mut files, args, prefix)?;

    Ok(files)
}

fn add_manifest_files(
    files: &mut HashMap<String, Utf8PathBuf>,
    metadata: &scarb_metadata::Metadata,
    prefix: &Utf8Path,
) -> Result<(), CliError> {
    let manifest_path = voyager::manifest_path(metadata);
    let manifest = manifest_path
        .strip_prefix(prefix)
        .map_err(|_| CliError::StripPrefix {
            path: manifest_path.clone(),
            prefix: prefix.to_path_buf(),
        })?;

    files.insert(manifest.to_string(), manifest_path.clone());

    // Handle workspace manifests
    add_workspace_manifest_if_needed(files, metadata, prefix)?;

    Ok(())
}

fn add_workspace_manifest_if_needed(
    files: &mut HashMap<String, Utf8PathBuf>,
    metadata: &scarb_metadata::Metadata,
    prefix: &Utf8Path,
) -> Result<(), CliError> {
    let workspace_manifest = &metadata.workspace.manifest_path;
    let manifest_path = voyager::manifest_path(metadata);

    let is_workspace = workspace_manifest != manifest_path && metadata.workspace.members.len() > 1;

    if is_workspace {
        let workspace_manifest_rel =
            workspace_manifest
                .strip_prefix(prefix)
                .map_err(|_| CliError::StripPrefix {
                    path: workspace_manifest.clone(),
                    prefix: prefix.to_path_buf(),
                })?;
        debug!("Including workspace root manifest: {}", workspace_manifest);
        files.insert(
            workspace_manifest_rel.to_string(),
            workspace_manifest.clone(),
        );
    }

    Ok(())
}

fn add_lock_file_if_requested(
    files: &mut HashMap<String, Utf8PathBuf>,
    args: &VerifyArgs,
    prefix: &Utf8Path,
) -> Result<(), CliError> {
    if args.lock_file {
        let lock_file_path = args.path.root_dir().join("Scarb.lock");
        if lock_file_path.exists() {
            let lock_file_rel =
                lock_file_path
                    .strip_prefix(prefix)
                    .map_err(|_| CliError::StripPrefix {
                        path: lock_file_path.clone(),
                        prefix: prefix.to_path_buf(),
                    })?;
            debug!("Including Scarb.lock file: {}", lock_file_path);
            files.insert(lock_file_rel.to_string(), lock_file_path.clone());
        } else {
            warn!(
                "--lock-file flag enabled but Scarb.lock not found at {}",
                lock_file_path
            );
        }
    }
    Ok(())
}

fn find_contract_file(
    package_meta: &PackageMetadata,
    sources: &[Utf8PathBuf],
) -> Result<Utf8PathBuf, CliError> {
    // Find the main source file for the package (conventionally src/lib.cairo or src/main.cairo)
    let possible_main_paths = vec!["src/lib.cairo", "src/main.cairo"];

    for path in possible_main_paths {
        let full_path = package_meta.root.join(path);
        if full_path.exists() {
            return Ok(full_path);
        }
    }

    // If we can't find a main file, use the first source file in the package
    let contract_file_path = sources
        .iter()
        .filter(|path| path.starts_with(&package_meta.root))
        .find(|path| path.extension() == Some("cairo"))
        .cloned()
        .ok_or(CliError::NoTarget)?;

    Ok(contract_file_path)
}

fn prepare_project_dir_path(args: &VerifyArgs, prefix: &Utf8Path) -> Result<String, CliError> {
    let project_dir_path = args
        .path
        .root_dir()
        .strip_prefix(prefix)
        .map_err(|_| CliError::StripPrefix {
            path: args.path.root_dir().clone(),
            prefix: prefix.to_path_buf(),
        })
        // backend expects this for cwd
        .map(|p| {
            if p == Utf8Path::new("") {
                Utf8Path::new(".")
            } else {
                p
            }
        })?;

    Ok(project_dir_path.to_string())
}

fn convert_to_file_info(files: HashMap<String, Utf8PathBuf>) -> Vec<FileInfo> {
    files
        .into_iter()
        .map(|(name, path)| FileInfo {
            name,
            path: path.into_std_path_buf(),
        })
        .collect_vec()
}

fn log_verification_info(
    args: &VerifyArgs,
    metadata: &scarb_metadata::Metadata,
    file_infos: &[FileInfo],
    contract_file: &str,
    license_info: &license::LicenseInfo,
) {
    let cairo_version = &metadata.app_version_info.cairo.version;
    let scarb_version = &metadata.app_version_info.version;

    info!(
        "Verifying contract: {} from {}",
        args.contract_name, contract_file
    );
    info!("licensed with: {}", license_info.display_string());
    info!("using cairo: {cairo_version} and scarb {scarb_version}");
    info!("These are the files that will be used for verification:");
    for file_info in file_infos {
        info!("{}", file_info.path.display());
    }
}

fn execute_verification(
    public: &ApiClient,
    args: &VerifyArgs,
    file_infos: Vec<FileInfo>,
    package_meta: PackageMetadata,
    contract_file: String,
    project_dir_path: String,
    license_info: &license::LicenseInfo,
) -> Result<String, CliError> {
    let metadata = args.path.metadata();
    let cairo_version = metadata.app_version_info.cairo.version.clone();
    let scarb_version = metadata.app_version_info.version.clone();

    let project_meta = ProjectMetadataInfo {
        cairo_version,
        scarb_version,
        contract_file,
        project_dir_path,
        package_name: package_meta.name,
    };

    public
        .verify_class(
            &args.class_hash,
            Some(license_info.display_string().to_string()),
            &args.contract_name,
            project_meta,
            &file_infos,
        )
        .map_err(CliError::from)
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
            println!("\nThe contract is now verified and visible on Voyager at https://voyager.online/class/{} .", status.class_hash());
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
