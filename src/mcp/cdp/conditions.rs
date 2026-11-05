//! Waiting for a condition, and asserting one.

use super::{timeout_prop, Cdp, NO_WAIT};
use crate::mcp::support::nonempty;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::sb_cdp::State;

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "wait_for_condition",
            "Wait For Condition",
            "Block until an element is present, visible, not visible or absent, until text \
             appears or disappears, or for a fixed number of seconds. Fails if the timeout \
             passes first.",
            Effect::Inspect,
            Schema::new()
                .optional(
                    "state",
                    Prop::choice(
                        &[
                            "present",
                            "visible",
                            "not_visible",
                            "absent",
                            "seconds_passed",
                        ],
                        "The condition to wait for",
                    )
                    .default("visible"),
                )
                .optional("selector", Prop::string("Element to watch"))
                .optional(
                    "text",
                    Prop::string(
                        "Wait for this text instead; present/visible wait for it to \
                                  appear, absent/not_visible for it to go",
                    ),
                )
                .optional(
                    "timeout",
                    Prop::number("Most seconds to wait; the exact wait for seconds_passed")
                        .min(0.0)
                        .default(5),
                ),
            wait_for_condition,
        ),
        ToolDef::new(
            "assert_condition",
            "Assert Condition",
            "Verify that an element is present or visible, that text is visible, or that the \
             title or URL matches. A failed check is an error. Element and text checks wait \
             up to the timeout; title and URL checks are immediate.",
            Effect::Inspect,
            Schema::new()
                .optional(
                    "check",
                    Prop::choice(
                        &[
                            "element_present",
                            "element_visible",
                            "text_visible",
                            "title",
                            "url",
                            "url_contains",
                        ],
                        "What to verify",
                    )
                    .default("element_visible"),
                )
                .optional(
                    "selector",
                    Prop::string("Element for element and text checks"),
                )
                .optional("expected", Prop::string("Expected text, title or URL"))
                .optional(
                    "exact",
                    Prop::boolean("For text_visible, require the whole text to match")
                        .default(false),
                )
                .optional("timeout", timeout_prop(5.0)),
            assert_condition,
        ),
    ]
}

async fn wait_for_condition(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let state = args.choice(
        "state",
        &[
            "present",
            "visible",
            "not_visible",
            "absent",
            "seconds_passed",
        ],
        "visible",
    )?;
    let selector = nonempty(&args, "selector")?;
    let text = nonempty(&args, "text")?;
    let timeout = args.seconds_or("timeout", 5.0)?;

    if state == "seconds_passed" {
        tokio::time::sleep(timeout).await;
        return Ok(format!("Waited for {}s", timeout.as_secs_f64()).into());
    }
    if selector.is_none() && text.is_none() {
        return Err(ToolError::invalid(
            "selector",
            "give a selector or text unless state is seconds_passed",
        ));
    }
    let session = ctx.session().await?;
    let page = &session.page;

    if let Some(text) = text {
        let within = page
            .locator(selector.unwrap_or("body"))
            .with_timeout(timeout);
        let expect = within.expect().timeout(timeout);
        return if matches!(state, "present" | "visible") {
            expect.to_contain_text(text).await?;
            Ok(format!("Text '{text}' found in {}.", selector.unwrap_or("the page")).into())
        } else {
            expect.not().to_contain_text(text).await?;
            Ok(format!(
                "Text '{text}' not found in {}.",
                selector.unwrap_or("the page")
            )
            .into())
        };
    }

    let wanted = match state {
        "present" => State::Present,
        "visible" => State::Visible,
        "not_visible" => State::Hidden,
        _ => State::Absent,
    };
    let selector = selector.unwrap_or_default();
    page.locator(selector)
        .with_timeout(timeout)
        .wait_for(wanted)
        .await?;
    Ok(format!("Element {selector} reached state '{state}'.").into())
}

async fn assert_condition(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    const CHECKS: [&str; 6] = [
        "element_present",
        "element_visible",
        "text_visible",
        "title",
        "url",
        "url_contains",
    ];
    let check = args.choice("check", &CHECKS, "element_visible")?;
    let selector = nonempty(&args, "selector")?;
    let expected = args.opt_str("expected")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let page = &session.page;

    if matches!(check, "element_present" | "element_visible") {
        let selector = selector.ok_or_else(|| {
            ToolError::invalid("selector", format!("required for check {check:?}"))
        })?;
        let expect = page
            .locator(selector)
            .with_timeout(timeout)
            .expect()
            .timeout(timeout);
        if check == "element_present" {
            expect.to_exist().await?;
            return Ok(format!("Confirmed {selector} is present.").into());
        }
        expect.to_be_visible().await?;
        return Ok(format!("Confirmed {selector} is visible.").into());
    }

    let expected = expected
        .ok_or_else(|| ToolError::invalid("expected", format!("required for check {check:?}")))?;
    let now = page.expect().timeout(NO_WAIT);
    match check {
        "text_visible" => {
            let target = selector.unwrap_or("html");
            let expect = page
                .locator(target)
                .with_timeout(timeout)
                .expect()
                .timeout(timeout);
            if args.bool_or("exact", false)? {
                expect.to_have_text(expected).await?;
            } else {
                expect.to_contain_text(expected).await?;
            }
            Ok(format!("Confirmed visible text {expected} in {target}.").into())
        }
        "title" => {
            now.to_have_title(expected).await?;
            Ok(format!("Confirmed title is '{expected}'.").into())
        }
        "url" => {
            now.to_have_url(expected).await?;
            Ok(format!("Confirmed URL is '{expected}'.").into())
        }
        _ => {
            now.to_contain_url(expected).await?;
            Ok(format!("Confirmed URL contains '{expected}'.").into())
        }
    }
}
