// MasterQA and test-case management wrappers for BaseCase.

use crate::api::master_qa::QaStep;

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

    /// Generates a Markdown test-case management report from QA steps.
    #[deprecated(
        since = "0.5.0",
        note = "use MasterQaSession::to_markdown on the session; save_qa_report writes the active one"
    )]
    pub fn qa_report_markdown(steps: &[QaStep], title: &str) -> String {
        let mut session = MasterQaSession::new(title);
        for step in steps {
            session.steps.push(step.clone());
        }
        session.to_markdown()
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

#[cfg(test)]
mod tests_masterqa {
    use super::*;

    #[test]
    fn markdown_report_format() {
        let steps = vec![QaStep {
            id: 1,
            question: "Page loads".to_owned(),
            passed: true,
            notes: None,
        }];
        let md = BaseCase::qa_report_markdown(&steps, "Login Test");
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
        let md = BaseCase::qa_report_markdown(&steps, "T");
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
