use clap::{builder::PossibleValue, ValueEnum};
use strum_macros::EnumIter;

#[derive(Debug, Clone, EnumIter, Copy)]
pub enum LicenseType {
    NoLicense,
    Unlicense,
    MIT,
    GPLv2,
    GPLv3,
    LGPLv2_1,
    LGPLv3,
    BSD2Clause,
    BSD3Clause,
    MPL2,
    OSL3,
    Apache2,
    AGPLv3,
    BSL1_1,
}

impl LicenseType {
    pub fn to_long_string(&self) -> String {
        let string_repr = match *self {
            Self::NoLicense => "No License (None)",
            Self::Unlicense => "The Unlicense (Unlicense)",
            Self::MIT => "MIT License (MIT)",
            Self::GPLv2 => "GNU General Public License v2.0 (GNU GPLv2)",
            Self::GPLv3 => "GNU General Public License v3.0 (GNU GPLv3)",
            Self::LGPLv2_1 => "GNU Lesser General Public License v2.1 (GNU LGPLv2.1)",
            Self::LGPLv3 => "GNU Lesser General Public License v3.0 (GNU LGPLv3)",
            Self::BSD2Clause => "BSD 2-clause \"Simplified\" license (BSD-2-Clause)",
            Self::BSD3Clause => "BSD 3-clause \"New\" Or Revisited license (BSD-3-Clause)",
            Self::MPL2 => "Mozilla Public License 2.0 (MPL-2.0)",
            Self::OSL3 => "Open Software License 3.0 (OSL-3.0)",
            Self::Apache2 => "Apache 2.0 (Apache-2.0)",
            Self::AGPLv3 => "GNU Affero General Public License (GNU AGPLv3)",
            Self::BSL1_1 => "Business Source License (BSL 1.1)",
        };
        string_repr.to_owned()
    }
}

impl ToString for LicenseType {
    fn to_string(&self) -> String {
        let string_repr = match *self {
            Self::NoLicense => "NoLicense",
            Self::Unlicense => "Unlicense",
            Self::MIT => "MIT",
            Self::GPLv2 => "GPLv2",
            Self::GPLv3 => "GPLv3",
            Self::LGPLv2_1 => "LGPLv2_1",
            Self::LGPLv3 => "LGPLv3",
            Self::BSD2Clause => "BSD2Clause",
            Self::BSD3Clause => "BSD3Clause",
            Self::MPL2 => "MPL2",
            Self::OSL3 => "OSL3",
            Self::Apache2 => "Apache2",
            Self::AGPLv3 => "AGPLv3",
            Self::BSL1_1 => "BSL1_1",
        };
        string_repr.to_owned()
    }
}

impl ValueEnum for LicenseType {
    fn from_str(input: &str, _ignore_case: bool) -> std::result::Result<Self, String> {
        match input {
            "NoLicense" => Ok(LicenseType::NoLicense),
            "Unlicense" => Ok(LicenseType::Unlicense),
            "MIT" => Ok(LicenseType::MIT),
            "GPLv2" => Ok(LicenseType::GPLv2),
            "GPLc3" => Ok(LicenseType::GPLv3),
            "LGPLv2_1" => Ok(LicenseType::LGPLv2_1),
            "LGPLv3" => Ok(LicenseType::LGPLv3),
            "BSD2Clause" => Ok(LicenseType::BSD2Clause),
            "BSD3Clause" => Ok(LicenseType::BSD3Clause),
            "MPL2" => Ok(LicenseType::MPL2),
            "OSL3" => Ok(LicenseType::OSL3),
            "Apache2" => Ok(LicenseType::Apache2),
            "AGPLv3" => Ok(LicenseType::AGPLv3),
            "BSL1_1" => Ok(LicenseType::BSL1_1),
            _ => Err(format!("Unknown license type: {}", input)),
        }
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        PossibleValue::new(self.to_string()).into()
    }

    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::NoLicense,
            Self::Unlicense,
            Self::MIT,
            Self::GPLv2,
            Self::GPLv3,
            Self::LGPLv2_1,
            Self::LGPLv3,
            Self::BSD2Clause,
            Self::BSD3Clause,
            Self::MPL2,
            Self::OSL3,
            Self::Apache2,
            Self::AGPLv3,
            Self::BSL1_1,
        ]
    }
}
