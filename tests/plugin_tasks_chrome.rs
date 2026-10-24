//! The task and test hooks of the plugin API against a real headless Chrome and a local HTTP
//! server: `plugin_host::run_task_until` launches, runs a task and closes the browser, and
//! `SbCdp::run_with_browser` calls `on_test_start` and `on_test_end` with the tab still open, where
//! a plugin fails a test that passed. The tests need Chrome, so they are ignored by default. Run
//! them with:
//!
//! ```text
//! cargo test --features "testing test-util plugins" --test plugin_tasks_chrome -- --ignored
//! ```

#![cfg(all(feature = "testing", feature = "test-util", feature = "plugins"))]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use seleniumbase_rs::common::shutdown::{Outcome, Shutdown};
use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, Metadata, NoPrompt, PageHandle, Plugin, PluginSet,
    Result as PluginResult, Sink, Task, TaskContext, TestInfo, TestOutcome, TestVerdict,
};
use seleniumbase_rs::plugin_host::run_task_until;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::testing::SbCdp;
use seleniumbase_rs::{Result, SeleniumBaseError, Settings};
use serde_json::{json, Value};

type Log = Arc<Mutex<Vec<String>>>;

/// Answers every path with a small page.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let address = listener.local_addr().expect("its address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request);
            let body = "<html><body>hello</body></html>";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    format!("http://{address}")
}

/// A plugin with a task and test hooks: it fails a test whose tab ended on `/fail`.
struct Scout {
    log: Log,
}

#[async_trait]
impl Plugin for Scout {
    fn name(&self) -> &str {
        "scout"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core)
            .capability(Capability::Task)
            .capability(Capability::TestHooks)
    }

    fn task(&self) -> Option<&dyn Task> {
        Some(self)
    }

    async fn on_test_start(&self, _test: &TestInfo) -> PluginResult<()> {
        self.log.lock().unwrap().push("start".to_owned());
        Ok(())
    }

    async fn on_test_end(
        &self,
        _test: &TestInfo,
        outcome: &TestOutcome,
        page: Option<&dyn PageHandle>,
    ) -> PluginResult<TestVerdict> {
        let at = match page {
            Some(page) => page.url().await?,
            None => String::new(),
        };
        self.log
            .lock()
            .unwrap()
            .push(format!("end {outcome:?} at {at}"));
        Ok(if at.ends_with("/fail") {
            TestVerdict::Fail("scout: the page ended on /fail".to_owned())
        } else {
            TestVerdict::Keep
        })
    }
}

#[async_trait]
impl Task for Scout {
    async fn run(&self, cx: &TaskContext<'_>) -> PluginResult<Value> {
        cx.sink
            .record(&json!({ "browser": cx.browser.id().as_str() }))?;
        Ok(json!({ "extensions": cx.browser.extensions().len() }))
    }
}

#[derive(Default)]
struct Lines(Mutex<Vec<Value>>);

impl Sink for Lines {
    fn record(&self, line: &Value) -> PluginResult<()> {
        self.0.lock().unwrap().push(line.clone());
        Ok(())
    }
}

fn options(log: &Log) -> Result<LaunchOptions> {
    LaunchOptions::builder()
        .headless(true)
        .plugins(
            PluginSet::new()
                .with(Scout { log: log.clone() })
                .expect("the plugin joins the set"),
        )
        .build()
}

#[tokio::test]
#[ignore = "needs a real Chrome"]
async fn a_task_runs_on_a_real_chrome_that_is_closed_afterwards() -> Result<()> {
    let log = Log::default();
    let sink = Lines::default();
    let (mut shutdown, _trigger) = Shutdown::manual();

    let outcome = run_task_until(
        &mut shutdown,
        options(&log)?,
        "scout",
        &json!({}),
        &sink,
        &NoPrompt,
    )
    .await?;

    assert_eq!(outcome, Outcome::Completed(json!({ "extensions": 0 })));
    let lines = sink.0.lock().unwrap().clone();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let id = lines[0]["browser"].as_str().expect("a browser id");
    assert!(id.starts_with("cdp:127.0.0.1:"), "{id}");
    Ok(())
}

#[tokio::test]
#[ignore = "needs a real Chrome"]
async fn the_test_hooks_see_the_real_tab_and_a_plugin_fails_a_passed_test() -> Result<()> {
    let site = serve();
    let log = Log::default();
    let logs = tempfile::tempdir().expect("a folder");
    let fixture = || {
        SbCdp::new(Settings::default())
            .logs_dir(logs.path())
            .without_report()
    };

    let passed = fixture()
        .run_with_browser(
            async || Browser::launch(options(&log)?).await,
            async |page| page.goto(format!("{site}/ok")).await,
        )
        .await;
    let failed = fixture()
        .run_with_browser(
            async || Browser::launch(options(&log)?).await,
            async |page| page.goto(format!("{site}/fail")).await,
        )
        .await;

    assert!(passed.is_ok(), "{passed:?}");
    let error = failed.expect_err("the plugin failed the test");
    assert!(
        matches!(error, SeleniumBaseError::AssertionFailed(_)),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("scout: the page ended on /fail"),
        "{error}"
    );
    assert_eq!(
        log.lock().unwrap().clone(),
        [
            "start".to_owned(),
            format!("end Passed at {site}/ok"),
            "start".to_owned(),
            format!("end Passed at {site}/fail"),
        ]
    );
    Ok(())
}
