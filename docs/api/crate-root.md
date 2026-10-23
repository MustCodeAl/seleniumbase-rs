---
title: "Public items of the crate root"
sidebarTitle: "Crate root"
description: "What seleniumbase_rs re-exports at its root and what each item is for."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Public items of the crate root

Everything below is importable as `seleniumbase_rs::Name` (from `src/lib.rs`).
Items marked with a feature need that Cargo feature; the default feature set
is empty.

### Browser, configuration and test runner

| Item | What it is |
|---|---|
| `BaseCase` | The WebDriver-engine test case; every method in the tables above. `BaseCase::new(config).await` connects, `BaseCase::without_session(config)` does not. |
| `BrowserConfig` | The session configuration. Fields and `with_*` builders are listed in [Getting Started](/tutorials/getting_started#configuration). `BrowserConfig::default()` reads no environment variables; `BrowserConfig::from_env()` returns `Result<BrowserConfig>` and honours the `SB_*` variables. |
| `Browser` | Enum of browsers: `Chrome` (default), `Chromium`, `Edge`, `Firefox`, `Brave`, `Comet`, `Opera`, `Safari`, `InternetExplorer`, `ChromeForTesting`, `ChromeHeadlessShell`. It parses from text such as `chrome`, `edge`, `ie`, `cft`, `chs` (`FromStr`). `is_chromium_based()` and `name()` are `const fn`s. |
| `DriverMode` | `WebDriver` (default), `Cdp` or `Uc` (undetected mode). |
| `BrowserSession` | Owns the WebDriver session, an optional CDP client and an optional auto-started driver process. `BrowserSession::connect(config).await` is what `BaseCase::new` calls. |
| `RunSettings`, `PauseKind` | Run-wide behaviour, Python's `--demo`, `--slow`, `--demo-sleep`, `--message-duration`, `--skip-js-waits`, `--record-sleep`, `--visual-baseline`, `--highlights`, `--interval`, `--verify-delay`, `--check-js`, `--wfa`, `--disable-beforeunload` and `--timeout-multiplier`: built with `RunSettings::default().with_slow_mode(true)` and the other `with_*` methods, given with `BrowserConfig::with_run_settings` or `BaseCase::set_run_settings`. The default changes nothing. `pause_for(PauseKind::Full \| Short)` is the pause after an action: full when the page changed, 1/3.4 of it otherwise in demo mode. |
| `Settings`, `SettingsOverrides` | The settings file (TOML or JSON) and the `SB_*` variables: `Settings::load()` reads both, `load_from(path)` one file; `validate()` refuses an unknown browser or mode; `to_browser_config()` makes the `BrowserConfig`. `plugins` is the `[plugins]` table (with `SB_PLUGINS`). `SettingsOverrides::apply(&mut settings)` lays command-line values over them. |
| `DriverId` | Names one browser of a `BaseCase`: `DriverId::DEFAULT` for the one it started with, or the id `get_new_driver` returned; `index()` is its position. It stays the same browser while the case lives. |
| `CookieExpiry` | What `load_cookies_with` does with expiries, Python's `expiry` argument: `Session` (the default, drop it), `Day` (24 hours), `Saved` (keep the saved one) or `Minutes(n)`. |
| `run_browser_test(config, test)` | Starts a `BaseCase`, runs `test` (an async closure over `&mut BaseCase`), always calls `quit`, and returns the test's error, the cleanup error, or both. A panic in the test still closes the browser and is then raised again. A test that ends with `skip_test` counts as passed. The run settings of `config` (`with_run_settings`) go to the case first. |
| `run_cdp_test(options, test)` | The same for the Pure CDP engine: launches a browser, runs `test` (an async closure over `&Page`) on its first tab and always closes the browser. `Plugins::run_cdp_test` adds the plugins. |
| `shutdown_reused_sessions() -> usize` | Closes every idle browser kept for `reuse_session` and returns how many it closed. A browser a test still holds is left alone. |
| `install_tls_provider()` | Installs `ring` as the process-wide rustls crypto provider unless one is installed. Call it before you build your own `reqwest` client in a program that uses this crate, which builds `reqwest` without a provider; a client built before any provider is set panics. |
| `RuntimeConfig`, `LogFormat` | Environment configuration (below). `LogFormat` is `Pretty` (default) or `Json`. |
| `Gui` (feature `gui`) | Native mouse and keyboard through `enigo`. Not stored in `BaseCase`; build a fresh one where you need it. |

### Errors

| Item | What it is |
|---|---|
| `Result<T>` | `std::result::Result<T, SeleniumBaseError>`. |
| `SeleniumBaseError` | The one error type (a `thiserror` enum). Variants: `ElementNotFound`, `ElementNotInteractable`, `StaleElement`, `InvalidSelector`, `WaitTimeout`, `BrowserLaunch`, `BrowserDisconnected`, `SessionNotStarted`, `Navigation`, `WebDriver`, `CdpDriver`, `Playwright`, `Patcher`, `Stealth`, `Network`, `Download`, `Io`, `Json`, `Parse`, `InvalidConfig`, `AssertionFailed`, `Unsupported`, `Gui`, `Screenshot`, `Pdf`, `Authentication`, `Mcp`, `PythonMigration`, `Skipped`, `TestLifecycle`, `Database`, `Plugin`. |
| Error helpers | `is_transient()` is true for `BrowserDisconnected`, `Network`, `Download` and `WaitTimeout`. `is_skipped()`, `is_element_error()`, `category()` (a stable snake_case tag such as `wait_timeout`), `hint()` (`Option<String>`), `exit_code()` (`u8`), `log()` and `log_in_context(operation)` (a structured `tracing::error!`). Constructors such as `SeleniumBaseError::element_not_found(selector)`, `wait_timeout(operation, duration)`, `invalid_config(reason)`, `assertion_failed(reason)`. |
| `ResultExt<T, E>` | On any `Result<T, E: Display>`: `sb_context(context)` turns the error into `InvalidConfig("context: error")`, and `log_err()` logs it with `tracing::error!` and passes it through. |

### Retry, selectors and logging

| Item | What it is |
|---|---|
| `RetryPolicy` | Fields `max_attempts`, `base_delay`, `backoff_factor`, `max_delay`. `RetryPolicy::default()` is 3 attempts, 200 ms, factor 2.0, at most 5 s. Presets: `webdriver()` (3, 250 ms, 3 s), `network()` (5, 500 ms, 10 s), `seleniumbase()` (6, 1 s, 32 s, as Python's `retry_on_exception`), `every(interval)` (a fixed interval that never gives up, for `poll_until`). `attempts()` is never below 1, `delay_for_attempt(n)` is zero for the first attempt, and `retry(..)` and `retry_blocking(..)` run an operation again, with backoff, while a predicate says the error is worth retrying. `utilities::retry` also has `poll_until` and `RateLimiter`. |
| `Selector` | A borrowed selector with its kind decided: `LinkText`, `PartialLinkText`, `Css`, `XPath`, `Id`. `Selector::auto(text)` guesses from the start of the string: `link=`, `link_text=`, `text=` is exact link text; `partial_link=`, `partial_link_text=`, `partial_text=`, `p_link=`, `p_link_text=`, `p_text=` is partial link text; `/`, `./`, `../`, `(` or an XPath axis such as `parent::` is XPath; anything else is CSS. `to_by()` and `auto_by(text)` return a `thirtyfour::By` and fail with `InvalidSelector` for empty text. The pure CDP engine uses the owned twin `utils::selectors::SelectorBuf`. |
| `init_tracing()`, `init_tracing_json()` | Install the global `tracing` subscriber (text or JSON) and bridge `log`. They read `RUST_LOG` (default `info`) and ignore a second call. |
| `init_tracing_with_filter(filter)`, `init_tracing_json_with_filter(filter)` | The same with an `EnvFilter` string; an unparsable filter falls back to `RUST_LOG`, then `info`. The process environment is not changed. |
| `init_tracing_from_runtime(&RuntimeConfig)` | Uses `SB_LOG_LEVEL` and `SB_LOG_FORMAT`; without a valid `SB_LOG_LEVEL` the filter is `RUST_LOG`, then `info`. |
| `UPSTREAM_SELENIUMBASE_VERSION` | The constant `"4.55.2"`, the Python SeleniumBase version the API tracks. |

### Charts and tours

| Item | What it is |
|---|---|
| `Chart`, `ChartType` | A chart that renders to HTML with Chart.js: `Chart::new(title, chart_type)` with `ChartType::Pie`, `Bar` (horizontal), `Line`, `Area` or `Column` (vertical). Data points are `(label, i32)` pairs; extra series go in `extra_series`. See [Charts](/tutorials/charts). |
| `TourTheme` | `SeleniumBase` (default), `Shepherd`, `IntroJs`, `DriverJs`, `Bootstrap`, `Hopscotch`; `as_str()` gives the lowercase name. See [Tours](/tutorials/tours). |

### Fingerprint and stealth

These are re-exported for convenience; the [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth)
tutorial is the reference for them.

| Item | What it is |
|---|---|
| Fingerprint types | `Fingerprint` (built once with `Fingerprint::builder()`, read through accessors, changed with `to_builder()`), `FingerprintBuilder`, `FingerprintError`, `Spoof<T>` (each setting: `Natural`, `Random`, `Value(T)` or `Off`), `StealthPolicy` (how the claims are delivered; presets `default()`, `balanced()`, `native_only()`, `max()`), `Dimension`, and the value types `ScreenSize`, `Geolocation`, `WebGl`, `WebRtcAddresses`, `MediaDevices`, `ClientHints`, `BrandVersion`, `ProxyConfig`, `BatteryProfile`, `ConnectionProfile`, `SpeechVoice`, `OsType`, `BrowserType`; the policy enums `CanvasNoise`, `AudioNoise`, `QuicMode`, `WebRtcPolicy`; `HumanizeConfig` and `CoherenceReport`. |
| Evasion providers | `default_registry()` returns an `EvasionRegistry` with every built-in provider. `session_registry(&Fingerprint)` is the registry for a pure CDP tab: it leaves out the providers named in `SESSION_OMITTED_PROVIDERS` (`"plugins"` and `"connection"`), because Chrome already answers for those itself. `EvasionProvider` is the trait for your own, `EvasionContext` the data a provider's script is built from, `EvasionConfig` its settings. |
| Patching | `find_system_chrome() -> Option<PathBuf>` (checks `CHROME_BIN` and `CHROMIUM_BIN`, well-known paths, then `which`), `engine_spoofing_args() -> Vec<String>` (launch flags that reduce engine-level automation signals), `ChromeBinaryPatcher`, `ChromedriverPatcher`, `EnginePatch`. See [Binary patching](/tutorials/binary_patching). |

### Python migration

`import_python(source: &str, options: &ImportOptions) -> ImportResult` converts
common Python SeleniumBase or Selenium statements into a Rust test.
`ImportOptions` has `source: PythonSource` (`Auto` by default, `SeleniumBase` or
`Selenium`) and `test_name` (default `imported_python_test`). `ImportResult` has
`rust` (the generated source), `diagnostics: Vec<ImportDiagnostic>` (each with
`line`, `severity` of `Warning` or `Error`, and `message`) and `detected_source`;
`is_complete()` is true when there are no error-level diagnostics. Dynamic
Python and unsupported locators stay in the output as `TODO` comments with a
diagnostic.

### Modules

The remaining public API sits in modules: `api`, `artifacts`, `behave`,
`browser`, `cli`, `common`, `config`, `core`, `error`, `js_code`, `macros`,
`plugins` (test-reporting plugins), `profile_payloads`, `resources`,
`sb_cdp` (the [pure CDP engine](/tutorials/pure_cdp)), `stealth`, `tracing_util`,
`utilities`, `utils`; and, behind features, `mcp` (`mcp-server`), `ocr` (`ocr`),
`plugin_api` (`plugins`, the browser plugin API), `builtin_plugins` (any of
`plugin-shield`, `plugin-block-resources`, `plugin-crx` or `plugins-builtin`: the
official plugins and their `registry()`, see [Choosing plugins](/plugins/selecting)),
`storage` (`turso`) and `visual` (`visual`).
