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
    cargo test --features s3,azure,gcp,playwright,mcp-server,full-tracing

# Run Clippy with the strictest settings for all features
lint:
    cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing -- -D warnings

# Build the library and all binaries
build:
    cargo build --all-targets --features s3,azure,gcp,playwright,mcp-server,full-tracing

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
