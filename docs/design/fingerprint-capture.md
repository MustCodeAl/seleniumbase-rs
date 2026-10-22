---
title: "Design: fingerprint capture, export and import"
---

# Fingerprint capture, export and import

Status: implemented, listed under `## Unreleased` in `CHANGELOG.md`. It builds on
[Fingerprint API v2](/design/fingerprint-api-v2) and adds no setting to it.

## Why

A fingerprint says what a browser should claim. The claim that is hardest to get
wrong is one measured on a real browser, and until now the only ways to get a
fingerprint were to write the values, to draw them, or to take a preset. This adds
the other direction: read what a real browser shows a page, keep it as a
`Fingerprint`, and hand it back to any launch path.

## One file format

There is one file format everywhere: the v2 `Fingerprint` JSON of
[Fingerprint API v2](/design/fingerprint-api-v2#json). A capture writes it, `export`
writes it, `check`, `diff` and `--fingerprint` read it, and the companion
extension validates it with `schemas/fingerprint.schema.json`. `CapturedFingerprint`
is a Rust value that carries the readings next to the fingerprint; it is not a
second file format.

## What a capture reads

One embedded script (`src/stealth/js/capture.js`) reads, in the page, what a
detection script could read. The same script runs on both engines: the pure CDP
engine evaluates it as an `async` function (`Page::capture_fingerprint`), the
WebDriver engine runs it as an asynchronous script (`BaseCase::capture_fingerprint`).
Rust maps the answer in `stealth::capture`, in one place, so both engines produce the
same fingerprint from the same readings.

| Reading | Setting it answers |
|---|---|
| `navigator.userAgent`, `platform`, `hardwareConcurrency`, `deviceMemory`, `maxTouchPoints`, `vendor`, `oscpu`, `productSub`, `buildID`, `appVersion` | the settings of the same names |
| `navigator.language`, `navigator.languages` | `locale`, `languages` (and `accept_languages`, derived: see below) |
| `Intl.DateTimeFormat().resolvedOptions().timeZone` | `timezone` |
| `screen.width`/`height`, `availWidth`/`availHeight`, `colorDepth`, `devicePixelRatio` | `screen`, `avail_screen`, `color_depth`, `pixel_ratio` |
| WebGL `UNMASKED_VENDOR_WEBGL`, `UNMASKED_RENDERER_WEBGL` | `webgl` |
| `navigator.userAgentData` and its high-entropy values | `client_hints`; no API reads `Off` |
| `enumerateDevices()` counts | `media_devices`; no API reads `Off` |
| `getBattery()`, `navigator.connection`, `speechSynthesis.getVoices()`, `navigator.bluetooth` | `battery`, `connection`, `speech_voices`, `bluetooth`; an API that is not there reads `Off` |

An answered setting is a `Value` (or `Off` for an API that is not there). A setting
the page could not answer stays `Natural`. The persona comes from the user agent
(`os_type`, `browser_type`) and `core_version` from its Chrome major, so the
fingerprint claims the Chrome that was measured. The policy is
`StealthPolicy::balanced()`, as in every preset.

`accept_languages` cannot be read by a page, but Chrome builds the header from
`navigator.languages` by a fixed rule (`en-US,en;q=0.9`), so a Chromium capture sets
it by that rule. Without it the user agent override would send the locale alone, a
tell. It is the one derived setting, and `compare` counts it as unverifiable.

The readings that no setting holds go to `CapturedFingerprint::unmapped`: window
sizes, `navigator.webdriver`, the plugin and mime-type counts, permission states,
a canvas hash, an audio hash and the headless markers. They are for people and for
checks, never applied.

## Library

```rust
// stealth::capture
pub struct CapturedFingerprint { pub fingerprint: Fingerprint, pub unmapped: Map<String, Value>, pub probe: Probe }
pub struct Probe { pub url: String, pub captured_at: String, pub browser_version: Option<String> }
CapturedFingerprint::from_probe(raw: &Value, captured_at: impl Into<String>) -> Result<Self, SeleniumBaseError>

// the engines
Page::capture_fingerprint(&self) -> Result<CapturedFingerprint, SeleniumBaseError>      // sb_cdp
BaseCase::capture_fingerprint(&self) -> Result<CapturedFingerprint, SeleniumBaseError>  // WebDriver

// stealth::fingerprint
Fingerprint::load(path) / save(path) -> Result<_, SeleniumBaseError>   // save is atomic
Fingerprint::from_json(&str) / to_json_pretty(&self)
Fingerprint::diff(&self, other: &Fingerprint) -> Vec<SettingDiff>      // SettingDiff { setting, expected, actual }
Fingerprint::compare_to(&self, observed: &Fingerprint) -> FingerprintComparison
Page::compare_fingerprint(&self, expected: &Fingerprint) / BaseCase::compare_fingerprint(..)
```

`save` writes a temporary file in the target folder, flushes it and renames it over
the target, so a reader never sees half a file; the file is private to its owner
(mode `0600`), because a fingerprint may hold proxy credentials. `load` refuses a
file above 4 MiB.

### diff and compare

`a.diff(&b)` reads `a` as what is expected and `b` as what is found. A setting
differs when `a` gives it (anything but `Natural`) and `b` holds another. A setting
`a` leaves `Natural` is never compared: the browser's own value agrees with any
value. Language lists compare without their quality values (`en-US,en;q=0.9` equals
`en-US,en`), a WebGL half that `a` leaves out is not compared, and media devices
compare by count.

`compare_to` is the check "does this live page present this fingerprint": capture,
then diff, with two differences. A setting the fingerprint claims that the capture
could not answer (`accept_languages`, `geolocation`, `fonts`, `proxy`, `webrtc`, and
any reading that failed) is listed as **unverifiable**, never as a match. And the
battery and the connection compare by presence, because their numbers move.
`FingerprintComparison::presents()` is true when nothing differs, and
`is_verified()` when nothing differs and nothing is unverifiable.

## JSON Schema

`schemas/fingerprint.schema.json` is generated from the types with `schemars`
(a dev-dependency; the derive is on only in the library's own tests). A test writes
the schema in memory and compares it with the file, so a change to a setting that
leaves the file stale fails; `SB_UPDATE_SCHEMAS=1 cargo test -p seleniumbase-rs
--lib schema` rewrites it. Every preset and a drawn identity are validated against
the schema in the same test.

## Command line

```
sbrs fingerprint capture [URL] [-o FILE] [--readings FILE]
sbrs fingerprint export (--preset NAME | --random [--seed N] [--os OS]) [-o FILE]
sbrs fingerprint check FILE
sbrs fingerprint diff A B
sbrs fingerprint presets
sbrs --fingerprint FILE <command that starts a browser>
```

- `capture` starts a browser as `sbrs open` does (`--headless`, `--browser`, and the
  rest apply), opens `URL` (a blank page by default), and prints the fingerprint JSON,
  or writes it to `FILE`. A summary goes to standard error. `--readings FILE` also
  writes the unmapped readings and the probe.
- `export` writes a preset or a seeded random identity (for `--os`, `windows`,
  `macos`, `linux`, `android` or `ios`; default this machine's).
- `check` loads a file and prints `validate()`: the errors, the warnings and the
  verdict. Exit status 2 when there is an error or the file does not load.
- `diff A B` prints each differing setting and exits 1 when any differs, 0 when none
  does (`A` is the expected side, as above).
- `--fingerprint FILE` applies a file to every command that starts a browser. The
  same file can be named by `SB_FINGERPRINT` or by a `[fingerprint]` table
  (`file = "..."`) in the settings file; the flag wins over the variable, which wins
  over the file. A fingerprint without `--uc` or `--cdp` would reach a Chromium page
  through its launch arguments only, so the flag selects the CDP mode for a Chromium
  browser when no mode was asked for. `sbrs devtools` applies it through the pure
  CDP launch options.

To check that a browser presents a file: `sbrs --fingerprint f.json capture -o seen.json`,
then `sbrs fingerprint diff f.json seen.json`.

## Not done

- A capture reads one page in one browser at one moment; it is not proof that the
  fingerprint is undetectable. The canvas and audio hashes are readings, and the
  claims of the policy (canvas noise, say) change them when the fingerprint is
  applied.
- Fonts, WebRTC addresses, the position and the proxy are not read. A permission
  prompt, a network request to a service and a font-measurement sweep were left out
  on purpose: each would change what is being measured or show the measurement.
- The seleniumbase-rs-stealth repository gets no code from this change. Its
  `StealthSession` takes a fingerprint with `.fingerprint(..)`; the core documents
  how to build one from a capture.
