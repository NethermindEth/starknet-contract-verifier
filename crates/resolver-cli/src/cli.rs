mod build;
mod resolver;
mod utils;

use crate::build::ResolveProjectArgs;
use crate::utils::detect_local_tools;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Builds the voyager-verify output")]
    ResolveProject(ResolveProjectArgs),
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::ResolveProject(args) => {
            if let Some(path) = &args.path {
                let (_local_scarb_version, local_cairo_version) = detect_local_tools(path);
                build::resolve_project(args, local_cairo_version)?;
            } else {
                println!("❌ path not provided");
                std::process::exit(1);
            }
        }
    };

    Ok(())
}
