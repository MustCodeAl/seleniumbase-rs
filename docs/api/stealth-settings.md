---
title: "Stealth settings"
sidebarTitle: "Stealth settings"
description: "Fingerprint, Spoof and StealthPolicy in this crate, and the launch settings that apply them."
---

<Note>

These types live in this crate (`seleniumbase_rs::evasion` and the root re-exports). They are not `BaseCase` methods. Names below are the public items. Argument names and return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

A [`Fingerprint`](#fingerprint) is what the browser should claim. Each setting is one [`Spoof`](#spoof): leave the browser's own value, draw a random one, use a given value, or turn that API off. A [`StealthPolicy`](#stealthpolicy) is how those claims are delivered and which protections run beside them. A policy never changes a setting.

UC mode is separate. It does not build a fingerprint and it does not patch chromedriver. See [CDP and UC helpers](/api/cdp-uc#uc-mode).

## Spoof

`Spoof<T>` is one setting. The default is `Natural`. A plain value converts with `Into`, so `"Europe/Berlin"` is `Spoof::Value`.

| State | Meaning |
|-------|---------|
| `Natural` | The browser's own value. Default. `is_natural()` is true. |
| `Random` | Draw a coherent value when the fingerprint is built. A built fingerprint does not still hold `Random`. |
| `Value(T)` | Exactly this value. `value()` and `into_value()` return it. |
| `Off` | The API is unavailable or denied. `build` returns an error that names the setting when that setting has no off state. |

| Method | Description |
|--------|-------------|
| `is_natural()`, `is_random()`, `is_value()`, `is_off()` | Which state this is. |
| `is_set()` | True for every state except `Natural`. |
| `as_ref()` | The same state, borrowing the value. |
| `map(f)` | The same state with the value passed through `f`. |

In JSON a `Spoof` is `"natural"`, `"random"`, `"off"`, or the value itself. `null` reads as `Natural`. On a string setting those three words are reserved: a value spelled like one of them reads as the keyword.

```rust
use seleniumbase_rs::Spoof;

let zone: Spoof<String> = "Europe/Berlin".into();
assert_eq!(zone.value().map(String::as_str), Some("Europe/Berlin"));
assert!(Spoof::<String>::default().is_natural());
assert!(Spoof::<String>::Random.is_random());
assert!(Spoof::<String>::Off.is_off());
```

## Fingerprint

`Fingerprint` is built with `Fingerprint::builder()` or a preset. The fields are private. `to_builder()` starts a builder from an existing fingerprint. `Default` is a builder with nothing set: every setting `Natural`.

| Method | Description |
|--------|-------------|
| `builder()` | A `FingerprintBuilder`. |
| `to_builder()` | A builder that starts from this fingerprint. |
| `windows_desktop()`, `edge_desktop()`, `macos_desktop()`, `linux_desktop()`, `android_mobile()`, `ios_mobile_safari()` | Fixed presets. Each uses `StealthPolicy::balanced()`. |
| `randomized(os, seed)` | A coherent identity for `OsType` and `seed`. The same seed repeats. Fonts, media devices, WebRTC addresses and the proxy stay unset. |
| `randomized_with_major(os, seed, major)` | As `randomized`, with the Chromium major fixed. Safari and iOS ignore `major`. |
| `browser_type()`, `os_type()`, `core_version()`, `seed()` | The persona, the Chromium major if one was set, and the noise seed. |
| `is_masked(dimension)` | Whether this fingerprint spoofs that `Dimension`. A dimension is on when one of its settings is not `Natural`, or when the policy turns that protection on. |
| `validate()` | A `CoherenceReport` of mismatches between values that were given. A `Natural` setting is not checked. |
| `policy()`, `humanize()` | The delivery policy and the humanized input timing. |
| `tracker_hosts()` | The policy's host list, or the crate's default list when that list is empty. |

Each other setting is a read accessor that returns `&Spoof<T>` (or `&Vec<String>` / `&HashMap<String, String>` for `custom_start_urls` and `cmd_params`): `user_agent`, `platform`, `hardware_concurrency`, `device_memory`, `max_touch_points`, `vendor`, `oscpu`, `product_sub`, `build_id`, `app_version`, `locale`, `languages`, `accept_languages`, `timezone`, `screen`, `avail_screen`, `pixel_ratio`, `color_depth`, `webgl`, `media_devices`, `geolocation`, `fonts`, `proxy`, `webrtc`, `battery`, `connection`, `speech_voices`, `bluetooth`, `client_hints`. `local_storage()` and `save_service_worker()` are `bool`.

`Off` is meaningful for media devices, geolocation, WebRTC, battery, connection, speech voices, Bluetooth and Client Hints. `Random` on a setting that has nothing to draw (`oscpu`, `build_id`, `proxy`) fails `build`.

```rust
use seleniumbase_rs::{Dimension, Fingerprint, OsType, Spoof, StealthPolicy};

fn build() -> Result<(), seleniumbase_rs::FingerprintError> {
    let fp = Fingerprint::builder()
        .timezone("Europe/Berlin")
        .hardware_concurrency(8)
        .webrtc(Spoof::Off)
        .policy(StealthPolicy::native_only())
        .grant_permissions(true)
        .build()?;
    assert!(fp.is_masked(Dimension::Timezone));
    assert!(!fp.is_masked(Dimension::Screen));
    assert!(fp.policy().native_spoofing && fp.policy().grant_permissions);

    let same = Fingerprint::randomized(OsType::Macos, 7);
    assert_eq!(same, Fingerprint::randomized(OsType::Macos, 7));
    Ok(())
}
```

## FingerprintBuilder

Each setting method takes `impl Into<Spoof<T>>`. Calls can come in any order. A later call for the same setting replaces an earlier one. `build()` refuses a useless `Off`, draws every `Random` from one seed, derives `languages` from a given `locale` and `platform` from a recognised user agent when those were not set, then applies the policy.

`seed(u64)` makes the draw repeatable. Without a seed, a draw uses fresh entropy and keeps the seed it used. The operating system is the one from `os_type`, else the one named by the user agent, else Windows.

| Method | Sets |
|--------|------|
| `browser_type(BrowserType)` | Persona. Unset: from a random user agent, else Chromium. |
| `os_type(OsType)` | Operating-system persona. |
| `core_version(u32)` | Chromium major the identity claims. |
| `seed(u64)` | Seed of the draw and of deterministic noise. |
| `user_agent`, `platform`, `hardware_concurrency`, `device_memory`, `max_touch_points`, `vendor`, `oscpu`, `product_sub`, `build_id`, `app_version` | `navigator` claims. `oscpu` and `build_id` have no random value. |
| `locale`, `languages`, `accept_languages`, `timezone` | Language and time zone. |
| `screen(width, height)` | Screen size in CSS pixels (`Spoof::Value`). |
| `screen_spoof(spoof)` | Screen size from any `Spoof<ScreenSize>`, including `Random`. |
| `avail_screen`, `pixel_ratio`, `color_depth` | Available screen, device pixel ratio, colour depth. |
| `webgl` | Unmasked vendor and renderer. A `(vendor, renderer)` pair works. |
| `media_devices` / `media_device_counts(audio_in, audio_out, video_in)` | What `enumerateDevices()` lists. `Off` lists none. |
| `geolocation` | Position `navigator.geolocation` reports. `(latitude, longitude)` works. `Off` denies it. |
| `fonts`, `speech_voices`, `client_hints`, `battery`, `connection`, `bluetooth` | Those APIs. `Off` makes the API unavailable or lists nothing. |
| `proxy` | Proxy the browser is launched with. No random value. An empty host fails `build`. |
| `webrtc` | Addresses WebRTC reports. `(public_ip, local_ip)` works. `Off` makes WebRTC unavailable. |
| `local_storage(bool)`, `save_service_worker(bool)` | Keep those between sessions of the profile. |
| `custom_start_urls`, `cmd_params` | URLs opened at start, and extra command-line parameters stored on the fingerprint. |
| `humanize(HumanizeConfig)` | Humanized input timing. |
| `policy(StealthPolicy)` | Base policy. Order against the settings does not matter. |
| `with_policy(change)` | A change applied after the base, in call order. |
| `webrtc_policy`, `canvas_noise`, `audio_noise`, `native_spoofing`, `grant_permissions`, `block_trackers`, `blocked_trackers`, `blocked_ports`, `disable_csp`, `quic` | Shortcuts that change the policy. They run after `policy()`, in the order they were called. |
| `build()` | `Result<Fingerprint, FingerprintError>`. |

## StealthPolicy

The fields can be read. Change a policy with the method of the same name (it takes the policy and returns it) or through the fingerprint builder.

| Field | Default | What it does |
|-------|---------|--------------|
| `webrtc_policy` | `None` | WebRTC IP-handling policy passed at launch. `None` passes no flag unless the fingerprint sets WebRTC, which uses `WebRtcPolicy::DisableNonProxiedUdp`. |
| `canvas_noise` | `CanvasNoise::Natural` | Noise on `toDataURL`, `toBlob` and `getImageData`. |
| `audio_noise` | `AudioNoise::Natural` | Noise on audio reads. |
| `native_spoofing` | `false` | Deliver claims through CDP and launch arguments where the browser can. |
| `native_tostring` | `true` | Mask `Function.prototype.toString`. |
| `chrome_runtime` | `false` | Install `window.chrome` runtime, app, `csi` and `loadTimes` stubs. |
| `headless_fixes` | `false` | `matchMedia`, permissions consistency, page visibility and the hairline media query. |
| `grant_permissions` | `false` | Grant common permissions at start. |
| `block_trackers` | `false` | Block tracker hosts. |
| `blocked_trackers` | empty | Hosts to block. Empty means the crate's default list. |
| `blocked_ports` | empty | Ports the browser may not reach on the loopback names. |
| `quic` | `QuicMode::Natural` | How QUIC is handled. |
| `disable_csp` | `false` | Bypass Content-Security-Policy for the page context. |
| `open_shadow_roots` | `true` | Force closed shadow roots open. A page can detect this. |

| Constructor | What it adds to the default |
|-------------|-----------------------------|
| `StealthPolicy::default()` | Only `native_tostring` and `open_shadow_roots`. |
| `balanced()` | `WebRtcPolicy::DisableNonProxiedUdp`, persistent canvas and audio noise, `chrome_runtime`, `headless_fixes`. This is the preset policy. |
| `native_only()` | The same WebRTC policy and `native_spoofing`. No noise and no `window.chrome` stubs. |
| `max()` | `balanced()`, plus tracker blocking, granted permissions and `QuicMode::Disabled`. |

`CanvasNoise` is `Natural` (no noise), `Persistent` (the fingerprint seed), `Random` (a fresh seed each time the script is generated) or `Low` (seeded noise on about one pixel in eight). `AudioNoise` is `Natural`, `Persistent` or `Random`. `adds_noise()` is false only for `Natural`. `QuicMode` is `Natural`, `Disabled`, `Enabled`, `ForceHttp2` or `Auto`. `WebRtcPolicy` is `DisableNonProxiedUdp` (the default), `PublicAndPrivateInterfaces` or `PublicInterfaceOnly`.

```rust
use seleniumbase_rs::{CanvasNoise, StealthPolicy};

let policy = StealthPolicy::balanced()
    .canvas_noise(CanvasNoise::Low)
    .grant_permissions(true);
assert!(policy.native_tostring && policy.chrome_runtime);
assert_eq!(policy.canvas_noise, CanvasNoise::Low);
```

`block_trackers(false)` also clears a custom host list. `blocked_trackers` with a non-empty list turns blocking on.

## Turning a fingerprint on

| Call | What the browser does |
|------|------------------------|
| `BrowserConfig::with_fingerprint(fingerprint)` | Presents that fingerprint. Launch arguments go to every Chromium-based session. In `DriverMode::Cdp` and `DriverMode::Uc` the session also installs the page script and sends the protocol overrides. Plain `DriverMode::WebDriver` sends the launch arguments only. |
| `LaunchOptionsBuilder::fingerprint(&fingerprint)` | Pure CDP. Launch settings (user agent, language, window size, WebRTC policy, proxy) come from the fingerprint unless you set them on the builder (or read them from a config), before or after this call. The script and protocol overrides run on every tab before the first navigation. Flags in the fingerprint's `cmd_params` are not passed; add flags you trust with `arg`. |
| `sbrs --fingerprint FILE` | Global flag (`SB_FINGERPRINT`). `FILE` is the JSON that `sbrs fingerprint capture` and `sbrs fingerprint export` write. The command line wins over `SB_FINGERPRINT`, which wins over the `[fingerprint]` table of the settings file. A user agent, language or proxy named by its own option wins over the fingerprint. A Chromium browser then runs in CDP mode unless `--uc` or another mode is given. |

```rust
use seleniumbase_rs::sb_cdp::LaunchOptionsBuilder;
use seleniumbase_rs::{BrowserConfig, DriverMode, Fingerprint};

let fp = Fingerprint::windows_desktop();
let webdriver = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_fingerprint(fp.clone());
let _cdp = LaunchOptionsBuilder::default().fingerprint(&fp);
let _ = webdriver;
```
