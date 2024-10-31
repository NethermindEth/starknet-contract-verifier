mod api;
mod args;
mod class_hash;
mod license;
mod resolver;
mod utils;
mod verify;

use crate::api::{does_class_exist, Network};
use crate::args::Args;
use crate::license::LicenseType;
use crate::resolver::TargetType;
use crate::utils::detect_local_tools;
use clap::Parser;
use console::{style, Emoji};
use dialoguer::{theme::ColorfulTheme, Select};
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Project type
    let target_type = TargetType::ScarbProject; // by default we assume the user is in a scarb project

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

            resolver::resolve_scarb(args.path.clone().into(), local_cairo_version, local_scarb_version)?
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

    // TODO: Unify those
    let network_enum = match args.network {
        args::Network::Mainnet => Network::Mainnet,
        args::Network::Testnet => Network::Sepolia,
        args::Network::Custom { public: _, private: _ } => Network::Custom
    };

    let class_hash: String = args.hash.to_string();
    loop {
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

    let verification_result = match target_type {
        TargetType::ScarbProject => {
            verify::verify_project(
                args,
                project_metadata,
                project_files,
                "".to_string(),
                None
            )
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
