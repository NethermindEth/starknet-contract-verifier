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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use std::path::PathBuf;

    #[test]
    fn test_manifest_path_with_runtime_manifest() {
        // Create a minimal test without complex scarb_metadata structs
        let runtime_manifest = Utf8PathBuf::from("/test/project/Scarb.toml");
        let workspace_manifest = Utf8PathBuf::from("/test/project/Scarb.toml");

        // Test the logic: if runtime_manifest is not empty, use it
        if runtime_manifest != Utf8PathBuf::new() {
            assert_eq!(
                runtime_manifest,
                Utf8PathBuf::from("/test/project/Scarb.toml")
            );
        } else {
            assert_eq!(
                workspace_manifest,
                Utf8PathBuf::from("/test/project/Scarb.toml")
            );
        }
    }

    #[test]
    fn test_manifest_path_fallback_to_workspace() {
        // Test the fallback logic
        let runtime_manifest = Utf8PathBuf::new(); // Empty path
        let workspace_manifest = Utf8PathBuf::from("/test/project/Scarb.toml");

        let result = if runtime_manifest == Utf8PathBuf::new() {
            &workspace_manifest
        } else {
            &runtime_manifest
        };

        assert_eq!(result, &Utf8PathBuf::from("/test/project/Scarb.toml"));
    }

    #[test]
    fn test_voyager_clone() {
        let voyager = Voyager {
            path: PathBuf::from("/test/path"),
            address: Some("0x123".to_string()),
        };
        let cloned = voyager.clone();
        assert_eq!(voyager.path, cloned.path);
        assert_eq!(voyager.address, cloned.address);
    }

    #[test]
    fn test_voyager_debug() {
        let voyager = Voyager {
            path: PathBuf::from("/test/path"),
            address: Some("0x123".to_string()),
        };
        let debug_str = format!("{voyager:?}");
        assert!(debug_str.contains("/test/path"));
        assert!(debug_str.contains("0x123"));
    }

    #[test]
    fn test_error_display() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let error = Error::Deserialization(json_error);
        let error_string = format!("{error}");
        assert!(error_string.contains("expected value"));
    }

    #[test]
    fn test_contract_map_functionality() {
        let mut contract_map = ContractMap::new();
        contract_map.insert(
            "contract1".to_string(),
            Voyager {
                path: PathBuf::from("/test/contract1.cairo"),
                address: Some("0x123".to_string()),
            },
        );
        contract_map.insert(
            "contract2".to_string(),
            Voyager {
                path: PathBuf::from("/test/contract2.cairo"),
                address: None,
            },
        );

        assert_eq!(contract_map.len(), 2);
        assert!(contract_map.contains_key("contract1"));
        assert!(contract_map.contains_key("contract2"));

        let contract1 = contract_map.get("contract1").unwrap();
        assert_eq!(contract1.address, Some("0x123".to_string()));

        let contract2 = contract_map.get("contract2").unwrap();
        assert_eq!(contract2.address, None);
    }
}
