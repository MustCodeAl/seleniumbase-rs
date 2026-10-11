# Just recipes for SeleniumBase Rust port
# https://github.com/casey/just

set fallback

_default:
    @just --list

# Run formatting checks
fmt:
    cargo fmt --all -- --check

# Run the full test suite (default features)
test:
    cargo test

# Run tests with all optional features enabled
# On macOS you may need to export DYLD_LIBRARY_PATH for the playwright feature.
test-all:
    cargo test --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui

# Run Clippy with the strictest settings for all features
lint:
    cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui -- -D warnings

# Build the library and all binaries
build:
    cargo build --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui

# Build the mdBook documentation
docs:
    mdbook build

# Serve the mdBook locally for editing
serve-docs:
    mdbook serve --open

# Run sbase CLI smoke checks
smoke:
    cargo run --quiet --bin sbase -- --help
    cargo run --quiet --bin sbase -- --version
    cargo run --quiet --bin sbase -- doctor

# Generate shell completions for Bash, Zsh, Fish, Elvish, and PowerShell
completions outdir="target/completions":
    cargo run --quiet --bin sbase -- completions bash > {{outdir}}/sbase.bash
    cargo run --quiet --bin sbase -- completions zsh > {{outdir}}/_sbase
    cargo run --quiet --bin sbase -- completions fish > {{outdir}}/sbase.fish
    cargo run --quiet --bin sbase -- completions elvish > {{outdir}}/sbase.elv
    cargo run --quiet --bin sbase -- completions powershell > {{outdir}}/_sbase.ps1

# Clean build artifacts and patch cache
clean:
    cargo clean
    rm -rf ~/.cache/seleniumbase-rs/patched-chromedriver

# Prepare a release: format, lint, test, docs, dry-run publish
release-check: fmt lint test-all docs
    cargo publish --dry-run

# Extract the upstream API: just parity-extract PATH/TO/SeleniumBase PATH/TO/seleniumbase-mcp
parity-extract seleniumbase mcp:
    python3 -I tools/parity/extract_api.py --seleniumbase {{seleniumbase}} --mcp {{mcp}} --out parity/upstream-api.json

# Add proposed entries to parity/api.toml for upstream names it does not list
parity-seed:
    python3 -I tools/parity/seed_manifest.py

# Check that parity/api.toml agrees with the upstream API and the Rust sources
parity-check:
    cargo test --features mcp-server --test parity

# Regenerate docs/parity.md and fail if it was out of date
parity-docs:
    python3 -I tools/parity/render_docs.py
    git diff --exit-code docs/parity.md

# On the 8 GB development machine run it with CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0
# CARGO_PROFILE_DEV_DEBUG=0. It runs format, lint, the domain purity check, the scrape tests,
# and the default build, which must not need the feature. Scrape gate, pure Rust, runs locally.
scrape-gate:
    cargo fmt --all -- --check
    cargo clippy --all-targets --features "mcp-server test-util scrape" -- -D warnings
    cargo test --features "mcp-server test-util scrape" --test scrape_domain_purity
    cargo test --features scrape --lib scrape::
    cargo test --features "mcp-server test-util scrape" --test scrape_browser
    cargo test --features "mcp-server test-util scrape" --test scrape_capture
    cargo test --features scrape --doc scrape
    cargo check --no-default-features

# NEEDS cmake and a C/C++ toolchain (BoringSSL is built from source) and about 1 GB of target
# disk. CI runs it on Linux (the `impersonate` job). DO NOT run it on the local 8 GB machine.
# The gate also runs the tests of HttpSession, the MITM proxy and StealthSession that need no
# Chrome (loopback servers and the scripted browser, which needs `test-util`). The real-Chrome
# ones (`impersonate_http_chrome`, `impersonate_mitm_chrome`, `stealth_session_chrome`) are
# `#[ignore]`d: run them by hand, one binary at a time.
# Impersonate gate (needs cmake, CI only).
imp-gate:
    cmake --version
    CMAKE_BUILD_PARALLEL_LEVEL=1 cargo check --features impersonate --all-targets
    CMAKE_BUILD_PARALLEL_LEVEL=1 cargo clippy --all-targets --features impersonate -- -D warnings
    CMAKE_BUILD_PARALLEL_LEVEL=1 cargo test --features "impersonate test-util" --test impersonate_http
    CMAKE_BUILD_PARALLEL_LEVEL=1 cargo test --features "impersonate test-util" --test impersonate_mitm
    CMAKE_BUILD_PARALLEL_LEVEL=1 cargo test --features "impersonate test-util" --test stealth_session

# Time and peak memory of a one-job `cargo check --lib --features scrape`. psl is a large
# generated crate, so this is the number to watch. macOS uses BSD time (-l), Linux GNU time (-v).
# Time and peak memory of the scrape check (one job).
measure:
    #!/usr/bin/env bash
    set -euo pipefail
    export CARGO_BUILD_JOBS=1
    if [ "$(uname)" = Darwin ]; then
        /usr/bin/time -l cargo check --lib --features scrape
    else
        /usr/bin/time -v cargo check --lib --features scrape
    fi
