---
title: "Design: settings regrouped and layered with figment"
sidebarTitle: "Settings with figment"
---

Status: design note, written before the code (branch `lane/design-settings`, from `90ca478e`).
Every line number below is from that commit. Implemented on `lane/settings-v2`: the pins (step 1), the report table (step 2), the run group (step 3), the `browser` and `session` groups of `BrowserConfig` (step 4: `BrowserSettings`, `SessionSettings`, `GridSettings`, one Grid address function), the `SB_NO_SANDBOX` rule, and findings 1, 2, 3, 7, 8, 9, 11 and 13, finding 4 (`to_launch_options` goes through `to_browser_config`) and finding 10 (the higher source decides between the spellings of the window size and of the language; done in `apply_env`, one source at a time, which is what the figment layering will do by construction), step 5 (`Settings` holds the core: `Settings::browser` and `session` are `BrowserSettings` and `SessionSettings`, the field column of the option table is the dotted path, a row kind writes one entry of `browser.switches`, `config::text` reads the text of a variable or of a flat file key into the typed field, `with_launch_options`, `check_launch_values` and `base_browser_config` are gone, and `check_config` lists the paths with their variables, open question 13); not yet: the figment layering (step 6; the nested `[browser]` and `[session]` tables of a settings file, the `key` column of the table, `SettingsOverrides` as pairs), the docs pages (step 7), and the part of finding 12 that is not the `LaunchSettings` rename. What figment does was
read on Context7 (the pages are listed under [Sources](#sources)); what those pages did not say is
marked "to verify". Statements about today's behaviour come from reading the code; none was run,
and each one that matters gets a test in [step 1](#implementation-steps) before anything moves.

## Goal

The owner's goals, as given:

1. Group `Settings` (`src/config/settings.rs:73-271`, 85 flat fields) into nested parts.
2. Compare `Settings`, `BrowserConfig` (`src/browser/config.rs:175-277`), `LaunchOptions` with
   its `Layer` (`src/sb_cdp/launch.rs:181-245`, `src/sb_cdp/launch/layers.rs:72-110`) and
   `TestSettings` (`src/config/test_settings.rs:175-241`) field by field, and extract a shared core
   only where they truly overlap.
3. Layer the sources with [figment](https://github.com/sergiobenitez/figment): defaults, then the
   settings file, then the `SB_*` environment, then explicit code, each winning over the one
   before.
4. Keep every existing `SB_*` variable name and every settings-file key working.
5. Break the Rust API freely (no users yet, no shims). Names follow this project's patterns: one
   name, one meaning; plain setter names on builders, `with_*` on owned values.

The comparison found two more types that overlap the four, and they are in the tables too:
`ReportSettings` (`src/core/test_run.rs:132-159`), which repeats 11 of the fields of
`TestSettings`, and `RuntimeConfig` (`src/config/runtime.rs:15-46`), which reads three of the
variables of `Settings` a second time.

## The answer in short

* **Settings becomes seven groups**: `browser`, `session`, `run`, `test`, `report`, `fingerprint`
  and `plugins` (plus `settings_file`, which names the file and is never read from one).
* **The shared core is three types that `Settings` and `BrowserConfig` both hold**:
  `BrowserSettings` (what both engines start: today's `browser::options::LaunchSettings`, renamed,
  with the launch fields of `BrowserConfig` moved in), `SessionSettings` (what only a `BaseCase`
  session reads: the driver mode, the WebDriver server, the Grid, reuse, capabilities) and
  `RunSettings` (`src/api/run_settings.rs:77-94`, which `Settings` today copies field by field).
  `Settings::to_browser_config` becomes three clones. `Layer` keeps its one merge rule
  (`layers.rs:121-166`) and reads the core through one function; the second mapping of
  `Settings::to_launch_options` (`src/config/launch_map.rs:261-339`) goes.
* **figment merges the sources** through two providers of our own, `SettingsFile` and `EnvVars`,
  because figment's `Env` and `Toml` providers cannot read our names (see
  [figment](#figment)). The option table (`src/config/test_options.rs:390-547`) stays the single
  list of variables, flags and file keys; each row gains the new field path.
* **No variable and no file key changes.** Flat keys (`dark_mode = true`) keep working; the
  nested form (`[browser.switches] dark-mode = true`) is accepted too.

## Today

### Where the settings are read

* `Settings::load` (`settings.rs:818-820`) and `load_from` (`:829-831`) call `load_with`
  (`:897-911`): the file named by `SB_SETTINGS_FILE` or found by `global_file_with` (`:801-809`:
  `sbase_config.toml`, `.sbase_config.toml`, `.sbase_config`), then `apply_overrides`
  (`:1058-1072`), then `validate` (`:670-695`), the plugin check and the fingerprint check.
* The file is read by `parse_settings` (`src/config/browser_options.rs:197-219`): TOML for a
  `.toml` name, JSON otherwise. `canonical_entries` (`:173-190`) renames each top-level key with
  `settings_key` (`:137-159`): a key may use dashes or underscores, any case, and any flag or
  `SB(...)` name of its row. An unknown key is refused (`unknown_setting`, `:162-170`), and two
  spellings of one setting are refused (`:181-184`). The tables of the file are `TABLES`
  (`:39`): `plugins`, `fingerprint`, `test`.
* The variables are read by `apply_env`, generated by `option_table!`
  (`test_options.rs:343-381`) from the rows of `sb_test_options!` (`:390-547`): 141 rows, of which
  114 read a variable (the 13 shorthands such as `--firefox` and the 14 rows with no variable do
  not). Each value is written by `OptionValue::assign` (`:173-256`) for the field's type; a blank
  value counts as unset; a value that does not parse becomes a `SettingsProblem` (`:275-289`),
  named with its flag by `named` (`settings.rs:1165-1178`).
* `SB_PLUGINS` is read apart (`settings.rs:1127`, `:1131-1134`): it replaces the `enable` list of
  the `[plugins]` table and keeps the plugin options; an empty value means no plugin.
* `Settings::check` (`:921-973`) reads everything leniently and reports every problem.
* `SettingsOverrides` (`:301-347`) is what the command line asked for: 18 typed fields and
  `options`, the other flags as `(SB_*, text)` pairs (`try_apply`, `:436-439`; `env`,
  `:449-480`).
* `to_browser_config` (`:980-988`) is `base_browser_config` (`:992-1045`) plus
  `with_launch_options` (`launch_map.rs:44-216`), which parses the text fields and calls the
  `with_*` builders of `BrowserConfig`. `to_launch_options` (`launch_map.rs:261-339`) builds pure
  CDP options from the same fields by a second, shorter mapping.

### Findings

Read from the code, not run; step 1 turns each into a test.

1. **A `[report]` table is refused by the loader.** `Settings::report` (`settings.rs:267-270`)
   says it is the `[report]` table, but `TABLES` (`browser_options.rs:39`) has no `report`, so
   `canonical_entries` refuses the key. The test that reads one
   (`tests/offline/reporting_run.rs:742-762`) uses `toml::from_str`, which skips
   `parse_settings`.
2. **The `[test]` table checks no key.** `TestSettings` is `#[serde(default)]` without
   `deny_unknown_fields` (`test_settings.rs:175-176`), so `[test] rerun = 2` is dropped without a
   word, while a misspelt top-level key is refused. Aliases (`ftrace`) are not accepted there.
3. **The report options are declared twice and read twice.** `TestSettings` and `ReportSettings`
   both hold `dashboard`, `dash_title`, `html`, `junit_xml`, `allure_dir`, `archive_logs`,
   `save_screenshot`, `no_screenshot`, `with_db_reporting`, `database_env` and the fail-page
   switch, which is `list_fail_page` in one (`test_settings.rs:203`) and `list_fail_pages` in the
   other (`test_run.rs:146`). `SB_HTML`, `SB_JUNIT_XML` and `SB_ALLURE_DIR` are read by
   `apply_env` and again by `ReportSettings::from_settings_with` (`test_run.rs:172-208`).
4. **`to_launch_options` maps less than `Layer::from_config`.** It passes the binary, headless,
   start page, incognito, guest, ad block, sandbox, proxy, profile, user agent, language, window,
   extensions, debugging port, fingerprint, plugins, and the arguments of `switch_args`
   (`src/browser/options/chromium_args.rs:64-130`). It does not pass `mobile`, `device_metrics`,
   `proxy_pac_url`, `proxy_bypass_list`, the virtual display, nor the switches that are
   preferences (`block_images`, `disable_js`, `disable_cookies`, `do_not_track`, `external_pdf`,
   `disable_csp`): `switch_args` makes no argument for them and the builder's `switch` is not
   called. `Layer::from_config` (`src/sb_cdp/launch/bridge.rs:103-189`) passes all of them. So
   `SB_MOBILE=true` seems to reach a `BaseCase` and not an `sb_cdp` fixture
   (`src/testing/fixture.rs:426`).
5. **`headless` has three defaults.** `Settings::default()` is headed (`settings.rs:537`),
   `BrowserConfig::default()` headless (`config.rs:364`), and `LaunchOptions` unset, which the
   engine reads as headed on macOS and Windows and headless on Linux (`launch.rs:165-166`). The
   page `docs/tutorials/headless.md:26` documents this on purpose.
6. **`SB_NO_SANDBOX` has a second reader with another rule.** `profile_of`
   (`layers.rs:382`) turns the sandbox off when the variable exists
   (`var_os("SB_NO_SANDBOX").is_some()`), so `SB_NO_SANDBOX=false` or a blank value turns it off.
   The table reads it with `parse_bool` (`settings.rs:1180-1188`) and a blank value as unset
   (`:1128`).
7. **`RuntimeConfig` reads three variables of the table again.** `from_vars_lenient`
   (`runtime.rs:110-159`) reads `SB_WEBDRIVER_URL`, `SB_CHROME_BIN` and
   `SB_REMOTE_WEBDRIVER_TIMEOUT` for `BrowserConfig::from_env` (`config.rs:351-384`), without the
   blank rule for the first two (a blank `SB_CHROME_BIN` becomes `Some("")`).
8. **`BrowserConfig::threads` is written and never read**: `with_threads` (`config.rs:568-571`)
   and `Debug` (`:325`) are its only uses (a literal search of `.threads`). The setting is read
   from `Settings::threads` by `sbrs test` (`crates/cli/src/test_args.rs:531`).
9. **Three functions build a Grid address**: `remote_address` (`launch_map.rs:387-418`, from
   `server`, `port`, `protocol`), `browser::options::grid_url` (`src/browser/options/remote.rs:97`,
   behind `BrowserConfig::with_grid`, `config.rs:628-638`), and
   `utilities::selenium_grid::grid_url` (`src/utilities/selenium_grid.rs:294`, another signature).
10. **Spellings of one setting win by spelling, not by source.** `window_size` wins over
    `window_width` and `window_height` (`launch_map.rs:101-103` runs after `settings.rs:1002-1004`)
    and `locale_code` over `locale` (`launch_map.rs:98-100`) whatever set each, so a
    `window_size` in the file beats `SB_WINDOW_WIDTH` and `SB_WINDOW_HEIGHT`.
11. **`settings_file` is a key that does nothing.** Its row (`test_options.rs:419`) has no dot, so
    `settings_key` accepts it, and the field is `#[serde(skip)]` (`settings.rs:146`).
12. **Names with two meanings**: `browser::options::LaunchSettings` (`src/browser/options/mod.rs:86`)
    and `seleniumbase_rs_plugin::LaunchSettings` (`docs/plugins/api/launch.md:147-167`, the view a
    plugin's `before_launch` gets); `config::settings::Settings` and `mcp::host::Settings`
    (`src/mcp/host.rs:67`); the enum `browser::config::Browser` and the struct `sb_cdp::Browser`,
    which `bridge.rs:14` imports as `BrowserKind`.
13. **User-facing variables missing from the listing** (`EXTRA_VARIABLES`,
    `src/config/env_vars.rs:75-276`): `SB_REC_KEEP_SECRETS` (`src/api/recorder.rs:658`),
    `SB_REC_FILE` (`src/api/recorder/output.rs:87`), `SB_S3_BUCKET`, `SB_S3_REGION`, `SB_S3_PREFIX`
    (`src/testing/report.rs:155-165`), `SB_OCR_MODELS` (`src/ocr.rs:74`), `SB_GRID_DIR`
    (`src/utilities/grid_server.rs:513`).

## The field table

Columns: the field and its line; its type; its default; the variable; the settings-file keys (the
first is the field, then every spelling `settings_key` accepts: the row's flags and `SB(...)`
names, with dashes or underscores, any case); where it overlaps; and its path after the regroup
([layout](#the-proposed-layout)). Abbreviations: **BC** `BrowserConfig`, **LS**
`BrowserConfig::launch` (`LaunchSettings`), **L** `Layer`, **RS** `RunSettings`, **RC**
`RuntimeConfig`, **TS** `TestSettings`, **RP** `ReportSettings`. "Derived" marks a new path that
several inputs write ([derived mappings](#derived-mappings)).

### Settings (`src/config/settings.rs`)

The defaults are `Settings::default()` (`:533-622`): `browser` is `"chrome"`, every `bool` is
`false`, every `Option` is `None`, the tables are empty. The variables are the rows at
`test_options.rs:395-494`.

| Field (line) | Type | Default | Variable | File keys | Overlaps | New path |
|---|---|---|---|---|---|---|
| `browser` (76) | `String` | `"chrome"` | `SB_BROWSER` | `browser` | BC `browser` (`Browser`); L `use_chromium` | `browser.kind` |
| `headless` (77) | `bool` | `false` | `SB_HEADLESS` | `headless`, `headless2` | BC `headless` (default `true`); L `headless` | `browser.headless` |
| `timeout_seconds` (82) | `Option<u64>` | `None` | `SB_TIMEOUT` | `timeout_seconds`, `timeout` | RS `default_timeout` (`settings.rs:1005-1007`) | `run.default_timeout` |
| `proxy` (83) | `Option<String>` | `None` | `SB_PROXY` | `proxy`, `proxy_server`, `proxy_string` | BC `proxy`; L `proxy` | `browser.proxy.server` |
| `proxy_pac_url` (84) | `Option<String>` | `None` | `SB_PROXY_PAC_URL` | `proxy_pac_url`, `pac_url` | BC `proxy_pac_url`; L `proxy_pac_url` | `browser.proxy.pac_url` |
| `window_width` (88) | `Option<u32>` | `None` | `SB_WINDOW_WIDTH` | `window_width` | LS `window_size`; L `window_size` | `browser.window_size` (derived) |
| `window_height` (91) | `Option<u32>` | `None` | `SB_WINDOW_HEIGHT` | `window_height` | as `window_width` | `browser.window_size` (derived) |
| `user_data_dir` (92) | `Option<String>` | `None` | `SB_USER_DATA_DIR` | `user_data_dir` | BC `user_data_dir`; L `user_data_dir` (`PathBuf`) | `browser.user_data_dir` |
| `extension_dir` (93) | `Option<String>` | `None` | `SB_EXTENSION_DIR` | `extension_dir` | BC `extension_dir`; L `extensions` (split at commas, `bridge.rs:161-168`) | `browser.extensions` |
| `locale` (94) | `Option<String>` | `None` | `SB_LOCALE` | `locale` | BC `locale`; L `lang` | `browser.lang` (derived) |
| `user_agent` (95) | `Option<String>` | `None` | `SB_USER_AGENT` | `user_agent`, `agent` | BC `user_agent`; L `user_agent` | `browser.user_agent` |
| `mode` (96) | `Option<String>` | `None` | `SB_MODE` | `mode` | BC `mode` (`DriverMode`) | `session.mode` |
| `ad_block` (97) | `bool` | `false` | `SB_AD_BLOCK` | `ad_block`, `adblock`, `block_ads`, `ad_block_on` | BC `ad_block`; L `ad_block` | `browser.ad_block` |
| `reuse_session` (100) | `bool` | `false` | `SB_REUSE_SESSION` | `reuse_session`, `rs` | BC `reuse_session` | `session.reuse` |
| `reuse_class_session` (104) | `bool` | `false` | `SB_REUSE_CLASS_SESSION` | `reuse_class_session`, `rcs` | LS `reuse_class`; implies BC `reuse_session` (`settings.rs:1030`) | `session.reuse_class` (derived) |
| `mobile` (105) | `bool` | `false` | `SB_MOBILE` | `mobile`, `mobile_emulator`, `is_mobile` | BC `mobile`; L `mobile` | `browser.mobile` |
| `threads` (106) | `Option<usize>` | `None` | `SB_THREADS` | `threads` | BC `threads` (never read, finding 8) | `test.threads` |
| `esc_end` (110) | `bool` | `false` | `SB_ESC_END` | `esc_end`, `ee` | LS switch `EscEnd` | `browser.switches.esc-end` |
| `rec_sb_cdp` (113) | `bool` | `false` | `SB_REC_SB_CDP` | `rec_sb_cdp` | LS switch `RecSbCdp` | `browser.switches.rec-sb-cdp` |
| `remote_webdriver_timeout_seconds` (117) | `Option<u64>` | `None` | `SB_REMOTE_WEBDRIVER_TIMEOUT` | `remote_webdriver_timeout_seconds`, `remote_webdriver_timeout` | BC field of the same name; RC `remote_webdriver_timeout` | `session.remote_webdriver_timeout_seconds` |
| `webdriver_url` (121) | `Option<String>` | `None` | `SB_WEBDRIVER_URL` | `webdriver_url` | BC `webdriver_url` (`String`); RC `webdriver_url`; L `cdp_url` with `cdp_discovery` | `session.webdriver_url` |
| `chrome_bin` (125) | `Option<PathBuf>` | `None` | `SB_CHROME_BIN` | `chrome_bin`, `binary_location`, `bl` | BC `browser_binary_path`; RC `chrome_bin`; L `executable_path` | `browser.executable_path` |
| `plugins` (132) | `PluginSelection` | empty | `SB_PLUGINS` (not a row) | `[plugins]` | LS `plugins` (built); L `selection` | `plugins` |
| `fingerprint` (138) | `FingerprintSettings` | empty | `SB_FINGERPRINT` | `[fingerprint] file` | BC `fingerprint` (loaded); L `fingerprint` | `fingerprint` |
| `start_page` (143) | `Option<String>` | `None` | `SB_START_PAGE` | `start_page`, `url` | BC `start_page`; L `url` | `browser.url` |
| `settings_file` (147) | `Option<PathBuf>` | `None` | `SB_SETTINGS_FILE` | `settings_file`, `settings` (accepted, ignored: finding 11) | none | `settings_file` |
| `headless1` (149) | `bool` | `false` | `SB_HEADLESS1` | `headless1` | LS switch `Headless1` | `browser.switches.headless1` |
| `block_images` (151) | `bool` | `false` | `SB_BLOCK_IMAGES` | `block_images` | LS switch `BlockImages`; L switch | `browser.switches.block-images` |
| `disable_js` (153) | `bool` | `false` | `SB_DISABLE_JS` | `disable_js` | LS/L switch `DisableJavaScript` | `browser.switches.disable-java-script` |
| `disable_cookies` (155) | `bool` | `false` | `SB_DISABLE_COOKIES` | `disable_cookies` | LS/L switch `DisableCookies` | `browser.switches.disable-cookies` |
| `do_not_track` (157) | `bool` | `false` | `SB_DO_NOT_TRACK` | `do_not_track` | LS/L switch `DoNotTrack` | `browser.switches.do-not-track` |
| `external_pdf` (159) | `bool` | `false` | `SB_EXTERNAL_PDF` | `external_pdf` | LS/L switch `ExternalPdf` | `browser.switches.external-pdf` |
| `enable_sync` (161) | `bool` | `false` | `SB_ENABLE_SYNC` | `enable_sync` | LS switch `EnableSync` (CDP unused, `bridge.rs:66`) | `browser.switches.enable-sync` |
| `disable_csp` (163) | `bool` | `false` | `SB_DISABLE_CSP` | `disable_csp`, `dcsp`, `no_csp` | LS/L switch `DisableCsp` | `browser.switches.disable-csp` |
| `use_auto_ext` (165) | `bool` | `false` | `SB_USE_AUTO_EXT` | `use_auto_ext`, `auto_ext` | LS switch `AutomationExtension` (CDP unused) | `browser.switches.automation-extension` |
| `proxy_driver` (167) | `bool` | `false` | `SB_PROXY_DRIVER` | `proxy_driver` | LS switch `ProxyForDriverDownload` (CDP unused) | `browser.switches.proxy-for-driver-download` |
| `dark_mode` (169) | `bool` | `false` | `SB_DARK_MODE` | `dark_mode`, `dark` | LS/L switch `DarkMode` | `browser.switches.dark-mode` |
| `devtools` (171) | `bool` | `false` | `SB_DEVTOOLS` | `devtools`, `open_devtools` | LS/L switch `Devtools` | `browser.switches.devtools` |
| `disable_gpu` (173) | `bool` | `false` | `SB_DISABLE_GPU` | `disable_gpu` | LS/L switch `DisableGpu` | `browser.switches.disable-gpu` |
| `disable_ws` (175) | `bool` | `false` | `SB_DISABLE_WS` | `disable_ws`, `dws`, `disable_web_security` | LS/L switch `DisableWebSecurity` | `browser.switches.disable-web-security` |
| `guest_mode` (177) | `bool` | `false` | `SB_GUEST_MODE` | `guest_mode`, `guest` | LS/L switch `Guest` | `browser.switches.guest` |
| `incognito` (179) | `bool` | `false` | `SB_INCOGNITO` | `incognito`, `incognito_mode` | LS/L switch `Incognito` | `browser.switches.incognito` |
| `no_sandbox` (181) | `bool` | `false` | `SB_NO_SANDBOX` | `no_sandbox` | LS/L switch `NoSandbox`; read again at `layers.rs:382` (finding 6) | `browser.switches.no-sandbox` |
| `swiftshader` (183) | `bool` | `false` | `SB_SWIFTSHADER` | `swiftshader` | LS/L switch `Swiftshader` | `browser.switches.swiftshader` |
| `enable_3d_apis` (185) | `bool` | `false` | `SB_ENABLE_3D_APIS` | `enable_3d_apis` | LS switch `Enable3dApis` (CDP ignores it, `layers.rs:214`) | `browser.switches.enable-3d-apis` |
| `maximize` (187) | `bool` | `false` | `SB_MAXIMIZE` | `maximize`, `maximize_window`, `fullscreen` | LS/L switch `Maximize` | `browser.switches.maximize` |
| `chromium_arg` (189) | `Option<ArgList>` | `None` | `SB_CHROMIUM_ARG` | `chromium_arg` | BC `extra_args` (and LS `disable_features` for `disable-features=` items, `launch_map.rs:346-363`); L `extra_args` | `browser.chromium.args` (derived) |
| `disable_features` (191) | `Option<String>` | `None` | `SB_DISABLE_FEATURES` | `disable_features` | LS `disable_features`; L `disable_features` | `browser.chromium.disable_features` |
| `host_resolver_rules` (193) | `Option<String>` | `None` | `SB_HOST_RESOLVER_RULES` | `host_resolver_rules` | LS `host_resolver_rules`; L `host_resolver_rules` (`Vec`) | `browser.chromium.host_resolver_rules` |
| `server` (195) | `Option<String>` | `None` | `SB_SERVER` | `server`, `servername` | BC `webdriver_url` (built by `remote_address`) | `session.grid.server` |
| `port` (197) | `Option<u16>` | `None` | `SB_PORT` | `port` | as `server` | `session.grid.port` |
| `protocol` (199) | `Option<String>` | `None` | `SB_PROTOCOL` | `protocol` | as `server` | `session.grid.protocol` |
| `locale_code` (201) | `Option<String>` | `None` | `SB_LOCALE_CODE` | `locale_code` | BC `locale` (as a `LocaleCode`); L `lang` | `browser.lang` (derived) |
| `window_size` (204) | `Option<String>` | `None` | `SB_WINDOW_SIZE` | `window_size`, `size` | LS `window_size`; L `window_size` | `browser.window_size` (derived) |
| `window_position` (206) | `Option<String>` | `None` | `SB_WINDOW_POSITION` | `window_position`, `position` | LS `window_position`; L `window_position` | `browser.window_position` |
| `device_metrics` (208) | `Option<String>` | `None` | `SB_DEVICE_METRICS` | `device_metrics`, `metrics` | LS `device_metrics`; L `device_metrics` | `browser.device_metrics` |
| `page_load_strategy` (210) | `Option<String>` | `None` | `SB_PAGE_LOAD_STRATEGY` | `page_load_strategy`, `pls` | LS `page_load_strategy` (CDP unused) | `session.page_load_strategy` |
| `driver_version` (212) | `Option<String>` | `None` | `SB_DRIVER_VERSION` | `driver_version` | LS `driver_version` | `session.driver_version` |
| `proxy_bypass_list` (214) | `Option<String>` | `None` | `SB_PROXY_BYPASS_LIST` | `proxy_bypass_list` | LS `proxy_bypass_list`; L `proxy_bypass_list` | `browser.proxy.bypass_list` |
| `cap_file` (216) | `Option<PathBuf>` | `None` | `SB_CAP_FILE` | `cap_file` | LS `capabilities` | `session.capabilities` (derived) |
| `cap_string` (218) | `Option<String>` | `None` | `SB_CAP_STRING` | `cap_string` | LS `capabilities`, merged over the file's | `session.capabilities` (derived) |
| `extension_zip` (220) | `Option<String>` | `None` | `SB_EXTENSION_ZIP` | `extension_zip`, `crx` | LS `extension_archives` (CDP unused) | `session.extension_archives` |
| `firefox_arg` (222) | `Option<String>` | `None` | `SB_FIREFOX_ARG` | `firefox_arg` | LS `firefox` | `session.firefox` (derived) |
| `firefox_pref` (224) | `Option<String>` | `None` | `SB_FIREFOX_PREF` | `firefox_pref` | LS `firefox` | `session.firefox` (derived) |
| `xvfb` (226) | `bool` | `false` | `SB_XVFB` | `xvfb` | LS `virtual_display`; L `virtual_display` | `browser.virtual_display` (derived) |
| `xvfb_metrics` (228) | `Option<String>` | `None` | `SB_XVFB_METRICS` | `xvfb_metrics` | LS `virtual_display` (size) | `browser.virtual_display` (derived) |
| `remote_debug` (230) | `bool` | `false` | `SB_REMOTE_DEBUG` | `remote_debug`, `remote_debugger` | LS `remote_debug_port` and `remote_debug_any_port`; L `debugging_port` | `browser.debugging_port` (derived) |
| `log_cdp_events` (232) | `bool` | `false` | `SB_LOG_CDP_EVENTS` | `log_cdp_events`, `log_cdp` | LS `cdp_events` (CDP unused) | `session.cdp_events` (derived) |
| `uc_cdp_events` (234) | `bool` | `false` | `SB_UC_CDP_EVENTS` | `uc_cdp_events`, `uc_cdp` | LS `cdp_events` | `session.cdp_events` (derived) |
| `demo` (236) | `bool` | `false` | `SB_DEMO` | `demo`, `demo_mode` | RS `demo_mode` | `run.demo_mode` |
| `slow` (238) | `bool` | `false` | `SB_SLOW` | `slow`, `slow_mode`, `slowmo` | RS `slow_mode` | `run.slow_mode` |
| `skip_js_waits` (240) | `bool` | `false` | `SB_SKIP_JS_WAITS` | `skip_js_waits`, `sjw` | RS `skip_js_waits` | `run.skip_js_waits` |
| `visual_baseline` (242) | `bool` | `false` | `SB_VISUAL_BASELINE` | `visual_baseline` | RS `reset_visual_baseline` | `run.reset_visual_baseline` |
| `crumbs` (245) | `bool` | `false` | `SB_CRUMBS` | `crumbs` | the process-wide `reuse::set_crumbs` (`launch_map.rs:220-224`) | `session.crumbs` |
| `demo_sleep` (247) | `Option<f64>` | `None` | `SB_DEMO_SLEEP` | `demo_sleep` | RS `demo_sleep` (`Duration`) | `run.demo_sleep` |
| `message_duration` (249) | `Option<f64>` | `None` | `SB_MESSAGE_DURATION` | `message_duration` | RS `message_duration` | `run.message_duration` |
| `highlights` (252) | `Option<u32>` | `None` | `SB_HIGHLIGHTS` | `highlights` | RS `highlights` | `run.highlights` |
| `interval` (254) | `Option<f64>` | `None` | `SB_INTERVAL` | `interval` | RS `interval` | `run.interval` |
| `verify_delay` (256) | `Option<f64>` | `None` | `SB_VERIFY_DELAY` | `verify_delay` | RS `verify_delay` | `run.verify_delay` |
| `check_js` (258) | `bool` | `false` | `SB_CHECK_JS` | `check_js`, `js_checking_on` | RS `check_js` | `run.check_js` |
| `wait_for_angularjs` (260) | `bool` | `false` | `SB_WAIT_FOR_ANGULARJS` | `wait_for_angularjs`, `wfa` | RS `wait_for_angularjs` | `run.wait_for_angularjs` |
| `disable_beforeunload` (262) | `bool` | `false` | `SB_DISABLE_BEFOREUNLOAD` | `disable_beforeunload` | RS `disable_beforeunload` | `run.disable_beforeunload` |
| `timeout_multiplier` (264) | `Option<f64>` | `None` | `SB_TIMEOUT_MULTIPLIER` | `timeout_multiplier` | RS `timeout_multiplier` | `run.timeout_multiplier` |
| `test` (266) | `TestSettings` | empty | see below | `[test]` | see below | `test` |
| `report` (270) | `ReportSettings` | empty | `SB_HTML`, `SB_JUNIT_XML`, `SB_ALLURE_DIR`, `SB_REPORT_DB` (read by `from_settings_with`) | `[report]` (refused, finding 1) | 11 fields of TS | `report` |

### TestSettings and TestData (`src/config/test_settings.rs`)

Every default is the type's `Default` (`reruns` 0, `reruns_delay` 0.0, `env` `None`, read as `test`
by `TestData::env`, `:98-100`). The keys sit in the `[test]` table, the `TestData` ones directly
(`#[serde(flatten)]`, `:180`); no alias is accepted there (finding 2). The variables are the rows
at `test_options.rs:494-529`.

| Field (line) | Type | Variable | File key | Overlaps | New path |
|---|---|---|---|---|---|
| `data.env` (77) | `Option<String>` | `SB_ENV` | `[test] env` | LS `test_data` | `test.data.env` |
| `data.account` (79) | `Option<String>` | `SB_ACCOUNT` | `[test] account` | LS `test_data` | `test.data.account` |
| `data.data` (81) | `Option<String>` | `SB_DATA` | `[test] data` | LS `test_data` | `test.data.data` |
| `data.var1`, `var2`, `var3` (83, 85, 87) | `Option<String>` | `SB_VAR1`, `SB_VAR2`, `SB_VAR3` | `[test] var1`, `var2`, `var3` | LS `test_data` | `test.data.var1`, ... |
| `data.variables` (91) | `Map<String, Value>` | `SB_VARIABLES` | `[test] variables` | LS `test_data` | `test.data.variables` |
| `pdb` (183) | `bool` | `SB_PDB` | `[test] pdb` | none | `test.pdb` |
| `trace` (185) | `bool` | `SB_TRACE` | `[test] trace` | none | `test.trace` |
| `final_debug` (188) | `bool` | `SB_FINAL_DEBUG` | `[test] final_debug` | none | `test.final_debug` |
| `reruns` (190) | `u32` | `SB_RERUNS` | `[test] reruns` | none | `test.reruns` |
| `reruns_delay` (192) | `f64` | `SB_RERUNS_DELAY` | `[test] reruns_delay` | none | `test.reruns_delay` |
| `time_limit` (195) | `Option<f64>` | `SB_TIME_LIMIT` | `[test] time_limit` | none | `test.time_limit` |
| `save_screenshot` (198) | `bool` | `SB_SAVE_SCREENSHOT` | `[test] save_screenshot` | RP `save_screenshot` | `report.save_screenshot` |
| `no_screenshot` (201) | `bool` | `SB_NO_SCREENSHOT` | `[test] no_screenshot` | RP `no_screenshot` | `report.no_screenshot` |
| `list_fail_page` (203) | `bool` | `SB_LIST_FAIL_PAGE` | `[test] list_fail_page` | RP `list_fail_pages` | `report.list_fail_page` |
| `archive_logs` (205) | `bool` | `SB_ARCHIVE_LOGS` | `[test] archive_logs` | RP `archive_logs` | `report.archive_logs` |
| `archive_downloads` (208) | `bool` | `SB_ARCHIVE_DOWNLOADS` | `[test] archive_downloads` | none | `report.archive_downloads` |
| `dashboard` (210) | `bool` | `SB_DASHBOARD` | `[test] dashboard` | RP `dashboard` | `report.dashboard` |
| `dash_title` (212) | `Option<String>` | `SB_DASH_TITLE` | `[test] dash_title` | RP `dash_title` | `report.dash_title` |
| `html` (214) | `Option<PathBuf>` | `SB_HTML` | `[test] html` | RP `html` | `report.html` |
| `junit_xml` (216) | `Option<PathBuf>` | `SB_JUNIT_XML` | `[test] junit_xml` | RP `junit_xml` | `report.junit_xml` |
| `allure_dir` (218) | `Option<PathBuf>` | `SB_ALLURE_DIR` | `[test] allure_dir` | RP `allure_dir` | `report.allure_dir` |
| `with_db_reporting` (221) | `bool` | `SB_WITH_DB_REPORTING` | `[test] with_db_reporting` | RP `with_db_reporting` | `report.with_db_reporting` |
| `database_env` (223) | `Option<String>` | `SB_DATABASE_ENV` | `[test] database_env` | RP `database_env` | `report.database_env` |
| `with_s3_logging` (225) | `bool` | `SB_WITH_S3_LOGGING` | `[test] with_s3_logging` | none | `report.with_s3_logging` |
| `recorder` (227) | `bool` | `SB_RECORDER` | `[test] recorder` | LS switch `RecorderMode` (`launch_map.rs:68`) | `browser.switches.recorder-mode` |
| `rec_behave` (229) | `bool` | `SB_REC_BEHAVE` | `[test] rec_behave` | LS switch `RecBehave` | `browser.switches.rec-behave` |
| `rec_print` (231) | `bool` | `SB_REC_PRINT` | `[test] rec_print` | LS switch `RecPrint` | `browser.switches.rec-print` |
| `rec_sb_mgr` (233) | `bool` | `SB_REC_SB_MGR` | `[test] rec_sb_mgr` | LS switch `RecSbMgr` | `browser.switches.rec-sb-mgr` |
| `rec_sleep` (235) | `bool` | `SB_REC_SLEEP` | `[test] rec_sleep` | RS `record_sleep` (`launch_map.rs:202-204`) | `run.record_sleep` |
| `runner` (238) | `Option<String>` | `SB_TEST_RUNNER` | `[test] runner` | none | `test.runner` |
| `databases` (240) | `BTreeMap<String, PathBuf>` | none | `[test.databases]` | RP `db` (picked by `database_env`, `test_run.rs:198-202`) | `report.databases` |

### ReportSettings (`src/core/test_run.rs:132-159`)

Default: `Default` (every switch off, every path `None`; `db_path` falls back to
`reports/results.db`, `:223-227`). Saved in `run.json`.

| Field (line) | Type | Variable | File key | Overlaps | New path |
|---|---|---|---|---|---|
| `dashboard` (136) | `bool` | `SB_DASHBOARD` (through TS) | `[report] dashboard` | TS `dashboard` | `report.dashboard` |
| `dash_title` (138) | `Option<String>` | `SB_DASH_TITLE` (TS) | `[report] dash_title` | TS | `report.dash_title` |
| `html` (140) | `Option<PathBuf>` | `SB_HTML` (read twice) | `[report] html` | TS | `report.html` |
| `junit_xml` (142) | `Option<PathBuf>` | `SB_JUNIT_XML` (twice) | `[report] junit_xml` | TS | `report.junit_xml` |
| `archive_logs` (144) | `bool` | `SB_ARCHIVE_LOGS` (TS) | `[report] archive_logs` | TS | `report.archive_logs` |
| `list_fail_pages` (146) | `bool` | `SB_LIST_FAIL_PAGE` (TS) | `[report] list_fail_pages` | TS `list_fail_page` | `report.list_fail_page` |
| `save_screenshot` (148) | `bool` | `SB_SAVE_SCREENSHOT` (TS) | `[report] save_screenshot` | TS | `report.save_screenshot` |
| `no_screenshot` (150) | `bool` | `SB_NO_SCREENSHOT` (TS) | `[report] no_screenshot` | TS | `report.no_screenshot` |
| `allure_dir` (152) | `Option<PathBuf>` | `SB_ALLURE_DIR` (twice) | `[report] allure_dir` | TS | `report.allure_dir` |
| `with_db_reporting` (154) | `bool` | `SB_WITH_DB_REPORTING` (TS) | `[report] with_db_reporting` | TS | `report.with_db_reporting` |
| `db` (156) | `Option<PathBuf>` | `SB_REPORT_DB` (`EXTRA_VARIABLES`, `env_vars.rs:172-179`) | `[report] db` | none | `report.db` |
| `database_env` (158) | `Option<String>` | `SB_DATABASE_ENV` (TS) | `[report] database_env` | TS | `report.database_env` |

### BrowserConfig (`src/browser/config.rs:175-277`)

No loader reads a `BrowserConfig` from a settings file: it derives serde (`:175`), and the 8
sites that deserialize one are tests (`config.rs:1332-1776`, `tests/offline/sb_cdp_attach.rs:324`
and `:739`). So it has no file key. The defaults are
`from_runtime_config(&RuntimeConfig::default())` (`:339-343`, `:356-384`); `from_env` (`:351-354`)
reads the variables of `RuntimeConfig`.

| Field (line) | Type | Default | Variable | Overlaps | New place |
|---|---|---|---|---|---|
| `webdriver_url` (177) | `String` | `http://localhost:4444` | `SB_WEBDRIVER_URL` (`from_env`) | Settings `webdriver_url`, `server`, `port`, `protocol`; RC | `session.webdriver_url` |
| `cdp_url` (182) | `Option<String>` | `None` | none | L `cdp_url` | stays |
| `cdp_discovery` (185) | `bool` | `false` | none | L `cdp_url` from `webdriver_url` (`bridge.rs:130-134`) | stays |
| `cdp_target` (191) | `Option<String>` | `None` | none | L `cdp_target` | stays |
| `browser` (193) | `Browser` | `Chrome` | none | Settings `browser`; L `use_chromium` | `browser.kind` |
| `headless` (194) | `bool` | `true` | none | Settings `headless` (default `false`); L `headless` | `browser.headless` |
| `mode` (195) | `DriverMode` | `WebDriver` | none | Settings `mode` | `session.mode` |
| `user_agent` (196) | `Option<String>` | `None` | none | Settings; L | `browser.user_agent` |
| `locale` (197) | `Option<String>` | `None` | none | Settings `locale`, `locale_code`; L `lang` | `browser.lang` |
| `ad_block` (220) | `bool` | `false` | none | Settings; L | `browser.ad_block` |
| `proxy` (221) | `Option<String>` | `None` | none | Settings; L | `browser.proxy.server` |
| `proxy_pac_url` (222) | `Option<String>` | `None` | none | Settings; L | `browser.proxy.pac_url` |
| `user_data_dir` (223) | `Option<String>` | `None` | none | Settings; L | `browser.user_data_dir` |
| `extension_dir` (224) | `Option<String>` | `None` | none | Settings; L `extensions` | `browser.extensions` |
| `start_page` (225) | `Option<String>` | `None` | none | Settings; L `url` | `browser.url` |
| `reuse_session` (243) | `bool` | `false` | none | Settings `reuse_session`, `reuse_class_session` | `session.reuse` |
| `mobile` (244) | `bool` | `false` | none | Settings; L | `browser.mobile` |
| `threads` (245) | `Option<usize>` | `None` | none | Settings `threads` | removed (finding 8) |
| `auto_start_driver` (246) | `bool` | `true` | none | none | `session.auto_start_driver` |
| `extra_args` (250) | `Vec<String>` | empty | none | Settings `chromium_arg`; L `extra_args` | `browser.chromium.args` |
| `fingerprint` (253) | `Option<Fingerprint>` | `None` | none | Settings `fingerprint` (the file); L | stays (the loaded value) |
| `browser_binary_path` (259) | `Option<PathBuf>` | RC `chrome_bin` | `SB_CHROME_BIN` (`from_env`) | Settings `chrome_bin`; L `executable_path` | `browser.executable_path` |
| `remote_webdriver_timeout_seconds` (269) | `Option<u64>` | RC | `SB_REMOTE_WEBDRIVER_TIMEOUT` (`from_env`) | Settings, same name | `session.remote_webdriver_timeout_seconds` |
| `launch` (276) | `LaunchSettings` | empty | none | below | split into `browser`, `session`, `run`, `test_data` |

`BrowserConfig::launch`, `LaunchSettings` (`src/browser/options/mod.rs:84-173`, `#[serde(default)]`,
flattened into `BrowserConfig`). Every default changes nothing (`:82-83`).

| Field (line) | Type | Fed by Settings | Read by Layer (`bridge.rs`) | New place |
|---|---|---|---|---|
| `switches` (89) | `BTreeSet<LaunchSwitch>` | 26 `bool` fields (`launch_map.rs:48-79`, `settings.rs:1008-1009`) | 15 of them (`layers.rs:195-219`) | `browser.switches` (a map) |
| `page_load_strategy` (92) | `Option<PageLoadStrategy>` | `page_load_strategy` | no (`bridge.rs:51`) | `session.page_load_strategy` |
| `window_size` (95) | `Option<WindowSize>` | `window_size`, `window_width`, `window_height` | `window_size` | `browser.window_size` |
| `window_position` (98) | `Option<WindowPosition>` | `window_position` | `window_position` | `browser.window_position` |
| `device_metrics` (101) | `Option<DeviceMetrics>` | `device_metrics` | `device_metrics` | `browser.device_metrics` |
| `proxy_bypass_list` (104) | `ProxyBypassList` | `proxy_bypass_list` | `proxy_bypass_list` | `browser.proxy.bypass_list` |
| `extension_archives` (107) | `Vec<ExtensionArchive>` | `extension_zip` | no (`:56`) | `session.extension_archives` |
| `firefox` (110) | `FirefoxOptions` | `firefox_arg`, `firefox_pref` | no (`:54`) | `session.firefox` |
| `capabilities` (113) | `CapabilityOverrides` | `cap_file`, `cap_string` | no (`:52`) | `session.capabilities` |
| `driver_version` (116) | `Option<DriverVersion>` | `driver_version` | no (`:53`) | `session.driver_version` |
| `remote_debug_port` (119) | `Option<u16>` | `remote_debug` | `debugging_port` (`:83-94`) | `browser.debugging_port` |
| `remote_debug_any_port` (127) | `bool` | `remote_debug` | `debugging_port` | `browser.debugging_any_port` |
| `cdp_events` (131) | `CdpEventCapture` | `log_cdp_events`, `uc_cdp_events` | no (`:55`) | `session.cdp_events` |
| `virtual_display` (134) | `Option<VirtualDisplay>` | `xvfb`, `xvfb_metrics` | `virtual_display` | `browser.virtual_display` |
| `disable_features` (138) | `Vec<String>` | `disable_features`, `chromium_arg` | `disable_features` | `browser.chromium.disable_features` |
| `host_resolver_rules` (142) | `Option<String>` | `host_resolver_rules` | `host_resolver_rules` | `browser.chromium.host_resolver_rules` |
| `ad_block_list` (146) | `Option<AdBlockList>` | none | no (`:57`) | `browser.ad_block_list` |
| `reuse_class` (152) | `Option<ReuseClass>` | `reuse_class_session` | no | `session.reuse_class` |
| `plugins` (159, feature `plugins`) | `BrowserPlugins` | none (`with_plugin_selection`) | `plugins` (`bridge.rs:160`) | `BrowserConfig::plugins` |
| `run` (166) | `RunSettings` | 15 fields (`launch_map.rs:164-208`, `settings.rs:1005-1007`) | no | `BrowserConfig::run` |
| `test_data` (172) | `TestData` | `test.data` (`launch_map.rs:210-214`) | no | `BrowserConfig::test_data` |

### Layer and LaunchOptions (`src/sb_cdp/launch/layers.rs`, `src/sb_cdp/launch.rs`)

A `Layer` is all unset by default (`#[derive(Default)]`, `layers.rs:72`); `assemble` (`:452-543`)
gives the `LaunchOptions` defaults. No `Layer` field has a variable except `no_sandbox`
(`SB_NO_SANDBOX`, `:382`, finding 6) and none has a file key; a `Settings` reaches them through
`BrowserConfig` or through `to_launch_options`.

| Field (line) | Type | From `BrowserConfig` (`bridge.rs`) | Builder setter (`builder.rs`) | `LaunchOptions` field (`launch.rs`) and default |
|---|---|---|---|---|
| `url` (74) | `Option<String>` | `start_page` (`:149`) | `url` (`:123`) | `url` (187), `None` |
| `cdp_url` (75) | `Option<Redacted>` | `cdp_url`, or `webdriver_url` with `cdp_discovery` (`:130-134`) | `cdp_url` (`:44`) | `cdp_url` (188) |
| `cdp_target` (76) | `Option<String>` | `cdp_target` | `cdp_target` (`:71`) | `cdp_target` (190) |
| `headless` (77) | `Option<bool>` | always `Some(headless)` (`:136`) | `headless` (`:134`) | `headless` (192): unset is headed with a virtual display (`layers.rs:511`), else the OS default |
| `use_chromium` (78) | `Option<bool>` | `browser == Chromium` (`:117-123`) | `use_chromium` (`:143`) | `source` (193), `Chrome` |
| `executable_path` (79) | `Option<PathBuf>` | `browser_binary_path` | `executable_path` (`:150`) | `source` |
| `ad_block` (80) | `Option<bool>` | `ad_block`, only when on (`:139`) | `ad_block` (`:172`) | `ad_block` (195), `false` |
| `proxy` (81) | `Option<ProxySpec>` | `proxy`, read as Python reads one (`:142-145`) | `proxy` (`:180`) | `proxy` (196) |
| `user_data_dir` (82) | `Option<PathBuf>` | `user_data_dir` | `user_data_dir` (`:190`) | `user_data_dir` (197) |
| `user_agent` (83) | `Option<String>` | `user_agent` | `user_agent` (`:197`) | `user_agent` (198) |
| `lang` (84) | `Option<String>` | `locale`, normalised (`:124`), plus the preference `intl.accept_languages` | `lang` (`:204`) | `lang` (199) |
| `window_size` (85) | `Option<(u32, u32)>` | `launch.window_size` | `window_size` (`:211`) | `window_size` (200) |
| `window_position` (86) | `Option<(i32, i32)>` | `launch.window_position` | `window_position` (`:218`) | `window_position` (201) |
| `webrtc` (87) | `Option<WebRtcPolicy>` | none | `webrtc_policy` (`:84`) | `webrtc` (203) |
| `shield_webrtc` (88) | `Option<bool>` | none | `shield_webrtc` (`:116`) | `shield_webrtc` (204), `false` |
| `fingerprint` (89) | `Option<SharedFingerprint>` | `fingerprint` | `fingerprint` (`:107`) | `identity` (206) |
| `mobile` (90) | `Option<bool>` | `mobile`, only when on (`:154`) | `mobile` (`:427`) | `device` (230) |
| `device_metrics` (91) | `Option<DeviceMetrics>` | `launch.device_metrics` | `device_metrics` (`:436`) | `device` |
| `proxy_bypass_list` (92) | `Option<ProxyBypassList>` | `launch.proxy_bypass_list` | `proxy_bypass_list` (`:521`) | `proxy_bypass_list` (232), empty |
| `proxy_pac_url` (93) | `Option<Redacted>` | `proxy_pac_url` | `proxy_pac_url` (`:533`) | `proxy_pac_url` (235) |
| `virtual_display` (94) | `Option<VirtualDisplay>` | `launch.virtual_display` | `virtual_display` (`:546`) | `virtual_display` (237) |
| `startup_timeout` (95) | `Option<Duration>` | none | `startup_timeout` (`:371`) | `startup_timeout` (242), 20 s (`launch.rs:37`) |
| `debugging_port` (96) | `Option<u16>` | `launch.remote_debug_port`, a free port when the default is busy (`:83-94`) | `debugging_port` (`:400`) | `debugging_port` (244), 0 |
| `plugins` (98, feature) | `Option<PluginSet>` | `plugins()` | `plugins` (`:302`) | `plugins` (241) |
| `extra_args` (99) | `Vec<String>` | `extra_args`, with `--` added (`:169-179`) | `arg` (`:235`) | `extra_args` (211) |
| `extensions` (100) | `Vec<PathBuf>` | `extension_dir`, split | `extension` (`:248`) | `extensions` (213) |
| `prefs` (101) | `Map<String, Value>` | the language preference | `preference` (`:262`) | `prefs` (215), plus the switches' preferences (`layers.rs:497-498`) |
| `switches` (103) | `BTreeMap<LaunchSwitch, bool>` | `launch.switches`, on only (`:185-187`) | `dark_mode`, `devtools`, `disable_web_security`, `swiftshader`, `maximize`, `incognito`, `guest`, `no_sandbox`, `switch` (`:446-567`) | `switches` (228), `mode` (194), `no_sandbox` (202) |
| `disable_features` (104) | `Vec<String>` | `launch.disable_features` | `disable_features` (`:483`) | `disable_features` (221) |
| `enable_features` (105) | `Vec<String>` | none | `enable_features` (`:498`) | `enable_features` (224) |
| `host_resolver_rules` (106) | `Vec<String>` | `launch.host_resolver_rules` | `host_resolver_rules` (`:514`) | `host_resolver_rules` (218) |
| `selection` (109, feature) | `PluginSelection` | none (Settings `plugins` through `to_launch_options`, `launch_map.rs:334-337`) | `plugins_named`, `plugin_selection` (`:330`, `:364`) | `plugins` |

`LaunchOptions` alone holds `identity`, `given` (the layers, `:210`), `device`, `source`, `mode`,
`no_sandbox` and `disable_quic` (`:226`, set by plugins only).

### RuntimeConfig (`src/config/runtime.rs:15-46`)

| Field (line) | Type | Default | Variable | Overlaps | New place |
|---|---|---|---|---|---|
| `webdriver_url` (18) | `String` | `http://localhost:4444` | `SB_WEBDRIVER_URL` | Settings, BC | read through `EnvVars` (below) |
| `chrome_bin` (21) | `Option<PathBuf>` | `None` | `SB_CHROME_BIN` | Settings, BC, L | read through `EnvVars` |
| `patch_cache_dir` (24) | `Option<PathBuf>` | `None` | `SB_PATCH_CACHE_DIR` | none | stays |
| `log_level` (29) | `Option<String>` | `None` | `SB_LOG_LEVEL` | none | stays |
| `log_format` (31) | `LogFormat` | `Pretty` | `SB_LOG_FORMAT` | none | stays |
| `shutdown_timeout` (33) | `Duration` | 30 s | `SB_SHUTDOWN_TIMEOUT_SECS` | none | stays |
| `chromedriver_port` (35) | `u16` | 0 | `SB_CHROMEDRIVER_PORT` | none | stays |
| `implicit_wait` (37) | `Duration` | 30 s | `SB_IMPLICIT_WAIT_SECS` | none | stays |
| `remote_webdriver_timeout` (45) | `Option<Duration>` | `None` | `SB_REMOTE_WEBDRIVER_TIMEOUT` | Settings, BC | read through `EnvVars` |

### What truly overlaps

* **Settings and BrowserConfig** hold the same values in two shapes: text in `Settings`, typed in
  `BrowserConfig`, joined by `with_launch_options` (173 lines, `launch_map.rs:44-216`) and
  `base_browser_config` (54 lines, `settings.rs:992-1045`). Of the 81 flat fields of `Settings`
  (85 less `plugins`, `fingerprint`, `test`, `report`), 47 are launch values (some of them
  switches only the WebDriver engine reads), 18 are WebDriver-only (mode, server, Grid, driver,
  capabilities, Firefox, CDP event log, reuse, crumbs), 14 are fields of `RunSettings` (the 15th
  is `[test] rec_sleep`), copied one by one, and 2 are neither (`threads`, `settings_file`).
* **BrowserConfig and Layer** overlap in 27 launch values, read in one place
  (`Layer::from_config`), and the merge rule is already single (`Layer::over`). `Layer` adds
  values only a caller of the builder sets (WebRTC, startup timeout, enabled features,
  preferences). They should not become one type: a `Layer` has every value unset so that layers
  merge, and a `BrowserConfig` has resolved values that the WebDriver engine reads directly.
* **TestSettings and ReportSettings** are one table written twice (11 fields).
* **TestSettings and BrowserConfig** share only `TestData` and the recorder switches.
* **RuntimeConfig** shares three variables with `Settings`.

## The proposed layout

```rust
// src/config/settings.rs
pub struct Settings {
    /// `[browser]`: what both engines start. Shared with `BrowserConfig::browser`.
    pub browser: BrowserSettings,
    /// `[session]`: what only a `BaseCase` session reads. Shared with `BrowserConfig::session`.
    pub session: SessionSettings,
    /// `[run]`: how a whole test run behaves. Shared with `BrowserConfig::run`.
    pub run: RunSettings,
    /// `[test]`: test data, debug modes, reruns, the runner.
    pub test: TestSettings,
    /// `[report]`: reports, evidence and the results database (the only report table).
    pub report: ReportSettings,
    /// `[fingerprint]`, unchanged.
    pub fingerprint: FingerprintSettings,
    /// `[plugins]`, unchanged.
    pub plugins: PluginSelection,
    /// Names the file, so it is never read from one (`SB_SETTINGS_FILE`).
    pub settings_file: Option<PathBuf>,
}

// src/browser/options/mod.rs: today's `LaunchSettings`, renamed and grown
pub struct BrowserSettings {
    pub kind: Browser,                          // SB_BROWSER
    pub executable_path: Option<PathBuf>,       // SB_CHROME_BIN
    pub headless: bool,                         // SB_HEADLESS
    pub url: Option<String>,                    // SB_START_PAGE
    pub user_agent: Option<String>,             // SB_USER_AGENT
    pub lang: Option<String>,                   // SB_LOCALE, SB_LOCALE_CODE
    pub mobile: bool,                           // SB_MOBILE
    pub device_metrics: Option<DeviceMetrics>,  // SB_DEVICE_METRICS
    pub window_size: Option<WindowSize>,        // SB_WINDOW_SIZE, SB_WINDOW_WIDTH and _HEIGHT
    pub window_position: Option<WindowPosition>,// SB_WINDOW_POSITION
    pub proxy: ProxySettings,                   // [browser.proxy]
    pub user_data_dir: Option<PathBuf>,         // SB_USER_DATA_DIR
    pub extensions: Vec<PathBuf>,               // SB_EXTENSION_DIR
    pub ad_block: bool,                         // SB_AD_BLOCK
    pub ad_block_list: Option<AdBlockList>,     // code only
    pub switches: BTreeMap<LaunchSwitch, bool>, // [browser.switches], 26 variables
    pub chromium: ChromiumSettings,             // [browser.chromium]
    pub virtual_display: Option<VirtualDisplay>,// SB_XVFB, SB_XVFB_METRICS
    pub debugging_port: Option<u16>,            // SB_REMOTE_DEBUG
    pub debugging_any_port: bool,               // SB_REMOTE_DEBUG
}

pub struct ProxySettings {
    pub server: Option<String>,                 // SB_PROXY
    pub pac_url: Option<String>,                // SB_PROXY_PAC_URL
    pub bypass_list: ProxyBypassList,           // SB_PROXY_BYPASS_LIST
}

pub struct ChromiumSettings {
    pub args: Vec<String>,                      // SB_CHROMIUM_ARG
    pub disable_features: Vec<String>,          // SB_DISABLE_FEATURES
    pub host_resolver_rules: Option<String>,    // SB_HOST_RESOLVER_RULES
}

// src/browser/options/session.rs (new)
pub struct SessionSettings {
    pub mode: DriverMode,                                 // SB_MODE
    pub webdriver_url: Option<String>,                    // SB_WEBDRIVER_URL
    pub grid: GridSettings,                               // [session.grid]
    pub remote_webdriver_timeout_seconds: Option<u64>,    // SB_REMOTE_WEBDRIVER_TIMEOUT
    pub driver_version: Option<DriverVersion>,            // SB_DRIVER_VERSION
    pub page_load_strategy: Option<PageLoadStrategy>,     // SB_PAGE_LOAD_STRATEGY
    pub capabilities: CapabilityOverrides,                // SB_CAP_FILE, SB_CAP_STRING
    pub extension_archives: Vec<ExtensionArchive>,        // SB_EXTENSION_ZIP
    pub firefox: FirefoxOptions,                          // SB_FIREFOX_ARG, SB_FIREFOX_PREF
    pub cdp_events: CdpEventCapture,                      // SB_LOG_CDP_EVENTS, SB_UC_CDP_EVENTS
    pub reuse: bool,                                      // SB_REUSE_SESSION
    pub reuse_class: Option<ReuseClass>,                  // SB_REUSE_CLASS_SESSION
    pub crumbs: bool,                                     // SB_CRUMBS
    pub auto_start_driver: bool,                          // code only
}

pub struct GridSettings {
    pub server: Option<String>,                           // SB_SERVER
    pub port: Option<u16>,                                // SB_PORT
    pub protocol: Option<GridProtocol>,                   // SB_PROTOCOL
}

// src/api/run_settings.rs: `RunSettings` gains serde (seconds as numbers); fields unchanged:
// demo_mode, slow_mode, demo_sleep, message_duration, default_timeout, skip_js_waits,
// record_sleep, reset_visual_baseline, highlights, interval, verify_delay, check_js,
// wait_for_angularjs, disable_beforeunload, timeout_multiplier.

// src/config/test_settings.rs: `TestSettings` keeps
// data (flattened), pdb, trace, final_debug, reruns, reruns_delay, time_limit, runner,
// and gains threads (SB_THREADS).

// src/core/test_run.rs: `ReportSettings` is the `[report]` group:
// dashboard, dash_title, html, junit_xml, allure_dir, archive_logs, archive_downloads,
// list_fail_page, save_screenshot, no_screenshot, with_db_reporting, database_env, db,
// databases, with_s3_logging.

// src/browser/config.rs
pub struct BrowserConfig {
    pub browser: BrowserSettings,
    pub session: SessionSettings,
    pub run: RunSettings,
    pub test_data: TestData,
    pub fingerprint: Option<Fingerprint>,   // loaded; Settings holds the file
    pub cdp_url: Option<String>,
    pub cdp_discovery: bool,
    pub cdp_target: Option<String>,
    #[cfg(feature = "plugins")]
    pub plugins: BrowserPlugins,
}
```

Why these groups and not others:

* **`browser`** is what both engines start, because it is what `Layer::from_config` reads. The type
  is renamed from `LaunchSettings` to `BrowserSettings` so that the plugin crate's
  `LaunchSettings`, a different thing, keeps the name alone (finding 12). The enum field is `kind`,
  the name `bridge.rs:14` already gives the enum.
* **`session`** is what only the WebDriver engine reads; `bridge::unused_settings`
  (`bridge.rs:43-78`) lists exactly these as unused by the pure CDP engine, plus the WebDriver-only
  switches, which stay in `browser.switches` because `LaunchSwitch` holds them.
* **`run`** is `RunSettings` itself, so there is no second copy of its 15 fields.
* **`proxy` is inside `browser`**, not beside it: both engines read the server, the PAC file and
  the bypass list, so they belong to the core. `proxy_driver` is a switch already.
* **No `timeouts` group**: the default wait and the multiplier are fields of `RunSettings`, the
  Remote WebDriver timeout belongs to the session, the time limit and the rerun delay to the test,
  and the shutdown and implicit waits to `RuntimeConfig`. A `timeouts` group would split
  `RunSettings` in two.
* **No `paths` group**: each path sits with what it configures (the binary and the profile with the
  browser, the reports with the report, the fingerprint file with the fingerprint).
* **Unknown keys are refused by the reader, not by serde.** `TestSettings` flattens `TestData`,
  and serde does not support `deny_unknown_fields` together with `flatten`, on the outer struct or
  the flattened one ([serde.rs](https://serde.rs/container-attrs.html)). The file reader knows
  every key from the table, so it refuses an unknown key in any group, `[test]` included
  (finding 2).
* **The field names of the core are the builder's** where the builder has the setting (`url`,
  `executable_path`, `lang`, `extensions`, `debugging_port`), so code that moves between
  `BrowserConfig` and `LaunchOptionsBuilder` meets one name. The Python spellings (`start_page`,
  `binary_location`, `locale`, `remote_debug`) stay as file keys, flags and variables through the
  table.

## The shared core and the one merge rule

The core is `BrowserSettings`, `SessionSettings` and `RunSettings`, held by both `Settings` and
`BrowserConfig`. With it:

* **`Settings::to_browser_config`** stops converting: it clones `browser`, `session` and `run`,
  takes `test.data`, loads the fingerprint file and applies `session.crumbs` (the process-wide
  switch it applies today, `launch_map.rs:220-224`). `with_launch_options`, `check_launch_values`
  and `base_browser_config` go, because the values are typed when they are read
  ([figment](#figment)).
* **`Layer::from_config`** reads `config.browser` through one function, `Layer::of_browser`, next
  to `Layer::of_fingerprint` (`layers.rs:170-190`). It destructures `BrowserSettings` without
  `..`, so a field added to the core does not compile until the pure CDP engine reads it or names
  it unused. `unused_settings` reads `config.session` as a whole.
* **`Settings::to_launch_options`** becomes
  `LaunchOptions::builder().browser_config(&self.to_browser_config()?).build()`, after the one
  check it has of its own (the pure CDP engine starts Chrome, Chromium or a named binary,
  `launch_map.rs:269-278`). Its second mapping goes, and with it finding 4.
* **The merge rule stays where it is.** There are two merges, of different things, each in one
  place: figment merges the sources of a `Settings` (file, environment, command line), and
  `Layer::over` merges the builder's settings over a `BrowserConfig`'s and a fingerprint's
  (`layers.rs:1-15`). `BrowserConfig` merges nothing; it is a plain value.
* **The switches become a map** (`BTreeMap<LaunchSwitch, bool>`, as in `Layer`, `layers.rs:103`)
  so that a key path can name one switch and a higher source can turn one off. The WebDriver
  engine reads a switch as on when the map says `true`.

## figment

### Providers

figment ([`/sergiobenitez/figment` on Context7](#sources)) merges providers; `merge` prefers the
incoming value and `join` the existing one (Figment.md, configuration.md). The chain:

```rust
Figment::new()
    .merge(SettingsFile::read(path)?)       // the file: sbase_config.toml or SB_SETTINGS_FILE
    .merge(EnvVars::from_process())         // the SB_* variables
    .merge(EnvVars::from_pairs(&overrides)) // the command line (SettingsOverrides)
    .extract::<Settings>()
```

* **Defaults: no provider.** Every group is `#[serde(default)]`, so a key no provider gives takes
  the group's `Default`, which is today's `Settings::default()`. `Serialized::defaults` exists
  (providers-Serialized-Named.md) and is not needed.
* **`SettingsFile`** (new, `src/config/settings_file.rs`, crate-private) implements
  `figment::Provider` (`metadata` and `data`, the two methods the `Env` and `Serialized` impls show,
  providers-Env.md and providers-Serialized-Named.md). It reads TOML or JSON with the parsers we
  have (`toml` 1.1, `Cargo.toml:120`; `serde_json`), renames each key with the table (flat keys,
  aliases and the moved `[test]` keys, as `canonical_entries` does now) into the nested dictionary,
  accepts the nested tables as they are, and refuses unknown keys and duplicates with today's
  messages. Its metadata is named after the file, with an interpolater that gives back the key as
  the file wrote it (Metadata.md).
* **`EnvVars`** (new, in `src/config/env_vars.rs`, crate-private) reads every row of the table
  that has a variable, plus `SB_PLUGINS` and `SB_REPORT_DB`, from a lookup function (the process,
  or the pairs of `SettingsOverrides`). A blank value is unset (except `SB_PLUGINS`). It emits each
  value as text at the row's path, builds the [derived mappings](#derived-mappings), and names its
  metadata "`SB_*` environment variable" (or "command line"), with an interpolater that turns a
  path back into the row's variable (or flag).
* **Explicit code** is the last word in two ways: the `SettingsOverrides` provider above (what
  `sbrs` asks for), and plain assignment on the `Settings` that `load` returns
  (`settings.browser.headless = true`). No figment type is public.
* **Why not figment's own providers.** `Env` lowercases names and `split("_")` turns every `_`
  into a nesting level (providers-Env.md), so `SB_DARK_MODE` would become `dark.mode`; its metadata
  writes a failing key as the upper-cased path (`BROWSER.SWITCHES.DARK-MODE`), not the variable;
  its value parser is one function for every key (`Env::parser`) and reads `true` and `false` but
  not `yes`, `on` or `1`, which `parse_bool` accepts (`settings.rs:1180-1188`), and reads
  `[a, b]` as an array (configuration.md). `Toml::file` and `Json::file` cannot rename aliases or
  refuse an unknown key with our message, and a missing file is an empty dictionary unless
  `required(true)` (providers-Data-Format.md). So figment does the merging, the provenance and the
  extraction, and the two providers do our names.
* **Typed reading.** The values arrive as text from the variables and as TOML or JSON values from
  the file. Each typed field takes a `deserialize_with` reader (`src/config/text.rs`, new) that
  accepts both: `switch` (a boolean, or text through `parse_bool`), numbers, seconds (0 or more,
  `test_options.rs:215-222`), lists (`ArgList`'s string-or-array, `browser_options.rs:102-121`),
  and `FromStr` for `Browser`, `DriverMode`, `GridProtocol`, `WindowSize`, `WindowPosition`,
  `DeviceMetrics`, `PageLoadStrategy`, `DriverVersion`, `ProxyBypassList`, `VirtualDisplay`. These
  replace `OptionValue` (`test_options.rs:173-256`) and the `parse` calls of `with_launch_options`
  (`launch_map.rs:98-118`), so a value is refused in one place, when it is read.

To verify on docs.rs before step 6: how a provider builds a nested `Dict` and a string `Value`
(the pages show `Value::serialize` and the internal `nest`, not a public constructor), whether
`Metadata::named` takes an owned `String` (the `Env` impl sets `md.name` itself), the crate's
current version, MSRV and licence, and which default features can be turned off (none of figment's
format features is needed); that `merge` merges nested dictionaries key by key (the pages say
only that `merge` prefers the incoming value and `admerge` concatenates arrays; `SB_PLUGINS` and
`session.capabilities` rely on it); and that a `#[serde(flatten)]` field (`TestSettings::data`)
extracts through figment's deserializer.

### How the variables map to paths

There is no mechanical rule: the group of a variable is not in its name, and the names hold `_`.
Every row of the table names its variable and gains its path (the column "New path" above), and
`EnvVars` reads the rows. The mappings whose last path segment is not the variable's own name:

| Variable | Path |
|---|---|
| `SB_BROWSER` | `browser.kind` |
| `SB_CHROME_BIN` | `browser.executable_path` |
| `SB_START_PAGE` | `browser.url` |
| `SB_LOCALE`, `SB_LOCALE_CODE` | `browser.lang` (derived) |
| `SB_EXTENSION_DIR` | `browser.extensions` (split at commas) |
| `SB_PROXY`, `SB_PROXY_PAC_URL`, `SB_PROXY_BYPASS_LIST` | `browser.proxy.server`, `.pac_url`, `.bypass_list` |
| `SB_WINDOW_WIDTH` and `SB_WINDOW_HEIGHT` | `browser.window_size` (derived) |
| `SB_XVFB`, `SB_XVFB_METRICS` | `browser.virtual_display` (derived) |
| `SB_REMOTE_DEBUG` | `browser.debugging_port` and `browser.debugging_any_port` (derived) |
| `SB_CHROMIUM_ARG` | `browser.chromium.args` (derived) |
| `SB_DISABLE_JS` | `browser.switches.disable-java-script` |
| `SB_USE_AUTO_EXT` | `browser.switches.automation-extension` |
| `SB_PROXY_DRIVER` | `browser.switches.proxy-for-driver-download` |
| `SB_DISABLE_WS` | `browser.switches.disable-web-security` |
| `SB_GUEST_MODE` | `browser.switches.guest` |
| `SB_RECORDER` | `browser.switches.recorder-mode` |
| the other 20 switch variables | `browser.switches.` and the `LaunchSwitch` name (`src/browser/options/switches.rs:25-111`, kebab case) |
| `SB_REUSE_SESSION` | `session.reuse` |
| `SB_REUSE_CLASS_SESSION` | `session.reuse_class` (`test-module` when on) |
| `SB_SERVER`, `SB_PORT`, `SB_PROTOCOL` | `session.grid.server`, `.port`, `.protocol` |
| `SB_CAP_FILE`, `SB_CAP_STRING` | `session.capabilities` (derived) |
| `SB_EXTENSION_ZIP` | `session.extension_archives` |
| `SB_FIREFOX_ARG`, `SB_FIREFOX_PREF` | `session.firefox` (derived) |
| `SB_LOG_CDP_EVENTS`, `SB_UC_CDP_EVENTS` | `session.cdp_events` (derived) |
| `SB_TIMEOUT` | `run.default_timeout` |
| `SB_DEMO`, `SB_SLOW` | `run.demo_mode`, `run.slow_mode` |
| `SB_VISUAL_BASELINE` | `run.reset_visual_baseline` |
| `SB_REC_SLEEP` | `run.record_sleep` |
| `SB_THREADS` | `test.threads` |
| `SB_TEST_RUNNER` | `test.runner` |
| `SB_ENV`, `SB_ACCOUNT`, `SB_DATA`, `SB_VAR1`, `SB_VAR2`, `SB_VAR3`, `SB_VARIABLES` | `test.data.env`, ... (as today) |
| `SB_FINGERPRINT` | `fingerprint.file` (as today) |
| `SB_PLUGINS` | `plugins.enable`: an array, so `merge` replaces the file's list and keeps its options, as today (`settings.rs:1131-1134`); empty means none |
| `SB_REPORT_DB` | `report.db` |
| `SB_SETTINGS_FILE` | none: it chooses the file before figment runs |

The settings-file keys follow the same rows. The ones that change group: every `[test]` key of the
report (`save_screenshot`, `no_screenshot`, `list_fail_page`, `archive_logs`,
`archive_downloads`, `dashboard`, `dash_title`, `html`, `junit_xml`, `allure_dir`,
`with_db_reporting`, `database_env`, `with_s3_logging`, `databases`) goes to `report`; `[test]
recorder`, `rec_behave`, `rec_print`, `rec_sb_mgr` go to `browser.switches`; `[test] rec_sleep` to
`run.record_sleep`; `[report] list_fail_pages` to `report.list_fail_page`. The table row gains a
`key` column for the file key (today's `field`, `test_options.rs:88-90`), and `field` becomes the
new path; `sbrs options` prints the key, as `key_cell` (`browser_options.rs:283-289`) does.

### Derived mappings

Where several inputs write one value. Within one source the later row of the table wins, as today;
across sources the higher source wins, which changes finding 10.

| Value | Inputs | Today | Proposed |
|---|---|---|---|
| `browser.window_size` | `window_size`; `window_width` and `window_height` | `window_size` wins whatever set it; one of width and height alone is refused (`settings.rs:685-693`) | the higher source wins; within one, `window_size`; one of the pair alone is refused, naming its variable or key |
| `browser.lang` | `locale`, `locale_code` | `locale_code` wins whatever set it | the higher source wins; within one, `locale_code`, normalised by `LocaleCode` |
| `browser.virtual_display` | `xvfb`, `xvfb_metrics` | a display when `xvfb` is on, of the metrics' size (`launch_map.rs:144-154`); metrics alone are only checked | the same, built where both are known (the source that sets `xvfb`, with the metrics of any source) |
| `browser.debugging_port` | `remote_debug` | port 9222 (the row's help, `test_options.rs:476`), another port when that one is busy (`remote_debug_any_port`, `src/browser/options/mod.rs:120-127`) | `debugging_port` and `debugging_any_port` written together |
| `browser.chromium.args` | `chromium_arg` | `disable-features=` items join `disable_features`, the others get `--` (`launch_map.rs:346-363`) | the same, when read |
| `session.capabilities` | `cap_file`, `cap_string` | the file, then the string merged over it (`launch_map.rs:119-128`) | the file's object and the string's object as dictionaries; figment's merge puts the higher source's keys over the lower's (to verify, below) |
| `session.firefox` | `firefox_arg`, `firefox_pref` | both added to one `FirefoxOptions` | the same |
| `session.cdp_events` | `log_cdp_events`, `uc_cdp_events` | `Always` over `UndetectedOnly` | the same, within one source |
| `session.reuse_class` | `reuse_class_session` | on: `TestModule`, and reuse on (`settings.rs:1010`, `:1030`) | on: `TestModule`; reuse is on when `reuse` or `reuse_class` |
| `session.webdriver_url` | `webdriver_url`; `server`, `port`, `protocol` | the Grid address unless `webdriver_url` is set (`launch_map.rs:89-97`) | `grid` stays its own group; `SessionSettings::webdriver_url()` gives the URL, else the Grid address, else `http://localhost:4444`, through one Grid address function (finding 9) |

### Errors that name the source

* **A variable or command-line value that does not parse** is a figment error whose origin is the
  `EnvVars` provider; its interpolater turns the path back into the variable, and `named`
  (`settings.rs:1165-1178`) adds the flag, so the text stays
  `SB_HEADLESS (--headless): cannot parse bool: maybe` (the test at `settings.rs:1196-1217`
  keeps passing).
* **A file value** names the file and the key as the file wrote it:
  `sbase_config.toml: headless: cannot parse bool: maybe`. The error's `kind`, `path` and
  `origins()` (errors.md) give the parts; the message is formatted by us, not by figment's
  `Display`, whose exact text the pages do not give.
* **An unknown key and a duplicate** keep today's messages (`browser_options.rs:162-170`,
  `:181-184`), raised by `SettingsFile` before figment runs.
* **A name that is wrong** (`SB_BROWSER=chrom`, `mode = "stealth"`) is refused when it is read, by
  the `FromStr` reader, with its source, instead of by `validate` with the generic "set by
  --browser, SB_BROWSER or the settings file" (`settings.rs:671-683`).
* **`Settings::check`** keeps reporting every problem: it extracts, records the failing key and its
  source, drops that key from its provider, and extracts again, at most once per row (141). A file
  that cannot be read at all is dropped whole, as today (`settings.rs:935-944`). The problems are
  sorted by the table's row order, so the output order does not depend on the struct's field
  order. `SettingsProblem::source` keeps its values: the file, the variable, `validation`,
  `plugins`, `fingerprint`.

## Migration

### Call sites

Counted with TokenSave literal searches on `90ca478e` (lines, not distinct calls). The golden
templates are not in the index and were counted with `rtk rg -g '*.golden'` (a fallback).

| What | Lines | Must change |
|---|---|---|
| `Settings::load`, `load_from`, `load_global`, `load_plugin_selection`, `load_plugin_table` | 109 (code, docs, CHANGELOG) | 0: the signatures stay |
| `to_browser_config()` | 69 | 0 |
| `to_launch_options()` | 13 (11 in code) | 0 |
| `.browser_config(` | 66 | 0 |
| `Settings` struct literals | 41: `src/` 33 (`config/launch_map.rs` 16, `config/settings.rs` 8, `cli/plugin_cmd.rs` 4, `testing/lifecycle.rs` 2, `testing/report.rs` 1, `testing/fixture.rs` 1, `mcp/cdp/tasks.rs` 1), `tests/` 8 | all; most of `launch_map.rs` is deleted |
| `..Settings::default()` | 37 | with their literals |
| Flat `Settings` fields read outside `src/config` | about 32: `src/mcp/dev/diagnose.rs:686-714` about 20, `examples/settings_config.rs:82-107` 6, `src/testing/lifecycle.rs:194-195` 3, `crates/cli/src/sbrs.rs:3687-3688` 2, `crates/cli/src/test_args.rs:531` 1 | all |
| `settings.test.` | 54 (`src/` 23, `crates/cli` 16, `tests/` 8, docs 7) | 15 that touch moved fields (`test_args.rs:569-572`, `:629-630`, `:1125`, `:1223`; `sbrs.rs:3728`; `test_options.rs:708`; `testing/report.rs:104`; `tests/offline/reporting_run.rs:713-716`) |
| `TestSettings` / `ReportSettings` | 114 / 101 | where they name a moved field |
| `SettingsOverrides` | 13 (code: `crates/cli/src/sbrs.rs:30`, `:3366`, `:3383`, `:3651`; `examples/settings_config.rs:31`, `:75`, `:78`; `src/lib.rs:198`; `src/config/mod.rs:37`) | the 3 literals, if it becomes pairs only |
| `BrowserConfig {` | 106 (struct literals and functions returning one) | the literals |
| `config.launch.` | 82 | all: `config.browser.`, `config.session.`, `config.run.`, `config.test_data` |
| `config.headless` / `.browser_binary_path` | 23 / 18 | all |
| `BrowserConfig`'s `with_*` methods | about 80 (`docs/design/bon_builders.md:22`) | bodies only; names and signatures stay |
| Generated code | `src/utilities/importer/emit.rs:436` writes `Settings::from_env()?.headless` | 1 |
| Golden templates (`tests/golden`) | 37 files, 46 lines: 6 calls whose signatures stay (`bc`, `bc_rec`, `cm`, `cm_uc`, `dm`, `dm_zh`); the other lines name an `SB_*` variable (the three read are comments naming `SB_HEADLESS`); none writes a field | 0 |
| Contract tests | `tests/contract_companion.rs:367-385` and `tests/contract_plugins.rs:245` use `BrowserConfig::default()` and `with_plugins` | 0 |

The compiler finds every field site; the counts size the work.

### Consumers

| Repository | Uses | Must change |
|---|---|---|
| seleniumbase-rs-stealth | 82 lines of `LaunchOptions` and `LaunchOptionsBuilder`; `Settings` 0 in code (its 22 hits are HTTP/2 `Frame::Settings`); `BrowserConfig` 0 in code (`README.md:261`, `docs/design/stealth_names.md:235`); no `SB_*` variable | 0: the builder does not change |
| tauri-plugin-sbrs-automation | `README.md:315-319` (`BrowserConfig::default().with_webdriver_url(..)`); its own `Settings` (`src/lib.rs:92`) is unrelated; no `LaunchOptions`, no `SB_*` | 0 |
| seleniumbase-rs-companion | `BrowserConfig::default()` written by its generator (`src/shared/codegen/basecase.ts:130-132`, tests `tests/codegen.test.ts:152`, `:156`); 11 lines of `LaunchOptions` (`e2e/src/lab.rs:46`, `e2e/src/scratch.rs:105-106`, `src/shared/codegen/cdp.ts:133-145`, tests); no `SB_*` | 0 |

The consumers were counted from their own indexes as last synced: stealth and the Tauri plugin
about four hours before the count, the companion about two days and eighteen hours before. A
change made since then may be missing.

### Docs pages to update

* `docs/tutorials/settings_and_config.md` (the hierarchy at 14-37, the file at 38-76, the loading
  at 77-104, the variables at 105-201, the runtime variables at 202-278, the Remote WebDriver
  timeout at 279-309, `BrowserConfig` in code at 310-332).
* `docs/help/customizing_test_runs.md` (the settings file at 254-342, the precedence at 452-505).
* `docs/help/test_options.md` (40, 293, and the key column of every option).
* `docs/tutorials/headless.md` (26, 101, 124, 219-269, 1050-1055).
* `docs/api/crate-root.md:33`, `docs/api/testing-fixtures.md:21` and `:75`,
  `docs/help/common_problems.md:471`, `docs/glossary.md:1554` (`RunSettings`) and `:1720` (the
  layers), `docs/ABI_API.md:72`, `docs/README.md:53`, `:185`, `:355`, `DOCS.md:73`, `:266`.
* The `BrowserConfig` literals in `README.md:339`, `docs/tutorials/cdp_mode.md:51`,
  `docs/stealth/introduction.md:64` and `examples/stealth_options.rs:43`; `examples/settings_config.rs`.
* The parity notes that say "the keys are the Settings fields" (`docs/parity.md:1045-1047`,
  `:1196`; `parity/api.toml:5379`, `:5384`, `:5389`, `:6068`).
* Generated and checked: `src/mcp/dev/api_index.json` (`SB_UPDATE_API_INDEX=1 cargo test -p
  seleniumbase-rs --features mcp-server api_index`), the consumer contracts (`mise run
  contracts`). The CLI's flags do not change, so `completions/sbrs.usage.kdl` and
  `docs/help/cli_reference.md` do not either.
* The tests that check the table: `tests/offline/test_options_table.rs` (its
  `the_variables_of_the_first_release_are_still_read`, `:118-154`, lists 25 variables; step 1
  makes it every variable) and `crates/cli/tests/offline/cli_env_vars.rs`.
* No change: the plugin pages (`docs/plugins/*.md`) use only `[plugins]`; the pages that name
  `settings.test.reruns` and `time_limit` (`docs/rust-test-tooling.md:300-301`, `:705-706`,
  `docs/best-practices.md:628`) keep working.

## Implementation steps

One commit each; each runs the gate of `CLAUDE.md` (fmt, clippy with `-D warnings`, the tests).

1. **`test(config): pin every variable, file key, precedence and error of the settings`.** A
   table-driven test over `TEST_OPTIONS`: for every row that reads a variable, a valid sample value
   changes the field the row names, read back through `serde_json::to_value(&settings)` and the
   row's path (as `env_vars::default_of` does, `env_vars.rs:299-319`), so the test survives the
   regroup by its path column alone. The same for every file key and every alias `settings_key`
   accepts, in TOML and JSON, through `Settings::from_file`. Precedence: default, file, variable,
   pair, for one key of each group; a blank variable is unset; `SB_PLUGINS` replaces `enable` and
   keeps the options; a later pair wins (`settings.rs:1074-1078`). Error texts: a bad variable, a
   bad file value, an unknown key, a duplicate key, `Settings::check` listing three problems. The
   findings as tests of today's behaviour: 1 (`[report]` refused), 2 (`[test]` typo dropped), 4
   (`to_launch_options` without `mobile`), 6 (`SB_NO_SANDBOX=false` turns the sandbox off), 10
   (file `window_size` over `SB_WINDOW_WIDTH`). The step that fixes one flips its test.
2. **`refactor(report): one report table`.** `ReportSettings` is the `[report]` group; the 14
   report fields leave `TestSettings`; `[test]` keeps reading their keys through the table;
   `TABLES` gains `report` (finding 1); `list_fail_pages` becomes `list_fail_page`;
   `from_settings_with` shrinks to the database choice (finding 3). `canonical_entries` checks the
   keys of `[test]` and `[report]` against the table, as it checks the top-level keys (finding 2;
   not `deny_unknown_fields`, which serde does not allow beside `flatten`). Tests: step 1's,
   flipped for findings 1 and 2; `run.json` round trip.
3. **`refactor(run): RunSettings is the run group`.** `RunSettings` gains serde, seconds as numbers
   (`default_timeout` whole seconds, as `SB_TIMEOUT` is today); `Settings::run` and
   `BrowserConfig::run` replace the 14 flat fields, `[test] rec_sleep` and `LaunchSettings::run`. Tests: step 1's; a
   `[run]` table and the flat keys give the same `RunSettings`.
4. **`refactor(browser): BrowserSettings and SessionSettings`.** `LaunchSettings` becomes
   `BrowserSettings` with the launch fields of `BrowserConfig` moved in, the switches as a map,
   `ProxySettings` and `ChromiumSettings`; `SessionSettings` and `GridSettings` hold the
   WebDriver-only fields; `BrowserConfig` is regrouped and its `with_*` bodies re-pointed;
   `threads` leaves it (finding 8); one Grid address function (finding 9). `Layer::of_browser`
   destructures the core without `..`. Tests: the bridge tests (`bridge.rs:192` on) and
   `layers.rs:546-1091` unchanged in meaning; serde round trips of `BrowserConfig`
   (`config.rs:1332-1776`) updated.
5. **`refactor(config): Settings holds the core`.** `Settings::browser` and `session` are the core
   types, read typed through `src/config/text.rs`; `to_browser_config` clones;
   `with_launch_options`, `check_launch_values` and `to_launch_options`'s mapping go
   (`launch_map.rs` keeps `remote_address` only if step 4 did not remove it); `to_launch_options`
   goes through `browser_config` (finding 4). Tests: step 1's, flipped for finding 4; a phone, a
   PAC file, a bypass list and `block_images` in `Settings` reach `LaunchOptions`.

   *As implemented (2026-11-06).* The table keeps its seven columns: the first is the dotted path
   (`browser.proxy.server`), and a row of `browser.switches` has the kind
   `switch_in(DarkMode, "dark-mode")`, which names the `LaunchSwitch` and the name it has in the
   serialized settings (the path the pins read). The flat key of a row is looked up by its
   variable (`FLAT_KEYS`), because several rows share one path. The rows that several variables
   write have a reader of their own in `src/config/text.rs`, picked by the variable in
   `apply_row!`: the window size (`SB_WINDOW_WIDTH` and `_HEIGHT` are read together; a lone
   side is a size with a side of zero until `validate` refuses it, so the other side may come
   from a higher source), the language, the display, the debugging port, `chromium_arg` and
   `disable_features`, the capabilities, the Firefox options, the event log and the reuse
   class. The settings file reads the flat keys of the browser and the session through the
   same readers (a file is one more source of variables), and the tables `[run]`, `[test]`,
   `[report]`, `[plugins]` and `[fingerprint]` through serde. What is not in step 5: the nested
   `[browser]` and `[session]` tables of a file (step 6). Where the sequential reading differs
   from the note: `SB_XVFB_METRICS` is used only when `SB_XVFB` is on in the same source or
   a lower one, and the capabilities and the Chromium features are added across sources.

6. **`feat(config): layer the settings with figment`.** `SettingsFile` and `EnvVars`;
   `Settings::load`, `load_from`, `from_env`, `check` and `apply_options` on one `Figment`; the
   option table gains `key` and the new `field` path; `apply_env` and `OptionValue` go;
   `SettingsOverrides` becomes its pairs; `RuntimeConfig` drops its three shared fields and
   `BrowserConfig::from_env` reads them through `EnvVars` (finding 7); `profile_of` reads
   `SB_NO_SANDBOX` through the same reader (finding 6); the cross-source spellings follow the
   source (finding 10). Tests: step 1's, flipped for findings 6, 7 and 10; a figment error from the
   file names the file and the key as written; `check` sorts by row.
7. **`docs: settings regrouped`.** The pages listed above, the CHANGELOG entry (the breaking
   changes, and the behaviour changes of findings 4, 6, 10), the parity notes, the API index, the
   contracts, and `EXTRA_VARIABLES` with the variables of finding 13.

## Open questions

1. **Group names.** The owner named `browser`, `timeouts`, `paths`, `reporting` and `proxy` as
   examples. *Recommendation:* `browser`, `session`, `run`, `test`, `report`, with `proxy` inside
   `browser`, and no `timeouts` or `paths` group (reasons in [the layout](#the-proposed-layout));
   `report`, not `reporting`, because the type is `ReportSettings` and the table `[report]`.
2. **The file syntax.** Flat keys only, or also the nested tables? *Recommendation:* both: the flat
   key is what the docs show (it is the option's name, as `sbrs options` lists it), the nested form
   is what `Settings` serializes to; one setting written in both forms is refused, as two spellings
   are today.
3. **Field names of the core.** The builder's names (`url`, `executable_path`, `lang`,
   `extensions`, `debugging_port`) or the Python ones (`start_page`, `chrome_bin`, `locale`,
   `remote_debug`)? *Recommendation:* the builder's, because consumers use the builder (stealth 82
   lines, the companion 11) and nothing changes for them; the Python names stay as keys, flags and
   variables.
4. **The default of `headless`.** One default, or one per entry point? *Recommendation:* one per
   entry point, each written once and tested: `BrowserSettings::default()` and `Settings` headed
   (Python's default, as `docs/tutorials/headless.md:101` says), `BrowserConfig::default()`
   headless, `LaunchOptions` unset.
5. **figment in the public API.** *Recommendation:* no figment type in a public signature; the
   providers are crate-private and examples import only our crates.
6. **`SettingsOverrides`.** Keep its 18 typed fields, or only the `(SB_*, text)` pairs?
   *Recommendation:* the pairs only: the table is the one list, and `env` already hands them on as
   pairs.
7. **Cross-source spellings** (finding 10). *Recommendation:* the source first, then the row order;
   a test pins it and the CHANGELOG says so.
8. **One reader of a variable** (findings 6 and 7). *Recommendation:* every variable of the table is
   read by `EnvVars` and nothing else; `BrowserConfig::from_env` stays as narrow as it is (the
   three runtime variables) but reads them through it; the build-time `SB_NO_SANDBOX` of a bare
   builder stays (decision 2 of `docs/design/bon_builders.md:603-604`) with the table's rule.
9. **The recorder switches.** `EscEnd`, `RecSbCdp` and the `Rec*` switches change nothing about the
   launch (`switches.rs:55-75`) but live in `LaunchSwitch`. *Recommendation:* leave them in
   `browser.switches` in this lane; a recorder group can come later.
10. **The figment dependency.** *Recommendation:* add it in step 6 only after checking on docs.rs
    and crates.io its version (Context7 gave none), its MSRV against `rust-version = "1.99"`, its
    licence and `cargo deny check`, with the default features off.
    *Checked 2026-11-05 (scratch crate with `figment = { version = "0.10.19", default-features =
    false }`, the repository's `deny.toml`):* figment 0.10.19, released 2024-05-17, `MIT OR
    Apache-2.0`, no `rust-version` declared (it builds on the 1.99 toolchain); with the default
    feature (a no-op `env`) off, its normal dependencies are `serde` and `uncased` (build:
    `version_check`) and nothing else; `cargo deny check` reports `advisories ok, bans ok,
    licenses ok, sources ok` and `cargo audit` finds no vulnerability in the 13 crates. Its last
    release is eighteen months old.
    The "to verify" items were run in the same crate (four tests, figment 0.10.19): `merge`
    joins nested dictionaries key by key and the later provider wins; a `#[serde(flatten)]` group
    and a flattened map of plugin options extract; an array of the later layer replaces the
    earlier one (what `SB_PLUGINS` needs); an integer extracts into an `f64`; `Metadata::named`
    takes an owned `String`; an extraction error carries `path`, `metadata` (the name of the
    provider) and `profile`, and `Metadata::interpolate` gives the key as the source writes it.
    `Error::origins()`, which the Context7 pages show, is not in 0.10.19 (the pages describe the
    repository's master): read `error.metadata` and `error.profile`.


11. **The other names with two meanings** (finding 12). *Recommendation:* `LaunchSettings` is
    resolved here by the rename; `mcp::host::Settings` and the `Browser` enum go to a naming lane.
12. **The listing.** *Recommendation:* add the variables of finding 13 to `EXTRA_VARIABLES`, so
    `sbrs options --env` lists them; they stay out of `Settings`.
13. **The `check_config` tool of the `dev` MCP server** prints the flat names
    (`src/mcp/dev/diagnose.rs:686-714`). *Recommendation:* print the new paths, with the variable
    beside each, in step 5.

## Sources

Read through Context7 on 2026-11-05 (library `/sergiobenitez/figment`, the generated reference
pages of the repository; no version was given):

* [providers-Env.md](https://github.com/sergiobenitez/figment/blob/master/_autodocs/api-reference/providers-Env.md):
  `Env::split`, `filter`, `map`, `filter_map`, `only`, `ignore`, `parser`, and the `Provider`
  impl of `Env` (its metadata and interpolater);
* [configuration.md](https://github.com/sergiobenitez/figment/blob/master/_autodocs/configuration.md):
  join and merge precedence, the grammar of environment values, the options of `Serialized`;
* [Figment.md](https://github.com/sergiobenitez/figment/blob/master/_autodocs/api-reference/Figment.md):
  `join` against `merge`, with `extract_inner`;
* [errors.md](https://github.com/sergiobenitez/figment/blob/master/_autodocs/errors.md): the
  `Kind` table, and inspecting `kind`, `path`, `origins()` and chained errors;
* [Error.md](https://github.com/sergiobenitez/figment/blob/master/_autodocs/api-reference/Error.md):
  origins and what `Display` lists;
* [Metadata.md](https://github.com/sergiobenitez/figment/blob/master/_autodocs/api-reference/Metadata.md):
  `Metadata::named`, `interpolater`, `interpolate`;
* [providers-Serialized-Named.md](https://github.com/sergiobenitez/figment/blob/master/_autodocs/api-reference/providers-Serialized-Named.md):
  `Serialized::defaults`, `globals`, `default`, `global`, and its `Provider` impl;
* [providers-Data-Format.md](https://github.com/sergiobenitez/figment/blob/master/_autodocs/api-reference/providers-Data-Format.md):
  `Data::required`, false by default.

And serde (library `/websites/serde_rs`): [container attributes](https://serde.rs/container-attrs.html),
[field attributes](https://serde.rs/field-attrs.html) and
[struct flattening](https://serde.rs/attr-flatten.html), for `flatten` and `deny_unknown_fields`.
