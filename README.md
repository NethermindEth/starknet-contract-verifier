# starknet-contract-verifier

Client for the [Voyager Starknet block explorer](https://voyager.online), that allows you to verify your starknet classes.

## Getting started

### Prerequisites

#### Scarb

Contract verifier works with [Scarb](https://docs.swmansion.com/scarb) based projects. The tool assumes that `scarb` command is available in the envirenment and project is building properly by executing `scarb bulid`.

#### Project configuration

In order to verify your contract, you'll need to add a `tool.voyager` table in your `Scarb.toml`, for example:

```toml
[package]
name = "my_project"
version = "0.1.0"

[dependencies]
starknet = ">=2.4.0"

[[target.starknet-contract]]
sierra = true

# Add the following section
[tool.voyager]
my_contract = { path = "src/main.cairo" }
```

The path should point to the file containing cairo module that you wish to verify. In the example above, the cairo contract in question is located at `src/main.cairo`.

*Note* that only one contract should be provided in this section as multi contract verification is not supported yet.

### Verification

`starknet-contract-verifier` provides two subcommands: `submit` and `status`. For both cases user needs to select the network with which they want to interact via the `--network` argument. Possible cases are:
- `mainnet`, main starknet newtwork,
- `testnet`, sepolia test networ,
- `custom`, set provide custom addresses via `--public` and `--private arguments.

#### Submiting for verification

In order to submit contract for verification user needs to provide several arguments:
- `--path`, path to directory containing scarb project (If omitted it will use current workingi directory),
- `--name`, name which will be used in the block explorer for the verified contract, 
- `--hash`, class hash of the declared contract.

there are more options, each of them is documented in the `--help` output.

If the submission is successful, client will output the verification job id.

#### Checking job status

User can query the verification job status using `status` command and providing job id as the `--job` argument value.

## Building the project

`starknet-contract-verifier` is a rust/cargo project. In order to build it you'll need rust and cargo set up. You can do it easily using [rustup](https://rustup.rs/).

```bash
curl https://sh.rustup.rs -sSf | sh -s
```

Then you should be able to build via:

```bash
cargo build
```

or you can run the executable directly using:

```bash
cargo run
```
