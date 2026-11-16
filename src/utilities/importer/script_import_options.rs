//! How a script import chooses a language and a test name.

use super::ScriptLanguage;

/// How to import.
#[derive(Clone, Debug)]
pub struct ScriptImportOptions {
    /// The source language; `None` detects it from the content.
    pub language: Option<ScriptLanguage>,
    /// The name of the test generated for a script that has no test titles
    /// (codegen output, a plain script).
    pub test_name: String,
}

impl Default for ScriptImportOptions {
    fn default() -> Self {
        Self {
            language: None,
            test_name: "imported_test".to_owned(),
        }
    }
}
