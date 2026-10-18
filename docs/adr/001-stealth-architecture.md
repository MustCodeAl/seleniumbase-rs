---
title: "ADR 001: Stealth and Fingerprinting Architecture"
sidebarTitle: "001: Stealth architecture"
---

## Status
Accepted

## Context
Python SeleniumBase provides multiple undetected/stealth modes. The Rust port must support a similar capability without relying on Python-specific patches.

## Decision
Implement a layered stealth stack:

1. **Binary patching** (`src/stealth/patcher.rs`) removes `$cdc_` and other automation markers from the chromedriver executable (`ChromedriverPatcher`). `ChromeBinaryPatcher` does the same kind of work on a cached copy of the Chrome binary, for native spoofing (see 7).
2. **Launch arguments and preferences** (`StealthOptions`) disable automation indicators and harden the browser profile.
3. **CDP overrides** (`src/stealth/evasions.rs`) build, from a `Fingerprint`, the parameters of `Network.setUserAgentOverride`, `Emulation.setDeviceMetricsOverride` and `Emulation.setLocaleOverride` (and the timezone, geolocation and similar overrides) in `safe_cdp_overrides` (`cdp_overrides` is deprecated: it also builds the proxy credential header). `BrowserSession::apply_fingerprint` sends them at session start, together with the session script registered through `Page.addScriptToEvaluateOnNewDocument`. The pure CDP engine (`sb_cdp::identity`) sends the screen of a desktop persona as `Emulation.setDeviceMetricsOverride` with a `0` by `0` viewport plus `screenWidth` and `screenHeight`, so the browser reports the screen itself and the page keeps the viewport its window has.
4. **JavaScript providers** (`src/stealth/providers/`) patch runtime fingerprints. Each of the built-in providers (`default_registry()` holds 29) covers one dimension: navigator, screen, WebGL, canvas and audio noise, fonts, media devices, WebRTC, plugins, timezone and more. Providers patch prototypes rather than instances and start their snippet with `SB_PREAMBLE`, whose helpers make a replaced function report a native name, length and `[native code]` source. `bootstrap_script` and the Playwright session run `default_registry()`. The WebDriver sessions in UC and CDP mode and the pure CDP engine run `session_registry()` (`evasions::session_script`), which is the default without `plugins` and `connection` (27 providers): a real Chrome already has `navigator.plugins`, `mimeTypes`, the PDF viewer flag and `navigator.connection` and answers for them itself, so a replaced getter would be one more thing a page can find that a real Chrome does not have. `navigator_props` no longer sets `pdfViewerEnabled`, and `client_hints` defines `navigator.userAgentData` only in a secure context, which is the only place Chrome has it.
5. **One setting, four states.** Every setting of a `Fingerprint` is one `Spoof<T>`: `Natural` (the browser's own value, the default), `Random` (a coherent value drawn when the fingerprint is built), `Value(T)` (a plain value converts to it) or `Off` (the API is unavailable or denied, for the settings where that means something). The value and the intent are one field, so they cannot disagree, and a built fingerprint holds no `Random` and is never mutated (`to_builder()` starts the next one). `Fingerprint::is_masked(Dimension)` is the one predicate the providers, the launch arguments, the protocol overrides and the proxy ask. How the claims are delivered is a separate `StealthPolicy`, set with `policy`, `with_policy` and shortcuts in any order; a policy never changes a setting. See `docs/design/fingerprint-api-v2.md`.
6. **Profiles** (`src/profile_payloads/`) translate an external anti-detect profile payload into a `BrowserConfig` carrying a concrete `Fingerprint`.
7. **Native vs. JS spoofing** is toggled by `StealthPolicy::native_spoofing`. When it is on, the CDP overrides and launch arguments carry the values the browser can supply itself (user agent and Client Hints, screen size, locale), and the providers skip the JavaScript patch for them. For example, `window_geometry` then only patches `colorDepth` and `pixelDepth`. When `BrowserConfig::browser_binary_path` is unset, `BrowserSession` also patches a cached copy of the system Chrome binary (`SB_CHROME_BIN` or the one found on the system) before launch and uses it.

The pure CDP engine (`src/sb_cdp`, see ADR 002) does not use chromedriver, so layer 1's chromedriver patch does not apply to it. It reuses `safe_cdp_overrides` (the `cdp_overrides` without the proxy credential header) for layer 3 and `session_registry` for layer 4.

This stack works on what a page can read inside the browser. The TLS handshake, the HTTP/2 connection start and the header order are outside it; ADR 003 records how the separate stealth scraping layer handles them.

## Consequences
- Positive: Modular providers are easy to extend and test independently.
- Positive: Native spoofing reduces the amount of injected JavaScript and improves performance.
- Negative: Maintaining parity with evolving anti-bot systems requires ongoing provider updates.
