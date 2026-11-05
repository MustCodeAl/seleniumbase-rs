//! Understanding the page and finding selectors that work.

use serde_json::{json, Value};

use super::{bounded, page_op, selector_prop, tagged, DevSession};
use crate::error::selector_advice;
use crate::mcp::support::nonempty;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "inspect_page",
            "Inspect Page",
            "Outline the current page for writing a test: URL, title, ready state, headings, \
             forms, iframes, shadow-DOM hosts, and every control with its role, accessible name, \
             a selector that finds it, and whether it is visible and enabled. Bounded by limit; \
             says when it was cut short.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "scope",
                    Prop::string("Only look inside the first element this selector matches"),
                )
                .optional(
                    "limit",
                    Prop::integer("Most controls to list, 1 to 500")
                        .min(1.0)
                        .default(50),
                )
                .optional(
                    "include_hidden",
                    Prop::boolean("Also list controls that are not visible").default(false),
                ),
            inspect_page::<S>,
        ),
        ToolDef::new(
            "suggest_selectors",
            "Suggest Selectors",
            "Find the element meant by its text, its role or a rough selector, and return ranked \
             stable selectors for it (test attributes, id, name, aria-label, link text, text \
             XPath, short CSS path), each checked against the page: how many elements it \
             matches and whether it finds this one.",
            Effect::Observe,
            Schema::new()
                .optional(
                    "text",
                    Prop::string("Text the element shows or is labelled with"),
                )
                .optional(
                    "role",
                    Prop::string("ARIA role such as button, link, textbox or checkbox"),
                )
                .optional("selector", Prop::string("A rough selector that finds it"))
                .optional(
                    "limit",
                    Prop::integer("Most selectors per element, 1 to 20")
                        .min(1.0)
                        .default(5),
                ),
            suggest_selectors::<S>,
        ),
        ToolDef::new(
            "diagnose_selector",
            "Diagnose Selector",
            "Explain why a selector finds nothing or cannot be used: invalid syntax (with the \
             browser's message), no match, matches that are hidden, empty, off screen, disabled \
             or covered by another element, matches inside an iframe or a shadow root, and near \
             misses such as a similar id or the same text in different case. Ends with next \
             steps.",
            Effect::Observe,
            Schema::new().required("selector", selector_prop()),
            diagnose_selector::<S>,
        ),
    ]
}

async fn inspect_page<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let limit = bounded(&args, "limit", 50, 500)?;
    let scope = nonempty(&args, "scope")?.map(tagged);
    let include_hidden = args.bool_or("include_hidden", false)?;
    let mut session = ctx.session().await?;
    let outline = page_op(
        &mut *session,
        "inspect",
        &json!({ "limit": limit, "scope": scope, "include_hidden": include_hidden }),
    )
    .await?;
    if let Some(error) = outline.get("error").and_then(Value::as_str) {
        return Err(ToolError::invalid("scope", error));
    }
    Ok(outline.into())
}

async fn suggest_selectors<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let text = nonempty(&args, "text")?;
    let role = nonempty(&args, "role")?;
    let rough = nonempty(&args, "selector")?;
    if text.is_none() && role.is_none() && rough.is_none() {
        return Err(ToolError::invalid(
            "text",
            "give the element's text, its role or a rough selector",
        ));
    }
    let limit = bounded(&args, "limit", 5, 20)?;
    let mut session = ctx.session().await?;
    let mut found = page_op(
        &mut *session,
        "suggest",
        &json!({ "text": text, "role": role, "selector": rough.map(tagged), "limit": limit }),
    )
    .await?;
    if found["elements"].as_array().is_some_and(Vec::is_empty) {
        found["advice"] = Value::from(
            "No element matched. Check the spelling of the text, or call inspect_page to see the \
             controls the page has.",
        );
    }
    Ok(found.into())
}

async fn diagnose_selector<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let raw = args.str("selector")?;
    let mut session = ctx.session().await?;
    let engine = session.engine();
    let mut report = page_op(
        &mut *session,
        "diagnose",
        &json!({ "selector": tagged(raw), "raw": raw }),
    )
    .await?;
    drop(session);

    let mut advice: Vec<String> = selector_advice(raw).into_iter().collect();
    if raw.contains(":contains(") {
        let xpath = crate::utils::selectors::css_to_xpath(raw).ok();
        advice.push(match xpath {
            Some(xpath) => format!(
                ":contains() is not CSS, and querySelector rejects it. The same element in \
                 XPath: {xpath}"
            ),
            None => ":contains() is not CSS, and querySelector rejects it; use XPath such as \
                     //button[contains(., \"text\")]."
                .to_owned(),
        });
    }
    report["verdict"] = Value::from(verdict(&report));
    report["advice"] = Value::from(advice);
    report["next_steps"] = Value::from(diagnosis_steps(raw, &report, engine));
    Ok(report.into())
}

/// One sentence saying what is wrong with the selector, from a diagnosis.
fn verdict(report: &Value) -> String {
    if let Some(error) = report["syntax_error"].as_str() {
        return format!("The browser cannot parse this selector: {error}");
    }
    let matches = report["matches"].as_u64().unwrap_or(0);
    if matches == 0 {
        if let Some(frame) = report["frames"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|frame| frame["matches"].as_u64().unwrap_or(0) > 0)
        {
            return format!(
                "Nothing matches in the page itself, but {} element(s) match inside the frame {}: \
                 the element is in that frame's document.",
                frame["matches"], frame["frame"]
            );
        }
        if let Some(host) = report["shadow_roots"]
            .as_array()
            .and_then(|hosts| hosts.first())
        {
            return format!(
                "Nothing matches in the page itself, but {} element(s) match inside the shadow \
                 root of {}; selectors do not reach into a shadow root.",
                host["matches"], host["host"]
            );
        }
        if let Some(miss) = report["near_misses"]
            .as_array()
            .and_then(|misses| misses.first())
        {
            return format!(
                "Nothing matches. Close on the page: {} {} (try {}).",
                miss["kind"].as_str().unwrap_or("value"),
                miss["found"],
                miss["try"]
            );
        }
        return "Nothing on this page matches the selector.".to_owned();
    }
    let first = &report["elements"][0];
    let many = if matches > 1 {
        format!(" The selector matches {matches} elements; tools act on the first.")
    } else {
        String::new()
    };
    if report["visible_matches"].as_u64().unwrap_or(0) == 0 {
        let why = if first["display"] == "none" {
            "it has display: none".to_owned()
        } else if first["visibility"] != "visible" {
            format!(
                "it has visibility: {}",
                first["visibility"].as_str().unwrap_or("hidden")
            )
        } else if first["opacity"].as_f64() == Some(0.0) {
            "it has opacity: 0".to_owned()
        } else {
            "it has no size".to_owned()
        };
        return format!("{matches} element(s) match, but none is visible: the first, {why}.{many}");
    }
    if first["enabled"] == false {
        return format!("The element is disabled, so it cannot be used yet.{many}");
    }
    if let Some(cover) = first["covered_by"].as_str() {
        return format!("The element is covered by {cover}, which would get the click.{many}");
    }
    if first["in_viewport"] == false {
        return format!(
            "The element is outside the window; tools scroll it into view first.{many}"
        );
    }
    format!("The selector works: the first match is visible, enabled and not covered.{many}")
}

/// Up to three concrete next steps after a diagnosis.
fn diagnosis_steps(raw: &str, report: &Value, engine: &str) -> Vec<String> {
    let quoted = Value::from(raw);
    let mut steps = Vec::new();
    let matches = report["matches"].as_u64().unwrap_or(0);
    if report["syntax_error"].is_string() || matches == 0 {
        if let Some(frame) = report["frames"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|frame| frame["matches"].as_u64().unwrap_or(0) > 0)
        {
            steps.push(if engine == "webdriver" {
                format!(
                    "switch_to_frame {{\"selector\": {}}}, then use the selector again.",
                    frame["frame"]
                )
            } else {
                "The Pure CDP tools search the top document only; read the frame's content \
                 with run_javascript, or open the frame's URL in a tab."
                    .to_owned()
            });
        }
        if let Some(miss) = report["near_misses"]
            .as_array()
            .and_then(|misses| misses.first())
        {
            steps.push(format!("Try the near miss: {}.", miss["try"]));
        }
        steps.push(format!(
            "suggest_selectors {{\"text\": \"<the text on the element>\"}}: working selectors \
             for {quoted} as it should be."
        ));
        steps.push("inspect_page {}: see the controls the page has now.".to_owned());
    } else {
        let first = &report["elements"][0];
        if report["visible_matches"].as_u64().unwrap_or(0) == 0 {
            steps.push(if engine == "webdriver" {
                format!("wait_for_element {{\"selector\": {quoted}, \"timeout\": 15}}: it may be shown later.")
            } else {
                format!(
                    "wait_for_condition {{\"selector\": {quoted}, \"state\": \"visible\", \
                     \"timeout\": 15}}: it may be shown later."
                )
            });
            steps.push(
                "If it opens from a menu or a tab, act on that first, then check again.".to_owned(),
            );
        }
        if first["covered_by_selector"].is_string() {
            steps.push(format!(
                "Close or remove what covers it first ({}).",
                first["covered_by_selector"]
            ));
        }
        if matches > 1 {
            steps.push(format!(
                "suggest_selectors {{\"selector\": {quoted}}}: a selector that matches only the \
                 element you mean."
            ));
        }
    }
    steps.truncate(3);
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verdict_names_the_first_reason_a_selector_cannot_be_used() {
        let verdict_of = |report: Value| verdict(&report);
        assert!(verdict_of(
            json!({ "syntax_error": "'a[' is not a valid selector", "matches": 0 })
        )
        .starts_with("The browser cannot parse this selector"));
        assert!(verdict_of(json!({
            "matches": 0,
            "frames": [{ "frame": "iframe#pay", "matches": 1 }],
        }))
        .contains("inside the frame \"iframe#pay\""));
        assert!(verdict_of(json!({
            "matches": 0, "frames": [], "shadow_roots": [],
            "near_misses": [{ "kind": "id", "found": "submit-btn", "try": "#submit-btn" }],
        }))
        .contains("try \"#submit-btn\""));
        assert!(verdict_of(json!({
            "matches": 2, "visible_matches": 0,
            "elements": [{ "display": "none", "visibility": "visible", "opacity": 1 }],
        }))
        .contains("display: none"));
        assert!(verdict_of(json!({
            "matches": 1, "visible_matches": 1,
            "elements": [{ "enabled": true, "covered_by": "div.cookie-banner", "in_viewport": true }],
        }))
        .contains("covered by div.cookie-banner"));
        assert!(verdict_of(json!({
            "matches": 1, "visible_matches": 1,
            "elements": [{ "enabled": true, "in_viewport": true }],
        }))
        .starts_with("The selector works"));
    }
}
