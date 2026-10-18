---
title: "Browser Profiles"
---

`seleniumbase-rs` can import external browser profile payloads in a generic
anti-detect JSON format. The format is exposed through the
`seleniumbase_rs::profile_payloads` module and the Tauri profile-manager example.

This is **not** tied to any third-party product. The payload describes a browser
persona, a set of masking modes, and optional runtime overrides. `seleniumbase-rs`
translates the parts it can apply directly into `BrowserConfig`, command-line
flags, CDP calls, and window settings.

## What you will learn

- The structure of a profile payload.
- How the words of a flag map to `BrowserConfig` and `Fingerprint`.
- How to load a profile programmatically.
- How to use the Tauri profile-manager example.

## Supported fields

The `ProfileParams` type mirrors a common `POST /profile/create` body:

- `name`, `browser_type` (`chromium`, `firefox`, or `mobile_safari`), `os_type`
- `folder_id`, `tags`, `notes`, `times`
- `core_version`, `core_minor_version`, `auto_update_core`
- `parameters.flags` — masking mode for WebRTC, audio, fonts, geolocation,
  graphics, navigator, ports, proxy, screen, timezone, canvas noise, QUIC, and
  startup behavior.
- `parameters.fingerprint` — explicit values for navigator, localization,
  timezone, graphics, WebRTC, media devices, screen, geolocation, ports, fonts,
  and extra command-line parameters.
- `parameters.storage` — local vs cloud storage options.
- `parameters.proxy` — HTTP/HTTPS/SOCKS proxy with optional credentials and
  traffic saving.
- `parameters.custom_start_urls` — up to 5 URLs to open on launch.

<Note>

**Backward compatibility:** The parser still accepts the legacy strings
`mimic` and `stealthfox`, but they are treated as aliases for `chromium` and
`firefox`. New profiles should use the generic names.

</Note>

## Complete field reference

### Top-level profile fields

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Profile name. |
| `browser_type` | `chromium` \| `firefox` \| `mobile_safari` | yes | Browser engine. Legacy aliases `mimic`/`stealthfox` still work. |
| `os_type` | `linux` \| `macos` \| `windows` \| `android` \| `ios` | yes | Operating-system persona. |
| `automation` | `selenium` \| `playwright` \| `puppeteer` | no | Automation backend. `selenium` is default; `playwright`/`puppeteer` map to CDP mode. |
| `is_headless` | boolean | no | Run the browser headlessly. Default `false`. |
| `folder_id` | string | no | Persistent folder identifier for local storage. |
| `core_version` | number | no | The Chromium major version. A drawn user agent and drawn Client Hints claim it (not for Firefox or iOS Safari personas). |
| `core_minor_version` | number | no | Browser minor version (informational). |
| `auto_update_core` | boolean | no | Whether to auto-update the core version. |
| `tags` | string[] | no | Arbitrary tags. |
| `times` | number | no | Usage counter. Default `1`. |
| `notes` | string | no | Human-readable notes. |

### `parameters`

| Field | Type | Required | Description |
|---|---|---|---|
| `flags` | object | yes | One word or literal per key (see below). |
| `fingerprint` | object | yes | Explicit fingerprint values. A value is used unless the flag for it says `natural`, or `off` for the keys that can be off. |
| `storage` | object | yes | `is_local` and `save_service_worker`. |
| `proxy` | object | no | Proxy configuration. |
| `custom_start_urls` | string[] | no | URLs opened on launch. Maximum **5**; extras are ignored. |

### `parameters.flags`

The masking flags (`webrtc_masking`, `proxy_masking`, `geolocation_popup`,
`audio_masking`, `navigator_masking`, `localization_masking`,
`timezone_masking`, `graphics_masking`, `fonts_masking`,
`media_devices_masking`, `screen_masking`, `ports_masking`,
`geolocation_masking`, `canvas_noise` and `quic_mode`) take the words and the
literal formats in the one table in
[Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#keywords-in-json-payloads).
A flag the payload leaves out is `auto`, except `ports_masking` (`mask`: the ports
of `fingerprint.ports` are blocked), `canvas_noise` (seeded noise) and `quic_mode`
(`disabled`). The rest:

| Flag | Accepted values | Required | Notes |
|---|---|---|---|
| `graphics_noise` | `natural`, `mask` | no | Also accepts `low`, `medium`, `high`, `off`. Ignored: it never drove graphics noise. |
| `disable_csp` | `true`, `false` | no | Bypass Content-Security-Policy for the page context. |
| `grant_permissions` | `true`, `false` | no | Grant common browser permissions at startup so prompts do not fire. |
| `startup_behavior` | `recover`, `custom` | no | `recover` restores last-session tabs; `custom` opens `custom_start_urls`. |
| `native_spoofing` | `true`, `false` | no | When `true`, spoofing is applied through CDP/launch args instead of JS where possible, making it invisible to page-side inspection. |

### `parameters.fingerprint`

| Sub-field | Fields | Required when flag is `custom` | Notes |
|---|---|---|---|
| `navigator` | `hardware_concurrency`, `device_memory`, `user_agent`, `platform`, `os_cpu` | `hardware_concurrency`, `user_agent`, `platform` | `os_cpu` is optional; mostly relevant for Firefox personas. |
| `localization` | `languages`, `locale`, `accept_languages` | all | Pass an empty string for `languages`/`locale` if you only need `accept_languages`. |
| `timezone` | `zone` | yes | IANA zone such as `America/New_York`. |
| `graphic` | `vendor`, `renderer`, `vendor_id`, `renderer_id` | `vendor`, `renderer` | `vendor_id`/`renderer_id` are optional GPU identifiers attached to the WebGL context. |
| `webrtc` | `public_ip` | yes | Also supports `local_ip` via the flag literal. |
| `media_devices` | `audio_outputs`, `audio_inputs`, `video_inputs` | all | |
| `screen` | `width`, `height`, `pixel_ratio` | all | `width` 360–5000, `height` 640–3000, `pixel_ratio` 1.0–5.0. |
| `geolocation` | `accuracy`, `altitude`, `longitude`, `latitude` | all except `accuracy` | |
| `ports` | number[] | no | List of ports to block/allow. |
| `fonts` | string[] | no | List of installed fonts. |
| `cmd_params` | `{ params: [{ flag, value }] }` | no | Extra Chromium command-line flags. |
| `max_touch_points` | number | no | Android/iOS only; default `5`. |

### `parameters.proxy`

| Field | Type | Required | Description |
|---|---|---|---|
| `type` | `http` \| `https` \| `socks5` | yes | Proxy scheme. |
| `host` | string | yes | Proxy host. IPv6 addresses should **not** include brackets when supplied in this object (use them only in `proxy_masking` URL literals). |
| `port` | number | yes | Proxy port. |
| `username` | string | no | Basic-auth username. |
| `password` | string | no | Basic-auth password. |
| `save_traffic` | boolean | no | When `true`, disables image/video loading to save proxy bandwidth. Default `false`. |

## The words of a flag

A flag is a word or a literal value. Every key reads the same words, and each
setting ends in one of the four states of a `Spoof`. One table in
[Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#keywords-in-json-payloads)
lists, for every key, the words that give `Natural`, `Random`, a value and `Off`,
and the format of the literal it reads. A value in `fingerprint.*` is the switch:
unless the flag says `natural` (or `off` for a key that can be off), the setting
uses the value.

An unknown word, or a literal not in the format of its key, is logged and read as
`natural`; it is never guessed at. A profile gets the same drawn values on every
launch, because its folder and name seed the draw.

### Concrete mask-mode examples

<Tabs>

<Tab title="A user agent and platform">

```json
{
  "name": "custom-ua-profile",
  "browser_type": "chromium",
  "os_type": "windows",
  "parameters": {
    "fingerprint": {
      "navigator": {
        "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
        "platform": "Win32",
        "hardware_concurrency": 8,
        "device_memory": 8
      }
    }
  }
}
```

The values turn the navigator on by themselves: the launcher passes the supplied
`user_agent` to `--user-agent` and the `platform` value into the evasion
bootstrap. Add `"navigator_masking": "natural"` to the flags to keep the
browser's own navigator whatever the profile holds, or `"mask"` to draw the
settings the profile does not give (the memory, the touch points, the vendor)
from the same machine.

</Tab>

<Tab title="A screen, a position and a time zone">

```json
{
  "name": "berlin-desktop",
  "browser_type": "chromium",
  "os_type": "windows",
  "parameters": {
    "fingerprint": {
      "screen": { "width": 1920, "height": 1080, "pixel_ratio": 1 },
      "geolocation": { "latitude": 52.52, "longitude": 13.405, "accuracy": 100 },
      "timezone": { "zone": "Europe/Berlin" }
    }
  }
}
```

The screen applies the exact resolution. The geolocation emits
`Emulation.setGeolocationOverride` with the supplied coordinates and, because a
position answers the geolocation popup with `allow`, grants the permission. The
time zone is `Europe/Berlin`; a flag set to `mask` with no time zone in the
profile would draw one that matches the rest of the profile.

</Tab>

<Tab title="iOS Mobile Safari">

This profile uses `mobile_safari` and `ios`.

```json
{
  "name": "iphone-consumer",
  "browser_type": "mobile_safari",
  "os_type": "ios",
  "core_version": 26,
  "parameters": {
    "flags": {
      "webrtc_masking": "natural",
      "native_spoofing": true
    },
    "fingerprint": {
      "navigator": {
        "user_agent": "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1",
        "platform": "iPhone",
        "hardware_concurrency": 4,
        "device_memory": 8
      },
      "screen": { "width": 390, "height": 844, "pixel_ratio": 3 },
      "timezone": { "zone": "America/New_York" },
      "geolocation": { "latitude": 40.758, "longitude": -73.9855, "accuracy": 20 },
      "graphic": { "vendor": "Apple Inc.", "renderer": "Apple GPU" }
    }
  }
}
```

Use `browser_type: mobile_safari` and `os_type: ios` to activate iOS personas.
`native_spoofing: true` pushes the user agent, platform, locale, and screen
metrics through CDP so page-side JS cannot detect the override.

</Tab>

</Tabs>

## Literal value shortcuts

Most masking flags also accept a concrete literal string instead of a word. The
parser reads the flag as `mask` with that value and writes the value into the
fingerprint so it is applied automatically. The format of each literal is in the
Value column of the
[table of keywords](/tutorials/fingerprint_stealth#keywords-in-json-payloads).

### Full literal-value profile

```json
{
  "name": "US_Ecom_Buyer_01",
  "browser_type": "chromium",
  "folder_id": "folder_88a2b1",
  "os_type": "windows",
  "core_version": 133,
  "parameters": {
    "flags": {
      "audio_masking": "noise",
      "fonts_masking": "Arial,Helvetica,Open Sans,Roboto,Segoe UI,Times New Roman",
      "geolocation_masking": "40.7128,-74.0060,10.5",
      "geolocation_popup": "allow",
      "graphics_masking": "Intel Inc.~Intel(R) Iris(R) Xe Graphics~WebGL 2.0",
      "graphics_noise": "medium",
      "localization_masking": "en-US,en;q=0.9",
      "media_devices_masking": "audioinput:default:Mic1|videoinput:default:Cam1",
      "navigator_masking": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
      "ports_masking": "block:80,443,3000,8080,9222",
      "proxy_masking": "socks5://user123:pass456@192.168.1.50:1080",
      "screen_masking": "1920x1080x24",
      "quic_mode": "disabled",
      "timezone_masking": "America/New_York",
      "webrtc_masking": "public:192.168.1.50|local:10.0.0.5",
      "canvas_noise": "random"
    }
  }
}
```

The parser extracts every literal value above and populates the matching
`Fingerprint` fields, so the stealth bootstrap and launch args can use them
without any extra code.

#### A proxy

```json
{
  "name": "proxy-profile",
  "browser_type": "chromium",
  "parameters": {
    "proxy": {
      "type": "http",
      "host": "proxy.example.com",
      "port": 8080,
      "username": "alice",
      "password": "secret",
      "save_traffic": false
    }
  }
}
```

The `parameters.proxy` block becomes
`--proxy-server=http://alice:secret@proxy.example.com:8080`: `proxy_masking`
defaults to `auto`, which uses a proxy that has a host. Set
`proxy_masking: natural` (or `direct`) to launch without a proxy although the
block is there.

#### Native-level spoofing (`native_spoofing: true`)

```json
{
  "name": "native-spoof",
  "browser_type": "chromium",
  "os_type": "windows",
  "parameters": {
    "flags": {
      "navigator_masking": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 …",
      "screen_masking": "1920x1080x24",
      "timezone_masking": "America/New_York",
      "geolocation_masking": "40.7580,-73.9855,10.5",
      "native_spoofing": true
    }
  }
}
```

When `native_spoofing` is `true`, the user agent, screen size, timezone, and
geolocation are applied through CDP/launch args rather than JavaScript. Page
scripts see the fake values as native browser properties with no JS override to
inspect.

#### WebRTC disabled / masked

```json
{
  "parameters": {
    "flags": { "webrtc_masking": "mask" }
  }
}
```

`webrtc_masking: mask` forces `--force-webrtc-ip-handling-policy=disable_non_proxied_udp`
so public IP leakage is reduced, and draws the addresses WebRTC reports. The
default, `auto`, passes the same policy and reports no address of its own unless
the profile holds a WebRTC IP. `webrtc_masking: natural` leaves the browser's
default policy in place, and `disabled` (or `off`, `block`) makes WebRTC
unavailable.

To spoof specific public/local IPs, use the literal `public:IP|local:IP` format
or the `fingerprint.webrtc.public_ip` field:

```json
{
  "parameters": {
    "flags": { "webrtc_masking": "public:172.56.21.89|local:10.0.0.5" },
    "fingerprint": {
      "webrtc": { "public_ip": "172.56.21.89" }
    }
  }
}
```

## Mapping to SeleniumBase concepts

Not every anti-detect flag has a direct WebDriver/Chrome capability. The Rust
port applies what it can and preserves the rest as metadata:

| Profile field | Applied via |
|---|---|
| `browser_type` | `Browser::Chrome` or `Browser::Firefox` |
| `os_type == android` | mobile emulation (`max_touch_points`, mobile UA) |
| `automation` | `DriverMode::WebDriver` for `selenium`; `DriverMode::Cdp` for `playwright`/`puppeteer` |
| `is_headless` | `BrowserConfig.headless` |
| `fingerprint.navigator.user_agent` | `--user-agent` argument |
| `fingerprint.navigator.hardware_concurrency` | `navigator.hardwareConcurrency` override |
| `fingerprint.navigator.device_memory` | `navigator.deviceMemory` override |
| `fingerprint.navigator.os_cpu` | `navigator.oscpu` override (Firefox) |
| `fingerprint.max_touch_points` | `navigator.maxTouchPoints` override |
| `fingerprint.localization.locale` | `--lang` argument |
| `parameters.proxy` | `--proxy-server` argument |
| `parameters.proxy.save_traffic` | `ad_block` / image-blocking launch args |
| `fingerprint.graphic.vendor_id` / `renderer_id` | attached to WebGL context for introspection scripts |
| `fingerprint.screen` | `BaseCase::set_window_size` at runtime |
| `fingerprint.geolocation` | `Emulation.setGeolocationOverride` CDP call |
| `fingerprint.cmd_params` | extra Chromium arguments |
| `parameters.custom_start_urls` | first URL is used as `start_page`; extras are opened at runtime (max 5) |

Flags such as canvas noise, font masking, WebRTC masking, and idle-time behavior
are stored in the profile and exposed to the stealth bootstrap, custom CDP
scripts, extensions, or future anti-detect injection features.

## Programmatic usage

```rust
use seleniumbase_rs::profile_payloads::ProfileParams;
use serde_json::json;

let raw = json!({
    "name": "custom-profile",
    "browser_type": "chromium",
    "os_type": "windows",
    "parameters": {
        "flags": {
            "webrtc_masking": "mask"
        },
        "fingerprint": {
            "navigator": {
                "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ...",
                "platform": "Win32",
                "hardware_concurrency": 8
            },
            "screen": { "width": 1920, "height": 1080, "pixel_ratio": 1 }
        },
        "storage": { "is_local": true }
    }
});

let params: ProfileParams = serde_json::from_value(raw).unwrap();
let config = params.to_browser_config("http://localhost:4444");

let mut sb = seleniumbase_rs::BaseCase::new(config).await.unwrap();
params.apply_runtime_overrides(&mut sb).await.unwrap();
```

## Tauri profile-manager app

The `examples/tauri-profile-manager` app accepts a profile JSON payload in the
**Import profile JSON** section. It converts the payload into a local profile
and launches it with the converted `BrowserConfig`. Profiles imported this way
show an `external` badge in the profile list.

## Further reading

- [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth)
- [BrowserLeaks](https://browserleaks.com/)

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Profile fails to parse | Unknown browser or OS type | Use `chromium`/`firefox`/`mobile_safari` and `linux`/`macos`/`windows`/`android`/`ios`. |
| A value is ignored | The flag for it is `natural` (or `off`, for the keys that can be off) | Set the flag to `auto` or `mask`, or leave it out. |
| A flag does nothing | The word is not one of the words above; it is logged and read as `natural` | Check the log for the keyword, and use a word from the table. |
| Proxy not applied | `proxy_masking` is `natural` or `disabled`, or the proxy has no host | Leave `proxy_masking` at `auto` (or set `custom`) and provide `parameters.proxy` with a host. |
| Native spoofing has no effect | Driver is in pure WebDriver mode | Use `DriverMode::Cdp` or `DriverMode::Uc`. |
