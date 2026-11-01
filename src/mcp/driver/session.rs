//! Starting and closing the browser.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, start_schema};
use crate::mcp::{Effect, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
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
    ]
}
