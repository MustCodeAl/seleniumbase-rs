//! Writing what a session found to the output directory: a debug bundle and the
//! session as a Rust test.

use serde_json::{json, Value};

use super::{bounded, page_op, DevSession};
use crate::error::SeleniumBaseError;
use crate::mcp::export::{current_session, export};
use crate::mcp::support::{json_output, nonempty, write_file};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "capture_debug_bundle",
            "Capture Debug Bundle",
            "Save what a bug report needs in a new folder under the output directory: a \
             screenshot, the page source (password and hidden input values removed), the URL, \
             title, viewport and user agent, console messages, failed requests, cookie and \
             storage names (never their values) and the last tool calls (typed text and scripts \
             redacted).",
            Effect::Overwrite,
            Schema::new()
                .optional(
                    "name",
                    Prop::string("A label for the folder; letters, digits, - and _ are kept"),
                )
                .optional(
                    "calls",
                    Prop::integer("How many recent tool calls to include, 1 to 200")
                        .min(1.0)
                        .default(20),
                ),
            capture_debug_bundle::<S>,
        ),
        ToolDef::new(
            "export_session_as_test",
            "Export Session As Test",
            "Write the successful steps of this browser session (since the last start_browser) \
             as a Rust #[tokio::test] using BaseCase, into the output directory. Steps with no \
             BaseCase equivalent become // skipped: comments and are listed in the result.",
            Effect::Overwrite,
            Schema::new().optional(
                "test_name",
                Prop::string("Name of the test function and file, such as checkout_works")
                    .default("exported_session"),
            ),
            export_session_as_test::<S>,
        ),
    ]
}

/// What `bundle.json` says it leaves out. It has to match what
/// `CallRecord::redacted` in `journal.rs` does to a tool call.
const REDACTION_NOTE: &str = "Cookie and storage values are left out, password and hidden input values \
                              are replaced in page.html, typed text and scripts in tool calls are \
                              replaced by their length, the results of successful tool calls are \
                              replaced by their length, and only the error text of a failed call is kept.";

async fn capture_debug_bundle<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let label = crate::artifacts::safe_stem(nonempty(&args, "name")?.unwrap_or("bundle"), "bundle");
    let wanted_calls = bounded(&args, "calls", 20, 200)?;
    let folder = format!(
        "debug_bundles/{label}_{}",
        chrono::Local::now().format("%Y%m%d_%H%M%S_%3f")
    );
    let settings = ctx.settings().clone();
    let path_of = |file: &str| settings.output_path(Some(&folder), file);

    let mut problems: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut session = ctx.session().await?;
    let engine = session.engine();

    let mut facts = page_op(&mut *session, "environment", &json!({})).await?;
    match session.screenshot().await {
        Ok(png) => {
            let path = path_of("screenshot.png")?;
            write_file(&path, &png).await?;
            files.push(path.display().to_string());
        }
        Err(error) => problems.push(format!("screenshot: {error}")),
    }
    match page_op(&mut *session, "source", &json!({})).await {
        Ok(source) => {
            let path = path_of("page.html")?;
            write_file(
                &path,
                source["html"].as_str().unwrap_or_default().as_bytes(),
            )
            .await?;
            files.push(path.display().to_string());
            facts["page_source_redacted_inputs"] = source["redacted_inputs"].clone();
        }
        Err(error) => problems.push(format!("page source: {error}")),
    }
    let console = page_op(&mut *session, "console", &json!({ "limit": 500 })).await;
    let timeline = page_op(&mut *session, "resources", &json!({ "limit": 500 })).await;
    let page_errors = session.page_errors().await;
    let recorded = session.network_failures(false);
    let cookies = session.cookie_names().await;
    drop(session);

    let (calls, dropped) = ctx.calls();
    let recent: Vec<Value> = calls
        .iter()
        .rev()
        .take(wanted_calls)
        .rev()
        .map(crate::mcp::journal::CallRecord::redacted)
        .collect();
    let mut outcome = |part: &str, result: Result<Value, ToolError>| {
        result.unwrap_or_else(|error| {
            problems.push(format!("{part}: {error}"));
            Value::Null
        })
    };
    let bundle = json!({
        "captured_at": chrono::Local::now().to_rfc3339(),
        "engine": engine,
        "page": facts,
        "console": outcome("console", console),
        "page_errors": outcome("page errors", page_errors.map(Value::from).map_err(ToolError::from)),
        "failed_requests": {
            "from_performance_timeline": outcome("performance timeline", timeline),
            "recorded": recorded.map(|(failures, dropped)| json!({ "failures": failures, "dropped": dropped })),
        },
        "cookie_names": outcome("cookies", cookies.map(Value::from).map_err(ToolError::from)),
        "tool_calls": { "recent": recent, "older_dropped": dropped },
        "redaction": REDACTION_NOTE,
        "problems": problems,
    });
    let path = path_of("bundle.json")?;
    let text = serde_json::to_string_pretty(&bundle).map_err(SeleniumBaseError::from)?;
    write_file(&path, text.as_bytes()).await?;
    files.push(path.display().to_string());
    json_output(json!({
        "folder": settings.output_path(None, &folder)?.display().to_string(),
        "files": files,
        "problems": bundle["problems"],
    }))
}

/// Whether `name` can be both a Rust function name and a file name.
pub(in crate::mcp) fn is_test_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 64
}

async fn export_session_as_test<S: DevSession>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    let name = args.str_or("test_name", "exported_session")?;
    if !is_test_name(name) {
        return Err(ToolError::invalid(
            "test_name",
            "use letters, digits and _ only, starting with a letter, at most 64 characters",
        ));
    }
    let (calls, dropped) = ctx.calls();
    let exported = export(current_session(&calls));
    if exported.steps == 0 && exported.skipped.is_empty() {
        return Err(ToolError::Refused(
            "this session has no successful steps to export yet; open a page and act on it first"
                .to_owned(),
        ));
    }
    let code = exported.recorder.to_rust_test(name);
    let path = ctx
        .settings()
        .output_path(Some("exported_tests"), &format!("{name}.rs"))?;
    write_file(&path, code.as_bytes()).await?;
    let skipped: Vec<Value> = exported
        .skipped
        .iter()
        .map(|(tool, reason)| json!({ "tool": tool, "reason": reason }))
        .collect();
    let mut note = "The test drives the browser through WebDriver (BaseCase), whichever server \
                    recorded the session. Put it under tests/ and run it with cargo test."
        .to_owned();
    if dropped > 0 {
        note.push_str(" The call log keeps the last 200 calls; earlier steps are not in it.");
    }
    json_output(json!({
        "path": path.display().to_string(),
        "steps": exported.steps,
        "skipped": skipped,
        "note": note,
        "code": code,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_redaction_note_says_what_a_bug_report_keeps_of_a_tool_call() {
        assert!(
            REDACTION_NOTE
                .contains("results of successful tool calls are replaced by their length"),
            "{REDACTION_NOTE}"
        );
        assert!(
            REDACTION_NOTE.contains("only the error text of a failed call is kept"),
            "{REDACTION_NOTE}"
        );
    }

    #[test]
    fn test_names_must_be_rust_identifiers_and_file_names() {
        assert!(is_test_name("checkout_works"));
        assert!(is_test_name("_t2"));
        for bad in ["", "2fast", "a-b", "../x", "a b", "a.rs", &"x".repeat(65)] {
            assert!(!is_test_name(bad), "{bad:?}");
        }
    }
}
