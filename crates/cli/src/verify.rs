use anyhow::Result;

use dyn_compiler::dyn_compiler::SupportedCairoVersions;

use crate::{
    api::{
        dispatch_class_verification_job, poll_verification_status, FileInfo, Network,
        ProjectMetadataInfo,
    },
    args,
    args::Args,
    resolver::get_dynamic_compiler,
};

pub fn verify_project(
    args: Args,
    metadata: ProjectMetadataInfo,
    files: Vec<FileInfo>,
    api_key: String,
    max_retries: Option<u32>,
) -> Result<()> {
    let network_enum = match args.network {
        args::Network::Mainnet => Network::Mainnet,
        args::Network::Testnet => Network::Sepolia,
        args::Network::Custom { public: _, private: _ } => Network::Custom
    };

    let dispatch_response = dispatch_class_verification_job(
        api_key.as_str(),
        network_enum.clone(),
        args.hash.as_ref(),
        "No License (None)",
        &args.name,
        metadata,
        files,
    );

    let job_id = match dispatch_response {
        Ok(response) => response,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to dispatch verification job: {}",
                e
            ));
        }
    };

    // Retry for 5 minutes
    let poll_result = poll_verification_status(
        api_key.as_str(),
        network_enum,
        &job_id,
        max_retries.unwrap_or(180),
    );

    match poll_result {
        Ok(_response) => Ok(()),
        Err(e) => Err(anyhow::anyhow!(
            "Error while polling verification status: {}",
            e
        )),
    }
}

pub fn _verify_file(args: Args, cairo_version: SupportedCairoVersions) -> Result<()> {
    let absdir = args.path.make_absolute()?;

    let compiler = get_dynamic_compiler(cairo_version);
    compiler.compile_file(absdir.as_ref())
}
