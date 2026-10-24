//! Running the task of a plugin (feature `plugins`).
//!
//! A [`Task`](seleniumbase_rs_plugin::Task) is work a plugin does when a person starts it by
//! name: `sbrs plugin run crawler`, or the MCP tool `run_plugin_task`. [`run_task`] is the whole
//! run:
//!
//! 1. the name is checked against the plugins of the options, so a mistake is reported before a
//!    browser starts ([`check_task`]);
//! 2. the browser is launched with those plugins, so their launch hooks run;
//! 3. the task runs on it, with the browser as the plugin sees it in a hook, the input, a
//!    [`Sink`] for its results, the clock of the set and a [`Prompt`];
//! 4. a SIGINT or SIGTERM drops the task at its next await point
//!    ([`Outcome::Interrupted`]);
//! 5. the browser is closed in every case, which runs the plugins' `before_close` and
//!    `on_close`.
//!
//! The results the task writes to the sink stay there when the task fails or is interrupted.

use seleniumbase_rs_plugin::{PluginSet, Prompt, Sink};
use serde_json::Value;

use crate::common::shutdown::{cleanup_within, timeout_from_env, Outcome, Shutdown};
use crate::error::SeleniumBaseError;
use crate::sb_cdp::plugin_hooks;
use crate::sb_cdp::{Browser, LaunchOptions};

/// Refuses `name` unless a plugin of `set` has that name and a task, and says which plugins do.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when no plugin of the set called `name` offers a task
/// (the message lists the ones that do), and [`SeleniumBaseError::Plugin`] when it offers one
/// without holding the capability for it.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::plugin_api::PluginSet;
/// use seleniumbase_rs::plugin_host::check_task;
///
/// let error = check_task(&PluginSet::new(), "crawler").unwrap_err();
/// assert_eq!(error.exit_code(), 2);
/// assert!(error.to_string().contains("no enabled plugin `crawler` offers a task"));
/// ```
pub fn check_task(set: &PluginSet, name: &str) -> Result<(), SeleniumBaseError> {
    let with_task = set.task_names();
    if !with_task.contains(&name) {
        let known = if with_task.is_empty() {
            "none of the enabled plugins has one".to_owned()
        } else {
            format!("the enabled plugins with a task: {}", with_task.join(", "))
        };
        return Err(SeleniumBaseError::invalid_config(format!(
            "no enabled plugin `{name}` offers a task ({known})"
        )));
    }
    set.task(name)
        .map(drop)
        .map_err(|error| SeleniumBaseError::plugin(name, error.to_string()))
}

/// Launches a browser with the plugins of `options`, runs the task of the plugin `name` on it and
/// closes it, as the module documentation describes. SIGINT and SIGTERM are listened for from
/// before the launch.
///
/// # Errors
///
/// What [`check_task`], the launch and the task return, as [`SeleniumBaseError`]: the plugin's
/// name is in [`SeleniumBaseError::Plugin`] when the task fails or panics.
pub async fn run_task(
    options: LaunchOptions,
    name: &str,
    input: &Value,
    sink: &dyn Sink,
    prompt: &dyn Prompt,
) -> Result<Outcome<Value>, SeleniumBaseError> {
    let mut shutdown = Shutdown::install()?;
    run_task_until(&mut shutdown, options, name, input, sink, prompt).await
}

/// [`run_task`] with a [`Shutdown`] of your own, such as [`Shutdown::manual`] in a test.
///
/// # Errors
///
/// As [`run_task`].
pub async fn run_task_until(
    shutdown: &mut Shutdown,
    options: LaunchOptions,
    name: &str,
    input: &Value,
    sink: &dyn Sink,
    prompt: &dyn Prompt,
) -> Result<Outcome<Value>, SeleniumBaseError> {
    check_task(&options.plugins, name)?;
    let browser = Browser::launch(options).await?;
    run_task_on(shutdown, browser, name, input, sink, prompt).await
}

/// Runs the task of the plugin `name` on `browser`, which has the plugins, and closes the browser
/// afterwards whatever happens.
///
/// # Errors
///
/// [`SeleniumBaseError::Plugin`] naming the plugin when it has no task or lacks the capability
/// for one, when the task fails, and when it panics.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::common::shutdown::{Outcome, Shutdown};
/// use seleniumbase_rs::plugin_api::{NoPrompt, NullSink};
/// use seleniumbase_rs::plugin_host::run_task_on;
/// use seleniumbase_rs::sb_cdp::Browser;
///
/// # async fn demo(browser: Browser) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let mut shutdown = Shutdown::install()?;
/// let input = serde_json::json!({ "start": "https://example.com/" });
/// match run_task_on(&mut shutdown, browser, "crawler", &input, &NullSink, &NoPrompt).await? {
///     Outcome::Completed(summary) => println!("{summary}"),
///     Outcome::Interrupted(signal) => eprintln!("{signal}: stopped"),
/// }
/// # Ok(())
/// # }
/// ```
pub async fn run_task_on(
    shutdown: &mut Shutdown,
    browser: Browser,
    name: &str,
    input: &Value,
    sink: &dyn Sink,
    prompt: &dyn Prompt,
) -> Result<Outcome<Value>, SeleniumBaseError> {
    let ran = shutdown
        .race(plugin_hooks::run_task(&browser, name, input, sink, prompt))
        .await;
    // The browser closes whether the task finished, failed or was cut off; a failure to close
    // is logged, because the task's own result is the answer.
    cleanup_within(timeout_from_env(), async {
        if let Err(error) = browser.close().await {
            tracing::warn!(%error, "the browser did not close cleanly after a plugin task");
        }
    })
    .await;
    match ran {
        Outcome::Completed(result) => result.map(Outcome::Completed),
        Outcome::Interrupted(signal) => Ok(Outcome::Interrupted(signal)),
    }
}
