//! Action recorder that turns browser interactions into Rust test scripts.
//!
//! [`ActionRecorder`] collects a sequence of [`RecordedAction`] values and can
//! emit a standalone Rust `main` function, a `#[tokio::test]` function, or a
//! saved test file. It is used by the recorder mode to replay manual browser
//! sessions as automated tests.
//!
//! [`RecorderSession`] adds a live capture/replay layer on top of the recorder:
//! it injects a small JavaScript listener into the page, drains the captured
//! navigation/click/type events into an action list, serializes them to JSON or
//! YAML, and replays them against a [`BaseCase`](crate::BaseCase).
//!
//! A recording of `BaseCase` calls can be written for either engine:
//! [`to_rust_script`](ActionRecorder::to_rust_script) and
//! [`to_rust_test`](ActionRecorder::to_rust_test) call `BaseCase`, and
//! [`to_cdp_script`](ActionRecorder::to_cdp_script) and
//! [`to_cdp_test`](ActionRecorder::to_cdp_test) call the pure CDP engine
//! ([`sb_cdp`](crate::sb_cdp)), as Python's `--rec-sb-cdp` does.

mod cdp;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::api::base_case::BaseCase;
use crate::SeleniumBaseError;

/// A single recorded browser action.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RecordedAction {
    /// Method name on [`BaseCase`](crate::BaseCase) that the action maps to.
    pub name: String,
    /// Primary argument, typically a selector or URL.
    #[serde(default)]
    pub target: Option<String>,
    /// Secondary argument, such as text to type.
    #[serde(default)]
    pub value: Option<String>,
    /// Unix timestamp in milliseconds when the action was recorded.
    #[serde(default)]
    pub timestamp_ms: u128,
}

impl RecordedAction {
    /// Builds an action with the current wall-clock timestamp.
    pub fn new(name: &str, target: Option<&str>, value: Option<&str>) -> Self {
        Self {
            name: name.to_owned(),
            target: target.map(ToOwned::to_owned),
            value: value.map(ToOwned::to_owned),
            timestamp_ms: now_ms(),
        }
    }
}

/// Collector and code generator for recorded actions.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ActionRecorder {
    /// Ordered list of recorded actions.
    pub actions: Vec<RecordedAction>,
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

impl ActionRecorder {
    /// Records a new action with the current timestamp.
    pub fn record(&mut self, name: &str, target: Option<&str>, value: Option<&str>) {
        self.actions.push(RecordedAction::new(name, target, value));
    }

    /// Records an action only if [`to_rust_script`](Self::to_rust_script) can
    /// turn it into a call.
    ///
    /// For a person typing actions in, this is the difference between "recorded"
    /// and a `// skipped:` comment found in the saved file later.
    ///
    /// # Errors
    ///
    /// Returns why the action cannot be generated: an unknown name (with the
    /// closest known one when there is one), or a missing or malformed argument.
    /// The recording is left unchanged.
    pub fn try_record(
        &mut self,
        name: &str,
        target: Option<&str>,
        value: Option<&str>,
    ) -> Result<(), String> {
        let action = RecordedAction::new(name, target, value);
        rust_call(&action)?;
        self.actions.push(action);
        Ok(())
    }

    /// Generates a standalone `#[tokio::main]` Rust program from the recording.
    ///
    /// An action the generator cannot turn into a call (an unknown name, or one
    /// missing an argument) becomes a `// skipped: ...` comment, so a gap in
    /// the program is never silent.
    pub fn to_rust_script(&self) -> String {
        let mut out = String::from(
            "use seleniumbase_rs::{BaseCase, BrowserConfig};\n\n#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n    let mut sb = BaseCase::new(BrowserConfig::default()).await?;\n",
        );
        self.write_actions(&mut out);
        out.push_str("    sb.quit().await?;\n    Ok(())\n}\n");
        out
    }

    /// Generates a `#[tokio::test]` async function from the recording.
    ///
    /// See [`to_rust_script`](Self::to_rust_script) for actions that cannot be
    /// generated.
    pub fn to_rust_test(&self, test_name: &str) -> String {
        let fn_name = rust_identifier(test_name);
        let mut out =
            String::from("use seleniumbase_rs::{BaseCase, BrowserConfig};\n\n#[tokio::test]\n");
        out.push_str(&format!(
            "async fn {}() -> Result<(), Box<dyn std::error::Error>> {{\n    let mut sb = BaseCase::new(BrowserConfig::default()).await?;\n",
            fn_name
        ));
        self.write_actions(&mut out);
        out.push_str("    sb.quit().await?;\n    Ok(())\n}\n");
        out
    }

    /// Writes the recording as a Rust test file into `dir/{name}.rs`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is empty or
    /// contains a path separator (the file always goes directly into `dir`),
    /// and [`SeleniumBaseError::Io`] if the file cannot be written.
    pub fn save_recording_as_test(
        &self,
        dir: &Path,
        name: &str,
    ) -> Result<PathBuf, SeleniumBaseError> {
        check_file_name(name)?;
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}.rs"));
        std::fs::write(&path, self.to_rust_test(name))?;
        Ok(path)
    }

    /// Generates a Gherkin feature from the recording, for the `behave` runner
    /// with the steps `AsyncStepRegistry::register_common_browser_steps` adds.
    ///
    /// Opening a page, clicking, typing, and going back or forward become
    /// steps. An action with no such step (or text that would break a step's
    /// quotes, or typing no text, which the step cannot match) becomes a
    /// `# skipped:` comment, so a gap is never silent.
    pub fn to_gherkin(&self, feature_name: &str) -> String {
        let title: String = feature_name
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let mut out = format!("Feature: {title}\n\n  Scenario: {title}\n");
        let mut keyword = "Given";
        for action in &self.actions {
            match gherkin_step(action) {
                Ok(step) => {
                    out.push_str(&format!("    {keyword} {step}\n"));
                    keyword = "And";
                }
                Err(reason) => out.push_str(&format!("    # skipped: {reason}\n")),
            }
        }
        out
    }

    /// Writes the recording as a Gherkin feature file into `dir/{name}.feature`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is empty or
    /// contains a path separator, and [`SeleniumBaseError::Io`] if the file
    /// cannot be written.
    pub fn save_recording_as_feature(
        &self,
        dir: &Path,
        name: &str,
    ) -> Result<PathBuf, SeleniumBaseError> {
        check_file_name(name)?;
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}.feature"));
        std::fs::write(&path, self.to_gherkin(name))?;
        Ok(path)
    }

    /// Generates a standalone `#[tokio::main]` program for the pure CDP engine
    /// ([`sb_cdp`](crate::sb_cdp)) from the recording: it launches a
    /// [`Browser`](crate::sb_cdp::Browser), takes its first
    /// [`Page`](crate::sb_cdp::Page), and replays the actions as `Page` and
    /// `Locator` calls. This is what Python's `--rec-sb-cdp` writes with
    /// `sb_cdp` calls.
    ///
    /// Every selector and value is written as a Rust string literal. An action
    /// with no pure CDP call (a frame switch, a JavaScript click, an alert, ...),
    /// an unknown action, or one missing an argument becomes a `// skipped: ...`
    /// comment, so a gap in the program is never silent.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::api::recorder::ActionRecorder;
    ///
    /// let mut recorder = ActionRecorder::default();
    /// recorder.record("open", Some("https://example.com"), None);
    /// recorder.record("click", Some("#go"), None);
    ///
    /// let program = recorder.to_cdp_script();
    /// assert!(program.contains("page.goto(\"https://example.com\").await?;"));
    /// assert!(program.contains("page.locator(\"#go\").click().await?;"));
    /// ```
    pub fn to_cdp_script(&self) -> String {
        cdp::program(&self.actions, "#[tokio::main]\nasync fn main()")
    }

    /// Generates a `#[tokio::test]` async function for the pure CDP engine from
    /// the recording. See [`to_cdp_script`](Self::to_cdp_script) for the calls
    /// and for actions that cannot be generated.
    pub fn to_cdp_test(&self, test_name: &str) -> String {
        let entry = format!("#[tokio::test]\nasync fn {}()", rust_identifier(test_name));
        cdp::program(&self.actions, &entry)
    }

    /// Writes the recording as a pure CDP test file into `dir/{name}.rs`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is empty or
    /// contains a path separator, and [`SeleniumBaseError::Io`] if the file
    /// cannot be written.
    pub fn save_recording_as_cdp_test(
        &self,
        dir: &Path,
        name: &str,
    ) -> Result<PathBuf, SeleniumBaseError> {
        check_file_name(name)?;
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}.rs"));
        std::fs::write(&path, self.to_cdp_test(name))?;
        Ok(path)
    }

    fn write_actions(&self, out: &mut String) {
        for action in &self.actions {
            match rust_call(action) {
                Ok(call) => line(out, &call),
                // `{:?}` keeps a hostile name from ending the comment.
                Err(reason) => line(out, &format!("// skipped: {reason}")),
            }
        }
    }
}

/// Rejects a file name that is empty or would leave the folder it is saved in.
fn check_file_name(name: &str) -> Result<(), SeleniumBaseError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\'])
        || name.contains('\0')
    {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "'{name}' is not a usable test file name: it must be non-empty and contain no path separator"
        )));
    }
    Ok(())
}

/// The Gherkin step (without its keyword) that replays `action`, or why there
/// is none.
fn gherkin_step(action: &RecordedAction) -> Result<String, String> {
    let name = ALIASES
        .iter()
        .find(|(alias, _)| *alias == action.name)
        .map_or(action.name.as_str(), |(_, canonical)| *canonical);
    // A quote or a line break in an argument would end the step early.
    let plain = |text: &str, what: &str| -> Result<String, String> {
        if text.contains(['"', '\n', '\r']) {
            Err(format!(
                "{name}: the {what} {text:?} has a quote or a line break"
            ))
        } else {
            Ok(text.to_owned())
        }
    };
    let target = action
        .target
        .as_deref()
        .ok_or_else(|| format!("{name} has no target"));
    match name {
        "open" => Ok(format!("I open \"{}\"", plain(target?, "URL")?)),
        "click" => Ok(format!("I click \"{}\"", plain(target?, "selector")?)),
        "type_text" => {
            let value = action
                .value
                .as_deref()
                .ok_or_else(|| format!("{name} has no value"))?;
            // The step's pattern is `I type "(.+?)" into "(.+?)"`: no text, no match.
            if value.is_empty() {
                return Err(format!(
                    "{name}: the text is empty, and the I type step needs at least one character"
                ));
            }
            Ok(format!(
                "I type \"{}\" into \"{}\"",
                plain(value, "text")?,
                plain(target?, "selector")?
            ))
        }
        "go_back" => Ok("I go back".to_owned()),
        "go_forward" => Ok("I go forward".to_owned()),
        // The name is whatever was recorded. Written plainly, a line break in it would end the
        // `# skipped:` comment and start a Scenario or a step, so it is escaped.
        other => Err(format!(
            "{}: no Gherkin step for this action",
            other.escape_debug()
        )),
    }
}

/// How a recorded action's `target` and `value` become the arguments of the
/// [`BaseCase`] method it calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
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
}

/// Every action the generator can turn into a call: the recorded name, the
/// [`BaseCase`] method it calls, and how its arguments are built.
///
/// The recorded names are those `BaseCase` records itself, those
/// [`RecorderSession`] captures, and those older recordings use.
const CALLS: &[(&str, &str, Shape)] = &[
    ("open", "open", Shape::Target),
    ("click", "click", Shape::Target),
    ("type_text", "type_text", Shape::TargetValue),
    ("clear", "clear", Shape::Target),
    ("click_link_text", "click_link_text", Shape::Target),
    ("click_link", "click_link", Shape::Target),
    ("click_partial_link", "click_partial_link", Shape::Target),
    (
        "click_partial_link_text",
        "click_partial_link_text",
        Shape::Target,
    ),
    ("submit", "submit", Shape::Target),
    ("hover", "hover", Shape::Target),
    (
        "select_option_by_text",
        "select_option_by_text",
        Shape::TargetValue,
    ),
    (
        "select_option_by_value",
        "select_option_by_value",
        Shape::TargetValue,
    ),
    (
        "select_option_by_index",
        "select_option_by_index",
        Shape::TargetIndex,
    ),
    ("select_all", "select_all", Shape::Target),
    ("unselect_all", "unselect_all", Shape::Target),
    ("switch_to_frame", "switch_to_frame", Shape::Target),
    (
        "switch_to_default_content",
        "switch_to_default_content",
        Shape::NoArgs,
    ),
    (
        "switch_to_parent_frame",
        "switch_to_parent_frame",
        Shape::NoArgs,
    ),
    ("drag_and_drop", "drag_and_drop", Shape::TargetValue),
    ("cdp_click_element", "cdp_click_element", Shape::Target),
    (
        "wait_for_element_visible",
        "wait_for_element_visible",
        Shape::TargetSeconds,
    ),
    (
        "wait_for_element_absent",
        "wait_for_element_absent",
        Shape::TargetSeconds,
    ),
    (
        "wait_for_element_not_visible",
        "wait_for_element_not_visible",
        Shape::TargetSeconds,
    ),
    (
        "wait_for_element_present",
        "wait_for_element_present",
        Shape::TargetSeconds,
    ),
    (
        "wait_for_element_clickable",
        "wait_for_element_clickable",
        Shape::TargetSeconds,
    ),
    (
        "wait_for_ready_state_complete",
        "wait_for_ready_state_complete",
        Shape::NoArgs,
    ),
    ("assert_text", "assert_text", Shape::ValueTarget),
    (
        "assert_text_visible",
        "assert_text_visible",
        Shape::ValueTarget,
    ),
    (
        "assert_text_not_visible",
        "assert_text_not_visible",
        Shape::ValueTarget,
    ),
    ("assert_exact_text", "assert_exact_text", Shape::ValueTarget),
    ("assert_attribute", "assert_attribute", Shape::TargetPair),
    ("assert_title", "assert_title", Shape::Target),
    (
        "assert_title_contains",
        "assert_title_contains",
        Shape::Target,
    ),
    ("assert_link_text", "assert_link_text", Shape::Target),
    ("maximize_window", "maximize_window", Shape::NoArgs),
    ("set_window_size", "set_window_size", Shape::Size),
    ("switch_to_window", "switch_to_window", Shape::Target),
    (
        "switch_to_new_window",
        "switch_to_new_window",
        Shape::NoArgs,
    ),
    (
        "switch_to_newest_window",
        "switch_to_newest_window",
        Shape::NoArgs,
    ),
    (
        "switch_to_default_window",
        "switch_to_default_window",
        Shape::NoArgs,
    ),
    ("go_back", "go_back", Shape::NoArgs),
    ("go_forward", "go_forward", Shape::NoArgs),
    ("refresh", "refresh", Shape::NoArgs),
    ("delete_all_cookies", "delete_all_cookies", Shape::NoArgs),
    ("delete_cookie", "delete_cookie", Shape::Target),
    ("add_cookie", "add_cookie", Shape::TargetValue),
    ("double_click", "double_click", Shape::Target),
    ("context_click", "context_click", Shape::Target),
    (
        "click_with_offset",
        "click_with_offset",
        Shape::TargetOffset,
    ),
    (
        "double_click_with_offset",
        "double_click_with_offset",
        Shape::TargetOffset,
    ),
    ("scroll_to", "scroll_to", Shape::Target),
    ("scroll_to_bottom", "scroll_to_bottom", Shape::NoArgs),
    ("scroll_to_top", "scroll_to_top", Shape::NoArgs),
    ("scroll_into_view", "scroll_into_view", Shape::Target),
    ("smooth_scroll_to", "smooth_scroll_to", Shape::Target),
    ("slow_scroll_to", "slow_scroll_to", Shape::Target),
    ("check_if_unchecked", "check_if_unchecked", Shape::Target),
    ("uncheck_if_checked", "uncheck_if_checked", Shape::Target),
    ("add_text", "add_text", Shape::TargetValue),
    ("send_keys", "send_keys", Shape::TargetValue),
    ("js_click", "js_click", Shape::Target),
    ("js_type", "js_type", Shape::TargetValue),
    ("js_update_text", "js_update_text", Shape::TargetValue),
    ("js_click_if_present", "js_click_if_present", Shape::Target),
    ("js_click_all", "js_click_all", Shape::Target),
    ("jquery_click", "jquery_click", Shape::Target),
    ("jquery_click_all", "jquery_click_all", Shape::Target),
    ("jquery_type", "jquery_type", Shape::TargetValue),
    (
        "jquery_update_text",
        "jquery_update_text",
        Shape::TargetValue,
    ),
    ("set_attribute", "set_attribute", Shape::TargetPair),
    ("remove_attribute", "remove_attribute", Shape::TargetValue),
    ("choose_file", "choose_file", Shape::TargetValue),
    (
        "wait_for_and_accept_alert",
        "wait_for_and_accept_alert",
        Shape::Seconds,
    ),
    (
        "wait_for_and_dismiss_alert",
        "wait_for_and_dismiss_alert",
        Shape::Seconds,
    ),
    ("accept_alert", "accept_alert", Shape::NoArgs),
    ("dismiss_alert", "dismiss_alert", Shape::NoArgs),
    ("type_alert_text", "type_alert_text", Shape::Target),
    ("clear_local_storage", "clear_local_storage", Shape::NoArgs),
    (
        "remove_local_storage_item",
        "remove_local_storage_item",
        Shape::Target,
    ),
    (
        "set_local_storage_item",
        "set_local_storage_item",
        Shape::TargetValue,
    ),
    ("close_window", "close_window", Shape::NoArgs),
    ("highlight_click", "highlight_click", Shape::Target),
    ("open_new_window", "open_new_window", Shape::NoArgs),
    (
        "click_visible_elements",
        "click_visible_elements",
        Shape::Target,
    ),
    ("find_element", "find_element", Shape::TargetDiscarded),
    ("find_elements", "find_elements", Shape::TargetDiscarded),
    ("get_shadow_root", "get_shadow_root", Shape::TargetDiscarded),
    ("slow_click", "slow_click", Shape::Target),
    ("human_type", "human_type", Shape::TargetValue),
    ("human_click", "human_click", Shape::Target),
    ("uc_click", "uc_click", Shape::Target),
    ("uc_type", "uc_type", Shape::TargetValue),
    ("update_text", "update_text", Shape::TargetValue),
    ("sleep", "sleep", Shape::Sleep),
    ("click_xpath", "click_xpath", Shape::Target),
    // `open_new_tab` is special: with a URL it is `open_new_tab_url`.
    ("open_new_tab", "open_new_tab", Shape::NoArgs),
];

/// Other names for an action in `CALLS`: those `RecorderSession` and older
/// recordings use.
const ALIASES: &[(&str, &str)] = &[
    ("goto", "open"),
    ("navigate", "open"),
    ("type", "type_text"),
];

/// Actions `BaseCase` records without what it takes to run them again.
const UNEXPORTABLE: &[(&str, &str)] = &[(
    "drag_and_drop_with_offset",
    "the recording keeps the two selectors but not the offset",
)];

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
fn canonical_name(recorded: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == recorded)
        .map_or(recorded, |(_, canonical)| *canonical)
}

/// A string as a Rust literal.
fn literal(text: &str) -> String {
    format!("{text:?}")
}

/// A whole number parsed from recorded text, as a Rust literal.
fn number<T: std::str::FromStr + std::fmt::Display>(
    action: &str,
    what: &str,
    text: &str,
) -> Result<String, String> {
    text.trim()
        .parse::<T>()
        .map(|n| n.to_string())
        .map_err(|_| format!("{action}: {what} {text:?} is not a whole number in range"))
}

/// The line of Rust that replays `action`, or why there is none.
fn rust_call(action: &RecordedAction) -> Result<String, String> {
    let name = canonical_name(&action.name);
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

    let target = action.target.as_deref();
    let value = action.value.as_deref();
    let need_target = || target.ok_or_else(|| format!("{name} has no target"));
    let need_value = || value.ok_or_else(|| format!("{name} has no value"));

    let (method, args) = match shape {
        Shape::NoArgs => {
            // `open_new_tab` remembers the page it opened.
            match (*method, target) {
                ("open_new_tab", Some(url)) => ("open_new_tab_url", literal(url)),
                _ => (*method, String::new()),
            }
        }
        Shape::Target | Shape::TargetDiscarded => (*method, literal(need_target()?)),
        Shape::TargetValue => (
            *method,
            format!("{}, {}", literal(need_target()?), literal(need_value()?)),
        ),
        Shape::ValueTarget => (
            *method,
            format!("{}, {}", literal(need_value()?), literal(need_target()?)),
        ),
        Shape::TargetSeconds => (*method, format!("{}, 10", literal(need_target()?))),
        Shape::Seconds => (*method, "10".to_owned()),
        Shape::TargetPair => {
            let pair = need_value()?;
            let (key, content) = pair
                .split_once('=')
                .ok_or_else(|| format!("{name}: value {pair:?} is not name=content"))?;
            (
                *method,
                format!(
                    "{}, {}, {}",
                    literal(need_target()?),
                    literal(key.trim()),
                    literal(content.trim())
                ),
            )
        }
        Shape::Size => {
            let size = need_target()?;
            let (width, height) = size
                .split_once(',')
                .ok_or_else(|| format!("{name}: target {size:?} is not width,height"))?;
            (
                *method,
                format!(
                    "{}, {}",
                    number::<u32>(name, "width", width)?,
                    number::<u32>(name, "height", height)?
                ),
            )
        }
        Shape::TargetOffset => {
            let offset = need_value()?;
            let (x, y) = offset
                .split_once(',')
                .ok_or_else(|| format!("{name}: value {offset:?} is not x, y"))?;
            (
                *method,
                format!(
                    "{}, {}, {}",
                    literal(need_target()?),
                    number::<i64>(name, "x offset", x)?,
                    number::<i64>(name, "y offset", y)?
                ),
            )
        }
        Shape::TargetIndex => (
            *method,
            format!(
                "{}, {}",
                literal(need_target()?),
                number::<usize>(name, "index", need_value()?)?
            ),
        ),
        Shape::Sleep => {
            let text = need_value()?;
            let seconds = text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|s| s.is_finite() && *s >= 0.0)
                .ok_or_else(|| format!("{name}: {text:?} is not a number of seconds"))?;
            // `sleep` returns nothing, so there is no `?`.
            return Ok(format!("sb.{method}({seconds:?}).await;"));
        }
    };

    let discard = if *shape == Shape::TargetDiscarded {
        "let _ = "
    } else {
        ""
    };
    Ok(format!("{discard}sb.{method}({args}).await?;"))
}

fn line(out: &mut String, content: &str) {
    out.push_str("    ");
    out.push_str(content);
    out.push('\n');
}

/// Words that cannot name a function, in any edition (`gen` is reserved from
/// 2024, and the test may be compiled there).
const KEYWORDS: &[&str] = &[
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
    "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// A name made safe to use as a Rust function name.
fn rust_identifier(name: &str) -> String {
    let mut ident: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if ident.is_empty() || ident == "_" {
        return "recorded_test".to_owned();
    }
    if ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    if KEYWORDS.contains(&ident.as_str()) {
        ident.push('_');
    }
    ident
}

/// Result of replaying one [`RecordedAction`].
#[derive(Clone, Debug, Serialize)]
pub struct ReplayStep {
    /// Action name that was replayed.
    pub name: String,
    /// Human-readable description of the replayed step.
    pub description: String,
    /// Error message when the step failed.
    pub error: Option<String>,
}

impl ReplayStep {
    /// Returns `true` when the step completed without an error.
    pub fn passed(&self) -> bool {
        self.error.is_none()
    }
}

/// Summary returned by [`RecorderSession::replay`].
#[derive(Clone, Debug, Default, Serialize)]
pub struct ReplayReport {
    /// Per-action results in replay order.
    pub steps: Vec<ReplayStep>,
    /// Number of actions that were not understood by the replay engine.
    pub skipped: usize,
}

impl ReplayReport {
    /// Returns the number of steps that failed.
    pub fn failed(&self) -> usize {
        self.steps.iter().filter(|step| !step.passed()).count()
    }

    /// Returns the number of steps that passed.
    pub fn passed(&self) -> usize {
        self.steps.iter().filter(|step| step.passed()).count()
    }

    /// Returns `true` when every replayed step succeeded.
    pub fn is_success(&self) -> bool {
        self.failed() == 0
    }
}

/// Live recording session that captures browser interactions while driving a
/// [`BaseCase`](crate::BaseCase).
///
/// The session can be driven programmatically (see [`RecorderSession::goto`],
/// [`RecorderSession::click`], [`RecorderSession::type_text`] and
/// [`RecorderSession::assert_text`]) or attached to a live page with
/// [`RecorderSession::install`] + [`RecorderSession::drain`] so that manual user
/// interactions are captured too.
///
/// ```
/// use seleniumbase_rs::api::recorder::RecorderSession;
///
/// let mut session = RecorderSession::new();
/// session.push("open", Some("https://example.com"), None);
/// session.push("click", Some("#submit"), None);
/// assert_eq!(session.actions().len(), 2);
/// assert!(session.to_json().unwrap().contains("https://example.com"));
/// assert!(session.to_yaml().contains("click"));
/// ```
#[derive(Clone, Debug, Default, Serialize)]
pub struct RecorderSession {
    actions: Vec<RecordedAction>,
    last_url: Option<String>,
    #[serde(skip)]
    installed: bool,
    /// Whether the listeners are also installed in every document the browser
    /// loads from now on, not just the current one.
    #[serde(skip)]
    on_new_documents: bool,
}

impl RecorderSession {
    /// Creates an empty recording session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a session pre-populated with previously recorded actions.
    pub fn from_actions(actions: Vec<RecordedAction>) -> Self {
        Self {
            actions,
            last_url: None,
            installed: false,
            on_new_documents: false,
        }
    }

    /// Returns the recorded actions in capture order.
    pub fn actions(&self) -> &[RecordedAction] {
        &self.actions
    }

    /// Consumes the session and returns the recorded actions.
    pub fn into_actions(self) -> Vec<RecordedAction> {
        self.actions
    }

    /// Returns `true` when nothing has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Returns the number of recorded actions.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Appends an action to the recording without touching the browser.
    pub fn push(&mut self, name: &str, target: Option<&str>, value: Option<&str>) {
        self.actions.push(RecordedAction::new(name, target, value));
    }

    /// Converts the recording into an [`ActionRecorder`] so it can be exported
    /// as Rust source.
    pub fn to_action_recorder(&self) -> ActionRecorder {
        ActionRecorder {
            actions: self.actions.clone(),
        }
    }

    /// Serializes the recording as pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, SeleniumBaseError> {
        serde_json::to_string_pretty(&self.actions).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to serialize recording: {e}"))
        })
    }

    /// Serializes the recording as a YAML action list.
    pub fn to_yaml(&self) -> String {
        if self.actions.is_empty() {
            return "[]\n".to_owned();
        }
        let mut out = String::new();
        for action in &self.actions {
            out.push_str(&format!("- name: {}\n", yaml_scalar(&action.name)));
            if let Some(target) = &action.target {
                out.push_str(&format!("  target: {}\n", yaml_scalar(target)));
            }
            if let Some(value) = &action.value {
                out.push_str(&format!("  value: {}\n", yaml_scalar(value)));
            }
            out.push_str(&format!("  timestamp_ms: {}\n", action.timestamp_ms));
        }
        out
    }

    /// Writes the recording to `path`. A `.yaml`/`.yml` extension selects YAML,
    /// anything else writes JSON.
    pub fn save(&self, path: &Path) -> Result<PathBuf, SeleniumBaseError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = if is_yaml_path(path) {
            self.to_yaml()
        } else {
            self.to_json()?
        };
        std::fs::write(path, data)?;
        Ok(path.to_path_buf())
    }

    /// Loads a recorded action list from a file written by [`Self::save`].
    ///
    /// A `.yaml`/`.yml` extension selects YAML, anything else JSON, the same
    /// choice [`save`](Self::save) makes.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the file cannot be read, and
    /// [`SeleniumBaseError::InvalidConfig`] if it is not a recording.
    pub fn load(path: &Path) -> Result<Vec<RecordedAction>, SeleniumBaseError> {
        let raw = std::fs::read_to_string(path)?;
        if is_yaml_path(path) {
            Self::parse_yaml(&raw)
        } else {
            Self::parse_json(&raw)
        }
    }

    /// Loads a recorded action list from a file written by [`Self::save`].
    ///
    /// The name is from when only JSON was written. It now does what
    /// [`load`](Self::load) does, so a `.yaml` recording replays too.
    ///
    /// # Errors
    ///
    /// See [`load`](Self::load).
    pub fn load_json(path: &Path) -> Result<Vec<RecordedAction>, SeleniumBaseError> {
        Self::load(path)
    }

    /// Parses the YAML that [`to_yaml`](Self::to_yaml) writes.
    ///
    /// Only that format is understood: a list of items with `name`, `target`,
    /// `value` and `timestamp_ms` keys, whose strings are double-quoted. It is
    /// not a general YAML parser.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] naming the line that is
    /// not understood.
    pub fn parse_yaml(raw: &str) -> Result<Vec<RecordedAction>, SeleniumBaseError> {
        let bad = |number: usize, why: &str| {
            SeleniumBaseError::InvalidConfig(format!("recording line {number}: {why}"))
        };
        let mut actions: Vec<RecordedAction> = Vec::new();
        for (index, text) in raw.lines().enumerate() {
            let number = index + 1;
            let trimmed = text.trim_end();
            if trimmed.is_empty() || trimmed.trim_start().starts_with('#') || trimmed == "[]" {
                continue;
            }
            let entry = if let Some(rest) = trimmed.strip_prefix("- ") {
                actions.push(RecordedAction {
                    name: String::new(),
                    target: None,
                    value: None,
                    timestamp_ms: 0,
                });
                rest
            } else if trimmed.starts_with("  ") {
                trimmed.trim_start()
            } else {
                return Err(bad(number, "expected `- name: ...` or an indented key"));
            };
            let Some(action) = actions.last_mut() else {
                return Err(bad(number, "a key before the first `- ` item"));
            };
            let (key, scalar) = entry
                .split_once(": ")
                .ok_or_else(|| bad(number, "expected `key: value`"))?;
            let string = || {
                serde_json::from_str::<Option<String>>(scalar)
                    .map_err(|_| bad(number, "expected a double-quoted string"))
            };
            match key {
                "name" => action.name = string()?.unwrap_or_default(),
                "target" => action.target = string()?,
                "value" => action.value = string()?,
                "timestamp_ms" => {
                    action.timestamp_ms = scalar
                        .parse()
                        .map_err(|_| bad(number, "expected a whole number"))?;
                }
                other => return Err(bad(number, &format!("unknown key `{other}`"))),
            }
        }
        if let Some(position) = actions.iter().position(|action| action.name.is_empty()) {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "recording item {} has no name",
                position + 1
            )));
        }
        Ok(actions)
    }

    /// Parses a recorded action list from JSON text.
    ///
    /// Both a bare array and an object with an `actions` key are accepted.
    pub fn parse_json(raw: &str) -> Result<Vec<RecordedAction>, SeleniumBaseError> {
        if let Ok(actions) = serde_json::from_str::<Vec<RecordedAction>>(raw) {
            return Ok(actions);
        }
        let recorder: ActionRecorder = serde_json::from_str(raw).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse recorded actions: {e}"))
        })?;
        Ok(recorder.actions)
    }

    /// JavaScript that installs the in-page event listeners.
    ///
    /// The script is idempotent: running it again on the same page does not
    /// duplicate the listeners. What the listeners capture is kept in the
    /// browser's `sessionStorage`, so it survives a navigation to another page
    /// of the same origin. Events not yet drained when the browser moves to a
    /// different origin stay behind, as storage is kept per origin.
    pub fn install_script() -> &'static str {
        INSTALL_SCRIPT
    }

    /// JavaScript that returns and clears the captured in-page events.
    pub fn drain_script() -> &'static str {
        DRAIN_SCRIPT
    }

    /// Parses the JSON payload produced by [`Self::drain_script`].
    pub fn parse_drained(raw: &str) -> Vec<RecordedAction> {
        Self::parse_captured(raw)
            .into_iter()
            .map(|event| event.action())
            .collect()
    }

    fn parse_captured(raw: &str) -> Vec<Captured> {
        serde_json::from_str(raw).unwrap_or_default()
    }
}

/// One event as the in-page listeners report it: a [`RecordedAction`] and the
/// address of the page it happened on.
#[derive(Deserialize)]
struct Captured {
    name: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    timestamp_ms: u64,
    #[serde(default)]
    url: Option<String>,
}

impl Captured {
    fn action(self) -> RecordedAction {
        RecordedAction {
            name: self.name,
            target: self.target,
            value: self.value,
            timestamp_ms: u128::from(self.timestamp_ms),
        }
    }
}

/// Browser-driving helpers. These require a live [`BaseCase`].
impl RecorderSession {
    /// Navigates to `url` and records a `goto` action.
    pub async fn goto(&mut self, sb: &mut BaseCase, url: &str) -> Result<(), SeleniumBaseError> {
        // Flush pending in-page events first so the recording stays ordered.
        if self.installed {
            let _ = self.drain(sb).await;
        }
        sb.open(url).await?;
        // Remember the address the browser is at, not the one it was asked
        // for: `https://example.com` becomes `https://example.com/`, and a
        // redirect changes it altogether. Otherwise the next `drain` sees a
        // difference and records a second `goto` for the same page.
        self.last_url = Some(
            sb.get_current_url()
                .await
                .unwrap_or_else(|_| url.to_owned()),
        );
        self.push("goto", Some(url), None);
        if self.installed {
            let _ = self.install(sb).await;
        }
        Ok(())
    }

    /// Clicks `css` and records a `click` action.
    ///
    /// Skips the local record when in-page capture is active, because the
    /// installed listeners already observe the click.
    pub async fn click(&mut self, sb: &mut BaseCase, css: &str) -> Result<(), SeleniumBaseError> {
        sb.click(css).await?;
        if !self.installed {
            self.push("click", Some(css), None);
        }
        Ok(())
    }

    /// Types `text` into `css` and records a `type` action.
    pub async fn type_text(
        &mut self,
        sb: &mut BaseCase,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        sb.type_text(css, text).await?;
        if !self.installed {
            self.push("type", Some(css), Some(text));
        }
        Ok(())
    }

    /// Asserts that `css` contains `text` and records an `assert_text` action.
    /// The text comes first and the selector second, like
    /// [`BaseCase::assert_text`].
    pub async fn assert_text(
        &mut self,
        sb: &mut BaseCase,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        // Flush pending in-page events first so the recording stays ordered.
        if self.installed {
            let _ = self.drain(sb).await;
        }
        sb.assert_text(text, css).await?;
        self.push("assert_text", Some(css), Some(text));
        Ok(())
    }

    /// Installs the in-page listeners so manual interactions are captured.
    ///
    /// While installed, [`RecorderSession::click`] and
    /// [`RecorderSession::type_text`] stop recording locally so each
    /// interaction is captured exactly once.
    ///
    /// In CDP mode (`DriverMode::Cdp` or `DriverMode::Uc`) the listeners are
    /// also registered to be installed in every page the browser loads from
    /// now on, so an interaction right after a navigation is captured too. In
    /// any other session that registration is skipped, and a new page gets the
    /// listeners at the next call to [`drain`](Self::drain): an interaction
    /// before that is not captured.
    pub async fn install(&mut self, sb: &BaseCase) -> Result<(), SeleniumBaseError> {
        sb.execute_script(Self::install_script()).await?;
        if !self.on_new_documents {
            // A frame has its own document; only the page itself is recorded,
            // as the selectors are relative to it.
            let source = format!(
                "(function () {{\nif (window.top !== window) {{ return; }}\n{INSTALL_SCRIPT}\n}})();"
            );
            self.on_new_documents = sb
                .execute_cdp_with_params(
                    "Page.addScriptToEvaluateOnNewDocument",
                    json!({ "source": source }),
                )
                .await
                .is_ok();
        }
        self.installed = true;
        Ok(())
    }

    /// Returns `true` once the in-page listeners have been installed.
    pub fn is_installed(&self) -> bool {
        self.installed
    }

    /// Drains buffered in-page events into the recording.
    ///
    /// Also reinstalls the listeners when a navigation wiped them and records a
    /// `goto` action whenever the page URL changed since the last drain.
    pub async fn drain(&mut self, sb: &mut BaseCase) -> Result<usize, SeleniumBaseError> {
        let before = self.actions.len();
        let raw = sb.execute_script(Self::drain_script()).await;
        let captured = match raw {
            Ok(value) => Self::parse_captured(value.as_str().unwrap_or_default()),
            Err(_) => Vec::new(),
        };

        // Each event knows the page it happened on. Seeing a new address means
        // the browser went there first, so the `goto` goes before the event.
        for event in captured {
            if let Some(url) = event.url.clone() {
                self.note_page(&url);
            }
            self.actions.push(event.action());
        }

        // A navigation with no event after it still needs its `goto`, and the
        // page needs the listeners again.
        if let Ok(url) = sb.get_current_url().await {
            self.note_page(&url);
        }
        let _ = self.install(sb).await;

        Ok(self.actions.len() - before)
    }

    /// Records a `goto` if the browser is now on a page other than the last one
    /// the recording knows.
    fn note_page(&mut self, url: &str) {
        if self.last_url.as_deref() != Some(url) {
            self.last_url = Some(url.to_owned());
            self.actions
                .push(RecordedAction::new("goto", Some(url), None));
        }
    }

    /// Replays `actions` against `sb`.
    ///
    /// Supported action names (with aliases) are `goto`/`open`/`navigate`,
    /// `click`, `type`/`type_text`/`update_text`, `assert_text`,
    /// `assert_title`, `assert_url`, and `select_option_by_value`. Anything else
    /// is counted as skipped.
    pub async fn replay(
        sb: &mut BaseCase,
        actions: &[RecordedAction],
    ) -> Result<ReplayReport, SeleniumBaseError> {
        let mut report = ReplayReport::default();
        for action in actions {
            let target = action.target.as_deref();
            let value = action.value.as_deref();
            let (description, result) = match action.name.as_str() {
                "goto" | "open" | "navigate" => match target {
                    Some(url) => (format!("goto {url}"), sb.open(url).await),
                    None => {
                        report.skipped += 1;
                        continue;
                    }
                },
                "click" => match target {
                    Some(css) => (format!("click {css}"), sb.click(css).await),
                    None => {
                        report.skipped += 1;
                        continue;
                    }
                },
                "type" | "type_text" | "update_text" => match (target, value) {
                    (Some(css), Some(text)) => (
                        format!("type {text:?} into {css}"),
                        sb.type_text(css, text).await,
                    ),
                    _ => {
                        report.skipped += 1;
                        continue;
                    }
                },
                "select_option_by_value" => match (target, value) {
                    (Some(css), Some(val)) => (
                        format!("select {val:?} in {css}"),
                        sb.select_option_by_value(css, val).await,
                    ),
                    _ => {
                        report.skipped += 1;
                        continue;
                    }
                },
                "assert_text" => match (target, value) {
                    (Some(css), Some(text)) => (
                        format!("assert_text {text:?} in {css}"),
                        sb.assert_text(text, css).await,
                    ),
                    _ => {
                        report.skipped += 1;
                        continue;
                    }
                },
                "assert_title" => match target {
                    Some(title) => (
                        format!("assert_title {title:?}"),
                        sb.assert_title_contains(title).await,
                    ),
                    None => {
                        report.skipped += 1;
                        continue;
                    }
                },
                "assert_url" => match target {
                    Some(url) => (
                        format!("assert_url {url:?}"),
                        sb.assert_url_contains(url).await,
                    ),
                    None => {
                        report.skipped += 1;
                        continue;
                    }
                },
                _ => {
                    report.skipped += 1;
                    continue;
                }
            };
            report.steps.push(ReplayStep {
                name: action.name.clone(),
                description,
                error: result.err().map(|e| e.to_string()),
            });
        }
        Ok(report)
    }
}

const INSTALL_SCRIPT: &str = r##"
if (window.__sbRecorderInstalled) { return "already-installed"; }
window.__sbRecorderInstalled = true;
window.__sbRecordedActions = window.__sbRecordedActions || [];
var KEY = "__sbRecordedActions";
var push = function (name, target, value) {
    var event = {
        name: name, target: target, value: value,
        timestamp_ms: Date.now(), url: window.location.href
    };
    // sessionStorage outlives the page: a click that leaves it, or a field
    // changed just before, would be lost with a script variable. A page that
    // forbids storage (about:blank, data: URLs) falls back to the variable.
    try {
        var stored = JSON.parse(window.sessionStorage.getItem(KEY) || "[]");
        stored.push(event);
        window.sessionStorage.setItem(KEY, JSON.stringify(stored));
    } catch (e) {
        window.__sbRecordedActions.push(event);
    }
};
var cssPath = function (el) {
    if (!el || el.nodeType !== 1) { return null; }
    if (el.id) { return "#" + CSS.escape(el.id); }
    var parts = [];
    var node = el;
    while (node && node.nodeType === 1 && parts.length < 6) {
        var part = node.tagName.toLowerCase();
        if (node.id) { parts.unshift("#" + CSS.escape(node.id)); break; }
        var parent = node.parentNode;
        if (parent) {
            var siblings = Array.prototype.filter.call(
                parent.children, function (c) { return c.tagName === node.tagName; }
            );
            if (siblings.length > 1) {
                part += ":nth-of-type(" + (siblings.indexOf(node) + 1) + ")";
            }
        }
        parts.unshift(part);
        node = node.parentNode;
        if (!node || node.nodeType !== 1 || node.tagName.toLowerCase() === "html") { break; }
    }
    return parts.join(" > ");
};
var isTextInput = function (el) {
    if (!el || !el.tagName) { return false; }
    var tag = el.tagName.toLowerCase();
    if (tag === "textarea") { return true; }
    if (tag !== "input") { return false; }
    var type = (el.getAttribute("type") || "text").toLowerCase();
    return ["text", "search", "email", "url", "tel", "password", "number"].indexOf(type) >= 0;
};
document.addEventListener("click", function (e) {
    if (isTextInput(e.target)) { return; }
    var selector = cssPath(e.target);
    if (selector) { push("click", selector, null); }
}, true);
document.addEventListener("change", function (e) {
    var el = e.target;
    var selector = cssPath(el);
    if (!selector || !el || !el.tagName) { return; }
    var tag = el.tagName.toLowerCase();
    if (tag === "select") {
        push("select_option_by_value", selector, el.value);
    } else if (isTextInput(el)) {
        push("type", selector, el.value);
    }
}, true);
return "installed";
"##;

const DRAIN_SCRIPT: &str = r##"
var KEY = "__sbRecordedActions";
var actions = [];
try {
    actions = JSON.parse(window.sessionStorage.getItem(KEY) || "[]");
    window.sessionStorage.removeItem(KEY);
} catch (e) {
    actions = [];
}
var loose = window.__sbRecordedActions || [];
window.__sbRecordedActions = [];
return JSON.stringify(actions.concat(loose));
"##;

fn yaml_scalar(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
}

/// Whether `path` names a YAML file, by its extension.
fn is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recorder() -> ActionRecorder {
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com"), None);
        recorder.record("click", Some("#login"), None);
        recorder.record("type_text", Some("#username"), Some("user"));
        recorder.record("add_text", Some("#username"), Some("@example.com"));
        recorder.record("clear", Some("#search"), None);
        recorder.record("submit", Some("#form"), None);
        recorder.record("hover", Some("#menu"), None);
        recorder.record(
            "select_option_by_text",
            Some("#country"),
            Some("United States"),
        );
        recorder.record("select_option_by_value", Some("#country"), Some("us"));
        recorder.record("switch_to_frame", Some("#frame"), None);
        recorder.record("switch_to_default_content", None, None);
        recorder.record("drag_and_drop", Some("#source"), Some("#target"));
        recorder.record("cdp_click_element", Some("#shadow-btn"), None);
        recorder.record("wait_for_element_visible", Some("#result"), None);
        recorder.record("wait_for_element_absent", Some("#spinner"), None);
        recorder.record("assert_text", Some("#msg"), Some("Welcome"));
        recorder.record("assert_text_visible", Some("#msg"), Some("Welcome"));
        recorder.record("assert_text_not_visible", Some("#error"), Some("Failed"));
        recorder.record("assert_attribute", Some("#logo"), Some("src=/logo.png"));
        recorder.record("assert_title", Some("Example"), None);
        recorder.record("assert_link_text", Some("Home"), None);
        recorder.record("maximize_window", None, None);
        recorder.record("switch_to_window", Some("handle-123"), None);
        recorder.record("switch_to_new_window", None, None);
        recorder.record("go_back", None, None);
        recorder.record("go_forward", None, None);
        recorder.record("refresh", None, None);
        recorder.record("delete_all_cookies", None, None);
        recorder.record("double_click", Some("#item"), None);
        recorder.record("context_click", Some("#item"), None);
        recorder.record("scroll_to", Some("#footer"), None);
        recorder.record("scroll_to_bottom", None, None);
        recorder.record("scroll_to_top", None, None);
        recorder.record("check_if_unchecked", Some("#agree"), None);
        recorder.record("uncheck_if_checked", Some("#spam"), None);
        recorder.record("send_keys", Some("#search"), Some("hello"));
        recorder.record("js_click", Some("#btn"), None);
        recorder.record("js_type", Some("#hidden"), Some("value"));
        recorder.record("set_attribute", Some("#x"), Some("data-id=42"));
        recorder.record("choose_file", Some("#upload"), Some("/tmp/file.txt"));
        recorder.record("click_partial_link_text", Some("Terms"), None);
        recorder.record("wait_for_and_accept_alert", None, None);
        recorder.record("wait_for_and_dismiss_alert", None, None);
        recorder.record("click_link_text", Some("Logout"), None);
        recorder.record("set_window_size", Some("1280,720"), None);
        recorder.record("add_cookie", Some("session"), Some("abc"));
        recorder.record("delete_cookie", Some("session"), None);
        recorder.record("open_new_tab", None, None);
        recorder.record("click_visible_elements", Some(".btn"), None);
        recorder.record("find_element", Some("#x"), None);
        recorder.record("find_elements", Some(".x"), None);
        recorder.record("get_shadow_root", Some("#host"), None);
        recorder.record("slow_click", Some("#y"), None);
        recorder.record("human_type", Some("#z"), Some("abc"));
        recorder.record("human_click", Some("#z"), None);
        recorder.record("update_text", Some("#w"), Some("new"));
        recorder.record("click_xpath", Some("//button"), None);
        recorder.record("js_click_all", Some(".check"), None);
        recorder.record("jquery_click", Some(".jq"), None);
        recorder
    }

    #[test]
    fn generated_rust_script_is_syntactically_valid() {
        let source = sample_recorder().to_rust_script();
        syn::parse_file(&source).expect("generated main script should parse as Rust");
    }

    #[test]
    fn generated_rust_test_is_syntactically_valid() {
        let recorder = sample_recorder();
        let source = recorder.to_rust_test("login_flow_test");
        assert!(source.contains("#[tokio::test]"));
        assert!(source.contains("async fn login_flow_test()"));
        syn::parse_file(&source).expect("generated test should parse as Rust");
    }

    #[test]
    fn save_recording_as_test_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = sample_recorder()
            .save_recording_as_test(dir.path(), "sample_test")
            .unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("async fn sample_test()"));
    }

    #[test]
    fn rust_identifier_sanitizes_names() {
        assert_eq!(rust_identifier("my test"), "my_test");
        assert_eq!(rust_identifier("123-test"), "_123_test");
        assert_eq!(rust_identifier(""), "recorded_test");
        assert_eq!(rust_identifier("_"), "recorded_test");
    }

    #[test]
    fn rust_identifier_only_uses_characters_a_rust_name_may_have() {
        // `char::is_alphanumeric` accepts all of these; a Rust name does not.
        assert_eq!(rust_identifier("test\u{b2}"), "test_");
        assert_eq!(rust_identifier("na\u{ef}ve"), "na_ve");
        assert_eq!(rust_identifier("\u{4e2d}\u{6587}"), "__");
    }

    #[test]
    fn rust_identifier_does_not_produce_a_keyword() {
        for keyword in [
            "fn", "match", "async", "type", "gen", "Self", "self", "crate",
        ] {
            let ident = rust_identifier(keyword);
            assert_eq!(ident, format!("{keyword}_"));
            syn::parse_str::<syn::Ident>(&ident)
                .unwrap_or_else(|e| panic!("{ident} is not a valid name: {e}"));
        }
        // And a test named after a keyword still generates a function that parses.
        let source = ActionRecorder::default().to_rust_test("fn");
        syn::parse_file(&source).expect("a test named after a keyword parses");
    }

    fn sample_session() -> RecorderSession {
        let mut session = RecorderSession::new();
        session.push("goto", Some("https://example.com"), None);
        session.push("click", Some("#submit"), None);
        session.push("type", Some("#name"), Some("Ada \"A\" Lovelace"));
        session.push("assert_text", Some("#msg"), Some("Welcome"));
        session
    }

    #[test]
    fn recorder_session_tracks_actions() {
        let session = sample_session();
        assert_eq!(session.len(), 4);
        assert!(!session.is_empty());
        assert_eq!(session.actions()[0].name, "goto");
        assert_eq!(RecorderSession::new().len(), 0);
        assert!(RecorderSession::new().is_empty());
    }

    #[test]
    fn recorder_session_json_round_trips() {
        let session = sample_session();
        let json = session.to_json().unwrap();
        let parsed = RecorderSession::parse_json(&json).unwrap();
        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[2].value.as_deref(), Some("Ada \"A\" Lovelace"));
    }

    #[test]
    fn recorder_session_parses_wrapped_json() {
        let raw = r#"{"actions":[{"name":"goto","target":"https://example.com"}]}"#;
        let parsed = RecorderSession::parse_json(raw).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "goto");
        assert_eq!(parsed[0].timestamp_ms, 0);
    }

    #[test]
    fn recorder_session_rejects_invalid_json() {
        assert!(RecorderSession::parse_json("not json").is_err());
    }

    #[test]
    fn recorder_session_emits_yaml() {
        let yaml = sample_session().to_yaml();
        assert!(yaml.contains("- name: \"goto\""));
        assert!(yaml.contains("  target: \"#submit\""));
        assert!(yaml.contains("timestamp_ms:"));
        assert_eq!(RecorderSession::new().to_yaml(), "[]\n");
    }

    #[test]
    fn recorder_session_saves_json_and_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let session = sample_session();

        let json_path = session
            .save(&dir.path().join("nested").join("actions.json"))
            .unwrap();
        assert!(json_path.exists());
        assert_eq!(RecorderSession::load_json(&json_path).unwrap().len(), 4);

        let yaml_path = session.save(&dir.path().join("actions.yaml")).unwrap();
        let yaml = std::fs::read_to_string(&yaml_path).unwrap();
        assert!(yaml.starts_with("- name: \"goto\""));
    }

    #[test]
    fn recorder_session_converts_to_action_recorder() {
        let recorder = sample_session().to_action_recorder();
        let script = recorder.to_rust_script();
        // `goto` and `type` are what `RecorderSession` records; the generated
        // program must navigate and type, not silently drop them.
        assert!(
            script.contains("sb.open(\"https://example.com\").await?;"),
            "{script}"
        );
        assert!(script.contains("sb.click(\"#submit\")"));
        assert!(
            script.contains("sb.type_text(\"#name\", \"Ada \\\"A\\\" Lovelace\").await?;"),
            "{script}"
        );
        assert!(script.contains("sb.assert_text(\"Welcome\", \"#msg\")"));
        assert!(!script.contains("skipped"), "{script}");
        syn::parse_file(&script).expect("generated script should parse");
    }

    #[test]
    fn recorder_session_from_actions_preserves_order() {
        let actions = sample_session().into_actions();
        let session = RecorderSession::from_actions(actions);
        assert_eq!(session.actions()[3].name, "assert_text");
    }

    #[test]
    fn drained_payload_is_parsed() {
        let raw = r##"[{"name":"click","target":"#a","value":null,"timestamp_ms":7}]"##;
        let actions = RecorderSession::parse_drained(raw);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].timestamp_ms, 7);
        assert!(RecorderSession::parse_drained("boom").is_empty());
    }

    #[test]
    fn injection_scripts_are_present() {
        assert!(RecorderSession::install_script().contains("__sbRecorderInstalled"));
        assert!(RecorderSession::install_script().contains("addEventListener(\"click\""));
        assert!(RecorderSession::drain_script().contains("__sbRecordedActions"));
    }

    #[test]
    fn replay_report_counts_results() {
        let report = ReplayReport {
            steps: vec![
                ReplayStep {
                    name: "click".to_owned(),
                    description: "click #a".to_owned(),
                    error: None,
                },
                ReplayStep {
                    name: "assert_text".to_owned(),
                    description: "assert_text".to_owned(),
                    error: Some("boom".to_owned()),
                },
            ],
            skipped: 1,
        };
        assert_eq!(report.passed(), 1);
        assert_eq!(report.failed(), 1);
        assert!(!report.is_success());
        assert!(ReplayReport::default().is_success());
    }

    // ---- code generation ---------------------------------------------------

    /// What `BaseCase` accepts: each method's name and its parameter types
    /// (`self` left out), read from the source of every `impl BaseCase`.
    fn basecase_methods() -> std::collections::HashMap<String, Vec<String>> {
        fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    rust_files(&path, found);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    found.push(path);
                }
            }
        }
        fn type_name(ty: &syn::Type) -> String {
            match ty {
                syn::Type::Reference(reference) => format!("&{}", type_name(&reference.elem)),
                syn::Type::Path(path) => path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .unwrap_or_default(),
                _ => "other".to_owned(),
            }
        }

        let mut files = Vec::new();
        rust_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api"),
            &mut files,
        );
        let mut methods = std::collections::HashMap::new();
        for file in files {
            let source = std::fs::read_to_string(&file).unwrap();
            // A file that is only part of an item (none today) is skipped.
            let Ok(syntax) = syn::parse_file(&source) else {
                continue;
            };
            for item in syntax.items {
                let syn::Item::Impl(implementation) = item else {
                    continue;
                };
                let is_base_case = implementation.trait_.is_none()
                    && matches!(&*implementation.self_ty,
                        syn::Type::Path(path) if path.path.is_ident("BaseCase"));
                if !is_base_case {
                    continue;
                }
                for member in implementation.items {
                    let syn::ImplItem::Fn(function) = member else {
                        continue;
                    };
                    let parameters = function
                        .sig
                        .inputs
                        .iter()
                        .filter_map(|argument| match argument {
                            syn::FnArg::Typed(typed) => Some(type_name(&typed.ty)),
                            syn::FnArg::Receiver(_) => None,
                        })
                        .collect();
                    methods.insert(function.sig.ident.to_string(), parameters);
                }
            }
        }
        methods
    }

    /// The `target` and `value` that make `shape` generate a call.
    fn sample_for(shape: Shape) -> (Option<&'static str>, Option<&'static str>) {
        match shape {
            Shape::NoArgs | Shape::Seconds => (None, None),
            Shape::Target | Shape::TargetDiscarded | Shape::TargetSeconds => (Some("#t"), None),
            Shape::TargetValue | Shape::ValueTarget => (Some("#t"), Some("v")),
            Shape::TargetPair => (Some("#t"), Some("name=content")),
            Shape::Size => (Some("800,600"), None),
            Shape::TargetOffset => (Some("#t"), Some("5, -7")),
            Shape::TargetIndex => (Some("#t"), Some("2")),
            Shape::Sleep => (None, Some("1.5")),
        }
    }

    /// The method and the arguments of the `sb.method(args).await?;` on a line.
    fn parse_call(line: &str) -> (String, Vec<syn::Expr>) {
        let code = line
            .trim()
            .trim_start_matches("let _ = ")
            .trim_end_matches(';');
        let expr: syn::Expr = syn::parse_str(code).unwrap_or_else(|e| panic!("{line}: {e}"));
        // Only a method that cannot fail (`sleep`) is awaited without a `?`.
        let awaited = match expr {
            syn::Expr::Try(attempt) => *attempt.expr,
            other => other,
        };
        let syn::Expr::Await(awaited) = awaited else {
            panic!("{line} does not `.await`");
        };
        let syn::Expr::MethodCall(call) = *awaited.base else {
            panic!("{line} is not a method call");
        };
        assert!(
            matches!(&*call.receiver, syn::Expr::Path(path) if path.path.is_ident("sb")),
            "{line} is not called on `sb`"
        );
        (call.method.to_string(), call.args.into_iter().collect())
    }

    fn literal_kind(expr: &syn::Expr) -> &'static str {
        match expr {
            syn::Expr::Lit(literal) => match &literal.lit {
                syn::Lit::Str(_) => "string",
                syn::Lit::Int(_) => "integer",
                syn::Lit::Float(_) => "float",
                _ => "other",
            },
            // A negative number is a unary minus on a literal.
            syn::Expr::Unary(unary) => literal_kind(&unary.expr),
            _ => "other",
        }
    }

    fn calls_in(script: &str) -> Vec<&str> {
        script
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("sb.") || line.starts_with("let _ = sb."))
            .filter(|line| !line.starts_with("sb.quit"))
            .collect()
    }

    #[test]
    fn every_generated_call_is_a_basecase_method_taking_those_arguments() {
        let methods = basecase_methods();
        assert!(
            methods.len() > 100,
            "read only {} BaseCase methods; is the oracle broken?",
            methods.len()
        );

        let mut recorder = ActionRecorder::default();
        for (name, _, shape) in CALLS {
            let (target, value) = sample_for(*shape);
            recorder.record(name, target, value);
        }
        recorder.record("open_new_tab", Some("https://example.com"), None);
        let script = recorder.to_rust_script();
        assert!(!script.contains("skipped"), "{script}");

        let calls = calls_in(&script);
        assert_eq!(calls.len(), CALLS.len() + 1, "one call per action");

        let mut problems = Vec::new();
        for line in calls {
            let (method, arguments) = parse_call(line);
            let Some(parameters) = methods.get(&method) else {
                problems.push(format!("{line}: BaseCase has no method `{method}`"));
                continue;
            };
            if parameters.len() != arguments.len() {
                problems.push(format!(
                    "{line}: `{method}` takes {} argument(s) {parameters:?}, the call passes {}",
                    parameters.len(),
                    arguments.len()
                ));
                continue;
            }
            for (parameter, argument) in parameters.iter().zip(&arguments) {
                let kind = literal_kind(argument);
                let fits = match parameter.as_str() {
                    "&str" => kind == "string",
                    "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64"
                    | "isize" => kind == "integer",
                    "f64" => kind == "float",
                    _ => false,
                };
                if !fits {
                    problems.push(format!(
                        "{line}: `{method}` wants a {parameter}, the call passes a {kind}"
                    ));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    fn call(name: &str, target: Option<&str>, value: Option<&str>) -> Result<String, String> {
        rust_call(&RecordedAction::new(name, target, value))
    }

    #[test]
    fn the_table_has_no_duplicates_and_its_aliases_point_into_it() {
        for (index, (name, _, _)) in CALLS.iter().enumerate() {
            assert!(
                CALLS[index + 1..].iter().all(|(other, _, _)| other != name),
                "{name} is listed twice"
            );
        }
        for (alias, canonical) in ALIASES {
            assert!(
                CALLS.iter().any(|(name, _, _)| name == canonical),
                "{alias} points at {canonical}, which is not generated"
            );
            assert!(
                CALLS.iter().all(|(name, _, _)| name != alias),
                "{alias} is both an alias and an action"
            );
        }
        for (name, _) in UNEXPORTABLE {
            assert!(CALLS.iter().all(|(known, _, _)| known != name));
        }
    }

    #[test]
    fn what_a_recorder_session_records_is_what_the_generator_turns_into_code() {
        // `goto` and `type` were dropped from the generated program.
        assert_eq!(
            call("goto", Some("https://example.com"), None).unwrap(),
            "sb.open(\"https://example.com\").await?;"
        );
        assert_eq!(
            call("navigate", Some("https://example.com"), None).unwrap(),
            "sb.open(\"https://example.com\").await?;"
        );
        assert_eq!(
            call("type", Some("#q"), Some("rust")).unwrap(),
            "sb.type_text(\"#q\", \"rust\").await?;"
        );
        assert_eq!(
            call("select_option_by_value", Some("#c"), Some("us")).unwrap(),
            "sb.select_option_by_value(\"#c\", \"us\").await?;"
        );
    }

    #[test]
    fn the_actions_basecase_records_are_generated_with_the_arguments_it_recorded() {
        assert_eq!(
            call("open_new_tab", Some("https://example.com/x"), None).unwrap(),
            "sb.open_new_tab_url(\"https://example.com/x\").await?;"
        );
        assert_eq!(
            call("open_new_tab", None, None).unwrap(),
            "sb.open_new_tab().await?;"
        );
        assert_eq!(
            call("click_with_offset", Some("#a"), Some("10, -4")).unwrap(),
            "sb.click_with_offset(\"#a\", 10, -4).await?;"
        );
        assert_eq!(
            call("select_option_by_index", Some("#s"), Some("2")).unwrap(),
            "sb.select_option_by_index(\"#s\", 2).await?;"
        );
        assert_eq!(
            call("click_partial_link", Some("Terms"), None).unwrap(),
            "sb.click_partial_link(\"Terms\").await?;"
        );
    }

    #[test]
    fn text_assertions_pass_their_arguments_in_the_order_the_methods_take_them() {
        // The text comes first and the selector second, as in Python.
        assert_eq!(
            call("assert_text", Some("#m"), Some("Hi")).unwrap(),
            "sb.assert_text(\"Hi\", \"#m\").await?;"
        );
        assert_eq!(
            call("assert_exact_text", Some("#m"), Some("Hi")).unwrap(),
            "sb.assert_exact_text(\"Hi\", \"#m\").await?;"
        );
        assert_eq!(
            call("assert_text_visible", Some("#m"), Some("Hi")).unwrap(),
            "sb.assert_text_visible(\"Hi\", \"#m\").await?;"
        );
    }

    #[test]
    fn an_action_that_cannot_be_generated_becomes_a_comment_that_says_why() {
        let mut recorder = ActionRecorder::default();
        recorder.record("levitate", Some("#a"), None);
        recorder.record("click", None, None);
        recorder.record("type_text", Some("#a"), None);
        recorder.record("drag_and_drop_with_offset", Some("#a"), Some("#b"));
        recorder.record("set_window_size", Some("wide,tall"), None);
        recorder.record("assert_attribute", Some("#a"), Some("no equals sign"));
        recorder.record("click", Some("#fine"), None);

        let script = recorder.to_rust_script();
        for expected in [
            "// skipped: unknown action \"levitate\"",
            "// skipped: click has no target",
            "// skipped: type_text has no value",
            "// skipped: drag_and_drop_with_offset: the recording keeps the two selectors",
            "// skipped: set_window_size: width \"wide\" is not a whole number",
            "// skipped: assert_attribute: value \"no equals sign\" is not name=content",
            "sb.click(\"#fine\").await?;",
        ] {
            assert!(
                script.contains(expected),
                "missing {expected:?} in\n{script}"
            );
        }
        syn::parse_file(&script).expect("skipped actions leave a valid program");
    }

    #[test]
    fn a_mistyped_action_names_the_closest_known_one() {
        let mut recorder = ActionRecorder::default();
        let reason = recorder.try_record("clik", Some("#a"), None).unwrap_err();
        assert_eq!(reason, "unknown action \"clik\"; did you mean \"click\"?");
        assert!(recorder.actions.is_empty(), "a refused action is not kept");

        let reason = recorder
            .try_record("levitate", Some("#a"), None)
            .unwrap_err();
        assert_eq!(
            reason, "unknown action \"levitate\"",
            "nothing is near enough to guess"
        );
    }

    #[test]
    fn try_record_keeps_only_what_the_generator_can_use() {
        let mut recorder = ActionRecorder::default();
        recorder.try_record("click", Some("#ok"), None).unwrap();
        assert_eq!(
            recorder.try_record("click", None, None).unwrap_err(),
            "click has no target"
        );
        assert!(recorder.try_record("type_text", Some("#a"), None).is_err());

        assert_eq!(recorder.actions.len(), 1);
        let script = recorder.to_rust_script();
        assert!(script.contains("sb.click(\"#ok\")"), "{script}");
        assert!(!script.contains("skipped"), "{script}");
    }

    #[test]
    fn nothing_recorded_can_add_code_to_the_generated_program() {
        let mut recorder = ActionRecorder::default();
        // A name with a newline would end a `//` comment and start a statement.
        recorder.record("levitate\nstd::process::exit(1);", None, None);
        recorder.record("set_window_size", Some("1;std::process::exit(1),2"), None);
        recorder.record("click", Some("\"); std::process::exit(1); (\""), None);
        recorder.record(
            "select_option_by_index",
            Some("#a"),
            Some("1); std::process::exit(1"),
        );
        recorder.record("click_with_offset", Some("#a"), Some("1, 2); evil("));

        let script = recorder.to_rust_script();
        assert!(
            !script
                .lines()
                .any(|line| line.trim_start().starts_with("std::process")),
            "{script}"
        );
        // `let sb`, the one click, `quit` and `Ok(())`; nothing else.
        let file = syn::parse_file(&script).unwrap();
        let syn::Item::Fn(main) = &file.items[1] else {
            panic!("expected main in {script}");
        };
        assert_eq!(main.block.stmts.len(), 4, "{script}");
    }

    #[test]
    fn awkward_text_arrives_in_the_generated_program_exactly_as_recorded() {
        let awkward = [
            "say \"hi\"",
            "back\\slash",
            "line\nbreak",
            "braces {} {{x}}",
            "tab\tand unicode \u{fc}\u{2713}\u{1f600}",
            "'single'",
            "",
        ];
        for text in awkward {
            let line = call("type_text", Some("#f"), Some(text)).unwrap();
            let (method, arguments) = parse_call(&line);
            assert_eq!(method, "type_text");
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) = &arguments[1]
            else {
                panic!("{line}: second argument is not a string");
            };
            assert_eq!(value.value(), text, "{line}");
        }
    }

    #[test]
    fn a_recording_cannot_be_saved_outside_the_folder_it_was_given() {
        let dir = tempfile::tempdir().unwrap();
        let inside = dir.path().join("tests");
        for name in ["../escape", "a/b", "..", ".", "", "a\\b", "x\0y"] {
            let error = ActionRecorder::default()
                .save_recording_as_test(&inside, name)
                .unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig(_)),
                "{name:?}: {error:?}"
            );
        }
        assert!(!dir.path().join("escape.rs").exists());
        assert!(
            !inside.exists() || std::fs::read_dir(&inside).unwrap().next().is_none(),
            "nothing was written"
        );

        // A name with dots and dashes is fine; the function gets a clean name.
        let saved = ActionRecorder::default()
            .save_recording_as_test(&inside, "my-login.flow")
            .unwrap();
        assert_eq!(saved, inside.join("my-login.flow.rs"));
        assert!(std::fs::read_to_string(saved)
            .unwrap()
            .contains("async fn my_login_flow()"));
    }

    // ---- loading what was saved ---------------------------------------------

    #[test]
    fn a_yaml_recording_reads_back_exactly() {
        let mut session = RecorderSession::new();
        session.push("goto", Some("https://example.com/?q=a: b"), None);
        session.push(
            "type",
            Some("#name"),
            Some("Ada \"A\" Lovelace\nline two \\ \u{2713} #hash: colon"),
        );
        session.push("click", None, None);

        let back = RecorderSession::parse_yaml(&session.to_yaml()).unwrap();
        assert_eq!(back.len(), 3);
        for (written, read) in session.actions().iter().zip(&back) {
            assert_eq!(written.name, read.name);
            assert_eq!(written.target, read.target);
            assert_eq!(written.value, read.value);
            assert_eq!(written.timestamp_ms, read.timestamp_ms);
        }
        assert!(RecorderSession::parse_yaml("[]\n").unwrap().is_empty());
        assert!(RecorderSession::parse_yaml("").unwrap().is_empty());
    }

    #[test]
    fn load_reads_a_saved_file_whatever_its_format() {
        let dir = tempfile::tempdir().unwrap();
        let session = sample_session();
        for file in ["actions.json", "actions.yaml", "actions.YML", "actions"] {
            let path = session.save(&dir.path().join(file)).unwrap();
            for loaded in [
                RecorderSession::load(&path).unwrap(),
                RecorderSession::load_json(&path).unwrap(),
            ] {
                assert_eq!(loaded.len(), 4, "{file}");
                assert_eq!(loaded[2].value.as_deref(), Some("Ada \"A\" Lovelace"));
            }
        }
    }

    #[test]
    fn a_yaml_recording_with_a_mistake_names_the_line() {
        for (raw, line) in [
            ("- name: \"a\"\n  target: unquoted\n", 2),
            ("  name: \"a\"\n", 1),
            ("- name: \"a\"\n  colour: \"red\"\n", 2),
            ("- name: \"a\"\n  timestamp_ms: soon\n", 2),
            ("garbage\n", 1),
        ] {
            let error = RecorderSession::parse_yaml(raw).unwrap_err().to_string();
            assert!(error.contains(&format!("line {line}")), "{raw:?}: {error}");
        }
        let nameless = RecorderSession::parse_yaml("- target: \"x\"\n").unwrap_err();
        assert!(nameless.to_string().contains("no name"), "{nameless}");
    }

    #[test]
    fn a_drained_event_remembers_the_page_it_happened_on_but_the_recording_does_not_keep_it() {
        let raw = r##"[{"name":"click","target":"#a","value":null,"timestamp_ms":7,"url":"http://x.test/a"}]"##;
        let captured = RecorderSession::parse_captured(raw);
        assert_eq!(captured[0].url.as_deref(), Some("http://x.test/a"));

        let actions = RecorderSession::parse_drained(raw);
        assert_eq!(actions[0].timestamp_ms, 7);
        let saved = serde_json::to_string(&actions).unwrap();
        assert!(!saved.contains("x.test"), "{saved}");
    }

    #[test]
    fn a_goto_is_recorded_once_for_each_new_page() {
        let mut session = RecorderSession::new();
        session.note_page("http://x.test/a");
        session.note_page("http://x.test/a");
        session.note_page("http://x.test/b");
        session.note_page("http://x.test/a");
        let targets: Vec<_> = session
            .actions()
            .iter()
            .map(|action| (action.name.as_str(), action.target.as_deref()))
            .collect();
        assert_eq!(
            targets,
            [
                ("goto", Some("http://x.test/a")),
                ("goto", Some("http://x.test/b")),
                ("goto", Some("http://x.test/a")),
            ]
        );
    }

    #[test]
    fn the_listeners_keep_what_they_capture_where_a_navigation_cannot_erase_it() {
        let install = RecorderSession::install_script();
        assert!(install.contains("sessionStorage.setItem"));
        assert!(install.contains("url: window.location.href"));
        let drain = RecorderSession::drain_script();
        assert!(drain.contains("sessionStorage.getItem"));
        assert!(drain.contains("sessionStorage.removeItem"));
        // Both read and write the same key.
        assert!(install.contains("__sbRecordedActions") && drain.contains("__sbRecordedActions"));
    }

    // ---- live capture, against a real browser --------------------------------

    use crate::browser::config::{BrowserConfig, DriverMode};

    /// Serves a page with a field and a link, and a second page with a button,
    /// on a loopback port. Returns the address.
    fn serve_pages() -> String {
        use std::io::{Read, Write};
        const FIRST: &str =
            r#"<!doctype html><title>A</title><input id="name"><a id="next" href="/b">Next</a>"#;
        const SECOND: &str =
            r#"<!doctype html><title>B</title><button id="done" type="button">Done</button>"#;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0_u8; 2048];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]);
                let body = if request.starts_with("GET /b") {
                    SECOND
                } else {
                    FIRST
                };
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            }
        });
        format!("http://{address}")
    }

    /// What a recording holds, as `(name, target, value)`.
    fn steps(session: &RecorderSession) -> Vec<(String, Option<String>, Option<String>)> {
        session
            .actions()
            .iter()
            .map(|a| (a.name.clone(), a.target.clone(), a.value.clone()))
            .collect()
    }

    fn step(
        name: &str,
        target: Option<&str>,
        value: Option<&str>,
    ) -> (String, Option<String>, Option<String>) {
        (
            name.to_owned(),
            target.map(ToOwned::to_owned),
            value.map(ToOwned::to_owned),
        )
    }

    /// Records the typing and clicks of a visit to two pages, with a navigation
    /// between them, and checks what the recording holds.
    ///
    /// Only a session in CDP mode can have the listeners installed in every page
    /// the browser loads later. Any other session gets them on the new page when
    /// the recording is next drained, so `drain_on_the_new_page` drains once
    /// there before the first click.
    async fn record_across_a_navigation(mode: DriverMode, drain_on_the_new_page: bool) {
        let base = serve_pages();
        let mut sb = BaseCase::new(BrowserConfig {
            headless: true,
            mode,
            ..BrowserConfig::default()
        })
        .await
        .expect("Chrome starts");
        let mut session = RecorderSession::new();

        // A URL the browser normalizes (`http://host:port` becomes
        // `http://host:port/`) must not be recorded a second time on the next
        // drain.
        session.goto(&mut sb, &base).await.unwrap();
        session.install(&sb).await.unwrap();
        session.drain(&mut sb).await.unwrap();
        assert_eq!(
            steps(&session),
            [step("goto", Some(&base), None)],
            "a drain with nothing new adds nothing"
        );

        sb.type_text("#name", "Ada").await.unwrap();
        // Leaving the page must not lose the typing or the click itself.
        sb.click("#next").await.unwrap();
        sb.wait_for_element_visible("#done", 5).await.unwrap();
        if drain_on_the_new_page {
            session.drain(&mut sb).await.unwrap();
        }
        sb.click("#done").await.unwrap();
        session.drain(&mut sb).await.unwrap();
        sb.quit().await.unwrap();

        assert_eq!(
            steps(&session),
            [
                step("goto", Some(&base), None),
                step("type", Some("#name"), Some("Ada")),
                step("click", Some("#next"), None),
                step("goto", Some(&format!("{base}/b")), None),
                step("click", Some("#done"), None),
            ]
        );
    }

    /// Run with `cargo test --lib recorder -- --ignored`.
    #[tokio::test]
    #[ignore = "launches a real Chrome through chromedriver"]
    async fn a_live_recording_keeps_what_happened_either_side_of_a_navigation() {
        record_across_a_navigation(DriverMode::WebDriver, true).await;
    }

    #[tokio::test]
    #[ignore = "launches a real Chrome through chromedriver"]
    async fn in_cdp_mode_a_new_page_records_from_its_first_moment() {
        record_across_a_navigation(DriverMode::Cdp, false).await;
    }

    fn gherkin_recorder() -> ActionRecorder {
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com"), None);
        recorder.record("type_text", Some("#name"), Some("Ada"));
        recorder.record("click", Some("#go"), None);
        recorder.record("go_back", None, None);
        recorder.record("hover", Some("#menu"), None);
        recorder.record("type_text", Some("#q"), Some("say \"hi\""));
        recorder
    }

    #[test]
    fn a_recording_becomes_a_gherkin_scenario_and_says_what_it_skipped() {
        let feature = gherkin_recorder().to_gherkin("Sign up");
        assert_eq!(
            feature,
            "Feature: Sign up\n\n  Scenario: Sign up\n\
             \x20   Given I open \"https://example.com\"\n\
             \x20   And I type \"Ada\" into \"#name\"\n\
             \x20   And I click \"#go\"\n\
             \x20   And I go back\n\
             \x20   # skipped: hover: no Gherkin step for this action\n\
             \x20   # skipped: type_text: the text \"say \\\"hi\\\"\" has a quote or a line break\n"
        );
    }

    #[test]
    fn nothing_recorded_can_add_lines_to_the_generated_feature() {
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com"), None);
        // A name with a line break would end a `# skipped:` comment and start
        // a Scenario or a step of the attacker's choosing.
        recorder.record(
            "x\nScenario: injected\n    Given I open \"https://evil.test\"",
            None,
            None,
        );
        recorder.record("y\r    Then I click \"#evil\"", None, None);
        recorder.record("z\u{2028}Scenario: split", None, None);
        recorder.record("w\u{85}Scenario: split", None, None);
        recorder.record("click", Some("#ok"), None);

        let feature = recorder.to_gherkin("Sign up");
        assert!(
            !feature.contains(['\r', '\u{85}', '\u{2028}', '\u{2029}']),
            "{feature:?}"
        );
        let lines: Vec<String> = feature.lines().map(String::from).collect();
        let parsed = crate::behave::parse_feature(&lines);
        assert_eq!(parsed.scenarios.len(), 1, "{feature}");
        let steps: Vec<String> = parsed.scenarios[0]
            .steps
            .iter()
            .map(crate::behave::Step::full_text)
            .collect();
        assert_eq!(
            steps,
            [
                "Given I open \"https://example.com\"",
                "And I click \"#ok\""
            ],
            "{feature}"
        );
        let skipped = feature
            .lines()
            .filter(|line| line.trim_start().starts_with("# skipped:"))
            .count();
        assert_eq!(skipped, 4, "{feature}");
    }

    #[test]
    fn an_empty_text_is_skipped_because_the_type_step_cannot_match_it() {
        use crate::behave::async_steps::AsyncStepRegistry;
        let mut registry =
            AsyncStepRegistry::new(crate::BaseCase::without_session(BrowserConfig::default()));
        registry.register_common_browser_steps();
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com"), None);
        recorder.record("type_text", Some("#q"), Some(""));
        recorder.record("click", Some("#go"), None);

        let feature = recorder.to_gherkin("Search");
        let steps: Vec<&str> = feature
            .lines()
            .map(str::trim)
            .filter(|l| ["Given ", "And "].iter().any(|k| l.starts_with(k)))
            .collect();
        assert_eq!(steps.len(), 2, "{feature}");
        for step in steps {
            assert!(registry.has_handler(step), "no handler for {step:?}");
        }
        assert!(
            feature.contains("    # skipped: type_text: the text is empty"),
            "{feature}"
        );
    }

    #[test]
    fn every_gherkin_step_has_a_handler_in_the_common_browser_steps() {
        use crate::behave::async_steps::AsyncStepRegistry;
        let mut registry =
            AsyncStepRegistry::new(crate::BaseCase::without_session(BrowserConfig::default()));
        registry.register_common_browser_steps();
        let feature = gherkin_recorder().to_gherkin("Sign up");
        let steps: Vec<&str> = feature
            .lines()
            .map(str::trim)
            .filter(|l| ["Given ", "And "].iter().any(|k| l.starts_with(k)))
            .collect();
        assert_eq!(steps.len(), 4);
        for step in steps {
            assert!(registry.has_handler(step), "no handler for {step:?}");
        }
    }

    #[test]
    fn a_feature_is_saved_beside_the_name_and_a_path_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = gherkin_recorder()
            .save_recording_as_feature(dir.path(), "signup")
            .expect("saved");
        assert_eq!(path, dir.path().join("signup.feature"));
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("Feature: signup"));
        assert!(gherkin_recorder()
            .save_recording_as_feature(dir.path(), "../signup")
            .is_err());
    }

    #[test]
    fn a_recorded_sleep_is_generated_without_a_question_mark() {
        let mut recorder = ActionRecorder::default();
        recorder.record("sleep", None, Some("2"));
        recorder.record("sleep", None, Some("0.5"));
        let script = recorder.to_rust_script();
        assert!(script.contains("    sb.sleep(2.0).await;\n"), "{script}");
        assert!(script.contains("    sb.sleep(0.5).await;\n"), "{script}");
        assert!(recorder.try_record("sleep", None, Some("soon")).is_err());
        assert!(recorder.try_record("sleep", None, Some("-1")).is_err());
        assert!(recorder.try_record("sleep", None, None).is_err());
    }
}
