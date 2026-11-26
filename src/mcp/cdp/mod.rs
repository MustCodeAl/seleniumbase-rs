//! The Pure CDP server: [`sb_cdp`](crate::sb_cdp) as MCP tools.
//!
//! One Chrome session is shared by every call. `start_browser` launches it,
//! the other tools drive it, and `close_browser` ends it. Related actions are
//! grouped behind one tool with an `action`, `mode` or `state` argument, which
//! keeps the tool list short enough for a model to choose from. The developer
//! tools follow them; on this engine `get_failed_requests` can also watch the
//! tab's `Network` events.

mod cdp_session;
mod challenges;
mod clicks;
mod conditions;
mod files;
mod forms;
mod navigation;
mod page;
mod script;
mod session;
mod storage;
mod tasks;
mod windows;

pub use cdp_session::Cdp;

use std::time::Duration;

use super::devtools;
use super::{Host, Prop, Settings, ToolDef};
use crate::error::SeleniumBaseError;

const SERVER_NAME: &str = "seleniumbase-cdp";

const INSTRUCTIONS: &str = "\
Drives Chrome over the DevTools Protocol with no WebDriver, so pages see an \
ordinary browser. Call start_browser once, use the other tools, then call \
close_browser. Selectors may be CSS (preferred), XPath, or link=text. \
Failed actions come back as reports you can act on: they name the page's state \
and the next tools to try. inspect_page outlines the page, diagnose_selector says \
why a selector fails, suggest_selectors finds one that works, and \
export_session_as_test turns the session into a Rust test.";

/// How long the pointer rests on a menu before the item under it is clicked.
const HOVER_PAUSE: Duration = Duration::from_millis(250);

/// How long a check that must not wait is allowed to look.
const NO_WAIT: Duration = Duration::ZERO;

/// Builds the Pure CDP server, with the developer tools.
#[must_use]
pub fn host(settings: Settings) -> Host<Cdp> {
    let mut tools = super::tool::mark_untrusted(
        tools(),
        &["get_content", "get_attributes", "run_javascript"],
    );
    tools.extend(devtools::tools());
    tools.extend(devtools::environment_tools());
    tools.extend(super::pagetools::tools());
    // A task's results and errors are what the pages it visited held.
    tools.push(tasks::tool().untrusted());
    // `wait_for_email` exists only for an operator who turned the imap plugin on with options.
    #[cfg(feature = "plugin-imap")]
    if super::email::offered(settings.plugin_selection()) {
        tools.push(super::email::tool::<Cdp>());
    }
    Host::new(SERVER_NAME, INSTRUCTIONS, tools, settings)
        .probe(devtools::probe::<Cdp>)
        .refs(super::pagetools::verify::<Cdp>)
        .resources(super::resources::standard())
        .prompts(super::prompts::browser())
}

fn selector_prop() -> Prop {
    Prop::string("CSS selector, XPath, or link=text")
}

fn timeout_prop(default: f64) -> Prop {
    Prop::number("Seconds to wait for the element")
        .min(0.0)
        .default(default)
}

fn tools() -> Vec<ToolDef<Cdp>> {
    [
        session::tools(),
        navigation::tools(),
        page::tools(),
        clicks::tools(),
        forms::tools(),
        conditions::tools(),
        storage::tools(),
        windows::tools(),
        challenges::tools(),
        files::tools(),
        script::tools(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Navigates for the `navigate` tool. With plugins (feature `plugins`) their
/// `before_navigate` hooks see an MCP client as who asked.
#[cfg(feature = "plugins")]
pub(super) async fn navigate_for_client(
    page: &crate::sb_cdp::Page,
    url: &str,
) -> Result<(), SeleniumBaseError> {
    page.goto_from(url, seleniumbase_rs_plugin::NavOrigin::Mcp)
        .await
}

#[cfg(not(feature = "plugins"))]
pub(super) async fn navigate_for_client(
    page: &crate::sb_cdp::Page,
    url: &str,
) -> Result<(), SeleniumBaseError> {
    page.goto(url).await
}
