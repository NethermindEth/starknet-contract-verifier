use std::process::Command;

use dyn_compiler::dyn_compiler::{SupportedCairoVersions, SupportedScarbVersions};

pub fn detect_local_tools() -> (SupportedScarbVersions, SupportedCairoVersions) {
    let versioning = Command::new("scarb").arg("--version").output().expect(
        "
            Unable to detect local Scarb installation. 
            This CLI depends on Scarb and thus require it to be installed in the local machine.
            You can install Scarb at https://docs.swmansion.com/scarb/. 
        ",
    );

    let versioning_str = String::from_utf8(versioning.stdout).unwrap();
    let scarb_version = versioning_str.split('\n').collect::<Vec<&str>>()[0]
        .split(" ")
        .collect::<Vec<&str>>()[1];
    let cairo_version = versioning_str.split('\n').collect::<Vec<&str>>()[1]
        .split(" ")
        .collect::<Vec<&str>>()[1];

    let scarb_version = match scarb_version {
        "2.4.0" => SupportedScarbVersions::V2_4_0,
        "2.4.1" => SupportedScarbVersions::V2_4_1,
        "2.4.2" => SupportedScarbVersions::V2_4_2,
        "2.4.3" => SupportedScarbVersions::V2_4_3,
        _ => {
            println!("Unsupported scarb version {}. We thus do not guarantee compatibility and compilation might fail as a result.", scarb_version);
            // Use latest Scarb version as default.
            SupportedScarbVersions::V2_4_3
        }
    };

    let cairo_version = match cairo_version {
        "2.4.0" => SupportedCairoVersions::V2_4_0,
        "2.4.1" => SupportedCairoVersions::V2_4_1,
        "2.4.2" => SupportedCairoVersions::V2_4_2,
        "2.4.3" => SupportedCairoVersions::V2_4_3,
        _ => {
            println!("Unsupported scarb version {}. We thus do not guarantee compatibility and compilation might fail as a result.", scarb_version);
            // Use latest Scarb version as default.
            SupportedCairoVersions::V2_4_3
        }
    };

    (scarb_version, cairo_version)
}
