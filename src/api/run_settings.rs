//! Run-wide settings for a [`BaseCase`](crate::BaseCase): the Python
//! `--demo`, `--slow`, `--fast`, `--demo-sleep`, `--message-duration`,
//! `--skip-js-waits`, `--record-sleep` and `--visual-baseline` options.
//!
//! Python reads these from the pytest command line. Here they are plain data a
//! test hands to [`BaseCase::set_run_settings`](crate::BaseCase::set_run_settings):
//!
//! ```
//! use std::time::Duration;
//! use seleniumbase_rs::api::run_settings::RunSettings;
//!
//! let settings = RunSettings::default()
//!     .with_slow_mode(true)
//!     .with_demo_sleep(Duration::from_millis(500));
//! assert_eq!(settings.pause(false), Duration::from_millis(500));
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

use crate::api::html::BeautifulSoup;
use crate::artifacts::{confined_path, safe_stem};
use crate::error::SeleniumBaseError;

/// How long demo mode and slow mode pause after an action unless
/// [`RunSettings::with_demo_sleep`] says otherwise (Python's
/// `DEFAULT_DEMO_MODE_TIMEOUT`, 0.5 s in 4.55.2).
pub const DEFAULT_DEMO_PAUSE: Duration = Duration::from_millis(500);

/// How long [`BaseCase::post_message`](crate::BaseCase::post_message) shows a
/// message unless [`RunSettings::with_message_duration`] says otherwise.
pub const DEFAULT_MESSAGE_SECONDS: u64 = 3;

/// The settings that change how a whole test run behaves.
///
/// The default changes nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunSettings {
    demo_mode: bool,
    slow_mode: bool,
    demo_sleep: Option<Duration>,
    message_duration: Option<Duration>,
    skip_js_waits: bool,
    record_sleep: bool,
    reset_visual_baseline: bool,
}

impl RunSettings {
    /// Highlight each element and pause after `open`, `click`, `type_text`,
    /// `update_text` and `press_keys`, so a person can follow the run (`--demo`).
    #[must_use]
    pub fn with_demo_mode(mut self, on: bool) -> Self {
        self.demo_mode = on;
        self
    }

    /// Pause after those same actions without highlighting (`--slow`).
    #[must_use]
    pub fn with_slow_mode(mut self, on: bool) -> Self {
        self.slow_mode = on;
        self
    }

    /// Python's `--fast`. It does nothing here and never did what this method
    /// first claimed: Python reads `--fast` only in CDP mode, where it skips
    /// short waits this crate never makes. It does not cancel demo or slow mode.
    #[deprecated(
        since = "0.5.0",
        note = "Python's --fast skips CDP-mode waits this crate never makes; it has no effect here"
    )]
    #[must_use]
    pub fn with_fast_mode(self, _on: bool) -> Self {
        self
    }

    /// The pause demo mode and slow mode make after an action, instead of
    /// [`DEFAULT_DEMO_PAUSE`] (`--demo-sleep`).
    #[must_use]
    pub fn with_demo_sleep(mut self, pause: Duration) -> Self {
        self.demo_sleep = Some(pause);
        self
    }

    /// How long `post_message` keeps a message on the page, rounded up to whole
    /// seconds (`--message-duration`).
    #[must_use]
    pub fn with_message_duration(mut self, duration: Duration) -> Self {
        self.message_duration = Some(duration);
        self
    }

    /// Make `wait_for_ready_state_complete` return at once (`--skip-js-waits`).
    /// A direct call to `wait_for_angularjs` still waits.
    #[must_use]
    pub fn with_skip_js_waits(mut self, on: bool) -> Self {
        self.skip_js_waits = on;
        self
    }

    /// Put `BaseCase::sleep` calls into the recording (`--record-sleep`).
    #[must_use]
    pub fn with_record_sleep(mut self, on: bool) -> Self {
        self.record_sleep = on;
        self
    }

    /// Make `check_window` replace its stored baseline screenshot instead of
    /// comparing with it (`--visual-baseline`).
    #[must_use]
    pub fn with_visual_baseline_reset(mut self, on: bool) -> Self {
        self.reset_visual_baseline = on;
        self
    }

    /// Whether demo mode is on.
    #[must_use]
    pub fn demo_mode(&self) -> bool {
        self.demo_mode
    }

    /// Whether `wait_for_ready_state_complete` returns at once.
    #[must_use]
    pub fn skip_js_waits(&self) -> bool {
        self.skip_js_waits
    }

    /// Whether `sleep` calls are recorded.
    #[must_use]
    pub fn record_sleep(&self) -> bool {
        self.record_sleep
    }

    /// Whether `check_window` replaces its baseline.
    #[must_use]
    pub fn reset_visual_baseline(&self) -> bool {
        self.reset_visual_baseline
    }

    /// How long to pause after an action.
    ///
    /// Zero when neither demo nor slow mode is on. In demo mode
    /// a `tiny` action (typing) pauses for 1/3.4 of the full time, as in
    /// Python. Slow mode always pauses for the full time.
    #[must_use]
    pub fn pause(&self, tiny: bool) -> Duration {
        if !(self.demo_mode || self.slow_mode) {
            return Duration::ZERO;
        }
        let full = self.demo_sleep.unwrap_or(DEFAULT_DEMO_PAUSE);
        if self.demo_mode && tiny {
            full.div_f64(3.4)
        } else {
            full
        }
    }

    /// The whole seconds `post_message` keeps a message on the page.
    #[must_use]
    pub fn message_seconds(&self) -> u64 {
        self.message_duration
            .map_or(DEFAULT_MESSAGE_SECONDS, |d| d.as_secs_f64().ceil() as u64)
    }
}

/// The `level` argument of `check_window`: 0 (a dry run), 1, 2 or 3.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for any other value.
pub(crate) fn check_level(level: f64) -> Result<u8, SeleniumBaseError> {
    (0..=3)
        .find(|&allowed| f64::from(allowed) == level)
        .ok_or_else(|| {
            SeleniumBaseError::InvalidConfig(format!(
                "Parameter \"level\" must be set to 0, 1, 2, or 3, not {level}!"
            ))
        })
}

/// The folder of the baseline `check_window` keeps for the check `name`:
/// `visual_baseline/<test>/<name>`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is not a plain name.
pub(crate) fn baseline_dir(name: &str) -> Result<PathBuf, SeleniumBaseError> {
    let name = if name.is_empty() { "default" } else { name };
    confined_path(&test_baseline_folder(), name)
}

/// The running test's folder of visual baselines: `visual_baseline/<test>`.
pub(crate) fn test_baseline_folder() -> PathBuf {
    // Python's test id is the test's name. Rust has no such thing, but the
    // test harness names the thread that runs a test after it.
    let thread = std::thread::current();
    let path = thread.name().unwrap_or_default();
    let test = safe_stem(path.rsplit("::").next().unwrap_or(path), "default");
    Path::new("visual_baseline").join(test)
}

/// The running test's folder of pixel baselines for
/// [`BaseCase::assert_visual_match`](crate::BaseCase::assert_visual_match):
/// `visual_baseline/<test id>`; see `test_id`.
///
/// Unlike `check_window`'s folder, which keeps Python's name (the test
/// function alone), it tells apart tests of the same name in other modules
/// or test files, so none compares its screenshot with another's.
#[cfg(feature = "visual")]
pub(crate) fn visual_match_folder() -> PathBuf {
    let thread = std::thread::current();
    let exe = std::env::current_exe().ok();
    let binary = exe
        .as_deref()
        .and_then(Path::file_stem)
        .and_then(std::ffi::OsStr::to_str);
    Path::new("visual_baseline").join(test_id(thread.name().unwrap_or_default(), binary))
}

/// The longest [`test_id`]; a longer one keeps its end, the most specific
/// part.
#[cfg(feature = "visual")]
const MAX_TEST_ID_LEN: usize = 120;

/// A file name for the test run by the thread named `thread` in the program
/// `binary` (its file name without the extension): the test binary's name
/// when it is one cargo built (`cart` for `cart-0123456789abcdef`), then the
/// module path and the test function, joined with `.`, as
/// `cart.checkout.renders` for `checkout::renders` in `tests/cart.rs`.
///
/// Each part keeps only ASCII letters, digits, `-` and `_`, so the id is one
/// plain file name; `default` when nothing is left.
#[cfg(feature = "visual")]
fn test_id(thread: &str, binary: Option<&str>) -> String {
    let parts: Vec<String> = binary
        .and_then(cargo_test_binary)
        .into_iter()
        .chain(thread.split("::"))
        .map(crate::artifacts::replace_unsafe)
        .filter(|part| !part.is_empty())
        .collect();
    let id = parts.join(".");
    let start = id.len().saturating_sub(MAX_TEST_ID_LEN);
    // Only ASCII is left, so any byte index is a character boundary.
    let id = id[start..].trim_start_matches('.');
    if id.is_empty() {
        "default".to_owned()
    } else {
        id.to_owned()
    }
}

/// The crate or test-file name of a test binary cargo built, whose file name
/// is that name, a dash and a 16-digit hexadecimal hash.
#[cfg(feature = "visual")]
fn cargo_test_binary(stem: &str) -> Option<&str> {
    let (name, hash) = stem.rsplit_once('-')?;
    let is_hash = hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    (is_hash && !name.is_empty()).then_some(name)
}

/// The files of a baseline that must all be there for it to be compared.
const BASELINE_FILES: [&str; 5] = [
    "baseline.png",
    "page_url.txt",
    "tags_level_1.txt",
    "tags_level_2.txt",
    "tags_level_3.txt",
];

/// What each level of `tags_level_N.txt` fails on, as Python words it.
const LEVEL_FAILURES: [&str; 3] = [
    "HTML tags don't match the baseline!",
    "HTML tag attribute names don't match the baseline!",
    "HTML tag attribute values don't match the baseline!",
];

/// The tags below `<body>` in document order, at the three levels: `[tag]`,
/// `[tag, [attribute names]]` and `[tag, [[name, value]]]`, attributes sorted
/// by name. These are the entries of Python's `tags_level_1.txt` to
/// `tags_level_3.txt`.
fn tag_levels(html: &str) -> Result<[Vec<Value>; 3], SeleniumBaseError> {
    let mut levels: [Vec<Value>; 3] = Default::default();
    for node in BeautifulSoup::parse(html).find_all("body *")? {
        let tag = &node.tag_name;
        let mut attributes: Vec<_> = node.attributes.iter().collect();
        attributes.sort();
        let names: Vec<_> = attributes.iter().map(|(name, _)| name).collect();
        let pairs: Vec<_> = attributes
            .iter()
            .map(|(name, value)| json!([name, attribute_value(tag, name, value)]))
            .collect();
        levels[0].push(json!([tag]));
        levels[1].push(json!([tag, names]));
        levels[2].push(json!([tag, pairs]));
    }
    Ok(levels)
}

/// An attribute's value as BeautifulSoup gives it: a list of words for the
/// attributes that hold a list (`class`, `rel` on a link, ...), else a string.
fn attribute_value(tag: &str, name: &str, value: &str) -> Value {
    let is_list = matches!(
        (tag, name),
        (_, "class" | "accesskey" | "dropzone")
            | ("a" | "link" | "area", "rel")
            | ("a" | "link", "rev")
            | ("td" | "th", "headers")
            | ("form", "accept-charset")
            | ("object", "archive")
            | ("icon", "sizes")
            | ("iframe", "sandbox")
            | ("output", "for")
    );
    if is_list {
        json!(value.split_whitespace().collect::<Vec<_>>())
    } else {
        json!(value)
    }
}

/// `scheme://host[:port]` of `url`, as Python's `get_domain_url` gives it.
fn domain(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(parsed) if parsed.origin().is_tuple() => parsed.origin().ascii_serialization(),
        Ok(parsed) => format!("{}://", parsed.scheme()),
        Err(_) => url.to_owned(),
    }
}

/// Where the entries first differ, if they do.
fn first_difference(saved: &[Value], current: &[Value]) -> Option<String> {
    match saved.iter().zip(current).position(|(old, new)| old != new) {
        Some(at) => Some(format!(
            "The first difference is at element {}: the baseline has {} and the page has {}.",
            at + 1,
            saved[at],
            current[at]
        )),
        None if saved.len() != current.len() => Some(format!(
            "The baseline has {} elements and the page has {}.",
            saved.len(),
            current.len()
        )),
        None => None,
    }
}

fn write_baseline(dir: &Path, file: &str, bytes: &[u8]) -> Result<(), SeleniumBaseError> {
    std::fs::write(dir.join(file), bytes).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to write {file} of the baseline: {e}"))
    })
}

fn read_baseline(dir: &Path, file: &str) -> Result<String, SeleniumBaseError> {
    std::fs::read_to_string(dir.join(file)).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to read {file} of the baseline: {e}"))
    })
}

/// Saves the structure of the page in `dir` as the baseline, or compares it
/// with the baseline that is there.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::AssertionFailed`] for a difference and another
/// error if the baseline cannot be read or written.
pub(crate) fn check_page_structure(
    dir: &Path,
    url: &str,
    html: &str,
    png: &[u8],
    reset: bool,
    level: u8,
) -> Result<(), SeleniumBaseError> {
    let current = tag_levels(html)?;
    if reset || !BASELINE_FILES.iter().all(|file| dir.join(file).is_file()) {
        std::fs::create_dir_all(dir).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to create the baseline folder: {e}"))
        })?;
        write_baseline(dir, "baseline.png", png)?;
        write_baseline(dir, "page_url.txt", url.as_bytes())?;
        for (file, entries) in BASELINE_FILES[2..].iter().zip(&current) {
            write_baseline(dir, file, serde_json::to_string(entries)?.as_bytes())?;
        }
        return Ok(());
    }
    write_baseline(dir, "latest.png", png)?;
    if domain(read_baseline(dir, "page_url.txt")?.trim()) != domain(url) {
        return Err(SeleniumBaseError::AssertionFailed(
            "Page Domain Mismatch Failure: Current Page Domain doesn't match the Page Domain \
             of the Baseline! Can't compare two completely different sites! Reset the \
             baseline with RunSettings::with_visual_baseline_reset."
                .to_owned(),
        ));
    }
    // Level 0 is the dry run: it compares all three and reports every difference.
    let compared = if level == 0 {
        0..3
    } else {
        usize::from(level) - 1..usize::from(level)
    };
    let mut failures = Vec::new();
    for index in compared {
        let saved: Vec<Value> =
            serde_json::from_str(&read_baseline(dir, BASELINE_FILES[index + 2])?)?;
        if let Some(difference) = first_difference(&saved, &current[index]) {
            failures.push(format!(
                "<Level {}> Visual Diff Failure: {} {difference}",
                index + 1,
                LEVEL_FAILURES[index]
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(SeleniumBaseError::AssertionFailed(failures.join("\n")))
    }
}

/// What `check_window` does with the outcome of comparing a window with its
/// baseline, given the `level` the caller asked for.
///
/// Level 0 is a dry run, as in Python SeleniumBase: a difference is reported
/// on stderr but does not fail the test. Any other level fails on a difference.
/// A baseline that cannot be read or written is a real error at every level.
///
/// # Errors
///
/// Returns `result`'s error unless it is a difference found at level 0.
pub(crate) fn apply_check_level(
    result: Result<(), SeleniumBaseError>,
    level: f64,
) -> Result<(), SeleniumBaseError> {
    match result {
        Err(SeleniumBaseError::AssertionFailed(difference)) if level <= 0.0 => {
            tracing::warn!(%difference, "check_window found a difference at level 0 (a dry run)");
            Ok(())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn difference() -> Result<(), SeleniumBaseError> {
        Err(SeleniumBaseError::AssertionFailed(
            "Visual regression detected for 'home'".to_owned(),
        ))
    }

    #[test]
    fn level_zero_is_a_dry_run_that_does_not_fail_on_a_difference() {
        assert!(apply_check_level(difference(), 0.0).is_ok());
        assert!(apply_check_level(Ok(()), 0.0).is_ok());
    }

    #[test]
    fn every_other_level_fails_on_a_difference() {
        for level in [1.0, 2.0, 3.0] {
            let error = apply_check_level(difference(), level).unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::AssertionFailed(_)),
                "level {level}: {error:?}"
            );
        }
        assert!(apply_check_level(Ok(()), 3.0).is_ok());
    }

    #[test]
    fn level_zero_still_fails_when_the_baseline_cannot_be_read() {
        let unreadable = Err(SeleniumBaseError::InvalidConfig(
            "failed to read baseline: denied".to_owned(),
        ));
        assert!(matches!(
            apply_check_level(unreadable, 0.0),
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
    }

    #[test]
    fn the_default_demo_pause_is_python_half_a_second() {
        let settings = RunSettings::default().with_demo_mode(true);
        assert_eq!(settings.pause(false), Duration::from_millis(500));
        assert_eq!(DEFAULT_DEMO_PAUSE, Duration::from_millis(500));
    }

    #[test]
    #[allow(deprecated)]
    fn fast_mode_leaves_demo_and_slow_pauses_alone() {
        let settings = RunSettings::default()
            .with_demo_mode(true)
            .with_slow_mode(true)
            .with_fast_mode(true);
        assert_eq!(settings.pause(false), DEFAULT_DEMO_PAUSE);
    }

    #[test]
    fn the_default_changes_nothing() {
        let settings = RunSettings::default();
        assert_eq!(settings.pause(false), Duration::ZERO);
        assert_eq!(settings.pause(true), Duration::ZERO);
        assert_eq!(settings.message_seconds(), DEFAULT_MESSAGE_SECONDS);
        assert!(!settings.skip_js_waits() && !settings.record_sleep());
    }

    #[test]
    fn slow_mode_pauses_for_the_full_time_even_for_typing() {
        let settings = RunSettings::default().with_slow_mode(true);
        assert_eq!(settings.pause(false), DEFAULT_DEMO_PAUSE);
        assert_eq!(settings.pause(true), DEFAULT_DEMO_PAUSE);
    }

    #[test]
    fn demo_mode_pauses_a_shorter_time_for_typing() {
        let settings = RunSettings::default()
            .with_demo_mode(true)
            .with_demo_sleep(Duration::from_secs(34));
        assert_eq!(settings.pause(false), Duration::from_secs(34));
        assert_eq!(settings.pause(true), Duration::from_secs(10));
    }

    #[test]
    fn a_message_lasts_whole_seconds_rounded_up() {
        let settings = RunSettings::default().with_message_duration(Duration::from_millis(1500));
        assert_eq!(settings.message_seconds(), 2);
        let settings = settings.with_message_duration(Duration::from_secs(7));
        assert_eq!(settings.message_seconds(), 7);
    }

    fn case(settings: RunSettings) -> crate::BaseCase {
        let mut case = crate::BaseCase::without_session(crate::BrowserConfig::default());
        case.set_run_settings(settings);
        case
    }

    #[tokio::test]
    async fn sleep_is_recorded_only_when_asked() {
        let quiet = case(RunSettings::default());
        quiet.sleep(0.0).await;
        assert!(quiet.recorded_actions().unwrap().is_empty());

        let recording = case(RunSettings::default().with_record_sleep(true));
        recording.sleep(0.0).await;
        let actions = recording.recorded_actions().unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].name, "sleep");
        assert_eq!(actions[0].value.as_deref(), Some("0"));
    }

    #[tokio::test]
    async fn skipping_the_js_waits_returns_before_asking_the_page() {
        // Without a browser the page cannot be asked, so a wait that did ask
        // would run to its timeout and fail.
        let started = std::time::Instant::now();
        case(RunSettings::default().with_skip_js_waits(true))
            .wait_for_ready_state_complete()
            .await
            .expect("skipped");
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_case_keeps_the_settings_it_is_given() {
        let settings = RunSettings::default().with_demo_mode(true);
        let case = case(settings.clone());
        assert_eq!(case.run_settings(), &settings);
    }

    /// A page with a `head`, which no level looks at, and two tags in `body`.
    const PAGE: &str = r#"<!doctype html><html><head><title>t</title><meta charset="utf-8"></head><body><div id="a" class="x y"><p data-k="v">Hi</p></div></body></html>"#;
    const URL: &str = "https://example.com/start";

    /// A folder holding the baseline of `PAGE`, saved by a first check.
    fn baseline() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary folder");
        check(&dir, URL, PAGE, 3).expect("the first check saves the baseline");
        dir
    }

    fn check(
        dir: &tempfile::TempDir,
        url: &str,
        html: &str,
        level: u8,
    ) -> Result<(), SeleniumBaseError> {
        check_page_structure(dir.path(), url, html, b"png", false, level)
    }

    /// Whether the check at `level` fails with a difference.
    fn differs(html: &str, level: u8) -> bool {
        match check(&baseline(), URL, html, level) {
            Ok(()) => false,
            Err(SeleniumBaseError::AssertionFailed(_)) => true,
            Err(other) => panic!("not a difference: {other:?}"),
        }
    }

    #[test]
    fn the_three_levels_list_the_tags_below_body_in_document_order() {
        let levels = tag_levels(PAGE).unwrap();

        assert_eq!(levels[0], [json!(["div"]), json!(["p"])]);
        assert_eq!(
            levels[1],
            [json!(["div", ["class", "id"]]), json!(["p", ["data-k"]])]
        );
        assert_eq!(
            levels[2],
            [
                json!(["div", [["class", ["x", "y"]], ["id", "a"]]]),
                json!(["p", [["data-k", "v"]]])
            ]
        );
    }

    #[test]
    fn the_first_check_saves_the_baseline_whatever_the_level() {
        for level in 0..=3 {
            let dir = tempfile::tempdir().unwrap();
            let nested = dir.path().join("test").join("home");

            check_page_structure(&nested, URL, PAGE, b"png", false, level).unwrap();

            assert_eq!(std::fs::read(nested.join("baseline.png")).unwrap(), b"png");
            assert_eq!(
                std::fs::read_to_string(nested.join("page_url.txt")).unwrap(),
                URL
            );
            let levels = tag_levels(PAGE).unwrap();
            for (index, saved) in levels.iter().enumerate() {
                let file = nested.join(format!("tags_level_{}.txt", index + 1));
                let text = std::fs::read_to_string(file).unwrap();
                assert_eq!(&serde_json::from_str::<Vec<Value>>(&text).unwrap(), saved);
            }
        }
    }

    #[test]
    fn a_repainted_page_with_the_same_structure_is_not_a_difference() {
        let dir = baseline();

        check_page_structure(dir.path(), URL, PAGE, b"another picture", false, 3).unwrap();

        assert_eq!(
            std::fs::read(dir.path().join("baseline.png")).unwrap(),
            b"png"
        );
        assert_eq!(
            std::fs::read(dir.path().join("latest.png")).unwrap(),
            b"another picture"
        );
    }

    #[test]
    fn a_change_of_text_is_not_a_difference_at_any_level() {
        // Python's level 3 compares attribute values, not the text.
        let changed = PAGE.replace("Hi", "Goodbye");
        for level in 0..=3 {
            assert!(!differs(&changed, level), "level {level}");
        }
    }

    #[test]
    fn each_level_sees_what_it_compares_and_the_lower_ones_do_not() {
        let new_value = PAGE.replace("id=\"a\"", "id=\"b\"");
        let new_attribute = PAGE.replace("<p data-k", "<p title=\"t\" data-k");
        let new_tag = PAGE.replace("</p>", "</p><span></span>");

        assert!(!differs(&new_value, 1));
        assert!(!differs(&new_value, 2));
        assert!(differs(&new_value, 3));

        assert!(!differs(&new_attribute, 1));
        assert!(differs(&new_attribute, 2));
        assert!(differs(&new_attribute, 3));

        assert!(differs(&new_tag, 1));
        assert!(differs(&new_tag, 2));
        assert!(differs(&new_tag, 3));
    }

    #[test]
    fn the_message_names_the_level_and_the_first_element_that_differs() {
        let swapped = PAGE.replace("<p ", "<span ").replace("</p>", "</span>");

        let error = check(&baseline(), URL, &swapped, 1)
            .unwrap_err()
            .to_string();

        assert!(error.contains("<Level 1>"), "{error}");
        assert!(error.contains("element 2"), "{error}");
        assert!(error.contains("span"), "{error}");
    }

    #[test]
    fn level_zero_reports_every_level_and_the_dry_run_then_lets_it_pass() {
        let changed = PAGE.replace("</p>", "</p><span></span>");

        let error = check(&baseline(), URL, &changed, 0).unwrap_err();

        let text = error.to_string();
        for level in ["<Level 1>", "<Level 2>", "<Level 3>"] {
            assert!(text.contains(level), "{text}");
        }
        assert!(apply_check_level(Err(error), 0.0).is_ok());
    }

    #[test]
    fn the_domain_must_match_the_baseline() {
        let dir = baseline();

        check(&dir, "https://example.com/elsewhere?q=1", PAGE, 1).expect("same domain");
        for level in [0, 1, 3] {
            let error = check(&dir, "https://other.example/start", PAGE, level)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("Page Domain Mismatch"),
                "level {level}: {error}"
            );
        }
    }

    #[test]
    fn a_reset_or_a_missing_file_sets_the_baseline_again() {
        let changed = PAGE.replace("</p>", "</p><span></span>");
        let dir = baseline();
        assert!(check(&dir, URL, &changed, 1).is_err());

        check_page_structure(dir.path(), URL, &changed, b"new", true, 1).expect("reset");
        check(&dir, URL, &changed, 3).expect("the changed page is the baseline now");
        assert_eq!(
            std::fs::read(dir.path().join("baseline.png")).unwrap(),
            b"new"
        );

        std::fs::remove_file(dir.path().join("tags_level_2.txt")).unwrap();
        check(&dir, URL, PAGE, 3).expect("an incomplete baseline is set again");
        assert!(dir.path().join("tags_level_2.txt").is_file());
        check(&dir, URL, PAGE, 3).expect("and then it is compared");
    }

    #[test]
    fn an_unreadable_baseline_is_an_error_even_at_level_zero() {
        let dir = baseline();
        std::fs::write(dir.path().join("tags_level_1.txt"), "not json").unwrap();

        let error = check(&dir, URL, PAGE, 0).unwrap_err();

        assert!(
            !matches!(error, SeleniumBaseError::AssertionFailed(_)),
            "{error:?}"
        );
        assert!(apply_check_level(Err(error), 0.0).is_err());
    }

    #[test]
    fn the_level_is_0_1_2_or_3() {
        for (level, expected) in [(0.0, 0), (1.0, 1), (2.0, 2), (3.0, 3)] {
            assert_eq!(check_level(level).unwrap(), expected);
        }
        for level in [-1.0, 0.5, 4.0, f64::NAN, f64::INFINITY] {
            assert!(
                matches!(check_level(level), Err(SeleniumBaseError::InvalidConfig(_))),
                "{level}"
            );
        }
    }

    #[test]
    fn the_baseline_folder_is_per_test_and_per_name() {
        // The test harness names each test's thread after the test.
        let expected =
            Path::new("visual_baseline").join("the_baseline_folder_is_per_test_and_per_name");

        assert_eq!(baseline_dir("home").unwrap(), expected.join("home"));
        assert_eq!(baseline_dir("").unwrap(), expected.join("default"));
        for name in ["../home", "a/b", ".."] {
            assert!(baseline_dir(name).is_err(), "{name}");
        }
    }

    #[cfg(feature = "visual")]
    #[test]
    fn the_pixel_baseline_folder_names_the_test_by_its_whole_path() {
        // Two tests called `renders` in two modules, or in two test files,
        // must not compare against each other's screenshots.
        let expected = Path::new("visual_baseline").join(
            "seleniumbase_rs.api.run_settings.tests.\
             the_pixel_baseline_folder_names_the_test_by_its_whole_path",
        );

        assert_eq!(visual_match_folder(), expected);
    }

    #[cfg(feature = "visual")]
    #[test]
    fn a_test_id_is_one_plain_file_name() {
        let binary = Some("cart-0123456789abcdef");
        assert_eq!(
            test_id("checkout::renders", binary),
            "cart.checkout.renders"
        );
        assert_eq!(test_id("renders", binary), "cart.renders");
        assert_ne!(
            test_id("renders", Some("shop-0123456789abcdef")),
            test_id("renders", binary),
            "same-named tests of two test files"
        );
        // Not a test binary cargo built: no name from it.
        assert_eq!(test_id("a::renders", Some("mytool")), "a.renders");
        assert_eq!(test_id("main", Some("tool-v1")), "main");
        assert_eq!(test_id("", None), "default");
        assert_eq!(test_id("../../etc/passwd", None), "______etc_passwd");
        assert_eq!(test_id("a::::b::", None), "a.b");

        let long = format!("{}::renders", "m".repeat(200));
        let id = test_id(&long, binary);
        assert_eq!(id.len(), MAX_TEST_ID_LEN);
        assert!(id.ends_with(".renders"), "{id}");
        assert!(crate::artifacts::bare_file_name(&id).is_ok(), "{id}");
    }
}
