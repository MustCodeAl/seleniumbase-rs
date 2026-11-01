//! CDP mode, CAPTCHAs and one-time codes: what stands between the session and a
//! protected page.

use crate::api::base_case::BaseCase;
use crate::mcp::webdriver::{self as w, selector_prop};
use crate::mcp::{Effect, Prop, Schema, ToolDef};

pub(super) fn tools() -> Vec<ToolDef<BaseCase>> {
    vec![
        ToolDef::new(
            "activate_cdp_mode",
            "Activate CDP Mode",
            "Switch the session to CDP mode, which adds stealth and DevTools-based \
methods; optionally open a URL. Needs a Chromium browser.",
            Effect::Act,
            Schema::new().optional("url", Prop::string("Page to open once active")),
            w::activate_cdp_mode,
        ),
        ToolDef::new(
            "solve_captcha",
            "Solve CAPTCHA",
            "Try to pass a CAPTCHA widget on the page (Turnstile, reCAPTCHA, hCaptcha, \
Friendly Captcha, DataDome). Does nothing if none is found; check the page \
afterwards.",
            Effect::Act,
            Schema::new(),
            w::solve_captcha,
        ),
        ToolDef::new(
            "get_mfa_code",
            "Get MFA Code",
            "Compute the current one-time code for a TOTP secret.",
            Effect::Observe,
            Schema::new().required("totp_key", Prop::string("The base32 TOTP secret")),
            w::get_mfa_code,
        ),
        ToolDef::new(
            "enter_mfa_code",
            "Enter MFA Code",
            "Type the current one-time code for a TOTP secret into a field.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .required("totp_key", Prop::string("The base32 TOTP secret")),
            w::enter_mfa_code,
        ),
    ]
}
