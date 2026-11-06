---
title: "Design: Mimic, a fifth state for every fingerprint setting"
sidebarTitle: "Mimic state"
---

Status: proposal (2026-11-06), no code yet. It answers the owner's request of 2026-11-06:
"i need each individual fingerprint option to use presets too ... so the fifth option set
would be preset", because "i want it to be able to show a chrome canvas even when using
firefox or any other browser", and then, for every field, "same with the other fields".
The owner suggested the name ("maybe instead of preset it could be called mimic, its up to
u"); the lead chose **Mimic** (see [The name](#the-name)).

Facts below were read from the code at `lane/design-presets` and are cited with
`file:line`. Facts about other projects name their source (Context7 or docs.rs). Anything
marked _to verify_ was not checked and must be checked by the lane before it builds on it.

## The request

Today every setting of a `Fingerprint` is one `Spoof` with four states
([One setting, four states](/stealth/fingerprints#one-setting-four-states)): `Natural`,
`Random`, `Value(v)` and `Off`. The owner wants a fifth state for **every** setting: the
setting shows what a named, real browser or device shows. The main case is the canvas: render
as Chrome 156 on Windows 11 with an Intel UHD 620, or as Chrome on macOS with an Apple GPU,
in any browser, Firefox included, and not as noise. The same holds for every other setting:
a Chrome navigator, Chrome Client Hints, a Chrome WebGL, Chrome audio, fonts, screen, voices,
network information and so on, in Firefox, Safari or Edge, and the other way round where it
makes sense.

## What exists

### The setting and its four states

- `Spoof<T>` is `src/evasion/spoof.rs:27-49`: `Natural` (the default, `:51-56`), `Random`,
  `Value(T)` and `Off`. `is_set()` is "every state but `Natural`" (`:83-87`). It is not
  `#[non_exhaustive]` (`:36-37`), so a fifth variant breaks every exhaustive `match` outside
  the crate.
- In JSON a `Spoof` is one of the words `natural`, `random`, `off` or the value itself, and
  `null` reads as `Natural` (`spoof.rs:184-191`, `:220-254`). For a string setting the three
  words are reserved (`:33-35`). The test-only JSON Schema lists the same three words
  (`:193-218`).
- The settings are the private `Fields` of `Fingerprint` (`src/evasion/fingerprint.rs:515-641`,
  `pub struct Fingerprint(Fields)` at `:746`): `user_agent`, `platform`,
  `hardware_concurrency`, `device_memory`, `max_touch_points`, `vendor`, `oscpu`,
  `product_sub`, `build_id`, `app_version`, `locale`, `languages`, `accept_languages`,
  `timezone`, `screen`, `avail_screen`, `pixel_ratio`, `color_depth`, `webgl`,
  `media_devices`, `geolocation`, `fonts`, `proxy`, `webrtc`, `battery`, `connection`,
  `speech_voices`, `bluetooth` and `client_hints`. Beside them sit the claimed browser
  (`browser_type`, `os_type`, `core_version`, `:516-524`) and the seed.
- **Canvas and audio are not settings.** They are `CanvasNoise` and `AudioNoise` of the
  `StealthPolicy` (`src/evasion/policy.rs:27-45`, `:64-78`), and `is_masked` asks the policy
  for them (`fingerprint.rs:956-957`). The WebGL setting is only the unmasked vendor and
  renderer strings (`src/evasion/settings.rs:117-130`); the `webgl` provider answers
  `getParameter` 37445 and 37446 and nothing else (`src/evasion/providers/builtin.rs:604-642`).
- `build()` refuses `Off` on the 21 settings that have no off state
  (`fingerprint.rs:1913-1941`, error `OffNotAvailable` at `:414-422`), resolves every `Random`
  (`:1886-1897`), then fills `languages` and `platform` that were left out (`:1899-1906`).
  A built fingerprint holds no `Random`.
- The pages that teach the four states: `docs/stealth/fingerprints.md:12-54`,
  `docs/adr/001-stealth-architecture.md:19`, `docs/design/fingerprint-api-v2.md:57`,
  `docs/glossary.md:1812-1816`, `docs/stealth/advanced/providers.md:79`,
  `docs/tutorials/pure_cdp.md:739`, `docs/stealth/profiles.md:36`,
  `docs/tutorials/browser_profiles.md:433`, and the doc comments
  `src/evasion/fingerprint.rs:7-15`, `src/evasion/spoof.rs:1-21` and
  `src/profile_payloads/profile.rs:772`.

### What already holds real values (the sources a mimic can name)

No data should be copied twice. These already exist:

| What | Where | What it holds |
|---|---|---|
| Whole-fingerprint presets | `Fingerprint::windows_desktop` (`fingerprint.rs:1285-1311`), `edge_desktop` (`:1319`), `macos_desktop`, `linux_desktop`, `android_mobile`, `ios_mobile_safari` (`:1414`); `with_machine_extras` (`:1270-1283`) adds battery, network information, voices and Bluetooth | One fixed identity each. `sbrs fingerprint export --preset` names them (`PRESET_NAMES`, `src/cli/fingerprint_cmd.rs:19`, `preset` at `:33-43`). |
| Chrome builds | `KNOWN_FULL_VERSIONS` (`src/evasion/randomize.rs:66-87`; 154 and 156 from Google's releases, 155 measured on a Mac), `PRESET_CHROMIUM_MAJOR` (`:35`), `MOBILE_SAFARI_VERSIONS` (`:106-111`), `chromium_hints` (`:1591-1623`), `edge_user_agent` and `edge_hints` (`:1518-1553`) | User agents and Client Hints the way Chromium builds them. |
| Machines | `TierSpec` `ENTRY`, `MID`, `HIGH` (`:150-210`), `WINDOWS_ADAPTERS` (`:284-301`), `LINUX_ADAPTERS` (`:304-365`), `MAC_MODELS` (`:386-400`), `ANDROID_DEVICES` (`:465-473`), `IPHONE_SCREENS` (`:476-483`), `macos_webgl` and `linux_preset_webgl` (`:1396-1417`) | Cores, memory, screens and GPU strings the way each OS reports them. The rows have no names. |
| Places | `LOCALE_TABLE` (`:591-969`), `Locale` (`:489-502`) | Locale, languages, time zone and the main city's position. |
| System extras | `system_fonts` (`:1073-1157`), `speech_voices_for` (`:1161-1192`), `SpeechVoice::default_set` (`fingerprint.rs:272-296`), `BatteryProfile::default` (`:227-236`), `ConnectionProfile::default` (`:248-257`) | Fonts, voices, a desktop battery, a 4G connection. |
| Phone screens for emulation | `DeviceMetrics` (`src/browser/options/device.rs:40-48`), `MOBILE_USER_AGENT` (`:19`) | A second source of phone screens, used by Chrome's mobile emulation. |
| A real browser read back | `CapturedFingerprint` (`src/evasion/capture.rs:111-124`), the probe `src/evasion/js/capture.js` (a canvas hash at `:247-264`, an audio hash at `:266-292`, the permission answers at `:231`) | What `sbrs fingerprint capture` reads from a running browser ([Fingerprint capture](/design/fingerprint-capture)). |
| Measured wire tables (stealth repository) | `crates/codec/src/measurement.rs`: one file, `chrome-155-macos.json` (`:51-54`); `WireMeasurement` (`:140-166`) holds the user agent and `client_hints: Option<ClientHints>` (core's type) beside TLS and HTTP/2; `Provenance` (`:126-138`) | The only measured browser: Chrome 155 on macOS. "Only what was measured exists" (`:1-11`). |
| Profile payloads | `src/profile_payloads/` | Another tool's JSON, read into a `Fingerprint`; its words map onto the four states (`profile.rs:765-800`, `docs/stealth/profiles.md:42-58`). Values, not named sources. |

### How Random stays coherent

`build()` takes the OS (the given one, else the one the user agent names, else Windows,
`fingerprint.rs:1876-1884`), then one seeded `randomize::draw` decides a machine and a place
(`randomize.rs:1039-1063`, `Profile` and `Draw` at `:1003-1031`), and every `Random` setting is
read from that one draw ([How Random stays coherent](/stealth/fingerprints#how-random-stays-coherent)).
So a random GPU is one the OS reports, a random position lies in the time zone's city, and
random Client Hints name the major the user agent names. `validate()` (`fingerprint.rs:978-1073`)
checks platform against OS, user agent against OS, touch points, locale against languages,
the GPU's graphics API against the OS (`check_graphics`, `:1078-1114`), Mobile Safari against a
`Chrome/` token (`check_browser_persona`, `:1117-1123`) and locale against time zone
(`check_place`, `:1127-1143`).

`BrowserType` (`fingerprint.rs:66-81`) already names the claimed browser: `Chromium`,
`Firefox` ("conceptual; WebDriver mode uses Chromium", `:75`) and `MobileSafari`.

### Who reads and writes a Spoof

- **A file.** `Fingerprint::from_json`, `load` and `save` (`src/evasion/fingerprint/file.rs:42`,
  `:65`, `:119`); `Fields` is `deny_unknown_fields` and leaves `Natural` out
  (`fingerprint.rs:512-514`, `:531`). `diff`, `compare_to` and `claimed_settings` compare two
  fingerprints setting by setting (`fingerprint/compare.rs:157`, `:194`, `:212`).
- **`SB_*` text.** None carries a setting. `SB_FINGERPRINT` names a file, and so do the global
  `--fingerprint FILE` (`crates/cli/src/sbrs.rs:647-651`) and the `[fingerprint]` table, whose
  only key is `file` (`FingerprintSettings`, `src/config/settings.rs:59-65`).
- **The CLI.** `sbrs fingerprint capture`, `export`, `check`, `diff` and `presets`
  (`crates/cli/src/sbrs.rs:2045`, `src/cli/fingerprint_cmd.rs`; `ExportSource` at `:60-72` is a
  preset name or a seeded random identity).
- **The MCP `check_fingerprint` tool** parses a fingerprint or a profile payload and returns the
  coherence report and a summary (`src/mcp/dev/diagnose.rs:894-939`). The summary reads
  `.value()` (`:872-883`), so a setting in any other state shows as `null`.
- **The stealth crates** only build with the states (`.geolocation(Spoof::Off)` and
  `Spoof::Natural` in `crates/scrape/src/egress/geo.rs:383` and `:396`, `.proxy(Spoof::Natural)`
  in `crates/scrape/src/passport/identity.rs:180`, `.user_agent(Spoof::Natural)` in
  `crates/wire/src/client/tests.rs:110`); a literal search finds no `match` on `Spoof` there.
  The core contract `tests/contract_stealth.rs:51-63` imports `Spoof`.

### How a fingerprint reaches each browser

| Launch path | Engine | Page script | Protocol overrides | Launch arguments |
|---|---|---|---|---|
| Pure CDP engine (`sb_cdp`) | Chromium | `session_script`, the providers of `session_registry` (`src/evasion/providers/mod.rs:311`) | CDP | yes |
| WebDriver with `DriverMode::Uc` or `DriverMode::Cdp` | any `is_chromium_based` browser: Chrome, Chromium, Edge, Brave, Opera (`src/browser/config.rs:82-87`) | the same script, through `Page.addScriptToEvaluateOnNewDocument` (`src/browser/session/fingerprint.rs:18-34`, `src/evasion/cdp.rs:77-91`) | `safe_cdp_overrides` | yes |
| WebDriver, default mode | Chromium | none | none | only the launch arguments (`docs/stealth/introduction.md:83-85`) |
| Playwright session | Chromium (`sb_cdp`) | `bootstrap_script`, the providers of `default_registry` (`providers/mod.rs:271`) | its own path (`docs/stealth/introduction.md:86-88`) | its own path |
| WebDriver, Firefox (geckodriver) | Gecko | **none**: CDP and UC mode are refused for a browser that is not Chromium-based (`src/browser/session/launch.rs:24-30`), and no other path injects | none | Firefox preferences take the user agent and language of `BrowserConfig`, not of the fingerprint (`src/browser/session/capabilities.rs:180-210`) |
| WebDriver, Safari | WebKit | **none** (CDP refused, as above) | none | only the page load strategy (`capabilities.rs:64-68`, `:91-101`) |

So a fingerprint reaches only Chromium today. The session script is written for Chromium: it
leaves plugins, MIME types and the network report to Chrome (`SESSION_OMITTED_PROVIDERS`,
`providers/mod.rs:285`), and `window.chrome` stubs are a policy switch (`chrome_runtime`,
`policy.rs:130`). Whether msedgedriver answers thirtyfour's CDP command endpoint is _to verify_.

**The Firefox path that is missing.** WebDriver BiDi is the standard way to inject into Firefox:

- thirtyfour 0.38.1 (the version `Cargo.lock` resolves) has a `bidi` feature (docs.rs feature
  list of 0.38.1), with `src/bidi/modules/script.rs` sending `script.addPreloadScript` and
  `script.removePreloadScript`, and `src/bidi/modules/emulation.rs` sending
  `emulation.setUserAgentOverride`, `setLocaleOverride`, `setTimezoneOverride`,
  `setGeolocationOverride`, `setScreenSettingsOverride`, `setTouchOverride` and others (docs.rs
  source of 0.38.1). `CapabilitiesHelper::enable_bidi` sets `webSocketUrl: true` (Context7,
  `/websites/rs_thirtyfour`). This crate builds thirtyfour with `reqwest` and `cdp` only
  (`Cargo.toml:82`, `crates/cli/Cargo.toml:52`), so the feature is off.
- geckodriver hands out the BiDi WebSocket when the session asks for `webSocketUrl` (Context7,
  `/mozilla/firefox-devtools-mcp`). Puppeteer lists `Page.evaluateOnNewDocument` as fully
  supported over WebDriver BiDi, in Chrome and Firefox, and its BiDi layer registers preload
  scripts with `script.addPreloadScript` (Context7, `/puppeteer/puppeteer`).
- The same Mozilla page warns that Marionette "sets `navigator.webdriver = true` and changes
  other browser fingerprint signals".
- Which `emulation.*` commands Firefox implements, whether preload scripts reach workers, and
  whether safaridriver speaks BiDi at all are _to verify_.

## The name

The fifth state is **`Mimic`**: the setting shows what a named real browser build, machine or
place shows. The name says what the state does. "Preset" would have been a third meaning: it
already names the whole-fingerprint presets (`Fingerprint::windows_desktop()` and the rest,
`sbrs fingerprint export --preset`, `sbrs fingerprint presets`) and the policy presets
(`StealthPolicy::balanced()`, `docs/design/fingerprint-api-v2.md:84`).

"mimic" must then have one meaning. Today it has three:

1. **The old name of the stealth repository's wire crate.** Stealth renamed `seleniumbase-rs-mimic`
   to `seleniumbase-rs-wire` (stealth #6, `docs/design/stealth_names.md:220` and `:288-308` there);
   the facade's features are now `codec`, `scrape`, `wire` and `http3`, default `scrape`
   (stealth `Cargo.toml:10-16`). The leftovers are listed under
   [What the lanes touch](#what-the-lanes-touch); they go first.
2. **A legacy `browser_type` string in profile payloads**, read as Chrome
   (`src/profile_payloads/profile.rs:1268-1275`, test `:1564-1572`,
   `docs/tutorials/browser_profiles.md:63-65` and `:81`). It is another tool's word in its own
   payload format, not a name of ours: it stays an accepted input, and the page says so.
3. The English verb in prose (`DOCS.md:80`, `src/evasion/humanize.rs:187`), which is fine.

## The proposal

### 1. The state and what it holds

```rust
pub enum Spoof<T> {
    Natural,
    Random,
    Value(T),
    /// What the named real build, machine, place or rendering shows.
    Mimic(Specimen),
    Off,
}
```

A `Specimen` is the name of one real thing whose values were recorded or published. It has
four kinds, each named in this project's own words (the randomizer already "draws a machine
and a place", `randomize.rs:1018-1038`; Client Hints "name a real build",
`docs/stealth/fingerprints.md:155-161`):

| Kind | One of | What it holds | Where its rows come from |
|---|---|---|---|
| `Build` | a browser release on one OS family, such as `chrome-156-windows` or `firefox-145-macos` | user agent, `appVersion`, `vendor`, `productSub`, `oscpu` and `buildID` (Firefox), Client Hints, which APIs the browser has (`userAgentData`, `connection`, `deviceMemory`, `getBattery`, `bluetooth`, `window.chrome`), its plugins and MIME types, the permission answers of a fresh profile, the voices it adds | `KNOWN_FULL_VERSIONS`, `chromium_hints`, `edge_hints`, `MOBILE_SAFARI_VERSIONS`; Firefox and Safari builds are new rows |
| `Machine` | a device model, such as `macbook-air-13-m3`, `thinkpad-t480-uhd-620` or `pixel-8` | OS, cores, memory, touch points, screen (size, available size, pixel ratio, colour depth), GPU strings, fonts, media device kinds, battery kind (desktop, laptop, phone) | `MAC_MODELS`, `TierSpec` with the adapter tables, `ANDROID_DEVICES`, `IPHONE_SCREENS`, `DeviceMetrics`; the rows get names |
| `Place` | a locale row with its city, such as `de-berlin` | locale, languages, `Accept-Language`, time zone, position | `LOCALE_TABLE` |
| `Rendering` | one `Build` on one `Machine`, recorded, such as `chrome-156-windows-uhd-620` | what the reference drawings return: canvas, WebGL parameters and pixels, audio, text metrics; plus `Provenance` (date, tool, browser), as stealth's measured tables carry it | new JSON files, captured (see [Where the reference outputs come from](#where-the-reference-outputs-come-from)) |

- **One table per kind, read by both states.** `Random` draws a row; `Mimic` names one. The
  rows the randomizer draws today get a name column, so no value is written twice.
- **Names** are lower case with hyphens, as the stealth crate names its measured files
  (`chrome-155-macos`, `WireMeasurement::file_stem`, `measurement.rs:301`), and unique across
  the four kinds, so a name says its kind. A test checks it.
- **Setting one.** `.screen(Machine::MACBOOK_AIR_13_M3)`, `.user_agent(Build::CHROME_156_WINDOWS)`,
  `.timezone(Place::DE_BERLIN)`, `.canvas(Rendering::CHROME_156_WINDOWS_UHD_620)`, or
  `Spoof::Mimic(..)` written out. The `From` impls are per concrete value type (`Spoof<ScreenSize>`,
  `Spoof<u32>`, `Spoof<String>`, ...): a blanket `impl<T> From<Machine> for Spoof<T>` would
  overlap with `From<T> for Spoof<T>` at `T = Machine`.
- **Setting all.** `Fingerprint::builder().mimic(Rendering::CHROME_156_WINDOWS_UHD_620)` sets
  every setting its build and machine record, and the claimed browser (below); a setting given
  afterwards still wins, and is checked. `Fingerprint::mimic(name)` is the same, built.
- **A file** writes the name, never the values: `"screen": {"mimic": "macbook-air-13-m3"}`.
  The object form cannot be taken for a value of any setting, string settings included (a
  `"mimic:..."` string would be one more reserved spelling for them). A name the crate does not
  know fails the load and lists the known names of that kind.
- **Text.** Where a setting is written as text (a CLI flag), the spelling is
  `mimic:macbook-air-13-m3`. No `SB_*` variable carries a setting today, and none is added:
  `SB_FINGERPRINT` keeps naming a file.
- **A built fingerprint keeps the name**, as it keeps `Off` (and unlike `Random`, which it
  resolves). The values come from the built-in tables, which are embedded and parsed once, the
  way the stealth crate embeds its measured files (`measurement.rs:47-54`). The providers read a
  setting through one crate-private accessor that answers `Value` and `Mimic` alike, so a mimic
  is never half applied; `is_masked` already counts it, since `is_set()` is every state but
  `Natural`.
- **The rendering data stays out of fingerprint files.** A rendering's reference outputs can be
  large; a name keeps the file small.

### 2. One claimed browser

A Chrome canvas next to a Firefox navigator is a contradiction, so the claimed browser is one
choice for the whole fingerprint: the existing `browser_type`, `os_type` and `core_version`
(`fingerprint.rs:516-524`), plus the build's full version. Every `Mimic` follows it.

`build()` refuses, with a `FingerprintError` that names the setting and both sides:

1. a kind the setting cannot take (`timezone` cannot mimic a `Build`), and a `Mimic` on `proxy`
   or `webrtc`, the way `Off` is refused today;
2. two mimics that disagree on the engine, the major version or the OS (`user_agent` from
   `chrome-156-windows`, `client_hints` from `chrome-155-macos`);
3. a `Rendering` whose build or machine is not the build or machine of the other mimics (a
   UHD 620 canvas next to an NVIDIA `webgl`);
4. a canvas or audio mimic with canvas or audio noise on: noise breaks the exact output a
   mimic exists to show.

When `browser_type` was not set, the first `Build` or `Rendering` sets it, as a user agent sets
the OS today. `validate()` reports a `Value` or `Random` that contradicts a mimic (a
`1920x1080` screen next to `Machine::IPHONE_15`) as an error, as it reports a platform that
contradicts the OS. This follows "consistency before coverage": a mimic that does not fit is
refused, not applied in part.

**The claimed engine against the running one.** The fingerprint does not know which browser
will run it; the launch does. A claimed engine that is not the running engine (a Chrome build in
Firefox) is a cross-engine claim. It is refused at launch unless the policy opts in
(`StealthPolicy::cross_engine(true)`), and the coherence report then lists what cannot be
hidden (see [A Chrome canvas in Firefox](#a-chrome-canvas-in-firefox)).

**A mimic can remove an API.** When the claimed build has no such API, the setting's mimic
removes it, as `Off` does, also for settings that have no `Off` of their own: `device_memory`
mimicking a Firefox build in Chrome deletes `navigator.deviceMemory`. A Chrome build in Firefox
adds the Chrome-only APIs instead (below).

### 3. Every setting

How the claim is produced: **changed** (a getter or a protocol override returns another value),
**added** (an API the running engine lacks is defined), **removed** (an API the claimed build
lacks is deleted), **replayed** (recorded outputs are returned). "Cross-engine" is a claimed
engine other than the running one; it needs the Firefox path and the policy opt-in.

| Setting | Mimics | In Chromium | Cross-engine (claimed engine is not the running one) | What can still betray it | First lane |
|---|---|---|---|---|---|
| `user_agent`, `app_version` | `Build`, `Rendering` | changed: `--user-agent`, `Network.setUserAgentOverride`, the getters | changed: the getters and, in Firefox, `general.useragent.override` | the request headers, TLS and HTTP/2 of the running engine; the JavaScript engine | yes |
| `client_hints` | `Build`, `Rendering` | changed: the override's metadata and the `client_hints` provider | added in Firefox and Safari: `navigator.userAgentData` and its interface | the `Sec-CH-UA` request headers, which no page script can add; workers | yes |
| `vendor`, `product_sub`, `platform` | `Build` (`platform` also `Machine`) | changed | changed | the engine tells below | yes |
| `oscpu`, `build_id` | `Build` (Firefox builds) | removed for a Chrome build (Chrome has neither); added for a Firefox build | removed in Firefox for a Chrome build | `in` checks on `Navigator.prototype`, workers | no: with the Firefox builds |
| `hardware_concurrency`, `max_touch_points` | `Machine`, `Rendering` | changed | changed | CSS `pointer` and `hover` media queries against touch points; workers | yes |
| `device_memory` | `Machine`, `Rendering` | changed; removed for a Firefox or Safari build | added in Firefox and Safari (a Chromium-only API) | the interface in workers | yes |
| `screen`, `avail_screen`, `pixel_ratio`, `color_depth` | `Machine`, `Rendering` | changed: `Emulation.setDeviceMetricsOverride`, `window_geometry` | changed: BiDi `emulation.setScreenSettingsOverride` (Firefox support _to verify_) | CSS `device-width` and `resolution` media queries against `screen.*` | yes |
| `webgl` (vendor and renderer strings) | `Machine`, `Rendering` | changed: `getParameter` 37445 and 37446 | changed | every other WebGL value, until `webgl_output` is mimicked too | yes |
| `webgl_output` (new: parameters, extensions, shader precisions, context attributes, `readPixels` of the reference drawings) | `Rendering` | changed and replayed | changed and replayed | a drawing that was not recorded; limits that disagree with the extensions | yes, replay |
| `canvas` (new) | `Rendering` | replayed | replayed | see [Render mimics](#render-mimics) | yes, replay |
| `audio` (new) | `Rendering` | replayed | replayed | an audio graph that was not recorded; `sampleRate`, `baseLatency` | yes, replay |
| `text_metrics` (new: `measureText` and text boxes of the reference strings, emoji) | `Rendering` | replayed | replayed | DOM text boxes of other strings | no: second lane |
| `fonts` | `Machine` (its OS's fonts), `Rendering` | changed: `FontFaceSet.check` and `load`, `FontFace` (`builtin.rs:1107-1190`) | changed | text measured against the fallback when a claimed font is not installed; the provider does not touch measurement | yes, with a warning until `text_metrics` |
| `media_devices` | `Machine`, `Rendering` | changed | changed | device id format per engine (_to verify_) | yes |
| `speech_voices` | `Build` with `Machine` (the OS's voices and the browser's own) | changed | changed | `voiceschanged` timing | yes |
| `battery` | `Machine` (its kind); the API's presence follows the `Build` | changed, or removed | added in Firefox and Safari (`getBattery`, `BatteryManager`) | the interface object, workers | yes |
| `connection` (network information) | `Build` (whether the API exists) | changed; removed for a Firefox or Safari build | added in Firefox and Safari: `navigator.connection` and `NetworkInformation`, an API they do not have | the interface, its `change` events, workers, the network client hints Chrome sends when a site asks for them (_to verify_) | yes |
| `bluetooth` | `Build` (whether the API exists), `Machine` (an adapter) | changed, or removed | added in Firefox and Safari | the `Bluetooth` interface | yes |
| `locale`, `languages`, `accept_languages`, `timezone`, `geolocation` | `Place` (a city of `LOCALE_TABLE`) | changed: the existing overrides | changed: Firefox preferences and BiDi `emulation.*` (_to verify_) | the exit IP's country (the proxy's job) | yes |
| `proxy` | none: it is ours, not a browser's | | | | refused |
| `webrtc` (addresses) | none: addresses belong to the network | | | | refused |

The policy switches stay policy: the WebRTC IP-handling policy, `grant_permissions`, canvas and
audio noise, `native_tostring`, `headless_fixes`. Under a `Build` mimic, what depends on the
engine follows the build, not a switch: the `window.chrome` stubs, plugins and MIME types, the
permission answers and `canPlayType` (the `chrome_runtime`, `plugins`, `permissions` and
`media_codecs` providers, `builtin.rs:247`, `:433`, `:388`, `:1374`). Two values have no setting
and get none: `navigator.webdriver` is always `false` (`webdriver` provider, `builtin.rs:184-211`),
which every real browser says; Do Not Track (read by the capture, `capture.js:72`) is part of
the build's fresh-profile record.

### 4. Natural, Random, Value, Off and Mimic

- `Natural` stays the default: the browser's own value. A mimic never falls back to it silently.
- `Random` draws from the rows that fit the claimed browser and the drawn machine and place.
  For the render settings it draws a recorded `Rendering` of that build and machine, and
  `build()` returns `Incoherent` when none was recorded.
- `Value(v)` is exactly `v`. For the render settings `v` is a recorded set of outputs, such as
  one the user captured on their own machine with `sbrs fingerprint capture --rendering`; that
  is how a rendering the crate does not ship is used.
- `Off` keeps its eight settings.
- One name, one meaning across core and stealth: `Mimic`, `Specimen`, `Build`, `Machine`,
  `Place` and `Rendering` are core's; stealth's measured tables name the `Build` they measured
  (`chrome-155-macos`) instead of a name of their own.

## Render mimics

A canvas mimic means "render as engine X on platform Y", in any browser, and not noise.

### (a) Replay

Record what a real Chrome returns for the drawings of the common open-source fingerprinting
scripts (self-hosted copies of FingerprintJS and CreepJS, and our own probe's drawing,
`capture.js:247-264`), keyed by a hash of the drawing: the canvas size and the sequence of 2D
calls with their arguments and state. On readback, a drawing whose key is recorded gets the
recorded output.

- **Exact** for those drawings. FingerprintJS's canvas source is two images, one geometry-only
  and one with text (Context7, `/fingerprintjs/fingerprintjs`, its "Canvas stabilization"
  section), and its own table counts the canvas as browser-dependent in practice: that is the
  value a mimic must equal.
- **Useless for any other drawing**, which gets the running browser's own pixels. A detector
  that draws one pixel more than the recorded script, or draws a random text each visit, sees
  the real engine. The docs must say so.
- **Every readback path must agree for one drawing**: `toDataURL` (every type and quality),
  `toBlob`, `getImageData`, `drawImage` of the canvas onto another canvas, WebGL `texImage2D`
  of the canvas, `createImageBitmap`, `OffscreenCanvas` (`convertToBlob`,
  `transferToImageBitmap`) and canvases in workers. The canvas noise provider covers the first
  three on the main thread (`builtin.rs:644-740`); no provider names `OffscreenCanvas` or
  `Worker` today (a literal search of `src/evasion` finds none).
- **Cost:** small. A JavaScript hook and a table of a few images per rendering; the work is the
  captures (one real Chrome per OS and GPU) and keeping them current when a script changes its
  drawing.

### (b) Re-render in Skia (CanvasKit)

Record every 2D call; on readback, replay the calls into Skia compiled to WebAssembly
(CanvasKit) and return those pixels. It covers any drawing, in principle.

- **CanvasKit's Canvas2D emulation is incomplete:** "measureText returns width only, textAlign
  and textBaseAlign are not supported, and fillText does not support the width parameter"
  (Context7, `/google/skia`, `modules/canvaskit/npm_build/README.md`). The lane would write its
  own mapping from the 2D API onto CanvasKit's core API.
- **Fonts:** text needs the claimed OS's fonts. Windows and Apple system fonts (Segoe UI, SF,
  Apple Color Emoji) are not ours to ship (_check each licence_); metric-compatible open fonts
  give other glyph pixels, so text would not match a real Windows Chrome.
- **Chrome is not CanvasKit:** Chrome rasterizes accelerated canvases on the GPU and others on
  the CPU, with its own Skia version, font back end and settings; a CanvasKit build answers
  with its own. The result is Skia-like pixels, not the claimed machine's pixels. Against a
  detector that knows real hashes, an unknown hash is not a Windows Chrome either. Whether
  geometry-only drawings match Chrome's CPU raster exactly is _to verify_ by experiment.
- **Cost:** the wasm module's size and its start in every document (_to measure_), a page's
  Content Security Policy without `wasm-unsafe-eval` blocking the module (_to verify_ for
  init scripts and preload scripts), and replaying every call log on every readback.

### (c) What must agree with the canvas

A mimicked canvas betrays itself when another surface shows the running engine or machine:

- `measureText` (width and the bounding boxes) and the size of DOM text (`offsetWidth`,
  `getBoundingClientRect`, `getClientRects`, SVG `getComputedTextLength`), for the same strings
  and fonts;
- emoji, drawn in the canvas and in the DOM (each OS has its own emoji font);
- WebGL: `readPixels`, `getParameter`, `getSupportedExtensions`, `getShaderPrecisionFormat`,
  and the unmasked strings;
- `OfflineAudioContext` output and `AnalyserNode` data, which the audio noise provider changes
  today (`builtin.rs:753-817`), and `AudioContext.sampleRate`;
- the engine tells no setting can hide: properties only one engine has (Chrome's
  `userAgentData`, `connection`, `deviceMemory`; Firefox's `oscpu`, `buildID`), CSS features
  (`-moz-` and `-webkit-` properties, `CSS.supports`), the JavaScript engine's error messages,
  stack format and `Error.captureStackTrace`, `Function.prototype.toString` of native functions,
  values computed in workers, and the network: TLS, HTTP/2 and header order belong to the running
  engine (only a proxy in front can change them, as the stealth crate's wire proxy does for a
  real Chrome, `docs/glossary.md:1198`; whether it can front Firefox is _to verify_).

### A Chrome canvas in Firefox

What it **can** do: with replay and the Firefox path, the canvas value that FingerprintJS-like
and CreepJS-like scripts compute for their own drawings equals the value a real Chrome on the
claimed machine gives.

What it **cannot** do: make Firefox pass as Chrome to a detector that checks the engine. The
JavaScript engine, CSS, workers, the request headers and the TLS handshake still say Firefox,
and a Chrome canvas inside an engine that is plainly Firefox is a stronger signal than Firefox's
own canvas. So the useful case is the same engine on another machine (Chrome on this Mac
claiming Chrome on Windows with a UHD 620), and a cross-engine claim is an explicit opt-in
whose report lists what it cannot hide.

### Recommendation

A staged mix, replay first:

1. **First lane (Chromium):** the `Mimic` state with its four kinds, the claimed-browser
   check, mimics for every value setting from the existing rows, and the new `canvas`,
   `webgl_output` and `audio` settings replayed from recorded renderings on the main thread
   (all readback paths listed above except `OffscreenCanvas` and workers, which the lane marks as
   not covered). An unrecorded drawing keeps the browser's own pixels, and the page says so.
2. **Second lane (Firefox path):** thirtyfour's `bidi` feature, the session script through
   `script.addPreloadScript`, the overrides through BiDi `emulation.*`, providers that know the
   claimed and the running engine, the Firefox builds (`oscpu`, `build_id`), `text_metrics`,
   and `OffscreenCanvas` and workers if preload scripts reach them.
3. **Third lane (cross-engine):** the added and removed APIs per build, the `cross_engine`
   opt-in and its report.
4. **A spike, not a lane:** CanvasKit re-rendering, to measure size, start time and the match
   rate against real Chrome for geometry-only drawings before anything depends on it.

### Where the reference outputs come from

- **Can be built now, on this Mac (Apple M1):** Chrome on macOS with an Apple GPU, and, where
  installed, Firefox and Safari on macOS for the other direction.
- **Can be built now, in a Linux container (OrbStack):** Chrome on Linux, but with software
  GL (no GPU in the container), which looks like a virtual machine: good for tests, not as a
  claim. Whether Google Chrome ships for Linux on arm64 or needs an emulated amd64 container is
  _to verify_.
- **Need captures from another machine:** every Windows rendering (the claim most worth having,
  such as `chrome-156-windows-uhd-620`), other Mac GPUs, Android and iOS devices.
- **The licences of the self-hosted scripts** decide whether a copy may sit in the repository as
  a test fixture (the licences of FingerprintJS's major versions and of CreepJS are _to
  verify_). Recommendation: a pinned copy under the test fixtures only, never shipped.

## What the lanes touch

**Before the state takes the name: "mimic" leftovers** (one docs lane, core and stealth):

| Repository | Item |
|---|---|
| core | `README.md:520`, `CLAUDE.md:52`, `ROADMAP.md:52` and `:386`, `context7.json:31`, `docs/ABI_API.md:49` and `:52`, `docs/api/crate-root.md:19`, `docs/comparison.md:104`, `docs/glossary.md:952`, `:1198`, `:1672` and `:1848`, `docs/stealth/introduction.md:56`, `src/lib.rs:37`, `src/sb_cdp/proxy_auth.rs:18` |
| core | `docs/docs.json:646-660`: the product and group "Stealth API: mimic" and its pages `stealth/api/mimic`, `mimic-client`, `mimic-convert`, `mimic-http-session`, `mimic-mitm`, `mimic-tls-server`, `mimic-stealth-session`, `mimic-capture-h2`, renamed with their files `docs/stealth/api/mimic*.md` |
| core | the scraping and stealth pages that name the crate, its feature or `crates/mimic`: `docs/scraping/stealthy/overview.md`, `first-fetch.md`, `http-session.md`, `browser-scraper.md`, `docs/scraping/advanced-stealthy/stealth-session.md`, `examples.md`, `limits.md`, `identity.md`, `docs/stealth/advanced/mitm-proxy.md`, `wire-formats.md`, `docs/stealth/api/codec-ids.md`, `scrape.md`, `scrape-runtime.md` |
| core | the "Moved" banners of `docs/adr/003-stealth-scraping-layout.md:6` and `docs/design/stealth_parity_plan.md:6`, which call `mimic` the current name and `seleniumbase-rs-wire` the first name of what is now `codec` |
| core | **`tests/offline/docs_current.rs:30-36`**: the guard still forbids `seleniumbase_rs_wire`, `seleniumbase-rs-wire` and `crates/wire` (the 0.3.0 name of `codec`) and points to the `mimic` names. It must flip (forbid the `mimic` crate names, allow `wire`) before any page can name `seleniumbase-rs-wire`. |
| core, kept | `CHANGELOG.md` and `docs/changelog.md` (history); the profile payload alias `mimic` and its two lines in `docs/tutorials/browser_profiles.md` (another tool's input word, now said to be one); the English verb |
| stealth | `docs/contributing/boringssl.md:3` and `:53`, `scripts/boringssl-prebuilt.sh:7` and `:42`: they pass `--features seleniumbase-rs-stealth/mimic`, a feature that no longer exists (now `wire`) |
| stealth | `docs/design/http3.md:44`, `:309`, `:327` and `:331-332` (`crates/mimic`, written before the rename: reword, or mark as written before it); `docs/design/stealth_names.md` is the rename decision itself and stays |

**The state lane (first lane):**

| Area | Items |
|---|---|
| Code | `src/evasion/spoof.rs` (the variant, `as_ref`, `map`, `word`, serde, schema); a new `src/evasion/mimic.rs` (`Specimen`, `Build`, `Machine`, `Place`, `Rendering`, the lookup) and `src/evasion/mimic/renderings/*.json`; `src/evasion/randomize.rs` (named rows, read by `Random` and `Mimic`); `src/evasion/fingerprint.rs` (`Fields` gains `canvas`, `webgl_output` and `audio`; the builder setters and `mimic`; `build()` checks; `refuse_useless_off`'s twin for mimics; `is_masked` for `Canvas` and `Audio`; `validate`; the errors); `src/evasion/fingerprint/compare.rs` and `schema.rs`; `src/evasion/providers/builtin.rs` (the providers read the accessor that answers `Value` and `Mimic`; a new replay provider for canvas, WebGL output and audio) and `providers/mod.rs` (its place in both registries) |
| CLI | `src/cli/fingerprint_cmd.rs` (`export --mimic NAME`; `presets` also lists the mimic names by kind), `crates/cli/src/sbrs.rs`; then `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` (`SB_UPDATE_COMPLETIONS=1`) |
| MCP | `src/mcp/dev/diagnose.rs:872-883` (the summary names a mimic instead of `null`); the `sb` server's fingerprint tools if they print settings; `src/mcp/dev/api_index.json` (`SB_UPDATE_API_INDEX=1`) |
| Stealth contract | `tests/contract_stealth.rs` (imports `Spoof`; check with `mise run contracts`); stealth's measured tables name their `Build` |
| Docs | `docs/stealth/fingerprints.md` ("One setting, five states", the settings table, a "Mimic" section), `docs/api/stealth-settings.md`, `docs/adr/001-stealth-architecture.md:19`, `docs/design/fingerprint-api-v2.md:57` (a note that it grew), `docs/glossary.md` (`Spoof`, and new entries `Mimic`, `Specimen`, `Rendering`), `docs/stealth/advanced/providers.md:79`, `docs/tutorials/pure_cdp.md:739`, `docs/stealth/profiles.md:36`, `docs/tutorials/browser_profiles.md:433`, `docs/stealth/introduction.md` (what reaches which browser), CHANGELOG under Breaking (`Spoof` gains a variant) |
| Generated | `tools/docs_layout_tree.py` (the new module), `mise run layout-tree` |

**The Firefox lane (second lane)** adds thirtyfour's `bidi` feature in both manifests (then
`cargo deny check`), `webSocketUrl` in `apply_firefox_capabilities` (`capabilities.rs:107-126`),
a BiDi branch in `BrowserSession::apply_fingerprint` (`session/fingerprint.rs:18-34`), lifts the
refusal in `validate_mode_support` (`launch.rs:24-30`) for the fingerprint path only, and an
`#[ignore]`d test in the `firefox_geckodriver` binary.

## Tests

- `Spoof` round trips: the JSON object form for every kind and every value type, the text form
  `mimic:NAME`, `null` still `Natural`, an unknown name refused with the known names listed.
- The catalogue: names unique across kinds; every `Machine` row with its OS's `Build` passes
  `validate()` with no warning; `Random` and `Mimic` read the same rows (a seed's drawn machine
  is a named row).
- Refusals, each written first and failing before the change: a kind the setting cannot take;
  `proxy` and `webrtc` as mimics; two builds that disagree on engine, major or OS; a rendering
  on another machine; canvas noise with a canvas mimic; `Random` canvas with no recording for
  the drawn machine.
- `compare_to` against a capture: a mimicked setting is expected to equal the catalogue's value.
- The replay provider, without a browser: the script holds the recorded outputs and the keys,
  and an unknown key falls through to the browser's own method.
- `#[ignore]`d headless Chrome tests in `tests/chrome`, on local fixture pages with pinned copies
  of the reference scripts: a canvas mimic reaches the page, `toDataURL`, `toBlob` and
  `getImageData` agree with each other and with the recorded rendering, and a drawing that was
  not recorded gets the browser's own pixels. They run only when the owner says the Mac is free.
- After the Firefox lane, the same fixture in Firefox headless, through geckodriver.
- `tests/offline/docs_current.rs` with the flipped rows, and `mise run contracts`.

## Open questions

1. **What the state holds.** Recommendation: `Mimic(Specimen)`, with the four kinds `Build`,
   `Machine`, `Place` and `Rendering`. Alternatives: `RealBrowser` (does not fit a place or a
   machine alone), `Original` (reads as the browser's own value, which is `Natural`), one
   unnamed id string (loses the kind in the type).
2. **Keep the name in the built fingerprint, or resolve it to `Value` as `Random` is
   resolved?** Recommendation: keep it. Files stay small, a rendering's data never lands in
   them, and the file says what it claims. The cost is that a crate upgrade can change the
   values behind a name, as it can change what a seed draws today
   ([Randomised identities](/stealth/fingerprints#randomised-identities)); a file that must
   outlive an upgrade can still be exported with values.
3. **Canvas, WebGL output and audio become settings.** Recommendation: yes; the noise stays
   policy, and a mimic with noise on is refused. Alternative: a policy `CanvasNoise::Mimic`,
   which would mix a claim into the policy that "never changes a setting".
4. **Cross-engine claims.** Recommendation: refused at launch unless the policy opts in, and
   reported. Alternative: allowed, with warnings only.
5. **First lane Chromium, Firefox second.** Recommendation: yes, because nothing reaches
   Firefox today and the canvas data is the same for both. If Firefox matters most to the owner,
   the Firefox lane can go first, with the existing script and no mimic.
6. **Re-rendering with CanvasKit.** Recommendation: a measured spike after the first lane, no
   commitment before its numbers.
7. **Renderings the crate does not ship.** Recommendation: `Value` of a captured rendering
   (`sbrs fingerprint capture --rendering`); no registry of user names in the first lane.
8. **A place as a mimic or as a shortcut.** Recommendation: `Place` as a kind, since `Random`
   draws the same rows; `.mimic(..)` of a rendering does not set the place (the capture
   machine's place means nothing).
9. **The leftover "mimic" names.** Recommendation: one docs lane in both repositories before the
   state lane, with the `docs_current` guard flipped in the same commit as the pages.
10. **A word for mimics in profile payloads.** Recommendation: none; the payload format is
    another tool's, and its legacy `browser_type` alias stays an input.
11. **An `SB_MIMIC` variable or a global `--mimic NAME`.** Recommendation: not in the first lane;
    `export --mimic NAME` writes a file, and `SB_FINGERPRINT` names it.
