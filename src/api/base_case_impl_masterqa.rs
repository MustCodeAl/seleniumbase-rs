// MasterQA and test-case management wrappers for BaseCase.

use crate::api::master_qa::QaStep;

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
    /// active QA session. If no session is active, a temporary one is used.
    pub fn manual_verify(&mut self, question: &str) -> bool {
        if let Some(session) = self.qa_session.as_mut() {
            session.verify(question)
        } else {
            MasterQaSession::new("Manual verification").verify(question)
        }
    }

    /// Generates a Markdown test-case management report from QA steps.
    pub fn qa_report_markdown(steps: &[QaStep], title: &str) -> String {
        let mut session = MasterQaSession::new(title);
        for step in steps {
            session.steps.push(step.clone());
        }
        session.to_markdown()
    }

    /// Saves a Markdown test-case report to the logs directory or a given path.
    pub fn save_qa_report<P: AsRef<Path>>(
        &self,
        steps: &[QaStep],
        title: &str,
        path: Option<P>,
    ) -> Result<std::path::PathBuf, SeleniumBaseError> {
        let markdown = Self::qa_report_markdown(steps, title);
        let target = match path {
            Some(p) => p.as_ref().to_owned(),
            None => {
                let logs = ensure_latest_logs_dir()?;
                artifact_path(&logs, "qa_report", "md")
            }
        };
        std::fs::write(&target, markdown)?;
        Ok(target)
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
}
