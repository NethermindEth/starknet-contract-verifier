use std::env;

use anyhow::Result;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb_ui::Verbosity;
use std::path::PathBuf;

use voyager_resolver_cairo_2_2_0::compiler::scarb_utils::get_contracts_to_verify;
use voyager_resolver_cairo_2_2_0::compiler::VoyagerGenerator;
use voyager_resolver_cairo_2_2_0::utils::run_scarb_build;

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
fn test_simple_project() -> Result<()> {
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
    let resolve = ops::resolve_workspace(&ws)?;
    let package_ids = resolve.packages.keys().cloned().collect();
    ops::compile(package_ids, &ws).unwrap();

    let reduced_project_path = source_dir.join("voyager-verify/local");
    println!(
        "Reduced project path: {}",
        reduced_project_path.to_str().unwrap()
    );
    run_scarb_build(reduced_project_path.to_str().unwrap()).unwrap();
    Ok(())
}

#[test]
fn test_project_with_remap() -> Result<()> {
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
    let resolve = ops::resolve_workspace(&ws)?;
    let package_ids = resolve.packages.keys().cloned().collect();
    ops::compile(package_ids, &ws).unwrap();

    let reduced_project_path = source_dir.join("voyager-verify/project_with_remap");
    println!(
        "Reduced project path: {}",
        reduced_project_path.to_str().unwrap()
    );
    run_scarb_build(reduced_project_path.to_str().unwrap()).unwrap();
    Ok(())
}

#[test]
fn test_project_w_import_from_attachment() -> Result<()> {
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
    let resolve = ops::resolve_workspace(&ws)?;
    let package_ids = resolve.packages.keys().cloned().collect();
    ops::compile(package_ids, &ws).unwrap();

    let reduced_project_path = source_dir.join("voyager-verify/local");
    println!(
        "Reduced project path: {}",
        reduced_project_path.to_str().unwrap()
    );
    run_scarb_build(reduced_project_path.to_str().unwrap()).unwrap();
    Ok(())
}
