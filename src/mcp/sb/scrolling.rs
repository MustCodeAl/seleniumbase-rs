//! Scrolling the page.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "scroll_into_view",
            "Scroll Into View",
            "Scroll an element into view.",
            Effect::Set,
            Schema::new().required("selector", selector_prop()),
            w::scroll_into_view,
        ),
        ToolDef::new(
            "scroll_to_top",
            "Scroll To Top",
            "Scroll to the top of the page.",
            Effect::Set,
            Schema::new(),
            w::scroll_to_top,
        ),
        ToolDef::new(
            "scroll_to_bottom",
            "Scroll To Bottom",
            "Scroll to the bottom of the page.",
            Effect::Set,
            Schema::new(),
            w::scroll_to_bottom,
        ),
        ToolDef::new(
            "scroll_up",
            "Scroll Up",
            "Scroll up by a share of the window height.",
            Effect::Act,
            Schema::new().optional(
                "amount",
                Prop::integer("Percent of the window height")
                    .default(25)
                    .min(0.0),
            ),
            w::scroll_up,
        ),
        ToolDef::new(
            "scroll_down",
            "Scroll Down",
            "Scroll down by a share of the window height.",
            Effect::Act,
            Schema::new().optional(
                "amount",
                Prop::integer("Percent of the window height")
                    .default(25)
                    .min(0.0),
            ),
            w::scroll_down,
        ),
    ]
}
