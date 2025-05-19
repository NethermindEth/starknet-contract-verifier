# starknet-contract-verifier

Client for the [Voyager Starknet block explorer](https://voyager.online), that allows you to verify your starknet classes.

## Quickstart guide

### Scarb

Contract verifier works with [Scarb](https://docs.swmansion.com/scarb) based projects. The tool assumes that `scarb` command is available in the environment and project is building properly by executing `scarb build`.

#### Supported versions

Client is version agnostic, the Scarb/Cairo versions support is determined by the server availability. As of writing this (2024) Cairo up to 2.11.4 is supported with newer versions being added with a slight lag after release.

### Project configuration

In order to verify your contract, you'll need to add a `tool.voyager` table in your `Scarb.toml`, for example:

```toml
[package]
name = "my_project"
version = "0.1.0"
license = "MIT"  # Optional: Define license here using a valid SPDX identifier

[dependencies]
starknet = ">=2.11.2"

[[target.starknet-contract]]
sierra = true

# Add the following section
[tool.voyager]
my_contract = { path = "src/main.cairo" }
```

The `my_contract` field name has to match the name of the contract that you want to verify. The path should point to the file containing the Cairo module that you wish to verify. In the example above, the Cairo contract in question is located at `src/main.cairo`.

*Note* that only one contract should be provided in this section as multi-contract verification is not supported yet.

### Get `starknet-contract-verifier`

Right now in order to obtain the `starknet-contract-verifier`, clone this repository:

```bash
git clone https://github.com/NethermindEth/starknet-contract-verifier.git
cd starknet-contract-verifier
```

### Setup rust

`starknet-contract-verifier` is a rust/cargo project. In order to build it you'll need rust and cargo set up. You can do it easily using [rustup](https://rustup.rs/).

```bash
curl https://sh.rustup.rs -sSf | sh -s
```

### Submit your contract

You are good to go, execute:

```bash
cargo run -- --network sepolia submit \
  --hash <YOUR_CONTRACT_CLASS_HASH> \
  --path <PATH_TO_YOUR_SCARB_PROJECT> \
  --license <SPDX_LICENSE_ID> # if not provided in Scarb.toml
  --execute \
```

When successful you'll be given verification job id, which you can pass to:

```bash
cargo run -- --network sepolia status --job <JOB_ID>
```

to check the verification status. Afterwards visit [Voyager website](https://sepolia.voyager.online/) and search for your class hash to see the *verified* badge.

## Detailed information

### Verification

`starknet-contract-verifier` provides two subcommands: `submit` and `status`. For both cases user needs to select the network with which they want to interact via the `--network` argument. Possible cases are:

- `mainnet`, main starknet network (default API endpoints: https://api.voyager.online/beta and https://voyager.online)
- `sepolia`, test network (default API endpoints: https://sepolia-api.voyager.online/beta and https://sepolia.voyager.online)
- `custom`, set custom addresses via `--public` and `--private` arguments

#### Submitting for verification

In order to submit contract for verification user needs to provide several arguments:

- `--path`, path to directory containing scarb project (If omitted it will use current working directory)
- `--hash`, class hash of the declared contract
- `--execute`, flag to actually execute the verification (without this flag, it will only show what would be done)
- `--license`, SPDX license identifier (optional, will use license from Scarb.toml if defined there, otherwise defaults to "All Rights Reserved")
  - The license should be a valid [SPDX license identifier](https://spdx.org/licenses/) such as MIT, Apache-2.0, etc.
- `--watch`, wait indefinitely for verification result (optional)

There are more options, each of them is documented in the `--help` output.

If the submission is successful, client will output the verification job id.

#### Checking job status

User can query the verification job status using `status` command and providing job id as the `--job` argument value. The status check will poll the server with exponential backoff until the verification is complete or fails.
