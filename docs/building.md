# Building and installing

How to build `seleniumbase-rs` from source, which features cost what, and how to
run it in a container. For writing and running tests with it, start with
[Getting started](tutorials/getting_started.md); for working on the crate, see
`CONTRIBUTING.md`.

## What you need

Pick your system below and paste its box into a terminal. Each box installs what
every build needs: Rust 1.99 (the repository's `rust-toolchain.toml` selects it),
a C compiler, and Google Chrome. The extras for single features come after.
Nothing here needs Python.

### Let an AI assistant do it

Paste this into an AI coding assistant that can run commands on your machine
(Claude Code, Cursor, Copilot and the like):

```text
Set up this computer to build and test the Rust library seleniumbase-rs
(https://github.com/MustCodeAl/seleniumbase-rs). Follow its docs/building.md
("What you need") for my operating system:

1. Find out which OS and CPU this is. Install what is missing, and only that:
   Rust through rustup (the repo's rust-toolchain.toml picks 1.99), a C compiler
   (Xcode command line tools / build-essential / Visual Studio 2022 Build Tools
   with C++), and Google Chrome. Use Homebrew on macOS, apt or dnf on Linux,
   winget on Windows. Ask me before anything that needs an administrator
   password, and don't change system security settings yourself.
2. Clone the repository (or use this folder if it is the repository) and run:
   cargo build --bin sbase, then sbase doctor (target/debug/sbase doctor, or
   target\debug\sbase.exe doctor on Windows).
3. Run the checks and say how many tests passed and failed:
   cargo fmt --all -- --check
   cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
   cargo test --features "mcp-server test-util"
4. Only if I ask for a feature, install its extras from the "Extras for single
   features" part of docs/building.md (s3, gui, impersonate, the Tauri example).
Don't edit any code. Finish with a short list of what you installed, the
versions, and anything that failed with its first error line.
```

### macOS

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
brew install --cask google-chrome
```

Optional browsers and drivers (drivers otherwise download by themselves):

```bash
brew install --cask microsoft-edge firefox
brew install chromedriver geckodriver
```

Safari is built in. Allow automation once, then turn on Safari ▸ Develop ▸
Allow Remote Automation:

```bash
sudo safaridriver --enable
```

### Debian and Ubuntu

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

Google Chrome on x86-64:

```bash
wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
sudo apt-get install -y ./google-chrome-stable_current_amd64.deb
```

Chromium instead (also on arm64), and Firefox:

```bash
sudo apt-get install -y chromium firefox-esr
```

A display for headed browsers on a server:

```bash
sudo apt-get install -y xvfb
```

### Fedora

```bash
sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl chromium
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

### Windows (PowerShell)

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Google.Chrome
```

Edge comes with Windows. Firefox:

```powershell
winget install Mozilla.Firefox
```

Open a new terminal after installing, so the new programs are on the `PATH`.

### Check it worked

```bash
git clone https://github.com/MustCodeAl/seleniumbase-rs.git
cd seleniumbase-rs
cargo build --bin sbase
./target/debug/sbase doctor
```

On Windows the last line is `.\target\debug\sbase.exe doctor`. It prints the
browsers it found and whether their drivers match.

### Extras for single features

Most features need nothing more: `tui`, `scrape`, `mcp-server`, `turso`,
`playwright`, `azure`, `gcp`, `full-tracing` and `test-util`. The rest:

**`s3`** needs OpenSSL on Linux (nothing on macOS and Windows):

```bash
sudo apt-get install -y libssl-dev pkg-config
```

**`gui`** (desktop input and native dialogs). On macOS nothing to install; at
run time allow the program under System Settings ▸ Privacy & Security ▸
Accessibility. On Windows nothing. On Debian and Ubuntu:

```bash
sudo apt-get install -y libgtk-3-dev libx11-dev libxdo-dev libxkbcommon-dev libxtst-dev libssl-dev pkg-config
```

**`impersonate`** builds BoringSSL from source and needs CMake and libclang.

macOS (Xcode's tools bring clang and libclang):

```bash
brew install cmake
```

Debian and Ubuntu (Fedora: `sudo dnf install -y cmake clang clang-devel`):

```bash
sudo apt-get install -y cmake clang libclang-dev
```

Windows, after the Build Tools above:

```powershell
winget install Kitware.CMake
winget install NASM.NASM
winget install LLVM.LLVM
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

Optional and faster: Ninja (`brew install ninja`, `sudo apt-get install -y
ninja-build`, `winget install Ninja-build.Ninja`) with `CMAKE_GENERATOR=Ninja`.
On Windows, Ninja builds must run from a "Developer PowerShell for VS 2022".

**The Tauri example** (`examples/tauri-profile-manager`) needs Node LTS (`brew
install node`, `winget install OpenJS.NodeJS.LTS`). On Windows it also uses
WebView2, which comes with Windows 10 and 11. On Debian and Ubuntu:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev libssl-dev libx11-dev libxdo-dev
```

**Repository tools:** mdBook for the docs (`brew install mdbook` or `cargo
install mdbook`), Python 3.10+ for the parity tables in `tools/parity`, and
`just` and `cargo-deny` (`brew install just cargo-deny` or `cargo install just
cargo-deny`). With [mise](https://mise.jdx.dev), `mise install` sets up Rust,
Python, Node, CMake, Ninja, mdBook, just and cargo-deny in one step.

### What has been built where

Everything on macOS (Apple Silicon). The default build, the test features and
`impersonate` on Debian 12 arm64 in Docker. The default build, the test
features, the real-browser tests and the Tauri example on Windows 11 x64.
`impersonate` has not been built on Windows yet. The Linux `gui` and Tauri
package lists come from the CI configuration, which has not been able to run
(see [Operating systems](platforms.md)).

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
docker build -t seleniumbase-rs .
docker run --rm seleniumbase-rs --help
docker run --rm seleniumbase-rs doctor
docker run --rm --network none --shm-size=1g seleniumbase-rs --headless smoke \
    'data:text/html,<title>Smoke</title><h1>hi</h1>' --title-contains Smoke
```

- On a machine with little memory, limit Cargo's parallelism in the build:
  `docker build --build-arg CARGO_BUILD_JOBS=2 -t seleniumbase-rs .`
- The arguments after the image name are `sbase` arguments. `--headless` comes
  before the subcommand. `sbase open` and `smoke` also read `SB_HEADLESS` (and the
  other `SB_*` variables; `--headless` beats the variable), and without one of
  them Chromium fails in a container for lack of a display.
- Drop `chromium-driver` from the `Dockerfile` if you only use the pure CDP
  engine (and then drop the `ln -s` step that links it).

Verified on an Apple-silicon Mac with Docker (so linux/arm64, Debian 12,
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
docker build -f docker/Dockerfile.verify -t seleniumbase-rs:verify-tools .
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
