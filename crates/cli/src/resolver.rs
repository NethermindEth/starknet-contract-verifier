// use voyager_resolver_cairo_1_1_0::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV1_1_0;
// use voyager_resolver_cairo_1_1_1::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV1_1_1;
// use voyager_resolver_cairo_2_0_0::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV2_0_0;
// use voyager_resolver_cairo_2_0_1::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV2_0_1;
// use voyager_resolver_cairo_2_0_2::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV2_0_2;
// use voyager_resolver_cairo_2_1_0::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV2_1_0;
// use voyager_resolver_cairo_2_1_1::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV2_1_1;
// use voyager_resolver_cairo_2_2_0::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGeneratorV2_2_0;
use voyager_resolver_cairo::dyn_compiler::VoyagerGeneratorWrapper as VoyagerGenerator;

use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedCairoVersions};

pub fn get_dynamic_compiler(cairo_version: SupportedCairoVersions) -> Box<dyn DynamicCompiler> {
    match cairo_version {
        // SupportedCairoVersions::V1_1_0 => Box::new(VoyagerGeneratorV1_1_0),
        // SupportedCairoVersions::V1_1_1 => Box::new(VoyagerGeneratorV1_1_1),
        // SupportedCairoVersions::V2_0_0 => Box::new(VoyagerGeneratorV2_0_0),
        // SupportedCairoVersions::V2_0_1 => Box::new(VoyagerGeneratorV2_0_1),
        // SupportedCairoVersions::V2_0_2 => Box::new(VoyagerGeneratorV2_0_2),
        // SupportedCairoVersions::V2_1_0 => Box::new(VoyagerGeneratorV2_1_0),
        // SupportedCairoVersions::V2_1_1 => Box::new(VoyagerGeneratorV2_1_1),
        // SupportedCairoVersions::V2_2_0 => Box::new(VoyagerGeneratorV2_2_0),
        SupportedCairoVersions::V2_6_0 => Box::new(VoyagerGenerator),
    }
}
