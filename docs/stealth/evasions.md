---
title: "Evasions and launch flags"
sidebarTitle: "Evasions and launch flags"
---

A fingerprint reaches Chrome in three forms: launch flags, given when the
browser starts; evasions, page scripts that run in every document before the
page's own scripts; and native-level overrides sent over the DevTools Protocol.

## Launch flags

`evasions::launch_args(&fingerprint)` returns the Chromium arguments a
fingerprint stands for:

- switches that remove automation markers and first-run noise, such as
  `--disable-blink-features=AutomationControlled`, `--disable-infobars` and
  `--no-first-run` (and, on Linux only, `--no-sandbox`);
- `--user-agent`, `--lang` and `--window-size` when the fingerprint gives a
  user agent, a locale or a screen size;
- `--force-webrtc-ip-handling-policy` only when WebRTC is masked (see
  [Proxies and network](/stealth/proxies#webrtc));
- `--proxy-server` when the fingerprint names a proxy, never with its password;
- `--disable-quic` or `--enable-quic` when the policy's `quic` setting asks
  for it.

Switches that a profile file carries go through `evasions::permitted_switch`:
a switch must be a plain name with a one-line value and be on the permitted
list, so a payload cannot hand Chrome a flag such as `--renderer-cmd-prefix`,
which runs a command. Refused switches are left out with a warning. Add flags
you trust yourself (`.arg(..)` on the pure CDP launch options).

UC mode adds its own launch arguments and leaves out `enable-automation`; see
[What is patched automatically](/tutorials/uc_mode#what-is-patched-automatically)
and [Extra evasions you can apply](/tutorials/uc_mode#extra-evasions-you-can-apply).
The engine-spoofing arguments are on
[Binary patching](/stealth/advanced/binary-patching#engine-spoofing-arguments).

## Evasion scripts

`evasions::session_script(&fingerprint)` is the script a session that drives a
real Chrome runs before any page script. The pure CDP engine and the WebDriver
sessions both use it, so one fingerprint gives the same page-side identity in
either. `evasions::bootstrap_script` is the full set, for a browser that lacks
`navigator.plugins`, `mimeTypes` and `navigator.connection`. Both are
assembled from the [evasion providers](/stealth/advanced/providers) in
priority order. WebDriver sessions also run `evasions::cdc_scrub()` on every
navigation, which removes the `cdc_` markers chromedriver leaves on `window`.

## Native-level (CDP) spoofing

When a session has a CDP connection (`DriverMode::Uc`, `DriverMode::Cdp`, or the
Pure CDP engine), a fingerprint is applied in two layers:

1. The evasion script, assembled from the providers below and registered with
   `Page.addScriptToEvaluateOnNewDocument`. Page scripts can *in principle*
   detect a JavaScript patch through `Function.prototype.toString`,
   `Object.getOwnPropertyNames`, or descriptor inspection, which is why the
   providers use the `sb` helpers to look native.
2. The protocol overrides from `evasions::safe_cdp_overrides`, which leave out the
   `Proxy-Authorization` header that would hand the proxy's login to every site.
   A command is sent
   for a setting that holds a value, so a `Natural` setting sends nothing:

| Command | Sent when |
|---|---|
| `Network.setUserAgentOverride` (user agent, platform, `userAgentMetadata`, and `acceptLanguage`) | `user_agent` is a value. `acceptLanguage` is sent only when `accept_languages` or `locale` is a value. The metadata comes from `client_hints` when set and is derived from the OS and user agent otherwise: Chrome's brand list, or Edge's when the user agent has an `Edg/` token. `client_hints` `Off` sends none. |
| `Emulation.setDeviceMetricsOverride` | `screen` is a value. `pixel_ratio` is the scale factor (1.0 if unset) and `mobile` follows `os_type`. `safe_cdp_overrides` sets the viewport (`width`, `height`) to the screen, and the WebDriver path sends it as it is. The Pure CDP engine changes it for a desktop persona: it sends a `0` by `0` viewport, which the protocol reads as "no override", plus `screenWidth` and `screenHeight`, so the browser itself reports `screen.width` and `screen.height` and the page keeps the viewport its window really has. A mobile persona is a phone-sized page, so its viewport stays the screen. |
| `Emulation.setLocaleOverride` | `locale` is a value. |
| `Emulation.setTimezoneOverride` | `timezone` is a value. |
| `Emulation.setGeolocationOverride` | `geolocation` is a position (accuracy 100 and altitude 0 unless set). |
| `Browser.grantPermissions` | The geolocation permission, when `geolocation` is a position, and the permissions listed under `grant_permissions` when that policy field is on. |
| `Network.setBlockedURLs` | One command, because a second would replace the first: the tracker hosts when the policy blocks trackers, and `*://localhost:PORT/*` (and `127.0.0.1`, `[::1]`) for each of `blocked_ports`. A session with ad blocking joins the ad block list to it. |
| `Page.setBypassCSP` | `disable_csp` is on. |
| `Emulation.setFocusEmulationEnabled` | `headless_fixes` is on. |

So a UC or CDP session already gets these overrides without `native_spoofing`,
and the JavaScript providers patch the same properties on top of them.
`native_spoofing` makes the providers stand down where a protocol override covers
the property, so the browser reports the value itself instead of a replaced
JavaScript getter:

```rust
use seleniumbase_rs_stealth::Fingerprint;

let fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 …")
    .platform("Win32")
    .screen(1920, 1080)
    .locale("en-US")
    .timezone("America/New_York")
    .geolocation((40.7580, -73.9855))
    .native_spoofing(true)
    .build()?;
Ok::<(), seleniumbase_rs_stealth::evasion::fingerprint::FingerprintError>(())
```

When `native_spoofing` is true:

* `navigator_props` leaves out `platform`, `userAgent` and `appVersion` (a given
  `app_version` is still scripted).
* `window_geometry` emits only `colorDepth` and `pixelDepth`; the size and scale
  come from `Emulation.setDeviceMetricsOverride`.
* `localization`, `timezone`, the JavaScript `geolocation` provider that answers a
  position, and `client_hints` are skipped. A denied geolocation (`Off`) is still
  scripted: no protocol command denies the request for one tab.
* The JS providers stay for everything the protocol does not cover:
  `hardwareConcurrency`, `deviceMemory`, `maxTouchPoints`, `vendor`, WebGL
  vendor and renderer, canvas and audio noise, media devices, fonts, battery,
  connection, speech, Bluetooth, the `navigator.webdriver` getter and the
  marker cleanup.
* A WebDriver session (Chrome, Chromium or Edge) also launches a patched copy of
  the browser. If `BrowserConfig::browser_binary_path` is unset, the session
  finds the system Chrome (`SB_CHROME_BIN`, else `find_system_chrome()`), writes a
  patched copy through `ChromeBinaryPatcher` with `EnginePatch::chrome_binary()`
  (webdriver markers, `navigator.webdriver` assignments and the `HeadlessChrome`
  user-agent marker), and sets `browser_binary_path` to it. It returns an
  `InvalidConfig` error naming `SB_CHROME_BIN` if no Chrome is found. A
  `browser_binary_path` that is already set is used as it is and is not patched;
  that includes the one `BrowserConfig::from_env()` takes from `SB_CHROME_BIN`.

### Caveats

<Warning>

* The protocol overrides and the script need a CDP connection. In
  `DriverMode::WebDriver`, neither is applied whether or not `native_spoofing` is
  set: only the launch arguments (and, with `native_spoofing`, the patched
  binary) take effect.
* `colorDepth` and `pixelDepth` are not covered by the device metrics, so a
  minimal JS patch is still emitted for them.
* Some signals (WebGL, canvas noise, audio noise) have no protocol override and
  stay JavaScript-based in native mode.
* Whether a given detector can still tell the difference is not something this
  crate measures.

</Warning>

## Complementary defenses

Runtime fingerprints work best when the browser also hides engine-level
automation markers:

* Patch the `chromedriver` binary with `ChromedriverPatcher` (for example
  `ChromedriverPatcher::new(path).patch(EnginePatch::all())?`) to remove injected
  `cdc_` and `__webdriver` signatures. See the
  [Binary Patching tutorial](/stealth/advanced/binary-patching).
* Pass the extra Chromium flags returned by `engine_spoofing_args()` (a
  `Vec<String>`) through `BrowserConfig::with_extra_args`, which replaces the
  list (`push_extra_arg` adds one), or through `StealthOptions::extra_args`.
* Combining a `Fingerprint`, UC mode, binary patching, and engine args covers the
  most layers this crate offers. None of it guarantees that a given detector
  passes.

