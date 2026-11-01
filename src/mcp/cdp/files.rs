//! Saving the page, written to the server's output directory.

use super::{save_page, Cdp};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![ToolDef::new(
        "save_page",
        "Save Page",
        "Save the active tab as a PNG screenshot, an HTML file or a PDF in the server's \
         output directory. Replaces a file of the same name.",
        Effect::Overwrite,
        Schema::new()
            .optional(
                "format",
                Prop::choice(&["screenshot", "html", "pdf"], "What to save").default("screenshot"),
            )
            .optional(
                "filename",
                Prop::string("Defaults to screenshot.png, page_source.html or page.pdf"),
            )
            .optional("folder", Prop::string("Sub-folder of the output directory")),
        save_page,
    )]
}
