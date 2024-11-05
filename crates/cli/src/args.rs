use camino::{Utf8Path, Utf8PathBuf};
use clap;
use reqwest::Url;
use spdx::LicenseId;
use std::{
    env, io,
    path::{Path, PathBuf},
    string::ToString,
};
use thiserror::Error;

use crate::class_hash::ClassHash;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectDir(Utf8PathBuf);

impl ToString for ProjectDir {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Error, Debug)]
pub enum ProjectDirError {
    #[error("{0} doesn't contain Scarb project")]
    NoScarb(Utf8PathBuf),
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("UTF-8 error")]
    Utf8(#[from] camino::FromPathBufError),
}

impl ProjectDir {
    fn find_scarb(dir: Utf8PathBuf) -> Result<ProjectDir, ProjectDirError> {
        match dir.join("scarb.toml").try_exists() {
            Ok(_) => Ok(ProjectDir(dir)),
            Err(err) => Err(match err.kind() {
                io::ErrorKind::NotFound => ProjectDirError::NoScarb(dir),
                _ => ProjectDirError::from(err),
            }),
        }
    }

    pub fn new(dir: PathBuf) -> Result<Self, ProjectDirError> {
        let absolute = if dir.is_absolute() {
            dir
        } else {
            let mut cwd = env::current_dir()?;
            cwd.push(dir);
            cwd
        };

        let utf8 = Utf8PathBuf::try_from(absolute)?;
        ProjectDir::find_scarb(utf8)
    }

    pub fn cwd() -> Result<ProjectDir, ProjectDirError> {
        let cwd = env::current_dir()?;
        ProjectDir::new(cwd)
    }
}

impl From<ProjectDir> for Utf8PathBuf {
    fn from(value: ProjectDir) -> Self {
        value.0
    }
}

impl AsRef<Utf8Path> for ProjectDir {
    fn as_ref(&self) -> &Utf8Path {
        self.0.as_path()
    }
}

impl AsRef<Utf8PathBuf> for ProjectDir {
    fn as_ref(&self) -> &Utf8PathBuf {
        &self.0
    }
}

impl AsRef<Path> for ProjectDir {
    fn as_ref(&self) -> &Path {
        self.0.as_std_path()
    }
}

impl AsRef<str> for ProjectDir {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

pub fn project_dir_value_parser(raw: &str) -> Result<ProjectDir, ProjectDirError> {
    ProjectDir::new(PathBuf::from(raw))
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
pub enum Commands {
    /// Submit smart contract for verification
    Submit(SubmitArgs),

    /// Check verification job status
    Status {
        /// Verification job id
        #[arg(long, value_name = "UUID")]
        job: String,
    },
}

fn license_value_parser(license: &str) -> Result<LicenseId, String> {
    let id = spdx::license_id(license);
    id.ok_or({
        let guess = spdx::imprecise_license_id(license)
            .map_or(String::new(), |(lic, _): (LicenseId, usize)| {
                format!(", do you mean: {}?", lic.name)
            });
        format!("Unrecognized license: {license}{guess}")
    })
}

#[derive(clap::Args)]
pub struct SubmitArgs {
    /// Path to Scarb project root DIR
    #[arg(
        long,
        value_name = "DIR",
        value_hint = clap::ValueHint::DirPath,
        value_parser = project_dir_value_parser,
        default_value_t = ProjectDir::cwd().unwrap(),
    )]
    pub path: ProjectDir,

    /// Class HASH to verify
    #[arg(
        long,
        value_name = "HASH",
        value_parser = ClassHash::new
    )]
    pub hash: ClassHash,

    /// Desired class NAME
    #[arg(long, value_name = "NAME")]
    pub name: String,

    /// SPDX license identifier
    #[arg(
        long,
        value_name = "SPDX",
        value_parser = license_value_parser,
    )]
    pub license: Option<LicenseId>,
}

#[derive(clap::ValueEnum, Clone)]
pub enum NetworkKind {
    /// Target the Mainnet
    Mainnet,

    /// Target Sepolia testnet
    Testnet,

    /// Target custom network
    Custom,
}

#[derive(Clone)]
pub struct Network {
    /// Custom public API adress
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
                .to_owned(),
            private: matches
                // this cast is possible because we set value_parser
                .get_one::<Url>("private")
                // This should never panic because of the default_value
                // and required_if_eq used in the clap::Args
                // implementation for Network
                .expect("Custom network API private Url is missig!")
                .to_owned(),
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
            .to_owned();
        self.private = matches
            // this cast is possible because we set value_parser
            .get_one::<Url>("private")
            // This should never panic because of the default_value
            // and required_if_eq used in the clap::Args
            // implementation for Network
            .expect("Custom network API private URL is missig!")
            .to_owned();
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
                        "testnet",
                        "https://sepolia-api.voyager.online/beta",
                    ),
                ])
                .required_if_eq("network", "custom")
                .env("CUSTOM_PUBLIC_API_ENDPOINT_URL"),
        )
        .arg(
            clap::Arg::new("private")
                .long("private")
                .help("Custom interval API address")
                .value_hint(clap::ValueHint::Url)
                .value_parser(Url::parse)
                .default_value_ifs([
                    ("network", "mainnet", "https://voyager.online"),
                    ("network", "testnet", "https://sepolia.voyager.online"),
                ])
                .required_if_eq("network", "custom")
                .env("CUSTOM_INTERNAL_API_ENDPOINT_URL"),
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
                        "testnet",
                        "https://sepolia-api.voyager.online/beta",
                    ),
                ])
                .required_if_eq("network", "custom")
                .env("CUSTOM_PUBLIC_API_ENDPOINT_URL"),
        )
        .arg(
            clap::Arg::new("private")
                .long("private")
                .help("Custom interval API address")
                .value_hint(clap::ValueHint::Url)
                .default_value_ifs([
                    ("network", "mainnet", "https://api.voyager.online"),
                    ("network", "testnet", "https://sepolia-api.voyager.online"),
                ])
                .required_if_eq("network", "custom")
                .env("CUSTOM_INTERNAL_API_ENDPOINT_URL"),
        )
    }
}
