//! Opening a URL and moving through the history.

use serde_json::{json, Value};

use super::{navigate_for_client, Cdp};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "open_url",
            "Open URL",
            "Navigate the active tab to a URL and wait for it to load. A bare host such as \
             example.com gets https:// added.",
            Effect::Act,
            Schema::new().required("url", Prop::string("Address to open")),
            open_url,
        ),
        ToolDef::new(
            "manage_history",
            "Manage History",
            "Go back or forward, reload the page ignoring the cache, or list the tab's \
             navigation history.",
            Effect::Mixed,
            Schema::new().optional(
                "action",
                Prop::choice(&["back", "forward", "reload", "list"], "What to do").default("list"),
            ),
            manage_history,
        ),
    ]
}

async fn open_url(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let url = args.str("url")?;
    navigate_for_client(&ctx.session().await?.page, url).await?;
    Ok(format!("Navigated to {url}").into())
}

async fn manage_history(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let action = args.choice("action", &["back", "forward", "reload", "list"], "list")?;
    let session = ctx.session().await?;
    let page = &session.page;
    match action {
        "back" => {
            page.back().await?;
            Ok("Navigated back.".into())
        }
        "forward" => {
            page.forward().await?;
            Ok("Navigated forward.".into())
        }
        "reload" => {
            page.hard_reload().await?;
            Ok("Page reloaded.".into())
        }
        _ => {
            let history = page.execute("Page.getNavigationHistory", json!({})).await?;
            let entries: Vec<Value> = history["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|entry| {
                    json!({
                        "id": entry["id"],
                        "url": entry["url"],
                        "user_typed_url": entry["userTypedURL"],
                        "title": entry["title"],
                        "transition_type": entry["transitionType"],
                    })
                })
                .collect();
            Ok(json!({ "position": history["currentIndex"], "entries": entries }).into())
        }
    }
}
