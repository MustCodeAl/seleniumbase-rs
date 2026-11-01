//! CAPTCHAs: what stands between the session and a protected page.

use super::{solve_captcha, Cdp};
use crate::mcp::{Effect, Schema, ToolDef};

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
