---
title: "Evasion providers"
sidebarTitle: "Evasion providers"
---

The page scripts a fingerprint turns into
([Evasions and launch flags](/stealth/evasions#evasion-scripts)) are assembled
by a registry of providers, in priority order. You can add your own.


Every evasion is an `EvasionProvider`: a small, self-contained unit that returns
a JavaScript snippet for an `EvasionContext` (the session fingerprint plus a
deterministic seed and runtime config). Providers are held by an
`EvasionRegistry` and assembled into a single bootstrap script in priority order
(lower `priority()` runs first). The script is one function; each snippet sits in
its own `try`/`catch`, after a `// provider: <name>` comment. `EvasionConfig`
turns either of those off.

```rust
use seleniumbase_rs_stealth::{default_registry, EvasionContext, Fingerprint};

let fp = Fingerprint::windows_desktop();
let ctx = EvasionContext::new(&fp);            // seed derived from the fingerprint
let script = default_registry().bootstrap(&ctx);
assert!(script.contains("webdriver"));
```

`evasions::bootstrap_script(&fp)` is a thin wrapper over
`default_registry().bootstrap(&EvasionContext::new(&fp))`, so existing callers
keep working while new evasions are added centrally. `evasions::session_script(&fp)`
is the same over `session_registry`, the script a browser session runs.

## Adding your own evasion

<Steps>

<Step title="Implement EvasionProvider" icon="code">

Implement `EvasionProvider` for a unit struct. Return the JavaScript from
`script`, gate inclusion with `applies`, and order it with `priority`.
`providers::SB_PREAMBLE` brings in the `sb` helpers that make a replaced
function look native; the example below skips it for brevity.

</Step>

<Step title="Register it at runtime" icon="plug">

```rust
use seleniumbase_rs_stealth::{default_registry, EvasionContext, EvasionProvider, Fingerprint};

struct HidePrint;
impl EvasionProvider for HidePrint {
    fn name(&self) -> &str { "hide_print" }
    fn priority(&self) -> i32 { 140 }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some("window.print = function() {};".to_owned())
    }
}

let mut registry = default_registry();
registry.register_provider(Box::new(HidePrint));
let fp = Fingerprint::windows_desktop();
let script = registry.bootstrap(&EvasionContext::new(&fp));
assert!(script.contains("hide_print"));
```

</Step>

</Steps>

See [Contributing](/CONTRIBUTING) for adding a built-in provider.

## Built-in providers

`default_registry()` registers 29 providers, listed here in execution order. "Runs
when" is the provider's `applies` condition. A dimension "is on" when
`Fingerprint::is_masked` says so: a setting that belongs to it is not `Natural`,
or, for the dimensions without a setting of their own, the policy switches it on
(see [One setting, four states](/stealth/fingerprints#one-setting-four-states)).

The Pure CDP engine and the WebDriver `Uc` and `Cdp` modes do not use
`default_registry()`. They use `evasions::session_script(&fp)`, which is
`session_registry(&fp)`: the same list without `plugins` and `connection`
(`SESSION_OMITTED_PROVIDERS`), which is 27 providers. A real Chrome already has
`navigator.plugins`, `navigator.mimeTypes`, `navigator.connection` and the PDF
viewer flag, and answers for them itself, so a replaced getter is one more thing
a page can find that a real Chrome does not have. The assembled session script
never mentions `plugins`, `mimeTypes`, `pdfViewerEnabled` or `connection`.
`default_registry()` is unchanged, for callers that want those spoofs, such as a
browser that really lacks them, and it is what `evasions::bootstrap_script` and
the Playwright session use.

| Priority | Name | Runs when | What it does |
|---|---|---|---|
| 5 | `native_tostring` | the policy's `native_tostring` is on | Makes replaced functions print `[native code]` from `Function.prototype.toString`, and installs the `sb` helpers the other providers use. |
| 10 | `webdriver` | always | Makes `navigator.webdriver` read `false` (a getter on the navigator prototype) and deletes known automation hooks from `window` (`callPhantom`, `_phantom`, `__nightmare`, `__selenium_unwrapped`, `__webdriver_evaluate`, `__driver_evaluate`, `domAutomation`, and others). |
| 15 | `cdp_markers` | always | Deletes `$cdc_`, `__webdriver`, `__selenium`, `__driver`, `$chrome_` and `__fxdriver` properties from `window`, its prototype chain and `document`. |
| 20 | `chrome_runtime` | the policy's `chrome_runtime` is on | `window.chrome.app`, `loadTimes`, `csi` and `runtime` stubs. |
| 30 | `navigator_props` | Navigator is on | `userAgent`, `appVersion` and `platform` (not under `native_spoofing`), then `hardwareConcurrency`, `deviceMemory`, `maxTouchPoints`, `vendor`, `productSub`, and `oscpu` and `buildID`, each only when it is a value. A given `app_version` is scripted even under `native_spoofing`. It does not touch `pdfViewerEnabled`, `plugins` or `mimeTypes`: Chrome answers for them itself. |
| 35 | `permissions` | the policy's `headless_fixes` is on | Makes `permissions.query({ name: 'notifications' })` agree with `Notification.permission`. |
| 40 | `plugins` | Navigator is on | Five PDF-viewer plugins and their MIME types. In `default_registry()` only. |
| 45 | `window_geometry` | Screen is on | `screen.width`, `height`, `availWidth`, `availHeight`, `colorDepth`, `pixelDepth`, plus `outerWidth`, `outerHeight` and `devicePixelRatio`, each only when it is a value. `avail_screen` sets the available size; without it it is the screen less 40 pixels in height. The window is as large as the persona's screen, and never smaller than the viewport it really has. Under `native_spoofing`, only `colorDepth` and `pixelDepth`. |
| 50 | `webgl` | Graphics is on | `UNMASKED_VENDOR_WEBGL` and `UNMASKED_RENDERER_WEBGL` from WebGL1 and WebGL2 `getParameter`. A half the `WebGl` value lacks stays the browser's. |
| 51 | `webgpu` | Graphics is on, and the `WebGl` value names a GPU maker | `GPUAdapterInfo.vendor` and `architecture` name the GPU the WebGL renderer names (`nvidia` and `lovelace` for an RTX 40 card, `apple` and `metal-3` for a Mac, `qualcomm` and `adreno-7xx` for a phone), so WebGPU and WebGL agree, which fpscanner and CreepJS check. A Linux persona whose GPU is not Intel's gets no adapter at all, as Chrome on Linux has WebGPU for Intel Gen 12 and later only. The init script does not reach workers. |
| 55 | `canvas_noise` | the policy's `canvas_noise` adds noise | Deterministic noise, a function of the seed and the pixel position, on `toDataURL`, `toBlob` and `getImageData`. `Low` touches one pixel in eight and `Random` draws a new seed for each script. |
| 60 | `audio_noise` | the policy's `audio_noise` adds noise | Deterministic noise on `AudioBuffer.getChannelData` and `copyFromChannel` and on `AnalyserNode.getFloatFrequencyData`, seeded by the fingerprint's seed (a fresh one for `Random`). |
| 65 | `webrtc` | `webrtc` is set, or the policy names a WebRTC IP policy | `Off`: removes `RTCPeerConnection`, `RTCSessionDescription`, `RTCIceCandidate` and the rest of WebRTC. Otherwise drops STUN and TURN servers from `RTCPeerConnection` configs, and rewrites IPs in SDP and `RTCIceCandidate` for the addresses the value holds. |
| 70 | `battery` | Battery is on | `navigator.getBattery()` from the `BatteryProfile`; `Off` removes it. |
| 72 | `connection` | Connection is on | `NetworkInformation` `effectiveType`, `rtt`, `downlink` and `saveData` from the `ConnectionProfile`; `Off` removes `connection`. In `default_registry()` only. |
| 75 | `media_devices` | MediaDevices is on | `enumerateDevices`, from the labelled devices or generated from the input and output counts; `Off` lists none. |
| 76 | `fonts` | `fonts` is a non-empty list | Limits `document.fonts.check`, `load` and `FontFace` to the configured fonts. |
| 78 | `speech` | Speech is on | `speechSynthesis.getVoices()` from the voices; `Off` lists none. |
| 80 | `bluetooth` | Bluetooth is on | A `navigator.bluetooth` stub whose `getAvailability` answers the value, only if the browser has none; `Off` removes `navigator.bluetooth`. |
| 85 | `headless` | the policy's `headless_fixes` is on | `matchMedia` answers (`prefers-reduced-motion: reduce` and `prefers-color-scheme: dark` false, `any-pointer: fine` true) and `document.hidden` / `visibilityState`. |
| 87 | `prepare_stack_trace` | never | Reserved name. It emits no script. |
| 88 | `client_hints` | ClientHints is on, and not `native_spoofing` | `navigator.userAgentData`, including `getHighEntropyValues`, but only in a secure context (`window.isSecureContext`): that is the only place Chrome has the property, so on a plain `http:` page there is none to read and none is added. `localhost` and `127.0.0.1` are secure contexts. `Off` removes `userAgentData`. |
| 89 | `media_codecs` | always | `canPlayType` answers for MP4, AAC and MP3 types when the browser says it cannot play them. WebM and everything else stay the browser's own. |
| 90 | `timezone` | `timezone` is a value, and not `native_spoofing` | `Intl.DateTimeFormat` and the local-time methods of `Date` (`getTimezoneOffset`, `getHours`, `toString`, the `toLocale*String` family and others). `new Date(y, m, d)` and `Date.parse` of a string without an offset keep the machine's zone. |
| 92 | `localization` | Localization is on, and not `native_spoofing` | `navigator.language` and `navigator.languages`: the `languages` the fingerprint holds, else its `accept_languages`, else the list its `locale` implies. |
| 95 | `geolocation` | `geolocation` is a position and not `native_spoofing`, or it is `Off` | `getCurrentPosition` and `watchPosition`. They answer in JavaScript with the position, and refuse with `PERMISSION_DENIED` for `Off`. |
| 110 | `hairline` | the policy's `headless_fixes` is on | Modernizr's hairline check: `offsetHeight` of the element with id `modernizr` reads `1`. |
| 120 | `iframe` | never | Reserved name. It emits no script. |
| 125 | `attach_shadow` | the policy's `open_shadow_roots` is on | Forces `Element.attachShadow` to `mode: 'open'`. |
| 130 | `tracker_block` | the policy blocks trackers | Makes `fetch`, `XMLHttpRequest` and `navigator.sendBeacon` to the tracker hosts (and their subdomains) fail like an unreachable host. Scripts, images and frames loaded by markup are not covered. |

List them at runtime with `default_registry().provider_names()` (all 29, including
the two reserved ones), `session_registry(&fp).provider_names()` (the 27 a
browser session runs), `applicable_names(&fp)` (those that run for a fingerprint),
or over MCP with the `list_evasion_providers` tool, which lists the session ones.

