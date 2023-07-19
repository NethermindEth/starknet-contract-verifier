use std::env;

use anyhow::Result;
use camino::Utf8PathBuf;
use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedCairoVersions, SupportedScarbVersions};
use scarb::{ops, compiler::CompilerRepository, core::Config, ui::Verbosity};

use crate::{compiler::VoyagerGenerator, utils::run_starknet_compile};

pub struct VoyagerGeneratorWrapper;

impl DynamicCompiler for VoyagerGeneratorWrapper {
    fn get_supported_scarb_versions(&self) -> Vec<SupportedScarbVersions> {
        vec![SupportedScarbVersions::V0_4_0, SupportedScarbVersions::V0_4_1]
    }

    fn get_supported_cairo_versions(&self) -> Vec<SupportedCairoVersions> {
        vec![SupportedCairoVersions::V1_1_0, SupportedCairoVersions::V1_1_1]
    }

    fn compile_project(
        &self,
        project_path: Utf8PathBuf
    ) -> Result<()> {
        let manifest_path = project_path.join("Scarb.toml");

        let mut compilers  = CompilerRepository::empty();
        compilers.add(Box::new(VoyagerGenerator)).unwrap();

        let config = Config::builder(manifest_path)
                .ui_verbosity(Verbosity::Verbose)
                .log_filter_directive(env::var_os("SCARB_LOG"))
                .compilers(compilers)
                .build()
                .unwrap();

            let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();
            ops::compile(&ws)
    }

    fn compile_file(
        &self,
        file_path: Utf8PathBuf
    ) -> Result<()> {
        //TODO detect_corelib will try to use the local corelib.
        // Once cairo is released, it will probably be able to use
        // the corelib from the release.
        run_starknet_compile(file_path.as_str())
    }
}