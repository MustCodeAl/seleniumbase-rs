# Test Plugins

A test plugin runs code around a test: when it starts, when it fails (with the
browser still open, so it can take evidence), and when it finishes. The ones
that ship with the crate save a screenshot and the page source on failure and
write a report. They are the counterpart of Python SeleniumBase's pytest
plugins.

## What you will learn

- Attach the bundled plugins to a test run
- Use the same plugins with the Pure CDP engine
- Write your own plugin
- What happens when a plugin itself fails

## Attaching plugins to a test

Build a `Plugins` set and run the test through it. It works like
`run_browser_test`, and returns the same result:

```rust,no_run
use seleniumbase_rs::plugins::observer::Plugins;
use seleniumbase_rs::plugins::page_source::PageSourceOnFailurePlugin;
use seleniumbase_rs::plugins::reports::ReportPlugin;
use seleniumbase_rs::plugins::screen_shots::ScreenshotOnFailurePlugin;
use seleniumbase_rs::BrowserConfig;

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let mut plugins = Plugins::new()
    .with(ScreenshotOnFailurePlugin::new("latest_logs"))
    .with(PageSourceOnFailurePlugin::new("latest_logs"))
    .with(ReportPlugin::json("latest_logs/report.json"));

plugins
    .run_browser_test("login_works", BrowserConfig::default(), |sb| {
        Box::pin(async move {
            sb.open("https://example.com").await?;
            sb.assert_title("Example Domain").await
        })
    })
    .await?;
# Ok(())
# }
```

Reuse one `Plugins` value for every test in a run. A `ReportPlugin` keeps the
results it has seen and rewrites its file after each test, so the report is
valid even if the run stops part-way.

## The bundled plugins

| Plugin | When | What it does |
|---|---|---|
| `ScreenshotOnFailurePlugin` | a test fails | Saves a PNG named after the test and the time. |
| `PageSourceOnFailurePlugin` | a test fails | Saves the page's HTML the same way. |
| `ReportPlugin::json` / `::html` | a test finishes | Rewrites a JSON or HTML report of all tests so far. |
| `ResultStorePlugin` (`turso` feature) | a test finishes | Records the test in a [`ResultStore`](results_and_profiles.md), for trends and `sbase report`. |

Details worth knowing:

- Files are never overwritten: a second failure of the same test in the same
  second gets a numbered name.
- A test name is turned into letters, digits, `-` and `_` before it is used in
  a file name, so a test called `../../etc/passwd` cannot write outside the
  directory you chose.
- If the browser cannot take the screenshot (it has crashed, say), **no file is
  written**. An earlier version wrote a text note into a `.png` file; a file that
  only pretends to be a screenshot is worse than none.

## With the Pure CDP engine

Evidence comes from anything that implements `PageEvidence`. It is implemented
for `BaseCase` and for `sb_cdp::Page`, so the same plugins work with either.
Outside `run_browser_test`, call the hooks yourself:

```rust,no_run
use seleniumbase_rs::plugins::observer::Plugins;
use seleniumbase_rs::plugins::screen_shots::ScreenshotOnFailurePlugin;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let browser = Browser::launch(LaunchOptions::default()).await?;
let page = browser.default_page().await?;
let mut plugins = Plugins::new().with(ScreenshotOnFailurePlugin::new("latest_logs"));

plugins.started("checkout").await;
let outcome = async {
    page.goto("https://example.com").await?;
    page.locator("#buy").click().await
}
.await;
if let Err(error) = &outcome {
    plugins.failed("checkout", error, &page).await; // takes the screenshot now
}
plugins.finished("checkout", std::time::Duration::ZERO, &outcome).await;
# Ok(())
# }
```

## Writing your own

Implement `TestPlugin`. Every hook is optional and returns a `Result`:

```rust
use async_trait::async_trait;
use seleniumbase_rs::plugins::observer::{PageEvidence, TestPlugin};
use seleniumbase_rs::SeleniumBaseError;

struct SlackOnFailure;

#[async_trait]
impl TestPlugin for SlackOnFailure {
    async fn test_failed(
        &mut self,
        test: &str,
        error: &SeleniumBaseError,
        page: &dyn PageEvidence,
    ) -> Result<(), SeleniumBaseError> {
        let png = page.screenshot_png().await?;
        // ... post `test`, `error` and `png` somewhere ...
        # let _ = (test, error, png);
        Ok(())
    }
}
```

## When a plugin fails

A plugin can never change how a test turns out. If a hook returns an error, it
is logged as a warning (with the plugin, hook and test name as fields) and the
remaining plugins still run. The test's own result, combined with any error
from closing the browser, is what you get back.

## Uploading what a plugin saved

`S3LoggingPlugin`, `AzureLoggingPlugin` and `GcpLoggingPlugin` are uploaders,
not hooks: call their `upload_file` after a test on the files the failure
plugins wrote. See [Cloud Integrations](cloud_integrations.md).

## Migrating from the old plugin API

Earlier releases had a `SeleniumBasePlugin` trait with `before_command` and
`after_command`, a `PluginManager`, and a `DbReportingPlugin`. Nothing ever
called the manager, so none of their hooks ran. They are removed. Use
`TestPlugin` and `Plugins`; `ReportPlugin` replaces `DbReportingPlugin` (the
same JSON row format, one row per test instead of one per command), and
`ResultStorePlugin` covers the database case.
There is no per-command hook any more: the per-test hooks are the ones with a
natural place to run.

## What has and has not been verified

The plugins, the dispatch order, failure isolation, the file naming and the
reports are covered by tests that need no browser, and the screenshot and
page-source plugins are tested through the Pure CDP mock. `Plugins::run_browser_test`
(the `BaseCase` path) has not been run against a real WebDriver on the machine
this was written on.
