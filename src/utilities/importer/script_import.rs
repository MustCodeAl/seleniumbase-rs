//! The Rust a script import produced, and what it could not convert.

use super::{ImportDiagnostic, ScriptLanguage};

/// The generated Rust and what could not be converted.
#[derive(Clone, Debug)]
pub struct ScriptImport {
    /// The Rust file: one `#[tokio::test]` per test.
    pub rust: String,
    /// One diagnostic per statement left as a `TODO`, with its line.
    pub diagnostics: Vec<ImportDiagnostic>,
    /// The language the source was read as.
    pub language: ScriptLanguage,
    /// The names of the generated test functions.
    pub tests: Vec<String>,
}

impl ScriptImport {
    /// Whether everything was converted: no statement became a `TODO`.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.diagnostics.is_empty()
    }
}
