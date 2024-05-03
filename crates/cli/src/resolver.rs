use voyager_resolver_cairo_2_4_3::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV2_4_3;

use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedCairoVersions};

pub fn get_dynamic_compiler(cairo_version: SupportedCairoVersions) -> Box<dyn DynamicCompiler> {
    Box::new(VoyagerGeneratorV2_4_3)
}
