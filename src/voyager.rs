use camino::Utf8PathBuf;
use scarb_metadata::{Metadata, PackageId};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};
use thiserror::Error;

pub type ContractMap = HashMap<String, Voyager>;

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
pub struct Voyager {
    pub path: PathBuf,
    pub address: Option<String>,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Deserialization(#[from] serde_json::Error),
}

// Use this instead of metadata.runtime_manifest, because of:
// https://docs.rs/scarb-metadata/latest/scarb_metadata/struct.Metadata.html#compatibility
// > With very old Scarb versions (<0.5.0), this field may end up being
// > empty path upon deserializing from scarb metadata call. In this
// >  case, fall back to WorkspaceMetadata.manifest field value.
// but I've actually got this in scarb 0.5.1, so...
#[must_use]
pub fn manifest_path(metadata: &Metadata) -> &Utf8PathBuf {
    if metadata.runtime_manifest == Utf8PathBuf::new() {
        &metadata.workspace.manifest_path
    } else {
        &metadata.runtime_manifest
    }
}

/// # Errors
///
/// Will return `Err` if `tool.voyager` section can't be deserialized.
pub fn tool_section(metadata: &Metadata) -> Result<HashMap<PackageId, ContractMap>, Error> {
    let mut voyager: HashMap<PackageId, ContractMap> = HashMap::new();
    for package in &metadata.packages {
        if !metadata.workspace.members.contains(&package.id) {
            continue;
        }

        if let Some(tool) = package.tool_metadata("voyager") {
            let contracts =
                serde_json::from_value::<ContractMap>(tool.clone()).map_err(Error::from)?;
            voyager.insert(package.id.clone(), contracts);
        }
    }
    Ok(voyager)
}
