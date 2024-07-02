mod api;
mod license;
mod resolver;
mod utils;
mod verify;

use crate::license::LicenseType;
use crate::utils::detect_local_tools;
use camino::Utf8PathBuf;
use console::{style, Emoji};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use indicatif::{HumanDuration, ProgressStyle};
use regex::Regex;
use std::{env, time::Instant};
use strum::IntoEnumIterator;
use verify::VerifyProjectArgs;

#[allow(dead_code)]
enum TargetType {
    ScarbProject,
    File,
}

fn main() -> anyhow::Result<()> {
    println!(
        "{} {} Getting project information...",
        style("[1/3]").bold().dim(),
        Emoji("📝", "")
    );

    // Network selection
    let is_debug_network = env::var("DEBUG_NETWORK").is_ok();
    let network_items = if is_debug_network {
        vec!["Mainnet", "Sepolia", "Integration", "Local"]
    } else {
        vec!["Mainnet", "Sepolia"]
    };
    let network_index = Select::with_theme(&ColorfulTheme::default())
        .items(&network_items)
        .with_prompt("Which network would you like to verify on : ")
        .default(0)
        .interact_opt()
        .expect("Aborted at network selection, terminating...")
        .expect("Aborted at network selection, terminating...");

    // Project type + Path entry
    let target_type = TargetType::ScarbProject; // by default we assume the user is in a scarb project
    let is_current_dir_scarb = env::current_dir()?.join("scarb.toml").exists();
    let path = if is_current_dir_scarb {
        env::current_dir()?.to_str().unwrap().trim().to_string()
    } else {
        // TODO, add TargetType::File path input here
        Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter Path to scarb project root:")
            .interact_text()
            .expect("Aborted at path input, terminating...")
            .trim()
            .to_string()
    };
    let utf8_path: Utf8PathBuf = Utf8PathBuf::from(path);
    if !utf8_path.exists() {
        panic!("Path does not exist");
    }

    // Start the whole process
    let _spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈");

    println!(
        "{} {} Resolving project...",
        style("[2/3]").bold().dim(),
        Emoji("🔗", "")
    );

    // Resolve project
    let (project_files, project_metadata) = match target_type {
        TargetType::File => {
            panic!("Single contract file verification is not yet implemented, please use a scarb project instead.");
        }
        TargetType::ScarbProject => {
            let (local_scarb_version, local_cairo_version) = detect_local_tools();
            // TODO: do a first pass to find all the contracts in the project
            // For now we keep using the hardcoded value in the scarb.toml file

            resolver::resolve_scarb(utf8_path.clone(), local_cairo_version, local_scarb_version)?
        }
    };

    // TODO: try to calculate the class hash automatically later after contract selection?
    // println!(
    //     "{} {} Calculating class hash...",
    //     style("[x/x]").bold().dim(),
    //     Emoji("🔍  ", "")
    // );
    let re = Regex::new(r"^0x[a-fA-F0-9]{64}$").unwrap();
    let class_hash: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Input class hash to verify : ")
        .validate_with(|input: &String| -> Result<(), &str> {
            if re.is_match(input) {
                Ok(())
            } else {
                Err("This is not a class hash")
            }
        })
        .interact()?;

    // Get name that you want to use for the contract
    let class_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter your desired class name: ")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.len() > 50 {
                Err("Given name is too long")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .expect("Aborted at class name input, terminating...")
        .trim()
        .to_string();

    // Check if account contract
    // TODO: Is there a way to detect this automatically?
    // println!(
    //     "{} {} Checking if account contract...",
    //     style("[x/x]").bold().dim(),
    //     Emoji("📃  ", "")
    // );
    let is_account_contract: bool = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Is this an Account Class?")
        .interact()?;

    // Set license for your contract code
    let licenses: Vec<LicenseType> = LicenseType::iter().collect();
    let license_index = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select license you'd like to verify under :")
        .items(&licenses)
        .default(0)
        .interact_opt()
        .expect("Aborted at license version selection, terminating...")
        .expect("Aborted at license version selection, terminating...");

    let verification_start = Instant::now();
    println!(
        "{} {} Verifying project...",
        style("[3/3]").bold().dim(),
        Emoji("🔍", "")
    );

    // Parse args into VerifyProjectArgs
    let verify_args = VerifyProjectArgs {
        network: network_items[network_index].to_string(),
        hash: class_hash,
        license: licenses[license_index],
        name: class_name,
        is_account_contract: Some(is_account_contract),
        max_retries: Some(10),
        api_key: "".to_string(),
        path: utf8_path,
    };

    let verification_result = match target_type {
        TargetType::ScarbProject => {
            verify::verify_project(verify_args, project_metadata, project_files)
        }
        TargetType::File => panic!("Single contract file verification is not yet implemented"),
    };

    match verification_result {
        Ok(_) => {
            println!(
                "{} Successfully verified in {}",
                Emoji("✅", ""),
                HumanDuration(verification_start.elapsed())
            );
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!(
            "verification failed {} {}",
            Emoji("❌", ""),
            e
        )),
    }
}
