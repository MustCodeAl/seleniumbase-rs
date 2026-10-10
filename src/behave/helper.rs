//! A small Gherkin parser: features, backgrounds, scenarios, scenario
//! outlines with `Examples:` tables, and tags.
//!
//! Data tables and doc strings attached to a step are skipped, not stored: a
//! [`Step`] carries only its keyword and text.

use std::fs;
use std::path::Path;

/// The words that start a step. `*` is Gherkin's keyword-free step.
const STEP_KEYWORDS: [&str; 6] = ["Given", "When", "Then", "And", "But", "*"];

/// A single Gherkin step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The keyword as written: `Given`, `When`, `Then`, `And`, `But` or `*`.
    pub keyword: String,
    /// The text after the keyword, trimmed.
    pub text: String,
}

impl Step {
    /// Reads a step from a line such as `Given I am logged in`.
    ///
    /// The line must start with a keyword (`Given`, `When`, `Then`, `And`,
    /// `But` or `*`) as a whole word; anything else is not a step.
    ///
    /// ```
    /// use seleniumbase_rs::behave::Step;
    ///
    /// let step = Step::parse("  When I click \"Go\"").unwrap();
    /// assert_eq!((step.keyword.as_str(), step.text.as_str()), ("When", "I click \"Go\""));
    /// assert!(Step::parse("Android is a phone").is_none());
    /// ```
    pub fn parse(line: &str) -> Option<Step> {
        let trimmed = line.trim();
        for keyword in STEP_KEYWORDS {
            if let Some(rest) = trimmed.strip_prefix(keyword) {
                // The keyword is a whole word: `Android is a phone` is not
                // `And` followed by `roid is a phone`.
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
    ///
    /// ```
    /// use seleniumbase_rs::behave::Step;
    ///
    /// let step = Step { keyword: "Given".into(), text: "I am logged in".into() };
    /// assert_eq!(step.full_text(), "Given I am logged in");
    /// ```
    pub fn full_text(&self) -> String {
        format!("{} {}", self.keyword, self.text)
    }
}

/// A scenario parsed from a feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    /// The text after `Scenario:` (or `Scenario Outline:`).
    pub name: String,
    /// Tags written above the scenario, plus the tags written above any of its
    /// `Examples:` tables. Use [`Feature::effective_tags`] to include the
    /// feature's own tags.
    pub tags: Vec<String>,
    /// The scenario's steps, not including the feature's background.
    pub steps: Vec<Step>,
    /// `true` for a `Scenario Outline:` (or `Scenario Template:`).
    pub is_outline: bool,
    /// One list of `(column, value)` pairs per row of the `Examples:` tables.
    pub examples: Vec<Vec<(String, String)>>,
}

/// A feature file parsed into structured form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    /// The text after `Feature:`.
    pub name: String,
    /// Tags written above the feature.
    pub tags: Vec<String>,
    /// The steps of the `Background:`, run before every scenario.
    pub background: Vec<Step>,
    /// The scenarios and scenario outlines, in file order.
    pub scenarios: Vec<Scenario>,
}

impl Feature {
    /// The tags of `scenario` together with the feature's own, which every
    /// scenario inherits.
    ///
    /// ```
    /// use seleniumbase_rs::behave::parse_feature;
    ///
    /// let text = "@web\nFeature: f\n@smoke\nScenario: s\nGiven a\n";
    /// let lines: Vec<String> = text.lines().map(String::from).collect();
    /// let feature = parse_feature(&lines);
    /// assert_eq!(
    ///     feature.effective_tags(&feature.scenarios[0]),
    ///     vec!["@web".to_string(), "@smoke".to_string()]
    /// );
    /// ```
    pub fn effective_tags(&self, scenario: &Scenario) -> Vec<String> {
        let mut tags = self.tags.clone();
        for tag in &scenario.tags {
            if !tags.contains(tag) {
                tags.push(tag.clone());
            }
        }
        tags
    }
}

/// Read a Gherkin feature file and return its lines.
pub fn read_feature_file<P: AsRef<Path>>(path: P) -> Result<Vec<String>, std::io::Error> {
    let content = fs::read_to_string(path)?;
    Ok(content.lines().map(|l| l.to_string()).collect())
}

fn extract_tags(line: &str) -> Vec<String> {
    line.split_whitespace()
        .filter(|t| t.starts_with('@'))
        .map(|t| t.to_owned())
        .collect()
}

/// Splits a table row such as `| a |  | c |` into its cells.
///
/// Empty cells are kept, so every value stays under its own column. `\|`,
/// `\\` and `\n` are the Gherkin escapes for a pipe, a backslash and a newline.
fn split_table_row(row: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = row.trim().chars().peekable();
    if chars.peek() == Some(&'|') {
        chars.next();
    }
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('|') => cell.push('|'),
                Some('n') => cell.push('\n'),
                Some('\\') => cell.push('\\'),
                Some(other) => {
                    cell.push('\\');
                    cell.push(other);
                }
                None => cell.push('\\'),
            },
            '|' => {
                cells.push(cell.trim().to_owned());
                cell.clear();
            }
            other => cell.push(other),
        }
    }
    // A row without its closing pipe still has a last cell.
    if !cell.trim().is_empty() {
        cells.push(cell.trim().to_owned());
    }
    cells
}

/// Extract scenarios from a feature file as (name, step_lines).
pub fn parse_scenarios(lines: &[String]) -> Vec<(String, Vec<String>)> {
    let feature = parse_feature(lines);
    feature
        .scenarios
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                s.steps.iter().map(|step| step.full_text()).collect(),
            )
        })
        .collect()
}

/// Parses the lines of a feature file into a [`Feature`].
///
/// Besides `Scenario:` and `Scenario Outline:` the parser accepts
/// `Scenario Template:` and `Scenarios:` (the alternative spellings of an
/// outline and of `Examples:`). Tags above an `Examples:` table are added to
/// the outline's own tags, so they apply to every row of every table. Lines
/// inside a `"""` or ```` ``` ```` doc string are skipped.
///
/// ```
/// use seleniumbase_rs::behave::parse_feature;
///
/// let text = "Feature: login\nScenario: ok\nGiven a\nThen b\n";
/// let lines: Vec<String> = text.lines().map(String::from).collect();
/// let feature = parse_feature(&lines);
/// assert_eq!(feature.name, "login");
/// assert_eq!(feature.scenarios[0].steps.len(), 2);
/// ```
pub fn parse_feature(lines: &[String]) -> Feature {
    let mut feature = Feature {
        name: String::new(),
        tags: Vec::new(),
        background: Vec::new(),
        scenarios: Vec::new(),
    };
    let mut pending_tags: Vec<String> = Vec::new();
    let mut current_scenario: Option<Scenario> = None;
    let mut in_background = false;
    let mut in_examples = false;
    let mut example_header: Vec<String> = Vec::new();
    // The fence that opened the doc string we are inside, if any.
    let mut doc_string: Option<&str> = None;

    for line in lines {
        // A byte order mark, as Windows editors write at the start of a file,
        // is not white space; left in, it would hide the first line's keyword
        // or tag.
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
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(name) = trimmed.strip_prefix("Feature:") {
            feature.name = name.trim().to_string();
            feature.tags = pending_tags.clone();
            pending_tags.clear();
            continue;
        }

        if trimmed.starts_with('@') {
            pending_tags.extend(extract_tags(trimmed));
            continue;
        }

        if trimmed.starts_with("Background:") {
            in_background = true;
            in_examples = false;
            pending_tags.clear();
            continue;
        }

        let scenario_header = [
            ("Scenario Outline:", true),
            ("Scenario Template:", true),
            ("Scenario:", false),
        ]
        .into_iter()
        .find_map(|(keyword, is_outline)| {
            trimmed
                .strip_prefix(keyword)
                .map(|name| (name.trim().to_string(), is_outline))
        });
        if let Some((name, is_outline)) = scenario_header {
            if let Some(scenario) = current_scenario.take() {
                feature.scenarios.push(scenario);
            }
            current_scenario = Some(Scenario {
                name,
                tags: pending_tags.clone(),
                steps: Vec::new(),
                is_outline,
                examples: Vec::new(),
            });
            pending_tags.clear();
            in_background = false;
            in_examples = false;
            continue;
        }

        if trimmed.starts_with("Examples:") || trimmed.starts_with("Scenarios:") {
            in_examples = true;
            example_header.clear();
            // Tags written above the table belong to this outline. Leaving
            // them pending would hand them to whatever scenario comes next.
            match current_scenario.as_mut() {
                Some(scenario) => {
                    for tag in pending_tags.drain(..) {
                        if !scenario.tags.contains(&tag) {
                            scenario.tags.push(tag);
                        }
                    }
                }
                None => pending_tags.clear(),
            }
            continue;
        }

        if in_examples {
            if trimmed.starts_with('|') {
                let cells = split_table_row(trimmed);
                if example_header.is_empty() {
                    example_header = cells;
                } else if let Some(scenario) = current_scenario.as_mut() {
                    let row: Vec<(String, String)> =
                        example_header.iter().cloned().zip(cells).collect();
                    scenario.examples.push(row);
                }
            }
            continue;
        }

        if let Some(step) = Step::parse(trimmed) {
            if in_background {
                feature.background.push(step);
            } else if let Some(scenario) = current_scenario.as_mut() {
                scenario.steps.push(step);
            }
        }
    }

    if let Some(scenario) = current_scenario {
        feature.scenarios.push(scenario);
    }
    feature
}

/// Replaces each `<column>` in `text` with the row's value for that column.
///
/// The text is scanned once, so a value that itself contains `<other>` is
/// inserted as written. A `<name>` that is not a column of the row stays.
fn substitute(text: &str, row: &[(String, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let value = after.find('>').and_then(|end| {
            row.iter()
                .find(|(key, _)| key == &after[..end])
                .map(|(_, v)| (end, v))
        });
        match value {
            Some((end, value)) => {
                out.push_str(value);
                rest = &after[end + 1..];
            }
            None => {
                out.push('<');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Expands a scenario outline into one scenario per row of its `Examples:`
/// tables, replacing every `<column>` in the steps and in the name.
///
/// A scenario that is not an outline is returned as it is. An outline with no
/// example rows expands to nothing: Gherkin runs no scenario for it, and
/// running its steps with the `<column>` placeholders left in would test text
/// nobody wrote.
///
/// ```
/// use seleniumbase_rs::behave::{expand_scenario_outline, parse_feature};
///
/// let text = "Feature: f\nScenario Outline: visit <page>\nGiven I open <page>\n\
///             Examples:\n| page |\n| home |\n| cart |\n";
/// let lines: Vec<String> = text.lines().map(String::from).collect();
/// let feature = parse_feature(&lines);
/// let expanded = expand_scenario_outline(&feature.scenarios[0]);
/// assert_eq!(expanded.len(), 2);
/// assert_eq!(expanded[1].steps[0].text, "I open cart");
/// assert_eq!(expanded[1].name, "visit cart (example 2)");
/// ```
pub fn expand_scenario_outline(scenario: &Scenario) -> Vec<Scenario> {
    if !scenario.is_outline {
        return vec![scenario.clone()];
    }
    scenario
        .examples
        .iter()
        .enumerate()
        .map(|(i, row)| Scenario {
            name: format!("{} (example {})", substitute(&scenario.name, row), i + 1),
            tags: scenario.tags.clone(),
            steps: scenario
                .steps
                .iter()
                .map(|step| Step {
                    keyword: step.keyword.clone(),
                    text: substitute(&step.text, row),
                })
                .collect(),
            is_outline: false,
            examples: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scenarios() {
        let lines = vec![
            "Feature: login".into(),
            "Scenario: valid login".into(),
            "Given user is on login page".into(),
            "When user enters credentials".into(),
            "Then user is logged in".into(),
        ];
        let scenarios = parse_scenarios(&lines);
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].0, "valid login");
        assert_eq!(scenarios[0].1.len(), 3);
    }

    #[test]
    fn test_parse_feature_with_tags_and_background() {
        let lines = vec![
            "@smoke".into(),
            "Feature: login".into(),
            "Background:".into(),
            "Given the app is open".into(),
            "Scenario: valid login".into(),
            "Given user is on login page".into(),
            "Then user is logged in".into(),
        ];
        let feature = parse_feature(&lines);
        assert_eq!(feature.tags, vec!["@smoke"]);
        assert_eq!(feature.background.len(), 1);
        assert_eq!(feature.scenarios.len(), 1);
    }

    #[test]
    fn test_scenario_outline_expansion() {
        let scenario = Scenario {
            name: "login".into(),
            tags: vec![],
            steps: vec![Step {
                keyword: "Given".into(),
                text: "user is on <page>".into(),
            }],
            is_outline: true,
            examples: vec![vec![("page".into(), "home".into())]],
        };
        let expanded = expand_scenario_outline(&scenario);
        assert_eq!(expanded[0].steps[0].text, "user is on home");
    }
}
