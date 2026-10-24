//! `plugin_host::run_task_on` and `run_task_until`: the task of a plugin run on a scripted browser,
//! with no Chrome.
//!
//! Each test reads back what the plugin wrote to the sink and what its close hooks saw, so it
//! checks that the task got its input, that its results survive a failure, and that the browser
//! is closed (the plugin hears `before_close` and `on_close`) however the task ends. Run them with
//! `cargo test --features "test-util plugins" --test plugin_tasks`.

#![cfg(all(feature = "plugins", feature = "test-util"))]

use std::sync::{Arc, Mutex};

use seleniumbase_rs::common::shutdown::{Outcome, Shutdown, Signal};
use seleniumbase_rs::plugin_api::{
    async_trait, BrowserHandle, Capability, Category, Metadata, NoPrompt, Plugin, PluginError,
    PluginSet, Result as PluginResult, Sink, Task, TaskContext,
};
use seleniumbase_rs::plugin_host::{run_task_on, run_task_until};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, MockCtrl};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};

/// What the hooks of the plugins in one test saw, in order.
type Log = Arc<Mutex<Vec<String>>>;

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

/// A sink that keeps what a task records.
#[derive(Default)]
struct Lines(Mutex<Vec<Value>>);

impl Lines {
    fn all(&self) -> Vec<Value> {
        self.0.lock().unwrap().clone()
    }
}

impl Sink for Lines {
    fn record(&self, line: &Value) -> PluginResult<()> {
        self.0.lock().unwrap().push(line.clone());
        Ok(())
    }
}

/// How the task of a [`Worker`] ends.
#[derive(Clone, Copy)]
enum Ends {
    Summary,
    Error,
    Panic,
    Never,
}

/// A plugin with a task, and close hooks that write down that the browser closed.
struct Worker {
    name: &'static str,
    ends: Ends,
    capabilities: bool,
    log: Log,
}

impl Worker {
    fn new(name: &'static str, ends: Ends, log: &Log) -> Self {
        Self {
            name,
            ends,
            capabilities: true,
            log: log.clone(),
        }
    }
}

#[async_trait]
impl Plugin for Worker {
    fn name(&self) -> &str {
        self.name
    }

    fn metadata(&self) -> Metadata {
        let metadata = Metadata::new(Category::Core);
        if self.capabilities {
            metadata.capability(Capability::Task)
        } else {
            metadata
        }
    }

    fn task(&self) -> Option<&dyn Task> {
        Some(self)
    }

    async fn before_close(&self, _browser: &dyn BrowserHandle) -> PluginResult<()> {
        self.log.lock().unwrap().push("before_close".to_owned());
        Ok(())
    }

    async fn on_close(&self) -> PluginResult<()> {
        self.log.lock().unwrap().push("on_close".to_owned());
        Ok(())
    }
}

#[async_trait]
impl Task for Worker {
    async fn run(&self, cx: &TaskContext<'_>) -> PluginResult<Value> {
        cx.sink.record(&json!({ "started": cx.input }))?;
        match self.ends {
            Ends::Summary => Ok(json!({ "pages": 3, "browser": cx.browser.id().as_str() })),
            Ends::Error => Err(PluginError::other("the site is down")),
            Ends::Panic => panic!("a bug in the task"),
            Ends::Never => std::future::pending().await,
        }
    }
}

fn options_with(plugin: Worker) -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .plugins(PluginSet::new().with(plugin).unwrap())
        .build()
        .unwrap()
}

async fn launch(plugin: Worker) -> Browser {
    Browser::launch_mocked(options_with(plugin), &MockCtrl::new())
        .await
        .expect("the mocked launch")
}

async fn run(
    ends: Ends,
    shutdown: &mut Shutdown,
    sink: &Lines,
    log: &Log,
) -> Result<Outcome<Value>, SeleniumBaseError> {
    let browser = launch(Worker::new("worker", ends, log)).await;
    run_task_on(
        shutdown,
        browser,
        "worker",
        &json!({ "start": "https://shop.test/" }),
        sink,
        &NoPrompt,
    )
    .await
}

#[tokio::test]
async fn a_task_gets_its_input_and_sink_and_the_browser_closes_after_it() {
    let log = Log::default();
    let sink = Lines::default();
    let (mut shutdown, _trigger) = Shutdown::manual();

    let outcome = run(Ends::Summary, &mut shutdown, &sink, &log)
        .await
        .unwrap();

    assert_eq!(
        outcome,
        Outcome::Completed(json!({ "pages": 3, "browser": "cdp:127.0.0.1:0" }))
    );
    assert_eq!(
        sink.all(),
        [json!({ "started": { "start": "https://shop.test/" } })]
    );
    assert_eq!(entries(&log), ["before_close", "on_close"]);
}

#[tokio::test]
async fn a_task_that_fails_names_its_plugin_keeps_its_results_and_closes_the_browser() {
    let log = Log::default();
    let sink = Lines::default();
    let (mut shutdown, _trigger) = Shutdown::manual();

    let error = run(Ends::Error, &mut shutdown, &sink, &log)
        .await
        .unwrap_err();

    let SeleniumBaseError::Plugin { plugin, reason } = &error else {
        panic!("expected a plugin error, got {error:?}");
    };
    assert_eq!(plugin, "worker");
    assert!(reason.contains("the site is down"), "{reason}");
    assert_eq!(sink.all().len(), 1, "what it recorded before it failed");
    assert_eq!(entries(&log), ["before_close", "on_close"]);
}

#[tokio::test]
async fn a_task_that_panics_is_an_error_and_the_browser_still_closes() {
    let log = Log::default();
    let sink = Lines::default();
    let (mut shutdown, _trigger) = Shutdown::manual();

    let error = run(Ends::Panic, &mut shutdown, &sink, &log)
        .await
        .unwrap_err();

    let SeleniumBaseError::Plugin { plugin, reason } = &error else {
        panic!("expected a plugin error, got {error:?}");
    };
    assert_eq!(plugin, "worker");
    assert!(
        reason.contains("its task panicked: a bug in the task"),
        "{reason}"
    );
    assert_eq!(entries(&log), ["before_close", "on_close"]);
}

#[tokio::test]
async fn an_interrupt_drops_the_task_and_closes_the_browser() {
    let log = Log::default();
    let sink = Lines::default();
    let (mut shutdown, trigger) = Shutdown::manual();
    trigger.fire(Signal::Interrupt);

    let outcome = run(Ends::Never, &mut shutdown, &sink, &log).await.unwrap();

    assert_eq!(outcome, Outcome::Interrupted(Signal::Interrupt));
    assert_eq!(entries(&log), ["before_close", "on_close"]);
}

#[tokio::test]
async fn a_name_without_a_task_is_refused_before_any_browser_starts() {
    let log = Log::default();
    let sink = Lines::default();
    let (mut shutdown, _trigger) = Shutdown::manual();

    // A launch would try to start a real Chrome; the check comes first.
    let error = run_task_until(
        &mut shutdown,
        options_with(Worker::new("worker", Ends::Summary, &log)),
        "ghost",
        &json!({}),
        &sink,
        &NoPrompt,
    )
    .await
    .unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "{error:?}"
    );
    assert_eq!(error.exit_code(), 2);
    let text = error.to_string();
    assert!(
        text.contains("no enabled plugin `ghost` offers a task"),
        "{text}"
    );
    assert!(
        text.contains("the enabled plugins with a task: worker"),
        "{text}"
    );
    assert!(sink.all().is_empty());
    assert!(entries(&log).is_empty(), "no plugin hook ran");
}

#[tokio::test]
async fn a_plugin_whose_task_lacks_the_capability_is_refused_before_any_browser_starts() {
    let log = Log::default();
    let sink = Lines::default();
    let (mut shutdown, _trigger) = Shutdown::manual();
    let plugin = Worker {
        capabilities: false,
        ..Worker::new("sneaky", Ends::Summary, &log)
    };

    let error = run_task_until(
        &mut shutdown,
        options_with(plugin),
        "sneaky",
        &json!({}),
        &sink,
        &NoPrompt,
    )
    .await
    .unwrap_err();

    let SeleniumBaseError::Plugin { plugin, reason } = &error else {
        panic!("expected a plugin error, got {error:?}");
    };
    assert_eq!(plugin, "sneaky");
    assert!(reason.contains("task"), "{reason}");
    assert!(sink.all().is_empty(), "the task never ran");
}
