# seleniumbase-rs: knowledge map

A map of the repository for a coding assistant. It says where things live, how to
build and check them, and what has bitten before. Keep it short and correct;
fix a line here when the code stops matching it. Human contributors want
`CONTRIBUTING.md` and `docs/DEVELOPER_GUIDE.md`.

## How to work here (read this first)

- Start from the module map below rather than a repository-wide search, and use
  rust-analyzer (or `cargo check`) for types and references before a full build.
- Recommended: [TokenSave](https://github.com/aovestdipaperino/tokensave) for code
  search, callers, impact and reads (`tokensave init`, then its MCP tools; when
  `.tokensave/` exists, use it instead of `grep` or whole-file reads), and
  [RTK](https://www.rtk-ai.app/) for condensed command output (`rtk cargo test`,
  `rtk git log`; exit codes unchanged), and
  [mr-boxington](https://mr-boxington.jdx.dev) (`mbx`) for cached builds: plain
  `cargo` then gives each worktree its own target and reuses compiled work; do
  not set `RUSTC_WRAPPER` or `CARGO_TARGET_DIR` with it.
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
`Dockerfile` base image must move with it). The repository is a Cargo workspace
(`default-members` = both packages): the library `seleniumbase-rs` at the root has
no binary and no command-line crate; the binaries `sbrs` (CLI) and `sbrs-mcp`
(MCP server) are the package `seleniumbase-rs-cli` in `crates/cli`, which turns
`pdf`, `mcp-server`, `mcp-client` and `plugins-builtin` on by default and forwards `tui`, `gui`
and `turso`. The
library's default features are empty; `gui`, `tui`, `turso`, `playwright`,
`plugins`, `ocr`, `visual`, `pdf` are opt-in. The
experimental stealth layer (wire formats, `BrowserScraper`, `HttpSession`,
`MitmProxy`, `StealthSession`; formerly the features `scrape` and `impersonate`)
moved out in 0.3.0 to the workspace
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
(crates `seleniumbase-rs-codec`, `-scrape`, `-mimic`), which uses only the
public API of this crate: keep items it imports public (`evasion::randomize`,
`sb_cdp`).

## Two engines

| Engine | Where | Notes |
|---|---|---|
| WebDriver (thirtyfour) | `src/browser/`, `src/api/` (`BaseCase`) | Classic Selenium API plus the stealth/UC layer in `src/evasion/`. |
| Pure CDP, no driver | `src/sb_cdp/` | Talks to Chrome directly. `Browser`, `Page`, `Locator`, `BrowserContext`, `BrowserPool`, `SessionStore`, `Identity`, request interception, a mock transport (`Browser::new_mocked`, `MockCtrl`) for tests without Chrome. |

## Module map (`src/`)

| Path | Role |
|---|---|
| `api/` | `BaseCase` (`base_case.rs`) and the capability traits in `traits.rs` (`BrowserApi`, `ElementApi`, `AssertionApi`, `ScreenshotApi`, selectors passed as `css: &str`). `base_case_impls/` splits its methods by theme (`_dom`, `_nav`, `_mouse`, `_storage`, `_tours`, ...). Also `recorder`, `scenario`, `presentation`, `master_qa`, `chart`, `tour`, `pdf`, `html`, `html_inspector`, `dialog`, `gui`, `cdp_driver`/`cdp_page`, `runner` (`run_browser_test`). |
| `browser/` | `BrowserConfig`/`Browser`/`DriverMode` (`config.rs`), `BrowserSession` (`session.rs`), `launcher`, `driver_access` (repairs and runs drivers), `driver_install` (geckodriver and msedgedriver), `driver_version` (matches chromedriver to Chrome), `downloader`, `downloads` (the `downloaded_files` folder browsers are told to save into), `reuse` (reused sessions, `--crumbs`), `options/` (typed launch settings: Chromium switches, mobile mode), `playwright`. |
| `sb_cdp/` | The pure CDP engine: `browser`, `page`, `locator`, `context`, `client` (protocol), `launch` (`LaunchOptionsBuilder`), `pool` (`BrowserPool`, `PoolOptions`), `session_store`, `state` (`Cookie`, storage), `identity` (`Identity`), `intercept`, `input`, `human`, `captcha`, `download` (`Page::download_file`, `download_by_click`, `save_file_as`, `assert_downloaded_file`), `breakpoint` (`Page::breakpoint`), `capture` (`Page::capture_fingerprint`), `proxy_auth`, `webrtc`, `har` (`Page::start_har`, `HarRecorder`, the HAR 1.2 serde types), `expect`, `mock`, `helper.js`. Held to the pedantic lint set. |
| `evasion/` | `fingerprint.rs` (`Fingerprint`, one `Spoof` per setting, `ProxyConfig`, `ClientHints`), `spoof.rs`, `settings.rs` (value types), `policy.rs` (`StealthPolicy`), `providers/` (`EvasionRegistry`, `default_registry`, `builtin.rs`), `randomize`, `capture` (`CapturedFingerprint`: reading a real browser's fingerprint), `fingerprint/` (parts of `fingerprint.rs`: `compare`, `file`, `schema`), `behavior` and `humanize` (human-like input), `patcher` (Chrome/chromedriver binary patching), `reactor`, `uc`, `evasions` (launch flags), `options`, `cdp` (DevTools wrapper for WebDriver), `js/`. |
| `profile_payloads/` | Parses external browser-profile JSON into a `Fingerprint`. |
| `mcp/` | MCP servers `cdp`, `driver`, `sb` and `dev` (`tool`, `host`, `webdriver`, `cdp`, `devtools`, `stealth`, `schema`; `dev/` is the server with no browser: `find_api`, `parity_lookup`, `python_to_rust`, `scaffold`, `cli_help`, plugins, `install_driver`, `explain_error`, `check_config`, `check_fingerprint`, `diagnose_environment`; `api_index.json` is the index `find_api` searches and `spec.rs` reads the `sbrs` usage spec); started by `crates/cli/src/mcp_server.rs`. Held to the pedantic lint set. |
| `cli/` | The library half of the command line, with no parser: `commands.rs`, `devtools.rs`, `fingerprint_cmd.rs` (`sbrs fingerprint`), `plugin_cmd.rs` (`sbrs plugin list|info`, the check of `--plugin` names and options), `prompt.rs`, `scaffold.rs`, `scripts/` (`sb_grid`, `sb_mkdir`, `sb_mkfile`, `sb_recorder`, `sb_commander` needs `tui`, ...). |
| `crates/cli/` | Package `seleniumbase-rs-cli`: `src/sbrs.rs` (the command line, declared with the `usage` derives `Cli`, `Subcommands`, `Args` and `ValueEnum`), `src/fig.rs` (the Fig spec, written from the usage tables), `src/report.rs`, `src/fingerprint.rs` (`sbrs fingerprint`), `src/failure.rs` (a command line that does not parse prints the command's page, then the error), `src/completion.rs` (the patched zsh and PowerShell completion scripts), `src/reference.rs` (renders `docs/help/cli_reference.md` for the test that keeps it current), `src/mcp_server.rs`, `src/mcp_install/` (`sbrs-mcp install`, `uninstall` and `clients`: one module for the clients' locations and formats, one for editing JSON as text, one for `config.toml`; its tests point an `Env` at a temporary folder and never touch a real client config), and `tests/` for every test that runs the real `sbrs` or `sbrs-mcp` binary. Owns `usage` (the package `usage-rs`) and, for the tests, `usage-lib`. |
| `testing/` | Feature `testing`: the rstest `sb`/`sb_cdp` fixtures (`Lifecycle`, reruns, evidence); `testing/bdd` (feature `bdd`): Gherkin scenarios as cargo tests through rstest-bdd (`SbCase`, `SbCdpCase`). |
| `testing/` | Feature `testing` (optional `rstest`): the `sb` and `sb_cdp` fixtures. `fixture.rs` (`Sb`, `SbCdp`: a handle that holds the settings and the test id, `run` owns the browser), `lifecycle.rs` (attempts, reruns, `Finalizer`, the `TestCase` seam), `evidence.rs` (`latest_logs/<test id>/`), `deadline.rs` (the hard time limit a prompt suspends); `common/test_id.rs` is the always-compiled test id. |
| `plugins/` | Reporting and logging plugins (S3, GCP, Azure, screenshots, page source, driver manager, observer). |
| `config/` | Settings, runtime config, `proxy_list`, `ad_block_list`. |
| `utilities/` | `retry.rs` (`RetryPolicy`, `RateLimiter`), `grid_server.rs` (`GridProcess::start/state/stop/ended`), `selenium_grid`, `selenium_ide`, `python_importer`. |
| `utils/` | `selectors`, `shadow`, `urls`, `geometry`, `extensions/` (bundled browser extensions), `translate/` (language dictionaries). |
| `common/` | `shutdown` (graceful shutdown registry), `orphan_guard` (browsers and drivers end with the program that started them), `debug_prompt` (the pause of `BaseCase::breakpoint` and of `--trace`, `--pdb`, `--final-debug`), `tls` (the rustls `ring` provider), `encryption`, `obfuscate`/`unobfuscate`. |
| `plugin_host/` | `plugins` feature: turns plugin names and options (`[plugins]`, `SB_PLUGINS`, `--plugin`) into a `PluginSet` (`convert.rs`, `guard.rs`). |
| `storage/` | `turso` feature: `ResultStore`, `ProfileVault`. |
| `visual.rs` | `visual` feature: `VisualBaseline` (screenshot baselines compared by SSIM, `check`, `check_page`), behind `BaseCase::assert_visual_match`. Not `check_window`, which compares the HTML layout. |
| rest | `error.rs` (`SeleniumBaseError`), `macros.rs`, `artifacts.rs`, `core/` (`test_run`, `report_helper`, `log_helper`, and the `dashboard`, `junit` and `allure` reports), `js_code/`, `resources/`, `ocr.rs` (`ocr` feature), `tracing_util.rs` (the crate logs with `tracing`). |

Elsewhere: `tests/` (integration tests of the library; `tests/golden` is compiled into the
library through `include_str!`, `tests/support` is shared with `crates/cli/tests`), `benches/` (`cdp_latency`, `cpu`), `examples/`
(`tauri-profile-manager` has its own manifest), `docs/` (the Docs7 site, whose menu is `docs.json` and whose pages `just mdx` builds; also mdBook, `SUMMARY.md`),
`parity/`, `.config/nextest.toml`, `.github/workflows/` (`ci.yml`, `docker.yml`,
`completions.yml`), `Dockerfile`, `Makefile`, `justfile`, `deny.toml`.

## Build and check

Use the same environment flags everywhere so build artifacts are shared:

```bash
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all -- --check
cargo clippy --all-targets --features "mcp-server test-util testing" -- -D warnings
cargo test --features "mcp-server test-util testing"
cargo audit && cargo deny check
mdbook build
```

These cover both packages. `-p seleniumbase-rs` builds the library alone with its
own default features (a build of both unifies the CLI's `pdf` and `mcp-server`
into the library); `-p seleniumbase-rs-cli` the command line alone. Install the
binaries with `cargo install --path crates/cli`.

- Count failures from the exit code or from `test result` lines, never from a
  filtered `grep`; read the whole output.
- Tests that launch a real Chrome are named `*_chrome.rs` (plus `browser_smoke`)
  and are `#[ignore]`d. Run them one binary at a time (the nextest `chrome` group
  has one thread). Mock-driven tests (`tests/sb_cdp_*.rs`) need no browser. The
  tests that run the `sbrs` binaries are in `crates/cli/tests`.
- The stealth layer's gates, BoringSSL build (cmake) and Chrome tests live in
  the seleniumbase-rs-stealth repository, not here.
- Publishing: gate the exact commit, `git fetch`, confirm `origin/main` has not
  moved, fast-forward push `"${C}:refs/heads/main"` (in zsh `"$C:refs"` reads as the `:r`
  modifier and pushes nothing), then check local equals
  remote. No force-push. Commits carry no `Co-Authored-By` or Claude attribution.

## Conventions

- Errors are `SeleniumBaseError` (thiserror). `unwrap()` only in tests.
- A public item gets a doc comment; a bug fix gets a test that fails without it.
- A new `[[bin]]`, `[[bench]]` or `[[example]]`, in either manifest, must also be
  added to the `Dockerfile` skeleton step, or the image build fails. A
  command-line crate in the library's `[dependencies]` fails
  `tests/library_manifest.rs`; it belongs in `crates/cli/Cargo.toml`.
- A new Python-parity API gets an entry in `parity/api.toml`.
- Tutorials in `docs/tutorials/` are not compiled; keep their code true by hand.
  Only the `sbrs mkdir`/`mkfile` golden templates are compiled as doctests.
- `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` are generated from
  the declarations in `crates/cli/src/sbrs.rs`; a test fails when either is stale.
  Write them again with
  `SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`.
  `docs/completions/fig-sbrs.ts` is `sbrs completions fig` through prettier (see the
  README beside it) and is not checked by a test.
- The `dev` server's `cli_help` embeds `completions/sbrs.usage.kdl` and reads it with the
  small KDL reader in `src/mcp/dev/spec.rs` (the library may not use `usage-lib`), so a new
  command or flag reaches it once the spec is written again. A `usage` upgrade that writes
  the KDL differently shows up as a failing `mcp::dev::spec` test.
- `src/mcp/dev/api_index.json` (what the `dev` server's `find_api` searches) is generated
  from the public methods of `BaseCase` and the pure CDP engine; a test fails when it is
  stale. Write it again with
  `SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index`.

## Gotchas

- macOS delays the first run of a freshly written script, so tests that fake
  `java` run it once with `--warm` before timing it.
- `sbrs commander` exits with status 2 and a hint when built without `tui`.
- usage 6 is looser than clap was in three places that the declarations cannot close,
  so `crates/cli/src/strict.rs` reads the parser's events before the real parse and
  refuses them with status 2: a flag repeated after a subcommand (an inline struct
  variant of `Commands` cannot say `args_override_self`, so the check holds every
  command to the root's rule; a test walks the spec), a value written after a switch
  (`--headless=false`; the parser drops it unless the switch is declared `bool_value`,
  so every switch must be, and a test fails for one that is not) and a `help` path that
  names no command (`sbrs help nosuch`). The help lists the commands alphabetically,
  not in declaration order.
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
