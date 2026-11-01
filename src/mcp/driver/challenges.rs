//! CDP mode and CAPTCHAs: what stands between the session and a protected page.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver as w;
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "activate_cdp_mode",
            "Activate CDP Mode",
            "Switch the session to CDP mode, which adds stealth and DevTools-based methods. \
             Optionally open a URL. Needs a Chromium browser.",
            Effect::Act,
            Schema::new().optional("url", Prop::string("Page to open once active")),
            w::activate_cdp_mode,
        ),
        ToolDef::new(
            "solve_captcha",
            "Solve CAPTCHA",
            "Try to pass a CAPTCHA widget on the page (Turnstile, reCAPTCHA, hCaptcha, Friendly \
             Captcha, DataDome). Does nothing if none is found; check the page afterwards.",
            Effect::Act,
            Schema::new(),
            w::solve_captcha,
        ),
    ]
}
