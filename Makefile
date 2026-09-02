BINARY := filler

.PHONY: build test visualizer visualizer-no-open fmt clippy clean

build:
	cargo build --release --bin $(BINARY)

test:
	cargo test

visualizer:
	cargo run --bin visualizer

visualizer-no-open:
	cargo run --bin visualizer -- --no-open

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

clean:
	cargo clean
