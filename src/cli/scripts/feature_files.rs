//! The feature files `sbrs behave-gui` lists and runs: finding them, telling
//! each scenario by its line, and running chosen scenarios through the crate's
//! behave runner ([`crate::behave`]).
//!
//! The runner needs step definitions, and those are Rust code in the user's own
//! program, which `sbrs` cannot load. The app therefore runs with the built-in
//! steps ([`builtin_steps`]); a scenario that uses a step nobody defined is
//! reported as [`Verdict::Undefined`], as `behave` does, not as a failure.

use std::io;
use std::path::{Path, PathBuf};

use crate::behave::steps::step_pattern;
use crate::behave::{
    parse_feature, run_feature_text, CommonSteps, RunFilter, ScenarioResult, StepRegistry,
};

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
    /// The first line of the scenario's text, counted from 0: the tags above
    /// its keyword are part of it.
    start: usize,
    /// One past the last line of the scenario's text, counted from 0.
    end: usize,
}

/// A feature file read from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFile {
    /// The path as shown, with `/` between folders.
    pub path: String,
    /// The text after `Feature:`; empty if the file has none.
    pub name: String,
    /// The scenarios, in file order.
    pub scenarios: Vec<ScenarioInfo>,
    lines: Vec<String>,
    /// One past the last line before the first scenario: the feature line, its
    /// tags and the background.
    header_end: usize,
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
    let name = parse_feature(&lines).name;
    let scenarios = scenario_blocks(&lines);
    let header_end = scenarios.first().map_or(lines.len(), |first| first.start);
    FeatureFile {
        path: shown.to_owned(),
        name,
        scenarios,
        lines,
        header_end,
    }
}

/// The scenario keyword of a trimmed line, as the behave parser reads it: the
/// name after it, and whether it is an outline.
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

/// Finds the scenarios in `lines` and the stretch of lines each one owns.
///
/// Lines inside a doc string are not looked at, as in the behave parser. A
/// scenario owns the tags directly above its keyword and everything up to the
/// tags of the next one.
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
        let mut start = index;
        while start > 0 && lines[start - 1].trim().starts_with('@') {
            start -= 1;
        }
        if let Some(previous) = blocks.last_mut() {
            previous.end = start;
        }
        blocks.push(ScenarioInfo {
            name,
            line: index + 1,
            outline,
            start,
            end: lines.len(),
        });
    }
    blocks
}

impl FeatureFile {
    /// The text of the feature with only the scenarios `chosen` (indexes into
    /// [`scenarios`](Self::scenarios)): the feature line, its tags and the
    /// background, then those scenarios as they are written, in file order.
    #[must_use]
    pub fn text_of(&self, chosen: &[usize]) -> String {
        let mut picked: Vec<usize> = chosen
            .iter()
            .copied()
            .filter(|index| *index < self.scenarios.len())
            .collect();
        picked.sort_unstable();
        picked.dedup();
        let mut text = String::new();
        let mut push = |lines: &[String]| {
            for line in lines {
                text.push_str(line);
                text.push('\n');
            }
        };
        push(&self.lines[..self.header_end]);
        for index in picked {
            let scenario = &self.scenarios[index];
            push(&self.lines[scenario.start..scenario.end]);
        }
        text
    }
}

/// How a scenario ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Every step ran and passed.
    Passed,
    /// A step failed.
    Failed,
    /// A step has no definition, so the scenario could not run.
    Undefined,
    /// Nothing ran: the tags left it out, or it is an outline with no examples.
    Skipped,
}

/// The result of one scenario, or one row of an outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioRun {
    /// The feature file, as shown.
    pub file: String,
    /// The line of the scenario in it.
    pub line: usize,
    /// The scenario's name; an outline row has `(example N)` after it.
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

/// The steps the app can run: the placeholders of
/// [`CommonSteps`](crate::behave::CommonSteps) (`I do nothing`, `the system is
/// ready`, `I wait N seconds`) and the value steps that really compare.
#[must_use]
pub fn builtin_steps() -> StepRegistry {
    let mut registry = StepRegistry::new();
    CommonSteps::register_noop(&mut registry, &step_pattern("I do nothing"));
    CommonSteps::register_noop(&mut registry, &step_pattern("the system is ready"));
    CommonSteps::register_delay_steps(&mut registry);
    // The value steps, not the placeholders that pass whatever they are given.
    let _value = CommonSteps::register_value_steps(&mut registry);
    registry
}

/// Runs the scenarios `chosen` of `feature`, each with fresh built-in steps so
/// that one scenario's value does not leak into the next, and the tags in
/// `filter`.
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
    let mut runs = Vec::new();
    for index in picked {
        let scenario = &feature.scenarios[index];
        let results = run_feature_text(&feature.text_of(&[index]), &builtin_steps(), filter);
        let run = |name: String, verdict, messages| ScenarioRun {
            file: feature.path.clone(),
            line: scenario.line,
            name,
            verdict,
            messages,
        };
        if results.is_empty() {
            runs.push(run(
                scenario.name.clone(),
                Verdict::Skipped,
                vec![
                    "nothing ran: the tags left it out, or it is an outline with no examples"
                        .to_owned(),
                ],
            ));
        }
        for result in results {
            let verdict = verdict_of(&result);
            runs.push(run(result.name, verdict, result.messages));
        }
    }
    runs
}

/// How a result of the runner ended. The runner says a step has no handler in
/// the message of the failure; that is an undefined step, not a failing one.
fn verdict_of(result: &ScenarioResult) -> Verdict {
    if result.passed {
        Verdict::Passed
    } else if result
        .messages
        .first()
        .is_some_and(|message| message.starts_with("no handler registered for step"))
    {
        Verdict::Undefined
    } else {
        Verdict::Failed
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

  @slow
  Scenario Outline: wait <n>
    When I wait <n> seconds
    Examples:
      | n |
      | 1 |
      | 2 |

  Scenario: undefined step
    Given a step nobody defined
";

    fn login() -> FeatureFile {
        split_feature("features/login.feature", LOGIN)
    }

    fn names(runs: &[ScenarioRun]) -> Vec<(&str, Verdict)> {
        runs.iter()
            .map(|run| (run.name.as_str(), run.verdict))
            .collect()
    }

    // ------------------------------------------------------------------
    // Splitting a file
    // ------------------------------------------------------------------

    #[test]
    fn the_scenarios_are_found_with_their_names_and_lines() {
        let feature = login();

        assert_eq!(feature.path, "features/login.feature");
        assert_eq!(feature.name, "Log in");
        let found: Vec<_> = feature
            .scenarios
            .iter()
            .map(|s| (s.name.as_str(), s.line, s.outline))
            .collect();
        assert_eq!(
            found,
            [
                ("valid login", 10, false),
                ("wrong value", 14, false),
                ("wait <n>", 19, true),
                ("undefined step", 26, false),
            ]
        );
    }

    #[test]
    fn the_scenarios_are_the_ones_the_behave_parser_finds() {
        let feature = login();
        let lines: Vec<String> = LOGIN.lines().map(String::from).collect();
        let parsed = crate::behave::parse_feature(&lines);

        let ours: Vec<_> = feature.scenarios.iter().map(|s| s.name.clone()).collect();
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
    fn a_file_with_no_scenarios_has_only_a_header() {
        let feature = split_feature("f.feature", "Feature: empty\n");

        assert_eq!(feature.name, "empty");
        assert!(feature.scenarios.is_empty());
        assert_eq!(feature.text_of(&[]), "Feature: empty\n");
    }

    #[test]
    fn the_text_of_chosen_scenarios_keeps_the_header_the_tags_and_the_order() {
        let feature = login();

        let text = feature.text_of(&[2, 0]);

        assert_eq!(
            text,
            "\
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

  @slow
  Scenario Outline: wait <n>
    When I wait <n> seconds
    Examples:
      | n |
      | 1 |
      | 2 |

"
        );
    }

    #[test]
    fn a_cut_feature_text_parses_to_the_scenarios_chosen() {
        let feature = login();

        let text = feature.text_of(&[1, 3]);

        let lines: Vec<String> = text.lines().map(String::from).collect();
        let parsed = crate::behave::parse_feature(&lines);
        let names: Vec<_> = parsed.scenarios.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["wrong value", "undefined step"]);
        assert_eq!(parsed.background.len(), 1, "the background comes along");
        assert_eq!(parsed.tags, ["@web"]);
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
    fn each_chosen_scenario_ends_passed_failed_or_undefined() {
        let feature = login();

        let runs = run_scenarios(&feature, &[0, 1, 3], &RunFilter::all());

        assert_eq!(
            names(&runs),
            [
                ("valid login", Verdict::Passed),
                ("wrong value", Verdict::Failed),
                ("undefined step", Verdict::Undefined),
            ]
        );
        assert!(runs[0].messages.is_empty());
        assert!(
            runs[1].messages[0].contains("the value is \"5\", expected \"6\""),
            "{:?}",
            runs[1].messages
        );
        assert!(
            runs[2].messages[0].contains("no handler registered for step"),
            "{:?}",
            runs[2].messages
        );
    }

    #[test]
    fn a_scenario_carries_its_file_and_line() {
        let feature = login();

        let runs = run_scenarios(&feature, &[1], &RunFilter::all());

        assert_eq!(runs[0].file, "features/login.feature");
        assert_eq!(runs[0].line, 14);
    }

    #[test]
    fn an_outline_gives_one_result_per_example_row() {
        let feature = login();

        let runs = run_scenarios(&feature, &[2], &RunFilter::all());

        assert_eq!(
            names(&runs),
            [
                ("wait 1 (example 1)", Verdict::Passed),
                ("wait 2 (example 2)", Verdict::Passed)
            ]
        );
        assert!(runs.iter().all(|run| run.line == 19));
    }

    #[test]
    fn the_value_of_one_scenario_does_not_leak_into_the_next() {
        let text = "Feature: f\n\
                    Scenario: sets it\nGiven I set the value to \"5\"\n\
                    Scenario: starts empty\nThen the value should be \"\"\n";
        let feature = split_feature("f.feature", text);

        let runs = run_scenarios(&feature, &[0, 1], &RunFilter::all());

        assert_eq!(
            names(&runs),
            [
                ("sets it", Verdict::Passed),
                ("starts empty", Verdict::Passed)
            ]
        );
    }

    #[test]
    fn tags_leave_scenarios_out_and_they_are_reported_as_skipped() {
        let feature = login();
        let only_smoke = parse_tags("@smoke");

        let runs = run_scenarios(&feature, &[0, 1, 2], &only_smoke);

        // The feature's @web does not match; @smoke is on the first only.
        assert_eq!(
            names(&runs),
            [
                ("valid login", Verdict::Passed),
                ("wrong value", Verdict::Skipped),
                ("wait <n>", Verdict::Skipped),
            ]
        );
        assert!(
            runs[1].messages[0].contains("tags"),
            "{:?}",
            runs[1].messages
        );
    }

    #[test]
    fn the_totals_count_each_verdict() {
        let feature = login();
        let runs = run_scenarios(&feature, &[0, 1, 2, 3], &parse_tags("-@slow"));

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
    }

    #[test]
    fn the_built_in_steps_know_the_placeholders_and_the_value_steps() {
        let steps = builtin_steps();

        for step in [
            "Given I do nothing",
            "Given the system is ready",
            "When I wait 3 seconds",
            "Given I set the value to \"x\"",
        ] {
            assert!(steps.has_handler(step), "{step}");
        }
        assert!(!steps.has_handler("Given I open \"https://example.com\""));
    }
}
