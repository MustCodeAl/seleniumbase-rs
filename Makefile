# Make fallback for the SeleniumBase Rust port.
# Prefer `just` (https://github.com/casey/just) for richer recipes.

.PHONY: all fmt test test-all lint build docs serve-docs smoke completions clean release-check

all: fmt lint test-all docs

fmt:
	cargo fmt --all -- --check

test:
	cargo test

test-all:
	cargo test --features s3,azure,gcp,playwright,mcp-server,full-tracing

lint:
	cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing -- -D warnings

build:
	cargo build --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing

docs:
	mdbook build

serve-docs:
	mdbook serve --open

smoke:
	cargo run --quiet --bin sbase -- --help
	cargo run --quiet --bin sbase -- --version
	cargo run --quiet --bin sbase -- doctor

completions:
	mkdir -p target/completions
	cargo run --quiet --bin sbase -- completions bash > target/completions/sbase.bash
	cargo run --quiet --bin sbase -- completions zsh > target/completions/_sbase
	cargo run --quiet --bin sbase -- completions fish > target/completions/sbase.fish
	cargo run --quiet --bin sbase -- completions elvish > target/completions/sbase.elv
	cargo run --quiet --bin sbase -- completions powershell > target/completions/_sbase.ps1

clean:
	cargo clean
	rm -rf ~/.cache/seleniumbase-rs/patched-chromedriver

release-check: fmt lint test-all docs
	cargo publish --dry-run
