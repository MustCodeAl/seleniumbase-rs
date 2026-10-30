---
title: "Browser Profiles"
sidebarTitle: "Browser profiles"
---

`seleniumbase-rs` can import external browser profile payloads in a generic
anti-detect JSON format. The format is exposed through the
`seleniumbase_rs::profile_payloads` module and the Tauri profile-manager example.

This is **not** tied to any third-party product. The payload describes a browser
persona, a set of masking modes, and optional runtime overrides. `seleniumbase-rs`
translates the parts it can apply directly into `BrowserConfig`, command-line
flags, <Tooltip tip="Chrome DevTools Protocol: the protocol DevTools uses to control Chrome.">CDP</Tooltip> calls, and window settings.

```mermaid
flowchart LR
  P["Profile payload (JSON)"] --> S["ProfileParams"]
  S --> C["to_browser_config: BrowserConfig"]
  S --> R["apply_runtime_overrides: CDP calls and window settings"]
  C --> B["Browser"]
  R --> B
```

*From a payload to a running browser.*

<Info>

**What you will learn**

- The structure of a profile payload.
- How the words of a flag map to `BrowserConfig` and `Fingerprint`.
- How to load a profile programmatically.
- How to use the Tauri profile-manager example.

</Info>

<Visibility for="agents">

Summary for AI exports: a profile payload (`ProfileParams`, from `seleniumbase_rs::profile_payloads`) describes a browser persona with `name`, `browser_type`, `os_type`, `parameters.flags` (a word or literal per key), `parameters.fingerprint` (explicit values), `parameters.storage` and `parameters.proxy`. `params.to_browser_config(url)` builds the `BrowserConfig` and `apply_runtime_overrides(&mut sb)` applies the CDP and window parts. An unknown flag word is logged and read as `natural`.

</Visibility>

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

<ParamField body="name" type="string" required>

Profile name.

</ParamField>

<ParamField body="browser_type" type="chromium | firefox | mobile_safari" required>

Browser engine. Legacy aliases `mimic`/`stealthfox` still work.

</ParamField>

<ParamField body="os_type" type="linux | macos | windows | android | ios" required>

Operating-system persona.

</ParamField>

<ParamField body="automation" type="selenium | playwright | puppeteer" default="selenium">

Automation backend. `selenium` is default; `playwright`/`puppeteer` map to CDP mode.

</ParamField>

<ParamField body="is_headless" type="boolean" default="false">

Run the browser headlessly.

</ParamField>

<ParamField body="folder_id" type="string">

Persistent folder identifier for local storage.

</ParamField>

<ParamField body="core_version" type="number">

The Chromium major version. A drawn user agent and drawn Client Hints claim it (not for Firefox or iOS Safari personas).

</ParamField>

<ParamField body="core_minor_version" type="number">

Browser minor version (informational).

</ParamField>

<ParamField body="auto_update_core" type="boolean">

Whether to auto-update the core version.

</ParamField>

<ParamField body="tags" type="string[]">

Arbitrary tags.

</ParamField>

<ParamField body="times" type="number" default="1">

Usage counter.

</ParamField>

<ParamField body="notes" type="string">

Human-readable notes.

</ParamField>

### `parameters`

<ParamField body="flags" type="object" required>

One word or literal per key (see below).

</ParamField>

<ParamField body="fingerprint" type="object" required>

Explicit fingerprint values. A value is used unless the flag for it says `natural`, or `off` for the keys that can be off.

</ParamField>

<ParamField body="storage" type="object" required>

`is_local` and `save_service_worker`.

</ParamField>

<ParamField body="proxy" type="object">

Proxy configuration.

</ParamField>

<ParamField body="custom_start_urls" type="string[]">

URLs opened on launch. Maximum **5**; extras are ignored.

</ParamField>

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

<ParamField body="graphics_noise" type="natural | mask">

Also accepts `low`, `medium`, `high`, `off`. Ignored: it never drove graphics noise.

</ParamField>

<ParamField body="disable_csp" type="true | false">

Bypass Content-Security-Policy for the page context.

</ParamField>

<ParamField body="grant_permissions" type="true | false">

Grant common browser permissions at startup so prompts do not fire.

</ParamField>

<ParamField body="startup_behavior" type="recover | custom">

`recover` restores last-session tabs; `custom` opens `custom_start_urls`.

</ParamField>

<ParamField body="native_spoofing" type="true | false">

When `true`, spoofing is applied through CDP/launch args instead of JS where possible, making it invisible to page-side inspection.

</ParamField>

### `parameters.fingerprint`

Fields marked required are needed when the flag for the group is `custom`.

<ParamField body="navigator" type="object">

<Expandable title="fields">

<ParamField body="hardware_concurrency" required />

<ParamField body="device_memory" />

<ParamField body="user_agent" required />

<ParamField body="platform" required />

<ParamField body="os_cpu">

Optional; mostly relevant for Firefox personas.

</ParamField>

</Expandable>

</ParamField>

<ParamField body="localization" type="object">

Pass an empty string for `languages`/`locale` if you only need `accept_languages`.

<Expandable title="fields">

<ParamField body="languages" required />

<ParamField body="locale" required />

<ParamField body="accept_languages" required />

</Expandable>

</ParamField>

<ParamField body="timezone" type="object">

<Expandable title="fields">

<ParamField body="zone" required>

IANA zone such as `America/New_York`.

</ParamField>

</Expandable>

</ParamField>

<ParamField body="graphic" type="object">

<Expandable title="fields">

<ParamField body="vendor" required />

<ParamField body="renderer" required />

<ParamField body="vendor_id">

Optional GPU identifier attached to the WebGL context.

</ParamField>

<ParamField body="renderer_id">

Optional GPU identifier attached to the WebGL context.

</ParamField>

</Expandable>

</ParamField>

<ParamField body="webrtc" type="object">

Also supports `local_ip` via the flag literal.

<Expandable title="fields">

<ParamField body="public_ip" required />

</Expandable>

</ParamField>

<ParamField body="media_devices" type="object">

<Expandable title="fields">

<ParamField body="audio_outputs" required />

<ParamField body="audio_inputs" required />

<ParamField body="video_inputs" required />

</Expandable>

</ParamField>

<ParamField body="screen" type="object">

<Expandable title="fields">

<ParamField body="width" required>

360–5000.

</ParamField>

<ParamField body="height" required>

640–3000.

</ParamField>

<ParamField body="pixel_ratio" required>

1.0–5.0.

</ParamField>

</Expandable>

</ParamField>

<ParamField body="geolocation" type="object">

<Expandable title="fields">

<ParamField body="accuracy" />

<ParamField body="altitude" required />

<ParamField body="longitude" required />

<ParamField body="latitude" required />

</Expandable>

</ParamField>

<ParamField body="ports" type="number[]">

List of ports to block/allow.

</ParamField>

<ParamField body="fonts" type="string[]">

List of installed fonts.

</ParamField>

<ParamField body="cmd_params" type="{ params: [{ flag, value }] }">

Extra Chromium command-line flags.

</ParamField>

<ParamField body="max_touch_points" type="number" default="5">

Android/iOS only.

</ParamField>

### `parameters.proxy`

<ParamField body="type" type="http | https | socks5" required>

Proxy scheme.

</ParamField>

<ParamField body="host" type="string" required>

Proxy host. IPv6 addresses should **not** include brackets when supplied in this object (use them only in `proxy_masking` URL literals).

</ParamField>

<ParamField body="port" type="number" required>

Proxy port.

</ParamField>

<ParamField body="username" type="string">

Basic-auth username.

</ParamField>

<ParamField body="password" type="string">

Basic-auth password.

</ParamField>

<ParamField body="save_traffic" type="boolean" default="false">

When `true`, disables image/video loading to save proxy bandwidth.

</ParamField>

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

<Tab title="A user agent and platform" icon="id-card">

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

<Tab title="A screen, a position and a time zone" icon="location-dot">

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

<Tab title="iOS Mobile Safari" icon="mobile">

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
use seleniumbase_rs::json::json;

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

let params: ProfileParams = seleniumbase_rs::json::from_value(raw).unwrap();
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

<Columns cols={2}>

<Card color="#1a85d4" title="Fingerprint & Stealth Profiles" icon="fingerprint" href="/tutorials/fingerprint_stealth">

The table of keywords for every flag.

</Card>

<Card color="#199f43" title="BrowserLeaks" icon="globe" href="https://browserleaks.com/" />

</Columns>

## Troubleshooting

<AccordionGroup>

<Accordion title="Profile fails to parse" icon="triangle-exclamation">

**Likely cause.** Unknown browser or OS type.

**Fix.** Use `chromium`/`firefox`/`mobile_safari` and `linux`/`macos`/`windows`/`android`/`ios`.

</Accordion>

<Accordion title="A value is ignored" icon="eye-slash">

**Likely cause.** The flag for it is `natural` (or `off`, for the keys that can be off).

**Fix.** Set the flag to `auto` or `mask`, or leave it out.

</Accordion>

<Accordion title="A flag does nothing" icon="flag">

**Likely cause.** The word is not one of the words above; it is logged and read as `natural`.

**Fix.** Check the log for the keyword, and use a word from the table.

</Accordion>

<Accordion title="Proxy not applied" icon="network-wired">

**Likely cause.** `proxy_masking` is `natural` or `disabled`, or the proxy has no host.

**Fix.** Leave `proxy_masking` at `auto` (or set `custom`) and provide `parameters.proxy` with a host.

</Accordion>

<Accordion title="Native spoofing has no effect" icon="shield">

**Likely cause.** Driver is in pure WebDriver mode.

**Fix.** Use `DriverMode::Cdp` or `DriverMode::Uc`.

</Accordion>

</AccordionGroup>
