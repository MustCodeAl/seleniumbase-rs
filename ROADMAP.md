# Roadmap

What is done, what is missing, and what to build next, in order. Update the
status lines as work lands. The goal is feature parity with the latest Python
[SeleniumBase](https://github.com/seleniumbase/SeleniumBase) (4.55.2) and
[seleniumbase-mcp](https://github.com/seleniumbase/seleniumbase-mcp), designed
the way a Rust library would be, not copied from Python's architecture.

Design rules that apply to everything below:

- One canonical name per capability. No aliases (`type_text`, not `type` and
  `type_text`).
- Strong types and enums over booleans and strings; builders for complex
  construction; errors as `SeleniumBaseError`.
- Every feature ships with tests. Browser-dependent tests are `#[ignore]` and
  named in the table below; everything else runs in plain `cargo test`.
- Quality gate: `cargo fmt --all -- --check`, `cargo clippy --all-targets
  --features "mcp-server test-util" -- -D warnings`, `cargo test --features
  "mcp-server test-util"`, `cargo check --no-default-features`, `cargo deny
  check`. (`--all-features` needs a `libpython` for the `playwright` feature.)

## Done

| Area | State |
| --- | --- |
| Dependencies | All direct crates at their latest releases; no git sources. |
| Request interception and per-context proxies | `Page::intercept` with typed rules; `ContextOptions` and `BrowserPool::acquire_with` give a lease its own proxy, with the password answered per session. Verified on real Chrome. |
| Fingerprint on Pure CDP, shared traits | `LaunchOptionsBuilder::fingerprint`, `Page::apply_fingerprint`, `Page::js_errors`, and the four capability traits implemented for `sb_cdp::Page`. Verified on real Chrome 155. |
| Test plugins | `plugins::observer::{Plugins, TestPlugin, PageEvidence}`, with `ScreenshotOnFailurePlugin`, `PageSourceOnFailurePlugin`, `ReportPlugin` and `ResultStorePlugin`. Hooks run around a test and see the page on failure, with `BaseCase` and `sb_cdp::Page`. Tested without a browser and through the Pure CDP mock; the `BaseCase` runner is not yet run on a real driver. |
| Turso storage (`turso` feature) | `ResultStore` and `sbase report` for test-run history and flaky tests; `ProfileVault` for encrypted profiles. |
| Browser pool | `BrowserPool`, `Lease`, `BrowserContext`, `SessionStore`: bounded, fair, isolated, recycled, with in-memory session sharing. Verified on real Chrome. |
| WebRTC leak shield | `Page::webrtc_report`, `Page::shield_webrtc`, `LaunchOptionsBuilder::{shield_webrtc, webrtc_policy}` (the existing `WebRtcPolicy`). Verified on real Chrome. |
| Behavioural stealth engine | `stealth::behavior` (pure, seedable) and `Page::human` (click, type, scroll at a human pace). Verified on real Chrome. |
| Profile randomisation | `Fingerprint::randomized(os, seed)`: a coherent identity (WebGL, hardware, locale, time zone, Client Hints, noise seed) for all five `OsType`s that passes `validate()` for every seed tried (1,500 per OS plus edge cases). Not yet run against a real fingerprinting page. |
| Pure CDP engine (`sb_cdp`) | `Browser`, `Page`, `Locator`, input, cookies/storage/window/emulation, retrying assertions, mock browser, CAPTCHA solving, same-origin frames (`page.locator("#frame").locator("button")`). Verified on real Chrome (`tests/sb_cdp_chrome.rs`). |
| MCP `cdp` server | 24 tools, mock-tested (`tests/mcp_cdp.rs`) and verified on real Chrome (`tests/mcp_cdp_chrome.rs`). |
| MCP `driver` / `sb` servers | 26 and 88 tools plus 8 stealth tools; catalogue and offline behaviour tested (`tests/mcp_webdriver.rs`). |
| `seleniumbase-mcp` binary | `--server cdp\|driver\|sb`, default `sb`. |
| `sbase` CLI | `encrypt` and `decrypt` (AES-256-GCM, passphrase from `SB_ENCRYPTION_KEY`). |
| BaseCase | Actions wait for their element; `nested_click`, `solve_captcha`, `fast_type`, `js_click_if_visible`, `get_gui_element_rect/center`, `jq_format`, `post_message`, `save_as_html_to_logs`, `save_teardown_screenshot`, `switch_to_default_driver`, `wait_for_angularjs`, `get_saved_cookies`. |
| Toolchain and dependencies | Rust 1.99 (`rust-version`, the `Dockerfile` and the CI msrv job), `zip` 9.0 and a current lockfile. `cargo audit` reports no vulnerabilities (three unmaintained-crate warnings: `bincode` 1.3.3, `fxhash`, `ttf-parser` 0.25.1); `cargo deny check` passes. |
| Chromedriver that matches Chrome | `browser::driver_version` compares major versions. The launcher downloads the driver of Chrome's milestone when the cached one differs, and `sbase install` and `sbase doctor` make the same check. A replaced driver is written as a new file, because macOS kills a signed binary that is overwritten in place. Driver 155.0.8059.40 is installed here; the real-driver suites in section 1 are still to be run. |
| Parity honesty audit | 26 manifest entries that did less than their Python method were fixed (with tests; the page scripts are in `src/api/page_scripts.rs`) or re-marked in `parity/api.toml`; the 196 composed CDP entries carry a real note each. Unit tests run without a browser; the real-browser checks are `#[ignore]`d in `src/api/page_scripts.rs` (pure CDP) and `tests/parity_honesty_chrome.rs` (chromedriver). |
| Per-file review pass | Bugs found by reading were fixed, each with a test, across the book (`docs/`), `src/api` (the top-level modules and `base_case_impls`), `behave`, `cli`, `common`, `plugins`, the stealth page scripts, `utils/extensions` and the Playwright path. The CLI generators and recorders are run as the real binary in `tests/cli_generators.rs` and `tests/cli_templates.rs`. |
| Recorder | Every action `BaseCase` records or the live recorder captures becomes a call in the generated Rust. What cannot be generated becomes a `// skipped:` comment that says why, and the stdin recorder (`ActionRecorder::try_record`) refuses such a line and names the closest known action. |
| Knowledge map and plans | `CLAUDE.md` (module map, checks, gotchas) and `docs/design/stealth_parity_plan.md` (20 work packages for the stealthscraper-rs feature set, JA3/JA4 and Akamai impersonation). |
| Stealth scraping foundation (`scrape`, `impersonate`) | Work packages 0 to 17 of the plan are merged: wire parsers (`ClientHello`, JA3, JA4, peetprint, Akamai); the measured table with one entry, Chrome 155 on macOS, and the capture tools (`capture_tls`, `capture_hints`, `capture_h2`); `ImpersonationProfile`, `CustomProfileBuilder` and the wreq client; an RFC 6265 cookie jar; `StealthIdentity` and its identity file; `SessionPolicy`; challenge detection and a mitigation policy; a proxy pool; per-site state; `BrowserScraper`; the ephemeral-CA TLS server; and the hardening of the pure CDP page script (`session_registry`). Tested with committed captures, a scripted browser and `#[ignore]`d tests that drive a local Chrome (`tests/*_chrome.rs`), not against live bot protection. Only macOS and Chrome 155 are measured. See `docs/tutorials/stealth_scraping.md`. |
| `HttpSession`, `MitmProxy`, `StealthSession` (`impersonate`; work packages 14 to 16) | `HttpSession` continues a browser page's session as plain requests with the measured handshake and headers (an RFC 6265 jar, redirects by hand, bounded bodies, challenge judging, the hand-over from a page and back). `MitmProxy` is a loopback proxy that ends a real Chrome's TLS with the ephemeral CA and replays each request through the impersonating client; WebSocket is relayed as `wss://` only. `StealthSession` moves between the two by the mitigation and session policies and starts the proxy only when a browser is needed. Tested against loopback sites, a scripted browser and, in `#[ignore]`d tests, a local Chrome 155 on macOS run headless (`tests/impersonate_http_chrome.rs`, `tests/impersonate_mitm_chrome.rs`, `tests/stealth_session_chrome.rs`); the trust of the SPKI-list switch is measured there too. Not run against a live service, and not a stable API. Only macOS and Chrome 155. |

## 1. Finish the parity upgrade (next)

1. **Verify the WebDriver MCP servers on a real driver.**
   `tests/mcp_webdriver_chrome.rs` is written but unrun. The chromedriver now
   matches Chrome (155; `sbase install` fetches the right one), so run it,
   `tests/browser_smoke.rs` and `tests/mcp_cdp_chrome.rs`, one binary at a
   time, and fix whatever they find.
2. **BaseCase actions wait (done; verify on a real driver).** `click`,
   `type_text`, `get_text`, `hover`, `submit`, ... now poll for their element up
   to `set_timeout` (10 s default) and fail with `WaitTimeout` naming the
   selector. The three `tests/browser_smoke.rs` tests that prove it
   (`an_action_waits_for_an_element_that_appears_late` and two more) are
   written but unrun (see item 1). Per-call
   timeout variants are still to do; the MCP layer passes its own timeout by
   waiting first.
3. **BaseCase semantic differences (documented; decide which to remove).**
   Eighteen are recorded as `divergence` notes in `parity/api.toml` and shown
   in `docs/parity.md`: `click_nth_visible_element` counts from 0, `scroll_up`
   and `scroll_down` take no amount, `open_new_tab` always switches,
   `download_file` goes through the browser, cookie files are JSON, and the
   argument order of `assert_text`, `assert_exact_text` and `wait_for_text`
   differs. Changing any of them shifts existing callers, so do it as a
   deliberate breaking release, not quietly.
4. **Python names with no Rust counterpart (done).** Every one is classified
   in `parity/api.toml`: `get_jqc_button_input`, `get_jqc_form_inputs`,
   `get_jqc_text_input` and the unittest harness (`setUp`, `tearDown`,
   `setUpClass`, `tearDownClass`, `main`, `run`, `skip`, `has_exception`) are not
   applicable; `get_google_auth_password` and `type` are composed.
5. **`sbase` CLI gaps.** Four commands are `planned` because the Rust command
   only looks like the Python one: `caseplans` and `behave-gui` (and their
   aliases `case-plans` and `gui-behave`) write a fixed file instead of
   launching the Case Plans Generator and Behave Commander GUIs. `objectify` is
   not applicable (it rewrites Python source) and `mkrec` is mapped onto
   `sbase record`. `sbase` now exits with status 6 and prints the cause when
   one of the file-writing commands fails.
6. **pytest-style options and settings.** Compare the Python `--option` list
   and `SB()`/`Driver()` kwargs with `BrowserConfig`/`RuntimeConfig`; add the
   missing ones.
7. **Pure CDP: verify the compositions.** Every `sb.cdp.*` method now has a
   mapping in `parity/api.toml`, checked to exist. Check that each composition
   behaves like the Python method (waits, visibility rules), with real-Chrome
   tests for the ones that matter.

## 2. Make future upstream updates cheap (done; keep the manifest current)

Built: `parity/upstream.toml`, `tools/parity/extract_api.py`,
`tools/parity/seed_manifest.py`, `tools/parity/render_docs.py`,
`parity/api.toml`, `tests/parity.rs`, `docs/UPSTREAM_SYNC.md`, the generated
`docs/parity.md`, `just parity-*` recipes and a weekly `upstream-watch`
workflow (untested: Actions are billing-locked).

Current state against 4.55.2 (`just parity-docs` has the full table): 862 entries are implemented, 375 composed from existing Rust APIs, 96 not applicable with a stated reason and 32 `planned`. An audit of the entries marked done (three reviewers, two skeptics per finding) found 26 whose Rust did less than, or something other than, the Python method. The methods were fixed where the change was modest (`press_keys`, `select_all`, `set_text`, the deferred asserts, `switch_to_frame_of_element`, `select_option_by_index`, `bring_to_front`, `find_text`, `highlight`, `ad_block`, `check_window` (now the tag-structure levels, as Python 4.55.2 does), and others), and the entries were re-marked where it was not. A second audit of the composed and not-applicable entries found 34 that said more than the Rust does. Fixed in the code: text selectors (`SelectorBuf::containing_text`, and `:contains()` in any selector), `Page::evaluate` dropping a final `return`, `goto_if_not_url`, `active_element`, `Locator::{all_waiting, viewport_rect, click_and_hold, flash_with}`, `Browser::{debugging_host, grant_all_permissions, grant_permissions_for}` with all 23 permissions, `Emulation::locale` setting the language, a real test time limit and `set_default_timeout`, the start page opening with `BaseCase::new`, skipped tests, `sbase version`, `obfuscate` and `unobfuscate`. Re-marked: the ad-block options (the WebDriver engine only turns images off), `--esc-end`, `--rec-sb-cdp` and `gui_click_captcha` became `planned`; the settings file, the failure-artifact options, `skip` and `--rec-sb-mgr` became `composed`; `enable_ws` and `sbase download` became not applicable; and many notes now say what differs. The 32 `planned` entries are the work still to do:

- `BaseCase` (6): `generate_referral`, `generate_referral_chain`, `generate_traffic`, `generate_traffic_chain` (a real click-through between two pages, not a `?ref=` parameter), `set_content_to_frame` (swap the page for an iframe's content) and `install_addon` (Firefox add-ons through geckodriver).
- pytest options (13): `--reuse-session`, `--reuse_session`, `--rs`, `--rcs`: `BrowserConfig::reuse_session` is stored but nothing shares a browser across a run. `--ad-block`, `--ad_block`, `--adblock`, `--block-ads`, `--block_ads`: a WebDriver launch only turns images off and blocks no ads (the pure CDP engine and `BaseCase::ad_block` do). `--ee`, `--esc-end`, `--esc_end`: nothing injects the ESC listener that skips the test. `--rec-sb-cdp`: no recording is written as pure CDP code.
- `SB()` arguments (3): `ad_block`, `ad_block_on`, `rec_sb_cdp` (same reasons).
- `Driver()` arguments (2): `ad_block`, `ad_block_on`.
- `sbase` (4): `caseplans`, `case-plans`, `behave-gui`, `gui-behave`.
- CDP engine (4): `download_file`, `assert_downloaded_file`, `save_file_as`, `gui_click_captcha` (a click with the operating system's pointer; `Page::solve_captcha` clicks with DevTools events).

What is left beyond those is depth: the run-wide knobs (`RunSettings`) cover demo, slow and fast pauses on `open`, `click`, `type_text`, `update_text` and `press_keys` only; and the compositions still want the real-Chrome checks of item 7 above (each composed CDP entry now says how to write it in Rust, but its waits and visibility rules have not been compared with Python's one by one).

## 3. Documentation

`sb_cdp` and MCP chapters in the book; a `.mcp.json` example for each
`--server`; update `DEVELOPER_GUIDE.md`'s module map; the parity table from
step 2.

## 4. Quality and CI

- Extend the Microsoft Pragmatic Rust Guidelines lint set (now on `sb_cdp` and
  `mcp`) to the rest of the crate, module by module.
- GitHub Actions on the account are billing-locked, so no workflow has run
  since the layout change. Re-run them once billing is fixed, starting with the
  `ubuntu-latest` image change on 2026-10-19.
- `cargo deny` reports one advisory-ignore entry that no longer matches; remove
  it.

## 5. After the upgrade is published

### Turso (optional `turso` feature, off by default)

- Done: `storage::ResultStore` (runs, results, flaky-test detection), the
  `sbase report` command, and `storage::ProfileVault` (profiles sealed with
  AES-256-GCM, bound to their id, passphrase changeable in one transaction).
  Covered by unit tests, `tests/cli_report.rs` and doctests. See
  `docs/tutorials/results_and_profiles.md`.
- Done: the Tauri example (`examples/tauri-profile-manager`) keeps profiles,
  tags and folders in a `ProfileVault` instead of clear-text JSON, migrating the
  old files only after reading every document back and comparing it. The vault
  passphrase comes from `SB_PROFILE_PASSPHRASE` or the OS keychain. It also has a
  Pure CDP engine (isolated context, per-profile proxy, WebRTC shield) and a
  Randomize action built on `Fingerprint::randomized`. Verified: the example's
  tests and one real-Chrome launch. Not verified: the real keychain, the Tauri
  window, and the WebDriver/Docker path.
- To do: wire `record_outcome` into `run_browser_test` and the Python-style
  `with_db_reporting` / `database_env` options (`Plugins::run_browser_test` and
  `ResultStorePlugin` are the place, and now exist; what is left is a
  `BrowserConfig`/environment switch that attaches them).
- Known: the feature needs Rust 1.90 (`roaring`, via `turso_core`), which the
  crate's `rust-version` (1.99) covers. `cfg_block`, also via `turso_core`,
  declares no licence in its manifest; `cargo deny check` passes.

### Five architecture improvements

1. **Behavioural stealth engine (done).** `stealth::behavior` plans curved
   pointer paths (Fitts's law, minimum-jerk speed, tremor, overshoot) and
   Gaussian typing with optional typos; `Page::human` plays them on a page.
   Still to do: a WebDriver `human_click` with a real pointer path (needs a
   working driver to verify), and wiring `Fingerprint::humanize` through
   `sb_cdp::Browser` once item 5 lands.
2. **Async browser pool (done).** `BrowserPool`/`Lease` over isolated
   `BrowserContext`s, with an in-memory `SessionStore` for cookie and local-
   storage sharing. Verified on real Chrome. Still to do: a `Fingerprint` per
   lease (item 5; `Fingerprint::randomized` now supplies the identities), and a
   per-lease proxy (item 3), both of which build on `BrowserContext`.
3. **CDP request interception (done).**
   - Done: `Page::intercept` with typed `Rule`s (block, fulfil, modify) and a
     request log, verified on real Chrome. The existing `CdpReactor` was checked
     and does work (it adds headers to every tab's requests); it is global and
     header-only, so `Page::intercept` is the per-page replacement.
   - Done: per-context proxy routing with per-session password handling
     (`ContextOptions`, `acquire_with`), verified on real Chrome against a
     password-protected proxy.
   - Done: WebRTC leak shielding. A probe (`Page::webrtc_report`) that
     classifies the candidates a page can gather, and a relay-only shield
     (`Page::shield_webrtc`, `LaunchOptionsBuilder::shield_webrtc`). Verified on
     real Chrome 155: the default gathers two `.local` host candidates, and so
     does every value of Chrome's own `--force-webrtc-ip-handling-policy`; the
     shield gathers none. `sb_cdp` reuses the existing `WebRtcPolicy` for the
     flag rather than defining its own.
4. **Hardware and profile randomisation (done).**
   `Fingerprint::randomized(os, seed)` draws a whole machine (a plausible GPU
   or Apple chip with the cores, memory and screens that machine ships with) and a
   whole place (locale, time zone, coordinates in that zone's city), then
   derives the WebGL strings and PCI ids, user agent, `navigator` values, Client
   Hints (built with Chromium's own decoy-brand algorithm) and a canvas/audio
   noise seed from them. It supports all five `OsType`s, reuses the seedable
   `humanize::Rng` (which gained `next_u64` and `pick`), and passes
   `validate()` with no warnings for every seed tried: 1,500 per OS plus edge
   cases such as `0` and `u64::MAX` (`tests/fingerprint_randomized.rs`). That
   is a sample, not a proof over all 2^64 seeds. Limitations:
   - The tables are plausible, not sampled from real traffic, and nothing has
     been run against a real fingerprinting page or a live bot-detection
     service.
   - The user agent claims one of a fixed window of Chromium majors (153-155)
     and a few Mobile Safari releases, and Client Hints report full versions as
     `major.0.0.0`. Refresh the window with each release; a browser of another
     version contradicts the identity.
   - Fonts, media devices and the proxy are left unset.
   - A seed maps to the same identity only within one crate version, because
     adding a table row shifts the draws; persist the `Fingerprint`, not the
     seed.
   - Not wired into `sb_cdp::Browser` (item 5).
5. **Integration (done).**
   - `LaunchOptionsBuilder::fingerprint(&Fingerprint)` and
     `Page::apply_fingerprint` make tabs match a fingerprint through the
     existing `evasions::{bootstrap_script, cdp_overrides}`. The proxy-password
     header is never sent, permissions go to the tab's own context, and the
     fingerprint's `cmd_params` are not passed to Chrome (flags can run
     commands). Verified on real Chrome 155: a page reads the fingerprint's user
     agent, platform, language, time zone, cores, screen and WebGL renderer.
   - Found on the way: `evasions::cdp_overrides` granted `clipboardRead` and
     `clipboardWrite`, which Chrome rejects (the name is `clipboardReadWrite`),
     and the WebDriver path swallowed the error, so those permissions were never
     granted. Fixed and tested against Chrome.
   - `BrowserApi`, `ElementApi`, `AssertionApi` and `ScreenshotApi` are
     implemented for `sb_cdp::Page`, and `ElementApi::find_element` (the
     `thirtyfour::WebElement` leak) is removed from the trait; `BaseCase` keeps
     its own. `Page::js_errors` and `assert_no_js_errors` back
     `AssertionApi::assert_no_js_errors`.
   - Still to do: a `Fingerprint` per pool lease (`ContextOptions`), wiring
     `HumanizeConfig` through the Pure CDP `human` engine, and having the Tauri
     example call `Page::apply_fingerprint` instead of its own copy.
   - The `EvasionRegistry` is used through `bootstrap_script`; selecting
     individual providers per tab is not exposed.

### Modules with no consumer (decide: wire in, keep as API, or delete)

A scan found 23 source files whose public items are referenced from nowhere
else in the crate, its tests, examples or docs, all unchanged since the first
import on 2026-08-04. Four were the old plugin files, now rebuilt or removed;
19 remain. Nothing here has been deleted, because some are intentional library
surface. Decide each:

- `src/api/playwright.rs` (free functions for the `playwright` feature)
- `src/behave/common_steps.rs` (`CommonSteps`)
- `src/config/ad_block_list.rs` (re-exported, used by nothing else)
- `src/plugins/driver_manager.rs` (`DriverStack`)
- `src/utils/extensions/{ad_block,disable_csp,proxy_auth,recorder,sbase_ext}.rs`
- `src/utils/translate/master_dict.rs` (the `translate` CLI command is also
  still planned, so this one probably wants wiring in)

Now wired in and tested: `cli/scripts/{logo_helper,rich_helper,run}.rs` (the
`sbase test` command and the banner), `config/proxy_list.rs`,
`resources/assets.rs`, and `utilities/selenium_grid.rs` with the new
`grid_server.rs` and `sbase grid` (it starts a Selenium Server jar you supply;
nothing is downloaded). It has not been run against a real Selenium Server jar
or Java on this machine; the tests do not need one. The translate vocabulary work
was left out because it was unfinished when its agent stopped
(`utils/translate/language.rs` and a test for `master_dict::entries()` do not
exist yet).

The scan counts name mentions, so an item used only through a glob import or a
macro would show up here wrongly; check before deleting.

### Audit of the files untouched since 2026-08-04

Two read-only audits covered `src/api/**`, `src/stealth/*`,
`src/profile_payloads`, `tracing_util.rs` and `src/utilities`. **Every item
below was found by reading the code; none was run or confirmed against Chrome**,
so check each with a test before fixing it.

Fixed and tested (these were confirmed by reading and are covered by unit
tests): a payload's Chrome switches that run programs, load code or reroute
traffic are refused (`stealth::evasions::permitted_switch`, applied to the
WebDriver, Playwright and profile paths); `folder_id` can no longer leave
`./profile-data`; start pages are limited to five web pages; derived `Debug`
on `StealthOptions` and both `ProxyConfig` types no longer prints proxy
passwords; the CDP reactor no longer spins at 100% CPU after the socket closes
and keeps a request's own headers when it overrides one; mobile defaults no
longer override a caller's user agent and window size; `--no-sandbox` is
Linux-only and `--disable-gpu` headless-only (WebDriver path, not run against a
real driver). Also fixed: file names a caller supplies (`save_page_source`,
the cookie files, `save_data_as`, `append_data_to_file`, the downloads helpers)
can no longer climb out of their folder, through `artifacts::confined_path`,
with a test that fails if a bare `filename` is joined onto a directory again;
the shadow DOM script builders now close their function (checked by running
them in a real Chrome); the HTML report escapes what tests print and is written
atomically.

Fixed since, with tests: `master_qa.rs` no longer treats closed input as "yes"
(and escapes `|` and newlines in its Markdown table); the Python importer writes
`1.0` for a one-second sleep, rejects a sleep that would panic, ignores a
comment's apostrophes, and no longer mistakes a Selenium script that mentions
`seleniumbase.io` for a SeleniumBase one; the Selenium IDE reader handles
multi-line rows and entities; `tracing_util` builds its filter instead of
changing `RUST_LOG`; `stealth/cdp.rs` reports a failed click or domain enable
instead of returning `Ok` (WebDriver path, not run against a real driver); the
tour export writes valid CSS and escapes the tour's name; the chromedriver
patcher keeps its first `.orig` backup, replaces the binary with a rename
instead of writing into it, and keeps its permissions; the `cdc_` cleanup script
in `uc.rs` can run twice (checked in a real Chrome).

Also added, on request: if macOS will not start chromedriver, the launcher now
fixes the file and starts it again (`browser::driver_access`). A missing
execute permission is added; the `com.apple.quarantine` flag is removed; and a
driver killed as it starts, which is what Apple Silicon does to a binary whose
signature no longer matches (what patching does to it), is signed again ad hoc.
Patching a chromedriver on macOS signs it again straight away. Checked on this
machine by altering a copy of the installed chromedriver 152, which macOS
killed, and starting it through `launch_chromedriver()`. Not changed: the
Chrome browser patcher still removes the signature
(`strip_binary_signature`), which probably leaves an arm64 Chrome copy unable to
start; that needs the same ad hoc signing and a test against a real patched
Chrome.

Fixed since, with tests (the four `src/api` fixes were also run in a real
Chrome by the agent that made them): `dialog.rs` `prompt()` asks for text
(`37981ef`); `CdpDriver` sends its commands to the tab, not the browser socket
(`21fdbea`); the HTML inspector gives each issue a selector that matches its
element (`ab4eda5`); deferred assertions check the page they were made for
(`5c34b88`); chart series line up by label (`7069d42`); the presentation
escapes its title (`01e5787`); `get_origin` keeps the port (`741f7b3`);
`display_chart` and `begin_presentation` keep the file whose path they return
(`a505a0f`).

Open, most serious first:

- `patcher.rs` (the Chrome browser patcher): the cache key ignores the patch
  set; the patched Chrome copy is written non-atomically and probably cannot
  start on macOS or Linux because only the executable is copied (unverified).
- `uc.rs`: non-configurable `defineProperty` calls outside `try`.
- Providers (`stealth/providers/builtin.rs`): the font check mishandles
  shorthand such as `12px Arial`; the timezone provider patches only
  `Intl.DateTimeFormat`, so `getTimezoneOffset` disagrees; canvas and audio
  noise differ on every read despite the "deterministic" claim; page-visible
  globals `__sbNative`, `__sbVendorId`, `__sbBlockedTrackers`. No provider's
  JavaScript has ever been parsed or run in a test.
- `html_inspector.rs`: radio-group issues may still come out in `HashMap`
  order (not re-checked after the selector fix).
- `tour.rs` splices unescaped text into HTML and JavaScript (not re-checked;
  `chart.rs` and `presentation.rs` are fixed above).

The first-wave agents' finished work is all merged and gated: artifacts, shadow
DOM and the report; the CLI scaffolding; the `sbase test`, `grid-hub` and
`grid-node` commands with the config and resources work; the graceful shutdown
handler (SIGTERM and SIGINT close browsers); and the one retry/polling API in
`utilities::retry`. That last one replaced `common::{decorators,exceptions}`;
those two public modules are kept as `#[deprecated]` shims, so nothing breaks.
The worktrees under `.claude/worktrees/` can be removed. Merging the shutdown
handler also exposed a stack overflow in `sbase` (its large `run` future was
copied on the 8 MiB main stack in debug builds); the future is now boxed.

Performance harnesses are in `benches/` (`cargo bench --bench cpu`, `--bench
cdp_latency`); results and what they led to are in `docs/benchmarks.md`. Two
changes came from them and are covered by tests against a real Chrome: every
tab is told it has focus (a click on a tab that was not frontmost used to wait
for the 30 s command timeout), and a click sends its move, press and release
together (33.4 ms to 0.48 ms; eight tabs from 239 to 3,223 clicks a second).
There is still no comparison with Python SeleniumBase, because it is not
installed here. Not yet looked at: pipelining `drag`'s steps, the combined
visibility-and-centre evaluate for locators (worth about 0.1 ms), and the
WebDriver path, which was not measured at all.

## 6. Stealth scraping: what is left

The plan is `docs/design/stealth_parity_plan.md`; its status table says what is
merged. In order:

1. **Security review fixes** (work package 18): the two review passes over the
   MITM, credential handling and egress stickiness are done; the fixes they
   found are in progress.
2. **Final gates** (work package 19): the re-exports in `lib.rs`, `scrape/mod.rs`,
   `impersonate/mod.rs` and `wire/mod.rs` and the documentation of `HttpSession`,
   `MitmProxy` and `StealthSession` are written. Still to run: clippy over all
   targets for each feature, doctests, `cargo audit` and `cargo deny`.
3. **Run it headed and against more than local servers.** The Chrome-driving
   tests of the proxy, the HTTP hand-over and the session run headless on
   macOS with Chrome 155 against servers written for the tests.
4. **WebSocket through the proxy** is relayed for `wss://` with the measured
   WebSocket hello. Still open: a tab's first request through the proxy cannot
   open one (its `CONNECT` gets the `407` and Chrome does not retry it), `ws://`
   is refused, and compression (`permessage-deflate`) is passed through untested.
5. **The first request to an HTTP/1.1-only origin** carries the HTTP/2
   `priority` header in `HttpSession` and `MitmProxy`; later ones do not. The
   protocol is known only after the connection opens.
6. **Measure more systems.** Windows, Linux and Safari are refused
   (`NotMeasured`) until someone runs the capture tools on them. A Chrome update
   leaves the measured profile behind until `capture_tls`, `capture_hints` and
   `capture_h2` are run again.
7. **Known gaps.** `trust_anchors` needs a newer `btls` and `wreq` than the exact
   pins; the ML-DSA signature algorithms and the GREASE signature algorithm are
   not sent. A wire test fails by name when one of them closes.
8. **Not planned for now:** HTTP/3, CHIPS cookies, `Critical-CH` retries, and the
   `--remote-debugging-pipe` transport.

## Publishing

Work happens on a feature branch. To publish: run the quality gate on a clean
checkout of the exact commits, check that `origin/main` has not moved, fast-
forward `main`, push without force, and verify that local and remote hashes
match.

## Decisions to remember

- The `SeleniumBasePlugin` trait, `PluginManager` and `DbReportingPlugin` were
  removed: nothing ever called the manager, so none of their hooks ran, and the
  screenshot plugin wrote a text note into a `.png`. `TestPlugin` and `Plugins`
  replace them with hooks that fire and do real work. There is no per-command
  hook any more.
- The old `seleniumbase-mcp` tool names (`quit`, `screenshot`, the one-argument
  `assert_text`, `list_macros`) were replaced by the upstream names.
- `Cookies::save` writes JSON, not Python pickles.
- A failed browser action is returned to the model as an error result, never as
  a protocol error.
- Files written by MCP tools stay inside `SB_MCP_OUTPUT_DIR` (default
  `./mcp_output`); waits are capped at one hour.
