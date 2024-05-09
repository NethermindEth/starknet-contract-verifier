use voyager_resolver_cairo::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGenerator;

use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedCairoVersions};

pub fn get_dynamic_compiler(cairo_version: SupportedCairoVersions) -> Box<dyn DynamicCompiler> {
    match cairo_version {
        SupportedCairoVersions::V2_2_0 => Box::new(VoyagerGenerator),
    }
}
