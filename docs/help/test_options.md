---
title: "Test options"
sidebarTitle: "Test options"
description: "Every option of Python SeleniumBase's pytest plugin as a flag of sbrs test, with its SB_* variable, settings key and Python name."
---

`sbrs test` takes every option of Python SeleniumBase's pytest plugin as a flag,
in each of its spellings (`--no-sandbox`, `--no_sandbox`, `--dcsp`). It runs
`cargo test` (or `cargo nextest`) and hands each flag you give to the tests as
an `SB_*` environment variable. The `sb` and `sb_cdp` fixtures read the
variables with `Settings::load`, so the same option works in a test, in the
settings file and in a plain `cargo test`.

<Info>

**What you will learn**

- How a flag becomes an `SB_*` variable, a settings key and a fixture setting.
- Which options select, run, repeat and debug tests, and which belong to the browser.
- Which options are accepted and ignored, and why.
- How the behave `-D` options map to `sbrs test`.

</Info>

<Visibility for="agents">

Summary for AI exports: `sbrs test [FILTER] [flags] [-- HARNESS ARGS]` runs the project's tests with Cargo and exports each flag as an `SB_*` variable (`--headless` is `SB_HEADLESS=true`, `--env qa` is `SB_ENV=qa`). A switch is `--flag` or `--flag=false`; `-` and `_` spellings and Python's aliases are the same flag. The pytest options `-k`, `-m`, `-n`, `-s`, `--co`, `--reruns`, `--reruns-delay`, `--time-limit`, `--pdb`, `--trace` and `--final-debug` are `sbrs test` flags too. `sbrs test --help` and `sbrs options` list them, and the table is `seleniumbase_rs::config::test_options::TEST_OPTIONS`. A run empties `latest_logs/` and `downloaded_files/` first (`--archive-logs` and `--archive-downloads` keep them); plain `cargo test` never does.

</Visibility>

## How an option reaches a test

```bash Terminal
sbrs test login --headless --browser firefox --env qa --reruns 2
```

That runs `cargo test login` with `SB_HEADLESS=true`, `SB_BROWSER=firefox`,
`SB_ENV=qa` and `SB_RERUNS=2` in the environment of the tests. Only the flags you
pass become variables, so everything else still comes from the settings file
(`sbase_config.toml`, or the file `-c FILE` / `--settings-file` names) and from
the variables you exported yourself.

- **Precedence.** A flag of `sbrs test` wins over an `SB_*` variable already in
  the environment, which wins over the settings file, which wins over the
  default. A variable that is empty or blank counts as unset. The options of
  `sbrs` before the command (`sbrs --headless test`) are forwarded too, and a
  flag after `test` wins over them.
- **Switches.** A switch is `--flag`, which is true, or `--flag=false`, so a
  setting the file or the environment turned on can be turned off for one run.
  Write the value with `=`, as the help shows it (`--headless[=HEADLESS]`): a
  separate word is not read as the value.
- **Spellings.** Every flag is accepted with `-` and with `_`
  (`--no_sandbox`, `--demo_sleep=2`) and under each of Python's aliases. The
  name before each entry below is the one `sbrs test --help` shows; its other
  spellings follow it.
- **Browser flags.** Two flags that choose different values for one variable
  (`--firefox --chrome`) are refused: `--chrome and --firefox both choose
  SB_BROWSER: give only one of them`.
- **In a test.** `Sb::configure` changes the same settings for one test, and the
  keys are the names of the settings (see [Writing Tests with the sb
  Fixture](/tutorials/writing_tests#what-the-run-configuration-does)).

<Note>

`-k`, `-m`, `-n`, `-s`, `--co`, `--collect-only`, `--reruns`, `--reruns-delay`,
`--pdb` and `--trace` are options of pytest itself and of its plugins
(pytest-xdist, pytest-rerunfailures), not of SeleniumBase's own plugin, so the
[parity table](/parity) has no entry for them. They are flags of `sbrs test` all
the same, with the meaning described in the **Run control** and **Debug modes**
groups below; `sbrs test --help` lists them. Every option of SeleniumBase's
plugin has a row in the table of options. The ones that do nothing here are
listed under [Accepted and ignored](#accepted-and-ignored).

</Note>

<Warning>

`--archive-logs` and the clearing it replaces happen only under `sbrs test`.
At the start of a run (never for `--co`) `sbrs test` empties `latest_logs/` and
`downloaded_files/` of each package it tests, or with `--archive-logs` and
`--archive-downloads` moves them to `archived_logs/` and `archived_files/`. A
plain `cargo test` or `cargo nextest run` never touches them.

</Warning>

## The options by group

<AccordionGroup>

<Accordion title="Test data" icon="database" defaultOpen>

The tests read these from the fixture: `sb.env()`, `sb.account()`, `sb.data()`,
`sb.var1()` to `sb.var3()` and `sb.variables()` (on `SbCdp`, `data()` returns the
`TestData` that holds them). They sit in the `[test]` table of the settings file.

<ParamField path="--env" type="string, one of 24">

The environment the tests run against. `SB_ENV`, key `env`. Python: `--env`, and
`environment` in `SB(...)`. `sb.env()` is `test` when none was given. The choices
are `qa`, `rc`, `staging`, `develop`, `production`, `performance`, `replica`,
`fedramp`, `offline`, `online`, `master`, `remote`, `legacy`, `local`, `alpha`,
`beta`, `demo`, `gdpr`, `main`, `test`, `gov`, `new`, `old` and `uat`, in any
letter case. Another value exits with status 2: `invalid value 'x' for --env
(SB_ENV): choose from qa, rc, staging, ...`.

</ParamField>

<ParamField path="--account" type="string">

A free account name. `SB_ACCOUNT`, key `account`. Python: `--account`.

</ParamField>

<ParamField path="--data" type="string">

Free test data. `SB_DATA`, key `data`. Python: `--data`.

</ParamField>

<ParamField path="--var1, --var2, --var3" type="string">

Free variables. `SB_VAR1` to `SB_VAR3`, keys `var1` to `var3`. Python: `--var1`
to `--var3`.

</ParamField>

<ParamField path="--variables" type="dictionary">

A dictionary of values. `SB_VARIABLES`, key `variables`. Python: `--variables`.
It is a JSON object (`--variables='{"k":1}'`) or the Python dictionary literal
Python accepts (`--variables="{'k': 1}"`). Anything else exits with status 2:
`Expecting a dictionary for "variables"!`.

</ParamField>

</Accordion>

<Accordion title="Debug modes" icon="bug">

These stop a test in a prompt with the browser open. The
[Debug modes tutorial](/tutorials/debug_modes) walks through them.

<ParamField path="--pdb" type="switch">

A post-mortem prompt when a test fails, after its last attempt, with the
browser still open. `SB_PDB`, key `pdb`. Python: pytest's `--pdb`.

</ParamField>

<ParamField path="--trace" type="switch">

A prompt before the body of each test. `SB_TRACE`, key `trace`. Python: pytest's
`--trace`.

</ParamField>

<ParamField path="--final-debug, --ftrace, --fdebug, --final-trace" type="switch">

A prompt after each test, before the browser quits. `SB_FINAL_DEBUG`, key
`final_debug`. Python: `--final-trace` and `--fdebug`, with the other two
spellings.

</ParamField>

<ParamField path="SB_BREAKPOINTS=0" type="variable">

Turns `sb.breakpoint(..)` and `page.breakpoint(..)` off: `0`, `false`, `no` or
`off`, as Python's `PYTHONBREAKPOINT=0` does. It is a variable only, with no
flag.

</ParamField>

Any of `--pdb`, `--trace` and `--final-debug` makes `sbrs test` add
`-- --test-threads 1 --nocapture`, so one test talks to the terminal at a time,
and refuse `--runner nextest` with status 2 (`--pdb needs the cargo runner:
nextest gives tests no standard input`).

</Accordion>

<Accordion title="Run control" icon="play">

Every `sbrs test` accepts `FILTER` (run the tests whose name contains this
text), `--test NAME` (one file from `tests/`), `--example NAME`, `--release`,
`--features a,b` and harness arguments after `--`. `FILTER` and `-k` together
exit with status 2.

<ParamField path="-k" type="expression">

Select tests by name, as pytest's `-k`: `login or signup`, `not slow`,
`(a or b) and not c`. Each word is a substring of the test's name, and the match
is case-sensitive (pytest's is not). With the cargo runner an expression of the
shape `(a or b) and not c and not d` is run as filters `a b` and skips `c d`:
`-k "login or signup"` is `cargo test login signup`, `-k "not admin"` is
`cargo test -- --skip admin`. Anything else, such as `a and b`, exits with
status 2 and asks for `--runner nextest`, where the expression becomes the
filterset `test(~a) and test(~b)`.

</ParamField>

<ParamField path="-m" type="expression">

Select tests by marker, as pytest's `-m`. A marker is a module name: `-m smoke`
runs the tests in a `mod smoke { .. }` at any depth, as `cargo test smoke::`
(nextest: `test(/(^|::)smoke(::|$)/)`). The expression grammar and its limits
are those of `-k`.

</ParamField>

<ParamField path="-n, --threads" type="number or auto">

How many tests run at once, as pytest-xdist's `-n`. `SB_THREADS`, key `threads`.
A number becomes `-- --test-threads N` (nextest: `--test-threads N`) and also
`SB_THREADS=N`; `auto` adds nothing, so Cargo decides. Anything else exits with
status 2. A debug flag or a recorder flag forces one thread.

</ParamField>

<ParamField path="-s, --no-capture, --nocapture" type="switch">

Show what the tests print as they run (`-- --nocapture`, nextest: `--no-capture`).
Python: pytest's `-s`.

</ParamField>

<ParamField path="--co, --collect-only" type="switch">

List the tests that would run and how many (`collected N items`), then exit
with status 0 without starting a browser or touching a folder. With the cargo
runner it is `cargo test -- --list`. It cannot be combined with `--example`.

</ParamField>

<ParamField path="--reruns" type="count" default="0">

Run a failing test again this many times, each time with a new browser.
`SB_RERUNS`, key `reruns`. Python: pytest-rerunfailures' `--reruns`. Each rerun
prints `RERUN <test id> (attempt 2 of 3) after: <error>` to standard error. A
skipped test is not rerun, and neither is a Gherkin scenario. It applies to
tests that run through `Sb::run` and `SbCdp::run`; a plain `#[test]` is not
rerun. `sbrs test` does not use nextest's `--retries`, because its delay can
only be set in a file.

</ParamField>

<ParamField path="--reruns-delay" type="seconds" default="0">

Seconds to wait between reruns. `SB_RERUNS_DELAY`, key `reruns_delay`.

</ParamField>

<ParamField path="--time-limit, --timelimit" type="seconds">

Fail a test that runs longer than this many seconds, with `This test has
exceeded the time limit of N seconds!`, and close its browser. `SB_TIME_LIMIT`,
key `time_limit`. Python: `--time-limit` and `time_limit` in `SB(...)`. Time
spent at a debug prompt does not count. A Gherkin scenario cannot be stopped
halfway: its limit is the browser's own, so the next open, wait or action fails
once it has run out.

</ParamField>

<ParamField path="--timeout-multiplier" type="number">

Multiply the default waits by this number: the default wait becomes
`ceil(multiplier * seconds)`, a multiplier below 0.5 is raised to 0.5, and a
timeout a call names is never multiplied. `SB_TIMEOUT_MULTIPLIER`, key
`timeout_multiplier`. Python: `--timeout-multiplier`.

</ParamField>

<ParamField path="--runner" type="cargo or nextest" default="cargo">

The test runner. `SB_TEST_RUNNER`, key `runner` in the `[test]` table. `nextest`
runs `cargo nextest`, so it needs `cargo-nextest` installed (`cargo install
cargo-nextest --locked`). Another value exits with status 2: `invalid runner
'x': choose from cargo, nextest`.

</ParamField>

<ParamField path="--all-scripts" type="switch">

After the tests, run every example of the tested packages with
`cargo run --example NAME` in the package folder, with the environment of the
tests. An example whose `required-features` are not all on is skipped, and each
gets 300 seconds (or `SB_TIME_LIMIT`). It prints `example NAME ... ok` or
`example NAME ... FAILED (status N)` and a status other than 0 when one failed.
It cannot be combined with `--test` or `--example`. Python: `--all-scripts`.

</ParamField>

<ParamField path="-c, --settings-file, --settings" type="path">

The settings file the tests read instead of `sbase_config.toml`. `SB_SETTINGS_FILE`,
key `settings_file`. A relative path is made absolute first, because a test's
working directory is its package's.

</ParamField>

</Accordion>

<Accordion title="Logs, evidence and reports" icon="file-lines">

A failing test leaves `latest_logs/<test id>/` with `screenshot.png`,
`page_source.html` and `basic_test_info.txt`. [Writing Tests with the sb
Fixture](/tutorials/writing_tests#reports-of-a-run) describes what each report
holds.

<ParamField path="--screenshot, --save-screenshot, --ss" type="switch">

Save the screenshot and page source of passing tests too. `SB_SAVE_SCREENSHOT`,
key `save_screenshot`. Python: `--screenshot`, `--save-screenshot`, `--ss`.

</ParamField>

<ParamField path="--no-screenshot, --ns" type="switch">

Save no screenshot of failing tests. `SB_NO_SCREENSHOT`, key `no_screenshot`. It
wins over `--screenshot`, as in Python.

</ParamField>

<ParamField path="--list-fail-page, --list-fail-pages, --lfp" type="switch">

Print `=> Fail Page: <url>` after a failing test. `SB_LIST_FAIL_PAGE`, key
`list_fail_page`.

</ParamField>

<ParamField path="--archive-logs" type="switch">

Move `latest_logs/` to `archived_logs/logs_<unix seconds>/` instead of emptying
it, when `sbrs test` starts. `SB_ARCHIVE_LOGS`, key `archive_logs`. Only
`sbrs test` does it (see the warning above).

</ParamField>

<ParamField path="--archive-downloads" type="switch">

Move `downloaded_files/` to `archived_files/downloads_<unix seconds>/` instead
of emptying it. `SB_ARCHIVE_DOWNLOADS`, key `archive_downloads`. The operating
system's Downloads folder is never touched. A name that exists gets a `-1`,
`-2`, ... suffix.

</ParamField>

<ParamField path="--dashboard" type="switch">

Write `dashboard.html`, live while the tests run and final when they end.
`SB_DASHBOARD`, key `dashboard`.

</ParamField>

<ParamField path="--dash-title" type="string">

The title of the dashboard and the HTML report. `SB_DASH_TITLE`, key
`dash_title`; an underscore stands for a space.

</ParamField>

<ParamField path="--html" type="path">

Write an HTML report to this file. `SB_HTML`, key `html`. A relative path is
made absolute first.

</ParamField>

<ParamField path="--junit-xml, --junitxml" type="path">

Write a JUnit XML report to this file. `SB_JUNIT_XML`, key `junit_xml`.

</ParamField>

<ParamField path="--alluredir, --allure-dir" type="path">

Write Allure results to this folder. `SB_ALLURE_DIR`, key `allure_dir`. The tested
crate needs the feature `allure` of `seleniumbase-rs`.

</ParamField>

<ParamField path="--show-report" type="switch">

When the tests end, open the HTML report (else the dashboard) in a browser window
and wait until it is closed. It needs `--html` or `--dashboard`, and exits with
status 2 otherwise. It has no variable.

</ParamField>

<ParamField path="--with-db-reporting" type="switch">

Record each test's outcome in the results database. `SB_WITH_DB_REPORTING`, key
`with_db_reporting`. It needs an `sbrs` built with the `turso` feature and exits
with status 2 without it. `sbrs report history` reads the database.

</ParamField>

<ParamField path="--database-env" type="string">

Which results database to use: the `[test.databases]` table maps a name to a
path. `SB_DATABASE_ENV`, key `database_env`.

</ParamField>

<ParamField path="--with-s3-logging" type="switch">

Upload each test's logs to S3. `SB_WITH_S3_LOGGING`, key `with_s3_logging`.
When a test ends, its `latest_logs/<test id>/` folder goes to the bucket
`SB_S3_BUCKET` names (region `SB_S3_REGION`, else `AWS_REGION`, else
`us-east-1`; key prefix `SB_S3_PREFIX`). It needs the feature `s3` of
`seleniumbase-rs` in the tested crate; without it, or without a bucket, it logs a
warning and does nothing.

</ParamField>

</Accordion>

<Accordion title="Run settings and page behaviour" icon="sliders">

Each of these becomes a setting of the run (`RunSettings`) that the fixture
applies to the `BaseCase`. See [Run settings](/tutorials/run_settings).

<ParamField path="--highlights" type="count">

How many times to flash a highlighted element. `SB_HIGHLIGHTS`, key `highlights`.
It sets the loop count of every highlight, over the count a call names.

</ParamField>

<ParamField path="--interval" type="seconds">

Seconds between the steps of a tour or presentation, which then move on by
themselves. `SB_INTERVAL`, key `interval`.

</ParamField>

<ParamField path="--verify-delay" type="seconds">

Seconds to wait before each MasterQA verification. `SB_VERIFY_DELAY`, key
`verify_delay`.

</ParamField>

<ParamField path="--check-js" type="switch">

Check each page for JavaScript errors after it opens (Chromium). `SB_CHECK_JS`,
key `check_js`. Python: `--check-js`, and `js_checking_on` in `SB(...)`.

</ParamField>

<ParamField path="--wait-for-angularjs, --wfa" type="switch">

Wait for AngularJS after opening a page and clicking (up to 2 seconds).
`SB_WAIT_FOR_ANGULARJS`, key `wait_for_angularjs`.

</ParamField>

<ParamField path="--disable-beforeunload" type="switch">

Remove the `beforeunload` prompt after each page opens (Chromium).
`SB_DISABLE_BEFOREUNLOAD`, key `disable_beforeunload`.

</ParamField>

<ParamField path="--maximize, --maximize-window, --fullscreen" type="switch">

Maximize the browser window. `SB_MAXIMIZE`, key `maximize`.

</ParamField>

<ParamField path="--crumbs" type="switch">

Delete the cookies when a reused browser is handed to the next test.
`SB_CRUMBS`, key `crumbs`. Without it a reused browser keeps its cookies, as in
Python.

</ParamField>

<ParamField path="--demo, --demo-mode" type="switch">

Highlight each element and pause after an action. `SB_DEMO`, key `demo`.

</ParamField>

<ParamField path="--slow, --slow-mode, --slowmo" type="switch">

Pause briefly after each action. `SB_SLOW`, key `slow`.

</ParamField>

<ParamField path="--demo-sleep" type="seconds">

Seconds to pause after each action in demo mode. `SB_DEMO_SLEEP`, key
`demo_sleep`.

</ParamField>

<ParamField path="--message-duration" type="seconds">

Seconds a message stays on the page. `SB_MESSAGE_DURATION`, key
`message_duration`.

</ParamField>

<ParamField path="--skip-js-waits, --sjw" type="switch">

Do not wait for the page's JavaScript to settle. `SB_SKIP_JS_WAITS`, key
`skip_js_waits`.

</ParamField>

<ParamField path="--visual-baseline" type="switch">

Set the visual baselines again instead of comparing. `SB_VISUAL_BASELINE`, key
`visual_baseline`.

</ParamField>

</Accordion>

<Accordion title="Recorder" icon="circle-dot">

With any of `--rec`, `--rec-print`, `--rec-behave` and `--rec-sb-mgr` on,
`sbrs test` runs one test at a time, because the recorder's breakpoint asks a
person. See [Recorder Mode](/tutorials/recorder_mode).

<ParamField path="--rec, --record, --recorder, --codegen" type="switch">

Record the browser actions as a test. `SB_RECORDER`, key `recorder`. Python: the
four flags, and `recorder_ext` in `SB(...)`. When the test ends the recording is
written to `recordings/<test>_rec.rs`.

</ParamField>

<ParamField path="--rec-print" type="switch">

Print the recording when the test ends. `SB_REC_PRINT`, key `rec_print`.

</ParamField>

<ParamField path="--rec-sb-mgr" type="switch">

Write the recording as a script, not a test. `SB_REC_SB_MGR`, key `rec_sb_mgr`.

</ParamField>

<ParamField path="--rec-behave, --rec-gherkin" type="switch">

Also write the recording as a Gherkin feature. `SB_REC_BEHAVE`, key
`rec_behave`.

</ParamField>

<ParamField path="--rec-sleep, --record-sleep" type="switch">

Keep the sleeps of a recording. `SB_REC_SLEEP`, key `rec_sleep`.

</ParamField>

<ParamField path="--rec-sb-cdp" type="switch">

Write recordings as pure CDP code. `SB_REC_SB_CDP`, key `rec_sb_cdp`.

</ParamField>

</Accordion>

<Accordion title="Browser and page options" icon="browser">

The settings of the browser, as the settings file and the `SB_*` variables
carry them everywhere else (see [Settings and
Configuration](/tutorials/settings_and_config) and [Customizing Test
Runs](/help/customizing_test_runs)). "Applies to" says which browsers read the
option; the others accept the flag and ignore it. The key is the name in the
settings file, and Python's `SB(...)` keyword is the key unless the last column
names another. A switch takes `--flag=false`.

**Which browser**

| Flags | Variable | Key | Meaning |
|---|---|---|---|
| `--browser BROWSER` | `SB_BROWSER` | `browser` | `chrome`, `chromium`, `edge`, `firefox`, `brave`, `comet`, `opera`, `safari`, `ie`, `cft` or `chs` |
| `--chrome`, `--edge`, `--firefox`, `--brave`, `--comet`, `--opera`, `--safari`, `--ie`, `--cft`, `--chs`, `--chromium` (also `--use-chromium`) | `SB_BROWSER` | `browser` | Shorthand for `--browser NAME`; `--chromium` is also `use_chromium` in `SB(...)` |
| `--mode MODE` | `SB_MODE` | `mode` | `webdriver`, `cdp` or `uc` |
| `--uc` (also `--undetected`, `--undetectable`) | `SB_MODE` | `mode` | Shorthand for `--mode uc`; Python keywords `uc`, `undetected`, `undetectable` |
| `--headless` (also `--headless2`) | `SB_HEADLESS` | `headless` | Chrome's new headless mode |
| `--headed` (also `--gui`) | `SB_HEADLESS` | `headless` | Shorthand for `SB_HEADLESS=false`; Python keyword `headed` |
| `--headless1` | `SB_HEADLESS1` | `headless1` | Chrome's old headless mode, which runs as chrome-headless-shell (Chromium) |
| `--binary-location PATH` (also `--bl`) | `SB_CHROME_BIN` | `chrome_bin` | The browser binary to run; Python keyword `binary_location` |
| `--driver-version VERSION` | `SB_DRIVER_VERSION` | `driver_version` | A number, `latest`, `stable`, `beta`, ... |
| `--proxy-driver` | `SB_PROXY_DRIVER` | `proxy_driver` | Download the driver through the proxy |
| `--xvfb`, `--xvfb-metrics WIDTH,HEIGHT` | `SB_XVFB`, `SB_XVFB_METRICS` | `xvfb`, `xvfb_metrics` | A virtual display for a headed browser (Linux only) |

**Window, phone and page**

| Flags | Variable | Key | Meaning |
|---|---|---|---|
| `--window-size WIDTH,HEIGHT` | `SB_WINDOW_SIZE` | `window_size` | Python keyword `size` too |
| `--window-position X,Y` | `SB_WINDOW_POSITION` | `window_position` | Python keyword `position` too |
| `--mobile` (also `--mobile-emulator`) | `SB_MOBILE` | `mobile` | Emulate a mobile phone (Chromium); Python keyword `is_mobile` |
| `--device-metrics WIDTH,HEIGHT,PIXEL_RATIO` (also `--metrics`) | `SB_DEVICE_METRICS` | `device_metrics` | Emulate a phone screen (Chromium) |
| `--locale LOCALE` | `SB_LOCALE` | `locale` | The browser locale |
| `--locale-code CODE` | `SB_LOCALE_CODE` | `locale_code` | A language code such as `fr` or `ar_eg`; it wins over `--locale` |
| `--user-agent TEXT` (also `--agent`, `-a`) | `SB_USER_AGENT` | `user_agent` | The user agent string |
| `--start-page URL` (also `--url`) | `SB_START_PAGE` | `start_page` | The page every browser opens first |
| `--page-load-strategy STRATEGY` (also `--pls`) | `SB_PAGE_LOAD_STRATEGY` | `page_load_strategy` | `normal`, `eager` or `none` |
| `--user-data-dir DIR` | `SB_USER_DATA_DIR` | `user_data_dir` | The browser profile folder |
| `--extension-dir DIR` | `SB_EXTENSION_DIR` | `extension_dir` | Unpacked extension folders, separated by commas |
| `--extension-zip LIST` (also `--crx`) | `SB_EXTENSION_ZIP` | `extension_zip` | Extension `.zip` or `.crx` files, separated by commas |
| `--esc-end` (also `--ee`) | `SB_ESC_END` | `esc_end` | End the current test when ESC is pressed in the browser |
| `--fingerprint PATH` | `SB_FINGERPRINT` | `fingerprint.file` | A fingerprint file every browser presents |

**Network**

| Flags | Variable | Key | Meaning |
|---|---|---|---|
| `--proxy VALUE` (also `--proxy-server`, `--proxy-string`) | `SB_PROXY` | `proxy` | `HOST:PORT` or `USER:PASS@HOST:PORT` |
| `--proxy-pac-url URL` (also `--pac-url`) | `SB_PROXY_PAC_URL` | `proxy_pac_url` | A proxy auto-config URL |
| `--proxy-bypass-list LIST` | `SB_PROXY_BYPASS_LIST` | `proxy_bypass_list` | Hosts that skip the proxy |
| `--ad-block` (also `--adblock`, `--block-ads`) | `SB_AD_BLOCK` | `ad_block` | Block the requests of ad and tracker hosts (Chromium); Python keyword `ad_block_on` too |
| `--server TEXT`, `--port PORT`, `--protocol TEXT` | `SB_SERVER`, `SB_PORT`, `SB_PROTOCOL` | `server`, `port`, `protocol` | The Selenium Grid to connect to; Python keyword `servername` for the server |
| `--host-resolver-rules TEXT` | `SB_HOST_RESOLVER_RULES` | `host_resolver_rules` | Chromium rules that map host names to addresses |
| `--disable-csp` (also `--dcsp`, `--no-csp`) | `SB_DISABLE_CSP` | `disable_csp` | Strip the Content Security Policy from every response (Chromium) |

**Chromium switches and preferences**

| Flags | Variable | Key | Meaning |
|---|---|---|---|
| `--chromium-arg LIST` | `SB_CHROMIUM_ARG` | `chromium_arg` | Extra switches, separated by commas |
| `--disable-features LIST` | `SB_DISABLE_FEATURES` | `disable_features` | Features to switch off, separated by commas |
| `--disable-gpu` | `SB_DISABLE_GPU` | `disable_gpu` | Switch the GPU off |
| `--enable-3d-apis` | `SB_ENABLE_3D_APIS` | `enable_3d_apis` | Keep the GPU on in a headless session, for WebGL |
| `--swiftshader` | `SB_SWIFTSHADER` | `swiftshader` | Render with SwiftShader, a software WebGL renderer |
| `--no-sandbox` | `SB_NO_SANDBOX` | `no_sandbox` | Start Chromium without its sandbox |
| `--disable-ws` (also `--dws`, `--disable-web-security`) | `SB_DISABLE_WS` | `disable_ws` | Switch web security off |
| `--dark-mode` (also `--dark`) | `SB_DARK_MODE` | `dark_mode` | Force Chrome's dark theme |
| `--devtools` (also `--open-devtools`) | `SB_DEVTOOLS` | `devtools` | Open the developer tools with each tab (headed only) |
| `--guest-mode` (also `--guest`) | `SB_GUEST_MODE` | `guest_mode` | Open the browser as a guest |
| `--incognito` (also `--incognito-mode`) | `SB_INCOGNITO` | `incognito` | Open the browser in an incognito window |
| `--use-auto-ext` (also `--auto-ext`) | `SB_USE_AUTO_EXT` | `use_auto_ext` | Keep Chrome's automation extension |
| `--remote-debug` (also `--remote-debugger`) | `SB_REMOTE_DEBUG` | `remote_debug` | Open Chrome's remote debugger on port 9222 |
| `--log-cdp-events` (also `--log-cdp`) | `SB_LOG_CDP_EVENTS` | `log_cdp_events` | Log the DevTools Protocol events |
| `--uc-cdp-events` (also `--uc-cdp`) | `SB_UC_CDP_EVENTS` | `uc_cdp_events` | The same, in undetected mode only |
| `--block-images` | `SB_BLOCK_IMAGES` | `block_images` | Do not load images |
| `--disable-js` | `SB_DISABLE_JS` | `disable_js` | Turn JavaScript off |
| `--disable-cookies` | `SB_DISABLE_COOKIES` | `disable_cookies` | Refuse every cookie |
| `--do-not-track` | `SB_DO_NOT_TRACK` | `do_not_track` | Send the `DNT: 1` header |
| `--external-pdf` | `SB_EXTERNAL_PDF` | `external_pdf` | Download PDF files instead of showing them |
| `--enable-sync` | `SB_ENABLE_SYNC` | `enable_sync` | Leave Chrome Sync on |

**Sessions, Firefox and capabilities**

| Flags | Variable | Key | Meaning |
|---|---|---|---|
| `--reuse-session` (also `--rs`) | `SB_REUSE_SESSION` | `reuse_session` | Share browsers among the tests of the run |
| `--reuse-class-session` (also `--rcs`) | `SB_REUSE_CLASS_SESSION` | `reuse_class_session` | Share a browser among the tests of one module |
| `--firefox-arg LIST` | `SB_FIREFOX_ARG` | `firefox_arg` | Firefox arguments, separated by commas |
| `--firefox-pref LIST` | `SB_FIREFOX_PREF` | `firefox_pref` | Firefox preferences, `NAME:VALUE` separated by commas |
| `--cap-file PATH` | `SB_CAP_FILE` | `cap_file` | A JSON file of capabilities to add to the session's |
| `--cap-string JSON` | `SB_CAP_STRING` | `cap_string` | A JSON object of capabilities; it wins over `--cap-file` |

</Accordion>

<Accordion title="Accepted and ignored" icon="ban" id="accepted-and-ignored">

These spellings parse and do nothing, so a command line written for Python
keeps working. They are `not-applicable` in the parity table.

| Flags | Why they do nothing |
|---|---|
| `--with-testing-base`, `--with-basic-test-info`, `--with-page-source`, `--with-screen-shots` | Always on in the `sb` fixture, as upstream |
| `--log-path` | Deprecated upstream; logs always go to `latest_logs/` |
| `--with-selenium` | Deprecated upstream; always on |
| `--is-pytest` | An internal pytest flag |
| `--uc-sub`, `--uc-subprocess` | Deprecated upstream; undetected mode always uses a subprocess |
| `--enable-ws`, `--enable-web-security` | Web security is never switched off unless asked |
| `--fast`, `--fast-mode` | The pure CDP engine makes no wait to skip |
| `--multi-proxy` | Each pure CDP browser answers its own proxy login, so no switch is needed |

Two keywords of Python's `SB(...)` have no flag: `raise_test_failure` (`rtf`),
because `Sb::run` always returns the error and the caller decides, and `test`,
because `Sb::run` is `SB(test=True)` and `run_browser_test` is the form that is
not.

</Accordion>

</AccordionGroup>

## The behave options

`sbrs behave-options` prints how Python's behave options map to `sbrs test`,
because Gherkin scenarios are `cargo test` tests now (see [Gherkin scenarios
with rstest-bdd](/tutorials/behave)).

| behave | `sbrs test` |
|---|---|
| `-D NAME[=VALUE]` | The flag of the same name: `-D headless` is `sbrs test --headless`, `-D env=qa` is `sbrs test --env qa`. The names are `browser`, `headless`, `demo`, `slow`, `rs`, `crumbs`, `maximize`, `dashboard`, `incognito`, `guest`, `dark`, `uc`, `pdb`, `recorder`, `rec-print`, `save-screenshot`, `archive-logs`, `check-js`, `start-page`, `agent`, `mobile`, `metrics`, `ad-block`, `settings-file`, `env`, `data`, `disable-csp`, `remote-debug`, `server`, `port` and `proxy` |
| `--dry-run`, `-d` | `sbrs test --co` |
| `--stop` | `sbrs test --runner nextest`: nextest stops at the first failure by default, and cargo stops after the first failing test binary |
| `-q`, `--no-snippets` | Not applicable: rstest-bdd checks every step at compile time |
| `--tags` | `scenarios!(dir, tags = "...")` at compile time, or `sbrs test -m` with a module name |

## Next steps

<Columns cols={3}>

<Card color="#1a85d4" title="Debug modes" icon="bug" horizontal href="/tutorials/debug_modes">

Stop in a prompt with the browser open.

</Card>

<Card color="#199f43" title="Writing Tests with the sb Fixture" icon="flask" horizontal href="/tutorials/writing_tests">

Reruns, time limits, evidence and reports.

</Card>

<Card color="#d47628" title="CLI reference" icon="terminal" horizontal href="/help/cli_reference">

Every flag of every `sbrs` command.

</Card>

</Columns>
