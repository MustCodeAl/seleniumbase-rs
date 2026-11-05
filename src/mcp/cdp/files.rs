//! Saving the page, written to the server's output directory.

use super::Cdp;
use crate::mcp::support::{nonempty, write_file};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

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
