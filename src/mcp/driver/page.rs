//! Reading the page: its address, title, source and text, and the state of
//! elements.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop};
use crate::mcp::{Effect, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "get_current_url",
            "Get Current URL",
            "Get the URL of the current page.",
            Effect::Observe,
            Schema::new(),
            w::get_current_url,
        ),
        ToolDef::new(
            "get_title",
            "Get Title",
            "Get the title of the current page.",
            Effect::Observe,
            Schema::new(),
            w::get_title,
        ),
        ToolDef::new(
            "get_page_source",
            "Get Page Source",
            "Get the full HTML source of the current page.",
            Effect::Observe,
            Schema::new(),
            w::get_page_source,
        ),
        ToolDef::new(
            "get_text",
            "Get Text",
            "Get the visible text of an element. Fails if it does not appear within the default wait.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::get_text,
        ),
        ToolDef::new(
            "find_elements_count",
            "Find Elements Count",
            "Count the elements on the page that match a selector.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::find_elements_count,
        ),
        ToolDef::new(
            "is_element_visible",
            "Is Element Visible",
            "Say whether an element matching the selector is visible right now.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::is_element_visible,
        ),
    ]
}
