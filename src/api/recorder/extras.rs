//! The actions the page script cannot see: what a test does through
//! [`BaseCase`](crate::BaseCase) methods that leave no trace in the page.
//!
//! Python keeps these in `__extra_actions`, stamped with the browser's
//! `Date.now()`. [`BaseCase`](crate::BaseCase) logs every call of a method
//! that records itself ([`RecordedAction`], stamped with the local clock);
//! at the end of a recording [`extra_raw_actions`] turns the entries that
//! [`extra_layout`] knows into [`RawAction`]s that [`process`](super::process)
//! merges with the ones the page recorded.
//!
//! A click or a typed text is not in the list: the page script sees those as
//! they happen. What is in the list are the calls that change no page event
//! (`hover` through the driver's action chain, a JavaScript click, an alert
//! that was accepted) or that only check something (the `assert_*` calls).

use serde_json::{json, Value};

use super::raw::text;
use super::{RawAction, RecordedAction, RecordedCommand};

/// Where the arguments of a logged call go in a raw action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Layout {
    /// `[kind, selector, origin]`.
    Selector,
    /// `[kind, "", origin]`.
    Bare,
    /// `[kind, [selector, text], origin]`.
    Typed,
    /// `[kind, selector, [url, origin]]`: a click made with a script.
    ScriptClick,
    /// `[kind, [text, selector], origin]`: a text assertion.
    TextAssert,
    /// `[kind, value, origin]`: the call's own argument (a title, a URL).
    Value,
    /// `[kind, seconds, origin]`, only when sleeps are recorded.
    Sleep,
}

/// The log names that become raw actions, with the kind Python writes and
/// where the arguments go. A call that is not here is left out of the
/// recording.
pub(crate) fn extra_layout(command: &RecordedCommand) -> Option<(&'static str, Layout)> {
    use RecordedCommand as C;
    Some(match command {
        C::Hover { .. } => ("hover", Layout::Selector),
        C::ContextClick { .. } => ("r_clk", Layout::Selector),
        C::Highlight { .. } => ("hi_li", Layout::Selector),
        C::JsClick { .. } => ("js_cl", Layout::ScriptClick),
        C::JsClickAll { .. } => ("js_ca", Layout::ScriptClick),
        C::JqueryClick { .. } => ("jq_cl", Layout::ScriptClick),
        C::JqueryClickAll { .. } => ("jq_ca", Layout::ScriptClick),
        C::JsType { .. } => ("js_ty", Layout::Typed),
        C::JqueryType { .. } => ("jq_ty", Layout::Typed),
        C::PressKeys { .. } => ("pkeys", Layout::Typed),
        C::SetValue { .. } => ("s_val", Layout::Typed),
        C::EnterMfaCode { .. } => ("e_mfa", Layout::Typed),
        C::AcceptAlert => ("acc_a", Layout::Bare),
        C::DismissAlert => ("dis_a", Layout::Bare),
        C::ShowFileChoosers => ("sh_fc", Layout::Bare),
        C::Sleep { .. } => ("sleep", Layout::Sleep),
        C::WaitForElementVisible { .. } => ("wf_el", Layout::Selector),
        C::AssertElement { .. } => ("as_el", Layout::Selector),
        C::AssertElementPresent { .. } => ("as_ep", Layout::Selector),
        C::AssertElementNotVisible { .. } => ("asenv", Layout::Selector),
        C::AssertNonEmptyText { .. } => ("asnet", Layout::Selector),
        C::AssertLinkText { .. } => ("as_lt", Layout::Value),
        C::AssertTitle { .. } => ("as_ti", Layout::Value),
        C::AssertTitleContains { .. } => ("as_tc", Layout::Value),
        C::AssertUrl { .. } => ("a_url", Layout::Value),
        C::AssertUrlContains { .. } => ("a_u_c", Layout::Value),
        C::AssertText { .. } => ("as_te", Layout::TextAssert),
        C::AssertExactText { .. } => ("as_et", Layout::TextAssert),
        C::AssertTextNotVisible { .. } => ("astnv", Layout::TextAssert),
        C::AssertExactTextNotVisible { .. } => ("aetnv", Layout::TextAssert),
        C::DeferredAssertElement { .. } => ("da_el", Layout::Selector),
        C::DeferredAssertElementPresent { .. } => ("da_ep", Layout::Selector),
        C::DeferredAssertNonEmptyText { .. } => ("danet", Layout::Selector),
        C::DeferredAssertText { .. } => ("da_te", Layout::TextAssert),
        C::DeferredAssertExactText { .. } => ("da_et", Layout::TextAssert),
        _ => return None,
    })
}

/// Whether a page at `url` is one that is recorded: Python records `http`,
/// `https` and `file` pages only.
pub(crate) fn recordable(url: &str) -> bool {
    url.contains("http:") || url.contains("https:") || url.contains("file:")
}

/// The origin of `url` (`https://a.test:8080`), as `window.location.origin`
/// gives it; `file://` for a file; empty when `url` is not an address.
pub(crate) fn origin_of(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return String::new();
    };
    if scheme == "file" {
        return "file://".to_owned();
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    format!("{scheme}://{}", &rest[..end])
}

fn is_open(kind: &str) -> bool {
    matches!(kind, "begin" | "_url_" | "f_url")
}

/// The raw actions for the entries of the call log.
///
/// * `log` is the log of [`BaseCase`](crate::BaseCase), oldest first.
/// * `page_actions` are the actions the page recorded; their opens say which
///   origin each call was made on (a call has the origin of the last page
///   that was opened before it).
/// * `clock_offset_ms` is the browser's clock minus the local one, so a log
///   entry is stamped in browser time. Entries before `start_ms` are left out.
/// * `fallback_origin` is the origin of calls made before any page opened.
///
/// `open` makes two actions, as Python's does: `o_url` a millisecond before
/// `_url_`. `sleep` is kept only with `record_sleep`.
pub(crate) fn extra_raw_actions(
    log: &[RecordedAction],
    page_actions: &[RawAction],
    clock_offset_ms: i64,
    start_ms: i64,
    record_sleep: bool,
    fallback_origin: &str,
) -> Vec<RawAction> {
    let mut opens: Vec<Open> = page_actions
        .iter()
        .filter(|action| is_open(&action.kind))
        .map(|action| Open {
            time: action.time_ms,
            origin: text(&action.arg).to_owned(),
            url: text(&action.detail).to_owned(),
        })
        .collect();
    let mut out = Vec::new();
    for entry in log {
        let time = i64::try_from(entry.timestamp_ms)
            .unwrap_or(i64::MAX)
            .saturating_add(clock_offset_ms);
        if time < start_ms {
            continue;
        }
        let target = entry.command.target().unwrap_or_default();
        let value = entry.command.value().unwrap_or_default();
        if matches!(entry.command, RecordedCommand::Open { .. }) {
            let origin = origin_of(&target);
            out.push(RawAction::new(
                "o_url",
                json!(origin),
                json!(target),
                time - 1,
            ));
            out.push(RawAction::new("_url_", json!(origin), json!(target), time));
            opens.push(Open {
                time,
                origin,
                url: target.clone(),
            });
            continue;
        }
        let Some((kind, layout)) = extra_layout(&entry.command) else {
            continue;
        };
        if layout == Layout::Sleep && !record_sleep {
            continue;
        }
        let (origin, url) = origin_at(&opens, time, fallback_origin);
        let (arg, detail): (Value, Value) = match layout {
            Layout::Selector => (json!(target), json!(origin)),
            Layout::Bare => (json!(""), json!(origin)),
            Layout::Typed => (json!([target, value]), json!(origin)),
            Layout::ScriptClick => (json!(target), json!([url, origin])),
            Layout::TextAssert => (json!([value, target]), json!(origin)),
            Layout::Value => (json!(target), json!(origin)),
            Layout::Sleep => (
                value
                    .parse::<f64>()
                    .map_or_else(|_| json!(value), |seconds| json!(seconds)),
                json!(origin),
            ),
        };
        out.push(RawAction::new(kind, arg, detail, time));
    }
    out
}

/// The origin of the page that was open at `time`: that of the last open at
/// or before it, else of the first open, else `fallback`.
fn origin_at(opens: &[Open], time: i64, fallback: &str) -> (String, String) {
    opens
        .iter()
        .filter(|open| open.time <= time)
        .max_by_key(|open| open.time)
        .or_else(|| opens.iter().min_by_key(|open| open.time))
        .map_or_else(
            || (fallback.to_owned(), String::new()),
            |open| (open.origin.clone(), open.url.clone()),
        )
}

/// A page that was opened: when, on which origin, at which address.
struct Open {
    time: i64,
    origin: String,
    url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, target: Option<&str>, value: Option<&str>, at: u128) -> RecordedAction {
        RecordedAction {
            command: RecordedCommand::from_parts(name, target, value),
            timestamp_ms: at,
        }
    }

    fn begin(origin: &str, at: i64) -> RawAction {
        RawAction::new("begin", json!(origin), json!(format!("{origin}/")), at)
    }

    #[test]
    fn origins_and_recordable_pages() {
        assert_eq!(
            origin_of("https://a.test:8080/x?y#z"),
            "https://a.test:8080"
        );
        assert_eq!(origin_of("http://a.test"), "http://a.test");
        assert_eq!(origin_of("file:///tmp/p.html"), "file://");
        assert_eq!(origin_of("about:blank"), "");
        assert!(recordable("https://a.test/") && recordable("file:///x") && recordable("http://a"));
        assert!(!recordable("about:blank") && !recordable("data:text/html,x"));
    }

    #[test]
    fn an_open_is_o_url_a_millisecond_before_url() {
        let log = [entry("open", Some("https://a.test/p"), None, 1_000)];
        let rows = extra_raw_actions(&log, &[], 0, 0, false, "");
        let shown: Vec<_> = rows
            .iter()
            .map(|row| {
                (
                    row.kind.as_str(),
                    row.time_ms,
                    row.arg.clone(),
                    row.detail.clone(),
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                (
                    "o_url",
                    999,
                    json!("https://a.test"),
                    json!("https://a.test/p")
                ),
                (
                    "_url_",
                    1_000,
                    json!("https://a.test"),
                    json!("https://a.test/p")
                ),
            ]
        );
    }

    #[test]
    fn every_layout_puts_the_arguments_where_the_generators_read_them() {
        let page = [begin("https://a.test", 10)];
        let log = [
            entry("hover", Some("#h"), None, 100),
            entry("js_click", Some("#j"), None, 101),
            entry("jquery_type", Some("#q"), Some("abc"), 102),
            entry("accept_alert", None, None, 103),
            entry("assert_text", Some("h1"), Some("Hello"), 104),
            entry("assert_title", Some("Home"), None, 105),
            entry("sleep", None, Some("1.5"), 106),
        ];
        let rows = extra_raw_actions(&log, &page, 0, 0, true, "");
        let by_kind = |kind: &str| rows.iter().find(|row| row.kind == kind).unwrap();
        let origin = json!("https://a.test");
        assert_eq!(by_kind("hover").arg, json!("#h"));
        assert_eq!(by_kind("hover").detail, origin);
        assert_eq!(
            by_kind("js_cl").detail,
            json!(["https://a.test/", "https://a.test"])
        );
        assert_eq!(by_kind("jq_ty").arg, json!(["#q", "abc"]));
        assert_eq!(by_kind("acc_a").arg, json!(""));
        assert_eq!(by_kind("as_te").arg, json!(["Hello", "h1"]));
        assert_eq!(by_kind("as_ti").arg, json!("Home"));
        assert_eq!(by_kind("sleep").arg, json!(1.5));
    }

    #[test]
    fn they_are_the_raw_actions_that_python_processes_into_calls() {
        let page = [begin("https://a.test", 10)];
        let log = [
            entry("hover", Some("#h"), None, 100),
            entry("assert_text", Some("h1"), Some("Hello"), 104),
            entry("assert_url_contains", Some("/p"), None, 105),
        ];
        let rows = extra_raw_actions(&log, &page, 0, 0, false, "");
        let mut all = page.to_vec();
        all.extend(rows);
        let calls = super::super::to_actions(
            &super::super::process(all, 0, &[]),
            crate::DriverMode::WebDriver,
        );
        let names: Vec<_> = calls.iter().map(|call| call.command.name()).collect();
        assert_eq!(
            names,
            ["goto", "hover", "assert_text", "assert_url_contains"]
        );
        assert_eq!(calls[2].command.target().as_deref(), Some("h1"));
        assert_eq!(calls[2].command.value().as_deref(), Some("Hello"));
    }

    #[test]
    fn sleeps_clicks_and_old_entries_are_left_out() {
        let page = [begin("https://a.test", 10)];
        let log = [
            entry("sleep", None, Some("2"), 500),
            entry("click", Some("#a"), None, 501),
            entry("hover", Some("#old"), None, 5),
            entry("hover", Some("#new"), None, 600),
        ];
        let rows = extra_raw_actions(&log, &page, 0, 100, false, "");
        let kinds: Vec<_> = rows
            .iter()
            .map(|row| (&row.kind[..], row.arg.clone()))
            .collect();
        assert_eq!(kinds, [("hover", json!("#new"))]);
        let with_sleep = extra_raw_actions(&log, &page, 0, 100, true, "");
        assert!(with_sleep.iter().any(|row| row.kind == "sleep"));
    }

    #[test]
    fn a_call_has_the_origin_of_the_page_open_at_its_time_and_the_browser_clock() {
        let page = [begin("https://a.test", 10), begin("https://b.test", 300)];
        // The browser's clock is 1000 ms ahead of the local one.
        let log = [
            entry("hover", Some("#a"), None, 100),
            entry("hover", Some("#b"), None, 400),
        ];
        let rows = extra_raw_actions(&log, &page, 1_000, 0, false, "https://fallback.test");
        assert_eq!(rows[0].time_ms, 1_100);
        // Both calls come after both opens in browser time.
        assert_eq!(rows[0].detail, json!("https://b.test"));
        let early = extra_raw_actions(&log, &page, 0, 0, false, "https://fallback.test");
        assert_eq!(early[0].detail, json!("https://a.test"));
        assert_eq!(early[1].detail, json!("https://b.test"));
        let none = extra_raw_actions(&log, &[], 0, 0, false, "https://fallback.test");
        assert_eq!(none[0].detail, json!("https://fallback.test"));
    }
}
