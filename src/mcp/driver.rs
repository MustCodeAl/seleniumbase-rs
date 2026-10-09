//! The `driver` server: the 26 tools of upstream's `Driver()` server, on
//! [`BaseCase`].

use super::webdriver::{self as w, selector_prop, start_schema, timeout_prop};
use super::{Effect, Host, Prop, Schema, Settings, ToolDef};
use crate::api::base_case::BaseCase;

const SERVER_NAME: &str = "seleniumbase-driver";

const INSTRUCTIONS: &str = "\
Drives a browser through WebDriver, with undetected mode on by default. Call \
start_browser once, use the other tools, then call close_browser. Failed actions \
come back as errors you can read and retry; use wait_for_element before acting on \
content that loads late.";

/// Builds the `driver` server.
#[must_use]
pub fn host(settings: Settings) -> Host<BaseCase> {
    Host::new(SERVER_NAME, INSTRUCTIONS, tools(), settings)
}

fn dropdown_schema(option: Prop) -> Schema {
    Schema::new()
        .required(
            "dropdown_selector",
            Prop::string("Selector of the <select> element"),
        )
        .required("option", option)
}

#[expect(
    clippy::too_many_lines,
    reason = "the tool table reads best as one list"
)]
fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "start_browser",
            "Start Browser",
            "Start the browser session the other tools use. Does nothing if one is running.",
            Effect::Set,
            start_schema(),
            w::start_browser,
        ),
        ToolDef::new(
            "close_browser",
            "Close Browser",
            "Close the browser and end the session. Safe to call when none is running.",
            Effect::Set,
            Schema::new(),
            w::close_browser,
        ),
        ToolDef::new(
            "open_url",
            "Open URL",
            "Navigate to a URL and wait for the document to load. A bare host gets https:// added.",
            Effect::Act,
            Schema::new().required("url", Prop::string("Address to open")),
            w::open_url,
        ),
        ToolDef::new(
            "go_back",
            "Go Back",
            "Go back one page in the browser history.",
            Effect::Act,
            Schema::new(),
            w::go_back,
        ),
        ToolDef::new(
            "go_forward",
            "Go Forward",
            "Go forward one page in the browser history.",
            Effect::Act,
            Schema::new(),
            w::go_forward,
        ),
        ToolDef::new(
            "refresh_page",
            "Refresh Page",
            "Reload the current page.",
            Effect::Act,
            Schema::new(),
            w::refresh_page,
        ),
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
            dropdown_schema(Prop::string("The 0-based index, as a number or numeric string")),
            w::select_option_by_index,
        ),
        ToolDef::new(
            "wait_for_element",
            "Wait For Element",
            "Wait until an element is visible. Fails if the timeout passes first.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(10.0)),
            |ctx, args| w::wait_for_element(ctx, args, 10.0),
        ),
        ToolDef::new(
            "switch_to_frame",
            "Switch To Frame",
            "Move focus into an iframe so later tools act inside it.",
            Effect::Act,
            Schema::new().required("selector", Prop::string("Selector of the iframe")),
            w::switch_to_frame,
        ),
        ToolDef::new(
            "switch_to_default_content",
            "Switch To Default Content",
            "Move focus back out to the main page.",
            Effect::Set,
            Schema::new(),
            w::switch_to_default_content,
        ),
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
        ToolDef::new(
            "activate_cdp_mode",
            "Activate CDP Mode",
            "Switch the session to CDP mode, which adds stealth and DevTools-based methods. \
             Optionally open a URL. Needs a Chromium browser.",
            Effect::Act,
            Schema::new().optional("url", Prop::string("Page to open once active")),
            w::activate_cdp_mode,
        ),
        ToolDef::new(
            "solve_captcha",
            "Solve CAPTCHA",
            "Try to pass a CAPTCHA widget on the page (Turnstile, reCAPTCHA, hCaptcha, Friendly \
             Captcha, DataDome). Does nothing if none is found; check the page afterwards.",
            Effect::Act,
            Schema::new(),
            w::solve_captcha,
        ),
        ToolDef::new(
            "save_screenshot",
            "Save Screenshot",
            "Save a PNG screenshot of the page in the server's output directory.",
            Effect::Overwrite,
            Schema::new().optional(
                "filename",
                Prop::string("File name").default("screenshot.png"),
            ),
            w::save_screenshot_file,
        ),
        ToolDef::new(
            "execute_script",
            "Execute Script",
            "Run JavaScript in the page and return the result; use `return` to produce a value.",
            Effect::Mixed,
            Schema::new().required("script", Prop::string("The JavaScript to run")),
            w::execute_script,
        ),
    ]
}
