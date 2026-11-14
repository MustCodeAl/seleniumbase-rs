---
title: "Design: the WebDriver engine's DevTools client without thirtyfour types"
sidebarTitle: "The DevTools client"
---

Status: proposal (2026-11-05). One of the lanes that
[the element note](/design/element_type) recommends in open question 9 (one lane for each
thirtyfour type left in the public API). This one is `CdpClient`, the client through which a
WebDriver session sends DevTools commands. It comes after
[the network throttle note](/design/network_throttle), which takes
`CdpClient::set_network_conditions` away.

## The problem

`CdpClient` (`src/evasion/cdp.rs:64-75`) shows two thirtyfour types:

```rust
/// Thin wrapper around [`ChromeDevTools`] for issuing CDP commands.
pub struct CdpClient {
    pub dev_tools: ChromeDevTools,
}

impl CdpClient {
    /// Builds a CDP client from a shared session handle.
    pub fn from_handle(handle: Arc<thirtyfour::session::handle::SessionHandle>) -> Self
}
```

- **`ChromeDevTools` is deprecated** since thirtyfour 0.37.0 (Context7, `/websites/rs_thirtyfour`,
  `extensions/cdp/struct.ChromeDevTools`): thirtyfour points to its `Cdp` type, which
  `WebDriver::cdp()` gives ("a cheap operation that clones the underlying session handle"), and
  whose `send_raw(method, params)` sends a command by name. `src/evasion/cdp.rs:7-12` silences
  the warning.
- **Nobody reads the field** outside the file: a literal search for `.dev_tools` elsewhere
  finds nothing.
- **`from_handle` has 8 callers, all in the crate, all the same call**,
  `CdpClient::from_handle(driver.handle().clone())`: `src/browser/session/cdp.rs:47`,
  `session/connect.rs:62`, `:108`, `session/emulation.rs:50`, `:104`,
  `session/lifecycle.rs:109`, `src/browser/plugin_hooks.rs:799` and `src/browser/reuse.rs:738`.
- **The public ways to a client** are `BrowserSession::devtools()`, which builds one in every
  mode (`session/cdp.rs:30-48`) and which the recorder uses (`base_case_impl_recorder.rs:114`),
  and the public functions of `evasion::uc` that take `&CdpClient` (`src/evasion/uc.rs:221`,
  `:227`, `:242`, `:255`, `:281`).

Two things are wrong besides the types:

1. **A refused command is `Unsupported`.** Every method turns the driver's error into
   `SeleniumBaseError::Unsupported(e.to_string())` (`evasion/cdp.rs:89`, `:103`, `:123`, `:138`,
   `:151`, `:159`, `:168`, `:177`, `:230`), and so do `activate_cdp_mode` and `enable_uc_mode`
   (`session/cdp.rs:127`, `:251`). So the same `BaseCase::execute_cdp_cmd`
   (`base_case_impl_misc.rs:364-371`) fails as `CdpDriver` on a direct CDP attachment, where it
   goes to `Page::execute` (`session/cdp.rs:146-148`, which documents `CdpDriver`,
   `src/sb_cdp/page.rs:137-154`), and as `Unsupported` on a WebDriver session: exit code 2, "the
   input was wrong", instead of 4 (`src/error.rs:567-577`), and without the advice `cdp_hint`
   gives about a closed tab or a destroyed context (`error.rs:993-1014`).
2. **Three names for one call.** `execute(method)`, `execute_with_params(method, params)` and
   `send_command(cmd, params)`, which drops the answer (`evasion/cdp.rs:93-152`), where the pure
   CDP engine has one, `Page::execute(method, params)` (`sb_cdp/page.rs:148`).
   `execute_with_params` has 13 callers in the crate; `send_command` has none.

### Python parity

Python's `execute_cdp_cmd(script, *args, **kwargs)` calls Selenium's
`driver.execute_cdp_cmd` (`base_case.py:3554-3557`); this crate's is
`BaseCase::execute_cdp_cmd` (`parity/api.toml:597-599`). SeleniumBase hands out no DevTools
object of its own, so there is no Python name to follow for the client.

### Consumers

None. Stealth, the plugins, the Tauri plugin and the companion name neither `CdpClient` nor
`evasion::cdp`; stealth's contract names `evasion::fingerprint`, `humanize`, `randomize` and
`behavior` only (`tests/contract_stealth.rs:37-83`). In this repository,
`tests/offline/sb_cdp_domain_hygiene.rs:38` imports
`evasion::cdp::{is_detectable_enable, DETECTABLE_DOMAINS}`, which stay. No docs page names `CdpClient` outside the design notes.

## The proposal

1. **The field becomes private**, and holds what the commands go through:

   ```rust
   /// The DevTools client of a WebDriver session: protocol commands sent
   /// through the session's driver.
   pub struct CdpClient {
       cdp: thirtyfour::cdp::Cdp,
   }
   ```

2. **`from_handle` becomes `pub(crate) fn new(driver: &WebDriver) -> Self`**, from
   `driver.cdp()`, and the 8 call sites become `CdpClient::new(driver)`. The public way to a
   client stays `BrowserSession::devtools()`.
3. **Commands go through `Cdp::send_raw`** instead of the deprecated `ChromeDevTools`, and the
   `#![expect(deprecated)]` at `evasion/cdp.rs:7-12` goes, once the network throttle lane has
   removed `set_network_conditions`. The lane first checks that `send_raw` still posts to the
   driver's `/goog/cdp/execute`: the `MockWebDriver` tests assert that path
   (`base_case_impl_cdp_tests.rs:55`, `session/emulation.rs:269`, `browser/reuse.rs:1286-1304`,
   `browser/plugin_hooks.rs:1702`, `src/api/cdp_page.rs:1121`,
   `base_case_impl_recorder.rs:648`), so they tell at once if it does not. If it does not, the
   private field stays a `ChromeDevTools` with its `expect`, and the public API is the same.
4. **A refused command is `CdpDriver`** with the browser's message, as on the pure CDP engine.
   `Unsupported` stays for a session that has no DevTools at all (`session/cdp.rs:41-45`,
   `:298-304`).
5. **One name for one call**: `CdpClient::execute(&self, method: &str, params: Value) ->
   Result<Value, SeleniumBaseError>`, as `Page::execute`. `execute_with_params` and
   `send_command` go, and the callers of the old `execute(method)` pass `json!({})`.

## What the lane touches

| Area | Items |
|---|---|
| Code | `src/evasion/cdp.rs` (the struct, `new`, the methods, the errors, the `expect`); the 8 `from_handle` sites; the 13 `execute_with_params` sites (one `ast_grep_rewrite`); `src/browser/session/cdp.rs:117-151`, `:247-252` (the `Unsupported` mapping) |
| Tests | the `MockWebDriver` tests above, unchanged, as the check that the path stayed; new tests below |
| Docs | none name the client; the doc comments of `devtools()` and `execute_cdp_cmd` say which error a refusal is; CHANGELOG under Breaking |
| Generated | `src/mcp/dev/api_index.json` only if a `BaseCase` signature changes (none does) |
| Contracts | unchanged; check with `mise run contracts` |

## Tests

- A `MockWebDriver` test (written first): a session whose driver refuses a DevTools command
  answers `BaseCase::execute_cdp_cmd` with `CdpDriver`, not `Unsupported`, and `exit_code()`
  is 4.
- A test that a direct CDP attachment and a WebDriver session give the same variant for the
  same refused command (the first through the mock transport, `Browser::new_mocked`).
- The existing tests that read `/goog/cdp/execute` pass unchanged.

## Open questions

1. **Should `CdpClient` stay public?** `BrowserSession::devtools()` returns it and
   `evasion::uc` takes it. Recommendation: yes, with no thirtyfour type in it; it is how a user
   of `BrowserSession` sends a DevTools command in the plain WebDriver mode.
2. **Its path.** `evasion::cdp::CdpClient` is not about evasion. Recommendation: leave the path;
   a move is a rename that gains nothing for this lane.
3. **`BaseCase` has three names for a raw command too**: `execute_cdp` and
   `execute_cdp_with_params` (`base_case.rs:1266-1276`) and Python's `execute_cdp_cmd`.
   Recommendation: a later lane keeps `execute_cdp_cmd`, the parity name, and removes the
   other two with the pages that teach them (`docs/tutorials/cdp_mode.md:38`).
4. **Two public types named `CdpEvent`**: `browser::options::CdpEvent`
   (`src/browser/options/cdp_events.rs:67`, re-exported at `options/mod.rs:60`; what
   `BrowserSession::cdp_events` returns, `session/cdp.rs:200`) and `sb_cdp::CdpEvent`
   (`src/sb_cdp/client.rs:63`, re-exported at `sb_cdp/mod.rs:118`). One name, two types.
   Recommendation: out of this lane; it goes on the list for the naming pass.
5. **Firefox and Safari.** `devtools()` on their sessions gives a client whose commands the
   driver refuses (`session/cdp.rs:33-34`). Recommendation: unchanged; the refusal becomes
   `CdpDriver` with the driver's message.
