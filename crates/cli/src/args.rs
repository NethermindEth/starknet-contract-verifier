use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use reqwest::Url;
use std::{
    env,
    io,
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
                io::ErrorKind::NotFound =>
                    ProjectDirError::NoScarb(dir),
                _ =>
                    ProjectDirError::from(err),
            })
        }
    }

    pub fn new(dir: PathBuf) -> Result<Self, ProjectDirError> {
        let utf8 = Utf8PathBuf::try_from(dir)?;
        ProjectDir::find_scarb(utf8)
    }

    pub fn cwd() -> Result<ProjectDir, ProjectDirError> {
        let cwd = env::current_dir()?;
        ProjectDir::new(cwd)
    }

    // TODO: make path absolute during construction?
    pub fn make_absolute(self: Self) -> Result<Self, ProjectDirError> {
        let mut cwd = env::current_dir()?;
        cwd.push(self.0);
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

fn project_dir_value_parser(raw: &str) -> Result<ProjectDir, ProjectDirError> {
    ProjectDir::new(PathBuf::from(raw))
}

#[derive(Parser)]
#[command(name = "Starknet Contract Verifier")]
#[command(author = "Nethermind")]
#[command(version = "0.1.0")]
#[command(about = "Verify Starknet classes on Voyager block explorer")]
#[command(long_about = "")]
pub struct Args {
    /// Network to verify on
    #[command(subcommand)]
    pub network: Network,

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

    /// Valid SPDX license identifier
    #[arg(long, value_name = "SPDX")]
    pub license: Option<String>,
}

#[derive(Subcommand)]
pub enum Network {
    Mainnet,
    Testnet,
    Custom {
        /// Public Api URL
        #[arg(
            long,
            value_name = "URL",
            env = "CUSTOM_PUBLIC_API_ENDPOINT_URL",
        )]
        public: Url,

        /// Internal Api URL
        #[arg(
            long,
            value_name = "URL",
            env = "CUSTOM_INTERNAL_API_ENDPOINT_URL",
        )]
        private: Url
    }
}

