use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use scarb_metadata::{Metadata, MetadataCommand, PackageMetadata};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::PathBuf,
};
use thiserror::Error;
use url::Url;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("Couldn't parse {name} path: {path}")]
    DependencyPath { name: String, path: String },

    #[error("scarb metadata failed for {name}: {path}")]
    MetadataError { name: String, path: PathBuf },

    #[error(transparent)]
    Utf8(#[from] camino::FromPathBufError),
}

pub fn gather_packages(
    metadata: &Metadata,
    packages: &mut Vec<PackageMetadata>,
) -> Result<(), ResolverError> {
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
            let url =
                Url::parse(&dependency.source.repr).map_err(|_| ResolverError::DependencyPath {
                    name: name.clone(),
                    path: dependency.source.repr.clone(),
                })?;

            if url.scheme().starts_with("path") {
                let path = url
                    .to_file_path()
                    .map_err(|_| ResolverError::DependencyPath {
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
        .filter(|&(k, _)| !workspace_packages_names.contains(&k))
        .collect();

    for (name, manifest) in out_of_workspace_dependencies {
        let new_meta = MetadataCommand::new()
            .json()
            .manifest_path(manifest)
            .exec()
            .map_err(|_| ResolverError::MetadataError {
                name: name.clone(),
                path: manifest.clone(),
            })?;
        gather_packages(&new_meta, packages)?;
    }

    Ok(())
}

pub fn package_sources(package_metadata: &PackageMetadata) -> Result<Vec<Utf8PathBuf>, ResolverError> {
    let mut sources: Vec<Utf8PathBuf> = WalkDir::new(package_metadata.root.clone())
        .into_iter()
        .filter_map(|f| f.ok())
        .filter(|f| f.file_type().is_file())
        .filter(|f| {
            if let Some(ext) = f.path().extension() {
                if ext == OsStr::new(CAIRO_EXT) {
                    return true;
                }
            };

            return false;
        })
        .map(|dir_entry| dir_entry.into_path())
        .map(Utf8PathBuf::try_from)
        .try_collect()?;

    sources.push(package_metadata.manifest_path.clone().into());
    let package_root = &package_metadata.root;

    if let Some(lic) = package_metadata
        .manifest_metadata
        .license_file
        .as_ref()
        .map(Utf8Path::new)
        .map(Utf8Path::to_path_buf)
    {
        sources.push(package_root.join(lic))
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
    paths: &Vec<Utf8PathBuf>,
    first_guess: P,
) -> Utf8PathBuf {
    let mut ancestors = Utf8Path::ancestors(first_guess.as_ref());
    let mut biggest_prefix: &Utf8Path = first_guess.as_ref();
    while let Some(prefix) = ancestors.next() {
        if paths.iter().all(|src| src.starts_with(prefix)) {
            biggest_prefix = prefix;
            break;
        }
    }
    biggest_prefix.to_path_buf()
}

const CAIRO_EXT: &str = "cairo";
