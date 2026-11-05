//! Scrolling, the browser window and its tabs.

use serde_json::{json, Value};

use super::Cdp;
use crate::mcp::support::{json_output, nonempty};
use crate::sb_cdp::Scroll;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "scroll_page",
            "Scroll Page",
            "Scroll the page up or down by a share of the window height, or to the very top \
             or bottom.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "direction",
                    Prop::choice(&["up", "down", "top", "bottom"], "Where to scroll")
                        .default("down"),
                )
                .optional(
                    "amount",
                    Prop::integer(
                        "Percent of the window height for up and down; 200 is two screens",
                    )
                    .min(0.0)
                    .default(25),
                ),
            scroll_page,
        ),
        ToolDef::new(
            "manage_window",
            "Manage Window",
            "Read or change the browser window's position, size and state.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "action",
                    Prop::choice(
                        &["get_rect", "set_rect", "maximize", "minimize"],
                        "What to do",
                    )
                    .default("get_rect"),
                )
                .optional("x", Prop::integer("Left edge, for set_rect"))
                .optional("y", Prop::integer("Top edge, for set_rect"))
                .optional("width", Prop::integer("Width, for set_rect"))
                .optional("height", Prop::integer("Height, for set_rect")),
            manage_window,
        ),
        ToolDef::new(
            "manage_tabs",
            "Manage Tabs",
            "List, open, switch between and close tabs. Closing the active tab moves to the \
             newest remaining one.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "action",
                    Prop::choice(
                        &[
                            "list_tabs",
                            "open_new_tab",
                            "switch_to_tab",
                            "switch_to_newest_tab",
                            "close_active_tab",
                        ],
                        "What to do",
                    )
                    .default("list_tabs"),
                )
                .optional(
                    "url",
                    Prop::string("Page for open_new_tab; defaults to about:blank"),
                )
                .optional(
                    "tab_index",
                    Prop::integer("Tab number from list_tabs, for switch_to_tab"),
                )
                .optional(
                    "switch_to",
                    Prop::boolean("Make a newly opened tab the active one").default(true),
                ),
            manage_tabs,
        ),
    ]
}

async fn scroll_page(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let direction = args.choice("direction", &["up", "down", "top", "bottom"], "down")?;
    let amount = args.i64_or("amount", 25)?;
    if amount < 0 && matches!(direction, "up" | "down") {
        return Err(ToolError::invalid("amount", "cannot be negative"));
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a scroll percentage is far below 2^53"
    )]
    let percent = amount as f64;
    let (to, done) = match direction {
        "up" => (
            Scroll::PageUp(percent),
            format!("Scrolled up by {amount}%."),
        ),
        "down" => (
            Scroll::PageDown(percent),
            format!("Scrolled down by {amount}%."),
        ),
        "top" => (Scroll::Top, "Scrolled to the top.".to_owned()),
        _ => (Scroll::Bottom, "Scrolled to the bottom.".to_owned()),
    };
    ctx.session().await?.page.scroll(to).await?;
    Ok(done.into())
}

async fn manage_window(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let action = args.choice(
        "action",
        &["get_rect", "set_rect", "maximize", "minimize"],
        "get_rect",
    )?;
    let session = ctx.session().await?;
    let window = session.page.window();

    match action {
        "get_rect" => json_output(window.bounds().await?),
        "set_rect" => {
            let (Some(x), Some(y), Some(width), Some(height)) = (
                args.opt_i64("x")?,
                args.opt_i64("y")?,
                args.opt_i64("width")?,
                args.opt_i64("height")?,
            ) else {
                return Err(ToolError::invalid(
                    "action",
                    "set_rect needs x, y, width and height",
                ));
            };
            window.set_bounds(x, y, width, height).await?;
            Ok(format!("Window set to ({x}, {y}, {width}x{height})").into())
        }
        "maximize" => {
            window.maximize().await?;
            Ok("Window maximized.".into())
        }
        _ => {
            window.minimize().await?;
            Ok("Window minimized.".into())
        }
    }
}

async fn manage_tabs(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let action = args.choice(
        "action",
        &[
            "list_tabs",
            "open_new_tab",
            "switch_to_tab",
            "switch_to_newest_tab",
            "close_active_tab",
        ],
        "list_tabs",
    )?;
    let mut session = ctx.session().await?;

    match action {
        "list_tabs" => {
            let tabs: Vec<Value> = session
                .browser
                .list_pages()
                .await?
                .into_iter()
                .enumerate()
                .map(|(index, tab)| json!({ "index": index, "url": tab.url, "title": tab.title }))
                .collect();
            Ok(Output::Json(Value::Array(tabs)))
        }
        "open_new_tab" => {
            let url = nonempty(&args, "url")?.unwrap_or("about:blank");
            let switch_to = args.bool_or("switch_to", true)?;
            let page = session.browser.new_page_at(url).await?;
            if switch_to {
                page.bring_to_front().await?;
                session.page = page;
            }
            Ok(format!("Opened new tab (url={url:?}, switch_to={switch_to})").into())
        }
        "switch_to_tab" => {
            let index = args
                .opt_usize("tab_index")?
                .ok_or_else(|| ToolError::invalid("tab_index", "required for switch_to_tab"))?;
            let mut tabs = session.browser.pages().await?;
            if index >= tabs.len() {
                return Err(ToolError::invalid(
                    "tab_index",
                    format!("{index} is out of range; there are {} tabs", tabs.len()),
                ));
            }
            let page = tabs.swap_remove(index);
            page.bring_to_front().await?;
            session.page = page;
            Ok(format!("Switched to tab {index}").into())
        }
        "switch_to_newest_tab" => {
            let page = session.browser.newest_page().await?;
            page.bring_to_front().await?;
            session.page = page;
            Ok("Switched to newest tab.".into())
        }
        _ => {
            session.page.close().await?;
            match session.browser.newest_page().await {
                Ok(page) => {
                    page.bring_to_front().await?;
                    session.page = page;
                    Ok("Closed active tab; switched to the newest remaining tab.".into())
                }
                Err(_) => Ok("Closed active tab; no tabs remain, open a new one.".into()),
            }
        }
    }
}
