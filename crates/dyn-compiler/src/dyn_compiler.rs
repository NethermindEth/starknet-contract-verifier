use anyhow::Result;
use camino::Utf8PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum SupportedCairoVersions {
    // V1_1_0,
    // V1_1_1,
    // V2_0_0,
    // V2_0_1,
    // V2_0_2,
    // V2_1_0,
    // V2_1_1,
    // V2_2_0,
    V2_6_0,
}

impl ToString for SupportedCairoVersions {
    fn to_string(&self) -> String {
        match self {
            // SupportedCairoVersions::V1_1_0 => "1.1.0".into(),
            // SupportedCairoVersions::V1_1_1 => "1.1.1".into(),
            // SupportedCairoVersions::V2_0_0 => "2.0.0".into(),
            // SupportedCairoVersions::V2_0_1 => "2.0.1".into(),
            // SupportedCairoVersions::V2_0_2 => "2.0.2".into(),
            // SupportedCairoVersions::V2_1_0 => "2.1.0".into(),
            // SupportedCairoVersions::V2_1_1 => "2.1.1".into(),
            // SupportedCairoVersions::V2_2_0 => "2.2.0".into(),
            SupportedCairoVersions::V2_6_0 => "2.6.0".into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SupportedScarbVersions {
    // V0_4_0,
    // V0_4_1,
    // V0_5_0,
    // V0_5_1,
    // V0_5_2,
    // V0_6_1,
    // V0_6_2,
    // V0_7_0,
    V2_6_0,
}

impl ToString for SupportedScarbVersions {
    fn to_string(&self) -> String {
        match self {
            // SupportedScarbVersions::V0_4_0 => "0.4.0".into(),
            // SupportedScarbVersions::V0_4_1 => "0.4.1".into(),
            // SupportedScarbVersions::V0_5_0 => "0.5.0".into(),
            // SupportedScarbVersions::V0_5_1 => "0.5.1".into(),
            // SupportedScarbVersions::V0_5_2 => "0.5.2".into(),
            // SupportedScarbVersions::V0_6_1 => "0.6.1".into(),
            // SupportedScarbVersions::V0_6_2 => "0.6.2".into(),
            // SupportedScarbVersions::V0_7_0 => "0.7.0".into(),
            SupportedScarbVersions::V2_6_0 => "2.6.0".into(),
        }
    }
}

/**
 * This trait is required to be implemented by the voyager resolvers.
 * This allows us to use multiple version of scarb + cairo in the same project,
 * and compile scarb projects easily,
 */
pub trait DynamicCompiler {
    fn get_supported_scarb_versions(&self) -> Vec<SupportedScarbVersions>;

    fn get_supported_cairo_versions(&self) -> Vec<SupportedCairoVersions>;

    fn get_contracts_to_verify_path(&self, project_path: &Utf8PathBuf) -> Result<Vec<Utf8PathBuf>>;

    fn compile_project(&self, project_path: &Utf8PathBuf) -> Result<()>;

    fn compile_file(&self, file_path: &Utf8PathBuf) -> Result<()>;
}
