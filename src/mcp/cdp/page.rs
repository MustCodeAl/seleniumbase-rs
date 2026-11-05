//! Reading the page: elements, their content and attributes, and quick checks.

use serde_json::{json, Value};

use super::{selector_prop, timeout_prop, Cdp, NO_WAIT};
use crate::mcp::support::{is_missing, json_output, nonempty};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::sb_cdp::State;

pub(super) fn tools() -> Vec<ToolDef<Cdp>> {
    vec![
        ToolDef::new(
            "find_elements",
            "Find Elements",
            "List the elements matching a selector with their tag and text. Elements cannot \
             be held between calls; to act on one, use a tool that takes the selector.",
            Effect::Observe,
            Schema::new()
                .required("selector", selector_prop())
                .optional("timeout", timeout_prop(0.5))
                .optional(
                    "include_html",
                    Prop::boolean("Also return each element's outer HTML").default(false),
                ),
            find_elements,
        ),
        ToolDef::new(
            "get_content",
            "Get Content",
            "Read an element's visible text or HTML, or the absolute URLs found in it. Reads \
             the whole page body by default.",
            Effect::Observe,
            Schema::new()
                .optional("selector", selector_prop().default("body"))
                .optional(
                    "output_format",
                    Prop::choice(&["text", "html", "urls"], "What to return").default("text"),
                )
                .optional("timeout", timeout_prop(5.0)),
            get_content,
        ),
        ToolDef::new(
            "get_attributes",
            "Get Attributes",
            "Read one attribute of the first matching element, or all of them when no \
             attribute is named.",
            Effect::Observe,
            Schema::new()
                .required("selector", selector_prop())
                .optional(
                    "attribute",
                    Prop::string("Attribute name, such as href or value"),
                )
                .optional("timeout", timeout_prop(5.0)),
            get_attributes,
        ),
        ToolDef::new(
            "check_if_condition",
            "Check Condition",
            "Answer true or false right now whether an element is present or visible, or \
             whether some text is visible in it. Does not wait; use wait_for_condition to wait.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "check",
                    Prop::choice(
                        &["present", "visible"],
                        "What to check; ignored when text is given",
                    )
                    .default("visible"),
                )
                .optional("selector", selector_prop().default("body"))
                .optional(
                    "text",
                    Prop::string("Check that this text is visible in the element"),
                ),
            check_if_condition,
        ),
    ]
}

async fn find_elements(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", 0.5)?;
    let include_html = args.bool_or("include_html", false)?;
    let session = ctx.session().await?;

    let found = session.page.locator(selector).with_timeout(timeout);
    match found.wait_for(State::Present).await {
        Ok(()) => {}
        Err(error) if is_missing(&error) => {
            return Ok(json!({ "count": 0, "matches": [] }).into());
        }
        Err(error) => return Err(error.into()),
    }
    let matches: Vec<Value> = found
        .infos()
        .await?
        .into_iter()
        .map(|info| {
            let mut entry = json!({ "tag_name": info.tag, "text": info.text });
            if include_html {
                entry["html"] = Value::String(info.html);
            }
            entry
        })
        .collect();
    Ok(json!({ "count": matches.len(), "matches": matches }).into())
}

async fn get_content(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str_or("selector", "body")?;
    let format = args.choice("output_format", &["text", "html", "urls"], "text")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;

    let element = session.page.locator(selector).with_timeout(timeout);
    match format {
        "text" => Ok(element.text().await?.into()),
        "html" => Ok(element.html().await?.into()),
        _ => Ok(Output::Json(json!(element.urls().await?))),
    }
}

async fn get_attributes(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let selector = args.str("selector")?;
    let timeout = args.seconds_or("timeout", 5.0)?;
    let session = ctx.session().await?;
    let element = session.page.locator(selector).with_timeout(timeout);

    match nonempty(&args, "attribute")? {
        Some(name) => Ok(Output::Json(
            element
                .attribute(name)
                .await?
                .map_or(Value::Null, Value::String),
        )),
        None => json_output(element.info().await?.attributes),
    }
}

async fn check_if_condition(ctx: Ctx<Cdp>, args: Args) -> Result<Output, ToolError> {
    let check = args.choice("check", &["present", "visible"], "visible")?;
    let selector = args.str_or("selector", "body")?;
    let text = nonempty(&args, "text")?;
    let session = ctx.session().await?;
    let element = session.page.locator(selector).with_timeout(NO_WAIT);

    let answer = if let Some(text) = text {
        match element.text().await {
            Ok(shown) => element.is_visible().await? && shown.contains(text),
            Err(error) if is_missing(&error) => false,
            Err(error) => return Err(error.into()),
        }
    } else if check == "present" {
        element.exists().await?
    } else {
        element.is_visible().await?
    };
    Ok(Output::Json(Value::Bool(answer)))
}
