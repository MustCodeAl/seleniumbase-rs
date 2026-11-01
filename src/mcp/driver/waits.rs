//! Waiting for an element.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![ToolDef::new(
        "wait_for_element",
        "Wait For Element",
        "Wait until an element is visible. Fails if the timeout passes first.",
        Effect::Inspect,
        Schema::new()
            .required("selector", selector_prop())
            .optional("timeout", timeout_prop(10.0)),
        |ctx, args| w::wait_for_element(ctx, args, 10.0),
    )]
}
