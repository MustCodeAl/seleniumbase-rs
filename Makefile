# Make fallback for the SeleniumBase Rust port.
# Prefer `just` (https://github.com/casey/just) for richer recipes.

.PHONY: all fmt test test-all lint build docs serve-docs smoke completions docker clean clean-cache release-check

all: fmt lint test-all docs

fmt:
	cargo fmt --all -- --check

test:
	cargo test

test-all:
	cargo test --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui

lint:
	cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui -- -D warnings

build:
	cargo build --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui

docs:
	mdbook build

serve-docs:
	mdbook serve --open

smoke:
	cargo run --quiet --bin sbrs -- --help
	cargo run --quiet --bin sbrs -- --version
	cargo run --quiet --bin sbrs -- doctor

completions:
	mkdir -p target/completions
	cargo run --quiet --bin sbrs -- completions bash > target/completions/sbrs.bash
	cargo run --quiet --bin sbrs -- completions zsh > target/completions/_sbrs
	cargo run --quiet --bin sbrs -- completions fish > target/completions/sbrs.fish
	cargo run --quiet --bin sbrs -- completions elvish > target/completions/sbrs.elv
	cargo run --quiet --bin sbrs -- completions powershell > target/completions/_sbrs.ps1

# Chromium needs a browser inside, and Slim cannot see it load, so no --no-slim
# means a slimmed image without Chromium.
docker:
	docker build --no-slim -t seleniumbase-rs .

clean:
	cargo clean

# The patched-browser cache `sbrs patch-chrome` fills (the platform cache
# directory: ~/.cache on Linux, ~/Library/Caches on macOS) and downloaded drivers.
clean-cache:
	rm -rf "$${XDG_CACHE_HOME:-$$HOME/.cache}/sb-chrome-patches" "$$HOME/Library/Caches/sb-chrome-patches" downloaded_drivers

release-check: fmt lint test-all docs
	cargo publish --dry-run
