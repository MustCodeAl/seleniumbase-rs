//! Running a parsed feature against a step registry.
//!
//! A scenario runs its feature's `Background:` steps first, then its own, and
//! stops at the first step that fails. Scenario outlines run once per example
//! row. The result of each scenario is a [`ScenarioResult`]; nothing here
//! panics on a failing step.

use crate::behave::async_steps::AsyncStepRegistry;
use crate::behave::helper::{expand_scenario_outline, parse_feature, Step};
use crate::behave::steps::StepRegistry;
use std::fs;
use std::path::Path;

/// Result of running one scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioResult {
    /// The scenario's name; `name (example N)` for a row of an outline.
    pub name: String,
    /// `true` when every step ran and none failed.
    pub passed: bool,
    /// Why the scenario failed: either `no handler registered for step: ...`
    /// or `step '...' failed: ...` with the handler's own message. Empty for a
    /// passing scenario.
    pub messages: Vec<String>,
}

/// Optional filter for running scenarios by tag.
///
/// A scenario runs if it has at least one of `include_tags` (or `include_tags`
/// is empty) and none of `exclude_tags`. The feature's own tags count as the
/// scenario's. A leading `@` is optional on either side, as it is on the
/// `behave` command line: `smoke` and `@smoke` are the same tag.
#[derive(Debug, Clone, Default)]
pub struct RunFilter {
    /// Run only scenarios with one of these tags; empty means no restriction.
    pub include_tags: Vec<String>,
    /// Skip scenarios with any of these tags.
    pub exclude_tags: Vec<String>,
}

impl RunFilter {
    /// A filter that lets every scenario through.
    pub fn all() -> Self {
        Self::default()
    }

    /// Whether a scenario with `tags` should run.
    ///
    /// ```
    /// use seleniumbase_rs::behave::RunFilter;
    ///
    /// let filter = RunFilter {
    ///     include_tags: vec!["smoke".into()],
    ///     exclude_tags: vec!["@slow".into()],
    /// };
    /// assert!(filter.matches(&["@smoke".into()]));
    /// assert!(!filter.matches(&["@smoke".into(), "slow".into()]));
    /// assert!(!filter.matches(&[]));
    /// ```
    pub fn matches(&self, tags: &[String]) -> bool {
        let has = |wanted: &String| {
            tags.iter()
                .any(|tag| tag.trim_start_matches('@') == wanted.trim_start_matches('@'))
        };
        if !self.include_tags.is_empty() && !self.include_tags.iter().any(has) {
            return false;
        }
        !self.exclude_tags.iter().any(has)
    }
}

/// A scenario ready to run: the feature's background steps, then its own.
struct Planned {
    name: String,
    steps: Vec<Step>,
}

/// Parses `text` and lists the scenarios `filter` lets through, outlines
/// expanded.
fn plan(text: &str, filter: &RunFilter) -> Vec<Planned> {
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let feature = parse_feature(&lines);
    let mut planned = Vec::new();
    for scenario in &feature.scenarios {
        for expanded in expand_scenario_outline(scenario) {
            if !filter.matches(&feature.effective_tags(&expanded)) {
                continue;
            }
            let mut steps = feature.background.clone();
            steps.extend(expanded.steps);
            planned.push(Planned {
                name: expanded.name,
                steps,
            });
        }
    }
    planned
}

/// The text to look a step up by.
///
/// `And`, `But` and `*` continue the step before them, as in `behave`: a
/// registry that only knows `Given I am logged in` also runs `And I am logged
/// in` after a `Given`. The step as written is tried first, so a handler
/// registered for the exact `And ...` text still wins.
fn lookup_text<'a>(
    step: &'a Step,
    last_primary: &mut Option<&'a str>,
    has_handler: impl Fn(&str) -> bool,
) -> String {
    let literal = step.full_text();
    if !matches!(step.keyword.as_str(), "And" | "But" | "*") {
        *last_primary = Some(step.keyword.as_str());
        return literal;
    }
    if !has_handler(&literal) {
        if let Some(primary) = *last_primary {
            let inherited = format!("{primary} {}", step.text);
            if has_handler(&inherited) {
                return inherited;
            }
        }
    }
    literal
}

fn run_steps(steps: &[Step], registry: &StepRegistry) -> Result<(), String> {
    let mut last_primary = None;
    for step in steps {
        let text = lookup_text(step, &mut last_primary, |s| registry.has_handler(s));
        if !registry.has_handler(&text) {
            return Err(format!("no handler registered for step: {text}"));
        }
        registry
            .run(&text)
            .map_err(|e| format!("step '{text}' failed: {e}"))?;
    }
    Ok(())
}

async fn run_steps_async(steps: &[Step], registry: &AsyncStepRegistry) -> Result<(), String> {
    let mut last_primary = None;
    for step in steps {
        let text = lookup_text(step, &mut last_primary, |s| registry.has_handler(s));
        if !registry.has_handler(&text) {
            return Err(format!("no handler registered for step: {text}"));
        }
        registry
            .run(&text)
            .await
            .map_err(|e| format!("step '{text}' failed: {e}"))?;
    }
    Ok(())
}

fn result_of(name: String, outcome: Result<(), String>) -> ScenarioResult {
    match outcome {
        Ok(()) => ScenarioResult {
            name,
            passed: true,
            messages: Vec::new(),
        },
        Err(message) => ScenarioResult {
            name,
            passed: false,
            messages: vec![message],
        },
    }
}

/// Runs the scenarios in feature text that `filter` lets through.
///
/// ```
/// use seleniumbase_rs::behave::{run_feature_text, RunFilter, StepRegistry};
///
/// let mut registry = StepRegistry::new();
/// registry.register("Given a cat", Box::new(|_, _| Ok(())));
/// registry.register_regex(r#"^Then it says "(.+)"$"#, Box::new(|_, caps| {
///     if caps[0] == "meow" { Ok(()) } else { Err(format!("a cat does not say {}", caps[0])) }
/// }));
///
/// let feature = "Feature: pets\n\
///                Scenario: cat\nGiven a cat\nThen it says \"meow\"\n\
///                Scenario: wrong cat\nGiven a cat\nThen it says \"woof\"\n";
/// let results = run_feature_text(feature, &registry, &RunFilter::all());
/// assert!(results[0].passed);
/// assert!(!results[1].passed);
/// assert!(results[1].messages[0].contains("a cat does not say woof"));
/// ```
pub fn run_feature_text(
    text: &str,
    registry: &StepRegistry,
    filter: &RunFilter,
) -> Vec<ScenarioResult> {
    plan(text, filter)
        .into_iter()
        .map(|scenario| result_of(scenario.name, run_steps(&scenario.steps, registry)))
        .collect()
}

/// Like [`run_feature_text`], for an [`AsyncStepRegistry`].
pub async fn run_feature_text_async(
    text: &str,
    registry: &AsyncStepRegistry,
    filter: &RunFilter,
) -> Vec<ScenarioResult> {
    let mut results = Vec::new();
    for scenario in plan(text, filter) {
        let outcome = run_steps_async(&scenario.steps, registry).await;
        results.push(result_of(scenario.name, outcome));
    }
    results
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
    Ok(run_feature_text(&content, registry, filter))
}

/// Run all scenarios in a feature file using an [`AsyncStepRegistry`].
pub async fn run_feature_file_async<P: AsRef<Path>>(
    path: P,
    registry: &AsyncStepRegistry,
) -> Result<Vec<ScenarioResult>, std::io::Error> {
    run_feature_file_with_filter_async(path, registry, &RunFilter::all()).await
}

/// Run scenarios in a feature file with tag filtering, using an
/// [`AsyncStepRegistry`].
pub async fn run_feature_file_with_filter_async<P: AsRef<Path>>(
    path: P,
    registry: &AsyncStepRegistry,
    filter: &RunFilter,
) -> Result<Vec<ScenarioResult>, std::io::Error> {
    let content = tokio::fs::read_to_string(path).await?;
    Ok(run_feature_text_async(&content, registry, filter).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::RefCell;
    use std::rc::Rc;

    /// A registry that records every step it runs, in order.
    fn recording_registry(patterns: &[&str]) -> (StepRegistry, Rc<RefCell<Vec<String>>>) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut registry = StepRegistry::new();
        for pattern in patterns {
            let seen = Rc::clone(&seen);
            registry.register(
                *pattern,
                Box::new(move |step, _| {
                    seen.borrow_mut().push(step.to_owned());
                    Ok(())
                }),
            );
        }
        (registry, seen)
    }

    #[test]
    fn test_run_feature_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pass.feature");
        fs::write(
            &path,
            "Feature: test\nScenario: pass\nGiven one\nThen two\n",
        )
        .unwrap();
        let mut registry = StepRegistry::new();
        registry.register("Given one", Box::new(|_, _| Ok(())));
        registry.register("Then two", Box::new(|_, _| Ok(())));
        let results = run_feature_file(&path, &registry).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
    }

    #[test]
    fn test_run_feature_with_background() {
        let (registry, seen) = recording_registry(&["Given setup", "Then check"]);
        let results = run_feature_text(
            "Feature: bg\nBackground:\nGiven setup\nScenario: s\nThen check\n",
            &registry,
            &RunFilter::all(),
        );
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
        assert_eq!(*seen.borrow(), ["Given setup", "Then check"]);
    }

    #[test]
    fn a_missing_feature_file_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let error =
            run_feature_file(dir.path().join("nope.feature"), &StepRegistry::new()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
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

    #[test]
    fn a_tag_filter_does_not_care_about_the_at_sign() {
        let filter = RunFilter {
            include_tags: vec!["smoke".into()],
            exclude_tags: vec!["skip".into()],
        };
        assert!(filter.matches(&["@smoke".into()]));
        assert!(!filter.matches(&["@smoke".into(), "@skip".into()]));
    }

    #[test]
    fn scenarios_inherit_the_tags_of_their_feature() {
        let (registry, seen) = recording_registry(&["Given a"]);
        let feature = "@web\nFeature: f\nScenario: tagged by the feature\nGiven a\n";
        let only_web = RunFilter {
            include_tags: vec!["@web".into()],
            exclude_tags: vec![],
        };
        assert_eq!(run_feature_text(feature, &registry, &only_web).len(), 1);
        let not_web = RunFilter {
            include_tags: vec![],
            exclude_tags: vec!["@web".into()],
        };
        assert!(run_feature_text(feature, &registry, &not_web).is_empty());
        assert_eq!(seen.borrow().len(), 1, "only the first run executed a step");
    }

    #[test]
    fn and_and_but_continue_the_keyword_before_them() {
        let (registry, seen) = recording_registry(&["Given a cat", "Then it purrs"]);
        let feature =
            "Feature: f\nScenario: s\nGiven a cat\nAnd a cat\nThen it purrs\nBut it purrs\n";

        let results = run_feature_text(feature, &registry, &RunFilter::all());

        assert!(results[0].passed, "{:?}", results[0].messages);
        assert_eq!(
            *seen.borrow(),
            [
                "Given a cat",
                "Given a cat",
                "Then it purrs",
                "Then it purrs"
            ]
        );
    }

    #[test]
    fn a_handler_for_the_and_text_itself_wins_over_the_inherited_one() {
        let (registry, seen) = recording_registry(&["Given a cat", "And a cat"]);

        run_feature_text(
            "Feature: f\nScenario: s\nGiven a cat\nAnd a cat\n",
            &registry,
            &RunFilter::all(),
        );

        assert_eq!(*seen.borrow(), ["Given a cat", "And a cat"]);
    }

    #[test]
    fn an_and_after_the_background_continues_the_background_keyword() {
        let (registry, seen) = recording_registry(&["Given a cat"]);

        let results = run_feature_text(
            "Feature: f\nBackground:\nGiven a cat\nScenario: s\nAnd a cat\n",
            &registry,
            &RunFilter::all(),
        );

        assert!(results[0].passed, "{:?}", results[0].messages);
        assert_eq!(seen.borrow().len(), 2);
    }

    #[test]
    fn a_failure_names_the_step_that_failed_and_skips_the_rest() {
        let (mut registry, seen) = recording_registry(&["Given one", "Then three"]);
        registry.register("When two", Box::new(|_, _| Err("boom".into())));

        let results = run_feature_text(
            "Feature: f\nScenario: s\nGiven one\nWhen two\nThen three\n",
            &registry,
            &RunFilter::all(),
        );

        assert!(!results[0].passed);
        assert_eq!(results[0].messages, ["step 'When two' failed: boom"]);
        assert_eq!(*seen.borrow(), ["Given one"], "'Then three' never ran");
    }

    #[test]
    fn an_undefined_step_is_reported_by_its_text() {
        let results = run_feature_text(
            "Feature: f\nScenario: s\nGiven something unheard of\n",
            &StepRegistry::new(),
            &RunFilter::all(),
        );

        assert_eq!(
            results[0].messages,
            ["no handler registered for step: Given something unheard of"]
        );
    }

    #[test]
    fn an_outline_without_examples_runs_nothing() {
        let (registry, seen) = recording_registry(&["Given a <thing>"]);

        let results = run_feature_text(
            "Feature: f\nScenario Outline: o\nGiven a <thing>\n",
            &registry,
            &RunFilter::all(),
        );

        assert!(results.is_empty());
        assert!(seen.borrow().is_empty(), "no step ran with a raw <thing>");
    }
}
