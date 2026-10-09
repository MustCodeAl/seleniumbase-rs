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
| Browser pool | `BrowserPool`, `Lease`, `BrowserContext`, `SessionStore`: bounded, fair, isolated, recycled, with in-memory session sharing. Verified on real Chrome. |
| WebRTC leak shield | `WebRtcPolicy`, `Page::webrtc_report`, `Page::shield_webrtc`, `LaunchOptionsBuilder::webrtc_policy`. Verified on real Chrome. |
| Behavioural stealth engine | `stealth::behavior` (pure, seedable) and `Page::human` (click, type, scroll at a human pace). Verified on real Chrome. |
| Pure CDP engine (`sb_cdp`) | `Browser`, `Page`, `Locator`, input, cookies/storage/window/emulation, retrying assertions, mock browser, CAPTCHA solving, same-origin frames (`page.locator("#frame").locator("button")`). Verified on real Chrome (`tests/sb_cdp_chrome.rs`). |
| MCP `cdp` server | 24 tools, mock-tested (`tests/mcp_cdp.rs`) and verified on real Chrome (`tests/mcp_cdp_chrome.rs`). |
| MCP `driver` / `sb` servers | 26 and 88 tools plus 8 stealth tools; catalogue and offline behaviour tested (`tests/mcp_webdriver.rs`). |
| `seleniumbase-mcp` binary | `--server cdp\|driver\|sb`, default `sb`. |
| `sbase` CLI | `encrypt` and `decrypt` (AES-256-GCM, passphrase from `SB_ENCRYPTION_KEY`). |
| BaseCase | Actions wait for their element; `nested_click`, `solve_captcha`, `fast_type`, `js_click_if_visible`, `get_gui_element_rect/center`, `jq_format`, `post_message`, `save_as_html_to_logs`, `save_teardown_screenshot`, `switch_to_default_driver`, `wait_for_angularjs`, `get_saved_cookies`. |

## 1. Finish the parity upgrade (next)

1. **Verify the WebDriver MCP servers on a real driver.**
   `tests/mcp_webdriver_chrome.rs` is written but unrun: the installed
   chromedriver (151) does not match Chrome (155). Run it once with a matching
   driver (`sbase get chromedriver` or a download) and fix whatever it finds.
2. **BaseCase actions wait (done; verify on a real driver).** `click`,
   `type_text`, `get_text`, `hover`, `submit`, ... now poll for their element up
   to `set_timeout` (10 s default) and fail with `WaitTimeout` naming the
   selector. The three `tests/browser_smoke.rs` tests that prove it
   (`an_action_waits_for_an_element_that_appears_late` and two more) are
   written but unrun, for the same driver-mismatch reason as item 1. Per-call
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
4. **Python names with no Rust counterpart, to classify** (implement, or record
   as composed / not applicable with a reason):
   `get_jqc_button_input`, `get_jqc_form_inputs`, `get_jqc_text_input`
   (JQuery-Confirm dialogs; Rust has `show_prompt`/`show_confirm`),
   `get_google_auth_password` (composed: `get_mfa_code`), `type` (canonical:
   `type_text`), `setUp`/`tearDown`/`setUpClass`/`tearDownClass`/`main`/`run`/
   `skip`/`has_exception` (unittest harness).
5. **`sbase` CLI gaps.** Still planned: `translate` (hook up `utils::translate`),
   `grid-hub` and `grid-node` (launch Selenium Grid). The rest are mapped onto
   existing commands or marked not applicable in `parity/api.toml`.
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

Current state against 4.55.2 (`just parity-docs` has the full table): BaseCase 442 of 453 implemented, 11 not applicable; Driver 75 of 79; all 138 MCP tools; CDP Mode 212 of 215 mapped or not applicable (3 messenger/toast methods planned); CLI 33 of 36; options by theme below. Work these `planned` entries down, biggest wins first:

1. CDP Mode: the three messenger methods (`post_message`, `activate_messenger`,
   `set_messenger_theme`): an in-page toast for `sb_cdp`.
2. CLI commands (item 5 above).
3. Options. 121 distinct options and keyword arguments are still `planned`;
   the rest map onto `BrowserConfig`/`RuntimeConfig`, pass through as browser
   flags, or are Python test-runner plumbing. By theme:
   - *Browser preferences*: `block_images`, `disable_cookies`, `disable_js`,
     `do_not_track`, `enable_sync`, `disable_csp`, `external_pdf`,
     `page_load_strategy`, `fullscreen`, `device_metrics`, `mobile_emulator`.
   - *Demo and slow modes*: `demo`, `demo_sleep`, `highlights`,
     `message_duration`, `slow`, `slowmo`, `interval`, `verify_delay`, `fast`.
   - *Reporting and artifacts*: `dashboard`, `dash_title`, `crumbs`, `metrics`,
     `archive_logs`, `archive_downloads`, `log_path`, `save_screenshot`,
     `no_screenshot`, `list_fail_page`, `with_s3_logging`, `with_db_reporting`
     (the Turso reporting work).
   - *Recorder*: `rec`, `record`, `rec_sleep`, `rec_print`, `rec_behave`,
     `rec_gherkin`, `recorder_ext`.
   - *More browsers and drivers*: `safari`, `opera`, `brave`, `comet`, `ie`,
     `cft` (Chrome for Testing), `driver_version`.
   - *Proxies*: `multi_proxy`, `proxy_bypass_list`, `proxy_driver`.
   - *Undetected mode and debugging*: `uc_subprocess`, `uc_cdp_events`,
     `remote_debug`, `log_cdp`, `xvfb`.
   - *Waits*: `skip_js_waits`, `wait_for_angularjs`, `timeout_multiplier`,
     `time_limit`, `check_js`.
   - *Other*: `cap_file`, `cap_string`, `firefox_arg`, `firefox_pref`, `crx`,
     `extension_zip`, `auto_ext`.

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

- Test-run reporting, the equivalent of Python's `--database_env` reporting and
  `sbase report`.
- The Tauri example's profile store, replacing the clear-text `profiles.json`.

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
   lease (item 4/5), and a per-lease proxy (item 3), both of which build on
   `BrowserContext`.
3. **CDP request interception (done).**
   - Done: `Page::intercept` with typed `Rule`s (block, fulfil, modify) and a
     request log, verified on real Chrome. The existing `CdpReactor` was checked
     and does work (it adds headers to every tab's requests); it is global and
     header-only, so `Page::intercept` is the per-page replacement.
   - Done: per-context proxy routing with per-session password handling
     (`ContextOptions`, `acquire_with`), verified on real Chrome against a
     password-protected proxy.
   - Done: WebRTC leak shielding. `WebRtcPolicy::{Allow, Block}`, a probe
     (`Page::webrtc_report`) that classifies the candidates a page can gather, and
     `Page::shield_webrtc` for a tab that is already open. Verified on real
     Chrome: the default leaks two `.local` host candidates and `Block` leaks
     none. Chrome's `default_public_interface_only` was tried and dropped: it
     does not remove the `.local` candidates.
4. **Hardware and profile randomisation**: WebGL, canvas, timezone and locale,
   as `Fingerprint::randomized(os, seed)` that always passes `validate()`.
5. **Integration**: use `Fingerprint` and `EvasionRegistry` from
   `sb_cdp::Browser`; make the `ElementApi` traits stop leaking
   `thirtyfour::WebElement`. Everything must still build with
   `--no-default-features` and headless.

## Publishing

Work happens on a feature branch. To publish: run the quality gate on a clean
checkout of the exact commits, check that `origin/main` has not moved, fast-
forward `main`, push without force, and verify that local and remote hashes
match.

## Decisions to remember

- The old `seleniumbase-mcp` tool names (`quit`, `screenshot`, the one-argument
  `assert_text`, `list_macros`) were replaced by the upstream names.
- `Cookies::save` writes JSON, not Python pickles.
- A failed browser action is returned to the model as an error result, never as
  a protocol error.
- Files written by MCP tools stay inside `SB_MCP_OUTPUT_DIR` (default
  `./mcp_output`); waits are capped at one hour.
