use seleniumbase_rs::behave::{run_feature_file, AsyncStepRegistry, StepRegistry};
use seleniumbase_rs::BrowserConfig;

/// Demonstrates both the sync behave runner (for feature-file parsing) and the
/// async browser-backed registry (for real Gherkin step definitions).
#[tokio::main]
async fn main() {
    // Sync runner example: parse a feature file and execute steps with a sync registry.
    let feature = r#"Feature: Search
  Background:
    Given the search page is loaded
  @smoke
  Scenario: search for Rust
    When I search for "Rust"
    Then I see results for "Rust"
"#;
    let tmp = std::env::temp_dir().join("sb_behave_example.feature");
    std::fs::write(&tmp, feature).unwrap();

    let mut registry = StepRegistry::new();
    registry.register("Given the search page is loaded", Box::new(|_, _| Ok(())));
    registry.register_regex(
        r#"^When I search for "(.+?)"$"#,
        Box::new(|_, caps| {
            println!("Searching for: {}", caps[0]);
            Ok(())
        }),
    );
    registry.register_regex(
        r#"^Then I see results for "(.+?)"$"#,
        Box::new(|_, caps| {
            println!("Verified results for: {}", caps[0]);
            Ok(())
        }),
    );

    let results = run_feature_file(&tmp, &registry).unwrap();
    for result in &results {
        println!("Scenario '{}' passed={}", result.name, result.passed);
    }
    let _ = std::fs::remove_file(&tmp);

    // Async registry example: define steps that drive a real BaseCase.
    let config = BrowserConfig::default();
    let base_case = seleniumbase_rs::BaseCase::without_session(config);
    let mut async_registry = AsyncStepRegistry::new(base_case);
    async_registry.register_common_browser_steps();
    async_registry.register_regex(
        r#"^I verify the title is "(.+?)"$"#,
        Box::new(|bc, _step, caps| {
            let expected = caps[0].clone();
            #[allow(clippy::await_holding_refcell_ref)]
            Box::pin(async move {
                let title = bc
                    .borrow_mut()
                    .get_title()
                    .await
                    .map_err(|e| format!("title failed: {}", e))?;
                if title != expected {
                    return Err(format!("expected title '{}', got '{}'", expected, title));
                }
                Ok(())
            })
        }),
    );

    // This step would only succeed with a live browser session.
    println!("Async registry ready with browser-backed steps.");
}
