//! Converting Python SeleniumBase tests into Rust tests.
//!
//! [`convert`] reads a Python file with a small parser ([`parse`]), finds the
//! tests in it ([`units`]: classes with test methods, `def test_*`
//! functions, `with SB(...)` blocks and scripts), and writes each as an
//! rstest fixture test, the form `sbrs mkfile` writes:
//!
//! ```text
//! #[rstest]
//! #[tokio::test]
//! async fn name(sb: Sb) -> Result<()> {
//!     sb.run(async |sb| { ... Ok(()) }).await
//! }
//! ```
//!
//! A script that drives CDP Mode (`sb_cdp.Chrome()`, `sb.cdp.*`,
//! `activate_cdp_mode()`) becomes a pure CDP test (`sb_cdp: SbCdp`, `Page`
//! and `Locator` calls).
//!
//! The Python methods and their Rust calls are in [`table`] (the `BaseCase`
//! methods) and [`cdp`] (the pure CDP calls); the tests check both against the
//! real signatures. What is not translated stays as a `TODO` comment with a
//! diagnostic, so nothing is dropped silently.

mod ast;
mod cdp;
mod expr;
mod parse;
mod stmt;
mod table;
#[cfg(test)]
mod tests;
mod tr;
mod units;

use std::collections::{BTreeSet, HashSet};

use tr::{rust_ident, Tr};
use units::{Cases, Unit};

use crate::utilities::python_importer::{ImportDiagnostic, ImportSeverity};

/// A converted file.
pub(crate) struct Converted {
    /// The Rust source.
    pub(crate) rust: String,
    /// What was not converted.
    pub(crate) diagnostics: Vec<ImportDiagnostic>,
}

/// One test, written.
struct Written {
    /// The Rust function name.
    name: String,
    /// The body lines: indentation level and text.
    lines: Vec<(usize, String)>,
    /// A pure CDP test.
    cdp: bool,
    /// The `sb_cdp` names its lines use.
    imports: BTreeSet<&'static str>,
    /// The cases of a parameterized test.
    cases: Option<Cases>,
}

/// Converts a Python SeleniumBase file; `test_name` names the test when the
/// file holds one.
pub(crate) fn convert(source: &str, test_name: &str) -> Converted {
    let mut diagnostics = Vec::new();
    let module = match parse::parse_module(source) {
        Ok(module) => module,
        Err(error) => {
            let message = format!("could not read the file: {}", error.message);
            diagnostics.push(ImportDiagnostic {
                line: error.line,
                severity: ImportSeverity::Error,
                message: message.clone(),
            });
            let written = Written {
                name: fn_ident(test_name),
                lines: vec![(0, format!("// TODO(source line {}): {message}", error.line))],
                cdp: false,
                imports: BTreeSet::new(),
                cases: None,
            };
            return Converted {
                rust: render(None, test_name, &[], &[written]),
                diagnostics,
            };
        }
    };
    let found = units::discover(&module.stmts);
    let mut todos = Vec::new();
    for (line, message) in &found.todos {
        diagnostics.push(ImportDiagnostic {
            line: *line,
            severity: ImportSeverity::Warning,
            message: message.clone(),
        });
        todos.push(format!("// TODO(source line {line}): {message}"));
    }
    let single = found.units.len() == 1;
    let mut taken = HashSet::new();
    let mut written = Vec::new();
    for (index, unit) in found.units.iter().enumerate() {
        let base = if single {
            test_name.to_owned()
        } else if unit.name.is_empty() {
            format!("{test_name}_{}", index + 1)
        } else {
            unit.name.clone()
        };
        let name = unique(fn_ident(&base), &mut taken);
        let mut tr = Tr::new(
            &mut diagnostics,
            &module.comments,
            &module.code_lines,
            source,
        );
        translate_unit(&mut tr, unit);
        written.push(Written {
            name,
            lines: tr.out,
            cdp: unit.cdp,
            imports: tr.imports,
            cases: None,
        });
    }
    for (written, unit) in written.iter_mut().zip(found.units) {
        written.cases = unit.cases;
    }
    if written.is_empty() {
        written.push(Written {
            name: fn_ident(test_name),
            lines: vec![(0, "// TODO: no test was found in this file".to_owned())],
            cdp: false,
            imports: BTreeSet::new(),
            cases: None,
        });
        diagnostics.push(ImportDiagnostic {
            line: 1,
            severity: ImportSeverity::Warning,
            message: "no test was found in this file".to_owned(),
        });
    }
    diagnostics.sort_by_key(|diagnostic| diagnostic.line);
    Converted {
        rust: render(found.doc.as_deref(), test_name, &todos, &written),
        diagnostics,
    }
}

/// Translates the statements of one test into `tr`.
fn translate_unit(tr: &mut Tr<'_>, unit: &Unit) {
    tr.cdp = unit.cdp;
    for receiver in &unit.receivers {
        tr.receivers.insert(receiver.clone());
    }
    if let Some(cases) = &unit.cases {
        for (name, ty) in cases.names.iter().zip(&cases.types) {
            tr.declare(name, ty.clone());
        }
    }
    if let Some(args) = &unit.sb_args {
        tr.emit(format!(
            "// {args}: the browser settings come from the SB_* variables and the settings file"
        ));
    }
    tr.with_comments = false;
    tr.stmts(&unit.prologue);
    tr.with_comments = true;
    tr.comment_floor = if unit.name.is_empty() { 0 } else { unit.line };
    tr.stmts(&unit.body);
    tr.with_comments = false;
    tr.stmts(&unit.epilogue);
}

/// A file of one fixture test whose body is `lines` (a statement or comment
/// each), for the Selenium reading of `import_python`.
pub(crate) fn render_lines(test_name: &str, lines: &[String]) -> String {
    let written = Written {
        name: fn_ident(test_name),
        lines: lines.iter().map(|line| (0, line.clone())).collect(),
        cdp: false,
        imports: BTreeSet::new(),
        cases: None,
    };
    render(None, test_name, &[], &[written])
}

/// A Rust function name for a test.
fn fn_ident(name: &str) -> String {
    rust_ident(&name.to_ascii_lowercase())
}

/// `name`, or `name_2`, `name_3`... when it is taken.
fn unique(name: String, taken: &mut HashSet<String>) -> String {
    let mut candidate = name.clone();
    let mut count = 1;
    while !taken.insert(candidate.clone()) {
        count += 1;
        candidate = format!("{name}_{count}");
    }
    candidate
}

/// Four spaces per level.
fn indent(level: usize) -> String {
    "    ".repeat(level)
}

/// The whole Rust file.
fn render(doc: Option<&str>, test_name: &str, todos: &[String], tests: &[Written]) -> String {
    let mut out = String::new();
    match doc {
        Some(doc) if !doc.trim().is_empty() => {
            for line in doc.trim_matches('\n').lines() {
                let line = line.trim_end();
                if line.is_empty() {
                    out.push_str("//!\n");
                } else {
                    out.push_str(&format!("//! {line}\n"));
                }
            }
        }
        _ => out.push_str(&format!(
            "//! Converted from Python by `sbrs import-python`. Run it with `cargo test --test {test_name}`\n\
             //! when this file is in your crate's `tests/` folder; `SB_HEADLESS=true` runs it\n\
             //! without a window. Search for `TODO` to find what was not converted.\n"
        )),
    }
    out.push('\n');

    let base = tests.iter().any(|test| !test.cdp);
    let cdp = tests.iter().any(|test| test.cdp);
    let cdp_imports: BTreeSet<&str> = tests
        .iter()
        .flat_map(|test| test.imports.iter().copied())
        .collect();
    if !cdp_imports.is_empty() {
        let names: Vec<&str> = cdp_imports.into_iter().collect();
        out.push_str(&format!(
            "use seleniumbase_rs::sb_cdp::{{{}}};\n",
            names.join(", ")
        ));
    }
    let mut testing = vec!["rstest"];
    if base {
        testing.push("sb");
    }
    if cdp {
        testing.push("sb_cdp");
    }
    if base {
        testing.push("Sb");
    }
    if cdp {
        testing.push("SbCdp");
    }
    out.push_str(&format!(
        "use seleniumbase_rs::testing::{{{}}};\n",
        testing.join(", ")
    ));
    out.push_str("use seleniumbase_rs::{tokio, Result};\n");
    for todo in todos {
        out.push_str(todo);
        out.push('\n');
    }
    for test in tests {
        out.push('\n');
        out.push_str(&render_test(test));
    }
    out
}

/// One test function.
fn render_test(test: &Written) -> String {
    let mut out = String::from("#[rstest]\n");
    let mut params = String::new();
    if let Some(cases) = &test.cases {
        for row in &cases.rows {
            out.push_str(&format!("#[case({})]\n", row.join(", ")));
        }
        for (name, rust_type) in cases.names.iter().zip(&cases.rust_types) {
            params.push_str(&format!(", #[case] {}: {rust_type}", rust_ident(name)));
        }
    }
    out.push_str("#[tokio::test]\n");
    let (fixture, base_level) = if test.cdp {
        ("sb_cdp: SbCdp", 3)
    } else {
        ("sb: Sb", 2)
    };
    out.push_str(&format!(
        "async fn {}({fixture}{params}) -> Result<()> {{\n",
        test.name
    ));
    if test.cdp {
        out.push_str("    sb_cdp\n        .run(async |page| {\n");
    } else {
        out.push_str("    sb.run(async |sb| {\n");
    }
    for (level, text) in &test.lines {
        if text.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&indent(base_level + level));
            out.push_str(text);
            out.push('\n');
        }
    }
    out.push_str(&indent(base_level));
    out.push_str("Ok(())\n");
    if test.cdp {
        out.push_str("        })\n        .await\n}\n");
    } else {
        out.push_str("    })\n    .await\n}\n");
    }
    out
}
