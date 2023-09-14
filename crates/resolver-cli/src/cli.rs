mod build;
mod resolver;
mod utils;

use crate::build::ResolveProjectArgs;
use crate::utils::detect_local_tools;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, FuzzySelect, Input, Select, Confirm};
use regex::Regex;
use dyn_compiler::dyn_compiler::SupportedCairoVersions;
use strum::IntoEnumIterator;

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
    println!("Which network ?");
    let items = vec!["Mainnet", "Goerli", "Goerli-2"];

    let network: Option<usize> = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(1)
        .interact_opt()
        .unwrap();

    match network {
        Some(0) => println!("Network: Mainnet"),
        Some(1) => println!("Network: Goerli-1"),
        Some(2) => println!("Network: Goerli-2"),
        Some(_) => {
            println!("Network not recognized");
            std::process::exit(1);
        }
        None => {
            println!("Aborted at network selection, terminating...");
            std::process::exit(1);
        }
    }

    let re = Regex::new(r"^0x[a-fA-F0-9]{64}$").unwrap();

    let class_hash: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Input class hash or address to verify : ")
        .validate_with(|input: &String| -> Result<(), &str> {
            if re.is_match(input) {
                Ok(())
            } else {
                Err("This is not a valid address")
            }
        })
        .interact()
        .unwrap();

    // let versions = vec!["1.1.0", "1.1.1", "2.0.0", "2.0.1", "2.0.2"];
    let versions_enum: Vec<SupportedCairoVersions> = SupportedCairoVersions::iter().collect();

    let compiler_version_index: Option<usize> = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Compiler version of class hash : ")
        .items(&versions_enum)
        .default(versions_enum.len() - 1)
        .interact_opt()
        .unwrap();

    match compiler_version_index {
        Some(_) => (),
        None => {
            println!("Aborted at compiler version selection, terminating...");
            std::process::exit(1);
        }
    }

    let licenses = vec!["GNU General Public License", "MIT License", "Apache License", "BSD licenses"];
    let license: Option<usize> = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select license you'd like to verify under :")
        .items(&licenses)
        .default(0)
        .interact_opt()
        .unwrap();

    match license {
        Some(_) => (),
        None => {
            println!("Aborted at license version selection, terminating... ");
        }
    }

    // Check if account contract
    let is_account_contract: bool = Confirm::new().with_prompt("Is this an Account Contract?").interact()?;

    // Path entry

    let path: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter Contracts Path :")
        .interact_text().unwrap();

    // let cli = Cli::parse();

    let utf8_path: Option<Utf8PathBuf> = Some(path).map(Utf8PathBuf::from);
    // let (local_scarb_version, local_cairo_version) = detect_local_tools();

    let local_cairo_version = match compiler_version_index {
        Some(index) => versions_enum[index],
        None => std::process::exit(1)
    };

    match utf8_path {
        args => build::resolve_project(args, local_cairo_version),
    }?;

    Ok(())
}
