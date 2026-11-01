//! Running JavaScript in the page.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![ToolDef::new(
        "execute_script",
        "Execute Script",
        "Run JavaScript in the page and return the result; use `return` to produce a value.",
        Effect::Mixed,
        Schema::new().required("script", Prop::string("The JavaScript to run")),
        w::execute_script,
    )]
}
