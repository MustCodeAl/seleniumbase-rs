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
is empty. For stealth and scraping, the unified `seleniumbase-rs-stealth`
facade re-exports this core API alongside `fingerprint`, `codec`, `scrape` and
optional `wire`. Use `seleniumbase_rs_stealth` paths with the
[single dependency](/scraping/stealthy/overview#which-crate-to-depend-on). Optional core
APIs retain their feature gates.

### Browser, configuration and test runner

| Item | What it is |
|---|---|
| `BaseCase` | The browser test case. `BaseCase::new(config).await` connects; `BaseCase::without_session(config)` does not. WebDriver is the default; `cdp_url` or `cdp_discovery` attaches the pure engine for `cdp_*` methods without starting a driver. |
| `BrowserConfig` | The session configuration. Fields and `with_*` builders are listed in [Getting Started](/tutorials/getting_started#configuration). `BrowserConfig::default()` reads no environment variables; `BrowserConfig::from_env()` returns `Result<BrowserConfig>` and honours the `SB_*` variables. |
| `Browser` | Enum of browsers: `Chrome` (default), `Chromium`, `Edge`, `Firefox`, `Brave`, `Comet`, `Opera`, `Safari`, `InternetExplorer`, `ChromeForTesting`, `ChromeHeadlessShell`. It parses from text such as `chrome`, `edge`, `ie`, `cft`, `chs` (`FromStr`). `is_chromium_based()` and `name()` are `const fn`s. |
| `DriverMode` | `WebDriver` (default), `Cdp` or `Uc` (undetected mode). |
| `BrowserSession` | Owns a WebDriver session or a directly attached pure CDP page. `BrowserSession::connect(config).await` is what `BaseCase::new` calls. `pure_cdp_page() -> Option<&sb_cdp::Page>` exposes the attached page and its browser; it returns `None` for WebDriver. |
| `BiDi`, `BidiNetworkEvent`, `DEFAULT_BIDI_BUFFER_SIZE` | W3C WebDriver BiDi handle (`thirtyfour::bidi::BiDi`), network lifecycle enum, and default channel buffer size (`256`). `BidiNetworkEvent` (`#[non_exhaustive]`) has variants `BeforeRequestSent`, `ResponseStarted`, `ResponseCompleted`, `FetchError`, and `AuthRequired`, with accessors `url()`, `method()`, `status()`, `request_id()`, and `timestamp()`. Returned by `BaseCase::bidi_listen_network` and `BrowserSession::bidi_listen_network`; see [Lifecycle and navigation](/api/lifecycle-navigation#webdriver-bidi). |
| `Element` | One element, found by either engine: what `BaseCase::find_element`, `find_elements`, `find_text` and the other element-returning methods give back, and what `BrowserSession::find` and the waits return. On the WebDriver engine it is the node the browser had when it was found, as Python's `WebElement` is: it does not wait, and when the page replaces the node its methods fail with `StaleElement` naming the selector. On a direct CDP attachment it wraps the pure CDP engine's `Locator`, which looks again on every use, so it is never stale; an element from `find_elements` is pinned to its position in the match list. Its methods carry the names of `sb_cdp::Locator`: `text` (an input's value, else the rendered text), `html`, `tag`, `attribute` (the markup's), `property`, `bounding_box`, `is_visible`, `is_enabled`, `is_selected`, `click`, `double_click`, `right_click`, `hover`, `clear`, `send_keys`, `type_text`, `press(Key)`, `select_option(SelectBy)`, `submit`, `scroll_into_view`, `drag_to(&Element)`, and `find(selector)`, `find_all(selector)` and `parent()` for the elements inside and above it. A search stays inside the element: an XPath that starts at the root of the page (`//td`), or a union with a branch that does, is refused with `InvalidSelector` (write `.//td`), and an `<iframe>` element's frame is not entered (`switch_to_frame` first). `as_webdriver_element()` and `into_webdriver_element()` give the raw `drivers::WebElement` for a call it has no method for (`None` on a direct CDP attachment, where `as_locator()` gives the `Locator`). It is `Clone` and `Send + Sync`, built only by the crate. |
| `Key`, `SelectBy` | The pure CDP engine's named key and option chooser (`sb_cdp::Key`, `sb_cdp::SelectBy`), at the crate root because `Element::press` and `Element::select_option` take them too. `Key` is the only key type this crate exports: `thirtyfour`'s own `Key` (the private-use code points a WebDriver session types) is not re-exported from `drivers`, and `Element::press` turns this one into them. |
| `Cookie`, `SameSite` | A browser cookie, one type for both engines (`sb_cdp::{Cookie, SameSite}` is the same pair). `BaseCase::cookie(name)` gives an `Option<Cookie>`, `None` for a cookie the page does not have; `cookies()` gives the cookies of the current page as a `Vec<Cookie>`, the same in DevTools and plain WebDriver mode; `add_cookie(&cookie)` takes one. `sb_cdp::Cookies` hands out and takes them too. Build one with `Cookie::new(name, value)` and the builders `domain`, `path`, `expires`, `http_only`, `secure` and `same_site`. The fields are public: `name`, `value`, `domain` (empty when the browser reported none), `path` (`/` by default), `expires` (`Option<f64>`, seconds since the epoch, `None` for a session cookie), `http_only`, `secure` and `same_site` (`Option<SameSite>`; `SameSite` is `Strict`, `Lax` or `None`). `Debug` hides the value. It deserializes from the W3C cookie object that Python saves: `expiry` is read as `expires`, and a missing `path`, `domain`, `secure` or `httpOnly` takes its default. |
| `NetworkThrottle` | How the browser's network behaves, one type for both engines (it lives in `browser::options`, beside `DeviceMetrics`): the fields are `offline` (every request fails at once), `latency` (a `Duration` added before each response starts to arrive, whole milliseconds), and `download` and `upload` (the most bytes per second the page may receive and send, `Option<u32>`, `None` for no limit). The default changes nothing, so it also puts the connection back. `BaseCase::set_network_throttle(&throttle)` and `BrowserSession::set_network_throttle` take it in every driver mode on a Chromium browser (Firefox, Safari and Internet Explorer answer `Unsupported`, naming the browser), and `sb_cdp::Emulation::network_throttle(&throttle)` on the pure CDP engine; `Emulation::offline(bool)` is a throttle that only sets `offline`. A throttle belongs to the tab it was set on: it is not repeated for a window the session switches to. |
| `RunSettings`, `RunSettingsBuilder`, `PauseKind` | Run-wide behaviour, Python's `--demo`, `--slow`, `--demo-sleep`, `--message-duration`, `--skip-js-waits`, `--record-sleep`, `--visual-baseline`, `--highlights`, `--interval`, `--verify-delay`, `--check-js`, `--wfa`, `--disable-beforeunload` and `--timeout-multiplier`: built with `RunSettings::builder().slow_mode(true).build()` and the other `RunSettingsBuilder` setters, given with `BrowserConfigBuilder::run_settings` or `BaseCase::set_run_settings`. The default changes nothing. `pause_for(PauseKind::Full \| Short)` is the pause after an action: full when the page changed, 1/3.4 of it otherwise in demo mode. |
| `Settings`, `SettingsOverrides` | The settings file (TOML or JSON) and the `SB_*` variables, in the groups `browser`, `session`, `run`, `test`, `report`, `fingerprint` and `plugins`: `Settings::load()` reads both, `load_from(path)` one file; `from_file(path)` the file alone and `from_env()` the variables alone; each source wins over the one before for the settings it gives. `check()` lists every problem, sorted by the rows of the option table. `validate()` refuses a window with one side only; a value that is refused names its source (the variable and flag, or the file and the key as the file wrote it). `to_browser_config()` makes the `BrowserConfig`. `plugins` is the `[plugins]` table (with `SB_PLUGINS`). A file also accepts the nested `[browser]` and `[session]` tables that `Settings` serializes to, beside the flat keys. `SettingsOverrides` is a list of `(SB_* variable, text)` pairs, what a command line asked for: `try_apply(&mut settings)` (or `Settings::apply_options(&pairs)`) lays them over everything else, and `env()` hands them to a child process. |
| `DriverId` | Names one browser of a `BaseCase`: `DriverId::DEFAULT` for the one it started with, or the id `new_driver` returned; `index()` is its position. It stays the same browser while the case lives. |
| `CookieExpiry` | What `load_cookies_with` does with expiries, Python's `expiry` argument: `Session` (the default, drop it), `Day` (24 hours), `Saved` (keep the saved one) or `Minutes(n)`. |
| `run_browser_test(config, test)` | Starts a `BaseCase`, runs `test` (an async closure over `&mut BaseCase`), always calls `quit`, and returns the test's error, the cleanup error, or both. A panic in the test still closes the browser and is then raised again. A test that ends with `skip_test` counts as passed. The run settings of `config` (`run_settings`) go to the case first. |
| `run_cdp_test(options, test)` | The same for the Pure CDP engine: launches a browser, runs `test` (an async closure over `&Page`) on its first tab and always closes the browser. `Plugins::run_cdp_test` adds the plugins. |
| `shutdown_reused_sessions() -> usize` | Closes every idle browser kept for `reuse_session` and returns how many it closed. A browser a test still holds is left alone. |
| `install_tls_provider()` | Installs `ring` as the process-wide rustls crypto provider unless one is installed. Call it before you build your own `reqwest` client in a program that uses this crate, which builds `reqwest` without a provider; a client built before any provider is set panics. |
| `RuntimeConfig`, `LogFormat` | Environment configuration for logging, shutdown and the driver (below); the browser variables `SB_WEBDRIVER_URL`, `SB_CHROME_BIN` and `SB_REMOTE_WEBDRIVER_TIMEOUT` are settings, read by `Settings` and `BrowserConfig::from_env()` / `from_vars(lookup)`. `LogFormat` is `Pretty` (default) or `Json`. |
| `Gui` (feature `gui`) | Native mouse and keyboard through `enigo`. Not stored in `BaseCase`; build a fresh one where you need it. |

### Errors

| Item | What it is |
|---|---|
| `Result<T>` | `std::result::Result<T, SeleniumBaseError>`. |
| `SeleniumBaseError` | The one error type (a `thiserror` enum). Variants: `ElementNotFound`, `ElementNotInteractable`, `StaleElement`, `InvalidSelector`, `WaitTimeout`, `BrowserLaunch`, `BrowserDisconnected`, `SessionNotStarted`, `Navigation`, `WebDriver`, `CdpDriver`, `Playwright`, `Patcher`, `Stealth`, `Network`, `Download`, `Io`, `Json`, `Parse`, `InvalidConfig`, `AssertionFailed`, `Unsupported`, `Gui`, `Screenshot`, `Pdf`, `Authentication`, `Mcp`, `PythonMigration`, `Skipped`, `TestLifecycle`, `Database`, `DatabaseDriver`, `Plugin`, `Context`. |
| `DriverError`, `DriverErrorKind` | What `SeleniumBaseError::WebDriver` holds: what the driver (chromedriver, geckodriver, msedgedriver) answered when it failed a command. `kind()` is a `#[non_exhaustive]` `DriverErrorKind` in this crate's words: `ElementNotFound`, `StaleElement`, `ElementNotInteractable` (also a click that was intercepted, an element in an invalid state, a move target out of bounds), `FrameNotFound`, `WindowNotFound`, `AlertNotFound`, `CookieNotFound`, `ScriptFailed`, `TimedOut`, `BrowserLaunch`, `InvalidArgument`, `Unreachable` and `Other`; `message()` is the driver's text, which `{}` prints. `TimedOut` is the driver's command or script running out of time, not `WaitTimeout`, which is a wait of this crate running out. Two answers need no context and have a variant of their own: the driver's `invalid session id` is `BrowserDisconnected` and its `invalid selector` is `InvalidSelector`, so the waits that poll through the crate's shared helper (`wait_for_any_of_elements_present`, `wait_for_title`, `switch_to_window` and others) end with them at once. The `thirtyfour` error is the `source()` and no public item names its type; `From<drivers::WebDriverError> for SeleniumBaseError` stays, so `?` turns the result of an escape hatch (`Element::as_webdriver_element`, `BrowserSession::webdriver`) into one. |
| Error helpers | `is_transient()` is true for `BrowserDisconnected`, `Network`, `Download` and `WaitTimeout`. `is_skipped()`, `is_element_error()`, `category()` (a stable snake_case tag such as `wait_timeout`), `hint()` (`Option<String>`), `exit_code()` (`u8`), `log()` and `log_in_context(operation)` (a structured `tracing::error!`). Constructors such as `SeleniumBaseError::element_not_found(selector)`, `wait_timeout(operation, duration)`, `invalid_config(reason)`, `assertion_failed(reason)`. |
| `ResultExt<T, E>` | `E: Error + Send + Sync + 'static`. `sb_context(context)` turns the error into `Context` (`{context}: {error}`), and `source()` is the original error. A `SeleniumBaseError` cause keeps its category, exit code, hint and transient flag. `log_err()` logs it with `tracing::error!` and passes it through. |

### Retry, selectors and logging

| Item | What it is |
|---|---|
| `RetryPolicy` | Fields `max_attempts`, `base_delay`, `backoff_factor`, `max_delay`. `RetryPolicy::default()` is 3 attempts, 200 ms, factor 2.0, at most 5 s. Presets: `webdriver()` (3, 250 ms, 3 s), `network()` (5, 500 ms, 10 s), `seleniumbase()` (6, 1 s, 32 s, as Python's `retry_on_exception`), `every(interval)` (a fixed interval that never gives up, for `poll_until`). `attempts()` is never below 1, `delay_for_attempt(n)` is zero for the first attempt, and `retry(..)` and `retry_blocking(..)` run an operation again, with backoff, while a predicate says the error is worth retrying. `utilities::retry` also has `poll_until` and `RateLimiter`. |
| `By` | The 8 standard locator strategies defined by W3C WebDriver and Selenium: variants `Id`, `Name`, `XPath`, `LinkText`, `PartialLinkText`, `TagName`, `ClassName`, `Css` and associated string constants `By::ID`, `By::NAME`, `By::XPATH`, `By::LINK_TEXT`, `By::PARTIAL_LINK_TEXT`, `By::TAG_NAME`, `By::CLASS_NAME`, `By::CSS_SELECTOR`. `By` formats to its canonical strategy name with `Display`, parses from strategy strings with `FromStr`, and constructs selectors via `by.selector(val)` or `by.selector_ref(val)`. |
| `Keys` | Special keyboard key constants matching Python Selenium's `selenium.webdriver.common.keys.Keys`: `Keys::ENTER`, `Keys::RETURN`, `Keys::TAB`, `Keys::BACKSPACE`, `Keys::ESCAPE`, `Keys::SPACE`, `Keys::ARROW_UP`, `Keys::ARROW_DOWN`, `Keys::ARROW_LEFT`, `Keys::ARROW_RIGHT`, `Keys::HOME`, `Keys::END`, `Keys::PAGE_UP`, `Keys::PAGE_DOWN`, `Keys::INSERT`, `Keys::DELETE`, `Keys::SHIFT`, `Keys::CONTROL`, `Keys::ALT`, `Keys::META`/`COMMAND`, `Keys::F1`..`Keys::F12`, and numpad keys (`Keys::NUMPAD0`..`NUMPAD9`, `ADD`, `SUBTRACT`, etc.). `Keys::to_key(ch)` converts code points to native engine `Key`s. See [Keys](/api/keys). |
| `Select` | Select element wrapper matching Python Selenium's `selenium.webdriver.support.select.Select`: provides `select_by_visible_text`, `select_by_value`, `select_by_index`, `options`, `all_selected_options`, `first_selected_option`, and multi-select deselectors `deselect_all`, `deselect_by_visible_text`, `deselect_by_value`, `deselect_by_index`. |
| `Color` | CSS color parser and converter matching Python Selenium's `selenium.webdriver.support.color.Color`: parses hex strings (`#rgb`, `#rrggbb`, `#rgba`, `#rrggbbaa`), `rgb(...)`, `rgba(...)`, and CSS named colors (`red`, `blue`, `transparent`, etc.), converting to `.hex()`, `.rgb()`, and `.rgba()`. |
| `Alert` | Representation of an active JavaScript dialog alert/prompt/confirm: provides `.text().await`, `.accept().await`, `.dismiss().await`, and `.send_keys(text).await`. Returned by `sb.switch_to_alert().await`. |
| `Selector`, `SelectorRef` | Owned and borrowed element selectors supporting all 8 locator strategies (`Id`, `Name`, `XPath`, `LinkText`, `PartialLinkText`, `TagName`, `ClassName`, `Css`). `SelectorRef::auto(text)` and `Selector::from(text)` guess from the start of the string: `link=`, `link_text=`, `text=` is exact link text; `partial_link=`, `partial_link_text=`, `partial_text=`, `p_link=`, `p_link_text=`, `p_text=` is partial link text; `/`, `./`, `../`, `(` or an XPath axis such as `parent::` is XPath; anything else is CSS. A selector that crosses a shadow root (`host::shadow inner`) is CSS as a whole, and one that ends on `::shadow` is refused. Methods accepting selectors take `impl Into<Selector>`, supporting plain strings, `SelectorRef`, explicit constructors (`Selector::id`, `Selector::name`, `Selector::tag_name`, `Selector::class_name`, `Selector::xpath`, `Selector::css`, `Selector::link_text`, `Selector::partial_link_text`), tuple pairs `(By, value)` or `(&str, value)` (e.g. `(By::ID, "submit")`), and the `selector!` macro. `From` never fails and keeps the text as written; `FromStr` (`"#a".parse::<Selector>()`) is the strict form for text from outside (an MCP argument, a saved file) and refuses an empty selector or one that ends on `::shadow` with `InvalidSelector`. The `:contains("text")` pseudo-class is not CSS: a selector keeps its text, and `Selector::lookup_form` is the one place it becomes XPath when an engine looks the element up. |
| `init_tracing()`, `init_tracing_json()` | Install the global `tracing` subscriber (text or JSON) and bridge `log`. They read `RUST_LOG` (default `info`) and ignore a second call. |
| `init_tracing_with_filter(filter)`, `init_tracing_json_with_filter(filter)` | The same with an `EnvFilter` string; an unparsable filter falls back to `RUST_LOG`, then `info`. The process environment is not changed. |
| `init_tracing_from_runtime(&RuntimeConfig)` | Uses `SB_LOG_LEVEL` and `SB_LOG_FORMAT`; without a valid `SB_LOG_LEVEL` the filter is `RUST_LOG`, then `info`. |
| `UPSTREAM_SELENIUMBASE_VERSION` | The constant `"4.58.3"`, the Python SeleniumBase version the API tracks. |

### Macros

Macros exported at the crate root for concise test writing, configuration, and interactions:

| Item | What it is |
|---|---|
| `selector!` | Builds a `SelectorRef` variant at compile time (`selector!(css, "#id")`, `selector!(xpath, "//div")`, `selector!(id, "name")`, `selector!(link, "text")`). |
| `sb_test!` | Declares an async `#[tokio::test]` that launches a browser via `run_browser_test`, executes the test block, and guarantees clean browser termination via `quit()`. |
| `uc_config!` | Builds a default `BrowserConfig` configured for undetected (UC) mode. |
| `cdp_config!` | Builds a default `BrowserConfig` configured for pure CDP mode. |
| `sb_config!` | Ergonomic macro for constructing `BrowserConfig` with chained settings (`mode`, `headless`, `browser`, etc.). |
| `fingerprint!` | Instant constructor for built-in OS fingerprint presets (`fingerprint!(windows)`, `fingerprint!(macos)`, `fingerprint!(linux)`, `fingerprint!(android)`, `fingerprint!(ios)`). |
| `sb_fill_form!` | Fills sequential form fields with key-value pairs (`sb_fill_form!(sb, "#user" => "alice", "#pass" => "secret")`). |
| `sb_with_timeout!` | Runs an asynchronous block with a temporary override of the default wait timeout. |
| Action macros | Ergonomic wrappers calling `BaseCase` methods: `sb_open!`, `sb_click!`, `sb_type!`, `sb_hover!`, `sb_scroll_to!`, `sb_wait_for!`, `sb_wait_and_click!`, `sb_select!`, `sb_assert_text!`, `sb_assert_title!`, `sb_assert_title_contains!`, `sb_assert_url!`, `sb_assert_url_contains!`, `sb_assert_visible!`, `sb_assert_not_visible!`, `sb_screenshot!`, `sb_js!`, `sb_quit!`. |

### Charts and tours

| Item | What it is |
|---|---|
| `Chart`, `ChartType` | A chart that renders to HTML with Chart.js: `Chart::new(title, chart_type)` with `ChartType::Pie`, `Bar` (horizontal), `Line`, `Area` or `Column` (vertical). Data points are `(label, i32)` pairs; extra series go in `extra_series`. See [Charts](/tutorials/charts). |
| `TourTheme` | `SeleniumBase` (default), `Shepherd`, `IntroJs`, `DriverJs`, `Bootstrap`, `Hopscotch`; `as_str()` gives the lowercase name. See [Tours](/tutorials/tours). |

### Fingerprint and stealth

These are re-exported for convenience; the [Fingerprint & Stealth Profiles](/stealth/introduction)
tutorial is the reference for them.

| Item | What it is |
|---|---|
| Fingerprint types | `Fingerprint` (built once with `Fingerprint::builder()`, read through accessors, changed with `to_builder()`), `FingerprintBuilder`, `FingerprintError`, `Spoof<T>` (each setting: `Natural`, `Random`, `Value(T)`, `Mimic(Specimen)` or `Off`), `Specimen` (mimic target profile), `StealthPolicy` (how the claims are delivered; presets `default()`, `balanced()`, `native_only()`, `max()`), `Dimension`, and the value types `ScreenSize`, `Geolocation`, `WebGl`, `WebRtcAddresses`, `MediaDevices`, `ClientHints`, `BrandVersion`, `ProxyConfig`, `BatteryProfile`, `ConnectionProfile`, `SpeechVoice`, `OsType`, `BrowserType`; the policy enums `CanvasNoise`, `AudioNoise`, `QuicMode`, `WebRtcPolicy`; `HumanizeConfig` and `CoherenceReport`. |
| Evasion providers | `default_registry()` returns an `EvasionRegistry` with every built-in provider. `session_registry(&Fingerprint)` is the registry for a pure CDP tab: it leaves out the providers named in `SESSION_OMITTED_PROVIDERS` (`"plugins"` and `"connection"`), because Chrome already answers for those itself. `EvasionProvider` is the trait for your own, `EvasionContext` the data a provider's script is built from, `EvasionConfig` its settings. |
| Patching | `find_system_chrome() -> Option<PathBuf>` and `find_system_chrome_with(...)` (checks `CHROME_BIN` and `CHROMIUM_BIN`, well-known paths, then `which`), `engine_spoofing_args() -> Vec<String>` (launch flags that reduce engine-level automation signals), `ChromeBinaryPatcher`, `ChromedriverPatcher`, and `EnginePatch`. Diagnostic and rule types: `PatchPreset` (`Balanced`, `All`, `Aggressive`, `ChromeBinary`, with `as_str()`, `to_patch()`, and `FromStr`), `BinaryFormat` (`Elf`, `MachO`, `Pe`, `Unknown`, with `detect(bytes)` and `as_str()`), `BinaryPatchRule` (`try_new(needle, replacement)` and `neutralize(needle)` enforcing equal byte lengths), `MarkerCounts` (per-category counts across 12 marker families plus `total()`), `BinaryMarkerReport` (`inspect(path)`, `from_bytes(path, content)`, `needs_patch()`, `has_any_markers()`), and `PatchedBrowserBundle` (`executable` and optional `companion` library paths). See [Binary patching](/stealth/advanced/binary-patching). |

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
`browser`, `cli`, `common`, `config`, `core`, `drivers` (the raw `WebDriver`,
`WebElement` and `WebDriverError` of the escape hatches), `error`, `evasion`, `js_code`, `json` (`json!` and `Value`), `macros`,
`tokio` (the runtime, re-exported so `#[tokio::main]` needs no `tokio` line; see
[Imports and dependencies](/tutorials/getting_started#imports-and-dependencies)),
`plugins` (test-reporting plugins), `profile_payloads`, `resources`,
`sb_cdp` (the [pure CDP engine](/tutorials/pure_cdp)), `tracing_util`,
`utilities`, `utils`; and, behind features, `mcp` (`mcp-server`), `ocr` (`ocr`),
`plugin_api` (`plugins`, the browser plugin API), `builtin_plugins` (any of
`plugin-shield`, `plugin-block-resources`, `plugin-crx`, `plugin-fetcher`,
`plugin-extractor`, `plugin-batcher`, `plugin-crawler`, `plugin-browser`, `plugin-imap`, `plugin-tor` or `plugins-builtin`: the
official plugins and their `registry()`, see [Choosing plugins](/plugins/selecting)),
`storage` (`turso`) and `visual` (`visual`).
