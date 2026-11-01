//! Counting elements and asking whether they are present, visible, clickable or
//! selected.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "find_elements_count",
            "Find Elements Count",
            "Count the elements that match a selector.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::find_elements_count,
        ),
        ToolDef::new(
            "is_element_present",
            "Is Element Present",
            "Say whether an element matching the selector exists in the page.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::is_element_present,
        ),
        ToolDef::new(
            "is_element_visible",
            "Is Element Visible",
            "Say whether an element matching the selector is visible.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::is_element_visible,
        ),
        ToolDef::new(
            "is_element_clickable",
            "Is Element Clickable",
            "Say whether an element matching the selector can be clicked.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::is_element_clickable,
        ),
        ToolDef::new(
            "is_text_visible",
            "Is Text Visible",
            "Say whether some text is visible within an element.",
            Effect::Observe,
            Schema::new()
                .required("text", Prop::string("The text to look for"))
                .optional("selector", selector_prop().default("html")),
            w::is_text_visible,
        ),
        ToolDef::new(
            "is_selected",
            "Is Selected",
            "Say whether a checkbox or radio button is selected.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::is_selected,
        ),
    ]
}
