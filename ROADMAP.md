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
| Pure CDP engine (`sb_cdp`) | `Browser`, `Page`, `Locator`, input, cookies/storage/window/emulation, retrying assertions, mock browser, CAPTCHA solving. Verified on real Chrome (`tests/sb_cdp_chrome.rs`). |
| MCP `cdp` server | 24 tools, mock-tested (`tests/mcp_cdp.rs`) and verified on real Chrome (`tests/mcp_cdp_chrome.rs`). |
| MCP `driver` / `sb` servers | 26 and 88 tools plus 8 stealth tools; catalogue and offline behaviour tested (`tests/mcp_webdriver.rs`). |
| `seleniumbase-mcp` binary | `--server cdp\|driver\|sb`, default `sb`. |
| BaseCase | `nested_click`, `solve_captcha`, `fast_type`, `js_click_if_visible`, `get_gui_element_rect/center`, `jq_format`, `post_message`, `save_as_html_to_logs`, `save_teardown_screenshot`, `switch_to_default_driver`, `wait_for_angularjs`, `get_saved_cookies`. |

## 1. Finish the parity upgrade (next)

1. **Verify the WebDriver MCP servers on a real driver.**
   `tests/mcp_webdriver_chrome.rs` is written but unrun: the installed
   chromedriver (151) does not match Chrome (155). Run it once with a matching
   driver (`sbase get chromedriver` or a download) and fix whatever it finds.
2. **BaseCase actions do not wait.** `click`, `type_text`, `submit`, ... act
   immediately, while Python's accept a `timeout` and wait. The MCP layer works
   around it by waiting first (`mcp::webdriver::ready`). Fix at the source:
   make BaseCase actions wait up to the configured timeout, and add per-call
   timeout variants. Then drop the workaround.
3. **BaseCase semantic differences to resolve or document**
   - `click_nth_visible_element` is 0-based; Python's is 1-based.
   - `download_file` opens the URL in a browser tab; Python downloads over HTTP
     into a folder.
   - `scroll_up`/`scroll_down` take no amount; Python's take a viewport
     percentage.
   - `open_new_tab` always switches to the new tab; Python has `switch_to`.
   - Cookie files are JSON; Python's are pickles.
4. **Python names with no Rust counterpart, to classify** (implement, or record
   as composed / not applicable with a reason):
   `get_jqc_button_input`, `get_jqc_form_inputs`, `get_jqc_text_input`
   (JQuery-Confirm dialogs; Rust has `show_prompt`/`show_confirm`),
   `get_google_auth_password` (composed: `get_mfa_code`), `type` (canonical:
   `type_text`), `setUp`/`tearDown`/`setUpClass`/`tearDownClass`/`main`/`run`/
   `skip`/`has_exception` (unittest harness).
5. **`sbase` CLI gaps.** Missing: `methods`, `options`, `behave-options`,
   `encrypt`/`decrypt`/`obfuscate`/`unobfuscate` (reversible obfuscation, not
   security), `get`, `translate`, `convert`, `codegen`, `recorder`, `gui`,
   `proxy`, `download server`, `grid-hub`, `grid-node`, `extract-objects`,
   `inject-objects`, `revert-objects`. Implement the deterministic ones first.
6. **pytest-style options and settings.** Compare the Python `--option` list
   and `SB()`/`Driver()` kwargs with `BrowserConfig`/`RuntimeConfig`; add the
   missing ones.
7. **Pure CDP: iframes.** `src/sb_cdp/helper.js` resolves selectors in the main
   document only. Resolve through same-origin `iframe.contentDocument`, and add
   the frame offsets to `center()`. Test with `srcdoc` iframes on real Chrome.
8. **Pure CDP: check the 215 Python `sb.cdp.*` methods** each have a Rust
   composition (`page.locator(sel)...`). List the ones that do not and add them.

## 2. Make future upstream updates cheap (done; keep the manifest current)

Built: `parity/upstream.toml`, `tools/parity/extract_api.py`,
`tools/parity/seed_manifest.py`, `tools/parity/render_docs.py`,
`parity/api.toml`, `tests/parity.rs`, `docs/UPSTREAM_SYNC.md`, the generated
`docs/parity.md`, `just parity-*` recipes and a weekly `upstream-watch`
workflow (untested: Actions are billing-locked).

First classification of 4.55.2: BaseCase 440 of 453 matched by name and 11
not applicable; Driver 75 of 79; all 138 MCP tools; CDP Mode 56 composed and
159 still `planned`; CLI 17 of 36; pytest options 18 of 263. Work these
`planned` entries down, biggest wins first:

1. CDP Mode: map the 159 unmapped `sb.cdp.*` methods to a Rust composition, or
   port them (this also covers item 8 above).
2. CLI commands (item 5 above).
3. Options: most pytest options are Python-test-runner settings; mark the
   irrelevant ones `not-applicable` with a reason and port the rest into
   `BrowserConfig`/`RuntimeConfig`.

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

1. **Behavioural stealth engine**: human-like curved mouse paths, micro-jitter,
   Gaussian typing rhythm. Extend `stealth::humanize`; use one shared `Point`
   type with `sb_cdp`.
2. **Async browser pool**: a thread-safe pool of browsers with in-memory
   session and cookie sync between workers.
3. **CDP request interception**: per-tab proxy routing via browser contexts,
   typed intercept rules, WebRTC/mDNS leak shielding with a self-test. Verify
   first whether `CdpReactor` intercepts page traffic at all: it enables
   `Fetch` on the browser-level socket.
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
