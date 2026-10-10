//! `sbase behave-gui`: a prompt-driven writer for Gherkin feature files.

use crate::behave::Step;
use crate::cli::commands::write_new_file;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

const README: &str = "# BDD Features

This directory holds Gherkin feature files for behavior-driven tests.

Run them with the behave runner in this crate: register the steps in a
`seleniumbase_rs::behave::StepRegistry` (or an `AsyncStepRegistry` for steps
that drive a browser) and call `run_feature_file` on a file from this folder.
Steps are looked up with their keyword, for example `^Given I open \"(.+)\"$`.
";

/// Asks for a feature on the terminal and writes `features/<name>.feature`.
///
/// See [`run_gui_in`] for what is asked and checked.
pub fn run_gui() -> io::Result<PathBuf> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_gui_in(Path::new("features"), stdin.lock(), stdout.lock())
}

/// Asks for a feature, reading answers from `input` and writing the prompts to
/// `output`, and writes `<features_dir>/<name>.feature`.
///
/// - The name must be a plain file name; a path such as `../x` is refused
///   before anything else is asked.
/// - A step must start with `Given`, `When`, `Then`, `And`, `But` or `*`;
///   anything else is explained and asked for again.
/// - If the answers run out before `done`, nothing is written and the error
///   is [`io::ErrorKind::UnexpectedEof`].
/// - An existing feature file is never replaced (the error is
///   [`io::ErrorKind::AlreadyExists`]), and an existing `README.md` in the
///   folder is left as it is.
pub fn run_gui_in(
    features_dir: &Path,
    mut input: impl BufRead,
    mut output: impl Write,
) -> io::Result<PathBuf> {
    let feature_name = ask(
        &mut input,
        &mut output,
        "Feature file name (without .feature): ",
    )?;
    let feature_name = feature_file_stem(&feature_name)?;
    let feature_title = ask(&mut input, &mut output, "Feature title: ")?;
    let as_a = ask(&mut input, &mut output, "As a: ")?;
    let i_want = ask(&mut input, &mut output, "I want: ")?;
    let so_that = ask(&mut input, &mut output, "So that: ")?;

    let mut feature_content = format!(
        "Feature: {feature_title}\n  As a {as_a}\n  I want {i_want}\n  So that {so_that}\n\n"
    );

    let scenario_count = ask(&mut input, &mut output, "How many scenarios? ")?;
    let scenario_count = scenario_count.parse::<usize>().unwrap_or(1);

    for index in 1..=scenario_count {
        let scenario_name = ask(&mut input, &mut output, &format!("Scenario {index} name: "))?;
        feature_content.push_str(&format!("  Scenario: {scenario_name}\n"));

        loop {
            let step = ask(
                &mut input,
                &mut output,
                "  Add a step (Given/When/Then/And/But) or 'done': ",
            )?;
            if step.eq_ignore_ascii_case("done") {
                break;
            }
            match Step::parse(&step) {
                Some(step) => feature_content.push_str(&format!("    {}\n", step.full_text())),
                None if step.is_empty() => {}
                None => writeln!(
                    output,
                    "  Not a step: it must start with Given, When, Then, And, But or *."
                )?,
            }
        }
        feature_content.push('\n');
    }

    let feature_path = features_dir.join(format!("{feature_name}.feature"));
    write_new_file(&feature_path, &feature_content)?;
    // The README is a convenience: keep the user's own if there is one.
    match write_new_file(&features_dir.join("README.md"), README) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    Ok(feature_path)
}

/// The file name for a feature called `answer`, without the `.feature`.
fn feature_file_stem(answer: &str) -> io::Result<String> {
    let name = answer.strip_suffix(".feature").unwrap_or(answer);
    let plain = !name.is_empty()
        && !name.chars().all(|c| c == '.')
        && !name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        });
    if plain {
        Ok(name.to_owned())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{answer:?} is not a plain file name: use a name such as `login`, not a path"),
        ))
    }
}

/// Prints `message`, then returns the next line of `input`, trimmed.
fn ask(input: &mut impl BufRead, output: &mut impl Write, message: &str) -> io::Result<String> {
    write!(output, "{message}")?;
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "input ended before the feature was complete; nothing was written",
        ));
    }
    Ok(line.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behave::parse_feature;

    /// Answers for a one-scenario feature named `login`.
    const ANSWERS: &str = "login\nLog in\nuser\nto log in\nI can work\n1\nvalid login\n\
                           Given I am on the login page\nWhen I enter my details\n\
                           Then I see my account\ndone\n";

    fn run(dir: &Path, answers: &str) -> (io::Result<PathBuf>, String) {
        let mut output = Vec::new();
        let result = run_gui_in(dir, answers.as_bytes(), &mut output);
        (result, String::from_utf8(output).unwrap())
    }

    #[test]
    fn the_feature_it_writes_is_one_the_behave_parser_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let features = dir.path().join("features");

        let (path, _) = run(&features, ANSWERS);

        let path = path.unwrap();
        assert_eq!(path, features.join("login.feature"));
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<String> = text.lines().map(String::from).collect();
        let feature = parse_feature(&lines);
        assert_eq!(feature.name, "Log in");
        assert_eq!(feature.scenarios.len(), 1);
        assert_eq!(feature.scenarios[0].name, "valid login");
        let steps: Vec<String> = feature.scenarios[0]
            .steps
            .iter()
            .map(Step::full_text)
            .collect();
        assert_eq!(
            steps,
            [
                "Given I am on the login page",
                "When I enter my details",
                "Then I see my account"
            ]
        );
    }

    #[test]
    fn answers_that_run_out_end_the_prompt_instead_of_looping() {
        let dir = tempfile::tempdir().unwrap();
        // Ends in the middle of the step loop: this used to append empty
        // lines to the file for ever.
        let cut = ANSWERS.split("Then I see").next().unwrap();

        let (result, _) = run(dir.path(), cut);

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert!(
            !dir.path().join("login.feature").exists(),
            "nothing is written"
        );
    }

    #[test]
    fn a_line_that_is_not_a_step_is_explained_and_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let answers = "f\nt\na\nb\nc\n1\ns\nAndroid rocks\n\nGiven ok\ndone\n";

        let (path, out) = run(dir.path(), answers);

        assert!(out.contains("Not a step"), "{out}");
        let text = std::fs::read_to_string(path.unwrap()).unwrap();
        assert!(
            text.contains("    Given ok\n") && !text.contains("Android"),
            "{text}"
        );
        assert!(!text.contains("    \n"), "no blank step lines: {text:?}");
    }

    #[test]
    fn a_name_that_is_a_path_is_refused_before_anything_is_written() {
        let dir = tempfile::tempdir().unwrap();

        for name in ["../escape", "a/b", "a\\b", "", "..", "x:y"] {
            let (result, _) = run(dir.path(), &format!("{name}\n"));
            assert_eq!(
                result.unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{name:?}"
            );
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_trailing_dot_feature_is_not_doubled() {
        let dir = tempfile::tempdir().unwrap();

        let (path, _) = run(
            dir.path(),
            &ANSWERS.replacen("login\n", "login.feature\n", 1),
        );

        assert_eq!(path.unwrap(), dir.path().join("login.feature"));
    }

    #[test]
    fn running_it_twice_keeps_the_feature_and_the_readme_the_user_edited() {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), ANSWERS).0.unwrap();
        let feature = dir.path().join("login.feature");
        std::fs::write(&feature, "my edits").unwrap();
        std::fs::write(dir.path().join("README.md"), "my notes").unwrap();

        let (again, _) = run(dir.path(), ANSWERS);

        assert_eq!(again.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&feature).unwrap(), "my edits");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("README.md")).unwrap(),
            "my notes"
        );

        // Another feature in the same folder leaves the README alone too.
        let other = ANSWERS.replacen("login\n", "cart\n", 1);
        run(dir.path(), &other).0.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("README.md")).unwrap(),
            "my notes"
        );
    }
}
