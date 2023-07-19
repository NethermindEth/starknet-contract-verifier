use std::process::Command;

use dyn_compiler::dyn_compiler::{SupportedCairoVersions, SupportedScarbVersions};

pub fn detect_local_tools() -> (SupportedScarbVersions, SupportedCairoVersions) {
    let versioning = Command::new("scarb")
        .arg("--version")
        .output()
        .expect("Failed to detect local scarb.");

    let versioning_str = String::from_utf8(versioning.stdout).unwrap();
    let scarb_version = versioning_str.split('\n').collect::<Vec<&str>>()[0].split(" ").collect::<Vec<&str>>()[1];
    let cairo_version = versioning_str.split('\n').collect::<Vec<&str>>()[1].split(" ").collect::<Vec<&str>>()[1];

    let scarb_version = match scarb_version {
        "0.4.0" => SupportedScarbVersions::V0_4_0,
        "0.4.1" => SupportedScarbVersions::V0_4_1,
        "0.5.0" => SupportedScarbVersions::V0_5_0,
        "0.5.1" => SupportedScarbVersions::V0_5_1,
        "0.5.2" => SupportedScarbVersions::V0_5_2,
        _ => panic!("Unsupported scarb version: {}", scarb_version),
    };

    let cairo_version = match cairo_version {
        "1.1.0" => SupportedCairoVersions::V1_1_0,
        "1.1.1" => SupportedCairoVersions::V1_1_1,
        "2.0.0" => SupportedCairoVersions::V2_0_0,
        "2.0.1" => SupportedCairoVersions::V2_0_1,
        "2.0.2" => SupportedCairoVersions::V2_0_2,
        _ => panic!("Unsupported cairo version: {}", cairo_version),
    };

    (scarb_version, cairo_version)
}