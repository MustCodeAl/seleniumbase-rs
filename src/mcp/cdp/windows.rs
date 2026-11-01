//! Scrolling, the browser window and its tabs.

use super::{manage_tabs, manage_window, scroll_page, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

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
