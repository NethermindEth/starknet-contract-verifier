# starknet-contract-verifier

Client for the [Voyager Starknet block explorer](https://voyager.online), that allows you to verify your starknet classes.

## Installation

```bash
cargo install starknet-contract-verifier
```

## Quickstart guide

### Scarb

Contract verifier works with [Scarb](https://docs.swmansion.com/scarb) based projects. The tool assumes that `scarb` command is available in the environment and project is building properly by executing `scarb build`.

#### Supported versions

Client is version agnostic, the Scarb/Cairo versions support is determined by the server availability. As of writing this (2025) Cairo up to 2.11.4 is supported with newer versions being added with a slight lag after release.

### Project configuration

**⚠️ Important**: Every compiler configuration used for deployment must be placed under `[profile.release]` since the remote compiler will run `scarb --release build`. This includes any custom compiler settings, optimizations, or dependencies that are required for your contract to build correctly in the verification environment.

**Note**: At the moment, Sepolia-only verification is not available. However, classes verified on mainnet will appear verified on Sepolia as well.

For license information, you can specify it in your Scarb.toml:

```toml
[package]
name = "my_project"
version = "0.1.0"
license = "MIT"  # Optional: Define license here using a valid SPDX identifier

[dependencies]
starknet = ">=2.11.2"

[[target.starknet-contract]]
sierra = true

[profile.release.cairo]
# Add any compiler configurations needed for deployment here
# For example:
# sierra-replace-ids = false
# inlining-strategy = "avoid"
```

Alternatively, you can provide the license via the `--license` CLI argument when verifying your contract.

**Important**: For workspace projects with multiple packages, you must use the `--package` argument to specify which package to verify.

### Verify your contract

Once you have the verifier installed, execute:

```bash
starknet-contract-verifier --network mainnet verify \
    --class-hash <YOUR_CONTRACT_CLASS_HASH> \
    --contract-name <YOUR_CONTRACT_NAME> \
    --path <PATH_TO_YOUR_SCARB_PROJECT> \ # if you are running outside project root
    --license <SPDX_LICENSE_ID> # if not provided in Scarb.toml
    --execute
```

For workspace projects (multiple packages), you'll need to specify the package:

```bash
starknet-contract-verifier --network mainnet verify \
  --class-hash <YOUR_CONTRACT_CLASS_HASH> \
  --contract-name <YOUR_CONTRACT_NAME> \
  --package <PACKAGE_ID> \
  --execute
```

When successful you'll be given verification job id, which you can pass to:

```bash
starknet-contract-verifier --network mainnet status --job <JOB_ID>
```

to check the verification status. Afterwards visit [Voyager website](https://sepolia.voyager.online/) and search for your class hash to see the *verified* badge.

## Detailed information

### Verification

`starknet-contract-verifier` provides two subcommands: `verify` and `status`. For both cases user needs to select the network with which they want to interact via the `--network` argument. Possible cases are:

- `mainnet`, main starknet network (default API endpoints: <https://api.voyager.online/beta> and <https://voyager.online>)
- `sepolia`, test network (default API endpoints: <https://sepolia-api.voyager.online/beta> and <https://sepolia.voyager.online>)
- `custom`, set custom addresses via `--public` and `--private` arguments

#### Verification process

In order to verify a contract, you need to provide several arguments:

- `--class-hash`, class hash of the declared contract
- `--contract-name`, name of the contract to verify
- `--path`, path to directory containing scarb project (If omitted it will use current working directory)
- `--execute`, flag to actually execute the verification (without this flag, it will only show what would be done)
- `--license`, SPDX license identifier (optional, will use license from Scarb.toml if defined there, otherwise defaults to "All Rights Reserved")
  - The license should be a valid [SPDX license identifier](https://spdx.org/licenses/) such as MIT, Apache-2.0, etc.
- `--watch`, wait indefinitely for verification result (optional)
- `--package`, specify which package to verify (required for workspace projects with multiple packages)

There are more options, each of them is documented in the `--help` output.

If the verification submission is successful, client will output the verification job id.

#### Checking job status

User can query the verification job status using `status` command and providing job id as the `--job` argument value. The status check will poll the server with exponential backoff until the verification is complete or fails.
