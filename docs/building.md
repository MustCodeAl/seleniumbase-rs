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
- **For the optional `impersonate` feature only:** `cmake`, a C/C++ toolchain and
  libclang (the BoringSSL crate generates its bindings with bindgen), because it
  builds BoringSSL. On Debian 12 that is `apt-get install cmake clang libclang-dev`
  on top of the `rust` image, which built it on arm64. (Part of that feature
  exists; see [Stealth Scraping](tutorials/stealth_scraping.md) for what.)

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
| `impersonate` | browser-identical TLS and HTTP/2 (BoringSSL): the client, the TLS server, `HttpSession`, `MitmProxy` and `StealthSession` | needs cmake and a C/C++ toolchain; macOS and Chrome 155 only are measured |

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
runs as a non-root user (`sbase`) and has no `EXPOSE`, because `sbase` is a
command-line tool. The driver in `/usr/bin` is linked as
`downloaded_drivers/chromedriver` in the working directory, which is where the
WebDriver engine looks first, so the image needs no network to start a browser.

```bash
docker build --no-slim -t seleniumbase-rs .
docker run --rm seleniumbase-rs --help
docker run --rm seleniumbase-rs doctor
docker run --rm --network none --shm-size=1g seleniumbase-rs --headless smoke \
    'data:text/html,<title>Smoke</title><h1>hi</h1>' --title-contains Smoke
```

- On a machine with little memory, limit Cargo's parallelism in the build:
  `docker build --build-arg CARGO_BUILD_JOBS=2 -t seleniumbase-rs .`
- `--no-slim` belongs to the local build wrapper that runs Slim (see below). Plain
  `docker build` does not know it ("unknown flag: --no-slim"), so leave it out there.
- The arguments after the image name are `sbase` arguments. `--headless` comes
  before the subcommand. `sbase open` and `smoke` do not read `SB_HEADLESS` (only
  `--config` makes `sbase` apply the `SB_*` variables), and without `--headless`
  Chromium fails in a container for lack of a display.
- If you build through an image optimiser that decides what to keep by watching a
  run (Docker Slim, for example), exclude this image: `sbase --help` never starts
  Chromium, so the optimised image would lose the browser. Drop `chromium-driver`
  from the `Dockerfile` if you only use the pure CDP engine (and then drop the
  `ln -s` step that links it).

Verified on an Apple-silicon Mac under OrbStack (so linux/arm64, Debian 12,
Chromium 154.0.8037.92): the build succeeds in about four minutes with two Cargo
jobs and gives a 1.19 GB image; `--help` and `doctor` exit with status 0, and
`doctor` finds `/usr/bin/chromium` and a chromedriver of the same major version;
the `smoke` command above exits with status 0 with no network. `sbase install` in
the image downloads an aarch64 chromedriver (Chrome for Testing publishes
`linux-arm64` from Chrome 153 on).

### Run the tests in a container

`docker/Dockerfile.verify` is a throwaway image for the test suite: the Rust
toolchain, Debian's Chromium and chromedriver, `cmake` and `clang` for the
`impersonate` feature's BoringSSL, and Xvfb. `docker/verify-linux.sh` is its entry
point: it copies the read-only checkout mounted at `/src` to `/work`, so tests that
write beside their sources cannot touch your tree, and then runs your command there.
The build output lives in a named volume (about 4.4 GB for the runs below), and the
Cargo registry in another.

```bash
docker build --no-slim -f docker/Dockerfile.verify -t seleniumbase-rs:verify-tools .
docker volume create sbrs-verify-target
docker volume create sbrs-verify-registry

# Run a command on a copy of this checkout. 4 GB and two Cargo jobs fit an 8 GB machine.
run() {
  docker run --rm --memory=4g --memory-swap=4g --shm-size=1g -e CARGO_BUILD_JOBS=2 \
    -v "$PWD":/src:ro -v sbrs-verify-target:/target \
    -v sbrs-verify-registry:/usr/local/cargo/registry \
    seleniumbase-rs:verify-tools "$@"
}

run cargo test --locked --features "mcp-server test-util scrape" --no-fail-fast --lib --tests
run cargo test --locked --features "mcp-server test-util scrape" --no-fail-fast --doc \
    -- --test-threads=2
run cargo test --locked --features "impersonate test-util" --lib impersonate
run cargo test --locked --features "impersonate test-util" --no-fail-fast \
    --test impersonate_http --test impersonate_mitm --test stealth_session \
    --test impersonate_wire --test impersonate_capture_h2

# A real-browser binary, one at a time:
run cargo test --locked --features "mcp-server test-util scrape" --test sb_cdp_chrome \
    -- --ignored --test-threads=1

# The tests that need a headed Chrome get a virtual display:
docker run --rm --memory=4g --memory-swap=4g --shm-size=1g -e SBRS_XVFB=1 \
    -e CARGO_BUILD_JOBS=2 -v "$PWD":/src:ro -v sbrs-verify-target:/target \
    -v sbrs-verify-registry:/usr/local/cargo/registry seleniumbase-rs:verify-tools \
    cargo test --locked --features "mcp-server test-util scrape" --test scrape_capture_chrome \
    -- --ignored --test-threads=1
```

The doc tests need `-- --test-threads=2` in 4 GB: with the default of one thread per
CPU, the linker was killed for lack of memory in 66 of them. Give the container more
memory instead if you have it.

The image sets `CI=true` and `SB_NO_SANDBOX=1` (Chromium cannot start its sandbox as
root in a container, and the real-browser tests read `CI`) and `SB_HEADLESS=true`.
The results, and what they showed, are under
[Verified on Linux](platforms.md#verified-on-linux). Remove everything when you are
done:

```bash
docker rmi seleniumbase-rs:verify-tools seleniumbase-rs
docker volume rm sbrs-verify-target sbrs-verify-registry
```

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
development and the project's own checks run on macOS (Apple silicon). The suite
was also run on Linux arm64 (Debian 12, Chromium 154) in a container, as above.
Measured browser fingerprint data is macOS only for now. Per-system notes are in
[Operating systems](platforms.md).
