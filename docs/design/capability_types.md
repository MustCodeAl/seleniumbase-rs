---
title: "Design: launch capabilities without thirtyfour types"
sidebarTitle: "Launch capabilities"
---

Status: proposal (2026-11-05). One of the lanes that
[the element note](/design/element_type) recommends in open question 9 (one lane for each
thirtyfour type left in the public API). This one is thirtyfour's capability types, which
three public functions take.

## The problem

| Item | Where | thirtyfour type |
|---|---|---|
| `CapabilityOverrides::apply_to(&self, caps: &mut Capabilities)` | `src/browser/options/capabilities.rs:149-162` | `Capabilities` |
| `StealthOptions::apply_to<C: ChromiumLikeCapabilities>(&self, caps: &mut C) -> Result<(), SeleniumBaseError>` | `src/evasion/options.rs:128-256` | the `ChromiumLikeCapabilities` trait |
| `apply_undetected_args<C: ChromiumLikeCapabilities>(caps: &mut C) -> Result<(), SeleniumBaseError>` | `src/evasion/options.rs:425-435` | the same trait |

For them, `drivers` re-exports six items: `ChromiumLikeCapabilities`,
`BrowserCapabilitiesHelper`, `Capabilities`, `CapabilitiesHelper`, `DesiredCapabilities` and
`LoggingPrefsLogLevel` (`src/drivers.rs:32`, `:45-47`).

Who calls them:

- **`CapabilityOverrides::apply_to`**: `session_capabilities` (`src/browser/session/capabilities.rs:30`)
  and one unit test (`capabilities.rs:395`).
- **`StealthOptions::apply_to`**: `apply_chromium_capabilities`
  (`session/capabilities.rs:212-217`), which every Chromium-family session goes through
  (Chrome, Chromium, Brave, Comet, Opera, Chrome for Testing, Chrome Headless Shell and Edge,
  `:38-56`); `StealthOptions::args` (`options.rs:354-362`); and unit tests (`:474`, `:641`).
- **`apply_undetected_args`**: one caller, `StealthOptions::apply_to` itself (`options.rs:233`).
- **`LoggingPrefsLogLevel`** is used only inside `apply_to` (`options.rs:347-348`), and
  `CapabilitiesHelper` only privately (`session/capabilities.rs:94`, `:111`).

So `apply_to` is the crate's own plumbing that happens to be public. What a user does with
capabilities already goes through our types:

- extra capabilities: `BrowserConfig::with_capabilities(CapabilityOverrides)`
  (`src/browser/config.rs:1110`), read from JSON or from Python's dictionary by
  `CapabilityOverrides::from_text` and `from_file`;
- extra launch arguments: `StealthOptions::extra_args` (`options.rs:47-48`);
- what the options produce: `StealthOptions::args() -> Result<Vec<String>, SeleniumBaseError>`
  (`options.rs:354-362`) and `default_uc_args() -> Vec<String>` (`options.rs:437-440`), which
  name no thirtyfour type.

Pages and tests that use the thirtyfour path:

- `docs/stealth/advanced/binary-patching.md:149-167` builds `DesiredCapabilities::chrome()`,
  calls `opts.apply_to(&mut caps)?` and reads `caps.args()` through
  `BrowserCapabilitiesHelper`, all imported from `drivers`;
- `crates/own-imports/tests/drivers_and_json_from_our_crate.rs:38-43` builds a
  `DesiredCapabilities` from `drivers`;
- `docs/tutorials/getting_started.md:26`, `:374`, `docs/ABI_API.md:29` and `context7.json:34`
  list `DesiredCapabilities` among the `drivers` types;
- `docs/DEVELOPER_GUIDE.md:1101` tells contributors to update `StealthOptions::apply_to` when
  they change launch arguments. It is a contributor page and stays true.

`examples/stealth_options.rs` uses `StealthOptions { .. }` and `args()` only (`:31`, `:58-63`,
`:101`), so it is the pattern already.

### Python parity

Python SeleniumBase takes extra capabilities as `--cap-string` and `--cap-file`; this crate has
them as `CapabilityOverrides::from_text` and `from_file` (`parity/api.toml:4103-4109` and the
`cap_file` / `cap_string` rows after it). Neither side hands the user a capabilities object to
fill.

### Consumers

None. Stealth, the Tauri plugin and the companion name no capability type of ours, and no
`StealthOptions`; the plugins repository's `Capability` (`crates/seleniumbase-rs-plugin/src/capability.rs:72`)
is a plugin's permission, another thing (open question 3). No contract file names
`CapabilityOverrides`, `StealthOptions` or `drivers::`.

## The proposal

1. **`CapabilityOverrides::apply_to` becomes `pub(crate)`.** The type stays as it is: built from
   text or a file, merged with `merge`, read with `get` and `keys`, given to
   `BrowserConfig::with_capabilities`.
2. **`StealthOptions::apply_to` becomes `pub(crate)`, and `apply_undetected_args` private**
   (its one caller is in the same file).
3. **What the options produce stays public in our types**: `StealthOptions::args()` and
   `default_uc_args()`, unchanged.
4. **`drivers` loses the six capability items**: `ChromiumLikeCapabilities`,
   `BrowserCapabilitiesHelper`, `Capabilities`, `CapabilitiesHelper`, `DesiredCapabilities` and
   `LoggingPrefsLogLevel`. The crate keeps using them privately from `thirtyfour`.
5. **The binary-patching page** shows the same check with `opts.args()?`:

   ```rust
   use seleniumbase_rs::evasion::options::StealthOptions;
   use seleniumbase_rs::engine_spoofing_args;

   fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
       let mut opts = StealthOptions::default();
       opts.extra_args = engine_spoofing_args();
       assert!(opts
           .args()?
           .contains(&"--disable-blink-features=AutomationControlled".to_owned()));
       Ok(())
   }
   ```

## What the lane touches

| Area | Items |
|---|---|
| Code | `src/browser/options/capabilities.rs:149-162`; `src/evasion/options.rs:128-133`, `:425-435`; `src/drivers.rs:32`, `:45-47` (with the module doc, `:4-8`) |
| Tests | `crates/own-imports/tests/drivers_and_json_from_our_crate.rs:13-16`, `:38-43` (the capability test builds a `CapabilityOverrides` and a `BrowserConfig` instead) |
| Docs | `docs/stealth/advanced/binary-patching.md:149-167`; `docs/tutorials/getting_started.md:26`, `:374`; `docs/ABI_API.md:29`; `context7.json:34`; CHANGELOG under Breaking |
| Generated | none |
| Contracts | unchanged; check with `mise run contracts` |

## Tests

- A test that `StealthOptions::from(&config).args()` equals the `args` of
  `goog:chromeOptions` in the capabilities the session asks for, for the same configuration,
  so that `args()` stays the true public view of the launch. `session_capabilities`
  (`session/capabilities.rs:14-32`) is `pub(super)`, so the test goes into `capability_tests`
  beside it (`:219`).
- The own-imports test proves that a crate that imports only `seleniumbase_rs` can still give
  capabilities: `CapabilityOverrides::from_text` and `BrowserConfig::with_capabilities`.
- The existing unit tests of `apply_to` (`options.rs:466-479` and the others) keep compiling;
  they are in the crate.

## Open questions

1. **Does a user need the whole capability object that `StealthOptions` builds** (preferences,
   `excludeSwitches`, logging preferences), not only the arguments? Recommendation: no, until
   one asks; then a method that returns it as JSON (`json::Value`), not a thirtyfour type.
2. **The name `StealthOptions`.** It builds the launch of every Chromium-family session, not
   only of stealthy ones (`session/capabilities.rs:212-217`). Recommendation: out of this lane;
   a renaming note of its own, since `docs/stealth/` and the developer guide teach the name.
3. **"Capability" means two things in the crate's public paths**: a WebDriver capability
   (`CapabilityOverrides`, the session's capabilities) and a plugin's permission
   (`plugin_api::Capability`, from the plugins crate, re-exported at `src/lib.rs:104`).
   Recommendation: keep both: the first is the WebDriver protocol's word and Python's
   (`--cap-string`), the second lives in its own crate and its own pages
   (`docs/plugins/api/capabilities.md`), and the docs of each say which one they mean.
