// Recorder Mode on `BaseCase` (Python's `--rec`, `--recorder`, `--codegen`):
// the page script goes into every document, a watch loop keeps the recording
// of each tab, and `quit` writes what was recorded as a test under
// `recordings/`. Included in `base_case.rs` like the files beside it.

/// The page URL schemes Python refuses to record on.
const NOT_RECORDED_SCHEMES: [&str; 4] = ["data:", "about:", "chrome:", "edge:"];

/// What Python prints when Recorder Mode starts.
const RECORDER_ACTIVE: &str = "Recorder Mode ACTIVE. [ESC]: Pause. [~`]: Resume.";

/// The page's record of what a person did (see `RECORDER_JS`).
const RECORDED_ACTIONS_SCRIPT: &str = "return sessionStorage.getItem('recorded_actions');";

/// `"yes"` once the page script has run in this document. A tab that a page
/// opened with `window.open` starts with a copy of the opener's sessionStorage,
/// which says `"yes"` before the script has run in the new document.
const RECORDER_ACTIVATED_SCRIPT: &str = "return document.__sbRecorderRan === true ? sessionStorage.getItem('recorder_activated') : null;";

/// One breakpoint prompt at a time, however many tests run in parallel.
static RECORDER_BREAKPOINT_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Writes to the process's standard output without the test harness's
/// capture, so `RECORDING SAVED` reaches the person who ran the test.
fn say_on_stdout(text: &str) {
    use std::io::Write;
    let mut out = std::io::stdout();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

/// The local clock in milliseconds since the Unix epoch.
fn local_millis() -> i64 {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
}

/// Whether a failed command failed because a dialog is open, which Recorder
/// Mode leaves for the person to answer.
fn alert_is_open(error: &str) -> bool {
    error.to_ascii_lowercase().contains("unexpected alert open")
}

impl BaseCase {
    /// Whether Recorder Mode is on: asked for by the configuration
    /// ([`BrowserConfig::with_recorder_mode`], `--rec`), or switched on by
    /// [`activate_recorder`](Self::activate_recorder).
    #[must_use]
    pub fn recorder_mode(&self) -> bool {
        self.recording.on || self.config.recorder_mode()
    }

    /// Refuses Recorder Mode for a browser that is not Chromium-based, with
    /// Python's message, before a browser is started.
    pub(crate) fn check_recorder_browser(config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        if config.recorder_mode() && !config.browser.is_chromium_based() {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "Recorder Mode ONLY supports Chrome and Edge!\n(Your browser choice was: \"{}\")",
                config.browser
            )));
        }
        Ok(())
    }

    /// Starts Recorder Mode on a new case when the configuration asks for it:
    /// the page script is registered for every document of the first window,
    /// so the recording needs no extension and no DevTools mode.
    pub(crate) async fn start_recorder(&mut self) -> Result<(), SeleniumBaseError> {
        if self.config.recorder_mode() {
            self.begin_recording().await?;
        }
        Ok(())
    }

    /// Registers the page script in the window the session is on, notes when
    /// recording began and how far the browser's clock is from this one, and
    /// turns Recorder Mode on. Does nothing a second time.
    async fn begin_recording(&mut self) -> Result<(), SeleniumBaseError> {
        if self.recording.on {
            return Ok(());
        }
        self.register_recorder_script().await?;
        let local = local_millis();
        let browser = self
            .execute_script("return Date.now();")
            .await?
            .as_f64()
            // `as` saturates; a clock is nowhere near the limits.
            .map_or(local, |millis| millis as i64);
        self.recording.start_ms = browser;
        self.recording.clock_offset_ms = browser - local;
        self.recording.window_count = self
            .session
            .driver()
            .windows()
            .await
            .map_or(0, |handles| handles.len());
        self.recording.on = true;
        Ok(())
    }

    /// Has the window the session is on run the page script in each document
    /// it loads from now on (`Page.addScriptToEvaluateOnNewDocument`), once
    /// per window. It goes through the driver's DevTools passthrough, which
    /// works in every [`DriverMode`](crate::browser::config::DriverMode).
    async fn register_recorder_script(&mut self) -> Result<(), SeleniumBaseError> {
        let handle = self.session.driver().window().await?.to_string();
        if !self.recording.registered.insert(handle) {
            return Ok(());
        }
        self.session
            .devtools()?
            .execute_with_params(
                "Page.addScriptToEvaluateOnNewDocument",
                serde_json::json!({ "source": crate::api::recorder::recorder_injection() }),
            )
            .await?;
        Ok(())
    }

    /// Runs the page script in the document that is loaded now, unless it has
    /// run in this tab's origin already. Returns whether it ran.
    async fn run_recorder_script(&self) -> Result<bool, SeleniumBaseError> {
        let activated = self.execute_script(RECORDER_ACTIVATED_SCRIPT).await?;
        if activated.as_str() == Some("yes") {
            return Ok(false);
        }
        self.execute_script(crate::api::recorder::RECORDER_JS)
            .await?;
        Ok(true)
    }

    /// Python's `activate_recorder`: starts recording on the newest tab.
    ///
    /// A page whose address starts with `data:`, `about:`, `chrome:` or
    /// `edge:` is refused with Python's message (printed, not an error): open
    /// a real page first. Otherwise the page script is registered for the
    /// tab's next documents and run in this one unless it already runs, Recorder
    /// Mode is on, and `Recorder Mode ACTIVE. [ESC]: Pause. [~`]: Resume.` is
    /// printed and posted in the page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a browser that is not
    /// Chromium-based, and the driver's error when the tab cannot be reached.
    pub async fn activate_recorder(&mut self) -> Result<(), SeleniumBaseError> {
        if !self.config.browser.is_chromium_based() {
            return Err(SeleniumBaseError::InvalidConfig(
                "The SeleniumBase Recorder is for Chromium only!\n    (Supported browsers: Chrome and Edge)"
                    .to_owned(),
            ));
        }
        let url = self.get_current_url().await?;
        if NOT_RECORDED_SCHEMES
            .iter()
            .any(|scheme| url.starts_with(scheme))
        {
            say_on_stdout(
                "\nThe URL in Recorder-Mode cannot start with: \
                 \"data:\", \"about:\", \"chrome:\", or \"edge:\"!\n",
            );
            return Ok(());
        }
        self.switch_to_newest_window().await?;
        self.begin_recording().await?;
        self.register_recorder_script().await?;
        // Python suppresses a failure here: the page may be on its way elsewhere.
        let _ = self.run_recorder_script().await;
        say_on_stdout(&format!("\n{RECORDER_ACTIVE}\n"));
        let _ = self
            .post_message("Recorder Mode ACTIVE.<br>[ESC]: Pause. [~`]: Resume.")
            .await;
        Ok(())
    }

    /// The raw actions the page recorded in the tab the session is on:
    /// sessionStorage `recorded_actions`, with the switch back to the parent
    /// frame that a frame opened in a new tab needs. Only `http`, `https` and
    /// `file` pages are recorded; any other tab has none.
    async fn recorded_actions_on_active_tab(&self) -> Result<Vec<RawAction>, SeleniumBaseError> {
        let url = self.get_current_url().await?;
        if !crate::api::recorder::recordable(&url) {
            return Ok(Vec::new());
        }
        let stored = self.execute_script(RECORDED_ACTIONS_SCRIPT).await?;
        let Some(json) = stored.as_str().filter(|json| !json.is_empty()) else {
            return Ok(Vec::new());
        };
        let mut actions = RawAction::parse_list(json)?;
        crate::api::recorder::add_parent_frame_switches(&mut actions);
        Ok(actions)
    }

    /// Python's `save_recorded_actions()`: keeps what the page recorded in
    /// this tab, for a test that goes to another origin in the same tab (the
    /// page forgets its record then, because sessionStorage belongs to an
    /// origin). Most clicks that leave the origin open a new tab and need no
    /// call. Only `http`, `https` and `file` pages are kept.
    ///
    /// The old `save_recorded_actions(filename)` that wrote the call log to
    /// `latest_logs` is [`save_recording_json`](Self::save_recording_json).
    ///
    /// # Errors
    ///
    /// Returns the driver's error when the page cannot be read.
    pub async fn save_recorded_actions(&mut self) -> Result<(), SeleniumBaseError> {
        let url = self.get_current_url().await?;
        if !crate::api::recorder::recordable(&url) {
            return Ok(());
        }
        let actions = self.recorded_actions_on_active_tab().await?;
        self.recording.stash.extend(actions);
        self.recording
            .stash_origins
            .push(crate::api::recorder::origin_of(&url));
        Ok(())
    }

    /// Names the output of the recording: the file stem (`x` is written as
    /// `recordings/x_rec.rs`), the Gherkin feature and the scenario, which is
    /// also the name of the test function. The `sb` fixture calls it with the
    /// test's binary and function; without a call the names come from
    /// `SB_REC_FILE` and the name of the test's thread
    /// ([`RecordingTarget::from_environment`]).
    pub fn set_recording_target(&mut self, file_stem: &str, feature: &str, scenario: &str) {
        self.recording.target = Some(RecordingTarget::new(file_stem, feature, scenario));
    }

    /// [`set_recording_target`](Self::set_recording_target) with a whole
    /// [`RecordingTarget`], so the recording can go to another folder than
    /// `recordings`.
    pub fn set_recording_output(&mut self, target: RecordingTarget) {
        self.recording.target = Some(target);
    }

    /// One look at the browser while a person works in it. Returns `true`
    /// when no window is left.
    ///
    /// It follows a window that opened or closed (switching to the newest and
    /// starting the recorder there), starts the recorder in a tab that loaded
    /// a page after the last look, and keeps the tab's recorded actions so
    /// closing the browser does not lose them. A dialog that is open, or a
    /// page in the middle of a navigation, makes the look end early, and the
    /// next one tries again.
    async fn watch_tick(&mut self) -> bool {
        let handles = match self.session.driver().windows().await {
            Ok(handles) => handles,
            Err(error) => return !alert_is_open(&error.to_string()),
        };
        let Some(newest) = handles.last() else {
            return true;
        };
        if handles.len() != self.recording.window_count {
            self.recording.window_count = handles.len();
            let _ = self.session.driver().switch_to_window(newest.clone()).await;
        }
        let Ok(url) = self.get_current_url().await else {
            return false;
        };
        if crate::api::recorder::recordable(&url) {
            if !self.recording.on && self.begin_recording().await.is_err() {
                return false;
            }
            let _ = self.register_recorder_script().await;
            let _ = self.run_recorder_script().await;
            if let (Ok(handle), Ok(actions)) = (
                self.session.driver().window().await,
                self.recorded_actions_on_active_tab().await,
            ) {
                self.recording.cache.insert(handle.to_string(), actions);
            }
        }
        false
    }

    /// Watches the browser until the person presses SHIFT+ESC in it, every
    /// window is closed, or `limit` has passed (`mkrec --ee`). Recorder Mode
    /// is turned on first if it is not.
    ///
    /// # Errors
    ///
    /// Returns the error of [`activate_recorder`](Self::activate_recorder).
    pub async fn wait_for_recorder_end(
        &mut self,
        limit: Duration,
    ) -> Result<RecorderEnd, SeleniumBaseError> {
        if !self.recording.on {
            self.activate_recorder().await?;
        }
        let started = std::time::Instant::now();
        loop {
            if self.watch_tick().await {
                return Ok(RecorderEnd::NoWindowLeft);
            }
            // Armed again on every look: each new document loses the listener.
            let _ = self
                .execute_script(crate::api::recorder::SHIFT_ESC_JS)
                .await;
            let seen = self.execute_script("return document.sb_esc_end;").await;
            if seen.is_ok_and(|seen| seen.as_str() == Some("yes")) {
                return Ok(RecorderEnd::ShiftEsc);
            }
            if started.elapsed() >= limit {
                return Ok(RecorderEnd::TimeLimit);
            }
            tokio::time::sleep(crate::api::recorder::WATCH_TICK).await;
        }
    }

    /// Pauses the test so a person can do the manual part of a recording in
    /// the browser.
    ///
    /// If Recorder Mode is off it is started first, so a plain test can
    /// record from here. The prompt is written to standard error, which the
    /// test harness does not capture, so `--nocapture` is not needed, and
    /// answered on standard input: `c` (or `continue`) goes on and saves a
    /// screenshot to `latest_logs`, `q` (or `quit`) ends the test with
    /// [`SeleniumBaseError::Skipped`] (the recording is still written when
    /// the case quits), anything else asks again, and the end of the input
    /// (nextest, CI) goes on at once, so a breakpoint left in a test never
    /// hangs a run. Tests that run in parallel take turns at the prompt; run
    /// one at a time with `--test-threads 1` to see which is which.
    ///
    /// While it waits, every half second the browser is watched: a new tab
    /// gets the recorder, and what a tab recorded is kept. The time spent here
    /// does not count against the test's time limit.
    ///
    /// ```no_run
    /// # use seleniumbase_rs::{BaseCase, BrowserConfig};
    /// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut sb = BaseCase::new(BrowserConfig::default().with_recorder_mode(true)).await?;
    /// sb.open("https://example.com").await?;
    /// sb.recorder_breakpoint("Click around, then press c here").await?;
    /// sb.quit().await?; // writes recordings/..._rec.rs
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Skipped`] when the person quits, and the
    /// error of [`activate_recorder`](Self::activate_recorder).
    pub async fn recorder_breakpoint(&mut self, message: &str) -> Result<(), SeleniumBaseError> {
        self.recorder_breakpoint_with(
            message,
            &mut std::io::stderr(),
            crate::common::color::use_color(&std::io::stderr()),

            crate::common::debug_prompt::read_stdin_line,
        )
        .await
    }

    /// [`recorder_breakpoint`](Self::recorder_breakpoint) with the output and
    /// the input given, so a test can answer it.
    pub(crate) async fn recorder_breakpoint_with<R, F>(
        &mut self,
        message: &str,
        out: &mut dyn std::io::Write,
        color: bool,

        read_line: R,
    ) -> Result<(), SeleniumBaseError>
    where
        R: FnMut() -> F,
        F: std::future::Future<Output = std::io::Result<Option<String>>>,
    {
        if !self.recorder_mode() {
            self.activate_recorder().await?;
        }
        let _one_prompt = RECORDER_BREAKPOINT_GATE.lock().await;
        let started = std::time::Instant::now();
        // The hard time limit of the lifecycle does not run while a person works.
        #[cfg(feature = "testing")]
        let _clock_stopped = self.deadline.as_ref().map(|deadline| deadline.suspend());

        let exit = crate::api::recorder::run_prompt(
            message,
            out,
            read_line,
            async || self.watch_tick().await,
            crate::api::recorder::WATCH_TICK,
            color,

        )
        .await?;
        self.extend_time_limit(started.elapsed());
        match exit {
            crate::api::recorder::BreakpointExit::Quit => Err(SeleniumBaseError::Skipped(
                "recorder breakpoint: quit".to_owned(),
            )),
            crate::api::recorder::BreakpointExit::Continue => {
                if let Err(error) = self.save_screenshot_to_logs().await {
                    let _ = writeln!(out, "(could not save a screenshot: {error})");
                }
                Ok(())
            }
            crate::api::recorder::BreakpointExit::InputClosed
            | crate::api::recorder::BreakpointExit::WindowsGone => Ok(()),
        }
    }

    /// The raw actions of every tab: the open ones (the one the session is on
    /// first) read now, the tabs that closed since the last look from the
    /// record kept then, and the tabs saved by
    /// [`save_recorded_actions`](Self::save_recorded_actions). Also the
    /// origins of the tabs that were read.
    async fn harvest_tabs(&mut self) -> (Vec<RawAction>, Vec<String>) {
        let mut actions = Vec::new();
        let mut origins = Vec::new();
        let handles: Vec<String> = self
            .session
            .driver()
            .windows()
            .await
            .map(|handles| handles.iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        let current = self
            .session
            .driver()
            .window()
            .await
            .ok()
            .map(|handle| handle.to_string());
        let mut order: Vec<String> = current
            .iter()
            .filter(|handle| handles.contains(handle))
            .cloned()
            .collect();
        let others: Vec<String> = handles
            .iter()
            .filter(|handle| !order.contains(handle))
            .cloned()
            .collect();
        order.extend(others);
        for handle in &order {
            let live = match self.switch_to_window(handle).await {
                Ok(()) => match self.get_current_url().await {
                    Ok(url) if crate::api::recorder::recordable(&url) => {
                        origins.push(crate::api::recorder::origin_of(&url));
                        self.recorded_actions_on_active_tab().await.ok()
                    }
                    Ok(_) => Some(Vec::new()),
                    Err(_) => None,
                },
                Err(_) => None,
            };
            match live {
                Some(tab) => {
                    self.recording.cache.insert(handle.clone(), tab.clone());
                    actions.extend(tab);
                }
                // A dialog is open or the tab is gone: use what was kept.
                None => {
                    if let Some(kept) = self.recording.cache.get(handle) {
                        actions.extend(kept.iter().cloned());
                    }
                }
            }
        }
        for (handle, kept) in &self.recording.cache {
            if !order.contains(handle) {
                actions.extend(kept.iter().cloned());
            }
        }
        actions.extend(self.recording.stash.iter().cloned());
        origins.extend(self.recording.stash_origins.iter().cloned());
        (actions, origins)
    }

    /// Python's tear-down processing: reads the recording of every tab,
    /// merges the actions that the page cannot see (the entries of the call
    /// log in [`EXTRA_ACTIONS`](crate::api::recorder)), processes them as
    /// Python does, and writes the test (and, by the configuration, the
    /// Gherkin feature, a script or pure CDP code) under `recordings/`.
    ///
    /// It runs once; [`quit`](Self::quit) calls it first when Recorder Mode
    /// is on. Returns `None` when Recorder Mode is off, the recording was
    /// written already, or nothing was recorded.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] when a file cannot be written.
    pub async fn finish_recording(&mut self) -> Result<Option<SavedRecording>, SeleniumBaseError> {
        if !self.recording.on || self.recording.finished {
            return Ok(None);
        }
        self.recording.finished = true;
        let (mut raw, origins) = self.harvest_tabs().await;
        let fallback_origin = origins.first().cloned().unwrap_or_default();
        let extras = crate::api::recorder::extra_raw_actions(
            &self.recorded_actions()?,
            &raw,
            self.recording.clock_offset_ms,
            self.recording.start_ms,
            self.run_settings.record_sleep(),
            &fallback_origin,
        );
        raw.extend(extras);
        let uc = self.config.is_uc_enabled();
        let recorder = ActionRecorder::from_raw_actions(raw, self.recording.start_ms, &origins, uc);
        if recorder.actions.is_empty() {
            return Ok(None);
        }
        let target = self
            .recording
            .target
            .clone()
            .unwrap_or_else(RecordingTarget::from_environment);
        let format = if self.config.rec_sb_cdp() {
            RecordingFormat::Cdp
        } else if self.config.rec_sb_mgr() {
            RecordingFormat::Script
        } else {
            RecordingFormat::Test
        };
        let options = RecordingOptions {
                    format,
                    behave: self.config.rec_behave(),
                    print: self.config.rec_print(),
                    uc,
                    // The banner goes to standard output, which may be a terminal.
                    color: crate::common::color::use_color(&std::io::stdout()),
                };
        let saved = crate::api::recorder::write_recording(
            &recorder.actions,
            &target,
            &options,
            &mut std::io::stdout(),
        )?;
        Ok(Some(saved))
    }

    /// What [`quit`](Self::quit) does about the recording: writes it, and
    /// prints a failure as Python does without failing the quit.
    async fn finish_recording_at_quit(&mut self) {
        if let Err(error) = self.finish_recording().await {
            say_on_stdout(&format!(
                "\n (Recorder) Code-generation exception:\n{error}\n"
            ));
        }
    }
}

#[cfg(test)]
mod tests_recorder_mode {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    /// One browser tab of the mock: its handle, address and recorded actions.
    struct Tab {
        handle: &'static str,
        url: &'static str,
        stored: String,
    }

    fn tab(handle: &'static str, url: &'static str, stored: Value) -> Tab {
        Tab {
            handle,
            url,
            stored: if stored.is_null() {
                String::new()
            } else {
                stored.to_string()
            },
        }
    }

    /// A Chrome with `tabs` whose clock reads `now`; the script that records
    /// is "already running" in each tab and SHIFT+ESC was pressed.
    fn browser(tabs: Vec<Tab>, now: i64) -> MockWebDriver {
        let current = Arc::new(Mutex::new(tabs[0].handle.to_owned()));
        MockWebDriver::start(json!({}), move |request| {
            let here = current.lock().unwrap().clone();
            let open = tabs.iter().find(|tab| tab.handle == here);
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/window/handles") => {
                    Reply::Value(json!(tabs.iter().map(|tab| tab.handle).collect::<Vec<_>>()))
                }
                ("GET", "/window") => Reply::Value(json!(here)),
                ("POST", "/window") => {
                    let to = request.body["handle"].as_str().unwrap_or_default();
                    *current.lock().unwrap() = to.to_owned();
                    Reply::Value(Value::Null)
                }
                ("GET", "/url") => Reply::Value(json!(open.map_or("about:blank", |tab| tab.url))),
                ("POST", "/goog/cdp/execute") => Reply::Value(json!({})),
                ("POST", "/execute/sync") => {
                    let script = request.body["script"].as_str().unwrap_or_default();
                    if script.contains("return Date.now()") {
                        Reply::Value(json!(now))
                    } else if script.contains("getItem('recorded_actions')") {
                        Reply::Value(json!(open.map_or("", |tab| tab.stored.as_str())))
                    } else if script.contains("getItem('recorder_activated')")
                        || script.contains("return document.sb_esc_end")
                    {
                        Reply::Value(json!("yes"))
                    } else {
                        Reply::Value(Value::Null)
                    }
                }
                _ => Reply::Value(Value::Null),
            }
        })
    }

    fn config(driver: &MockWebDriver) -> BrowserConfig {
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url().to_owned())
            .with_recorder_mode(true);
        config.auto_start_driver = false;
        config
    }

    fn temp(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("sbrs-rec-mode-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    fn now() -> i64 {
        local_millis()
    }

    #[tokio::test]
    async fn recorder_mode_registers_the_page_script_in_plain_webdriver_mode() {
        let driver = browser(vec![tab("w1", "about:blank", Value::Null)], now());
        let sb = BaseCase::new(config(&driver)).await.unwrap();
        assert!(sb.recorder_mode());

        let commands = driver.requests("/goog/cdp/execute");
        assert_eq!(commands.len(), 1, "{commands:?}");
        assert_eq!(
            commands[0].body["cmd"],
            "Page.addScriptToEvaluateOnNewDocument"
        );
        let source = commands[0].body["params"]["source"].as_str().unwrap();
        assert!(source.contains("__sbRecorderHooked"), "{source}");
        assert!(
            source.contains("sessionStorage"),
            "the page script is in it"
        );
        let session = driver.requests("/session")[0].body.to_string();
        assert!(
            session.contains("\"unhandledPromptBehavior\":\"ignore\""),
            "a dialog must stay open: {session}"
        );
    }

    #[tokio::test]
    async fn another_browser_is_refused_before_it_starts() {
        let driver = browser(vec![tab("w1", "about:blank", Value::Null)], now());
        let mut config = config(&driver);
        config.browser = crate::browser::config::Browser::Firefox;
        let error = BaseCase::new(config)
            .await
            .err()
            .expect("Firefox cannot record");
        let text = error.to_string();
        assert!(
            text.contains("Recorder Mode ONLY supports Chrome and Edge!"),
            "{text}"
        );
        assert!(
            text.contains("(Your browser choice was: \"firefox\")"),
            "{text}"
        );
        assert!(
            driver.requests("/session").is_empty(),
            "no browser was asked for"
        );
    }

    #[tokio::test]
    async fn a_blank_page_is_refused_and_nothing_is_sent() {
        let driver = browser(vec![tab("w1", "about:blank", Value::Null)], now());
        let mut sb = BaseCase::new(config(&driver)).await.unwrap();
        let before = driver.all_requests().len();
        sb.activate_recorder().await.unwrap();
        let after: Vec<_> = driver.all_requests().into_iter().skip(before).collect();
        assert!(
            after
                .iter()
                .all(|request| request.path != "/goog/cdp/execute"
                    && request.path != "/execute/sync"),
            "{after:?}"
        );
    }

    #[tokio::test]
    async fn finishing_visits_every_tab_and_merges_the_log_in_time_order() {
        let base = now();
        let first = json!([
            ["begin", "https://a.test", "https://a.test/", base + 100_000],
            ["click", "#go", "", base + 100_500]
        ]);
        let second = json!([
            ["begin", "https://b.test", "https://b.test/", base + 101_000],
            ["click", "#x", "", base + 101_500]
        ]);
        let driver = browser(
            vec![
                tab("w1", "https://a.test/", first),
                tab("w2", "https://b.test/", second),
            ],
            base,
        );
        let mut sb = BaseCase::new(config(&driver)).await.unwrap();
        // A call the page cannot see, between the two tabs' actions.
        sb.recorder.lock().unwrap().actions.push(RecordedAction {
            name: "hover".to_owned(),
            target: Some("#h".to_owned()),
            value: None,
            timestamp_ms: u128::try_from(base + 100_700).unwrap(),
        });
        let folder = temp("harvest");
        sb.set_recording_output(
            RecordingTarget::new("shop", "Shop", "test_checkout").in_folder(&folder),
        );

        let saved = sb.finish_recording().await.unwrap().expect("a recording");

        let visited: Vec<_> = driver
            .requests("/window")
            .iter()
            .filter(|request| request.method == "POST")
            .map(|request| request.body["handle"].as_str().unwrap().to_owned())
            .collect();
        assert!(
            visited.contains(&"w1".to_owned()) && visited.contains(&"w2".to_owned()),
            "{visited:?}"
        );
        let code = std::fs::read_to_string(&saved.code).unwrap();
        let at = |needle: &str| {
            code.find(needle)
                .unwrap_or_else(|| panic!("{needle} in {code}"))
        };
        assert!(at("sb.click(\"#go\")") < at("sb.hover(\"#h\")"), "{code}");
        assert!(at("sb.hover(\"#h\")") < at("sb.click(\"#x\")"), "{code}");
        assert!(saved.json.exists());
        // It runs once.
        assert!(sb.finish_recording().await.unwrap().is_none());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[tokio::test]
    async fn without_recorder_mode_quit_writes_nothing() {
        let driver = browser(vec![tab("w1", "https://a.test/", Value::Null)], now());
        let mut config = config(&driver);
        config = config.with_recorder_mode(false);
        let mut sb = BaseCase::new(config).await.unwrap();
        let folder = temp("off");
        sb.set_recording_output(RecordingTarget::new("off", "Off", "t").in_folder(&folder));
        assert!(!sb.recorder_mode());
        assert!(sb.finish_recording().await.unwrap().is_none());
        sb.quit().await.unwrap();
        assert!(!folder.exists());
    }

    #[tokio::test]
    async fn quit_writes_the_recording_and_a_second_test_of_the_file_is_added() {
        let base = now();
        let stored = json!([["begin", "https://a.test", "https://a.test/", base + 100_000]]);
        let driver = browser(vec![tab("w1", "https://a.test/", stored)], base);
        let folder = temp("quit");
        let mut one = BaseCase::new(config(&driver)).await.unwrap();
        one.set_recording_output(
            RecordingTarget::new("pair", "Pair", "test_one").in_folder(&folder),
        );
        one.quit().await.unwrap();
        let mut two = BaseCase::new(config(&driver)).await.unwrap();
        two.set_recording_output(
            RecordingTarget::new("pair", "Pair", "test_two").in_folder(&folder),
        );
        two.quit().await.unwrap();

        let code = std::fs::read_to_string(folder.join("pair_rec.rs")).unwrap();
        assert!(
            code.contains("async fn test_one(") && code.contains("async fn test_two("),
            "{code}"
        );
        let prelude = "use seleniumbase_rs::testing::{rstest, sb, Sb};";
        assert_eq!(code.matches(prelude).count(), 1, "{code}");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[tokio::test]
    async fn a_dialog_that_is_open_does_not_end_the_watch() {
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/window/handles") => Reply::Error(500, "unexpected alert open"),
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = BaseCase::with_session(
            config(&driver),
            BrowserSession::connect(config(&driver)).await.unwrap(),
        );
        assert!(
            !sb.watch_tick().await,
            "the tick is skipped and the next one tries again"
        );
    }

    #[tokio::test]
    async fn no_window_left_ends_the_watch() {
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/window/handles") => Reply::Value(json!([])),
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = BaseCase::with_session(
            config(&driver),
            BrowserSession::connect(config(&driver)).await.unwrap(),
        );
        assert!(sb.watch_tick().await);
    }

    #[tokio::test]
    async fn shift_esc_and_the_time_limit_end_wait_for_recorder_end() {
        let driver = browser(vec![tab("w1", "https://a.test/", Value::Null)], now());
        let mut sb = BaseCase::new(config(&driver)).await.unwrap();
        let end = sb
            .wait_for_recorder_end(Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(end, RecorderEnd::ShiftEsc);
        assert!(
            driver
                .requests("/execute/sync")
                .iter()
                .any(|request| request.body["script"]
                    .as_str()
                    .is_some_and(|s| s.contains("__sbEscEndHooked"))),
            "the listener was armed"
        );
    }

    /// A reader that answers with `lines`, then ends.
    fn typed(
        lines: &[&str],
    ) -> impl FnMut() -> std::future::Ready<std::io::Result<Option<String>>> {
        let mut lines: std::collections::VecDeque<String> =
            lines.iter().map(|line| (*line).to_owned()).collect();
        move || std::future::ready(Ok(lines.pop_front()))
    }

    #[tokio::test]
    async fn a_breakpoint_goes_on_with_c_and_asks_again_for_other_text() {
        let driver = browser(vec![tab("w1", "https://a.test/", Value::Null)], now());
        let mut sb = BaseCase::new(config(&driver)).await.unwrap();
        let mut out = Vec::new();
        sb.recorder_breakpoint_with("now", &mut out, false, typed(&["x", "c"]))
            .await
            .unwrap();
        let said = String::from_utf8(out).unwrap();
        assert_eq!(
            said.matches("[RECORDER BREAKPOINT]: now").count(),
            2,
            "{said}"
        );
    }

    #[tokio::test]
    async fn a_breakpoint_quits_with_q_and_goes_on_at_the_end_of_the_input() {
        let driver = browser(vec![tab("w1", "https://a.test/", Value::Null)], now());
        let mut sb = BaseCase::new(config(&driver)).await.unwrap();
        let mut out = Vec::new();
        let quit = sb.recorder_breakpoint_with("now", &mut out, false, typed(&["q"]))
            .await;
        assert!(
            matches!(quit, Err(SeleniumBaseError::Skipped(_))),
            "{quit:?}"
        );
        let mut out = Vec::new();
        sb.recorder_breakpoint_with("again", &mut out, false, typed(&[]))
            .await
            .unwrap();
        assert!(String::from_utf8(out)
            .unwrap()
            .contains("input closed: continuing"));
    }
}
