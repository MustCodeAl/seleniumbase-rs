//! The feature files `sbrs behave-gui` lists and runs: finding them, telling
//! each scenario by its line, and running chosen scenarios with `cargo test`.
//!
//! The scenarios are bound to Rust tests with rstest-bdd (feature `bdd`, see
//! [`crate::testing::bdd`]), and the steps are Rust code in the user's own
//! program, which `sbrs` cannot load. So the app asks cargo to run the tests
//! instead: one `cargo test` for the chosen scenarios of a feature, with a name
//! filter for each ([`scenario_filter`]: the scenario's title in snake case, as
//! `scenarios!` names its tests). A scenario no test is named after is
//! reported as [`Verdict::Undefined`], as `behave` does for a step nobody
//! defined, not as a failure.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The folder `behave` looks in when none is given.
pub const DEFAULT_FOLDER: &str = "features";

/// One scenario (or outline) of a feature file, and where it sits in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioInfo {
    /// The text after `Scenario:` (or `Scenario Outline:`).
    pub name: String,
    /// The line of the keyword, counted from 1.
    pub line: usize,
    /// Whether it is a scenario outline.
    pub outline: bool,
    /// The tags written directly above its keyword, without the `@`.
    pub tags: Vec<String>,
}

/// A feature file read from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFile {
    /// The path as shown, with `/` between folders.
    pub path: String,
    /// The text after `Feature:`; empty if the file has none.
    pub name: String,
    /// The tags written above `Feature:`, without the `@`.
    pub tags: Vec<String>,
    /// The scenarios, in file order.
    pub scenarios: Vec<ScenarioInfo>,
}

/// The `*.feature` files under `target`, sorted by path.
///
/// `target` is a folder, searched down to its subfolders, or one file. A
/// folder whose name starts with a dot, and `target` and `node_modules`, are
/// not entered. A target that is not there has no files.
///
/// # Errors
///
/// Returns the error of reading a folder.
pub fn find_feature_files(target: &Path) -> io::Result<Vec<PathBuf>> {
    if target.is_file() {
        return Ok(vec![target.to_path_buf()]);
    }
    let mut found = Vec::new();
    if target.is_dir() {
        walk(target, &mut found)?;
    }
    found.sort();
    Ok(found)
}

/// Adds the `*.feature` files below `dir` to `found`. Links to folders are not
/// followed, so a link back up the tree cannot make the search go round.
fn walk(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if kind.is_dir() {
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                walk(&path, found)?;
            }
        } else if (kind.is_file() || path.is_file())
            && path
                .extension()
                .is_some_and(|extension| extension == "feature")
        {
            found.push(path);
        }
    }
    Ok(())
}

/// Reads and splits the feature file at `path`; `shown` is the path to show.
///
/// # Errors
///
/// Returns the error of reading the file.
pub fn load_feature(path: &Path, shown: &str) -> io::Result<FeatureFile> {
    let text = String::from_utf8_lossy(&std::fs::read(path)?).into_owned();
    Ok(split_feature(shown, &text))
}

/// Splits the lines of a feature file into a [`FeatureFile`] called `shown`.
#[must_use]
pub fn split_feature(shown: &str, text: &str) -> FeatureFile {
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let (name, tags) = feature_header(&lines);
    FeatureFile {
        path: shown.to_owned(),
        name,
        tags,
        scenarios: scenario_blocks(&lines),
    }
}

/// The tags in `lines`, which are tag lines, without the `@`.
fn tags_of<'a>(lines: impl Iterator<Item = &'a String>) -> Vec<String> {
    lines
        .flat_map(|line| line.split_whitespace())
        .filter_map(|word| word.strip_prefix('@'))
        .map(str::to_owned)
        .collect()
}

/// The name of the feature and the tags above it.
fn feature_header(lines: &[String]) -> (String, Vec<String>) {
    let found = lines.iter().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        trimmed
            .strip_prefix("Feature:")
            .map(|name| (index, name.trim().to_owned()))
    });
    match found {
        Some((index, name)) => {
            let first = (0..index)
                .rev()
                .take_while(|above| lines[*above].trim().starts_with('@'))
                .last()
                .unwrap_or(index);
            (name, tags_of(lines[first..index].iter()))
        }
        None => (String::new(), Vec::new()),
    }
}

/// The scenario keyword of a trimmed line: the name after it, and whether it
/// is an outline.
fn scenario_header(trimmed: &str) -> Option<(String, bool)> {
    [
        ("Scenario Outline:", true),
        ("Scenario Template:", true),
        ("Scenario:", false),
    ]
    .into_iter()
    .find_map(|(keyword, outline)| {
        trimmed
            .strip_prefix(keyword)
            .map(|name| (name.trim().to_owned(), outline))
    })
}

/// Finds the scenarios in `lines`. Lines inside a doc string are not looked
/// at. A scenario's tags are the ones directly above its keyword.
fn scenario_blocks(lines: &[String]) -> Vec<ScenarioInfo> {
    let mut blocks: Vec<ScenarioInfo> = Vec::new();
    let mut fence: Option<&str> = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if let Some(open) = fence {
            if trimmed.starts_with(open) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("\"\"\"") {
            fence = Some("\"\"\"");
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some("```");
            continue;
        }
        let Some((name, outline)) = scenario_header(trimmed) else {
            continue;
        };
        let first = (0..index)
            .rev()
            .take_while(|above| lines[*above].trim().starts_with('@'))
            .last()
            .unwrap_or(index);
        blocks.push(ScenarioInfo {
            name,
            line: index + 1,
            outline,
            tags: tags_of(lines[first..index].iter()),
        });
    }
    blocks
}

/// How a scenario ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Every test of it passed.
    Passed,
    /// A test of it failed.
    Failed,
    /// No test is named after it, or cargo did not run the tests.
    Undefined,
    /// Nothing ran: the tags left it out, or its test is `#[ignore]`d.
    Skipped,
}

/// The result of one scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioRun {
    /// The feature file, as shown.
    pub file: String,
    /// The line of the scenario in it.
    pub line: usize,
    /// The scenario's name.
    pub name: String,
    /// How it ended.
    pub verdict: Verdict,
    /// Why it did not pass; empty for a pass.
    pub messages: Vec<String>,
}

/// How many scenarios ended each way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Totals {
    /// Passed.
    pub passed: usize,
    /// Failed.
    pub failed: usize,
    /// Undefined.
    pub undefined: usize,
    /// Skipped.
    pub skipped: usize,
}

/// Counts the verdicts of `runs`.
#[must_use]
pub fn totals(runs: &[ScenarioRun]) -> Totals {
    let mut totals = Totals::default();
    for run in runs {
        match run.verdict {
            Verdict::Passed => totals.passed += 1,
            Verdict::Failed => totals.failed += 1,
            Verdict::Undefined => totals.undefined += 1,
            Verdict::Skipped => totals.skipped += 1,
        }
    }
    totals
}

/// Which scenarios run, by tag: the ones with at least one of the included
/// tags (all when there are none) and none of the excluded ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunFilter {
    /// Tags, without the `@`, of which a scenario needs one.
    pub include_tags: Vec<String>,
    /// Tags, without the `@`, that leave a scenario out.
    pub exclude_tags: Vec<String>,
}

impl RunFilter {
    /// The filter that lets every scenario through.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// Whether a scenario with `tags` (its own and its feature's) runs.
    #[must_use]
    pub fn allows(&self, tags: &[String]) -> bool {
        let has = |wanted: &String| tags.contains(wanted);
        (self.include_tags.is_empty() || self.include_tags.iter().any(has))
            && !self.exclude_tags.iter().any(has)
    }
}

/// Reads tags as `behave` takes them on the command line: `@smoke,@web` runs
/// scenarios with either, and `-@slow` or `~@slow` leaves out those with it.
/// Words are separated by spaces or commas; the `@` may be left off.
#[must_use]
pub fn parse_tags(input: &str) -> RunFilter {
    let mut filter = RunFilter::all();
    let words = input
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|word| !word.is_empty());
    for word in words {
        let excluded = word.strip_prefix('-').or_else(|| word.strip_prefix('~'));
        let tag = excluded.unwrap_or(word).trim_start_matches('@');
        if tag.is_empty() {
            continue;
        }
        if excluded.is_some() {
            filter.exclude_tags.push(tag.to_owned());
        } else {
            filter.include_tags.push(tag.to_owned());
        }
    }
    filter
}

/// The name filter of `cargo test` for the scenario called `title`: the title
/// in snake case, every run of characters that are not ASCII letters or digits
/// as one `_`. `Valid login` is `valid_login`; this is how `scenarios!`
/// names the tests it binds.
#[must_use]
pub fn scenario_filter(title: &str) -> String {
    let mut name = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            name.push(c.to_ascii_lowercase());
        } else if !name.ends_with('_') {
            name.push('_');
        }
    }
    name.trim_matches('_').to_owned()
}

/// The arguments of the `cargo test` that runs the scenarios `filters` (see
/// [`scenario_filter`]): one test at a time, so a browser prompt or a shared
/// resource is not fought over.
#[must_use]
pub fn cargo_test_args(filters: &[String]) -> Vec<String> {
    let mut args = vec!["test".to_owned()];
    args.extend(filters.iter().cloned());
    args.extend(["--".to_owned(), "--test-threads=1".to_owned()]);
    args
}

/// How a test of the libtest output ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestEnd {
    Ok,
    Failed,
    Ignored,
}

/// What `cargo test` printed, read: each test with how it ended, and the
/// output of the failed ones.
#[derive(Debug, Default, PartialEq, Eq)]
struct TestReport {
    tests: Vec<(String, TestEnd)>,
    failures: HashMap<String, String>,
}

/// Reads the output of `cargo test`: the `test <name> ... ok|FAILED|ignored`
/// lines, and the `---- <name> stdout ----` sections that follow `failures:`.
fn parse_libtest(output: &str) -> TestReport {
    let mut report = TestReport::default();
    let mut section: Option<String> = None;
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("test ") {
            if let Some((name, result)) = rest.split_once(" ... ") {
                let end = if result.starts_with("ok") {
                    Some(TestEnd::Ok)
                } else if result.starts_with("FAILED") {
                    Some(TestEnd::Failed)
                } else if result.starts_with("ignored") {
                    Some(TestEnd::Ignored)
                } else {
                    None
                };
                if let Some(end) = end {
                    report.tests.push((name.trim().to_owned(), end));
                }
                continue;
            }
        }
        if let Some(name) = line
            .strip_prefix("---- ")
            .and_then(|rest| rest.strip_suffix(" stdout ----"))
        {
            section = Some(name.to_owned());
        } else if line == "failures:" {
            section = None;
        } else if let Some(name) = &section {
            let text = report.failures.entry(name.clone()).or_default();
            text.push_str(line);
            text.push('\n');
        }
    }
    report
}

/// Whether the test `name` (a path such as `login::valid_login::case_1`) is
/// named after the scenario filter: some segment of it is the filter, or ends
/// with `_` and the filter (`login_valid_login`, as `scenarios!` writes it).
fn named_after(name: &str, filter: &str) -> bool {
    let tail = format!("_{filter}");
    name.split("::")
        .any(|segment| segment == filter || segment.ends_with(&tail))
}

/// The result of a scenario, from what `cargo test` printed.
fn verdict_of(filter: &str, report: &TestReport, ran: &str) -> (Verdict, Vec<String>) {
    let tests: Vec<&(String, TestEnd)> = report
        .tests
        .iter()
        .filter(|(name, _)| named_after(name, filter))
        .collect();
    if tests.is_empty() {
        let why = if report.tests.is_empty() {
            format!("cargo test ran no test: {ran}")
        } else {
            format!(
                "no test is named after `{filter}`: bind the scenario with #[scenario], \
                 or name the test after it"
            )
        };
        return (Verdict::Undefined, vec![why]);
    }
    let failed: Vec<String> = tests
        .iter()
        .filter(|(_, end)| *end == TestEnd::Failed)
        .map(|(name, _)| {
            let output = report.failures.get(name).map_or("", |text| text.trim());
            format!("{name}: {output}")
        })
        .collect();
    if !failed.is_empty() {
        return (Verdict::Failed, failed);
    }
    if tests.iter().all(|(_, end)| *end == TestEnd::Ignored) {
        return (Verdict::Skipped, vec!["the test is #[ignore]d".to_owned()]);
    }
    (Verdict::Passed, Vec::new())
}

/// The last lines of `output`, to say why cargo ran nothing.
fn tail(output: &str) -> String {
    let lines: Vec<&str> = output.lines().rev().take(8).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

/// Runs the scenarios `chosen` of `feature` with `cargo test` (one process for
/// all of them) in the current folder, leaving out the ones `filter` does not
/// allow, and says how each ended.
#[must_use]
pub fn run_scenarios(
    feature: &FeatureFile,
    chosen: &[usize],
    filter: &RunFilter,
) -> Vec<ScenarioRun> {
    let mut picked: Vec<usize> = chosen
        .iter()
        .copied()
        .filter(|index| *index < feature.scenarios.len())
        .collect();
    picked.sort_unstable();
    picked.dedup();
    let allowed = |index: &usize| {
        let scenario = &feature.scenarios[*index];
        let mut tags = feature.tags.clone();
        tags.extend(scenario.tags.iter().cloned());
        filter.allows(&tags)
    };
    let to_run: Vec<usize> = picked.iter().copied().filter(allowed).collect();
    let filters: Vec<String> = to_run
        .iter()
        .map(|index| scenario_filter(&feature.scenarios[*index].name))
        .collect();
    let (report, ran) = if filters.is_empty() {
        (TestReport::default(), String::new())
    } else {
        match Command::new("cargo")
            .args(cargo_test_args(&filters))
            .output()
        {
            Ok(output) => {
                let text = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                (parse_libtest(&text), tail(&text))
            }
            Err(error) => (
                TestReport::default(),
                format!("cargo could not start: {error}"),
            ),
        }
    };
    picked
        .iter()
        .map(|index| {
            let scenario = &feature.scenarios[*index];
            let (verdict, messages) = if to_run.contains(index) {
                verdict_of(&scenario_filter(&scenario.name), &report, &ran)
            } else {
                (
                    Verdict::Skipped,
                    vec!["nothing ran: the tags left it out".to_owned()],
                )
            };
            ScenarioRun {
                file: feature.path.clone(),
                line: scenario.line,
                name: scenario.name.clone(),
                verdict,
                messages,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOGIN: &str = "\
@web
Feature: Log in
  As a user
  I want to log in

  Background:
    Given the system is ready

  @smoke
  Scenario: valid login
    Given I set the value to \"5\"
    Then the value should be \"5\"

  Scenario: wrong value
    Given I set the value to \"5\"
    Then the value should be \"6\"

  @slow @wip
  Scenario Outline: wait <n>
    When I wait <n> seconds
    Examples:
      | n |
      | 1 |

  Scenario: undefined step
    Given a step nobody defined
";

    fn login() -> FeatureFile {
        split_feature("features/login.feature", LOGIN)
    }

    // ------------------------------------------------------------------
    // Splitting a file
    // ------------------------------------------------------------------

    #[test]
    fn the_scenarios_are_found_with_their_names_lines_and_tags() {
        let feature = login();

        assert_eq!(feature.path, "features/login.feature");
        assert_eq!(feature.name, "Log in");
        assert_eq!(feature.tags, ["web"]);
        let found: Vec<_> = feature
            .scenarios
            .iter()
            .map(|s| (s.name.as_str(), s.line, s.outline, s.tags.clone()))
            .collect();
        assert_eq!(
            found,
            [
                ("valid login", 10, false, vec!["smoke".to_owned()]),
                ("wrong value", 14, false, Vec::new()),
                (
                    "wait <n>",
                    19,
                    true,
                    vec!["slow".to_owned(), "wip".to_owned()]
                ),
                ("undefined step", 25, false, Vec::new()),
            ]
        );
    }

    #[test]
    fn the_scenarios_are_the_ones_the_outline_reader_finds() {
        use crate::cli::scripts::gherkin_outline::parse_feature;
        let lines: Vec<String> = LOGIN.lines().map(String::from).collect();
        let parsed = parse_feature(&lines);

        let ours: Vec<_> = login().scenarios.iter().map(|s| s.name.clone()).collect();
        let theirs: Vec<_> = parsed.scenarios.iter().map(|s| s.name.clone()).collect();
        assert_eq!(ours, theirs);
    }

    #[test]
    fn a_keyword_inside_a_doc_string_is_not_a_scenario() {
        let text = "Feature: f\n  Scenario: real\n    Given a doc\n      \"\"\"\n      Scenario: fake\n      \"\"\"\n";

        let feature = split_feature("f.feature", text);

        assert_eq!(feature.scenarios.len(), 1);
        assert_eq!(feature.scenarios[0].name, "real");
    }

    #[test]
    fn a_file_with_no_scenarios_or_no_feature_line_is_empty_not_an_error() {
        let feature = split_feature("f.feature", "Feature: empty\n");
        assert_eq!(feature.name, "empty");
        assert!(feature.scenarios.is_empty() && feature.tags.is_empty());
        assert_eq!(split_feature("g.feature", "").name, "");
    }

    // ------------------------------------------------------------------
    // Finding files
    // ------------------------------------------------------------------

    fn touch(dir: &Path, name: &str) {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "Feature: f\n").unwrap();
    }

    #[test]
    fn a_folder_is_searched_below_and_sorted_without_hidden_or_build_folders() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "features/b.feature",
            "features/a.feature",
            "features/sub/c.feature",
            "features/notes.md",
            "features/.hidden/d.feature",
            "features/target/e.feature",
            "features/node_modules/f.feature",
        ] {
            touch(dir.path(), name);
        }

        let found = find_feature_files(&dir.path().join("features")).unwrap();

        // Joined with `/` whatever the platform's separator, so the names read
        // the same on Windows.
        let shown: Vec<String> = found
            .iter()
            .map(|p| {
                p.strip_prefix(dir.path())
                    .unwrap()
                    .components()
                    .map(|part| part.as_os_str().to_str().unwrap())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();
        assert_eq!(
            shown,
            [
                "features/a.feature",
                "features/b.feature",
                "features/sub/c.feature"
            ]
        );
    }

    #[test]
    fn one_file_can_be_asked_for_and_a_missing_target_has_none() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "x.feature");

        assert_eq!(
            find_feature_files(&dir.path().join("x.feature")).unwrap(),
            [dir.path().join("x.feature")]
        );
        assert!(find_feature_files(&dir.path().join("nowhere"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_file_is_loaded_with_the_path_to_show() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.feature"), LOGIN).unwrap();

        let feature = load_feature(&dir.path().join("x.feature"), "features/x.feature").unwrap();

        assert_eq!(feature.path, "features/x.feature");
        assert_eq!(feature.scenarios.len(), 4);
        assert_eq!(
            load_feature(&dir.path().join("none.feature"), "n")
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
    }

    // ------------------------------------------------------------------
    // Running
    // ------------------------------------------------------------------

    #[test]
    fn a_title_becomes_the_name_filter_of_its_test() {
        for (title, filter) in [
            ("Valid login", "valid_login"),
            ("wait <n>", "wait_n"),
            ("  It's  \"quoted\" -- twice!  ", "it_s_quoted_twice"),
            ("Ünïcode ok", "n_code_ok"),
        ] {
            assert_eq!(scenario_filter(title), filter, "{title}");
        }
    }

    #[test]
    fn the_cargo_command_has_one_filter_per_scenario_and_one_test_thread() {
        let filters = ["valid_login".to_owned(), "wrong_value".to_owned()];

        assert_eq!(
            cargo_test_args(&filters),
            [
                "test",
                "valid_login",
                "wrong_value",
                "--",
                "--test-threads=1"
            ]
        );
    }

    const OUTPUT: &str = "\
running 5 tests
test login::valid_login ... ok
test login::wrong_value ... FAILED
test login::wait_n::case_1 ... ok
test login::wait_n::case_2 ... ok
test login::slow_one ... ignored, takes a minute

failures:

---- login::wrong_value stdout ----
thread 'login::wrong_value' panicked at tests/login.rs:9:5:
the value is \"5\", expected \"6\"

failures:
    login::wrong_value

test result: FAILED. 3 passed; 1 failed; 1 ignored
";

    #[test]
    fn the_output_of_cargo_test_is_read_test_by_test() {
        let report = parse_libtest(OUTPUT);

        assert_eq!(report.tests.len(), 5);
        assert_eq!(
            report.tests[1],
            ("login::wrong_value".to_owned(), TestEnd::Failed)
        );
        assert!(
            report.failures["login::wrong_value"].contains("expected \"6\""),
            "{:?}",
            report.failures
        );
    }

    #[test]
    fn each_scenario_ends_by_the_tests_named_after_it() {
        let report = parse_libtest(OUTPUT);
        let verdict = |filter: &str| verdict_of(filter, &report, "").0;

        assert_eq!(verdict("valid_login"), Verdict::Passed);
        assert_eq!(verdict("wrong_value"), Verdict::Failed);
        assert_eq!(
            verdict("wait_n"),
            Verdict::Passed,
            "every row of an outline"
        );
        assert_eq!(verdict("slow_one"), Verdict::Skipped);
        assert_eq!(verdict("nobody"), Verdict::Undefined);
        let (_, messages) = verdict_of("wrong_value", &report, "");
        assert!(messages[0].contains("expected \"6\""), "{messages:?}");
    }

    #[test]
    fn a_name_has_to_end_with_the_filter_not_just_contain_it() {
        assert!(named_after("login::valid_login", "valid_login"));
        assert!(named_after("login_valid_login", "valid_login"));
        assert!(!named_after("login::invalid_login", "valid_login"));
        assert!(!named_after("login::valid_login_twice", "valid_login"));
    }

    #[test]
    fn a_build_that_failed_says_so_instead_of_blaming_the_scenario() {
        let (verdict, messages) = verdict_of(
            "valid_login",
            &TestReport::default(),
            "error: could not compile `shop`",
        );

        assert_eq!(verdict, Verdict::Undefined);
        assert!(messages[0].contains("could not compile"), "{messages:?}");
    }

    #[test]
    fn tags_leave_scenarios_out_without_starting_cargo() {
        let feature = login();
        let only_smoke = parse_tags("@smoke");

        // None of the chosen scenarios has @smoke, so nothing is started.
        let runs = run_scenarios(&feature, &[1, 2, 3], &only_smoke);

        assert_eq!(runs.len(), 3);
        assert!(runs.iter().all(|run| run.verdict == Verdict::Skipped));
        assert!(
            runs[0].messages[0].contains("tags"),
            "{:?}",
            runs[0].messages
        );
        assert_eq!(
            (runs[0].file.as_str(), runs[0].line),
            ("features/login.feature", 14)
        );
    }

    #[test]
    fn the_feature_tags_count_for_its_scenarios() {
        let feature = login();

        // @web is the feature's: `-@web` leaves every scenario out.
        let runs = run_scenarios(&feature, &[0, 1, 2, 3], &parse_tags("-@web"));

        assert!(runs.iter().all(|run| run.verdict == Verdict::Skipped));
    }

    #[test]
    fn the_totals_count_each_verdict() {
        let run = |verdict| ScenarioRun {
            file: "f".to_owned(),
            line: 1,
            name: "s".to_owned(),
            verdict,
            messages: Vec::new(),
        };
        let runs = [
            run(Verdict::Passed),
            run(Verdict::Failed),
            run(Verdict::Undefined),
            run(Verdict::Skipped),
        ];

        assert_eq!(
            totals(&runs),
            Totals {
                passed: 1,
                failed: 1,
                undefined: 1,
                skipped: 1
            }
        );
        assert_eq!(totals(&[]), Totals::default());
    }

    #[test]
    fn tags_are_read_the_way_behave_takes_them() {
        let filter = parse_tags("@smoke, web  -@slow ~@wip");

        assert_eq!(filter.include_tags, ["smoke", "web"]);
        assert_eq!(filter.exclude_tags, ["slow", "wip"]);
        let none = parse_tags("  ");
        assert!(none.include_tags.is_empty() && none.exclude_tags.is_empty());
        assert!(filter.allows(&["web".to_owned()]));
        assert!(!filter.allows(&["web".to_owned(), "slow".to_owned()]));
        assert!(!filter.allows(&["other".to_owned()]));
        assert!(RunFilter::all().allows(&[]));
    }
}
