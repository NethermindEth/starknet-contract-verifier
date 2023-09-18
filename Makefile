build-darwin:
	cargo build --all --release --target aarch64-apple-darwin
	cargo build --all --release --target x86_64-apple-darwin

build-linux:
	cargo build --all --release --target x86_64-unknown-linux-gnu

build-windows:
	cargo build --all --release --target x86_64-pc-windows-gnu