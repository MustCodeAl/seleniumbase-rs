//! The `sb` server: the 88 tools of upstream's `SB()` server, on [`BaseCase`],
//! plus this crate's own stealth and developer tools.

mod assertions;
mod challenges;
mod clicks;
mod elements;
mod files;
mod forms;
mod navigation;
mod page;
mod pointer;
mod presentation;
mod script;
mod scrolling;
mod session;
mod storage;
mod waits;
mod windows;

use super::{devtools, stealth, Host, Settings, ToolDef};
use crate::api::base_case::BaseCase;

const SERVER_NAME: &str = "seleniumbase-sb";

const INSTRUCTIONS: &str = "\
Drives a browser through WebDriver with SeleniumBase's broad toolset, with undetected \
mode on by default. Call start_browser once, use the other tools, then call \
close_browser. Failed actions come back as reports you can act on: they name the \
page's state and the next tools to try. Use the wait_for_* tools before acting on \
content that loads late. inspect_page, diagnose_selector and suggest_selectors help \
find elements, and export_session_as_test turns the session into a Rust test. The \
stealth tools and diagnose_environment work without a browser.";

/// Builds the `sb` server, with the stealth and developer tools.
#[must_use]
pub fn host(settings: Settings) -> Host<BaseCase> {
    let mut tools = super::tool::mark_untrusted(
        tools(),
        &[
            "get_text",
            "get_html_source",
            "get_element_html",
            "get_attribute",
            "execute_script",
        ],
    );
    tools.extend(stealth::tools());
    tools.extend(devtools::tools());
    tools.extend(devtools::environment_tools());
    tools.extend(super::pagetools::tools());
    Host::new(SERVER_NAME, INSTRUCTIONS, tools, settings)
        .probe(devtools::probe::<BaseCase>)
        .refs(super::pagetools::verify::<BaseCase>)
        .resources(super::resources::standard())
        .prompts(super::prompts::browser())
}

fn tools() -> Vec<ToolDef<BaseCase>> {
    [
        session::tools(),
        navigation::tools(),
        page::tools(),
        elements::tools(),
        clicks::tools(),
        forms::tools(),
        pointer::tools(),
        waits::tools(),
        assertions::tools(),
        storage::tools(),
        scrolling::tools(),
        windows::tools(),
        challenges::tools(),
        files::tools(),
        script::tools(),
        presentation::tools(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
