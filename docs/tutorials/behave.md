# Behave / Gherkin Support

SeleniumBase for Rust includes a lightweight Gherkin parser and step runner so
you can write BDD-style feature files similar to Python's `behave` framework.

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

## Sync runner

Use the sync runner for feature-file parsing smoke tests or non-browser steps:

```rust
use seleniumbase_rs::behave::{run_feature_file, StepRegistry};

let mut registry = StepRegistry::new();
registry.register("Given the login page is open", Box::new(|_, _| Ok(())));
registry.register_regex(
    r#"^I type "(.+?)" into "(.+?)"$"#,
    Box::new(|_, caps| {
        println!("type {} into {}", caps[0], caps[1]);
        Ok(())
    }),
);

let results = run_feature_file("tests/login.feature", &registry)?;
for r in &results {
    println!("{} passed={}", r.name, r.passed);
}
```

## Browser-backed async steps

For real browser automation, use `AsyncStepRegistry` backed by a `BaseCase`:

```rust
use seleniumbase_rs::behave::AsyncStepRegistry;
use seleniumbase_rs::BrowserConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default();
    let base_case = seleniumbase_rs::BaseCase::without_session(config);
    let mut registry = AsyncStepRegistry::new(base_case);
    registry.register_common_browser_steps();

    registry.run(r#"I open "https://example.com""#).await?;
    registry.run(r#"I click "#submit""#).await?;
    Ok(())
}
```

## Tag filtering

Run only scenarios matching specific tags:

```rust
use seleniumbase_rs::behave::{run_feature_file_with_filter, RunFilter};

let filter = RunFilter {
    include_tags: vec!["@smoke".into()],
    exclude_tags: vec!["@skip".into()],
};
let results = run_feature_file_with_filter("tests/login.feature", &registry, &filter)?;
```

## Best practices

- Prefer regex step patterns over exact strings for reusable steps.
- Keep `Background` sections short and focused on shared preconditions.
- Use tags to organize smoke, regression, and slow tests.
- Async steps hold a `RefCell` borrow across awaits; run one step at a time.
