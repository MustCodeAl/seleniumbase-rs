use crate::behave::helper::{expand_scenario_outline, parse_feature, Step};
use crate::behave::steps::StepRegistry;
use std::fs;
use std::path::Path;

/// Result of running one scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioResult {
    pub name: String,
    pub passed: bool,
    pub messages: Vec<String>,
}

/// Optional filter for running scenarios by tag.
pub struct RunFilter {
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
}

impl RunFilter {
    pub fn all() -> Self {
        Self {
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
        }
    }

    pub fn matches(&self, tags: &[String]) -> bool {
        if !self.include_tags.is_empty() && !self.include_tags.iter().any(|t| tags.contains(t)) {
            return false;
        }
        if self.exclude_tags.iter().any(|t| tags.contains(t)) {
            return false;
        }
        true
    }
}

fn run_steps(steps: &[Step], registry: &StepRegistry) -> Result<(), String> {
    for step in steps {
        registry.run(&step.full_text())?;
    }
    Ok(())
}

/// Run all scenarios in a feature file using the provided step registry.
pub fn run_feature_file<P: AsRef<Path>>(
    path: P,
    registry: &StepRegistry,
) -> Result<Vec<ScenarioResult>, std::io::Error> {
    run_feature_file_with_filter(path, registry, &RunFilter::all())
}

/// Run scenarios in a feature file with tag filtering.
pub fn run_feature_file_with_filter<P: AsRef<Path>>(
    path: P,
    registry: &StepRegistry,
    filter: &RunFilter,
) -> Result<Vec<ScenarioResult>, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let feature = parse_feature(&lines);
    let mut results = Vec::new();

    for scenario in &feature.scenarios {
        let expanded = expand_scenario_outline(scenario);
        for expanded_scenario in expanded {
            if !filter.matches(&expanded_scenario.tags) {
                continue;
            }
            let mut steps = feature.background.clone();
            steps.extend(expanded_scenario.steps.clone());
            let mut messages = Vec::new();
            let passed = match run_steps(&steps, registry) {
                Ok(()) => true,
                Err(e) => {
                    messages.push(e);
                    false
                }
            };
            results.push(ScenarioResult {
                name: expanded_scenario.name,
                passed,
                messages,
            });
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_feature_file() {
        let tmp = std::env::temp_dir().join("sb_feature_test.feature");
        fs::write(&tmp, "Feature: test\nScenario: pass\nGiven one\nThen two\n").unwrap();
        let mut registry = StepRegistry::new();
        registry.register("Given one", Box::new(|_, _| Ok(())));
        registry.register("Then two", Box::new(|_, _| Ok(())));
        let results = run_feature_file(&tmp, &registry).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_run_feature_with_background() {
        let tmp = std::env::temp_dir().join("sb_feature_bg.feature");
        fs::write(
            &tmp,
            "Feature: bg\nBackground:\nGiven setup\nScenario: s\nThen check\n",
        )
        .unwrap();
        let mut registry = StepRegistry::new();
        registry.register("Given setup", Box::new(|_, _| Ok(())));
        registry.register("Then check", Box::new(|_, _| Ok(())));
        let results = run_feature_file(&tmp, &registry).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_tag_filter() {
        let filter = RunFilter {
            include_tags: vec!["@smoke".into()],
            exclude_tags: vec!["@skip".into()],
        };
        assert!(filter.matches(&["@smoke".into()]));
        assert!(!filter.matches(&["@other".into()]));
        assert!(!filter.matches(&["@smoke".into(), "@skip".into()]));
    }
}
