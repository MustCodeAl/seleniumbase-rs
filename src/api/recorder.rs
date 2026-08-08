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

use serde::{Deserialize, Serialize};
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

    /// Generates a standalone `#[tokio::main]` Rust program from the recording.
    pub fn to_rust_script(&self) -> String {
        let mut out = String::from(
            "use seleniumbase_rs::{BaseCase, BrowserConfig};\n\n#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n    let mut sb = BaseCase::new(BrowserConfig::default()).await?;\n",
        );
        self.write_actions(&mut out);
        out.push_str("    sb.quit().await?;\n    Ok(())\n}\n");
        out
    }

    /// Generates a `#[tokio::test]` async function from the recording.
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
    pub fn save_recording_as_test(
        &self,
        dir: &Path,
        name: &str,
    ) -> Result<PathBuf, SeleniumBaseError> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}.rs"));
        std::fs::write(&path, self.to_rust_test(name))?;
        Ok(path)
    }

    fn write_actions(&self, out: &mut String) {
        for action in &self.actions {
            self.write_action(out, action);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn write_action(&self, out: &mut String, action: &RecordedAction) {
        let target = action.target.as_deref();
        let value = action.value.as_deref();

        match action.name.as_str() {
            "open" => {
                if let Some(url) = target {
                    line(out, &format!("sb.open({:?}).await?;", url));
                }
            }
            "click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.click({:?}).await?;", css));
                }
            }
            "type_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.type_text({:?}, {:?}).await?;", css, text));
                }
            }
            "clear" => {
                if let Some(css) = target {
                    line(out, &format!("sb.clear({:?}).await?;", css));
                }
            }
            "click_link_text" => {
                if let Some(text) = target {
                    line(out, &format!("sb.click_link_text({:?}).await?;", text));
                }
            }
            "submit" => {
                if let Some(css) = target {
                    line(out, &format!("sb.submit({:?}).await?;", css));
                }
            }
            "hover" => {
                if let Some(css) = target {
                    line(out, &format!("sb.hover({:?}).await?;", css));
                }
            }
            "select_option_by_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.select_option_by_text({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "select_option_by_value" => {
                if let (Some(css), Some(val)) = (target, value) {
                    line(
                        out,
                        &format!("sb.select_option_by_value({:?}, {:?}).await?;", css, val),
                    );
                }
            }
            "switch_to_frame" => {
                if let Some(css) = target {
                    line(out, &format!("sb.switch_to_frame({:?}).await?;", css));
                }
            }
            "switch_to_default_content" => {
                line(out, "sb.switch_to_default_content().await?;");
            }
            "drag_and_drop" => {
                if let (Some(source), Some(dest)) = (target, value) {
                    line(
                        out,
                        &format!("sb.drag_and_drop({:?}, {:?}).await?;", source, dest),
                    );
                }
            }
            "cdp_click_element" => {
                if let Some(css) = target {
                    line(out, &format!("sb.cdp_click_element({:?}).await?;", css));
                }
            }
            "wait_for_element_visible" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_visible({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_element_absent" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_absent({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_element_not_visible" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_not_visible({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_element_present" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_present({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_element_clickable" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.wait_for_element_clickable({:?}, 10).await?;", css),
                    );
                }
            }
            "wait_for_ready_state_complete" => {
                line(out, "sb.wait_for_ready_state_complete().await?;");
            }
            "assert_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.assert_text({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "assert_text_visible" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.assert_text_visible({:?}, {:?}).await?;", text, css),
                    );
                }
            }
            "assert_text_not_visible" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.assert_text_not_visible({:?}, {:?}).await?;", text, css),
                    );
                }
            }
            "assert_exact_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.assert_exact_text({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "assert_attribute" => {
                if let (Some(css), Some(pair)) = (target, value) {
                    if let Some((attribute, expected)) = pair.split_once('=') {
                        line(
                            out,
                            &format!(
                                "sb.assert_attribute({:?}, {:?}, {:?}).await?;",
                                css,
                                attribute.trim(),
                                expected.trim()
                            ),
                        );
                    }
                }
            }
            "assert_title" => {
                if let Some(text) = target {
                    line(out, &format!("sb.assert_title({:?}).await?;", text));
                }
            }
            "assert_title_contains" => {
                if let Some(text) = target {
                    line(
                        out,
                        &format!("sb.assert_title_contains({:?}).await?;", text),
                    );
                }
            }
            "assert_link_text" => {
                if let Some(text) = target {
                    line(out, &format!("sb.assert_link_text({:?}).await?;", text));
                }
            }
            "maximize_window" => {
                line(out, "sb.maximize_window().await?;");
            }
            "set_window_size" => {
                if let Some(pair) = target {
                    if let Some((width, height)) = pair.split_once(',') {
                        line(
                            out,
                            &format!(
                                "sb.set_window_size({}, {}).await?;",
                                width.trim(),
                                height.trim()
                            ),
                        );
                    }
                }
            }
            "switch_to_window" => {
                if let Some(handle) = target {
                    line(out, &format!("sb.switch_to_window({:?}).await?;", handle));
                }
            }
            "switch_to_new_window" => {
                line(out, "sb.switch_to_new_window().await?;");
            }
            "switch_to_newest_window" => {
                line(out, "sb.switch_to_newest_window().await?;");
            }
            "switch_to_default_window" => {
                line(out, "sb.switch_to_default_window().await?;");
            }
            "switch_to_parent_frame" => {
                line(out, "sb.switch_to_parent_frame().await?;");
            }
            "go_back" => {
                line(out, "sb.go_back().await?;");
            }
            "go_forward" => {
                line(out, "sb.go_forward().await?;");
            }
            "refresh" => {
                line(out, "sb.refresh().await?;");
            }
            "delete_all_cookies" => {
                line(out, "sb.delete_all_cookies().await?;");
            }
            "delete_cookie" => {
                if let Some(name) = target {
                    line(out, &format!("sb.delete_cookie({:?}).await?;", name));
                }
            }
            "add_cookie" => {
                if let (Some(name), Some(val)) = (target, value) {
                    line(
                        out,
                        &format!("sb.add_cookie({:?}, {:?}).await?;", name, val),
                    );
                }
            }
            "double_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.double_click({:?}).await?;", css));
                }
            }
            "context_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.context_click({:?}).await?;", css));
                }
            }
            "scroll_to" => {
                if let Some(css) = target {
                    line(out, &format!("sb.scroll_to({:?}).await?;", css));
                }
            }
            "scroll_to_bottom" => {
                line(out, "sb.scroll_to_bottom().await?;");
            }
            "scroll_to_top" => {
                line(out, "sb.scroll_to_top().await?;");
            }
            "smooth_scroll_to" => {
                if let Some(css) = target {
                    line(out, &format!("sb.smooth_scroll_to({:?}).await?;", css));
                }
            }
            "check_if_unchecked" => {
                if let Some(css) = target {
                    line(out, &format!("sb.check_if_unchecked({:?}).await?;", css));
                }
            }
            "uncheck_if_checked" => {
                if let Some(css) = target {
                    line(out, &format!("sb.uncheck_if_checked({:?}).await?;", css));
                }
            }
            "add_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.add_text({:?}, {:?}).await?;", css, text));
                }
            }
            "send_keys" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.send_keys({:?}, {:?}).await?;", css, text));
                }
            }
            "js_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.js_click({:?}).await?;", css));
                }
            }
            "js_type" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.js_type({:?}, {:?}).await?;", css, text));
                }
            }
            "js_click_if_present" => {
                if let Some(css) = target {
                    line(out, &format!("sb.js_click_if_present({:?}).await?;", css));
                }
            }
            "js_click_all" => {
                if let Some(css) = target {
                    line(out, &format!("sb.js_click_all({:?}).await?;", css));
                }
            }
            "jquery_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.jquery_click({:?}).await?;", css));
                }
            }
            "set_attribute" => {
                if let (Some(css), Some(pair)) = (target, value) {
                    if let Some((attribute, new_value)) = pair.split_once('=') {
                        line(
                            out,
                            &format!(
                                "sb.set_attribute({:?}, {:?}, {:?}).await?;",
                                css,
                                attribute.trim(),
                                new_value.trim()
                            ),
                        );
                    }
                }
            }
            "remove_attribute" => {
                if let (Some(css), Some(attribute)) = (target, value) {
                    line(
                        out,
                        &format!("sb.remove_attribute({:?}, {:?}).await?;", css, attribute),
                    );
                }
            }
            "choose_file" => {
                if let (Some(css), Some(path)) = (target, value) {
                    line(
                        out,
                        &format!("sb.choose_file({:?}, {:?}).await?;", css, path),
                    );
                }
            }
            "click_partial_link_text" => {
                if let Some(text) = target {
                    line(
                        out,
                        &format!("sb.click_partial_link_text({:?}).await?;", text),
                    );
                }
            }
            "wait_for_and_accept_alert" => {
                line(out, "sb.wait_for_and_accept_alert(10).await?;");
            }
            "wait_for_and_dismiss_alert" => {
                line(out, "sb.wait_for_and_dismiss_alert(10).await?;");
            }
            "accept_alert" => {
                line(out, "sb.accept_alert().await?;");
            }
            "dismiss_alert" => {
                line(out, "sb.dismiss_alert().await?;");
            }
            "type_alert_text" => {
                if let Some(text) = target {
                    line(out, &format!("sb.type_alert_text({:?}).await?;", text));
                }
            }
            "clear_local_storage" => {
                line(out, "sb.clear_local_storage().await?;");
            }
            "remove_local_storage_item" => {
                if let Some(key) = target {
                    line(
                        out,
                        &format!("sb.remove_local_storage_item({:?}).await?;", key),
                    );
                }
            }
            "set_local_storage_item" => {
                if let (Some(key), Some(val)) = (target, value) {
                    line(
                        out,
                        &format!("sb.set_local_storage_item({:?}, {:?}).await?;", key, val),
                    );
                }
            }
            "close_window" => {
                line(out, "sb.close_window().await?;");
            }
            "highlight_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.highlight_click({:?}).await?;", css));
                }
            }
            "open_new_window" => {
                line(out, "sb.open_new_window().await?;");
            }
            "open_new_tab" => {
                line(out, "sb.open_new_tab().await?;");
            }
            "click_visible_elements" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("sb.click_visible_elements({:?}).await?;", css),
                    );
                }
            }
            "find_element" => {
                if let Some(css) = target {
                    line(out, &format!("let _ = sb.find_element({:?}).await?;", css));
                }
            }
            "find_elements" => {
                if let Some(css) = target {
                    line(out, &format!("let _ = sb.find_elements({:?}).await?;", css));
                }
            }
            "get_shadow_root" => {
                if let Some(css) = target {
                    line(
                        out,
                        &format!("let _ = sb.get_shadow_root({:?}).await?;", css),
                    );
                }
            }
            "slow_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.slow_click({:?}).await?;", css));
                }
            }
            "human_type" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.human_type({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "human_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.human_click({:?}).await?;", css));
                }
            }
            "uc_click" => {
                if let Some(css) = target {
                    line(out, &format!("sb.uc_click({:?}).await?;", css));
                }
            }
            "uc_type" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(out, &format!("sb.uc_type({:?}, {:?}).await?;", css, text));
                }
            }
            "update_text" => {
                if let (Some(css), Some(text)) = (target, value) {
                    line(
                        out,
                        &format!("sb.update_text({:?}, {:?}).await?;", css, text),
                    );
                }
            }
            "click_xpath" => {
                if let Some(xpath) = target {
                    line(out, &format!("sb.click_xpath({:?}).await?;", xpath));
                }
            }
            _ => {}
        }
    }
}

fn line(out: &mut String, content: &str) {
    out.push_str("    ");
    out.push_str(content);
    out.push('\n');
}

fn rust_identifier(name: &str) -> String {
    let mut ident: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if ident.is_empty() {
        ident.push_str("recorded_test");
    }
    if ident.chars().next().unwrap().is_ascii_digit() {
        ident.insert(0, '_');
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
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
            .unwrap_or(false);
        let data = if is_yaml {
            self.to_yaml()
        } else {
            self.to_json()?
        };
        std::fs::write(path, data)?;
        Ok(path.to_path_buf())
    }

    /// Loads a recorded action list from a JSON file written by [`Self::save`].
    pub fn load_json(path: &Path) -> Result<Vec<RecordedAction>, SeleniumBaseError> {
        let raw = std::fs::read_to_string(path)?;
        Self::parse_json(&raw)
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
    /// The script is idempotent: re-running it after a navigation reinstalls the
    /// listeners without duplicating them.
    pub fn install_script() -> &'static str {
        INSTALL_SCRIPT
    }

    /// JavaScript that returns and clears the captured in-page events.
    pub fn drain_script() -> &'static str {
        DRAIN_SCRIPT
    }

    /// Parses the JSON payload produced by [`Self::drain_script`].
    pub fn parse_drained(raw: &str) -> Vec<RecordedAction> {
        serde_json::from_str::<Vec<RecordedAction>>(raw).unwrap_or_default()
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
        self.last_url = Some(url.to_owned());
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
    pub async fn assert_text(
        &mut self,
        sb: &mut BaseCase,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        // Flush pending in-page events first so the recording stays ordered.
        if self.installed {
            let _ = self.drain(sb).await;
        }
        sb.assert_text(css, text).await?;
        self.push("assert_text", Some(css), Some(text));
        Ok(())
    }

    /// Installs the in-page listeners so manual interactions are captured.
    ///
    /// While installed, [`RecorderSession::click`] and
    /// [`RecorderSession::type_text`] stop recording locally so each
    /// interaction is captured exactly once.
    pub async fn install(&mut self, sb: &BaseCase) -> Result<(), SeleniumBaseError> {
        sb.execute_script(Self::install_script()).await?;
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
        let raw = sb.execute_script(Self::drain_script()).await;
        let mut captured = match raw {
            Ok(value) => {
                let text = value.as_str().map(ToOwned::to_owned).unwrap_or_default();
                Self::parse_drained(&text)
            }
            Err(_) => Vec::new(),
        };

        // Re-install after navigations, and note the navigation itself.
        if let Ok(url) = sb.get_current_url().await {
            if self.last_url.as_deref() != Some(url.as_str()) {
                self.last_url = Some(url.clone());
                self.actions
                    .push(RecordedAction::new("goto", Some(&url), None));
            }
        }
        let _ = self.install(sb).await;

        let count = captured.len();
        self.actions.append(&mut captured);
        Ok(count)
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
                        sb.assert_text(css, text).await,
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
var push = function (name, target, value) {
    window.__sbRecordedActions.push({
        name: name, target: target, value: value, timestamp_ms: Date.now()
    });
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
var actions = window.__sbRecordedActions || [];
window.__sbRecordedActions = [];
return JSON.stringify(actions);
"##;

fn yaml_scalar(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
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
        // `goto`/`type` are replay-only aliases, so only known names emit code.
        assert!(script.contains("sb.click(\"#submit\")"));
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
}
