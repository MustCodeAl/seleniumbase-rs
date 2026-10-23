//! A small reader for the outline of a Gherkin feature file: its name, its
//! scenarios and their steps. There is no runner here; features run as cargo
//! tests through rstest-bdd (see [`crate::testing::bdd`], feature `bdd`).
//!
//! The reader serves `sbrs behave-gui` (which lists scenarios), the
//! `behave-gui --new` writer and the tests that check the recorder's Gherkin.
//! Data tables and doc strings attached to a step are skipped, not stored: a
//! [`Step`] carries only its keyword and text.

/// The words that start a step. `*` is Gherkin's keyword-free step.
const STEP_KEYWORDS: [&str; 6] = ["Given", "When", "Then", "And", "But", "*"];

/// A single Gherkin step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Step {
    /// The keyword as written: `Given`, `When`, `Then`, `And`, `But` or `*`.
    pub(crate) keyword: String,
    /// The text after the keyword, trimmed.
    pub(crate) text: String,
}

impl Step {
    /// Reads a step from a line such as `Given I am logged in`.
    ///
    /// The line must start with a keyword as a whole word; anything else is
    /// not a step, so `Android is a phone` is not `And` followed by
    /// `roid is a phone`.
    pub(crate) fn parse(line: &str) -> Option<Step> {
        let trimmed = line.trim();
        for keyword in STEP_KEYWORDS {
            if let Some(rest) = trimmed.strip_prefix(keyword) {
                if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                    return Some(Step {
                        keyword: keyword.to_owned(),
                        text: rest.trim().to_owned(),
                    });
                }
            }
        }
        None
    }

    /// The step as written in the feature file, keyword included.
    pub(crate) fn full_text(&self) -> String {
        format!("{} {}", self.keyword, self.text)
    }
}

/// A scenario (or outline) of a feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct Scenario {
    /// The text after `Scenario:` (or `Scenario Outline:`).
    pub(crate) name: String,
    /// The scenario's steps, not including the feature's background.
    pub(crate) steps: Vec<Step>,
}

/// A feature file in outline.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct Feature {
    /// The text after `Feature:`.
    pub(crate) name: String,
    /// The scenarios and scenario outlines, in file order.
    pub(crate) scenarios: Vec<Scenario>,
}

/// Reads the lines of a feature file.
///
/// Besides `Scenario:` the reader accepts `Scenario Outline:` and
/// `Scenario Template:`. Lines inside a `"""` or ```` ``` ```` doc string,
/// comments and the lines of `Examples:` tables are skipped, and so are the
/// steps of the `Background:`.
#[cfg(test)]
pub(crate) fn parse_feature(lines: &[String]) -> Feature {
    let mut feature = Feature {
        name: String::new(),
        scenarios: Vec::new(),
    };
    let mut current: Option<Scenario> = None;
    let mut in_background = false;
    let mut in_examples = false;
    // The fence that opened the doc string we are inside, if any.
    let mut doc_string: Option<&str> = None;

    for line in lines {
        // A byte order mark, as Windows editors write at the start of a file,
        // is not white space; left in, it would hide the first line's keyword.
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if let Some(fence) = doc_string {
            if trimmed.starts_with(fence) {
                doc_string = None;
            }
            continue;
        }
        if trimmed.starts_with("\"\"\"") {
            doc_string = Some("\"\"\"");
            continue;
        }
        if trimmed.starts_with("```") {
            doc_string = Some("```");
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('@') {
            continue;
        }
        if let Some(name) = trimmed.strip_prefix("Feature:") {
            feature.name = name.trim().to_owned();
            continue;
        }
        if trimmed.starts_with("Background:") {
            in_background = true;
            in_examples = false;
            continue;
        }
        let header = ["Scenario Outline:", "Scenario Template:", "Scenario:"]
            .into_iter()
            .find_map(|keyword| trimmed.strip_prefix(keyword));
        if let Some(name) = header {
            feature.scenarios.extend(current.take());
            current = Some(Scenario {
                name: name.trim().to_owned(),
                steps: Vec::new(),
            });
            in_background = false;
            in_examples = false;
            continue;
        }
        if trimmed.starts_with("Examples:") || trimmed.starts_with("Scenarios:") {
            in_examples = true;
            continue;
        }
        if in_examples || in_background {
            continue;
        }
        if let (Some(step), Some(scenario)) = (Step::parse(trimmed), current.as_mut()) {
            scenario.steps.push(step);
        }
    }
    feature.scenarios.extend(current);
    feature
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn a_step_starts_with_a_whole_keyword() {
        let step = Step::parse("  When I click \"Go\"").unwrap();
        assert_eq!(
            (step.keyword.as_str(), step.text.as_str()),
            ("When", "I click \"Go\"")
        );
        assert_eq!(step.full_text(), "When I click \"Go\"");
        assert!(Step::parse("Android is a phone").is_none());
        assert_eq!(Step::parse("*").unwrap().text, "");
    }

    #[test]
    fn the_scenarios_and_their_steps_are_read_in_order() {
        let feature = parse_feature(&lines(
            "Feature: login\nScenario: valid login\nGiven user is on login page\n\
             When user enters credentials\nThen user is logged in\n",
        ));
        assert_eq!(feature.name, "login");
        assert_eq!(feature.scenarios.len(), 1);
        assert_eq!(feature.scenarios[0].name, "valid login");
        assert_eq!(feature.scenarios[0].steps.len(), 3);
    }

    #[test]
    fn tags_background_examples_and_doc_strings_are_not_steps() {
        let text = "\u{feff}@smoke\nFeature: f\nBackground:\nGiven the app is open\n\
                    @slow\nScenario Outline: visit <page>\nGiven I open <page>\n\
                    \"\"\"\nWhen a doc string\n\"\"\"\nExamples:\n| page |\n| home |\n\
                    Scenario Template: second\nThen b\n";
        let feature = parse_feature(&lines(text));
        let names: Vec<_> = feature.scenarios.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["visit <page>", "second"]);
        assert_eq!(feature.scenarios[0].steps.len(), 1);
        assert_eq!(feature.scenarios[1].steps[0].full_text(), "Then b");
    }
}
