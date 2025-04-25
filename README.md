# starknet-contract-verifier

Client for the [Voyager Starknet block explorer](https://voyager.online), that allows you to verify your starknet classes.

## Quick start guide

### Scarb

Contract verifier works with [Scarb](https://docs.swmansion.com/scarb) based projects. The tool assumes that `scarb` command is available in the environment and project is building properly by executing `scarb build`.

#### Supported versions

Client is version agnostic, the Scarb/Cairo versions support is determined by the server availability. As of writing this (09/01/2025) Cairo up to 2.9.1 is supported with newer versions being added few a slight lag after release.

### Get `starknet-contract-verifier`

Right now in order to obtain the `starknet-contract-verifier`, clone this repository:

``` bash
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

``` bash
cargo run -- --network mainnet submit \
  --contract <YOUR_CONTRACT_NAME>
  --name <NAME_TO_BE_PUBLISHED_AT> \
  --hash <YOUR_CONTRACT_CLASS_HASH> \
  --path <PATH_TO_YOUR_SCARB_PROJECT>
```

When successful you'll be given verification job id, which you can pass to:

``` bash
cargo run -- --network mainnet status --job <JOB_ID>
```

to check the verification status. Afterwards visit [Voyager website]() and search for your class hash to see *verified* badge.

## Detailed information

### Verification

`starknet-contract-verifier` provides two subcommands: `submit` and `status`. For both cases user needs to select the network with which they want to interact via the `--network` argument. Possible cases are:
- `mainnet`, main starknet network,
- `sepolia`, test network,
- `custom`, set provide custom addresses via `--public` and `--private arguments.

#### Submitting for verification

In order to submit contract for verification user needs to provide several arguments:
- `--path`, path to directory containing scarb project (If omitted it will use current working directory),
- `--contract`, name of the contract as in the source code,
- `--package`, if your project is using workspaces, you need to provide which package contains desired contract,
- `--name`, name which will be used in the block explorer for the verified contract, 
- `--hash`, class hash of the declared contract.

there are more options, each of them is documented in the `--help` output.

If the submission is successful, client will output the verification job id.

#### Checking job status

User can query the verification job status using `status` command and providing job id as the `--job` argument value.
