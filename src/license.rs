use camino::Utf8Path;
use log::{debug, warn};
use spdx::LicenseId;
use std::fs;

/// Represents different ways a license can be specified
#[derive(Debug, Clone)]
pub enum LicenseInfo {
    /// License specified via CLI argument
    Cli(LicenseId),
    /// License found directly in Scarb.toml
    Manifest(String),
    /// No license specified anywhere
    None,
}

impl LicenseInfo {
    /// Get the display string for the license
    pub fn display_string(&self) -> &str {
        match self {
            Self::Cli(id) => match id.name {
                // Map common license names to their SPDX identifiers
                "MIT License" => "MIT",
                "Apache License 2.0" => "Apache-2.0",
                "GNU General Public License v3.0 only" => "GPL-3.0-only",
                "BSD 3-Clause License" => "BSD-3-Clause",
                other => other,
            },
            Self::Manifest(direct) => direct,
            Self::None => "NONE",
        }
    }

    /// Check if license is specified
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Extract license information from various sources
pub fn resolve_license_info(
    cli_license: Option<LicenseId>,
    path_license: Option<LicenseId>,
    manifest_path: &Utf8Path,
) -> LicenseInfo {
    // Priority: CLI > path license > manifest license
    if let Some(license) = cli_license {
        return LicenseInfo::Cli(license);
    }

    if let Some(license) = path_license {
        return LicenseInfo::Cli(license);
    }

    if let Some(manifest_license) = extract_license_from_manifest(manifest_path) {
        return LicenseInfo::Manifest(manifest_license);
    }

    LicenseInfo::None
}

/// Extract license from Scarb.toml file
fn extract_license_from_manifest(manifest_path: &Utf8Path) -> Option<String> {
    let toml_content = fs::read_to_string(manifest_path).ok()?;

    // Look for exact "license = " field, not "license-file" or other variants
    if let Some(license_line) = toml_content.lines().find(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("license ") && trimmed.contains('=')
    }) {
        if let Some(license_value) = license_line.split('=').nth(1) {
            let license = license_value.trim().trim_matches('"').trim_matches('\'');
            debug!("Found license in Scarb.toml: {license}");
            return Some(license.to_string());
        }
    }

    None
}

/// Warn if no license is provided
pub fn warn_if_no_license(license_info: &LicenseInfo) {
    if license_info.is_none() {
        warn!("No license provided via CLI or in Scarb.toml, defaults to All Rights Reserved");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_license_info_display_string() {
        // Test manifest license
        let manifest_license = LicenseInfo::Manifest("MIT".to_string());
        assert_eq!(manifest_license.display_string(), "MIT");

        // Test no license
        let no_license = LicenseInfo::None;
        assert_eq!(no_license.display_string(), "NONE");
    }

    #[test]
    fn test_license_info_is_none() {
        let manifest_license = LicenseInfo::Manifest("MIT".to_string());
        assert!(!manifest_license.is_none());

        let no_license = LicenseInfo::None;
        assert!(no_license.is_none());
    }

    #[test]
    fn test_resolve_license_info_priority() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("Scarb.toml");
        let manifest_utf8_path = camino::Utf8PathBuf::from_path_buf(manifest_path).unwrap();

        // Create manifest with license
        fs::write(
            &manifest_utf8_path,
            r#"
[package]
name = "test"
version = "1.0.0"
license = "Apache-2.0"
"#,
        )
        .unwrap();

        // Manifest should be used when neither CLI nor path specified
        let result = resolve_license_info(None, None, &manifest_utf8_path);
        assert!(matches!(result, LicenseInfo::Manifest(_)));
        assert_eq!(result.display_string(), "Apache-2.0");
    }

    #[test]
    fn test_extract_license_from_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("Scarb.toml");
        let manifest_utf8_path = camino::Utf8PathBuf::from_path_buf(manifest_path).unwrap();

        // Test with double quotes
        fs::write(
            &manifest_utf8_path,
            r#"
[package]
name = "test"
version = "1.0.0"
license = "MIT"
"#,
        )
        .unwrap();

        let result = extract_license_from_manifest(&manifest_utf8_path);
        assert_eq!(result, Some("MIT".to_string()));

        // Test with single quotes
        fs::write(
            &manifest_utf8_path,
            r#"
[package]
name = "test"
version = "1.0.0"
license = 'Apache-2.0'
"#,
        )
        .unwrap();

        let result = extract_license_from_manifest(&manifest_utf8_path);
        assert_eq!(result, Some("Apache-2.0".to_string()));

        // Test with no license
        fs::write(
            &manifest_utf8_path,
            r#"
[package]
name = "test"
version = "1.0.0"
"#,
        )
        .unwrap();

        let result = extract_license_from_manifest(&manifest_utf8_path);
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_license_info_none() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("Scarb.toml");
        let manifest_utf8_path = camino::Utf8PathBuf::from_path_buf(manifest_path).unwrap();

        // Create manifest without license
        fs::write(
            &manifest_utf8_path,
            r#"
[package]
name = "test"
version = "1.0.0"
"#,
        )
        .unwrap();

        let result = resolve_license_info(None, None, &manifest_utf8_path);
        assert!(matches!(result, LicenseInfo::None));
        assert_eq!(result.display_string(), "NONE");
    }
}
