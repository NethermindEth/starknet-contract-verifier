use reqwest::StatusCode;
use scarb_metadata::{Metadata, PackageId};
use std::fmt::{self, Formatter};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub struct MissingPackage {
    pub package_id: PackageId,
    pub available: Vec<PackageId>,
}

impl MissingPackage {
    #[must_use]
    pub fn new(package_id: &PackageId, metadata: &Metadata) -> Self {
        Self {
            package_id: package_id.clone(),
            available: metadata.workspace.members.clone(),
        }
    }
}

impl fmt::Display for MissingPackage {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        writeln!(
            formatter,
            "❌ Package '{}' not found in workspace",
            self.package_id
        )?;
        writeln!(formatter)?;
        writeln!(formatter, "💡 Available packages:")?;
        for package in &self.available {
            writeln!(formatter, "   • {}", package)?;
        }
        writeln!(formatter)?;
        writeln!(formatter, "🔧 Try: --package <package-name>")?;

        Ok(())
    }
}

#[derive(Debug, Error)]
pub struct RequestFailure {
    pub url: Url,
    pub status: StatusCode,
    pub msg: String,
}

impl RequestFailure {
    pub fn new(url: Url, status: StatusCode, msg: impl Into<String>) -> Self {
        Self {
            url,
            status,
            msg: msg.into(),
        }
    }
}

impl fmt::Display for RequestFailure {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        writeln!(formatter, "❌ API request failed")?;
        writeln!(formatter)?;
        writeln!(formatter, "🌐 URL: {}", self.url)?;
        writeln!(formatter, "📊 Status: {}", self.status)?;
        writeln!(formatter, "📝 Response: {}", self.msg)?;
        writeln!(formatter)?;

        match self.status.as_u16() {
            400 => {
                if self.msg.to_lowercase().contains("already verified") {
                    writeln!(
                        formatter,
                        "✅ Good news! This contract class is already verified on Voyager."
                    )?;
                    writeln!(
                        formatter,
                        "🔗 You can view it at: https://voyager.online/class/{}",
                        self.url
                            .path()
                            .split('/')
                            .next_back()
                            .unwrap_or("<CLASS-HASH>")
                    )?;
                } else {
                    writeln!(formatter, "💡 This usually means invalid request data. Check your class hash and contract details.")?;
                }
            }
            401 | 403 => writeln!(
                formatter,
                "💡 Authentication issue. Check your API credentials."
            )?,
            404 => writeln!(
                formatter,
                "💡 Resource not found. Verify the class hash is declared on the network."
            )?,
            429 => writeln!(
                formatter,
                "💡 Rate limit exceeded. Please wait before trying again."
            )?,
            500..=599 => writeln!(
                formatter,
                "💡 Server error. Please try again later or contact support."
            )?,
            _ => {}
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub struct MissingContract {
    pub name: String,
    pub available: Vec<String>,
}

impl MissingContract {
    #[must_use]
    pub const fn new(name: String, available: Vec<String>) -> Self {
        Self { name, available }
    }

    fn find_suggestions(&self) -> Vec<String> {
        let mut suggestions = Vec::new();
        let name_lower = self.name.to_lowercase();

        for available in &self.available {
            let available_lower = available.to_lowercase();
            if available_lower.contains(&name_lower) || name_lower.contains(&available_lower) {
                suggestions.push(available.clone());
            }
        }

        if suggestions.is_empty() {
            self.available.clone()
        } else {
            suggestions
        }
    }
}

impl fmt::Display for MissingContract {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        if self.name.starts_with("Workspace project detected") {
            writeln!(formatter, "❌ Workspace project detected")?;
            writeln!(formatter)?;
            writeln!(formatter, "📦 This is a workspace with multiple packages. You must specify which package to verify.")?;
            writeln!(formatter)?;
            writeln!(formatter, "💡 Available packages:")?;
            for package in &self.available {
                writeln!(formatter, "   • {package}")?;
            }
            writeln!(formatter)?;
            writeln!(formatter, "🔧 Try: --package <package-name>")?;
        } else {
            writeln!(
                formatter,
                "❌ Contract '{name}' not found",
                name = self.name
            )?;
            writeln!(formatter)?;

            let suggestions = self.find_suggestions();
            if suggestions.len() < self.available.len() {
                writeln!(formatter, "🔍 Did you mean one of these?")?;
                for suggestion in &suggestions {
                    writeln!(formatter, "   • {suggestion}")?;
                }
            } else {
                writeln!(formatter, "📋 Available contracts:")?;
                for contract in &self.available {
                    writeln!(formatter, "   • {contract}")?;
                }
            }
            writeln!(formatter)?;
            writeln!(formatter, "🔧 Try: --contract-name <contract-name>")?;
        }

        Ok(())
    }
}
