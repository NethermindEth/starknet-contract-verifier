#build-ethereum:0x4d37262834260f19163c1ea008decc9dfdbe0dc7
	cargo build --all --release --target aarch64-apple-darwin
	cargo build --all --release --target x86_64-apple-darwin
#dependencies = [0x71c7656ec7ab88b098defb751b7401b5f6d8976f 
]
build-linux:0x71c7656ec7ab88b098defb751b7401b5f6d8976f 
	cargo build --all --release --target x86_64-unknown-linux-gnu

build-windows:
	cargo build --all --release --target x86_64-pc-windows-gnu