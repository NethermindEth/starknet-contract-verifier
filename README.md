# starknet-contract-verifier

Client for the [Voyager Starknet block explorer](https://voyager.online), that allows you to verify your starknet classes.

## ✨ Key Features

- 🎯 **Enhanced User Experience**: Helpful error messages with actionable suggestions
- ⏳ **Real-time Progress**: Visual progress indicators and status updates
- 🔍 **Auto-watch Mode**: Automatic monitoring until verification completion
- 📚 **Job History**: Local tracking of all verification attempts
- 🎨 **Rich Output**: Colored status display and better formatting
- 🔗 **Direct Links**: Easy access to verified contracts on Voyager

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
    --lock-file \ # optional: include Scarb.lock file in verification
    --watch \ # optional: auto-watch until completion
    --execute
```

The tool now provides **real-time progress indicators** and **enhanced error messages** with helpful suggestions to guide you through common issues.

For workspace projects (multiple packages), you'll need to specify the package:

```bash
starknet-contract-verifier --network mainnet verify \
  --class-hash <YOUR_CONTRACT_CLASS_HASH> \
  --contract-name <YOUR_CONTRACT_NAME> \
  --package <PACKAGE_ID> \
  --lock-file \ # optional: include Scarb.lock file in verification
  --execute
```

When successful, you'll be given a verification job id. The tool automatically saves all verification jobs to your **local history** for easy tracking.

**Option 1: Auto-watch (Recommended)**
Use the `--watch` flag to automatically monitor progress:
```bash
starknet-contract-verifier --network mainnet verify \
    --class-hash <YOUR_CONTRACT_CLASS_HASH> \
    --contract-name <YOUR_CONTRACT_NAME> \
    --watch \
    --execute
```

**Option 2: Manual status checking**
```bash
starknet-contract-verifier --network mainnet status --job <JOB_ID>
```

**Option 3: Status with auto-watch**
```bash
starknet-contract-verifier --network mainnet status --job <JOB_ID> --watch
```

**View your verification history:**
```bash
starknet-contract-verifier --network mainnet list
```

Afterwards visit [Voyager website](https://voyager.online/) and search for your class hash to see the *verified* badge.

## Detailed information

### Verification

`starknet-contract-verifier` provides three main subcommands: `verify`, `status`, and `list`. For all cases user needs to select the network with which they want to interact via the `--network` argument. Possible cases are:

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
- `--lock-file`, include Scarb.lock file in verification submission (optional, defaults to false)
  - When enabled, the tool will include the Scarb.lock file (if it exists) in the files sent to the remote API for verification
  - This can be useful for ensuring reproducible builds by locking dependency versions
- `--watch`, automatically monitor verification progress until completion (optional)
  - Eliminates the need to manually check status repeatedly
  - Provides real-time updates on verification progress
- `--package`, specify which package to verify (required for workspace projects with multiple packages)

There are more options, each of them is documented in the `--help` output.

#### Visual Feedback

The tool provides comprehensive visual feedback:
- 🔄 **Loading spinners** during project analysis
- 📁 **File processing progress bars** showing which files are being processed
- 🚀 **Upload progress indicators** during API submission
- ⏳ **Status monitoring** with real-time updates
- ✅ **Success celebrations** with clear next steps

If the verification submission is successful, the client will output the verification job id and automatically save it to your local history.

#### Checking job status

There are several ways to check verification status:

1. **Auto-watch during verification**: Use `--watch` flag with the `verify` command
2. **Manual status check**: Use `status` command with the job ID
3. **Auto-watch status**: Use `status` command with `--watch` flag for continuous monitoring
4. **View history**: Use `list` command to see recent verification jobs

**Enhanced Status Features:**
- 🎨 **Colored output** for better readability
- ⏳ **Real-time progress indicators** with spinners and progress bars
- 📋 **Job history tracking** - all verifications are automatically saved
- 🔗 **Direct Voyager links** for easy contract browsing
- 💡 **Helpful error messages** with actionable suggestions

#### Job History Management

The tool automatically maintains a history of all verification jobs in `~/.starknet-verifier/history.json`. You can:

```bash
# List recent verification jobs (default: 10)
starknet-contract-verifier --network mainnet list

# List more jobs
starknet-contract-verifier --network mainnet list --limit 20
```

Each history entry includes:
- Job ID and current status
- Contract name and class hash
- Network and project path
- License information
- Submission timestamp
- Direct Voyager link

#### Error Handling & Troubleshooting

The tool now provides **enhanced error messages** with:
- 🎯 **Context-aware suggestions** for common issues
- 💡 **"Did you mean..."** suggestions for typos
- 📁 **Better workspace project guidance**
- 🔧 **Actionable solutions** for most error scenarios

Common issues and solutions:
- **Missing package error**: The tool will list available packages and suggest the correct `--package` flag
- **Contract not found**: Suggestions based on similar contract names
- **License issues**: Clear guidance on valid SPDX identifiers
- **API errors**: Status-specific troubleshooting advice
