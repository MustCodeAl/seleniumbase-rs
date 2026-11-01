//! Running JavaScript in the page.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "evaluate",
            "Evaluate",
            "Evaluate a JavaScript expression in the page via the DevTools Protocol and \
return the result; a promise is awaited.",
            Effect::Mixed,
            Schema::new().required(
                "expression",
                Prop::string("Code to evaluate, such as document.title"),
            ),
            w::evaluate,
        ),
        ToolDef::new(
            "execute_script",
            "Execute Script",
            "Run JavaScript in the page and return the result; use `return` to produce a \
value.",
            Effect::Mixed,
            Schema::new().required("script", Prop::string("The JavaScript to run")),
            w::execute_script,
        ),
    ]
}
