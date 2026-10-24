// MasterQA and test-case management wrappers for BaseCase.

/// Title of the session `manual_verify` starts when none is active.
const MANUAL_VERIFY_TITLE: &str = "Manual verification";

/// The session a manual check is recorded in: the active one, or a new one
/// that then becomes the active session.
fn session_for_manual_check(slot: &mut Option<MasterQaSession>) -> &mut MasterQaSession {
    slot.get_or_insert_with(|| MasterQaSession::new(MANUAL_VERIFY_TITLE))
}

impl BaseCase {
    /// Returns a reference to the current QA session if one exists.
    pub fn qa_session(&self) -> Option<&MasterQaSession> {
        self.qa_session.as_ref()
    }

    /// Starts a new MasterQA verification session.
    pub fn start_qa_session(&mut self, title: &str) -> &mut MasterQaSession {
        self.qa_session = Some(MasterQaSession::new(title));
        self.qa_session.as_mut().unwrap()
    }

    /// Prompts the user for a manual verification step and records it in the
    /// active QA session.
    ///
    /// If no session is active, one titled "Manual verification" is started
    /// and stays active, so the step appears in [`BaseCase::qa_session`] and in
    /// reports built from it instead of being lost.
    pub fn manual_verify(&mut self, question: &str) -> bool {
        session_for_manual_check(&mut self.qa_session).verify(question)
    }

    /// Saves the active QA session ([`start_qa_session`](Self::start_qa_session),
    /// or the one [`manual_verify`](Self::manual_verify) starts) as a Markdown
    /// test-case report in the logs folder, and returns the path of the file.
    /// To choose the file, see [`save_qa_report_to`](Self::save_qa_report_to).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if there is no QA session,
    /// and an I/O error if the file cannot be written.
    pub fn save_qa_report(&self) -> Result<std::path::PathBuf, SeleniumBaseError> {
        let session = self.active_qa_session()?;
        let logs = ensure_latest_logs_dir()?;
        session.save_markdown(artifact_path(&logs, "qa_report", "md"))
    }

    /// Saves the active QA session as a Markdown report in the file `path`,
    /// and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if there is no QA session,
    /// and an I/O error if the file cannot be written.
    pub fn save_qa_report_to<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<std::path::PathBuf, SeleniumBaseError> {
        self.active_qa_session()?.save_markdown(path)
    }

    /// The active QA session, or the error that says there is none.
    fn active_qa_session(&self) -> Result<&MasterQaSession, SeleniumBaseError> {
        self.qa_session.as_ref().ok_or_else(|| {
            SeleniumBaseError::InvalidConfig(
                "there is no QA session to report: call start_qa_session or manual_verify first"
                    .to_owned(),
            )
        })
    }
}

/// How often MasterQA asks whether the person has answered the `confirm` box
/// it falls back to (Python: 0.05 s).
const MASTERQA_ALERT_POLL: Duration = Duration::from_millis(50);

/// How long the `confirm` box is given to open before it is looked for
/// (Python: 0.05 s).
const MASTERQA_ALERT_SETTLE: Duration = Duration::from_millis(50);

// What the checks of a MasterQA run need from the case: the dialog, its
// fallback and the facts a result row records. The wrapper that keeps count
// of the checks is [`MasterQA`](crate::api::master_qa::MasterQA).
impl BaseCase {
    /// Shows MasterQA's check number `count`, asking `question`, and waits
    /// for the person's verdict: `true` for YES / PASS, `false` for NO / FAIL.
    ///
    /// The page is waited for first. The box is a jquery-confirm dialog
    /// ([`jqc::masterqa_dialog_script`]); when the page cannot load
    /// jquery-confirm, the question is asked with the browser's own
    /// `confirm` box instead, as Python does.
    pub(crate) async fn masterqa_ask(
        &mut self,
        count: usize,
        question: &str,
    ) -> Result<bool, SeleniumBaseError> {
        self.wait_for_ready_state_complete().await?;
        let script = jqc::masterqa_dialog_script(count, question);
        match self.run_jqc_dialog(&script, jqc::FOCUS_BOX).await {
            Ok(answer) => Ok(answer.button == jqc::MASTERQA_PASS),
            Err(SeleniumBaseError::Download { .. }) => {
                self.masterqa_confirm(question, crate::api::master_qa::MAX_IDLE_TIME_BEFORE_QUIT)
                    .await
            }
            Err(error) => Err(error),
        }
    }

    /// Asks `question` with the browser's `confirm` box and waits for the
    /// person to answer it. If nobody answers for `max_idle`, the browser is
    /// quit and the wait fails, as Python's does.
    ///
    /// The box is opened from a timer, so that the script call returns before
    /// it opens: a WebDriver server answers a script that opens a box with
    /// "unexpected alert open". While the box is open only the alert commands
    /// are used on the page.
    pub(crate) async fn masterqa_confirm(
        &mut self,
        question: &str,
        max_idle: Duration,
    ) -> Result<bool, SeleniumBaseError> {
        let script = format!(
            "window.master_qa_result = null; \
             setTimeout(function () {{ \
             window.master_qa_result = confirm({}) ? {} : {}; }}, 0); return true;",
            quote(question),
            quote(jqc::MASTERQA_PASS),
            quote(jqc::MASTERQA_FAIL),
        );
        self.execute_script(&script).await?;
        tokio::time::sleep(MASTERQA_ALERT_SETTLE).await;

        let deadline = std::time::Instant::now() + max_idle;
        loop {
            match self.get_alert_text().await {
                // Python lets any alert that is not the question pass.
                Ok(text) if !text.contains('?') => break,
                Ok(_) => {}
                Err(_) => break,
            }
            if std::time::Instant::now() >= deadline {
                let _ = self.quit().await;
                let seconds = if max_idle.subsec_nanos() == 0 {
                    max_idle.as_secs().to_string()
                } else {
                    max_idle.as_secs_f64().to_string()
                };
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "{seconds} seconds passed without human action! Stopping..."
                )));
            }
            tokio::time::sleep(MASTERQA_ALERT_POLL).await;
        }
        let result = self.execute_script("return window.master_qa_result").await?;
        Ok(result.as_str() == Some(jqc::MASTERQA_PASS))
    }

    /// The address of the page, as a result row records it: the current URL,
    /// or the page's own idea of it when the driver cannot say.
    pub(crate) async fn masterqa_page_url(&self) -> String {
        match self.get_current_url().await {
            Ok(url) => url,
            Err(_) => self
                .execute_script("return document.URL")
                .await
                .ok()
                .and_then(|url| url.as_str().map(str::to_owned))
                .unwrap_or_default(),
        }
    }

    /// The name of the browser as a result row records it (`chrome`).
    pub(crate) fn masterqa_browser(&self) -> &'static str {
        self.config.browser.name()
    }

    /// Closes the second window, if another one was opened, and goes back to
    /// the first, as Python's MasterQA does after an error. It does what it
    /// can and ignores a failure.
    pub(crate) async fn masterqa_close_extra_window(&mut self) {
        let Ok(handles) = self.get_all_window_handles().await else {
            return;
        };
        if handles.len() < 2 {
            return;
        }
        let _ = self.switch_to_window(&handles[1]).await;
        let _ = self.close_window().await;
        let _ = self.switch_to_window(&handles[0]).await;
    }
}


#[cfg(test)]
mod tests_masterqa {
    use super::*;
    use crate::api::master_qa::QaStep;

    #[test]
    fn markdown_report_format() {
        let steps = vec![QaStep {
            id: 1,
            question: "Page loads".to_owned(),
            passed: true,
            notes: None,
        }];
        let mut session = MasterQaSession::new("Login Test");
        session.steps.extend(steps);
        let md = session.to_markdown();
        assert!(md.contains("# Login Test"));
        assert!(md.contains("| 1 | Page loads | PASS |"));
    }

    #[test]
    fn a_manual_check_without_a_session_starts_one_and_keeps_it() {
        let mut slot = None;
        session_for_manual_check(&mut slot).steps.push(QaStep {
            id: 1,
            question: "first".to_owned(),
            passed: true,
            notes: None,
        });
        let session = slot.as_ref().expect("the session stays active");
        assert_eq!(session.title, MANUAL_VERIFY_TITLE);

        // The next check joins it rather than starting another.
        assert_eq!(session_for_manual_check(&mut slot).steps.len(), 1);
    }

    #[test]
    fn a_manual_check_joins_the_session_the_test_started() {
        let mut slot = Some(MasterQaSession::new("Checkout"));
        assert_eq!(session_for_manual_check(&mut slot).title, "Checkout");
    }

    #[test]
    fn the_report_escapes_what_would_break_a_table_cell() {
        let steps = vec![QaStep {
            id: 1,
            question: "a | b\nc".to_owned(),
            passed: false,
            notes: Some("x|y".to_owned()),
        }];
        let mut session = MasterQaSession::new("T");
        session.steps.extend(steps);
        let md = session.to_markdown();
        assert!(md.contains("| 1 | a \\| b c | FAIL | x\\|y |"), "{md}");
    }

    fn answered(question: &str, passed: bool) -> QaStep {
        QaStep {
            id: 1,
            question: question.to_owned(),
            passed,
            notes: None,
        }
    }

    #[test]
    fn the_report_is_the_active_session_and_goes_to_the_path_it_is_given() {
        let mut sb =
            BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        sb.start_qa_session("Checkout")
            .steps
            .push(answered("The total is right", true));
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("qa.md");

        let saved = sb.save_qa_report_to(&target).unwrap();

        assert_eq!(saved, target);
        let report = std::fs::read_to_string(&target).unwrap();
        assert!(report.starts_with("# Checkout"), "{report}");
        assert!(report.contains("| 1 | The total is right | PASS |"), "{report}");
    }

    #[test]
    fn a_report_without_a_session_is_an_error_not_an_empty_file() {
        let sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("qa.md");

        assert!(matches!(
            sb.save_qa_report_to(&target),
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
        assert!(matches!(
            sb.save_qa_report(),
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
        assert!(!target.exists());
    }

    #[test]
    fn a_session_the_test_never_started_is_not_made_by_reporting() {
        let sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        let _ = sb.save_qa_report();
        assert!(sb.qa_session().is_none());
    }
}
