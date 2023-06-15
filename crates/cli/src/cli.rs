mod build;

use crate::build::VerifyFileArgs;
use build::VerifyProjectArgs;
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
    VerifyProject(VerifyProjectArgs),
    VerifyFile(VerifyFileArgs),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::VerifyProject(args) => build::verify_project(args),
        Commands::VerifyFile(args) => build::verify_file(args),
    }?;
    Ok(())
}
