mod args;
use crate::args::{Args, Commands, VerifyArgs};

use camino::{Utf8Path, Utf8PathBuf};
use chrono::{DateTime, Utc};
use clap::Parser;
use colored::*;
use dialoguer::Select;
use itertools::Itertools;
use log::{debug, info, warn};
use scarb_metadata::PackageMetadata;
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, UNIX_EPOCH};
use thiserror::Error;
use verifier::{
    api::{
        poll_verification_status, ApiClient, ApiClientError, FileInfo, ProjectMetadataInfo,
        VerificationJob, VerifyJobStatus,
    },
    class_hash::ClassHash,
    errors, license,
    project::ProjectType,
    resolver, voyager,
};

#[derive(Debug)]
struct VerificationContext {
    project_type: ProjectType,
    project_dir_path: String,
    contract_file: String,
    package_meta: PackageMetadata,
    file_infos: Vec<FileInfo>,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Args(#[from] crate::args::ProjectError),

    #[error(transparent)]
    Api(#[from] ApiClientError),

    #[error(transparent)]
    MissingPackage(#[from] errors::MissingPackage),

    #[error("[E015] Class hash '{0}' is not declared\n\nSuggestions:\n  • Verify the class hash is correct\n  • Check that the contract has been declared on the network\n  • Ensure you're using the correct network (mainnet/testnet)\n  • Use a block explorer to verify the class hash exists")]
    NotDeclared(ClassHash),

    #[error("[E016] No contracts selected for verification\n\nSuggestions:\n  • Use --contract-name <name> to specify a contract\n  • Check that contracts are defined in [tool.voyager] section\n  • Verify your Scarb.toml contains contract definitions\n  • Use 'scarb metadata' to list available contracts")]
    NoTarget,

    #[error("[E017] Multiple contracts found - only single contract verification is supported\n\nSuggestions:\n  • Use --contract-name <name> to specify which contract to verify\n  • Choose one from the available contracts\n  • Verify each contract separately")]
    MultipleContracts,

    #[error(transparent)]
    MissingContract(#[from] errors::MissingContract),

    #[error(transparent)]
    Resolver(#[from] resolver::Error),

    #[error("[E018] Path processing error: cannot strip '{prefix}' from '{path}'\n\nThis is an internal error. Please report this issue with:\n  • The full command you ran\n  • Your project structure\n  • The contents of your Scarb.toml")]
    StripPrefix {
        path: Utf8PathBuf,
        prefix: Utf8PathBuf,
    },

    #[error(transparent)]
    Utf8(#[from] camino::FromPathBufError),

    #[error(transparent)]
    Voyager(#[from] voyager::Error),

    #[error("[E019] File '{path}' exceeds maximum size limit of {max_size} bytes (actual: {actual_size} bytes)\n\nSuggestions:\n  • Reduce the file size by removing unnecessary content\n  • Split large files into smaller modules\n  • Check if the file contains generated or temporary content\n  • Use .gitignore to exclude large files that shouldn't be verified")]
    FileSizeLimit {
        path: Utf8PathBuf,
        max_size: usize,
        actual_size: usize,
    },

    #[error("[E024] File '{path}' has invalid file type (extension: {extension})\n\nSuggestions:\n  • Only include Cairo source files (.cairo)\n  • Include project configuration files (.toml, .lock)\n  • Include documentation files (.md, .txt)\n  • Remove binary or executable files from the project\n  • Allowed extensions: .cairo, .toml, .lock, .md, .txt, .json")]
    InvalidFileType {
        path: Utf8PathBuf,
        extension: String,
    },

    #[error("[E025] Invalid project type specified\n\nSpecified: {specified}\nDetected: {detected}\n\nSuggestions:\n{}", suggestions.join("\n  • "))]
    InvalidProjectType {
        specified: String,
        detected: String,
        suggestions: Vec<String>,
    },

    #[error("[E026] Dojo project validation failed\n\nSuggestions:\n  • Ensure dojo-core is listed in dependencies\n  • Check that Scarb.toml is properly configured for Dojo\n  • Verify project structure follows Dojo conventions\n  • Run 'sozo build' to test project compilation")]
    DojoValidationFailed,

    #[error("[E027] Interactive prompt failed\n\nSuggestions:\n  • Use --project-type=scarb or --project-type=dojo to skip prompt\n  • Ensure terminal supports interactive input\n  • Check that stdin is available")]
    InteractivePromptFailed(#[from] dialoguer::Error),
}

impl CliError {
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::Args(_) => "E020",
            Self::Api(e) => e.error_code(),
            Self::MissingPackage(e) => e.error_code().as_str(),
            Self::NotDeclared(_) => "E015",
            Self::NoTarget => "E016",
            Self::MultipleContracts => "E017",
            Self::MissingContract(e) => e.error_code().as_str(),
            Self::Resolver(e) => e.error_code(),
            Self::StripPrefix { .. } => "E018",
            Self::Utf8(_) => "E023",
            Self::Voyager(_) => "E999",
            Self::FileSizeLimit { .. } => "E019",
            Self::InvalidFileType { .. } => "E024",
            Self::InvalidProjectType { .. } => "E025",
            Self::DojoValidationFailed => "E026",
            Self::InteractivePromptFailed(_) => "E027",
        }
    }
}

fn display_verification_job_id(job_id: &str) {
    println!();
    println!("verification job id: {}", job_id.green().bold());
    println!();
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let Args { command: cmd } = Args::parse();

    match &cmd {
        Commands::Verify(args) => {
            let api_client = ApiClient::new(args.network_url.url.clone())?;

            let license_info = license::resolve_license_info(
                args.license,
                args.path.get_license(),
                args.path.manifest_path(),
            );

            license::warn_if_no_license(&license_info);

            let job_id = submit(&api_client, args, &license_info).map_err(|e| {
                if let CliError::Api(ApiClientError::Verify(ref verification_error)) = e {
                    eprintln!("\nSuggestions:");
                    for suggestion in verification_error.suggestions() {
                        eprintln!("  • {suggestion}");
                    }
                } else if let CliError::Api(ApiClientError::Failure(ref _request_failure)) = e {
                    // RequestFailure errors already include suggestions in their display
                }
                e
            })?;
            if job_id != "dry-run" {
                display_verification_job_id(&job_id);

                // If --watch flag is enabled, poll for verification result
                if args.watch {
                    let status = check(&api_client, &job_id).map_err(|e| {
                        if let CliError::Api(ApiClientError::Verify(ref verification_error)) = e {
                            eprintln!("\nSuggestions:");
                            for suggestion in verification_error.suggestions() {
                                eprintln!("  • {suggestion}");
                            }
                        } else if let CliError::Api(ApiClientError::Failure(ref _request_failure)) =
                            e
                        {
                            // RequestFailure errors already include suggestions in their display
                        }
                        e
                    })?;
                    info!("{status:?}");
                }
            }
        }
        Commands::Status(args) => {
            let api_client = ApiClient::new(args.network_url.url.clone())?;
            let status = check(&api_client, &args.job).map_err(|e| {
                if let CliError::Api(ApiClientError::Verify(ref verification_error)) = e {
                    eprintln!("\nSuggestions:");
                    for suggestion in verification_error.suggestions() {
                        eprintln!("  • {suggestion}");
                    }
                } else if let CliError::Api(ApiClientError::Failure(ref _request_failure)) = e {
                    // RequestFailure errors already include suggestions in their display
                }
                e
            })?;
            info!("{status:?}");
        }
    }
    Ok(())
}

fn submit(
    api_client: &ApiClient,
    args: &VerifyArgs,
    license_info: &license::LicenseInfo,
) -> Result<String, CliError> {
    info!("🚀 Starting verification for project at: {}", args.path);

    // Determine project type early in the process
    let project_type = determine_project_type(args)?;

    // Log the selected build tool
    match project_type {
        ProjectType::Dojo => info!("Using sozo build for Dojo project"),
        ProjectType::Scarb => info!("Using scarb build for Scarb project"),
        ProjectType::Auto => unreachable!("Auto should be resolved by now"),
    }

    let metadata = args.path.metadata();

    // Determine test_files setting - default to true for Dojo projects
    let include_test_files = match project_type {
        ProjectType::Dojo => {
            if !args.test_files {
                info!("🧪 Including test files by default for Dojo project");
            }
            true
        }
        _ => args.test_files,
    };

    // Gather packages and sources
    let packages = gather_packages_and_validate(metadata, args)?;
    let sources = collect_source_files(metadata, &packages, include_test_files)?;

    // Prepare project structure
    let (file_infos, package_meta, contract_file, project_dir_path) =
        prepare_project_for_verification(args, metadata, &packages, sources)?;

    // Log verification info
    log_verification_info(args, metadata, &file_infos, &contract_file, license_info);

    // Execute verification unless dry run is requested
    if !args.dry_run {
        let context = VerificationContext {
            project_type,
            project_dir_path,
            contract_file,
            package_meta,
            file_infos,
        };
        return execute_verification(api_client, args, context, license_info);
    }

    info!("Dry run mode: collected files for verification but skipping submission due to --dry-run flag");
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
    include_test_files: bool,
) -> Result<Vec<Utf8PathBuf>, CliError> {
    let mut sources: Vec<Utf8PathBuf> = vec![];
    for package in packages {
        let mut package_sources =
            resolver::package_sources_with_test_files(package, include_test_files)?;
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
    let contract_file_path = find_contract_file(package_meta, &sources, &args.contract_name)?;
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

    // Validate file sizes
    validate_file_sizes(&files)?;

    Ok(files)
}

fn validate_file_sizes(files: &HashMap<String, Utf8PathBuf>) -> Result<(), CliError> {
    const MAX_FILE_SIZE: usize = 1024 * 1024 * 20; // 20MB limit

    for path in files.values() {
        // Validate file type
        validate_file_type(path)?;

        // Validate file size
        if let Ok(metadata) = std::fs::metadata(path) {
            let size = metadata.len() as usize;
            if size > MAX_FILE_SIZE {
                return Err(CliError::FileSizeLimit {
                    path: path.clone(),
                    max_size: MAX_FILE_SIZE,
                    actual_size: size,
                });
            }
        }
    }
    Ok(())
}

fn validate_file_type(path: &Utf8PathBuf) -> Result<(), CliError> {
    // Get file extension
    let extension = path.extension().unwrap_or("");

    // Define allowed file types
    let allowed_extensions = ["cairo", "toml", "lock", "md", "txt", "json"];

    // Define common project files without extensions
    let allowed_no_extension_files = [
        "LICENSE",
        "README",
        "CHANGELOG",
        "NOTICE",
        "AUTHORS",
        "CONTRIBUTORS",
    ];

    // Check if extension is allowed
    if !allowed_extensions.contains(&extension) {
        // If no extension, check if it's a common project file
        if extension.is_empty() {
            let file_name = path.file_name().unwrap_or("");
            if !allowed_no_extension_files.contains(&file_name) {
                return Err(CliError::InvalidFileType {
                    path: path.clone(),
                    extension: extension.to_string(),
                });
            }
        } else {
            return Err(CliError::InvalidFileType {
                path: path.clone(),
                extension: extension.to_string(),
            });
        }
    }

    Ok(())
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
        debug!("Including workspace root manifest: {workspace_manifest}");
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
            debug!("Including Scarb.lock file: {lock_file_path}");
            files.insert(lock_file_rel.to_string(), lock_file_path.clone());
        } else {
            warn!("--lock-file flag enabled but Scarb.lock not found at {lock_file_path}");
        }
    }
    Ok(())
}

fn find_contract_file(
    package_meta: &PackageMetadata,
    sources: &[Utf8PathBuf],
    contract_name: &str,
) -> Result<Utf8PathBuf, CliError> {
    // First try to find a file that matches the contract name
    let contract_specific_paths = vec![
        format!("src/{}.cairo", contract_name),
        format!("src/systems/{}.cairo", contract_name),
        format!("src/contracts/{}.cairo", contract_name),
    ];

    for path in contract_specific_paths {
        let full_path = package_meta.root.join(&path);
        if full_path.exists() {
            return Ok(full_path);
        }
    }

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
    api_client: &ApiClient,
    args: &VerifyArgs,
    context: VerificationContext,
    license_info: &license::LicenseInfo,
) -> Result<String, CliError> {
    let metadata = args.path.metadata();
    let cairo_version = metadata.app_version_info.cairo.version.clone();
    let scarb_version = metadata.app_version_info.version.clone();

    // Create project metadata with build tool information
    debug!(
        "Creating ProjectMetadataInfo with project_type: {:?}",
        context.project_type
    );

    // Extract Dojo version if it's a Dojo project
    let dojo_version = if context.project_type == ProjectType::Dojo {
        info!("🔍 Dojo project detected - attempting to extract Dojo version from Scarb.toml");
        debug!(
            "📍 context.project_dir_path (relative): {}",
            context.project_dir_path
        );
        debug!(
            "📍 args.path.root_dir() (absolute): {}",
            args.path.root_dir()
        );

        // Use the absolute path for Dojo version extraction
        let absolute_project_path = args.path.root_dir().to_string();
        let extracted_version = extract_dojo_version(&absolute_project_path);
        match &extracted_version {
            Some(version) => info!("✅ Successfully extracted Dojo version: {version}"),
            None => warn!(
                "⚠️  Could not extract Dojo version from Scarb.toml - proceeding without version"
            ),
        }
        extracted_version
    } else {
        debug!("📦 Regular project (not Dojo) - skipping Dojo version extraction");
        None
    };

    let project_meta = ProjectMetadataInfo::new(
        cairo_version,
        scarb_version,
        context.project_dir_path,
        context.contract_file,
        context.package_meta.name,
        context.project_type,
        dojo_version,
    );
    debug!(
        "Created ProjectMetadataInfo with build_tool: {}, dojo_version: {:?}",
        project_meta.build_tool, project_meta.dojo_version
    );

    api_client
        .verify_class(
            &args.class_hash,
            Some(license_info.display_string().to_string()),
            &args.contract_name,
            project_meta,
            &context.file_infos,
        )
        .map_err(CliError::from)
}

fn extract_dojo_version(project_dir_path: &str) -> Option<String> {
    let scarb_toml_path = format!("{project_dir_path}/Scarb.toml");
    debug!("📁 Looking for Scarb.toml at: {scarb_toml_path}");

    // Read the Scarb.toml file
    let contents = match fs::read_to_string(&scarb_toml_path) {
        Ok(contents) => {
            debug!("📖 Successfully read Scarb.toml ({} bytes)", contents.len());
            contents
        }
        Err(e) => {
            warn!("❌ Failed to read Scarb.toml at {scarb_toml_path}: {e}");
            return None;
        }
    };

    // Parse the TOML content
    let parsed: toml::Value = match toml::from_str(&contents) {
        Ok(parsed) => {
            debug!("✅ Successfully parsed Scarb.toml as TOML");
            parsed
        }
        Err(e) => {
            warn!("❌ Failed to parse Scarb.toml: {e}");
            return None;
        }
    };

    // Navigate to dependencies.dojo.tag
    debug!("🔎 Searching for dependencies.dojo.tag in Scarb.toml");
    if let Some(dependencies) = parsed.get("dependencies") {
        debug!("✅ Found [dependencies] section");
        if let Some(dojo_dep) = dependencies.get("dojo") {
            debug!("✅ Found dojo dependency: {dojo_dep:?}");
            if let Some(tag) = dojo_dep.get("tag") {
                debug!("✅ Found tag field: {tag:?}");
                if let Some(tag_str) = tag.as_str() {
                    info!("🎯 Successfully extracted Dojo version from tag: {tag_str}");
                    return Some(tag_str.to_string());
                } else {
                    warn!("⚠️  Tag field exists but is not a string: {tag:?}");
                }
            } else {
                warn!("⚠️  Dojo dependency found but no 'tag' field");
            }
        } else {
            warn!("⚠️  Dependencies section found but no 'dojo' dependency");
        }
    } else {
        warn!("⚠️  No [dependencies] section found in Scarb.toml");
    }

    info!("❌ No Dojo version found in Scarb.toml");
    None
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

fn check(api_client: &ApiClient, job_id: &str) -> Result<VerificationJob, CliError> {
    let status = poll_verification_status(api_client, job_id).map_err(CliError::from)?;

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
                println!("Cairo version: {version}");
            }
            if let Some(dojo_version) = status.dojo_version() {
                println!("Dojo version: {dojo_version}");
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

fn determine_project_type(args: &VerifyArgs) -> Result<ProjectType, CliError> {
    match args.project_type {
        ProjectType::Scarb => Ok(ProjectType::Scarb),
        ProjectType::Dojo => {
            // Validate that this is actually a Dojo project
            validate_dojo_project(&args.path)?;
            Ok(ProjectType::Dojo)
        }
        ProjectType::Auto => {
            // Try automatic detection first
            match args.path.detect_project_type()? {
                ProjectType::Dojo => {
                    info!("Detected Dojo project automatically");
                    Ok(ProjectType::Dojo)
                }
                ProjectType::Scarb => {
                    info!("Detected Scarb project automatically");
                    Ok(ProjectType::Scarb)
                }
                ProjectType::Auto => {
                    // Fallback to interactive prompt
                    let options = vec![
                        "Regular Scarb project (uses scarb build)",
                        "Dojo project (uses sozo build)",
                    ];

                    let selection = Select::new()
                        .with_prompt("What type of project are you verifying?")
                        .items(&options)
                        .default(0)
                        .interact()?;

                    match selection {
                        0 => Ok(ProjectType::Scarb),
                        1 => {
                            validate_dojo_project(&args.path)?;
                            Ok(ProjectType::Dojo)
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

fn validate_dojo_project(project: &args::Project) -> Result<(), CliError> {
    // Check if sozo is available (optional warning)
    if std::process::Command::new("sozo")
        .arg("--version")
        .output()
        .is_err()
    {
        warn!("sozo command not found. Dojo project verification will be handled remotely.");
    }

    // Validate project has Dojo dependencies
    if project.detect_project_type()? != ProjectType::Dojo {
        return Err(CliError::InvalidProjectType {
            specified: "dojo".to_string(),
            detected: "scarb".to_string(),
            suggestions: vec![
                "Add dojo-core dependency to Scarb.toml".to_string(),
                "Use --project-type=scarb for regular Scarb projects".to_string(),
            ],
        });
    }

    Ok(())
}
