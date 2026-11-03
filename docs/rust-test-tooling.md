---
title: "Writing Browser Tests"
sidebarTitle: "Write a test"
---

{/* layout-tree-component:start */}

export const LayoutTree = ({ children }) => (
  <div className="not-prose layout-tree" style={{ margin: "1.25rem 0", fontFamily: "var(--font-sans)", fontSize: "14px", color: "var(--text-secondary)" }}>
    <style>{`
      .layout-tree summary { list-style: none; }
      .layout-tree summary::-webkit-details-marker { display: none; }
      .layout-tree .lt-row:hover { background-color: var(--surface-subtle); }
      .layout-tree .lt-fold { color: #d5a910; }
      .dark .layout-tree .lt-fold { color: #ffd452; }
      .layout-tree .lt-fold-closed { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-open { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-closed { display: flex; }
      .layout-tree .lt-hl { color: var(--primary); background-color: color-mix(in srgb, var(--primary) 8%, transparent); }
      .dark .layout-tree .lt-hl { color: var(--primary-light); }
      .layout-tree .lt-ic-gray { color: #84848a; }
      .dark .layout-tree .lt-ic-gray { color: #adadb1; }
      .layout-tree .lt-ic-blue { color: #1a85d4; }
      .dark .layout-tree .lt-ic-blue { color: #69b1ff; }
      .layout-tree .lt-ic-cyan { color: #1ca1c7; }
      .dark .layout-tree .lt-ic-cyan { color: #68cdf2; }
      .layout-tree .lt-ic-green { color: #199f43; }
      .dark .layout-tree .lt-ic-green { color: #5ecc71; }
      .layout-tree .lt-ic-indigo { color: #693acf; }
      .dark .layout-tree .lt-ic-indigo { color: #9d6afb; }
      .layout-tree .lt-ic-orange { color: #d47628; }
      .dark .layout-tree .lt-ic-orange { color: #ffa359; }
      .layout-tree .lt-ic-pink { color: #d32a61; }
      .dark .layout-tree .lt-ic-pink { color: #ff678d; }
      .layout-tree .lt-ic-purple { color: #a631be; }
      .dark .layout-tree .lt-ic-purple { color: #d568ea; }
      .layout-tree .lt-ic-red { color: #d52c36; }
      .dark .layout-tree .lt-ic-red { color: #ff6762; }
      .layout-tree .lt-ic-yellow { color: #d5a910; }
      .dark .layout-tree .lt-ic-yellow { color: #ffd452; }
    `}</style>
    {children}
  </div>
)

export const LayoutRow = ({ icon, name, note, highlight = false, button = false }) => (
  <div className={highlight ? "lt-row lt-hl" : "lt-row"} style={{ display: "flex", alignItems: "flex-start", gap: "6px", minHeight: "30px", padding: "5px 8px", margin: "0 2px", borderRadius: "6px", lineHeight: "20px", cursor: button ? "pointer" : "default" }}>
    <span style={{ width: "16px", height: "20px", flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", color: highlight ? "inherit" : "var(--text-muted)" }}>{icon}</span>
    <span style={{ whiteSpace: "nowrap", fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)", fontSize: "13px", fontWeight: 600, color: highlight ? "inherit" : "var(--text-primary)" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", fontSize: "13px", fontStyle: "italic", marginLeft: "10px", opacity: 0.9 }}>{note}</span> : null}
  </div>
)

export const LayoutFolderIcon = ({ kind }) => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    {kind === "empty" ? (
      <path fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" d="M2 4.2A1.2 1.2 0 0 1 3.2 3h3.1c.32 0 .62.13.85.35L8.4 4.6h4.4A1.2 1.2 0 0 1 14 5.8v6a1.2 1.2 0 0 1-1.2 1.2H3.2A1.2 1.2 0 0 1 2 11.8z" />
    ) : kind === "open" ? (
      <g fill="currentColor">
        <path opacity=".5" d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H13a1.5 1.5 0 0 1 1.5 1.5V7H4.2a1.5 1.5 0 0 0-1.4.95L1.5 11.3z" />
        <path d="M4.2 8h10.6a.7.7 0 0 1 .65.95l-1.6 4.1a1.5 1.5 0 0 1-1.4.95H2.2a.7.7 0 0 1-.65-.95l1.25-3.2A1.5 1.5 0 0 1 4.2 8z" />
      </g>
    ) : (
      <g fill="currentColor">
        <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H1.5z" />
        <path opacity=".6" d="M1.5 4.5V12A1.5 1.5 0 0 0 3 13.5h10a1.5 1.5 0 0 0 1.5-1.5V6A1.5 1.5 0 0 0 13 4.5z" />
      </g>
    )}
  </svg>
)

export const LayoutFileIcon = () => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    <path fill="currentColor" opacity=".5" d="M8 1v3a3 3 0 0 0 3 3h3v5.5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 12.5v-9A2.5 2.5 0 0 1 4.5 1z" />
    <path fill="currentColor" d="M9.5 1a.5.5 0 0 1 .354.146l4 4A.5.5 0 0 1 14 5.5V6h-3a2 2 0 0 1-2-2V1z" />
  </svg>
)

export const LayoutDir = ({ name, note, open = false, highlight = false, children }) =>
  children ? (
    <details open={open}>
      <summary>
        <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><span className="lt-fold-open" style={{ display: "flex" }}><LayoutFolderIcon kind="open" /></span><span className="lt-fold-closed"><LayoutFolderIcon kind="closed" /></span></span>} name={name} note={note} highlight={highlight} button />
      </summary>
      <div style={{ marginLeft: "17.5px", borderLeft: "1px solid var(--border-subtle)", paddingLeft: "6px" }}>
        {children}
      </div>
    </details>
  ) : (
    <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><LayoutFolderIcon kind="empty" /></span>} name={name} note={note} highlight={highlight} />
  )

export const layoutIconColor = (name) => {
  const n = String(name).toLowerCase()
  const ext = n.includes(".") ? n.split(".").pop() : ""
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.startsWith("docker-compose")) return "blue"
  if (["rs", "json", "jsonc", "html", "htm", "zip", "gz", "tgz", "crx", "xpi", "swift", "svg"].includes(ext)) return "orange"
  if (["ts", "tsx", "mts", "cts", "py"].includes(ext)) return "blue"
  if (["js", "mjs", "cjs", "jsx"].includes(ext)) return "yellow"
  if (["md", "mdx", "sh", "bash", "zsh", "fish", "ps1"].includes(ext)) return "green"
  if (["css", "scss"].includes(ext)) return "indigo"
  if (["png", "jpg", "jpeg", "gif", "webp", "ico"].includes(ext)) return "pink"
  if (ext === "go") return "cyan"
  return "gray"
}

export const LayoutFile = ({ name, note, highlight = false }) => (
  <LayoutRow icon={<span className={highlight ? "" : "lt-ic-" + layoutIconColor(name)} style={{ display: "flex" }}><LayoutFileIcon /></span>} name={name} note={note} highlight={highlight} />
)

{/* layout-tree-component:end */}

`seleniumbase-rs` does not ship its own test runner. `cargo test` is the one
runner and [rstest](https://docs.rs/rstest) supplies the fixtures: with the
`testing` feature the library adds `sb` (WebDriver) and `sb_cdp` (pure CDP),
which do what Python's `sb` fixture does with the pytest options. `sbrs test`
is `cargo test` (or `cargo nextest run`) with pytest's flags. This keeps tests
idiomatic and lets you use the full Rust testing ecosystem.

This page maps common testing concepts from Python and JavaScript to Rust, shows
the recommended test shape, and explains how to organize a growing suite.

<Visibility for="agents">

Summary for AI exports: write a browser test as `#[rstest] #[tokio::test] async fn name(sb: Sb) -> Result<()> { sb.run(async |sb| { .. }).await }` with `use seleniumbase_rs::testing::{rstest, sb, Sb}` and `use seleniumbase_rs::{tokio, Result}`, and the feature `testing` as the only dev-dependency. The body is an async closure over `&mut BaseCase`; it may run again on a rerun, so it must not consume what it captures. For the pure CDP engine ask for `sb_cdp: SbCdp` and call `sb_cdp.run(async |page| { .. })`. The fixture reads the settings file and the `SB_*` variables, reruns, enforces a time limit, leaves `latest_logs/<test id>/` on a failure and closes the browser. `run_browser_test(BrowserConfig::default(), async |sb| { .. })` and `run_cdp_test` are the explicit forms that read no configuration; `sb_test!` generates the same wrapper. Run with `cargo test NAME`, `cargo nextest run` or `sbrs test` (`-k`, `-m`, `-n`, `--co`, `--reruns`, `--pdb`). Put integration tests in `tests/` and shared helpers in `tests/helpers/`. Test data is read with `sb.data()`, `sb.env()`, `sb.account()`, `sb.var1()`, `sb.variables()`. A failure leaves `latest_logs/<test id>/screenshot.png`, `page_source.html` and `basic_test_info.txt`. Reports are off until asked: `sbrs test --dashboard --html PATH --junit-xml PATH --alluredir DIR` (or `SB_DASHBOARD`, `SB_HTML`, `SB_JUNIT_XML`, `SB_ALLURE_DIR`) writes `dashboard.html`, an HTML report, JUnit XML and Allure results (the Allure results need the feature `allure`); a plain `cargo test` with one of them on leaves the dashboard live until `sbrs report finish`.

</Visibility>

## Concept mapping

| Python or JavaScript concept | Rust equivalent |
|------------------------------|-----------------|
| `pytest` async plugin | `#[tokio::test]` |
| `def test(sb)` with the `sb` fixture | `#[rstest]` with `sb: Sb`, and `sb.run(..)` for the body |
| SeleniumBase `setUp` / `tearDown` | `Sb::run`, or `run_browser_test` |
| Test selection by name, `-k` and `-m` | `cargo test name`, or `sbrs test -k` and `-m` |
| Parallel test execution, `-n` | `cargo nextest run`, or `sbrs test -n` |
| `--reruns`, `--time-limit`, `--pdb` | the same flags of `sbrs test` |
| Feature-specific suites | Cargo features |
| Environment configuration | `BrowserConfig`, `sbase_config.toml`, `SB_*` env vars |
| Parameterized test cases | rstest's `#[case]` |

## Recommended test shape

Ask for the `sb` (WebDriver) or `sb_cdp` (pure CDP) fixture and hand the body of
the test to `run`. The library then does what Python's `sb` fixture does with the
pytest options: it reads the run configuration, reruns a failed test, stops one
that runs too long, leaves the evidence of a failure in `latest_logs/` and
closes the browser whatever happened. The one dev-dependency is the library with
the `testing` feature, which re-exports the `rstest` attribute and the Tokio
runtime:

```toml Cargo.toml
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing"] }
```

<Tabs>

<Tab title="WebDriver (sb)" icon="flask">

```rust tests/home.rs
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn user_can_open_the_home_page(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_element("h1").await?;
        sb.assert_text("Example Domain", "h1").await
    })
    .await
}
```

The body is an async closure over `&mut BaseCase` that returns `Result<()>`, so
`?` converts any error with a `From` impl into it, and so do the assertions of
`BaseCase`. A panic in the body fails the test too, after the browser is closed.

</Tab>

<Tab title="Pure CDP (sb_cdp)" icon="bolt">

```rust tests/home_cdp.rs
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn user_can_open_the_home_page(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp
        .run(async |page| {
            page.goto("https://example.com").await?;
            page.locator("h1").expect().to_contain_text("Example Domain").await?;
            Ok(())
        })
        .await
}
```

The body gets the first `Page` of a browser that needs no driver. A `Page` has
no test data of its own, so read it with `sb_cdp.data()`.

</Tab>

</Tabs>

The body is called again on a rerun, so it must not move out of what it
captures. For the test data (`sb.env()`, `sb.data()`, `sb.var1()`, ...), the
evidence and the reports, see [Test data](#test-data), [Evidence of a failure](#evidence-of-a-failure)
and [Reports of a run](#reports-of-a-run) below.

### Reruns, time limit and debugging

These are the options of pytest's plugins, as options of `sbrs test`, `SB_*`
variables and keys of the `[test]` table of the settings file:

| Python | Here | What it does |
|---|---|---|
| `--reruns N`, `--reruns-delay S` | `--reruns`, `--reruns-delay`, `settings.test.reruns` | Runs a failed test again with a new browser, after the delay |
| `--time-limit S` | `--time-limit`, `settings.test.time_limit` | Fails a test that runs longer, and closes its browser |
| `--pdb`, `--trace`, `--final-debug` | the same flags | A prompt with the browser open; see [Debug modes](/tutorials/debug_modes) |
| `--env`, `--data`, `--var1` | the same flags | The test data, read with `sb.env()`, `sb.data()`, `sb.var1()` |

Run the test:

```bash
cargo test user_can_open_the_home_page
sbrs test user_can_open_the_home_page --reruns 2
```

## A test with data, and several inputs

A test reads its data from the fixture. This one signs in with the account the
run was given, and falls back to `demo`:

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn valid_login(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com/login").await?;
        let user = sb.account().unwrap_or("demo").to_owned();
        sb.type_text("#user", &user).await?;
        sb.assert_title("Example Domain").await
    })
    .await
}
```

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

## Running the full suite

For small suites, the default test runner is fine. For larger suites, install and
use [cargo-nextest](https://nexte.st/). `sbrs test` runs either one and takes the
options of Python's pytest plugin (see [Test options](/help/test_options)):

<CodeGroup>

```bash cargo test
cargo test
```

```bash cargo nextest
cargo install cargo-nextest
cargo nextest run
```

```bash sbrs test
sbrs test                         # cargo test
sbrs test --runner nextest -n 4   # cargo nextest, four tests at a time
sbrs test -k "login or signup"    # select tests by name
sbrs test -m smoke                # select the tests in a module named smoke
sbrs test --co                    # list the tests, run nothing
```

</CodeGroup>

<Note>

Nextest runs each test as a separate process, which isolates browser failures
and makes parallel execution safer. The repository profile intentionally does
not retry failed tests, so flaky behavior is not hidden: use `--reruns N` on
the `sb` fixture's tests when you want a rerun. A debug prompt (`--pdb`,
`--trace`, `--final-debug`) needs the cargo runner, because nextest gives tests
no standard input; see [Debug modes](/tutorials/debug_modes).

</Note>

## Sharing helpers

As your suite grows, extract reusable helpers into modules or crates:

<View title="BaseCase (WebDriver)" icon="server">

```rust
// tests/helpers/mod.rs
use seleniumbase_rs::{BaseCase, Result};

pub async fn login(sb: &mut BaseCase, user: &str, pass: &str) -> Result<()> {
    sb.open("https://example.com/login").await?;
    sb.type_text("#username", user).await?;
    sb.type_text("#password", pass).await?;
    sb.click("#submit").await?;
    sb.assert_element_visible("#dashboard").await
}
```

`BaseCase::find_element` is still there when you want the `drivers::WebElement`
handle.

</View>

<View title="sb_cdp (pure CDP)" icon="bolt">

Use the capability traits (`BrowserApi`, `ElementApi`, `AssertionApi`,
`ScreenshotApi`) when you want helpers to work with any type that exposes the
same behavior:

```rust
use seleniumbase_rs::ElementApi;

async fn fill_email<E: ElementApi>(sb: &mut E, email: &str) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    sb.type_text("#email", email).await
}
```

Both `BaseCase` (WebDriver) and `sb_cdp::Page` (Pure CDP) implement the traits,
so the same helper runs on either. `ElementApi` no longer has `find_element`,
because its `drivers::WebElement` return type tied the trait to WebDriver.

</View>

## Parameterized tests

rstest's `#[case]` runs one body for several inputs. Each case is its own test,
with its own test id, its own folder of evidence and its own name for `-k`:

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[case("alice", "secret1")]
#[case("bob", "secret2")]
#[tokio::test]
async fn login_with_multiple_users(sb: Sb, #[case] user: &str, #[case] pass: &str) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com/login").await?;
        sb.type_text("#username", user).await?;
        sb.type_text("#password", pass).await?;
        sb.click("#submit").await?;
        sb.assert_element_visible("#dashboard").await
    })
    .await
}
```

## Test organization

A typical project layout looks like this:

<LayoutTree>
  <LayoutDir name="my_project" open>
    <LayoutFile name="Cargo.toml" />
    <LayoutFile name="sbase_config.toml" note="global defaults for every test" />
    <LayoutDir name="src">
      <LayoutFile name="main.rs" />
    </LayoutDir>
    <LayoutDir name="tests" open note="integration tests, each compiled as a separate binary">
      <LayoutDir name="helpers" note="shared helpers">
        <LayoutFile name="mod.rs" />
      </LayoutDir>
      <LayoutFile name="login.rs" />
      <LayoutFile name="checkout.rs" />
    </LayoutDir>
  </LayoutDir>
</LayoutTree>

Integration tests in `tests/` are compiled as separate binaries, which gives
each test its own browser process and reduces shared-state bugs.

## Configuration per test

For global defaults, use `sbase_config.toml`, `SB_*` environment variables or
the flags of `sbrs test` ([Test options](/help/test_options) lists them). See
[Settings and Configuration](/tutorials/settings_and_config) for details.

To change a setting for one test, which is the Rust form of Python's `SB(...)`
arguments, use `configure` on the fixture:

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn flaky_page(sb: Sb) -> Result<()> {
    let sb = sb.configure(|settings| {
        settings.headless = true;
        settings.test.reruns = 2;
        settings.test.time_limit = Some(60.0);
    });
    sb.run(async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_title_contains("Example").await
    })
    .await
}
```

For a test that must not depend on the environment at all, build a
`BrowserConfig` directly and use `run_browser_test`, which reads no
configuration:

```rust
use seleniumbase_rs::{run_browser_test, tokio, BrowserConfig, DriverMode, Result};

#[tokio::test]
async fn uc_smoke_test() -> Result<()> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(true);

    run_browser_test(config, async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_title_contains("Example").await
    })
    .await
}
```

## Macros for shorter tests

The `sb_test!` macro generates a `#[tokio::test]` wrapper and the
`run_browser_test` boilerplate:

```rust
use seleniumbase_rs::{sb_test, BrowserConfig};

sb_test!(home_page_loads, BrowserConfig::default(), |sb| {
    sb.open("https://example.com").await?;
    sb.assert_text("Example Domain", "h1").await
});
```

This expands to the same shape as the manual example above. It is optional but
useful for linear tests where the reduced noise improves readability.

See [Macros](/tutorials/macros) for the full macro catalog.

## Common pitfalls

<AccordionGroup>

<Accordion title="Using the old Box::pin closure" icon="code">

`run_browser_test` takes an async closure now, so `|sb| Box::pin(async move { .. })`
does not compile ("implementation of `AsyncFnOnce` is not general enough"). Write
`async |sb| { .. }`.

</Accordion>

<Accordion title="Moving out of what the body captures" icon="recycle">

The body of `sb.run` is called again on a rerun, so it must not consume what it
captures. Borrow the case's parameters (`&str`, a `&String`) or clone them
inside the closure; a body that moves a captured `String` out by value does not
compile.

</Accordion>


<Accordion title="Mixing sync and async cleanup" icon="arrows-rotate">

`BaseCase::quit` is async. Do not rely on `Drop` for cleanup; use `Sb::run`
(or `run_browser_test`) or call `quit` explicitly.

</Accordion>

<Accordion title="Heavy setup in every test" icon="weight-hanging">

If every test logs in, consider a helper or a shared session strategy, but be
aware that shared browsers can leak state.

</Accordion>

<Accordion title="Too much parallelism" icon="gauge-high">

Browsers are expensive. Start with low concurrency and increase only after
measuring.

</Accordion>

</AccordionGroup>

## Testing checklist

- [ ] Tests ask for the `sb` or `sb_cdp` fixture and hand their body to `run`, so the browser closes whatever happens.
- [ ] Helpers are extracted into `tests/helpers/` or a shared crate.
- [ ] Configuration is externalized for environment differences (the settings file, `SB_*` variables, `sbrs test` flags).
- [ ] The suite is run with `sbrs test`, `cargo test` or `cargo nextest run`.
- [ ] A failing test leaves its screenshot, page source and test info in `latest_logs/<test id>/`: the fixture does it, so look there first.
