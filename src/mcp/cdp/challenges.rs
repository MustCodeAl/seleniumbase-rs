//! CAPTCHAs: what stands between the session and a protected page.

use super::Cdp;
use crate::mcp::{Args, Ctx, Effect, Output, Schema, ToolDef, ToolError};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![ToolDef::new(
        "solve_captcha",
        "Solve CAPTCHA",
        "Try to pass a CAPTCHA widget on the page by clicking its checkbox (Cloudflare \
         Turnstile, reCAPTCHA, hCaptcha, FriendlyCaptcha) or dragging its slider \
         (DataDome). Does nothing if none is found. Success is not guaranteed; check the \
         page afterwards.",
        Effect::Act,
        Schema::new(),
        solve_captcha,
    )]
}

async fn solve_captcha(ctx: Ctx<Cdp>, _args: Args) -> Result<Output, ToolError> {
    let session = ctx.session().await?;
    let solved = session
        .page
        .solve_captcha_with(&crate::sb_cdp::SolveOptions::default())
        .await?;
    Ok(match solved {
        Some(solved) => format!("Attempted to solve a {} CAPTCHA: {solved}.", solved.captcha),
        None => "No supported CAPTCHA was found on the page.".to_owned(),
    }
    .into())
}
