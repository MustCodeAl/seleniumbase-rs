# seleniumbase-rs: knowledge map

A map of the repository for a coding assistant. It says where things live, how to
build and check them, and what has bitten before. Keep it short and correct;
fix a line here when the code stops matching it. Human contributors want
`CONTRIBUTING.md` and `docs/DEVELOPER_GUIDE.md`.

## How to work here (read this first)

- Start from the module map below rather than a repository-wide search, and use
  rust-analyzer (or `cargo check`) for types and references before a full build.
- Check a change with the commands under *Build and check*; count failures from
  the `test result` lines, never from a filtered search of the output.
- Tests that start a real browser are `#[ignore]`d `*_chrome.rs` files: run them one
  binary at a time. Mock-driven tests need no browser.
- Builds are heavy; on a machine with little memory use the environment under
  *Build and check* and one cargo command at a time.
- Commit small, one concern per commit; a bug fix comes with a test that fails
  without it.

## What this is

A Rust port of Python SeleniumBase (tracks 4.55.2; `parity/api.toml` records the
status of every Python API, `parity/upstream.toml` the upstream version,
`docs/UPSTREAM_SYNC.md` the process). Crate `seleniumbase-rs`, MIT, edition 2021,
`rust-version = "1.99"` (the newest stable; the `msrv` CI job and the
`Dockerfile` base image must move with it). Binaries: `sbrs` (CLI) and
`sbrs-mcp` (MCP server, feature `mcp-server`). Default features are
empty; `gui`, `tui`, `turso`, `rustwright`, `plugins`, `ocr`, `visual` are opt-in. The
experimental stealth layer (wire formats, `BrowserScraper`, `HttpSession`,
`MitmProxy`, `StealthSession`; formerly the features `scrape` and `impersonate`)
moved out in 0.3.0 to the workspace
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
(crates `seleniumbase-rs-codec`, `-scrape`, `-mimic`), which uses only the
public API of this crate: keep items it imports public (`stealth::randomize`,
`sb_cdp`).

## Two engines

| Engine | Where | Notes |
|---|---|---|
| WebDriver (thirtyfour) | `src/browser/`, `src/api/` (`BaseCase`) | Classic Selenium API plus the stealth/UC layer in `src/stealth/`. |
| Pure CDP, no driver | `src/sb_cdp/` | Talks to Chrome directly. `Browser`, `Page`, `Locator`, `BrowserContext`, `BrowserPool`, `SessionStore`, `Identity`, request interception, a mock transport (`Browser::new_mocked`, `MockCtrl`) for tests without Chrome. |

## Module map (`src/`)

| Path | Role |
|---|---|
| `api/` | `BaseCase` (`base_case.rs`) and the capability traits in `traits.rs` (`BrowserApi`, `ElementApi`, `AssertionApi`, `ScreenshotApi`, selectors passed as `css: &str`). `base_case_impls/` splits its methods by theme (`_dom`, `_nav`, `_mouse`, `_storage`, `_tours`, ...). Also `recorder`, `scenario`, `presentation`, `master_qa`, `chart`, `tour`, `pdf`, `html`, `html_inspector`, `dialog`, `gui`, `cdp_driver`/`cdp_page`, `runner` (`run_browser_test`). |
| `browser/` | `BrowserConfig`/`Browser`/`DriverMode` (`config.rs`), `BrowserSession` (`session.rs`), `launcher`, `driver_access` (repairs and runs drivers), `downloader`, `playwright`. |
| `sb_cdp/` | The pure CDP engine: `browser`, `page`, `locator`, `context`, `client` (protocol), `launch` (`LaunchOptionsBuilder`), `pool` (`BrowserPool`, `PoolOptions`), `session_store`, `state` (`Cookie`, storage), `identity` (`Identity`), `intercept`, `input`, `human`, `captcha`, `download` (`Page::download_file`, `download_by_click`, `save_file_as`, `assert_downloaded_file`), `proxy_auth`, `webrtc`, `har` (`Page::start_har`, `HarRecorder`, the HAR 1.2 serde types), `expect`, `mock`, `helper.js`. Held to the pedantic lint set. |
| `stealth/` | `fingerprint.rs` (`Fingerprint`, `StealthFlags`, `ProxyConfig`, `ClientHints`), `providers/` (`EvasionRegistry`, `default_registry`, `builtin.rs`), `randomize`, `behavior` and `humanize` (human-like input), `patcher` (Chrome/chromedriver binary patching), `reactor`, `uc`, `evasions` (launch flags), `options`, `cdp` (DevTools wrapper for WebDriver), `js/`. |
| `profile_payloads/` | Parses external browser-profile JSON into a `Fingerprint`. |
| `mcp/` | MCP server (`tool`, `host`, `webdriver`, `cdp`, `stealth`, `schema`); entry `src/bin/mcp_server.rs`. Held to the pedantic lint set. |
| `cli/` | `bin/sbrs.rs` (clap), `commands.rs`, `scaffold.rs`, `scripts/` (`sb_grid`, `sb_mkdir`, `sb_mkfile`, `sb_recorder`, `sb_commander` needs `tui`, ...), `bin/report.rs`. |
| `behave/` | Gherkin runner and step registry. |
| `plugins/` | Reporting and logging plugins (S3, GCP, Azure, screenshots, page source, driver manager, observer). |
| `config/` | Settings, runtime config, `proxy_list`, `ad_block_list`. |
| `utilities/` | `retry.rs` (`RetryPolicy`, `RateLimiter`), `grid_server.rs` (`GridProcess::start/state/stop/ended`), `selenium_grid`, `selenium_ide`, `python_importer`. |
| `utils/` | `selectors`, `shadow`, `urls`, `geometry`, `extensions/` (bundled browser extensions), `translate/` (language dictionaries). |
| `common/` | `shutdown` (graceful shutdown registry), `encryption`, `obfuscate`/`unobfuscate`, `exceptions`, `decorators`. |
| `storage/` | `turso` feature: `ResultStore`, `ProfileVault`. |
| `visual.rs` | `visual` feature: `VisualBaseline` (screenshot baselines compared by SSIM, `check`, `check_page`), behind `BaseCase::assert_visual_match`. Not `check_window`, which compares the HTML layout. |
| rest | `error.rs` (`SeleniumBaseError`), `macros.rs`, `artifacts.rs`, `core/report_helper.rs`, `js_code`, `resources/`, `tracing_util.rs` (the crate logs with `tracing`). |

Elsewhere: `tests/` (integration tests; `tests/golden` is compiled into the
binary through `include_str!`), `benches/` (`cdp_latency`, `cpu`), `examples/`
(`tauri-profile-manager` has its own manifest), `docs/` (mdBook, `SUMMARY.md`),
`parity/`, `.config/nextest.toml`, `.github/workflows/` (`ci.yml`, `docker.yml`,
`completions.yml`), `Dockerfile`, `Makefile`, `justfile`, `deny.toml`.

## Build and check

Use the same environment flags everywhere so build artifacts are shared:

```bash
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all -- --check
cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
cargo test --features "mcp-server test-util"
cargo audit && cargo deny check
mdbook build
```

- Count failures from the exit code or from `test result` lines, never from a
  filtered `grep`; read the whole output.
- Tests that launch a real Chrome are named `*_chrome.rs` (plus `browser_smoke`)
  and are `#[ignore]`d. Run them one binary at a time (the nextest `chrome` group
  has one thread). Mock-driven tests (`tests/sb_cdp_*.rs`) need no browser.
- The stealth layer's gates, BoringSSL build (cmake) and Chrome tests live in
  the seleniumbase-rs-stealth repository, not here.
- Publishing: gate the exact commit, `git fetch`, confirm `origin/main` has not
  moved, fast-forward push `"$C:refs/heads/main"`, then check local equals
  remote. No force-push. Commits carry no `Co-Authored-By` or Claude attribution.

## Conventions

- Errors are `SeleniumBaseError` (thiserror). `unwrap()` only in tests.
- A public item gets a doc comment; a bug fix gets a test that fails without it.
- A new `[[bin]]`, `[[bench]]` or `[[example]]` must also be added to the
  `Dockerfile` skeleton step, or the image build fails.
- A new Python-parity API gets an entry in `parity/api.toml`.
- Tutorials in `docs/tutorials/` are not compiled; keep their code true by hand.
  Only the `sbrs mkdir`/`mkfile` golden templates are compiled as doctests.

## Gotchas

- macOS delays the first run of a freshly written script, so tests that fake
  `java` run it once with `--warm` before timing it.
- `sbrs commander` exits with status 2 and a hint when built without `tui`.
- `thirtyfour` still depends on `zip` 8.6, so two `zip` versions are in the tree.
- Real-Chrome behaviour differs between headless and headed runs; check which
  one a test uses before trusting a timing.

## Work in flight

`ROADMAP.md` lists what is done and missing. Version 0.3.0 removed the stealth
layer (the features `scrape` and `impersonate`, modules `wire`, `scrape`,
`impersonate`); it continues, experimental, in
https://github.com/MustCodeAl/seleniumbase-rs-stealth, which depends on this
crate by git. What stays here for it is the hardening of the pure CDP page
script and the public items it imports. The design history is
`docs/design/stealth_parity_plan.md` and `docs/adr/003-stealth-scraping-layout.md`;
`docs/tutorials/stealth_scraping.md` now only points to the new repository.
