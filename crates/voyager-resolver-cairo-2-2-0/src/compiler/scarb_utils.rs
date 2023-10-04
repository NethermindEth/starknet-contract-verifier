use anyhow::{anyhow, ensure, Context, Result};
use scarb::flock::Filesystem;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{Document, Formatted, InlineTable, Item, Table, Value};

use cairo_lang_filesystem::db::FilesGroupEx;
use cairo_lang_filesystem::ids::{CrateLongId, Directory};
use cairo_lang_semantic::db::SemanticGroup;
use scarb::core::Package;

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
        for component in unit.components {
            let root = component.source_root();
            if root.exists() {
                let crate_id = db.intern_crate(CrateLongId::Real(component.name.as_str().into()));
                db.set_crate_root(crate_id, Some(Directory::Real(root.into())));
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
) -> Result<()> {
    for package in scarb_metadata.packages {
        // core and starknet are builtin dependencies of the compiler
        if package.name == "core" || package.name == "starknet" {
            continue;
        }
        let manifest_path = package.manifest_path;
        let target_path = target_dir.path_existent()?.join(package.name);
        generate_updated_scarb_toml(manifest_path.into_std_path_buf(), target_path.as_std_path())?;
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
pub fn generate_updated_scarb_toml(manifest_path: PathBuf, target_path: &Path) -> Result<()> {
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
        // starkent package dependency is builtin
        // within the compiler
        if *k == "starknet" {
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
    let verify_metadata = package.fetch_tool_metadata("voyager")?;
    let table_values = verify_metadata
        .as_table()
        .ok_or_else(|| anyhow!("verify metadata is not a table"))?
        .values()
        .map(|v| PathBuf::from(v.get("path").unwrap().as_str().unwrap()))
        .collect::<Vec<_>>();

    Ok(table_values)
}
