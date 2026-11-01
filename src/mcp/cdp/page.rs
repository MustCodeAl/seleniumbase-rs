//! Reading the page: elements, their content and attributes, and quick checks.

use super::{
    check_if_condition, find_elements, get_attributes, get_content, selector_prop, timeout_prop,
    Cdp,
};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "find_elements",
            "Find Elements",
            "List the elements matching a selector with their tag and text. Elements cannot \
             be held between calls; to act on one, use a tool that takes the selector.",
            Effect::Observe,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(0.5))
                .optional(
                    "include_html",
                    Prop::boolean("Also return each element's outer HTML").default(false),
                ),
            find_elements,
        ),
        ToolDef::new(
            "get_content",
            "Get Content",
            "Read an element's visible text or HTML, or the absolute URLs found in it. Reads \
             the whole page body by default.",
            Effect::Observe,
            Schema::new()
                .optional("selector", selector_prop().default("body"))
                .optional(
                    "output_format",
                    Prop::choice(&["text", "html", "urls"], "What to return").default("text"),
                )
                .optional("timeout", timeout_prop(5.0)),
            get_content,
        ),
        ToolDef::new(
            "get_attributes",
            "Get Attributes",
            "Read one attribute of the first matching element, or all of them when no \
             attribute is named.",
            Effect::Observe,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "attribute",
                    Prop::string("Attribute name, such as href or value"),
                )
                .optional("timeout", timeout_prop(5.0)),
            get_attributes,
        ),
        ToolDef::new(
            "check_if_condition",
            "Check Condition",
            "Answer true or false right now whether an element is present or visible, or \
             whether some text is visible in it. Does not wait; use wait_for_condition to wait.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "check",
                    Prop::choice(
                        &["present", "visible"],
                        "What to check; ignored when text is given",
                    )
                    .default("visible"),
                )
                .optional("selector", selector_prop().default("body"))
                .optional(
                    "text",
                    Prop::string("Check that this text is visible in the element"),
                ),
            check_if_condition,
        ),
    ]
}
