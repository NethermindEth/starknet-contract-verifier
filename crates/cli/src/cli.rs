mod resolver;
mod utils;
mod api;
mod verify;

use crate::verify::VerifyFileArgs;
use crate::verify::VerifyProjectArgs;
use crate::utils::detect_local_tools;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

// TODO 1: There's a need of refactoring all this to reduce repetition.
// TODO 2: support single file verification
#[derive(Subcommand)]
enum Commands {
    #[command(about = "Builds the voyager-verify output")]
    VerifyProject(VerifyProjectArgs),
    // VerifyFile(VerifyFileArgs),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let (local_scarb_version, local_cairo_version) = detect_local_tools();

    match cli.command {
        Commands::VerifyProject(args) => verify::verify_project(args, local_cairo_version, local_scarb_version),
        // Commands::VerifyFile(args) => build::verify_file(args, local_cairo_version),
    }?;
    Ok(())
}
