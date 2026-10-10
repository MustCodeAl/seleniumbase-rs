use std::io::{self, BufRead, Write};
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
            Self::read_line(&mut io::stdin().lock()).filter(|notes| !notes.is_empty())
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
                step.id,
                table_cell(&step.question),
                result,
                table_cell(notes)
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
        let passed = Self::answer(&mut io::stdin().lock());
        if passed.is_none() {
            println!("\nNo answer was given (input is closed); recording the step as failed.");
        }
        passed.unwrap_or(false)
    }

    /// Reads one answer: Enter or anything but `n` / `no` means yes, as the
    /// `[Y/n]` prompt says. `None` when there was no answer at all, because
    /// input is closed, so an unattended run cannot pass its manual checks.
    fn answer(input: &mut impl BufRead) -> Option<bool> {
        let line = Self::read_line(input)?.to_lowercase();
        Some(!(line == "n" || line == "no"))
    }

    /// One trimmed line, or `None` at end of input or on a read error.
    fn read_line(input: &mut impl BufRead) -> Option<String> {
        let mut line = String::new();
        match input.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_owned()),
        }
    }
}

/// `text` made safe for one cell of a Markdown table.
fn table_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// Stateless helper for simple one-off prompts.
pub struct MasterQA;

impl MasterQA {
    pub fn verify(question: &str) -> bool {
        MasterQaSession::new("Standalone verification").verify(question)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn answer(text: &str) -> Option<bool> {
        MasterQaSession::answer(&mut Cursor::new(text.as_bytes()))
    }

    #[test]
    fn enter_and_yes_pass_and_no_fails() {
        assert_eq!(answer("\n"), Some(true));
        assert_eq!(answer("y\n"), Some(true));
        assert_eq!(answer("yes\n"), Some(true));
        assert_eq!(answer("n\n"), Some(false));
        assert_eq!(answer(" No \n"), Some(false));
    }

    #[test]
    fn closed_input_is_no_answer_and_not_a_pass() {
        assert_eq!(answer(""), None);
    }

    #[test]
    fn a_table_cell_cannot_break_the_table() {
        assert_eq!(table_cell("a | b\nc"), "a \\| b c");
        let mut session = MasterQaSession::new("T");
        session.steps.push(QaStep {
            id: 1,
            question: "is a|b shown?".into(),
            passed: false,
            notes: Some("line 1\nline 2".into()),
        });
        let md = session.to_markdown();
        let row = md.lines().find(|l| l.starts_with("| 1 ")).unwrap();
        assert_eq!(row, "| 1 | is a\\|b shown? | FAIL | line 1 line 2 |");
    }
}
