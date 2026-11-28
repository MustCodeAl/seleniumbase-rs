---
title: "Mimic Specimens & Persona Synthesis"
sidebarTitle: "Mimic & randomize"
description: "API reference for seleniumbase_rs_stealth::identity::fingerprint::{mimic, randomize}: catalogued Specimen lookups and deterministic Chromium/Edge/Firefox tables."
---

<Badge color="green">Pure Rust (zero BoringSSL)</Badge> <Badge color="green">no unsafe</Badge>

`identity::fingerprint::mimic` models real-world browser, machine, place, and rendering baselines, while `identity::fingerprint::randomize` provides the deterministic persona tables behind `Fingerprint::randomized` and `Spoof::Random`.

## Recorded specimens (`identity::fingerprint::mimic`)

`Specimen` has four typed lookup keys:

| Type | Example lookup | Purpose |
|---|---|---|
| `Build` | `Build::named("chrome-156-windows")?` | Browser version, User-Agent, and Client Hints baseline |
| `Machine` | `Machine::named("macbook-13-m1")?` | Screen size, GPU (`WebGl`), hardware concurrency, and device memory |
| `Place` | `Place::named("de-de-berlin")?` | Locale, language list, IANA timezone, and geolocation |
| `Rendering` | `Rendering::named(...)` | Recorded drawing output for a specific build and machine |

```rust
use seleniumbase_rs_stealth::identity::fingerprint::mimic::{
    Build, Machine, Place, Specimen, UnknownSpecimen,
};
use seleniumbase_rs_stealth::identity::fingerprint::spoof::Spoof;
use seleniumbase_rs_stealth::identity::Fingerprint;

fn build_from_specimens() -> Result<Fingerprint, Box<dyn std::error::Error>> {
    let build = Build::named("chrome-156-windows")?;
    let machine = Machine::named("windows-uhd-620")?;
    let place = Place::named("de-de-berlin")?;

    let fp = Fingerprint::builder()
        .user_agent(Spoof::mimic(Specimen::Build(build)))
        .webgl(Spoof::mimic(Specimen::Machine(machine)))
        .timezone(Spoof::mimic(Specimen::Place(place)))
        .build()?;
    Ok(fp)
}
```

Unknown names return `UnknownSpecimen` (`err.name()`, `err.kind()`) listing valid candidates. Verified recordings also expose `Provenance` (`recorded`, `tool`, `browser`, `headless`, `host`).

## Deterministic tables (`identity::fingerprint::randomize`)

| Item | Signature / constant | Role |
|---|---|---|
| `CURRENT_STABLE_MAJOR` | `const u32` (`157`) | Current Chromium major targeted by synthesis |
| `PRESET_CHROMIUM_MAJOR` | `const u32` | Chromium major shared across built-in presets |
| `KNOWN_FULL_VERSIONS` | `const [KnownFullVersion; 4]` | Verified milestone builds (`FullVersionSource`, `known_full_version(major)`) |
| `chromium_user_agent_for_os` | `(OsType, u32) -> String` | Realistic Chromium UA for Windows, macOS, Linux, Android |
| `edge_user_agent` / `edge_user_agent_for_os` | `(&str / OsType, u32) -> String` | Coherent Edge desktop User-Agent |
| `firefox_user_agent` | `(OsType, u32) -> String` | Coherent Firefox UA alongside `gecko_oscpu` and `gecko_build_id` |
| `chromium_hints` / `edge_hints` | `(u32) -> ClientHints` | Matching `Sec-CH-UA` brand lists and full version metadata |
| `locale_of_timezone` | `(&str) -> Option<&'static Locale>` | Look up coherent locale/languages for an IANA timezone |

```rust
use seleniumbase_rs_stealth::identity::fingerprint::randomize::{
    chromium_hints, chromium_user_agent_for_os, known_full_version, locale_of_timezone,
    CURRENT_STABLE_MAJOR,
};
use seleniumbase_rs_stealth::identity::OsType;

let ua = chromium_user_agent_for_os(OsType::Macos, CURRENT_STABLE_MAJOR);
let hints = chromium_hints(CURRENT_STABLE_MAJOR);
let full = known_full_version(CURRENT_STABLE_MAJOR);
let locale = locale_of_timezone("America/New_York");

assert!(ua.contains("Macintosh"));
assert!(!hints.brands.is_empty());
assert!(full.is_some());
assert!(locale.is_some());
```
