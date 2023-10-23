use voyager_resolver_cairo_2_2_0::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV2_2_0;

use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedCairoVersions};

pub fn get_dynamic_compiler(cairo_version: SupportedCairoVersions) -> Box<dyn DynamicCompiler> {
    match cairo_version {
        SupportedCairoVersions::V2_2_0 => Box::new(VoyagerGeneratorV2_2_0),
        _ => todo!()
    }
}
