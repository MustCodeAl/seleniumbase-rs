//! Reading the page: its address, title, user agent, text and source.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

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
            "get_origin",
            "Get Origin",
            "Get the origin (scheme, host and port) of the current page.",
            Effect::Observe,
            Schema::new(),
            w::get_origin,
        ),
        ToolDef::new(
            "get_user_agent",
            "Get User Agent",
            "Get the browser's user agent string.",
            Effect::Observe,
            Schema::new(),
            w::get_user_agent,
        ),
        ToolDef::new(
            "get_text",
            "Get Text",
            "Get the visible text of an element; the whole page body by default.",
            Effect::Observe,
            Schema::new().optional("selector", selector_prop().default("body")),
            w::get_text,
        ),
        ToolDef::new(
            "get_html_source",
            "Get HTML Source",
            "Get the full HTML source of the current page.",
            Effect::Observe,
            Schema::new(),
            w::get_page_source,
        ),
        ToolDef::new(
            "get_element_html",
            "Get Element HTML",
            "Get the outer HTML of one element.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            w::get_element_html,
        ),
        ToolDef::new(
            "get_attribute",
            "Get Attribute",
            "Get one attribute of an element.",
            Effect::Observe,
            Schema::new()
                .required("selector", selector_prop())
                .required(
                    "attribute",
                    Prop::string("Attribute name, such as href or value"),
                ),
            w::get_attribute,
        ),
    ]
}
