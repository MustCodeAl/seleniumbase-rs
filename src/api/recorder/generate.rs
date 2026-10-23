//! Python's `generate_sbase_code` and `generate_gherkin`: processed raw
//! actions in, [`RecordedAction`]s and Gherkin out.
//!
//! [`to_actions`] is the step after [`process`](super::process): it maps each
//! kind the page recorder (and the API methods that add actions) writes to the
//! call Python writes for it, as a name, a target and a value that the code
//! generators of [`ActionRecorder`](super::ActionRecorder) turn into Rust. The
//! kinds that make no call (`_skip`, `sk_op`, `o_url`, `mo_dn`, `drags`,
//! `submi`, `chfil`, ...) produce nothing.

use serde_json::Value;

use super::raw::{text, RawAction};
use super::{keep_secrets, RecordedAction, SECRET_PLACEHOLDER};

/// Element `index` of a JSON array, or null.
fn nth(value: &Value, index: usize) -> &Value {
    static NULL: Value = Value::Null;
    value.get(index).unwrap_or(&NULL)
}

/// A JSON value as the text Python's `%s` writes.
fn shown(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Python's truthiness of a JSON value.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|float| float != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// Python's `urllib.parse.unquote(url, errors="strict")` for a URL that has a
/// `%`: the URL as it was when the escapes do not decode to UTF-8.
fn unquoted(url: &str) -> String {
    if !url.contains('%') {
        return url.to_owned();
    }
    let bytes = url.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = |at: usize| bytes.get(at).and_then(|byte| (*byte as char).to_digit(16));
        if bytes[index] == b'%' {
            if let (Some(high), Some(low)) = (hex(index + 1), hex(index + 2)) {
                // Two hex digits make a value below 256.
                out.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| url.to_owned())
}

/// Python's `unicodedata.normalize("NFKC", text)` for the characters a page's
/// text most often holds in a form NFKC changes, and the pilcrow that
/// `generate_sbase_code` removes: spaces of every width become a space, the
/// full-width ASCII block its ASCII, the ligatures `ﬀ`..`ﬆ` their letters,
/// `…` three dots and a few symbols their letters. Other characters stay as
/// they are (a full table needs a normalization crate this one does not
/// depend on).
fn normalized(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{00B6}' => {}
            '\u{00A0}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => {
                out.push(' ');
            }
            '\u{FF01}'..='\u{FF5E}' => {
                // The full-width block sits 0xFEE0 above ASCII.
                out.push(char::from_u32(c as u32 - 0xFEE0).unwrap_or(c));
            }
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB05}' | '\u{FB06}' => out.push_str("st"),
            '\u{2026}' => out.push_str("..."),
            '\u{2122}' => out.push_str("TM"),
            '\u{2120}' => out.push_str("SM"),
            '\u{00B2}' => out.push('2'),
            '\u{00B3}' => out.push('3'),
            '\u{00B9}' => out.push('1'),
            other => out.push(other),
        }
    }
    out
}

/// The typed text of an `input`: the text itself, or, for a password, the
/// placeholder that the generators read from `SB_RECORDED_SECRET` (the typed
/// text stays when `SB_REC_KEEP_SECRETS=true`). An Enter that ended the text
/// is kept.
fn typed(row: &RawAction) -> String {
    let typed = text(&row.detail);
    if row.secret && !keep_secrets() {
        if typed.ends_with('\n') {
            format!("{SECRET_PLACEHOLDER}\n")
        } else {
            SECRET_PLACEHOLDER.to_owned()
        }
    } else {
        typed.to_owned()
    }
}

/// Python's `generate_sbase_code`, producing calls instead of Python text.
///
/// `sorted` is the output of [`process`](super::process). `uc` turns
/// `type_text` into `press_keys`, as Python's `--uc` output does. The
/// timestamp of each action is the time the page recorded.
///
/// A text typed into a password field (the page script marks it) is
/// [`SECRET_PLACEHOLDER`] instead, unless `SB_REC_KEEP_SECRETS=true`.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::api::recorder::{process, to_actions, RawAction};
/// use serde_json::json;
///
/// let raw = vec![
///     RawAction::new("begin", json!("https://a.test"), json!("https://a.test/"), 1000),
///     RawAction::new("input", json!("#q"), json!("rust\n"), 1200),
/// ];
/// let actions = to_actions(&process(raw, 0, &[]), false);
/// assert_eq!(actions[0].name, "goto");
/// assert_eq!(actions[1].name, "type_text");
/// assert_eq!(actions[1].value.as_deref(), Some("rust\n"));
/// ```
pub fn to_actions(sorted: &[RawAction], uc: bool) -> Vec<RecordedAction> {
    sorted.iter().filter_map(|row| action(row, uc)).collect()
}

/// The call one processed action makes, if it makes one.
fn action(row: &RawAction, uc: bool) -> Option<RecordedAction> {
    let arg = text(&row.arg).to_owned();
    let detail = text(&row.detail).to_owned();
    let make = |name: &str, target: Option<String>, value: Option<String>| RecordedAction {
        name: name.to_owned(),
        target,
        value,
        timestamp_ms: u128::try_from(row.time_ms).unwrap_or(0),
    };
    let on = |name: &str| make(name, Some(arg.clone()), None);
    let bare = |name: &str| make(name, None, None);
    let pair = |name: &str| make(name, Some(arg.clone()), Some(detail.clone()));
    // `[selector, name, value]` of an attribute action.
    let attribute = |name: &str| {
        let parts = &row.arg;
        let value = format!("{}={}", shown(nth(parts, 1)), shown(nth(parts, 2)));
        make(name, Some(shown(nth(parts, 0))), Some(value))
    };
    // `[text, selector]` of a text assertion.
    let assertion = |name: &str| {
        let parts = &row.arg;
        make(
            name,
            Some(shown(nth(parts, 1))),
            Some(normalized(&shown(nth(parts, 0)))),
        )
    };
    Some(match row.kind.as_str() {
        "begin" | "_url_" => make("goto", Some(unquoted(&detail)), None),
        "f_url" => make("goto_if_not_url", Some(unquoted(&detail)), None),
        "click" => on("click"),
        "dbclk" => on("double_click"),
        "js_cl" => on("js_click"),
        "js_ca" => on("js_click_all"),
        "jq_cl" => on("jquery_click"),
        "jq_ca" => on("jquery_click_all"),
        "r_clk" => on("context_click"),
        "hover" => on("hover"),
        "hi_li" => on("highlight"),
        "canva" => {
            let parts = &row.arg;
            let offset = format!("{}, {}", shown(nth(parts, 1)), shown(nth(parts, 2)));
            make(
                "click_with_offset",
                Some(shown(nth(parts, 0))),
                Some(offset),
            )
        }
        "input" => make(
            if uc { "press_keys" } else { "type_text" },
            Some(arg.clone()),
            Some(typed(row)),
        ),
        "js_ty" => pair("js_type"),
        "jq_ty" => pair("jquery_type"),
        "pkeys" => pair("press_keys"),
        "e_mfa" => pair("enter_mfa_code"),
        "h_clk" => pair("hover_and_click"),
        "ddrop" => pair("drag_and_drop"),
        "s_opt" => pair("select_option_by_text"),
        "set_v" => pair("set_value"),
        "cho_f" => pair("choose_file"),
        "sw_fr" => on("switch_to_frame"),
        "sw_dc" => bare("switch_to_default_content"),
        "sw_pf" => bare("switch_to_parent_frame"),
        "s_c_f" => on("set_content_to_frame"),
        "s_c_d" => bare(if truthy(&row.arg) {
            "set_content_to_parent"
        } else {
            "set_content_to_default"
        }),
        "sleep" => make("sleep", None, Some(shown(&row.arg))),
        "wf_el" => on("wait_for_element"),
        "as_el" => on("assert_element"),
        "as_ep" => on("assert_element_present"),
        "asenv" => on("assert_element_not_visible"),
        "asnet" => on("assert_non_empty_text"),
        "da_el" => on("deferred_assert_element"),
        "da_ep" => on("deferred_assert_element_present"),
        "danet" => on("deferred_assert_non_empty_text"),
        "s_at_" => attribute("set_attribute"),
        "s_ats" => attribute("set_attributes"),
        "acc_a" => bare("accept_alert"),
        "dis_a" => bare("dismiss_alert"),
        "as_lt" => on("assert_link_text"),
        "as_ti" => on("assert_title"),
        "as_tc" => on("assert_title_contains"),
        "a_url" => on("assert_url"),
        "a_u_c" => on("assert_url_contains"),
        "as_df" => on("assert_downloaded_file"),
        "do_fi" => {
            let parts = &row.arg;
            let destination = shown(nth(parts, 1));
            make(
                "download_file",
                Some(shown(nth(parts, 0))),
                (!destination.is_empty()).then_some(destination),
            )
        }
        "as_at" => attribute("assert_attribute"),
        "as_te" => assertion("assert_text"),
        "as_et" => assertion("assert_exact_text"),
        "astnv" => assertion("assert_text_not_visible"),
        "aetnv" => assertion("assert_exact_text_not_visible"),
        "da_te" => assertion("deferred_assert_text"),
        "da_et" => assertion("deferred_assert_exact_text"),
        "s_scr" => on("save_screenshot"),
        "ss_tf" => {
            let parts = &row.arg;
            make(
                "save_screenshot_to_folder",
                Some(shown(nth(parts, 0))),
                Some(shown(nth(parts, 1))),
            )
        }
        "ss_tl" => bare("save_screenshot_to_logs"),
        "pdftl" => bare("save_as_pdf_to_logs"),
        "spstl" => bare("save_page_source_to_logs"),
        "sh_fc" => bare("show_file_choosers"),
        "pr_da" => bare("process_deferred_asserts"),
        "a_d_m" => bare("activate_demo_mode"),
        "d_d_m" => bare("deactivate_demo_mode"),
        "c_l_s" => bare("clear_local_storage"),
        "c_s_s" => bare("clear_session_storage"),
        "d_a_c" => bare("delete_all_cookies"),
        "go_bk" => bare("go_back"),
        "go_fw" => bare("go_forward"),
        "c_box" => on(if detail == "no" {
            "uncheck_if_checked"
        } else {
            "check_if_unchecked"
        }),
        _ => return None,
    })
}

/// Python's quoting of one argument: `"x"`, or `'x'` when `x` has a `"`.
fn quoted(argument: &str) -> String {
    if argument.contains('"') {
        format!("'{argument}'")
    } else {
        format!("\"{argument}\"")
    }
}

/// The quoting of a selector, or an argument that has both kinds of quote:
/// `"x"`, `'x'`, and `'x'` with its `'` escaped when it has both.
fn quoted_selector(argument: &str) -> String {
    if !argument.contains('"') {
        format!("\"{argument}\"")
    } else if !argument.contains('\'') {
        format!("'{argument}'")
    } else {
        format!("'{}'", argument.replace('\'', "\\'"))
    }
}

/// A typed text as a step argument: a line break is written `\n`, and the
/// placeholder of a password is `<SB_RECORDED_SECRET>`.
fn step_text(value: &str) -> String {
    value
        .replace(SECRET_PLACEHOLDER, "<SB_RECORDED_SECRET>")
        .replace('\n', "\\n")
}

/// The Gherkin step (without its keyword) in Python's `behave_helper`
/// wording, or why there is none.
fn gherkin_step(action: &RecordedAction) -> Result<String, String> {
    let name = super::calls::canonical_name(&action.name);
    let name = if name == "open" { "goto" } else { name };
    let target = action.target.as_deref();
    let value = action.value.as_deref();
    let need_target = || target.ok_or_else(|| format!("{name} has no target"));
    let need_value = || value.ok_or_else(|| format!("{name} has no value"));
    let one = |wording: &str| -> Result<String, String> {
        Ok(format!("{wording} {}", quoted(need_target()?)))
    };
    let element = |wording: &str| -> Result<String, String> {
        Ok(format!("{wording} {}", quoted_selector(need_target()?)))
    };
    let typing = |wording: &str, joint: &str| -> Result<String, String> {
        let text = step_text(need_value()?);
        Ok(format!(
            "{wording} {} {joint} {}",
            quoted(&text),
            quoted(need_target()?)
        ))
    };
    let two = |first: &str, joint: &str| -> Result<String, String> {
        Ok(format!(
            "{first} {} {joint} {}",
            quoted(need_target()?),
            quoted(need_value()?)
        ))
    };
    let assertion = |wording: &str| -> Result<String, String> {
        let text = quoted(&step_text(need_value()?));
        let selector = need_target()?;
        Ok(if selector == "html" {
            format!("{wording} {text}")
        } else {
            format!("{wording} {text} in {}", quoted(selector))
        })
    };
    Ok(match name {
        "goto" => one("Open")?,
        "goto_if_not_url" => one("Open if not")?,
        "click" => one("Click")?,
        "double_click" => one("Double click")?,
        "js_click" => one("JS click")?,
        "js_click_all" => one("JS click all")?,
        "jquery_click" => one("jQuery click")?,
        "jquery_click_all" => one("jQuery click all")?,
        "context_click" => one("Context click")?,
        "hover" => one("Hover")?,
        "highlight" => one("Highlight")?,
        "click_with_offset" => {
            let offset = need_value()?;
            format!("Click {} at ({offset})", quoted(need_target()?))
        }
        "type_text" => typing("Type", "into")?,
        "js_type" => typing("JS type", "in")?,
        "jquery_type" => typing("jQuery type", "in")?,
        "press_keys" => typing("Press keys", "in")?,
        "enter_mfa_code" => {
            let text = step_text(need_value()?);
            format!("Into {} do MFA {}", quoted(need_target()?), quoted(&text))
        }
        "hover_and_click" => format!(
            "Hover {} and click {}",
            quoted(need_target()?),
            quoted(need_value()?)
        ),
        "drag_and_drop" => two("Drag", "into")?,
        "select_option_by_text" => two("Find", "and select")?,
        "set_value" => format!(
            "Set value of {} to {}",
            quoted(need_target()?),
            quoted(need_value()?)
        ),
        "choose_file" => format!(
            "Into {} choose file {}",
            quoted(need_target()?),
            quoted(&need_value()?.replace('\\', "\\\\"))
        ),
        "switch_to_frame" => one("Switch to frame")?,
        "switch_to_default_content" => "Switch to default content".to_owned(),
        "switch_to_parent_frame" => "Switch to parent frame".to_owned(),
        "set_content_to_frame" => one("Set content to frame")?,
        "set_content_to_default" => "Set content to default".to_owned(),
        "set_content_to_parent" => "Set content to parent".to_owned(),
        "sleep" => format!("Sleep for {} seconds", need_value()?),
        "wait_for_element" => element("Wait for element")?,
        "assert_element" => element("Assert element")?,
        "assert_element_present" => element("Assert element present")?,
        "assert_element_not_visible" => element("Assert element not visible")?,
        "assert_non_empty_text" => element("Assert non-empty text in")?,
        "deferred_assert_element" => element("Deferred assert element")?,
        "deferred_assert_element_present" => element("Deferred assert element present")?,
        "deferred_assert_non_empty_text" => element("Deferred assert non-empty text in")?,
        "set_attribute" | "set_attributes" => {
            let start = if name == "set_attributes" {
                "Find all"
            } else {
                "Find"
            };
            let pair = need_value()?;
            let (attribute, content) = pair
                .split_once('=')
                .ok_or_else(|| format!("{name}: value {pair:?} is not name=content"))?;
            format!(
                "{start} {} and set {attribute} to \"{content}\"",
                quoted_selector(need_target()?)
            )
        }
        "accept_alert" => "Accept alert".to_owned(),
        "dismiss_alert" => "Dismiss alert".to_owned(),
        "assert_link_text" => one("Assert link text")?,
        "assert_title" => one("Assert title")?,
        "assert_title_contains" => one("Assert title contains")?,
        "assert_url" => one("Assert URL")?,
        "assert_url_contains" => one("Assert URL contains")?,
        "assert_downloaded_file" => one("Assert downloaded file")?,
        "download_file" => match value {
            None => format!("Download file \"{}\" to downloads", need_target()?),
            Some(destination) => {
                format!("Download file \"{}\" to \"{destination}\"", need_target()?)
            }
        },
        "assert_attribute" => {
            let pair = need_value()?;
            let (attribute, content) = pair.split_once('=').unwrap_or((pair, ""));
            let selector = quoted(need_target()?);
            if content.is_empty() {
                format!("In {selector} assert attribute \"{attribute}\"")
            } else {
                format!("In {selector} assert attribute/value \"{attribute}\"/\"{content}\"")
            }
        }
        "assert_text" => assertion("Assert text")?,
        "assert_exact_text" => assertion("Assert exact text")?,
        "assert_text_not_visible" => assertion("Assert text not visible")?,
        "assert_exact_text_not_visible" => assertion("Assert exact text not visible")?,
        "deferred_assert_text" => assertion("Deferred assert text")?,
        "deferred_assert_exact_text" => assertion("Deferred assert exact text")?,
        "save_screenshot" | "save_screenshot_to_folder" => match value {
            None => one("Save screenshot as")?,
            Some(folder) => format!(
                "Save screenshot to {} as {}",
                quoted(folder),
                quoted(need_target()?)
            ),
        },
        "save_screenshot_to_logs" => "Save screenshot to logs".to_owned(),
        "save_as_pdf_to_logs" => "Save as PDF to logs".to_owned(),
        "save_page_source_to_logs" => "Save page source to logs".to_owned(),
        "show_file_choosers" => "Show file choosers".to_owned(),
        "process_deferred_asserts" => "Process deferred asserts".to_owned(),
        "activate_demo_mode" => "Activate Demo Mode".to_owned(),
        "deactivate_demo_mode" => "Deactivate Demo Mode".to_owned(),
        "clear_local_storage" => "Clear Local Storage".to_owned(),
        "clear_session_storage" => "Clear Session Storage".to_owned(),
        "delete_all_cookies" => "Delete all cookies".to_owned(),
        "go_back" => "Go back".to_owned(),
        "go_forward" => "Go forward".to_owned(),
        "check_if_unchecked" => one("Check if unchecked")?,
        "uncheck_if_checked" => one("Uncheck if checked")?,
        // The name is whatever was recorded; a line break in it would end the
        // `# skipped:` comment and start a step, so it is escaped.
        other => {
            return Err(format!(
                "{}: no Gherkin step for this action",
                other.escape_debug()
            ))
        }
    })
}

/// A Gherkin feature of one scenario in the wording of Python's
/// `--rec-behave` (`behave/behave_helper.py`): `Feature:`, `Scenario:`, `Given`
/// for the first step and `And` for the rest. With `uc` the first step is
/// `Activate CDP Mode`.
///
/// A step with no wording becomes a `# skipped:` comment that says why.
///
/// The names and text of `feature` and `scenario` are written as they are,
/// with line breaks turned into spaces.
pub(super) fn gherkin_scenario(
    actions: &[RecordedAction],
    feature: &str,
    scenario: &str,
    uc: bool,
) -> String {
    let flat = |name: &str| -> String {
        name.chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect()
    };
    let mut out = format!(
        "Feature: {}\n\n  Scenario: {}\n",
        flat(feature),
        flat(scenario)
    );
    let mut keyword = "Given";
    let mut step = |out: &mut String, step: &str| {
        out.push_str(&format!("    {keyword} {step}\n"));
        keyword = "And";
    };
    if uc && !actions.is_empty() {
        step(&mut out, "Activate CDP Mode");
    }
    for action in actions {
        match gherkin_step(action) {
            Ok(text) => step(&mut out, &text),
            Err(reason) => out.push_str(&format!("    # skipped: {reason}\n")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn raw(kind: &str, arg: Value, detail: Value) -> RawAction {
        RawAction::new(kind, arg, detail, 1)
    }

    fn one(row: RawAction) -> Option<(String, Option<String>, Option<String>)> {
        action(&row, false).map(|a| (a.name, a.target, a.value))
    }

    fn expect(row: RawAction, name: &str, target: Option<&str>, value: Option<&str>) {
        assert_eq!(
            one(row),
            Some((
                name.to_owned(),
                target.map(ToOwned::to_owned),
                value.map(ToOwned::to_owned)
            ))
        );
    }

    #[test]
    fn opens_become_goto_and_goto_if_not_url_with_the_url_decoded() {
        expect(
            raw("begin", json!("o"), json!("https://a.test/a%20b?q=%C3%A9")),
            "goto",
            Some("https://a.test/a b?q=é"),
            None,
        );
        expect(
            raw("f_url", json!("o"), json!("https://a.test/x")),
            "goto_if_not_url",
            Some("https://a.test/x"),
            None,
        );
        // An escape that is not UTF-8 leaves the URL as it was.
        expect(
            raw("_url_", json!("o"), json!("https://a.test/%FF")),
            "goto",
            Some("https://a.test/%FF"),
            None,
        );
        // A stray percent sign is kept.
        assert_eq!(unquoted("100%"), "100%");
        assert_eq!(unquoted("%zz%41"), "%zzA");
    }

    #[test]
    fn selectors_clicks_and_typing() {
        expect(
            raw("click", json!("#go"), json!("")),
            "click",
            Some("#go"),
            None,
        );
        expect(
            raw("dbclk", json!("#go"), json!("")),
            "double_click",
            Some("#go"),
            None,
        );
        expect(
            raw("js_cl", json!("#go"), json!(["u", "o"])),
            "js_click",
            Some("#go"),
            None,
        );
        expect(
            raw("canva", json!(["#c", 3, 7]), json!("")),
            "click_with_offset",
            Some("#c"),
            Some("3, 7"),
        );
        expect(
            raw("input", json!("#q"), json!("hi\n")),
            "type_text",
            Some("#q"),
            Some("hi\n"),
        );
        expect(
            raw("pkeys", json!("#q"), json!("hi")),
            "press_keys",
            Some("#q"),
            Some("hi"),
        );
        expect(
            raw("c_box", json!("#b"), json!("yes")),
            "check_if_unchecked",
            Some("#b"),
            None,
        );
        expect(
            raw("c_box", json!("#b"), json!("no")),
            "uncheck_if_checked",
            Some("#b"),
            None,
        );
        expect(
            raw("s_opt", json!("#s"), json!("Two")),
            "select_option_by_text",
            Some("#s"),
            Some("Two"),
        );
    }

    #[test]
    fn uc_types_with_press_keys() {
        let rows = [raw("input", json!("#q"), json!("hi"))];
        assert_eq!(to_actions(&rows, true)[0].name, "press_keys");
        assert_eq!(to_actions(&rows, false)[0].name, "type_text");
    }

    #[test]
    fn a_typed_password_is_the_placeholder() {
        let row = raw("input", json!("#pw"), json!("hunter2\n")).secret();
        let actions = to_actions(&[row], false);
        assert_eq!(
            actions[0].value.as_deref(),
            Some("{{SB_RECORDED_SECRET}}\n")
        );
        let row = raw("input", json!("#pw"), json!("hunter2")).secret();
        assert_eq!(
            to_actions(&[row], false)[0].value.as_deref(),
            Some(SECRET_PLACEHOLDER)
        );
    }

    #[test]
    fn kinds_without_a_call_make_none() {
        for kind in [
            "_skip", "sk_op", "o_url", "mo_dn", "drags", "submi", "chfil", "ch_cl",
        ] {
            assert!(one(raw(kind, json!(""), json!(""))).is_none(), "{kind}");
        }
    }

    #[test]
    fn assertions_carry_the_selector_as_target_and_the_text_as_value() {
        expect(
            raw("as_el", json!("#m"), json!("o")),
            "assert_element",
            Some("#m"),
            None,
        );
        expect(
            raw(
                "as_te",
                json!(["Hi\u{00A0}there\u{00B6}", "h1"]),
                json!("o"),
            ),
            "assert_text",
            Some("h1"),
            Some("Hi there"),
        );
        expect(
            raw("as_et", json!(["Ｈｉ", "html"]), json!("o")),
            "assert_exact_text",
            Some("html"),
            Some("Hi"),
        );
        expect(
            raw("as_at", json!(["#a", "href", "/x"]), json!("o")),
            "assert_attribute",
            Some("#a"),
            Some("href=/x"),
        );
        expect(
            raw("s_at_", json!(["#a", "title", "t"]), json!("o")),
            "set_attribute",
            Some("#a"),
            Some("title=t"),
        );
        expect(
            raw("do_fi", json!(["https://a.test/f.zip", ""]), json!("o")),
            "download_file",
            Some("https://a.test/f.zip"),
            None,
        );
    }

    #[test]
    fn frames_and_the_nested_content_switch() {
        expect(
            raw("s_c_d", json!(true), json!("o")),
            "set_content_to_parent",
            None,
            None,
        );
        expect(
            raw("s_c_d", json!(false), json!("o")),
            "set_content_to_default",
            None,
            None,
        );
        expect(
            raw("sw_fr", json!("iframe"), json!("o")),
            "switch_to_frame",
            Some("iframe"),
            None,
        );
        expect(
            raw("sleep", json!(1.5), json!("o")),
            "sleep",
            None,
            Some("1.5"),
        );
    }

    #[test]
    fn the_gherkin_wording_is_pythons() {
        let actions = to_actions(
            &[
                raw("begin", json!("o"), json!("https://a.test/")),
                raw("f_url", json!("o"), json!("https://a.test/x")),
                raw("click", json!("a[href=\"/x\"]"), json!("")),
                raw("input", json!("#q"), json!("rust\n")),
                raw("as_te", json!(["Demo", "h1"]), json!("o")),
                raw("as_et", json!(["Demo", "html"]), json!("o")),
                raw("as_el", json!("#m"), json!("o")),
                raw("c_box", json!("#b"), json!("no")),
                raw("h_clk", json!(".menu"), json!(".item")),
            ],
            false,
        );
        let feature = gherkin_scenario(&actions, "RecorderTest", "test_recording", false);
        assert_eq!(
            feature,
            "Feature: RecorderTest\n\n  Scenario: test_recording\n    \
             Given Open \"https://a.test/\"\n    \
             And Open if not \"https://a.test/x\"\n    \
             And Click 'a[href=\"/x\"]'\n    \
             And Type \"rust\\n\" into \"#q\"\n    \
             And Assert text \"Demo\" in \"h1\"\n    \
             And Assert exact text \"Demo\"\n    \
             And Assert element \"#m\"\n    \
             And Uncheck if checked \"#b\"\n    \
             And Hover \".menu\" and click \".item\"\n"
        );
        let uc = gherkin_scenario(&actions[..1], "F", "s", true);
        assert!(uc.contains("Given Activate CDP Mode\n    And Open"), "{uc}");
    }

    #[test]
    fn a_password_and_an_unknown_action_in_gherkin() {
        let mut actions = to_actions(&[raw("input", json!("#pw"), json!("x")).secret()], false);
        actions.push(RecordedAction::new("levitate", Some("#a"), None));
        let feature = gherkin_scenario(&actions, "F", "s", false);
        assert!(
            feature.contains("Given Type \"<SB_RECORDED_SECRET>\" into \"#pw\""),
            "{feature}"
        );
        assert!(
            feature.contains("# skipped: levitate: no Gherkin step"),
            "{feature}"
        );
    }
}
