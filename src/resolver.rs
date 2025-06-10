use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use log::debug;
use scarb_metadata::{Metadata, MetadataCommand, PackageMetadata};
use std::{collections::HashMap, ffi::OsStr, path::PathBuf};
use thiserror::Error;
use url::Url;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Couldn't parse {name} path: {path}")]
    DependencyPath { name: String, path: String },

    #[error("scarb metadata failed for {name}: {path}")]
    MetadataError { name: String, path: PathBuf },

    #[error(transparent)]
    Utf8(#[from] camino::FromPathBufError),
}

/// # Errors
///
/// Will return `Err` if it can't read files from the directory that
/// metadata points to.
pub fn gather_packages(
    metadata: &Metadata,
    packages: &mut Vec<PackageMetadata>,
) -> Result<(), Error> {
    let mut workspace_packages: Vec<PackageMetadata> = metadata
        .packages
        .clone()
        .into_iter()
        .filter(|package_meta| metadata.workspace.members.contains(&package_meta.id))
        .filter(|package_meta| !packages.contains(package_meta))
        .collect();

    let workspace_packages_names = workspace_packages
        .iter()
        .map(|package| package.name.clone())
        .collect_vec();

    // find all dependencies listed by path
    let mut dependencies: HashMap<String, PathBuf> = HashMap::new();
    for package in &workspace_packages {
        for dependency in &package.dependencies {
            let name = &dependency.name;
            let url = Url::parse(&dependency.source.repr).map_err(|_| Error::DependencyPath {
                name: name.clone(),
                path: dependency.source.repr.clone(),
            })?;

            if url.scheme().starts_with("path") {
                let path = url.to_file_path().map_err(|()| Error::DependencyPath {
                    name: name.clone(),
                    path: dependency.source.repr.clone(),
                })?;
                dependencies.insert(name.clone(), path);
            }
        }
    }

    packages.append(&mut workspace_packages);

    // filter out dependencies already covered by workspace
    let out_of_workspace_dependencies: HashMap<&String, &PathBuf> = dependencies
        .iter()
        .filter(|&(k, _)| !workspace_packages_names.contains(k))
        .collect();

    for (name, manifest) in out_of_workspace_dependencies {
        let new_meta = MetadataCommand::new()
            .json()
            .manifest_path(manifest)
            .exec()
            .map_err(|_| Error::MetadataError {
                name: name.clone(),
                path: manifest.clone(),
            })?;
        gather_packages(&new_meta, packages)?;
    }

    Ok(())
}

/// # Errors
///
/// Will return `Err` if it can't read files from the directory that
/// metadata points to.
pub fn package_sources(package_metadata: &PackageMetadata) -> Result<Vec<Utf8PathBuf>, Error> {
    debug!("Collecting sources for package: {}", package_metadata.name);
    debug!("Package root: {}", package_metadata.root);
    debug!("Package manifest: {}", package_metadata.manifest_path);

    let mut sources: Vec<Utf8PathBuf> = WalkDir::new(package_metadata.root.clone())
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|f| f.file_type().is_file())
        .filter(|f| {
            // Include Cairo files
            if let Some(ext) = f.path().extension() {
                if ext == OsStr::new(CAIRO_EXT) {
                    return true;
                }
            }

            // Include Scarb.toml files (being more explicit)
            if f.file_name() == OsStr::new("Scarb.toml") {
                return true;
            }

            false
        })
        .map(walkdir::DirEntry::into_path)
        .map(Utf8PathBuf::try_from)
        .try_collect()?;

    // Ensure the package's own manifest is included
    if !sources.contains(&package_metadata.manifest_path) {
        sources.push(package_metadata.manifest_path.clone());
    }

    let package_root = &package_metadata.root;

    if let Some(lic) = package_metadata
        .manifest_metadata
        .license_file
        .as_ref()
        .map(Utf8Path::new)
        .map(Utf8Path::to_path_buf)
    {
        sources.push(package_root.join(lic));
    }

    if let Some(readme) = package_metadata
        .manifest_metadata
        .readme
        .as_deref()
        .map(Utf8Path::new)
        .map(Utf8Path::to_path_buf)
    {
        sources.push(package_root.join(readme));
    }

    Ok(sources)
}

pub fn biggest_common_prefix<P: AsRef<Utf8Path> + Clone>(
    paths: &[Utf8PathBuf],
    first_guess: P,
) -> Utf8PathBuf {
    let ancestors = Utf8Path::ancestors(first_guess.as_ref());
    let mut biggest_prefix: &Utf8Path = first_guess.as_ref();
    for prefix in ancestors {
        if paths.iter().all(|src| src.starts_with(prefix)) {
            biggest_prefix = prefix;
            break;
        }
    }
    biggest_prefix.to_path_buf()
}

const CAIRO_EXT: &str = "cairo";
