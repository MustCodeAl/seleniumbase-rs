//! Case plans: the markdown files that say what a test does, and the summary
//! of them, as the Python Case Plans Generator writes them.
//!
//! A test is named by its **id**, `tests/login.rs::valid_login` (the file, then
//! the test function), or the file alone when it holds no test function. The
//! plan of a test is `case_plans/<name>.md` in the folder of its file, where
//! the name is the id's last part with `::` turned into `.`:
//! `tests/case_plans/login.valid_login.md`.
//!
//! Everything here is plain data in and files out, with the folder the ids are
//! relative to passed in, so it is tested in a scratch folder. The terminal app
//! in `caseplans_tui` is a thin shell around it.

use std::io;
use std::path::{Component, Path, PathBuf};

use super::discovery::{rust_files, test_folders};
use crate::cli::commands::write_new_file;

/// The folder next to a test file that holds its plans.
pub const PLANS_FOLDER: &str = "case_plans";

/// The file the summary of all plans is written to, in the base folder.
pub const SUMMARY_FILE: &str = "case_summary.md";

/// The row the boilerplate plan has, and a plan whose steps were written does
/// not.
const BOILERPLATE_ROW: &str = "| 1 | Perform Action 1 | Verify Action 1 |";

/// The ids of the tests under `target`, relative to `base`.
///
/// `target` is a folder (relative to `base`, or absolute) or one `.rs` file.
/// A folder that has a `tests` or an `examples` folder is searched there, as
/// `sbase commander` does; any other folder is searched itself. Only the
/// `*.rs` files directly inside a folder count, in name order. In a file, each
/// test function is one id, in source order; a file with none is one id.
///
/// # Errors
///
/// Returns the error of reading a folder or a file.
pub fn test_ids(base: &Path, target: &Path) -> io::Result<Vec<String>> {
    let root = base.join(target);
    let files = if root.is_file() {
        vec![root]
    } else {
        let mut files = Vec::new();
        for folder in test_folders(&root) {
            files.extend(rust_files(&folder)?);
        }
        files
    };
    let mut ids = Vec::new();
    for file in files {
        let shown = shown_path(base, &file);
        let source = String::from_utf8_lossy(&std::fs::read(&file)?).into_owned();
        let functions = test_functions(&source);
        if functions.is_empty() {
            ids.push(shown);
        } else {
            ids.extend(functions.into_iter().map(|name| format!("{shown}::{name}")));
        }
    }
    Ok(ids)
}

/// `path` relative to `base`, with `/` between folders and no `.` parts; the
/// path as it is when it is not below `base`.
fn shown_path(base: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(base).unwrap_or(path);
    let mut shown = String::new();
    for part in relative.components() {
        match part {
            Component::CurDir => {}
            Component::RootDir => shown.push('/'),
            other => {
                if !shown.is_empty() && !shown.ends_with('/') {
                    shown.push('/');
                }
                shown.push_str(&other.as_os_str().to_string_lossy());
            }
        }
    }
    shown
}

/// The names of the test functions in Rust `source`, in order.
///
/// This reads the text, it does not parse it: a function counts when a `test`
/// attribute (`#[test]`, `#[tokio::test(..)]`, `#[rstest]`) comes before it,
/// with only other attributes, comments and blank lines in between.
#[must_use]
pub fn test_functions(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    // Set by a test attribute, until the function it belongs to.
    let mut pending = false;
    for line in source.lines() {
        let line = line.trim();
        if let Some(attribute) = line.strip_prefix("#[") {
            let (name, rest) = attribute.split_once(']').unwrap_or((attribute, ""));
            if is_test_attribute(name) {
                pending = true;
            }
            // `#[test] fn x() {}` on one line.
            if pending {
                if let Some(name) = function_name(rest.trim()) {
                    names.push(name);
                    pending = false;
                }
            }
            continue;
        }
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if pending {
            // Any code ends the run of attributes: it is the function, or it
            // is something a test attribute does not apply to.
            names.extend(function_name(line));
            pending = false;
        }
    }
    names
}

/// Whether the attribute `body` (what is between `#[` and `]`) marks a test:
/// its path ends in `test` or `rstest`.
fn is_test_attribute(body: &str) -> bool {
    let path = body
        .trim()
        .split(|c: char| c == '(' || c == '=' || c.is_whitespace())
        .next()
        .unwrap_or_default();
    matches!(path.rsplit("::").next(), Some("test" | "rstest"))
}

/// The name of the function declared by `line`, if it declares one:
/// `pub async fn name(..)`, `unsafe fn name<T>(..)`.
fn function_name(line: &str) -> Option<String> {
    let mut rest = line;
    loop {
        let (word, tail) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        match word {
            "pub" | "async" | "unsafe" | "const" => rest = tail.trim_start(),
            word if word.starts_with("pub(") => rest = tail.trim_start(),
            "fn" => {
                let name: String = tail
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                return (!name.is_empty()).then_some(name);
            }
            _ => return None,
        }
    }
}

/// The path of the plan of test `id`, below `base`.
#[must_use]
pub fn plan_path(base: &Path, id: &str) -> PathBuf {
    let (folder, address) = match id.rsplit_once('/') {
        Some((folder, address)) => (Some(folder), address),
        None => (None, id),
    };
    let mut path = base.to_path_buf();
    if let Some(folder) = folder {
        path.push(folder);
    }
    path.push(PLANS_FOLDER);
    path.push(plan_file_name(address));
    path
}

/// The file name of the plan of the test at `address` (the id without its
/// folders): `login.rs::valid_login` is `login.valid_login.md`.
#[must_use]
pub fn plan_file_name(address: &str) -> String {
    let name = address
        .replace(".rs::", ".")
        .replace("::", ".")
        .replace(' ', "_");
    format!("{name}.md")
}

/// The text of a new plan for test `id`: a title and a table of two
/// placeholder steps to be replaced.
#[must_use]
pub fn boilerplate(id: &str) -> String {
    let address = id.rsplit_once('/').map_or(id, |(_, address)| address);
    format!(
        "``{address}``\n\
         ---\n\
         | # | Step Description | Expected Result |\n\
         | - | ---------------- | --------------- |\n\
         {BOILERPLATE_ROW}\n\
         | 2 | Perform Action 2 | Verify Action 2 |\n"
    )
}

/// Whether test `id` has a plan under `base`.
#[must_use]
pub fn has_plan(base: &Path, id: &str) -> bool {
    plan_path(base, id).is_file()
}

/// Reads the plan of test `id`.
///
/// # Errors
///
/// Returns the error of reading the file, which is
/// [`io::ErrorKind::NotFound`] when there is no plan.
pub fn read_plan(base: &Path, id: &str) -> io::Result<String> {
    std::fs::read_to_string(plan_path(base, id))
}

/// Writes `text` as the plan of test `id`, replacing the plan that is there,
/// and returns its path. The folder is created if needed.
///
/// # Errors
///
/// Returns the error of creating the folder or writing the file.
pub fn write_plan(base: &Path, id: &str, text: &str) -> io::Result<PathBuf> {
    let path = plan_path(base, id);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(&path, text)?;
    Ok(path)
}

/// What [`create_boilerplates`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Created {
    /// The plans that were written.
    pub created: Vec<PathBuf>,
    /// How many of the tests already had a plan, which is left alone.
    pub kept: usize,
}

/// Writes a boilerplate plan for each of `ids` that has none. A plan that is
/// there is never replaced.
///
/// # Errors
///
/// Returns the error of creating a folder or a file.
pub fn create_boilerplates(base: &Path, ids: &[String]) -> io::Result<Created> {
    let mut made = Created::default();
    for id in ids {
        let path = plan_path(base, id);
        match write_new_file(&path, &boilerplate(id)) {
            Ok(()) => made.created.push(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => made.kept += 1,
            Err(error) => return Err(error),
        }
    }
    Ok(made)
}

/// How far a plan is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanKind {
    /// The steps were written: a table that is not the boilerplate.
    Customized,
    /// Still the template.
    Boilerplate,
    /// Too little to be a plan: it has no table yet.
    Unfinished,
}

/// Tells how far the plan `text` is.
#[must_use]
pub fn classify(text: &str) -> PlanKind {
    // A table needs the pipes of a header, a rule and a row, and the dashes of
    // the rule: Python's own test for "too little to be a plan".
    if text.matches('|').count() < 9 || text.matches('-').count() < 3 {
        PlanKind::Unfinished
    } else if text.contains(BOILERPLATE_ROW) {
        PlanKind::Boilerplate
    } else {
        PlanKind::Customized
    }
}

/// The summary of the plans that exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Plans with their own steps.
    pub customized: usize,
    /// Plans that are still the template.
    pub boilerplate: usize,
    /// Plans with no table.
    pub unfinished: usize,
    /// The markdown of `case_summary.md`.
    pub markdown: String,
}

/// Builds the summary of `plans`, pairs of a test id and the text of its plan.
/// `None` when there are no plans, since there is nothing to summarise.
#[must_use]
pub fn summarize(plans: &[(String, String)]) -> Option<Summary> {
    if plans.is_empty() {
        return None;
    }
    let mut counts = [0_usize; 3];
    let mut blocks = Vec::with_capacity(plans.len());
    for (id, text) in plans {
        let (kind, block) = fold(id, text);
        counts[match kind {
            PlanKind::Customized => 0,
            PlanKind::Boilerplate => 1,
            PlanKind::Unfinished => 2,
        }] += 1;
        blocks.push(block);
    }
    let [customized, boilerplate, unfinished] = counts;

    let mut lines = vec![
        "<h2>Summary of existing Case Plans</h2>".to_owned(),
        String::new(),
        "|   |    |   |".to_owned(),
        "| - | -: | - |".to_owned(),
        format!(
            "| {READY} | {customized} | {} |",
            if customized == 1 {
                "Case Plan with a customized table"
            } else {
                "Case Plans with customized tables"
            }
        ),
        format!(
            "| {BOILERPLATE} | {boilerplate} | {} |",
            if boilerplate == 1 {
                "Case Plan using boilerplate code"
            } else {
                "Case Plans using boilerplate code"
            }
        ),
        format!(
            "| {UNFINISHED} | {unfinished} | {} |",
            if unfinished == 1 {
                "Case Plan that is missing a table"
            } else {
                "Case Plans that are missing tables"
            }
        ),
        String::new(),
        "--------".to_owned(),
        String::new(),
        "<h3>\u{1F50E} (Click rows to expand) \u{1F50D}</h3>".to_owned(),
        String::new(),
    ];
    for block in blocks {
        lines.push(block);
        lines.push(String::new());
    }
    Some(Summary {
        customized,
        boilerplate,
        unfinished,
        markdown: lines.join("\n"),
    })
}

/// The marker of a plan with its own steps, one still the template, and one
/// with no table.
const READY: &str = "\u{1F535}";
const BOILERPLATE: &str = "\u{2B55}";
const UNFINISHED: &str = "\u{1F6A7}";

/// How far plan `text` of test `id` is, and its block of the summary: a
/// collapsed `<details>` with the plan inside, or one line when it has no
/// table.
fn fold(id: &str, text: &str) -> (PlanKind, String) {
    let kind = classify(text);
    let text = text.replace("\r\n", "\n");
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    if kind == PlanKind::Unfinished || lines.len() < 3 {
        return (
            PlanKind::Unfinished,
            format!("{UNFINISHED} <code><b>{id}</b></code>"),
        );
    }
    let marker = if kind == PlanKind::Boilerplate {
        BOILERPLATE
    } else {
        READY
    };
    // The first line is the title, ``address``; a plan without one gets it.
    let title = lines[0].trim();
    if title.starts_with("``") && title.ends_with("``") {
        lines[0] = format!("``{id}``");
    } else {
        lines.insert(0, format!("``{id}``"));
    }
    lines.insert(0, "<details>".to_owned());
    lines[1] = format!("<summary> {marker} <code><b>{id}</b></code></summary>");
    // The rule of dashes under the title is not shown; a blank line ends the
    // summary.
    let next = lines[2].trim().to_owned();
    if next.starts_with('-') && next.ends_with('-') {
        lines[2] = String::new();
    } else if !next.is_empty() {
        lines.insert(2, String::new());
    }
    if lines.last().is_some_and(|last| !last.trim().is_empty()) {
        lines.push(String::new());
    }
    lines.push("</details>".to_owned());
    (kind, lines.join("\n"))
}

/// Writes [`SUMMARY_FILE`] in `base` for the tests of `ids` that have a plan,
/// replacing the earlier one. `None` when none has a plan, and no file is
/// written.
///
/// # Errors
///
/// Returns the error of reading a plan or writing the summary.
pub fn write_summary(base: &Path, ids: &[String]) -> io::Result<Option<(PathBuf, Summary)>> {
    let mut plans = Vec::new();
    for id in ids {
        match read_plan(base, id) {
            Ok(text) => plans.push((id.clone(), text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let Some(summary) = summarize(&plans) else {
        return Ok(None);
    };
    let path = base.join(SUMMARY_FILE);
    std::fs::write(&path, &summary.markdown)?;
    Ok(Some((path, summary)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn write(dir: &Path, name: &str, text: &str) {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    // ------------------------------------------------------------------
    // Finding tests
    // ------------------------------------------------------------------

    #[test]
    fn a_test_attribute_names_the_function_below_it() {
        let source = "\
use super::*;

#[test]
fn plain() {}

#[tokio::test(flavor = \"multi_thread\")]
async fn with_a_runtime() {}

/// Docs between.
#[test]
#[ignore = \"needs Chrome\"]
// and a comment
pub(crate) async fn ignored_one() {}

#[rstest]
fn parameterised(x: u8) {}

#[test] fn on_one_line() {}
";

        assert_eq!(
            test_functions(source),
            [
                "plain",
                "with_a_runtime",
                "ignored_one",
                "parameterised",
                "on_one_line"
            ]
        );
    }

    #[test]
    fn code_that_is_not_a_test_is_not_listed() {
        let source = "\
#[cfg(test)]
mod tests {
    fn helper() {}

    // #[test]
    fn commented_out() {}

    #[derive(Debug)]
    struct Fixture;

    #[test]
    struct NotAFunction;

    fn after_it() {}
}
";

        assert!(
            test_functions(source).is_empty(),
            "{:?}",
            test_functions(source)
        );
    }

    #[test]
    fn a_folder_is_listed_file_by_file_and_test_by_test() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "tests/login.rs",
            "#[test]\nfn valid() {}\n#[test]\nfn invalid() {}\n",
        );
        write(dir.path(), "tests/helpers.rs", "pub fn setup() {}\n");
        write(dir.path(), "tests/about.rs", "#[test]\nfn page() {}\n");
        write(dir.path(), "tests/notes.txt", "not rust");
        write(dir.path(), "examples/demo.rs", "fn main() {}\n");

        let found = test_ids(dir.path(), Path::new(".")).unwrap();

        assert_eq!(
            found,
            [
                "tests/about.rs::page",
                "tests/helpers.rs",
                "tests/login.rs::valid",
                "tests/login.rs::invalid",
                "examples/demo.rs",
            ]
        );
    }

    #[test]
    fn a_folder_without_tests_or_examples_is_listed_itself() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "suite/one.rs", "#[test]\nfn a() {}\n");
        write(dir.path(), "suite/nested/two.rs", "#[test]\nfn b() {}\n");

        let found = test_ids(dir.path(), Path::new("suite")).unwrap();

        assert_eq!(found, ["suite/one.rs::a"]);
    }

    #[test]
    fn one_file_can_be_asked_for() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "tests/login.rs", "#[test]\nfn valid() {}\n");
        write(dir.path(), "tests/other.rs", "#[test]\nfn x() {}\n");

        let found = test_ids(dir.path(), Path::new("tests/login.rs")).unwrap();

        assert_eq!(found, ["tests/login.rs::valid"]);
    }

    #[test]
    fn a_target_that_is_not_there_has_no_tests() {
        let dir = tempfile::tempdir().unwrap();

        assert!(test_ids(dir.path(), Path::new("nowhere"))
            .unwrap()
            .is_empty());
    }

    // ------------------------------------------------------------------
    // Where plans live and what is in them
    // ------------------------------------------------------------------

    #[test]
    fn a_plan_lives_in_case_plans_next_to_its_test_file() {
        let base = Path::new("/work");

        assert_eq!(
            plan_path(base, "tests/login.rs::valid_login"),
            Path::new("/work/tests/case_plans/login.valid_login.md")
        );
        assert_eq!(
            plan_path(base, "tests/ui/login.rs::valid"),
            Path::new("/work/tests/ui/case_plans/login.valid.md")
        );
        assert_eq!(
            plan_path(base, "login.rs::valid"),
            Path::new("/work/case_plans/login.valid.md")
        );
        assert_eq!(
            plan_path(base, "tests/login.rs"),
            Path::new("/work/tests/case_plans/login.rs.md")
        );
    }

    #[test]
    fn the_plan_file_name_follows_the_python_test_id() {
        assert_eq!(
            plan_file_name("login.rs::valid_login"),
            "login.valid_login.md"
        );
        assert_eq!(plan_file_name("a.rs::b::c"), "a.b.c.md");
        assert_eq!(plan_file_name("my test.rs::x"), "my_test.x.md");
    }

    #[test]
    fn the_boilerplate_is_the_python_template_with_the_address_as_its_title() {
        assert_eq!(
            boilerplate("tests/login.rs::valid_login"),
            "``login.rs::valid_login``\n\
             ---\n\
             | # | Step Description | Expected Result |\n\
             | - | ---------------- | --------------- |\n\
             | 1 | Perform Action 1 | Verify Action 1 |\n\
             | 2 | Perform Action 2 | Verify Action 2 |\n"
        );
    }

    // ------------------------------------------------------------------
    // Reading, writing and creating
    // ------------------------------------------------------------------

    #[test]
    fn a_plan_is_written_read_back_and_replaced_on_save() {
        let dir = tempfile::tempdir().unwrap();
        let id = "tests/login.rs::valid";
        assert!(!has_plan(dir.path(), id));
        assert_eq!(
            read_plan(dir.path(), id).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );

        let path = write_plan(dir.path(), id, "first\n").unwrap();
        assert_eq!(path, dir.path().join("tests/case_plans/login.valid.md"));
        assert!(has_plan(dir.path(), id));
        write_plan(dir.path(), id, "second\n").unwrap();

        assert_eq!(read_plan(dir.path(), id).unwrap(), "second\n");
    }

    #[test]
    fn boilerplates_are_written_only_for_tests_without_a_plan() {
        let dir = tempfile::tempdir().unwrap();
        write_plan(dir.path(), "tests/a.rs::one", "my own steps\n").unwrap();

        let made = create_boilerplates(
            dir.path(),
            &ids(&["tests/a.rs::one", "tests/a.rs::two", "tests/b.rs"]),
        )
        .unwrap();

        assert_eq!(made.kept, 1);
        assert_eq!(
            made.created,
            [
                dir.path().join("tests/case_plans/a.two.md"),
                dir.path().join("tests/case_plans/b.rs.md"),
            ]
        );
        assert_eq!(
            read_plan(dir.path(), "tests/a.rs::one").unwrap(),
            "my own steps\n",
            "an existing plan is never replaced"
        );
        assert_eq!(
            read_plan(dir.path(), "tests/a.rs::two").unwrap(),
            boilerplate("tests/a.rs::two")
        );
    }

    // ------------------------------------------------------------------
    // Classifying and summarising
    // ------------------------------------------------------------------

    #[test]
    fn a_plan_is_customized_boilerplate_or_unfinished() {
        let template = boilerplate("tests/x.rs::t");
        let customized = template.replace("Perform Action 1", "Open the page");
        // The title and a dash, but no table.
        let unfinished = "``x.rs::t``\n---\nTODO\n";

        assert_eq!(classify(&template), PlanKind::Boilerplate);
        assert_eq!(classify(&customized), PlanKind::Customized);
        assert_eq!(classify(unfinished), PlanKind::Unfinished);
        assert_eq!(classify(""), PlanKind::Unfinished);
    }

    #[test]
    fn a_crlf_plan_is_classified_like_an_lf_one() {
        let template = boilerplate("tests/x.rs::t").replace('\n', "\r\n");

        assert_eq!(classify(&template), PlanKind::Boilerplate);
    }

    #[test]
    fn no_plans_means_no_summary() {
        assert_eq!(summarize(&[]), None);
    }

    #[test]
    fn the_summary_counts_each_kind_and_folds_each_plan_into_a_details_block() {
        let template = boilerplate("tests/a.rs::one");
        let customized = "``a.rs::two``\n---\n| # | Step | Result |\n| - | ---- | ------ |\n\
                          | 1 | Open | Page   |\n";
        let unfinished = "TODO\n".to_owned();

        let summary = summarize(&[
            ("tests/a.rs::one".to_owned(), template),
            ("tests/a.rs::two".to_owned(), customized.to_owned()),
            ("tests/a.rs::three".to_owned(), unfinished),
        ])
        .unwrap();

        assert_eq!(
            (summary.customized, summary.boilerplate, summary.unfinished),
            (1, 1, 1)
        );
        assert_eq!(
            summary.markdown,
            "<h2>Summary of existing Case Plans</h2>\n\
             \n\
             |   |    |   |\n\
             | - | -: | - |\n\
             | \u{1F535} | 1 | Case Plan with a customized table |\n\
             | \u{2B55} | 1 | Case Plan using boilerplate code |\n\
             | \u{1F6A7} | 1 | Case Plan that is missing a table |\n\
             \n\
             --------\n\
             \n\
             <h3>\u{1F50E} (Click rows to expand) \u{1F50D}</h3>\n\
             \n\
             <details>\n\
             <summary> \u{2B55} <code><b>tests/a.rs::one</b></code></summary>\n\
             \n\
             | # | Step Description | Expected Result |\n\
             | - | ---------------- | --------------- |\n\
             | 1 | Perform Action 1 | Verify Action 1 |\n\
             | 2 | Perform Action 2 | Verify Action 2 |\n\
             \n\
             </details>\n\
             \n\
             <details>\n\
             <summary> \u{1F535} <code><b>tests/a.rs::two</b></code></summary>\n\
             \n\
             | # | Step | Result |\n\
             | - | ---- | ------ |\n\
             | 1 | Open | Page   |\n\
             \n\
             </details>\n\
             \n\
             \u{1F6A7} <code><b>tests/a.rs::three</b></code>\n"
        );
    }

    #[test]
    fn plural_counts_use_the_plural_wording() {
        let template = boilerplate("tests/a.rs::one");

        let summary = summarize(&[
            ("tests/a.rs::one".to_owned(), template.clone()),
            ("tests/a.rs::two".to_owned(), template),
        ])
        .unwrap();

        assert!(summary
            .markdown
            .contains("| \u{2B55} | 2 | Case Plans using boilerplate code |"));
        assert!(summary
            .markdown
            .contains("| \u{1F535} | 0 | Case Plans with customized tables |"));
        assert!(summary
            .markdown
            .contains("| \u{1F6A7} | 0 | Case Plans that are missing tables |"));
    }

    #[test]
    fn a_plan_without_its_title_line_gets_one() {
        let body = "| # | Step | Result |\n| - | ---- | ------ |\n| 1 | Open | Page   |\n";

        let summary = summarize(&[("tests/a.rs::one".to_owned(), body.to_owned())]).unwrap();

        assert_eq!(summary.customized, 1);
        // The title the plan lacked is folded into the summary line, and a
        // blank line separates it from the table.
        assert!(
            summary.markdown.contains(
                "<details>\n\
             <summary> \u{1F535} <code><b>tests/a.rs::one</b></code></summary>\n\
             \n\
             | # | Step | Result |\n"
            ),
            "{}",
            summary.markdown
        );
    }

    #[test]
    fn the_summary_file_covers_only_tests_with_a_plan_and_is_written_in_the_base() {
        let dir = tempfile::tempdir().unwrap();
        let all = ids(&["tests/a.rs::one", "tests/a.rs::two"]);
        assert_eq!(write_summary(dir.path(), &all).unwrap(), None);
        assert!(!dir.path().join(SUMMARY_FILE).exists());

        create_boilerplates(dir.path(), &all[..1]).unwrap();
        let (path, summary) = write_summary(dir.path(), &all).unwrap().unwrap();

        assert_eq!(path, dir.path().join(SUMMARY_FILE));
        assert_eq!(summary.boilerplate, 1);
        assert_eq!(std::fs::read_to_string(path).unwrap(), summary.markdown);
        assert!(!summary.markdown.contains("tests/a.rs::two"));
    }
}
