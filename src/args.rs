use camino::Utf8PathBuf;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Url;
use scarb_metadata::{Metadata, MetadataCommand, MetadataCommandError};
use spdx::LicenseId;
use std::{env, fmt::Display, io, path::PathBuf};
use thiserror::Error;

use verifier::class_hash::ClassHash;

fn get_name_validation_regex() -> Result<&'static Regex, String> {
    lazy_static! {
        static ref VALID_NAME_REGEX: Result<Regex, regex::Error> = Regex::new(r"^[a-zA-Z0-9_-]+$");
    }

    match VALID_NAME_REGEX.as_ref() {
        Ok(regex) => Ok(regex),
        Err(_) => Err("Internal regex compilation error".to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project(Metadata);

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("[E020] Scarb project manifest not found at: {0}\n\nSuggestions:\n  • Check that you're in a Scarb project directory\n  • Verify that Scarb.toml exists in the specified path\n  • Run 'scarb init' to create a new project\n  • Use --manifest-path to specify the correct path")]
    MissingManifest(Utf8PathBuf),

    #[error("[E021] Failed to read project metadata\n\nSuggestions:\n  • Check that Scarb.toml is valid TOML format\n  • Verify all dependencies are properly declared\n  • Run 'scarb check' to validate your project\n  • Ensure scarb is installed and up to date")]
    MetadataError(#[from] MetadataCommandError),

    #[error("[E022] File system error\n\nSuggestions:\n  • Check file permissions\n  • Verify the path exists and is accessible\n  • Ensure you have read access to the directory")]
    Io(#[from] io::Error),

    #[error("[E023] Path contains invalid UTF-8 characters\n\nSuggestions:\n  • Use only ASCII characters in file paths\n  • Avoid special characters in directory names\n  • Check for hidden or control characters in the path")]
    Utf8(#[from] camino::FromPathBufError),
}

impl ProjectError {
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::MissingManifest(_) => "E020",
            Self::MetadataError(_) => "E021",
            Self::Io(_) => "E022",
            Self::Utf8(_) => "E023",
        }
    }
}

#[allow(dead_code)]
impl Project {
    pub fn new(manifest: &Utf8PathBuf) -> Result<Self, ProjectError> {
        manifest.try_exists().map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => ProjectError::MissingManifest(manifest.clone()),
            _ => ProjectError::from(err),
        })?;

        let root = manifest.parent().ok_or_else(|| {
            ProjectError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "Couldn't get parent directory of Scarb manifest file",
            ))
        })?;

        let metadata = MetadataCommand::new()
            .json()
            .manifest_path(manifest)
            .current_dir(root)
            .exec()?;

        Ok(Self(metadata))
    }

    pub const fn manifest_path(&self) -> &Utf8PathBuf {
        &self.0.workspace.manifest_path
    }

    pub const fn root_dir(&self) -> &Utf8PathBuf {
        &self.0.workspace.root
    }

    pub const fn metadata(&self) -> &Metadata {
        &self.0
    }

    pub fn get_license(&self) -> Option<LicenseId> {
        self.0.packages.first().and_then(|pkg| {
            pkg.manifest_metadata
                .license
                .as_ref()
                .and_then(|license_str| {
                    // Handle common SPDX identifiers directly
                    match license_str.as_str() {
                        "MIT" => spdx::license_id("MIT License"),
                        "Apache-2.0" => spdx::license_id("Apache License 2.0"),
                        "GPL-3.0" => spdx::license_id("GNU General Public License v3.0 only"),
                        "BSD-3-Clause" => spdx::license_id("BSD 3-Clause License"),
                        // Try exact match
                        _ => spdx::license_id(license_str).or_else(|| {
                            // Try imprecise matching
                            spdx::imprecise_license_id(license_str).map(|(lic, _)| lic)
                        }),
                    }
                })
        })
    }
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.manifest_path())
    }
}

pub fn project_value_parser(raw: &str) -> Result<Project, ProjectError> {
    let path = PathBuf::from(raw);

    let absolute = if path.is_absolute() {
        path
    } else {
        let mut cwd = env::current_dir()?;
        cwd.push(path);
        cwd
    };

    let utf8 = Utf8PathBuf::try_from(absolute)?;

    let manifest = if utf8.is_file() {
        utf8
    } else {
        utf8.join("Scarb.toml")
    };

    Project::new(&manifest)
}

#[derive(clap::Parser)]
#[command(name = "voyager")]
#[command(author = "Nethermind")]
#[command(version)]
#[command(about = "Verify Starknet smart contracts on block explorers")]
#[command(long_about = "
A command-line tool for verifying Starknet smart contracts on block explorers.

This tool allows you to verify that the source code of a deployed contract matches
the bytecode on the blockchain. It supports predefined networks (mainnet, sepolia)
and custom API endpoints, automatically handling project dependencies and source file collection.

Examples:
  # Verify a contract on mainnet
  voyager verify --network mainnet \\
    --class-hash 0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18 \\
    --contract-name MyContract

  # Verify using custom API endpoint
  voyager verify --url https://api.custom.com/beta \\
    --class-hash 0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18 \\
    --contract-name MyContract

  # Check verification status
  voyager status --network mainnet --job job-id-here

  # Check status using custom API
  voyager status --url https://api.custom.com/beta --job job-id-here
")]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Verify a smart contract against its deployed bytecode
    ///
    /// Submits the contract source code for verification against the deployed
    /// bytecode on the blockchain. By default submits for verification.
    /// Use --dry-run to preview what would be submitted without sending.
    ///
    /// Examples:
    ///   # Using predefined network
    ///   voyager verify --network mainnet \
    ///     --class-hash 0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18 \
    ///     --contract-name `MyContract`
    ///   
    ///   # Using custom API endpoint
    ///   voyager verify --url <https://api.custom.com/beta> \
    ///     --class-hash 0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18 \
    ///     --contract-name `MyContract`
    Verify(VerifyArgs),

    /// Check the status of a verification job
    ///
    /// Queries the verification service for the current status of a submitted
    /// verification job. The job ID is returned when you submit a verification.
    ///
    /// Examples:
    ///   # Using predefined network
    ///   voyager status --network mainnet --job 12345678-1234-1234-1234-123456789012
    ///   
    ///   # Using custom API endpoint
    ///   voyager status --url <https://api.custom.com/beta> --job 12345678-1234-1234-1234-123456789012
    Status(StatusArgs),
}

fn license_value_parser(license: &str) -> Result<LicenseId, String> {
    // First try for exact SPDX identifier match
    if let Some(id) = spdx::license_id(license) {
        return Ok(id);
    }

    // For common shorthand identifiers, try to map to the full name
    let mapped_license = match license {
        "MIT" => "MIT License",
        "Apache-2.0" => "Apache License 2.0",
        "GPL-3.0" => "GNU General Public License v3.0 only",
        "BSD-3-Clause" => "BSD 3-Clause License",
        _ => license,
    };

    // Try again with mapped name
    if let Some(id) = spdx::license_id(mapped_license) {
        return Ok(id);
    }

    // Try imprecise matching as a last resort
    if let Some((lic, _)) = spdx::imprecise_license_id(license) {
        return Ok(lic);
    }

    // Provide helpful error with suggestion if available
    let guess = spdx::imprecise_license_id(license)
        .map_or(String::new(), |(lic, _): (LicenseId, usize)| {
            format!(", do you mean: {}?", lic.name)
        });

    Err(format!("Unrecognized license: {license}{guess}"))
}

fn contract_name_value_parser(name: &str) -> Result<String, String> {
    // Check for minimum length
    if name.is_empty() {
        return Err("Contract name cannot be empty".to_string());
    }

    // Check for maximum length (reasonable limit)
    if name.len() > 100 {
        return Err("Contract name cannot exceed 100 characters".to_string());
    }

    // Check for valid characters: alphanumeric, underscore, hyphen
    let regex = get_name_validation_regex()?;
    if !regex.is_match(name) {
        return Err(
            "Contract name can only contain alphanumeric characters, underscores, and hyphens"
                .to_string(),
        );
    }

    // Check that it doesn't start with a hyphen or underscore
    if name.starts_with('-') || name.starts_with('_') {
        return Err("Contract name cannot start with a hyphen or underscore".to_string());
    }

    // Check that it doesn't end with a hyphen or underscore
    if name.ends_with('-') || name.ends_with('_') {
        return Err("Contract name cannot end with a hyphen or underscore".to_string());
    }

    // Additional security check: reject common system names
    let reserved_names = [
        "con", "aux", "prn", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ];
    if reserved_names.contains(&name.to_lowercase().as_str()) {
        return Err("Contract name cannot be a reserved system name".to_string());
    }

    Ok(name.to_string())
}

fn package_name_value_parser(name: &str) -> Result<String, String> {
    // Check for minimum length
    if name.is_empty() {
        return Err("Package name cannot be empty".to_string());
    }

    // Check for maximum length (reasonable limit)
    if name.len() > 100 {
        return Err("Package name cannot exceed 100 characters".to_string());
    }

    // Check for valid characters: alphanumeric, underscore, hyphen
    let regex = get_name_validation_regex()?;
    if !regex.is_match(name) {
        return Err(
            "Package name can only contain alphanumeric characters, underscores, and hyphens"
                .to_string(),
        );
    }

    Ok(name.to_string())
}

#[derive(clap::Args)]
pub struct VerifyArgs {
    /// Network to verify on (mainnet, sepolia). If not specified, --url is required
    #[arg(long, value_enum)]
    pub network: Option<NetworkKind>,

    #[command(flatten)]
    pub network_url: Network,

    /// Perform dry run (preview what would be submitted without sending)
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Path to Scarb project directory (default: current directory)
    #[arg(
        long,
        value_name = "DIR",
        value_hint = clap::ValueHint::DirPath,
        value_parser = project_value_parser,
        default_value = "."
    )]
    pub path: Project,

    /// Class hash of the deployed contract to verify
    #[arg(
        long = "class-hash",
        value_name = "HASH",
        value_parser = ClassHash::new
    )]
    pub class_hash: ClassHash,

    /// Wait indefinitely for verification result (polls until completion)
    #[arg(long, default_value_t = false)]
    pub watch: bool,

    /// SPDX license identifier (e.g., MIT, Apache-2.0)
    #[arg(
        long,
        value_name = "SPDX",
        value_parser = license_value_parser,
    )]
    pub license: Option<LicenseId>,

    /// Name of the contract for verification
    #[arg(
        long = "contract-name",
        value_name = "NAME",
        value_parser = contract_name_value_parser
    )]
    pub contract_name: String,

    /// Select specific package for verification (required for workspace projects)
    #[arg(
        long,
        value_name = "PACKAGE_ID",
        value_parser = package_name_value_parser
    )]
    pub package: Option<String>,

    /// Include Scarb.lock file in verification submission
    #[arg(long, default_value_t = false)]
    pub lock_file: bool,

    /// Include test files from src/ directory in verification submission
    #[arg(long, default_value_t = false)]
    pub test_files: bool,
}

#[derive(clap::Args)]
pub struct StatusArgs {
    /// Network to verify on (mainnet, sepolia). If not specified, --url is required
    #[arg(long, value_enum)]
    pub network: Option<NetworkKind>,

    #[command(flatten)]
    pub network_url: Network,

    /// Verification job ID (UUID format)
    #[arg(long, value_name = "UUID")]
    pub job: String,
}

#[derive(clap::ValueEnum, Clone)]
pub enum NetworkKind {
    /// Target the Mainnet
    Mainnet,

    /// Target Sepolia testnet
    Sepolia,
}

#[derive(Clone)]
pub struct Network {
    /// API endpoint URL
    pub url: Url,
}

impl clap::FromArgMatches for Network {
    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self, clap::Error> {
        let url = matches
            .get_one::<Url>("url")
            .ok_or_else(|| {
                clap::Error::raw(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "API URL is required when not using predefined networks",
                )
            })?
            .clone();

        Ok(Self { url })
    }

    fn from_arg_matches_mut(matches: &mut clap::ArgMatches) -> Result<Self, clap::Error> {
        Self::from_arg_matches(matches)
    }

    fn update_from_arg_matches(&mut self, matches: &clap::ArgMatches) -> Result<(), clap::Error> {
        let mut matches = matches.clone();
        self.update_from_arg_matches_mut(&mut matches)
    }

    fn update_from_arg_matches_mut(
        &mut self,
        matches: &mut clap::ArgMatches,
    ) -> Result<(), clap::Error> {
        self.url = matches
            .get_one::<Url>("url")
            .ok_or_else(|| {
                clap::Error::raw(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "API URL is required when not using predefined networks",
                )
            })?
            .clone();

        Ok(())
    }
}

// Can't derive the default value logic, hence hand rolled instance
impl clap::Args for Network {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            clap::Arg::new("url")
                .long("url")
                .help("API endpoint URL (required when --network is not specified)")
                .value_hint(clap::ValueHint::Url)
                .value_parser(Url::parse)
                .default_value_ifs([
                    ("network", "mainnet", "https://api.voyager.online/beta"),
                    (
                        "network",
                        "sepolia",
                        "https://sepolia-api.voyager.online/beta",
                    ),
                ])
                .required_unless_present("network"),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            clap::Arg::new("url")
                .long("url")
                .help("API endpoint URL (required when --network is not specified)")
                .value_hint(clap::ValueHint::Url)
                .value_parser(Url::parse)
                .default_value_ifs([
                    ("network", "mainnet", "https://api.voyager.online/beta"),
                    (
                        "network",
                        "sepolia",
                        "https://sepolia-api.voyager.online/beta",
                    ),
                ])
                .required_unless_present("network"),
        )
    }
}
