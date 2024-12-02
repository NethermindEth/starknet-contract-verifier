use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use scarb_metadata::{Metadata, PackageId, PackageMetadata};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{api::FileInfo, voyager};

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("Couldn't find package: {0}")]
    NoPackage(PackageId),

    #[error("Couldn't get package directory: {0}")]
    WrongDirectory(Utf8PathBuf),

    #[error("{file} is outside {dir}")]
    OutsideFile { file: PathBuf, dir: PathBuf },

    #[error("Duplicated contract: `{0}`")]
    Duplicate(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn relative_package_path(
    metadata: &Metadata,
    package_metadata: &PackageMetadata,
) -> Result<Utf8PathBuf, ResolverError> {
    let root = &metadata.workspace.root;
    let abs_package_root = &package_metadata.root;
    Ok(abs_package_root
        .strip_prefix(root)
        .map_err(|_| ResolverError::OutsideFile {
            file: abs_package_root.clone().into(),
            dir: root.clone().into(),
        })?
        .to_path_buf())
}

pub fn contract_paths(
    metadata: &Metadata,
) -> Result<HashMap<PackageId, Vec<PathBuf>>, ResolverError> {
    let package_contracts = voyager::tool_section(metadata);

    let package_paths: HashMap<PackageId, Vec<PathBuf>> = package_contracts
        .iter()
        .map(|(package_id, contracts)| {
            let paths = contracts.iter().fold(vec![], |mut acc, (_name, v)| {
                acc.push(v.path.clone());
                acc
            });
            (package_id.clone(), paths)
        })
        .filter(|i| !i.1.is_empty())
        .collect();

    Ok(package_paths)
}

pub fn gather_sources(metadata: &Metadata) -> Result<Vec<FileInfo>, ResolverError> {
    let local_sources: Vec<Vec<FileInfo>> = metadata
        .packages
        .iter()
        .filter(|&package_meta| metadata.workspace.members.contains(&package_meta.id))
        .map(|package_meta| {
            let sources = package_sources(package_meta);
            package_sources_file_info(metadata, sources)
        })
        .try_collect()?;

    let manifest_path = metadata.workspace.manifest_path.clone().into_std_path_buf();
    let root = metadata.workspace.root.clone().into_std_path_buf();
    let manifest_name = manifest_path
        .strip_prefix(&root)
        .map_err(|_| ResolverError::OutsideFile {
            file: manifest_path.clone(),
            dir: root,
        })?
        .to_path_buf();

    let mut sources = local_sources.into_iter().concat();
    // if workspace and package directory/manifest coincide, that
    // manifest will already be in the vec.
    let manifest_entry = FileInfo {
        name: manifest_name.to_string_lossy().into_owned(),
        path: manifest_path,
    };
    if let None = sources.iter().position(|e| *e == manifest_entry) {
        sources.push(manifest_entry);
    }

    Ok(sources)
}

pub fn package_sources_file_info(
    metadata: &Metadata,
    files: Vec<PathBuf>,
) -> Result<Vec<FileInfo>, ResolverError> {
    let prefix = &metadata.workspace.root;

    files
        .iter()
        .map(|path| {
            let name = path
                .strip_prefix(prefix)
                .map_err(|_| ResolverError::OutsideFile {
                    file: path.clone(),
                    dir: prefix.into(),
                })?
                .to_string_lossy()
                .into_owned();
            Ok(FileInfo {
                name,
                path: path.to_path_buf(),
            })
        })
        .try_collect()
}

pub fn package_sources(package_metadata: &PackageMetadata) -> Vec<PathBuf> {
    let mut sources = WalkDir::new(package_metadata.root.clone())
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
        .collect::<Vec<PathBuf>>();

    sources.push(package_metadata.manifest_path.clone().into());
    let package_root = &package_metadata.root;

    if let Some(lic) = package_metadata
        .manifest_metadata
        .license_file
        .as_ref()
        .map(Path::new)
        .map(|p| p.to_path_buf())
    {
        sources.push(package_root.join_os(lic))
    }

    if let Some(readme) = package_metadata
        .manifest_metadata
        .readme
        .as_deref()
        .map(Path::new)
        .map(|p| p.to_path_buf())
    {
        sources.push(package_root.join_os(readme));
    }

    sources
}

const CAIRO_EXT: &str = "cairo";
