use anyhow::Result;
use camino::Utf8PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum SupportedCairoVersions {
    V2_3_1,
}

impl ToString for SupportedCairoVersions {
    fn to_string(&self) -> String {
        match self {
            SupportedCairoVersions::V2_3_1 => "2.3.1".into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SupportedScarbVersions {
    V2_3_1,
}

impl ToString for SupportedScarbVersions {
    fn to_string(&self) -> String {
        match self {
            SupportedScarbVersions::V2_3_1 => "2.3.1".into(),
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
