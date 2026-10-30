---
title: "Writing Tests with the sb Fixture"
---

`cargo test` is the one test runner, and [rstest](https://docs.rs/rstest)
supplies the fixtures. A test asks for `sb: Sb` (WebDriver, a `BaseCase`) or
`sb_cdp: SbCdp` (pure CDP, a `Page`) and hands its body to `run`. The library
then does what Python's `sb` fixture does with the pytest options: it reads the
run configuration, reruns a failed test, stops one that runs too long, leaves the
evidence of a failure in `latest_logs/`, and closes the browser whatever
happened.

<Visibility for="agents">

Summary for AI exports: enable the feature `testing`; it is the only dev-dependency, because it re-exports the `rstest` attribute and the Tokio runtime. Write `#[rstest] #[tokio::test] async fn name(sb: Sb) -> Result<()> { sb.run(async |sb| { ... }).await }` with `use seleniumbase_rs::testing::{rstest, sb, Sb}` and `use seleniumbase_rs::{tokio, Result}`. The body is an async closure over `&mut BaseCase`; it may run again (reruns), so it must not consume what it captures. Settings come from the settings file, `SB_SETTINGS_FILE` and `SB_*` variables. Test data is read with `sb.data()`, `sb.env()`, `sb.account()`, `sb.var1()`, `sb.variables()`. A failure leaves `latest_logs/<test id>/screenshot.png`, `page_source.html` and `basic_test_info.txt`. Reports are off until asked: `sbrs test --dashboard --html PATH --junit-xml PATH --alluredir DIR` (or `SB_DASHBOARD`, `SB_HTML`, `SB_JUNIT_XML`, `SB_ALLURE_DIR`) writes `dashboard.html`, an HTML report, JUnit XML and Allure results (the Allure results need the feature `allure`); a plain `cargo test` with one of them on leaves the dashboard live until `sbrs report finish`.

</Visibility>

## Set up

```toml
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing"] }
```

That one line is the whole list. `seleniumbase_rs::testing::rstest` is the `#[rstest]`
attribute and `seleniumbase_rs::tokio` the Tokio runtime behind `#[tokio::test]`;
both attributes expand to paths that start with `rstest::` and `tokio::`, which resolve to
those two imports. [Imports and dependencies](/imports) has the full list.

## A test

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn valid_login(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com/login").await?;
        sb.type_text("#user", sb.account().unwrap_or("demo")).await?;
        sb.assert_title("Example Domain").await
    })
    .await
}
```

`sb.run` launches the browser, calls the closure with the open `BaseCase`, and
closes the browser. It returns the test's error, so `?` and the assertions of
`BaseCase` fail the test as usual; an `assert!` that panics fails it too, after
the browser is closed. The closure is called again on a rerun, so it must not
move out of what it captures.

For the pure CDP engine ask for `sb_cdp: SbCdp` and write
`sb_cdp.run(async |page| { ... })`; the body gets the first `Page`. A `Page`
has no test data of its own, so read it with `sb_cdp.data()`.

Use `#[case]` for several inputs; each case is its own test with its own id and
its own folder of evidence:

```rust
#[rstest]
#[case("qa", "https://qa.example.com")]
#[case("staging", "https://staging.example.com")]
#[tokio::test]
async fn home_page(sb: Sb, #[case] name: &str, #[case] url: &str) -> Result<()> {
    sb.run(async |sb| {
        sb.open(url).await?;
        sb.assert_element("h1").await
    })
    .await
}
```

## What the run configuration does

The fixture reads [`Settings::load`](/help/customizing_test_runs): the settings
file (`sbase_config.toml`, or the file `SB_SETTINGS_FILE` names), with the
`SB_*` variables on top. The test-related keys live in the `[test]` table and
have a variable each.

| Setting (`[test]`) | Variable | What it does |
|---|---|---|
| `reruns`, `reruns_delay` | `SB_RERUNS`, `SB_RERUNS_DELAY` | Runs a failed test again, with a new browser, after the delay in seconds. A skipped test is never rerun. Each rerun prints `RERUN <test id> (attempt 2 of 3) after: <error>` to standard error. |
| `time_limit` | `SB_TIME_LIMIT` | Stops a test that runs longer, in seconds, and fails it with `This test has exceeded the time limit of N seconds!`. Time spent at a debug prompt does not count. |
| `no_screenshot` | `SB_NO_SCREENSHOT` | Leaves the screenshot out of the evidence. |
| `save_screenshot` | `SB_SAVE_SCREENSHOT` | Saves the screenshot and page source of a passing test too. |
| `list_fail_page` | `SB_LIST_FAIL_PAGE` | Prints `=> Fail Page: <url>` after a failure. |
| `pdb`, `trace`, `final_debug` | `SB_PDB`, `SB_TRACE`, `SB_FINAL_DEBUG` | The [debug prompts](#debug-prompts). |
| `env`, `account`, `data`, `var1` to `var3`, `variables` | `SB_ENV`, `SB_ACCOUNT`, `SB_DATA`, `SB_VAR1` to `SB_VAR3`, `SB_VARIABLES` | The test data below. |

The browser settings (`headless`, `browser`, `proxy`, `dark_mode`, ...) apply
as they do everywhere else; see the [settings page](/tutorials/settings_and_config).
To change one setting for one test, the Rust form of Python's `SB(...)`
arguments, use `configure`:

```rust
#[rstest]
#[tokio::test]
async fn flaky_page(sb: Sb) -> Result<()> {
    let sb = sb.configure(|settings| {
        settings.test.reruns = 2;
        settings.test.time_limit = Some(60.0);
    });
    sb.run(async |sb| sb.open("https://example.com").await).await
}
```

## Test data

`--env`, `--account`, `--data`, `--var1` to `--var3` and `--variables` of Python
SeleniumBase are read from the `BaseCase` under the same names: `sb.env()`
(`test` when none was given, otherwise one of the 24 environments, in lower
case), `sb.environment()`, `sb.account()`, `sb.data()`, `sb.var1()`,
`sb.var2()`, `sb.var3()` and `sb.variables()`, a JSON map. `SB_VARIABLES` takes
a JSON object or the Python dictionary literal Python accepts.

## Evidence of a failure

A failed attempt leaves a folder named after the test, taken while the browser
is still open:

```text
latest_logs/cart.checkout.renders/
  screenshot.png          (not with no_screenshot)
  page_source.html
  basic_test_info.txt     test id, last page, duration, browser, time, error
```

The test id is the test binary, the module path and the function, joined with
`.`; an rstest case adds its name. It is also `sb.test_id()`. The folder is
written again by each attempt, so after a rerun it holds the last failure.

## Reports of a run

A run can write a dashboard, an HTML report, JUnit XML and Allure results. All
are off until you ask. Each is an option of `sbrs test`, an `SB_*` variable and
a key of the `[test]` table of the settings file; a flag wins over the variable,
and the variable over the file.

| Option | Variable | Writes | Needs |
|---|---|---|---|
| `--dashboard` | `SB_DASHBOARD` | `dashboard.html`: live while the tests run, the final page when the run ends | |
| `--dash-title TITLE` | `SB_DASH_TITLE` | the title of the dashboard and the report | |
| `--html PATH` | `SB_HTML` | one self-contained HTML report | |
| `--junit-xml PATH` (or `--junitxml`) | `SB_JUNIT_XML` | JUnit XML, for CI systems | |
| `--alluredir DIR` (or `--allure-dir`) | `SB_ALLURE_DIR` | Allure 2 results | the feature `allure` of `seleniumbase-rs` in the tested crate |
| `--show-report` | none | nothing: opens the HTML report (else the dashboard) in a browser window and waits until it is closed | `--html` or `--dashboard` |

A relative path is under the run's folder, the folder `sbrs test` was started in.
`--with-db-reporting` records the run in a results database, which
`sbrs report history` reads; see [Results Database and Encrypted
Profiles](/tutorials/results_and_profiles). `--archive-logs` keeps the old
`latest_logs/` in `archived_logs/`.

### With sbrs test

```bash
sbrs test --dashboard --html report.html --junit-xml report.xml --alluredir allure-results
```

`sbrs test` opens the run before Cargo (it empties `latest_logs/`, or archives
it with `--archive-logs`), prints where the dashboard is when `--dashboard` is
on, runs the tests, ends the run and prints where the reports went, in the
lines pytest prints:

```text
Dashboard: file:///path/to/project/dashboard.html
*************************************************
...the output of cargo test...
----- Generated html report: file:///path/to/project/report.html -----
----- generated xml file: /path/to/project/report.xml -----
----- Latest Logs dir: /path/to/project/latest_logs/ -----
```

When a test failed or never reported, a `----- Dashboard = file://... -----`
line comes first. The exit status is Cargo's, or 1 when a report you asked for
could not be written. The test processes write the Allure results as each test
ends, so `sbrs` has no line for them: open the folder with
`allure serve allure-results`.

### With a plain cargo test

```bash
SB_DASHBOARD=1 SB_HTML=report.html SB_JUNIT_XML=report.xml cargo test --features testing
sbrs report finish
```

With any of these on, the `sb`, `sb_cdp` and Gherkin fixtures record every test
in one run shared by all the test binaries of that `cargo test` (or nextest) run.
A test is untested on the dashboard while it runs, and each test that ends
rewrites the reports, so they are complete after every test. The standard test
harness cannot tell when the last test has ended, so the dashboard stays live
(it refreshes itself every 12 seconds) until you run `sbrs report finish`. It
ends the run in `SB_REPORT_ROOT`, else in the current folder: tests that
started and never ended become unreported, the reports are written a last time,
and the logs are archived when asked. In a folder with no run it prints a
message and exits with status 1.

| Flag of `sbrs report finish` | Effect |
|---|---|
| `--dashboard`, `--dash-title TITLE`, `--html PATH`, `--junit-xml PATH`, `--archive-logs` | replace the setting the run was started with |
| `--show-report` | open the HTML report or the dashboard and wait until it is closed |
| `--json` | print what was done as JSON instead of the `-----` lines |

`--json` prints `summary` (`total`, `passed`, `failed`, `skipped`, `untested`,
`unreported`, `duration_secs`), `dashboard`, `html`, `junit_xml`, `archived`,
`logs_dir`, `database_run` and `warnings`. There is no `--alluredir` here: the
Allure results are already written.

### What each output holds

**Dashboard.** `dashboard.html` in the run's folder follows the layout of
Python SeleniumBase's: a pie chart, a row of counts (`Failed`, `Skipped`,
`Passed`, `Untested`, `Total`) and a table of `Result`, `Test`, `Duration` and
`Links`, with a `Details` column for the error of a failed test. `Logs` and
`Data` lead to `latest_logs/` and to the test's own folder in it. While the run
is live the page refreshes itself and says it is awaiting results; when the run
ends the same file is the final report, with every test that never reported
shown as `UNREPORTED`. The page needs no script and no network.

**HTML report.** One self-contained page for a finished run: a summary line and
the same table. It has the pie chart only when the dashboard is on too. When a
run ends, the report is also kept as `last_report.html` in the run's folder.

**JUnit XML.** One `testsuite` per test binary (or per feature file of a
Gherkin scenario) and one `testcase` per test. A failure is a `failure`, a skip
a `skipped`, and a test that never ended an `error` of type `unreported`. A
failed test's `system-out` names the page it ended on and its evidence files
as `[[ATTACHMENT|path]]` lines, the form the Jenkins attachments plugin reads.
The shape of the file, with values as an illustration:

```xml
<?xml version="1.0" encoding="utf-8"?>
<testsuites name="sbrs" tests="3" failures="1" errors="0" skipped="1" time="6.120" timestamp="2026-10-24T09:30:00.000000">
  <testsuite name="login_chrome" errors="0" failures="1" skipped="1" tests="3" time="6.120" timestamp="2026-10-24T09:30:00.000000" hostname="ci-1">
    <testcase classname="login_chrome.tests" name="test_login" time="1.204"/>
    <testcase classname="login_chrome.tests" name="test_wrong_password" time="4.916">
      <failure message="..." type="...">...</failure>
      <system-out>Last Page: https://example.com/login
[[ATTACHMENT|/path/to/project/latest_logs/login_chrome.tests.test_wrong_password/screenshot.png]]</system-out>
    </testcase>
    <testcase classname="login_chrome.tests" name="test_sso" time="0.000">
      <skipped message="..."/>
    </testcase>
  </testsuite>
</testsuites>
```

**Allure results.** Add the feature to the tested crate, then give the folder:

```toml
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing", "allure"] }
```

Each test attempt writes a `<uuid>-result.json` (status, durations, labels,
the failure message and trace), the screenshot, page source and test info of a
failure as attachments, and `environment.properties` when it is missing:

```text
allure-results/
  <uuid>-result.json
  <uuid>-screenshot-attachment.png
  <uuid>-page_source-attachment.html
  <uuid>-basic_test_info-attachment.txt
  environment.properties
```

`allure serve allure-results` shows the run. A rerun is a new attempt with the same
history id, which Allure shows as a retry. The feature adds no dependency.


## Debug prompts

`trace` stops before the body, `pdb` after a failed last attempt (a
post-mortem, the browser still open), and `final_debug` after every test,
before the browser closes. `sb.breakpoint("message")` pauses the same way
anywhere in a body. The prompts look and read like
[pdbp](https://github.com/mdmintz/pdbp) (`Pdb+`), the debugger Python
SeleniumBase uses for `--pdb`, `--trace` and `breakpoint()`: a coloured label,
the code around the pause as a numbered listing, and short commands.

```text
[POST MORTEM]: The test failed.
  test:  login.valid_login   page: https://example.com/login  "Login"
> /work/shop/tests/login.rs(6) test body
   6         sb.run(async |sb| {
   7             sb.open("https://example.com/login").await?;
   8             sb.type_text("#user", "demo").await?;
   9  >>         sb.click("#go").await?;
  10             Ok(())
  11         })

  error: element not found: #go (strategy: css selector)
  step:  element not found   selector #go
Commands: c continue | r retry | s screenshot | src page source | u url | js <expr> | l list | w where | dt devtools | q quit | h help
Type 'c' and press [ENTER] to continue execution:
```

**The code.** A breakpoint lists the lines around its call and marks the call
with `->`. `trace`, `pdb` and `final_debug` list the body of the
`sb.run(async |sb| { .. })` call that holds the test (the place of that call is
recorded for you). A returned error carries no line number, so after a failure
the prompt marks with `>>` the lines of the body that name the selector or the
address the error is about, and the `error:` and `step:` lines under the
listing say what failed, with the selector and the address highlighted. The
file is looked for under the working directory and the folders above the
package; when it is not there the prompt says so and goes on.

| Command | What it does |
|---|---|
| `c`, `continue` | Goes on with the test. |
| `r`, `retry` | After a failure only: runs the test again with a new browser, once more than `reruns` allows. |
| `s`, `screenshot` | Saves the viewport as `breakpoint_<n>.png` in `latest_logs/<test id>/`. |
| `src`, `source` | Saves the page's HTML as `page_source_<n>.html` there. |
| `u`, `url` | Shows the page's address and title. |
| `js <expr>`, `pp <expr>` | Evaluates a JavaScript expression in the page and shows its value, indented. |
| `p <expr>` | The same, on one line. |
| `l`, `list` | Lists the code around the pause again, after other output has pushed it away. |
| `ll`, `longlist` | Lists more of it. |
| `w`, `where` | Shows where in the test the pause is, and the test and the page. |
| `dt`, `devtools` | Shows the address of Chrome's DevTools for the tab (the pure CDP engine; the WebDriver one has none to point at). It does not open a window. |
| `q`, `quit` | Stops the test. |
| `h`, `help`, `?` | Lists the commands. |

**Colour.** The prompt paints only when standard error is a terminal,
`NO_COLOR` is unset or empty and `TERM` is not `dumb`. A pipe, a file or a CI
capture gets the same text without escape sequences, and so does
`NO_COLOR=1`. Whatever comes from the page (its address, its title, the value
of `js`) or from a file is cleaned of control characters before it reaches the
terminal, so a page cannot move your cursor or set your window title.

Python's stepping commands (`n`, `s`, `u`, `d`) have no equivalent, and `s`
saves a screenshot where pdbp steps in. The listing is not redrawn after
every command, as pdbp's sticky mode is: type `l` to see it again.

<Warning>
A prompt needs a terminal: run `cargo test -- --test-threads 1 --nocapture`.
Where nobody can answer (CI, a pipe, `cargo nextest`) the prompt is skipped with
a warning in the log, so a flag left in a command never hangs a run.
`sb.breakpoint("message")` pauses the same way anywhere in a body.
</Warning>

## Seeing how every attempt ended

Implement `seleniumbase_rs::testing::Finalizer` and register it with
`sb.finalizer(Arc::new(MyFinalizer))` to be told, once per attempt and with the
page still open, how the attempt ended (`Outcome::Passed`, `Failed` or
`Skipped`), how long it took and which attempt it was. The dashboard, the
results database and the recorder plug in this way.

<Note>
`run_browser_test` and `run_cdp_test` stay the explicit forms that read no
configuration; use them when a test must not depend on the environment.
</Note>
