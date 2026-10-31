# Just recipes for SeleniumBase Rust port
# https://github.com/casey/just

set fallback

_default:
    @just --list

# Run formatting checks
fmt:
    cargo fmt --all -- --check

# Run the full test suite (default features) of the library and the command line
test:
    cargo test

# Test the library alone, with its own default features (no command line)
test-library:
    cargo test -p seleniumbase-rs

# Run tests with all optional features enabled
test-all:
    cargo test --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui,pdf

# Run Clippy with the strictest settings for all features
lint:
    cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui,pdf -- -D warnings

# Build the library and all binaries (the command line is crates/cli)
build:
    cargo build --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui,pdf

# Build the mdBook documentation
docs:
    mdbook build

# Build every docs page as the Docs7 site does (MDX); fails on undefined names,
# relative .md links, missing titles and pages that are in no menu (needs Bun)
mdx:
    bun install --cwd tools/mdx-check --frozen-lockfile
    bun tools/mdx-check/check.mjs docs

# Write the layout trees of the docs pages again: the tree of the repository, from the one
# description in tools/docs_layout_tree.py, and every copy of the tree component
# (`just layout-tree --check` changes nothing and lists what differs; `just mdx` runs that check)
layout-tree *args:
    python3 -I tools/docs_layout_tree.py {{args}}

# Serve the mdBook locally for editing
serve-docs:
    mdbook serve --open

# Run sbrs CLI smoke checks
smoke:
    cargo run --quiet -p seleniumbase-rs-cli --bin sbrs -- --help
    cargo run --quiet -p seleniumbase-rs-cli --bin sbrs -- --version
    cargo run --quiet -p seleniumbase-rs-cli --bin sbrs -- doctor

# Generate shell completions for Bash, Zsh, Fish, Elvish, and PowerShell
completions outdir="target/completions":
    cargo run --quiet -p seleniumbase-rs-cli --bin sbrs -- completions bash > {{outdir}}/sbrs.bash
    cargo run --quiet -p seleniumbase-rs-cli --bin sbrs -- completions zsh > {{outdir}}/_sbrs
    cargo run --quiet -p seleniumbase-rs-cli --bin sbrs -- completions fish > {{outdir}}/sbrs.fish
    cargo run --quiet -p seleniumbase-rs-cli --bin sbrs -- completions elvish > {{outdir}}/sbrs.elv
    cargo run --quiet -p seleniumbase-rs-cli --bin sbrs -- completions powershell > {{outdir}}/_sbrs.ps1

# Clean build artifacts and patch cache
clean:
    cargo clean
    rm -rf ~/.cache/seleniumbase-rs/patched-chromedriver

# Prepare a release: format, lint, test, docs, dry-run publish
release-check: fmt lint test-all docs
    cargo publish --dry-run -p seleniumbase-rs

# Extract the upstream API: just parity-extract PATH/TO/SeleniumBase PATH/TO/seleniumbase-mcp
parity-extract seleniumbase mcp:
    python3 -I tools/parity/extract_api.py --seleniumbase {{seleniumbase}} --mcp {{mcp}} --out parity/upstream-api.json

# Add proposed entries to parity/api.toml for upstream names it does not list
parity-seed:
    python3 -I tools/parity/seed_manifest.py

# Check that parity/api.toml agrees with the upstream API and the Rust sources
parity-check:
    cargo test -p seleniumbase-rs --features mcp-server --test parity

# Regenerate docs/parity.md and fail if it was out of date
parity-docs:
    python3 -I tools/parity/render_docs.py
    git diff --exit-code docs/parity.md

# Check that the contract files (tests/contract_*.rs) name everything the repositories that build
# on this crate use: it reads the sibling checkouts (see docs/contributing/consumers.md for their
# folders), `just contracts --check-build` also runs `cargo check --tests` in each against this
# checkout (heavy: for a release)
contracts *args:
    scripts/check-consumers.sh {{args}}

# Copy the companion extension's golden programs into tests/contract_companion/
contracts-sync:
    scripts/check-consumers.sh --sync

