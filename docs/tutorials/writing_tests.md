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

Summary for AI exports: enable the feature `testing` and add `rstest` and `tokio` (features `macros`, `rt`) as dev-dependencies. Write `#[rstest] #[tokio::test] async fn name(sb: Sb) -> Result<()> { sb.run(async |sb| { ... }).await }` with `use seleniumbase_rs::testing::{sb, Sb}`. The body is an async closure over `&mut BaseCase`; it may run again (reruns), so it must not consume what it captures. Settings come from the settings file, `SB_SETTINGS_FILE` and `SB_*` variables. Test data is read with `sb.data()`, `sb.env()`, `sb.account()`, `sb.var1()`, `sb.variables()`. A failure leaves `latest_logs/<test id>/screenshot.png`, `page_source.html` and `basic_test_info.txt`.

</Visibility>

## Set up

```toml
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing"] }
rstest = "0.26"
tokio = { version = "1", features = ["macros", "rt"] }
```

## A test

```rust
use rstest::rstest;
use seleniumbase_rs::testing::{sb, Sb};
use seleniumbase_rs::Result;

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

## Debug prompts

`trace` stops before the body, `pdb` after a failed last attempt (a
post-mortem, the browser still open), and `final_debug` after every test,
before the browser closes. Each prompt reads `c` to continue, `s` for a
screenshot, `src` for the page source, `u` for the address, `js <expression>`
and `q` to quit. Python's stepping commands (`n`, `s`, `r`) have no equivalent.

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
