//! The Pure CDP server: [`sb_cdp`](crate::sb_cdp) as MCP tools.
//!
//! One Chrome session is shared by every call. `start_browser` launches it,
//! the other tools drive it, and `close_browser` ends it. Related actions are
//! grouped behind one tool with an `action`, `mode` or `state` argument, which
//! keeps the tool list short enough for a model to choose from.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::{json, Value};
use url::Url;

use super::support::{cookie_file, is_missing, json_output, nonempty, write_file};
use super::{
    Args, Closeable, Ctx, Effect, Host, Output, Prop, Schema, Settings, Started, ToolDef, ToolError,
};
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Browser, Key, LaunchOptions, Locator, Page, Scroll, SelectBy, State};

const SERVER_NAME: &str = "seleniumbase-cdp";

const INSTRUCTIONS: &str = "\
Drives Chrome over the DevTools Protocol with no WebDriver, so pages see an \
ordinary browser. Call start_browser once, use the other tools, then call \
close_browser. Selectors may be CSS (preferred), XPath, or `tag:contains(\"text\")`. \
Failed actions come back as errors you can read and retry: fix the selector, wait \
for the page to settle with wait_for_condition, or look at the page with \
get_content.";

/// How long the pointer rests on a menu before the item under it is clicked.
const HOVER_PAUSE: Duration = Duration::from_millis(250);

/// How long a check that must not wait is allowed to look.
const NO_WAIT: Duration = Duration::ZERO;

/// The session behind the Pure CDP server: a browser and the tab in use.
#[derive(Debug)]
pub struct Cdp {
    browser: Browser,
    page: Page,
}

impl Cdp {
    /// Serves an existing browser, starting on its first tab.
    ///
    /// Use this to expose a browser you launched or connected to yourself,
    /// instead of letting `start_browser` launch one.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser has no tab to start on.
    pub async fn attach(browser: Browser) -> Result<Self, SeleniumBaseError> {
        let page = browser.default_page().await?;
        Ok(Self { browser, page })
    }
}

impl Closeable for Cdp {
    async fn close(self) {
        // The server is shutting down; there is nobody left to report to.
        let _ = self.browser.close().await;
    }
}

/// Builds the Pure CDP server.
#[must_use]
pub fn host(settings: Settings) -> Host<Cdp> {
    Host::new(SERVER_NAME, INSTRUCTIONS, tools(), settings)
}

fn selector_prop() -> Prop {
    Prop::string("CSS selector, XPath, or tag:contains(\"text\")")
}

fn timeout_prop(default: f64) -> Prop {
    Prop::number("Seconds to wait for the element")
        .min(0.0)
        .default(default)
}

#[expect(
    clippy::too_many_lines,
    reason = "the tool table reads best as one list"
)]
fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "start_browser",
            "Start Browser",
            "Launch the Chrome session the other tools use. Does nothing if one is already \
             running. Headless by default on Linux, headed elsewhere.",
            Effect::Set,
            Schema::new()
                .optional("url", Prop::string("Page to open as the browser starts"))
                .optional(
                    "headless",
                    Prop::boolean("Force headless (true) or headed (false)"),
                )
                .optional(
                    "use_chromium",
                    Prop::boolean("Use Chromium instead of Google Chrome").default(false),
                )
                .optional(
                    "browser_executable_path",
                    Prop::string(
                        "Path to the browser binary; cannot be combined with use_chromium",
                    ),
                )
                .optional(
                    "incognito",
                    Prop::boolean("Incognito window").default(false),
                )
                .optional(
                    "guest",
                    Prop::boolean("Guest profile; cannot be combined with incognito")
                        .default(false),
                )
                .optional(
                    "ad_block",
                    Prop::boolean("Block common ad requests").default(false),
                )
                .optional(
                    "proxy",
                    Prop::string("SERVER:PORT or USER:PASS@SERVER:PORT"),
                ),
            start_browser,
        ),
        ToolDef::new(
            "close_browser",
            "Close Browser",
            "Close the browser session and everything in it: tabs, cookies and history. Safe \
             to call when nothing is running.",
            Effect::Set,
            Schema::new(),
            close_browser,
        ),
        ToolDef::new(
            "get_page_info",
            "Get Page Info",
            "Report whether a browser is running and, if so, the active tab's URL, title, \
             origin and user agent.",
            Effect::Observe,
            Schema::new(),
            get_page_info,
        ),
        ToolDef::new(
            "open_url",
            "Open URL",
            "Navigate the active tab to a URL and wait for it to load. A bare host such as \
             example.com gets https:// added.",
            Effect::Act,
            Schema::new().required("url", Prop::string("Address to open")),
            open_url,
        ),
        ToolDef::new(
            "manage_history",
            "Manage History",
            "Go back or forward, reload the page ignoring the cache, or list the tab's \
             navigation history.",
            Effect::Mixed,
            Schema::new().optional(
                "action",
                Prop::choice(&["back", "forward", "reload", "list"], "What to do").default("list"),
            ),
            manage_history,
        ),
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
        ToolDef::new(
            "click_element",
            "Click Element",
            "Click an element. Use nth to pick one of several matches, all_matches to click \
             every visible one, only_if_visible to skip a missing element quietly, or \
             parent_selector to search inside a container.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "nth",
                    Prop::integer("Which match to click, counting from 1").min(1.0),
                )
                .optional(
                    "all_matches",
                    Prop::boolean("Click every visible match, in page order").default(false),
                )
                .optional(
                    "only_if_visible",
                    Prop::boolean("Click only if the element is already visible; never wait")
                        .default(false),
                )
                .optional(
                    "parent_selector",
                    Prop::string("Look for the selector inside this container"),
                )
                .optional("timeout", timeout_prop(5.0)),
            click_element,
        ),
        ToolDef::new(
            "hover_action",
            "Hover / Click / Drag",
            "Hover an element, hover one and then click another (for menus), or drag one \
             element onto another.",
            Effect::Mixed,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "secondary_selector",
                    Prop::string("The element to click after hovering, or to drop onto"),
                )
                .optional(
                    "action",
                    Prop::choice(&["hover", "hover_and_click", "drag_and_drop"], "What to do")
                        .default("hover"),
                )
                .optional("timeout", timeout_prop(5.0)),
            hover_action,
        ),
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
        ToolDef::new(
            "wait_for_condition",
            "Wait For Condition",
            "Block until an element is present, visible, not visible or absent, until text \
             appears or disappears, or for a fixed number of seconds. Fails if the timeout \
             passes first.",
            Effect::Inspect,
            Schema::new()
                .optional(
                    "state",
                    Prop::choice(
                        &[
                            "present",
                            "visible",
                            "not_visible",
                            "absent",
                            "seconds_passed",
                        ],
                        "The condition to wait for",
                    )
                    .default("visible"),
                )
                .optional("selector", Prop::string("Element to watch"))
                .optional(
                    "text",
                    Prop::string(
                        "Wait for this text instead; present/visible wait for it to \
                                  appear, absent/not_visible for it to go",
                    ),
                )
                .optional(
                    "timeout",
                    Prop::number("Most seconds to wait; the exact wait for seconds_passed")
                        .min(0.0)
                        .default(5),
                ),
            wait_for_condition,
        ),
        ToolDef::new(
            "assert_condition",
            "Assert Condition",
            "Verify that an element is present or visible, that text is visible, or that the \
             title or URL matches. A failed check is an error. Element and text checks wait \
             up to the timeout; title and URL checks are immediate.",
            Effect::Inspect,
            Schema::new()
                .optional(
                    "check",
                    Prop::choice(
                        &[
                            "element_present",
                            "element_visible",
                            "text_visible",
                            "title",
                            "url",
                            "url_contains",
                        ],
                        "What to verify",
                    )
                    .default("element_visible"),
                )
                .optional(
                    "selector",
                    Prop::string("Element for element and text checks"),
                )
                .optional("expected", Prop::string("Expected text, title or URL"))
                .optional(
                    "exact",
                    Prop::boolean("For text_visible, require the whole text to match")
                        .default(false),
                )
                .optional("timeout", timeout_prop(5.0)),
            assert_condition,
        ),
        ToolDef::new(
            "manage_cookies",
            "Manage Cookies",
            "List, clear, save or load the session's cookies. Saved files live in the \
             server's output directory, under saved_cookies. Cookies can hold logins; \
             handle them with care.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "action",
                    Prop::choice(&["get_all", "clear", "save", "load"], "What to do")
                        .default("get_all"),
                )
                .optional(
                    "filename",
                    Prop::string("File name for save and load; only its last path part is used")
                        .default("cookies.txt"),
                ),
            manage_cookies,
        ),
        ToolDef::new(
            "manage_storage",
            "Manage Storage",
            "Read or write one key of localStorage or sessionStorage for the current page's \
             origin.",
            Effect::Mixed,
            Schema::new()
                .required("key", Prop::string("The storage key"))
                .optional(
                    "value",
                    Prop::string("The value to store; required for set"),
                )
                .optional(
                    "storage",
                    Prop::choice(&["local", "session"], "Which storage").default("local"),
                )
                .optional(
                    "action",
                    Prop::choice(&["get", "set"], "Read or write").default("get"),
                ),
            manage_storage,
        ),
        ToolDef::new(
            "scroll_page",
            "Scroll Page",
            "Scroll the page up or down by a share of the window height, or to the very top \
             or bottom.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "direction",
                    Prop::choice(&["up", "down", "top", "bottom"], "Where to scroll")
                        .default("down"),
                )
                .optional(
                    "amount",
                    Prop::integer(
                        "Percent of the window height for up and down; 200 is two screens",
                    )
                    .min(0.0)
                    .default(25),
                ),
            scroll_page,
        ),
        ToolDef::new(
            "manage_window",
            "Manage Window",
            "Read or change the browser window's position, size and state.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "action",
                    Prop::choice(
                        &["get_rect", "set_rect", "maximize", "minimize"],
                        "What to do",
                    )
                    .default("get_rect"),
                )
                .optional("x", Prop::integer("Left edge, for set_rect"))
                .optional("y", Prop::integer("Top edge, for set_rect"))
                .optional("width", Prop::integer("Width, for set_rect"))
                .optional("height", Prop::integer("Height, for set_rect")),
            manage_window,
        ),
        ToolDef::new(
            "manage_tabs",
            "Manage Tabs",
            "List, open, switch between and close tabs. Closing the active tab moves to the \
             newest remaining one.",
            Effect::Mixed,
            Schema::new()
                .optional(
                    "action",
                    Prop::choice(
                        &[
                            "list_tabs",
                            "open_new_tab",
                            "switch_to_tab",
                            "switch_to_newest_tab",
                            "close_active_tab",
                        ],
                        "What to do",
                    )
                    .default("list_tabs"),
                )
                .optional(
                    "url",
                    Prop::string("Page for open_new_tab; defaults to about:blank"),
                )
                .optional(
                    "tab_index",
                    Prop::integer("Tab number from list_tabs, for switch_to_tab"),
                )
                .optional(
                    "switch_to",
                    Prop::boolean("Make a newly opened tab the active one").default(true),
                ),
            manage_tabs,
        ),
        ToolDef::new(
            "solve_captcha",
            "Solve CAPTCHA",
            "Try to pass a CAPTCHA widget on the page by clicking its checkbox (Cloudflare \
             Turnstile, reCAPTCHA, hCaptcha, FriendlyCaptcha) or dragging its slider \
             (DataDome). Does nothing if none is found. Success is not guaranteed; check the \
             page afterwards.",
            Effect::Act,
            Schema::new(),
            solve_captcha,
        ),
        ToolDef::new(
            "save_page",
            "Save Page",
            "Save the active tab as a PNG screenshot, an HTML file or a PDF in the server's \
             output directory. Replaces a file of the same name.",
            Effect::Overwrite,
            Schema::new()
                .optional(
                    "format",
                    Prop::choice(&["screenshot", "html", "pdf"], "What to save")
                        .default("screenshot"),
                )
                .optional(
                    "filename",
                    Prop::string("Defaults to screenshot.png, page_source.html or page.pdf"),
                )
                .optional("folder", Prop::string("Sub-folder of the output directory")),
            save_page,
        ),
        ToolDef::new(
            "run_javascript",
            "Run JavaScript",
            "Evaluate JavaScript in the active tab and return the result. A returned promise \
             is awaited. Use the dedicated tools where one fits.",
            Effect::Mixed,
            Schema::new().required(
                "expression",
                Prop::string("Code to evaluate, such as document.title"),
            ),
            run_javascript,
        ),
    ]
}

// ----------------------------------------------------------------------
// Session
// ----------------------------------------------------------------------

async fn start_browser(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let headless = args
        .opt_bool("headless")?
        .unwrap_or(cfg!(target_os = "linux"));
    let use_chromium = args.bool_or("use_chromium", false)?;
    let url = nonempty(&args, "url")?.map(str::to_owned);

    let mut options = LaunchOptions::builder()
        .headless(headless)
        .use_chromium(use_chromium)
        .incognito(args.bool_or("incognito", false)?)
        .guest(args.bool_or("guest", false)?)
        .ad_block(args.bool_or("ad_block", false)?);
    if let Some(url) = &url {
        options = options.url(url.as_str());
    }
    if let Some(path) = nonempty(&args, "browser_executable_path")? {
        options = options.executable_path(path);
    }
    if let Some(proxy) = nonempty(&args, "proxy")? {
        options = options.proxy(proxy);
    }
    let options = options.build()?;

    let started = ctx
        .start(async move {
            let browser = Browser::launch(options).await?;
            Ok::<_, ToolError>(Cdp::attach(browser).await?)
        })
        .await?;
    Ok(match started {
        Started::Created => format!(
            "Started Pure CDP Mode browser (url={}, headless={headless}, use_chromium={use_chromium})",
            url.as_deref().unwrap_or("none"),
        ),
        Started::AlreadyRunning => "A browser session is already running.".to_owned(),
    }
    .into())
}

async fn close_browser(ctx: Ctx<Cdp>, _args: Args) -> Result<Output, ToolError> {
    let Some(cdp) = ctx.take().await else {
        return Ok("No browser session is currently running.".into());
    };
    cdp.browser.close().await?;
    Ok("The browser session was closed.".into())
}

async fn get_page_info(ctx: Ctx<Cdp>, _args: Args) -> Result<Output, ToolError> {
    let Some(session) = ctx.try_session().await else {
        return Ok(json!({ "running": false }).into());
    };
    let page = &session.page;
    let info = async {
        Ok::<_, SeleniumBaseError>(json!({
            "running": true,
            "url": page.url().await?,
            "title": page.title().await?,
            "origin": page.evaluate_as::<String>("location.origin").await?,
            "user_agent": page.user_agent().await?,
        }))
    }
    .await;
    Ok(info
        .unwrap_or_else(|error| json!({ "running": false, "error": error.to_string() }))
        .into())
}

// ----------------------------------------------------------------------
// Navigation
// ----------------------------------------------------------------------

async fn open_url(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let url = args.str("url")?;
    ctx.session().await?.page.goto(url).await?;
    Ok(format!("Navigated to {url}").into())
}

async fn manage_history(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let action = args.choice("action", &["back", "forward", "reload", "list"], "list")?;
    let session = ctx.session().await?;
    let page = &session.page;
    match action {
        "back" => {
            page.back().await?;
            Ok("Navigated back.".into())
        }
        "forward" => {
            page.forward().await?;
            Ok("Navigated forward.".into())
        }
        "reload" => {
            page.hard_reload().await?;
            Ok("Page reloaded.".into())
        }
        _ => {
            let history = page.execute("Page.getNavigationHistory", json!({})).await?;
            let entries: Vec<Value> = history["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|entry| {
                    json!({
                        "id": entry["id"],
                        "url": entry["url"],
                        "user_typed_url": entry["userTypedURL"],
                        "title": entry["title"],
                        "transition_type": entry["transitionType"],
                    })
                })
                .collect();
            Ok(json!({ "position": history["currentIndex"], "entries": entries }).into())
        }
    }
}

// ----------------------------------------------------------------------
// Reading
// ----------------------------------------------------------------------

async fn find_elements(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", 0.5)?;
    let include_html = args.bool_or("include_html", false)?;
    let session = ctx.session().await?;

    let found = session.page.locator(selector).with_timeout(timeout);
    match found.wait_for(State::Present).await {
        Ok(()) => {}
        Err(error) if is_missing(&error) => {
            return Ok(json!({ "count": 0, "matches": [] }).into());
        }
        Err(error) => return Err(error.into()),
    }
    let matches: Vec<Value> = found
        .infos()
        .await?
        .into_iter()
        .map(|info| {
            let mut entry = json!({ "tag_name": info.tag, "text": info.text });
            if include_html {
                entry["html"] = Value::String(info.html);
            }
            entry
        })
        .collect();
    Ok(json!({ "count": matches.len(), "matches": matches }).into())
}

async fn get_content(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str_or("selector", "body")?;
    let format = args.choice("output_format", &["text", "html", "urls"], "text")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;

    let element = session.page.locator(selector).with_timeout(timeout);
    match format {
        "text" => Ok(element.text().await?.into()),
        "html" => Ok(element.html().await?.into()),
        _ => Ok(Output::Json(json!(urls_in(&session.page, &element).await?))),
    }
}

/// The absolute http(s), ftp and file URLs in an element's `href` and `src`
/// attributes, in page order and without repeats.
async fn urls_in(page: &Page, element: &Locator) -> Result<Vec<String>, SeleniumBaseError> {
    let base = Url::parse(&page.url().await?).ok();
    let mut found = vec![element.info().await?];
    found.extend(element.locator("[href], [src]").infos().await?);

    let mut seen = BTreeSet::new();
    let mut urls = Vec::new();
    for info in found {
        for attribute in ["href", "src"] {
            let Some(url) = info
                .attributes
                .get(attribute)
                .and_then(|raw| absolute_url(raw, base.as_ref()))
            else {
                continue;
            };
            if seen.insert(url.clone()) {
                urls.push(url);
            }
        }
    }
    Ok(urls)
}

fn absolute_url(raw: &str, base: Option<&Url>) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let url = match base {
        Some(base) => base.join(raw).ok()?,
        None => Url::parse(raw).ok()?,
    };
    matches!(url.scheme(), "http" | "https" | "ftp" | "file").then(|| url.into())
}

async fn get_attributes(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let element = session.page.locator(selector).with_timeout(timeout);

    match nonempty(&args, "attribute")? {
        Some(name) => Ok(Output::Json(
            element
                .attribute(name)
                .await?
                .map_or(Value::Null, Value::String),
        )),
        None => json_output(element.info().await?.attributes),
    }
}

async fn check_if_condition(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let check = args.choice("check", &["present", "visible"], "visible")?;
    let selector = args.str_or("selector", "body")?;
    let text = nonempty(&args, "text")?;
    let session = ctx.session().await?;
    let element = session.page.locator(selector).with_timeout(NO_WAIT);

    let answer = if let Some(text) = text {
        match element.text().await {
            Ok(shown) => element.is_visible().await? && shown.contains(text),
            Err(error) if is_missing(&error) => false,
            Err(error) => return Err(error.into()),
        }
    } else if check == "present" {
        element.exists().await?
    } else {
        element.is_visible().await?
    };
    Ok(Output::Json(Value::Bool(answer)))
}

// ----------------------------------------------------------------------
// Interacting
// ----------------------------------------------------------------------

async fn click_element(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let page = &session.page;

    if let Some(nth) = args.opt_i64("nth")? {
        let index = usize::try_from(nth)
            .ok()
            .and_then(|n| n.checked_sub(1))
            .ok_or_else(|| ToolError::invalid("nth", "counts from 1"))?;
        page.locator(selector)
            .with_timeout(timeout)
            .nth(index)
            .click()
            .await?;
        return Ok(format!("Clicked match #{nth} of {selector}").into());
    }

    if args.bool_or("all_matches", false)? {
        let visible = page.locator(selector).with_timeout(NO_WAIT).visible();
        let total = visible.count().await?;
        let mut clicked = 0_usize;
        for index in 0..total {
            match visible.nth(index).click().await {
                Ok(()) => clicked += 1,
                // A click that navigated away leaves nothing to click next.
                Err(error) if error.is_element_error() => break,
                Err(error) => return Err(error.into()),
            }
        }
        return Ok(format!("Clicked {clicked} visible matches of {selector}").into());
    }

    if args.bool_or("only_if_visible", false)? {
        let first = page
            .locator(selector)
            .with_timeout(NO_WAIT)
            .visible()
            .first();
        return if first.exists().await? {
            first.click().await?;
            Ok(format!("Clicked {selector}").into())
        } else {
            Ok(format!("No visible match for {selector}; nothing was clicked").into())
        };
    }

    if let Some(parent) = nonempty(&args, "parent_selector")? {
        page.locator(parent)
            .locator(selector)
            .with_timeout(timeout)
            .click()
            .await?;
        return Ok(format!("Clicked {selector} inside {parent}").into());
    }

    page.locator(selector).with_timeout(timeout).click().await?;
    Ok(format!("Clicked {selector}").into())
}

async fn hover_action(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let action = args.choice(
        "action",
        &["hover", "hover_and_click", "drag_and_drop"],
        "hover",
    )?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let page = &session.page;
    let primary = page.locator(selector).with_timeout(timeout);

    if action == "hover" {
        primary.hover().await?;
        return Ok(format!("Hovered {selector}").into());
    }

    let secondary_selector = nonempty(&args, "secondary_selector")?.ok_or_else(|| {
        ToolError::invalid(
            "secondary_selector",
            format!("required for action {action:?}"),
        )
    })?;
    let secondary = page.locator(secondary_selector).with_timeout(timeout);
    if action == "hover_and_click" {
        primary.hover().await?;
        tokio::time::sleep(HOVER_PAUSE).await;
        secondary.click().await?;
        Ok(format!("Hovered {selector} and clicked {secondary_selector}").into())
    } else {
        primary.drag_to(&secondary).await?;
        Ok(format!("Dragged {selector} onto {secondary_selector}").into())
    }
}

async fn type_text(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let text = args.str_or("text", "")?;
    let mode = args.choice(
        "mode",
        &[
            "fill_input",
            "append",
            "fast_type",
            "set_value",
            "clear_only",
        ],
        "fill_input",
    )?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let page = &session.page;
    let field = page.locator(selector).with_timeout(timeout);

    match mode {
        "fill_input" => {
            field.clear().await?;
            field.type_text(text).await?;
        }
        "append" => field.type_text(text).await?,
        "fast_type" => {
            field.clear().await?;
            // Inserting text in one step cannot press Enter, so do that
            // between the lines.
            for (index, line) in text.split('\n').enumerate() {
                if index > 0 {
                    page.keyboard().press(Key::Enter).await?;
                }
                if !line.is_empty() {
                    page.keyboard().insert_text(line).await?;
                }
            }
        }
        "set_value" => field.set_value(text).await?,
        _ => field.clear().await?,
    }
    Ok(format!("type_text(mode={mode}) done for {selector}").into())
}

async fn select_option(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let dropdown = args.str("dropdown_selector")?;
    let value = args.scalar("value")?;
    let by = args.choice("by", &["text", "value", "index"], "text")?;
    let choice =
        match by {
            "text" => SelectBy::Text(&value),
            "value" => SelectBy::Value(&value),
            _ => {
                SelectBy::Index(value.trim().parse().ok().ok_or_else(|| {
                    ToolError::invalid("value", "an index must be a whole number")
                })?)
            }
        };
    ctx.session()
        .await?
        .page
        .locator(dropdown)
        .select_option(choice)
        .await?;
    Ok(format!("Selected ({by}={value:?}) in {dropdown}").into())
}

async fn focus_element(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let action = args.choice(
        "action",
        &["scroll_to_element", "focus", "highlight"],
        "scroll_to_element",
    )?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let element = session.page.locator(selector).with_timeout(timeout);
    match action {
        "scroll_to_element" => element.scroll_into_view().await?,
        "focus" => element.focus().await?,
        _ => element.flash().await?,
    }
    Ok(format!("{action} done for {selector}").into())
}

// ----------------------------------------------------------------------
// Waiting and asserting
// ----------------------------------------------------------------------

async fn wait_for_condition(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let state = args.choice(
        "state",
        &[
            "present",
            "visible",
            "not_visible",
            "absent",
            "seconds_passed",
        ],
        "visible",
    )?;
    let selector = nonempty(&args, "selector")?;
    let text = nonempty(&args, "text")?;
    let timeout = args.seconds_or("timeout", 5.0)?;

    if state == "seconds_passed" {
        tokio::time::sleep(timeout).await;
        return Ok(format!("Waited for {}s", timeout.as_secs_f64()).into());
    }
    if selector.is_none() && text.is_none() {
        return Err(ToolError::invalid(
            "selector",
            "give a selector or text unless state is seconds_passed",
        ));
    }
    let session = ctx.session().await?;
    let page = &session.page;

    if let Some(text) = text {
        let within = page
            .locator(selector.unwrap_or("body"))
            .with_timeout(timeout);
        let expect = within.expect().timeout(timeout);
        return if matches!(state, "present" | "visible") {
            expect.to_contain_text(text).await?;
            Ok(format!("Text '{text}' found in {}.", selector.unwrap_or("the page")).into())
        } else {
            expect.not().to_contain_text(text).await?;
            Ok(format!(
                "Text '{text}' not found in {}.",
                selector.unwrap_or("the page")
            )
            .into())
        };
    }

    let wanted = match state {
        "present" => State::Present,
        "visible" => State::Visible,
        "not_visible" => State::Hidden,
        _ => State::Absent,
    };
    let selector = selector.unwrap_or_default();
    page.locator(selector)
        .with_timeout(timeout)
        .wait_for(wanted)
        .await?;
    Ok(format!("Element {selector} reached state '{state}'.").into())
}

async fn assert_condition(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    const CHECKS: [&str; 6] = [
        "element_present",
        "element_visible",
        "text_visible",
        "title",
        "url",
        "url_contains",
    ];
    let check = args.choice("check", &CHECKS, "element_visible")?;
    let selector = nonempty(&args, "selector")?;
    let expected = args.opt_str("expected")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let page = &session.page;

    if matches!(check, "element_present" | "element_visible") {
        let selector = selector.ok_or_else(|| {
            ToolError::invalid("selector", format!("required for check {check:?}"))
        })?;
        let expect = page
            .locator(selector)
            .with_timeout(timeout)
            .expect()
            .timeout(timeout);
        if check == "element_present" {
            expect.to_exist().await?;
            return Ok(format!("Confirmed {selector} is present.").into());
        }
        expect.to_be_visible().await?;
        return Ok(format!("Confirmed {selector} is visible.").into());
    }

    let expected = expected
        .ok_or_else(|| ToolError::invalid("expected", format!("required for check {check:?}")))?;
    let now = page.expect().timeout(NO_WAIT);
    match check {
        "text_visible" => {
            let target = selector.unwrap_or("html");
            let expect = page
                .locator(target)
                .with_timeout(timeout)
                .expect()
                .timeout(timeout);
            if args.bool_or("exact", false)? {
                expect.to_have_text(expected).await?;
            } else {
                expect.to_contain_text(expected).await?;
            }
            Ok(format!("Confirmed visible text {expected} in {target}.").into())
        }
        "title" => {
            now.to_have_title(expected).await?;
            Ok(format!("Confirmed title is '{expected}'.").into())
        }
        "url" => {
            now.to_have_url(expected).await?;
            Ok(format!("Confirmed URL is '{expected}'.").into())
        }
        _ => {
            now.to_contain_url(expected).await?;
            Ok(format!("Confirmed URL contains '{expected}'.").into())
        }
    }
}

// ----------------------------------------------------------------------
// Browser state
// ----------------------------------------------------------------------

async fn manage_cookies(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let action = args.choice("action", &["get_all", "clear", "save", "load"], "get_all")?;
    let session = ctx.session().await?;
    let cookies = session.page.cookies();

    match action {
        "get_all" => json_output(cookies.all().await?),
        "clear" => {
            cookies.clear().await?;
            Ok("Cookies cleared.".into())
        }
        _ => {
            let (path, name) =
                cookie_file(ctx.settings(), args.str_or("filename", "cookies.txt")?)?;

            if action == "save" {
                if let Some(dir) = path.parent() {
                    tokio::fs::create_dir_all(dir)
                        .await
                        .map_err(SeleniumBaseError::from)?;
                }
                let saved = cookies.save(&path, None).await?;
                Ok(format!("Saved {saved} cookies to {name}").into())
            } else {
                if !path.exists() {
                    return Err(ToolError::Refused(format!(
                        "{name} does not exist in the cookie directory"
                    )));
                }
                let loaded = cookies.load(&path, None).await?;
                Ok(format!("Loaded {loaded} cookies from {name}").into())
            }
        }
    }
}

async fn manage_storage(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let key = args.str("key")?;
    let which = args.choice("storage", &["local", "session"], "local")?;
    let action = args.choice("action", &["get", "set"], "get")?;
    let session = ctx.session().await?;
    let storage = if which == "local" {
        session.page.local_storage()
    } else {
        session.page.session_storage()
    };

    if action == "get" {
        return Ok(Output::Json(
            storage.get(key).await?.map_or(Value::Null, Value::String),
        ));
    }
    let value = args
        .opt_str("value")?
        .ok_or_else(|| ToolError::invalid("value", "required when action is set"))?;
    storage.set(key, value).await?;
    Ok(format!("Set {which}Storage[{key:?}]").into())
}

async fn scroll_page(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let direction = args.choice("direction", &["up", "down", "top", "bottom"], "down")?;
    let amount = args.i64_or("amount", 25)?;
    if amount < 0 && matches!(direction, "up" | "down") {
        return Err(ToolError::invalid("amount", "cannot be negative"));
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a scroll percentage is far below 2^53"
    )]
    let percent = amount as f64;
    let (to, done) = match direction {
        "up" => (
            Scroll::PageUp(percent),
            format!("Scrolled up by {amount}%."),
        ),
        "down" => (
            Scroll::PageDown(percent),
            format!("Scrolled down by {amount}%."),
        ),
        "top" => (Scroll::Top, "Scrolled to the top.".to_owned()),
        _ => (Scroll::Bottom, "Scrolled to the bottom.".to_owned()),
    };
    ctx.session().await?.page.scroll(to).await?;
    Ok(done.into())
}

async fn manage_window(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let action = args.choice(
        "action",
        &["get_rect", "set_rect", "maximize", "minimize"],
        "get_rect",
    )?;
    let session = ctx.session().await?;
    let window = session.page.window();

    match action {
        "get_rect" => json_output(window.bounds().await?),
        "set_rect" => {
            let (Some(x), Some(y), Some(width), Some(height)) = (
                args.opt_i64("x")?,
                args.opt_i64("y")?,
                args.opt_i64("width")?,
                args.opt_i64("height")?,
            ) else {
                return Err(ToolError::invalid(
                    "action",
                    "set_rect needs x, y, width and height",
                ));
            };
            window.set_bounds(x, y, width, height).await?;
            Ok(format!("Window set to ({x}, {y}, {width}x{height})").into())
        }
        "maximize" => {
            window.maximize().await?;
            Ok("Window maximized.".into())
        }
        _ => {
            window.minimize().await?;
            Ok("Window minimized.".into())
        }
    }
}

async fn manage_tabs(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let action = args.choice(
        "action",
        &[
            "list_tabs",
            "open_new_tab",
            "switch_to_tab",
            "switch_to_newest_tab",
            "close_active_tab",
        ],
        "list_tabs",
    )?;
    let mut session = ctx.session().await?;

    match action {
        "list_tabs" => {
            let tabs: Vec<Value> = session
                .browser
                .list_pages()
                .await?
                .into_iter()
                .enumerate()
                .map(|(index, tab)| json!({ "index": index, "url": tab.url, "title": tab.title }))
                .collect();
            Ok(Output::Json(Value::Array(tabs)))
        }
        "open_new_tab" => {
            let url = nonempty(&args, "url")?.unwrap_or("about:blank");
            let switch_to = args.bool_or("switch_to", true)?;
            let page = session.browser.new_page(Some(url)).await?;
            if switch_to {
                page.bring_to_front().await?;
                session.page = page;
            }
            Ok(format!("Opened new tab (url={url:?}, switch_to={switch_to})").into())
        }
        "switch_to_tab" => {
            let index = args
                .opt_usize("tab_index")?
                .ok_or_else(|| ToolError::invalid("tab_index", "required for switch_to_tab"))?;
            let mut tabs = session.browser.pages().await?;
            if index >= tabs.len() {
                return Err(ToolError::invalid(
                    "tab_index",
                    format!("{index} is out of range; there are {} tabs", tabs.len()),
                ));
            }
            let page = tabs.swap_remove(index);
            page.bring_to_front().await?;
            session.page = page;
            Ok(format!("Switched to tab {index}").into())
        }
        "switch_to_newest_tab" => {
            let page = session.browser.newest_page().await?;
            page.bring_to_front().await?;
            session.page = page;
            Ok("Switched to newest tab.".into())
        }
        _ => {
            session.page.close().await?;
            match session.browser.newest_page().await {
                Ok(page) => {
                    page.bring_to_front().await?;
                    session.page = page;
                    Ok("Closed active tab; switched to the newest remaining tab.".into())
                }
                Err(_) => Ok("Closed active tab; no tabs remain, open a new one.".into()),
            }
        }
    }
}

async fn solve_captcha(ctx: Ctx<Cdp>, _args: Args) -> Result<Output, ToolError> {
    let session = ctx.session().await?;
    Ok(match session.page.solve_captcha().await? {
        Some(kind) => format!("Attempted to solve a {kind} CAPTCHA."),
        None => "No supported CAPTCHA was found on the page.".to_owned(),
    }
    .into())
}

// ----------------------------------------------------------------------
// Output and scripting
// ----------------------------------------------------------------------

async fn save_page(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let format = args.choice("format", &["screenshot", "html", "pdf"], "screenshot")?;
    let default_name = match format {
        "screenshot" => "screenshot.png",
        "html" => "page_source.html",
        _ => "page.pdf",
    };
    let name = nonempty(&args, "filename")?.unwrap_or(default_name);
    let path = ctx
        .settings()
        .output_path(nonempty(&args, "folder")?, name)?;
    let session = ctx.session().await?;
    let page = &session.page;

    let bytes = match format {
        "screenshot" => page.screenshot().await?,
        "html" => page.content().await?.into_bytes(),
        _ => page.pdf().await?,
    };
    write_file(&path, &bytes).await?;
    Ok(format!("Saved {format} as {}", path.display()).into())
}

async fn run_javascript(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let expression = args.str("expression")?;
    Ok(Output::Json(
        ctx.session().await?.page.evaluate(expression).await?,
    ))
}
