mod api;
mod args;
mod class_hash;
mod license;
mod resolver;
mod utils;

use crate::{
    api::{ApiClient, ApiClientError, VerificationJob, poll_verification_status},
    args::{Args, Commands},
    resolver::TargetType,
    utils::detect_local_tools,
};
use args::SubmitArgs;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    let Args {
        command: cmd,
        network_url: network,
        network: _,
    } = Args::parse();

    let public = ApiClient::new(network.public)?;
    let private = ApiClient::new(network.private)?;

    match &cmd {
        Commands::Submit(args) => {
            let job_id = submit(public, private, args)?;
            println!("Contract submitted for verification, job id: {}", job_id);
        }
        Commands::Status { job } => {
            let status = check(public, job)?;
            println!("{status:?}")
        },
    }
    Ok(())
}

fn submit(
    public: ApiClient,
    private: ApiClient,
    args: &SubmitArgs,
) -> Result<String, ApiClientError> {
    // Project type
    let target_type = TargetType::ScarbProject; // by default we assume the user is in a scarb project

    // Resolve project
    let (project_files, project_metadata) = match target_type {
        TargetType::File => {
            panic!("Single contract file verification is not yet implemented, please use a scarb project instead.");
        }
        TargetType::ScarbProject => {
            let (local_scarb_version, local_cairo_version) = detect_local_tools();
            // TODO: do a first pass to find all the contracts in the project
            // For now we keep using the hardcoded value in the scarb.toml file

            resolver::resolve_scarb(
                args.path.clone().into(),
                local_cairo_version,
                local_scarb_version,
            )?
        }
    };

    // Check if the class exists on the network
    private.get_class(&args.hash).and_then(|does_exist| {
        if !does_exist {
            Err(ApiClientError::Other(anyhow::anyhow!(
                "This class hash does not exist for the given network. Please try again."
            )))
        } else {
            Ok(does_exist)
        }
    })?;

    public.verify_class(
        args.hash.clone(),
        // TODO: License
        "No License (None)", /* args.license} */
        args.name.as_ref(),
        project_metadata,
        project_files,
    )
}

fn check(public: ApiClient, job_id: &String) -> Result<VerificationJob, ApiClientError> {
    poll_verification_status(public, job_id)
}
