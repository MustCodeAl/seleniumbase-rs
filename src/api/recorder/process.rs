//! Python's `__process_recorded_actions`, without the I/O.
//!
//! Recorder Mode's page script writes what the browser saw; this module turns
//! that stream into the actions a person meant. It is a port of the
//! rewrite passes in SeleniumBase 4.56.2 (`fixtures/base_case.py`), in
//! Python's order and with Python's thresholds in milliseconds (16, 42, 440,
//! 900, 4200, 4800, 5200, 6500). The heuristics are kept as they are, quirks
//! included (a few compare the wrong URL, as noted where they do), so that
//! a recording means the same in both implementations; the golden tests in
//! `tests/recorder_pipeline.rs` compare the result with the Python code's.
//!
//! A pass changes the `kind` of an action to `_skip` (or to another kind) in
//! place and never removes one: [`to_actions`](super::to_actions) drops the
//! kinds that make no call.

use std::collections::BTreeMap;

use serde_json::Value;

use super::raw::{text, RawAction};

static NULL: Value = Value::Null;

/// Element `index` of a JSON array, or null.
fn nth(value: &Value, index: usize) -> &Value {
    value.get(index).unwrap_or(&NULL)
}

/// The kinds a page click, a JavaScript click and a jQuery click write.
const CLICKS: [&str; 5] = ["click", "js_cl", "js_ca", "jq_cl", "jq_ca"];
/// The kinds whose second argument is `[url, origin]`.
const SCRIPT_CLICKS: [&str; 4] = ["js_cl", "js_ca", "jq_cl", "jq_ca"];
/// The kinds that switch or set the frame.
const FRAMES: [&str; 5] = ["sw_fr", "sw_dc", "sw_pf", "s_c_d", "s_c_f"];

fn is_open(kind: &str) -> bool {
    kind == "begin" || kind == "_url_"
}

fn is_click(kind: &str) -> bool {
    CLICKS.contains(&kind)
}

fn is_frame(kind: &str) -> bool {
    FRAMES.contains(&kind)
}

/// Python's `s[:-n]`.
fn without_last(text: &str, n: usize) -> String {
    let count = text.chars().count();
    text.chars().take(count.saturating_sub(n)).collect()
}

/// Python's `url[:-3] if url.endswith("/#/") else url[:-1] if url.endswith("/")`.
fn trimmed(url: &str) -> String {
    if url.ends_with("/#/") {
        without_last(url, 3)
    } else if url.ends_with('/') {
        without_last(url, 1)
    } else {
        url.to_owned()
    }
}

/// The same trim for the second URL, where Python writes `url1[:-3]` for the
/// `/#/` case (a slip, kept: the heuristics were tuned around it).
fn trimmed_second(first: &str, url: &str) -> String {
    if url.ends_with("/#/") {
        without_last(first, 3)
    } else if url.ends_with('/') {
        without_last(url, 1)
    } else {
        url.to_owned()
    }
}

/// Milliseconds from action `earlier` to action `later`.
fn gap(rows: &[RawAction], later: usize, earlier: usize) -> i64 {
    rows[later].time_ms - rows[earlier].time_ms
}

/// Python's `__process_recorded_actions` without the I/O: drops actions
/// before `start_ms`, keys the rest by `"{time}-{kind}"` (a later action with
/// the same key replaces an earlier one), sorts by key as text, and runs
/// every rewrite pass in Python's order.
///
/// `origins_to_save` are the origins of the tabs the harvest visited (Python's
/// `__origins_to_save`): actions from those origins are kept even when no
/// navigation to them was recorded.
///
/// An action that no longer means anything has the kind `_skip`.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::api::recorder::{process, RawAction};
/// use serde_json::json;
///
/// let raw = vec![
///     RawAction::new("begin", json!("https://a.test"), json!("https://a.test/"), 1000),
///     RawAction::new("click", json!("#go"), json!(""), 1500),
///     RawAction::new("_url_", json!("https://a.test"), json!("https://a.test/next"), 1600),
/// ];
/// let rows = process(raw, 0, &[]);
/// let kinds: Vec<&str> = rows.iter().map(|row| row.kind.as_str()).collect();
/// // A page reached by a click is `f_url`: "go there unless already there".
/// assert_eq!(kinds, ["begin", "click", "f_url"]);
/// ```
pub fn process(raw: Vec<RawAction>, start_ms: i64, origins_to_save: &[String]) -> Vec<RawAction> {
    let mut by_key: BTreeMap<String, RawAction> = BTreeMap::new();
    for action in raw {
        if action.time_ms < start_ms {
            continue;
        }
        by_key.insert(format!("{}-{}", action.time_ms, action.kind), action);
    }
    let mut rows: Vec<RawAction> = by_key.into_values().collect();

    rename_skipped_opens(&mut rows);
    skip_opens_after_a_skipped_open(&mut rows);
    fold_checkbox_clicks(&mut rows);
    skip_checkbox_before_click(&mut rows);
    skip_repeated_open_after_skip(&mut rows);
    open_after_click(&mut rows);
    merge_adjacent_opens(&mut rows);
    drop_superseded_inputs(&mut rows);
    open_after_button_or_input(&mut rows);
    fold_double_clicks(&mut rows);
    drop_empty_double_clicks(&mut rows);
    fold_file_choosers(&mut rows);
    skip_input_after_mfa(&mut rows);
    open_after_submit(&mut rows);
    open_after_open_action(&mut rows);
    open_soon_after_click(&mut rows);
    fold_same_page_opens(&mut rows);
    trim_search_queries(&mut rows);
    skip_open_after_frame_and_open_action(&mut rows);
    skip_open_after_frame_action(&mut rows);
    skip_repeated_open(&mut rows);
    fold_js_typed_then_input(&mut rows);
    fold_jquery_click_then_input(&mut rows);
    fold_js_type_then_input(&mut rows);
    fold_jquery_type_then_input(&mut rows);

    let mut origins = known_origins(&rows);
    origins.extend(origins_to_save.iter().cloned());

    fold_hover_and_click(&mut rows, &origins);
    fold_file_choosers_with_origin(&mut rows, &origins);
    drop_repeated_file_choosers(&mut rows);
    round_canvas_offsets(&mut rows);
    keep_actions_of_known_origins(&mut rows, &mut origins);
    skip_input_after_typing_action(&mut rows);
    for row in &mut rows {
        if row.kind == "ch_cl" {
            "js_cl".clone_into(&mut row.kind);
        }
    }
    for row in &mut rows {
        if row.kind == "s_val" {
            "set_v".clone_into(&mut row.kind);
            unpair(row);
        }
    }
    rows
}

/// Python's `sw_fr` + `sk_fo` fix, applied to one tab's actions: a frame that
/// opened in a new tab (`sw_fr`, then `sk_fo`) is followed by a switch back to
/// the parent frame, a millisecond before the next action, unless that action
/// is the new tab's own `_url_`.
///
/// The harvest and `save_recorded_actions` call it on each tab.
pub fn add_parent_frame_switches(tab: &mut Vec<RawAction>) {
    let len = tab.len();
    for n in 0..len {
        if n > 2
            && tab[n - 2].kind == "sw_fr"
            && tab[n - 1].kind == "sk_fo"
            && tab[n].kind != "_url_"
        {
            let origin = tab[n - 2].detail.clone();
            let time = tab[n].time_ms - 1;
            tab.push(RawAction::new(
                "sw_pf",
                Value::String(String::new()),
                origin,
                time,
            ));
        }
    }
}

/// Python: `sk_fo` becomes `sk_op`.
fn rename_skipped_opens(rows: &mut [RawAction]) {
    for row in rows {
        if row.kind == "sk_fo" {
            "sk_op".clone_into(&mut row.kind);
        }
    }
}

/// An open right after `sk_op` (a page opened in a new tab already) is skipped.
fn skip_opens_after_a_skipped_open(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if is_open(&rows[n].kind) && rows[n - 1].kind == "sk_op" {
            "_skip".clone_into(&mut rows[n].kind);
        }
    }
}

/// A checkbox change right after the click that caused it is one action: the
/// click becomes `ch_cl` (later a JavaScript click) and the change is skipped.
/// The window is 16 ms after a click, 42 after a JavaScript click and 440
/// after a jQuery click.
fn fold_checkbox_clicks(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        let before = rows[n - 1].kind.as_str();
        let elapsed = gap(rows, n, n - 1);
        if rows[n].kind == "c_box"
            && ((before == "click" && elapsed < 16)
                || (before == "js_cl" && elapsed < 42)
                || (before == "jq_cl" && elapsed < 440))
        {
            "_skip".clone_into(&mut rows[n].kind);
            if rows[n - 1].kind == "click" {
                "ch_cl".clone_into(&mut rows[n - 1].kind);
            }
        }
    }
}

/// A click right after a checkbox change (within 16 ms) replaces the change.
fn skip_checkbox_before_click(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if rows[n - 1].kind == "c_box"
            && matches!(rows[n].kind.as_str(), "click" | "js_cl" | "jq_cl")
            && gap(rows, n, n - 1) < 16
        {
            "_skip".clone_into(&mut rows[n - 1].kind);
        }
    }
}

/// An open of the page that was just skipped after `sk_op` is skipped too.
fn skip_repeated_open_after_skip(rows: &mut [RawAction]) {
    for n in 2..rows.len() {
        if is_open(&rows[n].kind)
            && rows[n - 1].kind == "_skip"
            && rows[n - 2].kind == "sk_op"
            && rows[n].detail == rows[n - 1].detail
        {
            "_skip".clone_into(&mut rows[n].kind);
        }
    }
}

/// A page opened right after a click is `f_url` when the click led there: the
/// same address (modulo `www.`, `https` and a trailing slash), a link
/// selector, or an address that extends the click's within 6.5 s.
fn open_after_click(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if !(is_open(&rows[n].kind) && is_click(&rows[n - 1].kind)) {
            continue;
        }
        let before = &rows[n - 1];
        let selector = text(&before.arg);
        let first = if SCRIPT_CLICKS.contains(&before.kind.as_str()) {
            text(nth(&before.detail, 0))
        } else {
            text(&before.detail)
        };
        let url1 = trimmed(first);
        let url2 = trimmed_second(&url1, text(&rows[n].detail));
        let last_part = selector.rsplit(' ').next().unwrap_or("");
        if url1 == url2
            || url1 == url2.replace("www.", "")
            || url1 == url2.replace("https://", "http://")
            || last_part.starts_with("a[href=")
            || (!url1.is_empty()
                && (url2.starts_with(&url1) || url1.contains("?search"))
                && gap(rows, n, n - 1) < 6500)
        {
            "f_url".clone_into(&mut rows[n].kind);
        }
    }
}

/// Two opens in a row: the first is skipped when both are the same page, and
/// the second becomes `f_url` when it extends the first.
fn merge_adjacent_opens(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if !(is_open(&rows[n].kind) && is_open(&rows[n - 1].kind)) {
            continue;
        }
        let url1 = trimmed(text(&rows[n - 1].detail));
        let url2 = trimmed_second(&url1, text(&rows[n].detail));
        if url1.replace("www.", "") == url2.replace("www.", "")
            || url1.replace("http://", "https://") == url2
        {
            "_skip".clone_into(&mut rows[n - 1].kind);
        } else if url2.starts_with(&url1) {
            "f_url".clone_into(&mut rows[n].kind);
        }
    }
}

/// An input that is typed over by the next one, or one before a submit in a
/// textarea that is typed again, is skipped.
fn drop_superseded_inputs(rows: &mut [RawAction]) {
    for n in 0..rows.len() {
        if rows[n].kind == "input"
            && n > 0
            && rows[n - 1].kind == "input"
            && text(&rows[n - 1].detail).is_empty()
        {
            "_skip".clone_into(&mut rows[n - 1].kind);
        } else if rows[n].kind == "input"
            && n > 1
            && rows[n - 2].kind == "input"
            && rows[n - 1].kind == "submi"
            && text(&rows[n - 2].arg).starts_with("textarea")
            && rows[n - 2].arg == rows[n].arg
        {
            "_skip".clone_into(&mut rows[n - 2].kind);
        }
    }
}

/// A page opened within 6.5 s of a click on an `input` or `button`, or of a
/// typed text that ends with Enter, is `f_url`.
fn open_after_button_or_input(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        let before = rows[n - 1].kind.as_str();
        if is_open(&rows[n].kind)
            && (is_click(before) || before == "input")
            && gap(rows, n, n - 1) < 6500
        {
            let led_there = if is_click(before) {
                text(&rows[n - 1].arg).starts_with("input")
                    || text(&rows[n - 1].arg).starts_with("button")
            } else {
                text(&rows[n - 1].detail).ends_with('\n')
            };
            if led_there {
                "f_url".clone_into(&mut rows[n].kind);
            }
        }
    }
}

/// `click`, `click`, `dbclk` on one element is a double click.
fn fold_double_clicks(rows: &mut [RawAction]) {
    for n in 2..rows.len() {
        if rows[n].kind == "dbclk"
            && rows[n - 2].kind == "click"
            && rows[n - 1].kind == "click"
            && rows[n - 2].arg == rows[n - 1].arg
        {
            "_skip".clone_into(&mut rows[n - 2].kind);
            "_skip".clone_into(&mut rows[n - 1].kind);
            rows[n].arg = rows[n - 1].arg.clone();
        }
    }
}

/// A double click that found no element is skipped.
fn drop_empty_double_clicks(rows: &mut [RawAction]) {
    for row in rows {
        if row.kind == "dbclk" && text(&row.arg).is_empty() {
            "_skip".clone_into(&mut row.kind);
        }
    }
}

/// `chfil` followed by `cho_f`: the chooser is skipped and `cho_f` takes the
/// file path that `chfil` held.
fn fold_file_choosers(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if rows[n].kind == "cho_f" && rows[n - 1].kind == "chfil" {
            "_skip".clone_into(&mut rows[n - 1].kind);
            rows[n].detail = nth(&rows[n - 1].arg, 1).clone();
        }
    }
}

/// An input right after `e_mfa` is the code `e_mfa` typed.
fn skip_input_after_mfa(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if rows[n].kind == "input" && rows[n - 1].kind == "e_mfa" {
            "_skip".clone_into(&mut rows[n].kind);
        }
    }
}

/// A page opened right after a submit or an MFA code is `f_url`.
fn open_after_submit(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if is_open(&rows[n].kind) && matches!(rows[n - 1].kind.as_str(), "submi" | "e_mfa") {
            "f_url".clone_into(&mut rows[n].kind);
        }
    }
}

/// `o_url`, `_url_`, then an open within 4.8 s: the last is `f_url`.
fn open_after_open_action(rows: &mut [RawAction]) {
    for n in 2..rows.len() {
        if is_open(&rows[n].kind)
            && rows[n - 1].kind == "_url_"
            && rows[n - 2].kind == "o_url"
            && gap(rows, n, n - 1) < 4800
        {
            "f_url".clone_into(&mut rows[n].kind);
        }
    }
}

/// A page opened within 5.2 s of a click is `f_url`.
fn open_soon_after_click(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if is_open(&rows[n].kind) && is_click(&rows[n - 1].kind) && gap(rows, n, n - 1) < 5200 {
            "f_url".clone_into(&mut rows[n].kind);
        }
    }
}

/// An `f_url` right after a `_url_` of the same page (modulo a trailing slash,
/// `https` and `www.`) is that `_url_`; the earlier one is skipped.
fn fold_same_page_opens(rows: &mut [RawAction]) {
    let normal = |url: &str| {
        url.strip_suffix('/')
            .unwrap_or(url)
            .replace("http://", "https://")
            .replace("://www.", "://")
    };
    for n in 2..rows.len() {
        if rows[n].kind == "f_url"
            && rows[n - 1].kind == "_url_"
            && normal(text(&rows[n].detail)) == normal(text(&rows[n - 1].detail))
        {
            "_url_".clone_into(&mut rows[n].kind);
            "_skip".clone_into(&mut rows[n - 1].kind);
        }
    }
}

/// A search address `...?q=text&more` is cut to `...?q=text`.
fn trim_search_queries(rows: &mut [RawAction]) {
    for row in rows {
        let url = text(&row.detail);
        if row.kind == "f_url" {
            if let (Some(query), Some(and)) = (url.find("?q="), url.find('&')) {
                if query < and {
                    row.detail = Value::String(url.split('&').next().unwrap_or(url).to_owned());
                }
            }
        }
    }
}

/// A skipped open after `o_url`, which follows a frame action within 4.2 s of
/// it, is skipped.
fn skip_open_after_frame_and_open_action(rows: &mut [RawAction]) {
    for n in 3..rows.len() {
        if is_open(&rows[n].kind)
            && rows[n - 1].kind == "_skip"
            && rows[n - 2].kind == "o_url"
            && is_frame(&rows[n - 3].kind)
            && gap(rows, n, n - 3) < 4200
        {
            "_skip".clone_into(&mut rows[n].kind);
        }
    }
}

/// A page opened within 4.2 s of a frame action is skipped.
fn skip_open_after_frame_action(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if is_open(&rows[n].kind) && is_frame(&rows[n - 1].kind) && gap(rows, n, n - 1) < 4200 {
            "_skip".clone_into(&mut rows[n].kind);
        }
    }
}

/// An open of the page that `f_url` or `_url_` just opened (within 4.8 s) is
/// skipped.
fn skip_repeated_open(rows: &mut [RawAction]) {
    for n in 2..rows.len() {
        if is_open(&rows[n].kind)
            && matches!(rows[n - 1].kind.as_str(), "f_url" | "_url_")
            && rows[n].detail == rows[n - 1].detail
            && gap(rows, n, n - 1) < 4800
        {
            "_skip".clone_into(&mut rows[n].kind);
        }
    }
}

/// `input`, `js_cl`, `input` (the second typed text is the first plus Enter,
/// within 440 ms of the click): the first two are skipped.
fn fold_js_typed_then_input(rows: &mut [RawAction]) {
    for n in 3..rows.len() {
        let typed = text(&rows[n].detail);
        if rows[n].kind == "input"
            && rows[n - 1].kind == "js_cl"
            && rows[n - 2].kind == "input"
            && typed.ends_with('\n')
            && rows[n].arg == rows[n - 2].arg
            && without_last(typed, 1) == text(&rows[n - 2].detail)
            && gap(rows, n, n - 1) < 440
        {
            "_skip".clone_into(&mut rows[n - 1].kind);
            "_skip".clone_into(&mut rows[n - 2].kind);
        }
    }
}

/// A typed text ending with Enter within 900 ms of a jQuery click skips the
/// click.
fn fold_jquery_click_then_input(rows: &mut [RawAction]) {
    for n in 2..rows.len() {
        if rows[n].kind == "input"
            && rows[n - 1].kind == "jq_cl"
            && text(&rows[n].detail).ends_with('\n')
            && gap(rows, n, n - 1) < 900
        {
            "_skip".clone_into(&mut rows[n - 1].kind);
        }
    }
}

/// `js_ty`, two skipped actions, then the same text plus Enter within 440 ms:
/// the `js_ty` is skipped.
fn fold_js_type_then_input(rows: &mut [RawAction]) {
    for n in 3..rows.len() {
        let typed = text(&rows[n].detail);
        if rows[n].kind == "input"
            && rows[n - 3].kind == "js_ty"
            && rows[n - 1].kind == "_skip"
            && rows[n - 2].kind == "_skip"
            && typed.ends_with('\n')
            && without_last(typed, 1) == text(nth(&rows[n - 3].arg, 1))
            && gap(rows, n, n - 3) < 440
        {
            "_skip".clone_into(&mut rows[n - 3].kind);
        }
    }
}

/// `jq_ty`, one skipped action, then the same text plus Enter within 900 ms:
/// the `jq_ty` is skipped.
fn fold_jquery_type_then_input(rows: &mut [RawAction]) {
    for n in 2..rows.len() {
        let typed = text(&rows[n].detail);
        if rows[n].kind == "input"
            && rows[n - 2].kind == "jq_ty"
            && rows[n - 1].kind == "_skip"
            && typed.ends_with('\n')
            && without_last(typed, 1) == text(nth(&rows[n - 2].arg, 1))
            && gap(rows, n, n - 2) < 900
        {
            "_skip".clone_into(&mut rows[n - 2].kind);
        }
    }
}

/// The origins the recording opened pages on, without a trailing slash.
fn known_origins(rows: &[RawAction]) -> Vec<String> {
    let mut origins: Vec<String> = Vec::new();
    for row in rows {
        if is_open(&row.kind) || row.kind == "f_url" {
            let origin = text(&row.arg);
            let origin = origin.strip_suffix('/').unwrap_or(origin).to_owned();
            if !origins.contains(&origin) {
                origins.push(origin);
            }
        }
    }
    origins
}

/// A hover (`ho_cl`) before a click on a known origin, with the click: one
/// `h_clk` (hover this, click that).
fn fold_hover_and_click(rows: &mut [RawAction], origins: &[String]) {
    for n in 1..rows.len() {
        if rows[n].kind == "click"
            && rows[n - 1].kind == "ho_cl"
            && origins
                .iter()
                .any(|origin| origin == text(&rows[n - 1].detail))
        {
            "_skip".clone_into(&mut rows[n - 1].kind);
            "h_clk".clone_into(&mut rows[n].kind);
            rows[n].arg = nth(&rows[n - 1].arg, 0).clone();
            rows[n].detail = nth(&rows[n - 1].arg, 1).clone();
        }
    }
}

/// `chfil` on a known origin is `cho_f` with the selector and the path.
fn fold_file_choosers_with_origin(rows: &mut [RawAction], origins: &[String]) {
    for row in rows {
        if row.kind == "chfil" && origins.iter().any(|origin| origin == text(&row.detail)) {
            "cho_f".clone_into(&mut row.kind);
            unpair(row);
        }
    }
}

/// Two `sh_fc` in a row are one.
fn drop_repeated_file_choosers(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if rows[n].kind == "sh_fc" && rows[n - 1].kind == "sh_fc" {
            "_skip".clone_into(&mut rows[n - 1].kind);
        }
    }
}

/// Canvas offsets are whole pixels: `ceil(float(x))`.
fn round_canvas_offsets(rows: &mut [RawAction]) {
    for row in rows {
        if row.kind != "canva" {
            continue;
        }
        if let Some(parts) = row.arg.as_array_mut() {
            for part in parts.iter_mut().skip(1).take(2) {
                let number = match &*part {
                    Value::Number(number) => number.as_f64(),
                    Value::String(text) => text.trim().parse::<f64>().ok(),
                    _ => None,
                };
                if let Some(number) = number {
                    // `as` saturates; a pixel offset is nowhere near the limits.
                    *part = Value::from(number.ceil() as i64);
                }
            }
        }
    }
}

/// The actions the API methods record (not the page script) and the click
/// kinds: an action whose origin was never opened is skipped, and a `_url_`
/// opens its own origin. The two typed kinds (`js_ty`, `jq_ty`, `pkeys`,
/// `e_mfa`) hold `[selector, text]` in their first argument; this moves the
/// text to the second.
const FROM_THE_API: &[&str] = &[
    "_url_", "js_cl", "js_ca", "js_ty", "s_val", "jq_cl", "jq_ca", "jq_ty", "pkeys", "r_clk",
    "as_el", "as_ep", "asenv", "acc_a", "dis_a", "hi_li", "as_lt", "as_ti", "as_tc", "a_url",
    "a_u_c", "as_df", "do_fi", "as_at", "as_te", "astnv", "aetnv", "as_et", "asnet", "wf_el",
    "sw_fr", "sw_dc", "sw_pf", "s_c_f", "s_c_d", "hover", "sleep", "sh_fc", "s_at_", "s_ats",
    "a_d_m", "d_d_m", "c_l_s", "c_s_s", "d_a_c", "e_mfa", "go_bk", "go_fw", "s_scr", "ss_tf",
    "ss_tl", "pdftl", "spstl", "da_el", "da_ep", "da_te", "da_et", "danet", "pr_da",
];

fn from_the_api(kind: &str) -> bool {
    FROM_THE_API.contains(&kind)
}

/// Moves `[a, b]` held in `arg` to `arg = a`, `detail = b`.
fn unpair(row: &mut RawAction) {
    let first = nth(&row.arg, 0).clone();
    let second = nth(&row.arg, 1).clone();
    row.detail = second;
    row.arg = first;
}

fn keep_actions_of_known_origins(rows: &mut [RawAction], origins: &mut Vec<String>) {
    for row in rows {
        if !from_the_api(&row.kind) {
            continue;
        }
        let origin = if SCRIPT_CLICKS.contains(&row.kind.as_str()) {
            text(nth(&row.detail, 1))
        } else {
            text(&row.detail)
        };
        let origin = origin.strip_suffix('/').unwrap_or(origin).to_owned();
        if matches!(row.kind.as_str(), "js_ty" | "jq_ty" | "pkeys" | "e_mfa") {
            unpair(row);
        }
        if row.kind == "_url_" && !origins.contains(&origin) {
            origins.push(origin.clone());
        }
        if !origins.contains(&origin) {
            "_skip".clone_into(&mut row.kind);
        }
    }
}

/// An input that repeats the text of the typing action before it is skipped.
fn skip_input_after_typing_action(rows: &mut [RawAction]) {
    for n in 1..rows.len() {
        if rows[n].kind == "input"
            && matches!(rows[n - 1].kind.as_str(), "js_ty" | "jq_ty" | "pkeys")
            && rows[n].detail == rows[n - 1].detail
        {
            "_skip".clone_into(&mut rows[n].kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(kind: &str, arg: Value, detail: Value, time: i64) -> RawAction {
        RawAction::new(kind, arg, detail, time)
    }

    fn open(kind: &str, url: &str, time: i64) -> RawAction {
        row(kind, json!("https://a.test"), json!(url), time)
    }

    fn click(selector: &str, time: i64) -> RawAction {
        row("click", json!(selector), json!(""), time)
    }

    fn input(selector: &str, text: &str, time: i64) -> RawAction {
        row("input", json!(selector), json!(text), time)
    }

    fn kinds(rows: &[RawAction]) -> Vec<&str> {
        rows.iter().map(|row| row.kind.as_str()).collect()
    }

    /// `build(gap)` makes the actions with `gap` ms between the two that the
    /// threshold compares; the pass must act at `limit - 1` and not at `limit`.
    fn threshold(
        build: impl Fn(i64) -> Vec<RawAction>,
        pass: fn(&mut [RawAction]),
        index: usize,
        changes_to: &str,
        limit: i64,
    ) {
        let mut below = build(limit - 1);
        pass(&mut below);
        assert_eq!(
            below[index].kind,
            changes_to,
            "{} ms is inside the window",
            limit - 1
        );
        let mut at = build(limit);
        let unchanged = at[index].kind.clone();
        pass(&mut at);
        assert_eq!(
            at[index].kind, unchanged,
            "{limit} ms is outside the window"
        );
    }

    #[test]
    fn actions_before_the_start_are_dropped_and_the_rest_sorted_as_text() {
        // Python sorts the keys as text: "1000-click" comes before "90-click".
        let raw = vec![click("#late", 1000), click("#early", 5), click("#mid", 90)];
        let rows = process(raw, 10, &[]);
        let order: Vec<&str> = rows.iter().map(|row| text(&row.arg)).collect();
        assert_eq!(order, ["#late", "#mid"]);
    }

    #[test]
    fn an_action_with_the_same_time_and_kind_replaces_the_earlier_one() {
        let rows = process(vec![click("#a", 7), click("#b", 7)], 0, &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(text(&rows[0].arg), "#b");
    }

    #[test]
    fn sk_fo_becomes_sk_op_and_the_open_after_it_is_skipped() {
        let raw = vec![
            row("sk_fo", json!(""), json!("o"), 1),
            open("_url_", "https://a.test/x", 2),
        ];
        assert_eq!(kinds(&process(raw, 0, &[])), ["sk_op", "_skip"]);
    }

    #[test]
    fn a_checkbox_change_folds_into_the_click_that_caused_it() {
        let build = |elapsed| {
            vec![
                click("#box", 100),
                row("c_box", json!("#box"), json!("yes"), 100 + elapsed),
            ]
        };
        threshold(build, fold_checkbox_clicks, 1, "_skip", 16);
        let mut rows = build(5);
        fold_checkbox_clicks(&mut rows);
        assert_eq!(kinds(&rows), ["ch_cl", "_skip"]);

        let mut js = vec![row("js_cl", json!("#b"), json!(["u", "o"]), 0), {
            row("c_box", json!("#b"), json!("yes"), 42)
        }];
        fold_checkbox_clicks(&mut js);
        assert_eq!(
            js[1].kind, "c_box",
            "42 ms is outside a JavaScript click's window"
        );
        js[1].time_ms = 41;
        fold_checkbox_clicks(&mut js);
        assert_eq!(js[1].kind, "_skip");
        let mut jq = vec![row("jq_cl", json!("#b"), json!(["u", "o"]), 0), {
            row("c_box", json!("#b"), json!("no"), 440)
        }];
        fold_checkbox_clicks(&mut jq);
        assert_eq!(jq[1].kind, "c_box");
        jq[1].time_ms = 439;
        fold_checkbox_clicks(&mut jq);
        assert_eq!(jq[1].kind, "_skip");
    }

    #[test]
    fn a_click_right_after_a_checkbox_change_replaces_it() {
        let build = |elapsed| {
            vec![
                row("c_box", json!("#box"), json!("yes"), 100),
                click("#box", 100 + elapsed),
            ]
        };
        threshold(build, skip_checkbox_before_click, 0, "_skip", 16);
    }

    #[test]
    fn a_page_after_a_click_is_f_url_when_the_click_led_there() {
        let build = |elapsed| {
            vec![
                row("click", json!("#go"), json!("https://a.test/a"), 100),
                open("_url_", "https://a.test/a/b", 100 + elapsed),
            ]
        };
        threshold(build, open_after_click, 1, "f_url", 6500);
        // Same address modulo a trailing slash, `www.` or https: no time limit.
        for target in [
            "https://a.test/a/",
            "https://www.a.test/a",
            "http://a.test/a",
        ] {
            let mut rows = vec![
                row("click", json!("#go"), json!("https://a.test/a"), 100),
                open("_url_", target, 90_000),
            ];
            if target.starts_with("http://") {
                rows[0].detail = json!("http://a.test/a");
                rows[1].detail = json!("https://a.test/a");
            }
            open_after_click(&mut rows);
            assert_eq!(rows[1].kind, "f_url", "{target}");
        }
        // A link selector never needs the time.
        let mut rows = vec![
            row("click", json!("div a[href=\"/x\"]"), json!(""), 100),
            open("_url_", "https://elsewhere.test/", 90_000),
        ];
        open_after_click(&mut rows);
        assert_eq!(rows[1].kind, "f_url");
        // A JavaScript click keeps its URL in the first of `[url, origin]`.
        let mut rows = vec![
            row(
                "js_cl",
                json!("#go"),
                json!(["https://a.test/a", "https://a.test"]),
                100,
            ),
            open("_url_", "https://a.test/a", 90_000),
        ];
        open_after_click(&mut rows);
        assert_eq!(rows[1].kind, "f_url");
    }

    #[test]
    fn the_second_url_trim_keeps_pythons_slip() {
        // url2 ends with "/#/": Python writes `url1[:-3]`, not `url2[:-3]`.
        assert_eq!(
            trimmed_second("https://a.test/abc", "https://z.test/#/"),
            "https://a.test/"
        );
        assert_eq!(trimmed("https://a.test/#/"), "https://a.test");
        assert_eq!(trimmed("https://a.test/"), "https://a.test");
        assert_eq!(trimmed("ab"), "ab");
    }

    #[test]
    fn two_opens_in_a_row_collapse() {
        let mut same = vec![
            open("begin", "https://www.a.test/", 1),
            open("_url_", "https://a.test/", 2),
        ];
        // `www.` removed from both, trailing slash trimmed.
        merge_adjacent_opens(&mut same);
        assert_eq!(kinds(&same), ["_skip", "_url_"]);
        let mut http = vec![
            open("begin", "http://a.test", 1),
            open("_url_", "https://a.test", 2),
        ];
        merge_adjacent_opens(&mut http);
        assert_eq!(kinds(&http), ["_skip", "_url_"]);
        let mut longer = vec![
            open("begin", "https://a.test/a", 1),
            open("_url_", "https://a.test/a/b", 2),
        ];
        merge_adjacent_opens(&mut longer);
        assert_eq!(kinds(&longer), ["begin", "f_url"]);
    }

    #[test]
    fn an_empty_input_before_another_is_dropped() {
        let mut rows = vec![input("#q", "", 1), input("#q", "rust", 2)];
        drop_superseded_inputs(&mut rows);
        assert_eq!(kinds(&rows), ["_skip", "input"]);
        let mut area = vec![
            input("textarea#t", "a", 1),
            row("submi", json!("textarea#t"), json!("a\n"), 2),
            input("textarea#t", "a\n", 3),
        ];
        drop_superseded_inputs(&mut area);
        assert_eq!(kinds(&area), ["_skip", "submi", "input"]);
    }

    #[test]
    fn a_page_after_a_button_click_or_an_enter_is_f_url() {
        let build = |elapsed| {
            vec![
                click("button#go", 100),
                open("_url_", "https://a.test/r", 100 + elapsed),
            ]
        };
        threshold(build, open_after_button_or_input, 1, "f_url", 6500);
        let build = |elapsed: i64| {
            vec![
                click("#plain", 100),
                open("_url_", "https://a.test/r", 100 + elapsed),
            ]
        };
        let mut rows = build(1);
        open_after_button_or_input(&mut rows);
        assert_eq!(rows[1].kind, "_url_", "a plain element does not submit");
        let build = |elapsed| {
            vec![
                input("#q", "rust\n", 100),
                open("_url_", "https://a.test/r", 100 + elapsed),
            ]
        };
        threshold(build, open_after_button_or_input, 1, "f_url", 6500);
        let mut rows = vec![
            input("#q", "rust", 100),
            open("_url_", "https://a.test/r", 101),
        ];
        open_after_button_or_input(&mut rows);
        assert_eq!(rows[1].kind, "_url_");
    }

    #[test]
    fn click_click_dbclk_is_one_double_click() {
        let mut rows = vec![
            click("#a", 1),
            click("#a", 2),
            row("dbclk", json!(""), json!(""), 3),
        ];
        fold_double_clicks(&mut rows);
        assert_eq!(kinds(&rows), ["_skip", "_skip", "dbclk"]);
        assert_eq!(text(&rows[2].arg), "#a");
        let mut other = vec![
            click("#a", 1),
            click("#b", 2),
            row("dbclk", json!(""), json!(""), 3),
        ];
        fold_double_clicks(&mut other);
        assert_eq!(kinds(&other), ["click", "click", "dbclk"]);
        drop_empty_double_clicks(&mut other);
        assert_eq!(other[2].kind, "_skip");
    }

    #[test]
    fn a_file_chooser_takes_the_path_chfil_held() {
        let mut rows = vec![
            row(
                "chfil",
                json!(["#f", "/tmp/x.txt"]),
                json!("https://a.test"),
                1,
            ),
            row("cho_f", json!("#f"), json!("C:\\fakepath\\x.txt"), 2),
        ];
        fold_file_choosers(&mut rows);
        assert_eq!(kinds(&rows), ["_skip", "cho_f"]);
        assert_eq!(rows[1].detail, json!("/tmp/x.txt"));
    }

    #[test]
    fn a_chfil_on_a_known_origin_becomes_cho_f() {
        let mut rows = vec![row(
            "chfil",
            json!(["#f", "/tmp/x.txt"]),
            json!("https://a.test"),
            1,
        )];
        fold_file_choosers_with_origin(&mut rows, &["https://a.test".to_owned()]);
        assert_eq!(rows[0].kind, "cho_f");
        assert_eq!(
            (text(&rows[0].arg), text(&rows[0].detail)),
            ("#f", "/tmp/x.txt")
        );
    }

    #[test]
    fn pages_after_a_submit_an_mfa_code_or_an_open_action_are_f_url() {
        let mut rows = vec![
            row("submi", json!("#q"), json!("x\n"), 1),
            open("_url_", "https://a.test/r", 2),
        ];
        open_after_submit(&mut rows);
        assert_eq!(rows[1].kind, "f_url");
        let build = |elapsed| {
            vec![
                row("o_url", json!("o"), json!("u"), 10),
                open("_url_", "https://a.test/1", 11),
                open("_url_", "https://a.test/2", 11 + elapsed),
            ]
        };
        threshold(build, open_after_open_action, 2, "f_url", 4800);
        let mut mfa = vec![
            row("e_mfa", json!(["#c", "KEY"]), json!("o"), 1),
            input("#c", "123456", 2),
        ];
        skip_input_after_mfa(&mut mfa);
        assert_eq!(mfa[1].kind, "_skip");
    }

    #[test]
    fn a_page_soon_after_a_click_is_f_url() {
        let build = |elapsed| {
            vec![
                click("#plain", 100),
                open("_url_", "https://x.test/", 100 + elapsed),
            ]
        };
        threshold(build, open_soon_after_click, 1, "f_url", 5200);
    }

    #[test]
    fn an_f_url_of_the_page_just_opened_folds_into_it() {
        let mut rows = vec![
            open("begin", "https://a.test/", 1),
            open("_url_", "http://www.a.test/", 2),
            open("f_url", "https://a.test", 3),
        ];
        fold_same_page_opens(&mut rows);
        assert_eq!(kinds(&rows), ["begin", "_skip", "_url_"]);
    }

    #[test]
    fn a_search_query_is_cut_before_the_next_parameter() {
        let mut rows = vec![
            open("f_url", "https://s.test/search?q=rust&sca=1&x=2", 1),
            open("f_url", "https://s.test/?a=1&q=rust", 2),
        ];
        trim_search_queries(&mut rows);
        assert_eq!(text(&rows[0].detail), "https://s.test/search?q=rust");
        assert_eq!(text(&rows[1].detail), "https://s.test/?a=1&q=rust");
    }

    #[test]
    fn a_page_opened_by_a_frame_action_is_skipped() {
        let build = |elapsed| {
            vec![
                row("sw_fr", json!("iframe"), json!("o"), 100),
                open("_url_", "https://a.test/f", 100 + elapsed),
            ]
        };
        threshold(build, skip_open_after_frame_action, 1, "_skip", 4200);
        let build = |elapsed| {
            vec![
                row("sw_fr", json!("iframe"), json!("o"), 100),
                row("o_url", json!("o"), json!("u"), 101),
                row("_skip", json!(""), json!(""), 102),
                open("_url_", "https://a.test/f", 100 + elapsed),
            ]
        };
        threshold(
            build,
            skip_open_after_frame_and_open_action,
            3,
            "_skip",
            4200,
        );
    }

    #[test]
    fn an_open_of_the_page_just_opened_is_skipped() {
        let build = |elapsed| {
            vec![
                open("begin", "https://a.test/", 1),
                open("f_url", "https://a.test/x", 100),
                open("_url_", "https://a.test/x", 100 + elapsed),
            ]
        };
        threshold(build, skip_repeated_open, 2, "_skip", 4800);
    }

    #[test]
    fn a_typed_text_plus_enter_folds_the_actions_that_led_to_it() {
        let build = |elapsed| {
            vec![
                input("#q", "rust", 1),
                row("js_cl", json!("#q"), json!(["u", "o"]), 50),
                input("#q", "rust\n", 50 + elapsed),
            ]
        };
        // `n > 2` in Python: the pass needs four rows.
        let with_lead = |elapsed| {
            let mut rows = vec![click("#lead", 0)];
            rows.extend(build(elapsed));
            rows
        };
        threshold(with_lead, fold_js_typed_then_input, 2, "_skip", 440);
        let build = |elapsed| {
            vec![
                click("#lead", 0),
                row("jq_cl", json!("#q"), json!(["u", "o"]), 50),
                input("#q", "rust\n", 50 + elapsed),
            ]
        };
        threshold(build, fold_jquery_click_then_input, 1, "_skip", 900);
        let build = |elapsed| {
            vec![
                row("js_ty", json!(["#q", "rust"]), json!("o"), 10),
                row("_skip", json!(""), json!(""), 11),
                row("_skip", json!(""), json!(""), 12),
                input("#q", "rust\n", 10 + elapsed),
            ]
        };
        threshold(build, fold_js_type_then_input, 0, "_skip", 440);
        let build = |elapsed| {
            vec![
                row("jq_ty", json!(["#q", "rust"]), json!("o"), 10),
                row("_skip", json!(""), json!(""), 11),
                input("#q", "rust\n", 10 + elapsed),
            ]
        };
        threshold(build, fold_jquery_type_then_input, 0, "_skip", 900);
    }

    #[test]
    fn a_hover_then_a_click_is_one_h_clk() {
        let mut rows = vec![
            row(
                "ho_cl",
                json!([".menu", ".menu .item"]),
                json!("https://a.test"),
                1,
            ),
            click(".menu .item", 2),
        ];
        fold_hover_and_click(&mut rows, &["https://a.test".to_owned()]);
        assert_eq!(kinds(&rows), ["_skip", "h_clk"]);
        assert_eq!(
            (text(&rows[1].arg), text(&rows[1].detail)),
            (".menu", ".menu .item")
        );
    }

    #[test]
    fn canvas_offsets_are_rounded_up() {
        let mut rows = vec![row("canva", json!(["#c", 10.2, "3.0"]), json!(""), 1)];
        round_canvas_offsets(&mut rows);
        assert_eq!(rows[0].arg, json!(["#c", 11, 3]));
    }

    #[test]
    fn two_file_chooser_shows_are_one() {
        let mut rows = vec![
            row("sh_fc", json!(""), json!("o"), 1),
            row("sh_fc", json!(""), json!("o"), 2),
        ];
        drop_repeated_file_choosers(&mut rows);
        assert_eq!(kinds(&rows), ["_skip", "sh_fc"]);
    }

    #[test]
    fn an_api_action_from_an_unopened_origin_is_skipped() {
        let mut origins = vec!["https://a.test".to_owned()];
        let mut rows = vec![
            row("as_el", json!("h1"), json!("https://a.test/"), 1),
            row("as_el", json!("h1"), json!("https://b.test"), 2),
            row("js_cl", json!("#x"), json!(["u", "https://b.test"]), 3),
            row("pkeys", json!(["#q", "text"]), json!("https://a.test"), 4),
        ];
        keep_actions_of_known_origins(&mut rows, &mut origins);
        assert_eq!(kinds(&rows), ["as_el", "_skip", "_skip", "pkeys"]);
        assert_eq!((text(&rows[3].arg), text(&rows[3].detail)), ("#q", "text"));
        // A `_url_` opens its own origin (Python reads its second argument, the
        // address, and trims one slash).
        let mut rows = vec![
            open("_url_", "https://a.test/x", 1),
            row(
                "_url_",
                json!("https://c.test"),
                json!("https://c.test/"),
                2,
            ),
        ];
        keep_actions_of_known_origins(&mut rows, &mut origins);
        assert_eq!(kinds(&rows), ["_url_", "_url_"]);
        assert!(origins.contains(&"https://a.test/x".to_owned()));
        assert!(origins.contains(&"https://c.test".to_owned()));
    }

    #[test]
    fn an_input_that_repeats_the_typing_action_before_it_is_skipped() {
        let mut rows = vec![
            row("pkeys", json!("#q"), json!("hi"), 1),
            input("#q", "hi", 2),
        ];
        skip_input_after_typing_action(&mut rows);
        assert_eq!(kinds(&rows), ["pkeys", "_skip"]);
    }

    #[test]
    fn ch_cl_ends_as_a_js_click_and_s_val_as_set_v() {
        let rows = process(
            vec![
                row(
                    "begin",
                    json!("https://a.test"),
                    json!("https://a.test/"),
                    1,
                ),
                row("s_val", json!(["#r", "7"]), json!("https://a.test"), 2),
            ],
            0,
            &[],
        );
        assert_eq!(kinds(&rows), ["begin", "set_v"]);
        assert_eq!((text(&rows[1].arg), text(&rows[1].detail)), ("#r", "7"));
        // A click folded with its checkbox change is a JavaScript click.
        let rows = process(
            vec![
                row(
                    "begin",
                    json!("https://a.test"),
                    json!("https://a.test/"),
                    1,
                ),
                click("#box", 100),
                row("c_box", json!("#box"), json!("yes"), 105),
            ],
            0,
            &[],
        );
        assert_eq!(kinds(&rows), ["begin", "js_cl", "_skip"]);
    }

    #[test]
    fn a_frame_opened_in_a_new_tab_gets_a_parent_frame_switch() {
        let mut tab = vec![
            open("begin", "https://a.test/", 1),
            click("#c", 2),
            row("sw_fr", json!("iframe"), json!("https://a.test"), 10),
            row("sk_fo", json!(""), json!("https://a.test"), 11),
            click("#d", 20),
        ];
        add_parent_frame_switches(&mut tab);
        assert_eq!(tab.len(), 6);
        let added = &tab[5];
        assert_eq!((added.kind.as_str(), added.time_ms), ("sw_pf", 19));
        assert_eq!(added.detail, json!("https://a.test"));
        // Not when the next action is the new tab's `_url_`.
        let mut tab = vec![
            click("#c", 1),
            click("#c", 2),
            row("sw_fr", json!("iframe"), json!("o"), 10),
            row("sk_fo", json!(""), json!("o"), 11),
            open("_url_", "https://a.test/x", 20),
        ];
        add_parent_frame_switches(&mut tab);
        assert_eq!(tab.len(), 5);
    }
}
