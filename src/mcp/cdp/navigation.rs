//! Opening a URL and moving through the history.

use super::{manage_history, open_url, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

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
