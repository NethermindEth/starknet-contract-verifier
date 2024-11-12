use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

use crate::{
    api::{FileInfo, ProjectMetadataInfo},
    args::Project,
    voyager,
};
use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedCairoVersions, SupportedScarbVersions};
use voyager_resolver_cairo::compiler::scarb_utils::read_additional_scarb_manifest_metadata;
use voyager_resolver_cairo::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGenerator;

#[allow(dead_code)]
pub enum TargetType {
    ScarbProject,
    File,
}

pub fn get_dynamic_compiler(cairo_version: SupportedCairoVersions) -> Box<dyn DynamicCompiler> {
    match cairo_version {
        SupportedCairoVersions::V2_6_4 => Box::new(VoyagerGenerator),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ScarbTomlRawPackageData {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ScarbTomlRawData {
    package: ScarbTomlRawPackageData,
}

pub struct Voyager {
    path: PathBuf,
    address: Option<String>,
}

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error(
        "No contracts selected for verification. Add [tool.voyager] section to Scorb.toml file"
    )]
    NoTarget,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub fn resolve_scarb(
    project: Project,
    cairo_version: SupportedCairoVersions,
    scarb_version: SupportedScarbVersions,
) -> Result<(Vec<FileInfo>, ProjectMetadataInfo), ResolverError> {
    let contracts = voyager::tool_section(project.clone());
    if contracts.is_empty() {
        return Err(ResolverError::NoTarget);
    }

    println!("{contracts:?}");

    // // Extract necessary files from the Scarb project for the verified contract
    let compiler = get_dynamic_compiler(cairo_version);
    let contract_paths = compiler.get_contracts_to_verify_path(project.root_dir())?;

    println!("old: {contract_paths:?}");
    todo!();
    // // TODO move the contract selection before the resolving step as a 'pre-resolving' step
    // // in order to allow for automatic contracts discovery and selection
    // if contract_paths.is_empty() {
    //     return Err(anyhow::anyhow!("No contracts to verify"));
    // }
    // if contract_paths.len() > 1 {
    //     return Err(anyhow::anyhow!(
    //         "Only one contract can be verified at a time"
    //     ));
    // }

    // // Read the scarb metadata to get more information
    // // TODO: switch this to using scarb-metadata
    // let scarb_toml_content = fs::read_to_string(project.manifest_path())?;
    // let extracted_scarb_toml_data = todo!();
    //     // read_additional_scarb_manifest_metadata(scarb_toml_content.as_str())?;

    // // Compiler and extract the necessary files
    // compiler.compile_project(project.root_dir())?;

    // // Since we know that we extract the files into the `voyager-verify` directory,
    // // we'll read the files from there.
    // let extracted_files_dir = project.root_dir().join("voyager-verify");

    // // The compiler compiles into the original scarb package name
    // // As such we have to craft the correct path to the main package
    // let project_dir_path = extracted_files_dir.join(extracted_scarb_toml_data.name.clone());
    // let project_dir_path = project_dir_path
    //     .strip_prefix(extracted_files_dir.clone())
    //     .unwrap();

    // // Read project directory
    // let project_files = WalkDir::new(extracted_files_dir.as_path())
    //     .into_iter()
    //     .filter_map(|f| f.ok())
    //     .filter(|f| f.file_type().is_file())
    //     .filter(|f| {
    //         let file_path = f.path();

    //         let is_cairo_file = match file_path.extension() {
    //             Some(ext) => ext == "cairo",
    //             None => false,
    //         };
    //         let file_entry_name = file_path
    //             .file_name()
    //             .map(|f| f.to_string_lossy().into_owned())
    //             .unwrap_or("".into());

    //         let is_supplementary_file = file_entry_name.to_lowercase() == "scarb.toml"
    //             || file_entry_name == extracted_scarb_toml_data.license_file
    //             || file_entry_name == extracted_scarb_toml_data.readme;

    //         is_cairo_file || is_supplementary_file
    //     })
    //     .collect::<Vec<DirEntry>>();

    // let project_files = project_files
    //     .iter()
    //     .map(|f| {
    //         let actual_path = f.path().to_owned();
    //         let file_name = actual_path
    //             .strip_prefix(&extracted_files_dir)
    //             .unwrap()
    //             .to_str()
    //             .to_owned()
    //             .unwrap()
    //             .to_string();
    //         FileInfo {
    //             name: file_name,
    //             path: actual_path,
    //         }
    //     })
    //     .collect::<Vec<FileInfo>>();

    // let contract_file = format!(
    //     "{}/src/{}",
    //     extracted_scarb_toml_data.name.clone(),
    //     contract_paths[0].as_str()
    // );

    // let project_metadata = ProjectMetadataInfo {
    //     cairo_version,
    //     scarb_version,
    //     contract_file,
    //     project_dir_path: project_dir_path.as_str().to_owned(),
    // };

    // Ok((project_files, project_metadata))
}
