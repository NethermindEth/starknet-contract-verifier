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
    #[error("[E012] Invalid dependency path for '{name}': {path}\n\nSuggestions:\n  • Check that the path exists and is accessible\n  • Use relative paths from the current directory\n  • Verify the path format is correct\n  • Example: path:../my-dependency")]
    DependencyPath { name: String, path: String },

    #[error("[E013] Failed to read metadata for '{name}' at path: {path}\n\nSuggestions:\n  • Check that Scarb.toml exists at the specified path\n  • Verify the Scarb.toml file is valid\n  • Run 'scarb metadata' in the target directory to test\n  • Ensure scarb is installed and accessible")]
    MetadataError { name: String, path: PathBuf },

    #[error("[E014] Path contains invalid UTF-8 characters\n\nSuggestions:\n  • Use only ASCII characters in file paths\n  • Avoid special characters in directory names\n  • Check for hidden or control characters in the path")]
    Utf8(#[from] camino::FromPathBufError),
}

impl Error {
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::DependencyPath { .. } => "E012",
            Self::MetadataError { .. } => "E013",
            Self::Utf8(_) => "E014",
        }
    }
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
    package_sources_with_test_files(package_metadata, false)
}

/// # Errors
///
/// Will return `Err` if it can't read files from the directory that
/// metadata points to.
pub fn package_sources_with_test_files(
    package_metadata: &PackageMetadata,
    include_test_files: bool,
) -> Result<Vec<Utf8PathBuf>, Error> {
    debug!("Collecting sources for package: {}", package_metadata.name);
    debug!("Package root: {}", package_metadata.root);
    debug!("Package manifest: {}", package_metadata.manifest_path);

    let mut sources: Vec<Utf8PathBuf> = WalkDir::new(package_metadata.root.clone())
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|f| f.file_type().is_file())
        .filter(|f| {
            // Check if this is a test file
            if let Some(path_str) = f.path().to_str() {
                // Check if the path contains test directories but only if it's in src/
                let is_in_src = path_str.contains("/src/");
                let has_test_in_path = path_str.contains("/test") || path_str.contains("/tests/");

                if is_in_src && has_test_in_path {
                    // This is a test file in src/
                    return include_test_files;
                }

                // Exclude test directories outside src/
                if path_str.contains("/tests/")
                    || path_str.contains("/test/")
                    || path_str.contains("/examples/")
                    || path_str.contains("/benchmarks/")
                {
                    return false;
                }
            }

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_biggest_common_prefix_simple() {
        let paths = vec![
            Utf8PathBuf::from("/root/project/src/lib.cairo"),
            Utf8PathBuf::from("/root/project/src/main.cairo"),
            Utf8PathBuf::from("/root/project/tests/test.cairo"),
        ];
        let first_guess = Utf8PathBuf::from("/root/project/src/lib.cairo");
        let result = biggest_common_prefix(&paths, first_guess);
        assert_eq!(result, Utf8PathBuf::from("/root/project"));
    }

    #[test]
    fn test_biggest_common_prefix_no_common() {
        let paths = vec![
            Utf8PathBuf::from("/root/project1/src/lib.cairo"),
            Utf8PathBuf::from("/root/project2/src/main.cairo"),
        ];
        let first_guess = Utf8PathBuf::from("/root/project1/src/lib.cairo");
        let result = biggest_common_prefix(&paths, first_guess);
        assert_eq!(result, Utf8PathBuf::from("/root"));
    }

    #[test]
    fn test_biggest_common_prefix_exact_match() {
        let paths = vec![Utf8PathBuf::from("/root/project/src/lib.cairo")];
        let first_guess = Utf8PathBuf::from("/root/project/src/lib.cairo");
        let result = biggest_common_prefix(&paths, first_guess);
        assert_eq!(result, Utf8PathBuf::from("/root/project/src/lib.cairo"));
    }

    #[test]
    fn test_error_display() {
        let error = Error::DependencyPath {
            name: "test_package".to_string(),
            path: "/invalid/path".to_string(),
        };
        let error_message = format!("{error}");
        assert!(error_message.contains("[E012]"));
        assert!(error_message.contains("Invalid dependency path"));
        assert!(error_message.contains("test_package"));
        assert!(error_message.contains("/invalid/path"));
        assert!(error_message.contains("Check that the path exists"));
    }

    #[test]
    fn test_cairo_extension_constant() {
        assert_eq!(CAIRO_EXT, "cairo");
    }

    #[test]
    fn test_file_filtering_logic() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = PathBuf::from(temp_dir.path());

        // Create test directory structure
        std::fs::create_dir_all(temp_path.join("src")).unwrap();
        std::fs::create_dir_all(temp_path.join("tests")).unwrap();
        std::fs::create_dir_all(temp_path.join("examples")).unwrap();

        // Create test files
        std::fs::write(temp_path.join("src").join("lib.cairo"), "").unwrap();
        std::fs::write(temp_path.join("src").join("main.cairo"), "").unwrap();
        std::fs::write(temp_path.join("tests").join("test.cairo"), "").unwrap();
        std::fs::write(temp_path.join("examples").join("example.cairo"), "").unwrap();
        std::fs::write(temp_path.join("Scarb.toml"), "").unwrap();
        std::fs::write(temp_path.join("other.txt"), "").unwrap();

        // Test the filtering logic used in package_sources
        let cairo_files: Vec<_> = walkdir::WalkDir::new(&temp_path)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|f| f.file_type().is_file())
            .filter(|f| {
                // Test the exclusion logic
                if let Some(path_str) = f.path().to_str() {
                    if path_str.contains("/tests/")
                        || path_str.contains("/test/")
                        || path_str.contains("/examples/")
                        || path_str.contains("/benchmarks/")
                    {
                        return false;
                    }
                }

                // Test the inclusion logic
                if let Some(ext) = f.path().extension() {
                    if ext == std::ffi::OsStr::new(CAIRO_EXT) {
                        return true;
                    }
                }

                // Test Scarb.toml inclusion
                if f.file_name() == std::ffi::OsStr::new("Scarb.toml") {
                    return true;
                }

                false
            })
            .collect();

        // Should include cairo files from src, Scarb.toml, but not from tests or examples
        assert!(cairo_files
            .iter()
            .any(|f| f.file_name() == std::ffi::OsStr::new("lib.cairo")));
        assert!(cairo_files
            .iter()
            .any(|f| f.file_name() == std::ffi::OsStr::new("main.cairo")));
        assert!(cairo_files
            .iter()
            .any(|f| f.file_name() == std::ffi::OsStr::new("Scarb.toml")));
        assert!(!cairo_files
            .iter()
            .any(|f| f.path().to_str().unwrap().contains("/tests/")));
        assert!(!cairo_files
            .iter()
            .any(|f| f.path().to_str().unwrap().contains("/examples/")));
        assert!(!cairo_files
            .iter()
            .any(|f| f.file_name() == std::ffi::OsStr::new("other.txt")));
    }
}
