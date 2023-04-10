use std::env;


use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;
use std::path::PathBuf;

use voyager_resolver::compiler::scarb_utils::get_contracts_to_verify;
use voyager_resolver::compiler::VoyagerGenerator;
use voyager_resolver::utils::run_scarb_build;


#[test]
fn test_get_contracts_to_verify() {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(VoyagerGenerator)).unwrap();
    let source_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_data")
        .join("simple_project");
    let manifest_path = source_dir.join("Scarb.toml");

    let config = Config::builder(manifest_path.to_str().unwrap())
        .ui_verbosity(Verbosity::Verbose)
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
    let contracts = get_contracts_to_verify(package).unwrap();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0], PathBuf::from("contracts/ERC20.cairo"))
}

#[test]
fn test_simple_project() {
    let source_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_data")
        .join("simple_project");
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(VoyagerGenerator)).unwrap();

    let manifest_path = source_dir.join("Scarb.toml");

    let config = Config::builder(manifest_path.to_str().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        std::process::exit(1);
    });
    ops::compile(&ws).unwrap();

    let reduced_project_path = source_dir.join("voyager-verify/local");
    println!(
        "Reduced project path: {}",
        reduced_project_path.to_str().unwrap()
    );
    run_scarb_build(reduced_project_path.to_str().unwrap()).unwrap();
}

#[test]
fn test_project_with_remap() {
    let source_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_data")
        .join("project_with_remap");
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(VoyagerGenerator)).unwrap();

    let manifest_path = source_dir.join("Scarb.toml");

    let config = Config::builder(manifest_path.to_str().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        std::process::exit(1);
    });
    ops::compile(&ws).unwrap();

    let reduced_project_path = source_dir.join("voyager-verify/local");
    println!(
        "Reduced project path: {}",
        reduced_project_path.to_str().unwrap()
    );
    run_scarb_build(reduced_project_path.to_str().unwrap()).unwrap();
}

#[test]
fn test_project_w_import_from_attachment() {
    let source_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_data")
        .join("project_w_import_from_attachment");
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(VoyagerGenerator)).unwrap();

    let manifest_path = source_dir.join("Scarb.toml");

    let config = Config::builder(manifest_path.to_str().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        std::process::exit(1);
    });
    ops::compile(&ws).unwrap();

    let reduced_project_path = source_dir.join("voyager-verify/local");
    println!(
        "Reduced project path: {}",
        reduced_project_path.to_str().unwrap()
    );
    run_scarb_build(reduced_project_path.to_str().unwrap()).unwrap();
}
