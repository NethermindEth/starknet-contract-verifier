use anyhow::Result;
use camino::{Utf8PathBuf};


#[derive(Debug)]
pub enum SupportedCairoVersions {
        V1_1_0,
        V1_1_1,
        V2_0_0,
        V2_0_1,
        V2_0_2,
}

#[derive(Debug)]
pub enum SupportedScarbVersions {
    V0_4_0,
    V0_4_1,
    V0_5_0,
    V0_5_1,
    V0_5_2,
}

/**
 * This trait is required to be implemented by the voyager resolvers.
 * This allows us to use multiple version of scarb + cairo in the same project,
 * and compile scarb projects easily,
 */
pub trait DynamicCompiler {
    fn get_supported_scarb_versions(&self) -> Vec<SupportedScarbVersions>;

    fn get_supported_cairo_versions(&self) -> Vec<SupportedCairoVersions>;

    fn compile_project(
        &self,
        project_path: Utf8PathBuf
    ) -> Result<()>;

    fn compile_file(
        &self,
        file_path: Utf8PathBuf
    ) -> Result<()>;
}