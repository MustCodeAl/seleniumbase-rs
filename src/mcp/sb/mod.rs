//! The `sb` server: the 88 tools of upstream's `SB()` server, on [`BaseCase`],
//! plus this crate's own stealth and developer tools.

mod assertions;
mod challenges;
mod clicks;
mod files;
mod input;
mod navigation;
mod page;
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
    tools.extend(super::fingerprint::tools());
    tools.extend(super::fingerprint::offline_tools());
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
        clicks::tools(),
        input::tools(),
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
