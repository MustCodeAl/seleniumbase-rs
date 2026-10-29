//! Action recorder that turns browser interactions into Rust test scripts.
//!
//! [`ActionRecorder`] collects a sequence of [`RecordedAction`] values and can
//! emit a standalone Rust `main` function, a `#[tokio::test]` function, or a
//! saved test file. It is used by the recorder mode to replay manual browser
//! sessions as automated tests.
//!
//! [`RecorderSession`] holds a recorded action list: it saves and loads it as
//! JSON or YAML, and replays it against a [`BaseCase`](crate::BaseCase). The
//! replay runs every action the generators write, because both are made from one
//! table (`calls.rs`).
//!
//! A recording of `BaseCase` calls can be written for either engine:
//! [`to_rust_script`](ActionRecorder::to_rust_script) and
//! [`to_rust_test`](ActionRecorder::to_rust_test) call `BaseCase`, and
//! [`to_cdp_script`](ActionRecorder::to_cdp_script) and
//! [`to_cdp_test`](ActionRecorder::to_cdp_test) call the pure CDP engine
//! ([`sb_cdp`](crate::sb_cdp)), as Python's `--rec-sb-cdp` does.
//!
//! Recorder Mode (a person acting in a browser) is a pipeline of three steps,
//! each a port of the Python code: the page script ([`RECORDER_JS`]) stores
//! [`RawAction`]s, [`process`] rewrites them as Python's
//! `__process_recorded_actions` does, and [`to_actions`] writes them as calls.
//! [`ActionRecorder::from_raw_actions`] runs the last two, and
//! [`to_rstest_test`](ActionRecorder::to_rstest_test) writes the default
//! output: a test that uses the `sb` fixture.

mod calls;

mod cdp;
mod generate;
mod process;
mod raw;

mod breakpoint;
mod extras;
mod output;
mod state;

pub(crate) use breakpoint::{run_prompt, BreakpointExit, WATCH_TICK};
pub(crate) use extras::{extra_raw_actions, origin_of, recordable};
pub use output::{
    write_recording, RecordingFormat, RecordingOptions, RecordingTarget, SavedRecording,
    RECORDINGS_FOLDER,
};
pub use state::RecorderEnd;
pub(crate) use state::RecorderState;

pub use generate::to_actions;
pub use process::{add_parent_frame_switches, process};
pub use raw::RawAction;

/// The Recorder Mode page script: a port of SeleniumBase 4.56.2's
/// `recorder_js`, as `src/api/recorder/js/recorder.js`.
///
/// It records what a person does in the page as `[kind, a, b, time]` actions
/// in sessionStorage `recorded_actions` (see [`RawAction`]) and draws the red
/// (recording), purple (assert element, key `^`) and teal (assert text, key
/// `&`) border. ESC pauses; `~` or `` ` `` resumes.
pub const RECORDER_JS: &str = include_str!("recorder/js/recorder.js");

/// The script that ends a recording made with `mkrec --ee`: it sets
/// `document.sb_esc_end` to `"yes"` when SHIFT+ESC is pressed.
pub const SHIFT_ESC_JS: &str = include_str!("recorder/js/shift_esc.js");

/// [`RECORDER_JS`] wrapped for `Page.addScriptToEvaluateOnNewDocument`: it
/// runs when each page has loaded, once per document.
///
/// Python wraps the script in `window.onload = function() { ... }`; this adds
/// a `load` listener instead, so a page that sets its own `onload` neither
/// loses it nor removes the recorder.
///
/// The browser runs the script in every frame of a page, but the script
/// records the top frame only, as Python's Recorder does (its extension has no
/// `all_frames`): the load of a frame is not a page to open, and what a person
/// does in a frame is seen from the top frame, where the focus leaves the page.
/// A CAPTCHA widget (reCAPTCHA, hCaptcha, Turnstile) in a frame is one
/// `captcha` action instead, which the generators write as a comment.

pub fn recorder_injection() -> String {
    format!(
        "if (!document.__sbRecorderHooked) {{ document.__sbRecorderHooked = true; \
         window.addEventListener('load', function () {{\n{RECORDER_JS}\n}}); }}"
    )
}

/// Pauses the test with the browser open so a person can record by hand: the
/// function form of [`BaseCase::recorder_breakpoint`](crate::BaseCase::recorder_breakpoint),
/// for a test that reads best with the breakpoint as a plain call.
///
/// ```no_run
/// use seleniumbase_rs::api::recorder::recorder_breakpoint;
/// # async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// sb.open("https://example.com").await?;
/// recorder_breakpoint(sb, "Click what you want to test").await?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// See [`BaseCase::recorder_breakpoint`](crate::BaseCase::recorder_breakpoint).
pub async fn recorder_breakpoint(
    sb: &mut BaseCase,
    message: &str,
) -> Result<(), SeleniumBaseError> {
    sb.recorder_breakpoint(message).await
}

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::api::base_case::BaseCase;
use crate::SeleniumBaseError;

use calls::{gherkin_mark, rust_call, Mark, ALIASES};

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
    /// steps, in the wording Python SeleniumBase's `--rec-behave` writes
    /// (`Open "url"`, `Click "selector"`, `Type "text" into "selector"`,
    /// `Go back`), so a feature runs in either implementation. An argument
    /// that holds a double quote is written in single quotes, as Python does.
    /// An action with no such step (or text that has a line break or both
    /// kinds of quote, or typing no text, which the step cannot match) becomes
    /// a `# skipped:` comment, so a gap is never silent.
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

    /// The actions of a Recorder Mode recording: the raw actions the page
    /// script stored, [`process`]ed as Python processes them and written as
    /// calls by [`to_actions`].
    ///
    /// `start_ms` is the browser's `Date.now()` when recording began (older
    /// actions are dropped), `origins_to_save` the origins of the tabs that
    /// were harvested, and `uc` is Python's `--uc` output.
    pub fn from_raw_actions(
        raw: Vec<RawAction>,
        start_ms: i64,
        origins_to_save: &[String],
        uc: bool,
    ) -> Self {
        Self {
            actions: to_actions(&process(raw, start_ms, origins_to_save), uc),
        }
    }

    /// The default Recorder Mode output: one rstest test that asks for the
    /// `sb` fixture ([`RSTEST_PRELUDE`]) and runs the recorded calls in
    /// `sb.run(...)`, which owns the browser, the reruns and the evidence.
    ///
    /// `test_name` becomes the function name. With `uc` the test starts with
    /// `activate_cdp_mode`, as Python's `--uc` output does (the caller turns
    /// `type_text` into `press_keys` when it builds the actions).
    ///
    /// A password typed into the browser is read from the environment
    /// variable `SB_RECORDED_SECRET` when the test runs. An action the
    /// generator cannot turn into a call becomes a `// skipped:` comment.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::api::recorder::ActionRecorder;
    ///
    /// let mut recorder = ActionRecorder::default();
    /// recorder.record("goto", Some("https://example.com"), None);
    /// recorder.record("assert_text", Some("h1"), Some("Example Domain"));
    ///
    /// let test = recorder.to_rstest_test("test_recording", false);
    /// assert!(test.contains("async fn test_recording(sb: Sb) -> Result<()> {"));
    /// assert!(test.contains("        sb.goto(\"https://example.com\").await?;"));
    /// ```
    pub fn to_rstest_test(&self, test_name: &str, uc: bool) -> String {
        let secret = format!("std::env::var({})?", literal(SECRET_VARIABLE));
        let mut body = String::new();
        let mut needs_secret = false;
        if uc && !self.actions.is_empty() {
            body.push_str("        sb.activate_cdp_mode().await?;\n");
        }
        for action in &self.actions {
            let statement = match rust_call(action) {
                Ok(call) => {
                    needs_secret |= call.contains(&secret);
                    call.replace(&secret, "recorded_secret()?")
                }
                // `{:?}` in the reason keeps a hostile name from ending the comment.
                Err(reason) => format!("// skipped: {reason}"),
            };
            body.push_str("        ");
            body.push_str(&statement);
            body.push('\n');
        }
        let mut out = String::from(RSTEST_PRELUDE);
        if needs_secret {
            out.push_str("use seleniumbase_rs::api::recorder::recorded_secret;\n");
        }
        out.push_str(&format!(
            "\n#[rstest]\n#[tokio::test]\nasync fn {}(sb: Sb) -> Result<()> {{\n    sb.run(async |sb| {{\n{body}        Ok(())\n    }})\n    .await\n}}\n",
            rust_identifier(test_name)
        ));
        out
    }

    /// Gherkin in the wording of Python's `--rec-behave`
    /// (`behave/behave_helper.py`): `Feature:` and `Scenario:` named as given,
    /// then `Given Open "url"` and `And Click "selector"`, one step per
    /// action. With `uc` the first step is `Activate CDP Mode`.
    ///
    /// A password typed into the browser is `<SB_RECORDED_SECRET>`. An
    /// action with no step becomes a `# skipped:` comment.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::api::recorder::ActionRecorder;
    ///
    /// let mut recorder = ActionRecorder::default();
    /// recorder.record("goto", Some("https://example.com"), None);
    /// recorder.record("click", Some("a"), None);
    ///
    /// let feature = recorder.to_gherkin_scenario("RecorderTest", "test_recording", false);
    /// assert!(feature.starts_with("Feature: RecorderTest\n\n  Scenario: test_recording\n"));
    /// assert!(feature.contains("    Given Open \"https://example.com\"\n    And Click \"a\"\n"));
    /// ```
    pub fn to_gherkin_scenario(&self, feature: &str, scenario: &str, uc: bool) -> String {
        generate::gherkin_scenario(&self.actions, feature, scenario, uc)
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
    // Python quotes an argument with " and, for one that has a ", with '. An
    // argument with both, or a line break, would end the step early.
    let plain = |text: &str, what: &str| -> Result<String, String> {
        if text.contains(['\n', '\r']) || (text.contains('"') && text.contains('\'')) {
            Err(format!(
                "{name}: the {what} {text:?} has both kinds of quote or a line break"
            ))
        } else if text.contains('"') {
            Ok(format!("'{text}'"))
        } else {
            Ok(format!("\"{text}\""))
        }
    };
    let target = action
        .target
        .as_deref()
        .ok_or_else(|| format!("{name} has no target"));
    match gherkin_mark(name) {
        Some(Mark::GherkinOpen) => Ok(format!("Open {}", plain(target?, "URL")?)),
        Some(Mark::GherkinClick) => Ok(format!("Click {}", plain(target?, "selector")?)),
        Some(Mark::GherkinType) => {
            let value = action
                .value
                .as_deref()
                .ok_or_else(|| format!("{name} has no value"))?;
            // The step's pattern is `Type "(.+?)" into "(.+?)"`: no text, no match.
            if value.contains(SECRET_PLACEHOLDER) {
                return Err(format!(
                    "{name}: the text is a password, which a feature file does not keep"
                ));
            }

            if value.is_empty() {
                return Err(format!(
                    "{name}: the text is empty, and the Type step needs at least one character"
                ));
            }
            Ok(format!(
                "Type {} into {}",
                plain(value, "text")?,
                plain(target?, "selector")?
            ))
        }
        Some(Mark::GherkinBack) => Ok("Go back".to_owned()),
        Some(Mark::GherkinForward) => Ok("Go forward".to_owned()),
        // The name is whatever was recorded. Written plainly, a line break in it would end the
        // `# skipped:` comment and start a Scenario or a step, so it is escaped.
        _ => Err(format!(
            "{}: no Gherkin step for this action",
            name.escape_debug()
        )),
    }
}

/// A string as a Rust literal.
fn literal(text: &str) -> String {
    format!("{text:?}")
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
    /// Number of actions the replay has no call for: an unknown name, or an
    /// action the recording does not hold enough of to run.
    pub skipped: usize,
    /// The recorded names of the skipped actions, in replay order.
    pub skipped_names: Vec<String>,
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

/// What a recording holds in place of a password that was typed in the
/// browser. The password is not written to the file: a generated test reads it
/// from the `SB_RECORDED_SECRET` environment variable, and so does a replay.
/// Set `SB_REC_KEEP_SECRETS=true` before recording to keep the typed text.
pub const SECRET_PLACEHOLDER: &str = "{{SB_RECORDED_SECRET}}";

/// The environment variable that holds the password [`SECRET_PLACEHOLDER`]
/// stands for.
pub const SECRET_VARIABLE: &str = "SB_RECORDED_SECRET";

/// The lines every generated rstest test starts with: the `rstest` attribute
/// and the `sb` fixture with its handle. They are in one place so that a
/// change of the fixture's path is a change here.
pub const RSTEST_PRELUDE: &str =
    "use rstest::rstest;\nuse seleniumbase_rs::testing::{sb, Sb};\nuse seleniumbase_rs::Result;\n";

/// The password a recording stands for, read from `SB_RECORDED_SECRET`
/// ([`SECRET_VARIABLE`]): what a generated test types in place of a password
/// that was typed into the browser while recording.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] when the variable is not set.
pub fn recorded_secret() -> Result<String, SeleniumBaseError> {
    std::env::var(SECRET_VARIABLE).map_err(|_| {
        SeleniumBaseError::InvalidConfig(format!(
            "the recording holds a password; set {SECRET_VARIABLE} to type it"
        ))
    })
}

/// Whether `SB_REC_KEEP_SECRETS` asks for typed passwords to be recorded as
/// typed.
fn keep_secrets() -> bool {
    std::env::var("SB_REC_KEEP_SECRETS")
        .is_ok_and(|value| value.eq_ignore_ascii_case("true") || value == "1")
}

/// The Rust expression for a recorded text: a string literal, or, for the
/// placeholder of a typed password, the environment variable that holds it.
fn text_argument(text: &str) -> String {
    match text.strip_prefix(SECRET_PLACEHOLDER) {
        Some("") => format!("&std::env::var({})?", literal(SECRET_VARIABLE)),
        Some(rest) => format!(
            "&format!(\"{{}}{{}}\", std::env::var({})?, {})",
            literal(SECRET_VARIABLE),
            literal(rest)
        ),
        None => literal(text),
    }
}

/// The text to type for a recorded `text`: the password in `SB_RECORDED_SECRET`
/// for the placeholder of a typed password, `text` itself for anything else.
fn revealed(text: &str) -> Result<String, SeleniumBaseError> {
    let Some(rest) = text.strip_prefix(SECRET_PLACEHOLDER) else {
        return Ok(text.to_owned());
    };
    let secret = std::env::var(SECRET_VARIABLE).map_err(|_| {
        SeleniumBaseError::InvalidConfig(format!(
            "the recording holds a password; set {SECRET_VARIABLE} to type it"
        ))
    })?;
    Ok(format!("{secret}{rest}"))
}

/// A list of recorded actions that can be saved, loaded and replayed.
///
/// The actions come from a recording ([`BaseCase::recorder_session`] turns the
/// `BaseCase` calls of a test into one, and [`ActionRecorder::from_raw_actions`]
/// the actions of a person in Recorder Mode), from a file
/// ([`load`](RecorderSession::load)), or from [`push`](RecorderSession::push).
/// A session is written as JSON or YAML ([`save`](RecorderSession::save)), as a
/// Rust test ([`to_action_recorder`](RecorderSession::to_action_recorder)), and
/// run again with [`replay`](RecorderSession::replay).
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
}

impl RecorderSession {
    /// Creates an empty recording session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a session pre-populated with previously recorded actions.
    pub fn from_actions(actions: Vec<RecordedAction>) -> Self {
        Self { actions }
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

    /// Writes the recording to `path`. A `.rs` extension writes a Rust test named after the file, a `.yaml`/`.yml` extension selects YAML,
    /// anything else writes JSON.
    pub fn save(&self, path: &Path) -> Result<PathBuf, SeleniumBaseError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let data = if is_yaml_path(path) {
            self.to_yaml()
        } else if is_rust_path(path) {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("recorded_test");
            self.to_action_recorder().to_rust_test(stem)
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
}

/// Replaying a recording.
impl RecorderSession {
    /// Replays `actions` against `sb`, one step each, and reports how each went.
    ///
    /// Every action the recorder can generate a call for is replayed with the
    /// same `BaseCase` method and arguments (the table behind
    /// [`ActionRecorder::to_rust_script`] holds both), so a recording of Recorder
    /// Mode, or of a test's own `BaseCase` calls, replays as it is generated.
    /// The names `navigate`, `type` and `cdp_click_element` mean what they mean
    /// in generated code.
    ///
    /// An action whose name is not in the table, or has no call to replay
    /// (`drag_and_drop_with_offset`: the recording does not keep its offset), is
    /// counted in [`ReplayReport::skipped`] and named in
    /// [`ReplayReport::skipped_names`]. An action that lacks an argument it needs
    /// is a failed step that says which. The steps after a failed one still run.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`], before anything runs, when
    /// the recording types a password and `SB_RECORDED_SECRET` is not set.
    pub async fn replay(
        sb: &mut BaseCase,
        actions: &[RecordedAction],
    ) -> Result<ReplayReport, SeleniumBaseError> {
        calls::replay(sb, actions).await
    }
}

fn yaml_scalar(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
}

/// Whether `path` names a Rust source file, by its extension.
fn is_rust_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

/// Whether `path` names a YAML file, by its extension.
fn is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
}

#[cfg(test)]
mod tests {
    use super::calls::{Shape, CALLS, UNEXPORTABLE};
    use super::*;

    #[test]
    fn the_page_script_records_the_top_frame_of_a_document_once() {
        // The browser's new-document hook runs the script in every frame, and
        // the tab watcher may run it again in a page that is still loading;
        // both guards come before the script defines or records anything.
        let top_frame = RECORDER_JS
            .find("if (window.self !== window.top) return;")
            .expect("the script leaves a frame at once");
        let once = RECORDER_JS
            .find("if (document.__sbRecorderRan === true) return;")
            .expect("the script runs once for each document");
        let first_definition = RECORDER_JS
            .find("var cssPathById")
            .expect("the script defines its selectors");
        assert!(top_frame < once && once < first_definition);
        // A CAPTCHA frame is one `captcha` action, not a step or a tab.
        assert!(RECORDER_JS.contains("['captcha', provider, window.location.origin, d_now]"));
        // The wrapper for the new-document hook carries the same script.
        assert!(recorder_injection().contains(RECORDER_JS));
    }

    #[test]

    fn a_captcha_is_a_comment_in_every_form_of_the_output() {
        let mut recorder = ActionRecorder::default();
        recorder.record("goto", Some("https://a.test/"), None);
        recorder.record("captcha", Some("recaptcha"), None);
        recorder.record("click", Some("#after"), None);
        let note = "skipped: captcha (recaptcha): solve it by hand or use";

        let script = recorder.to_rust_script();
        assert!(
            script.contains(&format!("// {note} sb.solve_captcha()")),
            "{script}"
        );
        assert!(script.contains("sb.click(\"#after\")"), "{script}");
        syn::parse_file(&script).expect("a comment leaves a valid program");

        let test = recorder.to_rstest_test("test_x", false);
        assert!(
            test.contains(&format!("// {note} sb.solve_captcha()")),
            "{test}"
        );

        let cdp = recorder.to_cdp_script();
        assert!(
            cdp.contains(&format!("// {note} page.solve_captcha()")),
            "{cdp}"
        );
        syn::parse_file(&cdp).expect("a comment leaves a valid program");

        let feature = recorder.to_gherkin_scenario("F", "S", false);
        assert!(
            feature.contains(&format!("# {note} sb.solve_captcha()")),
            "{feature}"
        );
    }

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
    fn a_cdp_click_is_generated_and_the_deprecated_name_is_written_as_cdp_click() {
        for recorded in ["cdp_click", "cdp_click_element"] {
            let mut recorder = ActionRecorder::default();
            recorder.record(recorded, Some("#go"), None);
            let script = recorder.to_rust_script();
            assert!(
                script.contains(r##"sb.cdp_click("#go").await?;"##),
                "{recorded}: {script}"
            );
            assert!(!script.contains("cdp_click_element"), "{recorded}");
            assert!(!script.contains("// skipped"), "{recorded}: {script}");
        }
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
            script.contains("sb.goto(\"https://example.com\").await?;"),
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
            skipped_names: vec!["levitate".to_owned()],
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
                // `x: impl Into<f64>` takes any number literal.
                syn::Type::ImplTrait(_) => "impl Into".to_owned(),

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
            Shape::NoArgs | Shape::Seconds | Shape::TabUrl => (None, None),
            Shape::Target | Shape::TargetDiscarded | Shape::TargetSeconds => (Some("#t"), None),
            Shape::TargetValue | Shape::ValueTarget | Shape::Cookie => (Some("#t"), Some("v")),
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
                syn::Lit::Bool(_) => "bool",
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
                    "impl Into" => kind == "integer" || kind == "float",
                    "&Value" => matches!(argument, syn::Expr::Reference(_)),

                    "bool" => kind == "bool",
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
        // `goto` is a call of its own, as Python's recorder writes it;
        // `navigate` is another name for `open`.
        assert_eq!(
            call("goto", Some("https://example.com"), None).unwrap(),
            "sb.goto(\"https://example.com\").await?;"
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
            call(
                "double_click_with_offset",
                Some("#a"),
                Some("10, -4, center")
            )
            .unwrap(),
            "sb.double_click_with_offset(\"#a\", 10, -4).center(true).await?;"
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
            "// skipped: drag_and_drop_with_offset: the recording keeps the selector",
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
    fn goto_if_not_url_is_generated_for_every_output() {
        let mut recorder = ActionRecorder::default();
        recorder.record("goto", Some("http://x.test/a"), None);
        recorder.record("click", Some("#next"), None);
        recorder.record("goto_if_not_url", Some("http://x.test/b"), None);
        let script = recorder.to_rust_script();
        assert!(
            script.contains("sb.goto_if_not_url(\"http://x.test/b\").await?;"),
            "{script}"
        );
        assert!(!script.contains("skipped"), "{script}");
        let cdp = recorder.to_cdp_script();
        assert!(cdp.contains("page.goto(\"http://x.test/b\")"), "{cdp}");
        assert!(!cdp.contains("skipped"), "{cdp}");
        let feature = recorder.to_gherkin("Two pages");
        assert!(
            feature.contains("And Open \"http://x.test/b\""),
            "{feature}"
        );
    }

    // ---- passwords and Rust output -------------------------------------------

    #[test]
    fn a_password_placeholder_is_generated_as_the_environment_variable() {
        let mut recorder = ActionRecorder::default();
        recorder.record("type", Some("#pw"), Some("{{SB_RECORDED_SECRET}}"));
        recorder.record("type", Some("#pw2"), Some("{{SB_RECORDED_SECRET}}\n"));
        let script = recorder.to_rust_script();
        assert!(
            script
                .contains("sb.type_text(\"#pw\", &std::env::var(\"SB_RECORDED_SECRET\")?).await?;"),
            "{script}"
        );
        assert!(
            script.contains(
                "sb.type_text(\"#pw2\", &format!(\"{}{}\", std::env::var(\"SB_RECORDED_SECRET\")?, \"\\n\")).await?;"
            ),
            "{script}"
        );
        assert!(!script.contains("skipped"), "{script}");
        syn::parse_file(&script).expect("generated script should parse");

        // The other outputs cannot read the variable, and say so.
        let cdp = recorder.to_cdp_script();
        assert!(!cdp.contains("{{SB"), "{cdp}");
        assert!(
            cdp.contains("// skipped: type: the text is a password"),
            "{cdp}"
        );
        let feature = recorder.to_gherkin("Login");
        assert!(!feature.contains("{{SB"), "{feature}");
        assert!(
            feature.contains("# skipped: type_text: the text is a password"),
            "{feature}"
        );
    }

    #[test]
    fn a_recorded_password_is_read_from_the_environment_when_replayed() {
        assert_eq!(revealed("ada").unwrap(), "ada");
        // Not set here: the replay says what to set.
        if std::env::var(SECRET_VARIABLE).is_err() {
            let error = revealed("{{SB_RECORDED_SECRET}}").unwrap_err().to_string();
            assert!(error.contains("SB_RECORDED_SECRET"), "{error}");
        }
    }

    #[test]
    fn a_recording_saved_with_the_rs_extension_is_a_rust_test() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().join("tests").join("search_flow.rs");
        let saved = sample_session().save(&path).expect("saved");
        let test = std::fs::read_to_string(saved).unwrap();
        assert!(
            test.contains("#[tokio::test]\nasync fn search_flow()"),
            "{test}"
        );
        assert!(test.contains("sb.click(\"#submit\").await?;"), "{test}");
        syn::parse_file(&test).expect("the test should parse");
    }

    // ---- Gherkin -------------------------------------------------------------

    fn gherkin_recorder() -> ActionRecorder {
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com"), None);
        recorder.record("type_text", Some("#name"), Some("Ada"));
        recorder.record("click", Some("#go"), None);
        recorder.record("go_back", None, None);
        recorder.record("hover", Some("#menu"), None);
        recorder.record("type_text", Some("#q"), Some("say \"hi\" and 'bye'"));
        recorder
    }

    #[test]
    fn a_recording_becomes_a_gherkin_scenario_and_says_what_it_skipped() {
        let feature = gherkin_recorder().to_gherkin("Sign up");
        assert_eq!(
            feature,
            "Feature: Sign up\n\n  Scenario: Sign up\n\
             \x20   Given Open \"https://example.com\"\n\
             \x20   And Type \"Ada\" into \"#name\"\n\
             \x20   And Click \"#go\"\n\
             \x20   And Go back\n\
             \x20   # skipped: hover: no Gherkin step for this action\n\
             \x20   # skipped: type_text: the text \"say \\\"hi\\\" and 'bye'\" has both kinds of quote or a line break\n"
        );
    }

    #[test]
    fn an_argument_with_a_double_quote_is_written_in_single_quotes_as_python_does() {
        let mut recorder = ActionRecorder::default();
        recorder.record("open", Some("https://example.com/?q=\"x\""), None);
        recorder.record("click", Some("a[title=\"Go\"]"), None);
        recorder.record("type_text", Some("#q"), Some("say \"hi\""));

        let feature = recorder.to_gherkin("Quotes");

        assert_eq!(
            feature,
            "Feature: Quotes\n\n  Scenario: Quotes\n\
                 \x20   Given Open 'https://example.com/?q=\"x\"'\n\
                 \x20   And Click 'a[title=\"Go\"]'\n\
                 \x20   And Type 'say \"hi\"' into \"#q\"\n"
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
        let parsed = crate::cli::scripts::gherkin_outline::parse_feature(&lines);
        assert_eq!(parsed.scenarios.len(), 1, "{feature}");
        let steps: Vec<String> = parsed.scenarios[0]
            .steps
            .iter()
            .map(crate::cli::scripts::gherkin_outline::Step::full_text)
            .collect();
        assert_eq!(
            steps,
            ["Given Open \"https://example.com\"", "And Click \"#ok\""],
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
        assert!(
            feature.contains("    # skipped: type_text: the text is empty"),
            "{feature}"
        );
    }

    #[test]
    fn every_gherkin_step_is_a_step_the_outline_reader_reads() {
        use crate::cli::scripts::gherkin_outline::{parse_feature, Step};
        // This checks the shape of what is written; with the feature `bdd` it
        // also checks that each step is a built-in step.
        let feature = gherkin_recorder().to_gherkin("Sign up");
        let lines: Vec<String> = feature.lines().map(String::from).collect();
        let parsed = parse_feature(&lines);
        let steps: Vec<String> = parsed.scenarios[0]
            .steps
            .iter()
            .map(Step::full_text)
            .collect();
        assert_eq!(steps.len(), 4, "{feature}");
        assert!(steps[0].starts_with("Given Open "), "{steps:?}");

        // With the feature `bdd`, every step the recorder writes is read by
        // exactly one built-in step of `testing::bdd::steps`, whichever
        // command it records and however the arguments are quoted.
        #[cfg(feature = "bdd")]
        {
            let mut recorder = gherkin_recorder();
            recorder.record("go_forward", None, None);
            recorder.record(
                "goto_if_not_url",
                Some("https://example.com/?q=\"x\""),
                None,
            );
            recorder.record("click", Some("a[title=\"Go\"]"), None);
            recorder.record("type_text", Some("#q"), Some("say \"hi\""));
            let feature = recorder.to_gherkin("Every command");
            let lines: Vec<String> = feature.lines().map(String::from).collect();
            let parsed = parse_feature(&lines);
            assert!(parsed.scenarios[0].steps.len() >= 8, "{feature}");
            for step in &parsed.scenarios[0].steps {
                let actions = crate::testing::bdd::steps::matching_actions(&step.text);
                assert_eq!(
                    actions.len(),
                    1,
                    "{} reads {actions:?}\n{feature}",
                    step.text
                );
            }
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
