//! Opening a URL and moving through the history.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
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
    ]
}
