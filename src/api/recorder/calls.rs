//! The table of recorded actions: one list that the code generators and the
//! replay are made from.
//!
//! Every action the recorder knows is one line of the `recorded_calls!` table
//! below: the name it is recorded under, the [`BaseCase`] method that does it,
//! how its `target` and `value` become that method's arguments ([`Shape`]),
//! and the output formats that have a form of it ([`Mark`]). The table
//! produces
//!
//! * [`CALLS`], which the generators read (`sb.method(args).await?;` for the
//!   `BaseCase` programs, the `Page` calls of the pure CDP program, the
//!   Gherkin steps), and
//! * the replay, which calls the same method with the same arguments.
//!
//! A name is added in one place and is both generated and replayed, so the two
//! cannot disagree about which actions exist or what their arguments mean. The
//! replay is compiled against `BaseCase`, so a method that changes its
//! signature stops the build instead of failing in a recorded test.

use crate::api::base_case::BaseCase;
use crate::SeleniumBaseError;

use super::{
    literal, recorded_secret, revealed, text_argument, RecordedAction, ReplayReport, ReplayStep,
    SECRET_PLACEHOLDER,
};

/// How a recorded action's `target` and `value` become the arguments of the
/// [`BaseCase`] method it calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Shape {
    /// `sb.method()`
    NoArgs,
    /// `sb.method(target)`
    Target,
    /// `sb.method(target, value)`
    TargetValue,
    /// `sb.method(value, target)`: the method takes the text first.
    ValueTarget,
    /// `sb.method(target, 10)`: a wait, with a ten-second timeout.
    TargetSeconds,
    /// `sb.method(10)`: a wait for an alert, with a ten-second timeout.
    Seconds,
    /// The value is `name=content`: `sb.method(target, "name", "content")`.
    TargetPair,
    /// The target is `width,height`: `sb.method(width, height)`.
    Size,
    /// The value is `x, y`: `sb.method(target, x, y)`.
    TargetOffset,
    /// The value is a whole number: `sb.method(target, n)`.
    TargetIndex,
    /// `let _ = sb.method(target).await?;`: the result is not needed.
    TargetDiscarded,
    /// `sb.sleep(2.5).await;`: the value is the pause in seconds, and the call
    /// cannot fail.
    Sleep,
    /// `sb.open_new_tab()`, or `sb.open_new_tab_url(target)` when the action
    /// remembers the page it opened.
    TabUrl,
}

/// What an action's output formats have in common with it beyond its call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mark {
    /// The pure CDP generator writes this action as `Page` and `Locator`
    /// calls (`cdp::cdp_call` holds the statements).
    Cdp,
    /// The Gherkin step `Open "url"`.
    GherkinOpen,
    /// The Gherkin step `Click "selector"`.
    GherkinClick,
    /// The Gherkin step `Type "text" into "selector"`.
    GherkinType,
    /// The Gherkin step `Go back`.
    GherkinBack,
    /// The Gherkin step `Go forward`.
    GherkinForward,
}

/// The Gherkin step an action is written as, if it has one.
pub(super) fn gherkin_mark(name: &str) -> Option<Mark> {
    marks(name)
        .iter()
        .copied()
        .find(|mark| !matches!(mark, Mark::Cdp))
}

/// Makes the table: [`CALLS`], [`marks`], and the dispatcher [`call`] that
/// replays an action. An entry is `name(Shape) [Mark, ...];`, and the name is
/// both the recorded name and the `BaseCase` method.
macro_rules! recorded_calls {
    ($( $name:ident ( $shape:ident ) $( [ $( $mark:ident ),* ] )? ; )*) => {
        /// Every action the generators can turn into a call, and the replay
        /// can run: the recorded name, the [`BaseCase`] method it calls, and
        /// how its arguments are built.
        ///
        /// The recorded names are those `BaseCase` records itself, those
        /// Recorder Mode writes, and those older recordings use.
        pub(super) const CALLS: &[(&str, &str, Shape)] = &[
            $( (stringify!($name), stringify!($name), Shape::$shape) ),*
        ];

        /// The output formats that have a form of the action `name`.
        pub(super) fn marks(name: &str) -> &'static [Mark] {
            match name {
                $( stringify!($name) => &[ $( $( Mark::$mark ),* )? ], )*
                _ => &[],
            }
        }

        /// Runs the action on `sb`: `Ok(true)` when it ran, `Ok(false)` when
        /// the table does not know its name, and the reason when it has no
        /// argument it needs or the call failed.
        async fn call(sb: &mut BaseCase, parts: &Parts<'_>) -> Result<bool, String> {
            match parts.name {
                $( stringify!($name) => { replay_call!($shape, sb, parts, $name); } )*
                _ => return Ok(false),
            }
            Ok(true)
        }
    };
}

/// The call that replays one entry of the table; the arms are the [`Shape`]s.
macro_rules! replay_call {
    (NoArgs, $sb:ident, $p:ident, $m:ident) => {
        ran($sb.$m().await)?
    };
    (Target, $sb:ident, $p:ident, $m:ident) => {{
        let target = $p.target()?;
        ran($sb.$m(target).await)?
    }};
    (TargetDiscarded, $sb:ident, $p:ident, $m:ident) => {{
        let target = $p.target()?;
        ran($sb.$m(target).await)?
    }};
    (TargetValue, $sb:ident, $p:ident, $m:ident) => {{
        let target = $p.target()?;
        let text = $p.text()?;
        ran($sb.$m(target, &text).await)?
    }};
    (ValueTarget, $sb:ident, $p:ident, $m:ident) => {{
        let value = $p.value()?;
        let target = $p.target()?;
        ran($sb.$m(value, target).await)?
    }};
    (TargetSeconds, $sb:ident, $p:ident, $m:ident) => {{
        let target = $p.target()?;
        ran($sb.$m(target, 10).await)?
    }};
    (Seconds, $sb:ident, $p:ident, $m:ident) => {
        ran($sb.$m(10).await)?
    };
    (TargetPair, $sb:ident, $p:ident, $m:ident) => {{
        let (target, key, content) = $p.pair()?;
        ran($sb.$m(target, key, content).await)?
    }};
    (Size, $sb:ident, $p:ident, $m:ident) => {{
        let (width, height) = $p.size()?;
        ran($sb.$m(width, height).await)?
    }};
    (TargetOffset, $sb:ident, $p:ident, $m:ident) => {{
        let (target, x, y, center) = $p.offset()?;
        ran($sb.$m(target, x, y, center).await)?
    }};
    (TargetIndex, $sb:ident, $p:ident, $m:ident) => {{
        let (target, index) = $p.index()?;
        ran($sb.$m(target, index).await)?
    }};
    (Sleep, $sb:ident, $p:ident, $m:ident) => {{
        let seconds = $p.seconds()?;
        $sb.$m(seconds).await;
    }};
    (TabUrl, $sb:ident, $p:ident, $m:ident) => {
        match $p.target {
            Some(url) => ran($sb.open_new_tab_url(url).await)?,
            None => ran($sb.$m().await)?,
        }
    };
}

recorded_calls! {
    open(Target) [Cdp, GherkinOpen];
    goto_if_not_url(Target) [GherkinOpen];
    click(Target) [Cdp, GherkinClick];
    type_text(TargetValue) [Cdp, GherkinType];
    clear(Target) [Cdp];
    click_link_text(Target) [Cdp];
    click_link(Target) [Cdp];
    click_partial_link(Target) [Cdp];
    click_partial_link_text(Target) [Cdp];
    submit(Target) [Cdp];
    hover(Target) [Cdp];
    select_option_by_text(TargetValue) [Cdp];
    select_option_by_value(TargetValue) [Cdp];
    select_option_by_index(TargetIndex) [Cdp];
    select_all(Target);
    unselect_all(Target);
    switch_to_frame(Target);
    switch_to_default_content(NoArgs);
    switch_to_parent_frame(NoArgs);
    drag_and_drop(TargetValue) [Cdp];
    cdp_click(Target) [Cdp];
    wait_for_element_visible(TargetSeconds) [Cdp];
    wait_for_element_absent(TargetSeconds) [Cdp];
    wait_for_element_not_visible(TargetSeconds) [Cdp];
    wait_for_element_present(TargetSeconds) [Cdp];
    wait_for_element_clickable(TargetSeconds);
    wait_for_ready_state_complete(NoArgs) [Cdp];
    assert_text(ValueTarget) [Cdp];
    assert_text_visible(ValueTarget) [Cdp];
    assert_text_not_visible(ValueTarget) [Cdp];
    assert_exact_text(ValueTarget) [Cdp];
    assert_attribute(TargetPair) [Cdp];
    assert_title(Target) [Cdp];
    assert_title_contains(Target) [Cdp];
    assert_link_text(Target);
    maximize_window(NoArgs) [Cdp];
    set_window_size(Size);
    switch_to_window(Target);
    switch_to_new_window(NoArgs);
    switch_to_newest_window(NoArgs);
    switch_to_default_window(NoArgs);
    go_back(NoArgs) [Cdp, GherkinBack];
    go_forward(NoArgs) [Cdp, GherkinForward];
    refresh(NoArgs) [Cdp];
    delete_all_cookies(NoArgs) [Cdp];
    delete_cookie(Target);
    add_cookie(TargetValue);
    double_click(Target) [Cdp];
    context_click(Target) [Cdp];
    click_with_offset(TargetOffset) [Cdp];
    double_click_with_offset(TargetOffset);
    scroll_to(Target) [Cdp];
    scroll_to_bottom(NoArgs) [Cdp];
    scroll_to_top(NoArgs) [Cdp];
    scroll_into_view(Target) [Cdp];
    smooth_scroll_to(Target) [Cdp];
    slow_scroll_to(Target) [Cdp];
    check_if_unchecked(Target) [Cdp];
    uncheck_if_checked(Target) [Cdp];
    add_text(TargetValue) [Cdp];
    send_keys(TargetValue) [Cdp];
    js_click(Target);
    js_type(TargetValue);
    js_update_text(TargetValue);
    js_click_if_present(Target);
    js_click_all(Target);
    jquery_click(Target);
    jquery_click_all(Target);
    jquery_type(TargetValue);
    jquery_update_text(TargetValue);
    set_attribute(TargetPair);
    remove_attribute(TargetValue);
    choose_file(TargetValue);
    wait_for_and_accept_alert(Seconds);
    wait_for_and_dismiss_alert(Seconds);
    accept_alert(NoArgs);
    dismiss_alert(NoArgs);
    type_alert_text(Target);
    clear_local_storage(NoArgs) [Cdp];
    remove_local_storage_item(Target) [Cdp];
    set_local_storage_item(TargetValue) [Cdp];
    close_window(NoArgs);
    highlight_click(Target);
    open_new_window(NoArgs);
    click_visible_elements(Target);
    find_element(TargetDiscarded);
    find_elements(TargetDiscarded);
    get_shadow_root(TargetDiscarded);
    slow_click(Target) [Cdp];
    human_type(TargetValue);
    human_click(Target);
    uc_click(Target);
    uc_type(TargetValue);
    update_text(TargetValue) [Cdp];
    sleep(Sleep) [Cdp];
    click_xpath(Target);
    // What Python's Recorder Mode writes (`recorder_helper.generate_sbase_code`).
    goto(Target) [Cdp, GherkinOpen];
    highlight(Target);
    press_keys(TargetValue) [Cdp];
    enter_mfa_code(TargetValue);
    hover_and_click(TargetValue);
    set_value(TargetValue);
    set_content_to_frame(Target);
    set_content_to_default(NoArgs);
    set_content_to_parent(NoArgs);
    wait_for_element(TargetSeconds) [Cdp];
    assert_element(Target) [Cdp];
    assert_element_present(Target) [Cdp];
    assert_element_not_visible(Target) [Cdp];
    assert_non_empty_text(Target);
    deferred_assert_element(Target);
    deferred_assert_element_present(Target);
    deferred_assert_non_empty_text(Target);
    deferred_assert_text(ValueTarget);
    deferred_assert_exact_text(ValueTarget);
    assert_exact_text_not_visible(ValueTarget) [Cdp];
    set_attributes(TargetPair);
    assert_url(Target) [Cdp];
    assert_url_contains(Target) [Cdp];
    assert_downloaded_file(Target);
    save_screenshot(Target);
    save_screenshot_to_logs(NoArgs);
    save_page_source_to_logs(NoArgs);
    show_file_choosers(NoArgs);
    process_deferred_asserts(NoArgs);
    activate_demo_mode(NoArgs);
    deactivate_demo_mode(NoArgs);
    clear_session_storage(NoArgs);
    // `open_new_tab` is special: with a URL it is `open_new_tab_url`.
    open_new_tab(TabUrl);
}

/// Other names for an action in `CALLS`: those older recordings use.
pub(super) const ALIASES: &[(&str, &str)] = &[
    // `goto` is a call of its own (`BaseCase::goto`, what Python's recorder writes).
    ("navigate", "open"),
    ("type", "type_text"),
    // Deprecated since 0.5.0; older recordings keep the name, and a generated
    // program must not call a deprecated method.
    ("cdp_click_element", "cdp_click"),
];

/// Actions `BaseCase` records without what it takes to run them again.
pub(super) const UNEXPORTABLE: &[(&str, &str)] = &[
    (
        "drag_and_drop_with_offset",
        "the recording keeps the two selectors but not the offset",
    ),
    (
        "download_file",
        "BaseCase::download_file takes a url, a file name and a timeout here, not Python's url and folder",
    ),
    (
        "save_as_pdf_to_logs",
        "BaseCase::save_as_pdf_to_logs needs a file name, which the recording does not hold",
    ),
    (
        "save_screenshot_to_folder",
        "BaseCase::save_screenshot takes a file name only, not a folder",
    ),
];

/// The known action closest to a mistyped `name`, if one is a small edit away.
fn closest_action(name: &str) -> Option<&'static str> {
    let allowed = (name.chars().count() / 3).clamp(1, 3);
    CALLS
        .iter()
        .map(|(known, _, _)| *known)
        .chain(ALIASES.iter().map(|(alias, _)| *alias))
        .map(|known| (edit_distance(name, known), known))
        .filter(|(distance, _)| *distance <= allowed)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, known)| known)
}

/// The number of single-character insertions, deletions and substitutions that
/// turn `a` into `b`.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ca == *cb {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The name `CALLS` knows an action by: `recorded` itself unless it is one of
/// the `ALIASES`.
pub(super) fn canonical_name(recorded: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == recorded)
        .map_or(recorded, |(_, canonical)| *canonical)
}

/// A whole number parsed from recorded text.
fn whole<T: std::str::FromStr>(action: &str, what: &str, text: &str) -> Result<T, String> {
    text.trim()
        .parse::<T>()
        .map_err(|_| format!("{action}: {what} {text:?} is not a whole number in range"))
}

/// A whole number parsed from recorded text, as a Rust literal.
pub(super) fn number<T: std::str::FromStr + std::fmt::Display>(
    action: &str,
    what: &str,
    text: &str,
) -> Result<String, String> {
    whole::<T>(action, what, text).map(|n| n.to_string())
}

/// The parts of the value of a recorded `click_with_offset`: the offset as
/// `x, y`, and whether it counts from the centre of the element (`x, y,
/// center`) instead of from its top-left corner, Python's `center` argument.
pub(super) fn offset_parts<'a>(
    action: &str,
    offset: &'a str,
) -> Result<(&'a str, &'a str, bool), String> {
    let wrong = || format!("{action}: value {offset:?} is not x, y");
    let mut parts = offset.split(',');
    let (Some(x), Some(y)) = (parts.next(), parts.next()) else {
        return Err(wrong());
    };
    let center = match parts.next().map(str::trim) {
        None => false,
        Some("center") => true,
        Some(_) => return Err(wrong()),
    };
    if parts.next().is_some() {
        return Err(wrong());
    }
    Ok((x, y, center))
}

/// A recorded action under the name the table knows it by, with the pieces its
/// arguments are made of. Each accessor checks what the [`Shape`] needs and
/// says what is missing or malformed in the words the generators use.
struct Parts<'a> {
    name: &'a str,
    target: Option<&'a str>,
    value: Option<&'a str>,
}

impl<'a> Parts<'a> {
    fn of(action: &'a RecordedAction) -> Self {
        Self {
            name: canonical_name(&action.name),
            target: action.target.as_deref(),
            value: action.value.as_deref(),
        }
    }

    fn target(&self) -> Result<&'a str, String> {
        self.target
            .ok_or_else(|| format!("{} has no target", self.name))
    }

    fn value(&self) -> Result<&'a str, String> {
        self.value
            .ok_or_else(|| format!("{} has no value", self.name))
    }

    /// The value as it is typed: a recorded password is read from the
    /// environment.
    fn text(&self) -> Result<String, String> {
        revealed(self.value()?).map_err(|error| error.to_string())
    }

    /// `(target, name, content)` of a value `name=content`.
    fn pair(&self) -> Result<(&'a str, &'a str, &'a str), String> {
        let pair = self.value()?;
        let (key, content) = pair
            .split_once('=')
            .ok_or_else(|| format!("{}: value {pair:?} is not name=content", self.name))?;
        Ok((self.target()?, key.trim(), content.trim()))
    }

    /// `(width, height)` of a target `width,height`.
    fn size(&self) -> Result<(u32, u32), String> {
        let size = self.target()?;
        let (width, height) = size
            .split_once(',')
            .ok_or_else(|| format!("{}: target {size:?} is not width,height", self.name))?;
        Ok((
            whole(self.name, "width", width)?,
            whole(self.name, "height", height)?,
        ))
    }

    /// `(target, x, y, center)` of a value `x, y` or `x, y, center`.
    fn offset(&self) -> Result<(&'a str, i64, i64, bool), String> {
        let (x, y, center) = offset_parts(self.name, self.value()?)?;
        Ok((
            self.target()?,
            whole(self.name, "x offset", x)?,
            whole(self.name, "y offset", y)?,
            center,
        ))
    }

    /// `(target, index)` of a value that is a whole number.
    fn index(&self) -> Result<(&'a str, usize), String> {
        Ok((self.target()?, whole(self.name, "index", self.value()?)?))
    }

    /// The pause of a `sleep`, in seconds.
    fn seconds(&self) -> Result<f64, String> {
        let text = self.value()?;
        text.trim()
            .parse::<f64>()
            .ok()
            .filter(|s| s.is_finite() && *s >= 0.0)
            .ok_or_else(|| format!("{}: {text:?} is not a number of seconds", self.name))
    }
}

/// The line of Rust that replays `action`, or why there is none.
pub(super) fn rust_call(action: &RecordedAction) -> Result<String, String> {
    let parts = Parts::of(action);
    let name = parts.name;
    if let Some((_, reason)) = UNEXPORTABLE
        .iter()
        .find(|(unexportable, _)| *unexportable == name)
    {
        return Err(format!("{name}: {reason}"));
    }
    let Some((_, method, shape)) = CALLS.iter().find(|(known, _, _)| *known == name) else {
        return Err(match closest_action(&action.name) {
            Some(near) => format!("unknown action {:?}; did you mean {near:?}?", action.name),
            None => format!("unknown action {:?}", action.name),
        });
    };

    let (method, args) = match shape {
        Shape::NoArgs => (*method, String::new()),
        // `open_new_tab` remembers the page it opened.
        Shape::TabUrl => match parts.target {
            Some(url) => ("open_new_tab_url", literal(url)),
            None => (*method, String::new()),
        },
        Shape::Target | Shape::TargetDiscarded => (*method, literal(parts.target()?)),
        Shape::TargetValue => (
            *method,
            format!(
                "{}, {}",
                literal(parts.target()?),
                text_argument(parts.value()?)
            ),
        ),
        Shape::ValueTarget => (
            *method,
            format!("{}, {}", literal(parts.value()?), literal(parts.target()?)),
        ),
        Shape::TargetSeconds => (*method, format!("{}, 10", literal(parts.target()?))),
        Shape::Seconds => (*method, "10".to_owned()),
        Shape::TargetPair => {
            let (target, key, content) = parts.pair()?;
            (
                *method,
                format!(
                    "{}, {}, {}",
                    literal(target),
                    literal(key),
                    literal(content)
                ),
            )
        }
        Shape::Size => {
            let (width, height) = parts.size()?;
            (*method, format!("{width}, {height}"))
        }
        Shape::TargetOffset => {
            let (target, x, y, center) = parts.offset()?;
            (*method, format!("{}, {x}, {y}, {center}", literal(target)))
        }
        Shape::TargetIndex => {
            let (target, index) = parts.index()?;
            (*method, format!("{}, {index}", literal(target)))
        }
        Shape::Sleep => {
            // `sleep` returns nothing, so there is no `?`.
            return Ok(format!("sb.{method}({:?}).await;", parts.seconds()?));
        }
    };

    let discard = if *shape == Shape::TargetDiscarded {
        "let _ = "
    } else {
        ""
    };
    Ok(format!("{discard}sb.{method}({args}).await?;"))
}

/// A call's outcome as the replay reports it: nothing, or the reason.
fn ran<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<(), String> {
    result.map(drop).map_err(|error| error.to_string())
}

/// What the report says a step did: the name, the target and the value.
fn describe(action: &RecordedAction) -> String {
    let mut text = action.name.clone();
    if let Some(target) = &action.target {
        text.push(' ');
        text.push_str(target);
    }
    if let Some(value) = &action.value {
        text.push_str(&format!(" {value:?}"));
    }
    text
}

/// Replays `actions` on `sb`, one step each: see
/// [`RecorderSession::replay`](super::RecorderSession::replay).
pub(super) async fn replay(
    sb: &mut BaseCase,
    actions: &[RecordedAction],
) -> Result<ReplayReport, SeleniumBaseError> {
    // A recording that types a password needs it before the browser moves.
    if actions.iter().any(|action| {
        action
            .value
            .as_deref()
            .is_some_and(|value| value.starts_with(SECRET_PLACEHOLDER))
    }) {
        recorded_secret()?;
    }

    let mut report = ReplayReport::default();
    for action in actions {
        let outcome = call(sb, &Parts::of(action)).await;
        if outcome == Ok(false) {
            report.skipped += 1;
            report.skipped_names.push(action.name.clone());
            continue;
        }
        report.steps.push(ReplayStep {
            name: action.name.clone(),
            description: describe(action),
            error: outcome.err(),
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::api::base_case::webdriver_support::{element, MockWebDriver, Reply};
    use crate::api::recorder::RecorderSession;
    use crate::browser::config::BrowserConfig;

    /// A WebDriver that finds every element and answers a few reads.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("e1")),
                ("POST", "/elements") => Reply::Value(json!([element("e1")])),
                ("GET", "/url") => Reply::Value(json!("http://x.test/")),
                ("GET", "/title") => Reply::Value(json!("Title")),
                ("GET", "/window/handles") => Reply::Value(json!(["h1", "h2"])),
                ("POST", "/window/new") => Reply::Value(json!({ "handle": "h2", "type": "tab" })),

                ("GET", "/element/e1/rect") => {
                    Reply::Value(json!({ "x": 10.0, "y": 20.0, "width": 100.0, "height": 50.0 }))
                }

                ("GET", path) if path.contains("/attribute/") || path.contains("/property/") => {
                    Reply::Value(json!("/x"))
                }

                ("GET", "/element/e1/text") => Reply::Value(json!("Hello")),
                ("GET", path) if path.ends_with("/displayed") || path.ends_with("/enabled") => {
                    Reply::Value(json!(true))
                }

                _ => Reply::Value(Value::Null),
            }
        })
    }

    fn session_less() -> BaseCase {
        BaseCase::without_session(BrowserConfig::default())
    }

    async fn replay_one(
        sb: &mut BaseCase,
        name: &str,
        target: Option<&str>,
        value: Option<&str>,
    ) -> ReplayReport {
        let action = RecordedAction::new(name, target, value);
        replay(sb, &[action]).await.expect("no password in this")
    }

    /// The commands sent to the driver since `before` requests, as
    /// `(method, path)`.
    fn sent_since(driver: &MockWebDriver, before: usize) -> Vec<(String, String)> {
        driver.all_requests()[before..]
            .iter()
            .map(|request| (request.method.clone(), request.path.clone()))
            .collect()
    }

    #[tokio::test]
    async fn every_name_that_needs_an_argument_is_a_step_and_none_is_skipped() {
        let mut sb = session_less();
        for (name, _, shape) in CALLS {
            // The others act on the browser at once; `each_kind_of_action_...`
            // replays a number of them against a driver.
            if matches!(shape, Shape::NoArgs | Shape::Seconds | Shape::TabUrl) {
                continue;
            }
            let report = replay_one(&mut sb, name, None, None).await;
            assert_eq!(
                (report.steps.len(), report.skipped),
                (1, 0),
                "{name} is in the table, so it is a step"
            );
        }
    }

    #[tokio::test]
    async fn an_action_that_lacks_an_argument_is_a_failed_step_that_names_it() {
        let mut sb = session_less();
        for (name, target, value, reason) in [
            ("click", None, None, "click has no target"),
            ("type_text", Some("#a"), None, "type_text has no value"),
            ("type_text", None, Some("x"), "type_text has no target"),
            ("type", None, Some("x"), "type_text has no target"),
            ("assert_text", Some("#a"), None, "assert_text has no value"),
            ("assert_text", None, Some("Hi"), "assert_text has no target"),
            (
                "wait_for_element_visible",
                None,
                None,
                "wait_for_element_visible has no target",
            ),
            ("sleep", None, None, "sleep has no value"),
            (
                "assert_attribute",
                Some("#a"),
                Some("no equals sign"),
                "assert_attribute: value \"no equals sign\" is not name=content",
            ),
            (
                "set_window_size",
                Some("wide,tall"),
                None,
                "set_window_size: width \"wide\" is not a whole number in range",
            ),
            (
                "click_with_offset",
                Some("#a"),
                Some("1"),
                "click_with_offset: value \"1\" is not x, y",
            ),
            (
                "select_option_by_index",
                Some("#a"),
                Some("two"),
                "select_option_by_index: index \"two\" is not a whole number in range",
            ),
            (
                "sleep",
                None,
                Some("soon"),
                "sleep: \"soon\" is not a number of seconds",
            ),
        ] {
            let report = replay_one(&mut sb, name, target, value).await;
            let step = &report.steps[0];
            assert_eq!(step.error.as_deref(), Some(reason), "{name}");
            assert_eq!(step.name, name, "the step keeps the recorded name");
            assert!(!report.is_success());
        }
    }

    #[tokio::test]
    async fn a_name_the_table_does_not_know_is_skipped_and_named() {
        let mut sb = session_less();
        let actions = [
            RecordedAction::new("levitate", Some("#a"), None),
            // The recording does not keep what this one needs.
            RecordedAction::new("drag_and_drop_with_offset", Some("#a"), Some("#b")),
            RecordedAction::new("download_file", Some("http://x.test/f"), None),
            RecordedAction::new("click", None, None),
        ];
        let report = replay(&mut sb, &actions).await.unwrap();
        assert_eq!(report.skipped, 3);
        assert_eq!(
            report.skipped_names,
            ["levitate", "drag_and_drop_with_offset", "download_file"]
        );
        assert_eq!(report.steps.len(), 1);
        assert!(report.skipped_names.iter().all(|name| name != "click"));
    }

    #[tokio::test]
    async fn each_kind_of_action_sends_the_command_that_does_it() {
        let driver = driver();
        let mut sb = driver.base_case().await;
        let cases = [
            ("open", Some("http://x.test/a"), None, "POST", "/url"),
            ("navigate", Some("http://x.test/a"), None, "POST", "/url"),
            ("goto", Some("http://x.test/a"), None, "POST", "/url"),
            (
                "goto_if_not_url",
                Some("http://x.test/b"),
                None,
                "POST",
                "/url",
            ),
            ("click", Some("#a"), None, "POST", "/element/e1/click"),
            (
                "type_text",
                Some("#a"),
                Some("hi"),
                "POST",
                "/element/e1/value",
            ),
            ("type", Some("#a"), Some("hi"), "POST", "/element/e1/value"),
            ("clear", Some("#a"), None, "POST", "/element/e1/clear"),
            ("go_back", None, None, "POST", "/back"),
            ("go_forward", None, None, "POST", "/forward"),
            ("refresh", None, None, "POST", "/refresh"),
            ("maximize_window", None, None, "POST", "/window/maximize"),
            (
                "set_window_size",
                Some("800,600"),
                None,
                "POST",
                "/window/rect",
            ),
            ("delete_all_cookies", None, None, "DELETE", "/cookie"),
            ("accept_alert", None, None, "POST", "/alert/accept"),
            ("dismiss_alert", None, None, "POST", "/alert/dismiss"),
            ("switch_to_default_content", None, None, "POST", "/frame"),
            (
                "switch_to_parent_frame",
                None,
                None,
                "POST",
                "/frame/parent",
            ),
            ("assert_title", Some("Title"), None, "GET", "/title"),
            (
                "assert_text",
                Some("#a"),
                Some("Hello"),
                "GET",
                "/element/e1/text",
            ),
            ("js_click", Some("#a"), None, "POST", "/execute/sync"),
            ("scroll_to_top", None, None, "POST", "/execute/sync"),
            ("hover", Some("#a"), None, "POST", "/actions"),
            ("find_element", Some("#a"), None, "POST", "/element"),
            (
                "wait_for_element_present",
                Some("#a"),
                None,
                "POST",
                "/element",
            ),
            (
                "wait_for_element_visible",
                Some("#a"),
                None,
                "GET",
                "/element/e1/displayed",
            ),
            (
                "click_with_offset",
                Some("#a"),
                Some("5, -7"),
                "POST",
                "/actions",
            ),
            (
                "assert_attribute",
                Some("#a"),
                Some("href=/x"),
                "GET",
                "/element/e1/attribute/href",
            ),
            ("open_new_tab", None, None, "POST", "/execute/sync"),
            (
                "open_new_tab",
                Some("http://x.test/n"),
                None,
                "POST",
                "/url",
            ),
            ("sleep", None, Some("0"), "", ""),
        ];
        for (name, target, value, method, path) in cases {
            let before = driver.all_requests().len();
            let report = replay_one(&mut sb, name, target, value).await;
            let step = &report.steps[0];
            assert!(step.passed(), "{name}: {:?}", step.error);
            let sent = sent_since(&driver, before);
            if path.is_empty() {
                assert!(sent.is_empty(), "{name} sent {sent:?}");
            } else {
                assert!(
                    sent.contains(&(method.to_owned(), path.to_owned())),
                    "{name} should send {method} {path}, sent {sent:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn the_arguments_reach_the_command_in_the_order_the_method_takes_them() {
        let driver = driver();
        let mut sb = driver.base_case().await;
        replay_one(&mut sb, "assert_text", Some("#msg"), Some("Hello")).await;
        replay_one(&mut sb, "set_window_size", Some("800,600"), None).await;
        replay_one(&mut sb, "type_text", Some("#a"), Some("typed")).await;
        replay_one(&mut sb, "open", Some("http://x.test/a"), None).await;

        let found: Vec<_> = driver
            .all_requests()
            .into_iter()
            .filter(|request| request.body["value"] == "#msg")
            .collect();
        assert!(
            !found.is_empty(),
            "assert_text(text, css): css is the target"
        );
        assert_eq!(driver.requests("/window/rect")[0].body["width"], 800);
        assert_eq!(driver.requests("/window/rect")[0].body["height"], 600);
        assert_eq!(
            driver.requests("/element/e1/value")[0].body["text"],
            "typed"
        );
        assert_eq!(driver.requests("/url")[0].body["url"], "http://x.test/a");
    }

    #[tokio::test]
    async fn goto_if_not_url_stays_on_the_page_it_is_on() {
        let driver = driver();
        let mut sb = driver.base_case().await;
        // The driver says the browser is at http://x.test/.
        let before = driver.all_requests().len();
        replay_one(&mut sb, "goto_if_not_url", Some("http://x.test/"), None).await;
        let sent = sent_since(&driver, before);
        assert!(
            !sent.contains(&("POST".to_owned(), "/url".to_owned())),
            "{sent:?}"
        );
    }

    #[tokio::test]
    async fn a_saved_recording_replays_from_json_and_from_yaml() {
        let driver = driver();
        let mut sb = driver.base_case().await;
        let mut session = RecorderSession::new();
        session.push("goto", Some("http://x.test/a"), None);
        session.push("type", Some("#q"), Some("rust \"lang\"\n"));
        session.push("click", Some("#go"), None);
        let folder = tempfile::tempdir().unwrap();

        for file in ["rec.json", "rec.yaml"] {
            let path = session.save(&folder.path().join(file)).unwrap();
            let actions = RecorderSession::load(&path).unwrap();
            let before = driver.all_requests().len();

            let report = replay(&mut sb, &actions).await.unwrap();

            assert!(report.is_success(), "{file}: {report:?}");
            assert_eq!((report.steps.len(), report.skipped), (3, 0), "{file}");
            let sent = sent_since(&driver, before);
            for expected in [
                ("POST", "/url"),
                ("POST", "/element/e1/value"),
                ("POST", "/element/e1/click"),
            ] {
                assert!(
                    sent.contains(&(expected.0.to_owned(), expected.1.to_owned())),
                    "{file}: {expected:?} not in {sent:?}"
                );
            }
        }
        let typed = driver.requests("/element/e1/value");
        assert_eq!(typed[0].body["text"], "rust \"lang\"\n");
    }

    #[tokio::test]
    async fn a_recording_with_a_password_needs_it_before_the_browser_moves() {
        if std::env::var(super::super::SECRET_VARIABLE).is_ok() {
            return;
        }
        let driver = driver();
        let mut sb = driver.base_case().await;
        let before = driver.all_requests().len();
        let actions = [
            RecordedAction::new("goto", Some("http://x.test/a"), None),
            RecordedAction::new("type", Some("#pw"), Some("{{SB_RECORDED_SECRET}}\n")),
        ];

        let error = replay(&mut sb, &actions).await.unwrap_err().to_string();

        assert!(error.contains("SB_RECORDED_SECRET"), "{error}");
        assert!(sent_since(&driver, before).is_empty(), "nothing ran");
    }

    #[tokio::test]
    async fn what_basecase_records_replays_without_a_skip_and_sends_the_same_commands() {
        let first = driver();
        let mut sb = first.base_case().await;
        let setup = first.all_requests().len();
        sb.start_recording();
        sb.open("http://x.test/a").await.unwrap();
        sb.type_text("#q", "rust").await.unwrap();
        sb.click("#go").await.unwrap();
        sb.assert_text("Hello", "#msg").await.unwrap();
        let session = sb.recorder_session().unwrap();
        let names: Vec<_> = session.actions().iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["open", "type_text", "click", "assert_text"]);

        let second = driver();
        let mut again = second.base_case().await;
        let setup_again = second.all_requests().len();
        let report = RecorderSession::replay(&mut again, session.actions())
            .await
            .unwrap();

        assert!(report.is_success(), "{report:?}");
        assert_eq!((report.steps.len(), report.skipped), (4, 0));
        assert_eq!(
            sent_since(&first, setup),
            sent_since(&second, setup_again),
            "the replay sends what the recorded calls sent"
        );
    }

    #[test]
    fn the_table_marks_what_the_generators_have_a_form_of() {
        assert_eq!(marks("open"), [Mark::Cdp, Mark::GherkinOpen]);
        assert_eq!(gherkin_mark("goto_if_not_url"), Some(Mark::GherkinOpen));
        assert_eq!(gherkin_mark("go_back"), Some(Mark::GherkinBack));
        assert_eq!(gherkin_mark("hover"), None);
        assert_eq!(marks("hover"), [Mark::Cdp]);
        assert!(marks("levitate").is_empty());
        assert!(
            marks("select_all").is_empty(),
            "no pure CDP call, no Gherkin step"
        );
    }
}
