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

use std::path::Path;
use std::time::Duration;

use crate::error::SeleniumBaseError;

/// How long demo mode and slow mode pause after an action unless
/// [`RunSettings::with_demo_sleep`] says otherwise (Python's
/// `DEFAULT_DEMO_MODE_TIMEOUT`).
pub const DEFAULT_DEMO_PAUSE: Duration = Duration::from_millis(1200);

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
    fast_mode: bool,
    demo_sleep: Option<Duration>,
    message_duration: Option<Duration>,
    skip_js_waits: bool,
    record_sleep: bool,
    reset_visual_baseline: bool,
}

impl RunSettings {
    /// Highlight each element and pause after `open`, `click`, `type_text` and
    /// `update_text`, so a person can follow the run (`--demo`).
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

    /// Run at full speed: no demo or slow pauses, whatever else is set
    /// (`--fast`).
    #[must_use]
    pub fn with_fast_mode(mut self, on: bool) -> Self {
        self.fast_mode = on;
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
    /// Zero in fast mode or when neither demo nor slow mode is on. In demo mode
    /// a `tiny` action (typing) pauses for 1/3.4 of the full time, as in
    /// Python. Slow mode always pauses for the full time.
    #[must_use]
    pub fn pause(&self, tiny: bool) -> Duration {
        if self.fast_mode || !(self.demo_mode || self.slow_mode) {
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

/// Compares `current`, a screenshot, with the baseline stored at `path`, or
/// stores it when there is none or when `reset` is set.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::AssertionFailed`] if the screenshot differs
/// from the baseline, and [`SeleniumBaseError::InvalidConfig`] if the baseline
/// cannot be read or written.
pub(crate) fn compare_or_store_baseline(
    path: &Path,
    current: &[u8],
    reset: bool,
) -> Result<(), SeleniumBaseError> {
    if path.exists() && !reset {
        let baseline = std::fs::read(path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to read baseline: {e}"))
        })?;
        if baseline != current {
            let name = path
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Visual regression detected for '{name}'"
            )));
        }
        return Ok(());
    }
    std::fs::write(path, current)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("failed to write baseline: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn fast_mode_wins_over_demo_and_slow() {
        let settings = RunSettings::default()
            .with_demo_mode(true)
            .with_slow_mode(true)
            .with_fast_mode(true);
        assert_eq!(settings.pause(false), Duration::ZERO);
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

    #[test]
    fn a_baseline_is_stored_compared_and_replaced_on_request() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().join("home.png");

        compare_or_store_baseline(&path, b"one", false).expect("first run stores the baseline");
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
        compare_or_store_baseline(&path, b"one", false).expect("the same screenshot matches");

        let error = compare_or_store_baseline(&path, b"two", false).unwrap_err();
        assert!(error.to_string().contains("Visual regression"), "{error}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"one",
            "a mismatch keeps the baseline"
        );

        compare_or_store_baseline(&path, b"two", true).expect("a reset replaces the baseline");
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
    }
}
