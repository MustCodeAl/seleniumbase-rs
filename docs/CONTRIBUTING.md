---
title: "Contributing to SeleniumBase-rs"
sidebarTitle: "Contributing"
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

Thank you for helping make `seleniumbase-rs` a powerful, idiomatic Rust
browser-automation framework. Whether you are fixing a bug, adding a new stealth
evasion, writing a tutorial, or improving the MCP server, this guide will get
you oriented quickly.

<Columns cols={2}>

<Card color="#1a85d4" title="Build from source" icon="hammer" href="/contributing/from-source">

Clone, set up Rust 1.99, build, and run the checks, including the real-Chrome tests.

</Card>

<Card color="#199f43" title="Debugging" icon="bug" href="/contributing/debugging">

Logs, headed runs, evidence when a test fails, and a mocked browser.

</Card>

<Card color="#d47628" title="Benchmarking" icon="gauge-high" href="/contributing/benchmarking">

Run the harnesses in `benches/` and compare a change fairly.

</Card>

<Card color="#a631be" title="Building and installing" icon="box-open" href="/building">

Features, binaries, the Dockerfile and build-time tips.

</Card>

</Columns>

<Visibility for="agents">

Summary for AI exports: every change must pass `cargo fmt --all -- --check`, `cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings` and `cargo test --features "mcp-server test-util"`; `cargo deny check` and `cargo audit` when `Cargo.lock` changed, and `mdbook build` when docs changed. Tests that need a real Chrome are `*_chrome.rs` (plus `browser_smoke`), `#[ignore]`d, run one binary at a time. A bug fix gets a test that fails without it. Commits carry no co-author or tool trailers; never force-push.

</Visibility>

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Quick Start](#quick-start)
3. [Development Environment](#development-environment)
4. [Project Layout](#project-layout)
5. [Architecture at a Glance](#architecture-at-a-glance)
6. [Code Style](#code-style)
7. [Error Handling](#error-handling)
8. [Tracing and Observability](#tracing-and-observability)
9. [Testing](#testing)
10. [Parity with Python SeleniumBase](#parity-with-python-seleniumbase)
11. [Adding a Stealth Evasion Provider](#adding-a-stealth-evasion-provider)
12. [Adding a Fingerprint Setting or Policy Field](#adding-a-fingerprint-setting-or-policy-field)
13. [Adding a Macro](#adding-a-macro)
14. [Adding an MCP Tool](#adding-an-mcp-tool)
15. [Adding an `sbrs` Subcommand](#adding-an-sbrs-subcommand)
16. [Adding a Profile Payload Field](#adding-a-profile-payload-field)
17. [Binary Patching](#binary-patching)
18. [Writing Documentation](#writing-documentation)
19. [Working with AI Assistants](#working-with-ai-assistants)
20. [Opening a Pull Request](#opening-a-pull-request)
21. [Publishing](#publishing)
22. [Licensing](#licensing)

## Code of Conduct

- Be respectful and constructive.
- Assume good intent.
- Focus feedback on code and design, not individuals.
- Do not include third-party anti-detect product names, trademarks, or proprietary
  payload examples in code, docs, or tests. Use generic terminology such as
  "external profile", "anti-detect payload", or "browser fingerprint".

## Quick Start

```bash Terminal
# Clone the crate
git clone https://github.com/MustCodeAl/seleniumbase-rs.git
cd seleniumbase-rs

# The checks every change must pass (the same ones CI runs)
cargo fmt --all -- --check
cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
cargo test --features "mcp-server test-util"

# Build the mdBook
mdbook build
```

Features, binaries, the Dockerfile and build-time tips are in
[docs/building.md](/building). Every step from a fresh machine to a passing
check is in [Build from source](/contributing/from-source).

<Tip>

Read `CLAUDE.md` once even if you are not using an AI assistant: it is a short map
of the modules, the commands and the things that have bitten before.

</Tip>

Three environment flags keep builds cheap on a small machine, and they let
several checkouts share compiled dependencies:

```bash Terminal
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
```

<Warning>

Do not run two heavy cargo commands at once in the same target directory (cargo
serialises them with a lock), and do not share one `CARGO_TARGET_DIR` between
several checkouts of this repository: they overwrite each other's compiled
library and doctests then fail with "extern location does not exist".
[mr-boxington](https://mr-boxington.jdx.dev) (below) avoids both problems: each
checkout gets its own target and they share compiled work through its cache.

</Warning>

## Development Environment

<Info>

With [mise](https://mise.jdx.dev), `mise install` sets up the tools below from
`mise.toml` (Rust 1.99 with clippy and rustfmt, Python 3.14 for the parity
scripts, Node for the Tauri example, mdBook, just and cargo-deny), and
`mise run gate` runs the checks a change has to pass; `mise tasks` lists the
rest (`chrome`, `parity-docs`, `docs`, `smoke`).

</Info>

### Required

- [Rust](https://rustup.rs/) 1.99 or newer (stable channel); `rust-version` in
  `Cargo.toml` is the newest stable release and the <Tooltip tip="Minimum supported Rust version: the CI job that checks the crate with exactly the declared rust-version.">`msrv`</Tooltip> CI job enforces it.
- `cargo`, `rustfmt` and `clippy`.
- Python 3.10+ only for the scripts in `tools/` (the parity tables). The crate
  itself, every feature included, needs no Python.

### Optional but recommended

- [mdBook](https://rust-lang.github.io/mdBook/) for documentation:
  `cargo install mdbook`.
- [mr-boxington](https://mr-boxington.jdx.dev) (`mbx`), a shared cache for Cargo
  builds: `cargo install mbx --locked && mbx setup`, or with mise
  `mise use --global --tool-option mr_boxington=true rust mr-boxington`. Keep
  running `cargo` as usual: every checkout and worktree gets its own managed
  `target`, compiled work is reused between them (and in CI), and the cache
  prunes itself. It caches nothing while `RUSTC_WRAPPER` is set (remove
  sccache), and it leaves a `CARGO_TARGET_DIR` you set alone.
- [cargo-nextest](https://nexte.st/) for faster test runs
  (`cargo install cargo-nextest`; it does not run doctests, so also run
  `cargo test --doc`). `.config/nextest.toml` has a one-thread `chrome` group
  for the tests that need a real Chrome.
- `cargo-deny` and `cargo-audit` for the dependency checks, and `just` for the
  recipes in the `justfile`.
- `uv` to regenerate the parity docs (`just parity-docs`).
- A Chrome (and, for the WebDriver engine, a matching chromedriver: `sbrs
  install`) for the tests marked `#[ignore]`.
- `libwebkit2gtk-4.1-dev` (Linux) or Xcode (macOS) if you build the Tauri
  example.

### Recommended for working with an AI assistant

<Columns cols={2}>

<Card color="#1ca1c7" title="TokenSave" icon="diagram-project" href="https://github.com/aovestdipaperino/tokensave">

`brew install aovestdipaperino/tap/tokensave`, then `tokensave init` in the
checkout; `tokensave init` keeps `.tokensave/` out of git by adding it to
`.git/info/exclude`. A code graph of the repository
that an assistant queries over MCP (`tokensave serve`): symbols by name,
callers and callees, the impact of a change, and files read by symbol. The
assistant reads far less of the code to answer a question or make a change.

</Card>

<Card color="#d32a61" title="RTK" icon="compress" href="https://www.rtk-ai.app/">

`brew install rtk`. A command proxy that condenses the output of `cargo`,
`git`, test runners and similar tools before it reaches an assistant
(`rtk cargo test`, `rtk git log`), so long build and test logs do not fill its
context. Exit codes are unchanged.

</Card>

</Columns>

Neither is needed to build, test or contribute. How to set them up, with a
prompt that runs the checks, is in
[Build from source](/contributing/from-source#working-with-an-ai-assistant).

## Project Layout

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

## Architecture at a Glance

`seleniumbase-rs` is organized in layers:

1. **Two engines** — the WebDriver engine (`BrowserConfig`, `BrowserSession`
   and the `BaseCase` state machine over `thirtyfour`) and the pure CDP engine
   (`sb_cdp`), which talks to Chrome directly with no driver. They share the
   capability traits where it makes sense, not a common base class.
2. **API layer** — Capability traits (`BrowserApi`, `ElementApi`,
   `AssertionApi`, `ScreenshotApi`) and the `base_case_impls` modules. New
   helpers should be added to the smallest applicable module and then exposed
   through the trait or `BaseCase` impl.
3. **Stealth layer** — `Fingerprint` (one `Spoof` per setting) + `StealthPolicy` describe what to spoof;
   `EvasionProvider`s generate the JavaScript bootstrap; `ChromedriverPatcher`
   mutates the local driver binary; `StealthReactor` intercepts network requests
   via CDP.
4. **Tooling layer** — `sbrs` CLI, the MCP server, the test fixtures and Gherkin scenarios, and the
   Python importer.

When adding a feature, try to keep each layer independent. For example, a new
fingerprint provider should not depend on `BaseCase`, and a new CLI command
should not contain browser logic directly.

## Code Style

- Follow the Rust style enforced by `cargo fmt`.
- Keep Clippy clean with `-D warnings` (command in [Quick Start](#quick-start)).
  `src/mcp` and `src/sb_cdp` are held to the stricter pedantic lint set. An
  `#[allow(...)]` needs a `reason = "..."`, and a function over 100 lines fails
  the lint, so split it.
- Prefer explicit types on public APIs; this helps Copilot and other agents infer intent.
- Use `tracing` for logs instead of `println!` in library code.
- Avoid `unsafe` unless absolutely necessary and clearly documented.
- Keep functions small and focused. If a `BaseCase` helper grows beyond ~50
  lines, add it to the appropriate `base_case_impls` module.
- Prefer `?` and `crate::Result<T>` over manual `match` propagation.
- Use `thiserror` for new error variants and keep variants structured (avoid
  generic string payloads).

## Error Handling

`seleniumbase-rs` uses a single `SeleniumBaseError` enum in `src/error.rs`.

- Add a new variant when a failure mode needs special handling, a unique hint,
  or stable down-stream matching.
- Use helper constructors such as `SeleniumBaseError::element_not_found` instead
  of formatting strings by hand.
- Mark transient failures (network, driver not ready) by returning `true` from
  `is_transient()`.
- Provide actionable hints via `hint()`.
- Instrument error paths with `tracing::error!` and include context with
  `ResultExt::sb_context()`:

```rust Adding context
use seleniumbase_rs::{ResultExt, SeleniumBaseError};

some_fallible_op()
    .sb_context("downloading chromedriver")?;

SeleniumBaseError::driver_not_ready("chromedriver").log();
```

See `docs/ABI_API.md` and `docs/help/common_problems.md` for the error design
philosophy.

## Tracing and Observability

The crate uses `tracing` throughout. Common patterns:

```rust Instrumenting a function
use tracing::{info, warn, error, instrument};

#[instrument(skip(self), fields(url = %url))]
pub async fn open(&mut self, url: &str) -> crate::Result<()> {
    info!("opening page");
    // ...
}
```

Feature flags control subscriber output:

| Feature | Effect |
|---|---|
| `full-tracing` | Enables detailed span lifecycle logging. |
| `error-backtrace` | Declared for backtraces on `SeleniumBaseError`; no code reads it yet. |

Run the CLI with tracing enabled:

```bash Terminal
RUST_LOG=info cargo run --bin sbrs --features full-tracing -- open https://example.com
```

See `docs/tutorials/tracing.md` for subscriber configuration examples. How to
turn the logs on, and the other ways to find out why a run fails, are in
[Debugging](/contributing/debugging).

## Testing

- Write the test first when you can: the signature with a `todo!()` body, the
  tests, then the code. A bug fix gets a test that fails without the fix.
- Add unit tests for pure logic: selectors, fingerprint generation, macros,
  providers, JSON parsing.
- Use `#[tokio::test]` for async helpers that do not require a browser. The pure
  CDP engine has a mock browser (`Browser::new_mocked`, `MockCtrl`), so waits,
  retries and error paths can be tested without Chrome (see `tests/sb_cdp_*.rs`).
- Tests that need a real Chrome are named `*_chrome.rs` (plus `browser_smoke`)
  and are `#[ignore]`d. Run them one test binary at a time:
  `cargo test --test mcp_cdp_chrome -- --ignored`. The tests that run the
  `sbrs` binaries are in `crates/cli/tests`; name the package for those:
  `cargo test -p seleniumbase-rs-cli --test cli_e2e_chrome -- --ignored`.
- A test that fakes `java` or another executable must run the script once
  (`--warm`) before timing it: macOS can take seconds to run a freshly written
  script the first time.
- When writing doctests, mark browser-dependent snippets with `no_run` or
  `ignore`. Tutorials in `docs/tutorials/` are not compiled, so keep their code
  true by hand.

### Verification matrix (run before every PR)

```bash Terminal
cargo fmt --all -- --check
cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
cargo test --features "mcp-server test-util"
cargo check --features turso --all-targets            # code that only builds with turso
cargo deny --workspace check && cargo audit            # when Cargo.lock changed
mdbook build                                           # when docs changed
```

Count failures from the exit code and the `test result:` lines, not from a
filtered search of the output. The repository is a workspace of two packages, the
library `seleniumbase-rs` and the command line `seleniumbase-rs-cli`
(`crates/cli`), and the commands above cover both (`default-members`). Use
`-p seleniumbase-rs` for the library alone, with its own default features: the
command line turns `pdf` and `mcp-server` on, and a build of both unifies them.
A change that touches a `Cargo.toml` must keep the `Dockerfile` skeleton step in
step: a new `[[bin]]`, `[[bench]]`, `[[example]]` or `[[test]]`, in either
manifest, must be listed there, and `tests/dockerfile_skeleton.rs` fails if it
is not. `tests/library_manifest.rs` fails if a binary or a command-line crate
(`usage`, or the clap crates it replaced) comes back into the library package.

`cargo deny check` gates advisories, licenses, banned crates, and dependency
sources against `deny.toml`. Every advisory that is ignored there carries a
written rationale, so if a new one appears, either upgrade the dependency or
add an entry explaining why the risk is acceptable.

<Warning>

Do not widen the ignore list silently.

</Warning>

The crate declares `rust-version = "1.99"`, the newest stable release when it was
last raised, which the `msrv` CI job enforces with that exact toolchain. The
floor is a choice to track current stable, not something the crate's own source
needs, so when it moves, bump `rust-version`, the `msrv` job, and the
`Dockerfile` base image together.

<Tip>

On macOS, if a test binary cannot load `libpython`, prefix the test commands with:

```bash
DYLD_LIBRARY_PATH="$HOME/.local/share/mise/installs/python/3.14.6/lib" cargo test --features "mcp-server test-util"
```

</Tip>

## Parity with Python SeleniumBase

The project's goal is that every API of Python SeleniumBase 4.55.2 (and of its
MCP server) exists here or has a stated reason why not.

- `parity/api.toml` records one entry per upstream API with a `status`:
  `implemented` (Rust code and a test), `composed` (existing Rust calls already
  do it; name them in `rust`), `not-applicable` (with a `reason` specific to that
  API) or `planned`. Never mark something `implemented` without code and a test;
  `tests/parity.rs` checks that every Rust path in the manifest exists.
- After editing the manifest, regenerate `docs/parity.md` with `just parity-docs`
  (or `uv run tools/parity/render_docs.py`) and commit both. `just parity-check`
  verifies them.
- `parity/upstream.toml` and `docs/UPSTREAM_SYNC.md` describe how to move to a
  newer upstream release.

## Adding a Stealth Evasion Provider

The stealth layer uses a provider registry. `evasions::bootstrap_script(fp)` is
a thin wrapper over the registry.

<Steps>

<Step title="Implement EvasionProvider" icon="code">

Open `src/evasion/providers/builtin.rs` and implement `EvasionProvider`:

```rust src/evasion/providers/builtin.rs
use seleniumbase_rs::evasion::providers::{EvasionContext, EvasionProvider};
use seleniumbase_rs::evasion::fingerprint::Dimension;

pub struct MyEvasionProvider;

impl EvasionProvider for MyEvasionProvider {
    fn name(&self) -> &str {
        "my_evasion" // stable, unique, snake_case
    }

    fn priority(&self) -> i32 {
        // Lower runs earlier. native_toString is 5; navigator props ~30;
        // late/self-defense providers are 110+.
        100
    }

    fn applies(&self, fp: &crate::evasion::fingerprint::Fingerprint) -> bool {
        // The one predicate: on when the fingerprint holds a value for the
        // dimension, or its flag says Mask; off when the flag says Natural.
        fp.is_masked(Dimension::Navigator)
    }

    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let value = EvasionContext::escape(
            ctx.fingerprint.vendor.as_deref().unwrap_or("Example"),
        );
        Some(format!("(function() {{ /* use '{value}' */ }})();"))
    }
}
```

</Step>

<Step title="Register it" icon="list-ol">

Register it in `all()` at the bottom of `builtin.rs` (keep sorted by priority).

</Step>

<Step title="Wrap replaced natives" icon="mask">

Wrap replaced natives so `Function.prototype.toString` still reports
`[native code]`:

```js Wrapping a native
obj.method = (window.__sbNative || function(f){return f;})(patched, 'method');
```

The `native_to_string` provider (priority 5) installs `window.__sbNative`
before any other provider runs.

</Step>

<Step title="Seed the noise" icon="seedling">

Use `ctx.seed` for deterministic per-session noise (canvas, audio, WebGL).

</Step>

<Step title="Test it" icon="flask">

Add a unit test asserting the snippet contains expected markers and that
`applies` is gated correctly.

</Step>

<Step title="Document it" icon="book">

Update `docs/tutorials/fingerprint_stealth.md`.

</Step>

</Steps>

## Adding a Fingerprint Setting or Policy Field

A setting is what the browser claims (one `Spoof<T>` field of `Fingerprint`); a
policy field is how the claims are delivered (a field of `StealthPolicy` in
`src/evasion/policy.rs`).

For a setting:

<Steps>

<Step title="Add the field" icon="code">

Add the field to `Fields` in `src/evasion/fingerprint.rs`, with its default
in `impl Default for Fields` and `#[serde(skip_serializing_if = "Spoof::is_natural")]`,
and a read accessor in the `accessors!` list of `Fingerprint`. If its value is
not a plain type, add the value type to `src/evasion/settings.rs` with a
conversion from the obvious tuple.

</Step>

<Step title="Add the setter" icon="pen-to-square">

Add its setter to the `setters!` list of `FingerprintBuilder`, and its name to
`has_random` and `without_random`. Decide whether it has an off state: if not,
add it to `refuse_useless_off`; and what `Random` draws: add the arm to
`draw_random` (taking the value from the `Draw` in `randomize.rs`), or make it
an error there.

</Step>

<Step title="Give it a dimension" icon="layer-group">

Give the dimension it belongs to (or a new `Dimension` variant, which is
`#[non_exhaustive]`) an arm in `Fingerprint::is_masked`. Providers, launch
arguments and overrides ask `is_masked`, never a field by hand.

</Step>

<Step title="Validate it" icon="circle-check">

Add coherence validation in `Fingerprint::validate()` if the setting has
required companions.

</Step>

<Step title="Wire it in" icon="plug">

Wire the setting into the launcher (`browser/session.rs`), provider scripts,
or the reactor if applicable. A provider defines only a setting that is a
`Value`, and handles `Off` where it has one.

</Step>

<Step title="Read it from a payload" icon="file-import">

Add the payload key to `ProfileParams::fingerprint_builder` if a payload can
carry it, reading the common words of `Word`.

</Step>

<Step title="Document it" icon="book">

Document the setting in `docs/tutorials/fingerprint_stealth.md`.

</Step>

</Steps>

For a policy field, add the field, its default and its preset values to
`StealthPolicy`, a method of the same name, and a builder shortcut
(`with_policy`); a preset never changes a setting.

## Adding a Macro

Public macros live in `src/macros.rs` and are re-exported at the crate root.

```rust src/macros.rs
#[macro_export]
macro_rules! sb_focus {
    ($sb:expr, $css:expr) => {{
        $sb.focus($css)
    }};
}
```

<Steps>

<Step title="Add the macro" icon="code">

Add the macro with `#[macro_export]`.

</Step>

<Step title="Test it" icon="flask">

Write a compile-time unit test under the definition.

</Step>

<Step title="Document it" icon="book">

Update `docs/tutorials/macros.md` and the macro table in `README.md`.

</Step>

</Steps>

## Adding an MCP Tool

The MCP server (`src/mcp/`, started by `crates/cli/src/mcp_server.rs`) uses the
`rmcp` crate.

A tool is a `ToolDef` in `src/mcp/`; read `src/mcp/devtools.rs` for working
examples (they are generic over the session type, so one definition serves both
engines) and `src/mcp/stealth.rs` for how an extra toolset is attached.

<Steps>

<Step title="Declare it" icon="code">

Declare it with `ToolDef::new(name, title, description, effect, schema,
handler)`. The handler is an `async fn(Ctx<S>, Args) -> Result<Output,
ToolError>`. Pick the `Effect` honestly: `Observe` reads the open web,
`Inspect` looks at this machine, `Act` changes the page, `Set` changes the
session idempotently, `Overwrite` writes local files.

</Step>

<Step title="Report failures" icon="triangle-exclamation">

Return `ToolError::Failed(SeleniumBaseError)` for failures: the server turns it
into a failure report with the page's state, the hint and the next tools to
try. Files a tool writes go through the output directory
(`SB_MCP_OUTPUT_DIR`), never to an arbitrary path.

</Step>

<Step title="Register it" icon="list-ol">

Register it in the server that offers it (`cdp.rs`, `driver.rs`, `sb.rs`).

</Step>

<Step title="Test it" icon="flask">

Test it without Chrome first (`tests/mcp_cdp.rs`, `tests/mcp_devtools.rs`),
then with a real Chrome in a `*_chrome.rs` test if it reads a page.

</Step>

<Step title="Document it" icon="book">

Document it in `docs/tutorials/mcp_server.md`, add it to `CHANGELOG.md`, and
give it an entry in `parity/api.toml` if upstream has the same tool.

</Step>

</Steps>

## Adding an `sbrs` Subcommand

<Steps>

<Step title="Add the variant" icon="code">

Add the new variant to `Commands` in `crates/cli/src/sbrs.rs` with an
idiomatic `///` description (its first paragraph is the short help).

</Step>

<Step title="Parse the arguments" icon="sliders">

Declare its arguments and flags with `#[usage(...)]` attributes on the fields, as
the neighbouring variants do (`long`, `short`, `default`, `conflicts(...)`,
`value_enum`; a switch takes `bool_value`).

</Step>

<Step title="Handle it" icon="gears">

Add a handler branch in `async fn run`.

</Step>

<Step title="Clean up" icon="broom">

If the command launches a browser, prefer creating a `BaseCase` through
`run_browser_test()` so cleanup is deterministic.

</Step>

<Step title="Write the generated files" icon="arrows-rotate">

Write `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` again with
`SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`;
the same test fails in CI when they are stale.

</Step>

<Step title="Document it" icon="book">

Update `docs/help/commander.md` and `README.md` if the command is user-facing.

</Step>

</Steps>

## Adding a Profile Payload Field

External profile payloads are parsed in `src/profile_payloads/profile.rs`.

<Steps>

<Step title="Add the field" icon="code">

Add the field to `ProfileParams` (or the appropriate nested struct).

</Step>

<Step title="Map it" icon="arrow-right-arrow-left">

Map it to `BrowserConfig` or `Fingerprint` in `to_browser_config()` and
`browser()`.

</Step>

<Step title="Test it" icon="flask">

Add a unit test for parsing and round-tripping.

</Step>

<Step title="Document it" icon="book">

Update `docs/tutorials/browser_profiles.md`.

</Step>

</Steps>

## Binary Patching

`ChromedriverPatcher` modifies a local chromedriver binary to help evade
binary-signature detection.

- Patches are defined as `EnginePatch` structs in `src/evasion/patcher.rs`.
- Each patch has a human-readable `id`, byte sequence search/replace, and
  optional marker detection.
- Add new patches to `ChromedriverPatcher::default_patches()`.
- Always test patches against the driver versions listed in
  `docs/help/binary_patching.md`.
- Backups are stored next to the binary with `.sb-backup` extension.

## Writing Documentation

- All public items should have doc comments (`///`).
- Use examples in doc comments; they run as doctests.
- User-facing tutorials go in `docs/tutorials/`.
- Reference/help pages go in `docs/help/`.
- AI-agent guidance goes in `COPILOT.md` and `CLAUDE.md`.
- After adding a page, add it to `docs/SUMMARY.md`.
- A user-visible change also gets an entry in `CHANGELOG.md`.
- Say what was verified and how. A claim like "works in Chrome" needs a real
  Chrome behind it; otherwise say it was checked against the mock or by reading.
- Build the book locally before pushing: `mdbook build`.

## Working with AI Assistants

Assistants are welcome, and the repository is set up for them: `AGENTS.md` is the
short rule sheet any agent can read, `llms.txt` indexes the docs for models,
`CLAUDE.md` is the module map and rule list, `rust-analyzer.toml` makes analysis cover the
optional features, and `docs/tutorials/vibe_coding.md` shows how to use the MCP
servers to explore a page and export a test. Whoever opens the pull request owns
the result:

- Run the checks yourself and report their exit codes; do not paste "it should
  pass".
- Read the diff. Reject tests that cannot fail, assertions that were loosened to
  make a test pass, and `sleep` calls added instead of waits.
- Do not let an assistant mark a parity entry `implemented` without code and a
  test, or invent an API: check it against the code or the library's docs.
- Keep secrets out of prompts, logs and committed files.

## Opening a Pull Request

<Steps>

<Step title="Branch" icon="code-branch">

Branch from `main` unless told otherwise.

</Step>

<Step title="Commit" icon="code-commit">

Keep commits focused and atomic, with conventional-commit subjects
(`feat(scope): ...`, `fix(scope): ...`, `test: ...`, `docs: ...`, `refactor: ...`).
Commits carry no co-author or tool trailers, and never contain secrets.

</Step>

<Step title="Update tests and docs" icon="file-pen">

Update the tests, `CHANGELOG.md` and the docs in the same change.

</Step>

<Step title="Describe the change" icon="pen">

Fill out the PR description with:

- What changed and why.
- Verification commands that passed.
- Any breaking API changes and migration notes.

</Step>

<Step title="Check CI" icon="circle-check">

Ensure CI is green.

</Step>

</Steps>

### PR review checklist

- [ ] `cargo fmt --all -- --check` is clean.
- [ ] `cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings` is clean.
- [ ] `cargo test --features "mcp-server test-util"` passes.
- [ ] `cargo deny check` and `cargo audit` are clean if `Cargo.lock` changed.
- [ ] A new `[[bin]]`, `[[bench]]`, `[[example]]` or `[[test]]` is in the
      `Dockerfile` skeleton.
- [ ] `parity/api.toml` and `docs/parity.md` are updated for a new API.
- [ ] `mdbook build` succeeds if docs changed.
- [ ] New public items have doc comments.
- [ ] New features are covered by tests or examples.
- [ ] Documentation reflects the change.
- [ ] No third-party anti-detect product names or trademarks were added.

## Publishing

The crate is not published to crates.io; it is distributed from the repository.
To publish a change to `main`:

<Steps>

<Step title="Verify the exact commit" icon="list-check">

Run the full verification matrix on the exact commit you will push.

</Step>

<Step title="Fetch" icon="cloud-arrow-down">

`git fetch`, and confirm `origin/main` has not moved since you started.

</Step>

<Step title="Fast-forward push" icon="forward">

Push as a fast-forward only (for example
`git push origin <commit>:refs/heads/main`), then check that your local
branch and `origin/main` are the same commit.

</Step>

</Steps>

<Danger>

Never force-push.

</Danger>

A docs-only commit on top of a commit that passed the checks may be pushed
after it, and its message should say so.

## Licensing

By contributing, you agree that your contributions will be licensed under the
same license as the project (MIT).
