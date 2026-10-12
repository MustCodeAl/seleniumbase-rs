//! The Chrome DevTools Recorder's JSON export, the format `@puppeteer/replay`
//! runs: a title and a list of steps (`navigate`, `click`, `change`, ...),
//! each target given as several alternative selectors.
//!
//! The JSON is read with the JavaScript tokenizer and parser (a JSON document
//! is a JavaScript expression), so its strings are decoded the same way and
//! each step keeps its line number for diagnostics.

use super::emit::{float, rust_str, whole, Helper};
use super::lexer::{Lexed, Tok};
use super::parser::Expr;
use super::selector::{self, Step};
use super::translate::Translator;

/// The line and byte span of each element of the top-level `"steps"` array.
pub(super) fn step_spans(lexed: &Lexed) -> Vec<(usize, usize, usize)> {
    let tokens = &lexed.tokens;
    let mut depth = 0_usize;
    let mut index = 0;
    // Find `"steps": [` at the first level of the document.
    let mut array_start = None;
    while index < tokens.len() {
        match &tokens[index].tok {
            Tok::Punct("{" | "[") => depth += 1,
            Tok::Punct("}" | "]") => depth = depth.saturating_sub(1),
            Tok::Str(key)
                if depth == 1
                    && key == "steps"
                    && matches!(tokens.get(index + 1).map(|t| &t.tok), Some(Tok::Punct(":")))
                    && matches!(tokens.get(index + 2).map(|t| &t.tok), Some(Tok::Punct("["))) =>
            {
                array_start = Some(index + 3);
                break;
            }
            _ => {}
        }
        index += 1;
    }
    let Some(mut index) = array_start else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    while let Some(token) = tokens.get(index) {
        match token.tok {
            Tok::Punct("]") => break,
            Tok::Punct(",") => index += 1,
            _ => {
                let (line, start) = (token.line, token.start);
                // Skip the element: balanced brackets, or one token.
                let mut level = 0_usize;
                let mut end = token.end;
                while let Some(token) = tokens.get(index) {
                    match token.tok {
                        Tok::Punct("{" | "[") => level += 1,
                        Tok::Punct("}" | "]") => level = level.saturating_sub(1),
                        _ => {}
                    }
                    end = token.end;
                    index += 1;
                    if level == 0 {
                        break;
                    }
                }
                spans.push((line, start, end));
            }
        }
    }
    spans
}

fn field<'a>(entries: &'a [(String, Expr)], key: &str) -> Option<&'a Expr> {
    entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn text_field<'a>(entries: &'a [(String, Expr)], key: &str) -> Result<&'a str, String> {
    match field(entries, key) {
        Some(Expr::Str(text)) => Ok(text),
        _ => Err(format!("a step without a text `{key}` is not converted")),
    }
}

fn number_field(entries: &[(String, Expr)], key: &str) -> Option<f64> {
    match field(entries, key) {
        Some(Expr::Num(n)) => Some(*n),
        _ => None,
    }
}

/// The first selector of a step this crate can use: CSS first, then
/// `xpath/`, then `aria/` (a role approximation), then `text/`. Selectors
/// that go through frames or shadow roots (more than one part) and `pierce/`
/// are passed over.
fn choose_selector(entries: &[(String, Expr)]) -> Result<Vec<Step>, String> {
    let Some(Expr::Array(list)) = field(entries, "selectors") else {
        return Err("a step without selectors is not converted".to_owned());
    };
    let mut candidates: Vec<(u8, &str)> = Vec::new();
    for item in list {
        let text = match item {
            Expr::Array(parts) => match parts.as_slice() {
                [Expr::Str(text)] => text.as_str(),
                _ => continue,
            },
            Expr::Str(text) => text.as_str(),
            _ => continue,
        };
        let rank = if text.starts_with("pierce/") {
            continue;
        } else if text.starts_with("xpath/") {
            1
        } else if text.starts_with("aria/") {
            2
        } else if text.starts_with("text/") {
            3
        } else {
            0
        };
        candidates.push((rank, text));
    }
    candidates.sort_by_key(|(rank, _)| *rank);
    candidates
        .iter()
        .find_map(|(_, text)| selector::puppeteer(text).ok())
        .ok_or_else(|| "none of the step's selectors is one the importer converts".to_owned())
}

/// Translates the steps of a recording into the current test, and returns
/// the recording's title.
pub(super) fn translate(
    value: &Expr,
    spans: &[(usize, usize, usize)],
    translator: &mut Translator<'_>,
) -> Result<Option<String>, String> {
    let Expr::Object(entries) = value else {
        return Err("the file is not a JSON object".to_owned());
    };
    let title = match field(entries, "title") {
        Some(Expr::Str(title)) => Some(title.clone()),
        _ => None,
    };
    let Some(Expr::Array(steps)) = field(entries, "steps") else {
        return Err("the JSON has no \"steps\" array".to_owned());
    };
    for (index, step) in steps.iter().enumerate() {
        let (line, start, end) = spans.get(index).copied().unwrap_or((1, 0, 0));
        translator.at(line, start, end);
        match step_lines(step, translator) {
            Ok(lines) => {
                for line in lines {
                    translator.push_code(line);
                }
            }
            Err(reason) => translator.todo(&reason),
        }
    }
    Ok(title)
}

fn step_lines(step: &Expr, translator: &mut Translator<'_>) -> Result<Vec<String>, String> {
    let Expr::Object(entries) = step else {
        return Err("a step that is not an object is not converted".to_owned());
    };
    let kind = text_field(entries, "type")?;
    if let Some(target) = field(entries, "target") {
        if !matches!(target, Expr::Str(t) if t == "main") {
            return Err("a step in another tab or window is not converted".to_owned());
        }
    }
    if let Some(Expr::Array(frame)) = field(entries, "frame") {
        if !frame.is_empty() {
            return Err("a step inside a frame is not converted".to_owned());
        }
    }
    let locator = |translator: &mut Translator<'_>| -> Result<String, String> {
        let steps = choose_selector(entries)?;
        translator.locator(&steps)
    };
    let mut lines = match kind {
        "setViewport" => {
            if matches!(field(entries, "isMobile"), Some(Expr::Bool(true))) {
                return Err("a mobile viewport is not converted".to_owned());
            }
            let width = number_field(entries, "width").and_then(whole);
            let height = number_field(entries, "height").and_then(whole);
            let (Some(width), Some(height)) = (width, height) else {
                return Err(
                    "a viewport without a whole width and height is not converted".to_owned(),
                );
            };
            vec![format!(
                "page.window().set_bounds(0, 0, {width}, {height}).await?;"
            )]
        }
        "navigate" => {
            let url = text_field(entries, "url")?;
            return Ok(vec![format!("page.goto({}).await?;", rust_str(url))]);
        }
        "click" | "doubleClick" => {
            let target = locator(translator)?;
            let method = match (kind, field(entries, "button")) {
                ("click", None) => "click",
                ("click", Some(Expr::Str(b))) if b == "primary" => "click",
                ("click", Some(Expr::Str(b))) if b == "secondary" => "right_click",
                ("doubleClick", None) => "double_click",
                ("doubleClick", Some(Expr::Str(b))) if b == "primary" => "double_click",
                _ => return Err("this kind of click is not converted".to_owned()),
            };
            vec![format!("{target}.{method}().await?;")]
        }
        "hover" => vec![format!("{}.hover().await?;", locator(translator)?)],
        "change" => {
            let value = text_field(entries, "value")?;
            let target = locator(translator)?;
            translator.needs.helpers.insert(Helper::Change);
            vec![format!(
                "support::change(&{target}, {}).await?;",
                rust_str(value)
            )]
        }
        "keyDown" => vec![translator.key(text_field(entries, "key")?)?],
        // The key was pressed and released on `keyDown`.
        "keyUp" | "close" => Vec::new(),
        "scroll" => {
            if field(entries, "selectors").is_some() {
                vec![format!(
                    "{}.scroll_into_view().await?;",
                    locator(translator)?
                )]
            } else {
                let y = number_field(entries, "y").unwrap_or(0.0);
                let y = float(y).ok_or("a scroll offset that is not a number")?;
                translator.needs.sb_cdp.insert("Scroll");
                vec![format!("page.scroll(Scroll::To({y})).await?;")]
            }
        }
        "waitForElement" => {
            let count = number_field(entries, "count").unwrap_or(1.0);
            let operator = match field(entries, "operator") {
                Some(Expr::Str(op)) => op.as_str(),
                _ => ">=",
            };
            if !(count == 1.0 && operator == ">=") {
                return Err("waiting for a count of elements is not converted".to_owned());
            }
            let visible = !matches!(field(entries, "visible"), Some(Expr::Bool(false)));
            let state = if visible { "Visible" } else { "Present" };
            let target = locator(translator)?;
            translator.needs.sb_cdp.insert("State");
            vec![format!("{target}.wait_for(State::{state}).await?;")]
        }
        "waitForExpression" => {
            let expression = text_field(entries, "expression")?;
            vec![format!(
                "page.wait_for_function({}).await?;",
                rust_str(expression)
            )]
        }
        other => return Err(format!("the Recorder step type {other:?} is not converted")),
    };
    // A step that was recorded to cause a navigation waits for the new page.
    if let Some(Expr::Array(events)) = field(entries, "assertedEvents") {
        let navigates = events.iter().any(|event| match event {
            Expr::Object(event) => {
                matches!(field(event, "type"), Some(Expr::Str(t)) if t == "navigation")
            }
            _ => false,
        });
        if navigates {
            lines.push("page.wait_for_load().await?;".to_owned());
        }
    }
    Ok(lines)
}
