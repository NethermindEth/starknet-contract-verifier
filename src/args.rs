use camino::Utf8PathBuf;
use reqwest::Url;
use scarb_metadata::{Metadata, MetadataCommand, MetadataCommandError};
use spdx::LicenseId;
use std::{env, fmt::Display, io, path::PathBuf};
use thiserror::Error;

use verifier::class_hash::ClassHash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project(Metadata);

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("{0} doesn't contain Scarb project manifest")]
    MissingManifest(Utf8PathBuf),

    #[error("scarb metadata command failed")]
    MetadataError(#[from] MetadataCommandError),

    #[error("IO error")]
    Io(#[from] io::Error),

    #[error("UTF-8 error")]
    Utf8(#[from] camino::FromPathBufError),
}

#[allow(dead_code)]
impl Project {
    pub fn new(manifest: &Utf8PathBuf) -> Result<Self, ProjectError> {
        manifest.try_exists().map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => ProjectError::MissingManifest(manifest.clone()),
            _ => ProjectError::from(err),
        })?;

        let root = manifest.parent().ok_or(ProjectError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "Couldn't get parent directory of Scarb manifest file",
        )))?;

        let metadata = MetadataCommand::new()
            .json()
            .manifest_path(manifest)
            .current_dir(root)
            .exec()?;

        Ok(Project(metadata))
    }

    pub fn manifest_path(&self) -> &Utf8PathBuf {
        &self.0.workspace.manifest_path
    }

    pub fn root_dir(&self) -> &Utf8PathBuf {
        &self.0.workspace.root
    }

    pub fn metadata(&self) -> &Metadata {
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
#[command(name = "Starknet Contract Verifier")]
#[command(author = "Nethermind")]
#[command(version = "0.1.0")]
#[command(about = "Verify Starknet classes on Voyager block explorer")]
#[command(long_about = "")]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,

    /// Network to verify on
    #[arg(long, value_enum)]
    pub network: NetworkKind,

    #[command(flatten)]
    pub network_url: Network,
}

#[derive(clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Submit smart contract for verification.
    ///
    /// By default it will only report back to user what it is about
    /// to do. In order to actually execute pass --execute flag.
    Submit(SubmitArgs),

    /// Check verification job status
    Status {
        /// Verification job id
        #[arg(long, value_name = "UUID")]
        job: String,
    },
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

#[derive(clap::Args)]
pub struct SubmitArgs {
    /// Submit contract for verification.
    #[arg(short = 'x', long, default_value_t = false)]
    pub execute: bool,

    /// Path to Scarb project
    #[arg(
        long,
        value_name = "DIR",
        value_hint = clap::ValueHint::DirPath,
        value_parser = project_value_parser,
        default_value = env::current_dir().unwrap().into_os_string()
    )]
    pub path: Project,

    /// Class HASH to verify
    #[arg(
        long,
        value_name = "HASH",
        value_parser = ClassHash::new
    )]
    pub hash: ClassHash,

    /// Wait indefinitely for verification result
    #[arg(long, default_value_t = false)]
    pub watch: bool,

    /// SPDX license identifier
    #[arg(
        long,
        value_name = "SPDX",
        value_parser = license_value_parser,
    )]
    pub license: Option<LicenseId>,

    /// Select contract for submission
    #[arg(long, value_name = "NAME")]
    pub contract: Option<String>,
}

#[derive(clap::ValueEnum, Clone)]
pub enum NetworkKind {
    /// Target the Mainnet
    Mainnet,

    /// Target Sepolia testnet
    Sepolia,

    /// Target custom network
    Custom,
}

#[derive(Clone)]
pub struct Network {
    /// Custom public API address
    pub public: Url,

    /// Custom interval API address
    pub private: Url,
}

impl clap::FromArgMatches for Network {
    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self, clap::Error> {
        Ok(Self {
            public: matches
                // this cast is possible because we set value_parser
                .get_one::<Url>("public")
                // This should never panic because of the default_value
                // and required_if_eq used in the clap::Args
                // implementation for Network
                .expect("Custom network API public Url is missig!")
                .clone(),
            private: matches
                // this cast is possible because we set value_parser
                .get_one::<Url>("private")
                // This should never panic because of the default_value
                // and required_if_eq used in the clap::Args
                // implementation for Network
                .expect("Custom network API private Url is missig!")
                .clone(),
        })
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
        self.public = matches
            // this cast is possible because we set value_parser
            .get_one::<Url>("private")
            // This should never panic because of the default_value
            // and required_if_eq used in the clap::Args
            // implementation for Network
            .expect("Custom network API private URL is missig!")
            .clone();
        self.private = matches
            // this cast is possible because we set value_parser
            .get_one::<Url>("private")
            // This should never panic because of the default_value
            // and required_if_eq used in the clap::Args
            // implementation for Network
            .expect("Custom network API private URL is missig!")
            .clone();
        Ok(())
    }
}

// Can't derive the default value logic, hence hand rolled instance
impl clap::Args for Network {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            clap::Arg::new("public")
                .long("public")
                .help("Custom public API address")
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
                .required_if_eq("network", "custom"),
            // this would overwrite the defaults in _all_ the cases
            // .env("CUSTOM_PUBLIC_API_ENDPOINT_URL"),
        )
        .arg(
            clap::Arg::new("private")
                .long("private")
                .help("Custom interval API address")
                .value_hint(clap::ValueHint::Url)
                .value_parser(Url::parse)
                .default_value_ifs([
                    ("network", "mainnet", "https://voyager.online"),
                    ("network", "sepolia", "https://sepolia.voyager.online"),
                ])
                .required_if_eq("network", "custom"),
            // this would overwrite the defaults in _all_ the cases
            // .env("CUSTOM_INTERNAL_API_ENDPOINT_URL"),
        )
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        cmd.arg(
            clap::Arg::new("public")
                .long("public")
                .help("Custom public API address")
                .value_hint(clap::ValueHint::Url)
                .default_value_ifs([
                    ("network", "mainnet", "https://api.voyager.online/beta"),
                    (
                        "network",
                        "sepolia",
                        "https://sepolia-api.voyager.online/beta",
                    ),
                ])
                .required_if_eq("network", "custom"),
            // this would overwrite the defaults in _all_ the cases
            // .env("CUSTOM_PUBLIC_API_ENDPOINT_URL"),
        )
        .arg(
            clap::Arg::new("private")
                .long("private")
                .help("Custom interval API address")
                .value_hint(clap::ValueHint::Url)
                .default_value_ifs([
                    ("network", "mainnet", "https://api.voyager.online"),
                    ("network", "sepolia", "https://sepolia-api.voyager.online"),
                ])
                .required_if_eq("network", "custom"),
            // this would overwrite the defaults in _all_ the cases
            // .env("CUSTOM_INTERNAL_API_ENDPOINT_URL"),
        )
    }
}
