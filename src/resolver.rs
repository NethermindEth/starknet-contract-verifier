use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use log::debug;
use scarb_metadata::{Metadata, MetadataCommand, PackageMetadata};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    path::PathBuf,
};
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

    #[error("[E025] Failed to parse TOML file '{path}': {error}\n\nSuggestions:\n  • Check TOML syntax is valid\n  • Verify file is not corrupted\n  • Use a TOML validator tool")]
    TomlParseError { path: String, error: String },

    #[error("[E026] I/O error reading file '{path}': {error}\n\nSuggestions:\n  • Check file exists and is readable\n  • Verify file permissions\n  • Ensure disk space is available")]
    IoError { path: String, error: String },

    #[error("[E027] Module not found: '{module}' from '{parent_file}'\n\nSuggestions:\n  • Check that the module file exists\n  • Verify module name spelling\n  • Ensure proper file structure (module.rs or module/mod.rs)")]
    ModuleNotFound { module: String, parent_file: String },
}

impl Error {
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::DependencyPath { .. } => "E012",
            Self::MetadataError { .. } => "E013",
            Self::Utf8(_) => "E014",
            Self::TomlParseError { .. } => "E025",
            Self::IoError { .. } => "E026",
            Self::ModuleNotFound { .. } => "E027",
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

    // Check if this is a Cairo procedural macro package
    if is_cairo_procedural_macro_package(&package_metadata.manifest_path)? {
        debug!(
            "Package {} is a Cairo procedural macro package",
            package_metadata.name
        );

        // Validate Cargo.toml configuration
        let cargo_toml_path = package_metadata.root.join("Cargo.toml");
        if validate_cargo_toml_for_proc_macro(&cargo_toml_path)? {
            debug!("Cargo.toml validation passed for procedural macro package");
            return collect_procedural_macro_rust_files(package_metadata, include_test_files);
        } else {
            debug!("Cargo.toml validation failed - treating as regular Cairo package");
            // Fall through to regular Cairo file collection
        }
    }

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

            // Include Cairo files and Rust files
            if let Some(ext) = f.path().extension() {
                if ext == OsStr::new(CAIRO_EXT) || ext == OsStr::new("rs") {
                    return true;
                }
            }

            // Include Scarb.toml and Cargo.toml files (being more explicit)
            if f.file_name() == OsStr::new("Scarb.toml")
                || f.file_name() == OsStr::new("Cargo.toml")
            {
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

// TOML structures for parsing Cairo procedural macro manifests
#[derive(Debug, Deserialize)]
struct ScarbToml {
    #[serde(rename = "cairo-plugin")]
    cairo_plugin: Option<CairoPlugin>,
}

#[derive(Debug, Deserialize)]
struct CairoPlugin {
    // Empty struct - just need to detect presence of [cairo-plugin] section
}

#[derive(Debug, Deserialize)]
struct CargoToml {
    lib: Option<CargoLib>,
    dependencies: Option<std::collections::HashMap<String, toml::Value>>,
}

#[derive(Debug, Deserialize)]
struct CargoLib {
    #[serde(rename = "crate-type")]
    crate_type: Option<Vec<String>>,
}

/// Check if a package is a Cairo procedural macro package by examining its Scarb.toml
///
/// # Errors
/// Returns an error if the TOML file cannot be read or parsed
fn is_cairo_procedural_macro_package(scarb_toml_path: &Utf8Path) -> Result<bool, Error> {
    debug!("Checking if package is Cairo procedural macro: {scarb_toml_path}");

    let content = fs::read_to_string(scarb_toml_path).map_err(|e| Error::IoError {
        path: scarb_toml_path.to_string(),
        error: e.to_string(),
    })?;

    let scarb_toml: ScarbToml = toml::from_str(&content).map_err(|e| Error::TomlParseError {
        path: scarb_toml_path.to_string(),
        error: e.to_string(),
    })?;

    let is_proc_macro = scarb_toml.cairo_plugin.is_some();
    debug!("Package {scarb_toml_path} is procedural macro: {is_proc_macro}");

    Ok(is_proc_macro)
}

/// Validate that a Cargo.toml is configured correctly for a procedural macro
///
/// # Errors
/// Returns an error if the TOML file cannot be read or parsed
fn validate_cargo_toml_for_proc_macro(cargo_toml_path: &Utf8Path) -> Result<bool, Error> {
    debug!("Validating Cargo.toml for procedural macro: {cargo_toml_path}");

    if !cargo_toml_path.exists() {
        debug!("Cargo.toml not found: {cargo_toml_path}");
        return Ok(false);
    }

    let content = fs::read_to_string(cargo_toml_path).map_err(|e| Error::IoError {
        path: cargo_toml_path.to_string(),
        error: e.to_string(),
    })?;

    let cargo_toml: CargoToml = toml::from_str(&content).map_err(|e| Error::TomlParseError {
        path: cargo_toml_path.to_string(),
        error: e.to_string(),
    })?;

    // Check for crate-type = ["cdylib"]
    let has_cdylib = cargo_toml
        .lib
        .as_ref()
        .and_then(|lib| lib.crate_type.as_ref())
        .map(|types| types.contains(&"cdylib".to_string()))
        .unwrap_or(false);

    // Check for cairo-lang-macro dependency
    let has_cairo_macro_dep = cargo_toml
        .dependencies
        .as_ref()
        .map(|deps| deps.contains_key("cairo-lang-macro"))
        .unwrap_or(false);

    let is_valid = has_cdylib && has_cairo_macro_dep;
    debug!(
        "Cargo.toml validation - cdylib: {has_cdylib}, cairo-lang-macro: {has_cairo_macro_dep}, valid: {is_valid}"
    );

    Ok(is_valid)
}

/// Collect only the necessary Rust files for a Cairo procedural macro package
///
/// # Errors
/// Returns an error if files cannot be read or processed
fn collect_procedural_macro_rust_files(
    package_metadata: &PackageMetadata,
    include_test_files: bool,
) -> Result<Vec<Utf8PathBuf>, Error> {
    debug!(
        "Collecting procedural macro Rust files for package: {}",
        package_metadata.name
    );

    let mut required_files = HashSet::new();
    let package_root = &package_metadata.root;

    // Always include Cargo.toml for procedural macros
    let cargo_toml_path = package_root.join("Cargo.toml");
    if cargo_toml_path.exists() {
        debug!("Adding Cargo.toml: {cargo_toml_path}");
        required_files.insert(cargo_toml_path);
    }

    // Start with the main library file and recursively collect dependencies
    let lib_rs_path = package_root.join("src/lib.rs");
    if lib_rs_path.exists() {
        debug!("Starting dependency analysis from lib.rs: {lib_rs_path}");
        required_files.insert(lib_rs_path.clone());

        // Recursively collect module dependencies
        collect_rust_module_dependencies(&lib_rs_path, package_root, &mut required_files)?;
    }

    // Find files with procedural macro attributes
    collect_macro_implementation_files(package_root, &mut required_files)?;

    // Convert to Vec and filter based on include_test_files
    let mut sources: Vec<Utf8PathBuf> = required_files.into_iter().collect();

    // Filter test files if not requested
    if !include_test_files {
        sources.retain(|path| !should_exclude_rust_file(path));
    }

    // Sort for consistent output
    sources.sort();

    debug!(
        "Collected {} Rust files for procedural macro package",
        sources.len()
    );
    for source in &sources {
        debug!("  - {source}");
    }

    Ok(sources)
}

/// Recursively collect Rust module dependencies from a source file
///
/// # Errors
/// Returns an error if files cannot be read or parsed
fn collect_rust_module_dependencies(
    file_path: &Utf8Path,
    package_root: &Utf8Path,
    required_files: &mut HashSet<Utf8PathBuf>,
) -> Result<(), Error> {
    debug!("Analyzing module dependencies in: {file_path}");

    let content = fs::read_to_string(file_path).map_err(|e| Error::IoError {
        path: file_path.to_string(),
        error: e.to_string(),
    })?;

    // Parse for module declarations
    let module_declarations = parse_module_declarations(&content);
    debug!(
        "Found {} module declarations in {}",
        module_declarations.len(),
        file_path
    );

    for module_name in module_declarations {
        match resolve_module_file_path(file_path, &module_name, package_root) {
            Ok(module_file) => {
                if module_file.exists() && !required_files.contains(&module_file) {
                    debug!("Adding module dependency: {module_name} -> {module_file}");
                    required_files.insert(module_file.clone());

                    // Recursively collect dependencies of this module
                    collect_rust_module_dependencies(&module_file, package_root, required_files)?;
                }
            }
            Err(Error::ModuleNotFound { .. }) => {
                // Module not found - this might be a conditional compilation module
                // or an external crate. We'll log it but not fail the build.
                debug!("Module not found (might be conditional): {module_name} from {file_path}");
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

/// Parse Rust source code to find module declarations
fn parse_module_declarations(content: &str) -> Vec<String> {
    let mut modules = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        // Look for module declarations:
        // - mod module_name;
        // - pub mod module_name;
        // - pub(crate) mod module_name;
        if let Some(module_name) = extract_module_name_from_line(trimmed) {
            debug!("Found module declaration: {module_name}");
            modules.push(module_name);
        }
    }

    modules
}

/// Extract module name from a line containing a module declaration
fn extract_module_name_from_line(line: &str) -> Option<String> {
    // Handle various visibility modifiers
    let line = line.trim();

    // Remove pub qualifiers
    let line = if line.starts_with("pub(") {
        // Handle pub(crate), pub(super), etc.
        if let Some(pos) = line.find(')') {
            line[pos + 1..].trim()
        } else {
            return None;
        }
    } else if let Some(stripped) = line.strip_prefix("pub ") {
        stripped.trim()
    } else {
        line
    };

    // Check for "mod" keyword
    if let Some(stripped) = line.strip_prefix("mod ") {
        let rest = stripped.trim();

        // Handle "mod name;" pattern
        if let Some(semicolon_pos) = rest.find(';') {
            let module_name = rest[..semicolon_pos].trim();
            if is_valid_module_name(module_name) {
                return Some(module_name.to_string());
            }
        }

        // Handle "mod name {" pattern (inline module - we still want to track it)
        if let Some(brace_pos) = rest.find('{') {
            let module_name = rest[..brace_pos].trim();
            if is_valid_module_name(module_name) {
                return Some(module_name.to_string());
            }
        }
    }

    None
}

/// Check if a string is a valid Rust module name
fn is_valid_module_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
}

/// Resolve the file path for a given module name
///
/// # Errors
/// Returns an error if the module file cannot be found
fn resolve_module_file_path(
    parent_file: &Utf8Path,
    module_name: &str,
    package_root: &Utf8Path,
) -> Result<Utf8PathBuf, Error> {
    let parent_dir = parent_file.parent().unwrap_or(package_root);

    // Try module_name.rs first
    let module_rs = parent_dir.join(format!("{module_name}.rs"));
    if module_rs.exists() {
        debug!("Found module file: {module_rs}");
        return Ok(module_rs);
    }

    // Try module_name/mod.rs
    let module_mod_rs = parent_dir.join(module_name).join("mod.rs");
    if module_mod_rs.exists() {
        debug!("Found module mod.rs: {module_mod_rs}");
        return Ok(module_mod_rs);
    }

    Err(Error::ModuleNotFound {
        module: module_name.to_string(),
        parent_file: parent_file.to_string(),
    })
}

/// Find and collect files containing procedural macro implementations
///
/// # Errors
/// Returns an error if files cannot be read or processed
fn collect_macro_implementation_files(
    package_root: &Utf8Path,
    required_files: &mut HashSet<Utf8PathBuf>,
) -> Result<(), Error> {
    debug!("Searching for procedural macro implementation files in: {package_root}");

    let src_dir = package_root.join("src");
    if !src_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(&src_dir) {
        let entry = entry.map_err(|e| Error::IoError {
            path: src_dir.to_string(),
            error: e.to_string(),
        })?;

        if let Some(path_str) = entry.path().to_str() {
            if path_str.ends_with(".rs") {
                let rust_file_path = Utf8PathBuf::try_from(entry.path().to_path_buf())?;

                // Skip files we've already processed
                if required_files.contains(&rust_file_path) {
                    continue;
                }

                // Skip test files and examples
                if should_exclude_rust_file(&rust_file_path) {
                    continue;
                }

                if contains_procedural_macro_attributes(&rust_file_path)? {
                    debug!("Found procedural macro implementation in: {rust_file_path}");
                    required_files.insert(rust_file_path);
                }
            }
        }
    }

    Ok(())
}

/// Check if a Rust file contains procedural macro attributes
///
/// # Errors
/// Returns an error if the file cannot be read
fn contains_procedural_macro_attributes(file_path: &Utf8Path) -> Result<bool, Error> {
    let content = fs::read_to_string(file_path).map_err(|e| Error::IoError {
        path: file_path.to_string(),
        error: e.to_string(),
    })?;

    // Look for procedural macro attributes
    let macro_attributes = [
        "#[inline_macro]",
        "#[attribute_macro]",
        "#[derive_macro]",
        "#[post_process]",
    ];

    for attr in &macro_attributes {
        if content.contains(attr) {
            debug!("Found procedural macro attribute {attr} in {file_path}");
            return Ok(true);
        }
    }

    Ok(false)
}

/// Check if a Rust file should be excluded (tests, examples, etc.)
fn should_exclude_rust_file(file_path: &Utf8Path) -> bool {
    let path_str = file_path.as_str();

    // Exclude test files
    if path_str.contains("/tests/")
        || path_str.contains("/test/")
        || path_str.contains("test_")
        || path_str.ends_with("_test.rs")
    {
        return true;
    }

    // Exclude examples and benchmarks
    if path_str.contains("/examples/")
        || path_str.contains("/benches/")
        || path_str.contains("/doc/")
    {
        return true;
    }

    // Exclude main.rs (not used in cdylib crates)
    if path_str.ends_with("/main.rs") {
        return true;
    }

    false
}

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

    #[test]
    fn test_procedural_macro_detection() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf()).unwrap();

        // Create Scarb.toml with cairo-plugin section
        let scarb_toml_content = r#"
[package]
name = "test-macro"
version = "0.1.0"

[cairo-plugin]
"#;
        let scarb_toml_path = temp_path.join("Scarb.toml");
        std::fs::write(&scarb_toml_path, scarb_toml_content).unwrap();

        // Test detection
        let is_proc_macro = is_cairo_procedural_macro_package(&scarb_toml_path).unwrap();
        assert!(is_proc_macro);

        // Create Scarb.toml without cairo-plugin section
        let normal_scarb_toml = r#"
[package]
name = "test-normal"
version = "0.1.0"
"#;
        let normal_scarb_path = temp_path.join("normal_Scarb.toml");
        std::fs::write(&normal_scarb_path, normal_scarb_toml).unwrap();

        let is_not_proc_macro = is_cairo_procedural_macro_package(&normal_scarb_path).unwrap();
        assert!(!is_not_proc_macro);
    }

    #[test]
    fn test_cargo_toml_validation() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf()).unwrap();

        // Create valid Cargo.toml for procedural macro
        let valid_cargo_toml = r#"
[package]
name = "test-macro"
version = "0.1.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
cairo-lang-macro = "0.1.0"
"#;
        let cargo_toml_path = temp_path.join("Cargo.toml");
        std::fs::write(&cargo_toml_path, valid_cargo_toml).unwrap();

        let is_valid = validate_cargo_toml_for_proc_macro(&cargo_toml_path).unwrap();
        assert!(is_valid);

        // Create invalid Cargo.toml (missing cdylib)
        let invalid_cargo_toml = r#"
[package]
name = "test-normal"
version = "0.1.0"

[dependencies]
cairo-lang-macro = "0.1.0"
"#;
        let invalid_cargo_path = temp_path.join("invalid_Cargo.toml");
        std::fs::write(&invalid_cargo_path, invalid_cargo_toml).unwrap();

        let is_invalid = validate_cargo_toml_for_proc_macro(&invalid_cargo_path).unwrap();
        assert!(!is_invalid);
    }

    #[test]
    fn test_module_declaration_parsing() {
        let rust_code = r#"
// This is a comment
pub mod utils;
mod macros;
pub(crate) mod helper;
mod inline_module {
    // inline content
}

// More comments
use std::collections::HashMap;
"#;

        let modules = parse_module_declarations(rust_code);
        assert_eq!(modules.len(), 4);
        assert!(modules.contains(&"utils".to_string()));
        assert!(modules.contains(&"macros".to_string()));
        assert!(modules.contains(&"helper".to_string()));
        assert!(modules.contains(&"inline_module".to_string()));
    }

    #[test]
    fn test_module_name_extraction() {
        assert_eq!(
            extract_module_name_from_line("mod test;"),
            Some("test".to_string())
        );
        assert_eq!(
            extract_module_name_from_line("pub mod test;"),
            Some("test".to_string())
        );
        assert_eq!(
            extract_module_name_from_line("pub(crate) mod test;"),
            Some("test".to_string())
        );
        assert_eq!(
            extract_module_name_from_line("mod test {"),
            Some("test".to_string())
        );
        assert_eq!(extract_module_name_from_line("// mod test;"), None);
        assert_eq!(extract_module_name_from_line("use mod_something;"), None);
    }

    #[test]
    fn test_valid_module_name() {
        assert!(is_valid_module_name("valid_name"));
        assert!(is_valid_module_name("test123"));
        assert!(is_valid_module_name("_underscore"));
        assert!(!is_valid_module_name("123invalid"));
        assert!(!is_valid_module_name(""));
        assert!(!is_valid_module_name("invalid-name"));
    }

    #[test]
    fn test_procedural_macro_attribute_detection() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf()).unwrap();

        // Create Rust file with procedural macro attributes
        let rust_with_macro = r#"
use cairo_lang_macro::TokenStream;

#[inline_macro]
pub fn my_inline_macro(_input: TokenStream) -> TokenStream {
    // implementation
}

#[attribute_macro]
pub fn my_attribute(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // implementation
}
"#;
        let macro_file_path = temp_path.join("macro_impl.rs");
        std::fs::write(&macro_file_path, rust_with_macro).unwrap();

        let has_macros = contains_procedural_macro_attributes(&macro_file_path).unwrap();
        assert!(has_macros);

        // Create Rust file without procedural macro attributes
        let rust_without_macro = r#"
pub fn regular_function() -> i32 {
    42
}

#[derive(Debug)]
struct MyStruct {
    field: String,
}
"#;
        let normal_file_path = temp_path.join("normal.rs");
        std::fs::write(&normal_file_path, rust_without_macro).unwrap();

        let has_no_macros = contains_procedural_macro_attributes(&normal_file_path).unwrap();
        assert!(!has_no_macros);
    }

    #[test]
    fn test_rust_file_exclusion() {
        assert!(should_exclude_rust_file(&Utf8PathBuf::from(
            "/src/tests/test.rs"
        )));
        assert!(should_exclude_rust_file(&Utf8PathBuf::from(
            "/src/test_utils.rs"
        )));
        assert!(should_exclude_rust_file(&Utf8PathBuf::from(
            "/src/examples/example.rs"
        )));
        assert!(should_exclude_rust_file(&Utf8PathBuf::from("/src/main.rs")));
        assert!(should_exclude_rust_file(&Utf8PathBuf::from(
            "/benches/bench.rs"
        )));

        assert!(!should_exclude_rust_file(&Utf8PathBuf::from("/src/lib.rs")));
        assert!(!should_exclude_rust_file(&Utf8PathBuf::from(
            "/src/macros.rs"
        )));
        assert!(!should_exclude_rust_file(&Utf8PathBuf::from(
            "/src/utils.rs"
        )));
    }
}
