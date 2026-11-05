---
title: "Testing fixtures"
sidebarTitle: "Testing fixtures"
description: "The sb and sb_cdp fixtures: Sb, SbCdp, and run, behind the testing feature."
---

<Note>

These types are available with the Cargo feature `testing` (`seleniumbase_rs::testing`). They are not `BaseCase` methods. Names below are the public items. Argument names and return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

`cargo test` is the runner. A test asks for `sb: Sb` or `sb_cdp: SbCdp` with `#[rstest]` and puts the steps in `run`. The handle holds the settings and the test id. `run` owns the browser: it launches, applies the run configuration, and closes the browser on every path, including a panic. `Drop` cannot do that, so do not start the browser yourself and expect the fixture to close it.

The feature re-exports `rstest` and `fixture`. Import `seleniumbase_rs::tokio` for `#[tokio::test]`. A test crate lists `seleniumbase-rs` with feature `testing` and no `rstest` or `tokio` line of its own.

Without the feature, `run_browser_test(config, async |sb| { .. })` and `run_cdp_test(options, async |page| { .. })` launch, run and close. They do not apply reruns, the time limit, failure evidence or the debug prompts.

## What `run` does

Settings come from `Settings::load`: the settings file, `SB_SETTINGS_FILE`, then the `SB_*` variables (`sbrs test` sets those from its flags). A test can change them with `configure`.

| Run configuration | What `run` does with it |
|-------------------|-------------------------|
| Browser options, test data, run settings | Passed into the browser the attempt starts. |
| `reruns`, `reruns_delay` | Calls the closure again after a failure. The closure must not consume what it captures. |
| Time limit | Ends a hung attempt. |
| Screenshot settings | On a failure, writes `latest_logs/<test id>/` with `screenshot.png`, `page_source.html` and `basic_test_info.txt` while the browser is still open. |
| `trace`, `pdb`, `final_debug` | Debug prompts. The place of the `run` call is the spot they show. |
| Dashboard, HTML, JUnit, Allure, database reports | Records the test when those settings are on, unless `without_report` or `report_to` says otherwise. |

`run` returns the settings error, the launch error, the last attempt's error, the time-limit error, or the cleanup error. The panic of the last attempt is raised again after the browser closes.

## `Sb` and `sb`

`sb()` is the `#[rstest]` fixture. It is `Sb::from_env()`. The closure receives `&mut BaseCase`.

| Method | Description |
|--------|-------------|
| `Sb::from_env()` | Settings from the environment and the id of the test on this thread. A bad settings file does not fail here; `run` returns it. |
| `Sb::new(settings)` | These settings instead of the environment. |
| `configure(change)` | `change` is `FnOnce(&mut Settings)`. The Rust form of per-test `SB(...)` arguments. |
| `report_to(run)` | Record in this `TestRun` instead of the process run. |
| `without_report()` | Record in no run. |
| `logs_dir(dir)` | Failure evidence under `dir` instead of `latest_logs`. |
| `finalizer(finalizer)` | Tell this `Finalizer` how every attempt ended, with the page still open. |
| `test_id()` | Binary, module path and function joined with `.`, such as `login.valid_login`. It names the evidence folder. |
| `settings()` | `&Settings`, or `InvalidConfig` when they could not be read. |
| `run(test)` | `test` is `AsyncFn(&mut BaseCase) -> Result<()>`. Returns a future; await it. Not itself an `async fn`, so the caller location is kept for the debug prompts. |

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn valid_login(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_title("Example Domain").await
    })
    .await
}
```

`Sb::from_env().configure(|s| s.test.reruns = 2)` is a per-test override. Inside the closure the name `sb` is the `BaseCase`, so `sb.account()`, `sb.data()` and `sb.variables()` are the case's methods.

## `SbCdp` and `sb_cdp`

`sb_cdp()` is `SbCdp::from_env()`. `run` launches Chrome over CDP and passes the first `Page`. `from_env`, `new`, `configure`, `report_to`, `without_report`, `logs_dir`, `finalizer`, `test_id` and `settings` match `Sb`.

| Method | Description |
|--------|-------------|
| `data()` | The run's test data (`--env`, `--data`, `--var1`, ...). Empty when the settings could not be read. A `Page` has none of its own. |
| `run(test)` | `test` is `AsyncFn(&Page) -> Result<()>`. Launches with `settings.to_launch_options()`. |
| `run_with_browser(launch, test)` | Same lifecycle on the browser `launch` returns, once per attempt. Feature `test-util` (and the crate's own tests). Use it for `Browser::new_mocked()`. |

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn heading(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp
        .run(async |page| {
            page.goto("https://example.com").await?;
            page.locator("h1").click().await
        })
        .await
}
```

## `Finalizer`

Register one with `finalizer`. It does not change the test's result and returns no error.

| Item | Description |
|------|-------------|
| `begin(test_id)` | Once, before the first attempt starts a browser. Default does nothing. |
| `finish(end, page)` | Once per attempt that had a page, after the evidence and before the browser closes. |
| `launch_failed(end)` | Instead of `finish`, when the browser did not start. |
| `AttemptEnd` | `test_id`, `attempt` (from 1), `attempts` (reruns plus the first run), `outcome`, `duration`. |
| `Outcome::Passed` | The attempt passed. |
| `Outcome::Failed(error)` | An error, a panic or a time limit. |
| `Outcome::Skipped(error)` | The test skipped itself. The error holds the reason. |

An attempt that will run again has `attempt < attempts`. The last attempt is the test's result. The page argument is still open.
