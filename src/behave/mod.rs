//! Behave-style BDD: Gherkin feature files run against step definitions
//! written in Rust.
//!
//! 1. [`parse_feature`] turns the lines of a `.feature` file into a
//!    [`Feature`] (background, scenarios, outlines with `Examples:` tables,
//!    tags).
//! 2. A [`StepRegistry`] (or, for steps that drive a browser, an
//!    [`AsyncStepRegistry`]) maps step text to handlers. A step is looked up
//!    **with its keyword**, so a pattern reads `^Given I open "(.+)"$`.
//!    `And`, `But` and `*` continue the keyword before them, so one `Given`
//!    handler serves `Given ...` and the `And ...` lines after it.
//! 3. [`run_feature_text`] and [`run_feature_file`] run every scenario, outline
//!    rows included, and give one [`ScenarioResult`] per scenario.
//!    [`RunFilter`] picks scenarios by tag; a feature's tags count as its
//!    scenarios' tags. The `_async` runners do the same with an
//!    [`AsyncStepRegistry`].
//!
//! ```
//! use seleniumbase_rs::behave::{run_feature_text, RunFilter, StepRegistry};
//!
//! let mut registry = StepRegistry::new();
//! registry.register_regex(r#"^Given I have (\d+) cats$"#, Box::new(|_, caps| {
//!     caps[0].parse::<u32>().map(|_| ()).map_err(|e| e.to_string())
//! }));
//! registry.register("Then they are happy", Box::new(|_, _| Ok(())));
//!
//! let feature = "@pets\nFeature: cats\n\
//!                Scenario Outline: count <n>\nGiven I have <n> cats\nThen they are happy\n\
//!                Examples:\n| n |\n| 1 |\n| 2 |\n";
//! let results = run_feature_text(feature, &registry, &RunFilter::all());
//! assert_eq!(results.len(), 2);
//! assert!(results.iter().all(|r| r.passed));
//! ```

pub mod async_steps;
pub mod common_steps;
pub mod helper;
pub mod runner;
pub mod steps;

pub use async_steps::{AsyncStepHandler, AsyncStepRegistry};
pub use common_steps::CommonSteps;
pub use helper::{
    expand_scenario_outline, parse_feature, parse_scenarios, Feature, Scenario, Step,
};
pub use runner::{
    run_feature_file, run_feature_file_async, run_feature_file_with_filter,
    run_feature_file_with_filter_async, run_feature_text, run_feature_text_async, RunFilter,
    ScenarioResult,
};
pub use steps::{StepCaptures, StepHandler, StepRegistry};
