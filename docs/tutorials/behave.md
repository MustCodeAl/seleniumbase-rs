---
title: "Behave / Gherkin Support"
---

# Behave / Gherkin Support

SeleniumBase for Rust includes a lightweight Gherkin parser and step runner so
you can write BDD-style feature files similar to Python's `behave` framework.
This lets product owners, QA engineers, and developers share a common language
for describing acceptance criteria.

## What you will learn

- How to write `.feature` files.
- How to register sync and async step definitions.
- How to filter scenarios by tags.
- How to run feature files from Rust tests, and what the CLI offers.

## Feature files

Create a `.feature` file using standard Gherkin syntax:

```gherkin
Feature: Login

  Background:
    Given the login page is open

  @smoke
  Scenario: valid login
    When I type "user@example.com" into "#email"
    And I type "secret" into "#password"
    And I click "#submit"
    Then I see "Welcome"

  Scenario Outline: multiple users
    When I type "<email>" into "#email"
    And I type "<password>" into "#password"
    And I click "#submit"
    Then I see "<message>"

    Examples:
      | email              | password | message   |
      | user@example.com   | secret   | Welcome   |
      | admin@example.com  | admin    | Dashboard |
```

The parser reads `Feature:`, `Background:`, `Scenario:`, `Scenario Outline:`,
`Examples:` tables, `@tags` and steps that start with `Given`, `When`, `Then`,
`And` or `But`. Each outline row becomes its own scenario, named
`<name> (example N)`.

<Note>

Tags on a scenario are the scenario's own; tags written above `Feature:` are not
copied down to its scenarios.

</Note>

## Sync runner

Use the sync runner for feature-file parsing smoke tests or non-browser steps.
`run_feature_file` hands the registry each step **with its keyword**, for
example `When I type "x" into "#email"`, so exact patterns must include the
keyword and regexes must allow for it:

```rust,no_run
use seleniumbase_rs::behave::{run_feature_file, StepRegistry};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut registry = StepRegistry::new();
registry.register("Given the login page is open", Box::new(|_, _| Ok(())));
registry.register_regex(
    r#"^(?:When|And) I type "(.+?)" into "(.+?)"$"#,
    Box::new(|_, caps| {
        println!("type {} into {}", caps[0], caps[1]);
        Ok(())
    }),
);

let results = run_feature_file("tests/login.feature", &registry)?;
for r in &results {
    println!("{} passed={}", r.name, r.passed);
}
# Ok(())
# }
```

<Info>

A step with no matching handler fails its scenario with
`no handler registered for step: ...`; the scenario result's `messages` holds
the reason. The Background steps run before each scenario.

</Info>

## Browser-backed async steps

For real browser automation, use `AsyncStepRegistry` backed by a connected
`BaseCase`. It has no feature-file runner of its own: parse the file, expand the
outlines, and run each step's text yourself. The built-in browser steps are
anchored without a keyword, so pass `step.text`, not the full step:

```rust,no_run
use seleniumbase_rs::behave::{expand_scenario_outline, parse_feature, AsyncStepRegistry};
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sb = BaseCase::new(BrowserConfig::default()).await?;
    let mut registry = AsyncStepRegistry::new(sb);
    registry.register_common_browser_steps();

    let text = std::fs::read_to_string("tests/login.feature")?;
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let feature = parse_feature(&lines);

    for scenario in &feature.scenarios {
        for scenario in expand_scenario_outline(scenario) {
            for step in feature.background.iter().chain(&scenario.steps) {
                registry.run(&step.text).await?;
            }
        }
    }
    Ok(())
}
```

`register_common_browser_steps` registers these steps and no others:
`I open "<url>"`, `I go back`, `I go forward`, `I click "<selector>"` and
`I type "<text>" into "<selector>"`. Register your own with `register` and
`register_regex`, for example the `I see "..."` step above. A handler receives
the shared `Rc<RefCell<BaseCase>>`, the step text, and the regex captures.
<Warning>

`BaseCase::without_session` builds a `BaseCase` with no browser, for tests that
never touch it; any browser call on it panics.

</Warning>

## Tag filtering

Run only scenarios matching specific tags:

```rust,no_run
use seleniumbase_rs::behave::{run_feature_file_with_filter, RunFilter, StepRegistry};

# fn main() -> Result<(), std::io::Error> {
# let registry = StepRegistry::new();
let filter = RunFilter {
    include_tags: vec!["@smoke".into()],
    exclude_tags: vec!["@skip".into()],
};
let results = run_feature_file_with_filter("tests/login.feature", &registry, &filter)?;
# let _ = results;
# Ok(())
# }
```

A scenario runs when it has at least one of the `include_tags` (or the list is
empty) and none of the `exclude_tags`. `RunFilter::all()` matches everything.

## From the command line

`sbrs behave-gui` (build with the `tui` feature) is a terminal app that lists
the feature files under `features/`, runs the scenarios you mark through the
runner above and shows the results. It only has the built-in steps, since steps
written in Rust cannot be loaded into `sbrs`, so a scenario that uses your own
steps is reported as undefined there; run those from a test of your own, as in
this chapter. See [Behave Commander](../help/behave-gui.md).

`sbrs behave-gui --new` needs no feature: it prompts for a feature name, title,
scenarios and steps and writes `features/<name>.feature` (plus a
`features/README.md` that suggests the `cucumber` crate for a fuller BDD
runner).

## Best practices

- Prefer regex step patterns over exact strings for reusable steps, and allow
  for the keyword in the sync runner.
- Keep `Background` sections short and focused on shared preconditions.
- Use tags to organize smoke, regression, and slow tests.
- Async steps hold a `RefCell` borrow across awaits; run one step at a time.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Step not matched | In the sync runner, the pattern leaves out the keyword (`When`, `And`, ...), or has unescaped quotes or regex metacharacters | Match the keyword too, use raw strings, and test the regex first. |
| Step not matched in the async registry | You passed the step with its keyword | Pass `step.text`. |
| Scenario Outline fails | Placeholder not replaced | Ensure the `Examples` table headers match `<placeholders>`. |
| Borrow error across await | Step holds registry borrow while awaiting | Keep steps short and finish before the next step. |
