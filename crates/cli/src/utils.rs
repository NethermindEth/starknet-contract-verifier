use dyn_compiler::dyn_compiler::{SupportedCairoVersions, SupportedScarbVersions};
use std::process::Command;

const SCARB_VERSION_OUTPUT_LINES: usize = 3;

pub fn detect_local_tools() -> (SupportedScarbVersions, SupportedCairoVersions) {
    let versioning = Command::new("scarb").arg("--version").output().expect(
        "
            Unable to detect local Scarb installation. 
            This CLI depends on Scarb and thus require it to be installed in the local machine.
            You can install Scarb at https://docs.swmansion.com/scarb/. 
        ",
    );

    let versioning_str = String::from_utf8(versioning.stdout).unwrap();
    let version_list = versioning_str
        .split('\n')
        .filter(|x| !x.is_empty())
        .collect::<Vec<&str>>();
    if version_list.len() != SCARB_VERSION_OUTPUT_LINES {
        panic!("{}", String::from_utf8(versioning.stderr).unwrap());
    }
    let scarb_version = versioning_str.split('\n').collect::<Vec<&str>>()[0]
        .split(" ")
        .collect::<Vec<&str>>()[1];
    let cairo_version = versioning_str.split('\n').collect::<Vec<&str>>()[1]
        .split(" ")
        .collect::<Vec<&str>>()[1];

    let scarb_version = match scarb_version {
        "0.5.1" => SupportedScarbVersions::V0_5_1,
        _ => panic!("Unsupported scarb version: {}", scarb_version),
    };

    let cairo_version = match cairo_version {
        "2.0.1" => SupportedCairoVersions::V2_0_1,
        _ => {
            println!("Unsupported cairo version {}. We thus do not guarantee compatibility and compilation might fail as a result.", cairo_version);
            // Use latest Scarb version as default.
            SupportedCairoVersions::V2_0_1
        }
    };

    (scarb_version, cairo_version)
}
