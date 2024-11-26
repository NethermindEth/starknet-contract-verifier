# Starknet Contract Verifier

`starknet-contract-verifier` is a command-line tool for verifying your Starknet classes on supported block explorers.


## Supported Block Explorers

Currently, this tool supports:

- [Voyager Starknet Block Explorer](https://voyager.online)


## Supported Versions

We support the following Cairo and Scarb versions:

- **Cairo 1.1.0 & Scarb 0.4.0** through **Cairo 2.8.4 & Scarb 2.8.4**

Source code for each release is available under its respective branch (e.g., `release/2.4.3` for version 2.4.3).


## Getting Started

### Prerequisites

#### Installing Scarb

This CLI relies on Scarb for dependency resolution during compilation. Install Scarb by following the official [installation guide](https://docs.swmansion.com/scarb).

Ensure that the CLI version you install matches your Scarb version for compatibility.

### Configuration for Verification

To begin verification, add the following table to your `Scarb.toml`:

```toml
[package]
name = "my_project"
version = "0.1.0"

[dependencies]
starknet = ">=2.4.0"

[[target.starknet-contract]]
sierra = true

# Add this section
[tool.voyager]
MyContract = { path = "main.cairo" }
```

- Replace `main.cairo` with the relative path to your contract file inside the `src` directory. For example, if your contract is located at `src/main.cairo`, the path should be set to `main.cairo`.
- Only one contract is supported per verification (multi-contract verification is not yet supported).
- Replace `MyContract` with your contract's name. For example, if your contract is defined as shown below, the key should be set to `MyContract`:
```tocairo
#[starknet::contract]
mod MyContract {
    ...
}
```

### Verification Process

1. Clone this repository:

```bash
git clone git@github.com:NethermindEth/starknet-contract-verifier.git
```

2. Check out the release branch corresponding to your Cairo version:

```bash
cd starknet-contract-verifier
git checkout release/<version>
```

3. Start the verifier and follow the prompts:

```bash
cargo run --bin starknet-contract-verifier
```

If you manage your Scarb binary with `asdf`, ensure the verifier runs in your project directory to use the correct binary.

## Building the Verifier

You can build the binaries and add them to your system's `PATH`:

```bash
# Build all binaries
cargo build --all --release

# Add the target directory to your PATH
# depending on your shell this might be different.
# Add the following to the end of your shell configuration file
export PATH="$PATH:/path/to/starknet-contract-verifier/target/release"

# Now you can call the verifier directly
starknet-contract-verifier
```

This enables a more seamless experience.

## Building from source

If developing, install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh -s
```

> Note: Versions 2.4.3 and below require Rust < 1.77. Please ensure the correct Rust version for compatibility.

To build:

```bash
cargo build
```

## Limitations and Known Issues

### 1. Reorganized Modules After Verification

The verifier may reorganize module structures during dependency resolution, resulting in a generated project that differs slightly from the original.

### 2. Version Constraints in `Scarb.toml`

- The verifier works with Cairo compiler versions lower than its own (provided no breaking changes).
- Using strict versioning (e.g., `=2.4.3`) in `Scarb.toml` may restrict compatibility with other verifier versions. Consider using ranges (`>=2.4.0`) for greater flexibility.

## Contributing

We welcome contributions! Check the [issues](https://github.com/NethermindEth/starknet-contract-verifier/issues) and comment on one you're interested in so we can assign it to you.
