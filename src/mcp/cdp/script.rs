//! Running JavaScript in the page.

use super::Cdp;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![ToolDef::new(
        "run_javascript",
        "Run JavaScript",
        "Evaluate JavaScript in the active tab and return the result. A returned promise \
         is awaited. Use the dedicated tools where one fits.",
        Effect::Mixed,
        Schema::new().required(
            "expression",
            Prop::string("Code to evaluate, such as document.title"),
        ),
        run_javascript,
    )]
}

async fn run_javascript(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let expression = args.str("expression")?;
    Ok(Output::Json(
        ctx.session().await?.page.evaluate(expression).await?,
    ))
}
