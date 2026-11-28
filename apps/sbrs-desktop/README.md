# SeleniumBase Commander Desktop GUI

Desktop test runner and workflow companion for `seleniumbase-rs`, built with Tauri 3 (`tauri-runtime-wry`). It brings the original Python SeleniumBase desktop GUIs (`sbase commander`, `sbase behave-gui`, `sbase caseplans`, and `sbase recorder`) into a single native window.

## Features

- **Commander**: discovers `src/lib.rs`, `tests/*.rs`, `examples/*.rs`, and individual `#[test]` / `#[tokio::test]` / `#[rstest]` functions; supports multi-select queueing, `-k` filtering, `--features`, `--ignored`, browser and `SB_*` toggles (`SB_HEADLESS`, `SB_DEMO`, `SB_SLOW`, `SB_UC`, `SB_REUSE_SESSION`, `SB_INCOGNITO`, `SB_AD_BLOCK`), stop control, and live streaming console output.
- **Behave Commander**: discovers `features/**/*.feature` files and scenarios, filters by tag expression (`@smoke,-@slow`), runs checked scenarios via `cargo test`, and creates new `.feature` files.
- **Case Plans**: inspects, creates, and saves per-test `case_plans/<name>.md` markdown tables.
- **Recorder & Logs**: launches `sbrs mkrec <FILE>`, opens `sbrs recorder`, inspects `sbrs plugin list`, and previews `latest_logs/` failure screenshots (`screenshot.png`) and metadata (`basic_test_info.txt`).

## Build and Run

Because `apps/sbrs-desktop/src-tauri` declares its own `[workspace]`, its GUI and WebView dependencies stay isolated from the core library and CLI crates.

```bash
cd apps/sbrs-desktop
cargo tauri dev
```

To build a release bundle:

```bash
cd apps/sbrs-desktop
cargo tauri build
```

Run backend unit tests:

```bash
cd apps/sbrs-desktop/src-tauri
cargo test
```
