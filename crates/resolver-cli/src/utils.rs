use camino::Utf8PathBuf;
use dyn_compiler::dyn_compiler::{SupportedCairoVersions, SupportedScarbVersions};
use scarb_metadata::MetadataCommand;
use semver::Version;

// format Version output into x.x.x string
pub fn format_version(version: Version) -> String {
    format!(
        "{}.{}.{}",
        version.major.to_string(),
        version.minor.to_string(),
        version.patch.to_string(),
    )
}

pub fn detect_local_tools(path: &Utf8PathBuf) -> (SupportedScarbVersions, SupportedCairoVersions) {
    // init metadata command
    println!("init metadata from scarb-metadata");
    let mut cmd = MetadataCommand::new();
    let mut scarb_path = path.clone();
    scarb_path.push("Scarb.toml");
    cmd.manifest_path(scarb_path);

    if let Ok(metadata) = cmd.exec() {
        let scarb_ver_data = metadata.app_version_info.version;
        let scarb_ver_string = format_version(scarb_ver_data);
        let scarb_version = match &*scarb_ver_string {
            // "0.4.0" => SupportedScarbVersions::V0_4_0,
            // "0.4.1" => SupportedScarbVersions::V0_4_1,
            // "0.5.0" => SupportedScarbVersions::V0_5_0,
            // "0.5.1" => SupportedScarbVersions::V0_5_1,
            // "0.5.2" => SupportedScarbVersions::V0_5_2,
            // "0.7.0" => SupportedScarbVersions::V0_7_0,
            // "2.4.3" => SupportedScarbVersions::V2_4_3,
            "2.6.4" => SupportedScarbVersions::V2_6_4,
            _ => panic!("Unsupported scarb version: {}", scarb_ver_string),
        };
        println!("scarb ver: {:?}", scarb_version);
        let cairo_ver_data = metadata.app_version_info.cairo.version;
        let cairo_ver_string = format_version(cairo_ver_data);

        let cairo_version = match &*cairo_ver_string {
            // "1.1.0" => SupportedCairoVersions::V1_1_0,
            // "1.1.1" => SupportedCairoVersions::V1_1_1,
            // "2.0.0" => SupportedCairoVersions::V2_0_0,
            // "2.0.1" => SupportedCairoVersions::V2_0_1,
            // "2.0.2" => SupportedCairoVersions::V2_0_2,
            // "2.2.0" => SupportedCairoVersions::V2_2_0,
            // "2.4.3" => SupportedCairoVersions::V2_4_3,
            "2.6.3" => SupportedCairoVersions::V2_6_3,
            _ => panic!("Unsupported cairo version: {}", cairo_ver_string),
        };
        println!("cairo ver: {:?}", cairo_version);
        (scarb_version, cairo_version)
    } else {
        println!("scarb-metadata execution failure");
        std::process::exit(1);
    }
}
