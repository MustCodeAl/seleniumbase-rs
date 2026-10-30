---
title: "Building and installing"
sidebarTitle: "Install"
---

export const LayoutTree = ({ children }) => (
  <div className="not-prose layout-tree" style={{ margin: "1.25rem 0", fontFamily: "var(--font-sans)", fontSize: "14px", color: "var(--text-secondary)" }}>
    <style>{`
      .layout-tree summary { list-style: none; }
      .layout-tree summary::-webkit-details-marker { display: none; }
      .layout-tree .lt-row:hover { background-color: var(--surface-subtle); }
      .layout-tree .lt-fold { color: #d5a910; }
      .dark .layout-tree .lt-fold { color: #ffd452; }
      .layout-tree .lt-fold-closed { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-open { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-closed { display: flex; }
      .layout-tree .lt-hl { color: var(--primary); background-color: color-mix(in srgb, var(--primary) 8%, transparent); }
      .dark .layout-tree .lt-hl { color: var(--primary-light); }
      .layout-tree .lt-ic-gray { color: #84848a; }
      .dark .layout-tree .lt-ic-gray { color: #adadb1; }
      .layout-tree .lt-ic-blue { color: #1a85d4; }
      .dark .layout-tree .lt-ic-blue { color: #69b1ff; }
      .layout-tree .lt-ic-cyan { color: #1ca1c7; }
      .dark .layout-tree .lt-ic-cyan { color: #68cdf2; }
      .layout-tree .lt-ic-green { color: #199f43; }
      .dark .layout-tree .lt-ic-green { color: #5ecc71; }
      .layout-tree .lt-ic-indigo { color: #693acf; }
      .dark .layout-tree .lt-ic-indigo { color: #9d6afb; }
      .layout-tree .lt-ic-orange { color: #d47628; }
      .dark .layout-tree .lt-ic-orange { color: #ffa359; }
      .layout-tree .lt-ic-pink { color: #d32a61; }
      .dark .layout-tree .lt-ic-pink { color: #ff678d; }
      .layout-tree .lt-ic-purple { color: #a631be; }
      .dark .layout-tree .lt-ic-purple { color: #d568ea; }
      .layout-tree .lt-ic-red { color: #d52c36; }
      .dark .layout-tree .lt-ic-red { color: #ff6762; }
      .layout-tree .lt-ic-yellow { color: #d5a910; }
      .dark .layout-tree .lt-ic-yellow { color: #ffd452; }
    `}</style>
    {children}
  </div>
)

export const LayoutRow = ({ icon, name, note, highlight = false, button = false }) => (
  <div className={highlight ? "lt-row lt-hl" : "lt-row"} style={{ display: "flex", alignItems: "flex-start", gap: "6px", minHeight: "30px", padding: "5px 8px", margin: "0 2px", borderRadius: "6px", lineHeight: "20px", cursor: button ? "pointer" : "default" }}>
    <span style={{ width: "16px", height: "20px", flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", color: highlight ? "inherit" : "var(--text-muted)" }}>{icon}</span>
    <span style={{ whiteSpace: "nowrap", fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)", fontSize: "13px", fontWeight: 600, color: highlight ? "inherit" : "var(--text-primary)" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", fontSize: "13px", fontStyle: "italic", marginLeft: "10px", opacity: 0.9 }}>{note}</span> : null}
  </div>
)

export const LayoutFolderIcon = ({ kind }) => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    {kind === "empty" ? (
      <path fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" d="M2 4.2A1.2 1.2 0 0 1 3.2 3h3.1c.32 0 .62.13.85.35L8.4 4.6h4.4A1.2 1.2 0 0 1 14 5.8v6a1.2 1.2 0 0 1-1.2 1.2H3.2A1.2 1.2 0 0 1 2 11.8z" />
    ) : kind === "open" ? (
      <g fill="currentColor">
        <path opacity=".5" d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H13a1.5 1.5 0 0 1 1.5 1.5V7H4.2a1.5 1.5 0 0 0-1.4.95L1.5 11.3z" />
        <path d="M4.2 8h10.6a.7.7 0 0 1 .65.95l-1.6 4.1a1.5 1.5 0 0 1-1.4.95H2.2a.7.7 0 0 1-.65-.95l1.25-3.2A1.5 1.5 0 0 1 4.2 8z" />
      </g>
    ) : (
      <g fill="currentColor">
        <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H1.5z" />
        <path opacity=".6" d="M1.5 4.5V12A1.5 1.5 0 0 0 3 13.5h10a1.5 1.5 0 0 0 1.5-1.5V6A1.5 1.5 0 0 0 13 4.5z" />
      </g>
    )}
  </svg>
)

export const LayoutFileIcon = () => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    <path fill="currentColor" opacity=".5" d="M8 1v3a3 3 0 0 0 3 3h3v5.5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 12.5v-9A2.5 2.5 0 0 1 4.5 1z" />
    <path fill="currentColor" d="M9.5 1a.5.5 0 0 1 .354.146l4 4A.5.5 0 0 1 14 5.5V6h-3a2 2 0 0 1-2-2V1z" />
  </svg>
)

export const LayoutDir = ({ name, note, open = false, highlight = false, children }) =>
  children ? (
    <details open={open}>
      <summary>
        <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><span className="lt-fold-open" style={{ display: "flex" }}><LayoutFolderIcon kind="open" /></span><span className="lt-fold-closed"><LayoutFolderIcon kind="closed" /></span></span>} name={name} note={note} highlight={highlight} button />
      </summary>
      <div style={{ marginLeft: "17.5px", borderLeft: "1px solid var(--border-subtle)", paddingLeft: "6px" }}>
        {children}
      </div>
    </details>
  ) : (
    <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><LayoutFolderIcon kind="empty" /></span>} name={name} note={note} highlight={highlight} />
  )

export const layoutIconColor = (name) => {
  const n = String(name).toLowerCase()
  const ext = n.includes(".") ? n.split(".").pop() : ""
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.startsWith("docker-compose")) return "blue"
  if (["rs", "json", "jsonc", "html", "htm", "zip", "gz", "tgz", "crx", "xpi", "swift", "svg"].includes(ext)) return "orange"
  if (["ts", "tsx", "mts", "cts", "py"].includes(ext)) return "blue"
  if (["js", "mjs", "cjs", "jsx"].includes(ext)) return "yellow"
  if (["md", "mdx", "sh", "bash", "zsh", "fish", "ps1"].includes(ext)) return "green"
  if (["css", "scss"].includes(ext)) return "indigo"
  if (["png", "jpg", "jpeg", "gif", "webp", "ico"].includes(ext)) return "pink"
  if (ext === "go") return "cyan"
  return "gray"
}

export const LayoutFile = ({ name, note, highlight = false }) => (
  <LayoutRow icon={<span className={highlight ? "" : "lt-ic-" + layoutIconColor(name)} style={{ display: "flex" }}><LayoutFileIcon /></span>} name={name} note={note} highlight={highlight} />
)

How to build `seleniumbase-rs` from source, which features cost what, and how to
run it in a container. For writing and running tests with it, start with
[Getting started](/tutorials/getting_started); for working on the crate, see
`CONTRIBUTING.md`.

<Visibility for="agents">

Summary for AI exports: install Rust 1.99 (rustup), a C compiler and Google Chrome, then clone the repository and run `cargo build --bin sbrs` and `sbrs doctor`. Without a checkout, use `brew install mustcodeal/tap/sbrs` or `cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --locked`. The prebuilt release archives do not exist yet, so `cargo binstall` and mise's `github:` backend find nothing to download. The library has no default features; the command-line package `seleniumbase-rs-cli` turns on `pdf` and `mcp-server`.

</Visibility>

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

<Prompt description="Set up this machine to build and test seleniumbase-rs" actions={["copy"]}>

Set up this computer to build and test the Rust library seleniumbase-rs (https://github.com/MustCodeAl/seleniumbase-rs). Follow its docs/building.md ("What you need") for my operating system:

1. Find out which OS and CPU this is. Install what is missing, and only that: Rust through rustup (the repo's rust-toolchain.toml picks 1.99), a C compiler (Xcode command line tools / build-essential / Visual Studio 2022 Build Tools with C++), and Google Chrome. Use Homebrew on macOS, apt or dnf on Linux, winget on Windows. Ask me before anything that needs an administrator password, and don't change system security settings yourself.
2. Clone the repository (or use this folder if it is the repository) and run `cargo build --bin sbrs`, then `sbrs doctor` (`target/debug/sbrs doctor`, or `target\debug\sbrs.exe doctor` on Windows).
3. Run the checks and say how many tests passed and failed: `cargo fmt --all -- --check`, `cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings` and `cargo test --features "mcp-server test-util"`.
4. Only if I ask for a feature, install its extras from the "Extras for single features" part of docs/building.md (s3, gui, the Tauri example).

Don't edit any code. Finish with a short list of what you installed, the versions, and anything that failed with its first error line.

</Prompt>

<Tabs>

<Tab title="macOS" icon="apple" iconType="brands">

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

<Tab title="Debian and Ubuntu" icon="linux" iconType="brands">

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

<Tab title="Fedora" icon="linux" iconType="brands">

```bash
sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl chromium
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

</Tab>

<Tab title="Windows (PowerShell)" icon="windows" iconType="brands">

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

<Check>

The last line prints the browsers it found and whether their drivers match.

</Check>

### Extras for single features

Most features need nothing more: `tui`, `mcp-server`, `turso`, `playwright`,
`azure`, `gcp`, `full-tracing`, `test-util`, `ocr`, `visual`, `plugins` and the
built-in plugins (`plugin-shield`, `plugin-block-resources`, `plugin-crx`,
`plugin-fetcher`, `plugin-extractor`, `plugin-batcher`, `plugin-crawler`, `plugin-browser`, or
`plugins-builtin` for all of them). The rest:

<AccordionGroup>

<Accordion title="s3" icon="cloud" description="OpenSSL on Linux">

`s3` needs OpenSSL on Linux (nothing on macOS and Windows):

```bash
sudo apt-get install -y libssl-dev pkg-config
```

</Accordion>

<Accordion title="gui" icon="window-maximize" description="Desktop input and native dialogs">

`gui` (desktop input and native dialogs). On macOS nothing to install; at
run time allow the program under System Settings ▸ Privacy & Security ▸
Accessibility. On Windows nothing. On Debian and Ubuntu:

```bash
sudo apt-get install -y libgtk-3-dev libx11-dev libxdo-dev libxkbcommon-dev libxtst-dev libssl-dev pkg-config
```

</Accordion>

<Accordion title="The stealth scraping layer" icon="user-secret" description="Moved out in 0.3.0">

The stealth scraping layer (the former `scrape` and `impersonate` features)
moved in 0.3.0 to its own workspace,
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth).
Its BoringSSL build needs CMake and libclang (and NASM and LLVM on Windows);
those instructions live in that repository now. This crate needs none of them.

</Accordion>

<Accordion title="The Tauri example" icon="desktop" description="Node LTS">

The Tauri example (`examples/tauri-profile-manager`) needs Node LTS (`brew
install node`, `winget install OpenJS.NodeJS.LTS`). On Windows it also uses
WebView2, which comes with Windows 10 and 11. On Debian and Ubuntu:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev libssl-dev libx11-dev libxdo-dev
```

</Accordion>

<Accordion title="Repository tools" icon="wrench" description="mdBook, Python, just, cargo-deny">

mdBook for the docs (`brew install mdbook` or `cargo
install mdbook`), Python 3.10+ for the parity tables in `tools/parity`, and
`just` and `cargo-deny` (`brew install just cargo-deny` or `cargo install just
cargo-deny`). With [mise](https://mise.jdx.dev), `mise install` sets up Rust,
Python, Node, mdBook, just and cargo-deny in one step.

</Accordion>

</AccordionGroup>

### What has been built where

| System | Built and run |
|---|---|
| macOS (Apple Silicon) | Everything |
| Debian 12 arm64, in Docker | The default build and the test features |
| Windows 11 x64 | The default build, the test features, the real-browser tests and the Tauri example |

The Linux `gui` and Tauri package lists come from the CI configuration, which
has not been able to run (see [Operating systems](/building#operating-systems)).

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
Windows) holds these at its root:

<LayoutTree>
  <LayoutFile name="sbrs" highlight note="the CLI" />
  <LayoutFile name="sbrs-mcp" highlight note="the MCP server" />
  <LayoutDir name="completions" note="bash, zsh, fish, PowerShell, a Fig spec and the usage spec" />
  <LayoutDir name="warp" open>
    <LayoutDir name="workflows" note="the Warp workflows" />
  </LayoutDir>
  <LayoutFile name="README.md" />
  <LayoutFile name="LICENSE" />
</LayoutTree>

`SHA256SUMS` lists the checksum of every archive.

### cargo binstall

<Badge color="orange">No archives yet</Badge>

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
completions <shell>` (see [the CLI page](/tutorials/cli_usage)).

### mise

[mise](https://mise.jdx.dev) has two backends that fit.

<Tabs>

<Tab title="From release archives" icon="download">

The `github:` backend picks the archive for your system and puts both binaries on `PATH`:

<CodeGroup>

```bash Terminal
mise use -g github:MustCodeAl/seleniumbase-rs
```

```toml mise.toml
[tools."github:MustCodeAl/seleniumbase-rs"]
version = "latest"
```

</CodeGroup>

Until the release archives exist, mise reports "No versions found" for it.

</Tab>

<Tab title="Build from the git tag" icon="hammer">

To build from the git tag now, the `cargo:` backend runs `cargo install --git`
(Rust 1.99 must be on `PATH`, for example with `mise use -g rust@1.99`). The
repository has one package with binaries, `seleniumbase-rs-cli`, so cargo finds
it by itself, and it builds `sbrs-mcp` too (`features = ["tui"]` adds the
terminal UI):

<CodeGroup>

```bash Terminal
mise use -g 'cargo:https://github.com/MustCodeAl/seleniumbase-rs@tag:v0.2.0'
```

```toml mise.toml
[tools."cargo:https://github.com/MustCodeAl/seleniumbase-rs"]
version = "tag:v0.2.0"
```

</CodeGroup>

The binaries moved from the package `seleniumbase-rs` to `seleniumbase-rs-cli`
after v0.4.0. For a tag up to v0.4.0, add `features = ["mcp-server"]` to get
`sbrs-mcp`.

</Tab>

</Tabs>

mise installs no completions for these backends either; its per-tool
`postinstall` option can run `sbrs completions zsh` into your completions folder.

### Homebrew

<Badge color="green">Works today</Badge>

The tap builds from the release's source and installs the completions and the
Warp workflows as well (macOS and Linux):

```bash
brew install mustcodeal/tap/sbrs
```

### cargo install from git

<Badge color="green">Works today</Badge>

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
| `ocr` | `ocr::Ocr`, text read from screenshots and PNG images (`ocrs`, `rten`, `image`) | pure Rust; downloads two models (about 12 MB) on first use, see [Reading text from the screen](/tutorials/remaining_helpers#reading-text-from-the-screen-ocr) |
| `visual` | `visual::VisualBaseline` and `BaseCase::assert_visual_match`: screenshots compared with a baseline by structural similarity, with a diff image on a mismatch (`image-compare`, `image` with PNG only) | pure Rust; brings `rayon`; see [Visual regression testing](/tutorials/remaining_helpers#visual-regression-testing-pixels) |

Combine them with `--features "mcp-server tui"`. The features `scrape` and
`impersonate` were removed in 0.3.0; see [Stealth Scraping](/tutorials/stealth_scraping).

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
[Verified on Linux](/building#verified-on-linux). Remove everything when you are
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

- Use [mr-boxington](https://mr-boxington.jdx.dev) (`mbx`), a shared cache for
  Cargo builds: `cargo install mbx --locked && mbx setup` (or, with mise,
  `mise use --global --tool-option mr_boxington=true rust mr-boxington`), then
  keep running `cargo`. Each checkout and worktree gets its own managed
  `target`, compiled work is reused between them, and the cache prunes itself
  to a size budget (`mbx stats`, `mbx gc --dry-run`). Leave `RUSTC_WRAPPER`
  unset (it replaces sccache) and do not set `CARGO_TARGET_DIR`.
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
site. `docs/docs.json` is its configuration and its navigation (tabs and groups);
`docs/SUMMARY.md` is mdBook's table of contents. Add a new page to both.

<Steps>

<Step title="Check the pages" icon="circle-check">

Needs Bun. Builds every page as Docs7 does (MDX), and fails on a name an
expression uses that is not defined, a relative `.md` link (Docs7 serves those
raw; link to the site route, `/tutorials/selectors`), a page without a title,
and a page that is in no menu of `docs/docs.json`:

```bash
just mdx
```

</Step>

<Step title="Preview it locally" icon="eye">

Needs Node. Run it, then open http://localhost:4317:

```bash
npx --yes @upstash/docs7 dev ./docs --port 4317
```

</Step>

<Step title="Publish" icon="upload">

Open the Docs7 tab of your Context7 teamspace, choose **Add New
Site**, pick this repository, and set the branch to `main` and the folder to
`docs`. Every push to `main` then rebuilds the site.

</Step>

</Steps>

## Operating systems

What differs between macOS, Linux and Windows when you build and run
`seleniumbase-rs`. The CI workflow (`.github/workflows/ci.yml`) is set up to
build and test on all three, but has not been able to run yet; day-to-day
development and the project's own checks run on macOS (Apple silicon), so macOS
is the best-trodden path, and Windows 11 was checked by hand (see
[Windows](#windows)). The test suite was also run
on Linux (arm64, Debian 12, Chromium 154) in a container; what passed, and what
was not run, is under [Verified on Linux](#verified-on-linux).

<Visibility for="agents">

Summary for AI exports: macOS is the best-trodden path, Linux was verified in a container (arm64, Debian 12, Chromium 154) and Windows 11 was checked by hand. Name a browser with `SB_CHROME_BIN` or `CHROME_BIN`. As root or in a container the pure CDP engine needs `SB_NO_SANDBOX` (the WebDriver engine always passes `--no-sandbox` on Linux). `sbrs doctor` checks Chrome and chromedriver; `sbrs install` fetches the matching chromedriver.

</Visibility>

The packages each system and each feature needs, with the commands that install
them (Homebrew, apt, dnf, winget), are in
[Building and installing](#what-you-need).

### Which Chrome it finds

The library looks in the usual install locations, and you can always name one:

<Tabs>

<Tab title="macOS" icon="apple">

- **Looked at:** `/Applications/Google Chrome.app`, `~/Applications/Google Chrome.app`, Chromium, Chrome Canary
- **Override:** `SB_CHROME_BIN` or `CHROME_BIN`

</Tab>

<Tab title="Linux" icon="linux">

- **Looked at:** `google-chrome`, `google-chrome-stable`, `chromium` and `chromium-browser` in `/usr/bin` (and `/snap/bin/chromium`), then the same names on the `PATH`
- **Override:** `SB_CHROME_BIN`, `CHROME_BIN` or `CHROMIUM_BIN`

</Tab>

<Tab title="Windows" icon="windows">

- **Looked at:** `Program Files`, `Program Files (x86)` and the per-user `Google\Chrome\Application\chrome.exe`
- **Override:** `SB_CHROME_BIN` or `CHROME_BIN`

</Tab>

</Tabs>

For the WebDriver engine, `sbrs install` downloads the chromedriver that matches
your Chrome's major version, and `CHROMEDRIVER_PATH` points at one you already
have. The pure CDP engine (`sb_cdp`) needs no chromedriver on any system.

The WebDriver engine looks for `downloaded_drivers/chromedriver` in the working
directory first and uses it when its major version fits the browser; otherwise it
downloads the matching [Chrome for Testing](https://googlechromelabs.github.io/chrome-for-testing/)
driver. That download is `linux64` on x86-64 Linux and `linux-arm64` on arm64
Linux, which Chrome for Testing publishes from Chrome 153 on; for an older Chrome
on arm64, or any other Linux architecture, the error says that no build is
published and where to put a driver. A driver from your distribution works the
same way: link it into that folder, as the repository's `Dockerfile` does, and no
network is needed:

```bash Terminal
mkdir -p downloaded_drivers && ln -s /usr/bin/chromedriver downloaded_drivers/chromedriver
```

#### Firefox and Edge

A Firefox session starts **geckodriver** and an Edge session **msedgedriver**,
as Python SeleniumBase does. Each is looked for in `downloaded_drivers/` (right
in it, or in the newest `geckodriver-v<version>/` or
`msedgedriver-<version>/` folder there) and then on the `PATH`, so a Homebrew or
distribution geckodriver is used as it is. When none is found:

| Driver | Downloaded from | Which build |
|---|---|---|
| geckodriver | `github.com/mozilla/geckodriver/releases` | The newest release; one geckodriver drives a range of Firefox versions, so it is not matched to Firefox. `sbrs get geckodriver 0.36.0` pins one. |
| msedgedriver | `msedgedriver.microsoft.com` | The newest build for the installed Edge's major version (`LATEST_RELEASE_<major>_<OS>`), or the stable one when Edge cannot be found. A driver on disk or on the `PATH` for another Edge major version is replaced; `with_driver_version` pins one. |

The archives (tar.gz for geckodriver on Linux and macOS, zip otherwise) are
capped at 64 MB, unpacked without writing outside their folder, and made
runnable the way chromedriver is: the execute bit, and on macOS no quarantine
flag and, if the system kills the driver as it starts, a new ad-hoc signature.
Edge is looked for at `/Applications/Microsoft Edge.app` on macOS,
`Microsoft\Edge\Application\msedge.exe` under `Program Files`,
`Program Files (x86)` and the per-user folder on Windows, and
`microsoft-edge`, `microsoft-edge-stable` or `/opt/microsoft/msedge/msedge` on
Linux. Microsoft publishes no msedgedriver for arm64 Linux.

The session's options go in `moz:firefoxOptions` (headless, arguments,
preferences, the user agent and locale as preferences, `browser_binary_path` as
the Firefox program and `user_data_dir` as `-profile`) and `ms:edgeOptions` (the
same options Chrome gets). Chrome-only settings such as extensions, mobile
emulation and the CDP and UC modes have no Firefox
equivalent and are not sent; the session proxy is not passed to Firefox either.

### macOS

<Badge color="green">Best-trodden path</Badge>

- **A freshly written or replaced executable is slow the first time.** macOS
  scans it on first run, which can take seconds. The library writes a replaced
  chromedriver as a new file because macOS kills a signed binary that is
  overwritten in place; if a driver is "killed" or refused, run `sbrs install`
  again and see [Common problems](/help/common_problems).
- **Gatekeeper and quarantine.** A browser or driver downloaded by another tool
  can carry a quarantine flag; clear it with
  `xattr -d com.apple.quarantine <file>` if macOS refuses to open it.
- **Tests that fake `java` or other scripts** must run the script once before
  timing it, for the same first-run reason.
- **Memory.** On an 8 GB machine keep `CARGO_BUILD_JOBS` low and run one cargo
  command at a time.

### Linux

<Badge color="blue">Run in a container: arm64, Debian 12</Badge>

- **Install a browser from your distribution** (`chromium` and, for the
  WebDriver engine, `chromium-driver` on Debian and Ubuntu) or use Google
  Chrome's own package. The repository's `Dockerfile` shows the Debian setup.
- **Running as root or in a container** usually needs `--no-sandbox`. The
  WebDriver engine always passes it on Linux. For the pure CDP engine set
  `SB_NO_SANDBOX` (it is read as set or not set: `SB_NO_SANDBOX=0` also turns the
  sandbox off), or call `LaunchOptionsBuilder::no_sandbox(true)`. The real-browser
  tests of this repository read `CI` instead and pass `--no-sandbox` when it is
  set.

  <Warning>

  `--no-sandbox` weakens Chrome's isolation, so use it only in a container you control.

  </Warning>

- **No display.** Run headless (`SB_HEADLESS=true`, which `sbrs open` and the other
  browser commands read, or `--headless` before the subcommand; see
  [Headless and headed browsers](/tutorials/headless)). To
  see a window on a server, or to run the few tests that need a headed Chrome (the
  measurement tools), run under a virtual display such as Xvfb. Start `Xvfb` in the
  background and export `DISPLAY`; `xvfb-run` can wait forever for a signal that a
  background process ignores. The `gui` feature (native dialogs and GUI
  automation) links GTK and X11 and is off by default; the `tui` feature has no
  such requirement.
- **Shared memory.** In Docker, Chromium can crash under load with the default
  `/dev/shm`; start the container with `--shm-size=1g` (general Chromium advice).
- **Wayland.** The `gui` feature's input automation targets X11; the project has
  not tested it under Wayland.

#### Verified on Linux

Run on 2026-10-11 to 12 with `docker/Dockerfile.verify` and `docker/verify-linux.sh`
(commands in [Building and installing](#run-the-tests-in-a-container)).
The host was an Apple-silicon Mac, so the containers were
**linux/arm64** on **Debian 12 (bookworm)** with **Rust 1.99** and **Chromium
154.0.8037.92** from Debian's `chromium` and `chromium-driver` packages. Cargo ran
with `CARGO_BUILD_JOBS=2` in a container limited to 4 GB.

<Info>

These runs predate 0.3.0: the feature set still had `scrape`, so the counts below
include the tests of the stealth layer, which has since moved to
[seleniumbase-rs-stealth](/stealth/overview)
(its `impersonate` runs, the measured-persona checks and a flaky JA3 test fixed
then are no longer part of this crate).

</Info>

| Run | Result |
|---|---|
| Library tests, `--features "mcp-server test-util scrape"` | 2112 passed, 0 failed, 16 ignored |
| The other test binaries of that feature set (68) | 406 passed, 0 failed, 114 ignored (the real-browser tests, ignored by default) |
| Doc tests, same features | 182 passed, 0 failed, 10 ignored |
| Real Chromium 154: `browser_smoke` and 17 `*_chrome` binaries, one at a time with `-- --ignored --test-threads=1` | 112 passed, 0 failed (headless, except the 2 of `scrape_capture_chrome`, now in seleniumbase-rs-stealth, which ran headed on a virtual display) |
| `cargo test --test dockerfile_skeleton` | 9 passed |

What the runs showed:

- **A real bug, fixed:** the chromedriver download used the x86-64 `linux64` build on
  every Linux machine, so an arm64 container failed with "chromedriver exited as it
  started". It now asks for `linux-arm64`: `sbrs install` in the arm64 image
  fetches an aarch64 executable, and `browser_smoke` (7 tests) starts Chromium
  through a driver it downloaded that way.
- **Container limits, not bugs:** the doc tests need `-- --test-threads=2` in 4 GB
  (with eight threads the linker was killed for lack of memory in 66 of them); a
  headed browser needs a display, which Xvfb gives; Chromium needs `--no-sandbox`
  as root.

<Note>

Not run: x86-64 Linux, Google Chrome (only Debian's Chromium), other Linux
distributions, the `playwright`, `gui`, `tui`, `s3`, `azure`, `turso` and `gcp`
features, `stealth_audit_network` (it needs outside sites), `playwright_chrome`, and
the macOS-only `driver_repair_macos`. Whether the pure CDP engine starts as the
non-root `sbrs` user of the image with the sandbox on was not tried.

</Note>

### Windows

<Badge color="blue">Checked by hand: Windows 11</Badge>

**Status:** on Windows 11 x64 (Rust 1.99, Chrome 154 and 155, Edge 155, Firefox
153) formatting, clippy, the whole test suite and the real-browser tests pass,
and the Tauri example builds its `.msi` and setup `.exe`. (The stealth layer,
then the `impersonate` feature, built and passed there too; it now lives in
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
with its CMake, NASM and LLVM requirements.) Install steps:
[Building and installing](#what-you-need).

**What works differently**

- The Chrome and Edge version comes from the program file's own version
  (`chrome.exe --version` prints nothing on Windows). While an update waits for a
  restart, that is still the version that runs, so the right driver is fetched.
- Download folders are given to Chrome as plain `C:\...` paths.
- Build in a short folder (`C:\src\seleniumbase-rs`) if a deep `target\` path
  hits the 260-character limit. In Rust code, use raw strings for Windows paths.

<Warning>

**Not there yet on Windows**

- If the program that started a browser is killed, the browser can keep
  running (macOS and Linux end it; Windows needs a job object). A driver whose
  session fails, or that is dropped, is ended.
- The tests' leftover-process checks list processes with `sysinfo` now, so
  they run on Windows too, but they have not been run on a Windows PC yet.
- Measured fingerprints exist for Chrome 155 on macOS only; the TLS and HTTP/2
  tables refuse other systems with `NotMeasured`.

</Warning>

### WSL and remote machines

Run the Linux steps inside WSL. A browser inside WSL has no display unless you
use WSLg or run headless. To drive a browser on another machine, run
`sbrs-mcp` or your tests there, or point the WebDriver engine at a
Selenium Grid (`sbrs grid`) with `BrowserConfig::webdriver_url`.

### Check the setup on any system

```bash Terminal
sbrs doctor       # Chrome and chromedriver versions, and whether they match
sbrs install      # fetch the matching chromedriver
```

From an AI agent, the MCP tool `diagnose_environment` reports the same plus the
platform, the redacted `SB_*` settings and the output directory.

<Columns cols={2}>

<Card color="#1a85d4" title="Working with seleniumbase-rs" icon="rocket" href="/tutorials/getting_started" horizontal>

Write and run your first browser test.

</Card>

<Card color="#d47628" title="Command line" icon="terminal" href="/tutorials/cli_usage" horizontal>

One-off commands with `sbrs`, no project needed.

</Card>

<Card color="#a631be" title="Docker Guide" icon="box" href="/help/docker" horizontal>

Containerized configuration.

</Card>

</Columns>
