//! The `driver` server: the 26 tools of upstream's `Driver()` server, on
//! [`BaseCase`], with eight debugging tools it shares with the `sb` server.

mod assertions;
mod challenges;
mod files;
mod input;
mod navigation;
mod page;
mod script;
mod session;
mod waits;
mod windows;

use super::webdriver as w;
use super::{devtools, Host, Settings, ToolDef};
use crate::api::base_case::BaseCase;

const SERVER_NAME: &str = "seleniumbase-driver";

const INSTRUCTIONS: &str = "\
Drives a browser through WebDriver, with undetected mode on by default. Call \
start_browser once, use the other tools, then call close_browser. Failed actions \
come back as errors you can read and retry; use wait_for_element before acting on \
content that loads late. When something fails, inspect_page, diagnose_selector, \
suggest_selectors, get_console_messages and capture_debug_bundle say what the page \
looks like, diagnose_environment checks Chrome and the driver without a browser, and \
export_session_as_test turns the session into a Rust test.";

/// The debugging tools this server shares with `sb`: the definitions are the ones of
/// `devtools.rs`, picked by name.
const DEBUG_TOOLS: [&str; 6] = [
    "inspect_page",
    "suggest_selectors",
    "diagnose_selector",
    "get_console_messages",
    "capture_debug_bundle",
    "export_session_as_test",
];

/// Builds the `driver` server.
///
/// It keeps upstream's tool list and adds `get_page_info`, `diagnose_environment`
/// and the debugging tools of `DEBUG_TOOLS`: they run on `BaseCase` as they do
/// on `sb`. `get_failed_requests` and `audit_page` are left to `cdp` and `sb`.
/// A failed call is reported with the page's state, read the same way the `sb`
/// server reads it.
#[must_use]
pub fn host(settings: Settings) -> Host<BaseCase> {
    let mut tools =
        super::tool::mark_untrusted(tools(), &["get_text", "get_page_source", "execute_script"]);
    tools.push(w::page_info_tool());
    tools.extend(devtools::tools_named(&DEBUG_TOOLS));
    tools.extend(devtools::environment_tools());
    tools.extend(super::pagetools::tools());
    tools.extend(super::fingerprint::tools());
    Host::new(SERVER_NAME, INSTRUCTIONS, tools, settings)
        .with_probe(devtools::probe::<BaseCase>)
        .with_refs(super::pagetools::verify::<BaseCase>)
        .with_resources(super::resources::standard())
        .with_prompts(super::prompts::browser())
}

fn tools() -> Vec<ToolDef<BaseCase>> {
    [
        session::tools(),
        navigation::tools(),
        page::tools(),
        input::tools(),
        waits::tools(),
        windows::tools(),
        assertions::tools(),
        challenges::tools(),
        files::tools(),
        script::tools(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
