//! Assertions on text and elements.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "assert_text",
            "Assert Text",
            "Verify that text is visible on the page, or within one element. A failure is an error.",
            Effect::Inspect,
            Schema::new()
                .required("text", Prop::string("The text that must be visible"))
                .optional("selector", Prop::string("Limit the check to this element"))
                .optional("timeout", timeout_prop(5.0)),
            w::assert_text,
        ),
        ToolDef::new(
            "assert_element",
            "Assert Element",
            "Verify that an element is visible. A failure is an error.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::assert_element,
        ),
    ]
}
