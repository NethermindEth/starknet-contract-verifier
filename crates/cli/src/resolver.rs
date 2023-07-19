use voyager_resolver_cairo_1_1_0::compiler::VoyagerGenerator as VoyagerGeneratorV1_1_0;
use voyager_resolver_cairo_1_1_1::compiler::VoyagerGenerator as VoyagerGeneratorV1_1_1;
use voyager_resolver_cairo_2_0_0::compiler::VoyagerGenerator as VoyagerGeneratorV2_0_0;
use voyager_resolver_cairo_2_0_1::compiler::VoyagerGenerator as VoyagerGeneratorV2_0_1;
use voyager_resolver_cairo_2_0_2::compiler::VoyagerGenerator as VoyagerGeneratorV2_0_2;

use dyn_compiler::dyn_compiler::{DynamicCompiler, SupportedScarbVersions, SupportedCairoVersions};


pub enum VoyagerGeneratorWrapper {
    V1_1_0(VoyagerGeneratorV1_1_0),
    V1_1_1(VoyagerGeneratorV1_1_1),
    V2_0_0(VoyagerGeneratorV2_0_0),
    V2_0_1(VoyagerGeneratorV2_0_1),
    V2_0_2(VoyagerGeneratorV2_0_2),
}

// impl TryFrom<VoyagerGeneratorWrapper> for VoyagerGeneratorV1_1_0 {

// }

pub fn get_generator(cairo_version: SupportedCairoVersions) -> VoyagerGeneratorWrapper {
    match cairo_version {
        SupportedCairoVersions::V1_1_0 => {
            VoyagerGeneratorWrapper::V1_1_0(VoyagerGeneratorV1_1_0)
        },
        SupportedCairoVersions::V1_1_1 => {
            VoyagerGeneratorWrapper::V1_1_1(VoyagerGeneratorV1_1_1)
        },
        SupportedCairoVersions::V2_0_0 => {
            VoyagerGeneratorWrapper::V2_0_0(VoyagerGeneratorV2_0_0)
        },
        SupportedCairoVersions::V2_0_1 => {
            VoyagerGeneratorWrapper::V2_0_1(VoyagerGeneratorV2_0_1)
        },
        SupportedCairoVersions::V2_0_2 => {
            VoyagerGeneratorWrapper::V2_0_2(VoyagerGeneratorV2_0_2)
        },
    }
}