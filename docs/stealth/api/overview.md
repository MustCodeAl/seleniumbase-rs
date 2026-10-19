---
title: "Stealth API reference"
sidebarTitle: "Overview"
description: "Reference for the stealth API: the seleniumbase_rs::stealth module of this crate (fingerprints, policy, evasions, human input, binary patching, UC mode) and the three crates of the seleniumbase-rs-stealth repository (codec, scrape, mimic)."
---

The stealth API makes an automated browser look like a person's browser. It has
two parts. This crate's `seleniumbase_rs::stealth` module covers everything a
page can read from inside the browser: the identity a browser claims (a
`Fingerprint`), how the claim is delivered (a `StealthPolicy`), the scripts and
launch arguments that deliver it, human-paced input, and patched driver and
browser binaries. The separate
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
repository covers the wire: TLS and HTTP/2 fingerprints, an impersonating HTTP
client and a scraper. These pages document each public item as the code defines
it; the guides they link explain when to use what.

<RequestExample>

```rust One identity, WebDriver engine
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode, Fingerprint, OsType};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let fp = Fingerprint::randomized(OsType::Windows, 42);
let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_fingerprint(fp);
let mut sb = BaseCase::new(config).await?;
sb.open("https://example.com").await?;
# Ok(())
# }
```

```rust The same identity, pure CDP engine
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{Fingerprint, OsType};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let fp = Fingerprint::randomized(OsType::Windows, 42);
let browser = Browser::launch(LaunchOptions::builder().fingerprint(&fp).build()?).await?;
let page = browser.default_page().await?;
page.goto("https://example.com").await?;
# Ok(())
# }
```

```rust Check an identity before launch
use seleniumbase_rs::stealth::evasions::{launch_args, session_script};
use seleniumbase_rs::{Dimension, Fingerprint};

let fp = Fingerprint::builder().timezone("Europe/Berlin").build()?;
assert!(fp.validate().is_coherent());
assert!(fp.is_masked(Dimension::Timezone));
let script = session_script(&fp); // what every page runs first
let args = launch_args(&fp); // what Chrome is started with
# Ok::<(), seleniumbase_rs::FingerprintError>(())
```

</RequestExample>

## Two parts

| Part | Where | What it covers | State |
|---|---|---|---|
| `seleniumbase_rs::stealth` | This crate, `src/stealth/`. Always compiled: no feature flag. | The browser's identity and policy, page scripts, launch arguments, DevTools overrides, human input, chromedriver and Chrome patching, UC mode. | Part of the library. |
| `seleniumbase_rs_codec` | seleniumbase-rs-stealth, crate `seleniumbase-rs-codec` | Wire formats (`ClientHello`, JA3, JA4, peetprint, Akamai HTTP/2) and the measured browser tables. | <Badge color="orange">Experimental</Badge> |
| `seleniumbase_rs_scrape` | seleniumbase-rs-stealth, crate `seleniumbase-rs-scrape` | `BrowserScraper` (a real Chrome, no TLS spoofing), the stealth identity, challenge detection, cookie jar, proxy pool. | <Badge color="orange">Experimental</Badge> |
| `seleniumbase_rs_mimic` | seleniumbase-rs-stealth, crate `seleniumbase-rs-mimic` | The impersonating client (wreq with BoringSSL), `HttpSession`, `MitmProxy`, `StealthSession`. Builds BoringSSL, so it needs cmake and a C/C++ toolchain. | <Badge color="orange">Experimental</Badge> |

The three crates moved out of this one in 0.3.0 (they were the features
`scrape` and `impersonate`). They depend on this crate by git and use only its
public API, such as `stealth::randomize` (the table of places in
[`LOCALES`](/stealth/api/core-randomize#locales) is read by the scrape crate's geo
code) and `sb_cdp`. Their pages are [codec](/stealth/api/codec),
[scrape](/stealth/api/scrape) and [mimic](/stealth/api/mimic); the
[Stealth Scraping](/stealth/overview) overview explains how they fit together.

## Module map

<Badge color="gray">mod</Badge> `seleniumbase_rs::stealth`

```rust
pub mod behavior;
pub mod cdp;
pub mod dprocess;
pub mod evasions;
pub mod fingerprint;
pub mod humanize;
pub mod js;
pub mod options;
pub mod patcher;
pub mod policy;
pub mod providers;
pub mod randomize;
pub mod reactor;
pub mod settings;
pub mod spoof;
pub mod uc;
```

| Module | Holds | Page |
|---|---|---|
| `fingerprint` | `Fingerprint`, the presets, `Dimension`, `CoherenceReport`, `OsType`, `BrowserType`, the profile types | [Fingerprint](/stealth/api/core-fingerprint), [Settings](/stealth/api/core-settings) |
| `fingerprint` | `FingerprintBuilder`, `FingerprintError` | [FingerprintBuilder](/stealth/api/core-fingerprint-builder) |
| `spoof` | `Spoof`, the four states of one setting | [Spoof](/stealth/api/core-spoof) |
| `settings` | `ScreenSize`, `Geolocation`, `WebGl`, `WebRtcAddresses`, `MediaDevices` | [Settings](/stealth/api/core-settings) |
| `policy` | `StealthPolicy`, `CanvasNoise`, `AudioNoise` (and, from `fingerprint`, `QuicMode`, `WebRtcPolicy`) | [StealthPolicy](/stealth/api/core-policy) |
| `randomize` | The Chromium versions, `Locale`, `LOCALES`, the Client Hints builders | [Randomize](/stealth/api/core-randomize) |
| `providers`, `providers::builtin` | `EvasionProvider`, `EvasionRegistry`, `EvasionContext`, the 29 built-in providers | [Providers](/stealth/api/core-providers) |
| `evasions` | The page script, the launch arguments and the DevTools overrides of a fingerprint | [Evasions and launch](/stealth/api/core-evasions) |
| `behavior`, `humanize` | `Behavior`, `Humanizer`, the input plans, `Rng`, the older helpers | [Human input](/stealth/api/core-humanize) |
| `patcher`, `dprocess` | `EnginePatch`, `ChromedriverPatcher`, `ChromeBinaryPatcher`, process helpers | [Binary patching](/stealth/api/core-patcher) |
| `options`, `uc` | `StealthOptions`, the undetected-Chrome arguments and script | [UC mode and options](/stealth/api/core-uc-and-options) |
| `cdp`, `reactor`, `js` | `CdpClient`, `CdpReactor`, the page scripts of the recorder | [DevTools helpers](/stealth/api/core-cdp) |

## Re-exports

`seleniumbase_rs::stealth` re-exports, from its modules: `Fingerprint`,
`FingerprintError`, `StealthPolicy`, `Spoof`, `BatteryProfile`, `BrandVersion`,
`ClientHints`, `CoherenceReport`, `ConnectionProfile`, `HumanizeConfig`,
`SpeechVoice`, the deprecated `StealthFlags`; `engine_spoofing_args`,
`find_system_chrome`, `ChromeBinaryPatcher`, `ChromedriverPatcher`,
`EnginePatch`; `default_registry`, `session_registry`, `EvasionConfig`,
`EvasionContext`, `EvasionProvider`, `EvasionRegistry` and
`SESSION_OMITTED_PROVIDERS`.

The crate root re-exports every one of those, plus `AudioNoise`, `BrowserType`,
`CanvasNoise`, `Dimension`, `FingerprintBuilder`, `Geolocation`,
`MediaDevices`, `OsType`, `ProxyConfig`, `QuicMode`, `ScreenSize`, `WebGl`,
`WebRtcAddresses` and `WebRtcPolicy`, so `use seleniumbase_rs::{Fingerprint,
OsType, Spoof}` works. The [`fingerprint!`](/stealth/api/core-fingerprint#preset-macro)
macro is exported at the crate root too. See also [Crate root](/api/crate-root).

## Pages

<Columns cols={2}>

<Card color="#1a85d4" title="Fingerprint" icon="fingerprint" href="/stealth/api/core-fingerprint">

The identity a browser claims: accessors, presets, `randomized`, `is_masked`, `validate`, `Dimension`.

</Card>

<Card color="#199f43" title="FingerprintBuilder" icon="hammer" href="/stealth/api/core-fingerprint-builder">

Every builder method, how `build` draws `Random` settings, and `FingerprintError`.

</Card>

<Card color="#d47628" title="Spoof" icon="toggle-on" href="/stealth/api/core-spoof">

`Natural`, `Random`, `Value(T)` and `Off`: the four states of every setting.

</Card>

<Card color="#a631be" title="Settings" icon="sliders" href="/stealth/api/core-settings">

The value types: screen, position, WebGL, WebRTC, media devices, proxy, battery, Client Hints, personas.

</Card>

<Card color="#1ca1c7" title="StealthPolicy" icon="shield-halved" href="/stealth/api/core-policy">

How the claims are delivered: noise, native spoofing, stubs, blocking, QUIC, WebRTC.

</Card>

<Card color="#d32a61" title="Randomize" icon="dice" href="/stealth/api/core-randomize">

The Chromium versions, the table of places and the Client Hints builders.

</Card>

<Card color="#693acf" title="Providers" icon="puzzle-piece" href="/stealth/api/core-providers">

`EvasionProvider`, `EvasionRegistry`, `EvasionContext` and the 29 built-in providers.

</Card>

<Card color="#d5a910" title="Evasions and launch" icon="rocket" href="/stealth/api/core-evasions">

The page script, launch arguments and DevTools overrides, and how `BrowserConfig` and `LaunchOptionsBuilder` take a fingerprint.

</Card>

<Card color="#17a5af" title="Human input" icon="computer-mouse" href="/stealth/api/core-humanize">

`Behavior`, `Humanizer` and their plans, `HumanizeConfig`, `Rng` and the older helpers.

</Card>

<Card color="#d52c36" title="Binary patching" icon="wrench" href="/stealth/api/core-patcher">

`EnginePatch`, `ChromedriverPatcher`, `ChromeBinaryPatcher`, `engine_spoofing_args` and the process helpers.

</Card>

<Card color="#1a85d4" title="UC mode and options" icon="user-secret" href="/stealth/api/core-uc-and-options">

`StealthOptions`, the undetected-Chrome arguments and the UC stealth script.

</Card>

<Card color="#199f43" title="DevTools helpers" icon="terminal" href="/stealth/api/core-cdp">

`CdpClient`, the detectable domains, `CdpReactor` and the page scripts in `stealth::js`.

</Card>

<Card color="#d47628" title="codec" icon="file-code" href="/stealth/api/codec">

`seleniumbase-rs-codec`: wire formats and measured tables.

</Card>

<Card color="#a631be" title="scrape" icon="spider" href="/stealth/api/scrape">

`seleniumbase-rs-scrape`: `BrowserScraper`, identity and challenge detection.

</Card>

<Card color="#1ca1c7" title="mimic" icon="masks-theater" href="/stealth/api/mimic">

`seleniumbase-rs-mimic`: the impersonating client, `HttpSession`, `MitmProxy`, `StealthSession`.

</Card>

</Columns>

## Guides

- [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth): building,
  checking and applying a fingerprint, the policy, every provider.
- [UC Mode](/tutorials/uc_mode) and the [UC mode help page](/help/uc_mode):
  undetected Chrome with the WebDriver engine.
- [Binary Patching](/tutorials/binary_patching): chromedriver and Chrome
  patching.
- [Headless](/tutorials/headless): what headless changes and what the headless
  fixes do.
- [Fingerprint API v2](/design/fingerprint-api-v2): why a setting is one
  `Spoof` and the policy is separate.

<Visibility for="agents">

Summary for AI agents. Core stealth API: `seleniumbase_rs::stealth` (always compiled). Build an identity with `Fingerprint::builder()...build()?` (returns `Result<Fingerprint, FingerprintError>`), a preset (`Fingerprint::windows_desktop()`, `edge_desktop`, `macos_desktop`, `linux_desktop`, `android_mobile`, `ios_mobile_safari`) or `Fingerprint::randomized(OsType, seed)`. Each setting is a `Spoof<T>` (`Natural`, `Random`, `Value(T)`, `Off`); delivery is a `StealthPolicy` (`default`, `balanced`, `native_only`, `max`). Apply with `BrowserConfig::with_fingerprint(fp)` (WebDriver; script and overrides in `Cdp` and `Uc` modes) or `LaunchOptions::builder().fingerprint(&fp)` (pure CDP). Scripts come from `evasions::session_script(&fp)` (registry `session_registry`, 27 providers) or `bootstrap_script` (`default_registry`, 29). Wire-level stealth is in the separate crates `seleniumbase-rs-codec`, `-scrape` and `-mimic` of https://github.com/MustCodeAl/seleniumbase-rs-stealth.

</Visibility>
