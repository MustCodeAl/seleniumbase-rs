# ADR 001: Stealth and Fingerprinting Architecture

## Status
Accepted

## Context
Python SeleniumBase provides multiple undetected/stealth modes. The Rust port must support a similar capability without relying on Python-specific patches.

## Decision
Implement a layered stealth stack:

1. **Binary patching** (`src/stealth/patcher.rs`) removes `$cdc_` and other automation markers from the chromedriver executable (`ChromedriverPatcher`). `ChromeBinaryPatcher` does the same kind of work on a cached copy of the Chrome binary, for native spoofing (see 6).
2. **Launch arguments and preferences** (`StealthOptions`) disable automation indicators and harden the browser profile.
3. **CDP overrides** (`src/stealth/evasions.rs`) build, from a `Fingerprint`, the parameters of `Network.setUserAgentOverride`, `Emulation.setDeviceMetricsOverride` and `Emulation.setLocaleOverride` (and the timezone, geolocation and similar overrides) in `cdp_overrides`. `BrowserSession::apply_fingerprint` sends them at session start, together with the bootstrap script registered through `Page.addScriptToEvaluateOnNewDocument`.
4. **JavaScript providers** (`src/stealth/providers/`) patch runtime fingerprints. Each of the built-in providers (`default_registry()` holds 29) covers one dimension: navigator, screen, WebGL, canvas and audio noise, fonts, media devices, WebRTC, plugins, timezone and more. Providers patch prototypes rather than instances and start their snippet with `SB_PREAMBLE`, whose helpers make a replaced function report a native name, length and `[native code]` source.
5. **Profiles** (`src/profile_payloads/`) translate an external anti-detect profile payload into a `BrowserConfig` carrying a concrete `Fingerprint`.
6. **Native vs. JS spoofing** is toggled by `StealthFlags::native_spoofing`. When it is on, the CDP overrides and launch arguments carry the values the browser can supply itself (user agent and Client Hints, screen size, locale), and the providers skip the JavaScript patch for them. For example, `window_geometry` then only patches `colorDepth` and `pixelDepth`. When `BrowserConfig::browser_binary_path` is unset, `BrowserSession` also patches a cached copy of the system Chrome binary (`SB_CHROME_BIN` or the one found on the system) before launch and uses it.

The pure CDP engine (`src/sb_cdp`, see ADR 002) does not use chromedriver, so layer 1's chromedriver patch does not apply to it. It reuses `bootstrap_script` and `cdp_overrides` for layers 3 and 4.

## Consequences
- Positive: Modular providers are easy to extend and test independently.
- Positive: Native spoofing reduces the amount of injected JavaScript and improves performance.
- Negative: Maintaining parity with evolving anti-bot systems requires ongoing provider updates.
