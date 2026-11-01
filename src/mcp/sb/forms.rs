//! Typing into fields, submitting forms and choosing options.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
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
            "set_value",
            "Set Value",
            "Set an input's value directly, without key events.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .required("text", Prop::string("The value to set"))
                .optional("timeout", timeout_prop(5.0)),
            w::set_value,
        ),
        ToolDef::new(
            "clear_input",
            "Clear Input",
            "Empty an input or textarea.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::clear_input,
        ),
        ToolDef::new(
            "submit",
            "Submit",
            "Submit the form that contains an element.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::submit,
        ),
        ToolDef::new(
            "select_option_by_text",
            "Select Option By Text",
            "Choose a <select> option by its visible text.",
            Effect::Act,
            Schema::new()
                .required(
                    "dropdown_selector",
                    Prop::string("Selector of the <select> element"),
                )
                .required("option", Prop::string("The option's visible text"))
                .optional("timeout", timeout_prop(5.0)),
            w::select_option_by_text,
        ),
        ToolDef::new(
            "select_option_by_value",
            "Select Option By Value",
            "Choose a <select> option by its value attribute.",
            Effect::Act,
            Schema::new()
                .required(
                    "dropdown_selector",
                    Prop::string("Selector of the <select> element"),
                )
                .required("option", Prop::string("The option's value attribute"))
                .optional("timeout", timeout_prop(5.0)),
            w::select_option_by_value,
        ),
        ToolDef::new(
            "select_option_by_index",
            "Select Option By Index",
            "Choose a <select> option by its 0-based position.",
            Effect::Act,
            Schema::new()
                .required(
                    "dropdown_selector",
                    Prop::string("Selector of the <select> element"),
                )
                .required(
                    "option",
                    Prop::string("The 0-based index, as a number or numeric string"),
                )
                .optional("timeout", timeout_prop(5.0)),
            w::select_option_by_index,
        ),
    ]
}
