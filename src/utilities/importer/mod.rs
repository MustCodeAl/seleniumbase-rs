//! Converting Playwright and Puppeteer tests and scripts into Rust tests for
//! the pure CDP engine ([`sb_cdp`](crate::sb_cdp)).
//!
//! [`import_playwright`] reads `@playwright/test` files, `playwright codegen`
//! output and pytest-playwright, sync and async Python scripts.
//! [`import_puppeteer`] reads Puppeteer scripts, jest-puppeteer and Mocha
//! tests, and the Chrome DevTools Recorder's JSON export. Both write one
//! `#[tokio::test]` per test that launches a [`Browser`](crate::sb_cdp::Browser)
//! and drives its page with [`Page`](crate::sb_cdp::Page) and
//! [`Locator`](crate::sb_cdp::Locator) calls, the way
//! [`ActionRecorder::to_cdp_test`](crate::api::recorder::ActionRecorder::to_cdp_test)
//! writes a recording.
//!
//! Anything the importer does not convert (request interception, event
//! handlers, popups, loops, conditionals, helper functions, page objects,
//! fixtures other than the page) is kept in place as a
//! `// TODO(playwright): <statement>` (or `TODO(puppeteer)`) comment with an
//! [`ImportDiagnostic`] naming its line, so nothing is dropped silently;
//! [`ScriptImport::is_complete`] is then `false`. `docs/playwright-migration.md`
//! and `docs/puppeteer-migration.md` list every mapping.
//!
//! The input is untrusted. It is read by a small hand-written tokenizer and
//! parser with bounded nesting and size, and text from it reaches the Rust only
//! inside string literals and comments, both escaped, or as identifiers made of
//! `a-z`, `0-9` and `_`.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::utilities::importer::{import_playwright, ScriptImportOptions};
//!
//! let source = r#"
//! import { test, expect } from '@playwright/test';
//! test('shop has a title', async ({ page }) => {
//!   await page.goto('https://example.com/');
//!   await expect(page).toHaveTitle('Example Domain');
//! });
//! "#;
//! let imported = import_playwright(source, &ScriptImportOptions::default())?;
//! assert!(imported.is_complete());
//! assert!(imported.rust.contains("async fn shop_has_a_title()"));
//! assert!(imported.rust.contains("page.goto(\"https://example.com/\").await?;"));
//! assert!(imported.rust.contains("page.expect().to_have_title(\"Example Domain\").await?;"));
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

mod emit;
mod lexer;
mod parser;
mod program;
pub(crate) mod pyconv;
mod recorder;
mod selector;
mod translate;

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

pub use crate::utilities::python_importer::{ImportDiagnostic, ImportSeverity};
use crate::SeleniumBaseError;

use emit::{TestOut, Tool};
use lexer::Dialect;
use program::PageBinding;
use translate::{Flavor, Translator, Var};

/// The largest source file the importers read: 2 MB.
pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;

/// The language of a source file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ScriptLanguage {
    /// JavaScript (`.js`, `.mjs`, `.cjs`).
    JavaScript,
    /// TypeScript (`.ts`, `.mts`, `.cts`); type annotations are read past.
    TypeScript,
    /// Python (`.py`).
    Python,
    /// The Chrome DevTools Recorder's JSON export (`.json`), for Puppeteer.
    RecorderJson,
}

impl ScriptLanguage {
    /// The language a file extension stands for, if it is one of ours.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use seleniumbase_rs::utilities::importer::ScriptLanguage;
    ///
    /// assert_eq!(ScriptLanguage::from_path(Path::new("login.spec.ts")), Some(ScriptLanguage::TypeScript));
    /// assert_eq!(ScriptLanguage::from_path(Path::new("test_login.py")), Some(ScriptLanguage::Python));
    /// assert_eq!(ScriptLanguage::from_path(Path::new("notes.txt")), None);
    /// ```
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "ts" | "mts" | "cts" | "tsx" => Some(Self::TypeScript),
            "py" => Some(Self::Python),
            "json" => Some(Self::RecorderJson),
            _ => None,
        }
    }

    /// Guesses the language from the source text: a JSON object with
    /// `"steps"` is a Recorder export, Python is recognised by `def` and its
    /// Playwright imports, TypeScript by type annotations; anything else is
    /// JavaScript.
    #[must_use]
    pub fn detect(source: &str) -> Self {
        let trimmed = source.trim_start_matches('\u{feff}').trim_start();
        if trimmed.starts_with('{') && trimmed.contains("\"steps\"") {
            return Self::RecorderJson;
        }
        let python = source.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("def ")
                || line.starts_with("async def ")
                || line.starts_with("from playwright.")
                || line.starts_with("import pytest")
                || line.starts_with("with sync_playwright()")
        });
        if python {
            return Self::Python;
        }
        let typescript = [
            ": Page",
            ": Browser",
            ": Locator",
            "import type ",
            "interface ",
            ": string",
            ": number",
            ": Promise<",
        ]
        .iter()
        .any(|marker| source.contains(marker));
        if typescript {
            Self::TypeScript
        } else {
            Self::JavaScript
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Python => "Python",
            Self::RecorderJson => "Recorder JSON",
        }
    }
}

impl fmt::Display for ScriptLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

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

/// Converts Playwright tests or a Playwright script (JavaScript, TypeScript
/// or Python) into Rust tests for the pure CDP engine.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Unsupported`] for a source larger than
/// [`MAX_SOURCE_BYTES`] or a Recorder JSON file, and
/// [`SeleniumBaseError::Parse`] when the source cannot be tokenized (a string
/// or comment that is never closed). Statements the importer does not convert
/// are not errors: they become `TODO`s and diagnostics.
pub fn import_playwright(
    source: &str,
    options: &ScriptImportOptions,
) -> Result<ScriptImport, SeleniumBaseError> {
    import(Tool::Playwright, source, options)
}

/// Converts Puppeteer scripts, jest-puppeteer or Mocha tests (JavaScript or
/// TypeScript), or a Chrome DevTools Recorder JSON export, into Rust tests
/// for the pure CDP engine.
///
/// # Errors
///
/// As [`import_playwright`]; Python is refused, as Puppeteer has no Python
/// API.
pub fn import_puppeteer(
    source: &str,
    options: &ScriptImportOptions,
) -> Result<ScriptImport, SeleniumBaseError> {
    import(Tool::Puppeteer, source, options)
}

fn import(
    tool: Tool,
    source: &str,
    options: &ScriptImportOptions,
) -> Result<ScriptImport, SeleniumBaseError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(SeleniumBaseError::Unsupported(format!(
            "the source is {} bytes; the importer reads at most {MAX_SOURCE_BYTES} bytes (2 MB)",
            source.len()
        )));
    }
    let language = options
        .language
        .unwrap_or_else(|| ScriptLanguage::detect(source));
    let (flavor, tag) = match tool {
        Tool::Playwright => (Flavor::Playwright, "Playwright"),
        Tool::Puppeteer => (Flavor::Puppeteer, "Puppeteer"),
    };
    match (tool, language) {
        (Tool::Playwright, ScriptLanguage::RecorderJson) => {
            return Err(SeleniumBaseError::Unsupported(
                "a Chrome Recorder JSON export is a Puppeteer recording: use `sbrs import-puppeteer`"
                    .to_owned(),
            ))
        }
        (Tool::Puppeteer, ScriptLanguage::Python) => {
            return Err(SeleniumBaseError::Unsupported(
                "Puppeteer has no Python API: use `sbrs import-playwright` for Playwright Python"
                    .to_owned(),
            ))
        }
        _ => {}
    }
    let python = language == ScriptLanguage::Python;
    let dialect = if python { Dialect::Python } else { Dialect::Js };
    let lexed = lexer::tokenize(source, dialect).map_err(|error| SeleniumBaseError::Parse {
        input: format!("line {}: {}", error.line, error.message),
        expected: format!("{} the importer can read", language.name()),
    })?;
    let mut translator = Translator::new(flavor, python, source);
    let mut tests = Vec::new();
    let mut file_todos = Vec::new();
    let mut names = HashSet::new();
    let mut add_test =
        |base: String, ignore: Option<String>, lines, names: &mut HashSet<String>| {
            let mut name = base.clone();
            let mut n = 2;
            while !names.insert(name.clone()) {
                name = format!("{base}_{n}");
                n += 1;
            }
            tests.push(TestOut {
                name,
                ignore,
                lines,
            });
        };
    let fallback = emit::test_ident(&options.test_name, "imported_test");

    if language == ScriptLanguage::RecorderJson {
        let value = parser::parse_value(&lexed).map_err(|message| SeleniumBaseError::Parse {
            input: message,
            expected: "a Chrome Recorder JSON export".to_owned(),
        })?;
        let spans = recorder::step_spans(&lexed);
        translator.begin();
        translator.bind_page("page");
        let title = recorder::translate(&value, &spans, &mut translator).map_err(|message| {
            SeleniumBaseError::Parse {
                input: message,
                expected: "a Chrome Recorder JSON export".to_owned(),
            }
        })?;
        let lines = translator.finish();
        let name = title.map_or_else(|| fallback.clone(), |t| emit::test_ident(&t, &fallback));
        add_test(name, None, lines, &mut names);
    } else {
        let stmts = parser::parse(&lexed, dialect);
        let structure = if python {
            program::python(&stmts)
        } else {
            program::js(&stmts)
        };
        for (stmt, reason) in &structure.todos {
            file_todos.push(translator.file_todo(stmt, reason));
        }
        for case in &structure.cases {
            translator.begin();
            for (name, obj) in &case.objects {
                translator.bind(name, Var::Object(*obj));
            }
            match &case.page {
                PageBinding::Name(name) => translator.bind_page(name),
                PageBinding::Global => {
                    translator.bind_page("page");
                    translator.bind("browser", Var::Object(translate::Obj::Browser));
                    translator.bind("context", Var::Object(translate::Obj::Context));
                }
                PageBinding::Opened => {}
            }
            translator.run(&case.setup);
            translator.run(&case.body);
            translator.run(&case.teardown);
            let lines = translator.finish();
            let name = if case.title.is_empty() {
                fallback.clone()
            } else {
                emit::test_ident(&case.title.join(" "), &fallback)
            };
            add_test(name, case.skip.clone(), lines, &mut names);
        }
        if structure.cases.is_empty() {
            translator.diagnostics.push(ImportDiagnostic {
                line: 1,
                severity: ImportSeverity::Warning,
                message: format!("no {tag} test or script was found"),
            });
        }
    }
    let rust = emit::render(tool, &tests, &file_todos, &translator.needs);
    let mut diagnostics = translator.diagnostics;
    diagnostics.sort_by_key(|diagnostic| diagnostic.line);
    Ok(ScriptImport {
        rust,
        diagnostics,
        language,
        tests: tests.into_iter().map(|test| test.name).collect(),
    })
}

// Named so that `cargo test playwright` (and `puppeteer`) selects them.
#[cfg(test)]
#[path = "tests.rs"]
mod playwright_puppeteer_tests;
