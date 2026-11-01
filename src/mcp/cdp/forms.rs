//! Typing, choosing options and focus.

use super::{focus_element, select_option, selector_prop, timeout_prop, type_text, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "type_text",
            "Type Text",
            "Enter or clear text in an input, textarea or editable element. A newline in \
             the text presses Enter, except in set_value mode.",
            Effect::Mixed,
            Schema::new()
                .required("selector", selector_prop())
                .optional("text", Prop::string("The text to enter").default(""))
                .optional(
                    "mode",
                    Prop::choice(
                        &[
                            "fill_input",
                            "append",
                            "fast_type",
                            "set_value",
                            "clear_only",
                        ],
                        "fill_input clears then types key by key; append types after the \
                         current value; fast_type clears then inserts in one step; set_value \
                         assigns the value without key events; clear_only empties the field",
                    )
                    .default("fill_input"),
                )
                .optional("timeout", timeout_prop(5.0)),
            type_text,
        ),
        ToolDef::new(
            "select_option",
            "Select Option",
            "Choose an option in a native <select> dropdown. For dropdowns built from \
             divs or buttons, click them instead.",
            Effect::Act,
            Schema::new()
                .required(
                    "dropdown_selector",
                    Prop::string("Selector of the <select> element"),
                )
                .required(
                    "value",
                    Prop::string("The option's text, value or 0-based index"),
                )
                .optional(
                    "by",
                    Prop::choice(&["text", "value", "index"], "How to match the option")
                        .default("text"),
                ),
            select_option,
        ),
        ToolDef::new(
            "focus_element",
            "Focus Element",
            "Scroll an element into view, give it keyboard focus, or briefly outline it. \
             Does not click or type.",
            Effect::Set,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "action",
                    Prop::choice(&["scroll_to_element", "focus", "highlight"], "What to do")
                        .default("scroll_to_element"),
                )
                .optional("timeout", timeout_prop(5.0)),
            focus_element,
        ),
    ]
}
