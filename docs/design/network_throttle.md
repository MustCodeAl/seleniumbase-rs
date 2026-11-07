---
title: "Design: one network throttle for both engines"
sidebarTitle: "One network throttle"
---

Status: implemented on `lane/thirtyfour`, with every recommendation accepted (2026-11-07). One of the lanes that
[the element note](/design/element_type) recommends in open question 9 (one lane for each
thirtyfour type left in the public API), after [the cookie note](/design/cookie_type) and
[the error note](/design/driver_error). This one is thirtyfour's `NetworkConditions`, which
thirtyfour itself has deprecated.

The note was written before the code and is kept as it was; these things turned out differently
or were added. The line numbers it quotes were those of an earlier revision. It says `Emulation::offline` "is
tested", but the test it names only checks that no detectable domain is enabled and never read the
parameters; they are read now (`protocol_params`, and `tests/offline/sb_cdp_mobile.rs`). Context7
again showed neither `Network.emulateNetworkConditions` nor a deprecation of it, only
`Network.overrideNetworkState` and `Network.emulateNetworkConditionsByRule`, so open question 4
stays at "keep it"; what was seen instead is Chrome 157 with its chromedriver, in the plain
WebDriver mode: a latency of 400 ms made a local page load in 426 ms where it took 70 ms, an offline
tab failed the load with `net::ERR_INTERNET_DISCONNECTED`, and the default put it back to 70 ms.
Four things are the code's own: a crate-private `NetworkThrottle::commands()` (the two commands in
order) beside `protocol_params`, so both engines send them from one place; a session attached over
CDP (`cdp_url`) throttles its own tab, which the note did not mention; the session finds a browser
without a DevTools channel from the `browserName` its driver reported (`firefox`, `safari` and
`internet explorer`, as lower-case prefixes), because it keeps no browser kind; and the session
words a refused command as `CdpDriver` itself, because `CdpClient` still words every refusal as
`Unsupported` until [the DevTools client lane](/design/devtools_client). The example keeps
`DriverMode::Cdp`, because `clear_browser_cache`, which it calls between the loads, still needs the
CDP client. The `expect(deprecated)` of `src/evasion/cdp.rs` stays for `ChromeDevTools`, with its
reason reworded. `docs/ABI_API.md` still listed `Key` among the `drivers` items, which an earlier
lane had removed; that line is corrected together with the `NetworkConditions` one, and the
`docs_current` test now names `set_network_conditions`, `NetworkConditions` and
`drivers::ConnectionType` as gone.

## The problem

Three public methods take thirtyfour's type, and `drivers` re-exports it with its
`ConnectionType` (`src/drivers.rs:36-43`):

| Method | Where |
|---|---|
| `BaseCase::set_network_conditions(&self, conditions: &NetworkConditions)` | `src/api/base_case.rs:1310-1316` |
| `BrowserSession::set_network_conditions(&self, conditions: &NetworkConditions)` | `src/browser/session/cdp.rs:220-227` |
| `CdpClient::set_network_conditions(&self, conditions: NetworkConditions)` | `src/evasion/cdp.rs:222-232` |

What thirtyfour says about it (Context7, `/websites/rs_thirtyfour`,
`extensions/cdp/networkconditions.rs`; thirtyfour 0.38.1 is what `Cargo.lock` resolves):

- `NetworkConditions` and `ConnectionType` are `#[deprecated(since = "0.37.0")]`, with the note
  to use `thirtyfour::cdp::domains::network::NetworkConditions` through `WebDriver::cdp()`.
- They are the body of chromedriver's own endpoint `/session/{id}/chromium/network_conditions`,
  in snake_case on the wire, not of a DevTools command; thirtyfour's comment there sends
  `Network.emulateNetworkConditions` users to the CDP type instead.
- The fields: `offline: bool`, `latency: u32` (milliseconds), `download_throughput` and
  `upload_throughput: i32` (bytes per second, `-1` for no limit), and
  `connection_type: Option<ConnectionType>` (nine values, `None` to `Other`). The `Default` is
  no throttling.

Four things are wrong besides the type:

1. **The doc comments say "via CDP".** `base_case.rs:1310`, `session/cdp.rs:220` and
   `evasion/cdp.rs:222` say so, and the example says throttling "goes through the Chrome
   DevTools Protocol" (`examples/network_conditions.rs:9-12`). The call is thirtyfour's
   `ChromeDevTools::set_network_conditions` (`evasion/cdp.rs:227-229`), which thirtyfour
   documents as that chromedriver endpoint.
2. **It needs the CDP or UC mode for no reason.** `BrowserSession::set_network_conditions`
   asks `cdp_client()` (`session/cdp.rs:225`), which is `Unsupported` ("CDP is unavailable for
   this session. Enable mode cdp or uc.", `:298-304`) in the plain WebDriver mode. Yet the
   session can send DevTools commands in every mode: `devtools()` builds a client from the
   driver (`:30-48`), and `apply_ad_block` does the same (`session/emulation.rs:44-53`).
3. **A refusal is `Unsupported`** (`evasion/cdp.rs:230`), exit code 2, "the input was wrong"
   (`src/error.rs:572`), where the pure CDP engine reports a refused command as `CdpDriver`
   (`src/sb_cdp/state.rs:693-697`).
4. **Six places silence the deprecation**: `#![expect(deprecated)]` in
   `src/api/base_case.rs:1-6`, `src/browser/session/cdp.rs:5-10` and `src/evasion/cdp.rs:7-12`
   (shared there with `ChromeDevTools`), the `#[expect]` on the re-export
   (`src/drivers.rs:36-43`), the example (`examples/network_conditions.rs:24-29`) and the
   own-imports test (`crates/own-imports/tests/drivers_and_json_from_our_crate.rs:8-11`,
   `:45-55`).

### What the pure CDP engine has

- **No type.** `Emulation::offline(bool)` (`src/sb_cdp/state.rs:693-712`) sends
  `Network.enable`, then `Network.emulateNetworkConditions` with `offline`, a latency of 0 and
  both throughputs at `-1`. It is tested (`tests/offline/sb_cdp_domain_hygiene.rs:350-351`) and
  documented (`docs/concepts/network.md:567-568`). So throttling has no name of its own yet in
  either engine, and one type can serve both.
- **The pattern to follow is `DeviceMetrics`** (`src/browser/options/device.rs:41`): one type
  for both engines, taken by `Emulation::device_metrics` (`state.rs:755-772`) and by the
  WebDriver session's settings, with the DevTools commands built in one place
  (`DeviceMetrics::emulation_commands`, `device.rs:90-122`).

The protocol (Context7, `/websites/chromedevtools_github_io_devtools-protocol_tot`):
`Network.overrideNetworkState` ("Override the state of navigator.onLine and
navigator.connection": `offline`, `latency`, `downloadThroughput`, `uploadThroughput`,
`connectionType`) and `Network.emulateNetworkConditionsByRule` (conditions per URL pattern)
exist beside `Network.emulateNetworkConditions`, the command this crate already sends. The
pages Context7 returned did not show `Network.emulateNetworkConditions` itself, so whether the
protocol now marks it deprecated is to verify (open question 4).

### Python parity

SeleniumBase has no such method: its source names neither `set_network_conditions` nor
`emulateNetworkConditions`, and `parity/api.toml` has no row for it. The name is ours to
choose.

### Consumers

None. Stealth, the plugins, the Tauri plugin and the companion name neither
`NetworkConditions` nor `set_network_conditions`, and no contract file does. In this
repository: `examples/network_conditions.rs`, `README.md:315`, `docs/tutorials/cdp_mode.md:38`,
`:97`, `:215-225`, `docs/api/cdp-uc.md:40`, `docs/tutorials/getting_started.md:374`,
`docs/ABI_API.md:29`, the own-imports test and `src/mcp/dev/api_index.json:523`. No test calls
the method.

## The proposal

1. **One type, ours: `NetworkThrottle`**, in `src/browser/options/` beside `DeviceMetrics`,
   re-exported at the crate root (`seleniumbase_rs::NetworkThrottle`):

   ```rust
   /// How the browser's network behaves: cut off, slowed down, or as it is.
   /// The default changes nothing.
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
   pub struct NetworkThrottle {
       /// No network at all: every request fails at once.
       pub offline: bool,
       /// Added before each response starts to arrive.
       pub latency: Duration,
       /// The most bytes per second the page may receive; `None` for no limit.
       pub download: Option<u32>,
       /// The most bytes per second the page may send; `None` for no limit.
       pub upload: Option<u32>,
   }
   ```

   `Duration` and `Option` take the place of milliseconds in a `u32` and of the `-1`. The
   default is what the example uses to put the connection back
   (`examples/network_conditions.rs:69-70`). `connection_type` is left out (open question 2).

2. **One place builds the command.** A crate-private `NetworkThrottle::protocol_params(&self)
   -> Value` gives the `Network.emulateNetworkConditions` parameters (milliseconds, `-1` for
   `None`). `Emulation::offline(offline)` sends
   `NetworkThrottle { offline, ..Default::default() }` through it, which is the JSON it sends
   today (`state.rs:703-708`).

3. **The methods are named after the type**:
   - `Emulation::network_throttle(&self, throttle: &NetworkThrottle)` in `sb_cdp`, beside the
     other `Emulation` setters, which are named after what they set (`timezone`, `locale`,
     `geolocation`, `offline`, `device_metrics`, `touch`, `color_scheme`);
   - `BaseCase::set_network_throttle(&self, throttle: &NetworkThrottle)` and
     `BrowserSession::set_network_throttle`, in place of `set_network_conditions`, as the other
     `BaseCase` environment setters are `set_timezone` and `set_geolocation`;
   - `CdpClient::set_network_conditions` goes.

   The session sends `Network.enable` and `Network.emulateNetworkConditions` through the
   client of its mode, or through one it builds from the driver as `apply_ad_block` does, so
   the call works in every mode on a Chromium browser. On Firefox and Safari it is
   `Unsupported`, naming the browser. A refusal is `CdpDriver`, as on the pure CDP engine.

4. **`drivers::{ConnectionType, NetworkConditions}` go**, and with them five of the six
   `expect(deprecated)` places; the one in `src/evasion/cdp.rs` goes with `ChromeDevTools`, in
   the DevTools client lane.

5. **The example becomes `examples/network_throttle.rs`.** The examples are found by Cargo, not
   declared (the Dockerfile skeleton names only the declared ones, `Dockerfile:29-32`), so
   only `README.md:315` follows the rename.

## What the lane touches

| Area | Items |
|---|---|
| Code | `src/browser/options/` (`NetworkThrottle`, new, and its `mod.rs`); `src/lib.rs` (root re-export); `src/sb_cdp/state.rs:693-712` (`offline`, new `network_throttle`); `src/api/base_case.rs:1-6`, `:1310-1316`; `src/browser/session/cdp.rs:5-13`, `:220-227`; `src/evasion/cdp.rs:222-232`; `src/drivers.rs:36-43` |
| Examples | `examples/network_conditions.rs` becomes `examples/network_throttle.rs`; `README.md:315` |
| Tests | `crates/own-imports/tests/drivers_and_json_from_our_crate.rs:8-16`, `:45-55` (builds a `NetworkThrottle` from the crate root) |
| Docs | `docs/tutorials/cdp_mode.md:38`, `:97`, `:215-225`; `docs/api/cdp-uc.md:40`; `docs/concepts/network.md:567-568` (the new `Emulation` method beside `offline`); `docs/tutorials/getting_started.md:374`; `docs/ABI_API.md:29`; `docs/api/crate-root.md` (the new root item); CHANGELOG under Breaking |
| Generated | `src/mcp/dev/api_index.json` (`SB_UPDATE_API_INDEX=1`) |
| Contracts | unchanged; check with `mise run contracts` |

## Tests

- Unit tests of `protocol_params`: the default gives the JSON `Emulation::offline(false)`
  sends today; a throttle gives milliseconds and bytes per second; `None` gives `-1`.
- A mock-transport test (`Browser::new_mocked`, as `tests/offline/sb_cdp_domain_hygiene.rs`
  does for `offline`): `Emulation::network_throttle` sends `Network.enable`, then the command
  with those parameters.
- A `MockWebDriver` test in the plain WebDriver mode: `BrowserSession::set_network_throttle`
  posts both commands to `/goog/cdp/execute` (as `base_case_impl_cdp_tests.rs:19-55` checks
  that path). Written first; today it fails with "CDP is unavailable".
- The renamed example stays the real-browser check; it runs only when the owner says the Mac
  is free.

## Open questions

1. **The name `NetworkThrottle`.** Recommendation: yes; no item in the crate has "throttle" in
   its name (a literal search finds it only in comments, strings and Chrome switches), and it says what the
   value does. Alternative: `NetworkConditions`, which is thirtyfour's name and the protocol's.
2. **Leave out `connection_type`.** Recommendation: yes, until a test needs it. Context7 shows
   `connectionType` as what `Network.overrideNetworkState` reports to `navigator.connection`;
   what it changes for `Network.emulateNetworkConditions` is to verify, and what a page reads
   from `navigator` belongs with the fingerprint more than with throttling.
3. **Presets** such as Chrome's "Fast 3G". Recommendation: none; Chrome's numbers are not
   verified here (the example says "roughly", `examples/network_conditions.rs:57`), and a
   struct literal with `..Default::default()` is short.
4. **Which protocol command.** Recommendation: keep `Network.emulateNetworkConditions`, which
   the crate already sends and tests (`state.rs:702`); the lane checks on Context7 whether the
   protocol marks it deprecated, and moves to `Network.emulateNetworkConditionsByRule` only if
   so.
5. **Send it again on a window switch**, as the ad block is (`session/emulation.rs:73-87`).
   Recommendation: no; it belongs to the tab, as `Emulation` belongs to a `Page`, and the doc
   comment says so.
6. **`BaseCase::set_offline(bool)`.** Recommendation: no; `NetworkThrottle { offline: true,
   ..Default::default() }` does it on both engines.
