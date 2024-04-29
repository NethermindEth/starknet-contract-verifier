# starknet-contract-verifier

`starknet-contract-verifier` is a contract class verification cli that allows you to verify your starknet classes on a block explorer.

The list of the block explorer we currently support are:
- [Voyager Starknet block explorer](https://voyager.online).



## Getting started

### Prerequisite

#### Getting an api key

The verification CLI uses the public API of the block explorer under the hood, as such you will have to obtain your API key in order to start using the verifier.

You can get an API key from Voyager here with this form [https://forms.gle/34RE6d4aiiv16HoW6](https://forms.gle/34RE6d4aiiv16HoW6).

You can then set the api key via setting the environment variables.

```
API_KEY=<Your api key>
```

If you want to set the api key manually on each verifier call, you can also attach the variables like so:

```
API_KEY=<Your api key> starknet-contract-verifier
```

#### Adding configuration for the verification

In order to start verification, you'll need to add a table in your `Scarb.toml` as such:

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
my_contract = { path= "main.cairo" }
```

The path should be set to the path of whichever contract you would like to verify.

Note that only one contract should be provided in this section as multi contract verification is not supported yet.

### Verification

To get started on the verification of your cairo project, simply do the command

```bash
starknet-contract-verifier
```

If you are instead building from source and running it on your machine, you might want to do this instead:

```bash
cargo run --bin starknet-contract-verifier
```

You should be greeted with prompts that asks for the details of your cairo project & contracts, and will be guided step by step through the verification process.

## Building from source

If you are developing and building the project from source, you will first need to install rust.

```bash
curl https://sh.rustup.rs -sSf | sh -s
```

> Note: Builds for 2.4.3 and below only works with < Rust 1.77. As such please make sure that you have the correct rust version before building.

To build the project, simply do

```bash
cargo build
```

and the project should start building.