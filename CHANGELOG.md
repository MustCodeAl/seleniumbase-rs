# Changelog

All notable changes to `seleniumbase-rs`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The crate has not been
released yet (`0.1.0`), so everything below is under *Unreleased*; the target is
feature parity with Python SeleniumBase 4.55.2 and `seleniumbase-mcp`
(`ROADMAP.md` has the open work, `docs/parity.md` the per-API status).

## Unreleased

### Added

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
- **Parity**: every API in `parity/api.toml` now has a status (861 implemented,
  375 composed, 112 not-applicable with a reason, 17 planned). The composed and
  not-applicable entries are still to be reviewed.
- **Stealth scraping groundwork** (features `scrape` and `impersonate`, in
  `src/wire`, `src/scrape` and `src/impersonate`): egress redaction, routes, geo
  locales and a proxy pool; bot-protection challenge detection and a
  mitigation policy; per-site state with an atomic JSON store. The JA3/JA4/Akamai
  wire work and the TLS proxy are not written yet.
- A `rust-analyzer.toml` that loads the optional features so analysis covers
  `src/mcp` and `src/scrape`.
- `CLAUDE.md` (a map of the modules, checks and gotchas), `CHANGELOG.md`, and
  `docs/design/stealth_parity_plan.md` (the plan for the stealthscraper-rs
  feature set and JA3/JA4/Akamai impersonation).

### Changed

- `sbase` exits with a status that follows the error's category instead of
  always 1, and prints `error: ...` instead of `[ERROR] ...`.
- `TestResult` has a `failure` field, and `PageEvidence` gains `current_url` and
  `page_title` (with defaults).
- The `cdp` MCP server no longer offers `tag:contains("text")` selectors, which
  its engine never supported; `diagnose_selector` gives the XPath for one.
- Requires Rust 1.99 (`rust-version`, the `Dockerfile` base image and the CI msrv
  job); `zip` moved to 9.0 and the lockfile is current.
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
- `BaseCase::generate_referral`, `generate_traffic` and their chains,
  `set_content_to_frame` and `install_addon`, the `--reuse-session` options, the
  `caseplans` and `behave-gui` commands and three CDP downloads are now `planned`
  in `parity/api.toml`, because the Rust code only looked like the Python one.
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

### Fixed

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
