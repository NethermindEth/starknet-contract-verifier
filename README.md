# starknet-contract-verifier

`starknet-contract-verifier` is a contract class verification cli that allows you to verify your contract on the [Voyager Starknet block explorer](https://voyager.online).

## Getting started

To get started on the verification of your cairo project, simply do the command

```bash
starknet-contract-verifier
```

You should be greeted with prompts that asks for the details of your cairo project & contracts, and will be guided step by step through the verification process.


If you are instead building from source and running it on your machine, you might want to do this instead:

```bash
cargo run --bin starknet-contract-verifier
```

## Building from source

If you are developing and building the project from source, you will first need to install rust.

```bash
curl https://sh.rustup.rs -sSf | sh -s
```

> Note: Builds for 2.4.3 and below only works with <= Rust 1.77.

To build the project, simply do

```bash
cargo build
```

and the project should start building.