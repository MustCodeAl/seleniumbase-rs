# seleniumbase-rs: knowledge map

A map of the repository for a coding assistant. It says where things live, how to
build and check them, and what has bitten before. Keep it short and correct;
fix a line here when the code stops matching it. Human contributors want
`CONTRIBUTING.md` and `docs/DEVELOPER_GUIDE.md`.

## How to work here (read this first)

- **Always use TokenSave for code.** Find a symbol with `tokensave_search`
  (`literal: true` for exact text such as `unsafe {`), a concept with
  `tokensave_context`, read with `tokensave_read` (parameter `file`; `mode: "map"`
  for a file's symbols, `mode: "lines"` with `lines: "120-180"` for a slice),
  trace with `tokensave_callers`, `tokensave_callees` and `tokensave_impact`, list
  files with `tokensave_files`. Always pass `graph_root` = the repository root.
  Edit with `tokensave_str_replace` or `tokensave_multi_str_replace`. Never use
  `grep`, `find`, `cat`, `sed` or `head` on source. The index skips `yml`,
  `golden`, `png` and `zip` files; search those with `rtk grep`.
- Run every shell command through `rtk`. The Rust variants go behind it: `rtk rg`
  searches text, `rtk fd` finds files, `rtk sd` edits text, `rtk eza` lists
  directories, `rtk procs` lists processes. Forbidden: `grep`, `egrep`, `fgrep`,
  `ug`, `rtk grep`, `find`, `bfs`, `rtk find`, `ls`, `rtk ls`, `sed`, `ps`, `cat`.
  Homebrew GNU tools only by full path under `/opt/homebrew/opt/<pkg>/libexec/gnubin`,
  never `/usr/bin`.
- Use the Rust MCP (`mcp__rust-analyzer__rust_analyzer_*`: `set_workspace` to your
  worktree, then `diagnostics`, `references`, `definition`, `hover`, `rename`)
  before running cargo; it does not take the build lock. Cargo is for the gate of a
  work package.
- Machine limits (8 GB M1, little disk): agents use `CARGO_BUILD_JOBS=1`, the shared
  `CARGO_TARGET_DIR`, one cargo command at a time, and about three cargo-using
  agents at once. Agents commit often in their own worktree and never push; the lead
  merges, gates once per batch and publishes.
- Subagents get this file injected, but the user still wants the rules above
  restated at the top of every agent prompt.

## What this is

A Rust port of Python SeleniumBase (tracks 4.55.2; `parity/api.toml` records the
status of every Python API, `parity/upstream.toml` the upstream version,
`docs/UPSTREAM_SYNC.md` the process). Crate `seleniumbase-rs`, MIT, edition 2021,
`rust-version = "1.99"` (the newest stable; the `msrv` CI job and the
`Dockerfile` base image must move with it). Binaries: `sbase` (CLI) and
`seleniumbase-mcp` (MCP server, feature `mcp-server`). Default features are
empty; `gui`, `tui`, `turso`, `rustwright` are opt-in, and so are `scrape` (pure
Rust, adds `psl`) and `impersonate` (includes `scrape`, builds BoringSSL through
wreq and btls, needs cmake).

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
| `sb_cdp/` | The pure CDP engine: `browser`, `page`, `locator`, `context`, `client` (protocol), `launch` (`LaunchOptionsBuilder`), `pool` (`BrowserPool`, `PoolOptions`), `session_store`, `state` (`Cookie`, storage), `identity` (`Identity`), `intercept`, `input`, `human`, `captcha`, `download` (`Page::download_file`, `download_by_click`, `save_file_as`, `assert_downloaded_file`), `proxy_auth`, `webrtc`, `expect`, `mock`, `helper.js`. Held to the pedantic lint set. |
| `stealth/` | `fingerprint.rs` (`Fingerprint`, `StealthFlags`, `ProxyConfig`, `ClientHints`), `providers/` (`EvasionRegistry`, `default_registry`, `builtin.rs`), `randomize`, `behavior` and `humanize` (human-like input), `patcher` (Chrome/chromedriver binary patching), `reactor`, `uc`, `evasions` (launch flags), `options`, `cdp` (DevTools wrapper for WebDriver), `js/`. |
| `wire/` | (feature `scrape` or `impersonate`) Pure wire formats: `ja3`, `ja4`, `akamai`, `client_hello`, `tls_spec`, `h2_spec`, `h2_frames`, `header_spec`, `ids`, and `emulation`, the measured browser tables embedded from `wire/measured/*.json` (one entry: Chrome 155 on macOS). `BrowserKind` is in `mod.rs`. No I/O, checked by `tests/scrape_domain_purity.rs`. Held to the pedantic lint set. |
| `scrape/` | (feature `scrape`) `challenge` (`detect`, `markers`, `mitigation`), `egress` (`route`, `pool`, `geo`, `redact`), `jar` (`CookieJar`, `Set-Cookie`), `ledger` (`state`, `events`), `passport` (`StealthIdentity`, `SessionPolicy`), `error` (`ScrapeError`), and `runtime/`, which holds all the I/O: `BrowserScraper`, the CDP leg, `EgressSwitch`, `JsonStateStore`, the identity file, the tracing sink and `capture/`. The directories other than `runtime` are pure (same test). Pedantic lint set. |
| `impersonate/` | (feature `impersonate`) `client` and `convert` (the only files that name wreq types), `capture_h2`, `tls_server/` (`EphemeralCa`), `http_session/` (`HttpSession`), `mitm/` (`MitmProxy`), `stealth_session/` (`StealthSession`). Pedantic lint set. The stealth layer (`wire`, `scrape`, `impersonate`, `session_registry`) is experimental, not a stable API. |
| `profile_payloads/` | Parses external browser-profile JSON into a `Fingerprint`. |
| `mcp/` | MCP server (`tool`, `host`, `webdriver`, `cdp`, `stealth`, `schema`); entry `src/bin/mcp_server.rs`. Held to the pedantic lint set. |
| `cli/` | `bin/sbase.rs` (clap), `commands.rs`, `scaffold.rs`, `scripts/` (`sb_grid`, `sb_mkdir`, `sb_mkfile`, `sb_recorder`, `sb_commander` needs `tui`, ...), `bin/report.rs`. |
| `behave/` | Gherkin runner and step registry. |
| `plugins/` | Reporting and logging plugins (S3, GCP, Azure, screenshots, page source, driver manager, observer). |
| `config/` | Settings, runtime config, `proxy_list`, `ad_block_list`. |
| `utilities/` | `retry.rs` (`RetryPolicy`, `RateLimiter`), `grid_server.rs` (`GridProcess::start/state/stop/ended`), `selenium_grid`, `selenium_ide`, `python_importer`. |
| `utils/` | `selectors`, `shadow`, `urls`, `geometry`, `extensions/` (bundled browser extensions), `translate/` (language dictionaries). |
| `common/` | `shutdown` (graceful shutdown registry), `encryption`, `obfuscate`/`unobfuscate`, `exceptions`, `decorators`. |
| `storage/` | `turso` feature: `ResultStore`, `ProfileVault`. |
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
- Stealth features: `just scrape-gate` (pure Rust, runs here) and `just imp-gate`
  (cmake and BoringSSL, about 1 GB of target disk). For the `impersonate` code
  start with `cargo clippy --features impersonate --lib`; the Rust MCP does not
  load that feature. The Chrome tests of the stealth layer are
  `impersonate_http_chrome`, `impersonate_mitm_chrome` and `stealth_session_chrome`,
  and they run Chrome 155 headless on macOS.
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
  Only the `sbase mkdir`/`mkfile` golden templates are compiled as doctests.

## Gotchas

- macOS delays the first run of a freshly written script, so tests that fake
  `java` run it once with `--warm` before timing it.
- `sbase commander` exits with status 2 and a hint when built without `tui`.
- `thirtyfour` still depends on `zip` 8.6, so two `zip` versions are in the tree.
- The local Docker wrapper treats `EXPOSE` as a web app and probes the port;
  this image has none. Build it with `--no-slim`.
- Real-Chrome behaviour differs between headless and headed runs; check which
  one a test uses before trusting a timing.

## Work in flight

`ROADMAP.md` lists what is done and missing. The stealth work (features `scrape`
and `impersonate`: wire formats, the measured Chrome 155 on macOS, custom
JA3/JA4/Akamai inputs, `BrowserScraper`, `HttpSession`, `MitmProxy`,
`StealthSession`, the hardening of the pure CDP page script) is merged as work
packages WP0 to WP17, written as our own code with no dependency on
stealthscraper-rs. The WP18 security reviews are done and their fixes are in
progress; WP19 (re-exports, documentation, final gates) follows. Then examples
and benchmarks, last. Only macOS and Chrome 155 are measured; the layer is
experimental and was tested against local servers and a local Chrome, not live
services. The design is `docs/design/stealth_parity_plan.md`, the user-facing
page `docs/tutorials/stealth_scraping.md`, the layout rules
`docs/adr/003-stealth-scraping-layout.md`.
