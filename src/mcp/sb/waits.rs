//! Waiting for elements and text.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "wait_for_element",
            "Wait For Element",
            "Wait until an element is visible. Fails if the timeout passes first.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            |ctx, args| w::wait_for_element(ctx, args, 5.0),
        ),
        ToolDef::new(
            "wait_for_element_present",
            "Wait For Element Present",
            "Wait until an element exists in the page, visible or not.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::wait_for_element_present,
        ),
        ToolDef::new(
            "wait_for_element_not_visible",
            "Wait For Element Not Visible",
            "Wait until an element is no longer visible.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::wait_for_element_not_visible,
        ),
        ToolDef::new(
            "wait_for_element_absent",
            "Wait For Element Absent",
            "Wait until an element is gone from the page.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::wait_for_element_absent,
        ),
        ToolDef::new(
            "wait_for_text",
            "Wait For Text",
            "Wait until some text is visible within an element.",
            Effect::Inspect,
            Schema::new()
                .required("text", Prop::string("The text to wait for"))
                .optional("selector", selector_prop().default("html"))
                .optional("timeout", timeout_prop(5.0)),
            w::wait_for_text,
        ),
    ]
}
