use anyhow::{anyhow, ensure, Context, Result};
use scarb::flock::Filesystem;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{Document, Formatted, InlineTable, Item, Table, Value};

use cairo_lang_filesystem::db::FilesGroupEx;
use cairo_lang_filesystem::ids::{CrateLongId, Directory};
use cairo_lang_semantic::db::SemanticGroup;
use scarb::core::Package;

use crate::model::CairoModule;

#[derive(Debug, Deserialize)]
struct ScarbTomlRawPackageData {
    name: String,
    license_file: Option<String>,
    readme: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScarbTomlRawData {
    package: ScarbTomlRawPackageData,
}

#[derive(Debug)]
pub struct AdditionalScarbManifestMetadata {
    pub name: String,
    pub license_file: String,
    pub readme: String,
}

// Get the MetadataManifest from the Scarb.toml
// TODO: replace this with the scarb-metadata as an alternative.
pub fn read_additional_scarb_manifest_metadata(
    scarb_toml_content: &str,
) -> Result<AdditionalScarbManifestMetadata> {
    let scarb_metadata = toml::from_str::<ScarbTomlRawData>(scarb_toml_content)?;

    let scarb_metadata_package_name = scarb_metadata.package.name;
    let scarb_metadata_package_license_file =
        scarb_metadata.package.license_file.unwrap_or("".into());
    let scarb_metadata_package_readme = scarb_metadata.package.readme.unwrap_or("".into());

    Ok(AdditionalScarbManifestMetadata {
        name: scarb_metadata_package_name,
        license_file: scarb_metadata_package_license_file,
        readme: scarb_metadata_package_readme,
    })
}

/// Reads Scarb project metadata from manifest file.
pub fn read_scarb_metadata(manifest_path: &PathBuf) -> anyhow::Result<scarb_metadata::Metadata> {
    scarb_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .inherit_stderr()
        .exec()
        .map_err(Into::into)
}

/// Updates the crate roots in the compiler database using the metadata from a Scarb compilation.
/// The crate roots are set to the source roots of each compilation unit in the metadata.
/// The function does not return a value, but modifies the semantic group object referenced by `db`.
/// # Arguments
///
/// * `db` - A mutable reference to a `SemanticGroup` trait object.
/// * `scarb_metadata` - A `scarb_metadata::Metadata` struct containing metadata from a Scarb project.
pub fn update_crate_roots_from_metadata(
    db: &mut dyn SemanticGroup,
    scarb_metadata: scarb_metadata::Metadata,
) {
    for unit in scarb_metadata.compilation_units {
        // Filter out test crates since these causes error when attempting
        // to load the configurations below from the db.
        // TODO: Investigate inherent reason why test crates causes this issue.
        if unit.target.kind.eq("test") {
            continue;
        }
        for component in unit.components {
            let root = component.source_root();

            if root.exists() {
                let crate_id = db.intern_crate(CrateLongId::Real(component.name.as_str().into()));
                let mut crate_config = db
                    .crate_config(crate_id)
                    .expect("Failed to get crate root directory")
                    .clone();
                crate_config.root = Directory::Real(root.into());
                db.set_crate_config(crate_id, Some(crate_config));
            };
        }
    }
}

/// Extracted from Scarb's crate.
pub fn get_table_mut<'a>(doc: &'a mut Document, path: &[&str]) -> Result<&'a mut Item> {
    return visit(doc.as_item_mut(), path);

    fn visit<'a>(item: &'a mut Item, path: &[&str]) -> Result<&'a mut Item> {
        if let Some(segment) = path.first() {
            let item = item[segment].or_insert({
                let mut table = Table::new();
                table.set_implicit(true);
                Item::Table(table)
            });

            ensure!(
                item.is_table_like(),
                "the table `{segment}` could not be found."
            );
            visit(item, &path[1..])
        } else {
            assert!(item.is_table_like());
            Ok(item)
        }
    }
}

/// Generates an updated Scarb.toml files for all the packages in the given Scarb metadata.
/// To resolve dependencies to local paths.
///
/// # Arguments
///
/// * `scarb_metadata` - A `scarb_metadata::Metadata` object containing information about Scarb packages.
/// * `target_dir` - A `Filesystem` object representing the target directory where updated Scarb.toml files will be generated.
/// * `required_modules` - A `Vec` of `CairoModule` objects representing the modules required by the compiler.
///
/// # Errors
///
/// This function returns an error if:
///
/// * The `target_dir` is not valid.
/// * Generating an updated Scarb.toml file fails.
pub fn generate_scarb_updated_files(
    scarb_metadata: scarb_metadata::Metadata,
    target_dir: &Filesystem,
    required_modules: Vec<&CairoModule>,
) -> Result<()> {
    let mut metadata = scarb_metadata.clone();
    let required_packages = required_modules
        .iter()
        .map(|m| m.path.get_crate())
        .collect::<Vec<_>>();

    // Delete all unused packages from metadata
    // This include "core", "starknet" and scarb's "test_plugin"
    // and any other external dependencies not used in target contracts
    metadata
        .packages
        .retain(|package| required_packages.contains(&package.name));

    for package in metadata.packages {
        let manifest_path = package.manifest_path;
        let target_path = target_dir.path_existent()?.join(package.name);
        generate_updated_scarb_toml(
            manifest_path.into_std_path_buf(),
            target_path.as_std_path(),
            &required_packages,
        )?;
    }
    Ok(())
}

/**
 * Generates a new Scarb.toml manifest file that points to local dependencies.
 *
 * # Arguments
 *
 * * `manifest_path` - A `PathBuf` of the Scarb.toml file to be updated.
 * * `target_path` - A `Path` to the target directory for the updated Scarb.toml file.
 * * `required_packages` - A `Vec` of `String`s representing the names of the packages required by the compiler.
 *
 * # Errors
 *
 * This function will return an error if any of the following conditions are met:
 *
 * * The specified manifest_path does not exist.
 * * The specified manifest_path cannot be read.
 * * The specified manifest_path is not a valid TOML document.
 * * The specified target_path cannot be created.
 * * The updated Scarb.toml file cannot be written to the specified target_path.
 */
pub fn generate_updated_scarb_toml(
    manifest_path: PathBuf,
    target_path: &Path,
    required_packages: &[String],
) -> Result<()> {
    let manifest_path = fs::canonicalize(manifest_path)?;
    let original_raw_manifest = fs::read_to_string(&manifest_path)?;

    let mut doc = Document::from_str(&original_raw_manifest).with_context(|| {
        format!(
            "failed to read manifest at `{}`",
            manifest_path.to_string_lossy()
        )
    })?;

    let tab = get_table_mut(&mut doc, &["dependencies"])?;

    let binding = tab.clone();
    let table_keys = binding
        .as_table_like()
        .unwrap()
        .get_values()
        .iter()
        .map(|(k, _)| k[0].get())
        .collect::<Vec<_>>();

    table_keys.iter().for_each(|k| {
        // starknet package dependency is builtin within the compiler
        if *k == "starknet" {
            return;
        }
        // remove unused dependencies
        if !required_packages.contains(&k.to_string()) && *k != "starknet" {
            tab.as_table_like_mut().unwrap().remove(k);
            return;
        }

        let mut new_table = InlineTable::new();
        new_table.insert("path", Value::String(Formatted::new(format!("../{}", k))));
        tab.as_table_like_mut()
            .unwrap()
            .insert(k, Item::Value(Value::InlineTable(new_table)));
    });

    let new_raw_manifest = doc.to_string();

    let new_manifest_path = target_path.join("Scarb.toml");
    // Create the parent directories for the destination file if they don't exist
    if let Some(parent) = new_manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(new_manifest_path, new_raw_manifest.as_bytes())?;

    Ok(())
}

/// This function retrieves the relative path of the contracts that need to be verified from a
/// package's tool metadata.
///
/// # Arguments
///
/// * `package` - A reference to the package from which to retrieve the contracts to verify.
///
/// # Returns
///
/// * A `Result` containing a `Vec` of `String`s representing the contracts relative path to verify if successful.
///
/// # Errors
///
/// The function returns an error if:
///
/// * The tool metadata for "voyager" cannot be fetched from the package.
/// * The tool metadata is not a table.
///
pub fn get_contracts_to_verify(package: &Package) -> Result<Vec<PathBuf>> {
    let verify_metadata = package
        .fetch_tool_metadata("voyager")
        .with_context(|| "manifest has no [tool.voyager] section which is required")?;
    let table_values = verify_metadata
        .as_table()
        .ok_or_else(|| anyhow!("verify metadata is not a table"))?
        .values()
        .map(|v| PathBuf::from(v.get("path").unwrap().as_str().unwrap()))
        .collect::<Vec<_>>();

    Ok(table_values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_correctly_extract_the_scarb_toml_metadata() {
        let scarb_toml_content = r#"
        [package]
        name = "test_data"
        version = "0.1.0"
        license_file = "LICENSE"
        readme = "README.md"
        "#;

        let data = read_additional_scarb_manifest_metadata(scarb_toml_content).unwrap();

        assert_eq!(data.name, "test_data");
        assert_eq!(data.license_file, "LICENSE");
        assert_eq!(data.readme, "README.md");
    }

    #[test]
    fn should_correctly_extract_empty_scarb_toml_metadata() {
        let scarb_toml_content = r#"
        [package]
        name = "test_data_2"
        version = "0.1.0"
        "#;

        let data = read_additional_scarb_manifest_metadata(scarb_toml_content).unwrap();

        assert_eq!(data.name, "test_data_2");
        assert_eq!(data.license_file, "");
        assert_eq!(data.readme, "");
    }

    #[test]
    fn should_correctly_extract_existing_scarb_toml_metadata() {
        let scarb_toml_content = r#"
        [package]
        name = "test_data_2"
        version = "0.1.0"
        readme = "README.md"
        "#;

        let data = read_additional_scarb_manifest_metadata(scarb_toml_content).unwrap();

        assert_eq!(data.name, "test_data_2");
        assert_eq!(data.license_file, "");
        assert_eq!(data.readme, "README.md");
    }
}
