// Debug prompt: `breakpoint()` and the page the prompt looks at.

#[async_trait::async_trait]
impl crate::common::debug_prompt::PromptPage for BaseCase {
    async fn url(&self) -> Result<String, SeleniumBaseError> {
        self.get_current_url().await
    }

    async fn title(&self) -> Result<String, SeleniumBaseError> {
        self.get_title().await
    }

    async fn screenshot_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        self.screenshot_as_png().await
    }

    async fn page_source(&self) -> Result<String, SeleniumBaseError> {
        self.get_page_source().await
    }

    async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        self.execute_script(&format!("return ({expression});"))
            .await
    }
}

impl BaseCase {
    /// Pauses the test with the browser open, as Python's `breakpoint()` does
    /// in a SeleniumBase test.
    ///
    /// A prompt is written to standard error and the test waits for a command:
    /// `c` goes on, `s` saves a screenshot and `src` the page source to
    /// `latest_logs/<test id>/`, `u` shows the address and title, `js <expr>`
    /// evaluates an expression, `q` stops the test and `h` lists these. Work in
    /// the browser meanwhile. The time spent at the prompt is added to the
    /// [time limit](Self::set_time_limit).
    ///
    /// The pause is skipped, with a warning in the log, when standard input
    /// is not a terminal (CI, a pipe, `cargo nextest`), and silently when the
    /// environment variable `SB_BREAKPOINTS` is `0`, so a breakpoint left in a
    /// test never hangs a run. In a terminal, run `cargo test -- --test-threads 1
    /// --nocapture` so the answers reach this test.
    ///
    /// ```no_run
    /// # use seleniumbase_rs::{BaseCase, BrowserConfig};
    /// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    /// sb.open("https://example.com").await?;
    /// sb.breakpoint("Paused at the home page.").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::TestLifecycle`] when the person quits the
    /// prompt (`q`), and [`SeleniumBaseError::Io`] when the terminal cannot be
    /// read or written.
    pub async fn breakpoint(&mut self, message: &str) -> Result<(), SeleniumBaseError> {
        let Some(mut prompt) = crate::common::debug_prompt::breakpoint_prompt() else {
            return Ok(());
        };
        self.pause_at(
            &mut prompt,
            crate::common::debug_prompt::PromptKind::Breakpoint,
            message,
        )
        .await
    }

    /// Whether a recording window is open ([`start_recording`](Self::start_recording)),
    /// which generated recorder tests check before they call
    /// [`breakpoint`](Self::breakpoint) so a person can record by hand.
    #[must_use]
    pub fn recorder_mode(&self) -> bool {
        self.is_recording()
    }

    /// Pauses at `prompt` as `kind`, adds the time spent there to the time
    /// limit, and turns a quit into an error.
    pub(crate) async fn pause_at<I, W>(
        &mut self,
        prompt: &mut crate::common::debug_prompt::DebugPrompt<I, W>,
        kind: crate::common::debug_prompt::PromptKind,
        message: &str,
    ) -> Result<(), SeleniumBaseError>
    where
        I: crate::common::debug_prompt::LineInput,
        W: std::io::Write + Send,
    {
        let test_id = self
            .test_name
            .clone()
            .unwrap_or_else(crate::common::debug_prompt::thread_test_id);
        let started = std::time::Instant::now();
        let outcome = prompt.pause(kind, &test_id, message, &*self).await;
        self.extend_time_limit(started.elapsed());
        match outcome? {
            crate::common::debug_prompt::PromptOutcome::Continue => Ok(()),
            crate::common::debug_prompt::PromptOutcome::Quit => Err(
                SeleniumBaseError::test_lifecycle("stopped at the debug prompt"),
            ),
        }
    }

    /// Makes the time limit longer by `paused`, so the time a person spent at
    /// a prompt does not count against the test. Does nothing without a limit.
    fn extend_time_limit(&mut self, paused: Duration) {
        if let Some(limit) = self.time_limit.as_mut() {
            *limit += paused;
        }
    }
}

#[cfg(test)]
mod debug_prompt_tests {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use crate::common::debug_prompt::{DebugPrompt, LineInput, PromptKind};
    use serde_json::json;

    /// Answers one scripted line at a time, each after `wait`.
    struct SlowInput {
        lines: Vec<&'static str>,
        wait: Duration,
    }

    #[async_trait::async_trait]
    impl LineInput for SlowInput {
        async fn read_line(&mut self) -> std::io::Result<Option<String>> {
            tokio::time::sleep(self.wait).await;
            Ok((!self.lines.is_empty()).then(|| self.lines.remove(0).to_owned()))
        }
    }

    fn prompt(
        lines: Vec<&'static str>,
        wait: Duration,
        logs: &std::path::Path,
    ) -> DebugPrompt<SlowInput, Vec<u8>> {
        DebugPrompt::new(SlowInput { lines, wait }, Vec::new()).with_logs_dir(logs)
    }

    /// A WebDriver that answers every command with `null`.
    fn quiet_driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |_| Reply::Value(Value::Null))
    }

    #[tokio::test]
    async fn the_time_at_a_breakpoint_does_not_count_against_the_time_limit() {
        let logs = tempfile::tempdir().unwrap();
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        sb.set_time_limit(1).unwrap();
        let mut at_prompt = prompt(vec!["c"], Duration::from_millis(1200), logs.path());

        sb.pause_at(&mut at_prompt, PromptKind::Breakpoint, "wait for a person")
            .await
            .unwrap();

        // The test has now run for 1.2 s, past its 1 s limit, but the pause is
        // not part of that.
        assert!(sb.check_time_limit().is_ok());
    }

    #[tokio::test]
    async fn a_pause_without_a_time_limit_leaves_it_off() {
        let logs = tempfile::tempdir().unwrap();
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        let mut at_prompt = prompt(vec!["c"], Duration::ZERO, logs.path());

        sb.pause_at(&mut at_prompt, PromptKind::Breakpoint, "m")
            .await
            .unwrap();

        assert_eq!(sb.time_limit, None);
    }

    #[tokio::test]
    async fn quitting_the_prompt_fails_the_test_and_still_extends_the_limit() {
        let logs = tempfile::tempdir().unwrap();
        let driver = quiet_driver();
        let mut sb = driver.base_case().await;
        sb.set_time_limit(60).unwrap();
        let mut at_prompt = prompt(vec!["q"], Duration::from_millis(50), logs.path());

        let error = sb
            .pause_at(&mut at_prompt, PromptKind::Breakpoint, "m")
            .await
            .unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::TestLifecycle(text)
                if text == "stopped at the debug prompt"),
            "unexpected error: {error}"
        );
        assert!(sb.time_limit.unwrap() >= Duration::from_secs(60) + Duration::from_millis(50));
    }

    #[tokio::test]
    async fn the_prompt_reads_and_saves_what_the_webdriver_page_shows() {
        const PNG: &[u8] = b"\x89PNG-from-the-mock";
        let logs = tempfile::tempdir().unwrap();
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/url") => Reply::Value(json!("https://example.com/login")),
                ("GET", "/title") => Reply::Value(json!("Login")),
                ("GET", "/source") => Reply::Value(json!("<html>login</html>")),
                ("GET", "/screenshot") => Reply::Value(json!(BASE64_STANDARD.encode(PNG))),
                ("POST", "/execute/sync") => Reply::Value(json!(2)),
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = driver.base_case().await.with_test_name("login.valid_login");
        let mut at_prompt = prompt(
            vec!["u", "s", "src", "js 1 + 1", "c"],
            Duration::ZERO,
            logs.path(),
        );

        sb.pause_at(&mut at_prompt, PromptKind::Breakpoint, "look")
            .await
            .unwrap();

        let text = String::from_utf8_lossy(at_prompt.output()).into_owned();
        assert!(text.contains("test:  login.valid_login"), "{text}");
        assert!(
            text.contains("page: https://example.com/login  \"Login\""),
            "{text}"
        );
        assert!(text.contains("> 2\n"), "the js value: {text}");
        let folder = logs.path().join("login.valid_login");
        assert_eq!(std::fs::read(folder.join("breakpoint_1.png")).unwrap(), PNG);
        assert_eq!(
            std::fs::read_to_string(folder.join("page_source_1.html")).unwrap(),
            "<html>login</html>"
        );
        let script = driver.requests("/execute/sync").pop().unwrap().body["script"].clone();
        assert_eq!(
            script,
            json!("return (1 + 1);"),
            "the expression is wrapped"
        );
    }

    #[tokio::test]
    async fn a_browser_that_is_gone_does_not_stop_the_pause() {
        let logs = tempfile::tempdir().unwrap();
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/url" | "/title" | "/source" | "/screenshot") => {
                    Reply::Error(404, "no such window")
                }
                _ => Reply::Value(Value::Null),
            }
        });
        let mut sb = driver.base_case().await;
        let mut at_prompt = prompt(vec!["u", "s", "src", "c"], Duration::ZERO, logs.path());

        sb.pause_at(&mut at_prompt, PromptKind::Breakpoint, "look")
            .await
            .unwrap();

        let text = String::from_utf8_lossy(at_prompt.output()).into_owned();
        assert!(text.contains("page: (unavailable: "), "{text}");
        assert!(text.contains("could not take a screenshot: "), "{text}");
        assert!(text.contains("could not read the page source: "), "{text}");
    }

    #[tokio::test]
    async fn without_a_terminal_a_breakpoint_goes_on_at_once() {
        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() {
            return;
        }
        let mut sb = BaseCase::without_session(BrowserConfig::default());

        let started = std::time::Instant::now();
        sb.breakpoint("nobody is here").await.unwrap();

        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn recorder_mode_follows_the_recording_window() {
        let mut sb = BaseCase::without_session(BrowserConfig::default());
        assert!(!sb.recorder_mode());

        sb.start_recording();

        assert!(sb.recorder_mode());
    }
}
