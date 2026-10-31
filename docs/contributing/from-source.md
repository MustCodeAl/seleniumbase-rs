---
title: "Build from source"
description: "Clone seleniumbase-rs, set up Rust 1.99 and a C compiler, build the library and the sbrs binaries, and run the checks every change must pass."
sidebarTitle: "Build from source"
---

{/* layout-tree-component:start */}

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

{/* layout-tree-component:end */}

This page takes you from a fresh machine to a checkout that builds and passes
the checks. To install `sbrs` without a checkout, or for every system package a
feature needs, see [Building and installing](/building).

<Info>

**What you will learn**

- What to install first on macOS, Linux and Windows.
- How the workspace is laid out.
- How to build the library, the command line and their features.
- How to keep builds small on a machine with little memory.
- How to run the checks, including the tests that need a real Chrome.
- How to write the generated files again, and how to build the Docker image.

</Info>

<Visibility for="agents">

Summary for AI exports: clone https://github.com/MustCodeAl/seleniumbase-rs. You need Rust 1.99 (`rust-toolchain.toml` pins it, with clippy and rustfmt) and a C compiler. Set `CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`. The checks are `cargo fmt --all -- --check`, `cargo clippy --all-targets --features "mcp-server test-util testing bdd" -- -D warnings` and `cargo test --features "mcp-server test-util testing bdd"`. Tests that need a real Chrome are the `#[ignore]`d `*_chrome.rs` files plus `browser_smoke`: run them one binary at a time with `-- --ignored`. The library has no default features; the command line `crates/cli` turns on `pdf`, `mcp-server`, `mcp-client` and `plugins-builtin`.

</Visibility>

**At a glance**

- Rust 1.99, from `rust-toolchain.toml`
- Library at the root, no default features
- `sbrs` and `sbrs-mcp` in `crates/cli`
- Checks: fmt, clippy, test, deny, mdbook
- Real-Chrome tests: `*_chrome.rs`, one at a time

## What you need

- **Rust 1.99.** The workspace sets `rust-version = "1.99"` in `Cargo.toml`,
  and `rust-toolchain.toml` pins the `1.99` channel with `clippy` and
  `rustfmt`, so rustup uses that toolchain inside the checkout.
- **A C compiler and linker.** Some dependencies, such as `ring`, compile C
  code.
- **Git**, to clone the repository.
- **Google Chrome or Chromium**, only to run `sbrs` and the tests that drive a
  real browser. Building needs no browser.

Nothing here needs Python. Only the parity scripts in `tools/` do (Python 3.10
or newer).

<Tabs>

<Tab title="macOS" icon="apple">

```bash Terminal
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
brew install --cask google-chrome
```

</Tab>

<Tab title="Linux" icon="linux">

Debian and Ubuntu:

```bash Terminal
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
sudo apt-get install -y chromium
```

Fedora:

```bash Terminal
sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl git chromium
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

Google Chrome's own package works too; see [Building and installing](/building#what-you-need).

</Tab>

<Tab title="Windows" icon="windows">

```powershell PowerShell
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Git.Git
winget install Google.Chrome
```

Open a new terminal afterwards, so the new programs are on the `PATH`. Clone
into a short folder such as `C:\src\seleniumbase-rs`: a deep `target\` path can
hit the 260-character limit.

</Tab>

</Tabs>

### Extra packages for some features

Most features need nothing more. These do:

| Feature | Debian and Ubuntu | macOS and Windows |
|---|---|---|
| `s3` | `libssl-dev pkg-config` | nothing |
| `gui` | `libgtk-3-dev libx11-dev libxdo-dev libxkbcommon-dev libxtst-dev libssl-dev pkg-config` | nothing to build; on macOS allow the program under Accessibility at run time |
| The Tauri example | `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev libssl-dev libx11-dev libxdo-dev`, and Node LTS | Node LTS (Windows also uses WebView2, which comes with Windows 10 and 11) |

### Tools for the checks

| Tool | Used for | Install |
|---|---|---|
| `mdbook` | `mdbook build` | `cargo install mdbook` |
| `cargo-deny`, `cargo-audit` | the dependency checks | `cargo install cargo-deny cargo-audit` |
| `cargo-nextest` | faster test runs and the one-thread `chrome` test group | `cargo install cargo-nextest` |
| `just` | the recipes in the `justfile` | `cargo install just` |

<Tip>

With [mise](https://mise.jdx.dev), `mise install` sets up Rust 1.99, Python,
Node, mdBook, just and cargo-deny from `mise.toml`, and `mise run gate` runs the
checks a change must pass.

</Tip>

## Get the code

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs" />

<Steps>

<Step title="Clone" icon="code-branch">

```bash Terminal
git clone https://github.com/MustCodeAl/seleniumbase-rs
cd seleniumbase-rs
```

</Step>

<Step title="Check the toolchain" icon="wrench">

```bash Terminal
rustc --version
```

Inside the checkout it should print version 1.99, which `rust-toolchain.toml`
selects. If rustup says that toolchain is not installed, run
`rustup toolchain install 1.99`.

</Step>

<Step title="Build and check the setup" icon="stethoscope">

<CodeGroup>

```bash macOS and Linux
cargo build --bin sbrs
./target/debug/sbrs doctor
```

```powershell Windows
cargo build --bin sbrs
.\target\debug\sbrs.exe doctor
```

</CodeGroup>

`doctor` prints the `SB_*` settings, the browser it found, and the chromedriver
it found with its major version next to Chrome's.

</Step>

</Steps>

## Workspace layout

The repository is a Cargo workspace of two packages. `default-members` names
both, so a plain `cargo build` or `cargo test` at the root covers both.

{/* layout-tree:start */}

<LayoutTree>
<LayoutDir name="seleniumbase-rs" open note="the workspace root">
<LayoutFile name="Cargo.toml" highlight note="the workspace and the library package: version, rust-version 1.99, features" />
<LayoutFile name="Cargo.lock" note="the locked dependency versions" />
<LayoutFile name="rust-toolchain.toml" note="pins Rust 1.99 with clippy and rustfmt" />
<LayoutDir name="src" open note="the library (no binary): both engines, the MCP tools, the library half of the command line">
<LayoutFile name="lib.rs" highlight note="the crate root and its public re-exports" />
<LayoutFile name="error.rs" highlight note="SeleniumBaseError: hints, exit codes, selector advice" />
<LayoutDir name="api" note="BaseCase (base_case.rs, base_case_impls/), the capability traits, recorder, scenario, charts, tours" />
<LayoutDir name="browser" note="BrowserConfig, the launcher, the WebDriver session, driver access and downloads" />
<LayoutDir name="sb_cdp" note="the pure CDP engine: Browser, Page, Locator, pool, mock" />
<LayoutDir name="evasion" note="fingerprints, evasion providers, humanize, binary patching and the CDP reactor" />
<LayoutDir name="mcp" note="the MCP servers cdp, driver, sb and dev, and their tools" />
<LayoutDir name="cli" note="the library half of sbrs, with no parser: commands.rs, scaffold.rs, scripts/" />
<LayoutDir name="testing" note="the rstest sb fixtures; bdd/: Gherkin through rstest-bdd" />
<LayoutDir name="plugins" note="reporting and logging plugins" />
<LayoutDir name="plugin_host" note="resolves plugin names to a PluginSet (feature plugins); the plugins are in the seleniumbase-rs-plugins repository" />
<LayoutDir name="config" note="settings, runtime config, proxy and ad-block lists" />
<LayoutDir name="core" note="test result records and the Allure, JUnit and dashboard writers" />
<LayoutDir name="common" note="graceful shutdown, encryption, obfuscation, test ids, the debug prompt" />
<LayoutDir name="storage" note="ResultStore and ProfileVault (feature turso)" />
<LayoutDir name="utilities" note="RetryPolicy, the grid server, Selenium Grid and IDE, the Python importer" />
<LayoutDir name="utils" note="selectors, shadow DOM, URLs, geometry, bundled extensions, translations" />
<LayoutDir name="profile_payloads" note="parses external browser-profile JSON into a Fingerprint" />
<LayoutDir name="resources" note="files the crate manages on disk, such as downloaded browser drivers" />
<LayoutDir name="js_code" note="building blocks for the JavaScript the crate sends to a page" />
<LayoutFile name="artifacts.rs" note="where the files of a test run go (latest_logs) and how they are named" />
<LayoutFile name="drivers.rs" note="the WebDriver types behind BaseCase, under this crate's name" />
<LayoutFile name="json.rs" note="JSON values and json!, under this crate's name" />
<LayoutFile name="macros.rs" note="convenience macros for tests, such as sb_test!" />
<LayoutFile name="ocr.rs" note="reading text from screenshots (feature ocr)" />
<LayoutFile name="tracing_util.rs" note="tracing initialization helpers" />
<LayoutFile name="visual.rs" note="feature visual: VisualBaseline, screenshot baselines compared by SSIM" />
</LayoutDir>
<LayoutDir name="crates" open note="the workspace members besides the library">
<LayoutDir name="cli" open note="the package seleniumbase-rs-cli: the sbrs and sbrs-mcp binaries">
<LayoutFile name="Cargo.toml" note="default features: pdf, mcp-server, mcp-client, plugins-builtin" />
<LayoutDir name="src" open>
<LayoutFile name="sbrs.rs" highlight note="the sbrs command line" />
<LayoutFile name="mcp_server.rs" highlight note="the sbrs-mcp server" />
<LayoutFile name="fig.rs" note="the Fig completion spec" />
<LayoutFile name="report.rs" note="sbrs report" />
<LayoutFile name="reference.rs" note="renders docs/help/cli_reference.md from the usage spec" />
<LayoutFile name="strict.rs" note="refuses, with status 2, what the parser is too loose about" />
<LayoutDir name="mcp_install" note="sbrs-mcp install, uninstall and clients" />
</LayoutDir>
<LayoutDir name="tests" note="every test that runs the real sbrs or sbrs-mcp binary" />
</LayoutDir>
<LayoutDir name="own-imports" note="tests that import only seleniumbase_rs: the proof that its re-exports work" />
</LayoutDir>
<LayoutDir name="tests" note="integration tests of the library">
<LayoutDir name="golden" note="compiled into the library" />
<LayoutDir name="support" note="shared with crates/cli/tests" />
<LayoutFile name="sb_cdp_mock.rs" note="the pure CDP API against a mocked browser" />
<LayoutFile name="sb_cdp_chrome.rs" note="the same against a real Chrome, ignored by default" />
<LayoutFile name="browser_smoke.rs" note="the WebDriver path against a real Chrome, ignored by default" />
</LayoutDir>
<LayoutDir name="benches" note="the benchmarks">
<LayoutFile name="cdp_latency.rs" note="the pure CDP engine against a real Chrome" />
<LayoutFile name="cpu.rs" note="the library's own work, no browser" />
</LayoutDir>
<LayoutDir name="examples" note="runnable examples">
<LayoutDir name="tauri-profile-manager" note="has its own manifest and is not part of the workspace" />
</LayoutDir>
<LayoutDir name="docs" note="the pages of this site">
<LayoutFile name="docs.json" note="the Docs7 navigation and settings" />
<LayoutFile name="SUMMARY.md" note="the mdBook table of contents" />
<LayoutDir name="tutorials" note="step-by-step guides" />
<LayoutDir name="help" note="help pages" />
<LayoutDir name="design" note="design notes" />
<LayoutDir name="adr" note="architecture decision records" />
</LayoutDir>
<LayoutDir name="parity" note="the Python parity manifests">
<LayoutFile name="api.toml" note="the status of every Python SeleniumBase API" />
<LayoutFile name="upstream.toml" note="the upstream version this crate tracks" />
</LayoutDir>
<LayoutDir name="completions" note="the usage spec of sbrs">
<LayoutFile name="sbrs.usage.kdl" note="generated from crates/cli/src/sbrs.rs" />
</LayoutDir>
<LayoutDir name="schemas" note="JSON Schemas">
<LayoutFile name="fingerprint.schema.json" note="the schema of a Fingerprint file" />
</LayoutDir>
<LayoutDir name="contrib" note="files that are not part of the crate">
<LayoutDir name="tldr" note="draft tldr-pages entries for sbrs, kept to propose upstream" />
</LayoutDir>
<LayoutDir name="scripts" note="helper scripts">
<LayoutFile name="check-consumers.sh" note="just contracts: checks what the repositories that build on this crate use" />
</LayoutDir>
<LayoutDir name="tools" note="scripts for the docs and the parity manifests">
<LayoutDir name="mdx-check" note="just mdx: builds every docs page as Docs7 does" />
<LayoutDir name="parity" note="extracts, seeds and renders the parity manifest (docs/parity.md)" />
<LayoutDir name="layout_tree" note="the tree component that docs_layout_tree.py copies onto the pages" />
<LayoutFile name="docs_layout_tree.py" note="just layout-tree: writes the layout trees of the docs pages" />
<LayoutFile name="docs7_changelog.py" note="writes docs/changelog.md from CHANGELOG.md" />
</LayoutDir>
<LayoutDir name="docker" note="the Linux verification image (Dockerfile.verify, verify-linux.sh) and firefox-check/" />
<LayoutDir name=".config" note="configuration of tools">
<LayoutFile name="nextest.toml" note="the nextest configuration and its chrome test group" />
</LayoutDir>
<LayoutDir name=".github" note="GitHub Actions">
<LayoutDir name="workflows" note="ci, docker, completions, release and upstream-watch" />
</LayoutDir>
<LayoutDir name=".warp" note="the Warp terminal">
<LayoutDir name="workflows" note="the Warp workflows for sbrs" />
</LayoutDir>
<LayoutFile name="Dockerfile" note="the sbrs image; its skeleton step lists every target" />
<LayoutFile name="Makefile" note="make targets" />
<LayoutFile name="justfile" note="just recipes" />
<LayoutFile name="mise.toml" note="tools and tasks for mise" />
<LayoutFile name="deny.toml" note="the cargo-deny policy" />
<LayoutFile name="clippy.toml" note="clippy settings" />
<LayoutFile name="rust-analyzer.toml" note="settings for rust-analyzer" />
<LayoutFile name="book.toml" note="the mdBook configuration" />
<LayoutFile name="README.md" />
<LayoutFile name="CHANGELOG.md" note="what changed in each release" />
<LayoutFile name="ROADMAP.md" note="what is done and what is missing" />
<LayoutFile name="CLAUDE.md" note="the module map for coding assistants" />
</LayoutDir>
</LayoutTree>

{/* layout-tree:end */}

The module map of `src/` is in `CLAUDE.md` and in the
[Developer guide](/DEVELOPER_GUIDE#repository-layout).

## Build

<CodeGroup>

```bash Both packages
cargo build
```

```bash Library only
cargo build -p seleniumbase-rs
```

```bash Command line only
cargo build -p seleniumbase-rs-cli
```

```bash Release binaries
cargo build --release --locked --bin sbrs
cargo build --release --locked --bin sbrs-mcp
```

</CodeGroup>

### Features

| Package | On by default | Notes |
|---|---|---|
| `seleniumbase-rs` (the library) | nothing | Every feature is opt-in: `gui`, `tui`, `turso`, `playwright`, `plugins`, `ocr`, `visual`, `pdf`, `mcp-server`, `s3`, `azure`, `gcp`, `test-util`, `full-tracing`, `tokio-console` and more. |
| `seleniumbase-rs-cli` (`sbrs`, `sbrs-mcp`) | `pdf`, `mcp-server`, `mcp-client`, `plugins-builtin` | Forwards `tui`, `gui` and `turso` to the library, and also `plugins`, the `plugin-*` features, `test-util`, `full-tracing` and `tokio-console`. |

<Note>

A build of both packages unifies their features, so the command line's `pdf`
and `mcp-server` reach the library too. Use `-p seleniumbase-rs` to build and
test the library with its own defaults, as a project that depends on it would.
The `library` job in `.github/workflows/ci.yml` does exactly that.

</Note>

```bash Terminal
cargo build --features "mcp-server tui"                     # combine features
cargo run --example gui_automation --features gui
cargo build -p seleniumbase-rs-cli --no-default-features    # sbrs alone, without pdf, mcp-server or plugins
```

`sbrs-mcp` needs the `mcp-server` feature, so a build without default features
has `sbrs` only. Every feature and what it costs is in
[Building and installing](/building#features).

### Install the binaries

```bash Terminal
cargo install --path crates/cli                  # sbrs and sbrs-mcp
cargo install --path crates/cli --features tui   # and the terminal UI
```

## Keep builds small

A cold build compiles a large dependency tree. Three environment variables keep
it within a small machine's memory:

<Tabs>

<Tab title="macOS and Linux" icon="terminal">

```bash Terminal
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
```

</Tab>

<Tab title="Windows" icon="windows">

```powershell PowerShell
$env:CARGO_BUILD_JOBS = "2"; $env:CARGO_INCREMENTAL = "0"; $env:CARGO_PROFILE_DEV_DEBUG = "0"
```

</Tab>

</Tabs>

<ParamField path="CARGO_BUILD_JOBS" type="number" default="every core">

How many compiler processes run at once. `2` keeps memory low.

</ParamField>

<ParamField path="CARGO_INCREMENTAL" type="0 or 1">

`0` turns off the incremental cache in `target/`, which keeps it smaller.
mr-boxington (below) manages incremental state itself.

</ParamField>

<ParamField path="CARGO_PROFILE_DEV_DEBUG" type="number">

`0` leaves debug information out of development builds: smaller files and
faster links.

</ParamField>

Use the same values for every cargo command, so builds share their compiled
artifacts instead of compiling them again.

<Warning>

Run one heavy cargo command at a time: cargo serialises builds in one target
directory anyway. Do not share one `CARGO_TARGET_DIR` between several checkouts
or worktrees of this repository: they overwrite each other's compiled library,
and doctests then fail with "extern location does not exist". A development
`target/` with the test, bench and check artifacts grows past 10 GB.

</Warning>

<Tip>

Use [mr-boxington](https://mr-boxington.jdx.dev) (`mbx`), a shared cache for
Cargo builds: `cargo install mbx --locked && mbx setup`, or with mise
`mise use --global --tool-option mr_boxington=true rust mr-boxington`. Keep
running `cargo`: each checkout and worktree gets its own managed `target`, so
worktrees no longer collide, compiled work is reused between them, and the
cache prunes itself. It caches nothing while `RUSTC_WRAPPER` is set, and it
leaves a `CARGO_TARGET_DIR` you set alone. `mbx doctor` checks the setup.

</Tip>

## Run the checks

Every change must pass these, the same ones CI runs.

<Steps>

<Step title="Format" icon="align-left">

```bash Terminal
cargo fmt --all -- --check
```

</Step>

<Step title="Lint" icon="magnifying-glass">

```bash Terminal
cargo clippy --all-targets --features "mcp-server test-util testing bdd" -- -D warnings
```

`src/mcp` and `src/sb_cdp` are held to the stricter pedantic lint set. An
`#[allow(...)]` needs a `reason = "..."`.

</Step>

<Step title="Test" icon="flask">

```bash Terminal
cargo test --features "mcp-server test-util testing bdd"
```

Count failures from the exit code and the `test result:` lines, never from a
filtered search of the output.

</Step>

<Step title="Check the dependencies" icon="shield">

```bash Terminal
cargo audit
cargo deny check
```

Run them when `Cargo.lock` changed. `cargo deny check` reads `deny.toml`, where
every ignored advisory has a written reason.

</Step>

<Step title="Build the book" icon="book">

```bash Terminal
mdbook build
```

Run it when the docs changed. It writes the book to `book/`.

</Step>

</Steps>

<Tip>

With cargo-nextest, `cargo nextest run --features "mcp-server test-util testing bdd"`
replaces the test step, but it does not run doctests: add
`cargo test --doc --features "mcp-server test-util testing bdd"`. Code that only builds with
the results database is checked by `cargo check --features turso --all-targets`.

</Tip>

## Tests that need a real Chrome

<Badge color="orange" icon="globe">Needs Chrome</Badge>

Tests that start a real browser live in files named `*_chrome.rs`, plus
`browser_smoke.rs`; `edge_msedgedriver.rs` and `firefox_geckodriver.rs` drive
Edge and Firefox. They are `#[ignore]`d, so the default run skips them. Run them
one test binary at a time: a Chrome that is starved of CPU and memory misses its
20 second DevTools deadline.

<CodeGroup>

```bash Library
cargo test --features "mcp-server test-util testing bdd" --test sb_cdp_chrome -- --ignored --test-threads=1
```

```bash Command line
cargo test -p seleniumbase-rs-cli --test cli_e2e_chrome -- --ignored
```

```bash nextest
cargo nextest run --features "mcp-server test-util testing bdd" --run-ignored only
```

```bash mise
mise run chrome
```

</CodeGroup>

- `.config/nextest.toml` puts these binaries in a `chrome` test group with one
  thread, and reports a test as slow after 60 seconds (it is ended after three
  such periods). `--run-ignored only` runs the Edge and Firefox tests too.
- `mise run chrome` runs `tests/*_chrome.rs`, `tests/browser_smoke.rs` and
  `crates/cli/tests/*_chrome.rs` one binary at a time, and skips
  `playwright_chrome`.
- The WebDriver tests need a chromedriver that matches Chrome: `sbrs install chromedriver`
  downloads it into `downloaded_drivers/`. The pure CDP tests need none.
- As root or in a container, Chrome needs `--no-sandbox`: the tests pass it when
  `CI` is set. To run them in Docker with Chromium and Xvfb, see
  [Run the tests in a container](/building#run-the-tests-in-a-container).

## Write the generated files again

Some files are generated. Edit their source, then write them again:

| File | Made from | Command |
|---|---|---|
| `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` | the declarations in `crates/cli/src/sbrs.rs` | `SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec` |
| `docs/parity.md` | `parity/api.toml` | `just parity-docs` |
| `docs/completions/fig-sbrs.ts` | `sbrs completions fig`, through prettier | see `docs/completions/README.md` |

The `usage_spec` test fails when the first two are stale, and `just parity-docs`
fails when `docs/parity.md` was out of date. No test checks the Fig spec.

The navigation in `docs/docs.json` is written by hand. `just mdx` (needs Bun)
builds every page as the Docs7 site does and fails on an undefined name in an
expression, a relative `.md` link, a page without a title, and a page that is in
no menu of `docs/docs.json`.

## Make, just and mise

The repository has the same everyday tasks in three runners. Use whichever you
have.

| Task | just | make | mise |
|---|---|---|---|
| Format check | `just fmt` | `make fmt` | `mise run fmt` |
| Tests | `just test` | `make test` | `mise run test` |
| The library alone | `just test-library` | `make test-library` | - |
| Lint | `just lint` | `make lint` | `mise run lint` |
| Every check in a row | - | `make all` | `mise run gate` |
| Real-Chrome tests | - | - | `mise run chrome` |
| The book | `just docs` | `make docs` | `mise run docs` |
| The layout trees on the docs pages, written again from `tools/docs_layout_tree.py` | `just layout-tree` | - | - |
| `sbrs --help`, `--version` and `doctor` | `just smoke` | `make smoke` | `mise run smoke` |
| Shell completions into `target/completions` | `just completions` | `make completions` | - |

<Warning>

`just lint`, `just test-all`, `just build` and their `make` twins turn on
`s3,azure,gcp,playwright,mcp-server,full-tracing,gui,tui` (the justfile adds
`pdf`), so they need the `gui` and `s3` system packages. They are not the CI
checks above; `mise run gate` is closer to those.

</Warning>

## The Docker image

<CodeGroup>

```bash Build
docker build -t seleniumbase-rs .
```

```bash Build with little memory
docker build --build-arg CARGO_BUILD_JOBS=2 -t seleniumbase-rs .
```

```bash Run
docker run --rm seleniumbase-rs --help
docker run --rm seleniumbase-rs doctor
```

</CodeGroup>

The `Dockerfile` builds `sbrs` with Rust 1.99, without the command line's
default features, and copies it into a `debian:bookworm-slim` image with
Chromium and its driver. It runs as the non-root user `sbrs`. More is under
[Container](/building#container) and [Deployment](/deployment).

<Warning>

The `Dockerfile` caches dependencies with a skeleton step that creates an empty
file for every target the manifests declare. A new `[[bin]]`, `[[bench]]`,
`[[example]]` or `[[test]]`, in either manifest, must be added to that step, or
the image build fails. `tests/dockerfile_skeleton.rs` checks it.

</Warning>

## Working with an AI assistant

The repository is set up for coding assistants: `CLAUDE.md` is the module map,
the commands and the things that have bitten before, `AGENTS.md` is a short rule
sheet, `llms.txt` indexes the docs, and `rust-analyzer.toml` makes analysis cover
the `mcp-server` and `test-util` features. Two optional tools make an assistant
read far less:

<Columns cols={2}>

<Card color="#1a85d4" title="TokenSave" icon="diagram-project" href="https://github.com/aovestdipaperino/tokensave">

A code graph of the repository. Run `tokensave init` in the checkout; the
assistant then asks it for symbols, callers, the impact of a change, and reads
files by symbol instead of searching and reading whole files.

</Card>

<Card color="#199f43" title="RTK" icon="compress" href="https://www.rtk-ai.app/">

A command proxy that condenses the output of `cargo`, `git` and test runners
before it reaches the assistant: `rtk cargo test`, `rtk git log`. Exit codes
are unchanged.

</Card>

</Columns>

Neither is needed to build, test or contribute. `tokensave init` writes its
index to `.tokensave/`, which the repository's `.gitignore` does not list: if
`git status` shows it, add it to `.git/info/exclude`. Whoever opens the pull
request owns the result; see
[Working with AI assistants](/CONTRIBUTING#working-with-ai-assistants).

<Prompt description="Paste into an AI coding assistant that can run commands in this checkout" actions={["copy"]}>

Check this seleniumbase-rs checkout. Read CLAUDE.md first. Export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 and run one cargo command at a time: cargo fmt --all -- --check, then cargo clippy --all-targets --features "mcp-server test-util testing bdd" -- -D warnings, then cargo test --features "mcp-server test-util testing bdd". Do not edit any code. Report the exit code of each command, and the passed and failed counts from the "test result" lines, not from a filtered search of the output.

</Prompt>

## Build problems

<AccordionGroup>

<Accordion title="The linker or C compiler is missing" icon="screwdriver-wrench">

The build stops with `linker 'cc' not found` or a missing `link.exe`. Install
the build tools for your system: `xcode-select --install` on macOS,
`build-essential` (or `gcc gcc-c++ make`) on Linux, and the Visual Studio 2022
Build Tools with the C++ workload on Windows. Open a new terminal afterwards.

</Accordion>

<Accordion title="Rust is too old, or 1.99 is missing" icon="code-compare">

Cargo refuses a package that needs a newer `rustc`, or rustup says the 1.99
toolchain is not installed. Run `rustup toolchain install 1.99` and check
`rustc --version` inside the checkout. Every package of the workspace needs
1.99.

</Accordion>

<Accordion title="The build runs out of memory" icon="memory">

The linker is killed, or the machine stops responding. Set the three variables
from [Keep builds small](#keep-builds-small), lower `CARGO_BUILD_JOBS` further,
and stop other builds. In a 4 GB container the doctests also need
`-- --test-threads=2`.

</Accordion>

<Accordion title="Blocking waiting for file lock" icon="lock">

Another cargo command is building in the same target directory. Wait for it, or
stop it. rust-analyzer checks into a target directory of its own
(`targetDir = true` in `rust-analyzer.toml`), so it does not hold this lock.

</Accordion>

<Accordion title="extern location does not exist" icon="link-slash">

Two checkouts share one `CARGO_TARGET_DIR` and overwrote each other's compiled
library. Give each checkout its own target directory, and do not `cargo clean` a
target directory that another build is using.

</Accordion>

<Accordion title="The gui or s3 feature fails on Linux" icon="linux">

A missing system library: `gui` links GTK and X11, and `s3` needs OpenSSL. Install
the packages from [Extra packages for some features](#extra-packages-for-some-features).

</Accordion>

<Accordion title="The first build fails without a network" icon="wifi">

A first build downloads crates from crates.io, and the plugin crates from
GitHub. Run `cargo fetch` while you are online; later builds can then use
`--offline`.

</Accordion>

<Accordion title="A test says a generated file is stale" icon="arrows-rotate">

Write the file again with the command from
[Write the generated files again](#write-the-generated-files-again) and commit
the result with your change.

</Accordion>

<Accordion title="A manifest test or the Docker build fails" icon="docker">

`tests/dockerfile_skeleton.rs` fails, or the image build stops, after you added
a `[[bin]]`, `[[bench]]`, `[[example]]` or `[[test]]`: list it in the skeleton
step of the `Dockerfile`. `tests/library_manifest.rs` fails when a command-line
crate is in the library's `[dependencies]`: it belongs in
`crates/cli/Cargo.toml`.

</Accordion>

</AccordionGroup>

More system-specific notes are in [Operating systems](/building#operating-systems).

## Next steps

<Columns cols={2}>

<Card color="#d47628" title="Debugging" icon="bug" href="/contributing/debugging">

Logs, headed runs, evidence when a test fails, and tests without Chrome.

</Card>

<Card color="#a631be" title="Benchmarking" icon="gauge-high" href="/contributing/benchmarking">

Run the harnesses in `benches/` and compare a change fairly.

</Card>

<Card color="#1ca1c7" title="Contributing" icon="code-branch" href="/CONTRIBUTING">

Code style, testing rules and how to open a pull request.

</Card>

<Card color="#d32a61" title="Developer guide" icon="book-open" href="/DEVELOPER_GUIDE">

How `BaseCase`, the engines and the stealth layer fit together.

</Card>

</Columns>
