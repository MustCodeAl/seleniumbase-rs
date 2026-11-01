//! Moving focus into an iframe and back out.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "switch_to_frame",
            "Switch To Frame",
            "Move focus into an iframe so later tools act inside it.",
            Effect::Act,
            Schema::new().required("selector", Prop::string("Selector of the iframe")),
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
