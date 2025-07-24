//! Project type definitions and detection logic for build tool selection.
//!
//! This module provides functionality to detect and handle different types of Cairo projects:
//! - Regular Scarb projects (using `scarb build`)
//! - Dojo projects (using `sozo build`)
//! - Auto-detection based on dependencies and imports

/// Project type for build tool selection
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectType {
    /// Regular Scarb project (uses scarb build)
    Scarb,
    /// Dojo project (uses sozo build)
    Dojo,
    /// Auto-detect project type with interactive prompt
    Auto,
}

impl ProjectType {
    /// Get the build tool name for this project type
    pub const fn build_tool(&self) -> &'static str {
        match self {
            Self::Dojo => "sozo",
            _ => "scarb",
        }
    }
}

// Implement clap::ValueEnum for CLI usage
impl clap::ValueEnum for ProjectType {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Scarb, Self::Dojo, Self::Auto]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Scarb => clap::builder::PossibleValue::new("scarb")
                .help("Regular Scarb project (uses scarb build)"),
            Self::Dojo => {
                clap::builder::PossibleValue::new("dojo").help("Dojo project (uses sozo build)")
            }
            Self::Auto => clap::builder::PossibleValue::new("auto")
                .help("Auto-detect project type with interactive prompt"),
        })
    }
}

impl std::str::FromStr for ProjectType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "scarb" => Ok(Self::Scarb),
            "dojo" => Ok(Self::Dojo),
            "auto" => Ok(Self::Auto),
            _ => Err(format!(
                "Invalid project type: {s}. Valid options: scarb, dojo, auto"
            )),
        }
    }
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scarb => write!(f, "scarb"),
            Self::Dojo => write!(f, "dojo"),
            Self::Auto => write!(f, "auto"),
        }
    }
}
