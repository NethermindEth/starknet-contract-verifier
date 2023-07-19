// use camino::Utf8PathBuf;
// use scarb::compiler::CompilerRepository;
// use scarb::core::Config;
// use scarb::ops;
// use scarb::ui::Verbosity;


// pub fn dynamic_compile_project(source_dir: Utf8Path) -> Result<()>{
//     let mut compilers = CompilerRepository::empty();
//     compilers.add(Box::new(get_resolver(CairoVersion::V2_0_1).try_into().unwrap())).unwrap();
    
//     let manifest_path = source_dir.join("Scarb.toml");
    
//     let config = Config::builder(manifest_path)
//         .ui_verbosity(Verbosity::Verbose)
//         .log_filter_directive(env::var_os("SCARB_LOG"))
//         .compilers(compilers)
//         .build()
//         .unwrap();
    
//     let ws = ops::read_workspace(config.manifest_path(), &config).unwrap()
//     ops::compile(&ws)
// }

use camino::Utf8Path;


pub enum SupportedCairoVersions {
        V1_1_0,
        V1_1_1,
        V2_0_0,
        V2_0_1,
        V2_0_2,
}

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
    fn get_versions(&self) -> (SupportedScarbVersions, SupportedCairoVersions);

    fn compile_project(
        &self,
        project_path: Utf8Path
    ) -> Result<(), &str>;

    fn compile_file(
        &self,
        file_path: Utf8Path
    ) -> Result<(), &str>;
}