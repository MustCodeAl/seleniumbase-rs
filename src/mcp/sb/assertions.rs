//! Assertions on elements, text, the title and the URL, and on page errors.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop, timeout_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
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
        ToolDef::new(
            "assert_element_present",
            "Assert Element Present",
            "Verify that an element exists in the page. A failure is an error.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::assert_element_present,
        ),
        ToolDef::new(
            "assert_element_not_visible",
            "Assert Element Not Visible",
            "Verify that an element is not visible. A failure is an error.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::assert_element_not_visible,
        ),
        ToolDef::new(
            "assert_text",
            "Assert Text",
            "Verify that text is visible within an element. A failure is an error.",
            Effect::Inspect,
            Schema::new()
                .required("text", Prop::string("The text that must be visible"))
                .optional("selector", selector_prop().default("html"))
                .optional("timeout", timeout_prop(5.0)),
            w::assert_text,
        ),
        ToolDef::new(
            "assert_exact_text",
            "Assert Exact Text",
            "Verify that an element's text is exactly the given text. A failure is an \
error.",
            Effect::Inspect,
            Schema::new()
                .required("text", Prop::string("The exact text"))
                .optional("selector", selector_prop().default("html"))
                .optional("timeout", timeout_prop(5.0)),
            w::assert_exact_text,
        ),
        ToolDef::new(
            "assert_title",
            "Assert Title",
            "Verify the page title matches exactly. A failure is an error.",
            Effect::Inspect,
            Schema::new().required("title", Prop::string("The expected title")),
            w::assert_title,
        ),
        ToolDef::new(
            "assert_url",
            "Assert URL",
            "Verify the page URL matches exactly. A failure is an error.",
            Effect::Inspect,
            Schema::new().required("url", Prop::string("The expected URL")),
            w::assert_url,
        ),
        ToolDef::new(
            "assert_url_contains",
            "Assert URL Contains",
            "Verify the page URL contains some text. A failure is an error.",
            Effect::Inspect,
            Schema::new().required("substring", Prop::string("Text the URL must contain")),
            w::assert_url_contains,
        ),
        ToolDef::new(
            "assert_no_404_errors",
            "Assert No 404 Errors",
            "Verify that none of the page's links are broken (404). A failure is an \
error.",
            Effect::Inspect,
            Schema::new(),
            w::assert_no_404_errors,
        ),
        ToolDef::new(
            "assert_no_js_errors",
            "Assert No JS Errors",
            "Verify that the page logged no JavaScript errors. A failure is an error.",
            Effect::Inspect,
            Schema::new(),
            w::assert_no_js_errors,
        ),
    ]
}
