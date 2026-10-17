---
title: "Fingerprint & Stealth Profiles"
---

# Fingerprint & Stealth Profiles

Websites detect automation by looking at many browser signals: `navigator.webdriver`,
user agent, platform, screen size, hardware concurrency, WebGL vendor, canvas
noise, audio noise, time zone, geolocation, media devices, fonts, and more.
`seleniumbase-rs` can spoof these signals through a coherent `Fingerprint` and a
pluggable evasion system.

This page explains how to use built-in fingerprint presets, build custom
profiles, choose masking modes, and apply native-level spoofing through CDP.
It covers what a page can read inside the browser. The TLS and HTTP/2 side, and
the scraper built on it, is a separate project now; see
[Stealth Scraping](stealth_scraping.md).

## What you will learn

- How to use built-in fingerprint presets.
- How to generate a random but coherent `Fingerprint` from a seed.
- How to build a custom `Fingerprint`.
- How a value you give turns its own dimension on, and how the
  `StealthFlags` masking modes work.
- How to enable native-level CDP spoofing.
- Which launch paths apply a fingerprint, and what each one does.
- How the evasion provider registry assembles the JavaScript bootstrap.

## Quick start

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode, Fingerprint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fp = Fingerprint::windows_desktop();
    let config = BrowserConfig {
        mode: DriverMode::Uc,
        fingerprint: Some(fp),
        ..BrowserConfig::default()
    };

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://example.com").await?;
    sb.quit().await?;
    Ok(())
}
```

A fingerprint reaches the page only through a launch path that applies it:

- With `DriverMode::Uc` (above) or `DriverMode::Cdp`, the session registers the
  evasion script and sends the protocol overrides described under
  [Native-level (CDP) spoofing](#native-level-cdp-spoofing), and the launch
  arguments apply. The script is the one the Pure CDP engine runs,
  `evasions::session_script` (the 27 providers of `session_registry`); see
  [Built-in providers](#built-in-providers).
- With the default `DriverMode::WebDriver`, only the launch arguments apply (user
  agent, `--lang`, `--window-size`, the WebRTC policy, and the QUIC and proxy
  flags). No script is injected and no protocol override is sent.
- The Pure CDP engine and the Playwright session have their own paths, described
  below. The Pure CDP engine runs the same `session_script`. The Playwright
  session runs `bootstrap_script`, the 29 providers of `default_registry`.

## Fingerprint presets

Built-in presets set a coherent combination of user agent, platform, screen,
WebGL strings, a time zone with the position of its city, and masking flags
(`StealthFlags::balanced()`):

| Preset | What it claims |
|---|---|
| `Fingerprint::windows_desktop()` | Chrome 156 on Windows 10, 1920x1080, 8 cores, 8 GB, `en-US`, `America/New_York` (and the position of New York), an NVIDIA RTX 4070 Ti through ANGLE and Direct3D 11. |
| `Fingerprint::edge_desktop()` | The Windows preset as Microsoft Edge 156. The user agent ends with `Edg/156.0.0.0`, and the Client Hints name `Microsoft Edge` where Chrome names itself `Google Chrome` (`randomize::edge_hints`; Edge's full versions are not in the Chrome table, so they are the frozen `156.0.0.0`). |
| `Fingerprint::macos_desktop()` | Chrome 156 on macOS (`Macintosh; Intel Mac OS X 10_15_7`), 1920x1080, `America/Los_Angeles`, and the WebGL strings Chrome reports on a Mac: `Google Inc. (Apple)` and `ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)`. The user agent is Chrome's, not Safari's. |
| `Fingerprint::linux_desktop()` | Chrome 156 on Linux x86_64, 1920x1080, `America/Chicago`, an NVIDIA RTX 4070 through ANGLE over OpenGL (`Google Inc. (NVIDIA Corporation)`), worded as a generated Linux identity words it. |
| `Fingerprint::android_mobile()` | Chrome 156 on Android, in Chrome's frozen user agent (`Linux; Android 10; K`), 412x915 at a device pixel ratio of 3, 5 touch points, Adreno 740. |
| `Fingerprint::ios_mobile_safari()` | Mobile Safari 26.0 on an iPhone (`iPhone OS 18_6`, the iOS version Safari 26 freezes in its user agent), 390x844 at a device pixel ratio of 3, 5 touch points, `Apple GPU`, Safari-style Client Hints. |

Every preset also carries the position of the city of its time zone
(`randomize::locale_of_timezone`), so `validate()` has nothing to warn about
and the geolocation request is answered (see
[Native-level (CDP) spoofing](#native-level-cdp-spoofing)). All six presets
validate without errors or warnings; `tests/mcp_webdriver.rs` and the unit tests
of `stealth::fingerprint` check it.

Use the `fingerprint!` macro for shorter syntax. `safari` is an alias for `ios`,
and `edge` is the Edge preset:

```rust
use seleniumbase_rs::fingerprint;

let fp = fingerprint!(windows);
let fp = fingerprint!(edge);
let fp = fingerprint!(macos);
let fp = fingerprint!(linux);
let fp = fingerprint!(android);
let fp = fingerprint!(ios);
let fp = fingerprint!(safari); // same as ios
```

## Randomised identities

A preset is one identity; a fleet of sessions that all share it is easy to
spot. `Fingerprint::randomized(os, seed)` generates a different, believable
identity for each seed instead:

```rust
use seleniumbase_rs::{Fingerprint, OsType};

let fp = Fingerprint::randomized(OsType::Windows, 42);

// The same seed always gives the same identity.
assert_eq!(fp, Fingerprint::randomized(OsType::Windows, 42));

// It passes the coherence check, with no warnings either.
let report = fp.validate();
assert!(report.is_coherent() && report.warnings.is_empty());

// Another seed is another identity.
let other = Fingerprint::randomized(OsType::Windows, 43);
assert_ne!(fp, other);
```

Detectors check that signals agree with each other, so the generator never
randomises a field alone. It draws a whole machine and a whole place, then
derives every field from them:

| Dimension | What is generated |
|---|---|
| WebGL | Vendor and renderer the OS really reports: ANGLE over Direct3D 11 (Windows), ANGLE over Metal with an Apple chip (macOS), ANGLE over Mesa or the NVIDIA driver (Linux), an Adreno or Mali chip (Android), `Apple GPU` (iOS). |
| Hardware | `hardwareConcurrency`, `deviceMemory`, screen size, `colorDepth` and device pixel ratio that machine ships with: a Mac gets its chip's core count and its model's screen, a PC's class decides its cores, memory and screens, and a phone gets its own screen. |
| Place | A locale and language list, a time zone it is spoken in, and coordinates in that zone's main city, so `locale`, `timezone` and geolocation agree. |
| Software | User agent, `platform`, `vendor` and Client Hints (built the way Chromium builds them) that agree with the OS and with the claimed Chromium version. |
| Noise | A canvas `seed` and an `audio_seed` derived from it, different for every seed, so both noises run. |

All five `OsType` values are supported. The masking flags are
`StealthFlags::balanced()`, and every generated value turns its own dimension on,
so the generated values are applied as they are.

Things to know:

- **Stable within a release.** A seed maps to the same identity for one version
  of the crate. Adding a table row changes what a seed maps to, so store the
  generated `Fingerprint` (it is `Serialize`), not the seed, if an identity has
  to outlive an upgrade.
- **The Chromium version is a fixed window.** At this version of the crate the
  user agent claims Chromium 154, 155 or 156 (an iPhone identity claims Mobile
  Safari 18.1, 18.3.1, 18.5 or 26.0; Safari 26 names the frozen `iPhone OS 18_6`
  in its user agent). Pair the identity with a browser of that era, or change
  `user_agent`, `client_hints` and `core_version` together.
  `Fingerprint::randomized_with_major(os, seed, major)` fixes the Chromium major
  instead (Safari and iOS ignore it); the stealth scraper of seleniumbase-rs-stealth
  uses it so the identity claims the Chrome that is running (see
  [Stealth Scraping](stealth_scraping.md)).
- **iOS differs.** Safari has no `navigator.deviceMemory` and no
  `navigator.userAgentData`, so an iOS identity sets neither (`device_memory` and
  `client_hints` are `None`).
- **Client Hints carry a real full version.** The user agent keeps Chrome's
  frozen `major.0.0.0`, but `uaFullVersion` and `fullVersionList` name a real
  build from `randomize::KNOWN_FULL_VERSIONS`: 154.0.8037.100 and
  156.0.8078.35 from Google's published stable releases, and 155.0.8059.40 as
  Chrome 155 reported it on a Mac (`tests/client_hints_chrome.rs`). A major the
  table does not list (one forced with `randomized_with_major`) falls back to
  `major.0.0.0`.
- **Platform versions.** A Mac claims macOS 15.8.1, 26.7.1, 27.0.0 or 27.0.1, in
  the `major.minor.patch` form Chrome reports (`27.0.1` was read from Chrome 155
  on macOS 27.0.1). The Windows list (`10.0.0`, `13.0.0`, `15.0.0`, `19.0.0`)
  follows Microsoft's description of the value and was not read from a Windows
  Chrome.
- **Not set:** fonts, media devices and the proxy. Set them on the result if you
  need them.
- **Applying it.** For the Pure CDP engine, hand it to the launch options with
  `LaunchOptions::builder().fingerprint(&fp)` (see
  [Pure CDP](pure_cdp.md#looking-like-another-browser)); `page.apply_fingerprint(&fp)`
  does the same for one tab. For `BaseCase`, pass it to
  `BrowserConfig::with_fingerprint` and use `DriverMode::Uc` or `DriverMode::Cdp`.
  You can also feed it to the evasion registry yourself.

## Building a custom fingerprint

For full control, use the builder:

```rust
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ...")
    .platform("Win32")
    .screen(1920, 1080)
    .hardware_concurrency(8)
    .device_memory(8.0)
    .locale("en-US")
    .timezone("America/New_York")
    .webgl("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 ...)")
    .geolocation(40.7128, -74.0060)
    .media_device_counts(1, 1, 2) // audio inputs, audio outputs, video inputs
    .build();
```

There is no flag to flip afterwards. Giving a value is the intent to use it, so
each value setter turns on the masking dimension the value belongs to, and the
fingerprint above masks its navigator, language, screen, WebGL, time zone,
position and media devices.

### A value is the switch

Every `StealthFlags` mode defaults to `Auto`: the dimension is on exactly when
the fingerprint holds a value for it. `Fingerprint::is_masked(Dimension)` is the
one question the page-script providers, the launch arguments, the protocol
overrides and the proxy all ask, so a value is never half applied (a user agent
sent to the browser but not to the page, say).

| Setters | Turn on | What then acts |
|---|---|---|
| `user_agent`, `platform`, `hardware_concurrency`, `device_memory`, `max_touch_points`, `vendor`, `os_cpu`, `product_sub`, `build_id`, `app_version` | `Dimension::Navigator` | The `navigator_props` and `plugins` providers, `--user-agent`, `Network.setUserAgentOverride`. |
| `locale`, `languages`, `accept_languages` | `Localization` | The `localization` provider, `--lang`, `Emulation.setLocaleOverride`, and the `Accept-Language` of the user agent override. |
| `timezone` | `Timezone` | The `timezone` provider and `Emulation.setTimezoneOverride`. |
| `screen`, `avail_screen`, `pixel_ratio`, `color_depth` | `Screen` | The `window_geometry` provider, `--window-size` and `Emulation.setDeviceMetricsOverride`. |
| `webgl` | `Graphics` | The `webgl` provider. |
| `fonts` (when not empty) | `Fonts` | The `fonts` provider. |
| `media_device_counts`, `media_device_specs` | `MediaDevices` | The `media_devices` provider. |
| `geolocation`, `altitude`, `accuracy` | `Geolocation` | The `geolocation` provider, `Emulation.setGeolocationOverride`, and the geolocation permission (the popup is answered with `Allow`). |
| `webrtc_ips`, `webrtc_policy` | `WebRtc` | The `webrtc` provider and `--force-webrtc-ip-handling-policy`. |
| `canvas_seed`, `seed` | `Canvas` | The `canvas_noise` provider. |
| `audio_seed`, `seed` | `Audio` | The `audio_noise` provider. |
| `battery`, `connection`, `speech_voices` (when not empty), `client_hints` | `Battery`, `Connection`, `Speech`, `ClientHints` | The provider of the same name. |
| `proxy` | `Proxy` | `--proxy-server`, when the proxy has a host. |
| `blocked_ports` | `Ports` | The ports are refused on `localhost`, `127.0.0.1` and `[::1]` with `Network.setBlockedURLs`. |
| `blocked_trackers` (when not empty) | `Trackers` | The `tracker_block` provider and `Network.setBlockedURLs`. |

`seed(v)` is `canvas_seed(v)` and `audio_seed(v ^ 0x5DEE_CE66)`, which is the
noise a single seed gave before the two were separate. `webgl_ids` keeps the PCI
ids with the fingerprint for tools that read them; pages never see them, so it
turns nothing on.

The dimensions with no value of their own (Bluetooth, the `window.chrome` stubs
and the headless fixes) are on only as `Mask`, which `StealthFlags::balanced()`
does for them.

How the flags and the values meet:

- `.flags(base)` gives a base, such as `StealthFlags::balanced()`. Whether it is
  called before or after the value setters makes no difference, and it never
  switches off a dimension a value turned on.
- `.with_flags(|f| ...)` is the explicit way to switch a dimension off, or to
  `Mask` it. It runs last, after the base and the values, in the order given.
- `build()` fills two values the caller left out: `languages`, from the locale
  (the table's list for a known locale, else the locale and its language), and
  the OS persona and `navigator.platform`, from a recognised user agent. A value
  you set is never replaced.

```rust
use seleniumbase_rs::{Dimension, Fingerprint, MaskingMode, StealthFlags};

// The preset can come first or last: the time zone is on either way.
let fp = Fingerprint::builder()
    .flags(StealthFlags::balanced())
    .timezone("Europe/Berlin")
    .build();
assert!(fp.is_masked(Dimension::Timezone));

// An override turns it off again, whatever was given.
let fp = Fingerprint::builder()
    .timezone("Europe/Berlin")
    .with_flags(|f| f.timezone_masking = MaskingMode::Natural)
    .build();
assert!(!fp.is_masked(Dimension::Timezone));
```

A fingerprint written field by field, or loaded from JSON, is read the same
way: a flag the file does not name is `Auto`, and a file that says `Natural`
keeps that dimension off. Nothing turns a dimension on for you there except the
value itself, so there is no reason to set `fp.flags` after giving a value.

## Masking modes

`StealthFlags` controls which dimensions are spoofed. Most flags are a
`MaskingMode`, which has these values:

| Mode | What happens |
|---|---|
| `Auto` (the default) | The dimension is on when the fingerprint has a value for it and off otherwise. The providers define only the values the fingerprint holds. |
| `Natural` | Explicitly off: the page sees the browser's real value, whatever the fingerprint holds. The launch argument and the protocol override for the dimension are left out too. For `native_tostring_masking` it removes the `native_tostring` provider. |
| `Mask` | On. A value the fingerprint lacks is filled with a generic one (the WebGL provider falls back to `Intel Inc.` / `Intel Iris OpenGL Engine`, the time zone provider to `America/New_York`). The dimensions with no value of their own are on only as `Mask`. |
| `Custom` | Deprecated: the same as `Mask`. |
| `Disabled` | Deprecated: the same as `Natural`. |

Some flags are not a `MaskingMode`, and each has its own `Auto`:

- `canvas_noise` is a `CanvasNoiseMode`. `Auto` adds noise when the fingerprint
  has a `seed`. `Mask` adds seeded noise (from the user agent when there is no
  seed), `Persistent` is the same, `Random` draws a fresh seed for each document
  and `Low` touches one pixel in eight. `Natural` and `Disabled` add none.
- `proxy_masking` is a `ProxyMaskingMode`. `Auto` uses the fingerprint's proxy
  when it has a host. `Custom` uses it whatever its host; `Socks5`, `Http` and
  `Https` are the same as `Custom`, because the proxy's own `type` says how to
  reach it. `Disabled` and `Direct` never use it.
- `geolocation_popup` is a `PopupMode`. `Auto` is `Allow` for a fingerprint with
  a position and `Prompt` for one without; `Fingerprint::geolocation_popup()`
  returns it resolved. `Allow` answers the page's request with the position,
  `Block` answers it as a user who pressed "Block" would (`PERMISSION_DENIED`),
  and `Prompt` leaves the browser's own prompt.
- `ports_masking` and `graphics_noise` are a `NoiseMode`; `quic_mode` is a
  `QuicMode`; `startup_behavior` is a `StartupBehavior`; and `humanize`,
  `block_trackers`, `disable_csp`, `grant_permissions`, `native_spoofing` and
  `open_shadow_roots` are `bool`.

`StealthFlags::default()` leaves every mode at `Auto`.
`StealthFlags::balanced()` is the base of every preset: the dimensions that carry
a value of the fingerprint's own stay on `Auto`, and WebRTC, canvas noise,
battery, connection, speech, Bluetooth, Client Hints, the `window.chrome` stubs,
the headless fixes and `toString` masking are `Mask`.
`StealthFlags::mask_all()` sets every dimension to `Mask` (and `all_custom()`, its
deprecated name, did the same).

## Stealth flag reference

The `default()` column is `StealthFlags::default()` and the `balanced()` column
is `StealthFlags::balanced()`, which every preset and every `randomized`
identity uses. `Auto` means "on when the fingerprint has a value for it". "Provider"
names are the built-in evasion providers listed under
[Built-in providers](#built-in-providers).

| Flag | Type | `default()` | `balanced()` | What it controls |
|---|---|---|---|---|
| `webrtc_masking` | `MaskingMode` | `Auto` | `Mask` | The `webrtc` provider: drops STUN and TURN servers from `RTCPeerConnection` configs and, when `webrtc_public_ip` or `webrtc_local_ip` is set, rewrites the IPs in SDP and ICE candidates. The Chrome launch flag `--force-webrtc-ip-handling-policy` is passed when WebRTC is on, with `webrtc_policy` or, when none is set, `DisableNonProxiedUdp`. |
| `audio_masking` | `MaskingMode` | `Auto` | `Auto` | The `audio_noise` provider. `Auto` needs an `audio_seed`; `Mask` derives one from the canvas seed. |
| `graphics_noise` | `NoiseMode` | `Mask` | `Mask` | Ignored. It never drove graphics noise, and the Modernizr hairline fix it used to gate follows `headless_masking`. Kept so saved fingerprints load. |
| `geolocation_popup` | `PopupMode` | `Auto` | `Auto` | How a geolocation request is answered: see [Masking modes](#masking-modes). With `Allow`, the `geolocation` provider answers `getCurrentPosition` and `watchPosition` itself and the geolocation permission is granted; with `Block` the provider refuses them. |
| `navigator_masking` | `MaskingMode` | `Auto` | `Auto` | The `navigator_props` and `plugins` providers (`plugins` only in `default_registry`; the session script leaves `navigator.plugins` and `mimeTypes` to the browser), `--user-agent` and `Network.setUserAgentOverride`. |
| `localization_masking` | `MaskingMode` | `Auto` | `Auto` | The `localization` provider (`navigator.language` and `navigator.languages`), `--lang`, `Emulation.setLocaleOverride` and the `Accept-Language` of the user agent override. |
| `timezone_masking` | `MaskingMode` | `Auto` | `Auto` | The `timezone` provider and `Emulation.setTimezoneOverride`. |
| `graphics_masking` | `MaskingMode` | `Auto` | `Auto` | The `webgl` provider. |
| `fonts_masking` | `MaskingMode` | `Auto` | `Auto` | The `fonts` provider, and only when `Fingerprint::fonts` is not empty. |
| `media_devices_masking` | `MaskingMode` | `Auto` | `Auto` | The `media_devices` provider. Under `Auto` a count the fingerprint lacks is none; `Mask` fills it with one. |
| `screen_masking` | `MaskingMode` | `Auto` | `Auto` | The `window_geometry` provider, `--window-size` and `Emulation.setDeviceMetricsOverride`. |
| `geolocation_masking` | `MaskingMode` | `Auto` | `Auto` | The `geolocation` provider (only when `Fingerprint::latitude` is set), `Emulation.setGeolocationOverride` and the geolocation permission. |
| `ports_masking` | `NoiseMode` | `Mask` | `Mask` | Whether `Fingerprint::ports` are blocked: `Natural` and `Off` leave every port open, any other mode blocks the listed ports on `localhost`, `127.0.0.1` and `[::1]` with `Network.setBlockedURLs`, in the same command as the tracker hosts. |
| `proxy_masking` | `ProxyMaskingMode` | `Auto` | `Auto` | `--proxy-server=<url>` from `Fingerprint::proxy`: see [Masking modes](#masking-modes). |
| `quic_mode` | `QuicMode` | `Natural` | `Natural` | `Disabled` and `ForceHttp2` add `--disable-quic`; `Enabled` adds `--enable-quic`; `Natural` and `Auto` add nothing. |
| `canvas_noise` | `CanvasNoiseMode` | `Auto` | `Mask` | The `canvas_noise` provider: `Mask`, `Persistent`, `Random` and `Low` run it, `Auto` runs it when there is a `seed`, `Natural` and `Disabled` do not. |
| `startup_behavior` | `StartupBehavior` | `Recover` | `Recover` | Stored only. No built-in code reads it. |
| `battery_masking` | `MaskingMode` | `Auto` | `Mask` | The `battery` provider. |
| `connection_masking` | `MaskingMode` | `Auto` | `Mask` | The `connection` provider (only in `default_registry`; the session script leaves `navigator.connection` to the browser). |
| `speech_masking` | `MaskingMode` | `Auto` | `Mask` | The `speech` provider. |
| `bluetooth_masking` | `MaskingMode` | `Auto` | `Mask` | The `bluetooth` provider. It has no value of its own, so only `Mask` turns it on. |
| `client_hints_masking` | `MaskingMode` | `Auto` | `Mask` | The `client_hints` provider. It also needs `Fingerprint::client_hints`, and it is skipped under `native_spoofing`. |
| `native_tostring_masking` | `MaskingMode` | `Auto` | `Mask` | The `native_tostring` provider runs unless this is `Natural` (or `Disabled`). |
| `chrome_runtime_masking` | `MaskingMode` | `Auto` | `Mask` | The `chrome_runtime` provider. Only `Mask` turns it on: it no longer follows `navigator_masking`. |
| `headless_masking` | `MaskingMode` | `Auto` | `Mask` | The `headless`, `permissions` and `hairline` providers, and `Emulation.setFocusEmulationEnabled`. Only `Mask` turns them on. |
| `humanize` | `bool` | `false` | `false` | Ignored. `BaseCase::human_type` reads `Fingerprint::humanize` (a `HumanizeConfig`) and its `enabled` field. |
| `block_trackers` | `bool` | `false` | `false` | The `tracker_block` provider, and `Network.setBlockedURLs` for the hosts in `Fingerprint::tracker_hosts()`. A tracker list given to `blocked_trackers` turns it on. |
| `disable_csp` | `bool` | `false` | `false` | `Page.setBypassCSP`. |
| `grant_permissions` | `bool` | `false` | `false` | `Browser.grantPermissions` for `notifications`, `midi`, `midiSysex`, `clipboardReadWrite`, `clipboardSanitizedWrite`, `paymentHandler`, `backgroundSync`, `idleDetection` and `webAppInstallation`. The geolocation permission is granted by a position with the popup on `Allow`, not by this flag. |
| `native_spoofing` | `bool` | `false` | `false` | See [Native-level (CDP) spoofing](#native-level-cdp-spoofing). |
| `open_shadow_roots` | `bool` | `true` | `true` | The `attach_shadow` provider, which forces closed shadow roots open so automation can reach into them. A page can detect that, so turn it off where it matters. |

### Literal values in JSON payloads

A profile imported through `ProfileParams` carries these flags as strings
(`ProfileFlags`): `webrtc_masking`, `proxy_masking`, `geolocation_popup`,
`audio_masking`, `graphics_noise`, `ports_masking`, `navigator_masking`,
`localization_masking`, `timezone_masking`, `graphics_masking`, `fonts_masking`,
`media_devices_masking`, `screen_masking`, `geolocation_masking`, `quic_mode`,
`canvas_noise` and `startup_behavior`, plus the booleans `disable_csp`,
`grant_permissions` and `native_spoofing`. Every other flag (battery,
connection, speech, Bluetooth, Client Hints, `native_tostring`, `chrome_runtime`,
headless, `humanize`, `block_trackers`) takes its `balanced()` value.

A flag the payload leaves out defaults to `"mask"` for the navigator,
localization, time zone, graphics, fonts, screen, geolocation, WebRTC and ports
flags, and to `"auto"` for `audio_masking`, `media_devices_masking`,
`geolocation_popup` and `proxy_masking`: those four are on exactly when the
payload holds a value for them. The payload's flags are applied as the explicit
last word, so a flag the payload names `"natural"` stays off although the
payload also holds a value for it.

Many of the string flags accept a concrete literal. The parser detects the
literal, sets the mode to `Mask`, and writes the value into the matching
`Fingerprint` field:

| Flag | Literal example | Populated fingerprint field |
|---|---|---|
| `screen_masking` | `"1920x1080x24"` | `screen_width`, `screen_height`, `color_depth` |
| `geolocation_masking` | `"40.7128,-74.0060,10.5"` | `latitude`, `longitude`, `altitude` |
| `graphics_masking` | `"Intel Inc.~Intel(R) Iris(R) Xe Graphics"` | `webgl_vendor`, `webgl_renderer` |
| `navigator_masking` | full UA string | `user_agent` |
| `localization_masking` | `"en-US"` (any value containing `-`) | `locale`, `languages` and `accept_languages`, all set to that same string |
| `timezone_masking` | `"America/New_York"` | `timezone` |
| `fonts_masking` | `"Arial,Calibri,Consolas"` | `fonts` |
| `media_devices_masking` | `"audioinput:default:Mic1\|videoinput:default:Cam1"` | `media_devices` |
| `ports_masking` | `"block:80,443,3000"` | `ports` |
| `proxy_masking` | `"socks5://user:pass@host:1080"` | `proxy` |
| `webrtc_masking` | `"public:172.56.21.89\|local:10.0.0.5"` | `webrtc_public_ip`, `webrtc_local_ip` |

A screen literal also sets `color_depth` (24 when omitted), a geolocation literal
sets `accuracy` to 20, and a WebRTC literal sets the policy to
`WebRtcPolicy::PublicAndPrivateInterfaces`.

### Keywords in JSON payloads

Keywords are case-insensitive. A value the parser does not know gets the
"falls back to" mode.

| Payload flag | Keywords and the mode they give | Falls back to |
|---|---|---|
| `navigator_masking` | `natural`, `off`, `disabled`, `block`: `Natural`. `auto`: `Auto`. `mask`, `spoof`, `desktop_chrome`, `mobile_safari`: `Mask`. A string starting `Mozilla/` or containing `AppleWebKit` or `Chrome/` is a user agent. | `Mask` |
| `screen_masking` | `natural`, `native`, `off`, `disabled`, `block`: `Natural`. `auto`: `Auto`. `custom`, `mask`, `spoof`: `Mask`. | `Mask` |
| `geolocation_masking` | `natural`, `off`, `disabled`, `block`: `Natural`. `auto`: `Auto`. `custom`, `mask`, `spoof`: `Mask`. | `Mask` |
| `graphics_masking` | `natural`, `pass_through`, `off`, `disabled`, `block`: `Natural`. `auto`: `Auto`. `spoof`, `generic`, `mask`: `Mask`. | `Mask` |
| `localization_masking`, `timezone_masking` | `natural`, `off`, `disabled`, `block`: `Natural`. `auto`: `Auto`. `mask`, `spoof`, `custom`: `Mask`. A localization value must contain `-`, a time zone must contain `/`. | `Mask` |
| `fonts_masking` | `natural`, `off`, `disabled`, `block`: `Natural`. `auto`: `Auto`. `mask`, `spoof`, `system_only`, `random`, `blacklist`: `Mask`. Anything else is read as a comma-separated font list. | `Mask` |
| `media_devices_masking` | `natural`, `off`, `disabled`, `block`, `empty`: `Natural`. `auto`: `Auto`. `mask`, `spoof`, `noise`: `Mask`. A value starting `2_mic_1_cam`: `Mask` with 2 audio inputs, 1 audio output and 1 video input. | `Mask` |
| `audio_masking` | `natural`, `off`, `disabled`, `block`: `Natural`. `auto`: `Auto`. | `Mask` |
| `webrtc_masking` | `natural`, `off`: `Natural`, with no policy. `auto`: `Auto`. `disabled`, `block`: the policy `DisableNonProxiedUdp`, which turns WebRTC on whatever else the payload holds. `mask`, `spoof`: `Mask` with `DisableNonProxiedUdp`. `public_only`: `Mask` with `WebRtcPolicy::PublicInterfaceOnly`. | `Mask` |
| `proxy_masking` | `natural`, `off`, `direct`, `disabled`: `Disabled`. `auto`: `Auto`. `socks5`, `http`, `https`, `custom`: `Custom`, with no proxy address (the proxy's own `type` says how to reach it). A `socks5://`, `http://` or `https://` URL is parsed into `Fingerprint::proxy` (default port 1080 for SOCKS5, 8080 otherwise). | `Disabled` |
| `graphics_noise` | `natural`: `Natural`. `off`: `Off`. `low`, `medium`, `high`: that value. The flag is ignored; see the reference above. | `Mask` |
| `canvas_noise` | `auto`, `natural`, `disabled` (or `off`), `random`, `persistent`, `low`: that mode. If the key is absent: `Mask`. | `Mask` |
| `ports_masking` | `natural`, `off`, `disabled`: `Off`. `mask`, `spoof`, `stealth`: `Mask`. `block`, `block_all`: `Block`. `whitelist`: `Natural`. `block:80,443` or `80,443`: `Block` with those ports. | `Mask` |
| `geolocation_popup` | `auto`, `prompt`, `allow`, `block`. | `Auto` |
| `quic_mode` | `natural`, `auto`: `Natural`. `enabled`, `disabled`, `force_http2`. | `Disabled` |
| `startup_behavior` | `custom`. | `Recover` |

A payload that leaves out `quic_mode` gets `Disabled`, which adds `--disable-quic`;
`balanced()` leaves QUIC alone.

### Concrete examples

#### A navigator

```rust
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36")
    .platform("Win32")
    .hardware_concurrency(8)
    .device_memory(8.0)
    .build();
```

With `DriverMode::Uc` or `DriverMode::Cdp`, the `navigator_props` provider
overrides `navigator.userAgent`, `appVersion`, `platform`, `hardwareConcurrency`
and `deviceMemory` with the values you supplied. It defines only those: under
`Auto` it adds no `vendor` or `productSub` of its own. The same user agent is also
passed as `--user-agent` and sent as `Network.setUserAgentOverride`.

#### A screen, a position and a time zone

```rust
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::builder()
    .screen(1920, 1080)
    .geolocation(52.52, 13.405)
    .timezone("Europe/Berlin")
    .build();
```

With `DriverMode::Uc` or `DriverMode::Cdp`:

- The launch arguments include `--window-size=1920,1080`, and the `window_geometry`
  provider overrides the `screen` properties, `outerWidth`, `outerHeight` and
  `devicePixelRatio`. Only the values the fingerprint holds are defined: this one
  has no pixel ratio or colour depth, so those stay the browser's.
- `Emulation.setDeviceMetricsOverride` and `Emulation.setGeolocationOverride` are
  sent.
- The `timezone` provider patches `Intl.DateTimeFormat` and the local-time methods
  of `Date` to Berlin time, and `Emulation.setTimezoneOverride` is sent as well.
- A position answers the popup with `Allow`: the `geolocation` provider answers
  `getCurrentPosition` and `watchPosition` in JavaScript, and
  `Browser.grantPermissions` grants the geolocation permission, so a page that
  asks gets the position instead of waiting for a prompt nobody presses.

#### A proxy

```rust
use seleniumbase_rs::{Fingerprint, ProxyConfig};

let fp = Fingerprint::builder()
    .proxy(ProxyConfig {
        r#type: "http".to_owned(),
        host: "proxy.example.com".to_owned(),
        port: 8080,
        username: Some("alice".to_owned()),
        password: Some("secret".to_owned()),
        save_traffic: false,
    })
    .build();
```

`ProxyConfig` has no `FromStr`; build it as above. A proxy with a host is used
under `Auto`, and emits
`--proxy-server=http://alice:secret@proxy.example.com:8080` (the URL is
`<type>://<user>:<password>@<host>:<port>`; Chrome ignores the credentials in
this flag). A proxy with an empty host, such as `ProxyConfig::default()`, is
inert until `proxy_masking` is `Custom`. `Disabled` and `Direct` emit nothing.
`Debug` output of a `ProxyConfig` hides the password.

No path of this crate sends the proxy's password as a `Proxy-Authorization`
header: it would go to every site the tab visits. The pure CDP engine answers
the proxy's own challenge instead, and the WebDriver path uses
`safe_cdp_overrides`, which leaves the header out. Only the deprecated
`evasions::cdp_overrides` still contains it.

#### WebRTC

```rust
use seleniumbase_rs::{Fingerprint, WebRtcPolicy};

let fp = Fingerprint::builder()
    .webrtc_policy(WebRtcPolicy::DisableNonProxiedUdp)
    .build();
```

Giving a policy or WebRTC addresses turns the dimension on, and the dimension
acts in two ways:

- `webrtc_policy` picks a Chrome flag that is passed on every launch path that
  takes a fingerprint. `DisableNonProxiedUdp` gives
  `--force-webrtc-ip-handling-policy=disable_non_proxied_udp`,
  `PublicInterfaceOnly` gives `default_public_interface_only`, and
  `PublicAndPrivateInterfaces` gives `default_public_and_private_interfaces`.
  The field is an `Option`: a fingerprint with no policy and no WebRTC masking
  (`Fingerprint::default()`, an empty builder) passes no flag, and WebRTC masked
  without a policy uses `DisableNonProxiedUdp`. A saved file with a bare
  `webrtc_policy` still loads.
- The JavaScript `webrtc` provider drops STUN and TURN servers from new
  `RTCPeerConnection`s and rewrites IPs when `webrtc_public_ip` or
  `webrtc_local_ip` is set.

`webrtc_masking: Natural` switches both off.

## Native-level (CDP) spoofing

When a session has a CDP connection (`DriverMode::Uc`, `DriverMode::Cdp`, or the
Pure CDP engine), a fingerprint is applied in two layers:

1. The evasion script, assembled from the providers below and registered with
   `Page.addScriptToEvaluateOnNewDocument`. Page scripts can *in principle*
   detect a JavaScript patch through `Function.prototype.toString`,
   `Object.getOwnPropertyNames`, or descriptor inspection, which is why the
   providers use the `sb` helpers to look native.
2. The protocol overrides from `evasions::safe_cdp_overrides`. (The older
   `evasions::cdp_overrides` is deprecated: it also holds the `Proxy-Authorization`
   header that would hand the proxy's login to every site.) A command is sent
   for a dimension that is on (`Fingerprint::is_masked`) and holds the value it
   needs, so a dimension named `Natural` sends nothing:

| Command | Sent when |
|---|---|
| `Network.setUserAgentOverride` (user agent, platform, `userAgentMetadata`, and `acceptLanguage`) | `Navigator` is on and `user_agent` is set. `acceptLanguage` is sent only when `Localization` is on: `accept_languages`, else `locale`, else (`Mask` only) `en-US,en;q=0.9`. The metadata comes from `client_hints` when set and is derived from the OS and user agent otherwise: Chrome's brand list, or Edge's when the user agent has an `Edg/` token. |
| `Emulation.setDeviceMetricsOverride` | `Screen` is on and `screen_width` and `screen_height` are set. `pixel_ratio` is the scale factor (1.0 if unset) and `mobile` follows `os_type`. `safe_cdp_overrides` sets the viewport (`width`, `height`) to the screen, and the WebDriver path sends it as it is. The Pure CDP engine changes it for a desktop persona: it sends a `0` by `0` viewport, which the protocol reads as "no override", plus `screenWidth` and `screenHeight`, so the browser itself reports `screen.width` and `screen.height` and the page keeps the viewport its window really has. A mobile persona is a phone-sized page, so its viewport stays the screen. |
| `Emulation.setLocaleOverride` | `Localization` is on and `locale` is set. |
| `Emulation.setTimezoneOverride` | `Timezone` is on and `timezone` is set. |
| `Emulation.setGeolocationOverride` | `Geolocation` is on and `latitude` and `longitude` are set (accuracy 100 and altitude 0 unless set). |
| `Browser.grantPermissions` | The geolocation permission, when `Geolocation` is on and the popup resolves to `Allow` (a fingerprint with a position does), and the permissions listed under `grant_permissions` when that flag is on. |
| `Network.setBlockedURLs` | One command, because a second would replace the first: the tracker hosts when `Trackers` is on, and `*://localhost:PORT/*` (and `127.0.0.1`, `[::1]`) for each of `ports` when `Ports` is on. A session with ad blocking joins the ad block list to it. |
| `Page.setBypassCSP` | `disable_csp` is on. |
| `Emulation.setFocusEmulationEnabled` | `headless_masking` is `Mask`. |

So a UC or CDP session already gets these overrides without `native_spoofing`,
and the JavaScript providers patch the same properties on top of them.
`StealthFlags::native_spoofing` makes the providers stand down where a protocol
override covers the property, so the browser reports the value itself instead of
a replaced JavaScript getter:

```rust
use seleniumbase_rs::Fingerprint;

let fp = Fingerprint::builder()
    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 …")
    .platform("Win32")
    .screen(1920, 1080)
    .locale("en-US")
    .timezone("America/New_York")
    .geolocation(40.7580, -73.9855)
    .native_spoofing(true)
    .build();
```

When `native_spoofing` is true:

* `navigator_props` leaves out `platform`, `userAgent` and `appVersion`.
* `window_geometry` emits only `colorDepth` and `pixelDepth`; the size and scale
  come from `Emulation.setDeviceMetricsOverride`.
* `localization`, the JavaScript `geolocation` provider and `client_hints` are
  skipped, and `timezone` is skipped when a `timezone` is set.
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

## rustwright / Playwright mode

This mode needs the `playwright` Cargo feature (`rustwright`); it is not on by
default.

```rust
use seleniumbase_rs::browser::playwright::PlaywrightSession;
use seleniumbase_rs::Fingerprint;

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let fp = Fingerprint::windows_desktop();
let session = PlaywrightSession::launch_with_fingerprint(&fp).await?;
session.goto("https://example.com").await?;
session.close().await?;
# Ok(())
# }
```

`launch_with_fingerprint` opens a headed browser; use
`launch_headless_with_fingerprint` for headless. The session passes the
fingerprint's launch arguments, then evaluates the evasion script in the first
page and again after every `goto` and `new_page`, because rustwright cannot
register a script to run in each new document. The script therefore runs after
navigation, not before the page's own scripts, so a page that reads the identity
while it loads can see the browser's real one. The protocol overrides listed above
are not sent in this mode.

## External profile payloads

The `profile_payloads::ProfileParams` type maps to a `Fingerprint` via
`ProfileParams::to_fingerprint()`. `ProfileParams::to_browser_config(webdriver_url)`
returns a `BrowserConfig` with that fingerprint attached.

The returned `mode` follows the payload's `automation` field: `playwright` and
`puppeteer` give `DriverMode::Cdp`; `selenium` (the default when the field is
absent) and the Firefox browser types give `DriverMode::WebDriver`; anything else
gives `DriverMode::Uc`. The script and protocol overrides are applied only in
`Cdp` and `Uc` mode, so a payload that keeps `selenium` is launched with the launch
arguments only. The payload's `cmd_params` become Chrome flags, except switches
that run a program, load an extension, set the profile directory, a proxy or
remote debugging, or ignore certificate errors; those are refused. The Pure CDP
engine does not pass `cmd_params` at all.

## Provider architecture

Every evasion is an `EvasionProvider`: a small, self-contained unit that returns
a JavaScript snippet for an `EvasionContext` (the session fingerprint plus a
deterministic seed and runtime config). Providers are held by an
`EvasionRegistry` and assembled into a single bootstrap script in priority order
(lower `priority()` runs first). The script is one function; each snippet sits in
its own `try`/`catch`, after a `// provider: <name>` comment. `EvasionConfig`
turns either of those off.

```rust
use seleniumbase_rs::{default_registry, EvasionContext, Fingerprint};

let fp = Fingerprint::windows_desktop();
let ctx = EvasionContext::new(&fp);            // seed derived from the fingerprint
let script = default_registry().bootstrap(&ctx);
assert!(script.contains("webdriver"));
```

`evasions::bootstrap_script(&fp)` is a thin wrapper over
`default_registry().bootstrap(&EvasionContext::new(&fp))`, so existing callers
keep working while new evasions are added centrally. `evasions::session_script(&fp)`
is the same over `session_registry`, the script a browser session runs.

### Adding your own evasion

1. Implement `EvasionProvider` for a unit struct. Return the JavaScript from
   `script`, gate inclusion with `applies`, and order it with `priority`.
   `providers::SB_PREAMBLE` brings in the `sb` helpers that make a replaced
   function look native; the example below skips it for brevity.
2. Register it at runtime:

```rust
use seleniumbase_rs::{default_registry, EvasionContext, EvasionProvider, Fingerprint};

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

See [Contributing](../CONTRIBUTING.md) for adding a built-in provider.

### Built-in providers

`default_registry()` registers 29 providers, listed here in execution order. "Runs
when" is the provider's `applies` condition. A dimension "is on" when
`Fingerprint::is_masked` says so: its mode is `Mask`, or it is `Auto` and the
fingerprint holds a value for it (see [A value is the switch](#a-value-is-the-switch)).

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
| 5 | `native_tostring` | `native_tostring_masking` is not `Natural` or `Disabled` | Makes replaced functions print `[native code]` from `Function.prototype.toString`, and installs the `sb` helpers the other providers use. |
| 10 | `webdriver` | always | Makes `navigator.webdriver` read `false` (a getter on the navigator prototype) and deletes known automation hooks from `window` (`callPhantom`, `_phantom`, `__nightmare`, `__selenium_unwrapped`, `__webdriver_evaluate`, `__driver_evaluate`, `domAutomation`, and others). |
| 15 | `cdp_markers` | always | Deletes `$cdc_`, `__webdriver`, `__selenium`, `__driver`, `$chrome_` and `__fxdriver` properties from `window`, its prototype chain and `document`. |
| 20 | `chrome_runtime` | `chrome_runtime_masking` is `Mask` | `window.chrome.app`, `loadTimes`, `csi` and `runtime` stubs. |
| 30 | `navigator_props` | Navigator is on | `userAgent`, `appVersion` and `platform` (not under `native_spoofing`), then `hardwareConcurrency`, `deviceMemory` and `maxTouchPoints` when set, `vendor`, `productSub`, and `oscpu` and `buildID` when set. Under `Auto` it defines only the values the fingerprint holds; `Mask` fills `platform` from the OS, `vendor` with Chrome's (or Safari's) and `productSub` with `20030107`. It does not touch `pdfViewerEnabled`, `plugins` or `mimeTypes`: Chrome answers for them itself. |
| 35 | `permissions` | `headless_masking` is `Mask` | Makes `permissions.query({ name: 'notifications' })` agree with `Notification.permission`. A headless fix, so it no longer follows `navigator_masking`. |
| 40 | `plugins` | Navigator is on | Five PDF-viewer plugins and their MIME types. In `default_registry()` only. |
| 45 | `window_geometry` | Screen is on | `screen.width`, `height`, `availWidth`, `availHeight`, `colorDepth`, `pixelDepth`, plus `outerWidth`, `outerHeight` and `devicePixelRatio`, each only when the fingerprint holds the value (`Mask` fills a missing size with the OS's, colour depth 24 and a ratio of 1). `avail_width` and `avail_height` set the available size; without them it is the screen less 40 pixels in height. The window is as large as the persona's screen, and never smaller than the viewport it really has. Under `native_spoofing`, only `colorDepth` and `pixelDepth`. |
| 50 | `webgl` | Graphics is on | `UNMASKED_VENDOR_WEBGL` and `UNMASKED_RENDERER_WEBGL` from WebGL1 and WebGL2 `getParameter`. A half the fingerprint lacks stays the browser's, unless `Mask` fills it. |
| 55 | `canvas_noise` | Canvas is on | Deterministic noise, a function of the seed and the pixel position, on `toDataURL`, `toBlob` and `getImageData`. `Low` touches one pixel in eight and `Random` draws a new seed for each script. |
| 60 | `audio_noise` | Audio is on | Deterministic noise on `AudioBuffer.getChannelData` and `copyFromChannel` and on `AnalyserNode.getFloatFrequencyData`, seeded by `audio_seed`. |
| 65 | `webrtc` | WebRTC is on | Drops STUN and TURN servers from `RTCPeerConnection` configs; rewrites IPs in SDP and `RTCIceCandidate` when `webrtc_public_ip` or `webrtc_local_ip` is set. |
| 70 | `battery` | Battery is on | `navigator.getBattery()` (defaults: charging, level 0.94). |
| 72 | `connection` | Connection is on | `NetworkInformation` `effectiveType`, `rtt`, `downlink` and `saveData` (defaults: `4g`, 50, 10, `false`). In `default_registry()` only. |
| 75 | `media_devices` | MediaDevices is on | `enumerateDevices`, from `media_devices` (explicit labels) or generated from the input and output counts (a count the fingerprint lacks is none under `Auto` and 1 under `Mask`). |
| 76 | `fonts` | Fonts is on and `fonts` is not empty | Limits `document.fonts.check`, `load` and `FontFace` to the configured fonts. |
| 78 | `speech` | Speech is on | `speechSynthesis.getVoices()` (a default set of three voices unless `speech_voices` is set). |
| 80 | `bluetooth` | `bluetooth_masking` is `Mask` | A `navigator.bluetooth` stub, only if the browser has none. |
| 85 | `headless` | `headless_masking` is `Mask` | `matchMedia` answers (`prefers-reduced-motion: reduce` and `prefers-color-scheme: dark` false, `any-pointer: fine` true) and `document.hidden` / `visibilityState`. |
| 87 | `prepare_stack_trace` | never | Reserved name. It emits no script. |
| 88 | `client_hints` | ClientHints is on, `client_hints` is set, and not `native_spoofing` | `navigator.userAgentData`, including `getHighEntropyValues`, but only in a secure context (`window.isSecureContext`): that is the only place Chrome has the property, so on a plain `http:` page there is none to read and none is added. `localhost` and `127.0.0.1` are secure contexts. |
| 89 | `media_codecs` | always | `canPlayType` answers for MP4, AAC and MP3 types when the browser says it cannot play them. WebM and everything else stay the browser's own. |
| 90 | `timezone` | Timezone is on, unless `native_spoofing` is on and a `timezone` is set | `Intl.DateTimeFormat` and the local-time methods of `Date` (`getTimezoneOffset`, `getHours`, `toString`, the `toLocale*String` family and others). `new Date(y, m, d)` and `Date.parse` of a string without an offset keep the machine's zone. |
| 92 | `localization` | Localization is on, and not `native_spoofing` | `navigator.language` and `navigator.languages`: the `languages` the fingerprint holds, else its `accept_languages`, else the list its `locale` implies (`Mask` falls back to `en-US,en`). |
| 95 | `geolocation` | Geolocation is on and `latitude` is set (or the popup is `Block`), and not `native_spoofing` | `getCurrentPosition` and `watchPosition`. They answer in JavaScript when the popup resolves to `Allow` and refuse with `PERMISSION_DENIED` when it is `Block`. |
| 110 | `hairline` | `headless_masking` is `Mask` | Modernizr's hairline check: `offsetHeight` of the element with id `modernizr` reads `1`. |
| 120 | `iframe` | never | Reserved name. It emits no script. |
| 125 | `attach_shadow` | `open_shadow_roots` is on | Forces `Element.attachShadow` to `mode: 'open'`. |
| 130 | `tracker_block` | Trackers is on | Makes `fetch`, `XMLHttpRequest` and `navigator.sendBeacon` to the tracker hosts (and their subdomains) fail like an unreachable host. Scripts, images and frames loaded by markup are not covered. |

List them at runtime with `default_registry().provider_names()` (all 29, including
the two reserved ones), `session_registry(&fp).provider_names()` (the 27 a
browser session runs), `applicable_names(&fp)` (those that run for a fingerprint),
or over MCP with the `list_evasion_providers` tool, which lists the session ones.

## Profile coherence validation

`Fingerprint::validate()` returns a `CoherenceReport` with `errors` and
`warnings`; `is_coherent()` is true when there are no errors.

It reports an error for:

- a `navigator.platform` that does not fit `os_type` (a macOS user agent with a
  `Win32` platform), and a user agent with no token for `os_type`;
- a screen with a zero dimension;
- a WebGL renderer from another operating system's graphics API: Direct3D off
  Windows, Metal or an Apple GPU off macOS and iOS, Mesa off Linux, and Adreno,
  Mali or PowerVR off Android;
- a `MobileSafari` persona whose user agent has a `Chrome/` token.

It reports a warning for:

- a mobile user agent with `maxTouchPoints == 0`, and a desktop user agent with
  touch points;
- a `locale` that differs from the first of `languages`;
- a time zone with no geolocation;
- a locale from another country than the time zone, judged with the table of
  places `randomize::LOCALES` (a zone the table does not hold, or a locale with
  no region, is not judged);
- WebGL masked as `Mask` with no vendor and renderer on an OS other than macOS
  and iOS: the generic fill is an Intel renderer worded for a Mac.

It does not look at hardware.

```rust
use seleniumbase_rs::Fingerprint;

let report = Fingerprint::windows_desktop().validate();
assert!(report.is_coherent() && report.warnings.is_empty());
for warning in &report.warnings {
    eprintln!("warning: {warning}");
}
```

The MCP `validate_fingerprint` tool returns the same report for one of the six
built-in presets (`windows`, `edge`, `macos`, `linux`, `android`, `ios`); it does
not take an arbitrary fingerprint.

`tests/fingerprint_randomized.rs` asserts that `Fingerprint::randomized` gives no
errors and no warnings for every OS, for seeds 0 to 1499 and ten edge seeds
(`0`, `u64::MAX`, `1 << 63` and others). If you edit a generated profile by hand,
run `validate()` again afterwards.

## Humanized input

Detectors look for input that is too regular. The
`stealth::behavior` module plans input that is not: curved
pointer paths with a bell-shaped speed profile, Fitts's-law timing, tremor and
overshoot, and Gaussian typing with word and sentence pauses. It is pure and
seedable:

```rust
use seleniumbase_rs::sb_cdp::Point;
use seleniumbase_rs::stealth::behavior::{Behavior, Humanizer};

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let mut person = Humanizer::new(Behavior::builder().typing_wpm(70.0).seed(7).build()?);
let path = person.mouse_path(Point::new(0.0, 0.0), Point::new(200.0, 90.0), 60.0);
let keys = person.typing_plan("hello");
assert_eq!(path.last().map(|step| step.at), Some(Point::new(200.0, 90.0)));
assert_eq!(keys.len(), 5);
# Ok(())
# }
```

To play a plan out on a page, use `page.human(behavior)` from the Pure CDP
engine (see [Pure CDP](pure_cdp.md#human-paced-input)). `BaseCase::human_type`
uses the same typing model, taking its pace from `Fingerprint::humanize` when the
configuration has a fingerprint.

`Behavior::from_config(&fingerprint.humanize)` turns a fingerprint's
`HumanizeConfig` (its minimum and maximum keystroke delay) into the matching
behaviour.

The older `stealth::humanize` helpers (`bezier_mouse_path`,
`keystroke_delays`) remain, but they sample a Bézier curve at evenly spaced
parameter values and draw delays uniformly between two bounds; prefer
`behavior`.

## What is spoofed

With a CDP session and the `balanced()` flags (see the provider table for the
conditions), for the values the fingerprint holds:

* `navigator.webdriver` reads `false`, and known automation hooks are deleted
* `navigator.userAgent`, `platform`, `hardwareConcurrency`, `deviceMemory`,
  `maxTouchPoints`, `vendor`, `language` and `languages`
* `window.chrome.app`, `csi`, `loadTimes` and `runtime` stubs
* `navigator.plugins` and `navigator.mimeTypes` (`bootstrap_script` and the
  Playwright session; the session script leaves them to Chrome)
* `navigator.permissions.query` for notifications
* `screen.width`, `height`, `availWidth`, `availHeight`, `colorDepth`,
  `outerWidth`, `outerHeight` and `devicePixelRatio`
* WebGL `UNMASKED_VENDOR_WEBGL` and `UNMASKED_RENDERER_WEBGL`
* Canvas `toDataURL`, `toBlob` and `getImageData` noise
* Audio noise (when the fingerprint has an `audio_seed`, which `seed` and every
  `randomized` identity set, or `audio_masking` is `Mask`)
* `navigator.mediaDevices.enumerateDevices` (when the fingerprint holds device
  counts or labelled devices)
* Time zone, through `Intl.DateTimeFormat`, the local-time methods of `Date`, and
  `Emulation.setTimezoneOverride`
* Geolocation, through `Emulation.setGeolocationOverride`, the JavaScript
  provider, and the geolocation permission
* `$cdc_`, `__webdriver`, `__selenium` and similar marker properties
* Battery Status, Speech Synthesis, and Bluetooth APIs, and Network Information
  (the last only in `bootstrap_script`)
* Client Hints (`navigator.userAgentData`), when `client_hints` is set and the
  page is a secure context
* `matchMedia` answers and page visibility, `canPlayType` for MP4/AAC/MP3, and
  `attachShadow` roots forced open
* STUN and TURN servers dropped from `RTCPeerConnection` configs, and Chrome's
  WebRTC IP-handling flag
* Launch arguments such as `--disable-blink-features=AutomationControlled`
* Optionally (`native_spoofing`), fewer properties replaced in JavaScript, and a
  patched Chrome binary for WebDriver sessions

## Complementary defenses

Runtime fingerprints work best when the browser also hides engine-level
automation markers:

* Patch the `chromedriver` binary with `ChromedriverPatcher` (for example
  `ChromedriverPatcher::new(path).patch(EnginePatch::all())?`) to remove injected
  `cdc_` and `__webdriver` signatures. See the
  [Binary Patching tutorial](./binary_patching.md).
* Pass the extra Chromium flags returned by `engine_spoofing_args()` (a
  `Vec<String>`) through `BrowserConfig::with_extra_args`, which replaces the
  list (`push_extra_arg` adds one), or through `StealthOptions::extra_args`.
* Combining a `Fingerprint`, UC mode, binary patching, and engine args covers the
  most layers this crate offers. None of it guarantees that a given detector
  passes.

## Limitations

* Nothing on this page changes the TLS or HTTP/2 behaviour of the browser. A
  fingerprint reaches the page through scripts and protocol overrides; the
  `ClientHello`, the HTTP/2 connection start and the header order that leave the
  machine are those of the browser you launch. JA3, JA4, peetprint and Akamai
  parsers, a measured table for one browser (Chrome 155 on macOS), a client that
  sends that handshake or one you specify, and `MitmProxy`, which gives a real
  Chrome on the Pure CDP engine the impersonated handshake, are in the separate,
  experimental seleniumbase-rs-stealth crates since 0.3.0; see
  [Stealth Scraping](stealth_scraping.md). The design history is the
  [Stealth and impersonation plan](../design/stealth_parity_plan.md).
* Some dimensions (WebGL vendor/renderer, canvas/audio noise, media devices,
  fonts, battery, etc.) have no CDP override and remain JavaScript-based even
  when `native_spoofing` is enabled.
* Chromium source patching is not performed. The binary patchers edit byte
  strings in an existing `chromedriver` or Chrome binary.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Still flagged by bot detection | The evasion script was never installed (`DriverMode::WebDriver`), the dimension is `Natural` or the fingerprint holds no value for it, or the site checks what this page does not change: the browser's own TLS and HTTP/2 fingerprint (see [Stealth Scraping](stealth_scraping.md)), the IP address, the browser's real version | Use `DriverMode::Uc` or the Pure CDP engine, build the fingerprint from a preset or `Fingerprint::randomized`, check `fp.is_masked(Dimension::...)` for the dimension, and match the egress IP to the identity's place. |
| A value was given and the page does not show it | An explicit `Natural` (a `with_flags` override, a preset or file that names it) keeps the dimension off whatever the fingerprint holds | Remove the override, or set the mode to `Auto` or `Mask`. |
| `navigator.webdriver` still `true` | The bootstrap script did not run: `DriverMode::WebDriver`, or no fingerprint on the config. The `webdriver` provider has no flag that turns it off. | Use `DriverMode::Uc` or `Cdp`, or `Page::apply_fingerprint` on the Pure CDP engine. For driver-level markers, see binary patching. |
| Geolocation is wrong or never answers | No `latitude` and `longitude` on the fingerprint, no CDP session, or `geolocation_masking` is `Natural` | Set `.geolocation(lat, lon)` and use `DriverMode::Uc` or `Cdp`. The position grants the permission itself; `geolocation_popup: Block` refuses it on purpose. |
| Inconsistent locale and time zone | `locale` and `timezone` come from different countries. `validate()` warns about this pair. | Align both, or use `Fingerprint::randomized`, which draws them from one place. |
| WebRTC leaks local IP | `webrtc_masking` is `Natural`, or `webrtc_policy` is a looser policy | Leave `webrtc_masking` to `Mask` (the presets do) and keep `WebRtcPolicy::DisableNonProxiedUdp`. |

---

This page was brought up to date with the change that makes a value the switch by
reading `src/stealth/**`, `src/sb_cdp/{identity,launch}.rs`,
`src/browser/{config,session,playwright}.rs`, `src/profile_payloads/**`,
`src/mcp/stealth.rs` and `src/macros.rs` again. The code examples were compiled
against that source as a test of the crate; the tables were checked against the
`applies` conditions of the providers.
