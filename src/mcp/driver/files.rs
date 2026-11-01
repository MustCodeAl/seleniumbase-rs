//! Screenshots, written to the server's output directory.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![ToolDef::new(
        "save_screenshot",
        "Save Screenshot",
        "Save a PNG screenshot of the page in the server's output directory.",
        Effect::Overwrite,
        Schema::new().optional(
            "filename",
            Prop::string("File name").default("screenshot.png"),
        ),
        w::save_screenshot_file,
    )]
}
