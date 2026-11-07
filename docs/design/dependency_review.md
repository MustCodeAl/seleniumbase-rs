---
title: "Design: dependency review of the five repositories"
sidebarTitle: "Dependency review"
---

Status: review note, written before any change, on branch `lane/deps-review` (from `404208aa`, 2026-11-06). Nothing was compiled: the facts come from `cargo metadata`, `cargo tree -d`, `cargo outdated -R`, `cargo update --dry-run`, `cargo audit`, `cargo deny check`, the crates.io API, `bun outdated` and `bun audit`, and from the manifests' own comments. What could not be checked says "to verify". The note covers core and the repositories that depend on it: stealth, the plugins, the Tauri plugin and the browser companion. Two workspaces that sit inside them and have their own lockfiles are covered too: the HTTP/3 helper `crates/h3d` of stealth and the Tauri example app.

## What was checked, and how to read the tables

* A **direct dependency** is a crate named in `[dependencies]`, `[dev-dependencies]` or `[build-dependencies]` of any workspace member; path dependencies on the workspace's own crates are left out. Kind: `n` normal, `d` dev, `b` build, `opt` only optional.
* **Used** is the version in `Cargo.lock`. **Newest** is the highest stable release on crates.io (the highest release at all where the repository itself uses a pre-release).
* **Last release** is the date of the newest release that is not yanked, with a label against 2026-11-06: active (6 months or less), slow (6 to 18 months), quiet (more than 18 months). **Owners** counts the users and teams that crates.io lists, which says something about how many people could lose interest, and no more.
* **Pulls in** is the number of packages reachable from the crate through normal and build edges, with every feature of the workspace and every target (`cargo metadata`). `cargo tree` for the host target shows fewer. **MSRV** is the `rust-version` the used release declares.
* **Verdict**: keep, upgrade, replace (name what takes its place), remove (unused or replaceable with the standard library or a test fixture), or trim features. "upgrade (lock)" is a newer release inside the range of the manifest: `cargo update` takes it. "upgrade (breaking)" needs a manifest edit. The verdict follows from the data in the tables, and from the comments in the manifests; anything it rests on that was not read in the code says "to verify".
* `cargo udeps` needs a nightly toolchain and a full build, so it was **not run**: a crate that no code names would still show up as keep here. It is a step of the first upgrade lane.
* Duplicates are counted twice: crate names present in more than one version in the `cargo metadata` resolve (every target, so Windows-only copies show), and the `multiple-versions` warnings of `cargo deny`. The two differ (core 77 against 59) and the cause was not traced: to verify. `cargo tree -d` was not used for the count, since it also lists one version twice when its features differ.

## Summary

| Repository | Direct dependencies | Behind: lock / breaking | Replace or remove | Quiet upstream | Duplicated crates (metadata / `cargo deny`) | Vulnerabilities / allowed warnings (`cargo audit`) |
|---|---|---|---|---|---|---|
| Core: `seleniumbase-rs` | 71 | 7 / 1 | 6 | 7 | 77 / 59 | 0 / 5 |
| Stealth: `seleniumbase-rs-stealth` | 27 | 2 / 0 | 2 | 1 | 8 / 7 | 0 / 0 |
| Plugins: `seleniumbase-rs-plugins` | 16 | 1 / 0 | 0 | 1 | 9 / 7 | 0 / 1 |
| Tauri plugin: `tauri-plugin-sbrs-automation` | 15 | 0 / 3 | 1 | 1 | 28 / 25 | 0 / 0 |
| The HTTP/3 helper: `crates/h3d` of stealth | 13 | 0 / 1 | 0 | 2 | own workspace | 0 / 0 |
| The Tauri example app (`examples/tauri-profile-manager/src-tauri`) | 18 | n/a | n/a | n/a | own workspace | 17 / 17 |
| The companion (`seleniumbase-rs-companion`, TypeScript on Bun) | 5, all dev | 3 behind, none breaking | 0 | n/a | n/a | 1 high (`bun audit`) |

Two findings need action before any of the planned upgrade lanes: the Tauri example's lockfile has 17 open vulnerabilities, and the companion's `happy-dom` has one (both below).

## The ten actions that matter most, in order

1. **Refresh the Tauri example's lockfile** (`cargo update` in `examples/tauri-profile-manager/src-tauri`, `Cargo.lock` is tracked): 17 vulnerabilities, in `actix-http`, `actix-web`, `rustls`, `rustls-webpki`, `webpki-roots`, `webpki-root-certs`, `zstd`, `zstd-safe`, `zstd-sys`, `ctutils` and `h2`. `cargo update --dry-run` moves every one of them to a fixed release except `h2` 0.3.27 (it comes with `actix-http`; to verify after the update).
2. **Update `happy-dom` in the companion** from 21.0.1 to 21.0.4 (high: unsanitised import alias leads to host code execution, GHSA-834v-8h66-3h79; a dev dependency of the tests). `bun audit fix`.
3. **Refresh the lockfiles** with `cargo update`: core 28 packages locked (including `toml` 1.2.1, `zip` 9.4.1, `rmcp` 3.9.0, `usage-rs` 7.3.0, `flate2` 1.1.11), stealth 23, the plugins 3, the Tauri plugin 10. One lane each; core's goes through the GitLab pipeline.
4. **Drop `chrono`** for `jiff`: core's `mcp::devtools` is the last user (the manifest says so), and stealth's scrape and wire crates use it for timestamps. One clock crate across the family.
5. **Remove the `printpdf` dev dependency** from core: 155 crates, the unmaintained `bincode` 1, and a second `lopdf`, all for the fixtures of one test module (`src/api/pdf.rs:286`).
6. **Run `cargo udeps`** (nightly) on core, stealth and the plugins, and remove what it reports. Not done here.
7. **Decide the `full-tracing` feature**: `tracing-timing` (0.7.0, July 2025, the newest) pulls the unmaintained `fxhash`. Drop the feature if nobody uses it.
8. **Make the family publishable**: version numbers on the path dependencies (14 wildcard warnings in the plugins, 4 in stealth), a published plugin API, then core, then stealth (git dependencies and order below). Then turn `bans.wildcards` in the three new `deny.toml` files from `warn` to `deny`.
9. **Bump the core pins that lag**: stealth's git dependency is locked to core 0.7.0 and the Tauri plugin names tag `v0.7.0`; core is 0.8.0.
10. **Watch four upstream releases** on every lock refresh: `pdf-extract` (to leave `lopdf` 0.42, which holds `ttf-parser` and a second crypto stack), `ocrs` (to allow `rten` 0.27), `rustwright` (to drop `reqwest` 0.12 and `tokio-tungstenite` 0.29 from the tree) and `turso` (a stable 0.9 on a newer `bindgen`).

## Core: `seleniumbase-rs`

The workspace has four packages: the library `seleniumbase-rs`, the command line `crates/cli`, the tests-only `crates/own-imports` and the Playwright bridge `crates/playwright`. The Cargo.lock holds 869 packages. `cargo deny --workspace check` passes (advisories, bans, licenses and sources ok, 59 duplicate warnings) and `cargo audit` reports no vulnerability and five allowed warnings (all unmaintained crates, below).


Facts (one row per direct dependency):

| Crate | Kind | Used | Newest | Licence | Last release | Owners | MSRV | Pulls in |
|---|---|---|---|---|---|---|---|---|
| `async-trait` | n | 0.1.92 | 0.1.92 | MIT OR Apache-2.0 | 2026-08-08 (active) | 1 | 1.71 | 4 |
| `axum` | n opt | 0.8.9 | 0.8.9 | MIT | 2026-04-14 (slow) | 4 | 1.80 | 73 |
| `base64` | n | 0.23.1 | 0.23.1 | MIT OR Apache-2.0 | 2026-08-04 (active) | 2 | 1.71.0 | 0 |
| `bytes` | n | 1.12.1 | 1.12.1 | MIT | 2026-07-08 (active) | 3 | 1.57 | 7 |
| `chrono` | n | 0.4.45 | 0.4.45 | MIT OR Apache-2.0 | 2026-06-04 (active) | 2 | 1.62.0 | 48 |
| `console-subscriber` | n opt | 0.5.0 | 0.5.0 | MIT | 2025-10-30 (slow) | 3 | 1.74.0 | 118 |
| `crossterm` | n opt | 0.29.0 | 0.29.0 | MIT | 2025-04-05 (quiet) | 1 | 1.63.0 | 40 |
| `dhat` | d | 0.3.3 | 0.3.3 | MIT OR Apache-2.0 | 2024-02-04 (quiet) | 1 | - | 38 |
| `dirs` | n | 7.0.0 | 7.0.0 | MIT OR Apache-2.0 | 2026-09-05 (active) | 1 | - | 13 |
| `divan` | d | 0.1.21 | 0.1.21 | MIT OR Apache-2.0 | 2025-04-10 (quiet) | 2 | 1.80.0 | 20 |
| `enigo` | n opt | 0.6.1 | 0.6.1 | MIT | 2025-08-28 (slow) | 2 | 1.85 | 46 |
| `figment` | n | 0.10.19 | 0.10.19 | MIT OR Apache-2.0 | 2024-05-17 (quiet) | 1 | - | 12 |
| `flate2` | n | 1.1.10 | 1.1.11 | MIT OR Apache-2.0 | 2026-11-06 (active) | 1 | 1.67.0 | 6 |
| `futures-util` | n | 0.3.34 | 0.3.34 | MIT OR Apache-2.0 | 2026-08-11 (active) | 1 | 1.71 | 13 |
| `http` | n | 1.5.0 | 1.5.0 | MIT OR Apache-2.0 | 2026-07-29 (active) | 2 | 1.57.0 | 9 |
| `httpmock` | d | 0.9.0 | 0.9.0 | MIT | 2026-10-20 (active) | 1 | 1.88.0 | 103 |
| `image` | n opt | 0.25.10 | 0.25.10 | MIT OR Apache-2.0 | 2026-03-10 (slow) | 4 | 1.88.0 | 23 |
| `image-compare` | n opt | 0.5.0 | 0.5.0 | MIT | 2025-08-18 (slow) | 1 | - | 33 |
| `indicatif` | n | 0.18.6 | 0.18.6 | MIT | 2026-07-01 (active) | 2 | 1.85 | 31 |
| `insta` | d | 1.49.0 | 1.49.0 | Apache-2.0 | 2026-10-03 (active) | 2 | 1.66.0 | 39 |
| `jiff` | n | 0.2.39 | 0.2.39 | Unlicense OR MIT | 2026-11-02 (active) | 1 | 1.70 | 21 |
| `miette` | n | 7.6.0 | 7.6.0 | Apache-2.0 | 2025-04-27 (quiet) | 1 | 1.70.0 | 39 |
| `object_store` | n opt | 0.14.2 | 0.14.2 | MIT/Apache-2.0 | 2026-09-15 (active) | 5 | 1.85 | 226 |
| `ocrs` | n opt | 0.13.1 | 0.13.1 | MIT OR Apache-2.0 | 2026-09-13 (active) | 1 | - | 44 |
| `pdf-extract` | n opt | 0.12.1 | 0.12.1 | MIT | 2026-09-16 (active) | 1 | - | 90 |
| `printpdf` | d | 0.12.8 | 0.12.8 | MIT | 2026-09-05 (active) | 1 | 1.88 | 155 |
| `proc-macro2` | d | 1.0.107 | 1.0.107 | MIT OR Apache-2.0 | 2026-07-19 (active) | 1 | 1.71 | 1 |
| `rand` | n | 0.10.3 | 0.10.3 | MIT OR Apache-2.0 | 2026-09-20 (active) | 2 | 1.85 | 28 |
| `rand_distr` | n | 0.6.0 | 0.6.0 | MIT OR Apache-2.0 | 2026-02-10 (slow) | 2 | 1.85 | 32 |
| `ratatui` | n opt | 0.30.2 | 0.30.2 | MIT | 2026-06-19 (active) | 4 | 1.88.0 | 199 |
| `regex` | dn | 1.13.1 | 1.13.1 | MIT OR Apache-2.0 | 2026-07-15 (active) | 3 | 1.65 | 4 |
| `reqwest` | n | 0.13.5 | 0.13.5 | MIT OR Apache-2.0 | 2026-09-08 (active) | 1 | 1.85.0 | 187 |
| `rfd` | n opt | 0.17.2 | 0.17.2 | MIT | 2026-10-22 (active) | 1 | - | 77 |
| `ring` | n | 0.17.14 | 0.17.14 | Apache-2.0 AND ISC | 2025-03-11 (quiet) | 1 | 1.66.0 | 43 |
| `rmcp` | n opt | 3.8.0 | 3.9.0 | Apache-2.0 | 2026-11-05 (active) | 2 | 1.88 | 223 |
| `roxmltree` | d | 0.21.2 | 0.21.2 | MIT OR Apache-2.0 | 2026-10-24 (active) | 2 | 1.60 | 1 |
| `rpassword` | n | 7.5.4 | 7.5.4 | Apache-2.0 | 2026-05-31 (active) | 1 | 1.85 | 4 |
| `rstest` | n opt | 0.28.0 | 0.28.0 | MIT OR Apache-2.0 | 2026-10-18 (active) | 1 | 1.85.0 | 19 |
| `rstest-bdd` | dn | 0.6.0 | 0.6.0 | ISC | 2026-09-14 (active) | 1 | 1.88 | 136 |
| `rstest-bdd-macros` | n opt | 0.6.0 | 0.6.0 | ISC | 2026-09-14 (active) | 1 | 1.88 | 125 |
| `rten` | n opt | 0.26.0 | 0.27.0 | MIT OR Apache-2.0 | 2026-10-02 (active) | 1 | 1.94.0 | 39 |
| `rustls` | n | 0.23.46 | 0.23.46 | Apache-2.0 OR ISC OR MIT | 2026-10-14 (active) | 3 | 1.71 | 57 |
| `rustwright` | n | 0.3.0 | 0.3.0 | MIT | 2026-09-30 (active) | 1 | - | 192 |
| `schemars` | d | 1.2.3 | 1.2.3 | MIT | 2026-10-12 (active) | 1 | 1.74 | 62 |
| `scraper` | n | 0.29.0 | 0.29.0 | ISC | 2026-10-21 (active) | 3 | - | 51 |
| `seleniumbase-rs-plugin` | n opt | 0.1.3 (git) | n/a | MIT | n/a | n/a | 1.88 | 19 |
| `seleniumbase-rs-plugins-builtin` | n opt | 0.1.3 (git) | n/a | MIT | n/a | n/a | 1.88 | 154 |
| `serde` | n | 1.0.229 | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 (active) | 2 | 1.56 | 6 |
| `serde_json` | n | 1.0.154 | 1.0.154 | MIT OR Apache-2.0 | 2026-10-11 (active) | 2 | 1.71 | 15 |
| `serial_test` | d | 4.0.1 | 4.0.1 | MIT | 2026-07-25 (active) | 1 | 1.93.1 | 31 |
| `syn` | d | 3.0.9 | 3.0.9 | MIT OR Apache-2.0 | 2026-10-12 (active) | 1 | 1.71 | 3 |
| `sysinfo` | d | 0.40.1 | 0.40.1 | MIT | 2026-10-30 (active) | 1 | 1.95 | 28 |
| `tar` | n | 0.4.46 | 0.4.46 | MIT OR Apache-2.0 | 2026-05-18 (active) | 1 | 1.63 | 3 |
| `tempfile` | n | 3.27.0 | 3.27.0 | MIT OR Apache-2.0 | 2026-03-11 (slow) | 1 | 1.63 | 34 |
| `test-log` | d | 0.2.21 | 0.2.21 | Apache-2.0 OR MIT | 2026-05-30 (active) | 1 | 1.71 | 53 |
| `thirtyfour` | n | 0.38.1 | 0.38.1 | MIT OR Apache-2.0 | 2026-10-21 (active) | 2 | 1.88 | 203 |
| `thiserror` | n | 2.0.21 | 2.0.21 | MIT OR Apache-2.0 | 2026-09-23 (active) | 1 | 1.77 | 5 |
| `tokio` | dn | 1.54.1 | 1.54.1 | MIT | 2026-10-21 (active) | 3 | 1.85 | 25 |
| `tokio-tungstenite` | n | 0.30.0 | 0.30.0 | MIT | 2026-07-11 (active) | 2 | 1.85 | 63 |
| `toml` | n | 1.1.8 | 1.2.1 | MIT OR Apache-2.0 | 2026-11-06 (active) | 2 | 1.85 | 18 |
| `toml_edit` | n | 0.25.17 | 0.25.19 | MIT OR Apache-2.0 | 2026-11-06 (active) | 3 | 1.85 | 17 |
| `tracing` | n | 0.1.44 | 0.1.44 | MIT | 2025-12-18 (slow) | 3 | 1.65.0 | 10 |
| `tracing-log` | n | 0.2.0 | 0.2.0 | MIT | 2023-10-25 (quiet) | 2 | 1.56.0 | 4 |
| `tracing-subscriber` | n | 0.3.23 | 0.3.23 | MIT | 2026-03-13 (slow) | 3 | 1.65.0 | 38 |
| `tracing-timing` | n opt | 0.7.0 | 0.7.0 | MIT | 2025-07-05 (slow) | 1 | - | 84 |
| `turso` | n opt | 0.8.3 | 0.8.3 | MIT | 2026-10-28 (active) | 1 | - | 291 |
| `url` | n | 2.5.8 | 2.5.8 | MIT OR Apache-2.0 | 2026-01-05 (slow) | 5 | 1.63 | 37 |
| `usage-lib` | d | 7.2.0 | 7.3.0 | MIT | 2026-11-06 (active) | 1 | 1.91 | 33 |
| `usage-rs` | n | 7.2.0 | 7.3.0 | MIT | 2026-11-06 (active) | 1 | 1.91 | 7 |
| `which` | n | 8.0.6 | 8.0.6 | MIT | 2026-08-26 (active) | 2 | 1.70 | 1 |
| `zip` | n | 9.2.2 | 9.4.1 | MIT | 2026-11-06 (active) | 2 | 1.88 | 48 |

Use and verdict:

| Crate | Used by | Features | Why we use it | Verdict |
|---|---|---|---|---|
| `async-trait` | library | defaults | Object-safe async methods (the capability traits, plugin and launcher traits). | keep. Keep while the traits are used as `dyn`; check native async fn in traits against Rust 1.99 before dropping it (to verify). |
| `axum` | library | no defaults, `http1`, `tokio` | `mcp::http`: the router and listener that serve rmcp's streamable HTTP service. | keep. Only `tokio` and `http1`; optional (`mcp-server`). |
| `base64` | library | defaults | Base64 for CDP payloads (screenshots, PDFs) and HTTP headers. | keep |
| `bytes` | library | defaults | Names the types of thirtyfour's `HttpClient` trait, which `browser::reuse` implements. | keep. Already in the graph through thirtyfour. |
| `chrono` | library | defaults | Only `mcp::devtools` still uses it; the manifest says so. | replace. Move the one file that still uses it (`mcp::devtools`) to jiff, the crate's clock, and drop chrono. |
| `console-subscriber` | library | defaults | Feature `tokio-console`: a live view of async tasks. | keep. 118 crates (gRPC); opt-in, never default. |
| `crossterm` | library | defaults | Terminal backend of the Commander TUI (`tui`). | keep. One maintainer; the last release is from April 2025. Quiet: last release 2025-04-05. |
| `dhat` | library | defaults | Dev: allocation-site profiler of the `alloc_sites` bench. | keep. One maintainer, last release February 2024. Quiet: last release 2024-02-04. |
| `dirs` | library, cli | defaults | User cache and config folders (driver and browser downloads, settings file). | keep |
| `divan` | library | defaults | Dev: allocation counts of the `alloc` bench. | keep. Quiet: last release 2025-04-10. |
| `enigo` | library | defaults | `gui`: mouse and keyboard automation outside the browser. | keep. Opt-in; brings `windows` 0.61 next to sysinfo's 0.62 on Windows. |
| `figment` | library | no defaults | Layers the settings file, the `SB_*` variables and code (`config::settings`); no figment type is public. | keep. One maintainer, last release May 2024. It is only an internal layering step, so a replacement stays cheap; keep until it breaks. Quiet: last release 2024-05-17. |
| `flate2` | library | no defaults, `zlib-rs` | zlib and gzip with the zlib-rs backend. | upgrade (lock) |
| `futures-util` | library | no defaults, `sink`, `std` | Stream and sink combinators over the WebSocket connections. | keep |
| `http` | library | defaults | Same reason as bytes: the `HttpClient` trait of thirtyfour. | keep. Already in the graph through thirtyfour. |
| `httpmock` | library | defaults | Dev: a mock HTTP server for the download tests. | keep. 103 crates; brings `similar` 3 next to insta's `similar` 2. |
| `image` | library | no defaults, `png` | `ocr`, `visual`: PNG decoding of screenshots. | keep |
| `image-compare` | library | defaults | `visual`: SSIM between a screenshot and its baseline. | keep. One maintainer; the last release is from August 2025. |
| `indicatif` | library | defaults | Progress bars for driver and browser downloads. | keep. Drawn only when stdout is a terminal. |
| `insta` | library | defaults | Dev: snapshot tests of the MCP tool lists. | keep |
| `jiff` | library, cli | defaults | The clock: `Zoned` and `Timestamp` wherever the crate reads, writes or prints a time. | keep |
| `miette` | cli | no defaults, `fancy-no-syscall` | `crates/cli`: the report of a command that failed (graphical handler). | keep. Brings `unicode-width` 0.1 next to 0.2; the last release is from April 2025. Quiet: last release 2025-04-27. |
| `object_store` | library | no defaults, `fs` | `s3`, `azure`: uploads of test artifacts. | keep. 226 crates, the largest tree after turso and rmcp; opt-in. |
| `ocrs` | library | defaults | `ocr`: pure-Rust OCR on the rten engine. | keep |
| `pdf-extract` | library | defaults | `pdf`: text extraction behind `get_pdf_text`. | replace. 0.12.1 is the newest release and still on lopdf 0.42, which brings the unmaintained ttf-parser and a second crypto stack (aes 0.8, cbc 0.1, ecb 0.1, md-5 0.10, sha2 0.10). lopdf is at 0.47 and printpdf already uses 0.44. Wait for a release on a current lopdf, or evaluate another extractor. |
| `printpdf` | library | defaults | Dev: writes PDF fixtures for the `api::pdf` tests (`src/api/pdf.rs:286`). | remove. 155 crates, and the unmaintained bincode 1 (printpdf, azul-layout, hyphenation), for one test fixture. Check in a small PDF or write the bytes in the test, then drop it. |
| `proc-macro2` | library | `span-locations` | Dev: span locations for the test that writes the `find_api` index. | keep |
| `rand` | library | defaults | Randomness for human-paced input and identities. | keep. Three versions in the tree (0.8, 0.9, 0.10); only 0.10 is ours. |
| `rand_distr` | library | `std_math` | The Gaussian bell curve of human-paced input. | keep |
| `ratatui` | library | defaults | `tui`: the Commander terminal UI. | keep. Its termwiz and wezterm crates appear only in the all-target resolve; `cargo tree -i ratatui-termwiz` finds nothing for the host target. No trimming needed today. |
| `regex` | library, cli | defaults | Pattern matching on selectors, URLs and logs. | keep |
| `reqwest` | library, cli | no defaults, `charset`, `http2`, `json`, `rustls-no-provider`, `system-proxy` | Downloads (drivers, browsers), IP lookups, the MCP HTTP client. | keep. A second copy, 0.12, comes from rustwright. |
| `rfd` | library | defaults | `gui`: native file and message dialogs. | keep. Opt-in. |
| `ring` | library | defaults | Crypto provider of rustls and the hashes the crate needs. | keep. One maintainer and a long gap since the last release; rustls offers other providers if that changes. |
| `rmcp` | library | no defaults, `server`, `transport-io`, `transport-streamable-http-server` | `mcp-server`, `mcp-client`: the MCP SDK. | upgrade (lock). 223 crates; opt-in. |
| `roxmltree` | library | defaults | Dev: parses the JUnit XML the tests write. | keep |
| `rpassword` | cli | defaults | `crates/cli`: `sbrs encrypt` and `decrypt` read a passphrase without echo. | keep |
| `rstest` | library | no defaults | `testing`: the `#[fixture]` attribute of the `sb` and `sb_cdp` fixtures. | keep |
| `rstest-bdd` | library, own-imports | defaults | `bdd`: Gherkin features as cargo tests. | keep. Its i18n-embed dependency brings the unmaintained basic-toml (a build-time macro). i18n-embed and i18n-config are one maintainer and have not released since July 2025. |
| `rstest-bdd-macros` | library | defaults | `bdd`: the macros of rstest-bdd. | keep |
| `rten` | library | defaults | `ocr`: the inference engine under ocrs. | upgrade (breaking). 0.27.0 is out (2026-10-02) but ocrs 0.13.1 (2026-09-13) predates it; the lock cannot move until an ocrs release allows 0.27 (to verify). |
| `rustls` | library | no defaults, `logging`, `ring`, `std`, `tls12` | TLS provider choice for reqwest and the MCP clients (`ring`). | keep |
| `rustwright` | playwright | defaults | `crates/playwright`: the Rust Playwright port that Stealthy Playwright Mode attaches. | upgrade. Its rustwright-core pulls reqwest 0.12, tokio-tungstenite 0.29 and webpki-roots 0.26, a second copy of each. Newest is 0.3.0, so this waits for an upstream release on reqwest 0.13 (to verify). |
| `schemars` | library | defaults | Dev: writes `schemas/fingerprint.schema.json` from the types. | keep |
| `scraper` | library | no defaults | HTML parsing and CSS selection on page source. | keep |
| `seleniumbase-rs-plugin` | library | defaults | `plugins`: the plugin API, from the plugins repository. | replace. A git dependency pinned by `rev`; replace it with the crates.io version once the plugin crates are published (publish order below). |
| `seleniumbase-rs-plugins-builtin` | library | defaults | `plugin-*`: the built-in plugins, one feature each. | replace. Same git source and `rev` as the plugin API; replace together with it. |
| `serde` | library | `derive` | Serialisation of configuration, fingerprints, reports and protocol messages. | keep |
| `serde_json` | library, cli | `preserve_order` | JSON for CDP, WebDriver, MCP and the settings. | keep |
| `serial_test` | library, cli, playwright | defaults | Dev: `#[serial(chrome)]` and `#[serial(env)]` groups. | keep |
| `syn` | library, cli | `full`, `visit` | Dev: parses sources inside tests (scaffold output, purity checks, the API index). | keep. syn 3 sits beside syn 2 and syn 1, which other crates pull in. |
| `sysinfo` | library, cli | no defaults, `system` | Dev: lists processes for the leftover-process checks. | keep. windows 0.62 beside enigo's 0.61. |
| `tar` | library | no defaults | Reads the tar archives of driver and browser downloads. | keep |
| `tempfile` | library, cli | no defaults | Temporary files and folders for downloads, profiles and tests. | keep |
| `test-log` | library | no defaults, `color`, `trace` | Dev: prints the `tracing` output of a failing test. | keep |
| `thirtyfour` | library, cli | no defaults, `cdp`, `reqwest` | The WebDriver engine (`BaseCase`, `BrowserSession`). | keep. 203 crates. Five of its areas are being reworked in the thirtyfour lanes; this note does not judge them. |
| `thiserror` | library | defaults | `Error` derives for the error types. | keep |
| `tokio` | library, cli | `fs`, `io-util`, `macros`, `net`, `process`, `rt-multi-thread` ... | The async runtime. | keep |
| `tokio-tungstenite` | library | defaults | WebSocket client of the CDP connection (a dev dependency where only a test needs one). | keep |
| `toml` | library | defaults | Reads the settings file. | upgrade (lock) |
| `toml_edit` | cli | defaults | `crates/cli`: edits a client's `config.toml` and keeps its comments (`sbrs-mcp install`). | upgrade (lock) |
| `tracing` | library, cli | defaults | Logging: the crates log with `tracing`. | keep |
| `tracing-log` | library | defaults | Bridges `log` records into `tracing`. | keep. Two owners, last release October 2023; a finished crate. Quiet: last release 2023-10-25. |
| `tracing-subscriber` | library | `env-filter`, `fmt`, `json` | The subscriber that prints and filters the logs (`env-filter`, `fmt`, `json`). | keep |
| `tracing-timing` | library | defaults | Feature `full-tracing`: latency histograms per span (`src/tracing_util.rs:199`). | replace. Its fxhash is unmaintained (RUSTSEC-2025-0057) and 0.7.0, from July 2025, is the newest release. Drop the feature if nobody uses it, or build the histogram without it. |
| `turso` | library | no defaults | `turso`: the embedded database for `ResultStore` and `ProfileVault`. | keep. 291 crates, and bindgen 0.69.5 (with `which` 4, `nom` 7, `itertools` 0.12, `rustc-hash` 1, `shlex` 1) through turso_sdk_kit. A 0.9.0-pre.2 is out; stay on 0.8.3 until a stable 0.9. |
| `url` | library | defaults | URL parsing. | keep |
| `usage-lib` | cli | no defaults, `markdown` | Dev: renders the CLI reference page from the usage spec. | upgrade (lock) |
| `usage-rs` | cli | `completions` | `crates/cli`: the command-line parser, completions and usage spec. | upgrade (lock) |
| `which` | library, cli | defaults | Finds programs on `PATH` (browsers, drivers, `java`). | keep. A second copy, which 4, comes with bindgen under turso. |
| `zip` | library | no defaults, `deflate-flate2-zlib-rs` | Reads and writes zip archives (drivers, browsers, CRX files). | upgrade (lock) |

## Stealth: `seleniumbase-rs-stealth`

A workspace of four packages (the facade, `codec`, `scrape` and `wire`), 328 packages in the lock. `cargo deny --workspace check` and `cargo audit` pass (PR on `lane/deny-toml`). Three exact pins: `btls` and `tokio-btls` `=0.5.6` (BoringSSL) and `wreq` `=6.0.0-rc.31`, a release candidate. The core library comes in by git with no `rev`, so the lockfile decides which core commit stealth builds against.


Facts (one row per direct dependency):

| Crate | Kind | Used | Newest | Licence | Last release | Owners | MSRV | Pulls in |
|---|---|---|---|---|---|---|---|---|
| `async-trait` | dn | 0.1.92 | 0.1.92 | MIT OR Apache-2.0 | 2026-08-08 (active) | 1 | 1.71 | 4 |
| `base64` | n | 0.23.1 | 0.23.1 | MIT OR Apache-2.0 | 2026-08-04 (active) | 2 | 1.71.0 | 0 |
| `brotli` | n | 9.1.1 | 9.1.1 | BSD-3-Clause AND MIT | 2026-10-27 (active) | 3 | 1.59.0 | 3 |
| `btls` | n | 0.5.6 | 0.5.6 | Apache-2.0 | 2026-04-20 (slow) | 1 | 1.85 | 56 |
| `bytes` | n | 1.12.1 | 1.12.1 | MIT | 2026-07-08 (active) | 3 | 1.57 | 0 |
| `chrono` | n | 0.4.45 | 0.4.45 | MIT OR Apache-2.0 | 2026-06-04 (active) | 2 | 1.62.0 | 41 |
| `flate2` | n | 1.1.10 | 1.1.11 | MIT OR Apache-2.0 | 2026-11-06 (active) | 1 | 1.67.0 | 6 |
| `futures-util` | n | 0.3.34 | 0.3.34 | MIT OR Apache-2.0 | 2026-08-11 (active) | 1 | 1.71 | 5 |
| `http` | n | 1.5.0 | 1.5.0 | MIT OR Apache-2.0 | 2026-07-29 (active) | 2 | 1.57.0 | 2 |
| `http-body-util` | n | 0.1.5 | 0.1.5 | MIT | 2026-08-12 (active) | 1 | 1.61 | 6 |
| `hyper` | n | 1.12.0 | 1.12.0 | MIT | 2026-10-06 (active) | 2 | 1.63 | 42 |
| `hyper-util` | n | 0.1.21 | 0.1.21 | MIT | 2026-09-24 (active) | 1 | 1.85 | 57 |
| `psl` | n | 2.1.246 | 2.1.248 | MIT/Apache-2.0 | 2026-11-06 (active) | 1 | - | 1 |
| `ring` | n | 0.17.14 | 0.17.14 | Apache-2.0 AND ISC | 2025-03-11 (quiet) | 1 | 1.66.0 | 39 |
| `seleniumbase-rs` | dn | 0.7.0 (git) | n/a | MIT | n/a | n/a | 1.99 | 290 |
| `serde` | n | 1.0.229 | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 (active) | 2 | 1.56 | 6 |
| `serde_json` | n | 1.0.154 | 1.0.154 | MIT OR Apache-2.0 | 2026-10-11 (active) | 2 | 1.71 | 14 |
| `syn` | d | 3.0.9 | 3.0.9 | MIT OR Apache-2.0 | 2026-10-12 (active) | 1 | 1.71 | 3 |
| `tempfile` | n | 3.27.0 | 3.27.0 | MIT OR Apache-2.0 | 2026-03-11 (slow) | 1 | 1.63 | 30 |
| `thiserror` | n | 2.0.21 | 2.0.21 | MIT OR Apache-2.0 | 2026-09-23 (active) | 1 | 1.77 | 5 |
| `tokio` | dn | 1.54.1 | 1.54.1 | MIT | 2026-10-21 (active) | 3 | 1.85 | 15 |
| `tokio-btls` | n | 0.5.6 | 0.5.6 | MIT OR Apache-2.0 | 2026-04-20 (slow) | 1 | 1.85 | 66 |
| `tokio-tungstenite` | d | 0.30.0 | 0.30.0 | MIT | 2026-07-11 (active) | 2 | 1.85 | 50 |
| `tracing` | n | 0.1.44 | 0.1.44 | MIT | 2025-12-18 (slow) | 3 | 1.65.0 | 9 |
| `tracing-subscriber` | n | 0.3.23 | 0.3.23 | MIT | 2026-03-13 (slow) | 3 | 1.65.0 | 37 |
| `url` | n | 2.5.8 | 2.5.8 | MIT OR Apache-2.0 | 2026-01-05 (slow) | 5 | 1.63 | 35 |
| `wreq` | n | 6.0.0-rc.31 | 6.0.0-rc.31 | Apache-2.0 | 2026-10-30 (active) | 1 | 1.85 | 147 |

Use and verdict:

| Crate | Used by | Features | Why we use it | Verdict |
|---|---|---|---|---|
| `async-trait` | scrape, wire | defaults | Object-safe async methods (the capability traits, plugin and launcher traits). | keep. Keep while the traits are used as `dyn`; check native async fn in traits against Rust 1.99 before dropping it (to verify). |
| `base64` | wire | defaults | Base64 for CDP payloads (screenshots, PDFs) and HTTP headers. | keep |
| `brotli` | wire | no defaults, `std` | wire: decodes brotli bodies of captured responses. | keep |
| `btls` | wire | defaults | wire: BoringSSL TLS, for control of the JA3/JA4 fingerprint. | keep. Pinned `=0.5.6` together with tokio-btls; it builds BoringSSL (use the prebuilt, `docs/contributing/boringssl.md`). One maintainer. |
| `bytes` | wire | defaults | Names the byte buffer types of the HTTP stack the crate plugs into. | keep |
| `chrono` | scrape, wire | defaults | Timestamps in the scrape and wire crates. | replace. Replace with `jiff`, as in core: one clock crate across the family. |
| `flate2` | wire | no defaults, `zlib-rs` | zlib and gzip with the zlib-rs backend. | upgrade (lock) |
| `futures-util` | scrape, wire | no defaults, `sink`, `std` | Stream and sink combinators over the WebSocket connections. | keep |
| `http` | scrape, wire | defaults | HTTP types shared with reqwest, hyper and thirtyfour. | keep |
| `http-body-util` | wire | defaults | wire: body helpers of the MITM server. | keep |
| `hyper` | wire | no defaults, `http1`, `server` | wire: the HTTP/1 server of the MITM proxy. | keep |
| `hyper-util` | wire | no defaults, `tokio` | wire: the tokio glue of the MITM server. | keep |
| `psl` | scrape | defaults | scrape: the public-suffix list, for the registrable domain of a URL. | upgrade (lock) |
| `ring` | codec, wire | defaults | Crypto provider of rustls and the hashes the crate needs. | keep. One maintainer and a long gap since the last release; rustls offers other providers if that changes. |
| `seleniumbase-rs` | codec, scrape, stealth, +1 | `test-util` | The core library, every stealth crate builds on its types. | replace. A git dependency without `rev` or `version`, locked to 0.7.0 while core is 0.8.0 (`cargo update -p seleniumbase-rs` takes it to main). It blocks a crates.io release: replace it with the published version. |
| `serde` | codec, scrape, wire | `derive` | Serialisation of configuration, fingerprints, reports and protocol messages. | keep |
| `serde_json` | codec, scrape, wire | defaults | JSON for CDP, WebDriver, MCP and the settings. | keep |
| `syn` | scrape | `full`, `visit` | Dev: parses sources inside tests (scaffold output, purity checks, the API index). | keep. syn 3 sits beside syn 2 and syn 1, which other crates pull in. |
| `tempfile` | scrape, wire | defaults | Temporary files and folders for downloads, profiles and tests. | keep |
| `thiserror` | codec, scrape, wire | defaults | `Error` derives for the error types. | keep |
| `tokio` | scrape, wire | `fs`, `io-util`, `macros`, `net`, `process`, `rt-multi-thread` ... | The async runtime. | keep |
| `tokio-btls` | wire | defaults | wire: the tokio adapter of btls. | keep. Pinned `=0.5.6`, in step with btls. |
| `tokio-tungstenite` | wire | defaults | Dev: the WebSocket client of the MITM tests. | keep |
| `tracing` | scrape, wire | defaults | Logging: the crates log with `tracing`. | keep |
| `tracing-subscriber` | scrape | `env-filter`, `fmt`, `json` | scrape: prints and filters logs. | keep |
| `url` | scrape, wire | defaults | URL parsing. | keep |
| `wreq` | wire | no defaults, `brotli`, `deflate`, `gzip`, `socks`, `stream`, `tokio-rt` ... | wire: HTTP client with browser-like TLS and HTTP/2 settings. | keep. Pinned `=6.0.0-rc.31`: a release candidate, the newest release (2026-08-18). Upgrading is a lane of its own. |

## Plugins: `seleniumbase-rs-plugins`

A workspace of 14 crates (the plugin API, its test kit, the aggregator and eleven plugins; `template` is excluded), 145 packages in the lock. `cargo deny --workspace check` passes with one allowed advisory (`pin-utils`, below) and `cargo audit` reports no vulnerability. None of the crates depends on core, so the plugins can be published before it.


Facts (one row per direct dependency):

| Crate | Kind | Used | Newest | Licence | Last release | Owners | MSRV | Pulls in |
|---|---|---|---|---|---|---|---|---|
| `async-imap` | n | 0.12.0 | 0.12.0 | MIT OR Apache-2.0 | 2026-09-30 (active) | 2 | 1.88 | 54 |
| `async-trait` | n | 0.1.92 | 0.1.92 | MIT OR Apache-2.0 | 2026-08-08 (active) | 1 | 1.71 | 4 |
| `futures-util` | n | 0.3.34 | 0.3.34 | MIT OR Apache-2.0 | 2026-08-11 (active) | 1 | 1.71 | 11 |
| `mail-parser` | n | 1.0.1 | 1.0.1 | Apache-2.0 OR MIT | 2026-11-02 (active) | 1 | 1.98 | 8 |
| `ring` | n | 0.17.14 | 0.17.14 | Apache-2.0 AND ISC | 2025-03-11 (quiet) | 1 | 1.66.0 | 18 |
| `rustls` | n | 0.23.46 | 0.23.46 | Apache-2.0 OR ISC OR MIT | 2026-10-14 (active) | 3 | 1.71 | 24 |
| `serde` | n | 1.0.229 | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 (active) | 2 | 1.56 | 6 |
| `serde_json` | dn | 1.0.154 | 1.0.154 | MIT OR Apache-2.0 | 2026-10-11 (active) | 2 | 1.71 | 10 |
| `tempfile` | dn | 3.27.0 | 3.27.0 | MIT OR Apache-2.0 | 2026-03-11 (slow) | 1 | 1.63 | 23 |
| `thiserror` | n | 2.0.21 | 2.0.21 | MIT OR Apache-2.0 | 2026-09-23 (active) | 1 | 1.77 | 5 |
| `tokio` | dn | 1.54.1 | 1.54.1 | MIT | 2026-10-21 (active) | 3 | 1.85 | 13 |
| `tokio-rustls` | n | 0.26.6 | 0.26.6 | MIT OR Apache-2.0 | 2026-09-28 (active) | 3 | 1.71 | 37 |
| `wasmi` | n | 2.0.0 | 2.0.0 | MIT/Apache-2.0 | 2026-09-01 (active) | 1 | 1.86 | 20 |
| `wat` | d | 1.263.0 | 1.263.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 2026-10-30 (active) | 1 | 1.88.0 | 12 |
| `webpki-roots` | n | 1.0.11 | 1.0.11 | CDLA-Permissive-2.0 | 2026-10-23 (active) | 3 | 1.71 | 2 |
| `zip` | dn | 9.3.0 | 9.4.1 | MIT | 2026-11-06 (active) | 2 | 1.88 | 31 |

Use and verdict:

| Crate | Used by | Features | Why we use it | Verdict |
|---|---|---|---|---|
| `async-imap` | plugin-imap | no defaults, `runtime-tokio` | The IMAP client of the `imap` plugin. | keep. 0.12.0 is the newest release (2026-09-30) and the one that pulls the unmaintained pin-utils, thiserror 1, stop-token and async-channel 1. |
| `async-trait` | plugin, plugin-testing | defaults | Object-safe async methods (the capability traits, plugin and launcher traits). | keep. Keep while the traits are used as `dyn`; check native async fn in traits against Rust 1.99 before dropping it (to verify). |
| `futures-util` | plugin-batcher, plugin-imap | no defaults, `alloc`, `std` | Stream and sink combinators over the WebSocket connections. | keep |
| `mail-parser` | plugin-imap | defaults | The `imap` plugin parses a fetched mail to find its code or link. | keep |
| `ring` | plugin-tor | defaults | Crypto provider of rustls and the hashes the crate needs. | keep. One maintainer and a long gap since the last release; rustls offers other providers if that changes. |
| `rustls` | plugin-imap | no defaults, `ring`, `std`, `tls12` | TLS configuration of the IMAP connection (`ring`). | keep |
| `serde` | plugin, plugin-batcher, plugin-browser, +7 | `derive` | Serialisation of configuration, fingerprints, reports and protocol messages. | keep |
| `serde_json` | plugin, plugin-batcher, plugin-block-resources, +11 | defaults | Plugin options and results. | keep |
| `tempfile` | plugin-browser, plugin-crx, plugin-tor, +1 | defaults | Temporary files and folders for downloads, profiles and tests. | keep |
| `thiserror` | plugin, plugin-wasm | defaults | `Error` derives for the error types. | keep |
| `tokio` | plugin-imap, plugin-tor | `io-util`, `macros`, `net`, `rt`, `sync`, `test-util` ... | Runtime features each plugin needs (trimmed to `io-util`, `net`, `rt`, `sync`, `time`). | keep |
| `tokio-rustls` | plugin-imap | no defaults, `ring`, `tls12` | TLS stream of the IMAP connection. | keep |
| `wasmi` | plugin-wasm | no defaults, `auto-dispatch`, `stable`, `std`, `validate` | The `wasm` plugin: runs `.wasm` plugins in an interpreter. | keep. Uses wasmparser 0.228 while `wat` brings 0.263; align when wasmi moves up (to verify). |
| `wat` | plugin-wasm | defaults | Dev: text-to-wasm for the wasm plugin's tests. | keep |
| `webpki-roots` | plugin-imap | defaults | Trusted roots of the IMAP connection. | keep. 1.0.11 includes the fix of the 2026 roots advisories (RUSTSEC-2026-0381, fixed in 1.0.10). |
| `zip` | plugin-browser, plugin-crx | no defaults, `deflate-flate2-zlib-rs` | The `crx` and `browser` plugins read and write archives. | upgrade (lock) |

## Tauri plugin: `tauri-plugin-sbrs-automation`

One package. 409 packages in the lock. `cargo deny check` and `cargo audit` pass. The `tauri` and `tauri-plugin` pins are exact alphas (`=3.0.0-alpha.5` and `=3.0.0-alpha.4`).


Facts (one row per direct dependency):

| Crate | Kind | Used | Newest | Licence | Last release | Owners | MSRV | Pulls in |
|---|---|---|---|---|---|---|---|---|
| `base64` | n | 0.22.1 | 0.23.1 | MIT OR Apache-2.0 | 2026-08-04 (active) | 2 | 1.48.0 | 0 |
| `block2` | n | 0.6.2 | 0.6.2 | MIT | 2025-10-04 (slow) | 2 | 1.71 | 2 |
| `objc2` | n | 0.6.5 | 0.6.5 | MIT | 2026-10-05 (active) | 2 | 1.71 | 1 |
| `objc2-app-kit` | n | 0.3.2 | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 2025-10-04 (slow) | 2 | 1.71 | 13 |
| `objc2-core-foundation` | n | 0.3.2 | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 2025-10-04 (slow) | 2 | 1.71 | 10 |
| `objc2-foundation` | n | 0.3.2 | 0.3.2 | MIT | 2025-10-04 (slow) | 2 | 1.71 | 11 |
| `objc2-web-kit` | n | 0.3.2 | 0.3.2 | Zlib OR Apache-2.0 OR MIT | 2025-10-04 (slow) | 2 | 1.71 | 15 |
| `seleniumbase-rs` | n opt | 0.7.0 (git) | n/a | MIT | n/a | n/a | 1.99 | 269 |
| `serde` | n | 1.0.229 | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 (active) | 2 | 1.56 | 6 |
| `serde_json` | n | 1.0.154 | 1.0.154 | MIT OR Apache-2.0 | 2026-10-11 (active) | 2 | 1.71 | 14 |
| `sha2` | d | 0.10.9 | 0.11.1 | MIT OR Apache-2.0 | 2026-10-12 (active) | 3 | - | 9 |
| `tauri` | n | 3.0.0-alpha.5 | 3.0.0-alpha.6 | Apache-2.0 OR MIT | 2026-11-06 (active) | 1 | 1.95 | 356 |
| `tauri-plugin` | b | 3.0.0-alpha.4 | 3.0.0-alpha.4 | Apache-2.0 OR MIT | 2026-10-14 (active) | 1 | 1.95 | 196 |
| `tiny_http` | n | 0.12.0 | 0.12.0 | MIT OR Apache-2.0 | 2022-10-06 (quiet) | 3 | - | 4 |
| `tokio` | d | 1.54.1 | 1.54.1 | MIT | 2026-10-21 (active) | 3 | 1.85 | 18 |

Use and verdict:

| Crate | Used by | Features | Why we use it | Verdict |
|---|---|---|---|---|
| `base64` | plugin | defaults | macOS only: encodes the WKWebView snapshot. | upgrade. 0.23 is what core and stealth use; a one-line bump. |
| `block2` | plugin | defaults | Apple's C language extension of blocks | keep |
| `objc2` | plugin | defaults | macOS only: native access to the WKWebView. | keep. Tauri builds these crates already; only features are added. |
| `objc2-app-kit` | plugin | no defaults, `NSBitmapImageRep`, `NSImage`, `NSImageRep`, `NSResponder`, `NSView`, `std` | Bindings to the AppKit framework | keep |
| `objc2-core-foundation` | plugin | no defaults, `CFCGTypes`, `std` | Bindings to the CoreFoundation framework | keep |
| `objc2-foundation` | plugin | no defaults, `NSArray`, `NSData`, `NSDictionary`, `NSError`, `NSString`, `std` | Bindings to the Foundation framework | keep |
| `objc2-web-kit` | plugin | no defaults, `WKSnapshotConfiguration`, `WKWebView`, `block2`, `objc2-app-kit`, `objc2-core-foundation`, `std` | Bindings to the WebKit framework | keep |
| `seleniumbase-rs` | plugin | no defaults | Feature `stealth`: the library's `Page` and fingerprint API in the app's webviews. | replace. A git dependency on tag `v0.7.0` (core is 0.8.0) and no `version`: bump the tag with core, and replace it with the published version when core is on crates.io. |
| `serde` | plugin | `derive` | Serialisation of configuration, fingerprints, reports and protocol messages. | keep |
| `serde_json` | plugin | defaults | JSON for CDP, WebDriver, MCP and the settings. | keep |
| `sha2` | plugin | defaults | Dev: hashes inside tests. | upgrade. 0.11 is a breaking bump; the tree holds `digest` 0.10 and 0.11 side by side. |
| `tauri` | plugin | no defaults | The app framework the plugin extends. | upgrade (pre-release). Pinned `=3.0.0-alpha.5`, a pre-release; alpha.6 appeared today (2026-11-06). |
| `tauri-plugin` | plugin | `build` | Build dependency: generates the plugin's permission files. | keep. Pinned `=3.0.0-alpha.4`, a different alpha than `tauri`; keep them aligned when tauri moves. |
| `tiny_http` | plugin | defaults | A small HTTP server: the plugin serves its WebDriver endpoint from inside the app (role read from the plugin's description, to verify). | keep. Three owners but no release since October 2022; check it against `cargo audit` on each lock refresh. |
| `tokio` | plugin | `macros`, `rt`, `sync`, `time` | The async runtime. | keep |

### The Tauri example app

`examples/tauri-profile-manager/src-tauri` is a separate workspace with 18 direct dependencies and 785 packages in a tracked `Cargo.lock`. `cargo audit` fails there with **17 vulnerabilities** and 17 allowed warnings (several of the advisories are dated 2026-11-06):

| Crate | Version | Advisories | Fixed in |
|---|---|---|---|
| `actix-http` | 3.18.13 | RUSTSEC-2026-0448, -0446, -0455 (unbounded decompression, request smuggling, Brotli limits) | 3.20.3 |
| `actix-web` | 4.16.0 | RUSTSEC-2026-0447 (compressed bodies exhaust memory) | 4.18.2 |
| `rustls` | 0.23.45 | RUSTSEC-2026-0346 to -0349 (secrets in `Debug`, buffered data, Brotli allocation, downgrade) | 0.23.46 |
| `rustls-webpki` | 0.103.15 | RUSTSEC-2026-0382 (distrusted intermediate accepted) | 0.103.16 |
| `webpki-roots`, `webpki-root-certs` | 1.0.9 | RUSTSEC-2026-0381, -0383 (distrusted roots) | 1.0.10 |
| `zstd`, `zstd-safe`, `zstd-sys` | 0.13.3, 7.3.0, 2.1.1 | RUSTSEC-2026-0391, -0398, -0403, -0410 | 0.13.4 or 0.14.3, 7.4 or 8.2, 2.1.2 |
| `ctutils` | 0.4.3 | RUSTSEC-2026-0385 (not constant time) | 0.4.4 |
| `h2` | 0.3.27 | RUSTSEC-2026-0258 (unbounded empty DATA frames) | 0.4.16 (a different minor line: it comes with `actix-http` 3.18.13) |

`cargo update --dry-run` locks 106 packages and moves all of these but `h2` to the fixed releases (`actix-http` 3.20.3, `actix-web` 4.18.2, `rustls` 0.23.46, `zstd` 0.14.3, `tokio` 1.54.1). Whether `h2` 0.3.27 goes with the new `actix-http` is to verify after the update. The example is not published, but the lockfile is tracked and a developer builds it. The allowed warnings are `fxhash`, `zbus_names`, `zvariant` and `zvariant_derive` (unmaintained) and `rand` 0.7.3, `tokio` 1.53.2 and ten `zstd-safe` advisories (unsound); the `tokio` one goes with the refresh.

## The HTTP/3 helper: `crates/h3d` of stealth

A workspace of its own (`phantom-http` builds its own BoringSSL and the two builds cannot share a binary), so the stealth lock does not cover it. `cargo audit` passes on its lock. The HTTP/3 stack is pinned exactly (`phantom-http`, and `h3`, `h3-quinn`, `quinn` and `rcgen` for the tests), as the manifest explains.


Facts (one row per direct dependency):

| Crate | Kind | Used | Newest | Licence | Last release | Owners | MSRV | Pulls in |
|---|---|---|---|---|---|---|---|---|
| `base64` | n | 0.23.1 | 0.23.1 | MIT OR Apache-2.0 | 2026-08-04 (active) | 2 | 1.71.0 | 0 |
| `bytes` | d | 1.12.1 | 1.12.1 | MIT | 2026-07-08 (active) | 3 | 1.57 | 0 |
| `h3` | d | 0.0.8 | 0.0.8 | MIT | 2025-05-06 (quiet) | 3 | 1.70 | 28 |
| `h3-quinn` | d | 0.0.10 | 0.0.10 | MIT | 2025-05-06 (quiet) | 2 | 1.70 | 84 |
| `http` | n | 1.5.0 | 1.5.0 | MIT OR Apache-2.0 | 2026-07-29 (active) | 2 | 1.57.0 | 2 |
| `phantom-http` | n | 0.6.0 | 0.6.0 | MIT OR Apache-2.0 | 2026-11-01 (active) | 2 | 1.88 | 178 |
| `quinn` | d | 0.11.12 | 0.11.12 | MIT OR Apache-2.0 | 2026-09-14 (active) | 2 | 1.85 | 76 |
| `rcgen` | d | 0.13.2 | 0.14.10 | MIT OR Apache-2.0 | 2026-08-28 (active) | 2 | - | 57 |
| `rustls` | d | 0.23.46 | 0.23.46 | Apache-2.0 OR ISC OR MIT | 2026-10-14 (active) | 3 | 1.71 | 50 |
| `serde` | n | 1.0.229 | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 (active) | 2 | 1.56 | 6 |
| `serde_json` | n | 1.0.154 | 1.0.154 | MIT OR Apache-2.0 | 2026-10-11 (active) | 2 | 1.71 | 10 |
| `tempfile` | d | 3.27.0 | 3.27.0 | MIT OR Apache-2.0 | 2026-03-11 (slow) | 1 | 1.63 | 34 |
| `tokio` | dn | 1.54.1 | 1.54.1 | MIT | 2026-10-21 (active) | 3 | 1.85 | 15 |

Use and verdict:

| Crate | Used by | Features | Why we use it | Verdict |
|---|---|---|---|---|
| `base64` | h3d | defaults | Base64 for CDP payloads (screenshots, PDFs) and HTTP headers. | keep |
| `bytes` | h3d | defaults | Names the byte buffer types of the HTTP stack the crate plugs into. | keep |
| `h3` | h3d | defaults | Dev: upstream HTTP/3 for the local test server. | keep. Pinned `=0.0.8`, with h3-quinn and quinn, to what the tests were written against. Quiet: last release 2025-05-06. |
| `h3-quinn` | h3d | defaults | Dev: quinn adapter of h3 for the local test server. | keep. Pinned `=0.0.10`. Quiet: last release 2025-05-06. |
| `http` | h3d | defaults | HTTP types shared with reqwest, hyper and thirtyfour. | keep |
| `phantom-http` | h3d | defaults | The HTTP/3 client (its own BoringSSL), the whole reason the helper is a separate program. | keep. Pinned `=0.6.0` to the version the spike measured against Chrome 156; an upgrade reruns the spike (see the manifest). |
| `quinn` | h3d | no defaults, `runtime-tokio`, `rustls-ring` | Dev: QUIC endpoint of the local test server. | keep. Pinned `=0.11.12`. |
| `rcgen` | h3d | defaults | Dev: self-signed certificate of the test server. | upgrade (breaking). Pinned `=0.13.2`; 0.14 exists (breaking). Raise it with the quinn pins. |
| `rustls` | h3d | no defaults, `ring`, `std` | Dev: TLS of the test server. | keep |
| `serde` | h3d | `derive` | Serialisation of configuration, fingerprints, reports and protocol messages. | keep |
| `serde_json` | h3d | defaults | JSON for CDP, WebDriver, MCP and the settings. | keep |
| `tempfile` | h3d | defaults | Temporary files and folders for downloads, profiles and tests. | keep |
| `tokio` | h3d | `io-std`, `io-util`, `macros`, `process`, `rt-multi-thread`, `time` | The async runtime. | keep |

## The companion (`seleniumbase-rs-companion`)

TypeScript on Bun, not Rust: `package.json` lists five dev dependencies and no runtime dependency, and the repository was only read (`bun outdated`, `bun audit`; nothing was written).

| Package | Used | Newest | Verdict |
|---|---|---|---|
| `@types/bun` | 1.4.3 | 1.4.4 | upgrade (patch) |
| `@types/chrome` | 0.3.5 | 0.3.7 | upgrade (patch) |
| `@types/firefox-webext-browser` | 143.0.1 | 143.0.1 | keep |
| `happy-dom` | 21.0.1 | 21.0.4 | **upgrade now**: `bun audit` reports one high advisory (GHSA-834v-8h66-3h79, host code execution through an unsanitised import alias, versions 17.0.0 to 21.0.3). It is a dev dependency of the DOM tests, so the exposure is the test run, not the shipped extensions |
| `typescript` | 7.0.2 | 7.0.2 | keep |

## Unmaintained crates `cargo audit` allows today

Core allows five, the plugins one; each carries a reason in the `deny.toml` that allows it. The ways out:

| Crate | Advisory | Pulled in through | Way out |
|---|---|---|---|
| `basic-toml` 0.1.10 | RUSTSEC-2026-0374 | `rstest-bdd` 0.6.0, then `i18n-embed` 0.16.0, `i18n-embed-impl`, `i18n-config` 0.4.8 (feature `bdd`; a build-time macro) | None in this repository: it waits for `i18n-config` to move to the `toml` crate (one maintainer, no release since July 2025). Contained: `bdd` is opt-in and the crate never ships. |
| `bincode` 1.3.3 | RUSTSEC-2025-0141 | `printpdf` 0.12.8 (dev), `azul-layout`, `hyphenation` | Remove the `printpdf` dev dependency (action 5). |
| `fxhash` 0.2.1 | RUSTSEC-2025-0057 | `tracing-timing` 0.7.0 (feature `full-tracing`); also in the Tauri example | Drop the feature, or build the histogram without `tracing-timing` (action 7). |
| `pin-utils` 0.1.1 | RUSTSEC-2026-0441 | `async-imap` 0.12.0 (the `imap` plugin; core through `plugins-builtin`, and the plugins repository) | `async-imap` 0.12.0 is the newest release, so none yet: watch it. The plugin is opt-in and the crate is a one-line macro. |
| `ttf-parser` 0.25.1 | RUSTSEC-2026-0192 | `pdf-extract` 0.12.1, then `lopdf` 0.42.0 (feature `pdf`) | A `pdf-extract` release on a current `lopdf` (0.47 is out; `printpdf` already uses 0.44, which has no `ttf-parser`), or another extractor. |

## Duplicate versions, and what removes each

Core has 77 crate names in more than one version on the all-target resolve (`cargo deny` warns about 59); stealth 8 (7), the plugins 9 (7), the Tauri plugin 28 (25). The groups worth a decision, from the parent of each version:

| Duplicate | Held by | What removes it |
|---|---|---|
| `reqwest` 0.12 and 0.13, `tokio-tungstenite` and `tungstenite` 0.29 and 0.30, `webpki-roots` 0.26 and 1.0 | `rustwright-core` 0.3.0 (`crates/playwright`) | A `rustwright` release on `reqwest` 0.13 and `tokio-tungstenite` 0.30. Newest is 0.3.0: to verify upstream. |
| `lopdf` 0.42 and 0.44, with `aes`, `cbc`, `ecb`, `md-5`, `sha2`, `weezl`, `cipher`, `inout`, `block-padding`, `crypto-common`, `digest` | `pdf-extract` (0.42) against `printpdf` (0.44, dev) | Removing `printpdf` takes out the 0.44 side; `pdf-extract` on a current `lopdf` removes the rest. |
| `bindgen` 0.69 with `which` 4, `nom` 7, `itertools` 0.12, `rustc-hash` 1, `shlex` 1.3, `cexpr` | `turso_sdk_kit` 0.8.3 (feature `turso`, build time) | A `turso` release on a current `bindgen` (a 0.9.0 pre-release exists; to verify). |
| `windows` 0.61 and 0.62 with the `windows-*` family | `enigo` 0.6.1 (feature `gui`) against `sysinfo` 0.40 (dev) | An `enigo` release on `windows` 0.62. Windows only. |
| `thiserror` 1, `syn` 1, `async-channel` 1, `event-listener` 2, `pin-utils` | `async-imap` 0.12.0 and `stop-token` 0.7.0 (the `imap` plugin); `i18n-*` (feature `bdd`) | Upstream releases of `async-imap` and `i18n-embed`. |
| `base64` 0.22 and 0.23 | `azul-layout`, `hdrhistogram`, `headers`, `printpdf` on 0.22; `async-imap`, `httpmock`, `hyper-util`, `object_store` on 0.23 | Removing `printpdf` removes one 0.22 user; the rest wait for their crates. The Tauri plugin's own `base64` 0.22.1 is a one-line bump to 0.23. |
| `strum` 0.26 and 0.28, `rand` 0.8, 0.9 and 0.10, `getrandom` 0.2, 0.3 and 0.4 | `turso_core` and `ring` on the older lines; `ratatui` and our own `rand` on the newer | Nothing in this repository: they follow `turso`, `ring` and `rand`'s dependents. |
| stealth: `thiserror` 1 (`tokio-socks`), `shlex` 1.3 (`bindgen` of `btls`), `core-foundation` 0.9 (`system-configuration`), `syn` 2 and 3, `getrandom` 0.2 and `windows-sys` 0.52 (`ring`) | transitive | Nothing to do; all of it is outside the crate's own choices. |
| plugins: `wasmparser` 0.228 and 0.263, `hashbrown` 0.15 and 0.17 | `wasmi` 2.0.0 and `string-interner` 0.19 against `wat` 1.263.0 and `indexmap` | A `wasmi` release on the newer parser (to verify). |

## Git dependencies and the publish order

crates.io accepts a dependency with a `version` and a git source (it strips the git part), and refuses one with the git source alone or a path with no version. In the family today:

| Where | Dependency | Source | What blocks a release |
|---|---|---|---|
| core library | `seleniumbase-rs-plugin`, `seleniumbase-rs-plugins-builtin` (optional) | git, the same `rev` on both lines, `version = "0.1"` | The plugin crates are not on crates.io; publish them first, then replace the git source with the version. |
| stealth (four crates) | `seleniumbase-rs` | git, no `rev`, no `version`; locked to 0.7.0 | Core is not on crates.io; the line needs a `version`. The three workspace crates also name each other by path with no `version` (4 wildcard warnings in `cargo deny`). |
| plugins (13 crates) | `seleniumbase-rs-plugin` | path with no `version` (the aggregator's own path dependencies do carry `version = "0.1"`) | 14 wildcard warnings in `cargo deny`; add `version = "0.1"`. |
| Tauri plugin | `seleniumbase-rs` (optional, feature `stealth`) | git, tag `v0.7.0`, no `version` | Core is on 0.8.0; bump the tag with core, then use the published version. 1 wildcard warning. |

Publish order, each step after the one before: (1) the plugins repository: `seleniumbase-rs-plugin`, then `seleniumbase-rs-plugin-testing`, then the eleven plugin crates, then `seleniumbase-rs-plugins-builtin`; the plugins do not depend on core, so there is no cycle. (2) core's library `seleniumbase-rs` (its git lines replaced by versions; `crates/cli`, `crates/own-imports` and `crates/playwright` are `publish = false`). (3) stealth: `codec`, `scrape`, `wire`, with versions on the path dependencies. (4) The Tauri plugin, with the `stealth` feature on core's published version. `sbrs-h3d` stays unpublished (`publish = false`). The crates.io metadata gaps (`documentation`, `readme`, `keywords`, `categories`, `homepage`) are finding F13 of the API guidelines audit (`docs/design/api_guidelines_audit.md`, in its own PR).

## Upgrade lanes, in order

Each lane is small, one concern, with its own branch and gate. 1 and 2 are security and go first.

1. **`tauri-example-lock`**: `cargo update` in the example's workspace, `cargo audit` to prove it, decide `h2` 0.3.27 (the `actix-web` use).
2. **`companion-happy-dom`**: `bun audit fix` (happy-dom 21.0.4, plus the two type packages), `bun test`.
3. **`lock-core`**: `cargo update` in core (28 packages), the GitLab gate; then `lock-plugins` (3 packages), `lock-tauri` (10) and, once the stealth lanes have landed, `lock-stealth` (23, which also moves stealth from core 0.7.0 to main).
4. **`udeps`**: `cargo +nightly udeps --workspace --all-targets --all-features` on core, then stealth and the plugins, one at a time; remove what it reports.
5. **`chrono-to-jiff`**: core's `mcp::devtools` first, then stealth's scrape and wire.
6. **`drop-printpdf`**: replace the fixtures of the `api::pdf` tests with a checked-in PDF or bytes written in the test.
7. **`full-tracing`**: keep, replace or drop `tracing-timing` (the `fxhash` advisory).
8. **`publishable`**: versions on the path dependencies, crates.io metadata, `bans.wildcards = "deny"` in the three `deny.toml` files, then the publish order above.
9. **`pins`**: one review of every exact pin (`btls`, `tokio-btls`, `wreq` rc, `phantom-http`, the quinn family of `h3d`, the Tauri alphas) with the date of the next check.
10. **`watch`**: on every lock refresh read `cargo outdated -R` for `pdf-extract`, `ocrs` and `rten`, `rustwright`, `turso`, `async-imap`, `wasmi`.

## Not checked

* `cargo udeps`, so a dependency that no code names is not reported as `remove` except where the manifest or the code index showed it (`printpdf`, `chrono`).
* The stealth `spike/` crates, the plugin `template`, and `tools/mdx-check` (a Bun project of the docs check) in core: not reviewed.
* Feature-level trimming beyond what the manifests already do, and licence texts beyond the `license` field of each crate (the `cargo deny` licence check passes).
* Provenance of the crates (signatures, `cargo vet`), the CI workflow tool versions and the Docker base images.
* The crates.io owner count is the number of listed users and teams; the maintenance label uses release dates only, not issue activity.
* Newest-version and last-release data are from crates.io on 2026-11-06; several crates published a release that day (`usage-rs` 7.3.0, `psl` 2.1.248, `tauri` 3.0.0-alpha.6).
