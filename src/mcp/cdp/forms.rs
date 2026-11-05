//! Typing, choosing options and focus.

use super::{selector_prop, timeout_prop, Cdp};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::sb_cdp::{Key, SelectBy};

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "type_text",
            "Type Text",
            "Enter or clear text in an input, textarea or editable element. A newline in \
             the text presses Enter, except in set_value mode.",
            Effect::Mixed,
            Schema::new()
                .required("selector", selector_prop())
                .optional("text", Prop::string("The text to enter").default(""))
                .optional(
                    "mode",
                    Prop::choice(
                        &[
                            "fill_input",
                            "append",
                            "fast_type",
                            "set_value",
                            "clear_only",
                        ],
                        "fill_input clears then types key by key; append types after the \
                         current value; fast_type clears then inserts in one step; set_value \
                         assigns the value without key events; clear_only empties the field",
                    )
                    .default("fill_input"),
                )
                .optional("timeout", timeout_prop(5.0)),
            type_text,
        ),
        ToolDef::new(
            "select_option",
            "Select Option",
            "Choose an option in a native <select> dropdown. For dropdowns built from \
             divs or buttons, click them instead.",
            Effect::Act,
            Schema::new()
                .required(
                    "dropdown_selector",
                    Prop::string("Selector of the <select> element"),
                )
                .required(
                    "value",
                    Prop::string("The option's text, value or 0-based index"),
                )
                .optional(
                    "by",
                    Prop::choice(&["text", "value", "index"], "How to match the option")
                        .default("text"),
                ),
            select_option,
        ),
        ToolDef::new(
            "focus_element",
            "Focus Element",
            "Scroll an element into view, give it keyboard focus, or briefly outline it. \
             Does not click or type.",
            Effect::Set,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "action",
                    Prop::choice(&["scroll_to_element", "focus", "highlight"], "What to do")
                        .default("scroll_to_element"),
                )
                .optional("timeout", timeout_prop(5.0)),
            focus_element,
        ),
    ]
}

async fn type_text(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let text = args.str_or("text", "")?;
    let mode = args.choice(
        "mode",
        &[
            "fill_input",
            "append",
            "fast_type",
            "set_value",
            "clear_only",
        ],
        "fill_input",
    )?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let page = &session.page;
    let field = page.locator(selector).with_timeout(timeout);

    match mode {
        "fill_input" => field.type_text(text).await?,
        "append" => field.send_keys(text).await?,
        "fast_type" => {
            field.clear().await?;
            // Inserting text in one step cannot press Enter, so do that
            // between the lines.
            for (index, line) in text.split('\n').enumerate() {
                if index > 0 {
                    page.keyboard().press(Key::Enter).await?;
                }
                if !line.is_empty() {
                    page.keyboard().insert_text(line).await?;
                }
            }
        }
        "set_value" => field.set_value(text).await?,
        _ => field.clear().await?,
    }
    Ok(format!("type_text(mode={mode}) done for {selector}").into())
}

async fn select_option(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let dropdown = args.str("dropdown_selector")?;
    let value = args.scalar("value")?;
    let by = args.choice("by", &["text", "value", "index"], "text")?;
    let choice =
        match by {
            "text" => SelectBy::Text(&value),
            "value" => SelectBy::Value(&value),
            _ => {
                SelectBy::Index(value.trim().parse().ok().ok_or_else(|| {
                    ToolError::invalid("value", "an index must be a whole number")
                })?)
            }
        };
    ctx.session()
        .await?
        .page
        .locator(dropdown)
        .select_option(choice)
        .await?;
    Ok(format!("Selected ({by}={value:?}) in {dropdown}").into())
}

async fn focus_element(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let action = args.choice(
        "action",
        &["scroll_to_element", "focus", "highlight"],
        "scroll_to_element",
    )?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let element = session.page.locator(selector).with_timeout(timeout);
    match action {
        "scroll_to_element" => element.scroll_into_view().await?,
        "focus" => element.focus().await?,
        _ => element.flash().await?,
    }
    Ok(format!("{action} done for {selector}").into())
}
