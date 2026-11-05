//! What the page reports about itself: console messages, failed requests and
//! an accessibility audit.

use serde_json::{json, Map, Value};

use super::{bounded, page_op, DevSession};
use crate::error::SeleniumBaseError;
use crate::mcp::failure::shorten;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "get_console_messages",
            "Get Console Messages",
            "Read the page's console messages and uncaught script errors. Messages are recorded \
             from the moment start is true, which replaces the page's console functions (a page \
             script can notice that); the Pure CDP engine also reports uncaught errors since the \
             page loaded without it.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "level",
                    Prop::choice(
                        &["all", "error", "warning", "info", "log"],
                        "The least serious level to include",
                    )
                    .default("all"),
                )
                .optional(
                    "start",
                    Prop::boolean("Start recording console messages in this page").default(false),
                )
                .optional(
                    "clear",
                    Prop::boolean("Forget the messages after reading them").default(false),
                )
                .optional(
                    "limit",
                    Prop::integer("Most messages to return, 1 to 500")
                        .min(1.0)
                        .default(100),
                ),
            get_console_messages::<S>,
        )
        .untrusted(),
        ToolDef::new(
            "get_failed_requests",
            "Get Failed Requests",
            "List the page's requests that failed: HTTP status 400 and up from the browser's \
             performance timeline, and, once watch is true on the Pure CDP engine, every failed, \
             blocked or refused request of the active tab with its error.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "watch",
                    Prop::boolean("Start recording failed requests (Pure CDP engine)")
                        .default(false),
                )
                .optional(
                    "clear",
                    Prop::boolean("Forget the recorded failures after reading them").default(false),
                )
                .optional(
                    "limit",
                    Prop::integer("Most requests to return, 1 to 500")
                        .min(1.0)
                        .default(100),
                ),
            get_failed_requests::<S>,
        ),
        ToolDef::new(
            "audit_page",
            "Audit Page",
            "Check the page as it is now for accessibility and HTML defects: images without alt \
             text, controls without a label or name, duplicate ids, empty links and buttons, \
             heading order, missing lang or title, and more. Each finding names its rule and a \
             selector for the element.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "severity",
                    Prop::choice(&["all", "error"], "Report warnings too, or errors only")
                        .default("all"),
                )
                .optional(
                    "limit",
                    Prop::integer("Most findings to return, 1 to 500")
                        .min(1.0)
                        .default(50),
                ),
            audit_page::<S>,
        ),
    ]
}

async fn audit_page<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    use crate::api::html_inspector::{HtmlInspector, InspectionSeverity};

    let errors_only = args.choice("severity", &["all", "error"], "all")? == "error";
    let limit = bounded(&args, "limit", 50, 500)?;
    let mut session = ctx.session().await?;
    let page = session
        .evaluate("({ url: location.href, html: document.documentElement.outerHTML })")
        .await?;
    drop(session);
    let html = page["html"].as_str().unwrap_or_default().to_owned();
    let result = tokio::task::spawn_blocking(move || HtmlInspector::inspect(&html))
        .await
        .map_err(|error| SeleniumBaseError::unsupported(format!("the audit stopped: {error}")))??;

    let mut by_rule = Map::new();
    let mut issues = Vec::new();
    let (mut errors, mut warnings) = (0_usize, 0_usize);
    for issue in &result.issues {
        let error = issue.severity == InspectionSeverity::Error;
        if error {
            errors += 1;
        } else {
            warnings += 1;
        }
        if errors_only && !error {
            continue;
        }
        let counted = by_rule.entry(issue.rule.clone()).or_insert(Value::from(0));
        *counted = Value::from(counted.as_u64().unwrap_or(0) + 1);
        if issues.len() < limit {
            issues.push(json!({
                "rule": issue.rule,
                "severity": if error { "error" } else { "warning" },
                "message": issue.message,
                "selector": issue.selector,
            }));
        }
    }
    let listed = if errors_only {
        errors
    } else {
        errors + warnings
    };
    Ok(json!({
        "url": page["url"],
        "errors": errors,
        "warnings": warnings,
        "by_rule": by_rule,
        "issues": issues,
        "truncated": issues.len() < listed,
        "note": "Checks the page as it is now, after its scripts ran. These are static checks: \
                 they cannot judge colour contrast, focus order or what a screen reader says.",
    })
    .into())
}

async fn get_console_messages<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let level = args.choice("level", &["all", "error", "warning", "info", "log"], "all")?;
    let levels: Option<&[&str]> = match level {
        "error" => Some(&["error"]),
        "warning" => Some(&["error", "warning"]),
        "info" => Some(&["error", "warning", "info"]),
        "log" => Some(&["error", "warning", "info", "log"]),
        _ => None,
    };
    let start = args.bool_or("start", false)?;
    let clear = args.bool_or("clear", false)?;
    let limit = bounded(&args, "limit", 100, 500)?;
    let mut session = ctx.session().await?;
    let mut messages = page_op(
        &mut *session,
        "console",
        &json!({ "install": start, "clear": clear, "levels": levels, "limit": limit }),
    )
    .await?;
    let page_errors = session.page_errors().await?;
    let engine = session.engine();
    drop(session);

    let errors: Vec<Value> = page_errors
        .iter()
        .rev()
        .take(limit)
        .rev()
        .map(|error| shorten(&Value::from(error.as_str()), 500))
        .collect();
    messages["page_errors"] = Value::from(errors);
    if messages["recording"] == false {
        messages["note"] = Value::from(if engine == "cdp" {
            "Console messages are not being recorded: call again with start true. page_errors \
             lists the uncaught errors since the page loaded either way."
        } else {
            "Console messages and uncaught errors are not being recorded: call again with start \
             true. Recording lasts until the page navigates."
        });
    }
    Ok(messages.into())
}

async fn get_failed_requests<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let watch = args.bool_or("watch", false)?;
    let clear = args.bool_or("clear", false)?;
    let limit = bounded(&args, "limit", 100, 500)?;
    let mut session = ctx.session().await?;
    let timeline = page_op(&mut *session, "resources", &json!({ "limit": limit })).await?;
    let watching = if watch {
        session.watch_network().await?
    } else {
        session.network_failures(false).is_some()
    };
    let recorded = session.network_failures(clear);
    let engine = session.engine();
    drop(session);

    let mut answer = json!({
        "from_performance_timeline": timeline,
        "watching": watching,
    });
    if let Some((failures, dropped)) = recorded {
        let total = failures.len();
        answer["recorded"] = json!({
            "failures": failures.into_iter().rev().take(limit).rev().collect::<Vec<_>>(),
            "total": total,
            "dropped": dropped,
        });
    }
    answer["note"] = Value::from(match (engine, watching) {
        ("cdp", true) => {
            "recorded lists every failed, blocked or refused request of the tab since watching \
             started. The performance timeline only knows statuses of same-origin or CORS-enabled \
             resources."
        }
        ("cdp", false) => {
            "Only the performance timeline was read; it knows statuses of same-origin or \
             CORS-enabled resources only. Call again with watch true to record every failed \
             request from now on."
        }
        _ => {
            "Only the performance timeline was read (this engine cannot record network events): \
             it knows statuses of same-origin or CORS-enabled resources, and not requests that \
             never got an answer."
        }
    });
    Ok(answer.into())
}
