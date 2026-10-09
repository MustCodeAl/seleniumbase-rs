//! The `sb` server: the 88 tools of upstream's `SB()` server, on [`BaseCase`],
//! plus this crate's own stealth tools.

use super::webdriver::{self as w, selector_prop, start_schema, timeout_prop};
use super::{stealth, Effect, Host, Prop, Schema, Settings, ToolDef};
use crate::api::base_case::BaseCase;

const SERVER_NAME: &str = "seleniumbase-sb";

const INSTRUCTIONS: &str = "\
Drives a browser through WebDriver with SeleniumBase's broad toolset, with undetected \
mode on by default. Call start_browser once, use the other tools, then call \
close_browser. Failed actions come back as errors you can read and retry; use the \
wait_for_* tools before acting on content that loads late. The stealth tools at the \
end work without a browser.";

/// Builds the `sb` server.
#[must_use]
pub fn host(settings: Settings) -> Host<BaseCase> {
    let mut tools = tools();
    tools.extend(stealth::tools());
    Host::new(SERVER_NAME, INSTRUCTIONS, tools, settings)
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
            "Start the browser session the other tools use. Does nothing if one is \
running.",
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
            "get_page_info",
            "Get Page Info",
            "Report whether a browser is running and, if so, the page's URL, title, \
origin and user agent.",
            Effect::Observe,
            Schema::new(),
            w::get_page_info,
        ),
        ToolDef::new(
            "open_url",
            "Open URL",
            "Navigate to a URL and wait for the document to load. A bare host gets \
https:// added.",
            Effect::Act,
            Schema::new().required("url", Prop::string("Address to open")),
            w::open_url,
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
        ToolDef::new(
            "hover_and_click",
            "Hover And Click",
            "Hover one element, then click another; for menus that open on hover.",
            Effect::Act,
            Schema::new()
                .required("hover_selector", Prop::string("Element to hover"))
                .required(
                    "click_selector",
                    Prop::string("Element to click afterwards"),
                )
                .optional("timeout", timeout_prop(5.0)),
            w::hover_and_click,
        ),
        ToolDef::new(
            "drag_and_drop",
            "Drag And Drop",
            "Drag one element onto another.",
            Effect::Act,
            Schema::new()
                .required("drag_selector", Prop::string("Element to drag"))
                .required("drop_selector", Prop::string("Element to drop it on"))
                .optional("timeout", timeout_prop(5.0)),
            w::drag_and_drop,
        ),
        ToolDef::new(
            "nested_click",
            "Nested Click",
            "Click an element inside a parent element, or inside a parent iframe.",
            Effect::Act,
            Schema::new()
                .required("parent_selector", Prop::string("The container or iframe"))
                .required("selector", selector_prop()),
            w::nested_click,
        ),
        ToolDef::new(
            "choose_file",
            "Choose File",
            "Fill a file input with a file from this machine.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .required(
                    "file_path",
                    Prop::string("Path of the file on this machine"),
                )
                .optional("timeout", timeout_prop(5.0)),
            w::choose_file,
        ),
        ToolDef::new(
            "wait_for_element",
            "Wait For Element",
            "Wait until an element is visible. Fails if the timeout passes first.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            |ctx, args| w::wait_for_element(ctx, args, 5.0),
        ),
        ToolDef::new(
            "wait_for_element_present",
            "Wait For Element Present",
            "Wait until an element exists in the page, visible or not.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::wait_for_element_present,
        ),
        ToolDef::new(
            "wait_for_element_not_visible",
            "Wait For Element Not Visible",
            "Wait until an element is no longer visible.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::wait_for_element_not_visible,
        ),
        ToolDef::new(
            "wait_for_element_absent",
            "Wait For Element Absent",
            "Wait until an element is gone from the page.",
            Effect::Inspect,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(5.0)),
            w::wait_for_element_absent,
        ),
        ToolDef::new(
            "wait_for_text",
            "Wait For Text",
            "Wait until some text is visible within an element.",
            Effect::Inspect,
            Schema::new()
                .required("text", Prop::string("The text to wait for"))
                .optional("selector", selector_prop().default("html"))
                .optional("timeout", timeout_prop(5.0)),
            w::wait_for_text,
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
        ToolDef::new(
            "get_cookies",
            "Get Cookies",
            "List the session's cookies. They can hold logins; handle them with care.",
            Effect::Observe,
            Schema::new(),
            w::get_cookies,
        ),
        ToolDef::new(
            "delete_all_cookies",
            "Delete All Cookies",
            "Delete every cookie in the session.",
            Effect::Act,
            Schema::new(),
            w::delete_all_cookies,
        ),
        ToolDef::new(
            "save_cookies",
            "Save Cookies",
            "Save the session's cookies to a file in the server's output directory, under \
saved_cookies.",
            Effect::Overwrite,
            Schema::new().optional(
                "name",
                Prop::string("File name; only its last path part is used").default("cookies.txt"),
            ),
            w::save_cookies,
        ),
        ToolDef::new(
            "load_cookies",
            "Load Cookies",
            "Load cookies from a file saved by save_cookies.",
            Effect::Act,
            Schema::new().optional(
                "name",
                Prop::string("File name; only its last path part is used").default("cookies.txt"),
            ),
            w::load_cookies,
        ),
        ToolDef::new(
            "get_local_storage_item",
            "Get Local Storage Item",
            "Read one key of localStorage for the current page's origin.",
            Effect::Observe,
            Schema::new().required("key", Prop::string("The storage key")),
            w::get_local_storage_item,
        ),
        ToolDef::new(
            "set_local_storage_item",
            "Set Local Storage Item",
            "Write one key of localStorage for the current page's origin.",
            Effect::Set,
            Schema::new()
                .required("key", Prop::string("The storage key"))
                .required("value", Prop::string("The value to store")),
            w::set_local_storage_item,
        ),
        ToolDef::new(
            "get_session_storage_item",
            "Get Session Storage Item",
            "Read one key of sessionStorage for the current page's origin.",
            Effect::Observe,
            Schema::new().required("key", Prop::string("The storage key")),
            w::get_session_storage_item,
        ),
        ToolDef::new(
            "set_session_storage_item",
            "Set Session Storage Item",
            "Write one key of sessionStorage for the current page's origin.",
            Effect::Set,
            Schema::new()
                .required("key", Prop::string("The storage key"))
                .required("value", Prop::string("The value to store")),
            w::set_session_storage_item,
        ),
        ToolDef::new(
            "scroll_into_view",
            "Scroll Into View",
            "Scroll an element into view.",
            Effect::Set,
            Schema::new().required("selector", selector_prop()),
            w::scroll_into_view,
        ),
        ToolDef::new(
            "scroll_to_top",
            "Scroll To Top",
            "Scroll to the top of the page.",
            Effect::Set,
            Schema::new(),
            w::scroll_to_top,
        ),
        ToolDef::new(
            "scroll_to_bottom",
            "Scroll To Bottom",
            "Scroll to the bottom of the page.",
            Effect::Set,
            Schema::new(),
            w::scroll_to_bottom,
        ),
        ToolDef::new(
            "scroll_up",
            "Scroll Up",
            "Scroll up by a share of the window height.",
            Effect::Act,
            Schema::new().optional(
                "amount",
                Prop::integer("Percent of the window height")
                    .default(25)
                    .min(0.0),
            ),
            w::scroll_up,
        ),
        ToolDef::new(
            "scroll_down",
            "Scroll Down",
            "Scroll down by a share of the window height.",
            Effect::Act,
            Schema::new().optional(
                "amount",
                Prop::integer("Percent of the window height")
                    .default(25)
                    .min(0.0),
            ),
            w::scroll_down,
        ),
        ToolDef::new(
            "get_window_rect",
            "Get Window Rect",
            "Get the browser window's position and size.",
            Effect::Observe,
            Schema::new(),
            w::get_window_rect,
        ),
        ToolDef::new(
            "maximize_window",
            "Maximize Window",
            "Maximize the browser window.",
            Effect::Set,
            Schema::new(),
            w::maximize_window,
        ),
        ToolDef::new(
            "minimize_window",
            "Minimize Window",
            "Minimize the browser window.",
            Effect::Set,
            Schema::new(),
            w::minimize_window,
        ),
        ToolDef::new(
            "open_new_tab",
            "Open New Tab",
            "Open a new tab, and switch to it unless told not to.",
            Effect::Act,
            Schema::new().optional(
                "switch_to",
                Prop::boolean("Make the new tab the active one").default(true),
            ),
            w::open_new_tab,
        ),
        ToolDef::new(
            "switch_to_newest_tab",
            "Switch To Newest Tab",
            "Switch to the most recently opened tab.",
            Effect::Set,
            Schema::new(),
            w::switch_to_newest_tab,
        ),
        ToolDef::new(
            "switch_to_default_window",
            "Switch To Default Window",
            "Switch back to the first tab.",
            Effect::Set,
            Schema::new(),
            w::switch_to_default_window,
        ),
        ToolDef::new(
            "switch_to_frame",
            "Switch To Frame",
            "Move focus into an iframe so later tools act inside it.",
            Effect::Act,
            Schema::new().optional(
                "selector",
                Prop::string("Selector of the iframe").default("iframe"),
            ),
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
            "activate_cdp_mode",
            "Activate CDP Mode",
            "Switch the session to CDP mode, which adds stealth and DevTools-based \
methods; optionally open a URL. Needs a Chromium browser.",
            Effect::Act,
            Schema::new().optional("url", Prop::string("Page to open once active")),
            w::activate_cdp_mode,
        ),
        ToolDef::new(
            "solve_captcha",
            "Solve CAPTCHA",
            "Try to pass a CAPTCHA widget on the page (Turnstile, reCAPTCHA, hCaptcha, \
Friendly Captcha, DataDome). Does nothing if none is found; check the page \
afterwards.",
            Effect::Act,
            Schema::new(),
            w::solve_captcha,
        ),
        ToolDef::new(
            "get_mfa_code",
            "Get MFA Code",
            "Compute the current one-time code for a TOTP secret.",
            Effect::Observe,
            Schema::new().required("totp_key", Prop::string("The base32 TOTP secret")),
            w::get_mfa_code,
        ),
        ToolDef::new(
            "enter_mfa_code",
            "Enter MFA Code",
            "Type the current one-time code for a TOTP secret into a field.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .required("totp_key", Prop::string("The base32 TOTP secret")),
            w::enter_mfa_code,
        ),
        ToolDef::new(
            "save_screenshot",
            "Save Screenshot",
            "Save a PNG screenshot of the page in the server's output directory.",
            Effect::Overwrite,
            Schema::new()
                .optional("name", Prop::string("File name").default("screenshot.png"))
                .optional("folder", Prop::string("Sub-folder of the output directory")),
            w::save_screenshot,
        ),
        ToolDef::new(
            "save_page_source",
            "Save Page Source",
            "Save the page's HTML in the server's output directory.",
            Effect::Overwrite,
            Schema::new()
                .optional(
                    "name",
                    Prop::string("File name").default("page_source.html"),
                )
                .optional("folder", Prop::string("Sub-folder of the output directory")),
            w::save_page_source,
        ),
        ToolDef::new(
            "print_to_pdf",
            "Print To PDF",
            "Save the page as a PDF in the server's output directory.",
            Effect::Overwrite,
            Schema::new()
                .optional("name", Prop::string("File name").default("page.pdf"))
                .optional("folder", Prop::string("Sub-folder of the output directory")),
            w::print_to_pdf,
        ),
        ToolDef::new(
            "download_file",
            "Download File",
            "Download an http(s) URL, fetched by this server, into its output directory.",
            Effect::Overwrite,
            Schema::new()
                .required("file_url", Prop::string("The address to download"))
                .optional(
                    "destination_folder",
                    Prop::string("Sub-folder of the output directory").default("downloaded_files"),
                ),
            w::download_file,
        ),
        ToolDef::new(
            "evaluate",
            "Evaluate",
            "Evaluate a JavaScript expression in the page via the DevTools Protocol and \
return the result; a promise is awaited.",
            Effect::Mixed,
            Schema::new().required(
                "expression",
                Prop::string("Code to evaluate, such as document.title"),
            ),
            w::evaluate,
        ),
        ToolDef::new(
            "execute_script",
            "Execute Script",
            "Run JavaScript in the page and return the result; use `return` to produce a \
value.",
            Effect::Mixed,
            Schema::new().required("script", Prop::string("The JavaScript to run")),
            w::execute_script,
        ),
        ToolDef::new(
            "highlight",
            "Highlight",
            "Briefly outline an element, a few times, to show where it is.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional("loops", Prop::integer("How many times to flash").default(4))
                .optional("timeout", timeout_prop(5.0)),
            w::highlight,
        ),
        ToolDef::new(
            "flash",
            "Flash",
            "Briefly flash an element.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "duration",
                    Prop::number("Roughly how many seconds to flash for")
                        .min(0.0)
                        .default(1),
                ),
            w::flash,
        ),
        ToolDef::new(
            "sleep",
            "Sleep",
            "Pause for a number of seconds.",
            Effect::Inspect,
            Schema::new().required("seconds", Prop::number("How long to pause").min(0.0)),
            w::sleep,
        ),
    ]
}
