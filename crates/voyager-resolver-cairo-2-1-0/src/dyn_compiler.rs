use std::env;

use anyhow::Result;
use camino::Utf8PathBuf;
use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedCairoVersions, SupportedScarbVersions};
use itertools::Itertools;
use scarb::{ops, compiler::CompilerRepository, core::Config, ui::Verbosity};

use crate::{compiler::{VoyagerGenerator, scarb_utils::get_contracts_to_verify}, utils::run_starknet_compile};

pub struct VoyagerGeneratorWrapper;

impl DynamicCompiler for VoyagerGeneratorWrapper {
    fn get_supported_scarb_versions(&self) -> Vec<SupportedScarbVersions> {
        vec![SupportedScarbVersions::V0_4_0, SupportedScarbVersions::V0_4_1]
    }
    fn get_supported_cairo_versions(&self) -> Vec<SupportedCairoVersions> {
        vec![SupportedCairoVersions::V1_1_0, SupportedCairoVersions::V1_1_1]
    }

    fn get_contracts_to_verify_path(&self, project_path: &Utf8PathBuf) -> Result<Vec<Utf8PathBuf>> {
        let manifest_path = project_path.join("Scarb.toml");
        
        let mut compilers  = CompilerRepository::empty();
        compilers.add(Box::new(VoyagerGenerator)).unwrap();

        let config = Config::builder(manifest_path)
            // .ui_verbosity(Verbosity::Verbose)
            .log_filter_directive(env::var_os("SCARB_LOG"))
            .compilers(compilers)
            .build()
            .unwrap();

        let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
            eprintln!("error: {}", err);
            std::process::exit(1);
        });
        //
        let package = ws.current_package().unwrap();
        let contracts_path = get_contracts_to_verify(package)?;

        Ok(contracts_path.iter().map(|p| Utf8PathBuf::from_path_buf(p.to_path_buf()).unwrap()).collect_vec())
        
    }

    fn compile_project(
        &self,
        project_path: &Utf8PathBuf
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
        let resolve = ops::resolve_workspace(&ws)?;
        let package_ids = resolve.packages.keys().cloned().collect();
        ops::compile(package_ids, &ws)
    }

    fn compile_file(
        &self,
        file_path: &Utf8PathBuf
    ) -> Result<()> {
        //TODO detect_corelib will try to use the local corelib.
        // Once cairo is released, it will probably be able to use
        // the corelib from the release.
        run_starknet_compile(file_path.as_str())
    }
}