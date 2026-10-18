---
title: "ADR 003: Stealth Scraping Layout"
---

> **Moved:** on 2026-10-15 (version 0.3.0) this code moved to [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth) (crates `seleniumbase-rs-wire`, `-scrape`, `-impersonate`); the paths below are the ones it had here.

## Status
Accepted (2026-10-11). `HttpSession`, `MitmProxy` and `StealthSession` have since been merged into the
layout below; their public API is in the plan, not in this ADR.

## Context
ADR 001 covers what a page can read inside the browser. Stealth scraping adds the
wire: the TLS `ClientHello`, the HTTP/2 connection start and the header order,
plus the layer that keeps an identity, its cookies and its exit address together
while a scraper works through a site's defences. The design is in
[the plan](/design/stealth_parity_plan). Its first draft grouped the files
by hexagonal layer (`scrape/domain`, `scrape/adapters`). On 2026-10-11 the tree
was regrouped by concern, with the work packages and the type names unchanged.
This record states the layout as it is in the code and the rules that hold it.

## Decision

1. **Two opt-in features, and nothing else in the default build.** `scrape` is pure
   Rust and adds only `psl` (the Public Suffix List, for cookie domains).
   `impersonate` includes `scrape` and adds wreq and btls, which build BoringSSL
   (cmake and a C/C++ toolchain), with `hyper`, `brotli` and a few small crates. No
   default build gains a dependency or C code.

2. **Modules by concern, not by layer.**
   - `src/wire` (compiled with either feature): `ja3`, `ja4`, `akamai`, `ids`,
     `client_hello`, `tls_spec`, `h2_spec`, `h2_frames`, `header_spec` and
     `emulation`, with `BrowserKind` in `wire/mod.rs` and the measured JSON in
     `wire/measured`.
   - `src/scrape` (feature `scrape`): `challenge` (`detect`, `markers`,
     `mitigation`), `egress` (`route`, `pool`, `geo`, `redact`), `jar`
     (`store`, `set_cookie`), `ledger` (`state`, `events`), `passport`
     (`identity`, `policy`), `error`, and `runtime`.
   - `src/scrape/runtime` holds everything that does I/O: the `BrowserScraper`, the
     CDP leg, the egress switch, the JSON state store, the identity file, the
     tracing sink and the capture tools' readers. The other directories hold only
     pure code and the ports it needs (`StateStore`, `EventSink`, `GeoResolver`;
     `EgressSwitch` and `BrowserLauncher` sit in `runtime`).
   - `src/impersonate` (feature `impersonate`): `convert` and `client` (the only two
     files that name wreq types, so a change in the wreq release candidate stays in
     them), `capture_h2`, `tls_server`, `http_session/` (`HttpSession`), `mitm/` (the
     MITM proxy) and `stealth_session/` (`StealthSession`).

3. **Pure modules are enforced by a test.** `tests/scrape_domain_purity.rs` parses
   every file under `src/wire` and `src/scrape/{challenge,egress,jar,ledger,passport}`
   with `syn` and fails on any use of `tokio`, `std::fs`, `std::net`,
   `std::process`, `std::time` clock types, or `Instant::now` and `SystemTime::now`
   (also behind a rename). From `sb_cdp` it allows only the data types `Proxy`,
   `Cookie`, `SameSite` and `Captcha` and the `sync::locked` helper. Functions that
   need the time take `now` as an argument. The check is syntactic: it sees the
   names a file writes, not code reached through a type alias or a dependency, and
   its self-tests show it rejects an impure source and accepts a clean one. The
   gate is compiled only with the `scrape` feature.

4. **Measured JSON instead of hand-typed values.** Every TLS, HTTP/2, header and
   client-hint value is written by a capture tool (`capture_tls`, `capture_hints`,
   `capture_h2`) against a real browser, stored in `src/wire/measured/<browser>-<major>-<os>.json`
   together with its raw evidence, embedded with `include_str!` and parsed once. An
   unmeasured browser and system pair is `EmulationError::NotMeasured`, never a
   fallback to different values. Today that is Chrome 155.0.8059.40 on macOS. What
   BoringSSL or wreq cannot send is a named `KnownGap` in the file or a named
   refusal in `ImpersonateError` before a client exists.

5. **No dependency on other stealth crates.** Nothing from `stealthscraper-rs`,
   `impersonate-rs`, `impcurl`, `curl-impersonate-cli` or `wreq-util` is a
   dependency, optional or not. Their ideas informed the design; the code and the
   numbers are ours or measured here. wreq (a release candidate) and btls are pinned to
   exact versions, and the BoringSSL they link allows one copy in the dependency
   graph.

6. **House rules for the new modules.** One `thiserror` enum per module, mapped to
   `SeleniumBaseError::Stealth` at the crate boundary (`ScrapeError`). URLs travel as
   `RedactedUrl` and proxies as `EgressKey`, so a credential cannot reach a log,
   event, error or state file by construction. `#![forbid(unsafe_code)]` on the new
   modules, the pedantic lint set `sb_cdp` and `mcp` already use, and `tracing` for
   logging. Files that hold secrets (the identity file, the state file) are written
   atomically with owner-only permissions.

7. **Re-exports.** The main types of each layer are re-exported at the top of `wire`,
   `scrape` and `impersonate`, and a curated set at the crate root behind the same
   features. The `SpecError` of `wire::tls_spec` and the one of `wire::h2_spec` are
   re-exported as `TlsSpecError` and `H2SpecError`, so no two exported types share a
   name. The stealth layer is experimental and not a stable API.

## Consequences
- Positive: A concern is found by name, and its pure code is separate from the code
  that does I/O, so the pure part can be tested without a browser or a socket.
- Positive: The purity rule is checked by a test, not by convention.
- Positive: A wire value cannot drift from what a browser sent, because it is read
  from the capture and the capture's evidence is kept beside it.
- Positive: The default build and the default dependency graph are unchanged.
- Negative: The measured data covers one browser on one system. Every other pair is
  refused until someone captures it, and a Chrome update leaves the profile behind
  until the capture tools are run again.
- Negative: The `impersonate` feature needs cmake and a C/C++ toolchain, and pinned
  release-candidate dependencies that can change API.
- Negative: The purity check is syntactic and can be defeated by an alias. Review is
  still needed.
- Negative: The plan's older paths (`scrape/domain/...`, `scrape/adapters/...`) still
  appear in sections 2.1 and 5 of the plan until it is rewritten; the map at the top
  of its section 2 is the authority.

## Not decided here
The public API of `HttpSession`, `MitmProxy` and `StealthSession` is not recorded
in this ADR; see sections 3.11 to 3.13 of the plan and the tutorial. Measuring
more browsers and systems is open.

---

Verified against commit 4184e74 and the re-exports added after it (layout: `src/lib.rs`, `src/wire/mod.rs`,
`src/scrape/mod.rs`, `src/impersonate/mod.rs`; purity rules:
`tests/scrape_domain_purity.rs`; features and pins: `Cargo.toml`; measured data:
`src/wire/emulation.rs`, `src/wire/measured/chrome-155-macos.json`).
