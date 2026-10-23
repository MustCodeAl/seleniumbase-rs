---
title: "Changelog"
---

Generated from `CHANGELOG.md` by `tools/docs7_changelog.py`. The versions follow
Semantic Versioning.

## Latest release: v0.6.0

<Columns cols={3}>

<Card title="Release notes" icon="github" color="#1a85d4" href="https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.6.0">
  v0.6.0 on GitHub.
</Card>

<Card title="Source (.zip)" icon="file-zipper" color="#199f43" href="https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.6.0.zip">
  The source of this release as a zip archive.
</Card>

<Card title="Source (.tar.gz)" icon="box-archive" color="#d47628" href="https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.6.0.tar.gz">
  The source of this release as a tarball.
</Card>

</Columns>

The releases publish no prebuilt binaries; install the command line from the tag:

```bash Terminal
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs --tag v0.6.0 seleniumbase-rs-cli
```

## All releases

<Update label="Unreleased" description="Next release" tags={["New", "Fix"]}>

### Breaking

- **An unknown key in the settings file is an error.** `unknown setting "blok_images" in
  sbase_config.toml` names it (and `sbrs options` lists the options), and two spellings of one
  key (`dark` and `dark_mode`) are refused; `Settings.chromium_arg` is an `Option<ArgList>`.
- **Test results carry a status.** `TestResult.passed` is now `status: TestStatus` (passed,
  failed, skipped, untested, unreported) plus `log_dir`; `ReportSummary` counts the new states;
  `render_html_report` and `write_html_report[_async]` moved to `core::dashboard` and take a
  `RunSnapshot` and `PageOptions`.
- **The behave port is removed; Gherkin runs through rstest-bdd.** Module
  `seleniumbase_rs::behave` (`StepRegistry`, `AsyncStepRegistry`, `CommonSteps`, `run_feature_*`,
  `RunFilter`) and `examples/behave_feature.rs` are gone: a feature file is now a cargo test with
  `#[scenario]` and the `bdd::sb` fixture (see Added).
- **A recorded `goto` is written as `sb.goto(..)`**, its own BaseCase call, not `sb.open(..)`.

- **`Settings::screenshot_dir` and `SB_SCREENSHOT_DIR` are removed.** The library read neither;
  a settings file that still has the key loads as before.
- **The presentation and chart helpers have Python's names, arguments and files.** Each is a call
  builder with Python's keyword arguments, so `sb.create_pie_chart().title("T").unit("%").await?`,
  `sb.add_slide("<h1>Hi</h1>").notes("n").await?`, `sb.save_presentation().show_notes(true).await?`
  and `sb.display_chart().interval(Duration::from_secs(3)).await?`. A chart or a presentation has a
  name (`.chart_name(..)`, `.name(..)`; `default` when none is given), so several can exist at
  once; `create_*_chart` takes the title as a keyword, not first; `add_data_point` takes an `f64`
  and `.color(..)`; `add_series_to_chart()` starts a series that the next data points fill instead
  of taking its data; `extract_chart` returns the chart's HTML, not JSON; slides are HTML with
  `image`, `code`, `iframe`, `content2`, `notes` and a `transition`, so the Markdown subset is
  gone; and `create_presentation` takes a `PresentationTheme` and a `Transition`, not a title.
  Files go to `saved_presentations/my_presentation.html` and `saved_charts/my_chart.html`
  (`.filename(..)`, `.folder(..)`), not to `latest_logs`, and a name that does not end in
  `.html` is refused. `begin_presentation` shows a saved presentation and waits for its last
  slide, and returns `None` in a headless browser or on a virtual display; `PieChart`,
  `ChartSeries::add_data_point(i32)` and `Presentation::new(title)` are replaced.
- **`sbrs install` needs a driver name**, as in Python: `sbrs install` alone prints the
  install page and exits with status 2 instead of installing chromedriver.
- **`Browser` and `DriverVersion` have new variants.** `Browser::Remote` (`--browser=remote`) and
  `DriverVersion::{Keep, Previous}`: a `match` on either needs the new arms.
  `BrowserConfig::with_grid` returns a `Result`, because an empty server name is an error.
- **Chrome is no longer given a proxy login.** `--proxy=user:pass@host:port` used to put
  `user:pass@host:port` in `--proxy-server`, where Chrome reads no login and the process list
  shows it; Chrome now gets `--proxy-server=host:port`. A WebDriver session answers the proxy's
  login another way (see Added).


- **`RunSettings` no longer implements `Eq`.** It now holds the timeout multiplier, a
  floating-point number; `PartialEq` stays.
- **`set_jqc_theme` and `reset_jqc_theme` are synchronous and keep the theme in the case.**
  `set_jqc_theme(theme, color, width)` takes Python's three arguments (`color` and `width` are
  `Option<&str>`), checks them with Python's messages and stores them for the next `get_jqc_*`
  dialogs; it no longer writes `jconfirm.defaults.theme` into the page and takes `&mut self`.
  `reset_jqc_theme()` really brings back the factory theme (`bootstrap`, `blue`, `38%`, not
  `light`); Python's does nothing. Unlike Python's process-wide setting, the theme belongs to the
  case.
- **`post_message`, `post_success_message` and `post_error_message` show Python's Messenger
  widget and take Python's keyword arguments as builder methods.** They drew a plain box before and
  took no `duration`, `pause` or `style`. `sb.post_message("Hi").await?` still compiles; now
  `sb.post_message("Hi").duration(Duration::from_secs(1)).pause(false).style(MessageStyle::Success)
  .await?` is Python's `post_message("Hi", 1, False, "success")` (the success and error calls
  have `duration` and `pause`). The message is HTML, as in Python: escape text that is not yours.
  A message stays for its `duration`, else the run's message duration, at most 0.75 s where nobody
  watches, and the call returns 0.15 s after it has gone unless `pause` is false.
  `post_message_for(message, duration)` is removed: write `post_message(message).duration(duration)`.
  `post_message_and_highlight` posts a success message and does not wait for it.
- **The default Messenger theme is `future`, as in Python (it was `flat`).**
  `MessengerTheme::default()`, `MessengerSettings::default()` and `"default".parse::<MessengerTheme>()`
  give `future`, so a `MessengerLocation` alone, for `set_messenger_theme`, keeps the future theme.

### Added

- **SeleniumBase's built-in Gherkin steps (feature `bdd`).** All 98 step actions of SeleniumBase
  4.56.2 (its 669 wordings, in double or single quotes, under Given, When and Then) are steps of
  `testing::bdd::steps`, so a feature file can `Open "..."`, `Type "demo" into "#user"`,
  `Click "button"` and `Assert text "Welcome" in "h1"` with no step code of its own.
- **Every browser option in one table, three ways.** Each option of the run table is a settings
  file key (by any alias, with dashes or underscores), an `SB_*` variable and a global `sbrs`
  flag (`sbrs --dark --metrics 360,640,2 open URL`), in that order of precedence after the flag;
  `chromium_arg` takes a TOML array kept unsplit; `sbrs options` (`--markdown`, `--all`) lists
  them. The run settings `highlights`, `interval`, `verify_delay`, `check_js`,
  `wait_for_angularjs`, `disable_beforeunload` and `timeout_multiplier` now reach the browser.
- **A run's results across test binaries (`core::test_run`).** `TestRun` keeps one run's state in
  `<root>/.sbrs_run/` under a file lock, so every test binary of a `cargo test` or nextest run
  adds to the same report (`SB_RUN_ID`, else `NEXTEST_RUN_ID`, else the parent cargo process);
  `RunFinalizer` records each attempt of an `sb` fixture test with the evidence it took, and
  `finalize` marks unfinished tests unreported. `core::log_helper` has Python's log folder rules
  and writers (`log_test_failure_data`, `log_page_source`, `log_screenshot`, `archive_logs_if_set`);
  the `[report]` settings table and `SB_HTML`, `SB_JUNIT_XML`, `SB_ALLURE_DIR`, `SB_REPORT_DB` say
  where reports go. `RunReporterPlugin` reports through the plugin runner.
- **Allure results (feature `allure`).** `core::allure::AllureSink` writes Allure 2 results into
  `SB_ALLURE_DIR` (`<uuid>-result.json` per test with status, durations, labels, failure message and
  trace, plus the screenshot, page source and test info of a failure as attachments, and
  `environment.properties`), so `allure serve <folder>` shows the run. A run adds the sink itself
  when the folder is set, and a test is one result however many sinks hear of it. The JSON is
  written with `serde`: the feature adds no dependency.

- **Gherkin scenarios as cargo tests (feature `bdd`).** `testing::bdd::&#123;sb, sb_cdp, SbCase,
  SbCdpCase&#125;` run a feature file's scenarios through rstest-bdd 0.6: `#[scenario(path = ..)]` on
  an async test that takes `#[future(awt)] sb: SbCase`, and `#[given]`/`#[when]`/`#[then]` steps
  that take `sb: &mut SbCase`. A failed step leaves the screenshot, page source and test info in
  `latest_logs/<test id>/` with the page still open, and the browser always closes. `sbrs
  behave-gui` runs the scenarios you mark with `cargo test`.
- **Recorder Mode pipeline, ported from SeleniumBase 4.56.2.** `api::recorder` now carries
  Python's page script (`RECORDER_JS`, `SHIFT_ESC_JS`, `recorder_injection()`), its rewrite
  passes (`process`, `add_parent_frame_switches`, over `RawAction` rows) and its code generation
  (`to_actions`, `ActionRecorder::from_raw_actions`), and writes a recording as an rstest test on
  the `sb` fixture (`to_rstest_test`) or a Gherkin scenario (`to_gherkin_scenario`). A typed
  password is marked secret in the page and generated as `recorded_secret()?`, read from
  `SB_RECORDED_SECRET`. Golden tests check the processed rows and calls against SeleniumBase's own
  code on 11 recordings.
- **`sbrs install` fetches more.** `sbrs install` (and `get`) now installs `uc_driver` (a
  patched chromedriver), `cft` and `chs` (Chrome for Testing and its headless shell) and
  `chromium` (a Chromium snapshot with the chromedriver of the same revision, `--revision N`),
  reads the version words `previous`, `mlatest` and `keep`, and copies the driver into
  `SB_DRIVER_PATH_DIR` (else `/usr/local/bin`) with `-p`/`--path`, refusing a symlink at the
  target. `--browser chromium` finds a downloaded or installed Chromium, else downloads the
  newest snapshot, and matches its chromedriver to it.

- **`sbrs proxy`.** A minimal forward proxy for this machine: `sbrs proxy [--hostname HOST]
  [--port PORT]` listens on 127.0.0.1:8899, tunnels `CONNECT host:port` (so HTTPS passes
  through untouched) and forwards an absolute `http://` request once with `Connection: close`.
  It has no authentication and no TLS interception, warns when it listens on an address other
  than loopback, logs one line per request at INFO, refuses a request head over 64 KiB, serves
  at most 256 connections at once, drops a connection that carries no byte for 30 seconds and
  stops on Ctrl-C or SIGTERM with status 0. Use it with the global `--proxy 127.0.0.1:8899`.

- **`sbrs download server`.** Downloads Selenium Server 4 into `downloaded_drivers/`
  (`--version 4.x.y`). The default release, 4.51.0, has its SHA-256 pinned in the source and a
  download that differs is not saved; another version is saved as it arrives and printed with
  its SHA-256 and `not verified`. Python's Selenium 3 server is not offered. `sbrs grid-hub`
  and `sbrs grid-node` now run the newest jar in that folder when neither `--jar` nor
  `SB_SELENIUM_SERVER_JAR` names one.

- **`sbrs encrypt` and `sbrs decrypt` prompt on a terminal.** With no argument and a terminal on
  standard input they ask as Python's `obfuscate` and `unobfuscate` do: the text and its
  verification without echo, then the token with the line that decrypts it in a test, again
  and again until Ctrl-C (`Exiting...`, status 0). When `SB_ENCRYPTION_KEY` is not set they ask
  once for the passphrase without echo (`encrypt` twice, to catch a typo); it is never printed
  or saved. A pipe or file on standard input behaves as before.

- **A presentation plays by itself and a chart waits for its interval.** `--interval`
  (`RunSettings::with_interval`) is the autoplay time of `save_presentation` and
  `begin_presentation` (`autoSlide`) and how long `display_chart` shows a chart, unless the call
  gives its own `.interval(..)`. Themes (`PresentationTheme`: `serif`, `sky`, `white`, `black`,
  `simple`, `league`, `moon`, `night`, `beige`, `blood`, `solarized`) and transitions
  (`Transition`: `none`, `slide`, `fade`, `zoom`, `convex`, `concave`) are enums that also parse
  from Python's names. A chart can be put on a slide with `extract_chart`, and the deck loads the
  chart library once. Charts are still drawn with Chart.js: `unit` titles the value axis and
  ends the tooltip, `zero` starts a line or area chart at zero, `legend` and `labels` show or
  hide the legend and the values, and `libs` leaves the `<script src>` out of `extract_chart`.

- **The MCP servers offer resources, prompts, structured results and progress.** The `cdp`,
  `driver` and `sb` servers answer `resources/list`, `resources/templates/list` and
  `resources/read` for `sbrs://journal` (the calls so far, with typed text, scripts and results
  left out), `sbrs://session` (engine, URL, title, ready state) and `sbrs://output/{+path}` (a
  file under `SB_MCP_OUTPUT_DIR`: text as text, anything else as base64; `..`, an absolute path,
  an escape or a link that leaves the folder is refused), and `prompts/list` and `prompts/get`
  for `write_test`, `debug_failure` and `extract_data`. An object result of a tool is also sent
  as `structuredContent` next to its text, and a `tools/call` with `_meta.progressToken` gets
  `notifications/progress` from the tools that report (`Ctx::progress`). The tools that return
  page text (`get_content`, `get_attributes`, `get_text`, `get_page_source`, ...) start with a
  notice that the text is data and carry `_meta: {"sbrs/untrusted_content": true}`. For hosts
  built on the library: `ToolDef::with_output_schema` and `untrusted`, `Host::with_resources`,
  `with_prompts` and `call_with`, and the types `ResourceDef`, `PromptDef`, `Progress` and
  `ProgressSink`.

- **`sbrs test` takes every option of Python SeleniumBase's pytest plugin.** One flag for each row
  of the option table, built from the table itself: the browser and page options (`--firefox`,
  `--headed`, `--uc`, `--dark`, `--no-sandbox`, ...), the test data (`--env`, `--account`, `--data`,
  `--var1` to `--var3`, `--variables`, checked before Cargo starts), the debug modes (`--pdb`,
  `--trace`, `--final-debug`, which run one test at a time with its output shown), `--reruns`,
  `--time-limit`, the evidence and report options and the recorder. Each is accepted in every
  spelling and alias Python has (`--no-sandbox`, `--no_sandbox`, `--dcsp`); a switch can be turned
  off with `--flag=false`; the options that do nothing here (`--fast`, `--with-selenium`, ...) are
  accepted and ignored. They reach the tests as `SB_*` variables. The pytest options are there too:
  `-k` and `-m` (a marker is a module name; `a or b` and `not c` become libtest filters and skips,
  anything else asks for `--runner nextest`, where it is a filterset), `-n N|auto`, `-s`,
  `--co` (lists the tests and prints `collected N items`), `--runner cargo|nextest`,
  `-c FILE` or `--settings-file FILE` (passed on as an absolute `SB_SETTINGS_FILE`), and
  `--all-scripts`, which runs every example after the tests. `sbrs behave-options` prints how the
  behave options map to these flags.
- **`sbrs test` empties `latest_logs/` and `downloaded_files/` before a run, as pytest does**, or
  moves them to `archived_logs/logs_<epoch>/` and `archived_files/downloads_<epoch>/` with
  `--archive-logs` and `--archive-downloads`. It does so in the folder of each package the tests
  run in and never for `--co`; plain `cargo test` touches nothing.
- **Library helpers for a test launcher:** `cli::scripts::run::{Runner, list_names}` and the
  nextest, list, skip and capture builders of `TestRun`, `run_filter::translate` (the `-k` and `-m`
  expressions), `run_logs` (reset or archive the logs folders), `all_scripts` (the examples of a
  package) and `behave_options`.


- **Shell completion says what each value is.** A selector, text, key, value, attribute, property
  or CDP method (`--css`, `--text`, `--key`, `--value`, `--attribute`, `--property`, `--cmd`,
  `--params`, `--hover-css`, `--click-css`, `--source-css`, `--target-css`) no longer completes
  file names, and neither does a number, a URL (`open`, `smoke`, `download-file`, `print-pdf`,
  `pdf-text`, `record`, `mkfile --url`, `--start-page`, `--proxy-pac-url`) or a free-form option.
  Files complete by the kind a command takes (`.json` or `.yaml` for `export-recording` and
  `replay`, `.rs` for `mkfile` and the generated tests, `.png` for `screenshot`, `.toml` or
  `.json` for `--config`, ...), folders for `mkdir`, `--user-data-dir` and `--extension-dir`,
  programs for `patch-chrome`, `patch-chromedriver` and `devtools --browser-path`.
  `sbrs translate FILE` offers the language codes, and `sbrs fingerprint export --preset` its
  presets.
- **`sbrs install` completes its driver and its version.** The driver is a value enum
  (`chromedriver`, `geckodriver`, `edgedriver`; `firefoxdriver` and `msedgedriver` still work), and
  the version offers `browser`, `stable`, `latest`, `beta`, `dev` and `canary`. The Fig spec
  (`sbrs completions fig`) marks the global options `isPersistent` and keeps the file template of
  a path limited to extensions, and `docs/help/cli_reference.md` shows each command's examples in
  a `bash` block titled Examples and says the global options go before or after the command.
- **One option table for a test run (`config::test_options`).** Every Python SeleniumBase
  option, whether a pytest flag or an `SB(...)` keyword, is a row of `TEST_OPTIONS`: the `sbrs`
  flags that spell it (`_` and `-` alike), the `SB_*` variable that carries it, and the
  `Settings` field it lands in. `find_option("--dark")` finds a row by any spelling, and
  `tests/test_options_table.rs` fails when `parity/api.toml` lists an option the table does not
  spell. `Settings::load` reads the variables from the table, in the order and with the
  messages it always had.
- **`TestSettings` and `TestData` (`config::test_settings`), the `[test]` table of the settings
  file.** `TestData` holds what a test reads: `env` (one of the 24 environments of Python
  SeleniumBase, `test` by default, checked and lower-cased), `account`, `data`, `var1` to
  `var3` and `variables`, a JSON object or a Python dictionary literal
  (`TestData::parse_variables`). `TestSettings` holds the debug modes (`pdb`, `trace`,
  `final_debug`), `reruns` and `reruns_delay`, `time_limit`, the evidence and report switches,
  the recorder switches and `runner`. The settings stay on `LaunchSettings::test_data` for the
  `sb` fixture.
- **The rstest `sb` and `sb_cdp` fixtures (feature `testing`).** `#[rstest] #[tokio::test] async fn
  t(sb: Sb) -> Result&lt;()> &#123; sb.run(async |sb| &#123; .. &#125;).await &#125;` runs a test on a WebDriver
  `BaseCase` (or `sb_cdp: SbCdp` on a pure CDP `Page`) with the whole run configuration: the
  settings file, `SB_SETTINGS_FILE` and the `SB_*` variables. `run` reruns a failed test
  (`reruns`, `reruns_delay`, printing `RERUN <test id> (attempt 2 of 3) after: <error>`), stops one
  that runs past `time_limit` with Python's `This test has exceeded the time limit of N seconds!`
  (the time a prompt is open does not count), leaves `latest_logs/<test id>/screenshot.png`,
  `page_source.html` and `basic_test_info.txt` while the browser is still open
  (`no_screenshot`, `save_screenshot` for a pass, `list_fail_page`), asks at the `trace`, `pdb`
  and `final_debug` prompts (skipped with a warning where nobody is at a terminal), and closes the
  browser whatever happened. A panic is a failure too and is raised again after the last attempt.
  `Sb::configure` is the Rust form of Python's `SB(...)` arguments, and `testing::Finalizer` sees
  every attempt end with the page open, for the reports, the plugin host and the recorder.
  `docs/tutorials/writing_tests.md` shows it.
- **`BaseCase` reads the test data of the run** as Python's `sb` does: `data()`, `env()` and
  `environment()` (`test` by default), `account()`, `var1()` to `var3()`, `variables()`, and
  `test_id()`, the id that names the test's folder of evidence.
- **`common::test_id::current_test_id`**, the id of the running test (`cart.checkout.renders`: the
  test binary, the module path and the function, one plain file name), replaces the debug prompt's
  thread-name stub.
- **A key and a variable for each browser option.** `Settings` gains `dark_mode`, `devtools`,
  `incognito`, `guest_mode`, `disable_ws`, `swiftshader`, `enable_3d_apis`, `maximize`,
  `no_sandbox`, `disable_gpu`, `chromium_arg`, `disable_features`, `host_resolver_rules`,
  `server`, `port`, `protocol`, `locale_code`, `window_size`, `window_position`,
  `device_metrics`, `page_load_strategy`, `driver_version`, `proxy_bypass_list`, `cap_file`,
  `cap_string`, `extension_zip`, `firefox_arg`, `firefox_pref`, `xvfb`, `xvfb_metrics`,
  `remote_debug`, `log_cdp_events`, `block_images`, `disable_js`, `disable_cookies`,
  `do_not_track`, `external_pdf`, `enable_sync`, `disable_csp`, `use_auto_ext`, `demo`, `slow`,
  `skip_js_waits`, `crumbs` and more, each with its `SB_*` variable
  (`SB_DARK_MODE`, `SB_WINDOW_SIZE`, ...). `Settings::to_browser_config` puts them on the typed
  `BrowserConfig` builders, and a value that does not parse is refused by `Settings::validate`.
- **`Settings::to_launch_options` for the pure CDP engine.** The same settings, as the
  `LaunchOptions` of `sb_cdp`, with the Chromium switches both engines share.
- **`SB_SETTINGS_FILE` names the settings file** that `Settings::load` and `Settings::check`
  read instead of the global file of the working folder.


- **A WebDriver session logs in to a proxy that asks for a password.** `--proxy=user:pass@host:port`
  (or a PAC URL with a login) now works with Chrome, Edge and the other Chromium browsers on a
  chromedriver this machine runs, as the pure CDP engine already did. Python SeleniumBase writes a
  browser extension for it, which Chrome no longer loads from the command line; this crate attaches
  a DevTools client beside chromedriver (the `debuggerAddress` of the session's capabilities) that
  turns `Fetch` on in every tab, also one the page opens or a WebDriver new window, and answers the
  proxy's `407` with the login. A website's own `401` is never given the proxy account, and the
  login is neither in the browser's command line nor in an error message. The client closes with the
  session (and is started again for each lease with `--reuse-session`). A remote grid with a proxy
  login is refused when the session connects, before a browser starts, because the grid does not
  share its browser's DevTools endpoint: use the grid's proxy capability (`--cap-string`) or a proxy
  without a login.

- **The pure CDP engine takes the browser options of the WebDriver engine.**
  `LaunchOptionsBuilder::mobile` and `device_metrics` make every tab, the first and each one
  opened later, a phone: `Emulation.setDeviceMetricsOverride` with `mobile: true` (390 by 715
  at a ratio of 3 unless metrics are given), five touch points, and the `Android` user agent
  with matching client hints. `dark_mode`, `devtools`, `disable_web_security`, `swiftshader`,
  `maximize`, `disable_features`, `enable_features`, `host_resolver_rules`,
  `proxy_bypass_list`, `proxy_pac_url` (a login the PAC file asks for is answered over the
  protocol), `virtual_display` (Xvfb, Linux, headed) and `switch(LaunchSwitch, bool)` are
  public. `switch` writes `BlockImages`, `DisableJavaScript`, `DisableCookies`, `DoNotTrack`
  and `ExternalPdf` as profile preferences, makes `DisableCsp` call `Page.setBypassCSP` in
  every tab, and logs at debug level the switches that only the WebDriver engine can use.
  A maximized window gets no `--window-size`, a headless browser neither developer tools nor
  a maximized window, and this engine never switches the GPU off by itself.
  `LaunchOptionsBuilder::browser_config(&BrowserConfig)` reads everything of a
  `BrowserConfig` that the engine can use, as Python's `sb_cdp.Chrome(**kwargs)` reads its
  options, and lists at debug level what it cannot (page load strategy, capabilities, driver
  version, Firefox options, the DevTools event log, extension archives); a bad proxy or PAC
  address fails `build()` as it fails the WebDriver engine. `build()` refuses `mobile` together
  with a `fingerprint`, which both set the screen and the user agent.
- **`Emulation` changes a running tab.** `device_metrics(metrics, mobile)`,
  `clear_device_metrics()`, `touch(enabled)` and `color_scheme(ColorScheme)` (`Light`,
  `Dark`, `NoPreference`: what `prefers-color-scheme` reports).

- **Proxies are checked the way Python checks them.** `browser::options::ProxyAddress`
  (`[scheme://][user:pass@]host:port`, schemes `http`, `https`, `socks4`, `socks5`, `socks5h`),
  `ProxyScheme` and `PacUrl` (`[user:pass@]URL`, which must end in `.pac`) refuse a bad string
  with the password hidden (`user:***@`), and so do their `Debug` and `Display`. Firefox gets
  `network.proxy.*` preferences from a proxy, a PAC file and the bypass list; a login on a Firefox
  proxy, or on a SOCKS proxy, is refused with Python's message. `BrowserConfig::validate()` runs
  these checks (and the `--browser=remote` ones) before a browser starts; `BrowserSession::connect`
  calls it.
- **Selenium Grid addresses.** `browser::options::grid_url` and `GridProtocol` build the WebDriver
  URL Python's `get_remote_driver` builds (`USER:KEY@hub.browserstack.com` stays in the URL, an
  `https://` server forces https, `/wd/hub` is added, a port the protocol already uses is left
  out), and `BrowserConfig::with_grid(protocol, servername, port)` sets it (`localhost` keeps the
  local driver). `BrowserConfig::with_binary_location` is Python's `binary_location`.
- **`Browser::Remote`** (`--browser=remote`) sends the WebDriver server nothing but the
  capabilities of `--cap-file` and `--cap-string`; mobile mode, switches and window geometry do not
  apply to it.
- **`DriverVersion` keywords `keep`, `previous` (`latest-1`) and `mlatest`.** `keep` runs a
  chromedriver or msedgedriver that is already in `downloaded_drivers/` or on the `PATH` and
  downloads one only when there is none; `previous` is the newest driver of the major version
  before the stable one; `mlatest` is the newest build of the browser's own major version, which
  `browser` already was.
- **The remote debugger gives way.** `with_remote_debug(true)` takes a free port, and logs it, when
  9222 is in use; a port named with `with_remote_debug_port` that is in use is an error
  (`port N is already in use`). `BrowserSession::remote_debug_address()` says where it listens.



- **The Python page and wait options are in `RunSettings`:** `with_highlights` (`--highlights`,
  zero is unset, `DEFAULT_HIGHLIGHT_LOOPS` is 4), `with_interval` (`--interval`, zero is unset),
  `with_verify_delay` (`--verify-delay`, `DEFAULT_VERIFY_DELAY` is 500 ms), `with_check_js`
  (`--check-js`), `with_wait_for_angularjs` (`--wfa`), `with_disable_beforeunload`
  (`--disable-beforeunload`) and `with_timeout_multiplier` (`--timeout-multiplier`), each with a
  getter. The highlight count, the interval and the verify delay are held for the highlight
  calls, the tours and presentations, and MasterQA, which do not read them yet.
- **`open`, `go_back`, `go_forward`, `refresh` and `wait_for_ready_state_complete` run the page-load
  hooks the settings ask for:** the AngularJS wait, the check of the browser log for severe
  JavaScript errors and the switch-off of the `beforeunload` dialog (the last two on Chromium
  browsers only). `--skip-js-waits` skips all three. A click waits for AngularJS too under `--wfa`.
- **`--timeout-multiplier` scales the default wait:** `ceil(multiplier * seconds)` for every wait
  that names no time of its own, `set_timeout` included; a time a call names is kept, as in
  Python. A multiplier below 0.5 is raised to 0.5 and one that is not a finite number is ignored.
  `RunSettings::scale_timeout` gives the scaled time.
- **`api::call::CallFuture` is the future of the call builders:** an async method that takes
  Python's keyword arguments will return a builder whose methods set the keywords and whose
  `.await` runs the call, so `sb.method(arg).await?` keeps working. The empty module
  `api::demo` is where the demo mode will go; `api::messages` holds the Messenger call builder.
- **jquery-confirm dialog boxes: `get_jqc_button_input`, `get_jqc_text_input` and
  `get_jqc_form_inputs`.** Each opens a themed dialog in the page, waits for the person and returns
  what was clicked or typed (`api::jqc`). They are call builders: `.options([("theme", "material"),
  ("width", "52%")])` is Python's `options`, `.button(("Send", "green"))` is the text input's
  `button`, and `.theme`, `.color` and `.width` take the typed `JqcTheme`, `JqcColor` and
  `JqcWidth`. A button is a text, a text and a colour (`("Yes", "green")`) or a `JqcButton`; one
  button answers to Enter, `y` and `1`, several to `y` for `Yes`, `n` for `No` and their numbers.
  The arguments are checked with Python's messages. In a headless browser or on a virtual display
  no dialog opens and no script is sent: the calls return the text of the last button, `""` and
  `("", last button text)`, as Python does. A dialog waits as long as the person takes; the test's
  time limit ends the wait, and so does a page that goes away, with an error that says the dialog
  closed without an answer. `BaseCase::jqc_defaults` reads the stored theme.
- **`BaseCase::with_somebody_watching` (feature `test-util`)** makes a headless case behave as if a
  person watches, so a test can play the person for the dialogs that wait for one.
- **`activate_messenger` loads everything Python loads, and the pure CDP engine's too:** the five
  theme style sheets (flat, future, block, air, ice), the spinner, Underscore 1.13.6,
  SeleniumBase's own Messenger style and the scripts of the flat and future themes, then it starts
  Messenger with Python's options (future theme, bottom right, eight messages). Files the page
  already has are not added again. It is one script, so a page that blocks a file fails with an
  error that names it.
- **A message the page cannot show is printed:** when Messenger cannot be loaded (offline, or a
  policy that blocks the scripts) `post_message` prints ` * INFO message: text` (the style in
  capitals) and still pauses, as Python does; it does not fail the test. `api::messages::fallback_line`
  gives the line.

### Fixed

- **A message stays at most 0.75 s on a virtual display too:** `post_message` and its relatives
  cut the time when nobody watches, which is a headless browser or a headed one on Xvfb (Python:
  `self.headless or self.xvfb`); a headed browser on Xvfb kept the long time before.
- **`-n` belongs to `sbrs test`.** The global `-n/--threads` is now `sbrs test -n N` (or `auto`);
  `sbrs -n 2 test` no longer parses. `SB_THREADS` and the `threads` key of the settings file
  still set it.
- **`Settings::screenshot_dir` and `SB_SCREENSHOT_DIR` are removed.** The library read neither;
  a settings file that still has the key loads as before.

</Update>

<Update label="2026-10-23" description="v0.6.0" tags={["New", "Fix", "Breaking"]}>

**Downloads:** [Release notes](https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.6.0) · [Source (.zip)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.6.0.zip) · [Source (.tar.gz)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.6.0.tar.gz)

### Breaking

- **Downloads go to `./downloaded_files`, as in Python SeleniumBase.** `BaseCase::get_downloads_folder`,
  `assert_downloaded_file`, `download_file` and the other download helpers used the OS Downloads
  folder; they now use `downloaded_files` in the working directory
  (`browser::downloads::downloads_dir`), and a browser the library launches is told to save
  there: WebDriver sessions ask for it in their capabilities (the Chrome and Edge preferences
  `download.default_directory`, `download.prompt_for_download` and `download.directory_upgrade`,
  and the Firefox `browser.download.*` ones; a preference you set yourself wins), and the pure CDP
  engine sends `Browser.setDownloadBehavior` at launch, so `Page::assert_downloaded_file` finds a
  file without a folder being chosen first. `downloaded_files/` is in `.gitignore`.
- **A reused browser keeps its cookies unless `--crumbs`.** With `--reuse-session` or `--rcs` the
  browser handed to the next test used to have every cookie deleted; now the cookies stay (Python's
  default) and `SB_CRUMBS=1` or `browser::reuse::set_crumbs(true)` deletes them. The other windows
  are still closed, the page's storage cleared and the page set to `about:blank`.
- **A page command needs a page.** `get-text`, `get-title`, `get-attribute`, `screenshot`,
  `assert-element`, `hover`, `send-keys`, the alert commands and the other commands that act
  on a page used to act on `about:blank`, which no one wants. Without a start page (`--url`,
  `SB_START_PAGE` or the settings file) they now stop with `error: NAME acts on a page, and
  every sbrs command starts a fresh browser; give the page: sbrs --url https://example.com
  NAME ...` and status 2, before any browser starts. `cdp` needs no page.
- **A command that changes the profile needs `--user-data-dir`.** `clear-cookies`,
  `load-cookies`, `set-local-storage-item`, `remove-local-storage-item`,
  `clear-local-storage` and `cache-clear` changed a profile that a fresh browser throws away
  at the end; they now stop with `error: NAME changes the profile, which a fresh browser
  throws away; add --user-data-dir DIR` (status 2) unless the folder is named.
- **Fifteen commands are removed** because they have no observable effect in a one-shot
  browser: `go-back`, `go-forward`, `refresh`, `switch-to-window`, `switch-to-frame`,
  `switch-to-default-content`, `switch-to-parent-frame`, `switch-to-newest-window`,
  `switch-to-default-window`, `open-new-window`, `open-new-tab`, `close-window`,
  `set-window-position`, `get-active-element-css` and `throttle3g` (an unrecognised
  subcommand now). Several steps in one browser are `run-scenario`, `deferred` and the MCP
  server. `export-recording` stays: it writes a test from an action file and starts no browser.
- **A command with a URL of its own refuses `--url`.** `open`, `smoke`, `download-file`,
  `print-pdf`, `pdf-text`, `assert-pdf-text` and `record` stop with `error: NAME opens its
  own URL; leave out --start-page` when the command line names a start page; one that comes
  from `SB_START_PAGE` or the file is only a default and is ignored for them.
- **`devtools`, `choose-file` and `deferred` read the global options.** They no longer have
  an option of their own for `--headless`, `--user-data-dir`, `--proxy` or `--url`; the
  global ones of the same names do the same work, and give the page to open (`choose-file`
  and `deferred` act on the start page, a blank upload page when `choose-file` has none).
  The same global option on both sides of the command is a repeat and is refused.


### Added

- **A debug prompt: `BaseCase::breakpoint`, `sb_cdp::Page::breakpoint` and
  `common::debug_prompt`.** `sb.breakpoint("message").await?` pauses a test with the browser
  open and reads commands from the terminal: `c` goes on, `s` saves a screenshot and `src` the
  page source to `latest_logs/<test id>/`, `u` shows the address and title, `js <expr>` shows
  a value, `q` stops the test with a `TestLifecycle` error and `h` lists the commands. The time
  spent at the prompt is added to the time limit. The pause is skipped, with a warning in the
  log, when standard input is not a terminal (CI, `cargo nextest`), and `SB_BREAKPOINTS=0` turns
  it off, as Python's `PYTHONBREAKPOINT=0` does. `DebugPrompt`, `PromptKind`, `PromptOutcome`,
  `PromptPage` and `LineInput` are public, for the runner's `--trace`, `--pdb` and
  `--final-debug` (not wired to a flag yet). `BaseCase::recorder_mode` says whether a
  recording window is open.
- **A command line that does not parse prints the command's page, then the error.** Like
  Python's `sbase`, `sbrs mkfile`, `sbrs grid-hub`, `sbrs get-text` and `sbrs plugin info`
  print the usage, arguments, flags and examples of the command the words reached, a blank
  line, and last the parser's `error:` line and `tip:`, on standard error with status 2. A
  mistake found after parsing (a page command with no page, a start page for a command with a
  URL of its own) prints the same way. An unknown command, or none, prints the commands by
  group, names only, instead of the 112-line help. `--help`, `--version` and shell completion
  are as before.
- **Every command has an example and a help heading.** `sbrs <command> --help` ends with an
  `Examples:` section (`sbrs --url https://example.com get-text --css h1`), the commands are
  grouped as Page commands, Files and generators, Record and replay, Tests, Drivers, grid and
  servers, Import and convert, Security, and Info and setup in `sbrs --help`, and a test
  proves that every example parses. The examples are in the usage spec and in
  `docs/help/cli_reference.md`.
- **The Chromium switches are typed.** `BrowserConfig::with_dark_mode`, `with_guest_mode`,
  `with_incognito`, `with_devtools`, `with_disable_ws` / `with_enable_ws`, `with_swiftshader`,
  `with_enable_3d_apis`, `with_maximize`, `with_no_sandbox`, `with_disable_gpu`,
  `with_headless1` / `with_headless2`, `with_xvfb`, `with_chromium_arg` (Python's comma list,
  read by `parse_chromium_args`), `with_disable_features` and `with_host_resolver_rules`
  replace composing `extra_args` by hand. `--incognito` and `--guest` are left out, with a
  warning, when the session loads an extension. A headless session may keep its GPU
  (`with_enable_3d_apis`, `with_swiftshader`). Firefox gets `-private` and `-devtools`, and
  `security.csp.enable` for `--disable-csp`; Firefox and Safari are maximized once running.
- **`LocaleCode`** reads a language code in Python's form (`ar_eg`) or Chrome's (`ar-EG`) and
  writes Chrome's.
- **The start page: `--start-page`, `--url` and `--start_page`, `SB_START_PAGE`, and the
  `start_page` key of the settings file.** Every `sbrs` command starts a fresh browser, so
  a command that acts on a page now acts on this one: `sbrs --url https://example.com
  get-text --css h1`. The command line wins over `SB_START_PAGE`, which wins over the file;
  a bare host gets `https://` and a local file becomes a `file://` URL. `sbrs test --url X`
  hands it on to the tests as `SB_START_PAGE`. The library side is `Settings::start_page`
  (which `to_browser_config` copies into `BrowserConfig::start_page`),
  `SettingsOverrides::start_page` and `START_PAGE_ENV`.
- **`--save-screenshot`** saves a screenshot to `latest_logs/` just before a browser command
  quits and prints its path (`Saved screenshot: PATH`). A command that changes the page
  (`click-partial-link-text`, `hover`, `human-click`, `add-text`, ...) also prints where the
  browser ended up, `Now at: URL (TITLE)`, so a click that navigates shows where it went.
- **Every option of `sbrs` itself is global.** `sbrs get-title --headless --url
  https://example.com` works as well as `sbrs --headless --url https://example.com
  get-title`; the same option on both sides of the command is refused as a repeat (status 2).
  Every page command now ends with `quit`, where some left the browser to the drop-time
  cleanup.

- **Fingerprint capture, export and import.** A browser's fingerprint can now be read from
  a real browser and handed back to any launch path, as one file format everywhere: the v2
  `Fingerprint` JSON, described by the new `schemas/fingerprint.schema.json` (a test fails
  when the schema is stale). `Page::capture_fingerprint` (pure CDP) and
  `BaseCase::capture_fingerprint` (WebDriver) read what a page can see of the browser and
  return a `CapturedFingerprint` (the fingerprint, the readings no setting holds in
  `unmapped`, and a `ProbeInfo` with the page, the time and the browser version);
  `compare_fingerprint` checks that a live page presents a fingerprint and lists what no page
  can read as unverifiable instead of as a match. `Fingerprint::load`, `save` (atomic,
  mode `0600`), `from_json`, `to_json_pretty`, `diff` (`SettingDiff`), `compare_to`
  (`FingerprintComparison`) and `claimed_settings` are public.
- **`sbrs fingerprint capture|export|check|diff|presets`.** `capture [URL] [-o FILE]
  [--readings FILE]` prints or writes the fingerprint of the browser the global options
  start; `export (--preset NAME | --random [--seed N] [--os OS])` writes a preset or a seeded
  random identity; `check FILE` reports the errors and warnings of a file (status 2 when
  there is an error); `diff A B` lists the settings where `B` differs from what `A` expects
  (status 1 when any does); `presets` lists the presets.
- **The global `--fingerprint FILE`, `SB_FINGERPRINT` and a `[fingerprint]` table (`file =
  "..."`) in the settings file** make every browser a command starts present a fingerprint
  file. The command line wins over the variable, which wins over the file; a file that
  cannot be read stops the command before a browser starts. A Chromium browser runs in the
  CDP mode for it when no mode was asked for, and `sbrs devtools` applies it through the
  pure CDP launch options. `Settings::fingerprint`, `load_fingerprint`, `check_fingerprint`
  and `Settings::resolve_fingerprint_file` are the library side.

- **`sbrs-mcp install CLIENT...`, `sbrs-mcp uninstall CLIENT...` and `sbrs-mcp clients`**
  register the server with MCP clients, remove the registration, and list the clients
  with where each keeps its config and whether its config and program were found.
  One install registers two servers, so an assistant gets the browser and the
  development tools at once: the browser server as `seleniumbase` (`--server cdp`, the
  default, `driver` or `sb`) and the development server as `seleniumbase-dev`
  (`--server dev`, no browser). `--name` renames them (`NAME`, `NAME-dev`), `--only
  browser|dev` registers just one, `--output-dir` sets `SB_MCP_OUTPUT_DIR` (default:
  `sbrs-mcp` in your data folder), `--plugin` and `--plugin-opt` go to the browser
  server's arguments, `--scope project` writes the client's file in the current folder,
  `--print` shows what would be written or run, and `--force` replaces a different entry
  of the same name (without it the entry is left alone: exit status 2). The command
  registered is the absolute path of the running binary. Clients: `claude-code` (through
  `claude mcp add-json`), `claude-desktop`, `codex` (through `codex mcp add`, else
  `config.toml` edited with `toml_edit`), `cursor`, `gemini-cli`, `opencode`, `vscode` and
  `zed`. A config file is backed up to `FILE.bak-sbrs-mcp`, changed only in the entries
  concerned and replaced atomically, and a file with comments is not rewritten (the
  snippet to add by hand is printed). Running `sbrs-mcp` with no command still serves.
  See [the MCP server tutorial](/tutorials/mcp_server#install-into-a-client).
- **A fourth MCP server, `dev`** (`sbrs-mcp --server dev`, `mcp::Profile::Dev`,
  `mcp::dev::host`). It starts no browser and helps with developing with the
  library, using the `sbrs` command line and diagnosing problems, in twelve tools.
  Load it next to one browser server; the tools are not added to the browser
  servers, because every tool costs context in every session that loads one.
  - `find_api`: searches the public methods of `BaseCase` and of the pure CDP
    engine (`Browser`, `Page`, `Locator`, `BrowserContext`) and returns the type,
    the full signature, the first paragraph of the documentation, `is_async` and
    `deprecated`. The index is generated from the source by a test, committed as
    `src/mcp/dev/api_index.json`, and checked by a second test
    (`SB_UPDATE_API_INDEX=1` rewrites it).
  - `parity_lookup`: the status, Rust path and notes of a Python SeleniumBase 4.55.2
    name, from `parity/api.toml`, with close names when there is no exact match.
  - `python_to_rust`: the importer of `sbrs import-python`, with its diagnostics.
  - `scaffold`: the files `sbrs mkfile` and `sbrs mkdir` would create, as text; it
    writes nothing.
  - `cli_help`: the `sbrs` command line from its usage spec
    (`completions/sbrs.usage.kdl`), embedded and read by a small KDL reader in the
    library: commands, flags, defaults, choices and aliases.
  - `list_plugins` and `plugin_info`: the built-in plugins of the build, or how to
    get them.
  - `install_driver`: downloads a driver as `sbrs install` does and returns its path
    and version. Not read-only, destructive (it replaces a driver of another
    version) and open-world, which a new `Effect::Fetch` says in the annotations.
    The downloader is a parameter (`mcp::dev::host_with_installer`), so tests
    install without the network.
  - `explain_error`: rebuilds a `SeleniumBaseError` from its text or category and
    returns its hint, whether it is transient, the `sbrs` exit status, the next steps
    and the sections of the common-problems page that apply.
  - `check_config`: the settings file and the `SB_*` variables as `sbrs` reads them,
    every problem at once, a redacted summary of what would be launched, and the
    plugin selection. It starts nothing.
  - `check_fingerprint`: parses a fingerprint or an anti-detect-browser profile
    payload and returns the coherence report of `Fingerprint::validate`, the same
    report `validate_fingerprint` gives.
  - `diagnose_environment`, the tool `cdp` and `sb` have, shared.
- `Settings::check` and `Settings::check_with` read the settings like
  `Settings::load_from` but return every problem (`SettingsCheck`,
  `SettingsProblem`), not only the first, and name the variable of each.
- `diagnose_environment` reports the driver cache folder and what is in it, the cargo
  features the build has and lacks, and judges an output folder that does not exist
  yet by the nearest folder above it.

### Changed

- **Mobile mode is a whole phone.** `with_mobile(true)` without metrics now sends the
  `mobileEmulation` capability with the default screen, which is 390 by 715 at a ratio of 3
  (it was 411 by 731, and only a window size and a user agent). The undetected mode drops the
  capability, which a page can see, and sends the same phone over DevTools to every window
  (`BrowserSession::apply_mobile_emulation`: device metrics, five touch points and the user
  agent). Firefox, Safari and Internet Explorer log a warning and start as desktop browsers.
  `MOBILE_USER_AGENT` is public in `browser::options`. The pure CDP engine is not changed
  here.
- **`headless1` runs `chrome-headless-shell`** for Chrome with no program of its own named
  (downloaded when missing); the undetected mode refuses it as it refuses the shell.
  `chrome-headless-shell` is always headless. `Chrome`'s locale is written `ar-EG` for
  `--lang` and the `intl.accept_languages` preference, whichever way (`ar_eg`) it was given.
  A virtual display smaller than 1024 by 768 is raised to that size, as Python does.
- **`{config:?}` hides credentials.** The `Debug` output of `BrowserConfig` leaves the
  password out of the proxy, the PAC URL and the WebDriver server URL.


- The `driver` MCP server has eight debugging tools it did not have, the same
  definitions as on `sb`: `inspect_page`, `suggest_selectors`, `diagnose_selector`,
  `get_console_messages`, `capture_debug_bundle`, `get_page_info`,
  `export_session_as_test` and `diagnose_environment`. It is 34 tools now.
  `get_failed_requests` and `audit_page` stay with `cdp` and `sb`.
- `Profile::ALL` has a fourth profile, and `sbrs-mcp --server` accepts `dev`.
- `find_api` no longer returns a `deprecated` field: no method can carry one now.

### Removed

Every deprecated item is gone, with no shim, and nothing in `src/` is marked
`#[deprecated]` (`tests/no_deprecated.rs` keeps it so). Each one had a replacement since
0.5.0, or 0.1.0 for the `common` modules.

- `common::decorators` (`retry_on_exception`, `retry_on_exception_async`, `rate_limited`)
  and `common::exceptions` (`Exception`): use `utilities::retry::{RetryPolicy, RateLimiter}`
  and `SeleniumBaseError`.
- `StealthFlags`: use `StealthPolicy`. `StealthPolicy::{mask_all, all_custom}`: use
  `StealthPolicy::max`. `evasions::cdp_overrides`: use `safe_cdp_overrides`, which leaves out
  the `Proxy-Authorization` header.
- `profile_payloads::{Flags, Fingerprint, ProxyConfig}`: use `ProfileFlags`,
  `ProfileFingerprint` and `ProfileProxy`.
- `PlaywrightSession::&#123;launch_headless, launch_with_fingerprint,
  launch_headless_with_fingerprint&#125;`: use `launch_with_config` with a `BrowserConfig`.
- `run_browser_test_boxed`, `Plugins::run_browser_test_boxed` and `BrowserTestFuture`: pass
  an async closure, `async |sb| { .. }`, to `run_browser_test`.
- `RunSettings::{with_fast_mode, pause, message_seconds}` and `DEFAULT_MESSAGE_SECONDS`:
  `--fast` has no effect here, so there is no replacement; use `pause_for(PauseKind)`,
  `message_duration()` and `DEFAULT_MESSAGE_DURATION`.
- `BaseCase::&#123;cdp_type_text, cdp_click_element, save_saved_cookies, load_saved_cookies,
  qa_report_markdown&#125;`: use `cdp_insert_text`, `cdp_click`, `save_cookies`, `load_cookies`
  and `MasterQaSession::to_markdown`.
- `sb_cdp::Locator::click_at`: use `click_with_offset(x, y, false)`.
- `Settings::from_toml`: use `Settings::from_file`.
- The `assert_visible!` macro: use `sb_assert_visible!`.

### Fixed

- The recorder writes a test that runs. A page reached by a click or a key is recorded as
  `goto_if_not_url` (the first page stays `goto`), so the generated test no longer loads
  every page twice; the Rust, pure CDP and Gherkin outputs and `sbrs replay` understand it.
- `sbrs record` no longer loses what is done right after a page loads: it runs in a session
  with a DevTools connection, which installs the listeners in every page from its first
  moment, and looks at the page every 300 ms.
- An `alert`, `confirm` or `prompt` no longer ends `sbrs record` or gets dismissed by it:
  `RecorderSession::poll` tells an open dialog from a closed browser, and
  `RecorderSession::prepare_config` leaves dialogs open for the person.
- A click on the label of a checkbox is recorded once, not twice (the replay used to
  toggle the box twice); a click on a `<select>` is no longer recorded before its change.
- Enter in a field is recorded with the text (`type_text("#q", "text\n")`), so the replay
  sends the form again, instead of a typing and a `goto` of the page the form led to.
- The recorder's selectors find the element and nothing else: the id, then `data-testid`,
  `name`, `aria-label`, `placeholder` and the like, then the shortest path up from the
  element that is unique. A path cut after six levels used to fit other elements too.
- A password typed while recording is no longer written to the file in clear text: it is
  `{{SB_RECORDED_SECRET}}`, the generated test and `sbrs replay` read `SB_RECORDED_SECRET`
  from the environment, and `SB_REC_KEEP_SECRETS=true` keeps the typed text.
- `sbrs export-recording ACTIONS.json [-o TEST.rs]` writes a Rust test from an action file
  (it used to start a browser and export an empty recording). `sbrs record URL -o test.rs`
  and `sbrs mkrec --file test.rs --url URL` write the Rust test of what was done directly.

- zsh completion reaches the second level: `sbrs pl<Tab>` now gives `sbrs plugin ` with
  a space, so the next Tab offers `info` and `list`, and a completed flag such as
  `--url` gets its space too. usage 6.12's zsh script told zsh to add no suffix to any
  word; `sbrs completions zsh` now adds complete words with zsh's own space and keeps no
  suffix only for words that go on (`--flag=`, a folder `/`, `key:`). Shells installed
  from Homebrew pick it up with the next release.
- PowerShell completion after a space offers the next level: `sbrs plugin <Tab>` gives
  `info` and `list` instead of `plugin` again. usage 6.12's script cut the line at the
  command's text, which leaves out the trailing space; `sbrs completions powershell` pads
  it instead.

</Update>

<Update label="2026-10-17" description="v0.5.1" tags={["New", "Fix"]}>

**Downloads:** [Release notes](https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.5.1) · [Source (.zip)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.5.1.zip) · [Source (.tar.gz)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.5.1.tar.gz)

A patch release of the command line: it refuses again what clap refused in 0.4.0,
and `sbrs-mcp` names itself.

### Changed

- `sbrs-mcp` names itself: `sbrs-mcp --version` prints `sbrs-mcp 0.5.1`, and its help
  and the usage line of an error say `sbrs-mcp`, where they said `seleniumbase-rs`.

### Fixed

- A flag given twice after a subcommand, `sbrs wait-for-text --css a --css b`, is an
  error with status 2 again (`the argument '--css' cannot be used multiple times`), as it
  was with clap. 0.5.0 took the last value. A flag that is a list, such as
  `deferred --assert`, still repeats.
- A value written after a switch, `sbrs --headless=false open ...` or `--help=1`, is an
  error with status 2 again, as it was with clap. 0.5.0 accepted `true` and `false`
  there and ignored them.
- `sbrs help nosuch`, and `help` with any path that names no command, is an error with
  status 2 and an `unrecognized subcommand` message again. 0.5.0 printed the help of the
  last command it found, with status 0.

</Update>

<Update label="2026-10-17" description="v0.5.0" tags={["New", "Fix", "Breaking"]}>

**Downloads:** [Release notes](https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.5.0) · [Source (.zip)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.5.0.zip) · [Source (.tar.gz)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.5.0.tar.gz)

A release of API clean-ups and new structure. The fingerprint API is one
`Spoof` per setting with a separate `StealthPolicy`, built once and never
mutated. `BaseCase` text assertions and waits take their arguments in Python's
order, and the run settings, runner, CDP page and behave APIs are cleaned up.
The command line moved to its own workspace crate, `seleniumbase-rs-cli`, and
parses with `usage` instead of clap. The built-in plugins can be chosen by name
from features, the settings file, `SB_PLUGINS` or `sbrs --plugin`. PDF text
extraction is the opt-in feature `pdf`. The BREAKING changes, those that break a
call, are listed first.

### Breaking

- **`sbrs` and `sbrs-mcp` are built by the new workspace package
  `seleniumbase-rs-cli` (`crates/cli`), not by `seleniumbase-rs`.** The library
  stays at the repository root with the same name and path, so
  `seleniumbase-rs = { git = ... }` keeps working, but it no longer has a binary
  and no longer compiles clap, `clap_complete` or `clap_complete_fig` (15 crates
  fewer in a default build). The commands, flags, aliases, defaults and exit
  statuses of `sbrs` are the same (its parser changed, see *Changed*), with one
  difference: a plain `cargo build`, `cargo install` or `cargo run` now includes
  `pdf`, so `sbrs pdf-text` and `sbrs assert-pdf-text` work where they used to exit
  with status 2.

  ```bash
  # before
  cargo install --path . --bin sbrs
  cargo install --path . --bin sbrs-mcp --features mcp-server
  cargo install --git https://github.com/MustCodeAl/seleniumbase-rs --features mcp-server
  # after
  cargo install --path crates/cli
  cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli
  cargo binstall --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli
  ```

  The new package has `pdf` and `mcp-server` on by default (the release archives
  always had them), and `plugins-builtin` (the built-in browser plugins, see
  *Added*), and forwards `tui`, `gui`, `turso`, `test-util`,
  `full-tracing` and `tokio-console` to the library, so
  `cargo install ... seleniumbase-rs-cli --features tui` is the way to add the
  terminal UI. `default-members` names both packages, so `cargo build`,
  `cargo test` and `cargo run --bin sbrs` at the repository root work as before;
  use `-p seleniumbase-rs` for the library alone. The release archives keep their
  names (`seleniumbase-rs-v<version>-<target>`). The messages `sbrs` prints for a
  build without `tui` or `pdf` now name the package that builds it
  (`cargo install --git https://github.com/MustCodeAl/seleniumbase-rs
  seleniumbase-rs-cli --features tui`), instead of `seleniumbase-rs`, which no
  longer has a binary. The Homebrew formula has to build `--path crates/cli` from
  the first release that has this change.

- **PDF text extraction is the opt-in feature `pdf`.** `pdf-extract` (lopdf, fonts,
  ciphers) was 34 of the crates of a default build. `BaseCase::get_pdf_text`,
  `assert_pdf_text` and the module `api::pdf` need it; printing a page to PDF
  (`print_to_pdf`, `save_as_pdf`) does not. `sbrs pdf-text` and
  `assert-pdf-text` stay in `--help` and, in a build without the feature, exit
  with status 2 and say how to reinstall it with the feature
  (`cargo install --git https://github.com/MustCodeAl/seleniumbase-rs
  seleniumbase-rs-cli --features pdf`).

- **Text assertions take the text first and the selector second**, as Python's
  `assert_text(text, selector)` does. A call with the old order still compiles
  (both arguments are `&str`), so search for these names: `assert_text`,
  `assert_exact_text`, `assert_exact_text_not_visible`, `assert_shadow_text`,
  `RecorderSession::assert_text`, `AssertionApi::assert_text` and the macro
  `sb_assert_text!`. A failing assertion whose arguments look swapped says so in
  its message.

  ```rust
  // before
  sb.assert_text("h1", "Example Domain").await?;
  sb_assert_text!(sb, "#status", "Saved")?;
  // after
  sb.assert_text("Example Domain", "h1").await?;
  sb_assert_text!(sb, "Saved", "#status")?;
  ```

  The generators follow: the recorder, the scenario runner, `sbrs mkdir`,
  `sbrs mkfile`, the MCP export tools and the Python importer now write the new
  order. `RecorderSession::assert_text(sb, text, css)` and the
  `AssertionApi::assert_text(expected, css)` trait method changed with it. If
  you generated tests with an earlier version, run the same search over them.

- **The text waits take `(text, css, timeout_secs)`**, as Python's
  `wait_for_text(text, selector, timeout)` does. `wait_for_text`,
  `wait_for_exact_text_visible` and `wait_for_exact_text_not_visible` swapped
  their first two arguments (a silent change: both are `&str`).
  `wait_for_text_visible`, `wait_for_text_not_visible` and `wait_for_exact_text`
  gained the selector they lacked, so an old call stops compiling. Pass `"body"`
  to look at the whole page, as the old `wait_for_text_visible(text, timeout)`
  did.

  ```rust
  // before
  sb.wait_for_text("#status", "Saved", 10).await?;
  sb.wait_for_text_visible("Saved", 10).await?;
  sb.wait_for_exact_text("Saved", 10).await?;
  // after
  sb.wait_for_text("Saved", "#status", 10).await?;
  sb.wait_for_text_visible("Saved", "body", 10).await?;
  sb.wait_for_exact_text("Saved", "#status", 10).await?;
  ```

  `wait_for_text` now waits for the text to be displayed, like
  `wait_for_text_visible` (in Python the first is an alias of the second).

- **The deferred and delayed text checks take `(text, css)`.**
  `deferred_assert_exact_text`, `delayed_assert_exact_text` and
  `delayed_assert_text` took the selector first. `delayed_assert_text` also
  passed its arguments on in the other order, so `delayed_assert_text("Welcome",
  "h1")` looked for an element called `Welcome`; all three now follow Python's
  `(text, selector)`, like `deferred_assert_text`.

  ```rust
  // before
  sb.deferred_assert_exact_text("#status", "Ready").await?;
  // after
  sb.deferred_assert_exact_text("Ready", "#status").await?;
  ```

- **`find_text`, `find_exact_text` and `find_non_empty_text` follow Python.**
  They took no selector, searched all of `<body>` for the deepest element that
  held the text, and returned `Result<Option<WebElement>>` (the `Option` was
  never `None`). They now take the selector and return `Result<WebElement>`,
  the element of that selector, as Python's `find_text(text, selector)` does.
  Pass `"body"` to look at the page.

  ```rust
  // before
  if let Some(el) = sb.find_text("Welcome").await? { /* ... */ }
  // after
  let el = sb.find_text("Welcome", "h1").await?;
  ```

- **`post_message_and_highlight(message, css)` takes the element to highlight**,
  as Python's `post_message_and_highlight(message, selector)` does. It used to
  highlight the toast it had just shown. An error while highlighting is now
  returned instead of ignored.

  ```rust
  // before
  sb.post_message_and_highlight("Saved").await?;
  // after
  sb.post_message_and_highlight("Saved", "#save-button").await?;
  ```

- **The extra browsers are named by a `DriverId`, and switching to the default
  one is repeatable.** `get_new_driver(Option<BrowserConfig>)` returned the
  index of the browser it had just parked, and `switch_to_driver(index)` swapped
  the active browser with the one in that slot, so the slot held whichever
  browser was left last: `switch_to_default_driver()` (slot 0) reached the
  first browser only once, and its second call went back to the other. A
  `DriverId` now names a browser for good, and `switch_to_default_driver()`
  always reaches the first one. Changes:

  - `get_new_driver()` takes no argument and `get_new_driver_with(config)` takes
    the configuration (no more `None`); both return the new browser's
    `DriverId`.
  - `switch_to_driver(DriverId)` makes that browser active and can be repeated;
    `active_driver()` says which one is.
  - `quit_extra_driver(DriverId)` refuses `DriverId::DEFAULT` (that browser is
    closed by `quit()`); quitting the active extra browser falls back to the
    default one, as in Python. A browser that was quit is an error to switch
    to, not a disconnected session that panics.
  - `quit()` closes every browser the case opened, not only the active one.
  - `driver_count()` counts the browsers that are open, the active one
    included (it counted the parked ones, quit placeholders too).

  ```rust
  // before
  let first = sb.get_new_driver(None).await?; // the index of the parked browser
  sb.switch_to_driver(first).await?;          // swaps: the slot now holds the other
  // after
  let second = sb.get_new_driver().await?;    // the new browser's id
  sb.switch_to_default_driver().await?;       // always the first browser
  sb.switch_to_driver(second).await?;
  ```

- **`wait(seconds)` takes whole or fractional seconds, and no `u64`.** It is
  `sleep`, as in Python, and takes `impl Into<f64>`: `wait(2)` and `wait(0.5)`
  compile, but a `u64` variable (or a `usize`, an `i64`) no longer does.

  ```rust
  // before
  let seconds: u64 = 3;
  sb.wait(seconds).await;
  // after
  sb.wait(seconds as f64).await; // or keep the variable a u32 or an f64
  ```

- **`check_window`'s level is a `u8`**, not an `f64`: `check_window("home", 3)`
  instead of `check_window("home", 3.0)`; the same for `deferred_check_window`
  and `delayed_check_window`. A float literal stops compiling. As before, a
  level other than 0, 1, 2 or 3 is an `InvalidConfig` error.

  ```rust
  // before
  sb.check_window("home", 3.0).await?;
  // after
  sb.check_window("home", 3).await?;
  ```

- **The cookie helpers take a name and use `saved_cookies/`, as Python's do.**
  `save_cookies(path)` and `load_cookies(path)` took any file path, so a call
  written like Python's (`save_cookies("login")`) wrote a file called `login`
  in the working directory. They now take a name: the file is
  `saved_cookies/<name>.txt` in the working directory (only the last part of
  the name is used, `.txt` is added, a name ending with `/` is refused), still
  JSON. `save_cookies` returns the path it wrote. `get_saved_cookies(name)` is a
  method (it was an associated function that took a path), and
  `delete_saved_cookies(name)` deletes that file (it looked in `latest_logs` and
  the working directory). `load_cookies` now does what Python's does: a cookie
  of another domain gets the page's, and the expiry is dropped, so the cookies
  last for the browser session; `load_cookies_with(name, CookieExpiry)` keeps
  the saved expiry or sets a new one.

  For a file at a path of your own there are `save_cookies_to_path(&Path)` and
  `load_cookies_from_path(&Path, CookieExpiry)`; the MCP server and
  `sbrs save-cookies` / `load-cookies` use them. The Rust-only
  `save_saved_cookies` and `load_saved_cookies` are deprecated.

  ```rust
  // before
  sb.save_cookies("cookies.json").await?;           // any path
  sb.load_cookies("cookies.json").await?;
  // after
  sb.save_cookies("login").await?;                  // saved_cookies/login.txt
  sb.load_cookies("login").await?;
  sb.save_cookies_to_path(Path::new("cookies.json")).await?;      // any path
  sb.load_cookies_from_path(Path::new("cookies.json"), CookieExpiry::Saved).await?;
  ```

- **`assert_data_in_downloaded_file(data, file)` takes the data first**, as
  Python's does, and `assert_data_in_downloaded_file` and
  `assert_downloaded_file_regex` are `async`: they wait up to the default timeout
  for the download, as Python's do, instead of failing on a file that is still
  arriving. The data is compared without the whitespace at its ends.

  ```rust
  // before
  sb.assert_data_in_downloaded_file("report.csv", "id,name")?;
  // after
  sb.assert_data_in_downloaded_file("id,name", "report.csv").await?;
  ```

- **`sb_assert_title!` and `sb_assert_url!` check the whole value**, as
  Python's `assert_title` and `assert_url` do; they checked that the title or
  URL contained the text. The old behaviour is in the new
  `sb_assert_title_contains!` and `sb_assert_url_contains!`. A test that passed
  a part of the title keeps passing only with the `_contains` macro.

  ```rust
  // before
  sb_assert_title!(sb, "Dashboard")?;       // the title contained "Dashboard"
  // after
  sb_assert_title_contains!(sb, "Dashboard")?;
  ```

- **`run_browser_test` and `Plugins::run_browser_test` take an async closure**
  (`F: AsyncFnOnce(&mut BaseCase) -> Result<()>`) instead of a closure that
  returns a boxed future. The old form `|sb| Box::pin(async move { .. })` does
  not compile against the new bound, and neither did `|sb| async move { .. }`.
  Migration: `run_browser_test(config, |sb| Box::pin(async move { body }))`
  becomes `run_browser_test(config, async |sb| { body })`, and
  `|sb| Box::pin(demo(sb, url))` becomes `async |sb| demo(sb, url).await`
  (use `async move |sb|` in a loop that moves its variables). To keep an old
  closure compiling for now, call `run_browser_test_boxed`
  (`Plugins::run_browser_test_boxed`), which is deprecated like
  `BrowserTestFuture`. `sbrs mkdir`, `sbrs mkfile` and `sbrs import-python`
  generate the new form.

- **`RuntimeConfig::log_level` is an `Option<String>`** that defaults to `None`
  (it was `"info"`), so the binaries can tell "not set" from "set to info".
  Migration: `config.log_level` becomes `config.log_level.as_deref()
  .unwrap_or("info")` where you read it, and `config.log_level = Some(..)`
  where you set it.

- **`Settings::load(path: Option<P>)` is split in two**, so nobody writes
  `Settings::load(None::<&str>)`. Migration: `Settings::load(None::<&str>)?`
  becomes `Settings::load()?`, and `Settings::load(Some(path))?` becomes
  `Settings::load_from(path)?`. `Settings::from_toml` is deprecated; use
  `from_file`, which reads TOML for a `.toml` name.

- **`Settings::from_file`, `load`, `load_from` and `from_env` now validate**
  (`Settings::validate`), so input that used to load is an `InvalidConfig` error:
  an unknown `browser` or `mode`, a window width without a height (or the
  reverse; the missing side used to default to 1920 by 1080), and a window
  dimension of zero. Migration: spell the browser (`chrome`, `firefox`, ...) and
  mode (`webdriver`, `cdp`, `uc`) as `Browser` and `DriverMode` parse them, set
  both window dimensions or neither, and call `validate()` yourself on a
  `Settings` built by hand. A blank `SB_*` variable counts as unset (`SB_PLUGINS`
  is the exception: an empty value turns every plugin off).

- **`sb_test!` binds its `$sb` to a `&mut BaseCase`** instead of an owned
  `BaseCase`, and a skipped test passes instead of panicking. Migration: a body
  that moved `sb` (`sb.quit().await`, passing it by value) now borrows it, and
  the macro closes the browser.

- **`Settings::timeout_seconds`, `window_width` and `window_height` are
  `Option`s** that default to `None`. The old defaults (30 s, 1920, 1080) were read by
  nothing. Migration: `settings.timeout_seconds` is `settings.timeout_seconds
  .unwrap_or(10)` where you used the number; assign `Some(..)` where you set it.
  `screenshot_dir` is documented as stored for your code: the library never read
  it. `Settings` also has the new public fields `webdriver_url` and `chrome_bin`,
  and `LaunchSettings` the new `run`: a struct literal that names every field
  needs `..Default::default()`.

- **`sb_cdp` tab and window creation no longer takes an `Option` URL**, which
  needed `None::<&str>` to open a blank tab. `Browser::new_page`,
  `Browser::new_window`, `BrowserContext::new_page` and the pool lease's
  `new_page` take no argument and open a blank one; `new_page_at(url)` and
  `new_window_at(url)` open one on a URL (a URL without a scheme is opened
  over HTTPS, as before).

  ```rust
  // before
  let blank = browser.new_page(None::<&str>).await?;
  let docs = browser.new_page(Some("https://example.com")).await?;
  // after
  let blank = browser.new_page().await?;
  let docs = browser.new_page_at("https://example.com").await?;
  ```

- **`sb_cdp::Locator::type_text` empties the field before it types**, as Python's
  `sb.cdp.type` does ("clears the text field first"); it used to type after
  what the field held, which is Python's `send_keys` and the new
  `Locator::send_keys`. This changes behaviour without changing the signature,
  so a call that meant to append must be renamed. The MCP `type_text` tool keeps
  its `fill_input` and `append` modes. `CdpDriver::type_text` now sends key
  events too (it inserted the text in one step), like the other `type_text`
  methods.

  ```rust
  // before: appended
  page.locator("#note").type_text(" more").await?;
  // after
  page.locator("#note").send_keys(" more").await?;
  ```

- **`behave::AsyncStepRegistry` is `Send + Sync`**: a run can be handed to
  `tokio::spawn` and the registry shared in an `Arc`. Its handlers get the
  `BaseCase` as `SharedBaseCase` (`Arc<tokio::sync::Mutex<BaseCase>>`) instead
  of `Rc<RefCell<BaseCase>>`, return an `AsyncStepFuture` (`Send`) and must be
  `Send + Sync`; `with_base` is `async`. New: `base_case()` and
  `into_base_case()`, which hands the `BaseCase` back after a run so the caller
  can `quit` it (no step needs to).

  ```rust
  // before
  registry.register_regex(r#"^I click "(.+?)"$"#, Box::new(|bc, _step, caps| {
      let selector = caps[0].clone();
      Box::pin(async move { bc.borrow_mut().click(&selector).await.map_err(|e| e.to_string()) })
  }));
  // after: lock the shared BaseCase
  registry.register_regex(r#"^I click "(.+?)"$"#, Box::new(|bc, _step, caps| {
      let selector = caps[0].clone();
      Box::pin(async move { bc.lock().await.click(&selector).await.map_err(|e| e.to_string()) })
  }));
  // before: registry.with_base(|sb| ...);  after: registry.with_base(|sb| ...).await
  ```

- **`CdpDriver::evaluate` returns the value**, as Python's `sb.cdp.evaluate` does,
  and drops a `return ` that starts the last line; it used to return the raw
  CDP result object. `CdpDriver::find` reports a selector the page rejects as
  `InvalidSelector` (it was `CdpDriver`).

  ```rust
  // before
  let title = driver.evaluate("document.title").await?["value"].clone();
  // after
  let title = driver.evaluate("document.title").await?;
  ```

- **`api::cdp_page::CdpPage` is stricter and truer to Python's `sb.cdp`** (and
  the `BaseCase::cdp_*` methods on it):
  - `evaluate` (`BaseCase::cdp_evaluate`) returns the value instead of the raw
    CDP result object `{"type": ..., "value": ...}`, drops a final `return`, and
    a script that throws is `CdpDriver` (it was `Unsupported`).
  - `click` (`cdp_click`) waits up to 7 seconds for the element, scrolls it
    into view, and fails when the browser refuses an input event; it used to
    discard those errors and click an element below the fold off screen.
    `click_with_timeout` takes another wait.
  - `get_text`, `get_attribute` and `get_property` fail with `ElementNotFound`
    for an element that is not there; `get_text` used to answer `""` and the
    others `None`. An element without the attribute still gives `None`.
  - `select_option_by_value` fails with `InvalidSelector` for a value the
    dropdown has no option for (it set the value and reported success), and
    all three `select_option_by_*` fail with `ElementNotFound` for no dropdown.
  - Only `click` waits for its element; the reads, `type_text`, `fast_type` and
    the selects look once (`wait_for_element_present` first for a late
    element). A selector the browser rejects is `InvalidSelector` in all of
    them, and `wait_for_element_present` returns a lost connection at once
    instead of polling it until the timeout and calling it `ElementNotFound`.

  ```rust
  // before: the value was inside the result object
  let title = page.evaluate("document.title").await?["value"].clone();
  // after
  let title = page.evaluate("document.title").await?;
  ```

- **`click_with_offset` and `double_click_with_offset` take `center`** and count
  from the top-left corner of the element, as Python's do (`click_with_offset(
  selector, x, y, center=False)`). They passed `(x, y)` straight to WebDriver,
  which counts from the centre of the element, while their documentation said
  "from the top-left corner", so every click was off by half the element's
  size. The extra argument stops old calls compiling; to keep the old behaviour
  (and the click it really made) pass `true`, and to get what the
  documentation promised pass `false`. They also wait for the element to be
  displayed now. The recorder writes the new form
  (`sb.click_with_offset("#a", 10, -4, false)`), and a recording of an offset
  from the centre says `x, y, center`.

  ```rust
  // before: x and y counted from the centre, whatever the docs said
  sb.click_with_offset("#canvas", 10, -4).await?;
  // after
  sb.click_with_offset("#canvas", 10, -4, true).await?;  // from the centre, as before
  sb.click_with_offset("#canvas", 10, 4, false).await?;  // from the top-left corner
  ```

- **`save_qa_report` writes the active QA session.** It took the steps and the
  title again (`save_qa_report(&steps, title, path)`) and ignored the session
  the test had built, so the caller cloned the steps out and passed the title a
  second time. `save_qa_report()` writes the session's report to the logs
  folder, `save_qa_report_to(path)` to a file of your choice, and both are an
  `InvalidConfig` error when there is no session. `qa_report_markdown` is
  deprecated for `MasterQaSession::to_markdown`.

  ```rust
  // before
  let steps = sb.qa_session().map(|s| s.steps.clone()).unwrap_or_default();
  let report = sb.save_qa_report(&steps, "Login", None::<&str>)?;
  let there = sb.save_qa_report(&steps, "Login", Some("qa.md"))?;
  // after
  let report = sb.save_qa_report()?;
  let there = sb.save_qa_report_to("qa.md")?;
  ```

- **`BaseCase::set_messenger_theme` takes Python's arguments**, a
  `MessengerSettings` (theme, location, how many messages), or a
  `MessengerTheme` or `MessengerLocation` alone, as `Page::set_messenger_theme`
  does, instead of a CSS colour. It loads Messenger first. The colour setter it
  was is now `set_messenger_color`. A call with a colour string stops compiling.

  ```rust
  // before
  sb.set_messenger_theme("#336699").await?;
  // after
  sb.set_messenger_color("#336699").await?;       // the same thing
  sb.set_messenger_theme(MessengerTheme::Air).await?; // Python's set_messenger_theme
  ```

- **`post_message_for(message, duration)` takes a `Duration`**, not a number of
  whole seconds, so a message can last 2.55 s as Python's default does. It also
  waits for the message to go now (see Changed).

  ```rust
  // before
  sb.post_message_for("Saved", 2).await?;
  // after
  sb.post_message_for("Saved", Duration::from_secs(2)).await?;
  ```

- **A setting is one `Spoof<T>`.** `Natural` is the browser's own value (the
  default), `Random` is a valid value drawn when the fingerprint is built and
  coherent with the rest (same operating system, region and locale; repeatable
  with `.seed(u64)`), `Value(T)` is exactly this (a plain value converts to it),
  and `Off` turns the API off, for geolocation, WebRTC, media devices, battery,
  Bluetooth, speech voices, network information and Client Hints only.
  `MaskingMode`, `NoiseMode`, `PopupMode`, `ProxyMaskingMode`, `StartupBehavior`
  and `CanvasNoiseMode` are removed (`CanvasNoise` has `Natural`, `Persistent`,
  `Random` and `Low`).
- **`FingerprintBuilder::build()` returns `Result<Fingerprint, FingerprintError>`.**
  It fails, naming the setting, when `Off` is given to a setting with no off
  state, when `Random` is given to one with nothing to draw (`proxy`, `oscpu`,
  `build_id`), when a proxy has no host, and when nothing coherent can be drawn
  (a random locale next to a time zone the table of places lacks).
- **A built `Fingerprint` has private fields and read accessors**
  (`fp.screen()`, `fp.user_agent()`, ...). To change one, take `fp.to_builder()`,
  change it and build again. `FingerprintBuilder` setters take
  `impl Into<Spoof<T>>`; `geolocation`, `webgl`, `webrtc`, `avail_screen` and
  `media_devices` take a tuple, and `screen_spoof` is the `Spoof` form of
  `screen(width, height)`. The value types are `ScreenSize`, `Geolocation`,
  `WebGl`, `WebRtcAddresses` and `MediaDevices`.
- **`StealthFlags` is `StealthPolicy`** (`StealthFlags` stays as a deprecated
  alias). A policy is how the claims are delivered, and it never changes a
  setting: `webrtc_policy`, `canvas_noise`, `audio_noise`, `native_spoofing`,
  `native_tostring`, `chrome_runtime`, `headless_fixes`, `grant_permissions`,
  `block_trackers`, `blocked_trackers`, `blocked_ports`, `quic`, `disable_csp`
  and `open_shadow_roots`. Presets are `default()`, `balanced()`,
  `native_only()` and `max()`. Set it with `.policy(..)`, `.with_policy(|p| ..)`
  and the shortcuts `.webrtc_policy`, `.canvas_noise`, `.audio_noise`,
  `.native_spoofing`, `.grant_permissions`, `.block_trackers`,
  `.blocked_trackers`, `.blocked_ports`, `.disable_csp` and `.quic`, in any
  order.
- Removed: `FingerprintBuilder::&#123;flags, with_flags, canvas_seed, audio_seed,
  webrtc_ips, webgl_ids, altitude, accuracy, media_device_specs&#125;` and the
  deprecated `media_devices(a, b, c)` (the new `media_devices` takes a
  `Spoof<MediaDevices>`; `media_device_counts(a, b, c)` stays); the public fields
  of `Fingerprint`; `Fingerprint::&#123;fills_defaults, geolocation_popup,
  seed_value&#125;`. `os_cpu` is now `oscpu`. `Fingerprint::seed()` is a `u64`.
- The built-in presets set their battery, network information, speech voices of
  the system and Web Bluetooth, and Safari has none of the three it lacks (`Off`).
  `Fingerprint::randomized` is the builder with the settings of a persona
  `Random`; its noise seed is the seed it was given.
- A fingerprint is serialised with `"natural"` settings left out, `"random"` and
  `"off"` as words, and the value otherwise. A fingerprint saved by an older
  version (the flat `screen_width`, the `flags` object) no longer loads: it is an
  error that names the field, not a silent loss. A battery time that is infinite
  is written as `null`.
- A fingerprint with a `Random` setting that has no `.seed(..)` takes fresh
  entropy, and a fingerprint with nothing to draw takes a seed from its user
  agent, so a preset is the same every time.
- **Profile payloads** read one set of words for every key (see the table in
  `docs/tutorials/fingerprint_stealth.md`): `natural`, `native`, `pass_through`,
  `whitelist` are `Natural`; `auto`, and a flag the payload leaves out, are the
  value the payload holds, else `Natural` (the default was `mask` for the
  navigator, localization, time zone, graphics, fonts, screen, geolocation and
  WebRTC flags, which filled in generic values nobody asked for); `mask`,
  `spoof`, `custom`, `generic`, `random`, `noise` are the held value, else a
  drawn one; `off`, `disabled`, `block`, `empty` turn geolocation, WebRTC and
  media devices off (they used to leave them natural) and are read as `Natural`
  elsewhere; an unknown word is logged and read as `Natural` (it used to
  switch the dimension on and fill it with generic values). A literal is taken
  only in the format of its key (a locale or `Accept-Language` list, an IANA
  zone, a comma-separated list of fonts, a `Mozilla/` user agent, `vendor~renderer`);
  any other text is an unknown word. An empty string in a section is a value the
  payload does not give, a navigator section without a core count no longer
  invents 8, and the payload's `core_version` is the Chromium major a drawn user
  agent claims. A profile's folder and name seed its draws.
  `geolocation_popup` is the state of the geolocation setting: `block` denies,
  `prompt` asks. `to_browser_config`, `user_agent()`, `locale()`, `proxy_string()`
  and `apply_runtime_overrides` read the built fingerprint, so `natural` and the
  off words launch with no `--user-agent`, locale, proxy, window size or position.
- The Linux preset reports an ANGLE-over-OpenGL renderer instead of Direct3D,
  and the macOS preset reports ANGLE over Metal. The presets leave the position
  `Natural`, because a position is answered without a prompt and a fresh browser
  asks. The iOS preset claims no `navigator.deviceMemory` and no Client Hints
  (Safari has neither). `validate()` warns about a time zone without a position
  only when the policy grants permissions.
- `fingerprint!(edge)` is the new Edge preset, not the Windows Chrome one.
- `profile_payloads::{Flags, Fingerprint, ProxyConfig}` are renamed
  `ProfileFlags`, `ProfileFingerprint` and `ProfileProxy` (the old names are
  deprecated aliases).
- The MCP tool `build_stealth_bootstrap` is removed: it returned the same script
  as `get_stealth_bootstrap_script`. That script, and the providers
  `list_evasion_providers` lists, are now the 27 a browser session runs
  (`session_registry`), not the 29 of `default_registry`. `build_fingerprint`
  returns the new JSON shape (`screen: {width, height}`, no `flags`).
- `Fingerprint::default()` equals `Fingerprint::builder().build()`, and JSON
  without an `os_type` takes it from the user agent. A `Value(true)` or
  `Value(false)` for Bluetooth now changes `getAvailability()` on a browser that
  has Web Bluetooth.
- **The companion repository** [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
  takes this crate from git and still uses the old API (field writes such as
  `fp.timezone = ..`, `fp.flags.*`). Its `scrape` and `mimic` crates must move to
  `to_builder()...build()?` before it follows this change; only `codec` is known
  to build against it.
- `evasions::launch_args` passes `--no-sandbox` and `--disable-setuid-sandbox`
  only on Linux, as `StealthOptions` does.

Migrating a fingerprint:

```rust
// before: the intent is spread over a value, a flag and a mutation after build
let mut fp = Fingerprint::builder()
    .screen(1920, 1080)
    .geolocation(40.0, -74.0)
    .flags(StealthFlags::balanced())
    .build();
fp.flags.screen_masking = MaskingMode::Custom;
fp.flags.native_spoofing = true;
let width = fp.screen_width; // Option<u32>

// after: each intent is given once, and a built fingerprint is read, not edited
let fp = Fingerprint::builder()
    .screen(1920, 1080)
    .geolocation((40.0, -74.0))      // or Spoof::Random, or Spoof::Off to deny
    .policy(StealthPolicy::balanced())
    .native_spoofing(true)
    .build()?;
let width = fp.screen().value().map(|s| s.width);

// to change one thing later
let wider = fp.to_builder().screen(2560, 1440).build()?;
```

| Before | After |
|---|---|
| `fp.flags.x_masking = MaskingMode::Natural` | leave the setting out, or give `Spoof::Natural` |
| `MaskingMode::Mask` with no value (a generic fill) | `Spoof::Random` (a coherent drawn value) |
| `PopupMode::Block` | `.geolocation(Spoof::Off)` |
| `.webrtc_ips(public, local)` | `.webrtc((public, local))` |
| `.webgl(vendor, renderer)` and `.webgl_ids(..)` | `.webgl((vendor, renderer))`, or `WebGl::new(..).ids(..)` |
| `.geolocation(lat, lon).altitude(a).accuracy(m)` | `.geolocation(Geolocation::new(lat, lon).altitude(a).accuracy(m))` |
| `.flags(StealthFlags::balanced())` | `.policy(StealthPolicy::balanced())` |
| `.with_flags(\|f\| f.grant_permissions = true)` | `.grant_permissions(true)` or `.with_policy(\|p\| p.grant_permissions(true))` |
| `.canvas_seed(v)`, `.seed(v)` | `.seed(v)` and `.canvas_noise(CanvasNoise::Persistent)` |
| `fp.user_agent.as_deref()` | `fp.user_agent().as_str()` |
| `fp.is_masked(Dimension::Screen)` | unchanged |

### Added

- `sb_assert_visible!`, the macro `assert_visible!` was missing the `sb_`
  prefix of.
- `PlaywrightSession::connect_over_cdp(endpoint)` attaches to a running
  Chromium (for example `sb_cdp::Browser::http_url()`) and works on the page it
  has open; `close` detaches and leaves the browser running. Python reaches
  Playwright the same way (`sb.cdp.get_endpoint_url()`).
- `run_cdp_test(options, test)` and `Plugins::run_cdp_test(name, options,
  test)` run a test on the first tab of a Pure CDP browser and always close it,
  the way `run_browser_test` does for WebDriver.
- `RuntimeConfig::from_env_lenient` (the config and the list of variables that
  did not parse), `from_vars` and `from_vars_lenient` (read the variables from
  a function instead of the process environment).
- `BrowserConfig::with_run_settings(RunSettings)` (and `run_settings()`) sets
  the demo, slow, message and timeout settings a test starts with, as Python's
  `SB(demo=, slow=, demo_sleep=, message_duration=)` does. They stay out of
  configuration files and of the key that decides which tests share a browser.
  `RunSettings::with_default_timeout` is new and `Settings::timeout_seconds`
  feeds it. A `BaseCase` from `BaseCase::new` (so the test runners and
  `sb_test!` too) starts with them and with that default timeout.
- `BrowserConfig::with_remote_webdriver_timeout(Duration)` sets
  `remote_webdriver_timeout_seconds`, which had no builder; a part of a second
  rounds up. The common `with_*` builders of `BrowserConfig` gained doc
  comments.
- `Settings::load_from(path)` and `Settings::validate`; `DriverMode` implements
  `FromStr` and `Display` (`webdriver`, `cdp`, `uc`) and has `name()`.
- `RunSettings::pause_for(PauseKind)` names what `pause(tiny: bool)` took as a
  flag: `PauseKind::Full` after an action that changed the page, `Short` (1/3.4
  of the time in demo mode) after one that did not. `message_duration()` keeps
  the milliseconds of `--message-duration` (default `DEFAULT_MESSAGE_DURATION`,
  Python's 2.55 s) and `message_duration_headless()` cuts it to 0.75 s as
  Python does in a headless browser. New getters `slow_mode()` and
  `demo_sleep()`. `pause` and `message_seconds` are deprecated now that
  `BaseCase` uses the new ones.
- `BaseCase::demo_pauses()` (feature `test-util`) lists the `PauseKind` of each
  pause demo and slow mode have made, so a test can assert that an action
  paused without timing it.
- `BaseCase::click_if_visible_with_timeout(selector, Duration)` waits up to the
  time given for the element to be displayed before it gives up without an
  error, as Python's `click_if_visible(selector, timeout=...)` does;
  `click_if_visible(selector)` still looks once, like Python's `timeout=0`.
- `BaseCase::cdp_insert_text(text)`: `Input.insertText` into whatever has focus,
  the honest name of `cdp_type_text`, which types nothing (it inserts, with no
  key events).
- `RunSettings`, `PauseKind`, `Settings` and `SettingsOverrides` are re-exported
  at the crate root, beside `RuntimeConfig`.

- `sb_cdp::Page::origin`, `locale_code`, `screen_rect` (a `ScreenRect` of the
  display) and `click_active_element`: Python's `sb.cdp.get_origin`,
  `get_locale_code`, `get_screen_rect` and `click_active_element`.
- `sb_cdp::Locator::click_with_offset(x, y, center)` (the offset from the
  top-left corner, or from the centre) and `click_if_visible()` (one look, like
  Python's default `timeout=0`; `click_if_visible_with_timeout(timeout)` waits):
  Python's `sb.cdp.click_with_offset` and `click_if_visible`.
- `behave::StepRegistry::register_step` and `AsyncStepRegistry::register_step`
  (and `try_register_step`) register a handler for the step text alone, which
  matches whatever keyword the step is written with, or none, like behave's
  `@step`; `behave::step_pattern` (the regex they build) is public. The
  built-in async browser steps also understand the wording of Python
  SeleniumBase's `--rec-behave` recorder: `Open "url"`, `Click "selector"`,
  `Type text "text" into "selector"`, `Go back`, `Go forward`, in single or
  double quotes, so a recording from Python runs here.
- `sb_cdp::Page::set_messenger_theme` takes `impl Into<MessengerSettings>`, and
  `MessengerSettings` is `From<MessengerTheme>` and `From<MessengerLocation>`:
  `page.set_messenger_theme(MessengerTheme::Air)` changes one thing and keeps
  Python's defaults for the rest. Passing `MessengerSettings` still works.
- **The built-in plugins, one cargo feature each** (ADR 005, core side of
  phase 1): `plugin-shield`, `plugin-block-resources` and `plugin-crx`, each of
  which turns on `plugins` and compiles only its own plugin crate, and
  `plugins-builtin` for all three. They come from the crate
  `seleniumbase-rs-plugins-builtin`, re-exported as `seleniumbase_rs::builtin_plugins`.
- **Choosing plugins by name, with no code.** A `[plugins]` table in the
  settings file (`enable = [...]`, and one options table per plugin) and the
  `SB_PLUGINS` variable (a comma-separated list, which replaces `enable`; an
  empty value turns every plugin off) become a strict `PluginSet` through the
  new `plugin_host` module. `LaunchOptionsBuilder::plugins_named` and
  `plugin_selection` (errors at `build()`), and `BrowserConfig::with_plugins_named`
  and `with_plugin_selection`, take the names. An unknown name is an error that
  lists the known ones and the cargo features that add the others; options for
  a plugin that is not enabled, options a plugin refuses and a name enabled
  twice are errors too. A build without the `plugins` feature refuses a
  non-empty selection when it loads the settings (`Settings::plugins`). `sbrs`
  and `sbrs-mcp` apply the table and the variable, and take command-line
  options (next entry). See `docs/plugins/selecting.md`.
- **Plugins on the command line** (ADR 005, phase 1). `sbrs` and `sbrs-mcp` have
  the built-in plugins compiled in: `plugins-builtin` is a default feature of
  `seleniumbase-rs-cli` (with `pdf` and `mcp-server`, so the release archives and
  `cargo install` have them), and `plugins`, `plugin-shield`,
  `plugin-block-resources` and `plugin-crx` pass through to the library. They
  honour the `[plugins]` table of the settings file and `SB_PLUGINS` for every
  command that starts a browser in either engine (`BaseCase`, and `sbrs devtools`
  on the pure CDP engine), which `sbrs` ignored before, and add two global
  options that go before the command and repeat:
  `--plugin NAME`, and `--plugin-opt NAME.KEY=VALUE` with a TOML value, as in
  `sbrs --plugin shield --plugin-opt 'shield.level="strict"' open URL`. The
  precedence is the command line over `SB_PLUGINS` over the file (said in
  `--help`): `--plugin` adds a name after the ones the others turn on,
  `--plugin-opt` replaces one key and keeps the plugin's other options. New
  commands `sbrs plugin list` (name, category, whether it is on, summary) and
  `sbrs plugin info NAME` (category, phase, capabilities with their risk and
  whether the host enforces them, what it needs, ordering, the options in effect
  and, if the plugin refuses them or cannot start without any, why); an unknown
  name exits with status 2 and lists the known ones. `sbrs test` passes the
  names on to the tests as `SB_PLUGINS` and refuses `--plugin-opt`, which the
  variable cannot carry. `sbrs-mcp` takes `--plugin` and `--plugin-opt` too, and
  reads the table and the variable from the folder it runs in; the operator
  chooses the plugins, and no tool argument adds one. A mistake (an unknown
  name, an option of a plugin nothing turns on, an option the plugin refuses, a
  value that is not TOML such as `shield.level=strict`) is reported with exit
  status 2 before any browser starts or the MCP server listens. A build without
  `plugins` (`--no-default-features`) fails closed with the feature to build
  with. The Docker image is built without the default features and has no
  plugins. Only the `[plugins]` table of the settings file is read by `sbrs
  devtools`, `sbrs plugin list|info` and `sbrs-mcp`, so a value of the file that
  is wrong for a browser does not stop them; a command that starts no browser
  ignores the options, even a wrong one. A `--plugin` that turns a plugin back
  on after `SB_PLUGINS` left it out keeps the options the file gave it. Every
  active plugin is logged at `INFO` with the place that turned it on, and a
  plugin that declares `launch.proxy`, `artifacts.write` or a `host.*`
  capability is refused when only the `sbase_config.toml` of the current folder
  names it: a cloned project must not grant it (name it with `--plugin` or
  `SB_PLUGINS`, or use `--config`). The library side:
  `PluginSelection::apply_flags`, `enable_once`, `prune_unused`,
  `set_enabled_keeping_options` and `to_list` (and `PluginSelection` is `Eq`),
  `Settings::load_plugin_selection` and `load_plugin_table` (the plugin part of
  the settings only), the module `cli::plugin_cmd` (`list`, `info`,
  `check_selection`, `PluginChoice`, `PluginSource`, `apply_to_config`),
  `DevtoolsRequest::plugins`, `mcp::Settings::with_plugin_selection` and
  `mcp::serve_with`. See `docs/plugins/selecting.md`.
- **The shield plugin** is documented (`docs/plugins/shield.md`): what it turns
  off, its levels, WebRTC modes and options, and what has not been verified in a
  real Chrome.
- `Browser::extension_ids()` keeps the ids Chrome gave the extensions a launch
  loaded (`Extensions.loadUnpacked`), in load order.

- `Spoof<T>` with `is_natural`, `is_random`, `is_value`, `is_off`, `is_set`,
  `value`, `as_str` and `map`, and the policy types `StealthPolicy`,
  `CanvasNoise` and `AudioNoise`.
- `Fingerprint::&#123;to_builder, is_masked, validate, randomized,
  randomized_with_major&#125;`; `Dimension` lists what can be masked and
  `Fingerprint::is_masked(Dimension)` is the one question the page-script
  providers, the launch arguments, the protocol overrides and the proxy ask.
- `FingerprintBuilder::&#123;seed, core_version, avail_screen, product_sub,
  build_id, app_version, media_device_counts&#125;` and the policy shortcuts.
  `build()` derives the language list from the locale and the OS persona and
  `navigator.platform` from a recognised user agent, only when unset.
- `Fingerprint::edge_desktop()` (`Edg/` user agent, `Microsoft Edge` Client
  Hints through `randomize::{edge_user_agent, edge_hints}`), also in the MCP
  preset list; `randomize::locale_of_timezone`.
- `evasions::session_script`, the script the pure CDP engine runs, which the
  WebDriver `Uc` and `Cdp` modes now run too.
- An `Off` API is removed from the page, as a browser without it would be:
  `getBattery`, `navigator.connection`, `navigator.bluetooth`,
  `navigator.userAgentData`, WebRTC; `Off` media devices and speech voices list
  nothing; a denied geolocation answers `PERMISSION_DENIED`.
- `CanvasNoise::{Random, Persistent, Low}` and `AudioNoise::Random` add noise.
- `Fingerprint::validate()` reports a WebGL renderer from another OS's graphics
  API and a `MobileSafari` persona with a `Chrome/` user agent as errors, and a
  locale from another country than the time zone as a warning.

### Deprecated

- `assert_visible!`: use `sb_assert_visible!`.
- `save_saved_cookies` and `load_saved_cookies`: use `save_cookies` and
  `load_cookies`.
- `PlaywrightSession::launch_headless`, `launch_with_fingerprint` and
  `launch_headless_with_fingerprint`: they are shortcuts for
  `launch_with_config(&BrowserConfig::default().with_headless(..)
  .with_fingerprint(..))`.
- `run_settings::DEFAULT_MESSAGE_SECONDS` (3): use `DEFAULT_MESSAGE_DURATION`
  (2.55 s). `RunSettings::message_seconds` now rounds `message_duration()` up,
  so the two cannot drift apart; the default is still 3. It is deprecated too,
  with `RunSettings::pause(tiny)`: use `message_duration()` and
  `pause_for(PauseKind)`.

- `sb_cdp::Locator::click_at(Point)`: use `click_with_offset(x, y, false)`,
  Python's name for it. `click_at` calls it.
- `BaseCase::cdp_type_text`: renamed `cdp_insert_text`.
- `BaseCase::qa_report_markdown`: use `MasterQaSession::to_markdown`.
- `BaseCase::cdp_click_element`: use `cdp_click`, which it now calls (see
  Fixed). `sbrs cdp-click-element` and `cdp-type-text` keep their names.

- `evasions::cdp_overrides`: use `safe_cdp_overrides`, which leaves out the
  `Proxy-Authorization` header that would hand the proxy's login to every site.
- `StealthFlags` (use `StealthPolicy`), `StealthPolicy::{mask_all, all_custom}`
  (use `max`).

### Changed

- **`sbrs` and `sbrs-mcp` parse their command line with [usage](https://usage.jdx.dev)
  instead of clap.** `clap`, `clap_complete`, `clap_complete_fig` and `clap_usage`
  leave the workspace. The commands, flags, aliases, defaults and exit statuses are
  those of 0.4.0: both builds were run on 256 command lines, 250 act the same and the
  six that do not fall into the three groups below. `completions/sbrs.usage.kdl` is
  now written from the Rust declaration by a test that fails when the file is stale
  (`SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`
  writes it again), and so are the new page `docs/help/cli_reference.md` (a CLI
  reference rendered from the spec) and the Fig spec `docs/completions/fig-sbrs.ts`.
  What you can notice:

  - `sbrs --help` is laid out by usage: the sections are `Commands:` and `Flags:`, the
    commands are listed alphabetically instead of in the order they were declared, a
    short description keeps the final period of its doc comment, an alias is shown as
    `[aliases: new]`, and the usage line reads `Usage: sbrs [FLAGS] <SUBCOMMAND>`. The
    banner still comes first.
  - `sbrs completions bash|zsh|fish|elvish|powershell` print a short script that asks
    `sbrs` for the candidates when the completion key is pressed (so `sbrs` has to be
    on `PATH`), instead of a script that lists every command. A path argument
    completes file names, as before. `completions fig` is written by
    `crates/cli/src/fig.rs` from the tables of the parser: it marks an option as
    required or repeatable only when it is, where the `clap_complete_fig` output marked
    every option that takes a value as repeatable. `completions usage` prints the spec.
  - Three groups of command lines behave differently, because usage is looser than
    clap in these places. A flag repeated after a subcommand
    (`sbrs wait-for-text --css a --css b`) takes its last value instead of ending
    with status 2; the options before the
    subcommand and those of `grid-hub`, `grid-node` and `prompt` are still refused,
    since usage can set that rule on the root and on a struct, not on the inline
    variants of the command enum. A value written after a switch, `--headless=false`
    or `--headed=true`, is accepted and means what it says (the parser would have
    ignored the value and switched the option on); any other value, such as
    `--headless=0`, is still an error. `sbrs help nosuch` prints the main help and
    exits with 0, where it ended with status 2.

- `ActionRecorder::to_gherkin` writes Python SeleniumBase's `--rec-behave`
  wording, `Open "url"`, `Click "selector"`, `Type "text" into "selector"`,
  `Go back` and `Go forward`, where it wrote `I open "url"` and the like, so a
  recorded feature runs in either implementation. An argument that holds a
  double quote is written in single quotes, as Python does (it used to be
  skipped); one with both kinds of quote is still skipped. A feature written by
  an earlier version keeps running: the `I open` steps are still registered.
  The built-in `Type` step accepts what Python's recorder really writes,
  `Type "text" into "selector"` (it understood only `Type text ...`).
- **Demo mode and slow mode follow Python's rule for how long to pause, and
  reach the calls Python's do.** `open` always paused in full, `click` always
  in full and typing always short; Python pauses after an action in full when
  the page's URL changed and for 1/3.4 of the time when it did not (slow mode:
  always in full). `BaseCase` now reads the URL before and after such an
  action, in demo mode only, and pauses by `RunSettings::pause_for`. The pauses
  and the outline now also cover `double_click`, `context_click`, the link
  clicks, `click_active_element`, `choose_file`, the selects, `add_text`,
  `send_keys`, `set_value`, `submit`, `focus`, `go_back`, `go_forward`, the JS
  and jQuery clicks, `drag_and_drop`, the offset clicks and the window size and
  position calls; `press_keys` pauses in full after a typed Enter. A demo or
  slow run is longer for it: leave them off unless a person watches.
- **`post_message`, `post_message_for`, `post_success_message` and
  `post_error_message` last the run's message duration to the millisecond**
  (2.55 s by default, not 3 s), at most 0.75 s in a headless browser, and **wait
  for the message to go and 0.15 s more**, as Python's default `pause=True`
  does; they returned at once. They share one box: `post_message_for` no longer
  draws its own. `post_message_and_highlight` still does not wait.
- **`check_window` baselines can be told apart by test.** The baseline folder
  was named after the thread's last path part, so tests of the same function
  name in different modules or files shared one folder, and a body run by
  `#[tokio::main]` or `tokio::spawn` was filed under `main` or
  `tokio-runtime-worker`. `BaseCase::with_test_name("module::test")` (builder,
  with `test_name()` to read it back) names the test, and the baselines go in
  `visual_baseline/module__test/<name>`. `sb_test!` sets it from the module path
  and the function, so its tests get a new folder: the first run saves a
  baseline there again. Without a test name nothing changes.
- **The element assertions and waits mean what Python's mean.** `assert_element`,
  `assert_element_visible`, `assert_elements` and `wait_for_element` need the
  element to be displayed and wait for it (`wait_for_element` for its own
  timeout, the others for the default one); they used to look once for an
  element in the DOM, hidden or not. A test that asserted a hidden element (a hidden input, a
  `display: none` template) with `assert_element` must use
  `assert_element_present`, which now waits as well, like
  `assert_elements_present`. `wait_for_query_selector` is
  `wait_for_element_present`, as in Python, and `wait_for_element_present`
  takes `&self`. `AssertionApi::assert_element` is visible-and-waiting for the
  pure CDP `Page` too.
- **`sb_test!` counts a skipped test as passed.** A body that ends with
  `skip_test` used to panic with `test failed`, and now logs the reason and
  passes, like `run_browser_test` (libtest cannot report a skip). A test that
  relied on the panic must fail with an error instead.
- `load_cookies` waits for the page to finish loading first, as Python's does,
  and the domain it gives a foreign cookie is the page's host without its port
  (a browser driver refuses a cookie domain with a port).
- `assert_shadow_text(text, selector)`, the text waits and the deferred checks
  changed their argument order silently (both are `&str`). Only `assert_text`
  and `assert_exact_text` say "arguments look swapped" when they fail; search
  your tests for the other names listed above.
- `add_text` waits for the element to be displayed (it used to fail on an
  element that was not there yet), `send_keys` is `add_text`, and `update_text`
  is `type_text`, as in Python, where they are the same methods.
- `wait` is `sleep`, as in Python (the two differed: `wait` took whole seconds
  and skipped what `sleep` does); see the breaking change above for its new
  argument.
- The getters and assertions that only read the page take `&self` instead of
  `&mut self`: `get_title`, `get_current_url`, `get_text`, `assert_title`,
  `assert_title_contains`, `assert_url`, `assert_url_contains`, `assert_text`,
  `assert_exact_text`, `assert_text_visible`, `assert_text_not_visible`,
  `assert_exact_text_not_visible` and `assert_element_visible`, so a test can
  call them from a closure that holds only `&BaseCase`. Existing calls compile
  as before.
- **The text assertions wait, as Python's do.** `assert_text`,
  `assert_exact_text`, `assert_text_visible`, `assert_text_not_visible` and
  `assert_exact_text_not_visible` poll for up to the default timeout
  (`set_timeout`) instead of reading the page once, so a page that is still
  rendering no longer fails them. The first three also require the element to
  be displayed, and `assert_exact_text` and `is_exact_text_visible` ignore
  whitespace at the ends of both strings. A failing assertion now arrives after
  the timeout rather than at once: lower it with `set_timeout` where a test
  expects a failure. `assert_text_visible` is `assert_text`, as in Python. The
  deferred checks wait 2 seconds, whatever the default is.
- `convert_xpath_to_css` handles more XPath: paths (`//a/b` becomes `a > b`,
  `//a//b` becomes `a b`, `/html/body/...`), a position before the other tests
  (`tag[n]` becomes `tag:nth-of-type(n)`, `*[n]` becomes `*:nth-child(n)`),
  `contains(@a,'v')`, `starts-with(@a,'v')` (an addition; Python has none),
  `id('x')` and `or` (a comma list). A form with no exact CSS equivalent (a
  position after another test, `text()`, `parent::`) is still refused instead
  of being converted into something close.
- The Python importer passes `By.XPATH` locators through as XPath, so XPath
  that CSS cannot say is no longer refused.
- The Python importer matches the arguments of a call with Python's parameters:
  `self.assert_text("x")` gets Python's default selector `body`, and keyword
  arguments (`selector=`, `text=`, `timeout=`) work. It imports
  `assert_text_visible`, `assert_text_not_visible`,
  `assert_exact_text_not_visible`, `wait_for_text`, `wait_for_exact_text`, the
  deferred and delayed asserts, `process_deferred_asserts` and
  `enter_mfa_code`, which it used to leave as TODOs. An argument Rust cannot
  carry over (`timeout=` on an assertion, a `by` that is not CSS) is an error
  instead of being dropped. `send_keys` becomes `add_text`, not `type_text`:
  Python's and Selenium's `send_keys` do not clear the field. The timeout of
  `wait_for_element_visible` and `wait_for_element_present` is read from
  `timeout=`; a second positional argument is Python's `by`, not a timeout.
- **Both dependencies on the plugins repository are pinned to one revision**
  (`628991a`): `seleniumbase-rs-plugin` and `seleniumbase-rs-plugins-builtin`
  must name the same source, or Cargo builds two plugin API crates with
  unrelated `Plugin` traits. `tests/plugin_features.rs` fails when they differ.
  A plugin crate added with `cargo add --git` now needs the same `--rev`; the
  plugins guide says so.
- **Plugin settings reach the browser.** `LaunchSettings::host_resolver_rules`
  becomes one `--host-resolver-rules` switch, `disable_features` and
  `enable_features` one `--disable-features` and one `--enable-features` switch
  (the first merged with the `Translate` the library turns off), `webrtc_policy`
  `--force-webrtc-ip-handling-policy`, and `disable_quic` `--disable-quic`, on
  the pure CDP engine and through chromedriver. They were dropped before. The
  profile is shown to plugins before they run (persistent when `user_data_dir`
  is set), so `shield`'s `ephemeral_profile` can refuse a persistent one. A
  plugin that sets one of these fields for Firefox or Safari is refused.
- **Plugins are checked against the engine that hosts them**
  (`PluginSet::check_with`): the pure CDP engine supports every capability,
  WebDriver with Chromium, Firefox or Safari the ones its protocol allows, and a
  WebDriver server that answers `browserName: tauri` is re-checked as an
  embedded webview once the session exists. A plugin that declares no metadata
  keeps running on Firefox and Safari as before.
- **A plugin's raw `Fetch.*` command is refused**, on the browser and on a tab,
  in both engines: the host owns request interception, and the library's own
  proxy login uses it. A plugin that blocks URLs sends `Network.setBlockedURLs`.
- `SeleniumBaseError::Plugin` names the plugin for the `Unsupported`,
  `Capability`, `Undeclared`, `Conflict` and `Sealed` errors of the plugin API,
  and its hint explains them.

### Fixed

- `BaseCase::get_locale_code` made up `en-US` when the browser gave no
  language; Python returns what `navigator.language || navigator.languages[0]`
  answers, and so does this now (the empty string for no locale).
- `BaseCase::cdp_click_element` pressed at WebDriver's rectangle of the
  element, which is in the document, but the DevTools mouse event takes
  viewport coordinates, so on a page scrolled by N pixels the click landed N
  pixels too low (or on another element). It is now `cdp_click`: it scrolls the
  element into view and presses at its centre in the viewport. The
  documentation of `cdp_mouse_click` said "screen coordinates"; they are
  viewport coordinates. Behaviour change: like `cdp_click`, it now takes a CSS
  selector only (it went through WebDriver and also took XPath, `link=` and
  the like), and a missing element is waited for before it fails. Use `click`
  for the other selector forms. A recording writes the click as `cdp_click`,
  so generated programs do not call the deprecated method.
- `BaseCase::drag_and_drop` outlines the drop selector too in demo mode, as
  Python does; it outlined only the drag selector.
- `BaseCase::click_with_offset` and `double_click_with_offset` scroll the
  element into view before they measure it, as Python does, so an offset on an
  element below the fold lands in the right place.
- `BrowserConfig::with_run_settings` documents that it replaces the whole
  `RunSettings`, the default wait of `Settings::to_browser_config` included.
- `RunSettings` demo and slow mode pause for 0.5 s after an action, as Python
  4.55.2 does (`DEFAULT_DEMO_MODE_TIMEOUT`); `DEFAULT_DEMO_PAUSE` was 1.2 s.
  Pass `with_demo_sleep` to keep a longer pause.
- `RunSettings::with_fast_mode` (`--fast`) no longer cancels demo and slow
  mode. Python reads `--fast` only in CDP mode, where it skips short waits this
  crate never makes, so the method is deprecated and does nothing. Behaviour
  change: a run with demo or slow mode and `--fast` now pauses; drop the
  `with_fast_mode(true)` call to keep the old effect of "no pauses" by turning
  the mode off instead.
- `Settings::to_browser_config` honours `SB_WEBDRIVER_URL` and `SB_CHROME_BIN`
  (the new fields `Settings::webdriver_url` and `chrome_bin`). Before, they
  reached only `BrowserConfig::from_env` and `sbrs`, which patched them in by
  hand, so the `sbrs mkdir` test helper ignored them. `SB_WINDOW_WIDTH` and
  `SB_WINDOW_HEIGHT` now size the browser window (when both are set).
- An unknown browser or driver mode is an error. `SB_BROWSER=chrom` or
  `mode = "stealth"` used to give Chrome in WebDriver mode without a word;
  `Settings::from_file`, `load`, `load_from` and `from_env` now call the new
  `Settings::validate`, which also refuses a window width without a height.
- `sbrs` and `sbrs-mcp` fall back to `RUST_LOG` when `SB_LOG_LEVEL` is unset,
  blank or invalid (then to `info`); `RUST_LOG` used to be ignored, and the
  docs said both. One bad `SB_*` runtime variable no longer drops the good
  ones: both binaries now use `RuntimeConfig::from_env_lenient`, so a typo in
  `SB_SHUTDOWN_TIMEOUT_SECS` keeps `SB_LOG_LEVEL=debug` and is logged as a
  warning after logging starts.

- A test that panics no longer leaves its browser open. `run_browser_test`,
  `Plugins::run_browser_test` and `sb_test!` now share one lifecycle that
  catches the panic, closes the browser, tells the plugins the test failed and
  then raises the panic again. `sb_test!` used to panic on a skipped test and
  to hide the test's error when `quit` also failed; it now behaves like
  `run_browser_test`.

- `PlaywrightSession` (feature `playwright`) sizes the window from
  `BrowserConfig::with_window_size` instead of always starting maximised at
  1920 by 1080. Without a configured size it behaves as before.

- `BaseCase::activate_playwright_mode` (feature `playwright`) starts its
  Chromium as the case's `BrowserConfig` says (headless or not, proxy, user
  agent, locale, fingerprint, binary path, window size), where it always
  started a default headed one and ignored the configuration; the profile
  folder is left out, since the WebDriver browser holds it. `quit()` now closes
  the Playwright session too; it was left running. Behaviour change: a headless
  configuration now gives a headless Playwright browser, and a test that called
  `session.close()` itself before `quit()` should leave that to `quit()`.

- `sb_cdp::Locator` reads (`text`, `html`, `attribute`, `info`, `bounding_box`,
  `viewport_rect`) reported every failure as `ElementNotFound`. Only a timeout
  does now. An invalid CSS or XPath selector is `InvalidSelector` (for reads
  and for actions, which used to say `CdpDriver`), and a lost connection keeps
  its own error.
- The pure CDP programs `ActionRecorder::to_cdp_script` and `to_cdp_test` write
  follow `Locator::type_text` now emptying the field (see Breaking): a recorded
  `add_text` or `send_keys` is `.send_keys(..)`, which adds to the text, where
  `.type_text(..)` would have erased it; `type_text` and `update_text` are one
  `.type_text(..)` call, not a `clear` and a `type_text`; and a
  `click_with_offset` is `.click_with_offset(x, y, false)` (`true` for a
  recording that says `x, y, center`), not the deprecated `click_at`. A
  program recorded with an earlier version keeps working but clears the field
  before an `add_text`: record it again, or rename that call to `send_keys`.

A breaking change to the fingerprint API: **one field per setting, and intent
given once.** Before, every setting was kept twice, as a value and as a mode that
said whether to use it (`fp.screen_width` and `fp.flags.screen_masking`), which
allowed states that mean nothing, and callers mutated the fingerprint after
`build()` to say what they meant. Now every setting is one `Spoof<T>`
(`Natural`, `Random`, `Value`, `Off`), the behaviour that is not a setting is a
`StealthPolicy`, and a built `Fingerprint` is never mutated. The Python-parity
methods (`parity/api.toml`) are unchanged; the fingerprint API is Rust-only. The
design is `docs/design/fingerprint-api-v2.md`.

- `enter_totp_code` and `enter_mfa_code` now press Enter after the code, as
  Python's `enter_mfa_code` does (it types the code and a newline). A test that
  clicked a submit button after them submits twice and can drop the click.
- `wait_for_exact_text` looked at every element of the page, `<title>`,
  `<script>` and hidden elements included, so it could find text that no
  visible element showed. It now looks at the selected element and requires it
  to be displayed.
- `sb_assert_not_visible!` without a message did not compile, and `sb_wait_and_click!`
  without a message dropped the error of the wait. `sb_test!` quit the browser
  before it reported the test's result, so a quit that failed hid the test's own
  error; it now reports both.
- **Pooled browsers shared the state of a plugin.** `PluginSet::for_launch()`
  now runs before every launch, attach and WebDriver session, so a plugin that
  offers `per_launch` (the CRX plugin's temporary folders, shield's origin list)
  gets a fresh instance for each browser, and `on_close` reaches the instance
  that started.
- **A plugin's `Network.setBlockedURLs` replaced the blocklist of the tab.**
  Raw sends from several plugins, and the library's own ad-block list, are now
  merged into one list for each tab (each window with chromedriver). The
  tracker hosts of a fingerprint with `block_trackers` are part of that list on
  both engines, and with chromedriver a window keeps its union when the session
  switches away and back with `ad_block` on (switching re-sent the library's
  list alone, which replaced the plugins' patterns).
- **A switch in `extra_args` replaced the one plugins built.** On the pure CDP
  engine `--disable-features`, `--enable-features` and `--host-resolver-rules`
  given in `extra_args` are now joined into the one switch (as the WebDriver
  engine already did), and the WebRTC policy of the options wins over a
  `--force-webrtc-ip-handling-policy` argument; before, Chrome kept the last
  one and dropped the merged list, shield's included.
- **`SB_PLUGINS=` did not turn plugins off** when the file had an options table
  such as `[plugins.shield]` (the build failed with stray options), and in a
  build without the `plugins` feature the file's `[plugins]` table stopped the
  load before `SB_PLUGINS` could override it. The variable now drops the
  options of the plugins it no longer names and is applied before the check
  for plugin support.
- A plugin's `Network.setRequestInterception` is refused like `Fetch.*`.

</Update>

<Update label="2026-10-16" description="v0.4.0" tags={["New", "Fix"]}>

**Downloads:** [Release notes](https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.4.0) · [Source (.zip)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.4.0.zip) · [Source (.tar.gz)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.4.0.tar.gz)

A feature release: browser plugins now run in the WebDriver engine as well as
the Pure CDP one, the Pure CDP engine gains page script and style tags and HAR
recording, the opt-in `visual` feature compares screenshots by structural
similarity, `sbrs prompt` prints a browser and driver segment for shell prompts,
and the documentation gains a guide to testing pages with a CAPTCHA.

### Added

- **Browser plugins for the WebDriver engine** (feature `plugins`; ADR 004
  phase 3). `BrowserConfig::with_plugins` gives a `PluginSet` to
  `BaseCase::new` and `BrowserSession::connect`: the set is checked against the
  browser, `before_launch` changes the start-up settings (Chromium arguments,
  capability preferences, extension folders, proxy and headless; Firefox
  arguments, preferences and headless), and once the session runs the plugins
  get `on_browser`, then the packages and `on_page_created` for the first
  window, for each window `switch_to_new_window` or `open_new_window` opens, and
  for any other on `apply_plugins_to_window`; `quit` or a drop calls
  `on_close`, and `reconnect` runs the hooks again. On Chromium browsers
  plugins send protocol commands through chromedriver's DevTools channel; on
  Firefox and Safari `send` is refused and scripts for every document run at
  once and again after each `open`. A refusal or a failing hook stops the
  session with `SeleniumBaseError::Plugin`. Plugins with `reuse_session` are
  refused. See the WebDriver section of `docs/tutorials/plugins.md`.
- `sb_cdp::Page::add_script_tag` and `add_style_tag` add a script or a
  stylesheet to the current document from a URL or inline (`TagSource`),
  waiting for a file to load; `add_init_script` runs a script in every
  document the tab loads from then on and returns an `InitScriptId` for
  `remove_init_script`. Playwright's `addScriptTag`, `addStyleTag` and
  `addInitScript`.
- With the `plugins` feature, `sb_cdp::Page` implements the plugin API's
  `PageHandle`, so a program can hand one of its tabs to plugin code.
- Visual regression testing, behind the new opt-in `visual` feature:
  `visual::VisualBaseline` saves a screenshot as a baseline PNG on the first
  run and compares later screenshots with it by structural similarity (MSSIM
  over 8 × 8 windows on each colour channel, `image-compare`), failing below a
  threshold of 0.999 with `AssertionFailed` and leaving `<name>.actual.png` and
  a `<name>.diff.png` that paints the differences red. Screenshots of another
  size fail without a comparison. `check_page` takes a `BaseCase` or an
  `sb_cdp::Page`, and `BaseCase::assert_visual_match(name)` keeps its baselines
  in `visual_baseline/<test>/`, `<test>` being the test's whole path (test
  binary, module path and function, as `cart.checkout.renders`), and follows
  `with_visual_baseline_reset`.
  `check_window` is unchanged: it compares the HTML layout, as in Python.
- HAR recording for the pure CDP engine (no new dependency, no feature):
  `Page::start_har()` / `start_har_with(HarOptions)` return a `HarRecorder`,
  and `HarRecorder::finish()` returns a `Har` (HTTP Archive 1.2 serde types)
  that `Har::save(path)` writes as JSON. Entries come from the `Network`
  events of the tab and of its cross-site iframes, which run in targets of
  their own and are attached while recording, extra-info headers included, with cookies, query strings, POST
  bodies, one entry per redirect hop, failures with Chrome's `_error`, and
  `blocked`/`dns`/`connect`/`ssl`/`send`/`wait`/`receive` timings; each
  top-level navigation is a page with `onContentLoad` and `onLoad`. Response
  bodies are off by default and capped at 1 MiB each when turned on; the cap
  can be raised to 64 MiB (`MAX_CONTENT_BYTES`).
- `sbrs prompt` prints one short line for a shell prompt: the installed
  Chrome's major version and whether a matching chromedriver is at hand
  (`chrome 155 ✓driver`, `✗driver`, `?driver`), with `--ascii` marks and a
  `--format` of `{browser}`, `{major}`, `{driver}`, `{mark}` and
  `{driver_major}`. It starts no browser and uses no network: versions come
  from files (a macOS app's `Info.plist`, the version folder beside
  `chrome.exe`), and one that only `--version` tells is cached for ten minutes
  in the user cache folder. It prints nothing and exits with 0 when no browser
  is found (`--verbose` says why). The CLI page has setups for Bash, Zsh,
  Fish, PowerShell, Starship and Warp, and `.warp/workflows/` a Workflow for it.
- `browser::driver_version::major_without_running` reads a program's major
  version from its files only, and `version_flag_major` runs `--version` with a
  time limit.
- Tutorial [Testing Pages with a CAPTCHA](/tutorials/captcha_testing): vendor test keys, and tests that click the test widget and read the token.
- `docs/completions/`: a Fig completion spec for `sbrs`, prepared for withfig/autocomplete (not submitted), with the steps.

### Fixed

- The pure CDP engine takes DevTools messages up to 256 MiB, as Puppeteer
  does. Chrome sends each answer in one WebSocket frame, and one over the
  default 16 MiB, such as a large screenshot or response body, used to close
  the connection for every tab.
- `sbrs` exits quietly with status 0 when the reader of its output goes away
  (`sbrs completions zsh | head`), instead of printing "error: Broken pipe".
- Canvas noise ran only for one mode; `Persistent`, `Random` and `Low` switched
  it off.
- WebDriver sessions in the `Uc` and `Cdp` modes ran the 29-provider script of a
  browser without `navigator.plugins` (replaced getters a real Chrome does not
  have), while the pure CDP engine ran the 27-provider one.
- An allowed geolocation popup did not grant the geolocation permission, so the
  position override waited on a prompt nobody presses. The ports a fingerprint
  lists (`blocked_ports`) were stored and never blocked; they are now refused on
  `localhost`, `127.0.0.1` and `[::1]` through the same `Network.setBlockedURLs`
  command as the tracker list. The Modernizr hairline fix followed
  `graphics_noise` and was switched off by `Low`, `Medium` and `High`; it
  follows the headless fixes.
- Ad blocking in the `Cdp` and `Uc` modes replaced the fingerprint's tracker and
  port block, because a second `Network.setBlockedURLs` replaces the first; the
  command now carries both.
- `--user-agent`, `--lang`, `--window-size` and `--proxy-server` were sent twice,
  from the options and from the fingerprint; each is sent once, with the value
  that was already in effect.
- The `Accept-Language` of the user agent override was sent (with an invented
  `en-US,en;q=0.9`) whenever a user agent was set; it is sent only when the
  localization is on.
- The Tauri profile manager example applies a fingerprint with
  `Page::apply_fingerprint` instead of its own copy, which used the unsafe
  `cdp_overrides` and the 29-provider script. Its custom masking form reads and
  writes the new fingerprint JSON (a screen is one `screen` object, the selects say
  what to do with the settings left empty), and profiles saved in the old flat
  shape no longer load.

</Update>

<Update label="2026-10-15" description="v0.3.0" tags={["New", "Fix", "Breaking"]}>

**Downloads:** [Release notes](https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.3.0) · [Source (.zip)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.3.0.zip) · [Source (.tar.gz)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.3.0.tar.gz)

A breaking release: the experimental stealth scraping layer leaves this crate.

### Removed

- **BREAKING: the stealth scraping layer moved to its own repository,
  [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth).**
  The features `scrape` and `impersonate`, the modules `seleniumbase_rs::wire`,
  `seleniumbase_rs::scrape` and `seleniumbase_rs::impersonate`, and their
  crate-root re-exports (`BrowserScraper`, `CookieJar`, `StealthIdentity`,
  `SessionPolicy`, `ScrapeError`, `BrowserKind`, `Emulation`, `TlsSpec`,
  `H2Spec`, `HeaderSpec`, `ImpersonationProfile`, `CustomProfileBuilder`,
  `impersonation_client`, `HttpSession`, `MitmProxy`, `StealthSession`,
  `EphemeralCa` and the rest) are gone from this crate. They are now the crates
  `seleniumbase-rs-wire` (was the module `wire`), `seleniumbase-rs-scrape` (was
  the feature `scrape`) and `seleniumbase-rs-impersonate` (was the feature
  `impersonate`), which build on the public API of this crate and stay
  experimental. Depend on them by git, for example
  `seleniumbase-rs-scrape = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }`,
  and give `seleniumbase-rs` by its plain git URL next to them so both resolve to
  one commit ([Stealth Scraping](/tutorials/stealth_scraping)).
- With the layer go the optional dependencies `psl`, `wreq`, `btls`,
  `tokio-btls`, `brotli`, `hyper`, `hyper-util` and `http-body-util`; the
  examples `capture_tls`, `capture_hints`, `capture_h2`, `cookie_jar`,
  `browser_scraper`, `custom_profile`, `http_session` and `stealth_session`; the
  tests and fixtures of the layer; the `just scrape-gate`, `imp-gate` and
  `measure` recipes and their mise tasks; the `scrape` CI matrix entries and the
  `impersonate` CI job; and CMake, NASM and LLVM from the build requirements.
- `futures-util` without its default macros (`select!`/`join!`, unused), so the
  `futures-macro` proc macro is no longer compiled.
- `scraper` without its default features (`getopts` for its own CLI, unused
  parse-error text). `flate2` asks only for the `zlib-rs` backend `zip` already
  builds (`pdf-extract` still turns on its default one).
- The unused `tracing-futures` dependency (every build compiled it) and the
  unused `tracing-actix` one from the `full-tracing` feature.
- JSON logs come from `tracing-subscriber`'s own JSON format instead of the
  separate `json-subscriber` crate.
- The `playwright` feature no longer lists `rustwright-core` itself; `rustwright`
  already brings it without Python.
- HTTPS uses rustls with the `ring` crypto provider for every client, so the
  `aws-lc-sys` C library is no longer compiled. A program that builds its own
  reqwest client next to this crate calls `seleniumbase_rs::install_tls_provider()`
  first.
- reqwest no longer builds its `blocking` client: ending reused sessions at
  exit uses the async client on a small runtime of its own.
- `thirtyfour` is built without its `component` derive macros and its own
  driver `manager` (this crate downloads drivers itself), which also removes its
  older `zip` and `dirs`.
- `zip` is built with deflate only (the format of driver downloads and the
  bundled extensions) instead of every format; zstd (a C library), bzip2, lzma,
  xz, ppmd and AES no longer compile.

### Fixed

- A WebDriver URL with a path and no trailing slash (a Grid's
  `http://hub:4444/wd/hub`, or tauri-plugin-sbrs-automation's
  `http://127.0.0.1:4445/<token>`) lost its last path segment, because the
  client joins command paths as relative URLs; the slash is added when
  connecting.

### Changed

- The generated browser identity claims Chrome 154, 155 or 156 (was 153 to
  155), and its high-entropy Client Hints name a real build per major
  (`stealth::randomize::KNOWN_FULL_VERSIONS`: 155.0.8059.40 measured on macOS
  27.0.1 by `tests/client_hints_chrome.rs`, 154.0.8037.100 and 156.0.8078.35 from
  Google's version history) instead of `major.0.0.0`. Macs claim macOS 15.8.1,
  26.7.1, 27.0.0 or 27.0.1; iPhones may claim Safari 26.0 with the frozen
  `iPhone OS 18_6` (Safari 17.7 is gone). The Chrome presets, the `--mobile`
  user agent and the examples claim Chrome 156 (`PRESET_CHROMIUM_MAJOR`), the
  Android ones in Chrome's frozen `Linux; Android 10; K` form, and the iPhone
  preset is Safari 26.0. Seeds map to other identities than before.
- A fingerprint without Client Hints (the presets) gets its brand list, decoy
  brand and full versions from `chromium_hints(major)` instead of a fixed
  `Not(A:Brand` 24, the Client Hints platform name (`Windows`, not `Win32`), and
  a `fullVersionList`.

- The `sbrs` banner reads SeleniumBaseRust (figlet small font, 68 columns), and
  `sbrs --help` and `sbrs-mcp --help` say SeleniumBaseRust.
- The tests' leftover-process checks (Edge, geckodriver, the MCP stdio servers)
  list processes with `sysinfo` (a dev-dependency) instead of `pgrep` and
  `kill -0`, so they run on Windows too instead of being skipped there.
- Human-paced input draws its bell-curve values (`stealth::humanize::Rng::gaussian`,
  used by `stealth::behavior`) from `rand_distr`'s `StandardNormal` instead of a
  hand-written Box-Muller transform. The distribution is the same, but a seeded
  `Humanizer` now plans different paths and timings than before for the same
  seed. `humanize::Rng` implements `rand::TryRng` (so `rand::Rng`), and
  `rand_distr` is a new dependency.

### Added

- `sbrs devtools` launches Chrome with its DevTools endpoint open (`--port`,
  default 9222), prints the endpoint and configs for Google's
  `chrome-devtools-mcp` and Microsoft's Playwright MCP (`--json` for one line of
  JSON), and runs until Ctrl-C, which closes the browser. The Pure CDP MCP server
  has `get_devtools_endpoint`, and `Browser::devtools_endpoint()` and
  `LaunchOptionsBuilder::debugging_port` do the same from Rust. See
  `docs/tutorials/devtools_mcp.md`, which also covers Firefox's and Safari's MCP
  servers.
- `.github/workflows/release.yml` builds `sbrs` and `sbrs-mcp` for five targets
  on a version tag and attaches the archives, with completions and Warp
  workflows, to the release; `[package.metadata.binstall]` points `cargo
  binstall` at them. Not run yet (GitHub Actions is locked on the account).

- Feature `ocr` (off by default): `ocr::Ocr` reads the text a page shows from
  its screenshot (`read_page`, for a `BaseCase` or an `sb_cdp::Page`) or from PNG
  bytes (`text_in_png`), with [ocrs](https://github.com/robertknight/ocrs), pure
  Rust. Its two models (about 12 MB) are downloaded into the cache folder on
  first use and checked against pinned SHA-256 hashes; `SB_OCR_MODELS` names a
  folder of your own copies instead.
- **Browser plugins for the Pure CDP engine** (feature `plugins`, off by default;
  ADR 004 phase 2). `LaunchOptionsBuilder::plugins` takes a `PluginSet` from the
  plugin API crate
  [`seleniumbase-rs-plugin`](https://github.com/MustCodeAl/seleniumbase-rs-plugins),
  re-exported as `seleniumbase_rs::plugin_api`. `Browser::launch` checks the set,
  lets `before_launch` change the arguments, preferences, extensions, proxy and
  headless mode, and calls `on_browser`; every tab the library attaches to gets
  the plugins' packages and `on_page_created`; `Browser::close` (or the last
  drop) calls `on_close`. `Browser::connect_with_plugins` does the same for a
  running browser. A plugin that refuses or fails stops the launch or the tab
  with the new `SeleniumBaseError::Plugin`. Guide: `docs/tutorials/plugins.md`.
- `LaunchOptionsBuilder::extension` loads an unpacked extension over the
  protocol (`Extensions.loadUnpacked`, which branded Chrome 137 and later need
  instead of `--load-extension`), and `LaunchOptionsBuilder::preference` merges
  a preference into the profile's `Default/Preferences` before the browser
  starts.
- `Browser::launch_mocked` (feature `test-util`) runs the launch steps, plugins
  included, against a `MockCtrl`, which `MockCtrl::new` now creates for a test
  to script first.
- Feature `tokio-console` (off by default): `sbrs`, `sbrs-mcp` and programs that
  use the crate's logging serve [tokio-console](https://github.com/tokio-rs/console)
  for watching async tasks. Build with `RUSTFLAGS="--cfg tokio_unstable"`.

</Update>

<Update label="2026-10-12" description="v0.2.0" tags={["New"]}>

**Downloads:** [Release notes](https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.2.0) · [Source (.zip)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.2.0.zip) · [Source (.tar.gz)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.2.0.tar.gz)

### Changed

- **The binaries are renamed** so they no longer clash with Python
  SeleniumBase, which installs its own `sbase`: the CLI is now `sbrs` and the
  MCP server `sbrs-mcp` (was `seleniumbase-mcp`). Commands and flags are
  unchanged; replace the name in scripts and MCP client settings, and generate
  the shell completions again (`sbrs completions zsh > _sbrs`). The Docker
  image runs `sbrs` as user `sbrs`. `sbase_config.toml` keeps its name.

### Added

- `sbrs completions fig` writes a Fig completion spec, the format of
  withfig/autocomplete that Warp's built-in completions, inshellisense and Amazon
  Q use. Warp also uses the shell's own completions by default.
- `sbrs completions usage` prints a usage (KDL) spec for jdx/usage and mise,
  kept in `completions/sbrs.usage.kdl` and checked against the CLI by a test.
- Warp Workflows for common `sbrs` commands in `.warp/workflows/`.
- Every docs page has a title in front matter for Docs7; the mdBook build strips it.
- ADR 004 proposes a plugin framework for community crates, in the style of
  puppeteer-extra and Zorilla.

</Update>

<Update label="2026-10-12" description="v0.1.1" tags={["New", "Fix"]}>

**Downloads:** [Release notes](https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.1.1) · [Source (.zip)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.1.1.zip) · [Source (.tar.gz)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.1.1.tar.gz)

Windows fixes from a second round of runs on a Windows 11 PC (Chrome 154 and
155, Edge 155, Firefox 153). There, formatting, clippy, the whole test suite,
the real-Chrome tests, Edge, Firefox and the Tauri 3 installer build now pass.

### Fixed

- The browser version on Windows comes from the `chrome.exe` (or `msedge.exe`)
  that really runs, read from its file version: while an update waited for a
  restart, the newest version folder named a Chrome that was not running yet and
  the wrong chromedriver was fetched. The folder rule also skips a waiting update.
- Download folders are handed to Chrome as plain `C:\...` paths, not with the
  `\\?\` prefix `canonicalize` adds on Windows; a `blob:` download was cancelled.
- The tests that list processes with `pgrep` skip those checks on Windows.

### Added

- `rust-toolchain.toml`, so rustup uses Rust 1.99 in the repository by itself.
- The Windows status in `docs/platforms.md`. Known gap: on Windows nothing ends a
  browser whose program was killed (macOS and Linux have a process-group watcher).

</Update>

<Update label="2026-10-12" description="v0.1.0" tags={["New", "Fix"]}>

**Downloads:** [Release notes](https://github.com/MustCodeAl/seleniumbase-rs/releases/tag/v0.1.0) · [Source (.zip)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.1.0.zip) · [Source (.tar.gz)](https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/v0.1.0.tar.gz)

The first release, tagged `v0.1.0` on GitHub (not published to crates.io).
Windows is being verified: the first runs on a Windows PC found the problems
listed first under *Fixed*.

### Added

- **geckodriver and msedgedriver**: a Firefox session starts geckodriver and an
  Edge session msedgedriver itself, as Python SeleniumBase does, where both
  used to be handed chromedriver. Each is looked for in `downloaded_drivers/`
  and on the `PATH`, and downloaded when missing: geckodriver's newest GitHub
  release, and the msedgedriver of the installed Edge's major version from
  `msedgedriver.microsoft.com` (`LATEST_RELEASE_<major>_<OS>`); an msedgedriver
  that does not fit the installed Edge is replaced, and `with_driver_version`
  pins one. Downloads are capped at 64 MB, unpacked (zip or tar.gz) without
  writing outside their folder, and made runnable as chromedriver is. Both are
  started with the orphan guard and the macOS re-sign retry; geckodriver gets
  `--websocket-port 0` so Firefox sessions can run side by side. Firefox
  sessions now also send `browser_binary_path` as `moz:firefoxOptions.binary`,
  `user_data_dir` as `-profile`, and the user agent and locale as preferences.
  `sbase install` takes a driver and a version (`sbase install geckodriver
  0.36.0`, `sbase get edgedriver`), and `sbase get` is its alias. Checked on
  macOS with a real headless Edge 155 and the msedgedriver the crate downloaded
  (`tests/edge_msedgedriver.rs`); the crate also found and started Homebrew's
  geckodriver there, and downloaded geckodriver 0.37.1 and 0.36.0. A headless
  Firefox 153 ESR took the new `moz:firefoxOptions` through the geckodriver of
  the `docker/firefox-check` container (`tests/firefox_geckodriver.rs`); the
  crate itself ran on the Mac, not in the container.
- **Playwright and Puppeteer importers**: `sbase import-playwright` (alias
  `import-pw`) converts `@playwright/test` files, `playwright codegen` output and
  pytest-playwright, sync and async Python scripts; `sbase import-puppeteer`
  (alias `import-pptr`) converts Puppeteer scripts, jest-puppeteer and Mocha
  tests and Chrome DevTools Recorder JSON exports. Both write one
  `#[tokio::test]` per test on the pure CDP engine, keep every statement they
  do not convert as a `TODO` with a diagnostic naming its line, and read the
  input with a bounded hand-written parser whose text reaches the Rust only in
  escaped literals and comments (`utilities::importer`,
  `docs/playwright-migration.md`, `docs/puppeteer-migration.md`).
  `SelectorBuf::role`, `text`, `label` and `has_text` approximate `getByRole`,
  `getByText`, `getByLabel` and `filter({ hasText })` in XPath.
- **Pure CDP engine** (`sb_cdp`): `Browser`, `Page`, `Locator`, input, cookies,
  storage, window and emulation, retrying assertions, same-origin frames, CAPTCHA
  solving, a mock browser for tests without Chrome, request interception
  (`Page::intercept`), per-context proxies with per-session passwords, a WebRTC
  leak shield, and `Page::human` (curved pointer paths, human typing pace).
  `BrowserPool`, `Lease`, `BrowserContext` and `SessionStore` give bounded,
  isolated, recycled browsers with in-memory session sharing.
- **MCP servers**: `seleniumbase-mcp --server cdp|driver|sb` (24, 26 and 88
  tools, plus 8 stealth tools on `sb`), using the upstream tool names.
- **MCP developer tools** on `cdp` and `sb`: `inspect_page`, `suggest_selectors`,
  `diagnose_selector`, `get_console_messages`, `get_failed_requests`,
  `audit_page`, `capture_debug_bundle` (redacted), `export_session_as_test`
  (a `BaseCase` test from the session's steps) and `diagnose_environment`.
- **Failure reports**: a failed MCP tool call is answered with its kind, the
  call, the page's state (URL, title, ready state, script errors, selector
  matches; read within two seconds), the hint and the next tools to try, as
  text and structured content. Mistyped choices and tool names get a
  did-you-mean.
- **Errors for people**: every `SeleniumBaseError` has a hint, element errors
  point out a likely mistake in the selector, `exit_code()` maps categories to
  exit statuses, and `sbase` prints `error:`, `caused by:` and `hint:` lines.
  `FailureDetails` puts the kind, hint, page and evidence links of a failed test
  into the JSON and HTML reports (`ReportPlugin::with_evidence`).
- **Fingerprints**: `Fingerprint::randomized(os, seed)` draws a coherent
  machine and place for all five `OsType`s; `LaunchOptionsBuilder::fingerprint`
  and `Page::apply_fingerprint` apply one to the Pure CDP engine.
- **Test plugins and storage**: `plugins::observer::{Plugins, TestPlugin}` with
  screenshot, page-source, report and result-store plugins; with the `turso`
  feature, `ResultStore`, `ProfileVault` (AES-256-GCM) and `sbase report`.
- **`sbase`**: `encrypt`/`decrypt`, `test` (carries options into tests as `SB_*`
  variables), `grid`, `install`, `--proxy-list`, a typed recorder that refuses
  what it cannot generate, and `doctor` output that compares the chromedriver's
  version with Chrome's.
- **Chromedriver matching**: `browser::driver_version` and
  `downloader::download_chrome_driver_for(major)`. The launcher downloads the
  driver of the installed Chrome's milestone when the cached one is for another
  major version.
- `ActionRecorder::try_record` (reports why an action cannot be generated, with
  the closest known action), `pw_new_page` and `pw_close` for the Playwright
  feature, `extract_extension`, and iteration on `DriverStack`.
- Parity tooling: `parity/api.toml`, `parity/upstream.toml`, `docs/parity.md`
  and `docs/UPSTREAM_SYNC.md`.
- **Browsers**: launch options for Brave, Comet, Opera, Safari, Internet
  Explorer, Chrome for Testing and `chrome-headless-shell`, typed launch
  switches, capability overrides, Firefox preferences and CDP event capture
  (`browser::options`). `sbase install` picks the chromedriver version.
- **`RunSettings`** (`BaseCase::set_run_settings`): demo mode, slow and fast
  runs, message duration, skipping JavaScript waits, recording `sleep`, and
  resetting a visual baseline; `ActionRecorder::to_gherkin` and
  `save_recording_as_feature` export a recording as a Gherkin feature.
- **`sbase translate`** writes a JSON scenario's actions in another language, and
  the pure CDP `Page` gained `post_message`, `activate_messenger` and
  `set_messenger_theme`.
- **`sbase caseplans` and `sbase behave-gui` as terminal apps** (feature `tui`,
  aliases `case-plans` and `gui-behave`): the Python Case Plans Generator and
  Behave Commander without Tkinter. The first lists the Rust test functions under
  a folder and creates, edits and saves `case_plans/<test>.md` for each, and
  writes `case_summary.md`; the second lists `.feature` files and their
  scenarios, runs the marked ones (or all) through the behave runner with the
  built-in steps, with a tag filter, and shows PASS, FAIL, UNDEF and SKIP per
  scenario. The state of each is a pure state machine tested with plain values,
  and the screens are tested as exact `TestBackend` text. Without `tui` both
  print a hint and exit with status 2, as `commander` does. **Breaking**: the
  earlier file-writing behaviour moved to `sbase caseplans --starter` (the
  `CASE_PLANS.md` checklist) and `sbase behave-gui --new` (the prompt that writes a
  feature file), which work in every build.
- **`Page::gui_click_captcha`** (feature `gui`): clicks a CAPTCHA with the
  operating system's pointer, along a human path, at the screen point worked out
  from the window (`Browser.getWindowForTarget`), the viewport offset, the widget
  frame and the vendor's checkbox offset; `Page::gui_captcha_plan` gives the point
  without moving the pointer. Tested with unit tests and a mocked browser only: it
  has not moved the real pointer or been tried on a live CAPTCHA.
- **Parity**: every API in `parity/api.toml` now has a status (888 implemented,
  383 composed, 94 not-applicable with a reason, none planned). Two audits
  corrected it: 26 entries marked done, and 34 composed or not-applicable ones
  (see below).
- **`--esc-end`** (`--esc_end`, `--ee`): `BrowserConfig::with_esc_end`,
  `Settings::esc_end`, `SB_ESC_END` and `sbase --esc-end`. `BaseCase` puts a key
  listener in the page; once ESC is pressed, `open`, `click`, `click_link_text`,
  `highlight`, `wait_for_element_visible`, `wait_for_ready_state_complete` and the
  typing calls return `SeleniumBaseError::Skipped`, which `run_browser_test`
  counts as a skipped test.
- **`--rec-sb-cdp`**: `ActionRecorder::to_cdp_script`, `to_cdp_test` and
  `save_recording_as_cdp_test` write a recording as a pure CDP program (`Page`
  and `Locator` calls) for 51 recorded actions; any other becomes a
  `// skipped:` comment that quotes its `BaseCase` call. `BrowserConfig::with_rec_sb_cdp`,
  `Settings::rec_sb_cdp`, `SB_REC_SB_CDP` and `sbase --rec-sb-cdp` make
  `BaseCase::export_recording_as_rust` and `save_recording_to_logs` write it, and
  `BaseCase::export_recording_as_cdp_rust` asks for it by name.
  `tests/recorder_cdp_compiles.rs` compiles the program written for every
  supported action.
- **`--reuse-session`** (`--reuse_session`, `--rs`) and **`--rcs`**
  (`--reuse-class-session`, `--reuse_class_session`): `BrowserConfig::with_reuse_session`,
  `with_reuse_module_session` and `with_reuse_class_session(name)`,
  `Settings::reuse_session` and `reuse_class_session`, `SB_REUSE_SESSION`,
  `SB_REUSE_CLASS_SESSION` and the `sbase` flags. A process-wide registry
  (`browser::reuse`) keeps each shared browser's WebDriver server, session id and
  chromedriver; `BaseCase::new` leases an idle browser started with the same
  configuration and attaches to it with a client made in its own Tokio runtime
  (each `#[tokio::test]` has one), after cleaning it: other windows closed, every
  cookie and the open page's storage deleted, `about:blank`. `BaseCase::quit`
  gives it back. One test holds a browser at a time; a second one at the same
  time gets its own. Idle browsers close with their chromedriver after
  `reuse::idle_timeout` (5 s, `SB_REUSE_IDLE_SECS`), at
  `seleniumbase_rs::shutdown_reused_sessions()` and in `drain_cleanups`; on Unix
  the last one also ends with the test binary (see the process-group watcher
  under Fixed), on Windows it is left running. `--rcs` shares a
  browser per Rust module, read from the test thread's name.
  `BaseCase::session_id` and `BrowserSession::session_id` are new. `http` and
  `bytes` are now regular dependencies (they were optional, for `impersonate`).
  Checked against a mock WebDriver and in Chrome 155
  (`tests/reuse_session_chrome.rs`: two `#[tokio::test]` share one Chrome, the
  first test's cookie and storage are gone in the second, and the reaper stops
  chromedriver after the idle timeout).
- **WebDriver ad blocking**: `BrowserConfig::ad_block` makes a Chromium WebDriver
  session (Chrome, Chromium, Edge, Brave, Comet, Opera, Chrome for Testing,
  `chrome-headless-shell`) refuse the requests of ad and tracker hosts. When the
  session connects, reconnects or switches to a window, it sends
  `Network.enable` and `Network.setBlockedURLs` through chromedriver's DevTools
  channel. The hosts are the built-in `AdBlockList`, or a list of your own set
  with `BrowserConfig::with_ad_block_list` (new, as is `with_ad_block`), which
  replaces it. `AdBlockList::bounded_url_patterns` leaves out a host whose parent
  domain is listed and refuses a list that needs more than `MAX_URL_PATTERNS`
  (2,000) patterns, before any browser starts; an `AdBlockList` reads and writes
  as an array of hosts. `BrowserSession::apply_ad_block` repeats the block for a
  window reached some other way. Firefox, Safari and Internet Explorer have no
  DevTools channel through WebDriver and block nothing (a warning says so).
  Checked in a real Chrome 155 (`tests/ad_block_chrome.rs`): a page's own
  requests to listed hosts never reach the server; a frame of another site still
  loads and makes its own requests; a URL that names a port is not matched. The
  nine ad-block entries of the parity manifest moved from `planned` to
  `composed`.
- **Parity audit of the composed and not-applicable entries**, the Rust that
  came out of it:
  - Text selectors: `SelectorBuf::containing_text(text, tag)`, and a CSS selector
    with SeleniumBase's `:contains("text")` runs as XPath in `Page::locator` and
    in the WebDriver `By` conversion.
  - `Page::goto_if_not_url`, `Page::active_element` and `active_element_css`;
    `Page::evaluate` drops a `return ` that starts the last line, as Python does.
  - `Locator::all_waiting` (waits for a first match), `viewport_rect` (window
    coordinates, as `get_element_rect`), `click_and_hold` and `flash_with`.
  - `Browser::debugging_host` (and `http_url` follows it, so an attached remote
    browser reports its own host), `grant_permissions_for(origin, ..)` and
    `grant_all_permissions`; `Permission` now has the 23 kinds Python grants,
    and `Permission::name`.
  - `sbase version`, and `sbase obfuscate` and `unobfuscate` as aliases of
    `encrypt` and `decrypt`.
  - Real-Chrome tests for these in `tests/sb_cdp_parity_chrome.rs`, mock tests in
    `tests/sb_cdp_parity_mock.rs`.
- **Stealth scraping, the wire layer** (feature `scrape`, `src/wire`): parsers
  for TLS `ClientHello`s, JA3, JA4 (with `JA4_r` and the original-order
  variants), peetprint and Akamai HTTP/2 strings; `TlsSpec`, `H2Spec` and
  `HeaderSpec`, which say what BoringSSL and wreq can send and refuse the rest by
  name; the measured table `wire::emulation`, with one entry, Chrome 155.0.8059.40
  on macOS (any other browser and system is `NotMeasured`); and the capture tools
  that write it, `capture_tls` and `capture_hints` (`scrape`) and `capture_h2`
  (`impersonate`).
- **Stealth scraping, the impersonating client** (feature `impersonate`):
  `ImpersonationProfile`, `impersonation_client` (wreq on BoringSSL) and
  `CustomProfileBuilder`, where every part (JA3, `JA4_r`, peetprint, signature
  algorithms, Akamai string, header names, order and values, identity) can be
  supplied and what cannot be sent is refused with a named error before a client
  exists; and `impersonate::tls_server`, an in-memory CA that issues one leaf
  per host for loopback TLS. Known gaps against Chrome 155: the `trust_anchors`
  extension, the ML-DSA signature algorithms and a GREASE signature algorithm.
- **Stealth scraping, the scrape layer** (feature `scrape`, `src/scrape`): an
  RFC 6265 cookie jar with public-suffix checks; `StealthIdentity` and its
  owner-only identity file; `SessionPolicy`; challenge detection for seven
  vendors and a mitigation policy; a proxy pool with locales by exit country;
  per-site state with an atomic JSON store; redacted scraper events; and
  `BrowserScraper`, which fetches through a real Chrome on the pure CDP engine,
  solves challenges and rotates egress and profile. `Fingerprint::randomized_with_major`
  fixes the Chromium major an identity claims.
- **`HttpSession`** (feature `impersonate`): plain HTTP requests that continue a
  browser page's session with the measured TLS, HTTP/2 and headers of Chrome 155
  on macOS. It reads the cookies, user agent, languages and proxy from a page
  (`HttpSession::from_page`) or an identity, keeps an RFC 6265 jar, follows
  redirects by hand, bounds time, bodies and pace, judges replies with the
  mitigation and session policies, and writes cookies back to the page. A page of
  another Chrome major, one that says `HeadlessChrome`, or an unmeasured system is
  refused. The first request to an HTTP/1.1-only origin carries an HTTP/2
  `priority` header.
- **`MitmProxy`** (feature `impersonate`): a loopback proxy that ends a real
  Chrome's TLS with the ephemeral CA and replays each request through the
  impersonating client, behind a random token, with bounded connections, heads and
  bodies, and a hot-swappable egress and profile. Upgrades other than WebSocket
  are refused, and a request claiming another Chrome major is refused. Measured on
  Chrome 155 (headless, macOS): a chain carrying the pinned leaf key is accepted by
  a Chrome that has the SPKI-list switch and no proxy, and is never served to a
  page behind the proxy.
- **WebSocket through `MitmProxy`**: a `wss://` handshake goes upstream over
  HTTP/1.1 with the measured WebSocket `ClientHello`, the `101` is relayed, and the
  bytes are copied both ways with idle and per-direction byte bounds
  (`MitmOptions::websocket_idle_timeout`, `max_websocket_bytes`); half upgrades and
  other protocols are refused (`MitmError::MalformedUpgrade`, `NotIntercepted`).
- **The `MitmProxy` priming page**: a WebSocket may now be a tab's first request
  through the proxy. Chrome never shows a WebSocket's `407` to `Fetch` (measured
  on Chrome 155, tab and browser target alike), so `configure_launch` makes
  `MitmProxy::prime_url()` (`https://sb-mitm.invalid/` and a random path) the
  start page; the proxy answers it itself behind the token, and Chrome keeps the
  credentials for the browser context. A context created later loads the page
  itself. Anything else for that host is an empty `404`; nothing for it goes
  upstream. `MitmStats::primed` counts the loads.
- **`ws://` through `MitmProxy`**: Chrome sends a `ws://` WebSocket to an HTTP
  proxy as a `CONNECT` whose tunnel carries the handshake in plain text (measured
  on Chrome 155). The proxy now reads a tunnel's first bytes: a TLS handshake is
  ended as before, and a plain-text WebSocket handshake takes the checks, header
  rewrite, bounds and relay of a `wss://` one, upstream as `http://`. Any other
  plain text in a tunnel is still refused and never relayed.
- **`StealthSession`** (feature `impersonate`): the session that moves between
  `HttpSession` and a real Chrome behind `MitmProxy` by the mitigation and session
  policies. It starts the proxy when a browser is first needed, makes up to 4
  attempts per fetch by default, and rotates the profile and the egress in both
  modes. Tested against loopback sites, a scripted browser and, in ignored tests,
  a headless Chrome 155 on macOS; not against a live service.
- **Re-exports**: the main types of the stealth layer at the top of `wire`,
  `scrape` and `impersonate` and at the crate root behind the same features. The
  TLS and HTTP/2 `SpecError` enums are `TlsSpecError` and `H2SpecError`. The stealth
  layer is experimental and not a stable API.
- `docs/tutorials/stealth_scraping.md` and `docs/adr/003-stealth-scraping-layout.md`.
- A `rust-analyzer.toml` that loads the optional features so analysis covers
  `src/mcp` and `src/scrape`.
- `CLAUDE.md` (a map of the modules, checks and gotchas), `CHANGELOG.md`, and
  `docs/design/stealth_parity_plan.md` (the plan for the stealthscraper-rs
  feature set and JA3/JA4/Akamai impersonation).
- **Real referrals**: `BaseCase::generate_referral(start_page, destination_page,
  selector)`, `generate_traffic(.., loops, selector)`, `generate_referral_chain(pages)`
  and `generate_traffic_chain(pages, loops)` open the start page (or use the current
  one), build a link to the destination in it with a script and click it, so the
  destination receives a real `Referer`, as Python does. The four parity entries
  are `implemented`. Mock tests assert the order of the commands and that the URL
  stays inside its string literal; `tests/generate_referral_chrome.rs` (ignored)
  has a Chrome 155 visit a loopback site that records the `Referer` it receives.
- **`BaseCase::set_content_to_frame(frame)`** replaces the page with the content
  of an iframe on it, selected or named, as Python does; an iframe whose `src`
  is a web or file address opens in a new tab instead. `set_content_to_default`
  brings the page back (all swaps at once) and `set_content_to_parent` undoes
  one swap. With nothing swapped they still only leave the frame, as before.
  Tests against a fake browser behind the mock WebDriver cover the swap, the
  tab, nested swaps, a swap the page forgot and a missing frame;
  `tests/set_content_to_frame_chrome.rs` (ignored) does it in Chrome 155. The
  parity entry is `implemented`.
- **Downloads on the pure CDP engine**: `Page::download_file(url, dir)` has the
  page fetch an address and save it, `download_by_click(selector, dir)` follows
  the download a click starts, `save_file_as(url, path)` saves at an exact path
  and `assert_downloaded_file(name, timeout)` waits for a file in the folder the
  browser saves to; each returns the final path. They follow the browser's own
  download events (`Browser.setDownloadBehavior` with events), so a cancelled
  download, a bad HTTP status or a fetch the page may not make is
  `SeleniumBaseError::Download`, and a download that does not begin or end in
  time is `WaitTimeout`. `Browser::download_dir` says where downloads go. The
  three parity entries are `implemented`. Mock tests are in
  `tests/sb_cdp_download.rs`, Chrome 155 tests in
  `tests/sb_cdp_download_chrome.rs`.
- **Firefox add-ons**: `BaseCase::install_addon(path, temporary)` installs an
  `.xpi` file or an unpacked add-on directory through geckodriver's
  `moz/addon/install` command and returns the add-on id, and
  `uninstall_addon(id)` removes it with `moz/addon/uninstall`. Any other browser
  gets `SeleniumBaseError::Unsupported`; an empty path, one with `..` in it, one
  that does not exist and one that is neither an `.xpi` nor a directory are
  `InvalidConfig`, before anything is sent. The parity entry is `implemented`
  (it was `planned` and always returned `Unsupported`). Unit tests assert the
  exact endpoint and JSON body against the mock WebDriver. The same bodies were
  sent with `curl` to a real geckodriver 0.37.1 and Firefox 153 ESR in a Docker
  container (`docker/firefox-check`, which holds nothing of this crate): both
  an unpacked directory and an `.xpi` installed with `temporary: true`, the
  add-on's content script ran, and the uninstall stopped it.
  `tests/install_addon_firefox.rs` (ignored) runs `BaseCase` against that
  geckodriver and installs and removes an add-on.

### Changed

- `examples/tauri-profile-manager` is on Tauri 3: `tauri` `3.0.0-alpha.4`,
  `tauri-build` `3.0.0-alpha.3`, `tauri-plugin-opener` `3.0.0-alpha.2` and
  `@tauri-apps/cli` `3.0.0-alpha.4`, pinned exactly. No code had to change; on
  macOS its 115 tests pass and the Tauri 3 CLI builds the `.app` (the window was
  not opened in that check). One test of the example had gone stale when
  `Emulation::locale` started setting the language too; it now checks the last
  user-agent override.
- The `Dockerfile` links Debian's `chromedriver` as `downloaded_drivers/chromedriver` in
  the working directory, so the WebDriver engine in the image starts a browser with no
  network instead of downloading a driver, and its builder stage takes a
  `CARGO_BUILD_JOBS` build argument for small machines. `docker/Dockerfile.verify` and
  `docker/verify-linux.sh` run the test suite on Linux in a container (the sources
  mounted read-only, the build output in a volume, a virtual display for the headed
  tests). The suite, run that way on linux/arm64 with Chromium 154, is in
  `docs/platforms.md`.
- `sbase` exits with a status that follows the error's category instead of
  always 1, and prints `error: ...` instead of `[ERROR] ...`.
- `TestResult` has a `failure` field, and `PageEvidence` gains `current_url` and
  `page_title` (with defaults).
- The `cdp` MCP server no longer offers `tag:contains("text")` selectors, which
  its engine never supported; `diagnose_selector` gives the XPath for one. (A
  `:contains()` selector now does run, converted to XPath, in `Page::locator`.)
- **Breaking** (parity audit): `BaseCase::set_time_limit` now fails the test once
  it has run longer than the limit, at its next open, readiness wait or element
  action, and cuts waits to the time left; it used to only cap single waits.
  `set_default_timeout` raises or lowers the default wait (1 to 60 seconds)
  instead of setting the time limit, and `reset_default_timeout` restores it.
  `BaseCase::new` opens `start_page` when the configuration has one.
  `run_browser_test` (and `Plugins::run_browser_test`) count a test that ends in
  `skip_test` as passed and log the reason. `Emulation::locale` also sets
  `navigator.language` and `Accept-Language`, keeping the user agent and client
  hints. `Permission::ALL` has 23 entries instead of 7 and `Permission` has more
  variants. The parity entries for `--esc-end`, `--rec-sb-cdp` and
  `gui_click_captcha` are now `planned` (so were the ad-block options, until the
  WebDriver ad block above); the notes of many composed entries say what differs
  from Python.
- The pure CDP engine runs `session_registry` (27 providers: the default
  registry without `plugins` and `connection`), so a page reads Chrome's own
  `navigator.plugins`, `mimeTypes`, PDF viewer flag and `navigator.connection`;
  `navigator.userAgentData` is defined only in a secure context; and a desktop
  persona's screen is sent as a `0` by `0` device-metrics viewport plus
  `screenWidth` and `screenHeight`, so the page keeps its real viewport.
  `default_registry` still holds 29 providers.
- Requires Rust 1.99 (`rust-version`, the `Dockerfile` base image and the CI msrv
  job); `zip` moved to 9.0 and the lockfile is current.
- **Breaking**: `BaseCase::generate_referral`, `generate_traffic`,
  `generate_referral_chain` and `generate_traffic_chain` do what the Python
  methods do (see Added). Their old forms returned or opened a `?ref=<referrer>`
  URL, which sends no `Referer`; the old `(url, referrer)` signatures are gone.
- **Breaking**: `BaseCase::set_content_to_frame` takes the iframe to show (a
  selector or a name), not HTML to append as a new iframe; there is no
  replacement for the old behaviour, which `set_content` and `execute_script`
  cover.
- **Breaking**: `ad_block` in a WebDriver launch no longer passes
  `--blink-settings=imagesEnabled=false`. That turned every image off and blocked
  no ad; `with_block_images(true)` is the setting for it.
- `BaseCase` actions wait for their element (up to `set_timeout`, 10 s by
  default) and fail with `WaitTimeout` naming the selector.
- **Breaking**, from a parity audit (each method now does what the Python
  method of the name does): `press_keys(css, text)` types text into an element
  (the old one-named-key method is `press_key(name)`); `select_all(css)` returns
  the matching elements (the old select-every-option method is
  `select_all_options`); `bring_to_front(css)` raises an element (the window
  focus is `bring_active_window_to_front`); `console_log_script(script)` takes
  the expression to log; `deferred_check_window(name, level)` and
  `delayed_check_window(name, level)` take a name and a level;
  `assert_downloaded_file` is `async` and waits for the download;
  `switch_to_frame_of_element` returns the frame's name; `find_text`,
  `find_exact_text` and `find_non_empty_text` wait for visible text and fail with
  a timeout instead of returning `None`; `DeferredAssert` has three more
  variants. `check_window` at level 0 no longer fails on a difference.
- **Breaking**: `check_window` saves and compares the page's tag structure, as
  Python does, not the screenshot byte for byte. The baseline is the folder
  `visual_baseline/<test>/<name>` (`page_url.txt`, `baseline.png` and
  `tags_level_1.txt` to `tags_level_3.txt`); levels 1 to 3 compare the tags, the
  tags with their attribute names and the tags with their attribute values, and
  a level other than 0 to 3 is an error. A baseline saved as
  `visual_baseline/<name>.png` is not read; the first run saves a new one.
- `BaseCase::generate_referral`, `generate_traffic` and their chains,
  `set_content_to_frame` and `install_addon`, the `--reuse-session` options, the
  `caseplans` and `behave-gui` commands and three CDP downloads were marked
  `planned` in `parity/api.toml`, because the Rust code only looked like the
  Python one. The referral helpers, `set_content_to_frame`, `install_addon`, the
  CDP downloads, the `--reuse-session` options, the `caseplans` and `behave-gui`
  terminal apps and `gui_click_captcha` have since been written (see Added), so
  no entry is `planned` now.
  `objectify` is not applicable and `mkrec` is mapped onto `sbase record`.
- Waits report the error that stopped them, and link checks report real status
  codes.
- `Cookies::save` writes JSON, not Python pickles. The old `seleniumbase-mcp`
  tool names were replaced by the upstream names.
- **Breaking**: `SeleniumBasePlugin`, `PluginManager` and `DbReportingPlugin`
  were removed (nothing called the manager); `ElementApi::find_element` was
  removed from the trait; `From<AsyncStepRegistry> for StepRegistry` was removed;
  `common::{decorators, exceptions}` remain as `#[deprecated]` shims over
  `utilities::retry`.
- The real-Chrome tests of `MitmProxy` (`tests/impersonate_mitm_chrome.rs`) no
  longer all skip off the measured system: only the one that compares the browser
  with the measured persona does. The pin, loopback and SSRF refusal and
  WebSocket tests run on any Chrome or Chromium, one of another major being
  started with the profile's user agent. On linux/arm64 with Chromium 154
  (`docker/Dockerfile.verify`) seven of the eight now run and pass.

### Fixed

- **Windows**, found by the first runs on a Windows PC: every `sbase` command
  crashed at start with a stack overflow (a debug build needs more than the 1 MB
  a Windows main thread gets; `sbase` now runs on a 16 MB thread, and
  `tests/cli_e2e.rs` starts it under a 1 MB stack); the chromedriver for an older
  Chrome was never fetched, because `chrome.exe --version` prints nothing there
  (the version now comes from the folder beside the program, as in Python
  SeleniumBase); a driver whose session failed to start kept running (a dropped
  `DriverProcess` now ends it); text files were checked out with CRLF
  (`.gitattributes` makes them LF) and the parity scripts wrote CRLF; and four
  tests expected `/` in printed paths.
- **The `playwright` feature no longer needs Python.** `rustwright-core` was a
  direct dependency with its default `python` feature, which adds Python
  bindings through pyo3, so every program built with `playwright` linked
  `libpython` and failed to start without it (`dyld: Library not loaded:
  @rpath/libpython3.14.dylib`). It is now used with `default-features = false`,
  as `rustwright` itself does: pyo3 is gone from the dependency tree, the 54
  `playwright` library tests pass, and the test binary links no `libpython`.
- **Browsers no longer outlive the program that started them.** A chromedriver,
  the Chrome it starts, and a Chrome launched by the pure CDP engine kept running
  when the program that started them was killed (an interrupted test run, a
  stopped MCP server) or, for `--reuse-session`, simply ended. On macOS a leftover
  Chrome could hold the person's own profile, so the Chrome they opened closed at
  once. On Unix each launched driver or browser now joins a process group led by
  a small `/bin/sh` watcher that reads a pipe this program holds; when the
  program ends, by any means, the watcher sends the group SIGTERM and then
  SIGKILL. Checked by killing `sbase` and `seleniumbase-mcp --server cdp` with
  SIGKILL while Chrome 155 was open (nothing was left), and by unit tests,
  including one whose parent process is killed. Windows is not covered.
- `sbase cdp`, `cache-clear`, `throttle3g`, `cdp-click-element`, `cdp-type-text`,
  `print-pdf`, `pdf-text` and `assert-pdf-text` failed with "CDP is unavailable"
  unless `--cdp` was given; with no mode asked for they now use cdp mode on a
  Chromium browser. `screenshot --path` and `save-source --path` wrote into
  `latest_logs/` while printing the path given; they now write that path.
  `run-scenario` exited 0 when a step failed; it now exits 1. Text read from a
  PDF spells the ligatures U+FB00 to U+FB06 as their letters, so "fixture"
  printed by Chrome with an fi glyph is found. Found by the new
  `tests/cli_e2e_chrome.rs`, which runs the browser commands as the real binary
  on headless Chrome.
- `sbase decrypt` with the wrong passphrase said `unsupported operation:
  decryption failed: ring::error::Unspecified` and advised on browsers and engines;
  it now says the passphrase in `SB_ENCRYPTION_KEY` is wrong or the token was
  modified (exit status 2 as before). `sbase import-python` with a file it cannot
  read said only `No such file or directory (os error 2)`; it now names the file.
  `tests/cli_e2e.rs` runs every subcommand's `--help` and the offline commands.
- Log lines went to standard output, in colour even when the output was a file or
  a pipe. `seleniumbase-mcp` therefore wrote them into the JSON-RPC stream (three
  lines in a session that did nothing but connect), which a client reads as
  corrupt messages, and `sbase get-title | ...` carried the logs with the
  answer. The `init_tracing*` functions now write to standard error, with colour
  only on a terminal and not when `NO_COLOR` is set. `tests/mcp_stdio_chrome.rs`
  speaks JSON-RPC to the real binary and fails on any line that is not a message.
- The three MCP servers answered `initialize` with the server name `rmcp` and the
  library's version (`3.5.1`), so a client listed `cdp`, `driver` and `sb` as the
  MCP library. They now report `seleniumbase-cdp`, `seleniumbase-driver` and
  `seleniumbase-sb` with this crate's version.
- `sbase open`, `smoke` and the other commands that drive a browser ignored
  `SB_HEADLESS` and the other `SB_*` settings variables unless `--config` was
  given, and ignored `SB_WEBDRIVER_URL` and `SB_CHROME_BIN`; in a container that
  made `--headless` mandatory. They now start from the config file, apply the
  variables on top, and then the command-line options, so an explicit
  `--headless` or `--headed` still beats the environment. `--webdriver` now
  defaults to `SB_WEBDRIVER_URL`, then `http://localhost:4444`, and a bad
  variable value (`SB_HEADLESS=maybe`) stops the command with an error before
  a browser starts. `tests/cli_env.rs` runs the real binary against a fake
  WebDriver server.
- The chromedriver download asks Chrome for Testing for the `linux-arm64` build on
  an arm64 Linux machine (a Docker container on an Apple-silicon Mac, or on
  Graviton) instead of the x86-64 `linux64` one, which cannot run there: the
  WebDriver engine failed with "chromedriver exited as it started". The build
  exists from Chrome 153 on; on Linux machines of any other architecture the
  error now says that no build is published and where to put a driver.
- The real-Chrome tests that compare a browser with the measured Chrome 155 on macOS
  (`impersonate_capture_h2_chrome`, `impersonate_http_chrome`, `impersonate_mitm_chrome`,
  `stealth_session_chrome`) say they are skipped and pass on a system that has no
  measurement (Linux, Windows), instead of failing on `NotMeasured`. A macOS machine whose
  Chrome has another major version still fails them: that is the drift check.
- The wire test `a_custom_ja3_arrives_exactly_as_given` failed on about two runs in
  five, on any system: BoringSSL pads a ClientHello of 256 to 511 bytes up to 512
  with a padding extension, and the "classical groups only" variant falls on either
  side of that limit with the random size of the ECH GREASE payload. The test now
  asserts that rule (a padded hello is 512 bytes, an unpadded one is not in the
  range) and compares the JA3 and JA4 without the padding.
- `BrowserSession::reconnect` stops the chromedriver of the previous session when
  the new one starts its own, so `uc_open_with_reconnect` no longer leaves one
  chromedriver running per call.
- `js_click`, `js_type`, `set_attribute`, `remove_attribute`, `scroll_to`, the
  localStorage helpers and every other `BaseCase` page script quote their
  selectors and values with the crate's `js_code::quote`; a quote, a newline or
  U+2028 in test data could break out of the string. Waits that run out of time
  (`wait_for_title`, `wait_for_attribute`, `wait_for_exact_text`, the link-text
  waits, `wait_for_any_of_elements_present`, `download_file`) return `WaitTimeout`
  instead of `AssertionFailed` and name what they waited for.
  `wait_for_ready_state_complete` takes its default from the configured timeout.
- `BrowserSession::wait_for_element`, `wait_for_element_visible`,
  `wait_for_element_clickable`, `wait_for_element_absent` and `wait_for_text`
  return `WaitTimeout`, not `AssertionFailed`, when the time runs out, and so
  do the `BaseCase` methods that return their error as it is (`wait_for_element`,
  `wait_for_element_visible`, `wait_for_element_absent`, `wait_for_text`,
  `find_element`, `choose_file`, `slow_click`, `get_select_options` and
  `get_shadow_root`). The message names the locator (`CSS(#a)`) or the text, and
  the seconds waited. Code that matched `AssertionFailed` on these waits must
  match `WaitTimeout`.
- `CdpPage` (the DevTools page of a WebDriver session) quotes the selectors,
  attribute and property names, option texts and values it puts into its page
  scripts with `js_code::quote`, as `BaseCase` does. It escaped only the
  backslash and the single quote, so a newline or U+2028 in a value (the text of
  a `<textarea>`) was a syntax error in the script.
- The examples run offline against a loopback site (`examples/local_site`) and
  check what they claim; the `partial link=` selector prefix in the selectors
  tutorial is `partial_link=`.
- The native pointer helpers of the `gui` feature (`gui_click_element`,
  `gui_click_with_offset`, `gui_drag_and_drop`, `gui_move_to_element`,
  `gui_hover_and_click` and `uc_gui_handle_captcha`) aim at the element's place on
  the screen. The script that finds it scrolls the element into view and adds the
  browser toolbar, the window borders and the offset of each same-origin frame the
  element is in; it used to add only the window's position, so a click in a headed
  window landed a toolbar's height too high. `uc_gui_handle_captcha` waits for the
  element to be visible first. Browser pixels are taken to be screen pixels.
- `get_domain_url` keeps a port that is not the scheme's default
  (`http://localhost:8080`), as Python's does; it used to drop it.
- `wait_for_element_present` looks for the element once before it gives up, so an
  element that is already there is found with a timeout of zero.
- `assert_is_instance` and `assert_not_is_instance` look at the type of the value
  passed. They read the name of a `&dyn Any`, which is always `dyn core::any::Any`,
  so the first never passed and the second never failed. They take `&T` now; a
  call that passes `&value` compiles as before.
- `xpath_to_css` (so `convert_xpath_to_css` and the Python importer) converts only
  what has an exact CSS equivalent, a tag or `*` with attribute tests joined by
  `and`, and returns `InvalidSelector` for the rest. It used to drop a predicate it
  did not understand: `//div[2]` became `div` and an `or` became an AND. An id with
  a backslash or a line break no longer produces invalid CSS in `Selector::Id`,
  and `..` and `../` count as XPath.
- `normalize_page_url` (`open`, `Page::goto`, the scrape and impersonation fetches)
  matches schemes without regard to case, so `HTTPS://EXAMPLE.COM` is left alone,
  and refuses a file path and a scheme a browser cannot open (`ftp:`, `mailto:`,
  `javascript:`) with a message that says so, instead of turning them into an
  `https://` host.
- `Page::close` (pure CDP) waits, for at most two seconds, until the browser no
  longer lists the tab; `Browser::list_pages` right after a close could still show
  it, which made a real-Chrome test fail one run in five.
- `sbase objectify`, `caseplans`, `behave-gui`, `mkrec`, `mkchart`, `mkpres` and
  `print` exit with status 6 and print the cause when they cannot write or read
  their file; they used to exit 0. The WebDriver-hosted CDP page no longer sends
  `DOM.enable` (it finds elements through `Runtime.evaluate`), so no part of the
  engine enables a detectable domain.
- A chromedriver that does not match Chrome (the cause of "chromedriver keeps
  crashing"): the launcher, `sbase install` and `sbase doctor` now agree with the
  installed Chrome, and a replaced driver is written as a new file because macOS
  kills a signed binary overwritten in place.
- The stealth layer no longer enables the `Runtime` CDP domain; the proxy-auth
  extension is one Chrome can load; the page-script stubs were
  replaced with working scripts (checked in a real Chrome); Playwright honours
  `BrowserConfig` and keeps web security on.
- `CdpDriver` sends commands to the tab, not the browser socket; `prompt()` asks
  for text; deferred assertions check the page they were made for; chart series
  line up by label; the presentation escapes its title; the HTML inspector gives
  each issue a selector that matches its element; `get_origin` keeps the port;
  PDFs that need a password can be read.
- A WebDriver session with a fingerprint no longer sends the proxy login
  (`Proxy-Authorization`) to every site as an extra header; the pure CDP path
  already filtered it.
- A flaky Grid test (the stand-in `java` was slow on its first run).
- Parity audit: `set_text`, `set_text_content` and `highlight_update_text` set
  an input's value instead of its (invisible) `textContent`; the deferred
  asserts check at once, `deferred_assert_exact_text` is exact and
  `deferred_assert_non_empty_text` checks text; `switch_to_frame_of_element`
  finds elements in iframes; `select_option_by_index` fails for a missing option
  and fires `change`; `get_chrome_version` returns the version number and
  `get_chromium_version` the major version; `is_valid_url` follows Python's
  pattern; `ad_block` uses upstream's selector list; `add_js_code_from_link`
  downloads and injects the code; `post_success_message` and the other toasts go
  away; `highlight` restores the element's outline; `delete_saved_cookies`
  deletes the file `save_cookies` wrote; `console_log_string` writes to the
  console.
- The recorder generates Rust for every recorded action, keeps events across
  navigations, and the stdin recorder only says "recorded" for what it recorded.
- The CLI scaffolders no longer overwrite files; the interactive commands stop at
  the end of their input and on Ctrl-C; Gherkin runs the way `behave` does.
- `sbase translate` and `translate_scenario` rewrite the `action` of the steps in
  the root `steps` list only, not an `action` in any other object three levels
  down (such as `meta.x.action`), which was changed and counted as translated.
- `EgressRef::parse` lower-cases the host of `socks5` and `socks5h` specs, as it
  does for http and https, so one proxy spelled two ways has one `EgressKey`.
- `ActionRecorder::to_gherkin` skips a recorded `type_text` with no text and says
  so in a `# skipped:` comment; the written `I type "" into "..."` step matched
  no handler, so the generated scenario failed at that step.
- `sbase translate --overwrite` keeps the file's permissions (the replacement was
  left at mode 0600) and writes through a symbolic link instead of replacing it.

### Security

- Chrome switches in a profile payload that run programs, load code or reroute
  traffic are refused; file names a caller supplies cannot climb out of their
  folder; the HTML report escapes what tests print; proxy passwords no longer
  appear in `Debug` output.
- `ActionRecorder::to_gherkin` escapes the name of an action it cannot turn into
  a step, so a recorded name with a line break can no longer end the
  `# skipped:` comment and add a Scenario or steps to the feature.
- `capture_debug_bundle` no longer copies what earlier tool calls returned into
  `bundle.json`: the result of a successful call is replaced by its length, so a
  cookie, a stored token or a page's text read by `get_cookies`,
  `manage_cookies` or a storage tool is not saved with the bug report.

</Update>
