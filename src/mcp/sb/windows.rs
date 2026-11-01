//! The browser window, its tabs and its frames.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "get_window_rect",
            "Get Window Rect",
            "Get the browser window's position and size.",
            Effect::Observe,
            Schema::new(),
            w::get_window_rect,
        ),
        ToolDef::new(
            "maximize_window",
            "Maximize Window",
            "Maximize the browser window.",
            Effect::Set,
            Schema::new(),
            w::maximize_window,
        ),
        ToolDef::new(
            "minimize_window",
            "Minimize Window",
            "Minimize the browser window.",
            Effect::Set,
            Schema::new(),
            w::minimize_window,
        ),
        ToolDef::new(
            "open_new_tab",
            "Open New Tab",
            "Open a new tab, and switch to it unless told not to.",
            Effect::Act,
            Schema::new().optional(
                "switch_to",
                Prop::boolean("Make the new tab the active one").default(true),
            ),
            w::open_new_tab,
        ),
        ToolDef::new(
            "switch_to_newest_tab",
            "Switch To Newest Tab",
            "Switch to the most recently opened tab.",
            Effect::Set,
            Schema::new(),
            w::switch_to_newest_tab,
        ),
        ToolDef::new(
            "switch_to_default_window",
            "Switch To Default Window",
            "Switch back to the first tab.",
            Effect::Set,
            Schema::new(),
            w::switch_to_default_window,
        ),
        ToolDef::new(
            "switch_to_frame",
            "Switch To Frame",
            "Move focus into an iframe so later tools act inside it.",
            Effect::Act,
            Schema::new().optional(
                "selector",
                Prop::string("Selector of the iframe").default("iframe"),
            ),
            w::switch_to_frame,
        ),
        ToolDef::new(
            "switch_to_default_content",
            "Switch To Default Content",
            "Move focus back out to the main page.",
            Effect::Set,
            Schema::new(),
            w::switch_to_default_content,
        ),
    ]
}
