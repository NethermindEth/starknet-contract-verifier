# starknet-contract-verifier

Client for the [Voyager Starknet block explorer](https://voyager.online), that allows you to verify your starknet classes.

## Installation

### With asdf (Recommended)

First, install [asdf](https://asdf-vm.com/guide/getting-started.html) if you haven't already.

```bash
asdf plugin add voyager https://github.com/NethermindEth/asdf-voyager-verifier.git

asdf install voyager latest
```

### With Cargo

Alternatively, you can install directly from source:

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

**Using predefined networks:**
```bash
voyager verify --network mainnet \
    --class-hash <YOUR_CONTRACT_CLASS_HASH> \
    --contract-name <YOUR_CONTRACT_NAME> \
    --path <PATH_TO_YOUR_SCARB_PROJECT> \ # if you are running outside project root
    --license <SPDX_LICENSE_ID> # if not provided in Scarb.toml
    --lock-file \ # optional: include Scarb.lock file in verification
    --test-files # optional: include test files from src/ directory in verification
```

**Using custom API endpoint:**
```bash
voyager verify --url https://api.custom.com/beta \
    --class-hash <YOUR_CONTRACT_CLASS_HASH> \
    --contract-name <YOUR_CONTRACT_NAME> \
    --path <PATH_TO_YOUR_SCARB_PROJECT> \ # if you are running outside project root
    --license <SPDX_LICENSE_ID> # if not provided in Scarb.toml
    --lock-file \ # optional: include Scarb.lock file in verification
    --test-files # optional: include test files from src/ directory in verification
```

For workspace projects (multiple packages), you'll need to specify the package:

```bash
# With predefined network
voyager verify --network mainnet \
  --class-hash <YOUR_CONTRACT_CLASS_HASH> \
  --contract-name <YOUR_CONTRACT_NAME> \
  --package <PACKAGE_ID>

# With custom API endpoint
voyager verify --url https://api.custom.com/beta \
  --class-hash <YOUR_CONTRACT_CLASS_HASH> \
  --contract-name <YOUR_CONTRACT_NAME> \
  --package <PACKAGE_ID>
```

When successful you'll be given verification job id, which you can pass to:

```bash
# With predefined network
voyager status --network mainnet --job <JOB_ID>

# With custom API endpoint
voyager status --url https://api.custom.com/beta --job <JOB_ID>
```

to check the verification status. Afterwards visit [Voyager website](https://sepolia.voyager.online/) and search for your class hash to see the *verified* badge.

## Detailed information

### Verification

`voyager` provides two subcommands: `verify` and `status`. For both commands the user needs to specify either:

1. **Predefined network** via the `--network` argument:
   - `mainnet` - main starknet network (default API endpoint: <https://api.voyager.online/beta>)
   - `sepolia` - test network (default API endpoint: <https://sepolia-api.voyager.online/beta>)

2. **Custom API endpoint** via the `--url` argument:
   - `--url <URL>` - specify custom API endpoint URL (e.g., `https://api.custom.com/beta`)

Either `--network` or `--url` must be provided, but not both.

#### Verification process

In order to verify a contract, you need to provide several arguments:

- `--class-hash`, class hash of the declared contract
- `--contract-name`, name of the contract to verify
- `--path`, path to directory containing scarb project (If omitted it will use current working directory)
- `--dry-run`, perform dry run to preview what files would be collected and submitted without actually sending them for verification
- `--license`, SPDX license identifier (optional, will use license from Scarb.toml if defined there, otherwise defaults to "All Rights Reserved")
  - The license should be a valid [SPDX license identifier](https://spdx.org/licenses/) such as MIT, Apache-2.0, etc.
- `--lock-file`, include Scarb.lock file in verification submission (optional, defaults to false)
  - When enabled, the tool will include the Scarb.lock file (if it exists) in the files sent to the remote API for verification
  - This can be useful for ensuring reproducible builds by locking dependency versions
- `--test-files`, include test files from src/ directory in verification submission (optional, defaults to false)
  - When enabled, the tool will include test files (files with "test" or "tests" in their path) that are located within the src/ directory
  - This can be useful when your contract depends on test utilities or helper functions for verification
  - Only affects files within the src/ directory; test files in dedicated test directories are still excluded
- `--watch`, wait indefinitely for verification result (optional)
- `--package`, specify which package to verify (required for workspace projects with multiple packages)

There are more options, each of them is documented in the `--help` output.

If the verification submission is successful, client will output the verification job id.

#### Checking job status

User can query the verification job status using `status` command and providing job id as the `--job` argument value. The status check will poll the server with exponential backoff until the verification is complete or fails.
