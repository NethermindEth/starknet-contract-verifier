use reqwest::StatusCode;
use scarb_metadata::{Metadata, PackageId};
use std::fmt::{self, Formatter};
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Error)]
pub enum PackageIdentifier {
    Id(PackageId),
    Name(String),
}

impl fmt::Display for PackageIdentifier {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            PackageIdentifier::Id(p) => p.fmt(formatter),
            PackageIdentifier::Name(n) => n.fmt(formatter),
        }
    }
}

#[derive(Debug, Error)]
pub struct MissingPackage {
    pub package_id: PackageIdentifier,
    pub available: Vec<PackageId>,
}

impl MissingPackage {
    #[must_use]
    pub fn new(package_id: &PackageIdentifier, metadata: &Metadata) -> Self {
        Self {
            package_id: package_id.clone(),
            available: metadata.workspace.members.clone(),
        }
    }
}

impl fmt::Display for MissingPackage {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        writeln!(
            formatter,
            "Couldn't find package: {}, workspace have those packages available:",
            self.package_id
        )?;

        for package in &self.available {
            writeln!(formatter, "{package}")?;
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub struct RequestFailure {
    pub url: Url,
    pub status: StatusCode,
    pub msg: String,
}

impl RequestFailure {
    pub fn new(url: Url, status: StatusCode, msg: impl Into<String>) -> Self {
        Self {
            url,
            status,
            msg: msg.into(),
        }
    }
}

impl fmt::Display for RequestFailure {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(
            formatter,
            "{:?}\n returned {}, with:\n{}",
            self.url, self.status, self.msg
        )
    }
}

// TODO: Display suggestions
#[derive(Debug, Error)]
pub struct MissingContract {
    pub name: String,
    pub available: Vec<String>,
}

impl MissingContract {
    #[must_use]
    pub fn new(name: String, available: Vec<String>) -> Self {
        Self { name, available }
    }
}

impl fmt::Display for MissingContract {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        let contracts = self.available.join(", ");
        write!(
            formatter,
            "Contract: {} is not defined in the manifest file. Did you mean one of: {}?",
            self.name, contracts
        )
    }
}

#[derive(Debug, Error)]
pub struct NoPackageSelected {
    pub suggestions: Vec<PackageId>,
}

impl NoPackageSelected {
    #[must_use]
    pub fn new(metadata: &Metadata) -> Self {
        Self {
            suggestions: metadata.workspace.members.clone(),
        }
    }
}

impl fmt::Display for NoPackageSelected {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        writeln!(
            formatter,
            "Multiple packages found and no --package was selected. Workspace have those packages available:",
        )?;

        for package in &self.suggestions {
            writeln!(formatter, "{package}")?;
        }

        Ok(())
    }
}
