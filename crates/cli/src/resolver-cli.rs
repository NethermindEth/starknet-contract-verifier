mod api;
mod args;
mod class_hash;
mod resolver;
mod utils;

use crate::{
    args::ProjectDir,
    resolver::resolve_scarb,
    utils::detect_local_tools,
};

use clap::Parser;
use console::Emoji;

#[derive(Parser)]
pub struct Args {
    /// Path to Scarb project root DIR
    #[arg(
        long,
        value_name = "DIR",
        value_hint = clap::ValueHint::DirPath,
        value_parser = args::project_dir_value_parser,
        default_value_t = ProjectDir::cwd().unwrap(),
    )]
    pub path: ProjectDir,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Resolve project
    let (local_scarb_version, local_cairo_version) = detect_local_tools();
    let (_project_files, _project_metadata) = resolve_scarb(args.path, local_cairo_version, local_scarb_version)?;

    println!("{} Successfully resolved!", Emoji("✅", ""));
    Ok(())
}
