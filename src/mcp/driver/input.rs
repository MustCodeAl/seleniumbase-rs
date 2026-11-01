//! Clicking, typing and choosing options.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

fn dropdown_schema(option: Prop) -> Schema {
    Schema::new()
        .required(
            "dropdown_selector",
            Prop::string("Selector of the <select> element"),
        )
        .required("option", option)
}

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
            "type_text",
            "Type Text",
            "Type into an input or textarea, clearing it first unless told not to.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .required("text", Prop::string("The text to type"))
                .optional(
                    "clear_first",
                    Prop::boolean("Clear the field's existing content first").default(true),
                )
                .optional("timeout", timeout_prop(5.0)),
            w::type_text,
        ),
        ToolDef::new(
            "select_option_by_text",
            "Select Option By Text",
            "Choose a <select> option by its visible text.",
            Effect::Act,
            dropdown_schema(Prop::string("The option's visible text")),
            w::select_option_by_text,
        ),
        ToolDef::new(
            "select_option_by_value",
            "Select Option By Value",
            "Choose a <select> option by its value attribute.",
            Effect::Act,
            dropdown_schema(Prop::string("The option's value attribute")),
            w::select_option_by_value,
        ),
        ToolDef::new(
            "select_option_by_index",
            "Select Option By Index",
            "Choose a <select> option by its 0-based position.",
            Effect::Act,
            dropdown_schema(Prop::string(
                "The 0-based index, as a number or numeric string",
            )),
            w::select_option_by_index,
        ),
    ]
}
