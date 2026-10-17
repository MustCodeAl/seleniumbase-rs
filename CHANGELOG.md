# Changelog

All notable changes to `seleniumbase-rs`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). The target is feature parity with
Python SeleniumBase 4.55.2 and `seleniumbase-mcp` (`ROADMAP.md` has the open
work, `docs/parity.md` the per-API status).

## Unreleased

### Added

- **Browser plugins for the WebDriver engine** (feature `plugins`; ADR 004
  phase 3). `BrowserConfig::with_plugins` gives a `PluginSet` to
  `BaseCase::new` and `BrowserSession::connect`: the set is checked against the
  browser, `before_launch` changes the start-up settings (Chromium arguments,
  capability preferences, extension folders, proxy and headless; Firefox
  arguments, preferences and headless), and once the session runs the plugins
  get `on_browser`, then the packages and `on_page_created` for the first
  window, for each window `switch_to_new_window` or `open_new_window` opens, and
  for any other on `apply_plugins_to_window`; `quit` or a drop calls
  `on_close`, and `reconnect` runs the hooks again. On Chromium browsers
  plugins send protocol commands through chromedriver's DevTools channel; on
  Firefox and Safari `send` is refused and scripts for every document run at
  once and again after each `open`. A refusal or a failing hook stops the
  session with `SeleniumBaseError::Plugin`. Plugins with `reuse_session` are
  refused. See the WebDriver section of `docs/tutorials/plugins.md`.
- `sb_cdp::Page::add_script_tag` and `add_style_tag` add a script or a
  stylesheet to the current document from a URL or inline (`TagSource`),
  waiting for a file to load; `add_init_script` runs a script in every
  document the tab loads from then on and returns an `InitScriptId` for
  `remove_init_script`. Playwright's `addScriptTag`, `addStyleTag` and
  `addInitScript`.
- With the `plugins` feature, `sb_cdp::Page` implements the plugin API's
  `PageHandle`, so a program can hand one of its tabs to plugin code.
- Visual regression testing, behind the new opt-in `visual` feature:
  `visual::VisualBaseline` saves a screenshot as a baseline PNG on the first
  run and compares later screenshots with it by structural similarity (MSSIM
  over 8 × 8 windows on each colour channel, `image-compare`), failing below a
  threshold of 0.999 with `AssertionFailed` and leaving `<name>.actual.png` and
  a `<name>.diff.png` that paints the differences red. Screenshots of another
  size fail without a comparison. `check_page` takes a `BaseCase` or an
  `sb_cdp::Page`, and `BaseCase::assert_visual_match(name)` keeps its baselines
  in `visual_baseline/<test>/`, `<test>` being the test's whole path (test
  binary, module path and function, as `cart.checkout.renders`), and follows
  `with_visual_baseline_reset`.
  `check_window` is unchanged: it compares the HTML layout, as in Python.
- HAR recording for the pure CDP engine (no new dependency, no feature):
  `Page::start_har()` / `start_har_with(HarOptions)` return a `HarRecorder`,
  and `HarRecorder::finish()` returns a `Har` (HTTP Archive 1.2 serde types)
  that `Har::save(path)` writes as JSON. Entries come from the `Network`
  events of the tab and of its cross-site iframes, which run in targets of
  their own and are attached while recording, extra-info headers included, with cookies, query strings, POST
  bodies, one entry per redirect hop, failures with Chrome's `_error`, and
  `blocked`/`dns`/`connect`/`ssl`/`send`/`wait`/`receive` timings; each
  top-level navigation is a page with `onContentLoad` and `onLoad`. Response
  bodies are off by default and capped at 1 MiB each when turned on; the cap
  can be raised to 64 MiB (`MAX_CONTENT_BYTES`).
- `sbrs prompt` prints one short line for a shell prompt: the installed
  Chrome's major version and whether a matching chromedriver is at hand
  (`chrome 155 ✓driver`, `✗driver`, `?driver`), with `--ascii` marks and a
  `--format` of `{browser}`, `{major}`, `{driver}`, `{mark}` and
  `{driver_major}`. It starts no browser and uses no network: versions come
  from files (a macOS app's `Info.plist`, the version folder beside
  `chrome.exe`), and one that only `--version` tells is cached for ten minutes
  in the user cache folder. It prints nothing and exits with 0 when no browser
  is found (`--verbose` says why). The CLI page has setups for Bash, Zsh,
  Fish, PowerShell, Starship and Warp, and `.warp/workflows/` a Workflow for it.
- `browser::driver_version::major_without_running` reads a program's major
  version from its files only, and `version_flag_major` runs `--version` with a
  time limit.

### Fixed

- The pure CDP engine takes DevTools messages up to 256 MiB, as Puppeteer
  does. Chrome sends each answer in one WebSocket frame, and one over the
  default 16 MiB, such as a large screenshot or response body, used to close
  the connection for every tab.
- `sbrs` exits quietly with status 0 when the reader of its output goes away
  (`sbrs completions zsh | head`), instead of printing "error: Broken pipe".

## [0.3.0] - 2026-10-15

A breaking release: the experimental stealth scraping layer leaves this crate.

### Removed

- **BREAKING: the stealth scraping layer moved to its own repository,
  [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth).**
  The features `scrape` and `impersonate`, the modules `seleniumbase_rs::wire`,
  `seleniumbase_rs::scrape` and `seleniumbase_rs::impersonate`, and their
  crate-root re-exports (`BrowserScraper`, `CookieJar`, `StealthIdentity`,
  `SessionPolicy`, `ScrapeError`, `BrowserKind`, `Emulation`, `TlsSpec`,
  `H2Spec`, `HeaderSpec`, `ImpersonationProfile`, `CustomProfileBuilder`,
  `impersonation_client`, `HttpSession`, `MitmProxy`, `StealthSession`,
  `EphemeralCa` and the rest) are gone from this crate. They are now the crates
  `seleniumbase-rs-wire` (was the module `wire`), `seleniumbase-rs-scrape` (was
  the feature `scrape`) and `seleniumbase-rs-impersonate` (was the feature
  `impersonate`), which build on the public API of this crate and stay
  experimental. Depend on them by git, for example
  `seleniumbase-rs-scrape = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }`,
  and give `seleniumbase-rs` by its plain git URL next to them so both resolve to
  one commit ([Stealth Scraping](docs/tutorials/stealth_scraping.md)).
- With the layer go the optional dependencies `psl`, `wreq`, `btls`,
  `tokio-btls`, `brotli`, `hyper`, `hyper-util` and `http-body-util`; the
  examples `capture_tls`, `capture_hints`, `capture_h2`, `cookie_jar`,
  `browser_scraper`, `custom_profile`, `http_session` and `stealth_session`; the
  tests and fixtures of the layer; the `just scrape-gate`, `imp-gate` and
  `measure` recipes and their mise tasks; the `scrape` CI matrix entries and the
  `impersonate` CI job; and CMake, NASM and LLVM from the build requirements.
- `futures-util` without its default macros (`select!`/`join!`, unused), so the
  `futures-macro` proc macro is no longer compiled.
- `scraper` without its default features (`getopts` for its own CLI, unused
  parse-error text). `flate2` asks only for the `zlib-rs` backend `zip` already
  builds (`pdf-extract` still turns on its default one).
- The unused `tracing-futures` dependency (every build compiled it) and the
  unused `tracing-actix` one from the `full-tracing` feature.
- JSON logs come from `tracing-subscriber`'s own JSON format instead of the
  separate `json-subscriber` crate.
- The `playwright` feature no longer lists `rustwright-core` itself; `rustwright`
  already brings it without Python.
- HTTPS uses rustls with the `ring` crypto provider for every client, so the
  `aws-lc-sys` C library is no longer compiled. A program that builds its own
  reqwest client next to this crate calls `seleniumbase_rs::install_tls_provider()`
  first.
- reqwest no longer builds its `blocking` client: ending reused sessions at
  exit uses the async client on a small runtime of its own.
- `thirtyfour` is built without its `component` derive macros and its own
  driver `manager` (this crate downloads drivers itself), which also removes its
  older `zip` and `dirs`.
- `zip` is built with deflate only (the format of driver downloads and the
  bundled extensions) instead of every format; zstd (a C library), bzip2, lzma,
  xz, ppmd and AES no longer compile.

### Fixed

- A WebDriver URL with a path and no trailing slash (a Grid's
  `http://hub:4444/wd/hub`, or tauri-plugin-sbrs-automation's
  `http://127.0.0.1:4445/<token>`) lost its last path segment, because the
  client joins command paths as relative URLs; the slash is added when
  connecting.

### Changed

- The generated browser identity claims Chrome 154, 155 or 156 (was 153 to
  155), and its high-entropy Client Hints name a real build per major
  (`stealth::randomize::KNOWN_FULL_VERSIONS`: 155.0.8059.40 measured on macOS
  27.0.1 by `tests/client_hints_chrome.rs`, 154.0.8037.100 and 156.0.8078.35 from
  Google's version history) instead of `major.0.0.0`. Macs claim macOS 15.8.1,
  26.7.1, 27.0.0 or 27.0.1; iPhones may claim Safari 26.0 with the frozen
  `iPhone OS 18_6` (Safari 17.7 is gone). The Chrome presets, the `--mobile`
  user agent and the examples claim Chrome 156 (`PRESET_CHROMIUM_MAJOR`), the
  Android ones in Chrome's frozen `Linux; Android 10; K` form, and the iPhone
  preset is Safari 26.0. Seeds map to other identities than before.
- A fingerprint without Client Hints (the presets) gets its brand list, decoy
  brand and full versions from `chromium_hints(major)` instead of a fixed
  `Not(A:Brand` 24, the Client Hints platform name (`Windows`, not `Win32`), and
  a `fullVersionList`.

- The `sbrs` banner reads SeleniumBaseRust (figlet small font, 68 columns), and
  `sbrs --help` and `sbrs-mcp --help` say SeleniumBaseRust.
- The tests' leftover-process checks (Edge, geckodriver, the MCP stdio servers)
  list processes with `sysinfo` (a dev-dependency) instead of `pgrep` and
  `kill -0`, so they run on Windows too instead of being skipped there.
- Human-paced input draws its bell-curve values (`stealth::humanize::Rng::gaussian`,
  used by `stealth::behavior`) from `rand_distr`'s `StandardNormal` instead of a
  hand-written Box-Muller transform. The distribution is the same, but a seeded
  `Humanizer` now plans different paths and timings than before for the same
  seed. `humanize::Rng` implements `rand::TryRng` (so `rand::Rng`), and
  `rand_distr` is a new dependency.

### Added

- `sbrs devtools` launches Chrome with its DevTools endpoint open (`--port`,
  default 9222), prints the endpoint and configs for Google's
  `chrome-devtools-mcp` and Microsoft's Playwright MCP (`--json` for one line of
  JSON), and runs until Ctrl-C, which closes the browser. The Pure CDP MCP server
  has `get_devtools_endpoint`, and `Browser::devtools_endpoint()` and
  `LaunchOptionsBuilder::debugging_port` do the same from Rust. See
  `docs/tutorials/devtools_mcp.md`, which also covers Firefox's and Safari's MCP
  servers.
- `.github/workflows/release.yml` builds `sbrs` and `sbrs-mcp` for five targets
  on a version tag and attaches the archives, with completions and Warp
  workflows, to the release; `[package.metadata.binstall]` points `cargo
  binstall` at them. Not run yet (GitHub Actions is locked on the account).

- Feature `ocr` (off by default): `ocr::Ocr` reads the text a page shows from
  its screenshot (`read_page`, for a `BaseCase` or an `sb_cdp::Page`) or from PNG
  bytes (`text_in_png`), with [ocrs](https://github.com/robertknight/ocrs), pure
  Rust. Its two models (about 12 MB) are downloaded into the cache folder on
  first use and checked against pinned SHA-256 hashes; `SB_OCR_MODELS` names a
  folder of your own copies instead.
- **Browser plugins for the Pure CDP engine** (feature `plugins`, off by default;
  ADR 004 phase 2). `LaunchOptionsBuilder::plugins` takes a `PluginSet` from the
  plugin API crate
  [`seleniumbase-rs-plugin`](https://github.com/MustCodeAl/seleniumbase-rs-plugins),
  re-exported as `seleniumbase_rs::plugin_api`. `Browser::launch` checks the set,
  lets `before_launch` change the arguments, preferences, extensions, proxy and
  headless mode, and calls `on_browser`; every tab the library attaches to gets
  the plugins' packages and `on_page_created`; `Browser::close` (or the last
  drop) calls `on_close`. `Browser::connect_with_plugins` does the same for a
  running browser. A plugin that refuses or fails stops the launch or the tab
  with the new `SeleniumBaseError::Plugin`. Guide: `docs/tutorials/plugins.md`.
- `LaunchOptionsBuilder::extension` loads an unpacked extension over the
  protocol (`Extensions.loadUnpacked`, which branded Chrome 137 and later need
  instead of `--load-extension`), and `LaunchOptionsBuilder::preference` merges
  a preference into the profile's `Default/Preferences` before the browser
  starts.
- `Browser::launch_mocked` (feature `test-util`) runs the launch steps, plugins
  included, against a `MockCtrl`, which `MockCtrl::new` now creates for a test
  to script first.
- Feature `tokio-console` (off by default): `sbrs`, `sbrs-mcp` and programs that
  use the crate's logging serve [tokio-console](https://github.com/tokio-rs/console)
  for watching async tasks. Build with `RUSTFLAGS="--cfg tokio_unstable"`.

## [0.2.0] - 2026-10-12

### Changed

- **The binaries are renamed** so they no longer clash with Python
  SeleniumBase, which installs its own `sbase`: the CLI is now `sbrs` and the
  MCP server `sbrs-mcp` (was `seleniumbase-mcp`). Commands and flags are
  unchanged; replace the name in scripts and MCP client settings, and generate
  the shell completions again (`sbrs completions zsh > _sbrs`). The Docker
  image runs `sbrs` as user `sbrs`. `sbase_config.toml` keeps its name.

### Added

- `sbrs completions fig` writes a Fig completion spec, the format of
  withfig/autocomplete that Warp's built-in completions, inshellisense and Amazon
  Q use. Warp also uses the shell's own completions by default.
- `sbrs completions usage` prints a usage (KDL) spec for jdx/usage and mise,
  kept in `completions/sbrs.usage.kdl` and checked against the CLI by a test.
- Warp Workflows for common `sbrs` commands in `.warp/workflows/`.
- Every docs page has a title in front matter for Docs7; the mdBook build strips it.
- ADR 004 proposes a plugin framework for community crates, in the style of
  puppeteer-extra and Zorilla.

## [0.1.1] - 2026-10-12

Windows fixes from a second round of runs on a Windows 11 PC (Chrome 154 and
155, Edge 155, Firefox 153). There, formatting, clippy, the whole test suite,
the real-Chrome tests, Edge, Firefox and the Tauri 3 installer build now pass.

### Fixed

- The browser version on Windows comes from the `chrome.exe` (or `msedge.exe`)
  that really runs, read from its file version: while an update waited for a
  restart, the newest version folder named a Chrome that was not running yet and
  the wrong chromedriver was fetched. The folder rule also skips a waiting update.
- Download folders are handed to Chrome as plain `C:\...` paths, not with the
  `\\?\` prefix `canonicalize` adds on Windows; a `blob:` download was cancelled.
- The tests that list processes with `pgrep` skip those checks on Windows.

### Added

- `rust-toolchain.toml`, so rustup uses Rust 1.99 in the repository by itself.
- The Windows status in `docs/platforms.md`. Known gap: on Windows nothing ends a
  browser whose program was killed (macOS and Linux have a process-group watcher).

## [0.1.0] - 2026-10-12

The first release, tagged `v0.1.0` on GitHub (not published to crates.io).
Windows is being verified: the first runs on a Windows PC found the problems
listed first under *Fixed*.

### Added

- **geckodriver and msedgedriver**: a Firefox session starts geckodriver and an
  Edge session msedgedriver itself, as Python SeleniumBase does, where both
  used to be handed chromedriver. Each is looked for in `downloaded_drivers/`
  and on the `PATH`, and downloaded when missing: geckodriver's newest GitHub
  release, and the msedgedriver of the installed Edge's major version from
  `msedgedriver.microsoft.com` (`LATEST_RELEASE_<major>_<OS>`); an msedgedriver
  that does not fit the installed Edge is replaced, and `with_driver_version`
  pins one. Downloads are capped at 64 MB, unpacked (zip or tar.gz) without
  writing outside their folder, and made runnable as chromedriver is. Both are
  started with the orphan guard and the macOS re-sign retry; geckodriver gets
  `--websocket-port 0` so Firefox sessions can run side by side. Firefox
  sessions now also send `browser_binary_path` as `moz:firefoxOptions.binary`,
  `user_data_dir` as `-profile`, and the user agent and locale as preferences.
  `sbase install` takes a driver and a version (`sbase install geckodriver
  0.36.0`, `sbase get edgedriver`), and `sbase get` is its alias. Checked on
  macOS with a real headless Edge 155 and the msedgedriver the crate downloaded
  (`tests/edge_msedgedriver.rs`); the crate also found and started Homebrew's
  geckodriver there, and downloaded geckodriver 0.37.1 and 0.36.0. A headless
  Firefox 153 ESR took the new `moz:firefoxOptions` through the geckodriver of
  the `docker/firefox-check` container (`tests/firefox_geckodriver.rs`); the
  crate itself ran on the Mac, not in the container.
- **Playwright and Puppeteer importers**: `sbase import-playwright` (alias
  `import-pw`) converts `@playwright/test` files, `playwright codegen` output and
  pytest-playwright, sync and async Python scripts; `sbase import-puppeteer`
  (alias `import-pptr`) converts Puppeteer scripts, jest-puppeteer and Mocha
  tests and Chrome DevTools Recorder JSON exports. Both write one
  `#[tokio::test]` per test on the pure CDP engine, keep every statement they
  do not convert as a `TODO` with a diagnostic naming its line, and read the
  input with a bounded hand-written parser whose text reaches the Rust only in
  escaped literals and comments (`utilities::importer`,
  `docs/playwright-migration.md`, `docs/puppeteer-migration.md`).
  `SelectorBuf::role`, `text`, `label` and `has_text` approximate `getByRole`,
  `getByText`, `getByLabel` and `filter({ hasText })` in XPath.
- **Pure CDP engine** (`sb_cdp`): `Browser`, `Page`, `Locator`, input, cookies,
  storage, window and emulation, retrying assertions, same-origin frames, CAPTCHA
  solving, a mock browser for tests without Chrome, request interception
  (`Page::intercept`), per-context proxies with per-session passwords, a WebRTC
  leak shield, and `Page::human` (curved pointer paths, human typing pace).
  `BrowserPool`, `Lease`, `BrowserContext` and `SessionStore` give bounded,
  isolated, recycled browsers with in-memory session sharing.
- **MCP servers**: `seleniumbase-mcp --server cdp|driver|sb` (24, 26 and 88
  tools, plus 8 stealth tools on `sb`), using the upstream tool names.
- **MCP developer tools** on `cdp` and `sb`: `inspect_page`, `suggest_selectors`,
  `diagnose_selector`, `get_console_messages`, `get_failed_requests`,
  `audit_page`, `capture_debug_bundle` (redacted), `export_session_as_test`
  (a `BaseCase` test from the session's steps) and `diagnose_environment`.
- **Failure reports**: a failed MCP tool call is answered with its kind, the
  call, the page's state (URL, title, ready state, script errors, selector
  matches; read within two seconds), the hint and the next tools to try, as
  text and structured content. Mistyped choices and tool names get a
  did-you-mean.
- **Errors for people**: every `SeleniumBaseError` has a hint, element errors
  point out a likely mistake in the selector, `exit_code()` maps categories to
  exit statuses, and `sbase` prints `error:`, `caused by:` and `hint:` lines.
  `FailureDetails` puts the kind, hint, page and evidence links of a failed test
  into the JSON and HTML reports (`ReportPlugin::with_evidence`).
- **Fingerprints**: `Fingerprint::randomized(os, seed)` draws a coherent
  machine and place for all five `OsType`s; `LaunchOptionsBuilder::fingerprint`
  and `Page::apply_fingerprint` apply one to the Pure CDP engine.
- **Test plugins and storage**: `plugins::observer::{Plugins, TestPlugin}` with
  screenshot, page-source, report and result-store plugins; with the `turso`
  feature, `ResultStore`, `ProfileVault` (AES-256-GCM) and `sbase report`.
- **`sbase`**: `encrypt`/`decrypt`, `test` (carries options into tests as `SB_*`
  variables), `grid`, `install`, `--proxy-list`, a typed recorder that refuses
  what it cannot generate, and `doctor` output that compares the chromedriver's
  version with Chrome's.
- **Chromedriver matching**: `browser::driver_version` and
  `downloader::download_chrome_driver_for(major)`. The launcher downloads the
  driver of the installed Chrome's milestone when the cached one is for another
  major version.
- `ActionRecorder::try_record` (reports why an action cannot be generated, with
  the closest known action), `pw_new_page` and `pw_close` for the Playwright
  feature, `extract_extension`, and iteration on `DriverStack`.
- Parity tooling: `parity/api.toml`, `parity/upstream.toml`, `docs/parity.md`
  and `docs/UPSTREAM_SYNC.md`.
- **Browsers**: launch options for Brave, Comet, Opera, Safari, Internet
  Explorer, Chrome for Testing and `chrome-headless-shell`, typed launch
  switches, capability overrides, Firefox preferences and CDP event capture
  (`browser::options`). `sbase install` picks the chromedriver version.
- **`RunSettings`** (`BaseCase::set_run_settings`): demo mode, slow and fast
  runs, message duration, skipping JavaScript waits, recording `sleep`, and
  resetting a visual baseline; `ActionRecorder::to_gherkin` and
  `save_recording_as_feature` export a recording as a Gherkin feature.
- **`sbase translate`** writes a JSON scenario's actions in another language, and
  the pure CDP `Page` gained `post_message`, `activate_messenger` and
  `set_messenger_theme`.
- **`sbase caseplans` and `sbase behave-gui` as terminal apps** (feature `tui`,
  aliases `case-plans` and `gui-behave`): the Python Case Plans Generator and
  Behave Commander without Tkinter. The first lists the Rust test functions under
  a folder and creates, edits and saves `case_plans/<test>.md` for each, and
  writes `case_summary.md`; the second lists `.feature` files and their
  scenarios, runs the marked ones (or all) through the behave runner with the
  built-in steps, with a tag filter, and shows PASS, FAIL, UNDEF and SKIP per
  scenario. The state of each is a pure state machine tested with plain values,
  and the screens are tested as exact `TestBackend` text. Without `tui` both
  print a hint and exit with status 2, as `commander` does. **Breaking**: the
  earlier file-writing behaviour moved to `sbase caseplans --starter` (the
  `CASE_PLANS.md` checklist) and `sbase behave-gui --new` (the prompt that writes a
  feature file), which work in every build.
- **`Page::gui_click_captcha`** (feature `gui`): clicks a CAPTCHA with the
  operating system's pointer, along a human path, at the screen point worked out
  from the window (`Browser.getWindowForTarget`), the viewport offset, the widget
  frame and the vendor's checkbox offset; `Page::gui_captcha_plan` gives the point
  without moving the pointer. Tested with unit tests and a mocked browser only: it
  has not moved the real pointer or been tried on a live CAPTCHA.
- **Parity**: every API in `parity/api.toml` now has a status (888 implemented,
  383 composed, 94 not-applicable with a reason, none planned). Two audits
  corrected it: 26 entries marked done, and 34 composed or not-applicable ones
  (see below).
- **`--esc-end`** (`--esc_end`, `--ee`): `BrowserConfig::with_esc_end`,
  `Settings::esc_end`, `SB_ESC_END` and `sbase --esc-end`. `BaseCase` puts a key
  listener in the page; once ESC is pressed, `open`, `click`, `click_link_text`,
  `highlight`, `wait_for_element_visible`, `wait_for_ready_state_complete` and the
  typing calls return `SeleniumBaseError::Skipped`, which `run_browser_test`
  counts as a skipped test.
- **`--rec-sb-cdp`**: `ActionRecorder::to_cdp_script`, `to_cdp_test` and
  `save_recording_as_cdp_test` write a recording as a pure CDP program (`Page`
  and `Locator` calls) for 51 recorded actions; any other becomes a
  `// skipped:` comment that quotes its `BaseCase` call. `BrowserConfig::with_rec_sb_cdp`,
  `Settings::rec_sb_cdp`, `SB_REC_SB_CDP` and `sbase --rec-sb-cdp` make
  `BaseCase::export_recording_as_rust` and `save_recording_to_logs` write it, and
  `BaseCase::export_recording_as_cdp_rust` asks for it by name.
  `tests/recorder_cdp_compiles.rs` compiles the program written for every
  supported action.
- **`--reuse-session`** (`--reuse_session`, `--rs`) and **`--rcs`**
  (`--reuse-class-session`, `--reuse_class_session`): `BrowserConfig::with_reuse_session`,
  `with_reuse_module_session` and `with_reuse_class_session(name)`,
  `Settings::reuse_session` and `reuse_class_session`, `SB_REUSE_SESSION`,
  `SB_REUSE_CLASS_SESSION` and the `sbase` flags. A process-wide registry
  (`browser::reuse`) keeps each shared browser's WebDriver server, session id and
  chromedriver; `BaseCase::new` leases an idle browser started with the same
  configuration and attaches to it with a client made in its own Tokio runtime
  (each `#[tokio::test]` has one), after cleaning it: other windows closed, every
  cookie and the open page's storage deleted, `about:blank`. `BaseCase::quit`
  gives it back. One test holds a browser at a time; a second one at the same
  time gets its own. Idle browsers close with their chromedriver after
  `reuse::idle_timeout` (5 s, `SB_REUSE_IDLE_SECS`), at
  `seleniumbase_rs::shutdown_reused_sessions()` and in `drain_cleanups`; on Unix
  the last one also ends with the test binary (see the process-group watcher
  under Fixed), on Windows it is left running. `--rcs` shares a
  browser per Rust module, read from the test thread's name.
  `BaseCase::session_id` and `BrowserSession::session_id` are new. `http` and
  `bytes` are now regular dependencies (they were optional, for `impersonate`).
  Checked against a mock WebDriver and in Chrome 155
  (`tests/reuse_session_chrome.rs`: two `#[tokio::test]` share one Chrome, the
  first test's cookie and storage are gone in the second, and the reaper stops
  chromedriver after the idle timeout).
- **WebDriver ad blocking**: `BrowserConfig::ad_block` makes a Chromium WebDriver
  session (Chrome, Chromium, Edge, Brave, Comet, Opera, Chrome for Testing,
  `chrome-headless-shell`) refuse the requests of ad and tracker hosts. When the
  session connects, reconnects or switches to a window, it sends
  `Network.enable` and `Network.setBlockedURLs` through chromedriver's DevTools
  channel. The hosts are the built-in `AdBlockList`, or a list of your own set
  with `BrowserConfig::with_ad_block_list` (new, as is `with_ad_block`), which
  replaces it. `AdBlockList::bounded_url_patterns` leaves out a host whose parent
  domain is listed and refuses a list that needs more than `MAX_URL_PATTERNS`
  (2,000) patterns, before any browser starts; an `AdBlockList` reads and writes
  as an array of hosts. `BrowserSession::apply_ad_block` repeats the block for a
  window reached some other way. Firefox, Safari and Internet Explorer have no
  DevTools channel through WebDriver and block nothing (a warning says so).
  Checked in a real Chrome 155 (`tests/ad_block_chrome.rs`): a page's own
  requests to listed hosts never reach the server; a frame of another site still
  loads and makes its own requests; a URL that names a port is not matched. The
  nine ad-block entries of the parity manifest moved from `planned` to
  `composed`.
- **Parity audit of the composed and not-applicable entries**, the Rust that
  came out of it:
  - Text selectors: `SelectorBuf::containing_text(text, tag)`, and a CSS selector
    with SeleniumBase's `:contains("text")` runs as XPath in `Page::locator` and
    in the WebDriver `By` conversion.
  - `Page::goto_if_not_url`, `Page::active_element` and `active_element_css`;
    `Page::evaluate` drops a `return ` that starts the last line, as Python does.
  - `Locator::all_waiting` (waits for a first match), `viewport_rect` (window
    coordinates, as `get_element_rect`), `click_and_hold` and `flash_with`.
  - `Browser::debugging_host` (and `http_url` follows it, so an attached remote
    browser reports its own host), `grant_permissions_for(origin, ..)` and
    `grant_all_permissions`; `Permission` now has the 23 kinds Python grants,
    and `Permission::name`.
  - `sbase version`, and `sbase obfuscate` and `unobfuscate` as aliases of
    `encrypt` and `decrypt`.
  - Real-Chrome tests for these in `tests/sb_cdp_parity_chrome.rs`, mock tests in
    `tests/sb_cdp_parity_mock.rs`.
- **Stealth scraping, the wire layer** (feature `scrape`, `src/wire`): parsers
  for TLS `ClientHello`s, JA3, JA4 (with `JA4_r` and the original-order
  variants), peetprint and Akamai HTTP/2 strings; `TlsSpec`, `H2Spec` and
  `HeaderSpec`, which say what BoringSSL and wreq can send and refuse the rest by
  name; the measured table `wire::emulation`, with one entry, Chrome 155.0.8059.40
  on macOS (any other browser and system is `NotMeasured`); and the capture tools
  that write it, `capture_tls` and `capture_hints` (`scrape`) and `capture_h2`
  (`impersonate`).
- **Stealth scraping, the impersonating client** (feature `impersonate`):
  `ImpersonationProfile`, `impersonation_client` (wreq on BoringSSL) and
  `CustomProfileBuilder`, where every part (JA3, `JA4_r`, peetprint, signature
  algorithms, Akamai string, header names, order and values, identity) can be
  supplied and what cannot be sent is refused with a named error before a client
  exists; and `impersonate::tls_server`, an in-memory CA that issues one leaf
  per host for loopback TLS. Known gaps against Chrome 155: the `trust_anchors`
  extension, the ML-DSA signature algorithms and a GREASE signature algorithm.
- **Stealth scraping, the scrape layer** (feature `scrape`, `src/scrape`): an
  RFC 6265 cookie jar with public-suffix checks; `StealthIdentity` and its
  owner-only identity file; `SessionPolicy`; challenge detection for seven
  vendors and a mitigation policy; a proxy pool with locales by exit country;
  per-site state with an atomic JSON store; redacted scraper events; and
  `BrowserScraper`, which fetches through a real Chrome on the pure CDP engine,
  solves challenges and rotates egress and profile. `Fingerprint::randomized_with_major`
  fixes the Chromium major an identity claims.
- **`HttpSession`** (feature `impersonate`): plain HTTP requests that continue a
  browser page's session with the measured TLS, HTTP/2 and headers of Chrome 155
  on macOS. It reads the cookies, user agent, languages and proxy from a page
  (`HttpSession::from_page`) or an identity, keeps an RFC 6265 jar, follows
  redirects by hand, bounds time, bodies and pace, judges replies with the
  mitigation and session policies, and writes cookies back to the page. A page of
  another Chrome major, one that says `HeadlessChrome`, or an unmeasured system is
  refused. The first request to an HTTP/1.1-only origin carries an HTTP/2
  `priority` header.
- **`MitmProxy`** (feature `impersonate`): a loopback proxy that ends a real
  Chrome's TLS with the ephemeral CA and replays each request through the
  impersonating client, behind a random token, with bounded connections, heads and
  bodies, and a hot-swappable egress and profile. Upgrades other than WebSocket
  are refused, and a request claiming another Chrome major is refused. Measured on
  Chrome 155 (headless, macOS): a chain carrying the pinned leaf key is accepted by
  a Chrome that has the SPKI-list switch and no proxy, and is never served to a
  page behind the proxy.
- **WebSocket through `MitmProxy`**: a `wss://` handshake goes upstream over
  HTTP/1.1 with the measured WebSocket `ClientHello`, the `101` is relayed, and the
  bytes are copied both ways with idle and per-direction byte bounds
  (`MitmOptions::websocket_idle_timeout`, `max_websocket_bytes`); half upgrades and
  other protocols are refused (`MitmError::MalformedUpgrade`, `NotIntercepted`).
- **The `MitmProxy` priming page**: a WebSocket may now be a tab's first request
  through the proxy. Chrome never shows a WebSocket's `407` to `Fetch` (measured
  on Chrome 155, tab and browser target alike), so `configure_launch` makes
  `MitmProxy::prime_url()` (`https://sb-mitm.invalid/` and a random path) the
  start page; the proxy answers it itself behind the token, and Chrome keeps the
  credentials for the browser context. A context created later loads the page
  itself. Anything else for that host is an empty `404`; nothing for it goes
  upstream. `MitmStats::primed` counts the loads.
- **`ws://` through `MitmProxy`**: Chrome sends a `ws://` WebSocket to an HTTP
  proxy as a `CONNECT` whose tunnel carries the handshake in plain text (measured
  on Chrome 155). The proxy now reads a tunnel's first bytes: a TLS handshake is
  ended as before, and a plain-text WebSocket handshake takes the checks, header
  rewrite, bounds and relay of a `wss://` one, upstream as `http://`. Any other
  plain text in a tunnel is still refused and never relayed.
- **`StealthSession`** (feature `impersonate`): the session that moves between
  `HttpSession` and a real Chrome behind `MitmProxy` by the mitigation and session
  policies. It starts the proxy when a browser is first needed, makes up to 4
  attempts per fetch by default, and rotates the profile and the egress in both
  modes. Tested against loopback sites, a scripted browser and, in ignored tests,
  a headless Chrome 155 on macOS; not against a live service.
- **Re-exports**: the main types of the stealth layer at the top of `wire`,
  `scrape` and `impersonate` and at the crate root behind the same features. The
  TLS and HTTP/2 `SpecError` enums are `TlsSpecError` and `H2SpecError`. The stealth
  layer is experimental and not a stable API.
- `docs/tutorials/stealth_scraping.md` and `docs/adr/003-stealth-scraping-layout.md`.
- A `rust-analyzer.toml` that loads the optional features so analysis covers
  `src/mcp` and `src/scrape`.
- `CLAUDE.md` (a map of the modules, checks and gotchas), `CHANGELOG.md`, and
  `docs/design/stealth_parity_plan.md` (the plan for the stealthscraper-rs
  feature set and JA3/JA4/Akamai impersonation).
- **Real referrals**: `BaseCase::generate_referral(start_page, destination_page,
  selector)`, `generate_traffic(.., loops, selector)`, `generate_referral_chain(pages)`
  and `generate_traffic_chain(pages, loops)` open the start page (or use the current
  one), build a link to the destination in it with a script and click it, so the
  destination receives a real `Referer`, as Python does. The four parity entries
  are `implemented`. Mock tests assert the order of the commands and that the URL
  stays inside its string literal; `tests/generate_referral_chrome.rs` (ignored)
  has a Chrome 155 visit a loopback site that records the `Referer` it receives.
- **`BaseCase::set_content_to_frame(frame)`** replaces the page with the content
  of an iframe on it, selected or named, as Python does; an iframe whose `src`
  is a web or file address opens in a new tab instead. `set_content_to_default`
  brings the page back (all swaps at once) and `set_content_to_parent` undoes
  one swap. With nothing swapped they still only leave the frame, as before.
  Tests against a fake browser behind the mock WebDriver cover the swap, the
  tab, nested swaps, a swap the page forgot and a missing frame;
  `tests/set_content_to_frame_chrome.rs` (ignored) does it in Chrome 155. The
  parity entry is `implemented`.
- **Downloads on the pure CDP engine**: `Page::download_file(url, dir)` has the
  page fetch an address and save it, `download_by_click(selector, dir)` follows
  the download a click starts, `save_file_as(url, path)` saves at an exact path
  and `assert_downloaded_file(name, timeout)` waits for a file in the folder the
  browser saves to; each returns the final path. They follow the browser's own
  download events (`Browser.setDownloadBehavior` with events), so a cancelled
  download, a bad HTTP status or a fetch the page may not make is
  `SeleniumBaseError::Download`, and a download that does not begin or end in
  time is `WaitTimeout`. `Browser::download_dir` says where downloads go. The
  three parity entries are `implemented`. Mock tests are in
  `tests/sb_cdp_download.rs`, Chrome 155 tests in
  `tests/sb_cdp_download_chrome.rs`.
- **Firefox add-ons**: `BaseCase::install_addon(path, temporary)` installs an
  `.xpi` file or an unpacked add-on directory through geckodriver's
  `moz/addon/install` command and returns the add-on id, and
  `uninstall_addon(id)` removes it with `moz/addon/uninstall`. Any other browser
  gets `SeleniumBaseError::Unsupported`; an empty path, one with `..` in it, one
  that does not exist and one that is neither an `.xpi` nor a directory are
  `InvalidConfig`, before anything is sent. The parity entry is `implemented`
  (it was `planned` and always returned `Unsupported`). Unit tests assert the
  exact endpoint and JSON body against the mock WebDriver. The same bodies were
  sent with `curl` to a real geckodriver 0.37.1 and Firefox 153 ESR in a Docker
  container (`docker/firefox-check`, which holds nothing of this crate): both
  an unpacked directory and an `.xpi` installed with `temporary: true`, the
  add-on's content script ran, and the uninstall stopped it.
  `tests/install_addon_firefox.rs` (ignored) runs `BaseCase` against that
  geckodriver and installs and removes an add-on.

### Changed

- `examples/tauri-profile-manager` is on Tauri 3: `tauri` `3.0.0-alpha.4`,
  `tauri-build` `3.0.0-alpha.3`, `tauri-plugin-opener` `3.0.0-alpha.2` and
  `@tauri-apps/cli` `3.0.0-alpha.4`, pinned exactly. No code had to change; on
  macOS its 115 tests pass and the Tauri 3 CLI builds the `.app` (the window was
  not opened in that check). One test of the example had gone stale when
  `Emulation::locale` started setting the language too; it now checks the last
  user-agent override.
- The `Dockerfile` links Debian's `chromedriver` as `downloaded_drivers/chromedriver` in
  the working directory, so the WebDriver engine in the image starts a browser with no
  network instead of downloading a driver, and its builder stage takes a
  `CARGO_BUILD_JOBS` build argument for small machines. `docker/Dockerfile.verify` and
  `docker/verify-linux.sh` run the test suite on Linux in a container (the sources
  mounted read-only, the build output in a volume, a virtual display for the headed
  tests). The suite, run that way on linux/arm64 with Chromium 154, is in
  `docs/platforms.md`.
- `sbase` exits with a status that follows the error's category instead of
  always 1, and prints `error: ...` instead of `[ERROR] ...`.
- `TestResult` has a `failure` field, and `PageEvidence` gains `current_url` and
  `page_title` (with defaults).
- The `cdp` MCP server no longer offers `tag:contains("text")` selectors, which
  its engine never supported; `diagnose_selector` gives the XPath for one. (A
  `:contains()` selector now does run, converted to XPath, in `Page::locator`.)
- **Breaking** (parity audit): `BaseCase::set_time_limit` now fails the test once
  it has run longer than the limit, at its next open, readiness wait or element
  action, and cuts waits to the time left; it used to only cap single waits.
  `set_default_timeout` raises or lowers the default wait (1 to 60 seconds)
  instead of setting the time limit, and `reset_default_timeout` restores it.
  `BaseCase::new` opens `start_page` when the configuration has one.
  `run_browser_test` (and `Plugins::run_browser_test`) count a test that ends in
  `skip_test` as passed and log the reason. `Emulation::locale` also sets
  `navigator.language` and `Accept-Language`, keeping the user agent and client
  hints. `Permission::ALL` has 23 entries instead of 7 and `Permission` has more
  variants. The parity entries for `--esc-end`, `--rec-sb-cdp` and
  `gui_click_captcha` are now `planned` (so were the ad-block options, until the
  WebDriver ad block above); the notes of many composed entries say what differs
  from Python.
- The pure CDP engine runs `session_registry` (27 providers: the default
  registry without `plugins` and `connection`), so a page reads Chrome's own
  `navigator.plugins`, `mimeTypes`, PDF viewer flag and `navigator.connection`;
  `navigator.userAgentData` is defined only in a secure context; and a desktop
  persona's screen is sent as a `0` by `0` device-metrics viewport plus
  `screenWidth` and `screenHeight`, so the page keeps its real viewport.
  `default_registry` still holds 29 providers.
- Requires Rust 1.99 (`rust-version`, the `Dockerfile` base image and the CI msrv
  job); `zip` moved to 9.0 and the lockfile is current.
- **Breaking**: `BaseCase::generate_referral`, `generate_traffic`,
  `generate_referral_chain` and `generate_traffic_chain` do what the Python
  methods do (see Added). Their old forms returned or opened a `?ref=<referrer>`
  URL, which sends no `Referer`; the old `(url, referrer)` signatures are gone.
- **Breaking**: `BaseCase::set_content_to_frame` takes the iframe to show (a
  selector or a name), not HTML to append as a new iframe; there is no
  replacement for the old behaviour, which `set_content` and `execute_script`
  cover.
- **Breaking**: `ad_block` in a WebDriver launch no longer passes
  `--blink-settings=imagesEnabled=false`. That turned every image off and blocked
  no ad; `with_block_images(true)` is the setting for it.
- `BaseCase` actions wait for their element (up to `set_timeout`, 10 s by
  default) and fail with `WaitTimeout` naming the selector.
- **Breaking**, from a parity audit (each method now does what the Python
  method of the name does): `press_keys(css, text)` types text into an element
  (the old one-named-key method is `press_key(name)`); `select_all(css)` returns
  the matching elements (the old select-every-option method is
  `select_all_options`); `bring_to_front(css)` raises an element (the window
  focus is `bring_active_window_to_front`); `console_log_script(script)` takes
  the expression to log; `deferred_check_window(name, level)` and
  `delayed_check_window(name, level)` take a name and a level;
  `assert_downloaded_file` is `async` and waits for the download;
  `switch_to_frame_of_element` returns the frame's name; `find_text`,
  `find_exact_text` and `find_non_empty_text` wait for visible text and fail with
  a timeout instead of returning `None`; `DeferredAssert` has three more
  variants. `check_window` at level 0 no longer fails on a difference.
- **Breaking**: `check_window` saves and compares the page's tag structure, as
  Python does, not the screenshot byte for byte. The baseline is the folder
  `visual_baseline/<test>/<name>` (`page_url.txt`, `baseline.png` and
  `tags_level_1.txt` to `tags_level_3.txt`); levels 1 to 3 compare the tags, the
  tags with their attribute names and the tags with their attribute values, and
  a level other than 0 to 3 is an error. A baseline saved as
  `visual_baseline/<name>.png` is not read; the first run saves a new one.
- `BaseCase::generate_referral`, `generate_traffic` and their chains,
  `set_content_to_frame` and `install_addon`, the `--reuse-session` options, the
  `caseplans` and `behave-gui` commands and three CDP downloads were marked
  `planned` in `parity/api.toml`, because the Rust code only looked like the
  Python one. The referral helpers, `set_content_to_frame`, `install_addon`, the
  CDP downloads, the `--reuse-session` options, the `caseplans` and `behave-gui`
  terminal apps and `gui_click_captcha` have since been written (see Added), so
  no entry is `planned` now.
  `objectify` is not applicable and `mkrec` is mapped onto `sbase record`.
- Waits report the error that stopped them, and link checks report real status
  codes.
- `Cookies::save` writes JSON, not Python pickles. The old `seleniumbase-mcp`
  tool names were replaced by the upstream names.
- **Breaking**: `SeleniumBasePlugin`, `PluginManager` and `DbReportingPlugin`
  were removed (nothing called the manager); `ElementApi::find_element` was
  removed from the trait; `From<AsyncStepRegistry> for StepRegistry` was removed;
  `common::{decorators, exceptions}` remain as `#[deprecated]` shims over
  `utilities::retry`.
- The real-Chrome tests of `MitmProxy` (`tests/impersonate_mitm_chrome.rs`) no
  longer all skip off the measured system: only the one that compares the browser
  with the measured persona does. The pin, loopback and SSRF refusal and
  WebSocket tests run on any Chrome or Chromium, one of another major being
  started with the profile's user agent. On linux/arm64 with Chromium 154
  (`docker/Dockerfile.verify`) seven of the eight now run and pass.

### Fixed

- **Windows**, found by the first runs on a Windows PC: every `sbase` command
  crashed at start with a stack overflow (a debug build needs more than the 1 MB
  a Windows main thread gets; `sbase` now runs on a 16 MB thread, and
  `tests/cli_e2e.rs` starts it under a 1 MB stack); the chromedriver for an older
  Chrome was never fetched, because `chrome.exe --version` prints nothing there
  (the version now comes from the folder beside the program, as in Python
  SeleniumBase); a driver whose session failed to start kept running (a dropped
  `DriverProcess` now ends it); text files were checked out with CRLF
  (`.gitattributes` makes them LF) and the parity scripts wrote CRLF; and four
  tests expected `/` in printed paths.
- **The `playwright` feature no longer needs Python.** `rustwright-core` was a
  direct dependency with its default `python` feature, which adds Python
  bindings through pyo3, so every program built with `playwright` linked
  `libpython` and failed to start without it (`dyld: Library not loaded:
  @rpath/libpython3.14.dylib`). It is now used with `default-features = false`,
  as `rustwright` itself does: pyo3 is gone from the dependency tree, the 54
  `playwright` library tests pass, and the test binary links no `libpython`.
- **Browsers no longer outlive the program that started them.** A chromedriver,
  the Chrome it starts, and a Chrome launched by the pure CDP engine kept running
  when the program that started them was killed (an interrupted test run, a
  stopped MCP server) or, for `--reuse-session`, simply ended. On macOS a leftover
  Chrome could hold the person's own profile, so the Chrome they opened closed at
  once. On Unix each launched driver or browser now joins a process group led by
  a small `/bin/sh` watcher that reads a pipe this program holds; when the
  program ends, by any means, the watcher sends the group SIGTERM and then
  SIGKILL. Checked by killing `sbase` and `seleniumbase-mcp --server cdp` with
  SIGKILL while Chrome 155 was open (nothing was left), and by unit tests,
  including one whose parent process is killed. Windows is not covered.
- `sbase cdp`, `cache-clear`, `throttle3g`, `cdp-click-element`, `cdp-type-text`,
  `print-pdf`, `pdf-text` and `assert-pdf-text` failed with "CDP is unavailable"
  unless `--cdp` was given; with no mode asked for they now use cdp mode on a
  Chromium browser. `screenshot --path` and `save-source --path` wrote into
  `latest_logs/` while printing the path given; they now write that path.
  `run-scenario` exited 0 when a step failed; it now exits 1. Text read from a
  PDF spells the ligatures U+FB00 to U+FB06 as their letters, so "fixture"
  printed by Chrome with an fi glyph is found. Found by the new
  `tests/cli_e2e_chrome.rs`, which runs the browser commands as the real binary
  on headless Chrome.
- `sbase decrypt` with the wrong passphrase said `unsupported operation:
  decryption failed: ring::error::Unspecified` and advised on browsers and engines;
  it now says the passphrase in `SB_ENCRYPTION_KEY` is wrong or the token was
  modified (exit status 2 as before). `sbase import-python` with a file it cannot
  read said only `No such file or directory (os error 2)`; it now names the file.
  `tests/cli_e2e.rs` runs every subcommand's `--help` and the offline commands.
- Log lines went to standard output, in colour even when the output was a file or
  a pipe. `seleniumbase-mcp` therefore wrote them into the JSON-RPC stream (three
  lines in a session that did nothing but connect), which a client reads as
  corrupt messages, and `sbase get-title | ...` carried the logs with the
  answer. The `init_tracing*` functions now write to standard error, with colour
  only on a terminal and not when `NO_COLOR` is set. `tests/mcp_stdio_chrome.rs`
  speaks JSON-RPC to the real binary and fails on any line that is not a message.
- The three MCP servers answered `initialize` with the server name `rmcp` and the
  library's version (`3.5.1`), so a client listed `cdp`, `driver` and `sb` as the
  MCP library. They now report `seleniumbase-cdp`, `seleniumbase-driver` and
  `seleniumbase-sb` with this crate's version.
- `sbase open`, `smoke` and the other commands that drive a browser ignored
  `SB_HEADLESS` and the other `SB_*` settings variables unless `--config` was
  given, and ignored `SB_WEBDRIVER_URL` and `SB_CHROME_BIN`; in a container that
  made `--headless` mandatory. They now start from the config file, apply the
  variables on top, and then the command-line options, so an explicit
  `--headless` or `--headed` still beats the environment. `--webdriver` now
  defaults to `SB_WEBDRIVER_URL`, then `http://localhost:4444`, and a bad
  variable value (`SB_HEADLESS=maybe`) stops the command with an error before
  a browser starts. `tests/cli_env.rs` runs the real binary against a fake
  WebDriver server.
- The chromedriver download asks Chrome for Testing for the `linux-arm64` build on
  an arm64 Linux machine (a Docker container on an Apple-silicon Mac, or on
  Graviton) instead of the x86-64 `linux64` one, which cannot run there: the
  WebDriver engine failed with "chromedriver exited as it started". The build
  exists from Chrome 153 on; on Linux machines of any other architecture the
  error now says that no build is published and where to put a driver.
- The real-Chrome tests that compare a browser with the measured Chrome 155 on macOS
  (`impersonate_capture_h2_chrome`, `impersonate_http_chrome`, `impersonate_mitm_chrome`,
  `stealth_session_chrome`) say they are skipped and pass on a system that has no
  measurement (Linux, Windows), instead of failing on `NotMeasured`. A macOS machine whose
  Chrome has another major version still fails them: that is the drift check.
- The wire test `a_custom_ja3_arrives_exactly_as_given` failed on about two runs in
  five, on any system: BoringSSL pads a ClientHello of 256 to 511 bytes up to 512
  with a padding extension, and the "classical groups only" variant falls on either
  side of that limit with the random size of the ECH GREASE payload. The test now
  asserts that rule (a padded hello is 512 bytes, an unpadded one is not in the
  range) and compares the JA3 and JA4 without the padding.
- `BrowserSession::reconnect` stops the chromedriver of the previous session when
  the new one starts its own, so `uc_open_with_reconnect` no longer leaves one
  chromedriver running per call.
- `js_click`, `js_type`, `set_attribute`, `remove_attribute`, `scroll_to`, the
  localStorage helpers and every other `BaseCase` page script quote their
  selectors and values with the crate's `js_code::quote`; a quote, a newline or
  U+2028 in test data could break out of the string. Waits that run out of time
  (`wait_for_title`, `wait_for_attribute`, `wait_for_exact_text`, the link-text
  waits, `wait_for_any_of_elements_present`, `download_file`) return `WaitTimeout`
  instead of `AssertionFailed` and name what they waited for.
  `wait_for_ready_state_complete` takes its default from the configured timeout.
- `BrowserSession::wait_for_element`, `wait_for_element_visible`,
  `wait_for_element_clickable`, `wait_for_element_absent` and `wait_for_text`
  return `WaitTimeout`, not `AssertionFailed`, when the time runs out, and so
  do the `BaseCase` methods that return their error as it is (`wait_for_element`,
  `wait_for_element_visible`, `wait_for_element_absent`, `wait_for_text`,
  `find_element`, `choose_file`, `slow_click`, `get_select_options` and
  `get_shadow_root`). The message names the locator (`CSS(#a)`) or the text, and
  the seconds waited. Code that matched `AssertionFailed` on these waits must
  match `WaitTimeout`.
- `CdpPage` (the DevTools page of a WebDriver session) quotes the selectors,
  attribute and property names, option texts and values it puts into its page
  scripts with `js_code::quote`, as `BaseCase` does. It escaped only the
  backslash and the single quote, so a newline or U+2028 in a value (the text of
  a `<textarea>`) was a syntax error in the script.
- The examples run offline against a loopback site (`examples/local_site`) and
  check what they claim; the `partial link=` selector prefix in the selectors
  tutorial is `partial_link=`.
- The native pointer helpers of the `gui` feature (`gui_click_element`,
  `gui_click_with_offset`, `gui_drag_and_drop`, `gui_move_to_element`,
  `gui_hover_and_click` and `uc_gui_handle_captcha`) aim at the element's place on
  the screen. The script that finds it scrolls the element into view and adds the
  browser toolbar, the window borders and the offset of each same-origin frame the
  element is in; it used to add only the window's position, so a click in a headed
  window landed a toolbar's height too high. `uc_gui_handle_captcha` waits for the
  element to be visible first. Browser pixels are taken to be screen pixels.
- `get_domain_url` keeps a port that is not the scheme's default
  (`http://localhost:8080`), as Python's does; it used to drop it.
- `wait_for_element_present` looks for the element once before it gives up, so an
  element that is already there is found with a timeout of zero.
- `assert_is_instance` and `assert_not_is_instance` look at the type of the value
  passed. They read the name of a `&dyn Any`, which is always `dyn core::any::Any`,
  so the first never passed and the second never failed. They take `&T` now; a
  call that passes `&value` compiles as before.
- `xpath_to_css` (so `convert_xpath_to_css` and the Python importer) converts only
  what has an exact CSS equivalent, a tag or `*` with attribute tests joined by
  `and`, and returns `InvalidSelector` for the rest. It used to drop a predicate it
  did not understand: `//div[2]` became `div` and an `or` became an AND. An id with
  a backslash or a line break no longer produces invalid CSS in `Selector::Id`,
  and `..` and `../` count as XPath.
- `normalize_page_url` (`open`, `Page::goto`, the scrape and impersonation fetches)
  matches schemes without regard to case, so `HTTPS://EXAMPLE.COM` is left alone,
  and refuses a file path and a scheme a browser cannot open (`ftp:`, `mailto:`,
  `javascript:`) with a message that says so, instead of turning them into an
  `https://` host.
- `Page::close` (pure CDP) waits, for at most two seconds, until the browser no
  longer lists the tab; `Browser::list_pages` right after a close could still show
  it, which made a real-Chrome test fail one run in five.
- `sbase objectify`, `caseplans`, `behave-gui`, `mkrec`, `mkchart`, `mkpres` and
  `print` exit with status 6 and print the cause when they cannot write or read
  their file; they used to exit 0. The WebDriver-hosted CDP page no longer sends
  `DOM.enable` (it finds elements through `Runtime.evaluate`), so no part of the
  engine enables a detectable domain.
- A chromedriver that does not match Chrome (the cause of "chromedriver keeps
  crashing"): the launcher, `sbase install` and `sbase doctor` now agree with the
  installed Chrome, and a replaced driver is written as a new file because macOS
  kills a signed binary overwritten in place.
- The stealth layer no longer enables the `Runtime` CDP domain; the proxy-auth
  extension is one Chrome can load; the page-script stubs were
  replaced with working scripts (checked in a real Chrome); Playwright honours
  `BrowserConfig` and keeps web security on.
- `CdpDriver` sends commands to the tab, not the browser socket; `prompt()` asks
  for text; deferred assertions check the page they were made for; chart series
  line up by label; the presentation escapes its title; the HTML inspector gives
  each issue a selector that matches its element; `get_origin` keeps the port;
  PDFs that need a password can be read.
- A WebDriver session with a fingerprint no longer sends the proxy login
  (`Proxy-Authorization`) to every site as an extra header; the pure CDP path
  already filtered it.
- A flaky Grid test (the stand-in `java` was slow on its first run).
- Parity audit: `set_text`, `set_text_content` and `highlight_update_text` set
  an input's value instead of its (invisible) `textContent`; the deferred
  asserts check at once, `deferred_assert_exact_text` is exact and
  `deferred_assert_non_empty_text` checks text; `switch_to_frame_of_element`
  finds elements in iframes; `select_option_by_index` fails for a missing option
  and fires `change`; `get_chrome_version` returns the version number and
  `get_chromium_version` the major version; `is_valid_url` follows Python's
  pattern; `ad_block` uses upstream's selector list; `add_js_code_from_link`
  downloads and injects the code; `post_success_message` and the other toasts go
  away; `highlight` restores the element's outline; `delete_saved_cookies`
  deletes the file `save_cookies` wrote; `console_log_string` writes to the
  console.
- The recorder generates Rust for every recorded action, keeps events across
  navigations, and the stdin recorder only says "recorded" for what it recorded.
- The CLI scaffolders no longer overwrite files; the interactive commands stop at
  the end of their input and on Ctrl-C; Gherkin runs the way `behave` does.
- `sbase translate` and `translate_scenario` rewrite the `action` of the steps in
  the root `steps` list only, not an `action` in any other object three levels
  down (such as `meta.x.action`), which was changed and counted as translated.
- `EgressRef::parse` lower-cases the host of `socks5` and `socks5h` specs, as it
  does for http and https, so one proxy spelled two ways has one `EgressKey`.
- `ActionRecorder::to_gherkin` skips a recorded `type_text` with no text and says
  so in a `# skipped:` comment; the written `I type "" into "..."` step matched
  no handler, so the generated scenario failed at that step.
- `sbase translate --overwrite` keeps the file's permissions (the replacement was
  left at mode 0600) and writes through a symbolic link instead of replacing it.

### Security

- Chrome switches in a profile payload that run programs, load code or reroute
  traffic are refused; file names a caller supplies cannot climb out of their
  folder; the HTML report escapes what tests print; proxy passwords no longer
  appear in `Debug` output.
- `ActionRecorder::to_gherkin` escapes the name of an action it cannot turn into
  a step, so a recorded name with a line break can no longer end the
  `# skipped:` comment and add a Scenario or steps to the feature.
- `capture_debug_bundle` no longer copies what earlier tool calls returned into
  `bundle.json`: the result of a successful call is replaced by its length, so a
  cookie, a stored token or a page's text read by `get_cookies`,
  `manage_cookies` or a storage tool is not saved with the bug report.
