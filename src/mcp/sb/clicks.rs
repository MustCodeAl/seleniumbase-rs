//! Clicking elements and links.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "click_element",
            "Click Element",
            "Click an element, waiting up to the timeout for it to appear.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::click_element,
        ),
        ToolDef::new(
            "click_if_visible",
            "Click If Visible",
            "Click an element only if it is visible now; otherwise do nothing.",
            Effect::Act,
            Schema::new().required("selector", selector_prop()),
            w::click_if_visible,
        ),
        ToolDef::new(
            "click_visible_elements",
            "Click Visible Elements",
            "Click every visible element that matches, in page order, optionally only the \
first few.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "limit",
                    Prop::integer("Most elements to click; 0 means all").default(0),
                ),
            w::click_visible_elements,
        ),
        ToolDef::new(
            "click_nth_visible_element",
            "Click Nth Visible Element",
            "Click the nth visible element that matches, counting from 1.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .required(
                    "number",
                    Prop::integer("Which visible match, counting from 1"),
                ),
            w::click_nth_visible_element,
        ),
        ToolDef::new(
            "click_link",
            "Click Link",
            "Click a link by its exact visible text.",
            Effect::Act,
            Schema::new().required("link_text", Prop::string("The link's text")),
            w::click_link,
        ),
        ToolDef::new(
            "double_click",
            "Double Click",
            "Double-click an element.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::double_click,
        ),
        ToolDef::new(
            "context_click",
            "Context Click",
            "Right-click an element.",
            Effect::Act,
            Schema::new().required("selector", selector_prop()),
            w::context_click,
        ),
    ]
}
