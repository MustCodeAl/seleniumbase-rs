//! This crate's own stealth tools, which need no browser session.
//!
//! Exposes chromedriver patching for MCP servers.

use super::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::{ChromedriverPatcher, EnginePatch};

/// The stealth tools, for a server whose session type is `S`.
pub(super) fn tools<S: Send + 'static>() -> Vec<ToolDef<S>> {
    vec![ToolDef::new(
        "patch_chromedriver",
        "Patch Chromedriver",
        "Patch a chromedriver binary in place to remove the markers that identify it as \
             automation.",
        Effect::Overwrite,
        Schema::new()
            .required("path", Prop::string("Path of the chromedriver binary"))
            .optional(
                "backup",
                Prop::boolean("Keep a backup copy first").default(true),
            ),
        patch_chromedriver::<S>,
    )]
}

async fn patch_chromedriver<S>(_ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let path = args.str("path")?;
    let mut spec = EnginePatch::all();
    spec.backup = args.bool_or("backup", true)?;
    ChromedriverPatcher::new(path).patch(spec)?;
    Ok(format!("Patched chromedriver at {path}").into())
}
