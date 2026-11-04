---
title: "Identities and profiles"
sidebarTitle: "Identities and profiles"
---

An identity is the whole set of claims one browser makes. You can draw one
from a seed ([Randomised identities](/stealth/fingerprints#randomised-identities)),
load one from a profile file another tool wrote (below), or keep a whole
browser profile (user data directory, cookies, settings) between runs with
[Browser profiles](/tutorials/browser_profiles). This page covers the profile
payloads and the check that tells you whether the claims fit together. The
identity file of the stealth repository, which ties a fingerprint to cookies
and a session policy for scraping, is in
[Cookies, identity and challenges](/scraping/advanced-stealthy/identity).

## External profile payloads

The `profile_payloads::ProfileParams` type maps to a `Fingerprint` via
`ProfileParams::to_fingerprint()`. `ProfileParams::to_browser_config(webdriver_url)`
returns a `BrowserConfig` with that fingerprint attached.

The returned `mode` follows the payload's `automation` field: `playwright` and
`puppeteer` give `DriverMode::Cdp`; `selenium` (the default when the field is
absent) and the Firefox browser types give `DriverMode::WebDriver`; anything else
gives `DriverMode::Uc`. The script and protocol overrides are applied only in
`Cdp` and `Uc` mode, so a payload that keeps `selenium` is launched with the launch
arguments only. The payload's `cmd_params` become Chrome flags only when the switch is
on the crate's allowlist of fingerprint and quiet-browser switches (`lang`,
`user-agent`, `window-size`, `disable-features`, `force-webrtc-ip-handling-policy`,
`disable-quic`, ...); anything else, from `--renderer-cmd-prefix` to
`--remote-allow-origins` or `--disable-web-security`, is refused and logged, named
or not. The Pure CDP engine does not pass `cmd_params` at all.

The payload keeps its own format, because other tools write it: the flags are
strings (`ProfileFlags`) and the nested sections (`navigator`, `screen`, ...)
hold values. Every key reads the same words and ends in one of the four states of
a `Spoof`. The profile's `folder_id` and `name` seed the draws, so a profile gets
the same drawn values on every launch, and its `core_version` is the Chromium
major that a drawn user agent and drawn Client Hints claim (not for the Firefox
and Safari personas, whose core is not a Chrome version).

### Words every key reads

Keywords are case-insensitive. A flag the payload leaves out is `auto`. The
table below is the one place that lists the words and the literal format of each
key; this is what the groups of words do:

- The **natural** words give the browser's own value, whatever values the payload
  holds for it.
- **`auto`** (or no flag) gives the value the payload holds for the setting, and
  the browser's own where it holds none.
- The **mask** words give the value the payload holds, and a coherent drawn one
  where it holds none.
- The **off** words turn the API off. Only where an API can be off (geolocation,
  WebRTC, media devices); for any other key they are read as `Natural`, and logged.
- **A literal** is the value, in the format of its key. The other settings of the
  same key then follow the mask words: a held value, else drawn.

An unknown keyword is logged and read as `Natural`. So is a literal that is not in
the format of its key (`wibble` for a time zone, a user agent that does not open
with `Mozilla/` and a digit). It is never guessed at: it used to switch the
dimension on and fill it with generic values.

### Keywords in JSON payloads

One table for every key. The first row says what the words mean; each later row
names only what the key adds or does differently, and the literal formats it
reads.

| Payload key | Natural | Random | Value (literal) | Off |
|---|---|---|---|---|
| (the words above) | `natural`, `native`, `pass_through`, `whitelist`; `auto` with no value held | `mask`, `spoof`, `custom`, `generic`, `random`, `noise` | a value of the key | `off`, `disabled`, `block`, `empty` |
| `navigator_masking` | the common words | the common words, `desktop_chrome`, `mobile_safari`: user agent, platform, cores, memory, touch points and vendor | a user agent: `Mozilla/` and a digit, with the platform in parentheses (`Mozilla/5.0 (Windows NT 10.0; Win64; x64) ...`); it sets `user_agent` | read as Natural |
| `localization_masking` | the common words | the common words: locale, languages, accept-languages of the place | a locale (`de-DE`, `zh-Hant-TW`) or an `Accept-Language` list that opens with one (`en-US,en;q=0.9`): the first tag is the `locale`, the tags the `languages`, the text the `accept_languages` | read as Natural |
| `timezone_masking` | the common words | the common words: the place's zone | an IANA name, area and city: `America/New_York` | read as Natural |
| `screen_masking` | the common words | the common words: size, pixel ratio and colour depth of the machine | `1920x1080` or `1920x1080x24` (24 when omitted); it sets `screen` and `color_depth` | read as Natural |
| `graphics_masking` | the common words | the common words: an adapter the OS reports | `vendor~renderer`, both given: `Intel Inc.~Intel(R) Iris(R) Xe Graphics` | read as Natural |
| `fonts_masking` | the common words | the common words, `system_only`, `blacklist`: the system's fonts | a comma-separated list of fonts: `Arial,Calibri,Consolas`. One font is no list: give it in `fingerprint.fonts` | read as Natural |
| `media_devices_masking` | the common words | the common words | `2_mic_1_cam`, or `kind:id:label\|kind:id:label` (`audioinput:default:Mic1\|videoinput:default:Cam1`): the counts and the labelled devices | the off words: no device is listed |
| `geolocation_masking` | the common words | the common words: a spot in the place's city | `lat,lon` or `lat,lon,altitude` (accuracy 20): `40.7128,-74.0060,10.5` | the off words: the request is denied |
| `webrtc_masking` | the common words (and no IP policy) | the common words: a public and a local address | `public:IP\|local:IP`, IPv6 in brackets: `public:172.56.21.89\|local:10.0.0.5`; it also passes the policy `PublicAndPrivateInterfaces` | the off words: WebRTC is unavailable |
| `proxy_masking` | the common words, `direct`: no proxy | none: `mask`, `custom`, `socks5`, `http`, `https` use the payload's proxy section, as `auto` does | a `socks5://`, `http://` or `https://` URL: `socks5://user:pass@host:1080` (port 1080 for SOCKS5, 8080 otherwise) | read as Natural, and logged: a proxy has no off state |
| `geolocation_popup` | `prompt`: ask, whatever position the payload holds | not used | not used | `block`: deny, whatever position the payload holds; `allow` and `auto` follow `geolocation_masking` |
| `audio_masking` | the common words and the off words: no audio noise | `random`: fresh noise for each script; `mask`, `spoof`, `custom`, `generic`, `noise`: noise seeded by the profile | not used | not used |
| `canvas_noise` | `natural`, `auto`, `off`, `disabled`: no canvas noise | `random`: fresh noise for each script | `low`: noise on one pixel in eight; `persistent`, `mask`, `spoof`, `custom`, `generic`, `noise`: seeded noise. If the key is absent: seeded noise | not used |
| `ports_masking` | the common words, the off words: nothing blocked | the common words, `block`, `block_all`, `stealth`: the ports of the payload's `fingerprint.ports` are blocked | `block:80,443` or `80,443`: those ports (the policy's `blocked_ports`) | not used |
| `quic_mode` | `natural`, `auto` | not used | `enabled`, `disabled`, `force_http2`. If the key is absent: `disabled` | not used |

The audio, canvas, ports and QUIC keys set the policy and not a setting, so their
"states" are the values of the policy field in the same columns.
`graphics_noise` and `startup_behavior` are accepted and ignored. The booleans
`disable_csp`, `grant_permissions` and `native_spoofing` set the same fields of
the policy.

The settings of one key follow the word together: `navigator_masking: "mask"`
with a `navigator` section that gives a user agent, a platform and a core count
keeps those three and draws the memory, the touch points and the vendor, in the
same machine. A key whose draw cannot be made (a random locale next to a time
zone the table of places lacks) is logged and left `Natural`.

A literal in a flag is the value for the setting it names (the Value column
above). A section of the payload that gives the same setting wins over a literal,
and a section value that is an empty string is a value the payload does not give.
What the browser is launched with (`to_browser_config`) and what the runtime
overrides set (`apply_runtime_overrides`) are the built fingerprint's: a `natural`
flag launches without a `--user-agent`, a locale or a proxy, whatever the payload
holds.

`webrtc_masking` also sets the IP-handling policy that keeps WebRTC from leaking
the real address: every word but the natural ones passes `disable_non_proxied_udp`
(including a payload that says nothing), `public_only` passes
`default_public_interface_only`, and a literal address passes
`default_public_and_private_interfaces`.

### Payloads from older versions

A payload written for an older version loads unchanged, with the words above. The
differences you can see: an absent flag is `auto` (it was `mask` for the
navigator, localization, time zone, graphics, fonts, screen, geolocation and
WebRTC flags, which filled in generic values nobody asked for); `off` and
`disabled` turn geolocation, WebRTC and media devices off instead of leaving them
natural; an unknown word is `Natural` instead of `mask`.

## Profile coherence validation

`Fingerprint::validate()` returns a `CoherenceReport` with `errors` and
`warnings`; `is_coherent()` is true when there are no errors. Only values that
were given are checked: a `Natural` setting is the browser's own.

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
- a time zone with no geolocation (a geolocation that is `Off` is a choice, and
  does not warn);
- a locale from another country than the time zone, judged with the table of
  places `randomize::LOCALES` (a zone the table does not hold, or a locale with
  no region, is not judged).

It does not look at hardware.

```rust
use seleniumbase_rs_stealth::Fingerprint;

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
(`0`, `u64::MAX`, `1 << 63` and others). If you edit a generated profile with
`to_builder()`, run `validate()` again afterwards.

