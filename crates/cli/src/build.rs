use std::env::{self, current_dir};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;
use voyager_resolver_cairo_1_1_1::compiler::VoyagerGenerator;
use voyager_resolver_cairo_1_1_1::utils::run_starknet_compile;

#[derive(Args, Debug)]
pub struct VerifyProjectArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,
}

#[derive(Args, Debug)]
pub struct VerifyFileArgs {
    #[clap(help = "File path")]
    path: Utf8PathBuf,
}

pub fn verify_project(args: VerifyProjectArgs) -> Result<()> {
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

    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(VoyagerGenerator)).unwrap();

    let manifest_path = source_dir.join("Scarb.toml");

    let config = Config::builder(manifest_path)
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        std::process::exit(1);
    });
    ops::compile(&ws)
}

pub fn verify_file(args: VerifyFileArgs) -> Result<()> {
    let file_dir = match args.path.is_absolute() {
        true => args.path.clone(),
        false => {
            let mut current_path = current_dir().unwrap();
            current_path.push(args.path);
            Utf8PathBuf::from_path_buf(current_path).unwrap()
        }
    };

    //TODO detect_corelib will try to use the local corelib.
    // Once cairo is released, it will probably be able to use
    // the corelib from the release.
    run_starknet_compile(file_dir.as_str())
}

