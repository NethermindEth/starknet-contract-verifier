FROM rust:1.77-slim as builder

WORKDIR /usr/src/starknet-contract-verifier
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/starknet-contract-verifier/target/release/starknet-contract-verifier /usr/local/bin/starknet-contract-verifier

ENTRYPOINT ["starknet-contract-verifier"] 