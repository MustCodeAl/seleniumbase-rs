---
title: "Design: a lean core, closer to Python SeleniumBase"
sidebarTitle: "Core split"
---

Status: decided (2026-11-06). The owner left the decision to the lead ("dont need my approval for the split do whats best"), who took every recommendation below, open questions 1 to 15 included, in the lane order of the note: the split lands in 0.9.0, and Mimic's first (Chromium) lane is built in core's `src/evasion` under the note's two rules and moves to stealth with the rest of the layer. Written as a proposal, no code. It answers the owner's question of 2026-11-06: "wouldnt
it make sense to move evasion to stealth repo and other stuff to plugins repos to keep the base
repo more lean and closer in parity with python?" The lead answered with a rule, and the owner
accepted that answer as the brief for this note:

- **core** keeps what Python SeleniumBase has in its own package: UC mode and driver patching,
  CDP mode and the pure CDP engine, and the test plugins Python ships (S3 logging, the recorder,
  tours, MasterQA, charts, presentations);
- **stealth** (`seleniumbase-rs-stealth`) takes the fingerprint layer Python does not have:
  `Fingerprint` and `Spoof`, the randomizer, the evasion providers, fingerprint capture and
  compare, and the coming `Mimic` state (`docs/design/fingerprint_mimic.md` on
  `lane/design-presets`);
- **plugins** (`seleniumbase-rs-plugins`) takes the extras beyond Python, such as the GCP and
  Azure uploaders.

This note checks that rule against the code and against Python, and corrects it in eight places
([The short answer](#the-short-answer)). Code facts were read at `lane/design-split` (`4c81bf21`)
and are cited as `file:line`. Python facts were read in the SeleniumBase checkout and are cited
by path under `seleniumbase/` (this crate tracks 4.55.2, `src/lib.rs:44`; the checkout's index
was last synced 14 days ago). Plugin API facts were read on the plugins repository's `main`
(`c12d2ee`); core pins `0666904` (`Cargo.toml:155-156`), two commits older, which by their
subjects changed only the lock file and `rust-version`. Line counts are tokei's code lines:
comments and doc comments are left out, and the unit tests inside a file are counted. Anything
marked _to verify_ was not checked.

## The short answer

The rule holds, with eight corrections:

1. **Python's CDP mode sets a time zone, a locale, a user agent, a platform and a
   geolocation** (`undetected/cdp_driver/cdp_util.py:300-309`, `cdp_driver/browser.py:418-471`
   and `:516-517`). Here they reach the pure CDP engine only through a `Fingerprint`:
   `LaunchOptionsBuilder` has `user_agent` and `lang` (`src/sb_cdp/launch/builder.rs:197`,
   `:204`) but no time zone, geolocation or platform. They are core, and need plain launch
   settings before the fingerprint can leave.
2. **Python's UC mode patches chromedriver when it starts** (`undetected/__init__.py:133`,
   `self.patcher.auto()`). Ours does not: `docs/tutorials/uc_mode.md:27` says "It does not patch
   chromedriver", and nothing in `src/browser/` calls `ChromedriverPatcher`. That is a parity gap
   in core, not stealth work.
3. **Python's UC page script only deletes the `cdc_` properties** (`undetected/__init__.py:381-411`).
   Ours (`src/evasion/uc.rs:12-218`) also fakes `navigator.languages` (always `en-US`),
   `hardwareConcurrency` and `deviceMemory` (always 8), the plugin list, `window.chrome`, an
   "Intel Iris" WebGL renderer, permissions, media devices, iframes and `console.debug`. The
   extras are stealth; core's UC script becomes Python's.
4. **`evasion` holds core code that is not about evasion**: the WebDriver engine's DevTools
   client (`src/evasion/cdp.rs`), the Chromium capability builder every WebDriver session uses
   (`StealthOptions`, `src/evasion/options.rs:27`, called from
   `src/browser/session/capabilities.rs:216`), `find_system_chrome`
   (`src/evasion/patcher.rs:403`), and the recorder's page scripts
   (`src/evasion/js/active_css_js.rs` and `recorder_js.rs`, which are Python's
   `js_code/active_css_js.py` and `recorder_js.py`). They stay in core, under new paths.
5. **The WebRTC policy is Python's too.** CDP mode writes
   `webrtc.ip_handling_policy = disable_non_proxied_udp` into every profile
   (`cdp_driver/config.py:427-431`), and the WebDriver launcher does so with a proxy
   (`core/browser_launcher.py:2437-2439`). `WebRtcPolicy` stays in core; the plugin API's
   `LaunchSettings` already has one.
6. **Human input stays.** `evasion::behavior` and `evasion::humanize` (1,214 lines) drive the
   captcha solvers (`src/sb_cdp/captcha/engine.rs:30-31`, `src/sb_cdp/gui_captcha.rs:33`),
   which mirror Python's `solve_captcha` and `gui_*` methods (`core/sb_cdp.py:2678`, `:2911`).
   Their curved-path model goes beyond Python (the package has no "bezier"), but moving them
   would break a Python feature, so they leave `evasion` and stay in core.
7. **Python ships an MCP server**, as a separate package in the same repository
   (`mcp_servers/pyproject.toml:2`, `name = "seleniumbase-mcp"`). The MCP servers stay in this
   repository; only their fingerprint tools leave.
8. **The fingerprint layer is not the largest thing a default build compiles that Python
   lacks.** The Python-to-Rust importer in `src/utilities/` is about 17,200 lines, more than the
   14,500 the fingerprint move takes out of a default build ([Leanness](#leanness)). It is
   outside the owner's question (open question 13).

The GCP and Azure uploaders move as the rule says: Python has neither (`seleniumbase/plugins/`
has `s3_logging_plugin.py` and no other uploader), and nothing in `src/` calls them; only
`docs/tutorials/cloud_integrations.md:98` and `:131` teach them. S3 stays: the test run uses it
(`src/testing/report.rs:137`), as Python's `--with-s3-logging` does.

## What exists

### What a default build compiles

The library's default features are empty (`Cargo.toml:169-172`), so a build of the library
alone (`-p seleniumbase-rs`) compiles every module no feature gates. These are gated: `mcp`
(`mcp-server`), `ocr`, `plugin_host` (`plugins`), `storage` (`turso`), `testing` (`testing`,
`bdd`) and `visual` (`src/lib.rs:77-193`), and in `plugins/` the S3, Azure and GCP uploaders
(`src/plugins/mod.rs:11-21`). Of the 183,195 Rust lines of `src/`, 26,249 are in those gated
modules, so a default build compiles at most 156,946 (the `gui` and `tui` parts inside `api/`
and `cli/` are not subtracted). `evasion` and `profile_payloads` are always compiled
(`src/lib.rs:147`, `:152`). The command line turns `pdf`, `mcp-server`, `mcp-client` and
`plugins-builtin` on (`crates/cli/Cargo.toml:79`).

### Inventory

Lines are Rust code lines; a feature in brackets gates the module. Heavy dependencies are the
ones `Cargo.toml` names for the module, or that a `use` line in the module shows.

| Module | Lines | Heavy dependencies | Python counterpart (under `seleniumbase/`) | Verdict |
|---|---|---|---|---|
| `api/` | 37,908 (+863 JS) | thirtyfour, scraper (`api/html.rs:28`); pdf-extract [`pdf`]; enigo, rfd [`gui`] | `fixtures/base_case.py`, `masterqa/master_qa.py`, `core/tour_helper.py`, `core/recorder_helper.py`, `core/jqc_helper.py`, `console_scripts/sb_mkchart.py`, `sb_mkpres.py`, `core/sb_cdp.py` | keep, less `base_case_impls/base_case_impl_fingerprint.rs` (98) |
| `browser/` | 17,742 | thirtyfour, reqwest, indicatif | `core/browser_launcher.py`, `detect_b_ver.py`, `download_helper.py`, `capabilities_parser.py`, `proxy_helper.py`, `drivers/`, `console_scripts/sb_install.py` | keep, less `session/fingerprint.rs` (97) and native spoofing (`session/launch.rs:307-354`) |
| `cli/` | 20,578 | ratatui, crossterm [`tui`] | `console_scripts/` | keep, less `fingerprint_cmd.rs` (471) |
| `common/` | 3,656 | ring | `common/` | keep |
| `config/` | 7,471 | figment, toml | `config/`, `core/settings_parser.py` | keep, less the `[fingerprint]` table and `SB_FINGERPRINT` |
| `core/` | 5,086 | none | `core/log_helper.py`, `report_helper.py`, `testcase_manager.py`; the dashboard of `plugins/pytest_plugin.py` | keep |
| `cookie.rs`, `error.rs`, `artifacts.rs` | 381, 1,152, 513 | thiserror | `base_case.py` cookies, `fixtures/errors.py`, `common/exceptions.py`, the `latest_logs` handling of `core/log_helper.py` | keep |
| `evasion/` | 14,859 (+448 JS) | thirtyfour (`cdp.rs`, `options.rs`), tokio-tungstenite (`reactor.rs`), regex (`patcher.rs`), rand_distr (`humanize.rs`) | `undetected/` (`patcher.py`, `options.py`, `reactor.py`, `dprocess.py`, `cdp.py`) for about 3,600 lines of it; nothing for the rest | split, see below: about 3,600 stay, 11,204 go to stealth |
| `js_code/` | 112 | none | `js_code/` | keep; gains `active_css_js`, `recorder_js` and `best_selector.js` from `evasion/js/` |
| `macros.rs` | 387 | none | none (Rust only) | keep, less `fingerprint!` (`:257`) |
| `mcp/` [`mcp-server`] | 19,635 (+1,458 JSON/JS) | rmcp, axum, chrono | `mcp_servers/` (the separate package `seleniumbase-mcp`) | keep in this repository; the fingerprint tools go to stealth |
| `ocr.rs` [`ocr`] | 196 | ocrs, rten, image | none | move to plugins |
| `plugin_host/` [`plugins`] | 943 | seleniumbase-rs-plugin | none: the host of ADR 004 | keep: the hooks live where the engines are |
| `plugins/` | 1,154 | object_store [`s3`, `azure`], reqwest [`gcp`] | `plugins/base_plugin.py`, `driver_manager.py`, `page_source.py`, `screen_shots.py`, `s3_logging_plugin.py`, `core/s3_manager.py` | keep, less `azure_logging_plugin.rs` (79) and `gcp_logging_plugin.rs` (85), which move to plugins |
| `profile_payloads/` | 2,156 | none | none | move to stealth (it turns external profile JSON into a `Fingerprint`) |
| `resources/` | 181 | none | `resources/` | keep |
| `sb_cdp/` | 19,664 (+725 JS) | tokio-tungstenite | `undetected/cdp_driver/`, `core/sb_cdp.py` | keep, less `identity.rs` (492) and `capture.rs` (17) |
| `storage/` [`turso`] | 1,347 | turso | `plugins/db_reporting_plugin.py`, `core/mysql.py` (for `ResultStore`) | keep `ResultStore`; `ProfileVault` (643) has no counterpart and no caller here (open question 12) |
| `testing/` [`testing`, `bdd`] | 3,299 | rstest, rstest-bdd | `plugins/pytest_plugin.py` (the `sb` fixture), `plugins/sb_manager.py`, `behave/` | keep |
| `tracing_util` | 705 | tracing-subscriber | part of `core/log_helper.py` | keep |
| `utilities/` | 19,246 | none | `utilities/selenium_grid/`, `selenium_ide/` (our grid and IDE files, 1,541 lines); none for the importer (17,174 lines) or `retry.rs` (525) | keep; the importer is open question 13 |
| `utils/` | 4,108 | none | `fixtures/css_to_xpath.py`, `xpath_to_css.py`, `page_utils.py`, `shared_utils.py`, `translate/`, `extensions/` | keep |
| `visual.rs` [`visual`] | 538 | image-compare, image | none: Python's `check_window` (`base_case.py:11664`) compares the layout, not pixels | open question 11 |
| `drivers.rs`, `json.rs`, `lib.rs` | 12, 4, 162 | none | none (re-exports) | keep, less the fingerprint re-exports (`src/lib.rs:221-235`) |
| `crates/cli` | 18,172 | usage-rs, miette, toml_edit, rpassword | `console_scripts/run.py` (the `sbase` command) | keep, less `src/fingerprint.rs` (215), `--fingerprint` and `patch-chrome` |
| `crates/own-imports` | 57 | none | none (tests only) | keep |
| `crates/playwright` | 419 | rustwright | Stealthy Playwright Mode (`examples/cdp_mode/playwright/ReadMe.md:5`) | keep |

No module is marked "remove": each has either a Python counterpart, a caller, or a home
elsewhere.

### `src/evasion`, file by file

| File | Lines | What it is | Python | Verdict |
|---|---|---|---|---|
| `patcher.rs` | 684 | `ChromedriverPatcher` (`:127`), `patch_chromedriver` (`:679`), `find_system_chrome` (`:403`); `ChromeBinaryPatcher` (`:257`), `EnginePatch::chrome_binary` (`:92`), `engine_spoofing_args` (`:686-710`) | `undetected/patcher.py` (`gen_random_cdc` `:194`, `patch_exe` `:212`) for the driver patcher; nothing for the browser binary or the extra flags | split: the driver patcher stays; `find_system_chrome` moves to `browser::launcher`; `ChromeBinaryPatcher`, `EnginePatch::chrome_binary` and `engine_spoofing_args` go to stealth |
| `options.rs` | 903 | `StealthOptions`: the Chromium capabilities of every WebDriver session; `UC_ARGS` and `apply_undetected_args` (`:424-454`); `without_credentials` (`:92`); `DEFAULT_REMOTE_DEBUG_PORT` (`:23`) | `undetected/options.py` and the Chrome options of `core/browser_launcher.py` | stays, as `browser::options::chromium` with a name that is not "stealth"; its fingerprint input (`:12`, tests `:552-602`) goes |
| `uc.rs` | 264 | `UC_STEALTH_SCRIPT` (`:12-218`), `apply_uc_stealth` (`:221`); the user-agent, time zone and geolocation overrides (`:227-265`) | the cdc removal of `undetected/__init__.py:381-411`; the overrides are CDP mode's (`cdp_driver/browser.py:455-517`) | split: cdc removal stays; the overrides move to `browser::session::emulation`; the rest of the script goes to stealth |
| `cdp.rs` | 209 | `CdpClient`, the WebDriver engine's DevTools client; the domain hygiene (`:17-21`) | `undetected/cdp.py`; Selenium's `execute_cdp_cmd` | stays, as `browser::devtools` |
| `dprocess.rs` | 62 | starts chromedriver or Chrome detached (`:10-15`) | `undetected/dprocess.py` | stays |
| `reactor.rs` | 270 | `CdpReactor`: Fetch interception over a socket (`:13-16`) | `undetected/reactor.py` | stays (UC mode's CDP events) |
| `behavior.rs`, `humanize.rs` | 975, 239 | the human input model (`behavior.rs:1-20`), used by `sb_cdp::human`, the captcha solvers and `BaseCase::human_type` | `press_keys` and the `gui_*` moves of `core/sb_cdp.py`, without a curve model | stay in core, as `sb_cdp::human` (open question 3) |
| `js/` | 18 (+448 JS) | `active_css_js`, `recorder_js`, `best_selector.js` (`active_css_js.rs:27`); `capture_js` and `capture.js` | `js_code/active_css_js.py`, `recorder_js.py` | the recorder's scripts move to `js_code/`; `capture_js` and `capture.js` go to stealth |
| `fingerprint.rs`, `fingerprint/` | 2,557 + 1,257 | `Fingerprint`, its builder and presets; `compare`, `file`, `schema` | none | stealth |
| `spoof.rs`, `settings.rs`, `policy.rs` | 277, 260, 301 | `Spoof`, the value types, `StealthPolicy` | none | stealth |
| `randomize.rs` | 1,797 | coherent random identities, `Locale`, `chromium_hints`, `PRESET_CHROMIUM_MAJOR` | none | stealth; core keeps its own copy of the major its mobile user agent claims (`src/browser/options/device.rs:13`) |
| `providers/` | 616 + 2,140 | `EvasionProvider`, the registries, the built-in providers | none | stealth |
| `evasions.rs` | 963 | `session_script`, `bootstrap_script`, `safe_cdp_overrides` | none | stealth |
| `capture.rs` | 1,036 | `CapturedFingerprint` and the probe | none | stealth |
| `mod.rs` | 31 | re-exports | none | goes with the module |

Two items of `evasion::fingerprint` are core's: `WebRtcPolicy` (used by `src/sb_cdp/webrtc.rs:40`,
`src/plugin_host/convert.rs:7` and `LaunchOptionsBuilder::webrtc_policy`, `builder.rs:84`) moves
to `browser::options`; `default_tracker_hosts` is used by a test of the ad-block list only
(`src/config/ad_block_list.rs:445`) and goes with the fingerprint.

### How the fingerprint is wired in today

- **WebDriver engine.** `BrowserConfig::fingerprint` (`src/browser/config.rs:225-227`),
  `with_fingerprint` (`:527`) and `native_spoofing_enabled` (`:1226-1229`). The UC and CDP modes
  apply it (`src/browser/session/cdp.rs:264-313`, `session/fingerprint.rs:18-34`); native
  spoofing patches a copy of Chrome before launch (`session/launch.rs:307-354`); the
  fingerprint's trackers join the ad-block list in one `Network.setBlockedURLs`
  (`session/emulation.rs:135-142`, `:170-178`).
- **Pure CDP engine.** `LaunchOptionsBuilder::fingerprint` (`src/sb_cdp/launch/builder.rs:107`),
  `LaunchOptions::fingerprint` and `with_fingerprint` (`src/sb_cdp/launch.rs:271`, `:305`).
  `Layer::of_fingerprint` (`src/sb_cdp/launch/layers.rs:168-190`) turns a fingerprint into the
  user agent, language, window size, WebRTC policy and proxy of a launch, as the lowest layer:
  what the program names wins. `Page::apply_fingerprint` and `StealthPlan`
  (`src/sb_cdp/identity.rs:110`, `:248`), `Page::capture_fingerprint` (`src/sb_cdp/capture.rs`).
- **`BaseCase`.** `capture_fingerprint` and `compare_fingerprint`
  (`src/api/base_case_impls/base_case_impl_fingerprint.rs:42`, `:66`); `human_type` takes its
  pace from the fingerprint's `HumanizeConfig` (`src/api/base_case.rs:2496-2513`).
- **Settings and the command line.** `SB_FINGERPRINT` and the `[fingerprint]` table
  (`src/config/settings.rs:20`, `:61-126`, `:341`), the run option
  (`src/config/test_options.rs:712`), `sbrs --fingerprint FILE`
  (`crates/cli/src/sbrs.rs:647-655`), `sbrs fingerprint` (`:2037-2044`,
  `crates/cli/src/fingerprint.rs`, `src/cli/fingerprint_cmd.rs`), `sbrs patch-chrome` (`:2737`,
  `:4078`).
- **MCP.** `capture_fingerprint`, `compare_fingerprint`, `explain_fingerprint` and
  `patch_fingerprint` (`src/mcp/fingerprint.rs:491`, `:525`, `:563`, `:856`);
  `build_fingerprint` and `validate_fingerprint` (`src/mcp/stealth.rs:65`, `:87`);
  `check_fingerprint` of the dev server (`src/mcp/dev/diagnose.rs:93`); the prompts that name
  them (`src/mcp/prompts.rs:273-286`). `patch_chromedriver` (`src/mcp/stealth.rs:36`) is UC
  mode's and stays.
- **The crate root.** `src/lib.rs:221-235` re-exports the fingerprint types, the providers and
  the patchers; the crate docs name the presets (`src/lib.rs:27-30`); `fingerprint!`
  (`src/macros.rs:257`).

`StealthPolicy` has no field of its own anywhere (a search of `src/browser`, `src/config`,
`src/sb_cdp`, `src/api` and `crates/cli/src` finds none): it is the fingerprint's `policy()`, so
it goes where `Fingerprint` goes.

### What Python's UC and CDP modes do

UC mode (`seleniumbase/undetected/`):

- patches chromedriver at start (`__init__.py:133`; `patcher.py`, `Patcher` `:17-317`);
- deletes the `cdc_` properties before each `get` and each new window
  (`__init__.py:381-465`);
- launches Chrome detached (`dprocess.py`) and listens to CDP events (`reactor.py`);
- runs only on Chrome, Opera, Brave and Comet: another browser gets a message and runs without
  UC (`plugins/sb_manager.py:804-814`); "UC Mode doesn't support Edge now"
  (`core/browser_launcher.py:4614`). Edge still gets `--disable-blink-features=AutomationControlled`
  and the `enable-automation` switch removed (`core/browser_launcher.py:4577-4582`).

CDP mode (`undetected/cdp_driver/`, `core/sb_cdp.py`):

- takes `lang`, `agent`, `tzone`, `geoloc`, `mobile`, `ad_block`, `proxy`, `disable_csp`,
  `extension_dir` and more at start (`cdp_util.py:288-316`), and `platform` (`browser.py:434-437`);
- sends the time zone, the locale, the user agent with its platform (removing "Headless" from a
  headless user agent), a fixed ad-block list, the geolocation, the CSP bypass and a phone's
  metrics (`browser.py:450-525`);
- writes the WebRTC preferences into every profile (`config.py:427-431`);
- solves captchas and moves the mouse through pyautogui (`core/sb_cdp.py:2678`, `:2911`).

Nothing in the package randomizes an identity, injects a fingerprint script, noises a canvas or
compares a fingerprint.

### The plugin API today

ADR 004 puts the API in its own crate, `seleniumbase-rs-plugin`, and plugins never name a type of
this crate (`docs/adr/004-plugin-framework.md:50-59`). What a stealth plugin could use today:

- `before_launch(&mut LaunchSettings)`: `args`, `prefs`, `extensions`, `proxy`, `headless`,
  `host_resolver_rules`, `webrtc_policy`, `disable_features`, `enable_features`, `disable_quic`,
  and a read-only profile (`crates/seleniumbase-rs-plugin/src/lib.rs:355-377`);
- `on_page_created(&dyn PageHandle)` with `send`, `add_init_script`, `evaluate` and
  `block_urls` (`lib.rs:658-726`), and `on_browser`, `on_context_created`,
  `on_protocol_event`, `before_navigate` and `before_close` (`lib.rs:730-889`);
- `BrowserHandle::send` for browser-wide commands such as `Browser.grantPermissions`
  (`lib.rs:591-654`);
- capabilities that gate them: `LaunchArgs`, `LaunchProxy`, `PageScript`, `Cdp(domain)` and
  others (`capability.rs:72-137`); options from the settings through `Configurable`
  (`config.rs:61`);
- `Requirements`: `RUN_LAST`, `HEADFUL`, `LAUNCH_ONLY`, `CHROMIUM_ONLY` (`lib.rs:423-429`).

## The proposal

### 1. Three homes

| Home | Takes |
|---|---|
| core | UC mode as Python has it (driver patcher, launch arguments, cdc removal, detached launch, reactor), CDP mode and the pure CDP engine with Python's launch settings (time zone, geolocation, platform added), the WebRTC policy, human input, the plugin host, the test plugins Python ships, the MCP servers less their fingerprint tools |
| stealth | the fingerprint layer (11,204 lines of `evasion`), `profile_payloads`, `StealthPlan` and `apply_fingerprint`, capture and compare, `ChromeBinaryPatcher` and `engine_spoofing_args`, UC's extra script, Mimic, `sbrs fingerprint` and the fingerprint MCP tools |
| plugins | the GCP and Azure uploaders, OCR, and visual comparison if open question 11 says so |

### 2. What core keeps of `evasion`

`src/evasion` leaves core as a name. Its core part gets homes that say what it is:

- `src/undetected/` (Python's package name): `patcher.rs` (the driver patcher only), `uc.rs`
  (Python's cdc script), `dprocess.rs`, `reactor.rs`;
- `browser::devtools`: `CdpClient`;
- `browser::options::chromium`: `StealthOptions` renamed, with `UC_ARGS`,
  `without_credentials` and `DEFAULT_REMOTE_DEBUG_PORT`;
- `browser::launcher`: `find_system_chrome`;
- `browser::session::emulation`: the user-agent, time zone and geolocation overrides;
- `browser::options`: `WebRtcPolicy`;
- `sb_cdp::human`: `behavior` and `humanize`;
- `js_code/`: `active_css_js`, `recorder_js`, `best_selector.js`.

Three Python parity changes go with it, each its own lane:

- **UC mode patches the driver.** At launch, a UC session runs a patched copy of chromedriver,
  made and cached as `sbrs install uc_driver` makes it (`src/cli/scripts/sb_install.rs:357-364`),
  as Python's `Patcher.auto()` does.
- **UC's page script is Python's**: the cdc removal, and the `HeadlessChrome` user-agent fix
  both Python modes do (`cdp_driver/browser.py:459-466`; ours `src/browser/session/cdp.rs:270-281`).
- **Python's CDP launch settings**: `timezone`, `geolocation` and `platform` on
  `LaunchOptionsBuilder` and `BrowserConfig`, sent in Python's order
  (`cdp_driver/browser.py:450-525`), with no `Fingerprint` involved.

### 3. Stealth: one crate and one plugin

A new crate in the stealth repository (name open, see open question 1) holds what leaves core,
and gives it back through ADR 004:

- a `Stealth` plugin. `before_launch` writes the fingerprint's user agent, language, window size,
  WebRTC policy, proxy and QUIC setting, and the patched browser of native spoofing.
  `on_page_created` sends what `StealthPlan` holds today: the init script and the protocol
  commands. `on_context_created` does the same for a browser context, and `block_urls` blocks
  the trackers;
- extension traits on `sb_cdp::Page` and `BaseCase` for `apply_fingerprint`,
  `capture_fingerprint` and `compare_fingerprint`, which stealth can write because it depends on
  core;
- its own command line and MCP tools for what `sbrs fingerprint` and the fingerprint tools do
  today (open questions 8 and 9).

The plugin must implement the trait core calls. Two revisions of `seleniumbase-rs-plugin` are two
traits (`Cargo.toml:148-153`), so stealth imports the API through core's re-export,
`seleniumbase_rs::plugin_api` (feature `plugins`, `src/lib.rs:110-121`), and never names the
API crate itself.

What a program writes after the split (illustrative: the crate name and `Stealth::new` are
proposals; `PluginSet::new().with` and `.plugins` exist, `docs/adr/004-plugin-framework.md:115-118`,
`builder.rs:303`):

```rust
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_fingerprint::{Fingerprint, OsType, Stealth};

let fingerprint = Fingerprint::randomized(OsType::Windows, 42);
let options = LaunchOptions::builder()
    .plugins(PluginSet::new().with(Stealth::new(fingerprint)))
    .build()?;
let browser = Browser::launch(options).await?;
```

The stealth facade re-exports the whole of core today (`tests/contract_stealth.rs:182-185`); it
adds the new crate, so a user of the facade keeps most root paths.

### 4. The hooks the plugin API lacks

1. **A typed launch identity.** `LaunchSettings` has no user agent, language, window size or
   browser executable. A plugin can push raw `args`, but the host writes its own `--user-agent`,
   `--lang` and `--window-size` from its settings, so a switch can come twice. The precedence
   `Layer::of_fingerprint` keeps today (the program's settings win, `layers.rs:168-190`) moves
   into the host as the merge of these fields.
2. **A first place for page scripts.** The fingerprint script must run before every other
   plugin's packages and scripts. `Requirements` has `RUN_LAST` (`lib.rs:423`) and nothing that
   runs first: add `RUN_FIRST`, or an order for init scripts.
3. **`block_urls` on the WebDriver engine.** Only the pure CDP host implements it
   (`src/sb_cdp/plugin_hooks.rs:712`, `:813`). The WebDriver host merges a raw
   `Network.setBlockedURLs` (`src/browser/plugin_hooks.rs:731-736`) but leaves `block_urls` at
   its default, `NotSupported`.
4. **A preload path for Firefox.** On Firefox and Safari, `DriverPage::add_init_script` runs the
   script after each navigation (`src/browser/plugin_hooks.rs:743-772`), after the page's own
   scripts, which is too late for a fingerprint. WebDriver BiDi's `script.addPreloadScript` is
   the standard path (the Mimic note cites thirtyfour 0.38.1's `bidi` feature, which is off here,
   `Cargo.toml:82`). It belongs in the core host, because every plugin's scripts and packages
   need it, not only stealth's.
5. **Workers.** No path reaches dedicated, shared or service workers today: the only
   `Target.setAutoAttach` of the engine is for proxy logins (`src/sb_cdp/browser.rs:1448`). A hook
   that hands a plugin each attached worker is needed before worker coverage. The move loses
   nothing here.
6. **A plugin by name in `sbrs`.** The command line's registry knows only the built-in plugins
   (`plugin_host`), so `--plugin stealth` cannot work in core's binary unless core depends on
   stealth (open question 8).

Already enough: protocol commands (`PageHandle::send`, `BrowserHandle::send`,
`Capability::Cdp`), the proxy, the WebRTC policy and QUIC in `LaunchSettings`, contexts, and
options from the settings file.

### 5. The plugins repository

- **GCP and Azure** (85 and 79 lines; features `gcp` and `azure`, `Cargo.toml:185-186`) become
  plugin crates there: `Plugin`s holding `TestHooks`, `ArtifactsWrite` and `HostNet` that upload
  in `on_test_end`. Core loses the two features and `object_store/azure`; `object_log` stays for
  S3 (`src/plugins/mod.rs:11-12`); `mise.toml:57` and `:62` drop them from their feature lists.
- **OCR** (`src/ocr.rs`, 196 lines; ocrs, rten, image): no counterpart in Python, and a search
  for `crate::ocr` in `src/` finds no caller. It moves as a crate that reads PNG bytes.
- **Visual comparison** (`src/visual.rs`): open question 11, because `BaseCase` calls it
  (`src/api/base_case.rs:2679`).

## What users lose or must change

| Today | After the split |
|---|---|
| `BrowserConfig::fingerprint`, `with_fingerprint`, `native_spoofing_enabled` | `BrowserConfig::with_plugins` with the `Stealth` plugin |
| `LaunchOptionsBuilder::fingerprint`, `LaunchOptions::fingerprint`, `with_fingerprint` | `LaunchOptionsBuilder::plugins` with the `Stealth` plugin |
| a fingerprint for only a time zone, a place or a platform in CDP mode | `LaunchOptionsBuilder::timezone`, `geolocation`, `platform` (new in core) |
| `Page::apply_fingerprint`, `StealthPlan`, `Page::capture_fingerprint` | stealth's crate and its extension trait |
| `BaseCase::capture_fingerprint`, `compare_fingerprint` | stealth's extension trait on `BaseCase` |
| `BaseCase::human_type` paced by the fingerprint | paced by `Behavior`'s default, or one the program gives |
| UC mode's extra page script | stealth's plugin; core's UC mode removes the cdc properties only |
| UC mode with an unpatched driver | a patched driver at launch |
| root re-exports of the fingerprint types, providers, `CapturedFingerprint`, `ChromeBinaryPatcher`, `engine_spoofing_args` (`src/lib.rs:221-235`) | stealth; core keeps `ChromedriverPatcher`, `EnginePatch` and `find_system_chrome` (new path) |
| `seleniumbase_rs::evasion::...` paths | `undetected`, `browser::devtools`, `browser::options`, `sb_cdp::human`, `js_code`, or stealth |
| `fingerprint!` | stealth |
| `sbrs fingerprint ...`, `sbrs --fingerprint FILE`, `sbrs patch-chrome` | stealth's binary; `sbrs patch-chromedriver` stays |
| `SB_FINGERPRINT`, `[fingerprint] file`, the run option | the `Stealth` plugin's options |
| MCP `capture_fingerprint`, `compare_fingerprint`, `explain_fingerprint`, `patch_fingerprint`, `build_fingerprint`, `validate_fingerprint`, dev `check_fingerprint` | stealth's MCP tools; `patch_chromedriver` stays |
| `profile_payloads` | stealth |
| `plugins::gcp_logging_plugin`, `azure_logging_plugin`, features `gcp`, `azure` | the plugins repository |
| `ocr::Ocr`, feature `ocr` | the plugins repository |

Docs: 122 pages under `docs/` name a fingerprint (a count of files, not of the pages to change).
The ones that teach the moved API: `docs/stealth/` (`fingerprints`, `policies`, `profiles`,
`evasions`, `human-input`, `introduction`, `cdp-mode`, `proxies`, `troubleshooting`),
`docs/stealth/advanced/` (`binary-patching`, `providers`, `playwright`), `docs/tutorials/`
(`uc_mode`, `pure_cdp`, `browser_profiles`, `cloud_integrations`), `docs/api/crate-root.md`,
`docs/ABI_API.md:70`, `docs/glossary.md`, `docs/help/common_problems.md:74`,
`docs/DEVELOPER_GUIDE.md:1105`, `docs/adr/001-stealth-architecture.md`. They stay in this site
under Stealth, which already documents the stealth repository (`docs/stealth/api/`), with their
paths changed.

## Consumers and their contracts

| Consumer | What it imports of the moved API | After |
|---|---|---|
| stealth (`tests/contract_stealth.rs`) | `evasion::fingerprint::*` (`:37-75`), `evasion::humanize::Rng` (`:79`), `evasion::randomize::{Locale, LOCALES, chromium_hints}` (`:81`, `:209`), `evasion::behavior::Behavior` (`:83`), `Fingerprint` and `ProxyConfig` at the root (`:161`, `:204`) | owns the fingerprint types; the contract keeps `sb_cdp`, retry, shutdown, URLs, the error, and `Behavior` and `Rng` at their new core path |
| Tauri plugin (`tests/contract_tauri.rs`) | `sb_cdp::StealthPlan`, `Fingerprint`, `OsType` (`:22-26`, `:128`) | those three from stealth; `Browser`, `CdpInbox`, `CdpRequest`, `CdpTransport`, `Page` and `TransportError` stay in core |
| companion (`tests/contract_companion.rs`) | `Fingerprint`, `OsType` in its end-to-end crate, which reads a page report as a `Fingerprint` (`:63-68`, `:125-133`) | stealth's crate as a dev-dependency of that end-to-end crate |
| plugins (`tests/contract_plugins.rs`) | nothing: the reverse contract, what core uses of the API (`:1-17`) | gains the new `LaunchSettings` fields and the new requirement; adds the GCP, Azure and OCR crates |
| `tests/contract_script.rs` | its fixture's text names `evasion::fingerprint` (`:94-96`) | the fixture names a path that still exists (whether the fixture is compiled is _to verify_) |

Stealth depends on core by git with no revision (`tests/contract_stealth.rs:4-5`), so it meets
the removal the day it updates its lock file. The order below makes that day come after
stealth has its own copy.

## Leanness

What leaves a default library build (no features), in Rust code lines:

| Item | Lines |
|---|---|
| the fingerprint layer of `evasion` | 11,204 |
| `profile_payloads` | 2,156 |
| `src/sb_cdp/identity.rs` | 492 |
| `src/cli/fingerprint_cmd.rs` | 471 |
| `src/api/base_case_impls/base_case_impl_fingerprint.rs` | 98 |
| `src/browser/session/fingerprint.rs` | 97 |
| `src/sb_cdp/capture.rs` | 17 |
| **measured total** | **14,535** |
| `ChromeBinaryPatcher` and `engine_spoofing_args` in `patcher.rs`, the extra script in `uc.rs`, the fingerprint parts of `config/settings.rs`, `emulation.rs` and `macros.rs` | not measured on their own; a few hundred |

That is about 9% of the at most 156,946 lines a default build compiles, plus `capture.js` (307
lines of JavaScript). The command line, whose default build has `mcp-server`, also loses
`src/mcp/fingerprint.rs` (1,136), most of `src/mcp/stealth.rs` (118),
`crates/cli/src/fingerprint.rs` (215) and part of `src/mcp/dev/diagnose.rs` (1,440 in all).

Dependencies: no normal dependency leaves a default build. The moved code uses serde, rand,
base64 and regex, which stay; `rand_distr` stays with human input (`humanize.rs:86-91`). The
dev-dependency `schemars` leaves: only the fingerprint schema uses it (`Cargo.toml:254-256`; no
use outside `src/evasion/`). The plugins moves take the features `gcp`, `azure` (with
`object_store/azure`) and `ocr` (with ocrs and rten), none of which a default build compiles.

Compile units: the library is one crate, so no unit leaves a default build's graph; the gain is
the time that crate itself takes, which grows with its lines. It was not measured: this note ran
no cargo.

## Edge and Firefox

**Edge.** Python's UC mode does not run on Edge (`plugins/sb_manager.py:804-814`,
`core/browser_launcher.py:4614`). Ours accepts UC with Edge: `validate_mode_support` refuses only
a browser that is not Chromium-based (`src/browser/session/launch.rs:24-30`), and
`is_chromium_based` is true for Edge (`src/browser/config.rs:82-87`). The session then runs an
msedgedriver that nothing patches. That msedgedriver carries the same `cdc_` markers as
chromedriver is the lead's statement and is _to verify_ (a byte search of a downloaded
msedgedriver settles it).

- Core follows Python: UC mode with Edge is refused with Python's reason, in
  `validate_mode_support` (open question 6).
- Patching msedgedriver belongs in stealth, as an opt-in. It needs no new hook: the stealth
  helper patches a copy with core's `ChromedriverPatcher` (it works on any path,
  `patcher.rs:127-204`), starts it, and points the session at it with `auto_start_driver = false`
  and `webdriver_url` (`src/browser/options/session.rs:123`, `:183`). Whether the UC launch
  arguments then reach Edge through that path is _to verify_.

**Firefox.** Neither Python mode runs on Firefox, and no fingerprint reaches Firefox here either:
the UC and CDP modes refuse it (`launch.rs:24-30`), and the plugin host runs scripts there only
after each navigation (`src/browser/plugin_hooks.rs:743-772`). After the split:

- core: thirtyfour's `bidi` feature, `webSocketUrl` in the Firefox capabilities, and
  `script.addPreloadScript` behind `PageHandle::add_init_script` on the WebDriver host (hook 4).
  It serves every plugin;
- stealth: the Firefox fingerprint (providers that know the claimed and the running engine,
  Firefox's `oscpu` and `buildID`, BiDi `emulation.*` commands) through that hook.

## Sequencing

### The work in flight

- **0.8.0** (`release/0.8.0`; the workspace version is already 0.8.0, `Cargo.toml:17`). The
  split breaks the public API, so it lands in 0.9.0. 0.8.0 ships as it is.
- **Stealth API part 2.** Core's `lane/stealth-core` changes `evasion::randomize` (`0a59c08d`,
  `Locale::accept_language_list`); stealth's `lane/stealth-wire` and `lane/stealth-scrape` (and
  `lane/launch-options`) change what stealth imports. They define the contract the move carries,
  so they merge first.
- **Mimic** (`docs/design/fingerprint_mimic.md` on `lane/design-presets`), to be built today. Its
  first lane rewrites `src/evasion/spoof.rs`, `fingerprint.rs`, `randomize.rs`, `providers/` and
  adds `src/evasion/mimic.rs`; its second lane is Firefox.
- **The five thirtyfour notes** on `lane/design-thirtyfour`: `driver_error.md`,
  `network_throttle.md`, `devtools_client.md`, `capability_types.md` and `drivers_module.md`.
  The DevTools client note changes `src/evasion/cdp.rs` and the functions of `evasion::uc` that
  take a `CdpClient`, and its open question 2 recommends leaving `CdpClient` at
  `evasion::cdp`. Since the split moves it anyway, this note asks that lane to move it to
  `browser::devtools` in the same change. The capability note probably changes
  `src/evasion/options.rs` (_to verify_).

### Where Mimic is implemented

**Recommendation: in core, in `src/evasion`, today, as its first lane is written (Chromium
only).** The layer it extends lives there, and the split cannot land today: it needs the
untangle lane, the plugin API hooks and stealth's crate first. Built in core now, Mimic moves
with the rest of the layer, as one more module (`mimic.rs` and its renderings). The lane keeps
to two rules so that the later move stays a move:

- everything Mimic adds lives under `src/evasion/` or in the files its note already lists (the
  `sbrs fingerprint` code, `src/mcp/dev/diagnose.rs`, the stealth contract);
- no new type of `evasion` is used from `browser/`, `sb_cdp/` or `api/`.

Mimic's second lane (Firefox) waits for the split. Its core part is hook 4 (the BiDi preload
path); its fingerprint part is a stealth lane on stealth's crate.

The split helps Mimic later: its catalogue of real browser builds and stealth's measured wire
fingerprints (TLS, HTTP/2) describe the same browsers, and both will live in one repository.

The alternative, building Mimic in stealth from the start, needs stealth's crate first, which
delays Mimic until the move lanes are done.

### The order of the lanes

| # | Lane | Repository | After | What |
|---|---|---|---|---|
| 0 | release 0.8.0 | core | none | as planned |
| 1 | stealth API part 2 | core, stealth | none | in flight |
| 2 | Mimic, first lane | core | 1 | in `src/evasion`, by the rules above |
| 3 | thirtyfour lanes | core | none | the DevTools client lane moves `CdpClient` to `browser::devtools` |
| 4 | untangle | core | 2, 3 | moves only, no behaviour change: the homes of [section 2](#2-what-core-keeps-of-evasion); afterwards `evasion` holds only the fingerprint layer |
| 5 | Python's CDP settings | core | 4 | `timezone`, `geolocation`, `platform` on both engines without a `Fingerprint` |
| 6 | plugin API hooks | plugins, then core | none (with 4) | hooks 1 to 4; a new API revision; core moves its pin |
| 7 | stealth crate | stealth | 4, 6 | the layer, `profile_payloads`, `StealthPlan`, capture, the `Stealth` plugin, the extension traits, the binary and MCP tools; stealth's crates import from it |
| 8 | removal | core | 7 merged | deletes the layer, UC's extra script, the settings, the commands and the MCP tools; UC patches its driver; UC with Edge refused; contracts and docs |
| 9 | consumers | Tauri plugin, companion | 8 | their imports move to stealth's crate |
| 10 | uploaders and OCR | plugins, then core | none | any time; core drops `gcp`, `azure`, `ocr` |

Lanes 4, 5 and 8 touch the same launch files (`src/sb_cdp/launch/`, `src/browser/session/`), so
they run one after another. Lanes 6 and 10 run beside them.

## What the lanes touch

| Lane | Items |
|---|---|
| 4 untangle | `src/evasion/{cdp,options,uc,patcher,dprocess,reactor,behavior,humanize}.rs` and `js/`; their 55 `use crate::evasion` sites outside `src/evasion/` (one rewrite each); `crates/cli/src/sbrs.rs:33`, `:4070`; `src/lib.rs`; `tools/docs_layout_tree.py`; `tests/contract_stealth.rs` (`Behavior`, `Rng` paths) |
| 5 CDP settings | `src/sb_cdp/launch/{builder,layers}.rs`, `src/browser/config.rs`, `src/browser/session/emulation.rs`, `parity/api.toml` (the CDP-mode rows) |
| 6 hooks | `crates/seleniumbase-rs-plugin/src/lib.rs` (`LaunchSettings`, `Requirements`) in plugins; `src/sb_cdp/plugin_hooks.rs`, `src/browser/plugin_hooks.rs`, both `Cargo.toml` lines of the pin, `tests/contract_plugins.rs` |
| 8 removal | `src/evasion/`, `src/profile_payloads/`, `src/sb_cdp/{identity,capture}.rs`, `src/browser/session/{fingerprint,launch,cdp,emulation}.rs`, `src/browser/config.rs`, `src/config/{settings,test_options}.rs`, `src/cli/fingerprint_cmd.rs`, `crates/cli/src/{sbrs,fingerprint}.rs`, `src/mcp/{fingerprint,stealth,prompts}.rs`, `src/mcp/dev/diagnose.rs`, `src/macros.rs`, `src/lib.rs`; generated: `completions/sbrs.usage.kdl`, `docs/help/cli_reference.md`, `src/mcp/dev/api_index.json`, `schemas/fingerprint.schema.json`; contracts: stealth, Tauri, companion, script; docs as listed above; CHANGELOG under Breaking |
| 10 plugins | `src/plugins/{gcp_logging_plugin,azure_logging_plugin}.rs`, `src/ocr.rs`, `Cargo.toml` features, `mise.toml:57`, `:62`, `docs/tutorials/cloud_integrations.md` |

## Tests

- **Before the move (core, lane 4 or earlier):** a test that records, for each preset, the
  launch arguments, the init script and the protocol commands a fingerprint gives today
  (`StealthPlan::new(..).commands()`, `init_script()`, and the launch layer) into fixture files.
  Stealth's plugin must give the same, so the move is shown to change nothing.
- **Lane 4:** the whole suite, unchanged, and `mise run contracts` with the new paths.
- **Lane 5:** mock-transport tests (`Browser::new_mocked`) that the time zone, geolocation and
  platform reach `Emulation.setTimezoneOverride`, `Emulation.setGeolocationOverride` and
  `Network.setUserAgentOverride` in Python's order, written first.
- **Lane 6:** the plugin-testing fakes for each new hook; host tests that a plugin's user agent
  reaches one `--user-agent` and loses to the program's own; that a `RUN_FIRST` script is
  installed before every package; that WebDriver `block_urls` merges with the ad-block list; an
  `#[ignore]`d geckodriver test of the preload path.
- **Lane 8:** UC's page script holds the cdc removal and nothing else; a UC launch patches a
  fake chromedriver once and reuses the cached copy (as the patcher's tests fake one,
  `src/evasion/patcher.rs:717-826`); UC with Edge is refused; `tests/offline/docs_current.rs`
  forbids the removed paths.
- **Stealth:** the moved unit tests, the fixture comparison above, and its Chrome tests.

## Risks and costs

- **Two plugin API crates.** If stealth names `seleniumbase-rs-plugin` at another revision, its
  plugin implements a trait core does not call, and the error appears where the set is built.
  The rule in section 3 (import through `seleniumbase_rs::plugin_api`) avoids it.
- **A broken stealth build.** Stealth follows core's `main` with no revision; lane 8 must merge
  after lane 7, with the contract changed in the same commit.
- **UC detection.** UC users lose the extra script. The script fakes values that contradict a
  real machine (an "Intel Iris" renderer on any GPU, 8 cores everywhere,
  `src/evasion/uc.rs:46-48`, `:131-147`), so a detector may score the reduced script better, not
  worse; that is _to verify_ on fixture pages before lane 8, and the CHANGELOG says how to get
  the extras back (the `Stealth` plugin).
- **Ordering.** Without hook 2 a plugin's package can read `navigator` before the fingerprint
  script has changed it.
- **Cost.** About ten lanes in four repositories. Lane 8 is the largest (about 15,000 lines out,
  four contracts, about twenty docs pages). The Tauri plugin and the companion each need a
  release that follows.
- **Two copies for a while.** Between lanes 7 and 8 both repositories hold a `Fingerprint`;
  changes in that window (Mimic follow-ups) must go to stealth's copy only.
- **Stale Python facts.** The Python checkout's index is 14 days old; a newer SeleniumBase may
  have added what this note says it lacks.

## Open questions

1. **The stealth crate's name and shape.** Recommendation: one crate,
   `seleniumbase-rs-fingerprint`, holding the layer and the `Stealth` plugin, re-exported by
   stealth's facade. Alternative: a data crate and a plugin crate.
2. **The name of core's UC part.** Recommendation: `undetected`, Python's package name.
   Alternative: keep `evasion` for what stays, which keeps a name that no longer says what the
   module is.
3. **Human input in core.** Recommendation: yes, as `sb_cdp::human`, because the captcha solvers
   Python has use it. Alternative: move it to stealth and give the solvers Python's straight,
   tweened moves.
4. **UC's page script.** Recommendation: Python's (cdc removal and the `HeadlessChrome` fix),
   after the fixture-page check in [Risks and costs](#risks-and-costs).
5. **UC patches chromedriver at launch.** Recommendation: yes, with the cached copy of
   `sbrs install uc_driver`.
6. **UC with Edge.** Recommendation: refuse it with Python's reason; patching msedgedriver is a
   stealth helper, once the markers are confirmed.
7. **Python's CDP launch settings in core.** Recommendation: yes (lane 5), before the fingerprint
   leaves.
8. **`sbrs --fingerprint`, `SB_FINGERPRINT` and `[fingerprint]`.** Recommendation: they leave
   core; stealth ships a binary with the fingerprint commands, and the plugin reads its options
   from the plugin table of the settings. Alternative: a `crates/cli` feature that links stealth,
   which makes core depend on stealth.
9. **The fingerprint MCP tools.** Recommendation: stealth serves them; core's `sb` and `dev`
   servers keep `patch_chromedriver` and lose the rest.
10. **GCP and Azure as `Plugin`s or as plain crates.** Recommendation: `Plugin`s that upload in
    `on_test_end`, so `--plugin` turns them on.
11. **Visual comparison.** Recommendation: keep it in core for now. `BaseCase::assert_visual_match`
    calls it, it is gated, and a default build does not compile it; revisit when a plugin can add
    an assertion.
12. **`ProfileVault`.** Recommendation: move it to stealth with the profile payloads; it stores
    profiles with proxy passwords and has no caller here.
13. **The importer and the MCP servers in a default build.** Recommendation: a later lane, outside
    this split, makes each its own workspace crate in this repository (Python keeps its MCP
    server as a separate package, `mcp_servers/`). The importer alone is about 17,200 lines of a
    default library build.
14. **The docs.** Recommendation: the pages stay in this site under Stealth, with stealth's paths;
    no second site.
15. **When.** Recommendation: 0.9.0, after 0.8.0 ships, in the order of the lane table.
