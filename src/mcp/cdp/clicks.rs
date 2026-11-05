//! Clicking, hovering and dragging.

use super::{selector_prop, timeout_prop, Cdp, HOVER_PAUSE, NO_WAIT};
use crate::mcp::support::nonempty;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "click_element",
            "Click Element",
            "Click an element. Use nth to pick one of several matches, all_matches to click \
             every visible one, only_if_visible to skip a missing element quietly, or \
             parent_selector to search inside a container.",
            Effect::Act,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "nth",
                    Prop::integer("Which match to click, counting from 1").min(1.0),
                )
                .optional(
                    "all_matches",
                    Prop::boolean("Click every visible match, in page order").default(false),
                )
                .optional(
                    "only_if_visible",
                    Prop::boolean("Click only if the element is already visible; never wait")
                        .default(false),
                )
                .optional(
                    "parent_selector",
                    Prop::string("Look for the selector inside this container"),
                )
                .optional("timeout", timeout_prop(5.0)),
            click_element,
        ),
        ToolDef::new(
            "hover_action",
            "Hover / Click / Drag",
            "Hover an element, hover one and then click another (for menus), or drag one \
             element onto another.",
            Effect::Mixed,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "secondary_selector",
                    Prop::string("The element to click after hovering, or to drop onto"),
                )
                .optional(
                    "action",
                    Prop::choice(&["hover", "hover_and_click", "drag_and_drop"], "What to do")
                        .default("hover"),
                )
                .optional("timeout", timeout_prop(5.0)),
            hover_action,
        ),
    ]
}

async fn click_element(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let page = &session.page;

    if let Some(nth) = args.opt_i64("nth")? {
        let index = usize::try_from(nth)
            .ok()
            .and_then(|n| n.checked_sub(1))
            .ok_or_else(|| ToolError::invalid("nth", "counts from 1"))?;
        page.locator(selector)
            .with_timeout(timeout)
            .nth(index)
            .click()
            .await?;
        return Ok(format!("Clicked match #{nth} of {selector}").into());
    }

    if args.bool_or("all_matches", false)? {
        let visible = page.locator(selector).with_timeout(NO_WAIT).visible();
        let total = visible.count().await?;
        let mut clicked = 0_usize;
        for index in 0..total {
            match visible.nth(index).click().await {
                Ok(()) => clicked += 1,
                // A click that navigated away leaves nothing to click next.
                Err(error) if error.is_element_error() => break,
                Err(error) => return Err(error.into()),
            }
        }
        return Ok(format!("Clicked {clicked} visible matches of {selector}").into());
    }

    if args.bool_or("only_if_visible", false)? {
        let first = page
            .locator(selector)
            .with_timeout(NO_WAIT)
            .visible()
            .first();
        return if first.exists().await? {
            first.click().await?;
            Ok(format!("Clicked {selector}").into())
        } else {
            Ok(format!("No visible match for {selector}; nothing was clicked").into())
        };
    }

    if let Some(parent) = nonempty(&args, "parent_selector")? {
        page.locator(parent)
            .locator(selector)
            .with_timeout(timeout)
            .click()
            .await?;
        return Ok(format!("Clicked {selector} inside {parent}").into());
    }

    page.locator(selector).with_timeout(timeout).click().await?;
    Ok(format!("Clicked {selector}").into())
}

async fn hover_action(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let action = args.choice(
        "action",
        &["hover", "hover_and_click", "drag_and_drop"],
        "hover",
    )?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let page = &session.page;
    let primary = page.locator(selector).with_timeout(timeout);

    if action == "hover" {
        primary.hover().await?;
        return Ok(format!("Hovered {selector}").into());
    }

    let secondary_selector = nonempty(&args, "secondary_selector")?.ok_or_else(|| {
        ToolError::invalid(
            "secondary_selector",
            format!("required for action {action:?}"),
        )
    })?;
    let secondary = page.locator(secondary_selector).with_timeout(timeout);
    if action == "hover_and_click" {
        primary.hover().await?;
        tokio::time::sleep(HOVER_PAUSE).await;
        secondary.click().await?;
        Ok(format!("Hovered {selector} and clicked {secondary_selector}").into())
    } else {
        primary.drag_to(&secondary).await?;
        Ok(format!("Dragged {selector} onto {secondary_selector}").into())
    }
}
