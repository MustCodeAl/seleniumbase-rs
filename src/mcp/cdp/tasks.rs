//! Running the task of a plugin the operator enabled, on a browser of its own.

#[cfg(feature = "plugins")]
use std::sync::{Mutex, PoisonError};

#[cfg(feature = "plugins")]
use serde_json::{json, Value};

use super::Cdp;
#[cfg(feature = "plugins")]
use crate::mcp::failure::shorten;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

/// The most results of a plugin task that an answer carries; the count says how many there were.
#[cfg(feature = "plugins")]
const MAX_TASK_RECORDS: usize = 200;

/// The longest text of one value in the results of a plugin task.
#[cfg(feature = "plugins")]
const MAX_RECORD_TEXT: usize = 2000;

/// `run_plugin_task`: the task of a plugin the operator enabled, on a browser of its own.
pub(super) fn tool() -> ToolDef<Cdp> {
    ToolDef::new(
        "run_plugin_task",
        "Run Plugin Task",
        "Run the task of a plugin on a browser of its own (not the running session), and return its \
         summary and its first results. Only plugins the operator turned on for this server \
         (`sbrs-mcp --plugin NAME`) can run, and only ones that offer a task; a task that needs a \
         person at the keyboard cannot be run here (use `sbrs plugin run`). The browser is closed \
         when the task ends.",
        Effect::Mixed,
        Schema::new()
            .required("name", Prop::string("The plugin whose task to run"))
            .optional(
                "input",
                Prop::string("The task's input as JSON text, such as {\"start\":\"https://example.com/\"}; {} when omitted"),
            )
            .optional(
                "plugins",
                Prop::string(
                    "Comma-separated names of the other enabled plugins to run beside it; all \
                     of the operator's plugins when omitted",
                ),
            )
            .optional(
                "headless",
                Prop::boolean("Force headless (true) or headed (false)"),
            ),
        run_plugin_task,
    )
}

/// What a plugin task writes to its sink in an MCP call: the first results are kept for the
/// answer, and each one is counted and reported as progress.
#[cfg(feature = "plugins")]
struct TaskRecords {
    kept: Mutex<Vec<Value>>,
    count: std::sync::atomic::AtomicU32,
    reported: tokio::sync::mpsc::UnboundedSender<u32>,
}

#[cfg(feature = "plugins")]
impl TaskRecords {
    fn new(reported: tokio::sync::mpsc::UnboundedSender<u32>) -> Self {
        Self {
            kept: Mutex::new(Vec::new()),
            count: std::sync::atomic::AtomicU32::new(0),
            reported,
        }
    }

    /// The results kept and how many there were; the progress channel closes with `self`.
    fn finish(self) -> (Vec<Value>, u32) {
        let kept = self
            .kept
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        (kept, self.count.into_inner())
    }
}

#[cfg(feature = "plugins")]
impl seleniumbase_rs_plugin::Sink for TaskRecords {
    fn record(&self, line: &Value) -> seleniumbase_rs_plugin::Result<()> {
        let count = self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        {
            let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
            if kept.len() < MAX_TASK_RECORDS {
                kept.push(shorten(line, MAX_RECORD_TEXT));
            }
        }
        // A client that is not listening, or a forwarder that is gone, is nothing to stop a task for.
        let _ = self.reported.send(count);
        Ok(())
    }
}

/// How a task asks a person for help over MCP: it cannot.
#[cfg(feature = "plugins")]
struct NoPerson;

#[cfg(feature = "plugins")]
#[seleniumbase_rs_plugin::async_trait]
impl seleniumbase_rs_plugin::Prompt for NoPerson {
    async fn wait(
        &self,
        _message: &str,
        _page: &dyn seleniumbase_rs_plugin::PageHandle,
    ) -> seleniumbase_rs_plugin::Result<()> {
        Err(seleniumbase_rs_plugin::PluginError::other(
            "this task needs a person at the keyboard; run it with `sbrs plugin run`",
        ))
    }
}

/// The plugins a `run_plugin_task` call runs: `name` and the `others` (comma-separated) it names,
/// or all of the operator's plugins when it names none, with the operator's options.
///
/// # Errors
///
/// [`ToolError::Refused`] when `name` is not enabled for the server, and
/// [`ToolError::InvalidArgument`] when `others` names a plugin that is not.
#[cfg(feature = "plugins")]
fn task_selection(
    operator: &crate::config::plugins::PluginSelection,
    name: &str,
    others: Option<&str>,
) -> Result<crate::config::plugins::PluginSelection, ToolError> {
    let enabled = |plugin: &str| operator.enabled().iter().any(|enabled| enabled == plugin);
    let list = || {
        if operator.enabled().is_empty() {
            "none".to_owned()
        } else {
            operator.enabled().join(", ")
        }
    };
    if !enabled(name) {
        return Err(ToolError::Refused(format!(
            "the plugin `{name}` is not enabled for this server: the operator turns plugins on \
             with `sbrs-mcp --plugin NAME` and no tool argument adds one (enabled: {})",
            list()
        )));
    }
    let Some(others) = others else {
        return Ok(operator.clone());
    };
    let mut subset = crate::config::plugins::PluginSelection::new();
    for plugin in std::iter::once(name).chain(others.split(',').map(str::trim)) {
        if plugin.is_empty() || subset.enabled().iter().any(|chosen| chosen == plugin) {
            continue;
        }
        if !enabled(plugin) {
            return Err(ToolError::invalid(
                "plugins",
                format!(
                    "the plugin `{plugin}` is not enabled for this server (enabled: {})",
                    list()
                ),
            ));
        }
        subset = subset.enable(plugin);
        if let Some(options) = operator.options_of(plugin) {
            subset = subset.with_options(plugin, options.clone());
        }
    }
    Ok(subset)
}

async fn run_plugin_task(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    #[cfg(feature = "plugins")]
    {
        plugin_task(&ctx, &args).await
    }
    #[cfg(not(feature = "plugins"))]
    {
        let _ = (&ctx, &args);
        Err(ToolError::Refused(
            "this build of the server has no plugins; rebuild it with --features plugins-builtin"
                .to_owned(),
        ))
    }
}

#[cfg(feature = "plugins")]
async fn plugin_task(ctx: &Ctx<Cdp>, args: &Args) -> Result<Output, ToolError> {
    use crate::common::shutdown::{Outcome, Shutdown};

    let name = args.str("name")?;
    let selection = task_selection(
        ctx.settings().plugin_selection(),
        name,
        args.opt_str("plugins")?,
    )?;
    let input: Value = match args.opt_str("input")? {
        Some(text) => serde_json::from_str(text)
            .map_err(|error| ToolError::invalid("input", format!("not JSON: {error}")))?,
        None => json!({}),
    };
    let headless = args
        .opt_bool("headless")?
        .unwrap_or(cfg!(target_os = "linux"));

    // Every mistake in the name and the plugins is found before a browser starts.
    let options = crate::cli::plugin_cmd::task_launch_options(
        crate::config::settings::Settings {
            headless,
            plugins: selection,
            ..crate::config::settings::Settings::default()
        },
        name,
    )?;

    let (reported, mut counts) = tokio::sync::mpsc::unbounded_channel();
    let records = TaskRecords::new(reported);
    let progress = ctx.progress();
    let forwarder = tokio::spawn(async move {
        while let Some(count) = counts.recv().await {
            progress
                .report(count, None, format!("{count} results so far"))
                .await;
        }
    });
    // The server stops on its own signals; a task here is not raced against one of its own.
    let (mut shutdown, _never) = Shutdown::manual();
    let ran = crate::plugin_host::run_task_until(
        &mut shutdown,
        options,
        name,
        &input,
        &records,
        &NoPerson,
    )
    .await;
    let (kept, total) = records.finish();
    // The channel closed with the sink; the forwarder ends once it has said what is left.
    let _ = forwarder.await;

    match ran? {
        Outcome::Completed(summary) => Ok(json!({
            "plugin": name,
            "summary": summary,
            "records": kept,
            "record_count": total,
            "records_truncated": usize::try_from(total).is_ok_and(|total| total > kept.len()),
        })
        .into()),
        Outcome::Interrupted(signal) => Err(ToolError::Refused(format!(
            "the task of `{name}` was stopped by {signal}"
        ))),
    }
}

#[cfg(all(test, feature = "plugins"))]
mod tests {
    use seleniumbase_rs_plugin::Sink as _;
    use serde_json::json;

    use super::*;
    use crate::config::plugins::PluginSelection;

    fn operator() -> PluginSelection {
        PluginSelection::new()
            .enable("shield")
            .enable("crx")
            .with_options("shield", json!({ "level": "strict" }))
    }

    #[test]
    fn a_plugin_the_operator_did_not_enable_is_refused_and_the_enabled_ones_are_named() {
        let error = task_selection(&operator(), "crawler", None).unwrap_err();

        let ToolError::Refused(reason) = &error else {
            panic!("expected a refusal, got {error:?}");
        };
        assert!(reason.contains("`crawler` is not enabled"), "{reason}");
        assert!(reason.contains("enabled: shield, crx"), "{reason}");

        let none = task_selection(&PluginSelection::new(), "crawler", None).unwrap_err();
        assert!(none.to_string().contains("enabled: none"), "{none}");
    }

    #[test]
    fn without_a_list_the_task_runs_with_all_the_operators_plugins_and_options() {
        let chosen = task_selection(&operator(), "crx", None).unwrap();

        assert_eq!(chosen, operator());
    }

    #[test]
    fn a_list_picks_a_subset_with_the_operators_options_and_the_task_plugin_first() {
        let chosen = task_selection(&operator(), "crx", Some(" shield, ,crx")).unwrap();

        assert_eq!(chosen.enabled(), ["crx", "shield"]);
        assert_eq!(
            chosen.options_of("shield"),
            Some(&json!({ "level": "strict" }))
        );
        let alone = task_selection(&operator(), "shield", Some("")).unwrap();
        assert_eq!(alone.enabled(), ["shield"]);
    }

    #[test]
    fn a_list_cannot_add_a_plugin_the_operator_did_not_enable() {
        let error = task_selection(&operator(), "crx", Some("shield,tor")).unwrap_err();

        assert!(
            matches!(&error, ToolError::InvalidArgument { name, .. } if name == "plugins"),
            "{error:?}"
        );
        assert!(
            error.to_string().contains("`tor` is not enabled"),
            "{error}"
        );
    }

    #[test]
    fn the_results_are_counted_kept_up_to_a_limit_and_reported_in_order() {
        let (reported, mut counts) = tokio::sync::mpsc::unbounded_channel();
        let records = TaskRecords::new(reported);
        let total = u32::try_from(MAX_TASK_RECORDS + 5).unwrap();

        for index in 0..total {
            records.record(&json!({ "index": index })).unwrap();
        }
        let (kept, count) = records.finish();

        assert_eq!(count, total);
        assert_eq!(kept.len(), MAX_TASK_RECORDS);
        assert_eq!(kept[0], json!({ "index": 0 }));
        let mut seen = Vec::new();
        while let Ok(count) = counts.try_recv() {
            seen.push(count);
        }
        assert_eq!(seen, (1..=total).collect::<Vec<_>>());
    }

    #[test]
    fn a_long_value_in_a_result_is_shortened_for_the_answer() {
        let (reported, _counts) = tokio::sync::mpsc::unbounded_channel();
        let records = TaskRecords::new(reported);

        records
            .record(&json!({ "text": "x".repeat(MAX_RECORD_TEXT * 3) }))
            .unwrap();
        let (kept, _) = records.finish();

        let text = kept[0]["text"].as_str().unwrap();
        assert!(text.len() < MAX_RECORD_TEXT * 2, "{} bytes", text.len());
    }
}
