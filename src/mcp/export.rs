//! Turning the tool calls of a session into a Rust test.
//!
//! Each successful call that changed the page, waited for it or checked it
//! becomes a [`RecordedAction`] for [`ActionRecorder`], which writes the test.
//! A call that changed the page but has no `BaseCase` equivalent becomes a
//! `// skipped:` comment in its place, so a gap is never silent. Calls that
//! only read the page, and the browser's own start and close, are left out.

use serde_json::{Map, Value};

use super::journal::CallRecord;
use super::tool::Effect;
use crate::api::recorder::{ActionRecorder, RecordedAction};

/// What one call becomes in the exported test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Replay {
    /// Calls on `BaseCase`, as `(action, target, value)`.
    Steps(Vec<(&'static str, Option<String>, Option<String>)>),
    /// A change to the page the test cannot repeat, and why.
    Skip(String),
    /// Nothing: the call only read the page, or the test scaffold covers it.
    Leave,
}

/// A test made from a session.
#[derive(Debug, Clone)]
pub(crate) struct Export {
    /// The recording the test is written from.
    pub(crate) recorder: ActionRecorder,
    /// How many calls became steps.
    pub(crate) steps: usize,
    /// The calls that became `// skipped:` comments, with the reason.
    pub(crate) skipped: Vec<(String, String)>,
}

/// The calls of the current browser session: those after the last successful
/// `start_browser`, or all of them if there was none.
pub(crate) fn current_session(calls: &[CallRecord]) -> &[CallRecord] {
    let start = calls
        .iter()
        .rposition(|call| call.ok && call.tool == "start_browser")
        .map_or(0, |index| index + 1);
    &calls[start..]
}

/// Records every successful call of `calls` that the test should repeat.
pub(crate) fn export(calls: &[CallRecord]) -> Export {
    let mut export = Export {
        recorder: ActionRecorder::default(),
        steps: 0,
        skipped: Vec::new(),
    };
    for call in calls.iter().filter(|call| call.ok) {
        match replay(&call.tool, &call.arguments, call.effect) {
            Replay::Steps(steps) => {
                for (action, target, value) in steps {
                    export
                        .recorder
                        .record(action, target.as_deref(), value.as_deref());
                    export.steps += 1;
                }
            }
            Replay::Skip(reason) => {
                // An action name the generator does not know becomes a
                // `// skipped:` comment that quotes the name, so the reason
                // travels in it.
                let note = format!("{}: {reason}", call.tool);
                export
                    .recorder
                    .actions
                    .push(RecordedAction::new(&note, None, None));
                export.skipped.push((call.tool.clone(), reason));
            }
            Replay::Leave => {}
        }
    }
    export
}

/// A string argument, if given and not empty.
fn text(arguments: &Map<String, Value>, name: &str) -> Option<String> {
    match arguments.get(name) {
        Some(Value::String(text)) if !text.is_empty() => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

fn flag(arguments: &Map<String, Value>, name: &str) -> bool {
    arguments
        .get(name)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn one(action: &'static str, target: Option<String>, value: Option<String>) -> Replay {
    Replay::Steps(vec![(action, target, value)])
}

fn skip(reason: &str) -> Replay {
    Replay::Skip(reason.to_owned())
}

/// What a successful call of `tool` with `arguments` becomes in a test.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per tool reads best as one table"
)]
pub(crate) fn replay(tool: &str, arguments: &Map<String, Value>, effect: Effect) -> Replay {
    let arg = |name: &str| text(arguments, name);
    let selector = arg("selector");
    let html = || Some(arg("selector").unwrap_or_else(|| "html".to_owned()));
    match tool {
        "open_url" => one("open", arg("url"), None),
        "click_element" => {
            if arguments.contains_key("nth") || flag(arguments, "all_matches") {
                skip("clicking one match of several by position has no recorded equivalent")
            } else if flag(arguments, "only_if_visible") {
                skip("a click that may be skipped has no recorded equivalent")
            } else if arg("parent_selector").is_some() {
                skip("a click inside a container has no recorded equivalent")
            } else {
                one("click", selector, None)
            }
        }
        "click_visible_elements" => one("click_visible_elements", selector, None),
        "click_link" => one("click_link_text", arg("link_text"), None),
        "double_click" => one("double_click", selector, None),
        "context_click" => one("context_click", selector, None),
        "type_text" => match arg("mode").as_deref() {
            Some("append") => one("add_text", selector, arg("text")),
            Some("clear_only") => one("clear", selector, None),
            Some("set_value") => {
                skip("setting a value without key events has no recorded equivalent")
            }
            _ if arguments.get("clear_first") == Some(&Value::Bool(false)) => {
                one("send_keys", selector, arg("text"))
            }
            _ => one("type_text", selector, arg("text")),
        },
        "clear_input" => one("clear", selector, None),
        "submit" => one("submit", selector, None),
        "choose_file" => one("choose_file", selector, arg("file_path")),
        "select_option" => {
            let action = match arg("by").as_deref() {
                Some("value") => "select_option_by_value",
                Some("index") => "select_option_by_index",
                _ => "select_option_by_text",
            };
            one(action, arg("dropdown_selector"), arg("value"))
        }
        "select_option_by_text" | "select_option_by_value" | "select_option_by_index" => {
            let action = match tool {
                "select_option_by_value" => "select_option_by_value",
                "select_option_by_index" => "select_option_by_index",
                _ => "select_option_by_text",
            };
            one(action, arg("dropdown_selector"), arg("option"))
        }
        "hover_action" => match arg("action").as_deref() {
            Some("hover_and_click") => Replay::Steps(vec![
                ("hover", selector, None),
                ("click", arg("secondary_selector"), None),
            ]),
            Some("drag_and_drop") => one("drag_and_drop", selector, arg("secondary_selector")),
            _ => one("hover", selector, None),
        },
        "hover_and_click" => Replay::Steps(vec![
            ("hover", arg("hover_selector"), None),
            ("click", arg("click_selector"), None),
        ]),
        "drag_and_drop" => one("drag_and_drop", arg("drag_selector"), arg("drop_selector")),
        "manage_history" => match arg("action").as_deref() {
            Some("back") => one("go_back", None, None),
            Some("forward") => one("go_forward", None, None),
            Some("reload") => one("refresh", None, None),
            _ => Replay::Leave,
        },
        "go_back" => one("go_back", None, None),
        "go_forward" => one("go_forward", None, None),
        "refresh_page" => one("refresh", None, None),
        "focus_element" => match arg("action").as_deref() {
            Some("focus") => skip("giving focus has no recorded equivalent"),
            Some("highlight") => Replay::Leave,
            _ => one("scroll_to", selector, None),
        },
        "scroll_into_view" => one("scroll_into_view", selector, None),
        "scroll_to_top" => one("scroll_to_top", None, None),
        "scroll_to_bottom" => one("scroll_to_bottom", None, None),
        "scroll_page" => match arg("direction").as_deref() {
            Some("top") => one("scroll_to_top", None, None),
            Some("bottom") => one("scroll_to_bottom", None, None),
            _ => skip("scrolling by a share of the window has no recorded equivalent"),
        },
        "wait_for_condition" => wait_for_condition(arguments),
        "wait_for_element" | "assert_element" => one("wait_for_element_visible", selector, None),
        "wait_for_element_present" | "assert_element_present" => {
            one("wait_for_element_present", selector, None)
        }
        "wait_for_element_not_visible" | "assert_element_not_visible" => {
            one("wait_for_element_not_visible", selector, None)
        }
        "wait_for_element_absent" => one("wait_for_element_absent", selector, None),
        "wait_for_text" | "assert_text" => one("assert_text", html(), arg("text")),
        "assert_exact_text" => one("assert_exact_text", html(), arg("text")),
        "assert_title" => one("assert_title", arg("title"), None),
        "assert_condition" => assert_condition(arguments),
        "manage_cookies" => match arg("action").as_deref() {
            Some("clear") => one("delete_all_cookies", None, None),
            Some("load") => skip("loading saved cookies needs the cookie file next to the test"),
            _ => Replay::Leave,
        },
        "delete_all_cookies" => one("delete_all_cookies", None, None),
        "manage_storage" => match (arg("action").as_deref(), arg("storage").as_deref()) {
            (Some("set"), Some("session")) => {
                skip("setting sessionStorage has no recorded equivalent")
            }
            (Some("set"), _) => one("set_local_storage_item", arg("key"), arg("value")),
            _ => Replay::Leave,
        },
        "set_local_storage_item" => one("set_local_storage_item", arg("key"), arg("value")),
        "manage_window" => match arg("action").as_deref() {
            Some("maximize") => one("maximize_window", None, None),
            Some("set_rect") => match (arg("width"), arg("height")) {
                (Some(width), Some(height)) => {
                    one("set_window_size", Some(format!("{width},{height}")), None)
                }
                _ => skip("the window size was not given"),
            },
            Some("minimize") => skip("minimising the window has no recorded equivalent"),
            _ => Replay::Leave,
        },
        "maximize_window" => one("maximize_window", None, None),
        "manage_tabs" => match arg("action").as_deref() {
            Some("open_new_tab") => one("open_new_tab", arg("url"), None),
            Some("switch_to_newest_tab") => one("switch_to_newest_window", None, None),
            Some("switch_to_tab" | "close_active_tab") => {
                skip("switching or closing tabs by number has no recorded equivalent")
            }
            _ => Replay::Leave,
        },
        "open_new_tab" => one("open_new_tab", arg("url"), None),
        "switch_to_newest_tab" => one("switch_to_newest_window", None, None),
        "switch_to_default_window" => one("switch_to_default_window", None, None),
        "switch_to_frame" => one("switch_to_frame", selector, None),
        "switch_to_default_content" => one("switch_to_default_content", None, None),
        "run_javascript" | "execute_script" | "evaluate" => {
            skip("scripts are not replayed; add the call by hand if the test needs it")
        }
        "sleep" => skip("a fixed pause; wait for a condition instead"),
        "enter_mfa_code" => skip("a one-time code cannot be replayed"),
        "start_browser" | "close_browser" | "highlight" | "flash" => Replay::Leave,
        _ if matches!(effect, Effect::Act | Effect::Set | Effect::Mixed) => {
            Replay::Skip(format!("{tool} has no recorded equivalent"))
        }
        _ => Replay::Leave,
    }
}

/// `wait_for_condition` of the Pure CDP server.
fn wait_for_condition(arguments: &Map<String, Value>) -> Replay {
    let arg = |name: &str| text(arguments, name);
    let state = arg("state").unwrap_or_else(|| "visible".to_owned());
    if let Some(wanted) = arg("text") {
        let within = Some(arg("selector").unwrap_or_else(|| "html".to_owned()));
        return match state.as_str() {
            "present" | "visible" => one("assert_text", within, Some(wanted)),
            "not_visible" | "absent" => one("assert_text_not_visible", within, Some(wanted)),
            _ => skip("a fixed pause; wait for a condition instead"),
        };
    }
    let action = match state.as_str() {
        "present" => "wait_for_element_present",
        "not_visible" => "wait_for_element_not_visible",
        "absent" => "wait_for_element_absent",
        "seconds_passed" => return skip("a fixed pause; wait for a condition instead"),
        _ => "wait_for_element_visible",
    };
    one(action, arg("selector"), None)
}

/// `assert_condition` of the Pure CDP server.
fn assert_condition(arguments: &Map<String, Value>) -> Replay {
    let arg = |name: &str| text(arguments, name);
    let within = || Some(arg("selector").unwrap_or_else(|| "html".to_owned()));
    match arg("check").as_deref() {
        Some("element_present") => one("wait_for_element_present", arg("selector"), None),
        Some("text_visible") if flag(arguments, "exact") => {
            one("assert_exact_text", within(), arg("expected"))
        }
        Some("text_visible") => one("assert_text", within(), arg("expected")),
        Some("title") => one("assert_title", arg("expected"), None),
        Some("url" | "url_contains") => skip("URL checks have no recorded equivalent"),
        _ => one("wait_for_element_visible", arg("selector"), None),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A logged call, with the effect the servers declare for it: readers
    /// observe, everything else acts. Takes the `json!` temporary by value.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers pass `json!` temporaries"
    )]
    fn call(tool: &str, arguments: Value, ok: bool) -> CallRecord {
        let effect = if tool.starts_with("get_") {
            Effect::Observe
        } else {
            Effect::Act
        };
        CallRecord::new(tool, effect, arguments.as_object().unwrap(), ok, "")
    }

    #[test]
    fn a_session_becomes_a_test_of_its_successful_changes_and_checks() {
        let calls = [
            call("start_browser", json!({}), true),
            call("open_url", json!({ "url": "https://shop.test" }), true),
            call(
                "type_text",
                json!({ "selector": "#q", "text": "shoes" }),
                true,
            ),
            call("click_element", json!({ "selector": "#missing" }), false),
            call("click_element", json!({ "selector": "#go" }), true),
            call("get_content", json!({ "selector": "body" }), true),
            call(
                "assert_condition",
                json!({ "check": "text_visible", "selector": "h1", "expected": "Shoes" }),
                true,
            ),
        ];

        let export = export(current_session(&calls));
        let test = export.recorder.to_rust_test("search_works");

        assert_eq!(export.steps, 4);
        assert_eq!(export.skipped, vec![]);
        assert!(
            test.contains("sb.open(\"https://shop.test\").await?;"),
            "{test}"
        );
        assert!(test.contains("sb.type_text(\"#q\", \"shoes\").await?;"));
        assert!(test.contains("sb.click(\"#go\").await?;"));
        assert!(!test.contains("#missing"), "failed calls are not replayed");
        assert!(test.contains("sb.assert_text(\"h1\", \"Shoes\").await?;"));
    }

    #[test]
    fn a_change_with_no_equivalent_is_a_skipped_comment_in_its_place() {
        let calls = [
            call("open_url", json!({ "url": "https://a.test" }), true),
            call("run_javascript", json!({ "expression": "1" }), true),
            call("click_element", json!({ "selector": "a", "nth": 2 }), true),
        ];

        let export = export(&calls);
        let test = export.recorder.to_rust_test("t");

        assert_eq!(export.skipped.len(), 2);
        assert_eq!(export.skipped[0].0, "run_javascript");
        let skipped: Vec<&str> = test
            .lines()
            .filter(|line| line.contains("// skipped:"))
            .collect();
        assert_eq!(skipped.len(), 2, "{test}");
        assert!(skipped[0].contains("scripts are not replayed"), "{test}");
    }

    #[test]
    fn only_the_calls_after_the_last_start_belong_to_the_session() {
        let calls = [
            call("open_url", json!({ "url": "https://old.test" }), true),
            call("start_browser", json!({}), true),
            call("open_url", json!({ "url": "https://new.test" }), true),
        ];

        let session = current_session(&calls);

        assert_eq!(session.len(), 1);
        assert_eq!(session[0].arguments["url"], "https://new.test");
    }

    #[test]
    fn the_two_servers_spellings_of_a_call_give_the_same_step() {
        let cdp = replay(
            "select_option",
            json!({ "dropdown_selector": "#c", "value": "2", "by": "index" })
                .as_object()
                .unwrap(),
            Effect::Act,
        );
        let sb = replay(
            "select_option_by_index",
            json!({ "dropdown_selector": "#c", "option": 2 })
                .as_object()
                .unwrap(),
            Effect::Act,
        );
        assert_eq!(cdp, sb);
        assert_eq!(
            replay(
                "wait_for_condition",
                json!({ "state": "seconds_passed", "timeout": 3 })
                    .as_object()
                    .unwrap(),
                Effect::Inspect,
            ),
            Replay::Skip("a fixed pause; wait for a condition instead".to_owned())
        );
    }

    #[test]
    fn reading_tools_are_left_out_and_unknown_changes_are_skipped() {
        let read = replay("inspect_page", &Map::new(), Effect::Observe);
        let changed = replay("solve_captcha", &Map::new(), Effect::Act);
        assert_eq!(read, Replay::Leave);
        assert_eq!(
            changed,
            Replay::Skip("solve_captcha has no recorded equivalent".to_owned())
        );
    }

    #[test]
    fn hostile_text_stays_inside_its_string_literal() {
        let calls = [call(
            "type_text",
            json!({ "selector": "#q", "text": "\"); std::process::exit(1); (\"" }),
            true,
        )];
        let test = export(&calls).recorder.to_rust_test("t");
        assert!(
            test.contains(r##"sb.type_text("#q", "\"); std::process::exit(1); (\"").await?;"##),
            "{test}"
        );
    }
}
