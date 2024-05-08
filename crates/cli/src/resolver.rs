use camino::Utf8PathBuf;
use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::{fs, time::Instant};
use walkdir::{DirEntry, WalkDir};

use crate::api::{FileInfo, ProjectMetadataInfo};
use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedCairoVersions, SupportedScarbVersions};
use voyager_resolver_cairo::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGenerator;

pub fn get_dynamic_compiler(cairo_version: SupportedCairoVersions) -> Box<dyn DynamicCompiler> {
    match cairo_version {
        SupportedCairoVersions::V2_4_3 => Box::new(VoyagerGenerator),
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

pub fn resolve_scarb(
    path: Utf8PathBuf,
    cairo_version: SupportedCairoVersions,
    scarb_version: SupportedScarbVersions,
) -> anyhow::Result<(Vec<FileInfo>, ProjectMetadataInfo)> {
    // Start a spinner for the verification process
    let started = Instant::now();
    // let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
    //     .unwrap()
    //     .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈");

    println!(
        "{} {} Resolving contract: Extracting files from the Scarb project...",
        style("[1/4]").bold().dim(),
        Emoji("📃  ", "")
    );
    // Extract necessary files from the Scarb project for the verified contract
    let source_dir = if path.is_absolute() {
        path
    } else {
        let mut current_path = std::env::current_dir().unwrap();
        current_path.push(path);
        Utf8PathBuf::from_path_buf(current_path).unwrap()
    };

    let compiler = get_dynamic_compiler(cairo_version);

    let contract_paths = compiler.get_contracts_to_verify_path(&source_dir)?;

    // TODO move the contract selection before the resolving step as a 'pre-resolving' step
    // in order to allow for automatic contracts discovery and selection
    if contract_paths.is_empty() {
        return Err(anyhow::anyhow!("No contracts to verify"));
    }
    if contract_paths.len() > 1 {
        return Err(anyhow::anyhow!(
            "Only one contract can be verified at a time"
        ));
    }

    println!(
        "{} {}Resolving contract: minimizing dependencies...",
        style("[2/3]").bold().dim(),
        Emoji("🔗  ", "")
    );
    let steps = 4;
    let pb = ProgressBar::new(steps);

    // Read the scarb metadata to get more information
    let scarb_toml_content = fs::read_to_string(source_dir.join("Scarb.toml"))?;
    let scarb_metadata_package_name = toml::from_str::<ScarbTomlRawData>(&scarb_toml_content)?
        .package
        .name;
    pb.inc(1);

    // Compiler and extract the necessary files
    compiler.compile_project(&source_dir)?;
    pb.inc(2);

    // Since we know that we extract the files into the `voyager-verify` directory,
    // we'll read the files from there.
    let extracted_files_dir = source_dir.join("voyager-verify");

    // The compiler compiles into the original scarb package name
    // As such we have to craft the correct path to the main package
    let project_dir_path = extracted_files_dir.join(scarb_metadata_package_name);
    let project_dir_path = project_dir_path
        .strip_prefix(extracted_files_dir.clone())
        .unwrap();

    // Read project directory
    let project_files = WalkDir::new(extracted_files_dir.as_path())
        .into_iter()
        .filter_map(|f| f.ok())
        .filter(|f| f.file_type().is_file())
        .filter(|f| {
            f.path().extension().unwrap() == "cairo"
                || f.path()
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or("".into())
                    .to_lowercase()
                    == "scarb.toml"
        })
        .collect::<Vec<DirEntry>>();

    let project_files = project_files
        .iter()
        .map(|f| {
            let actual_path = f.path().to_owned();
            let file_name = actual_path
                .strip_prefix(&extracted_files_dir)
                .unwrap()
                .to_str()
                .to_owned()
                .unwrap()
                .to_string();
            FileInfo {
                name: file_name,
                path: actual_path,
            }
        })
        .collect::<Vec<FileInfo>>();

    let project_metadata = ProjectMetadataInfo {
        cairo_version,
        scarb_version,
        project_dir_path: project_dir_path.as_str().to_owned(),
    };

    pb.inc(1);
    pb.finish_and_clear();

    Ok((project_files, project_metadata))
}
