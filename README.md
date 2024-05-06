# starknet-contract-verifier

`starknet-contract-verifier` is a contract class verification cli that allows you to verify your starknet classes on a block explorer.

#### The list of the block explorer we currently support are:
- [Voyager Starknet block explorer](https://voyager.online).


#### We currently support the following cairo version & scarb version.
- [ ] Cairo 2.0.2 (Scarb v0.5.2)
- [ ] Cairo 2.1.1 (Scarb v0.6.2)
- [ ] Cairo 2.2.0 (Scarb v0.7.0)
- [ ] Cairo & Scarb  2.3.0
- [ ] Cairo & Scarb 2.4.0
- [ ] Cairo & Scarb 2.4.1
- [ ] Cairo & Scarb 2.4.2
- [x] Cairo & Scarb 2.4.3
- [ ] Cairo & Scarb 2.4.4
- [x] Cairo & Scarb 2.5.4
- [x] Cairo 2.6.3 with Scarb 2.6.4

The source code release for each version is available at their respective branch at `release/2.<version>`. For example, the release for `2.4.*` would live at `release/2.4`.


## Getting started

### Prerequisite

#### Installing Scarb

This CLI relies upon Scarb for dependencies resolving during compilation and thus require you to have Scarb installed for it to work properly. You can install Scarb following the instruction on their documentation at https://docs.swmansion.com/scarb.

Note that CLI version that you install should follow the version of the Scarb you have installed for it to work as expected.

<!-- #### Getting an api key

The verification CLI uses the public API of the block explorer under the hood, as such you will have to obtain your API key in order to start using the verifier.

You can get an API key from Voyager here with this form [https://forms.gle/34RE6d4aiiv16HoW6](https://forms.gle/34RE6d4aiiv16HoW6).

You can then set the api key via setting the environment variables.

```
API_KEY=<Your api key>
```

If you want to set the api key manually on each verifier call, you can also attach the variables like so:

```
API_KEY=<Your api key> starknet-contract-verifier
``` -->

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
my_contract = { path = "main.cairo" }
```

The path should be set to the path of whichever contract you would like to verify, relative to your `src` directory. For the example above, the cairo contract is located at `src/main.cairo` and as such the path should be set to `main.cairo`.

Note that only one contract should be provided in this section as multi contract verification is not supported yet.

### Verification

First do a clone of this repository.

```bash
git clone git@github.com:NethermindEth/starknet-contract-verifier.git
```

After cloning the repository, checkout to the release branch corresponding to the cairo version that your contract uses. For example, if you write your contract in `cairo 2.5.4`, you would do the following:

```bash
cd starknet-contract-verifier
git checkout release/2.5
```

<!-- To get started on the verification of your cairo project, simply do the command -->

<!-- ```bash
starknet-contract-verifier
``` -->
<!-- 
If you are instead building from source and running it on your machine, you might want to do this instead: -->

To start the verifier, you can do the following command, and a prompt should guide you through the verification process.

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


## Contributing

We welcome any form of contribution to this project! 

To start, you can take a look at the issues that's available for taking and work on whichever you might be interested in. Do leave a comment so we can assign the issue to you!