//! The pure CDP generator of the action recorder.
//!
//! [`ActionRecorder::to_rust_script`](super::ActionRecorder::to_rust_script)
//! writes a recording as `BaseCase` calls for the WebDriver engine. This module
//! writes the same recording as `Page` and `Locator` calls for the pure CDP
//! engine ([`sb_cdp`](crate::sb_cdp)), which is what Python's `--rec-sb-cdp`
//! does with `sb_cdp` calls.
//!
//! Every selector and value goes into the program as a Rust string literal
//! (`{:?}`), so recorded text can never become code. An action the pure CDP
//! engine has no call for is written as a `// skipped:` comment that says what
//! it was, as Python comments out the action.

use std::collections::BTreeSet;

use super::calls::{canonical_name, captcha_note, number, offset_parts, rust_call, CAPTCHA};
use super::{line, literal, RecordedAction};

/// A recorded action as pure CDP code.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CdpCall {
    /// The statements, one per line, without indentation.
    pub(super) lines: Vec<String>,
    /// The names the statements use from `seleniumbase_rs::sb_cdp`, besides
    /// `Browser` and `LaunchOptions`, which every program uses.
    pub(super) imports: Vec<&'static str>,
}

impl CdpCall {
    /// One statement that needs no import.
    fn statement(text: impl Into<String>) -> Self {
        Self::statements([text.into()])
    }

    /// Several statements that need no import.
    fn statements(lines: impl IntoIterator<Item = String>) -> Self {
        Self {
            lines: lines.into_iter().collect(),
            imports: Vec::new(),
        }
    }

    /// One statement that uses `import` from `sb_cdp`.
    fn importing(text: String, import: &'static str) -> Self {
        Self {
            lines: vec![text],
            imports: vec![import],
        }
    }
}

/// The longest pause a generated `sleep` is allowed: `Duration::from_secs_f64`
/// panics on a value it cannot hold, and a recording is not a place to find
/// that out.
const LONGEST_SLEEP_SECONDS: f64 = 1.0e9;

/// `page.locator(<selector>)` with the selector as a string literal.
fn locator(selector: &str) -> String {
    format!("page.locator({})", literal(selector))
}

/// The pure CDP statements for `action`, or why there are none.
///
/// The reasons are the ones the `BaseCase` generator gives for an unknown
/// action or a missing or malformed argument. An action that exists but has no
/// pure CDP call says so and quotes the `BaseCase` call it was.
pub(super) fn cdp_call(action: &RecordedAction) -> Result<CdpCall, String> {
    if action
        .value
        .as_deref()
        .is_some_and(|text| text.contains(super::SECRET_PLACEHOLDER))
    {
        return Err(format!(
            "{}: the text is a password; type it with the BaseCase program, which reads {}",
            action.name,
            super::SECRET_VARIABLE
        ));
    }

    let name = canonical_name(&action.name);
    let target = action.target.as_deref();
    let value = action.value.as_deref();
    let need_target = || target.ok_or_else(|| format!("{name} has no target"));
    let need_value = || value.ok_or_else(|| format!("{name} has no value"));
    // `page.locator(<target>).<call>.await?;`
    let on_target = |call: &str| -> Result<CdpCall, String> {
        Ok(CdpCall::statement(format!(
            "{}.{call}.await?;",
            locator(need_target()?)
        )))
    };
    // A click on the link whose text is the target; `prefix` is `link=` or `p_link=`.
    let link = |prefix: &str| -> Result<CdpCall, String> {
        let selector = format!("{prefix}{}", need_target()?);
        Ok(CdpCall::statement(format!(
            "{}.click().await?;",
            locator(&selector)
        )))
    };
    let select = |option: String| -> Result<CdpCall, String> {
        Ok(CdpCall::importing(
            format!(
                "{}.select_option({option}).await?;",
                locator(need_target()?)
            ),
            "SelectBy",
        ))
    };
    let wait = |state: &str| -> Result<CdpCall, String> {
        Ok(CdpCall::importing(
            format!(
                "{}.wait_for(State::{state}).await?;",
                locator(need_target()?)
            ),
            "State",
        ))
    };
    // `page.locator(<target>).expect()<negation>.<check>(<value>).await?;`
    let expect = |negation: &str, check: &str| -> Result<CdpCall, String> {
        Ok(CdpCall::statement(format!(
            "{}.expect(){negation}.{check}({}).await?;",
            locator(need_target()?),
            literal(need_value()?)
        )))
    };
    // A statement that takes nothing from the recording.
    let plain = |statement: &str| -> Result<CdpCall, String> { Ok(CdpCall::statement(statement)) };
    let local_storage = |call: &str| -> Result<CdpCall, String> {
        Ok(CdpCall::statement(format!(
            "page.local_storage().{call}.await?;"
        )))
    };

    match name {
        "open" | "goto" | "goto_if_not_url" => Ok(CdpCall::statement(format!(
            "page.goto({}).await?;",
            literal(need_target()?)
        ))),
        "click" | "cdp_click" | "slow_click" => on_target("click()"),
        "hover" => on_target("hover()"),
        "double_click" => on_target("double_click()"),
        "context_click" => on_target("right_click()"),
        "clear" => on_target("clear()"),
        "submit" => on_target("submit()"),
        "scroll_into_view" | "scroll_to" | "slow_scroll_to" | "smooth_scroll_to" => {
            on_target("scroll_into_view()")
        }
        "check_if_unchecked" => on_target("check()"),
        "uncheck_if_checked" => on_target("uncheck()"),
        // `Locator::type_text` empties the field and then types, as
        // `BaseCase::type_text` and Python's `sb.cdp.type` do.
        "type_text" | "update_text" => Ok(CdpCall::statement(format!(
            "{}.type_text({}).await?;",
            locator(need_target()?),
            literal(need_value()?)
        ))),
        // `Locator::send_keys` adds to what the field holds, as
        // `BaseCase::add_text` does. `type_text` here would empty the field
        // first, so a recorded `add_text` would erase what was there.
        "add_text" | "send_keys" => Ok(CdpCall::statement(format!(
            "{}.send_keys({}).await?;",
            locator(need_target()?),
            literal(need_value()?)
        ))),
        "click_link_text" | "click_link" => link("link="),
        "click_partial_link_text" | "click_partial_link" => link("p_link="),
        "select_option_by_text" => select(format!("SelectBy::Text({})", literal(need_value()?))),
        "select_option_by_value" => select(format!("SelectBy::Value({})", literal(need_value()?))),
        "select_option_by_index" => select(format!(
            "SelectBy::Index({})",
            number::<usize>(name, "index", need_value()?)?
        )),
        "drag_and_drop" => Ok(CdpCall::statement(format!(
            "{}.drag_to(&{}).await?;",
            locator(need_target()?),
            locator(need_value()?)
        ))),
        "wait_for_element_visible" => wait("Visible"),
        "wait_for_element_present" => wait("Present"),
        "wait_for_element_not_visible" => wait("Hidden"),
        "wait_for_element_absent" => wait("Absent"),
        "wait_for_ready_state_complete" => plain("page.wait_for_load().await?;"),
        "assert_text" | "assert_text_visible" => expect("", "to_contain_text"),
        "assert_text_not_visible" => expect(".not()", "to_contain_text"),
        "assert_exact_text" => expect("", "to_have_text"),
        "assert_attribute" => {
            let pair = need_value()?;
            let (key, content) = pair
                .split_once('=')
                .ok_or_else(|| format!("{name}: value {pair:?} is not name=content"))?;
            Ok(CdpCall::statement(format!(
                "{}.expect().to_have_attribute({}, {}).await?;",
                locator(need_target()?),
                literal(key.trim()),
                literal(content.trim())
            )))
        }
        "assert_title" => Ok(CdpCall::statement(format!(
            "page.expect().to_have_title({}).await?;",
            literal(need_target()?)
        ))),
        "assert_title_contains" => Ok(CdpCall::statement(format!(
            "page.expect().to_contain_title({}).await?;",
            literal(need_target()?)
        ))),
        // What Python's recorder writes for typing into a field with `--uc`.
        "press_keys" => Ok(CdpCall::statement(format!(
            "{}.send_keys({}).await?;",
            locator(need_target()?),
            literal(need_value()?)
        ))),
        "wait_for_element" => wait("Visible"),
        "assert_element" => on_target("expect().to_be_visible()"),
        "assert_element_present" => on_target("expect().to_exist()"),
        "assert_element_not_visible" => on_target("expect().not().to_be_visible()"),
        "assert_exact_text_not_visible" => expect(".not()", "to_have_text"),
        "assert_url" => Ok(CdpCall::statement(format!(
            "page.expect().to_have_url({}).await?;",
            literal(need_target()?)
        ))),
        "assert_url_contains" => Ok(CdpCall::statement(format!(
            "page.expect().to_contain_url({}).await?;",
            literal(need_target()?)
        ))),

        "go_back" => plain("page.back().await?;"),
        "go_forward" => plain("page.forward().await?;"),
        "refresh" => plain("page.reload().await?;"),
        "maximize_window" => plain("page.window().maximize().await?;"),
        "scroll_to_bottom" => Ok(CdpCall::importing(
            "page.scroll(Scroll::Bottom).await?;".to_owned(),
            "Scroll",
        )),
        "scroll_to_top" => Ok(CdpCall::importing(
            "page.scroll(Scroll::Top).await?;".to_owned(),
            "Scroll",
        )),
        "delete_all_cookies" => plain("page.cookies().clear().await?;"),
        "set_local_storage_item" => local_storage(&format!(
            "set({}, {})",
            literal(need_target()?),
            literal(need_value()?)
        )),
        "remove_local_storage_item" => {
            local_storage(&format!("remove({})", literal(need_target()?)))
        }
        "clear_local_storage" => local_storage("clear()"),
        "click_with_offset" => {
            let (x, y, center) = offset_parts(name, need_value()?)?;
            // The offset is a float; the whole numbers recorded are written as such.
            Ok(CdpCall::statement(format!(
                "{}.click_with_offset({}.0, {}.0, {center}).await?;",
                locator(need_target()?),
                number::<i64>(name, "x offset", x)?,
                number::<i64>(name, "y offset", y)?
            )))
        }
        CAPTCHA => Err(captcha_note(target, "page.solve_captcha()")),

        "sleep" => {
            let text = need_value()?;
            let seconds = text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|s| s.is_finite() && (0.0..=LONGEST_SLEEP_SECONDS).contains(s))
                .ok_or_else(|| format!("{name}: {text:?} is not a number of seconds"))?;
            // `sleep` returns nothing, so there is no `?`.
            Ok(CdpCall::statement(format!(
                "tokio::time::sleep(std::time::Duration::from_secs_f64({seconds:?})).await;"
            )))
        }
        _ => Err(match rust_call(action) {
            Ok(webdriver) => format!(
                "{name}: no pure CDP call for this action; the WebDriver form is {webdriver}"
            ),
            // An unknown action, or one `BaseCase` cannot replay either.
            Err(reason) => reason,
        }),
    }
}

/// The whole program: the imports, `entry` (the attribute lines and the
/// `async fn name()` that start the function), and a body that launches a
/// browser, replays `actions` and closes it.
pub(super) fn program(actions: &[RecordedAction], entry: &str) -> String {
    let mut imports: BTreeSet<&'static str> = BTreeSet::from(["Browser", "LaunchOptions"]);
    let mut body = String::new();
    for action in actions {
        match cdp_call(action) {
            Ok(call) => {
                imports.extend(call.imports);
                for statement in &call.lines {
                    line(&mut body, statement);
                }
            }
            // The reasons quote text with `{:?}`, so a hostile name or selector
            // cannot end the comment.
            Err(reason) => line(&mut body, &format!("// skipped: {reason}")),
        }
    }
    let names = imports.into_iter().collect::<Vec<_>>().join(", ");
    format!(
        "use seleniumbase_rs::sb_cdp::{{{names}}};\nuse seleniumbase_rs::tokio;\n\n\
         {entry} -> Result<(), Box<dyn std::error::Error>> {{\n    \
         let browser = Browser::launch(LaunchOptions::builder().build()?).await?;\n    \
         let page = browser.default_page().await?;\n\
         {body}    browser.close().await?;\n    Ok(())\n}}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::recorder::calls::{marks, Mark, ALIASES, CALLS};
    use crate::api::recorder::ActionRecorder;

    fn action(name: &str, target: Option<&str>, value: Option<&str>) -> RecordedAction {
        RecordedAction::new(name, target, value)
    }

    /// The lines `cdp_call` writes for an action, or its reason for skipping it.
    fn lines(name: &str, target: Option<&str>, value: Option<&str>) -> Result<Vec<String>, String> {
        cdp_call(&action(name, target, value)).map(|call| call.lines)
    }

    fn imports(name: &str, target: Option<&str>, value: Option<&str>) -> Vec<&'static str> {
        cdp_call(&action(name, target, value))
            .expect("the action has a pure CDP call")
            .imports
    }

    /// Each supported action, the arguments that make it generate, and what it
    /// becomes. A raw string with `##` holds the `"#` of a CSS id.
    #[allow(clippy::type_complexity)]
    fn expected() -> Vec<(
        &'static str,
        Option<&'static str>,
        Option<&'static str>,
        Vec<&'static str>,
    )> {
        vec![
            (
                "open",
                Some("https://example.com/a?b=1"),
                None,
                vec![r##"page.goto("https://example.com/a?b=1").await?;"##],
            ),
            (
                "goto",
                Some("https://example.com/a?b=1"),
                None,
                vec![r##"page.goto("https://example.com/a?b=1").await?;"##],
            ),
            (
                "press_keys",
                Some("#q"),
                Some("rust"),
                vec![r##"page.locator("#q").send_keys("rust").await?;"##],
            ),
            (
                "wait_for_element",
                Some("#late"),
                None,
                vec![r##"page.locator("#late").wait_for(State::Visible).await?;"##],
            ),
            (
                "assert_element",
                Some("#m"),
                None,
                vec![r##"page.locator("#m").expect().to_be_visible().await?;"##],
            ),
            (
                "assert_element_present",
                Some("#m"),
                None,
                vec![r##"page.locator("#m").expect().to_exist().await?;"##],
            ),
            (
                "assert_element_not_visible",
                Some("#m"),
                None,
                vec![r##"page.locator("#m").expect().not().to_be_visible().await?;"##],
            ),
            (
                "assert_exact_text_not_visible",
                Some("#m"),
                Some("Hi"),
                vec![r##"page.locator("#m").expect().not().to_have_text("Hi").await?;"##],
            ),
            (
                "assert_url",
                Some("https://example.com/"),
                None,
                vec![r##"page.expect().to_have_url("https://example.com/").await?;"##],
            ),
            (
                "assert_url_contains",
                Some("example"),
                None,
                vec![r##"page.expect().to_contain_url("example").await?;"##],
            ),
            (
                "click",
                Some("#go"),
                None,
                vec![r##"page.locator("#go").click().await?;"##],
            ),
            (
                "cdp_click",
                Some("#shadow"),
                None,
                vec![r##"page.locator("#shadow").click().await?;"##],
            ),
            (
                "slow_click",
                Some("#go"),
                None,
                vec![r##"page.locator("#go").click().await?;"##],
            ),
            (
                "hover",
                Some("#menu"),
                None,
                vec![r##"page.locator("#menu").hover().await?;"##],
            ),
            (
                "double_click",
                Some("#row"),
                None,
                vec![r##"page.locator("#row").double_click().await?;"##],
            ),
            (
                "context_click",
                Some("#row"),
                None,
                vec![r##"page.locator("#row").right_click().await?;"##],
            ),
            (
                "clear",
                Some("#q"),
                None,
                vec![r##"page.locator("#q").clear().await?;"##],
            ),
            (
                "submit",
                Some("form"),
                None,
                vec![r#"page.locator("form").submit().await?;"#],
            ),
            (
                "scroll_into_view",
                Some("#end"),
                None,
                vec![r##"page.locator("#end").scroll_into_view().await?;"##],
            ),
            (
                "scroll_to",
                Some("#end"),
                None,
                vec![r##"page.locator("#end").scroll_into_view().await?;"##],
            ),
            (
                "slow_scroll_to",
                Some("#end"),
                None,
                vec![r##"page.locator("#end").scroll_into_view().await?;"##],
            ),
            (
                "smooth_scroll_to",
                Some("#end"),
                None,
                vec![r##"page.locator("#end").scroll_into_view().await?;"##],
            ),
            (
                "check_if_unchecked",
                Some("#agree"),
                None,
                vec![r##"page.locator("#agree").check().await?;"##],
            ),
            (
                "uncheck_if_checked",
                Some("#spam"),
                None,
                vec![r##"page.locator("#spam").uncheck().await?;"##],
            ),
            (
                "type_text",
                Some("#q"),
                Some("rust"),
                vec![r##"page.locator("#q").type_text("rust").await?;"##],
            ),
            (
                "update_text",
                Some("#q"),
                Some("rust"),
                vec![r##"page.locator("#q").type_text("rust").await?;"##],
            ),
            (
                "add_text",
                Some("#q"),
                Some(" more"),
                vec![r##"page.locator("#q").send_keys(" more").await?;"##],
            ),
            (
                "send_keys",
                Some("#q"),
                Some("abc"),
                vec![r##"page.locator("#q").send_keys("abc").await?;"##],
            ),
            (
                "click_link_text",
                Some("Log out"),
                None,
                vec![r#"page.locator("link=Log out").click().await?;"#],
            ),
            (
                "click_link",
                Some("Log out"),
                None,
                vec![r#"page.locator("link=Log out").click().await?;"#],
            ),
            (
                "click_partial_link_text",
                Some("Log"),
                None,
                vec![r#"page.locator("p_link=Log").click().await?;"#],
            ),
            (
                "click_partial_link",
                Some("Log"),
                None,
                vec![r#"page.locator("p_link=Log").click().await?;"#],
            ),
            (
                "select_option_by_text",
                Some("#c"),
                Some("Canada"),
                vec![r##"page.locator("#c").select_option(SelectBy::Text("Canada")).await?;"##],
            ),
            (
                "select_option_by_value",
                Some("#c"),
                Some("ca"),
                vec![r##"page.locator("#c").select_option(SelectBy::Value("ca")).await?;"##],
            ),
            (
                "select_option_by_index",
                Some("#c"),
                Some("2"),
                vec![r##"page.locator("#c").select_option(SelectBy::Index(2)).await?;"##],
            ),
            (
                "drag_and_drop",
                Some("#a"),
                Some("#b"),
                vec![r##"page.locator("#a").drag_to(&page.locator("#b")).await?;"##],
            ),
            (
                "wait_for_element_visible",
                Some("#x"),
                None,
                vec![r##"page.locator("#x").wait_for(State::Visible).await?;"##],
            ),
            (
                "wait_for_element_present",
                Some("#x"),
                None,
                vec![r##"page.locator("#x").wait_for(State::Present).await?;"##],
            ),
            (
                "wait_for_element_not_visible",
                Some("#x"),
                None,
                vec![r##"page.locator("#x").wait_for(State::Hidden).await?;"##],
            ),
            (
                "wait_for_element_absent",
                Some("#x"),
                None,
                vec![r##"page.locator("#x").wait_for(State::Absent).await?;"##],
            ),
            (
                "wait_for_ready_state_complete",
                None,
                None,
                vec!["page.wait_for_load().await?;"],
            ),
            (
                "assert_text",
                Some("#m"),
                Some("Welcome"),
                vec![r##"page.locator("#m").expect().to_contain_text("Welcome").await?;"##],
            ),
            (
                "assert_text_visible",
                Some("#m"),
                Some("Welcome"),
                vec![r##"page.locator("#m").expect().to_contain_text("Welcome").await?;"##],
            ),
            (
                "assert_text_not_visible",
                Some("#m"),
                Some("Failed"),
                vec![r##"page.locator("#m").expect().not().to_contain_text("Failed").await?;"##],
            ),
            (
                "assert_exact_text",
                Some("#m"),
                Some("Welcome"),
                vec![r##"page.locator("#m").expect().to_have_text("Welcome").await?;"##],
            ),
            (
                "assert_attribute",
                Some("a"),
                Some("href=/home"),
                vec![r#"page.locator("a").expect().to_have_attribute("href", "/home").await?;"#],
            ),
            (
                "assert_title",
                Some("Home"),
                None,
                vec![r#"page.expect().to_have_title("Home").await?;"#],
            ),
            (
                "assert_title_contains",
                Some("Ho"),
                None,
                vec![r#"page.expect().to_contain_title("Ho").await?;"#],
            ),
            ("go_back", None, None, vec!["page.back().await?;"]),
            ("go_forward", None, None, vec!["page.forward().await?;"]),
            ("refresh", None, None, vec!["page.reload().await?;"]),
            (
                "maximize_window",
                None,
                None,
                vec!["page.window().maximize().await?;"],
            ),
            (
                "scroll_to_bottom",
                None,
                None,
                vec!["page.scroll(Scroll::Bottom).await?;"],
            ),
            (
                "scroll_to_top",
                None,
                None,
                vec!["page.scroll(Scroll::Top).await?;"],
            ),
            (
                "delete_all_cookies",
                None,
                None,
                vec!["page.cookies().clear().await?;"],
            ),
            (
                "set_local_storage_item",
                Some("theme"),
                Some("dark"),
                vec![r#"page.local_storage().set("theme", "dark").await?;"#],
            ),
            (
                "remove_local_storage_item",
                Some("theme"),
                None,
                vec![r#"page.local_storage().remove("theme").await?;"#],
            ),
            (
                "clear_local_storage",
                None,
                None,
                vec!["page.local_storage().clear().await?;"],
            ),
            (
                "click_with_offset",
                Some("#c"),
                Some("5, -7"),
                vec![r##"page.locator("#c").click_with_offset(5.0, -7.0, false).await?;"##],
            ),
            (
                "sleep",
                None,
                Some("1.5"),
                vec!["tokio::time::sleep(std::time::Duration::from_secs_f64(1.5)).await;"],
            ),
        ]
    }

    #[test]
    fn each_supported_action_becomes_the_pure_cdp_lines_that_do_the_same() {
        for (name, target, value, want) in expected() {
            assert_eq!(lines(name, target, value).unwrap(), want, "{name}");
        }
    }

    #[test]
    fn the_names_python_and_older_recordings_use_are_understood() {
        assert_eq!(
            lines("goto", Some("https://example.com"), None).unwrap(),
            [r#"page.goto("https://example.com").await?;"#]
        );
        assert_eq!(
            lines("navigate", Some("https://example.com"), None).unwrap(),
            [r#"page.goto("https://example.com").await?;"#]
        );
        assert_eq!(
            lines("type", Some("#q"), Some("rust")).unwrap(),
            [r##"page.locator("#q").type_text("rust").await?;"##]
        );
    }

    #[test]
    fn a_recorded_add_text_keeps_what_the_field_holds() {
        // `Locator::type_text` empties the field first, as Python's `type` does,
        // so an appending action written with it would erase the text it should
        // add to. `send_keys` is the call that appends.
        for name in ["add_text", "send_keys"] {
            let written = lines(name, Some("#note"), Some(" more")).unwrap();
            assert_eq!(
                written,
                [r##"page.locator("#note").send_keys(" more").await?;"##],
                "{name}"
            );
            for statement in &written {
                assert!(
                    !statement.contains("type_text") && !statement.contains("clear"),
                    "{name} would empty the field: {statement}"
                );
            }
        }
        // The actions that replace the text write the call that empties the
        // field itself, once.
        for name in ["type_text", "update_text", "type"] {
            let written = lines(name, Some("#note"), Some("new")).unwrap();
            assert_eq!(written.len(), 1, "{name}: {written:?}");
            assert!(
                written[0].contains(".type_text(\"new\")"),
                "{name}: {written:?}"
            );
        }
    }

    #[test]
    fn a_click_with_offset_counts_from_the_top_left_corner_unless_it_says_centre() {
        assert_eq!(
            lines("click_with_offset", Some("#c"), Some("5, -7")).unwrap(),
            [r##"page.locator("#c").click_with_offset(5.0, -7.0, false).await?;"##]
        );
        assert_eq!(
            lines("click_with_offset", Some("#c"), Some("5, -7, center")).unwrap(),
            [r##"page.locator("#c").click_with_offset(5.0, -7.0, true).await?;"##]
        );
        assert_eq!(
            lines("click_with_offset", Some("#c"), Some("5, -7, middle")).unwrap_err(),
            r#"click_with_offset: value "5, -7, middle" is not x, y"#
        );
    }

    #[test]
    fn every_supported_action_is_listed_and_is_one_the_webdriver_generator_knows() {
        let listed: Vec<&str> = expected().iter().map(|(name, ..)| *name).collect();
        // The table marks the actions that have a pure CDP call.
        let supported: Vec<&str> = CALLS
            .iter()
            .map(|(name, _, _)| *name)
            .filter(|name| marks(name).contains(&Mark::Cdp))
            .collect();
        for name in &listed {
            assert!(
                supported.contains(name),
                "{name} is not marked Cdp in the table"
            );
        }
        for name in &supported {
            assert!(
                listed.contains(name),
                "{name} has no expected lines in this test"
            );
        }
        for (alias, canonical) in ALIASES {
            assert!(
                supported.contains(canonical),
                "the alias {alias} points at {canonical}, which has no pure CDP call"
            );
        }
    }

    #[test]
    fn what_needs_an_import_says_so_and_nothing_else_does() {
        assert_eq!(
            imports("select_option_by_text", Some("#c"), Some("x")),
            ["SelectBy"]
        );
        assert_eq!(
            imports("wait_for_element_visible", Some("#x"), None),
            ["State"]
        );
        assert_eq!(imports("scroll_to_top", None, None), ["Scroll"]);
        assert!(imports("click_with_offset", Some("#c"), Some("1,2")).is_empty());
        for (name, target, value, _) in expected() {
            let needed = imports(name, target, value);
            let text = lines(name, target, value).unwrap().join("\n");
            for word in ["SelectBy", "State", "Scroll"] {
                assert_eq!(
                    text.contains(&format!("{word}::")),
                    needed.contains(&word),
                    "{name}: {word}"
                );
            }
        }
    }

    #[test]
    fn recorded_text_stays_inside_its_string_literal() {
        let hostile = "a\"); std::process::exit(1); (\"\n\\ \u{2028} 🦀";
        for (name, target, value, _) in expected() {
            // Put the hostile text where the action takes text.
            let target = target.map(|_| hostile);
            let value = match (name, value) {
                ("select_option_by_index", _) => Some("0"),
                ("click_with_offset", _) => Some("1, 2"),
                ("sleep", _) => Some("0.5"),
                ("assert_attribute", _) => Some("name=\"; x"),
                (_, Some(_)) => Some(hostile),
                (_, None) => None,
            };
            let Ok(call) = cdp_call(&action(name, target, value)) else {
                panic!("{name} refused hostile text");
            };
            for statement in &call.lines {
                assert_eq!(statement.lines().count(), 1, "{name}: {statement:?}");
                let source = format!("async fn f() {{ {statement} }}");
                syn::parse_file(&source)
                    .unwrap_or_else(|e| panic!("{name}: {statement:?} does not parse: {e}"));
            }
        }
        let call = lines("click", Some("a\"b\nc\\d"), None).unwrap();
        assert_eq!(call, [r#"page.locator("a\"b\nc\\d").click().await?;"#]);
    }

    #[test]
    fn an_action_with_no_pure_cdp_call_says_what_it_was() {
        for name in [
            "switch_to_frame",
            "js_click",
            "human_click",
            "find_element",
            "wait_for_element_clickable",
        ] {
            let reason = lines(name, Some("#f"), None).unwrap_err();
            assert!(
                reason.starts_with(&format!("{name}: no pure CDP call")),
                "{reason}"
            );
            assert!(
                reason.contains(&format!("sb.{name}(")),
                "the WebDriver form is quoted: {reason}"
            );
        }
        let reason = lines("accept_alert", None, None).unwrap_err();
        assert!(reason.contains("sb.accept_alert().await?;"), "{reason}");
    }

    #[test]
    fn an_unknown_action_or_a_missing_argument_is_reported_the_way_the_webdriver_generator_does() {
        assert_eq!(
            lines("clik", Some("#a"), None).unwrap_err(),
            r#"unknown action "clik"; did you mean "click"?"#
        );
        assert_eq!(
            lines("click", None, None).unwrap_err(),
            "click has no target"
        );
        assert_eq!(
            lines("type_text", Some("#a"), None).unwrap_err(),
            "type_text has no value"
        );
        assert_eq!(
            lines("select_option_by_index", Some("#a"), Some("two")).unwrap_err(),
            r#"select_option_by_index: index "two" is not a whole number in range"#
        );
        assert_eq!(
            lines("assert_attribute", Some("#a"), Some("no-equals")).unwrap_err(),
            r#"assert_attribute: value "no-equals" is not name=content"#
        );
        assert_eq!(
            lines("click_with_offset", Some("#a"), Some("5")).unwrap_err(),
            r#"click_with_offset: value "5" is not x, y"#
        );
        for bad in ["soon", "-1", "NaN", "inf", "1e300"] {
            let reason = lines("sleep", None, Some(bad)).unwrap_err();
            assert!(
                reason.contains("is not a number of seconds"),
                "{bad}: {reason}"
            );
        }
        assert!(lines("drag_and_drop_with_offset", Some("#a"), Some("#b"))
            .unwrap_err()
            .contains("not the offset"));
    }

    fn sample() -> ActionRecorder {
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com"), None);
        recorder.record("type_text", Some("#q"), Some("rust"));
        recorder.record("click", Some("button"), None);
        recorder.record("wait_for_element_visible", Some("#results"), None);
        recorder.record("switch_to_frame", Some("#f"), None);
        recorder.record("assert_title_contains", Some("Rust"), None);
        recorder
    }

    #[test]
    fn the_script_launches_a_browser_replays_the_actions_and_closes_it() {
        let script = sample().to_cdp_script();
        let want = r##"use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, State};
use seleniumbase_rs::tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let browser = Browser::launch(LaunchOptions::builder().build()?).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;
    page.locator("#q").type_text("rust").await?;
    page.locator("button").click().await?;
    page.locator("#results").wait_for(State::Visible).await?;
    // skipped: switch_to_frame: no pure CDP call for this action; the WebDriver form is sb.switch_to_frame("#f").await?;
    page.expect().to_contain_title("Rust").await?;
    browser.close().await?;
    Ok(())
}
"##;
        assert_eq!(script, want);
        syn::parse_file(&script).expect("the program parses");
    }

    #[test]
    fn a_script_without_extra_imports_imports_only_the_two_it_always_uses() {
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com"), None);
        let script = recorder.to_cdp_script();
        assert!(
            script.starts_with("use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};\nuse seleniumbase_rs::tokio;\n\n"),
            "{script}"
        );
        assert!(!script.contains("skipped"), "{script}");
    }

    #[test]
    fn the_imports_are_sorted_and_listed_once() {
        let mut recorder = ActionRecorder::default();
        recorder.record("wait_for_element_visible", Some("#a"), None);
        recorder.record("wait_for_element_absent", Some("#a"), None);
        recorder.record("scroll_to_top", None, None);
        recorder.record("click_with_offset", Some("#a"), Some("1, 2"));
        recorder.record("select_option_by_index", Some("#a"), Some("1"));
        let script = recorder.to_cdp_script();
        assert!(
            script.starts_with(
                "use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Scroll, SelectBy, State};\n"
            ),
            "{script}"
        );
    }

    #[test]
    fn a_skipped_action_cannot_end_its_comment_early() {
        let mut recorder = ActionRecorder::default();
        recorder.record("fly\nclick(\"#x\")", Some("#a"), None);
        recorder.record("switch_to_frame", Some("#a\nb\u{2028}c"), None);
        let script = recorder.to_cdp_script();
        for comment in script
            .lines()
            .filter(|l| l.trim_start().starts_with("// skipped"))
        {
            syn::parse_file(&format!("fn f() {{\n{comment}\n}}")).expect("a comment");
        }
        assert_eq!(script.matches("// skipped").count(), 2, "{script}");
        assert!(!script.contains("page.locator(\"#x\")"), "{script}");
    }

    #[test]
    fn the_test_form_has_the_attribute_and_a_safe_function_name() {
        let test = sample().to_cdp_test("login flow");
        assert!(
            test.contains("#[tokio::test]\nasync fn login_flow() -> Result<(), Box<dyn std::error::Error>> {\n"),
            "{test}"
        );
        assert!(!test.contains("fn main"), "{test}");
        assert!(
            test.contains("    browser.close().await?;\n    Ok(())\n}\n"),
            "{test}"
        );
        syn::parse_file(&test).expect("the test parses");
        assert!(sample().to_cdp_test("match").contains("async fn match_()"));
    }

    #[test]
    fn an_empty_recording_still_makes_a_complete_program() {
        let script = ActionRecorder::default().to_cdp_script();
        assert!(script.contains("let page = browser.default_page().await?;\n    browser.close()"));
        syn::parse_file(&script).expect("the program parses");
    }

    /// A recording with every supported action once, then two with no pure CDP
    /// call.
    fn everything() -> ActionRecorder {
        let mut recorder = ActionRecorder::default();
        for (name, target, value, _) in expected() {
            recorder.record(name, target, value);
        }
        recorder.record("switch_to_frame", Some("#frame"), None);
        recorder.record("js_click", Some("#hidden"), None);
        recorder
    }

    #[test]
    fn the_program_in_tests_golden_rust_is_what_the_generator_writes() {
        // `tests/recorder_cdp_compiles.rs` compiles this file against the real
        // `sb_cdp` API, so every call it holds is type-checked. After changing
        // the generator, write the file again with `SB_UPDATE_GOLDEN=1`.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden_rust/cdp_script.rs");
        let generated = everything().to_cdp_script();
        if std::env::var_os("SB_UPDATE_GOLDEN").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &generated).unwrap();
        }
        let golden = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            golden, generated,
            "tests/golden_rust/cdp_script.rs is out of date; run this test with SB_UPDATE_GOLDEN=1"
        );
    }

    #[test]
    fn the_test_file_goes_into_the_folder_under_the_name_given() {
        let dir = tempfile::tempdir().unwrap();
        let path = sample()
            .save_recording_as_cdp_test(dir.path(), "login")
            .unwrap();
        assert_eq!(path, dir.path().join("login.rs"));
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(saved, sample().to_cdp_test("login"));
        for bad in ["", ".", "..", "a/b", "a\\b"] {
            assert!(
                sample()
                    .save_recording_as_cdp_test(dir.path(), bad)
                    .is_err(),
                "{bad:?}"
            );
        }
    }
}
