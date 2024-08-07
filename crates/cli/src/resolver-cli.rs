mod api;
mod resolver;
mod utils;

use crate::resolver::{resolve_scarb, TargetType};
use crate::utils::detect_local_tools;

use camino::Utf8PathBuf;
use console::{style, Emoji};
use dialoguer::{theme::ColorfulTheme, Input};
use dirs::home_dir;
use std::env;

fn main() -> anyhow::Result<()> {
    println!(
        "{} {} Getting project information...",
        style("[1/2]").bold().dim(),
        Emoji("📝", "")
    );

    // Project type + Path entry
    let target_type = TargetType::ScarbProject; // by default we assume the user is in a scarb project
    let is_current_dir_scarb = env::current_dir()?.join("scarb.toml").exists();
    let utf8_path = if is_current_dir_scarb {
        let current_path = env::current_dir()?.to_str().unwrap().trim().to_string();
        Utf8PathBuf::from(&current_path)
    } else {
        loop {
            // TODO, add TargetType::File path input here
            let input_path = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter Path to scarb project root:")
                .interact_text()
                .expect("Aborted at path input, terminating...")
                .trim()
                .to_string();
            let mut utf8_input_path: Utf8PathBuf = Utf8PathBuf::from(&input_path);
            // Resolve path
            if utf8_input_path.starts_with("~") {
                if let Some(home) = home_dir() {
                    let home_utf8 = Utf8PathBuf::from_path_buf(home).unwrap();
                    utf8_input_path = home_utf8.join(utf8_input_path.strip_prefix("~").unwrap());
                }
            }
            if utf8_input_path.exists() {
                break utf8_input_path;
            } else {
                println!("Path does not exist. Please try again.");
            }
        }
    };

    println!(
        "{} {} Resolving project...",
        style("[2/2]").bold().dim(),
        Emoji("🔗", "")
    );

    // Resolve project
    let (_project_files, _project_metadata) = match target_type {
        TargetType::File => {
            panic!("Single contract file verification is not yet implemented, please use a scarb project instead.");
        }
        TargetType::ScarbProject => {
            let (local_scarb_version, local_cairo_version) = detect_local_tools();
            resolve_scarb(utf8_path.clone(), local_cairo_version, local_scarb_version)?
        }
    };

    println!("{} Successfully resolved!", Emoji("✅", ""));
    Ok(())
}
