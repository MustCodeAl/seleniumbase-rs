//! The Gherkin parser, the tag filter and the runner of `seleniumbase_rs::behave`,
//! through the public API.
//!
//! No browser is involved: the steps record what they were asked to do.

use std::cell::RefCell;
use std::rc::Rc;

use seleniumbase_rs::behave::{
    expand_scenario_outline, parse_feature, run_feature_file, run_feature_file_with_filter,
    CommonSteps, RunFilter, ScenarioResult, StepRegistry,
};

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

/// Writes `text` to a feature file and runs it.
fn run(text: &str, registry: &StepRegistry, filter: &RunFilter) -> Vec<ScenarioResult> {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("test.feature");
    std::fs::write(&path, text).expect("write feature");
    run_feature_file_with_filter(&path, registry, filter).expect("feature runs")
}

/// A registry whose exact steps record the text they were run with.
fn recording(patterns: &[&str]) -> (StepRegistry, Rc<RefCell<Vec<String>>>) {
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
fn a_word_that_only_starts_like_a_keyword_is_not_a_step() {
    let feature = parse_feature(&lines(
        "Feature: f\nScenario: s\nGiven a phone\nAndroid is a phone\nButter is yellow\n\
         Whenever it rings\nThen it rings\n",
    ));

    let keywords: Vec<&str> = feature.scenarios[0]
        .steps
        .iter()
        .map(|step| step.keyword.as_str())
        .collect();
    assert_eq!(keywords, ["Given", "Then"]);
}

#[test]
fn a_star_is_a_step_keyword() {
    let feature = parse_feature(&lines(
        "Feature: f\nScenario: s\nGiven a\n* b\n**bold** text\n",
    ));

    let steps = &feature.scenarios[0].steps;
    assert_eq!(steps.len(), 2);
    assert_eq!(
        (steps[1].keyword.as_str(), steps[1].text.as_str()),
        ("*", "b")
    );
}

#[test]
fn an_empty_example_cell_stays_in_its_column() {
    let feature = parse_feature(&lines(
        "Feature: f\nScenario Outline: o\nGiven <a> <b> <c>\n\
         Examples:\n| a | b | c |\n| 1 |   | 3 |\n",
    ));

    let expanded = expand_scenario_outline(&feature.scenarios[0]);

    assert_eq!(expanded.len(), 1);
    assert_eq!(expanded[0].steps[0].text, "1  3");
}

#[test]
fn an_escaped_pipe_belongs_to_its_cell() {
    let feature = parse_feature(&lines(
        "Feature: f\nScenario Outline: o\nGiven <a>\nExamples:\n| a |\n| x \\| y |\n",
    ));

    let expanded = expand_scenario_outline(&feature.scenarios[0]);

    assert_eq!(expanded[0].steps[0].text, "x | y");
}

#[test]
fn tags_above_an_examples_table_do_not_pass_to_the_next_scenario() {
    let feature = parse_feature(&lines(
        "Feature: f\nScenario Outline: o\nGiven <a>\n@slow\nExamples:\n| a |\n| 1 |\n\
         Scenario: next\nGiven b\n",
    ));

    assert!(
        feature.scenarios[1].tags.is_empty(),
        "'next' was tagged {:?}",
        feature.scenarios[1].tags
    );
    assert_eq!(
        feature.scenarios[0].tags,
        ["@slow"],
        "they belong to the outline"
    );
}

#[test]
fn lines_inside_a_doc_string_are_not_parsed() {
    let feature = parse_feature(&lines(
        "Feature: f\nScenario: s\nGiven a payload\n  \"\"\"\n  Given not a step\n  \
         Scenario: not a scenario\n  \"\"\"\nThen it is sent\n",
    ));

    assert_eq!(feature.scenarios.len(), 1);
    let texts: Vec<&str> = feature.scenarios[0]
        .steps
        .iter()
        .map(|step| step.text.as_str())
        .collect();
    assert_eq!(texts, ["a payload", "it is sent"]);
}

#[test]
fn scenario_template_and_scenarios_are_other_names_for_an_outline_and_examples() {
    let feature = parse_feature(&lines(
        "Feature: f\nScenario Template: t\nGiven <a>\nScenarios:\n| a |\n| 1 |\n| 2 |\n",
    ));

    assert!(feature.scenarios[0].is_outline);
    assert_eq!(expand_scenario_outline(&feature.scenarios[0]).len(), 2);
}

#[test]
fn a_value_that_looks_like_a_placeholder_is_inserted_as_written() {
    let feature = parse_feature(&lines(
        "Feature: f\nScenario Outline: o\nGiven <a> then <b>\nExamples:\n| a | b |\n| <b> | X |\n",
    ));

    let expanded = expand_scenario_outline(&feature.scenarios[0]);

    assert_eq!(expanded[0].steps[0].text, "<b> then X");
}

#[test]
fn an_outline_without_examples_runs_nothing() {
    let (registry, seen) = recording(&["Given a <thing>"]);

    let results = run(
        "Feature: f\nScenario Outline: o\nGiven a <thing>\n",
        &registry,
        &RunFilter::all(),
    );

    assert!(results.is_empty(), "{results:?}");
    assert!(seen.borrow().is_empty(), "ran with a raw placeholder");
}

#[test]
fn a_byte_order_mark_does_not_hide_the_first_line() {
    let feature = parse_feature(&lines("\u{feff}@web\nFeature: f\nScenario: s\nGiven a\n"));

    assert_eq!(feature.tags, ["@web"]);
}

#[test]
fn scenarios_inherit_the_tags_of_their_feature() {
    let (registry, _) = recording(&["Given a"]);
    let text = "@web\nFeature: f\nScenario: s\nGiven a\n";

    let included = run(
        text,
        &registry,
        &RunFilter {
            include_tags: vec!["@web".into()],
            exclude_tags: vec![],
        },
    );
    let excluded = run(
        text,
        &registry,
        &RunFilter {
            include_tags: vec![],
            exclude_tags: vec!["@web".into()],
        },
    );

    assert_eq!(included.len(), 1, "the feature's tag selects its scenarios");
    assert!(excluded.is_empty(), "and excludes them");
}

#[test]
fn a_tag_filter_does_not_need_the_at_sign() {
    let (registry, _) = recording(&["Given a"]);

    let results = run(
        "Feature: f\n@smoke\nScenario: s\nGiven a\n",
        &registry,
        &RunFilter {
            include_tags: vec!["smoke".into()],
            exclude_tags: vec![],
        },
    );

    assert_eq!(results.len(), 1);
}

#[test]
fn an_and_step_uses_the_handler_of_the_keyword_before_it() {
    let (registry, seen) = recording(&["Given a cat", "Then it purrs"]);

    let results = run(
        "Feature: f\nScenario: s\nGiven a cat\nAnd a cat\nThen it purrs\nBut it purrs\n",
        &registry,
        &RunFilter::all(),
    );

    assert!(results[0].passed, "{:?}", results[0].messages);
    assert_eq!(seen.borrow().len(), 4);
}

#[test]
fn an_optional_regex_group_does_not_shift_the_other_captures() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut registry = StepRegistry::new();
    let sink = Rc::clone(&seen);
    registry.register_regex(
        r#"^When I click( twice)? on "(.+?)"$"#,
        Box::new(move |_, caps| {
            sink.borrow_mut()
                .push(caps.last().cloned().unwrap_or_default());
            Ok(())
        }),
    );

    // The selector is the last capture whether or not the optional group
    // matched; what the handler reads at a fixed index must not change.
    registry.run(r#"When I click on "OK""#).unwrap();
    registry.run(r#"When I click twice on "OK""#).unwrap();
    assert_eq!(*seen.borrow(), ["OK", "OK"]);

    let positions = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&positions);
    registry.register_regex(
        r#"^Then I see( no)? "(.+?)"$"#,
        Box::new(move |_, caps| {
            sink.borrow_mut().push(caps.len());
            Ok(())
        }),
    );
    registry.run(r#"Then I see "a""#).unwrap();
    registry.run(r#"Then I see no "a""#).unwrap();
    assert_eq!(*positions.borrow(), [2, 2], "two groups, matched or not");
}

#[test]
fn the_common_steps_can_be_run_from_a_feature_file() {
    let mut registry = StepRegistry::new();
    CommonSteps::register_all(&mut registry);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("common.feature");
    std::fs::write(
        &path,
        "Feature: f\nScenario: s\nGiven the system is ready\nWhen I wait 2 seconds\nAnd I do nothing\n",
    )
    .unwrap();

    let results = run_feature_file(&path, &registry).unwrap();

    assert!(results[0].passed, "{:?}", results[0].messages);
}
