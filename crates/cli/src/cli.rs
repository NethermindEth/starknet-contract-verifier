mod api;
mod license;
mod resolver;
mod utils;
mod validation;
mod verify;

use crate::api::{does_class_exist, Network};
use crate::license::LicenseType;
use crate::resolver::TargetType;
use crate::utils::detect_local_tools;
use camino::Utf8PathBuf;
use console::{style, Emoji};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use dirs::home_dir;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use std::{
    env,
    str::FromStr,
    time::{Duration, Instant},
};
use strum::IntoEnumIterator;
use validation::is_class_hash_valid;
use verify::VerifyProjectArgs;

fn main() -> anyhow::Result<()> {
    // TODO: make this cli use a secure api
    // let api_key = match env::var("API_KEY") {
    //     Ok(api_key) => Some(api_key),
    //     Err(_) => None,
    // };

    // let api_key = match api_key {
    //     Some(key_values) => key_values,
    //     None => {
    //         println!("API_KEY not detected in environment variables. You can get one at https://forms.gle/34RE6d4aiiv16HoW6");
    //         return Ok(());
    //     }
    // };
    println!(
        "{} {} Getting project information...",
        style("[1/4]").bold().dim(),
        Emoji("📝", "")
    );

    // Project type + Path entry
    let target_type = TargetType::ScarbProject; // by default we assume the user is in a scarb project
    let is_current_dir_scarb = env::current_dir()?.join("scarb.toml").exists();
    let path = if is_current_dir_scarb {
        env::current_dir()?.to_str().unwrap().trim().to_string()
    } else {
        loop {
            // TODO, add TargetType::File path input here
            let input_path = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter Path to scarb project root:")
                .interact_text()
                .expect("Aborted at path input, terminating...")
                .trim()
                .to_string();
            let utf8_path: Utf8PathBuf = Utf8PathBuf::from(&input_path);
            if utf8_path.exists() {
                break input_path;
            } else {
                println!("Path does not exist. Please try again.");
            }
        }
    };
    // Resolve path
    let mut utf8_path = Utf8PathBuf::from(&path);
    if utf8_path.starts_with("~") {
        if let Some(home) = home_dir() {
            let home_utf8 = Utf8PathBuf::from_path_buf(home).unwrap();
            utf8_path = home_utf8.join(utf8_path.strip_prefix("~").unwrap());
        }
    }

    // Start the whole process
    let _spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈");

    println!(
        "{} {} Resolving project...",
        style("[2/4]").bold().dim(),
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
    println!(
        "{} {} Getting verification information...",
        style("[3/4]").bold().dim(),
        Emoji("🔍  ", "")
    );

    // -- Network selection --

    // Custom network selection
    let custom_internal_api_endpoint_url = env::var("CUSTOM_INTERNAL_API_ENDPOINT_URL");
    let custom_public_api_endpoint_url = env::var("CUSTOM_PUBLIC_API_ENDPOINT_URL");
    let is_custom_network =
        custom_internal_api_endpoint_url.is_ok() && custom_public_api_endpoint_url.is_ok();

    // Only show local if debug network option is up.
    let is_debug_network = env::var("DEBUG_NETWORK").is_ok();
    let network_items = if is_debug_network {
        vec!["Mainnet", "Sepolia", "Integration", "Local"]
    } else {
        vec!["Mainnet", "Sepolia"]
    };

    // defaults to the first item.
    let selected_network = if !is_custom_network {
        let network_index = Select::with_theme(&ColorfulTheme::default())
            .items(&network_items)
            .with_prompt("Which network would you like to verify on : ")
            .default(0)
            .interact_opt()
            .expect("Aborted at network selection, terminating...")
            .expect("Aborted at network selection, terminating...");

        network_items[network_index]
    } else {
        println!(
            "🔔 {}",
            style("Custom verification endpoint provided:").bold()
        );
        println!(
            "Internal endpoint url: {}",
            custom_internal_api_endpoint_url.unwrap_or("".to_string())
        );
        println!(
            "Public endpoint url: {}",
            custom_public_api_endpoint_url.unwrap_or("".to_string())
        );

        "custom"
    };

    let network_enum = Network::from_str(selected_network)?;
    let mut class_hash: String;
    loop {
        class_hash = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Input class hash to verify : ")
            .validate_with(|input: &String| -> Result<(), &str> {
                if is_class_hash_valid(input) {
                    Ok(())
                } else {
                    Err("This is not a class hash.")
                }
            })
            .interact()?;

        // Check if the class exists on the network
        match does_class_exist(network_enum.clone(), &class_hash) {
            Ok(true) => break,
            Ok(false) => {
                println!("This class hash does not exist for the given network. Please try again.")
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error while checking if class exists: {}",
                    e
                ))
            }
        }
    }

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
        style("[4/4]").bold().dim(),
        Emoji("🔍", "")
    );

    // Create and configure a progress bar
    let pb_verification = ProgressBar::new_spinner();
    pb_verification.set_style(_spinner_style);
    pb_verification.enable_steady_tick(Duration::from_millis(100));
    pb_verification.set_message("Please wait...");

    // Parse args into VerifyProjectArgs
    let verify_args = VerifyProjectArgs {
        network: selected_network.to_string(),
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

    // Stop and clear the progress bar
    pb_verification.finish_with_message("Done");

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
            "Verification failed! {} {}",
            Emoji("❌", ""),
            e
        )),
    }
}
