use std::io::{self, Write};
use std::path::Path;

/// A single manual verification step.
#[derive(Clone, Debug)]
pub struct QaStep {
    pub id: usize,
    pub question: String,
    pub passed: bool,
    pub notes: Option<String>,
}

/// Stateful manual QA session that records verification steps.
#[derive(Clone, Debug, Default)]
pub struct MasterQaSession {
    pub title: String,
    pub steps: Vec<QaStep>,
}

impl MasterQaSession {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            steps: Vec::new(),
        }
    }

    /// Prompts the user to verify a step and records the result.
    pub fn verify(&mut self, question: &str) -> bool {
        let passed = Self::prompt(question);
        self.steps.push(QaStep {
            id: self.steps.len() + 1,
            question: question.to_owned(),
            passed,
            notes: None,
        });
        passed
    }

    /// Prompts the user for a verification step with optional notes.
    pub fn verify_with_notes(&mut self, question: &str) -> bool {
        let passed = Self::prompt(question);
        let notes = if passed {
            None
        } else {
            println!("Enter failure notes (optional):");
            Self::read_line()
        };
        self.steps.push(QaStep {
            id: self.steps.len() + 1,
            question: question.to_owned(),
            passed,
            notes,
        });
        passed
    }

    pub fn all_passed(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|s| s.passed)
    }

    pub fn passed_count(&self) -> usize {
        self.steps.iter().filter(|s| s.passed).count()
    }

    /// Generates a Markdown test-case management report.
    pub fn to_markdown(&self) -> String {
        let mut md = format!("# {}\n\n", self.title);
        md.push_str("| Step | Question | Result | Notes |\n");
        md.push_str("|------|----------|--------|-------|\n");
        for step in &self.steps {
            let result = if step.passed { "PASS" } else { "FAIL" };
            let notes = step.notes.as_deref().unwrap_or("-");
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                step.id, step.question, result, notes
            ));
        }
        md.push_str(&format!(
            "\n**Summary:** {} / {} steps passed.\n",
            self.passed_count(),
            self.steps.len()
        ));
        md
    }

    /// Writes the Markdown report to a file.
    pub fn save_markdown<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<std::path::PathBuf, crate::error::SeleniumBaseError> {
        let path = path.as_ref();
        std::fs::write(path, self.to_markdown())?;
        Ok(path.to_owned())
    }

    fn prompt(question: &str) -> bool {
        print!("Manual QA verification required: {} [Y/n] ", question);
        let _ = io::stdout().flush();
        let input = Self::read_line().unwrap_or_default();
        let input = input.trim().to_lowercase();
        !(input == "n" || input == "no")
    }

    fn read_line() -> Option<String> {
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            Some(input.trim().to_owned())
        } else {
            None
        }
    }
}

/// Stateless helper for simple one-off prompts.
pub struct MasterQA;

impl MasterQA {
    pub fn verify(question: &str) -> bool {
        MasterQaSession::new("Standalone verification").verify(question)
    }
}
