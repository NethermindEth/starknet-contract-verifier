use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use scarb_metadata::{Metadata, MetadataCommand, PackageMetadata};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
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

// pub fn relative_package_path(
//     metadata: &Metadata,
//     package_metadata: &PackageMetadata,
// ) -> Result<Utf8PathBuf, ResolverError> {
//     let root = &metadata.workspace.root;
//     let abs_package_root = &package_metadata.root;
//     Ok(abs_package_root
//         .strip_prefix(root)
//         .map_err(|_| ResolverError::OutsideFile {
//             file: abs_package_root.clone().into(),
//             dir: root.clone().into(),
//         })?
//         .to_path_buf())
// }

// pub fn contract_paths(
//     metadata: &Metadata,
// ) -> Result<HashMap<PackageId, Vec<PathBuf>>, ResolverError> {
//     let package_contracts = voyager::tool_section(metadata);

//     let package_paths: HashMap<PackageId, Vec<PathBuf>> = package_contracts
//         .iter()
//         .map(|(package_id, contracts)| {
//             let paths = contracts.iter().fold(vec![], |mut acc, (_name, v)| {
//                 acc.push(v.path.clone());
//                 acc
//             });
//             (package_id.clone(), paths)
//         })
//         .filter(|i| !i.1.is_empty())
//         .collect();

//     Ok(package_paths)
// }

// pub fn gather_sources(metadata: &Metadata) -> Result<Vec<FileInfo>, ResolverError> {
//     let local_sources: Vec<Vec<FileInfo>> = metadata
//         .packages
//         .iter()
//         .filter(|&package_meta| metadata.workspace.members.contains(&package_meta.id))
//         .map(|package_meta| {
//             let sources = package_sources(package_meta);
//             package_sources_file_info(metadata, sources)
//         })
//         .try_collect()?;

//     let manifest_path = metadata.workspace.manifest_path.clone().into_std_path_buf();
//     let root = metadata.workspace.root.clone().into_std_path_buf();
//     let manifest_name = manifest_path
//         .strip_prefix(&root)
//         .map_err(|_| ResolverError::OutsideFile {
//             file: manifest_path.clone(),
//             dir: root,
//         })?
//         .to_path_buf();

//     let mut sources = local_sources.into_iter().concat();
//     // if workspace and package directory/manifest coincide, that
//     // manifest will already be in the vec.
//     let manifest_entry = FileInfo {
//         name: manifest_name.to_string_lossy().into_owned(),
//         path: manifest_path,
//     };
//     if let None = sources.iter().position(|e| *e == manifest_entry) {
//         sources.push(manifest_entry);
//     }

//     Ok(sources)
// }

// pub fn package_sources_file_info(
//     metadata: &Metadata,
//     files: Vec<PathBuf>,
// ) -> Result<Vec<FileInfo>, ResolverError> {
//     let prefix = &metadata.workspace.root;

//     files
//         .iter()
//         .map(|path| {
//             let name = path
//                 .strip_prefix(prefix)
//                 .map_err(|_| ResolverError::OutsideFile {
//                     file: path.clone(),
//                     dir: prefix.into(),
//                 })?
//                 .to_string_lossy()
//                 .into_owned();
//             Ok(FileInfo {
//                 name,
//                 path: path.to_path_buf(),
//             })
//         })
//         .try_collect()
// }

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
        .map(Path::to_path_buf)
    {
        sources.push(package_root.join_os(lic))
    }

    if let Some(readme) = package_metadata
        .manifest_metadata
        .readme
        .as_deref()
        .map(Path::new)
        .map(Path::to_path_buf)
    {
        sources.push(package_root.join_os(readme));
    }

    sources
}

pub fn biggest_common_prefix<P: AsRef<Utf8Path> + Clone>(
    paths: &Vec<PathBuf>,
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
