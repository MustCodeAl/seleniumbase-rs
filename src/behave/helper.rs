use std::fs;
use std::path::Path;

/// A single Gherkin step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub keyword: String,
    pub text: String,
}

impl Step {
    pub fn full_text(&self) -> String {
        format!("{} {}", self.keyword, self.text)
    }
}

/// A scenario parsed from a feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    pub name: String,
    pub tags: Vec<String>,
    pub steps: Vec<Step>,
    pub is_outline: bool,
    pub examples: Vec<Vec<(String, String)>>,
}

/// A feature file parsed into structured form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    pub name: String,
    pub tags: Vec<String>,
    pub background: Vec<Step>,
    pub scenarios: Vec<Scenario>,
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

fn parse_step(trimmed: &str) -> Option<Step> {
    for keyword in ["Given", "When", "Then", "And", "But"] {
        if let Some(rest) = trimmed.strip_prefix(keyword) {
            return Some(Step {
                keyword: keyword.to_owned(),
                text: rest.trim().to_owned(),
            });
        }
    }
    None
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

/// Parse a feature file into a structured `Feature`.
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

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with("Feature:") {
            feature.name = trimmed.trim_start_matches("Feature:").trim().to_string();
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
            continue;
        }

        if trimmed.starts_with("Scenario Outline:") || trimmed.starts_with("Scenario:") {
            if let Some(scenario) = current_scenario.take() {
                feature.scenarios.push(scenario);
            }
            let name = trimmed
                .trim_start_matches("Scenario Outline:")
                .trim_start_matches("Scenario:")
                .trim()
                .to_string();
            let is_outline = trimmed.starts_with("Scenario Outline:");
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

        if trimmed.starts_with("Examples:") {
            in_examples = true;
            example_header.clear();
            continue;
        }

        if in_examples {
            if trimmed.starts_with('|') {
                let cells: Vec<String> = trimmed
                    .split('|')
                    .skip(1)
                    .map(|c| c.trim().to_owned())
                    .filter(|c| !c.is_empty())
                    .collect();
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

        if let Some(step) = parse_step(trimmed) {
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

/// Expand a scenario outline by substituting `<placeholder>` values from examples.
pub fn expand_scenario_outline(scenario: &Scenario) -> Vec<Scenario> {
    if !scenario.is_outline || scenario.examples.is_empty() {
        return vec![scenario.clone()];
    }
    scenario
        .examples
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut steps = Vec::new();
            for step in &scenario.steps {
                let mut text = step.text.clone();
                for (key, value) in row {
                    text = text.replace(&format!("<{}>", key), value);
                }
                steps.push(Step {
                    keyword: step.keyword.clone(),
                    text,
                });
            }
            Scenario {
                name: format!("{} (example {})", scenario.name, i + 1),
                tags: scenario.tags.clone(),
                steps,
                is_outline: false,
                examples: Vec::new(),
            }
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
