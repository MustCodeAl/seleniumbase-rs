//! JSON-driven scenario runner and dashboard writer.
//!
//! A [`Scenario`] is a portable list of high-level browser actions described in
//! JSON. [`run_scenario`] replays those actions against a [`BaseCase`] and
//! produces a [`RunSummary`] that can be rendered to HTML with
//! [`write_dashboard`].
//!
//! Each step names its action with an `action` field in snake case, and the
//! rest of its fields are that action's arguments:
//!
//! ```json
//! {
//!   "name": "login",
//!   "steps": [
//!     { "action": "open", "url": "https://seleniumbase.io/simple/login" },
//!     { "action": "type_text", "css": "#username", "text": "demo_user" },
//!     { "action": "click", "css": "button" },
//!     { "action": "wait_for_text", "css": "h1", "text": "Welcome" }
//!   ]
//! }
//! ```
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::api::scenario::{Scenario, ScenarioStep};
//!
//! # fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let scenario = Scenario::from_json(
//!     r#"{"name": "demo", "steps": [
//!         {"action": "open", "url": "https://example.com"},
//!         {"action": "assert_text", "css": "h1", "contains": "Example"}
//!     ]}"#,
//! )?;
//! assert_eq!(scenario.steps.len(), 2);
//! assert!(matches!(scenario.steps[0], ScenarioStep::Open { .. }));
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};

use crate::api::base_case::BaseCase;
use crate::core::dashboard::PageOptions;
use crate::core::report_helper::{RunSnapshot, TestResult, TestStatus};
use crate::error::SeleniumBaseError;
use crate::utils::translate::scenario::scenario_to_english;

/// How long a `wait_for_text` step waits when its `timeout` is left out.
const DEFAULT_WAIT_SECONDS: u64 = 10;

fn default_wait_seconds() -> u64 {
    DEFAULT_WAIT_SECONDS
}

/// Result summary for a scenario run.
#[derive(Debug, Default, Clone)]
pub struct RunSummary {
    /// Name of the executed scenario.
    pub scenario_name: String,
    /// Total number of steps in the scenario.
    pub total_steps: usize,
    /// Number of steps that completed without error.
    pub passed_steps: usize,
    /// Number of steps that produced an error.
    pub failed_steps: usize,
    /// Human-readable error messages for failed steps. Each starts with the
    /// step's position and action, such as `step 3 (click): ...`.
    pub errors: Vec<String>,
    /// How long the run took, in seconds.
    pub duration_secs: f64,
}

impl RunSummary {
    /// Whether every step passed.
    pub fn is_success(&self) -> bool {
        self.failed_steps == 0
    }

    /// The run as the single test the dashboard shows: the scenario, which
    /// passed, or failed with its failed steps as the message.
    #[must_use]
    pub fn snapshot(&self) -> RunSnapshot {
        let (status, message) = if self.is_success() {
            (
                TestStatus::Passed,
                format!("{} of {} steps passed", self.passed_steps, self.total_steps),
            )
        } else {
            (
                TestStatus::Failed,
                format!(
                    "{} of {} steps failed\n{}",
                    self.failed_steps,
                    self.total_steps,
                    self.errors.join("\n")
                ),
            )
        };
        let result = TestResult::new(
            self.scenario_name.clone(),
            status,
            self.duration_secs,
            message,
        );
        RunSnapshot::from_results("Scenario Dashboard", vec![result])
    }
}

/// A scenario parsed from JSON, containing a sequence of browser actions.
#[derive(Debug, Deserialize, Serialize)]
pub struct Scenario {
    /// Human-readable name for the scenario.
    pub name: String,
    /// Ordered list of steps to execute.
    pub steps: Vec<ScenarioStep>,
}

impl Scenario {
    /// Parses a scenario from JSON text.
    ///
    /// A step's `action` may be written in any language of the translation
    /// dictionary (`"点击"` for `"click"`); see
    /// [`crate::utils::translate::scenario`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Json`] if the text is not valid JSON, has
    /// no `name` or `steps`, or contains a step whose `action` is unknown or
    /// that lacks one of its arguments. The message names the offending field.
    pub fn from_json(json: &str) -> Result<Self, SeleniumBaseError> {
        Ok(serde_json::from_str(&scenario_to_english(json))?)
    }
}

/// A single step inside a [`Scenario`].
///
/// The JSON `action` field is the variant's name in snake case, and the
/// variant's fields are the step's other keys.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ScenarioStep {
    /// Navigate to a URL.
    Open {
        /// The page to open.
        url: String,
    },
    /// Click an element.
    Click {
        /// Selector of the element.
        css: String,
    },
    /// Type text into a field after clearing it.
    TypeText {
        /// Selector of the field.
        css: String,
        /// The text to type.
        text: String,
    },
    /// Assert that an element's text contains a string.
    AssertText {
        /// Selector of the element.
        css: String,
        /// The text the element must contain.
        contains: String,
    },
    /// Assert that an element exists and is visible.
    AssertElement {
        /// Selector of the element.
        css: String,
    },
    /// Wait until an element contains a string.
    WaitForText {
        /// Selector of the element.
        css: String,
        /// The text to wait for.
        text: String,
        /// How long to wait, in seconds. Ten when left out.
        #[serde(default = "default_wait_seconds")]
        timeout: u64,
    },
    /// Pause for some seconds.
    Sleep {
        /// How long to pause; zero or less does not pause.
        seconds: f64,
    },
    /// Move the pointer over an element.
    Hover {
        /// Selector of the element.
        css: String,
    },
    /// Hover over one element, then click another that the hover reveals.
    HoverAndClick {
        /// Selector of the element to hover.
        hover_css: String,
        /// Selector of the element to click.
        click_css: String,
    },
    /// Choose a `<select>` option by its visible text.
    SelectOptionByText {
        /// Selector of the `<select>`.
        css: String,
        /// The option's text.
        text: String,
    },
    /// Choose a `<select>` option by its `value`.
    SelectOptionByValue {
        /// Selector of the `<select>`.
        css: String,
        /// The option's value.
        value: String,
    },
    /// Drag one element onto another.
    DragAndDrop {
        /// Selector of the element to drag.
        source_css: String,
        /// Selector of the drop target.
        target_css: String,
    },
    /// Make an iframe the current context.
    SwitchToFrame {
        /// Selector of the iframe.
        css: String,
    },
    /// Return to the top-level document.
    SwitchToDefaultContent,
    /// Empty a field.
    Clear {
        /// Selector of the field.
        css: String,
    },
    /// Click a link by its exact text.
    ClickLinkText {
        /// The link's text.
        text: String,
    },
    /// Submit a form.
    Submit {
        /// Selector of the form or one of its fields.
        css: String,
    },
    /// Accept the open JavaScript alert.
    AcceptAlert,
    /// Dismiss the open JavaScript alert.
    DismissAlert,
    /// Type into the open JavaScript prompt.
    TypeAlertText {
        /// The text to type.
        text: String,
    },
    /// Remove every item from `localStorage`.
    ClearLocalStorage,
    /// Remove one `localStorage` item.
    RemoveLocalStorageItem {
        /// The item's key.
        key: String,
    },
    /// Set a `localStorage` item.
    SetLocalStorageItem {
        /// The item's key.
        key: String,
        /// The value to store.
        value: String,
    },
    /// Switch to a window by its handle.
    SwitchToWindow {
        /// The window handle.
        handle: String,
    },
    /// Click an element with JavaScript instead of a mouse event.
    JsClick {
        /// Selector of the element.
        css: String,
    },
    /// Set a field's value with JavaScript instead of key events.
    JsType {
        /// Selector of the field.
        css: String,
        /// The text to set.
        text: String,
    },
    /// Set an attribute on an element.
    SetAttribute {
        /// Selector of the element.
        css: String,
        /// The attribute's name.
        attribute: String,
        /// The value to set.
        value: String,
    },
    /// Remove an attribute from an element.
    RemoveAttribute {
        /// Selector of the element.
        css: String,
        /// The attribute's name.
        attribute: String,
    },
    /// Choose a file in a file input.
    ChooseFile {
        /// Selector of the `<input type="file">`.
        css: String,
        /// Path of the file to upload.
        file_path: String,
    },
    /// Go back in the history.
    GoBack,
    /// Go forward in the history.
    GoForward,
    /// Reload the page.
    Refresh,
    /// Delete every cookie.
    DeleteAllCookies,
    /// Open a new window and switch to it.
    SwitchToNewWindow,
    /// Double-click an element.
    DoubleClick {
        /// Selector of the element.
        css: String,
    },
    /// Right-click an element.
    ContextClick {
        /// Selector of the element.
        css: String,
    },
    /// Maximize the browser window.
    MaximizeWindow,
    /// Scroll to the bottom of the page.
    ScrollToBottom,
    /// Scroll to the top of the page.
    ScrollToTop,
    /// Scroll an element into view.
    ScrollTo {
        /// Selector of the element.
        css: String,
    },
}

impl ScenarioStep {
    /// The step's `action` name, such as `"click"`.
    pub fn action(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.get("action")?.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "unknown".to_owned())
    }
}

/// Something that can carry out a [`ScenarioStep`]. [`BaseCase`] is the real
/// one; the seam lets the bookkeeping be tested without a browser.
trait StepRunner {
    async fn run_step(&mut self, step: &ScenarioStep) -> Result<(), SeleniumBaseError>;
}

impl StepRunner for BaseCase {
    async fn run_step(&mut self, step: &ScenarioStep) -> Result<(), SeleniumBaseError> {
        let sb = self;
        match step {
            ScenarioStep::Open { url } => sb.open(url).await,
            ScenarioStep::Click { css } => sb.click(css).await,
            ScenarioStep::TypeText { css, text } => sb.type_text(css, text).await,
            ScenarioStep::AssertText { css, contains } => sb.assert_text(contains, css).await,
            ScenarioStep::AssertElement { css } => sb.assert_element(css).await,
            ScenarioStep::WaitForText { css, text, timeout } => {
                sb.wait_for_text(text, css, *timeout).await
            }
            ScenarioStep::Sleep { seconds } => {
                sb.sleep(*seconds).await;
                Ok(())
            }
            ScenarioStep::Hover { css } => sb.hover(css).await,
            ScenarioStep::HoverAndClick {
                hover_css,
                click_css,
            } => sb.hover_and_click(hover_css, click_css).await,
            ScenarioStep::SelectOptionByText { css, text } => {
                sb.select_option_by_text(css, text).await
            }
            ScenarioStep::SelectOptionByValue { css, value } => {
                sb.select_option_by_value(css, value).await
            }
            ScenarioStep::DragAndDrop {
                source_css,
                target_css,
            } => sb.drag_and_drop(source_css, target_css).await,
            ScenarioStep::SwitchToFrame { css } => sb.switch_to_frame(css).await,
            ScenarioStep::SwitchToDefaultContent => sb.switch_to_default_content().await,
            ScenarioStep::Clear { css } => sb.clear(css).await,
            ScenarioStep::ClickLinkText { text } => sb.click_link_text(text).await,
            ScenarioStep::Submit { css } => sb.submit(css).await,
            ScenarioStep::AcceptAlert => sb.accept_alert().await,
            ScenarioStep::DismissAlert => sb.dismiss_alert().await,
            ScenarioStep::TypeAlertText { text } => sb.type_alert_text(text).await,
            ScenarioStep::ClearLocalStorage => sb.clear_local_storage().await,
            ScenarioStep::RemoveLocalStorageItem { key } => sb.remove_local_storage_item(key).await,
            ScenarioStep::SetLocalStorageItem { key, value } => {
                sb.set_local_storage_item(key, value).await
            }
            ScenarioStep::SwitchToWindow { handle } => sb.switch_to_window(handle).await,
            ScenarioStep::JsClick { css } => sb.js_click(css).await,
            ScenarioStep::JsType { css, text } => sb.js_type(css, text).await,
            ScenarioStep::SetAttribute {
                css,
                attribute,
                value,
            } => sb.set_attribute(css, attribute, value).await,
            ScenarioStep::RemoveAttribute { css, attribute } => {
                sb.remove_attribute(css, attribute).await
            }
            ScenarioStep::ChooseFile { css, file_path } => sb.choose_file(css, file_path).await,
            ScenarioStep::GoBack => sb.go_back().await,
            ScenarioStep::GoForward => sb.go_forward().await,
            ScenarioStep::Refresh => sb.refresh().await,
            ScenarioStep::DeleteAllCookies => sb.delete_all_cookies().await,
            ScenarioStep::SwitchToNewWindow => sb.switch_to_new_window().await,
            ScenarioStep::DoubleClick { css } => sb.double_click(css).await,
            ScenarioStep::ContextClick { css } => sb.context_click(css).await,
            ScenarioStep::MaximizeWindow => sb.maximize_window().await,
            ScenarioStep::ScrollToBottom => sb.scroll_to_bottom().await,
            ScenarioStep::ScrollToTop => sb.scroll_to_top().await,
            ScenarioStep::ScrollTo { css } => sb.scroll_to(css).await,
        }
    }
}

/// Executes every step of `scenario` against `sb` and returns a summary.
///
/// Each step is executed in order; failures are collected and reported in the
/// returned [`RunSummary`] rather than stopping execution early. A failure's
/// message starts with the step's position and action, so it can be matched
/// to the scenario file.
///
/// # Errors
///
/// Never fails today: a step that fails is reported in the summary. The
/// `Result` is kept for steps that may one day abort the run.
pub async fn run_scenario(
    sb: &mut BaseCase,
    scenario: &Scenario,
) -> Result<RunSummary, SeleniumBaseError> {
    Ok(run_steps(sb, scenario).await)
}

async fn run_steps(runner: &mut impl StepRunner, scenario: &Scenario) -> RunSummary {
    let started = std::time::Instant::now();
    let mut passed_steps = 0_usize;
    let mut errors = Vec::new();

    for (index, step) in scenario.steps.iter().enumerate() {
        match runner.run_step(step).await {
            Ok(()) => passed_steps += 1,
            Err(error) => errors.push(format!("step {} ({}): {error}", index + 1, step.action())),
        }
    }

    RunSummary {
        scenario_name: scenario.name.clone(),
        total_steps: scenario.steps.len(),
        passed_steps,
        failed_steps: scenario.steps.len().saturating_sub(passed_steps),
        errors,
        duration_secs: started.elapsed().as_secs_f64(),
    }
}

/// Writes the dashboard of a scenario run to `path`: the page of
/// [`core::dashboard`](crate::core::dashboard) that a test run gets, with one
/// row, the scenario.
///
/// The row is `PASS`, or `FAIL` with the failed steps as its message. The
/// scenario's name and the messages are escaped by the page, so they are shown
/// as written.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] naming the path if the file
/// cannot be written.
pub fn write_dashboard<P: AsRef<std::path::Path>>(
    summary: &RunSummary,
    path: P,
) -> Result<(), SeleniumBaseError> {
    let path = path.as_ref();
    let run = summary.snapshot();
    let options = PageOptions {
        title: run.title.clone(),
        live: false,
        pie: true,
        page_dir: path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map_or_else(
                || std::path::PathBuf::from("."),
                std::path::Path::to_path_buf,
            ),
    };
    crate::core::dashboard::write_dashboard(path, &run, &options).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!(
            "failed to write dashboard '{}': {e}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fails the steps at the given (1-based) positions and records the rest.
    struct FakeRunner {
        fail_at: Vec<usize>,
        ran: Vec<String>,
    }

    impl FakeRunner {
        fn failing(fail_at: &[usize]) -> Self {
            Self {
                fail_at: fail_at.to_vec(),
                ran: Vec::new(),
            }
        }
    }

    impl StepRunner for FakeRunner {
        async fn run_step(&mut self, step: &ScenarioStep) -> Result<(), SeleniumBaseError> {
            self.ran.push(step.action());
            if self.fail_at.contains(&self.ran.len()) {
                Err(SeleniumBaseError::element_not_found("#missing"))
            } else {
                Ok(())
            }
        }
    }

    /// One JSON object per step kind, in the order of the enum.
    const EVERY_STEP: &str = r##"{"name": "all", "steps": [
        {"action": "open", "url": "https://example.com"},
        {"action": "click", "css": "#a"},
        {"action": "type_text", "css": "#a", "text": "t"},
        {"action": "assert_text", "css": "#a", "contains": "c"},
        {"action": "assert_element", "css": "#a"},
        {"action": "wait_for_text", "css": "#a", "text": "t", "timeout": 3},
        {"action": "sleep", "seconds": 0.5},
        {"action": "hover", "css": "#a"},
        {"action": "hover_and_click", "hover_css": "#a", "click_css": "#b"},
        {"action": "select_option_by_text", "css": "#s", "text": "One"},
        {"action": "select_option_by_value", "css": "#s", "value": "1"},
        {"action": "drag_and_drop", "source_css": "#a", "target_css": "#b"},
        {"action": "switch_to_frame", "css": "iframe"},
        {"action": "switch_to_default_content"},
        {"action": "clear", "css": "#a"},
        {"action": "click_link_text", "text": "Home"},
        {"action": "submit", "css": "form"},
        {"action": "accept_alert"},
        {"action": "dismiss_alert"},
        {"action": "type_alert_text", "text": "t"},
        {"action": "clear_local_storage"},
        {"action": "remove_local_storage_item", "key": "k"},
        {"action": "set_local_storage_item", "key": "k", "value": "v"},
        {"action": "switch_to_window", "handle": "h"},
        {"action": "js_click", "css": "#a"},
        {"action": "js_type", "css": "#a", "text": "t"},
        {"action": "set_attribute", "css": "#a", "attribute": "x", "value": "y"},
        {"action": "remove_attribute", "css": "#a", "attribute": "x"},
        {"action": "choose_file", "css": "#f", "file_path": "/tmp/f"},
        {"action": "go_back"},
        {"action": "go_forward"},
        {"action": "refresh"},
        {"action": "delete_all_cookies"},
        {"action": "switch_to_new_window"},
        {"action": "double_click", "css": "#a"},
        {"action": "context_click", "css": "#a"},
        {"action": "maximize_window"},
        {"action": "scroll_to_bottom"},
        {"action": "scroll_to_top"},
        {"action": "scroll_to", "css": "#a"}
    ]}"##;

    #[test]
    fn every_documented_action_parses_and_keeps_its_name() {
        let scenario = Scenario::from_json(EVERY_STEP).unwrap();
        assert_eq!(scenario.name, "all");
        assert_eq!(scenario.steps.len(), 40);
        let names: Vec<String> = scenario.steps.iter().map(ScenarioStep::action).collect();
        let written: Vec<String> = serde_json::from_str::<serde_json::Value>(EVERY_STEP).unwrap()
            ["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step["action"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(names, written);
    }

    #[test]
    fn a_scenario_written_in_any_language_parses_to_the_same_steps() {
        use crate::utils::translate::{scenario::translate_scenario, Language};

        let english = Scenario::from_json(EVERY_STEP).unwrap();
        for language in Language::ALL {
            let written = translate_scenario(EVERY_STEP, language).unwrap();
            assert!(
                written.unknown.is_empty(),
                "{language}: {:?}",
                written.unknown
            );
            let scenario = Scenario::from_json(&written.text).unwrap();
            assert_eq!(
                serde_json::to_value(&scenario).unwrap(),
                serde_json::to_value(&english).unwrap(),
                "{language}"
            );
        }
    }

    #[test]
    fn a_scenario_survives_a_trip_through_json() {
        let scenario = Scenario::from_json(EVERY_STEP).unwrap();
        let again = Scenario::from_json(&serde_json::to_string(&scenario).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&scenario).unwrap(),
            serde_json::to_value(&again).unwrap()
        );
    }

    #[test]
    fn wait_for_text_waits_ten_seconds_unless_told_otherwise() {
        let scenario = Scenario::from_json(
            r#"{"name": "n", "steps": [
                {"action": "wait_for_text", "css": "h1", "text": "Hi"},
                {"action": "wait_for_text", "css": "h1", "text": "Hi", "timeout": 2}
            ]}"#,
        )
        .unwrap();
        let timeouts: Vec<u64> = scenario
            .steps
            .iter()
            .map(|step| match step {
                ScenarioStep::WaitForText { timeout, .. } => *timeout,
                other => panic!("unexpected step {other:?}"),
            })
            .collect();
        assert_eq!(timeouts, [10, 2]);
    }

    #[test]
    fn a_bad_scenario_is_rejected_with_a_message_that_names_the_problem() {
        let unknown = Scenario::from_json(r#"{"name": "n", "steps": [{"action": "levitate"}]}"#)
            .unwrap_err()
            .to_string();
        assert!(unknown.contains("levitate"), "{unknown}");

        let missing = Scenario::from_json(r#"{"name": "n", "steps": [{"action": "click"}]}"#)
            .unwrap_err()
            .to_string();
        assert!(missing.contains("css"), "{missing}");

        assert!(Scenario::from_json("not json").is_err());
        assert!(Scenario::from_json(r#"{"steps": []}"#).is_err());
    }

    #[tokio::test]
    async fn a_failed_step_does_not_stop_the_run_and_the_error_says_which_step() {
        let scenario = Scenario::from_json(
            r##"{"name": "five", "steps": [
                {"action": "open", "url": "https://example.com"},
                {"action": "click", "css": "#a"},
                {"action": "refresh"},
                {"action": "assert_element", "css": "#b"},
                {"action": "go_back"}
            ]}"##,
        )
        .unwrap();
        let mut runner = FakeRunner::failing(&[2, 4]);

        let summary = run_steps(&mut runner, &scenario).await;

        assert_eq!(runner.ran.len(), 5, "every step ran: {:?}", runner.ran);
        assert_eq!(summary.scenario_name, "five");
        assert_eq!(summary.total_steps, 5);
        assert_eq!(summary.passed_steps, 3);
        assert_eq!(summary.failed_steps, 2);
        assert!(!summary.is_success());
        assert_eq!(summary.errors.len(), 2);
        assert!(
            summary.errors[0].starts_with("step 2 (click): "),
            "{:?}",
            summary.errors
        );
        assert!(
            summary.errors[1].starts_with("step 4 (assert_element): "),
            "{:?}",
            summary.errors
        );
    }

    #[tokio::test]
    async fn a_scenario_where_everything_passes_is_a_success() {
        let scenario =
            Scenario::from_json(r#"{"name": "n", "steps": [{"action": "refresh"}]}"#).unwrap();
        let summary = run_steps(&mut FakeRunner::failing(&[]), &scenario).await;
        assert!(summary.is_success());
        assert!(summary.errors.is_empty());

        let empty = Scenario::from_json(r#"{"name": "none", "steps": []}"#).unwrap();
        let summary = run_steps(&mut FakeRunner::failing(&[]), &empty).await;
        assert_eq!((summary.total_steps, summary.passed_steps), (0, 0));
        assert!(summary.is_success());
    }

    #[test]
    fn the_dashboard_is_the_shared_page_with_one_row_and_escapes_what_it_was_given() {
        let summary = RunSummary {
            scenario_name: "<b>login</b> & \"more\"".to_owned(),
            total_steps: 3,
            passed_steps: 1,
            failed_steps: 2,
            errors: vec![
                "step 2 (click): <script>alert(1)</script>".to_owned(),
                "step 3 (open): a & b".to_owned(),
            ],
            duration_secs: 1.5,
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dashboard.html");
        write_dashboard(&summary, &path).unwrap();
        let html = std::fs::read_to_string(&path).unwrap();

        assert!(html.contains("Scenario Dashboard"), "{html}");
        assert!(html.contains("&lt;b&gt;login&lt;/b&gt;"), "{html}");
        assert!(html.contains("FAIL"), "{html}");
        assert!(html.contains("2 of 3 steps failed"), "{html}");
        assert!(html.contains("step 3 (open): a &amp; b"), "{html}");
        assert!(html.contains("<svg"), "the shared page draws the pie");
        assert!(
            !html.contains("http-equiv=\"refresh\""),
            "a scenario run is finished: {html}"
        );
        assert!(!html.contains("<script>alert"), "{html}");
    }

    #[test]
    fn a_passing_scenario_is_one_passing_row() {
        let summary = RunSummary {
            scenario_name: "login".to_owned(),
            total_steps: 2,
            passed_steps: 2,
            duration_secs: 0.25,
            ..RunSummary::default()
        };
        let run = summary.snapshot();

        assert_eq!(run.results.len(), 1);
        assert_eq!(run.results[0].status, TestStatus::Passed);
        assert_eq!(run.results[0].message, "2 of 2 steps passed");
        assert!(run.finished);
    }

    #[test]
    fn a_dashboard_that_cannot_be_written_names_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "x").unwrap();
        let path = file.join("d.html");

        let error = write_dashboard(&RunSummary::default(), &path)
            .unwrap_err()
            .to_string();
        assert!(error.contains("d.html"), "{error}");
    }

    /// Replays a scenario against a real Chrome through chromedriver.
    ///
    /// Run with `cargo test --lib scenario -- --ignored`.
    #[tokio::test]
    #[ignore = "launches a real Chrome through chromedriver"]
    async fn runs_a_scenario_against_a_real_browser() {
        use crate::browser::config::BrowserConfig;
        use std::io::{Read, Write};

        const PAGE: &str = r#"<!doctype html><title>Scenario</title>
            <h1 id="t">Welcome</h1><input id="name">
            <button id="go" onclick="document.getElementById('t').textContent='Hello '+document.getElementById('name').value">Go</button>"#;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0_u8; 2048];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                        PAGE.len()
                    )
                    .as_bytes(),
                );
            }
        });

        let scenario = Scenario::from_json(&format!(
            r##"{{"name": "real", "steps": [
                {{"action": "open", "url": "http://{address}/"}},
                {{"action": "assert_text", "css": "#t", "contains": "Welcome"}},
                {{"action": "type_text", "css": "#name", "text": "Ada"}},
                {{"action": "click", "css": "#go"}},
                {{"action": "wait_for_text", "css": "#t", "text": "Hello Ada", "timeout": 5}},
                {{"action": "assert_element", "css": "#does-not-exist"}}
            ]}}"##
        ))
        .unwrap();

        let mut sb = BaseCase::new(BrowserConfig {
            headless: true,
            ..BrowserConfig::default()
        })
        .await
        .expect("Chrome starts");
        let summary = run_scenario(&mut sb, &scenario).await.unwrap();
        sb.quit().await.unwrap();

        assert_eq!(summary.total_steps, 6);
        assert_eq!(summary.passed_steps, 5, "{:?}", summary.errors);
        assert_eq!(summary.errors.len(), 1);
        assert!(
            summary.errors[0].starts_with("step 6 (assert_element): "),
            "{:?}",
            summary.errors
        );
    }
}
