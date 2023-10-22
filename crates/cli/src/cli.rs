mod api;
mod resolver;
mod utils;
mod verify;

use crate::api::LicenseType;
use crate::utils::detect_local_tools;
use crate::verify::VerifyFileArgs;
use crate::verify::VerifyProjectArgs;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Select};
use regex::Regex;
use strum::IntoEnumIterator;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

// TODO 1: There's a need of refactoring all this to reduce repetition.
// TODO 2: support other types of project/file configurations
//          - single file
//          - non scarb
//          - multiple contracts in project
#[derive(Subcommand)]
enum Commands {
    #[command(about = "Builds the voyager-verify output")]
    VerifyProject(VerifyProjectArgs),
    // VerifyFile(VerifyFileArgs),
}

fn main() -> anyhow::Result<()> {
    let network_items = vec!["Mainnet", "Goerli"];

    let network_index: Option<usize> = Select::with_theme(&ColorfulTheme::default())
        .items(&network_items)
        .with_prompt("Which network would you like to verify on : ")
        .default(0)
        .interact_opt()?;

    match network_index {
        Some(_) => (),
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
        .interact()?;

    let licenses: Vec<LicenseType> = LicenseType::iter().collect();
    let license_index: Option<usize> = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select license you'd like to verify under :")
        .items(&licenses)
        .default(0)
        .interact_opt()?;

    match license_index {
        Some(_) => (),
        None => {
            println!("Aborted at license version selection, terminating... ");
        }
    }

    // Check if account contract
    let is_account_contract: bool = Confirm::new()
        .with_prompt("Is this an Account Contract?")
        .interact()?;

    // Path entry
    let path: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter Contracts Path :")
        .interact_text()
        .unwrap();

    // let utf8_path: Option<Utf8PathBuf> = Some(path).map(Utf8PathBuf::from);
    let utf8_path: Utf8PathBuf = Utf8PathBuf::from(path);

    let (local_scarb_version, local_cairo_version) = detect_local_tools();

    // Parse args into VerifyProjectArgs
    let verify_args = VerifyProjectArgs {
        network: network_items[network_index.unwrap()].to_string(),
        hash: class_hash,
        license: licenses[license_index.unwrap()],
        name: "test".to_string(),
        path: utf8_path,
        is_account_contract: Some(is_account_contract),
        max_retries: Some(10),
    };

    match verify_args {
        args => verify::verify_project(args, local_cairo_version, local_scarb_version),
        // Commands::VerifyFile(args) => build::verify_file(args, local_cairo_version),
    }?;
    Ok(())
}
