//! Running JavaScript in the page.

use super::{run_javascript, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![ToolDef::new(
        "run_javascript",
        "Run JavaScript",
        "Evaluate JavaScript in the active tab and return the result. A returned promise \
         is awaited. Use the dedicated tools where one fits.",
        Effect::Mixed,
        Schema::new().required(
            "expression",
            Prop::string("Code to evaluate, such as document.title"),
        ),
        run_javascript,
    )]
}
