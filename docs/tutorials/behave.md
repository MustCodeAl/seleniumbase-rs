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

Tags written above `Feature:` count as the tags of every scenario in it, so
`RunFilter` sees them on each one.

</Note>

## Sync runner

Use the sync runner for feature-file parsing smoke tests or non-browser steps.
`run_feature_file` hands the registry each step **with its keyword**, for
example `When I type "x" into "#email"`, so exact patterns must include the
keyword and regexes must allow for it. `register_step` does that for you: give
it the step's text as a regex, without keyword or anchors, and it matches
`Given`, `When`, `Then`, `And`, `But` or no keyword, like behave's `@step`:

```rust,no_run
use seleniumbase_rs::behave::{run_feature_file, StepRegistry};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut registry = StepRegistry::new();
registry.register("Given the login page is open", Box::new(|_, _| Ok(())));
registry.register_step(
    r#"I type "(.+?)" into "(.+?)""#,
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

`try_register_step` returns the regex error instead of panicking, for a pattern
that comes from outside the program. `register_regex` still takes a complete
pattern, and `step_pattern(text)` is the one `register_step` builds, for when
you want to combine it with your own anchors.

<Info>

A step with no matching handler fails its scenario with
`no handler registered for step: ...`; the scenario result's `messages` holds
the reason. The Background steps run before each scenario.

</Info>

## Browser-backed async steps

For real browser automation, use `AsyncStepRegistry` backed by a connected
`BaseCase`. It runs feature files the way the sync registry does:
`run_feature_file_async`, `run_feature_file_with_filter_async` and
`run_feature_text_async` read the Background, expand the outlines, apply the
tags and hand each step to the registry **with its keyword**. The built-in
browser steps accept any keyword (`Given`, `When`, `Then`, `And`, `But`) or
none, so they work in a feature file as written:

```rust,no_run
use seleniumbase_rs::behave::{run_feature_file_async, AsyncStepRegistry};
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sb = BaseCase::new(BrowserConfig::default()).await?;
    let mut registry = AsyncStepRegistry::new(sb);
    registry.register_common_browser_steps();
    // A step of your own: a handler gets the shared BaseCase, the step text
    // and the regex captures, and returns a future that is `Send`.
    registry.register_step(
        r#"I see "(.+?)""#,
        Box::new(|bc, _step, caps| {
            let wanted = caps[0].clone();
            Box::pin(async move {
                let page = bc
                    .lock()
                    .await
                    .get_text("body")
                    .await
                    .map_err(|e| e.to_string())?;
                if page.contains(&wanted) {
                    Ok(())
                } else {
                    Err(format!("`{wanted}` is not on the page"))
                }
            })
        }),
    );

    let results = run_feature_file_async("tests/login.feature", &registry).await?;
    for result in &results {
        println!("{} passed={}", result.name, result.passed);
    }

    // The registry owns the BaseCase until the run is over; take it back to
    // close the browser.
    if let Ok(mut sb) = registry.into_base_case() {
        sb.quit().await?;
    }
    Ok(())
}
```

`register_common_browser_steps` registers these steps and no others:
`I open "<url>"`, `I go back`, `I go forward`, `I click "<selector>"` and
`I type "<text>" into "<selector>"`, plus the wording that Python
SeleniumBase's `--rec-behave` recorder writes for the same: `Open "<url>"`
(also `Open 'url'` and `Open URL "url"`), `Go back`, `Go forward`,
`Click "<selector>"` (also `Click element "selector"`) and
`Type "<text>" into "<selector>"` (also `Type text ...`, and `in` for `into`),
in single or double quotes. `ActionRecorder::to_gherkin` writes this wording
too, so a recorded feature runs here and in Python SeleniumBase. Register
your own with `register`, `register_regex` and `register_step`, as the
`I see "..."` step above. A handler gets the shared
`BaseCase` (an `Arc<tokio::sync::Mutex<BaseCase>>`, locked while the step uses
the browser), the step text and the regex captures.

The registry is `Send + Sync`, so it can sit in an `Arc` and a run can be
handed to `tokio::spawn`. (The sync `StepRegistry` is not: its handlers may
hold an `Rc`.) Steps run one at a time.

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
this chapter. See [Behave Commander](/help/behave-gui).

`sbrs behave-gui --new` needs no feature: it prompts for a feature name, title,
scenarios and steps and writes `features/<name>.feature` (plus a
`features/README.md` that suggests the `cucumber` crate for a fuller BDD
runner).

## Best practices

- Prefer regex step patterns over exact strings for reusable steps, and allow
  for the keyword in the sync runner.
- Keep `Background` sections short and focused on shared preconditions.
- Use tags to organize smoke, regression, and slow tests.
- An async step locks the shared `BaseCase` with `bc.lock().await`; do not
  call `registry.run` from inside a step, which would wait for its own lock.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Step not matched | In the sync runner, the pattern leaves out the keyword (`When`, `And`, ...), or has unescaped quotes or regex metacharacters | Match the keyword too, use raw strings, and test the regex first. |
| Step not matched in the async registry | Same as the sync runner: the registry gets the step with its keyword | Match the keyword too, as the built-in steps do. |
| Scenario Outline fails | Placeholder not replaced | Ensure the `Examples` table headers match `<placeholders>`. |
| A run hangs | A step called `registry.run` or locked the `BaseCase` twice | Lock it once per step and run steps from the runner, not from each other. |
