# Building and installing

How to build `seleniumbase-rs` from source, which features cost what, and how to
run it in a container. For writing and running tests with it, start with
[Getting started](tutorials/getting_started.md); for working on the crate, see
`CONTRIBUTING.md`.

## What you need

- **Rust 1.99 or newer** (stable). `rust-version` in `Cargo.toml` is the newest
  stable release, and the CI `msrv` job builds with exactly that toolchain.
- **A Chrome or Chromium** to drive. The pure CDP engine (`sb_cdp`) needs
  nothing else; the WebDriver engine also needs a chromedriver of the same major
  version, which `sbase install` downloads and `sbase doctor` checks.
- **For the optional `impersonate` feature only:** `cmake` and a C/C++ toolchain,
  because it builds BoringSSL. (Part of that feature exists; see
  [Stealth Scraping](tutorials/stealth_scraping.md) for what.)

## Build

```bash
git clone https://github.com/MustCodeAl/seleniumbase-rs.git
cd seleniumbase-rs

cargo build --release --locked --bin sbase                              # the CLI
cargo build --release --locked --bin seleniumbase-mcp --features mcp-server   # the MCP server
```

There are two binaries: `sbase` (always built) and `seleniumbase-mcp` (needs the
`mcp-server` feature). To put them on your `PATH`:

```bash
cargo install --path . --bin sbase
cargo install --path . --bin seleniumbase-mcp --features mcp-server
```

The crate is not published to crates.io, so `cargo install seleniumbase-rs` does
not work; build from a checkout or add it as a git dependency.

## Features

No feature is on by default, so a plain build is small and pulls in none of the
desktop or cloud stacks.

| Feature | Adds | Note |
|---|---|---|
| `mcp-server` | the `seleniumbase-mcp` binary (`rmcp`) | needed for the MCP servers |
| `tui` | `sbase commander`, the terminal UI | without it the command prints a hint and exits with status 2 |
| `gui` | native OS dialogs and GUI automation | links GTK and X11 on Linux |
| `playwright` | the Playwright-compatible mode (`rustwright`) | |
| `s3`, `azure`, `gcp` | uploading reports and artifacts | |
| `turso` | `ResultStore`, `ProfileVault`, `sbase report` | an embedded database |
| `full-tracing`, `error-backtrace` | extra tracing, a backtrace on every error | for debugging |
| `test-util` | the scripted mock browser | for tests that drive pages without Chrome |
| `scrape` | wire fingerprints (JA3, JA4, Akamai), cookie jar, stealth identities, `BrowserScraper` (adds `psl`) | pure Rust; see [Stealth Scraping](tutorials/stealth_scraping.md) |
| `impersonate` | browser-identical TLS and HTTP/2 (BoringSSL) | client and TLS server exist; `HttpSession`, `MitmProxy` and `StealthSession` do not yet; needs cmake |

Combine them with `--features "mcp-server tui"`.

## Check your build

```bash
sbase --help
sbase doctor          # compares the chromedriver version with Chrome's
sbase install         # fetches the chromedriver that matches your Chrome
```

## Container

The `Dockerfile` builds `sbase` with locked dependencies in a Rust 1.99 stage and
copies it into a `debian:bookworm-slim` image with Chromium and its driver. It
runs as a non-root user and has no `EXPOSE`, because `sbase` is a command-line
tool.

```bash
docker build -t seleniumbase-rs .
docker run --rm seleniumbase-rs --help
```

If you build through an image optimiser that decides what to keep by watching a
run (Docker Slim, for example), exclude this image: `sbase --help` never starts
Chromium, so the optimised image would lose the browser. Drop `chromium-driver`
from the `Dockerfile` if you only use the pure CDP engine.

Adding a `[[bin]]`, `[[example]]`, `[[bench]]` or `[[test]]` to `Cargo.toml`
also needs a line in the `Dockerfile` skeleton step, or the image build fails;
`tests/dockerfile_skeleton.rs` checks it.

## Build time and disk

A cold build compiles a large dependency tree, and a development `target/`
directory with the test, bench and check artifacts grows past 10 GB. To keep
rebuilds cheap:

```bash
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
```

- Install `sccache` and set `[build] rustc-wrapper = "sccache"` in
  `~/.cargo/config.toml`; it needs `CARGO_INCREMENTAL=0` to cache anything. It
  adds its own cache directory (limit its size) and does not shrink `target/`.
- On a machine with little memory, keep `CARGO_BUILD_JOBS` low and run one cargo
  command at a time; cargo serialises builds in the same target directory.
- Several checkouts of this repository (for example git worktrees) must not share
  one `CARGO_TARGET_DIR`: they overwrite each other's compiled library.
- Do not `cargo clean` a target directory that other builds are using.

## Build the documentation

```bash
cargo install mdbook
mdbook build           # writes the book to book/
mdbook serve --open    # live preview while editing
```

The generated `docs/parity.md` is not edited by hand: `just parity-docs`
rebuilds it from `parity/api.toml`.

## Platforms

CI builds and tests on Linux, macOS and Windows (`.github/workflows/ci.yml`);
development and the project's own checks run on macOS (Apple silicon). Measured
browser fingerprint data is macOS only for now. Per-system notes are in
[Operating systems](platforms.md).
