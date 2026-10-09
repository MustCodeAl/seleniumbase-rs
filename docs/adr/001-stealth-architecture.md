# ADR 001: Stealth and Fingerprinting Architecture

## Status
Accepted

## Context
Python SeleniumBase provides multiple undetected/stealth modes. The Rust port must support a similar capability without relying on Python-specific patches.

## Decision
Implement a layered stealth stack:

1. **Chrome binary patching** (`src/stealth/patcher.rs`) removes `$cdc_` and other automation markers from the chromedriver executable.
2. **Launch arguments and preferences** (`StealthOptions`) disable automation indicators and harden the browser profile.
3. **CDP overrides** (`src/stealth/evasions.rs`) send `Network.setUserAgentOverride`, `Emulation.setDeviceMetricsOverride`, and `Emulation.setLocaleOverride` at session start.
4. **JavaScript providers** (`src/stealth/providers/`) patch runtime fingerprints for navigator, screen, WebGL, fonts, media devices, WebRTC, and plugins.
5. **Profiles** (`src/profile_payloads/`) let users declare a target persona and derive concrete fingerprints from it.
6. **Native vs. JS spoofing** is toggled by `StealthFlags::native_spoofing`; when enabled, CDP covers dimensions that the browser already exposes via Client Hints.

## Consequences
- Positive: Modular providers are easy to extend and test independently.
- Positive: Native spoofing reduces the amount of injected JavaScript and improves performance.
- Negative: Maintaining parity with evolving anti-bot systems requires ongoing provider updates.
