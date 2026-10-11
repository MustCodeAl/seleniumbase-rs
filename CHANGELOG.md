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
- **Parity**: every API in `parity/api.toml` now has a status (862 implemented,
  374 composed, 112 not-applicable with a reason, 17 planned). The composed and
  not-applicable entries are still to be reviewed.
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
  bodies, and a hot-swappable egress and profile. WebSocket upgrades are refused,
  and a request claiming another Chrome major is refused. Measured on Chrome 155
  (headless, macOS): a chain carrying the pinned leaf key is accepted by a Chrome
  that has the SPKI-list switch and no proxy, and is never served to a page behind
  the proxy.
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

### Changed

- `sbase` exits with a status that follows the error's category instead of
  always 1, and prints `error: ...` instead of `[ERROR] ...`.
- `TestResult` has a `failure` field, and `PageEvidence` gains `current_url` and
  `page_title` (with defaults).
- The `cdp` MCP server no longer offers `tag:contains("text")` selectors, which
  its engine never supported; `diagnose_selector` gives the XPath for one.
- The pure CDP engine runs `session_registry` (27 providers: the default
  registry without `plugins` and `connection`), so a page reads Chrome's own
  `navigator.plugins`, `mimeTypes`, PDF viewer flag and `navigator.connection`;
  `navigator.userAgentData` is defined only in a secure context; and a desktop
  persona's screen is sent as a `0` by `0` device-metrics viewport plus
  `screenWidth` and `screenHeight`, so the page keeps its real viewport.
  `default_registry` still holds 29 providers.
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
- **Breaking**: `check_window` saves and compares the page's tag structure, as
  Python does, not the screenshot byte for byte. The baseline is the folder
  `visual_baseline/<test>/<name>` (`page_url.txt`, `baseline.png` and
  `tags_level_1.txt` to `tags_level_3.txt`); levels 1 to 3 compare the tags, the
  tags with their attribute names and the tags with their attribute values, and
  a level other than 0 to 3 is an error. A baseline saved as
  `visual_baseline/<name>.png` is not read; the first run saves a new one.
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
