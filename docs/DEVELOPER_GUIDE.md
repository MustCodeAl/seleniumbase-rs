---
title: "Developer Guide"
sidebarTitle: "Developer guide"
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

This guide is for anyone who wants to understand, extend, or maintain the Rust
port of SeleniumBase (`seleniumbase-rs`). It covers the repository layout, the
`BaseCase` architecture, how to add new helpers, the stealth system, testing, and
release considerations.

<Info>

**What you will learn**

- How the repository is organized.
- How `BaseCase` methods are split across domain files.
- How to add a new `BaseCase` helper or evasion provider.
- How to run tests and interpret CI checks.
- Where to find the public API contract and stability notes.

</Info>

<Columns cols={2}>

<Card color="#1a85d4" title="Build from source" icon="hammer" href="/contributing/from-source">

Clone, build, keep builds small, and run the checks and the real-Chrome tests.

</Card>

<Card color="#199f43" title="Debugging" icon="bug" href="/contributing/debugging">

Logs, headed and slowed-down runs, evidence on failure, and a mocked browser.

</Card>

<Card color="#d47628" title="Benchmarking" icon="gauge-high" href="/contributing/benchmarking">

The `cpu` and `cdp_latency` harnesses, and how to compare a change.

</Card>

<Card color="#a631be" title="Contributing" icon="code-branch" href="/CONTRIBUTING">

Code style, testing rules and how to open a pull request.

</Card>

</Columns>

<Visibility for="agents">

Summary for AI exports: `BaseCase` is defined in `src/api/base_case.rs`, and its methods are split by domain across `src/api/base_case_impls/*.rs`. The capability traits are in `src/api/traits.rs`. Fallible calls return `crate::Result<T>` with `SeleniumBaseError`. No library feature is on by default; the `sbrs` and `sbrs-mcp` binaries are the package `seleniumbase-rs-cli` in `crates/cli`. Browser tests use `run_browser_test` so cleanup is awaited after success or failure.

</Visibility>

## Project goals

`seleniumbase-rs` aims to provide a Rust-native browser-automation and end-to-end
testing framework with parity to the Python SeleniumBase API. The public surface
is centered around `BaseCase`, a stateful test object that manages a browser
session, configuration, recorder, deferred assertions, tours, charts,
presentations, and optional stealth/CDP integrations.

## Repository layout

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
<LayoutFile name="trace.rs" note="sbrs trace show: opens a trace folder's viewer" />
<LayoutDir name="mcp_install" note="sbrs-mcp install, uninstall and clients" />
</LayoutDir>
<LayoutDir name="tests" note="every test that runs the real sbrs or sbrs-mcp binary" />
</LayoutDir>
<LayoutDir name="own-imports" note="tests that import only seleniumbase_rs: the proof that its re-exports work" />
<LayoutDir name="playwright" note="the package seleniumbase-rs-playwright: Stealthy Playwright Mode, rustwright attached to a stealthy sb_cdp browser" />
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
<LayoutDir name="examples" note="runnable examples" />
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
<LayoutFile name="check-consumers.sh" note="mise run contracts: checks what the repositories that build on this crate use" />
</LayoutDir>
<LayoutDir name="tools" note="scripts for the docs, the parity manifests, the Linux check and the release archives">
<LayoutDir name="mdx-check" note="mise run mdx: builds every docs page as Docs7 does" />
<LayoutDir name="parity" note="extracts, seeds and renders the parity manifest (docs/parity.md)" />
<LayoutDir name="layout_tree" note="the tree component that docs_layout_tree.py copies onto the pages" />
<LayoutDir name="linux-check" note="mise run linux-check: the test suite and the Chrome tests in a Linux container" />
<LayoutDir name="release" note="build-binaries.sh: the release archives for Linux and macOS Intel" />
<LayoutFile name="docs_layout_tree.py" note="mise run layout-tree: writes the layout trees of the docs pages" />
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
<LayoutFile name="mise.toml" note="the tools and the tasks of the repository (mise tasks ls)" />
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

## BaseCase architecture

`BaseCase` is a single struct that owns the test state:

```rust src/api/base_case.rs
pub struct BaseCase {
    session: BrowserSession,
    config: BrowserConfig,
    recorder: Arc<Mutex<ActionRecorder>>,
    tour: Option<Tour>,
    deferred: DeferredAsserts,
    presentation: Option<Presentation>,
    chart: Option<Chart>,
    qa_session: Option<MasterQaSession>,
    time_limit: Option<Duration>,
    started: Instant,
    timeout_secs: u64,
    gui_held: Option<(i32, i32)>,
}
```

The inherent implementation is split across `src/api/base_case.rs` (core
constructors and commonly used methods) and `src/api/base_case_impls/*.rs`
(domain-specific helpers). Each included file is an `impl BaseCase { ... }`
block; they rely on the imports declared at the top of `base_case.rs`.

Domain files:

<LayoutTree>
<LayoutDir name="src/api/base_case_impls" open>
<LayoutFile name="base_case_impl_common.rs" highlight note="constructors, lifecycle, and widely-shared helpers" />
<LayoutFile name="base_case_impl_alerts.rs" note="alert and prompt handling" />
<LayoutFile name="base_case_impl_browser.rs" note="browser introspection and state" />
<LayoutFile name="base_case_impl_charts.rs" note="chart generation helpers" />
<LayoutFile name="base_case_impl_dom.rs" note="DOM querying and manipulation" />
<LayoutFile name="base_case_impl_downloads.rs" note="file download helpers" />
<LayoutFile name="base_case_impl_links.rs" note="link and anchor helpers" />
<LayoutFile name="base_case_impl_storage.rs" note="cookies and web-storage helpers" />
<LayoutFile name="base_case_impl_window.rs" note="windows, frames, and tabs" />
<LayoutFile name="base_case_impl_mouse.rs" note="mouse and hover actions" />
<LayoutFile name="base_case_impl_nav.rs" note="navigation and URL helpers" />
<LayoutFile name="base_case_impl_media.rs" note="media and screenshot helpers" />
<LayoutFile name="base_case_impl_tours.rs" note="guided tour helpers" />
<LayoutFile name="base_case_impl_presentations.rs" note="presentation helpers" />
<LayoutFile name="base_case_impl_jslibs.rs" note="third-party JS library injection" />
<LayoutFile name="base_case_impl_misc.rs" note="test-control and message-overload helpers" />
<LayoutFile name="base_case_impl_pdf_html.rs" note="PDF and HTML parsing helpers" />
<LayoutFile name="base_case_impl_extra.rs" note="extra assertion and wait helpers" />
<LayoutFile name="base_case_impl_cdp_page.rs" note="CDP-page helpers" />
<LayoutFile name="base_case_impl_dialog_inspector.rs" note="dialog inspection helpers" />
<LayoutFile name="base_case_impl_shadow.rs" note="shadow-DOM helpers" />
<LayoutFile name="base_case_impl_gui.rs" note="GUI automation helpers; compiled only with the gui feature, so lint and test it with --features gui" />
<LayoutFile name="base_case_impl_masterqa.rs" note="MasterQA helpers" />
</LayoutDir>
</LayoutTree>

### Adding a new `BaseCase` helper

<Steps>

<Step title="Choose the domain file" icon="folder-open">

Choose the appropriate domain file in `src/api/base_case_impls/` (or create
a new one if the domain does not exist).

</Step>

<Step title="Add the method" icon="code">

Add the method inside the existing `impl BaseCase { ... }` block.

</Step>

<Step title="Return a Result" icon="reply">

Return `crate::Result<T>` (an alias for `Result<T, SeleniumBaseError>`).

</Step>

<Step title="Record the action" icon="video">

Record the action with `self.record(...)` when it represents a user-facing
test step.

</Step>

<Step title="Test it" icon="flask">

Add a unit test when the helper does not require a live browser. A test that
needs a `BaseCase` with a WebDriver session can start a `MockWebDriver` (in
`base_case_impl_common.rs`): it answers the WebDriver commands you choose and
records what the `BaseCase` sent.

</Step>

<Step title="Lint and run" icon="circle-check">

Run `cargo clippy --all-targets -- -D warnings` and `cargo test`.

</Step>

</Steps>

Example:

```rust Example helper
impl BaseCase {
    /// Asserts that `css` matches exactly `count` elements.
    pub async fn assert_element_count(
        &self,
        css: &str,
        count: usize,
    ) -> crate::Result<()> {
        let elements = self.find_elements(css).await?;
        if elements.len() == count {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(format!(
                "expected {count} elements for '{css}', found {}",
                elements.len()
            )))
        }
    }
}
```

## Capability traits

The public API is also exposed through capability traits in
`src/api/traits.rs`:

- `BrowserApi` - navigation and lifecycle
- `ElementApi` - element finding and interaction
- `AssertionApi` - test assertions
- `ScreenshotApi` - screenshot capture

These traits are implemented for `BaseCase`. Prefer them when writing helpers
that should work with any type exposing the same capability, and use them as a
blueprint when adding new cross-cutting concerns.

## Error handling

All fallible operations return `crate::Result<T>`, defined as:

```rust The Result alias
pub type Result<T> = std::result::Result<T, SeleniumBaseError>;
```

`SeleniumBaseError` is a `thiserror` enum covering WebDriver errors, I/O,
JSON, invalid configuration, assertions, CDP driver failures, GUI input, and
Playwright errors. Use `?` to propagate errors and add context in user-facing
messages.

### Error taxonomy

| Category | Example variants | Typical cause |
|---|---|---|
| Element | `ElementNotFound`, `ElementNotVisible`, `ElementNotInteractable` | Selector did not match or element is hidden. |
| Driver | `DriverNotReady`, `DriverCrashed`, `SessionNotCreated` | chromedriver is missing, incompatible, or crashed. |
| Config | `InvalidConfig`, `UnsupportedBrowser`, `InvalidUrl` | Bad `BrowserConfig` or profile payload. |
| I/O | `IoError`, `DownloadFailed`, `FileNotFound` | Filesystem or network transfer failure. |
| Assert | `AssertionFailed`, `DeferredAssertFailed`, `Skipped` | Test assertion mismatch or explicit skip. |
| CDP | `CdpError`, `CdpMethodFailed` | Chrome DevTools Protocol call failed. |
| GUI | `GuiInputError`, `OcrError` | Desktop automation or image parsing failure. |
| Playwright | `PlaywrightError` | Playwright runtime error. |

### Helper constructors

Prefer constructors over manual formatting:

```rust Helper constructors
use seleniumbase_rs::SeleniumBaseError;

SeleniumBaseError::element_not_found("#submit")
SeleniumBaseError::invalid_config("a proxy is required but the URL is empty")
SeleniumBaseError::driver_not_ready("chromedriver")
```

### Runtime diagnostics

Errors can emit `tracing::error!` events:

```rust Runtime diagnostics
use seleniumbase_rs::{ResultExt, SeleniumBaseError};

// Adds context and logs on failure
some_op().sb_context("while patching chromedriver")?;

// Logs without returning
SeleniumBaseError::download_failed(url).log();
```

The `error-backtrace` feature is declared but no code reads it yet, so it adds
no backtrace to `SeleniumBaseError`. For the backtrace of a panic, set
`RUST_BACKTRACE=1`.

## Feature flags

The crate uses Cargo features to keep heavy or optional dependencies
contained:

| Feature | Default | Purpose |
|---|---|---|
| `tui` | off | Terminal apps: `sbrs commander`, `sbrs caseplans`, `sbrs behave-gui`. Pulls `ratatui`/`crossterm`. |
| `gui` | off | Native OS dialogs and GUI automation. Pulls `rfd`/`enigo`; on Linux it needs GTK and X11 development packages. |
| `s3` | off | Enables AWS S3 artifact uploads. |
| `azure` | off | Enables Azure Blob Storage artifact uploads. |
| `gcp` | off | Placeholder for Google Cloud integrations. |
| `mcp-server` | off | Builds the `sbrs-mcp` binary using `rmcp`. |
| `full-tracing` | off | Verbose `tracing` span events for debugging. |
| `error-backtrace` | off | Declared for backtraces on `SeleniumBaseError`; no code reads it yet. |

JSON logs need no feature: set `SB_LOG_FORMAT=json` or call `init_tracing_json`.

No feature of the library is on by default, so a library user or a headless pool
pulls in neither the terminal UI nor the desktop automation stacks (nor any
command-line crate: `sbrs` is the workspace package `seleniumbase-rs-cli`, in
`crates/cli`, which forwards `tui`, `gui` and `turso`). Turn them on when you
want them:

<CodeGroup>

```toml Cargo.toml
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["gui", "tui"] }
```

```bash Commands
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --features tui   # sbrs commander
cargo run --example gui_automation --features gui
```

</CodeGroup>

<Note>

The crate compiles cleanly with no features and with `--all-features`; keep both
green when touching feature-gated code. CI also builds `gui,tui` on Linux and
macOS, because the default build no longer covers them.

</Note>

When adding a feature-gated API, use `#[cfg(feature = "...")]` and declare the
dependency as `optional = true` in `Cargo.toml`. Prefer `#[cfg(feature = "...")]`
over `#[cfg(not(feature = "..."))]` so the default build is the simpler path.

## Module deep dive

### `src/api/`

The public test API. `base_case.rs` defines the struct and core constructors;
`base_case_impls/*.rs` split hundreds of helpers by domain. New helpers should
land in the smallest domain file first, then be surfaced through `BaseCase` or a
capability trait.

{/* layout-tree:start src/api */}

<LayoutTree>
<LayoutDir name="api" note="BaseCase (base_case.rs, base_case_impls/), the capability traits, recorder, scenario, charts, tours">
<LayoutFile name="base_case.rs" note="the BaseCase struct and its constructors" />
<LayoutDir name="base_case_impls" open note="BaseCase helpers, one file per domain">
<LayoutFile name="base_case_impl_addon.rs" note="add-on helpers" />
<LayoutFile name="base_case_impl_alerts.rs" note="alerts" />
<LayoutFile name="base_case_impl_browser.rs" note="browser helpers" />
<LayoutFile name="base_case_impl_cdp_page.rs" note="CDP page helpers" />
<LayoutFile name="base_case_impl_cdp_tests.rs" note="tests of the CDP helpers" />
<LayoutFile name="base_case_impl_charts.rs" note="charts" />
<LayoutFile name="base_case_impl_common.rs" note="helpers shared by the domain files" />
<LayoutFile name="base_case_impl_debug.rs" note="debug helpers" />
<LayoutFile name="base_case_impl_demo_tests.rs" note="tests of the demo helpers" />
<LayoutFile name="base_case_impl_dialog_inspector.rs" note="the dialog inspector" />
<LayoutFile name="base_case_impl_dom.rs" note="DOM helpers" />
<LayoutFile name="base_case_impl_downloads.rs" note="downloads" />
<LayoutFile name="base_case_impl_driver_tests.rs" note="tests of the driver helpers" />
<LayoutFile name="base_case_impl_drivers.rs" note="driver helpers" />
<LayoutFile name="base_case_impl_element_tests.rs" note="tests of the element helpers" />
<LayoutFile name="base_case_impl_esc.rs" note="ending a test when ESC is pressed" />
<LayoutFile name="base_case_impl_extra.rs" note="extra helpers" />
<LayoutFile name="base_case_impl_fingerprint.rs" note="fingerprint helpers" />
<LayoutFile name="base_case_impl_form_tests.rs" note="tests of the form helpers" />
<LayoutFile name="base_case_impl_frame_tests.rs" note="tests of the frame helpers" />
<LayoutFile name="base_case_impl_gui.rs" note="GUI helpers" />
<LayoutFile name="base_case_impl_highlight.rs" note="highlighting elements" />
<LayoutFile name="base_case_impl_highlight_tests.rs" note="tests of highlighting" />
<LayoutFile name="base_case_impl_jslibs.rs" note="JavaScript libraries injected into the page" />
<LayoutFile name="base_case_impl_links.rs" note="links" />
<LayoutFile name="base_case_impl_logs_tests.rs" note="tests of the log helpers" />
<LayoutFile name="base_case_impl_masterqa.rs" note="MasterQA helpers" />
<LayoutFile name="base_case_impl_media.rs" note="media helpers" />
<LayoutFile name="base_case_impl_misc.rs" note="miscellaneous helpers" />
<LayoutFile name="base_case_impl_misc_calls_tests.rs" note="tests of the miscellaneous calls" />
<LayoutFile name="base_case_impl_mouse.rs" note="mouse helpers" />
<LayoutFile name="base_case_impl_mouse_tests.rs" note="tests of the mouse helpers" />
<LayoutFile name="base_case_impl_nav.rs" note="navigation helpers" />
<LayoutFile name="base_case_impl_parity.rs" note="parity helpers" />
<LayoutFile name="base_case_impl_pdf_html.rs" note="PDF and HTML helpers" />
<LayoutFile name="base_case_impl_presentations.rs" note="presentations" />
<LayoutFile name="base_case_impl_rec_cdp.rs" note="recorder helpers for CDP" />
<LayoutFile name="base_case_impl_recorder.rs" note="recorder helpers" />
<LayoutFile name="base_case_impl_shadow.rs" note="shadow DOM helpers" />
<LayoutFile name="base_case_impl_storage.rs" note="cookies and storage" />
<LayoutFile name="base_case_impl_switch_tests.rs" note="tests of window and frame switching" />
<LayoutFile name="base_case_impl_switching.rs" note="window and frame switching" />
<LayoutFile name="base_case_impl_test_data.rs" note="test data helpers" />
<LayoutFile name="base_case_impl_text_tests.rs" note="tests of the text helpers" />
<LayoutFile name="base_case_impl_tours.rs" note="tours" />
<LayoutFile name="base_case_impl_traffic_tests.rs" note="tests of the traffic helpers" />
<LayoutFile name="base_case_impl_window.rs" note="window helpers" />
</LayoutDir>
<LayoutFile name="call.rs" note="one call a test makes" />
<LayoutFile name="cdp_driver.rs" note="CdpDriver, the DevTools client used from BaseCase" />
<LayoutFile name="cdp_page.rs" note="CdpPage, one page of that client" />
<LayoutFile name="chart.rs" note="charts" />
<LayoutFile name="deferred.rs" note="assertions that run later" />
<LayoutFile name="demo.rs" note="demo mode: highlight an element and pause" />
<LayoutFile name="dialog.rs" note="dialogs" />
<LayoutFile name="gui.rs" note="GUI automation, feature gui" />
<LayoutFile name="html.rs" note="HTML reports and pages" />
<LayoutFile name="html_inspector.rs" note="the HTML inspector" />
<LayoutFile name="jqc.rs" note="jQuery-style helpers" />
<LayoutDir name="master_qa" open note="MasterQA reports">
<LayoutFile name="report.rs" note="the MasterQA report" />
<LayoutFile name="wrapper_tests.rs" note="tests of the MasterQA wrapper" />
</LayoutDir>
<LayoutFile name="master_qa.rs" note="MasterQA" />
<LayoutFile name="membership.rs" note="membership helpers" />
<LayoutFile name="messages.rs" note="messages shown on the page" />
<LayoutFile name="misc_calls.rs" note="miscellaneous calls" />
<LayoutFile name="mod.rs" note="the api module" />
<LayoutFile name="page_scripts.rs" note="scripts BaseCase runs in the page" />
<LayoutFile name="pdf.rs" note="PDF helpers, feature pdf" />
<LayoutFile name="presentation.rs" note="presentations" />
<LayoutDir name="recorder" open note="the page recorder: state, generated output and its page script">
<LayoutFile name="breakpoint.rs" note="the pause while a person records" />
<LayoutFile name="calls.rs" note="the recorded calls" />
<LayoutFile name="cdp.rs" note="recording over CDP" />
<LayoutFile name="extras.rs" note="extra recorded actions" />
<LayoutFile name="generate.rs" note="turns a recording into a test" />
<LayoutDir name="js" open note="scripts the recorder runs in the page">
<LayoutFile name="recorder.js" note="the recorder script" />
<LayoutFile name="shift_esc.js" note="SHIFT+ESC ends the recording" />
</LayoutDir>
<LayoutFile name="output.rs" note="where a recording is written" />
<LayoutFile name="process.rs" note="processes the recorded actions" />
<LayoutFile name="raw.rs" note="the raw recording" />
<LayoutFile name="state.rs" note="the recorder state" />
</LayoutDir>
<LayoutFile name="recorder.rs" note="the recorder on BaseCase" />
<LayoutFile name="run_settings.rs" note="the settings of one test run" />
<LayoutFile name="runner.rs" note="run_browser_test" />
<LayoutFile name="scenario.rs" note="scenarios: several steps in one browser" />
<LayoutFile name="switching.rs" note="switching windows and frames" />
<LayoutFile name="tour.rs" note="tours" />
<LayoutFile name="traits.rs" note="BrowserApi, ElementApi, AssertionApi and ScreenshotApi" />
</LayoutDir>
</LayoutTree>

{/* layout-tree:end */}

### `src/browser/`

Launch and session plumbing:

{/* layout-tree:start src/browser */}

<LayoutTree>
<LayoutDir name="browser" note="BrowserConfig, the launcher, the WebDriver session, driver access and downloads">
<LayoutFile name="binaries.rs" note="finding browser binaries" />
<LayoutFile name="chrome_for_testing.rs" note="Chrome for Testing builds" />
<LayoutFile name="chromium_snapshots.rs" note="Chromium snapshot builds" />
<LayoutFile name="config.rs" note="BrowserConfig, Browser and DriverMode" />
<LayoutFile name="downloader.rs" note="downloading a browser or a driver" />
<LayoutFile name="downloads.rs" note="the downloaded_files folder browsers save into" />
<LayoutFile name="driver_access.rs" note="repairs and runs drivers" />
<LayoutFile name="driver_install.rs" note="installs geckodriver and msedgedriver" />
<LayoutFile name="driver_version.rs" note="matches chromedriver to Chrome" />
<LayoutFile name="launcher.rs" note="chromedriver discovery, startup and StealthOptions" />
<LayoutFile name="mod.rs" note="the browser module" />
<LayoutDir name="options" open note="typed launch settings">
<LayoutFile name="capabilities.rs" note="capabilities added to the session" />
<LayoutFile name="cdp_events.rs" note="which DevTools events are logged" />
<LayoutFile name="chromium_args.rs" note="Chromium switches" />
<LayoutFile name="device.rs" note="mobile device emulation" />
<LayoutFile name="driver.rs" note="driver settings" />
<LayoutFile name="extensions.rs" note="extensions to load" />
<LayoutFile name="firefox.rs" note="Firefox arguments and preferences" />
<LayoutFile name="locale.rs" note="the locale" />
<LayoutFile name="mod.rs" note="the options module" />
<LayoutFile name="page_load.rs" note="how long a navigation waits" />
<LayoutFile name="plugins.rs" note="plugins turned on for the browser" />
<LayoutFile name="proxy.rs" note="the proxy" />
<LayoutFile name="remote.rs" note="a remote WebDriver server" />
<LayoutFile name="switches.rs" note="browser switches" />
<LayoutFile name="virtual_display.rs" note="a virtual display" />
<LayoutFile name="window.rs" note="window size and position" />
</LayoutDir>
<LayoutFile name="password_prompts.rs" note="password prompts" />
<LayoutFile name="plugin_hooks.rs" note="calls the plugin set from the WebDriver engine" />
<LayoutFile name="proxy_auth.rs" note="proxy authentication" />
<LayoutFile name="reuse.rs" note="reused sessions and --crumbs" />
<LayoutDir name="session" open note="BrowserSession, the WebDriver wrapper and the CDP session cache">
<LayoutFile name="capabilities.rs" note="the capabilities each browser is asked for" />
<LayoutFile name="cdp.rs" note="DevTools commands of the session" />
<LayoutFile name="connect.rs" note="starting a session, a leased one, or a direct CDP attachment" />
<LayoutFile name="elements.rs" note="finding and using elements" />
<LayoutFile name="emulation.rs" note="the ad block and the phone emulation" />
<LayoutFile name="fingerprint.rs" note="the fingerprint of the session" />
<LayoutFile name="launch.rs" note="the driver, the browser program and the virtual screen" />
<LayoutFile name="lifecycle.rs" note="reconnect, quit and the cleanup on drop" />
<LayoutFile name="mod.rs" note="the BrowserSession struct and its accessors" />
<LayoutFile name="navigation.rs" note="opening pages and moving through history" />
<LayoutFile name="page.rs" note="the page: title, source, scripts" />
<LayoutFile name="plugins.rs" note="browser plugins (feature plugins)" />
<LayoutFile name="waits.rs" note="waiting for elements and text" />
</LayoutDir>
<LayoutFile name="shadow_find.rs" note="finding elements in a shadow root" />
<LayoutFile name="virtual_display.rs" note="opens a virtual display on Linux" />
</LayoutDir>
</LayoutTree>

{/* layout-tree:end */}

### `src/evasion/`

Anti-detection layer:

{/* layout-tree:start src/evasion */}

<LayoutTree>
<LayoutDir name="evasion" note="fingerprints, evasion providers, humanize, binary patching and the CDP reactor">
<LayoutFile name="behavior.rs" note="human-like input" />
<LayoutFile name="capture.rs" note="CapturedFingerprint: reading a real browser fingerprint" />
<LayoutFile name="cdp.rs" note="the DevTools wrapper used from WebDriver" />
<LayoutFile name="dprocess.rs" note="detached chromedriver and browser processes" />
<LayoutFile name="evasions.rs" note="launch flags for the evasion layer" />
<LayoutDir name="fingerprint" open note="parts of fingerprint.rs">
<LayoutFile name="compare.rs" note="compares two fingerprints" />
<LayoutFile name="file.rs" note="reads and writes a fingerprint file" />
<LayoutFile name="schema.rs" note="the fingerprint JSON schema" />
</LayoutDir>
<LayoutFile name="fingerprint.rs" note="Fingerprint, one Spoof per setting, ProxyConfig and ClientHints" />
<LayoutFile name="humanize.rs" note="Bezier mouse paths, keystroke timing and the seedable Rng" />
<LayoutDir name="js" open note="page scripts the evasion layer ships">
<LayoutFile name="active_css_js.rs" note="the active CSS script" />
<LayoutFile name="best_selector.js" note="picks a selector for an element" />
<LayoutFile name="capture.js" note="reads a page fingerprint" />
<LayoutFile name="capture_js.rs" note="the capture script as Rust" />
<LayoutFile name="mod.rs" note="the js module" />
<LayoutFile name="recorder_js.rs" note="the recorder script as Rust" />
</LayoutDir>
<LayoutFile name="mod.rs" note="the evasion module" />
<LayoutFile name="options.rs" note="StealthOptions: launch arguments and preferences" />
<LayoutFile name="patcher.rs" note="ChromedriverPatcher and EnginePatch" />
<LayoutFile name="policy.rs" note="StealthPolicy" />
<LayoutDir name="providers" open note="the evasion registry and the built-in providers">
<LayoutFile name="builtin.rs" note="the built-in providers" />
<LayoutFile name="mod.rs" note="EvasionRegistry and default_registry" />
</LayoutDir>
<LayoutFile name="randomize.rs" note="the tables behind Fingerprint::randomized; the method is the API" />
<LayoutFile name="reactor.rs" note="the background CDP Fetch interceptor" />
<LayoutFile name="settings.rs" note="value types such as ScreenSize and WebGl" />
<LayoutFile name="spoof.rs" note="Spoof, one value and how it is applied" />
<LayoutFile name="uc.rs" note="undetected-chromedriver mode" />
</LayoutDir>
</LayoutTree>

{/* layout-tree:end */}

### `src/profile_payloads/`

Parses external anti-detect profile JSON into `BrowserConfig` + `Fingerprint`.
The mapping is explicit: every JSON field is declared in `ProfileParams`, then
converted in `ProfileParams::to_browser_config()` and `browser()`.

{/* layout-tree:start src/profile_payloads */}

<LayoutTree>
<LayoutDir name="profile_payloads" note="parses external browser-profile JSON into a Fingerprint">
<LayoutFile name="mod.rs" note="the profile_payloads module" />
<LayoutFile name="profile.rs" note="ProfileParams: every JSON field, then to_browser_config and browser" />
</LayoutDir>
</LayoutTree>

{/* layout-tree:end */}

### `src/cli/`

The library half of the command line. The binaries, the MCP server and the golden
tests share it. It has no command-line parser.

{/* layout-tree:start src/cli */}

<LayoutTree>
<LayoutDir name="cli" note="the library half of sbrs, with no parser: commands.rs, scaffold.rs, scripts/">
<LayoutFile name="commands.rs" note="what each sbrs command does, with no parser" />
<LayoutFile name="devtools.rs" note="Chrome with an open DevTools endpoint" />
<LayoutFile name="fingerprint_cmd.rs" note="sbrs fingerprint" />
<LayoutFile name="mod.rs" note="the cli module" />
<LayoutFile name="plugin_cmd.rs" note="sbrs plugin list and info, and the check of plugin names" />
<LayoutFile name="prompt.rs" note="the shell prompt line" />
<LayoutFile name="scaffold.rs" note="sbrs mkdir and mkfile" />
<LayoutDir name="scripts" open note="shared by the binaries, the MCP server and the golden tests">
<LayoutFile name="all_scripts.rs" note="the script registry" />
<LayoutFile name="behave_gui_tui.rs" note="the behave-gui terminal app" />
<LayoutFile name="behave_options.rs" note="how Python behave options map to sbrs test" />
<LayoutFile name="case_plans.rs" note="case plans" />
<LayoutFile name="caseplans_tui.rs" note="the caseplans terminal app" />
<LayoutFile name="discovery.rs" note="finds tests" />
<LayoutFile name="feature_files.rs" note="Gherkin feature files" />
<LayoutFile name="gherkin_outline.rs" note="Gherkin outlines" />
<LayoutFile name="logo_helper.rs" note="the logo" />
<LayoutFile name="mod.rs" note="the scripts module" />
<LayoutFile name="recorder_tui.rs" note="the recorder terminal app" />
<LayoutFile name="rich_helper.rs" note="terminal formatting" />
<LayoutFile name="run.rs" note="runs a selection of tests" />
<LayoutFile name="run_filter.rs" note="the -k and -m filters" />
<LayoutFile name="run_logs.rs" note="logs of a test run" />
<LayoutFile name="sb_behave_gui.rs" note="sbrs behave-gui" />
<LayoutFile name="sb_caseplans.rs" note="sbrs caseplans" />
<LayoutFile name="sb_commander.rs" note="sbrs commander" />
<LayoutFile name="sb_convert.rs" note="sbrs convert" />
<LayoutFile name="sb_grid.rs" note="sbrs grid-hub and grid-node" />
<LayoutFile name="sb_install.rs" note="sbrs install" />
<LayoutFile name="sb_mkchart.rs" note="sbrs mkchart" />
<LayoutFile name="sb_mkdir.rs" note="sbrs mkdir" />
<LayoutFile name="sb_mkfile.rs" note="sbrs mkfile" />
<LayoutFile name="sb_mkpres.rs" note="sbrs mkpres" />
<LayoutDir name="sb_objectify" open note="turns selectors in a Rust test into page-object constants">
<LayoutFile name="api.rs" note="which calls take a selector" />
<LayoutFile name="edit.rs" note="rewrites the test" />
<LayoutFile name="lexer.rs" note="tokenizes a Rust test" />
<LayoutFile name="model.rs" note="the page objects" />
<LayoutFile name="scan.rs" note="finds the selectors" />
<LayoutFile name="tests.rs" note="tests of objectify" />
</LayoutDir>
<LayoutFile name="sb_objectify.rs" note="sbrs objectify, extract-objects, inject-objects and revert-objects" />
<LayoutFile name="sb_print.rs" note="sbrs print" />
<LayoutFile name="sb_recorder.rs" note="sbrs recorder" />
<LayoutFile name="sb_translate.rs" note="sbrs translate" />
<LayoutFile name="tui_kit.rs" note="shared pieces of the terminal apps" />
</LayoutDir>
</LayoutDir>
</LayoutTree>

{/* layout-tree:end */}

### `crates/cli/`

The workspace package `seleniumbase-rs-cli`, which builds the two binaries and
owns the dependencies only they use (the `usage` command-line parser).

{/* layout-tree:start crates/cli */}

<LayoutTree>
<LayoutDir name="cli" open note="the package seleniumbase-rs-cli: the sbrs and sbrs-mcp binaries">
<LayoutFile name="Cargo.toml" note="default features: pdf, mcp-server, mcp-client, plugins-builtin" />
<LayoutDir name="src" open>
<LayoutFile name="sbrs.rs" highlight note="the sbrs command line" />
<LayoutFile name="mcp_server.rs" highlight note="the sbrs-mcp server" />
<LayoutFile name="fig.rs" note="the Fig completion spec" />
<LayoutFile name="report.rs" note="sbrs report" />
<LayoutFile name="reference.rs" note="renders docs/help/cli_reference.md from the usage spec" />
<LayoutFile name="strict.rs" note="refuses, with status 2, what the parser is too loose about" />
<LayoutFile name="trace.rs" note="sbrs trace show: opens a trace folder's viewer" />
<LayoutDir name="mcp_install" open note="sbrs-mcp install, uninstall and clients">
<LayoutFile name="apply.rs" note="writes one client's config" />
<LayoutFile name="args.rs" note="the install, uninstall and clients command line" />
<LayoutFile name="client.rs" note="the clients and where each keeps its config" />
<LayoutFile name="entry.rs" note="the two server entries" />
<LayoutFile name="env.rs" note="the machine, faked by tests" />
<LayoutFile name="external.rs" note="the claude and codex programs" />
<LayoutFile name="fsx.rs" note="atomic writes and the backup" />
<LayoutFile name="jsonedit.rs" note="edits JSON as text" />
<LayoutFile name="jsonfile.rs" note="the JSON layouts" />
<LayoutFile name="mod.rs" note="install, uninstall and clients" />
<LayoutFile name="report.rs" note="what one client prints" />
<LayoutFile name="tests.rs" note="tests against a temporary folder" />
<LayoutFile name="tomlfile.rs" note="edits Codex config.toml" />
</LayoutDir>
<LayoutDir name="args" open note="argument structs of the sbrs subcommands">
<LayoutFile name="content.rs" note="convert and the content commands" />
<LayoutFile name="generators.rs" note="mkdir, mkfile, mkchart, mkpres" />
<LayoutFile name="install.rs" note="sbrs install" />
<LayoutFile name="language.rs" note="the chart and presentation languages" />
<LayoutFile name="mod.rs" note="the args module" />
<LayoutFile name="objects.rs" note="extract-objects, inject-objects, objectify, revert-objects" />
<LayoutFile name="report.rs" note="sbrs report finish and, with the turso feature, report history" />
<LayoutFile name="tools.rs" note="the smaller tool commands" />
</LayoutDir>
<LayoutFile name="browser_flags.rs" note="the browser options, their SB_ variables and the settings file" />
<LayoutFile name="completion.rs" note="the patched shell completion scripts" />
<LayoutFile name="crypto.rs" note="sbrs encrypt and decrypt" />
<LayoutFile name="diagnostic.rs" note="the miette report printed when a command fails" />
<LayoutFile name="failure.rs" note="a command line that does not parse prints the command page, then the error" />
<LayoutFile name="fingerprint.rs" note="sbrs fingerprint" />
<LayoutFile name="history.rs" note="sbrs report history" />
<LayoutFile name="info.rs" note="sbrs version and the info commands" />
<LayoutDir name="mcp_client" open note="sbrs mcp, the MCP client">
<LayoutFile name="args.rs" note="the client command line" />
<LayoutFile name="mod.rs" note="the mcp_client module" />
<LayoutDir name="output" open note="how a tool result is printed">
<LayoutFile name="tests.rs" note="tests of that printing" />
</LayoutDir>
<LayoutFile name="output.rs" note="prints a tool result" />
</LayoutDir>
<LayoutFile name="mcp_cmd.rs" note="sbrs mcp" />
<LayoutFile name="proxy.rs" note="sbrs proxy" />
<LayoutFile name="recorder_flags.rs" note="the recorder flags" />
<LayoutFile name="test_args.rs" note="sbrs test" />
<LayoutFile name="test_session.rs" note="the test run sbrs test starts and finishes" />
</LayoutDir>
<LayoutDir name="tests" note="every test that runs the real sbrs or sbrs-mcp binary" />
</LayoutDir>
</LayoutTree>

{/* layout-tree:end */}

### `src/utilities/`

Migration and IDE tooling: Python importer, Selenium IDE parser, Selenium Grid
helpers.

{/* layout-tree:start src/utilities */}

<LayoutTree>
<LayoutDir name="utilities" note="RetryPolicy, the grid server, Selenium Grid and IDE, the Python importer">
<LayoutFile name="grid_server.rs" note="GridProcess: start, state, stop and ended" />
<LayoutDir name="importer" open note="the Python importer">
<LayoutFile name="emit.rs" note="writes the Rust test" />
<LayoutFile name="lexer.rs" note="tokenizes Python" />
<LayoutFile name="mod.rs" note="the importer module" />
<LayoutFile name="parser.rs" note="parses Python" />
<LayoutFile name="program.rs" note="a parsed program" />
<LayoutDir name="pyconv" open note="converts a Python test into Rust">
<LayoutFile name="ast.rs" note="the Python syntax the converter reads" />
<LayoutFile name="cdp.rs" note="CDP-mode calls" />
<LayoutFile name="expr.rs" note="expressions" />
<LayoutFile name="generated.rs" note="generated pieces of the converter" />
<LayoutFile name="mod.rs" note="the pyconv module" />
<LayoutFile name="parse.rs" note="the parser" />
<LayoutFile name="stmt.rs" note="statements" />
<LayoutFile name="table.rs" note="the method table" />
<LayoutFile name="tests.rs" note="tests of the converter" />
<LayoutFile name="tr.rs" note="the translation" />
<LayoutFile name="units.rs" note="units of a converted test" />
</LayoutDir>
<LayoutFile name="recorder.rs" note="imports a recording" />
<LayoutFile name="selector.rs" note="selectors in the imported test" />
<LayoutFile name="tests.rs" note="tests of the importer" />
<LayoutFile name="translate.rs" note="translates one call" />
</LayoutDir>
<LayoutFile name="mod.rs" note="the utilities module" />
<LayoutFile name="python_importer.rs" note="imports a Python SeleniumBase test" />
<LayoutFile name="retry.rs" note="RetryPolicy and RateLimiter" />
<LayoutFile name="selenium_grid.rs" note="Selenium Grid helpers" />
<LayoutFile name="selenium_ide.rs" note="the Selenium IDE parser" />
</LayoutDir>
</LayoutTree>

{/* layout-tree:end */}

## Browser session model

`BrowserSession` wraps a `thirtyfour::WebDriver` and exposes higher-level
methods such as `wait_for_element`, `text`, `click`, and `execute_script`. Most
`BaseCase` helpers delegate to the session. For CDP-only operations (such as
activating a stealth driver), `CdpDriver` and `CdpPage` provide a lightweight
Chrome DevTools Protocol client.

## Stealth architecture

The stealth system has four independent axes:

1. **Launch configuration** — `StealthOptions` emits Chromium args and prefs.
2. **Runtime JavaScript** — `EvasionProvider`s generate a bootstrap script.
3. **CDP-level overrides** — `evasions::safe_cdp_overrides()` sends
   `Network.setUserAgentOverride`, `Emulation.setDeviceMetricsOverride`,
   `Emulation.setTimezoneOverride`, `Emulation.setLocaleOverride`,
   `Emulation.setGeolocationOverride`, and related commands. When
   `StealthPolicy::native_spoofing` is enabled, providers skip JS patches for
   dimensions the browser can spoof natively, making the spoof invisible to
   page-side `toString()` / descriptor inspection.
4. **Network interception** — `StealthReactor` uses CDP `Fetch` to override
   headers and responses.
5. **Binary patching** — `ChromedriverPatcher` edits driver markers.

```mermaid
flowchart TD
    BC["BrowserConfig"]
    SO["StealthOptions"]
    FP["Fingerprint"]
    CP["ChromedriverPatcher"]
    LA["launch args"]
    BS["bootstrap script"]
    PB["patched binary"]
    SES["BrowserSession"]
    SR["StealthReactor (CDP Fetch)"]
    BC --> SO
    BC --> FP
    BC --> CP
    SO --> LA
    FP --> BS
    CP --> PB
    LA --> SES
    BS --> SES
    PB --> SES
    SES --> SR
```

Use these through `BaseCase::activate_cdp_mode(url)` or by setting
`DriverMode::Uc` in `BrowserConfig`.

### Fingerprint, settings and policy

`Fingerprint` is a value type describing the desired browser surface. It is
built once with `Fingerprint::builder()`, has private fields and read accessors,
and is changed only through `to_builder()`. It is composed of:

- `browser_type`, `os_type`, `core_version`, `seed` — the persona.
- one `Spoof<T>` per setting: `user_agent`, `platform`, `hardware_concurrency`,
  `screen`, `webgl`, `geolocation`, `timezone`, `locale`, `webrtc`, `media_devices`,
  `battery`, `client_hints`, `proxy`, and the rest of what a page can probe.
- a `StealthPolicy` — how the claims are delivered (WebRTC IP policy, canvas
  and audio noise, native spoofing, granted permissions, blocked ports and
  trackers).

A `Spoof<T>` is `Natural` (the browser's own value, the default), `Random` (a
coherent value drawn when the fingerprint is built), `Value(T)` (a plain value
converts to it) or `Off` (the API is unavailable or denied; only for geolocation,
WebRTC, media devices, battery, Bluetooth, speech voices, network information and
Client Hints). A built fingerprint holds no `Random`.

Providers, launch arguments and protocol overrides do not read the fields by
themselves: they ask `Fingerprint::is_masked(Dimension::...)`, so there is one
rule, and a provider defines only a setting that is a `Value`. A policy preset
never changes a setting.

`Fingerprint::validate()` returns a `CoherenceReport` of errors and warnings
(platform and user agent against the OS persona, a WebGL renderer from another
OS's graphics API, a locale from another country than the time zone, and so on).
Add a rule there when introducing inter-dependent fields.

### Provider execution order

Providers are sorted by priority and run only when `applies(fp)` returns true.
Current priority bands:

- 5 — infrastructure (`native_to_string`).
- 20–50 — navigator, screen, WebGL, plugins, codecs, permissions.
- 60–90 — WebDriver/headless cleanup, runtime, iframe, chrome object fixes.
- 100–130 — self-defense, notification, memory, battery, connection, WebRTC.

Lower numbers run first. A provider that makes replacements look native starts
its snippet with `providers::SB_PREAMBLE` and keeps its priority above 5, because
the `native_tostring` provider (priority 5) installs the helpers that preamble
uses. They live in the scope of the assembled script, not on `window`.

### CDP reactor

`StealthReactor` starts a background task that attaches to a CDP target and
listens for `Fetch.requestPaused` events. It can:

- Override request headers (`User-Agent`, `Accept-Language`, etc.).
- Block or mock responses.
- Intercept JS/CSS resources to inject the stealth bootstrap.

Add new interceptor rules in `reactor.rs` by matching on request URL patterns
and emitting `Fetch.continueRequest` / `FulfillRequest` calls.

### Binary patching

`ChromedriverPatcher` applies byte-level search/replace patches to a driver
binary. Each patch is an `EnginePatch`:

```rust An EnginePatch
EnginePatch {
    id: "cdc_marker",
    description: "Replace cdc_... markers",
    search: b"$cdc_",
    replace: b"$sbc_",
}
```

Patches run after the driver is downloaded or located. Backups are written with
`.sb-backup` extension and restored on mismatch. Add marker-detection tests
whenever a patch targets a new driver version.

### Adding new spoofing code

<Steps>

<Step title="A new JavaScript evasion" icon="code">

If the change is a new JavaScript evasion, add a provider in
`src/evasion/providers/builtin.rs` and register it in `all()`.

</Step>

<Step title="Launch arguments" icon="terminal">

If it changes launch arguments, update `StealthOptions::apply_to` in
`src/evasion/options.rs` or add a helper such as `engine_spoofing_args()` in
`src/evasion/patcher.rs`.

</Step>

<Step title="Binary patches" icon="microchip">

If it patches the binary, add an `EnginePatch` in
`src/evasion/patcher.rs`.

</Step>

<Step title="Re-export" icon="share-from-square">

Re-export new public types from `src/evasion/mod.rs` and `src/lib.rs`.

</Step>

<Step title="Test it" icon="flask">

Add a unit test that exercises the new logic without requiring a live browser.

</Step>

</Steps>

## Adding a macro

Macros are defined in `src/macros.rs` and re-exported at the crate root via
`#[macro_export]`:

<Steps>

<Step title="Define it" icon="code">

Add the macro definition in `src/macros.rs` using `$crate::` for any path.

</Step>

<Step title="Document it in code" icon="comment">

Include a short doc comment with an `ignore` example.

</Step>

<Step title="Test it" icon="flask">

Add a unit test in the `#[cfg(test)]` module if the macro builds a value that
can be asserted without a browser.

</Step>

<Step title="Document it" icon="book">

Update `docs/tutorials/utilities.md` with the macro name, signature, and example.

</Step>

</Steps>

## Adding an MCP tool

The `sbrs-mcp` binary (`--server cdp|driver|sb|dev`) is a thin wrapper over
the `mcp` module, built with the `mcp-server` feature. A server is a
`Host<S>`: a list of `ToolDef`s and the browser session `S` they share.

<Steps>

<Step title="Pick the server file" icon="server">

Pick the server folder: `src/mcp/cdp/` (the Pure CDP engine),
`src/mcp/driver/` or `src/mcp/sb/` (WebDriver, sharing the handlers in
`src/mcp/webdriver.rs`), or, for a tool that needs no browser, the `dev` server in
`src/mcp/dev/` (`api.rs`, `author.rs`, `help.rs`, `cli.rs`, `diagnose.rs`). Every tool costs
context in every session that loads its server, so a tool for the library or the
command line goes to `dev`, not to a browser server. Tools shared by servers are
defined once in `src/mcp/devtools.rs` and registered by name.

</Step>

<Step title="Write the handler" icon="code">

Write an `async fn(Ctx<S>, Args) -> Result<Output, ToolError>` handler. Read
arguments with the typed accessors on `Args` (`str`, `opt_bool`, `choice`,
`seconds_or`, ...), which name the argument in every error. Get the session
with `ctx.session().await?`. Return `Output::Text` or `Output::Json`.

</Step>

<Step title="Register it" icon="list-ol">

Register a `ToolDef::new(name, title, description, Effect, Schema, handler)`.
Choose the `Effect` that is true of the tool (it becomes the MCP annotation
hints) and describe each argument with `Prop`.

A server's tools are grouped by what they do, one file per group in the server's
folder (`session.rs`, `navigation.rs`, `page.rs`, `input.rs`, `storage.rs`,
`windows.rs`, ...). Each file has a `tools()` function, and the `tools()` of the
folder's `mod.rs` joins them in the order clients see. Add the `ToolDef` to the
file of its group, or add a file and list it in `mod.rs` where the tool belongs
in that order.

</Step>

<Step title="Write files safely" icon="folder-closed">

Write files only through `ctx.settings().output_path(folder, name)`, which
keeps them inside the output directory.

</Step>

<Step title="Test it" icon="flask">

Test it: `tests/mcp_cdp.rs` shows how to drive a tool against the mock
browser; `tests/mcp_webdriver.rs` covers the catalogues. `tests/mcp_tool_lists.rs`
compares what `tools/list` answers on each server (names in order, descriptions,
schemas, annotations) with `tests/golden/mcp_tools/*.json`; a new or changed tool
fails it until you write the snapshots again with
`UPDATE_GOLDEN=1 cargo test --features mcp-server --test mcp_tool_lists` and
commit them. A `dev` tool is tested in
its own file with `Host::call`, and `install_driver` through
`mcp::dev::host_with_installer` with a fake downloader, never the network.

The `dev` tool `find_api` searches `src/mcp/dev/api_index.json`, which a test
generates from the public methods of `BaseCase` and of the pure CDP `Browser`,
`Page`, `Locator` and `BrowserContext`. After you add, remove or change the
signature or the first doc paragraph of such a method, a test fails until you
write the index again with
`SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index`
and commit it.

The `dev` tool `cli_help` reads `completions/sbrs.usage.kdl`, the usage spec of the
command line, with the small KDL reader in `src/mcp/dev/spec.rs` (the library may
not depend on a command-line crate, so it cannot use `usage-lib`). A new command or
flag shows up in `cli_help` as soon as the spec is written again, with
`SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`.
The reader has its own tests, and one of them reads the real spec and counts its
commands and flags against the text.

</Step>

<Step title="Check the parity" icon="scale-balanced">

If the tool exists upstream, `tests/parity.rs` checks it against
`parity/api.toml`; see [Syncing with upstream](/UPSTREAM_SYNC).

</Step>

</Steps>

## Testing guidelines

- Add unit tests for pure helper logic (selectors, translations, HTML parsing,
  retry and rate-limit policies, JS builders).
- Avoid tests that require a live browser in the default suite; mark them with
  `#[ignore]` or place them under `tests/`.
- Use `BaseCase::without_session(config)` to construct a `BaseCase` for testing
  helpers that do not touch the browser.
- Use `run_browser_test` for async browser tests so cleanup is awaited after
  either success or failure. `Drop` cannot perform async WebDriver cleanup.
- The default nextest profile sets retries to zero. Add retries only around a
  measured, transient, and idempotent operation rather than masking a flaky
  test.
- Run the full verification matrix before pushing:

```bash Terminal
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --features s3,azure,gcp,playwright,mcp-server
cargo test --features s3,azure,gcp,playwright,mcp-server
cargo clippy --all-targets --features s3,azure,gcp,playwright,mcp-server -- -D warnings
cargo publish --dry-run
```

The checks CI runs today, the low-memory build settings and the tests that need
a real Chrome are in [Build from source](/contributing/from-source#run-the-checks).

## Tracing internals

The crate uses `tracing` with structured fields. Spans are created around major
operations (launch, open, click, patch). Events include:

- `seleniumbase.driver.launch` — browser/driver launch.
- `seleniumbase.stealth.patch` — binary patch applied.
- `seleniumbase.error` — structured error event with `error.category`,
  `error.transient`, and `error.hint`.

To add tracing to a new component:

```rust Instrumenting a component
use tracing::{info, instrument};

#[instrument(skip_all, fields(component = "my_component"))]
pub async fn do_work(&self) -> crate::Result<()> {
    info!("starting work");
    // ...
}
```

For JSON output, for ingestion by log analytics, set `SB_LOG_FORMAT=json` or call
`init_tracing_json`.

## Profile payload mapping

External profile payloads are normalized in `src/profile_payloads/profile.rs`.
The flow is:

```mermaid
flowchart LR
    J["JSON file"] --> P["ProfileParams"]
    P -->|"to_fingerprint()"| F["Fingerprint"]
    P -->|"to_browser_config()"| B["BrowserConfig (holds that Fingerprint)"]
```

When adding a new JSON field:

<Steps>

<Step title="Declare it" icon="code">

Add it to the corresponding `ProfileParams` struct with `#[serde(default)]`.

</Step>

<Step title="Convert it" icon="arrow-right-arrow-left">

Convert it in `fingerprint_builder()` (one `Spoof` per setting, through
`read` and `settle`), so the flag decides its state.

</Step>

<Step title="Leave launch-time settings to the fingerprint" icon="rocket">

Launch-time settings (`to_browser_config()`, `apply_runtime_overrides()`) read
the built fingerprint, never the raw payload, so a `natural` flag launches
nothing of that setting.

</Step>

<Step title="Test it" icon="flask">

Add a parser test and a round-trip coherence test.

</Step>

</Steps>

Keep parser logic defensive: unknown fields should not fail; invalid values
should produce `SeleniumBaseError::InvalidConfig` with the field name.

## Test patterns

### Pure-logic tests

```rust Pure-logic test
#[test]
fn selector_parses_link_text() {
    let s = Selector::LinkText("Dashboard".into());
    assert_eq!(s.to_by(), By::LinkText("Dashboard".to_string()));
}
```

### Async tests without browser

```rust Async test without a browser
#[tokio::test]
async fn a_value_turns_its_dimension_on() -> Result<(), FingerprintError> {
    let fp = Fingerprint::builder().timezone("Europe/Berlin").build()?;
    assert!(fp.is_masked(Dimension::Timezone));
    assert!(!fp.is_masked(Dimension::Screen));
    Ok(())
}
```

### Browser lifecycle tests

Use `run_browser_test` from `src/api/runner.rs`:

```rust Browser lifecycle test
use seleniumbase_rs::{run_browser_test, tokio, BrowserConfig};

#[tokio::test]
async fn example_test() -> seleniumbase_rs::Result<()> {
    run_browser_test(BrowserConfig::default(), async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_title_contains("Example").await?;
        Ok(())
    }).await
}
```

<Warning>

`run_browser_test` guarantees cleanup runs after either success or failure.
`Drop` cannot await async cleanup, so never construct and drop a browser in a
synchronous test.

</Warning>

### Marking slow/flaky tests

Place browser-backed tests in `examples/` or under `tests/` and gate them with a
feature flag. Do not add retries to unit tests unless the operation is measured
to be transient and idempotent.

### Real-browser integration tests

`tests/browser_smoke.rs` exercises the end-to-end path against a real Chrome
instance: navigation, assertions, typing, screenshots, and deferred asserts.
The tests are `#[ignore]`d by default and serialized with a shared lock so
parallel test threads do not race to bind chromedriver's port.

<CodeGroup>

```bash Local
# Locally, with Chrome and chromedriver installed:
cargo test --test browser_smoke -- --ignored
```

```bash Remote grid
# Against a remote grid instead of the auto-started local chromedriver:
SB_WEBDRIVER_URL=http://grid:4444 cargo test --test browser_smoke -- --ignored
```

</CodeGroup>

The `browser-smoke` job in `.github/workflows/ci.yml` runs this suite on every
push using the Chrome/chromedriver preinstalled on GitHub-hosted runners.

## Performance considerations

- Avoid allocating large strings in hot paths. `EvasionProvider::script` runs
  once per session, so moderate allocation is fine, but do not read files inside
  the provider loop.
- Use `tokio::spawn` for independent background work (e.g. the CDP reactor).
- Cache CDP sessions and WebDriver clients rather than reconnecting.
- For chart/tour generation, build the DOM with `std::fmt::Write` instead of
  repeated string concatenation.
- Run `cargo build --release` when benchmarking; debug builds are 10–100× slower.

How to run the harnesses in `benches/`, and compare a change fairly, is in
[Benchmarking](/contributing/benchmarking).

## Security considerations

- Never hardcode secrets. `BrowserConfig` proxy passwords and cloud credentials
  must come from environment variables or a secrets manager.
- Validate all user-provided URLs before navigation to prevent SSRF.
- Do not deserialize untrusted data with `pickle`-equivalent formats. Profile
  payloads use `serde_json` with explicit structs.
- The MCP server runs browser automation on behalf of clients. It should only be
  exposed to trusted local clients; see `docs/help/mcp_server.md` for the trust
  boundary.
- Binary patches operate on local files. Always create backups and verify
  checksums before writing.

## Debugging tips

- Set `RUST_LOG=seleniumbase_rs=debug` to see driver launch args and CDP traffic.
- Use `RUST_BACKTRACE=1` for the backtrace of a panic.
- Run `sbrs doctor` to print environment diagnostics: the `SB_*` settings, the Chrome it found, and the chromedriver it found with its version next to Chrome's.
- Inspect generated bootstrap with `sbrs stealth-bootstrap --fingerprint ...`.
- For CI-only failures, reproduce with `--features s3,azure,gcp,playwright,mcp-server`
  because feature gates change code paths.

A full walkthrough, from the logs to a mocked browser and the driver checks, is
in [Debugging](/contributing/debugging).

## Python importer architecture

`utilities::python_importer` is a conservative static converter for Python
SeleniumBase and Selenium WebDriver tests. It never executes Python and never
guesses a dynamic value. `import_python` is the entry point and has two readings:

- The SeleniumBase reading (`utilities/importer/pyconv/`) parses the file with a
  small Python parser (`parse.rs`, on the lexer the Playwright importer shares)
  into the statements and expressions of `ast.rs`, finds the tests (`units.rs`:
  classes with `test*` methods, `test*` functions, `with SB(...)` blocks and
  scripts) and translates each (`stmt.rs` for statements, `expr.rs` for values and
  calls, `cdp.rs` for CDP Mode and `sb_cdp.Chrome()` scripts, which become pure CDP
  tests). `mod.rs` writes the file in the rstest `sb` fixture form that
  `sbrs mkfile` writes.
- The Selenium reading (`driver.find_element(...)`) is the older line scanner of
  `python_importer.rs`; it writes into the same fixture form.

The Python methods and their Rust calls are one table, `table.rs`, made of the
hand-written entries and `generated.rs`, which `pyconv::tests` writes from the
signatures of `BaseCase`, the `implemented` rows of `parity/api.toml` and the
Python parameters of `parity/upstream-api.json`
(`SB_UPDATE_GENERATED=1 cargo test -p seleniumbase-rs --lib the_generated_table`
writes it again). A test checks every entry against the real signature (argument
count and kinds, `async`, `Result`, call builders and their methods) and against
the parity row, so a `BaseCase` change that breaks a mapping fails a test. The
`FIX-ASSERTS` shapes of `assert_in`, `wait_for_element` and `get_select_options`
are in the table; adjust the table, not the translator, when a signature moves.

`tests/golden/import_python/` holds the Python examples of
`parity/examples.toml` and what the importer writes for each. A test checks the
importer still writes the `.rs.golden` files (`SB_UPDATE_GOLDENS=1` writes them
again) and that every `sb.method(..)` call in them is a `BaseCase` method with that
many arguments; a `cfg(doctest)` item in `python_importer.rs` compiles all of them
as a user's crate would (`cargo test -p seleniumbase-rs --features testing --doc
ImportGoldensCompile`).

Unsupported statements produce source-located diagnostics and compiling `TODO`
comments. Keep that behavior when adding mappings: uncertain conversion must
remain visible rather than silently changing test semantics. A statement the
importer does not read is a warning; a call it knows but cannot write as the
Python wrote it (an argument Rust has no equal for, a value of the wrong kind,
an undefined name) is an error. Add a unit test for each new mapping, and a
golden when it changes what an example becomes.

## MCP server

The `sbrs-mcp` binary exposes a subset of `BaseCase` through the Model
Context Protocol. It is built by the package `seleniumbase-rs-cli`
(`crates/cli/src/mcp_server.rs`) when its `mcp-server` feature is enabled, which
it is by default. See `src/mcp/` for the tool definitions and `README.md` for
client configuration examples.

## Publishing

The crate is published from the crate-only tree (`seleniumbase-rs/main`). All
dependencies must come from crates.io (no git-only dependencies). Run
`cargo publish --dry-run` before tagging a release.

## Contributing

See `CONTRIBUTING.md` for the full contributor guide. In short:

1. Keep the public API backward-compatible when possible.
2. Follow the existing module organization.
3. Write doc comments for public items.
4. Update `README.md`, `DOCS.md`, and `docs/tutorials/` when adding user-facing
   features.
5. Run `cargo fmt` before committing.

## See also

<Columns cols={2}>

<Card color="#1ca1c7" title="ABI & API Stability" icon="file-contract" href="/ABI_API">

The public API contract.

</Card>

<Card color="#d32a61" title="Contributing" icon="code-branch" href="/CONTRIBUTING">

The full contributor guide.

</Card>

<Card color="#693acf" title="Rust Test Tooling" icon="flask" href="/rust-test-tooling">

Writing tests against `BaseCase`.

</Card>

<Card color="#d5a910" title="Build from source" icon="hammer" href="/contributing/from-source">

The toolchain, the checks and the real-Chrome tests.

</Card>

</Columns>

### AI agent conventions

This repository is designed to be agent-friendly:

- Public APIs are typed explicitly; use types as hints.
- Each module has a focused responsibility; do not add unrelated logic.
- When extending stealth, providers are the preferred plugin point.
- When adding docs, mirror the existing structure in `docs/SUMMARY.md`.
- Run the verification matrix after any non-trivial change.
- `COPILOT.md` contains additional guidance for autonomous work.
