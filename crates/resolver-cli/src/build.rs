use std::env::current_dir;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dyn_compiler::dyn_compiler::SupportedCairoVersions;

use crate::resolver::get_dynamic_compiler;

#[derive(Args, Debug)]
pub struct ResolveProjectArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,
}

#[derive(Args, Debug)]
pub struct ResolveFileArgs {
    #[clap(help = "File path")]
    path: Utf8PathBuf,
}

pub fn resolve_project(args: ResolveProjectArgs, cairo_version: SupportedCairoVersions) -> Result<()> {
    let source_dir = match args.path {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut current_path = current_dir().unwrap();
                current_path.push(path);
                Utf8PathBuf::from_path_buf(current_path).unwrap()
            }
        }
        None => Utf8PathBuf::from_path_buf(current_dir().unwrap()).unwrap(),
    };

    let compiler = get_dynamic_compiler(cairo_version);
    compiler.compile_project(&source_dir)
}
