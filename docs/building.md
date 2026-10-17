---
title: "Building and installing"
---

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

<Tip>

In a hurry? Give the prompt below to your AI coding assistant and let it
install and check everything.

</Tip>

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
   cargo build --bin sbrs, then sbrs doctor (target/debug/sbrs doctor, or
   target\debug\sbrs.exe doctor on Windows).
3. Run the checks and say how many tests passed and failed:
   cargo fmt --all -- --check
   cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
   cargo test --features "mcp-server test-util"
4. Only if I ask for a feature, install its extras from the "Extras for single
   features" part of docs/building.md (s3, gui, the Tauri example).
Don't edit any code. Finish with a short list of what you installed, the
versions, and anything that failed with its first error line.
```

<Tabs>

<Tab title="macOS">

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

</Tab>

<Tab title="Debian and Ubuntu">

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

</Tab>

<Tab title="Fedora">

```bash
sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl chromium
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

</Tab>

<Tab title="Windows (PowerShell)">

```powershell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Google.Chrome
```

Edge comes with Windows. Firefox:

```powershell
winget install Mozilla.Firefox
```

<Note>

Open a new terminal after installing, so the new programs are on the `PATH`.

</Note>

</Tab>

</Tabs>

### Check it worked

<CodeGroup>

```bash macOS and Linux
git clone https://github.com/MustCodeAl/seleniumbase-rs.git
cd seleniumbase-rs
cargo build --bin sbrs
./target/debug/sbrs doctor
```

```powershell Windows
git clone https://github.com/MustCodeAl/seleniumbase-rs.git
cd seleniumbase-rs
cargo build --bin sbrs
.\target\debug\sbrs.exe doctor
```

</CodeGroup>

The last line prints the browsers it found and whether their drivers match.

### Extras for single features

Most features need nothing more: `tui`, `mcp-server`, `turso`, `playwright`,
`azure`, `gcp`, `full-tracing`, `test-util`, `ocr`, `visual` and `plugins`. The
rest:

<AccordionGroup>

<Accordion title="s3">

`s3` needs OpenSSL on Linux (nothing on macOS and Windows):

```bash
sudo apt-get install -y libssl-dev pkg-config
```

</Accordion>

<Accordion title="gui">

`gui` (desktop input and native dialogs). On macOS nothing to install; at
run time allow the program under System Settings ▸ Privacy & Security ▸
Accessibility. On Windows nothing. On Debian and Ubuntu:

```bash
sudo apt-get install -y libgtk-3-dev libx11-dev libxdo-dev libxkbcommon-dev libxtst-dev libssl-dev pkg-config
```

</Accordion>

<Accordion title="The stealth scraping layer">

The stealth scraping layer (the former `scrape` and `impersonate` features)
moved in 0.3.0 to its own workspace,
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth).
Its BoringSSL build needs CMake and libclang (and NASM and LLVM on Windows);
those instructions live in that repository now. This crate needs none of them.

</Accordion>

<Accordion title="The Tauri example">

The Tauri example (`examples/tauri-profile-manager`) needs Node LTS (`brew
install node`, `winget install OpenJS.NodeJS.LTS`). On Windows it also uses
WebView2, which comes with Windows 10 and 11. On Debian and Ubuntu:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev libssl-dev libx11-dev libxdo-dev
```

</Accordion>

<Accordion title="Repository tools">

mdBook for the docs (`brew install mdbook` or `cargo
install mdbook`), Python 3.10+ for the parity tables in `tools/parity`, and
`just` and `cargo-deny` (`brew install just cargo-deny` or `cargo install just
cargo-deny`). With [mise](https://mise.jdx.dev), `mise install` sets up Rust,
Python, Node, mdBook, just and cargo-deny in one step.

</Accordion>

</AccordionGroup>

### What has been built where

Everything on macOS (Apple Silicon). The default build and the test features
on Debian 12 arm64 in Docker. The default build, the test features, the
real-browser tests and the Tauri example on Windows 11 x64. The Linux `gui` and Tauri
package lists come from the CI configuration, which has not been able to run
(see [Operating systems](platforms.md)).

## Build

```bash
git clone https://github.com/MustCodeAl/seleniumbase-rs.git
cd seleniumbase-rs

cargo build --release --locked --bin sbrs        # the CLI
cargo build --release --locked --bin sbrs-mcp    # the MCP server
```

The repository is a Cargo workspace of two packages: the library
`seleniumbase-rs` at the root, and the command line `seleniumbase-rs-cli` in
`crates/cli`, which holds both binaries. A project that depends on the library
compiles none of the command-line crates. `sbrs` is always built; `sbrs-mcp`
needs the `mcp-server` feature, which the command-line package turns on by
default together with `pdf`. To put the binaries on your `PATH`:

```bash
cargo install --path crates/cli                       # sbrs and sbrs-mcp
cargo install --path crates/cli --features tui        # and the terminal UI
```

Neither package is published to crates.io, so `cargo install seleniumbase-rs-cli`
does not work; build from a checkout, install from git as below, or add the
library as a git dependency.

## Install without a checkout

Three ways to get `sbrs` and `sbrs-mcp` (the package `seleniumbase-rs-cli`) onto
your `PATH` without cloning the repository. None of them uses crates.io.

<Warning>

The prebuilt archives do not exist yet. They come from
`.github/workflows/release.yml`, which builds `sbrs` and `sbrs-mcp` for
`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu` and `x86_64-pc-windows-msvc` when a `v*` tag is
pushed, and attaches them to that GitHub release. GitHub Actions is locked on
this repository by a billing problem, so the workflow has not run: the
releases v0.1.0 to v0.2.0 have no binary assets. Until it runs, `cargo
binstall` and mise's `github:` backend find nothing to download; the commands
that build from source (`cargo install --git`, mise's `cargo:` backend and
Homebrew) work today.

</Warning>

Each release archive (`seleniumbase-rs-v<version>-<target>.tgz`, `.zip` on
Windows) holds `sbrs` and `sbrs-mcp` at its root, `completions/` (bash, zsh,
fish, PowerShell, a Fig spec and the usage spec), the Warp workflows in
`warp/workflows/`, `README.md` and `LICENSE`. `SHA256SUMS` lists the checksum of
every archive.

### cargo binstall

[cargo-binstall](https://github.com/cargo-bins/cargo-binstall) downloads the
release archive for your system instead of compiling. `crates/cli/Cargo.toml`
tells it where the archives are (`[package.metadata.binstall]`). With `--git` it
reads that manifest from the repository, so no crates.io entry is needed:

```bash
cargo binstall --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli
```

It installs both binaries. Two things to know: `--git` takes the default
branch, so it fetches the release whose version the branch's `Cargo.toml`
names, and that release must have its archives; and the metadata turns off the
compile fallback, because cargo-binstall would compile from crates.io, where the
crate is not. cargo-binstall installs no completions: write them with `sbrs
completions <shell>` (see [the CLI page](tutorials/cli_usage.md)).

### mise

[mise](https://mise.jdx.dev) has two backends that fit. From the release
archives (the `github:` backend picks the archive for your system and puts both
binaries on `PATH`):

```bash
mise use -g github:MustCodeAl/seleniumbase-rs
```

or in `mise.toml`:

```toml
[tools."github:MustCodeAl/seleniumbase-rs"]
version = "latest"
```

Until the release archives exist, mise reports "No versions found" for it. To
build from the git tag now, the `cargo:` backend runs `cargo install --git`
(Rust 1.99 must be on `PATH`, for example with `mise use -g rust@1.99`). The
repository has one package with binaries, `seleniumbase-rs-cli`, so cargo finds
it by itself, and it builds `sbrs-mcp` too (`features = ["tui"]` adds the
terminal UI):

```bash
mise use -g 'cargo:https://github.com/MustCodeAl/seleniumbase-rs@tag:v0.2.0'
```

```toml
[tools."cargo:https://github.com/MustCodeAl/seleniumbase-rs"]
version = "tag:v0.2.0"
```

The binaries moved from the package `seleniumbase-rs` to `seleniumbase-rs-cli`
after v0.4.0. For a tag up to v0.4.0, add `features = ["mcp-server"]` to get
`sbrs-mcp`.

mise installs no completions for these backends either; its per-tool
`postinstall` option can run `sbrs completions zsh` into your completions folder.

### Homebrew

The tap builds from the release's source and installs the completions and the
Warp workflows as well (macOS and Linux):

```bash
brew install mustcodeal/tap/sbrs
```

### cargo install from git

What the methods above do when there is no archive, by hand:

```bash
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --locked
```

That builds `sbrs` and `sbrs-mcp`; add `--no-default-features` for `sbrs` alone,
without PDF text extraction. For a tag up to v0.4.0 the binaries are in the
package `seleniumbase-rs`:
`cargo install --git https://github.com/MustCodeAl/seleniumbase-rs --tag v0.4.0 --features mcp-server --locked`.

## Features

No feature of the library is on by default, so a plain build is small and pulls in
none of the desktop or cloud stacks. The command-line package
`seleniumbase-rs-cli` turns `pdf` and `mcp-server` on by default and forwards
`tui`, `gui` and `turso` to the library; the rest of the features below belong
to the library. Build the library alone, with its own defaults, with
`cargo build -p seleniumbase-rs`.

| Feature | Adds | Note |
|---|---|---|
| `mcp-server` | the `sbrs-mcp` binary (`rmcp`) | needed for the MCP servers |
| `tui` | `sbrs commander`, the terminal UI | without it the command prints a hint and exits with status 2 |
| `gui` | native OS dialogs and GUI automation | links GTK and X11 on Linux |
| `playwright` | the Playwright-compatible mode (`rustwright`) | |
| `s3`, `azure`, `gcp` | uploading reports and artifacts | |
| `turso` | `ResultStore`, `ProfileVault`, `sbrs report` | an embedded database |
| `full-tracing`, `error-backtrace` | extra tracing, a backtrace on every error | for debugging |
| `test-util` | the scripted mock browser | for tests that drive pages without Chrome |
| `ocr` | `ocr::Ocr`, text read from screenshots and PNG images (`ocrs`, `rten`, `image`) | pure Rust; downloads two models (about 12 MB) on first use, see [Reading text from the screen](tutorials/remaining_helpers.md#reading-text-from-the-screen-ocr) |
| `visual` | `visual::VisualBaseline` and `BaseCase::assert_visual_match`: screenshots compared with a baseline by structural similarity, with a diff image on a mismatch (`image-compare`, `image` with PNG only) | pure Rust; brings `rayon`; see [Visual regression testing](tutorials/remaining_helpers.md#visual-regression-testing-pixels) |

Combine them with `--features "mcp-server tui"`. The features `scrape` and
`impersonate` were removed in 0.3.0; see [Stealth Scraping](tutorials/stealth_scraping.md).

## Check your build

```bash
sbrs --help
sbrs doctor          # compares the chromedriver version with Chrome's
sbrs install         # fetches the chromedriver that matches your Chrome
```

## Container

The `Dockerfile` builds `sbrs` with locked dependencies in a Rust 1.99 stage and
copies it into a `debian:bookworm-slim` image with Chromium and its driver. It
runs as a non-root user (`sbrs`) and has no `EXPOSE`, because `sbrs` is a
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
- The arguments after the image name are `sbrs` arguments. `--headless` comes
  before the subcommand. `sbrs open` and `smoke` also read `SB_HEADLESS` (and the
  other `SB_*` variables; `--headless` beats the variable), and without one of
  them Chromium fails in a container for lack of a display.
- Drop `chromium-driver` from the `Dockerfile` if you only use the pure CDP
  engine (and then drop the `ln -s` step that links it).

<Check>

Verified on an Apple-silicon Mac with Docker (so linux/arm64, Debian 12,
Chromium 154.0.8037.92): the build succeeds in about four minutes with two Cargo
jobs and gives a 1.19 GB image; `--help` and `doctor` exit with status 0, and
`doctor` finds `/usr/bin/chromium` and a chromedriver of the same major version;
the `smoke` command above exits with status 0 with no network. `sbrs install` in
the image downloads an aarch64 chromedriver (Chrome for Testing publishes
`linux-arm64` from Chrome 153 on).

</Check>

### Run the tests in a container

`docker/Dockerfile.verify` is a throwaway image for the test suite: the Rust
toolchain, Debian's Chromium and chromedriver, and Xvfb. `docker/verify-linux.sh` is its entry
point: it copies the read-only checkout mounted at `/src` to `/work`, so tests that
write beside their sources cannot touch your tree, and then runs your command there.
The build output lives in a named volume (about 4.4 GB when these runs still built
the stealth layer too), and the Cargo registry in another.

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

run cargo test --locked --features "mcp-server test-util" --no-fail-fast --lib --tests
run cargo test --locked --features "mcp-server test-util" --no-fail-fast --doc \
    -- --test-threads=2

# A real-browser binary, one at a time:
run cargo test --locked --features "mcp-server test-util" --test sb_cdp_chrome \
    -- --ignored --test-threads=1

# A test that needs a headed Chrome gets a virtual display:
docker run --rm --memory=4g --memory-swap=4g --shm-size=1g -e SBRS_XVFB=1 \
    -e CARGO_BUILD_JOBS=2 -v "$PWD":/src:ro -v sbrs-verify-target:/target \
    -v sbrs-verify-registry:/usr/local/cargo/registry seleniumbase-rs:verify-tools \
    cargo test --locked --features "mcp-server test-util" --test NAME_chrome \
    -- --ignored --test-threads=1
```

<Note>

The doc tests need `-- --test-threads=2` in 4 GB: with the default of one thread per
CPU, the linker was killed for lack of memory in 66 of them. Give the container more
memory instead if you have it.

</Note>

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

<Warning>

Several checkouts of this repository (for example git worktrees) must not share
one `CARGO_TARGET_DIR`: they overwrite each other's compiled library. Do not `cargo clean` a target directory that other builds are using.

</Warning>

## Build the documentation

<CodeGroup>

```bash Install
cargo install mdbook
```

```bash Build
mdbook build           # writes the book to book/
```

```bash Preview
mdbook serve --open    # live preview while editing
```

</CodeGroup>

The generated `docs/parity.md` is not edited by hand: `just parity-docs`
rebuilds it from `parity/api.toml`.

### The Docs7 site

The same pages also publish as a [Docs7](https://context7.com/docs/docs7/quickstart)
site. `docs/docs.json` is its configuration, and its navigation follows
`docs/SUMMARY.md`; regenerate it after changing the table of contents:

```bash
python3 -I tools/docs7_nav.py
```

Preview it locally (needs Node), then open http://localhost:4317:

```bash
npx --yes @upstash/docs7 dev ./docs --port 4317
```

To publish, open the Docs7 tab of your Context7 teamspace, choose **Add New
Site**, pick this repository, and set the branch to `main` and the folder to
`docs`. Every push to `main` then rebuilds the site.

## Platforms

CI builds and tests on Linux, macOS and Windows (`.github/workflows/ci.yml`);
development and the project's own checks run on macOS (Apple silicon). The suite
was also run on Linux arm64 (Debian 12, Chromium 154) in a container, as above.
Measured browser fingerprint data is macOS only for now. Per-system notes are in
[Operating systems](platforms.md).
