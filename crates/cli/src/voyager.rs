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
pub enum VoyagerError {
    #[error(transparent)]
    DeserializationEorror(#[from] serde_json::Error),
}

pub fn tool_section(metadata: &Metadata) -> Result<HashMap<PackageId, ContractMap>, VoyagerError> {
    let mut voyager: HashMap<PackageId, ContractMap> = HashMap::new();
    for package in &metadata.packages {
        if !metadata.workspace.members.contains(&package.id) {
            continue;
        }

        if let Some(tool) = package.tool_metadata("voyager") {
            let contracts =
                serde_json::from_value::<ContractMap>(tool.clone()).map_err(VoyagerError::from)?;
            voyager.insert(package.id.clone(), contracts);
        }
    }
    Ok(voyager)
}
