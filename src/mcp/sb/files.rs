//! Screenshots, page source, PDFs and downloads, written to the server's
//! output directory.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
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
    ]
}
