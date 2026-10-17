//! Run a Gherkin feature file with step definitions written in Rust.
//!
//! Two runners are shown:
//!
//! 1. The sync [`StepRegistry`] and [`run_feature_file_with_filter`] parse a
//!    `.feature` file (Background, Scenario Outline with Examples, tags) and
//!    run every scenario. The steps are plain Rust closures that share state,
//!    so no browser is involved. This part always runs, offline.
//! 2. The [`AsyncStepRegistry`] owns a `BaseCase`, so its steps can drive a real
//!    browser. `run_feature_text_async` (and `run_feature_file_async`) run a
//!    feature against it as the sync runner does: Background, outlines, tags.
//!    The built-in steps understand SeleniumBase's own wording (`Open "..."`,
//!    `Click "..."`), and `register_step` adds one of yours that matches any
//!    keyword. This part starts Chrome, so it only runs when you pass
//!    `--browser`.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example behave_feature              # sync runner only
//! cargo run --example behave_feature -- --browser # also drive Chrome
//! ```
//!
//! The browser part needs Chrome and a matching chromedriver on your `PATH`
//! (`sbrs doctor` checks both); neither part needs the network. The process
//! exits with an error if a scenario that should pass fails, or the other way
//! round.

mod local_site;

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::behave::{
    run_feature_file_with_filter, run_feature_text_async, AsyncStepRegistry, RunFilter,
    StepRegistry,
};
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// `<term>` in the outline is replaced by each row of the Examples table.
const FEATURE: &str = r#"Feature: Search
  Background:
    Given the search page is loaded

  @smoke
  Scenario: search for Rust
    When I search for "Rust"
    Then I see results for "Rust"

  Scenario Outline: search for several terms
    When I search for "<term>"
    Then I see results for "<term>"

    Examples:
      | term   |
      | cargo  |
      | clippy |

  @wip
  Scenario: unfinished work
    When I search for "unfinished"
    Then I see results for "something else"
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    run_feature_with_sync_steps()?;

    if std::env::args().any(|arg| arg == "--browser") {
        run_steps_in_a_browser().await?;
    } else {
        println!("\n(pass --browser to also run browser-backed async steps)");
    }
    Ok(())
}

/// Step definitions that keep the last search in shared state, so the `Then`
/// step has something to check.
fn search_steps() -> StepRegistry {
    let last_search = Rc::new(RefCell::new(String::new()));
    let mut registry = StepRegistry::new();

    // Exact steps match the keyword and text as written in the feature file.
    let state = Rc::clone(&last_search);
    registry.register(
        "Given the search page is loaded",
        Box::new(move |_, _| {
            state.borrow_mut().clear();
            Ok(())
        }),
    );

    // Regex steps receive the capture groups. The pattern sees the keyword too.
    let state = Rc::clone(&last_search);
    registry.register_regex(
        r#"^When I search for "(.+?)"$"#,
        Box::new(move |_, captures| {
            state.borrow_mut().clone_from(&captures[0]);
            Ok(())
        }),
    );

    registry.register_regex(
        r#"^Then I see results for "(.+?)"$"#,
        Box::new(move |_, captures| {
            let searched = last_search.borrow();
            if *searched == captures[0] {
                Ok(())
            } else {
                Err(format!(
                    "searched for '{searched}' but the results are for '{}'",
                    captures[0]
                ))
            }
        }),
    );
    registry
}

fn describe(passed: bool) -> &'static str {
    if passed {
        "PASS"
    } else {
        "FAIL"
    }
}

fn run_feature_with_sync_steps() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("search.feature");
    std::fs::write(&path, FEATURE)?;
    let registry = search_steps();

    println!("Scenarios that are ready (everything except @wip):");
    let ready = RunFilter {
        include_tags: Vec::new(),
        exclude_tags: vec!["@wip".to_owned()],
    };
    let results = run_feature_file_with_filter(&path, &registry, &ready)?;
    for result in &results {
        println!("  {} {}", describe(result.passed), result.name);
    }
    if results.len() != 3 || results.iter().any(|result| !result.passed) {
        return Err(format!("expected 3 passing scenarios, got {results:?}").into());
    }

    println!("Scenarios tagged @wip, which are wrong on purpose:");
    let wip = RunFilter {
        include_tags: vec!["@wip".to_owned()],
        exclude_tags: Vec::new(),
    };
    let results = run_feature_file_with_filter(&path, &registry, &wip)?;
    for result in &results {
        println!(
            "  {} {} -> {}",
            describe(result.passed),
            result.name,
            result.messages.join("; ")
        );
    }
    if results.len() != 1 || results[0].passed {
        return Err("the @wip scenario should have failed".into());
    }
    Ok(())
}

/// Registers browser steps on a `BaseCase`, then runs a feature against them.
async fn run_steps_in_a_browser() -> Result<(), Box<dyn Error>> {
    let site = Site::serve_demo()?;
    let mut registry = AsyncStepRegistry::new(BaseCase::new(BrowserConfig::default()).await?);

    // `Open "..."`, `Go back`, `Click "..."`, `Type text "..." into "..."` and
    // the `I open "..."` style of the same steps.
    registry.register_common_browser_steps();
    // `register_step` takes the text without its keyword and matches `Then`,
    // `And` or none.
    registry.register_step(
        r#"I verify the title is "(.+?)""#,
        Box::new(|bc, _step, captures| {
            let expected = captures[0].clone();
            Box::pin(async move {
                let title = bc
                    .lock()
                    .await
                    .get_title()
                    .await
                    .map_err(|error| format!("could not read the title: {error}"))?;
                if title == expected {
                    Ok(())
                } else {
                    Err(format!("expected title '{expected}', got '{title}'"))
                }
            })
        }),
    );

    let feature = format!(
        r##"Feature: Demo page
  Scenario: type and click
    Given Open "{}"
    Then I verify the title is "Web Testing Page"
    When Type text "behave" into "#myTextInput"
    And Click "#myButton"
"##,
        site.url(DEMO_PATH)
    );
    println!("\nBrowser-backed scenarios:");
    let results = run_feature_text_async(&feature, &registry, &RunFilter::all()).await;
    for result in &results {
        println!("  {} {}", describe(result.passed), result.name);
    }

    // The registry owns the BaseCase until the run is over; take it back to
    // close the browser, even when a step failed.
    let mut base_case = registry
        .into_base_case()
        .map_err(|_| "a step is still holding the BaseCase")?;
    base_case.quit().await?;
    match results.iter().find(|result| !result.passed) {
        Some(failed) => {
            Err(format!("{} failed: {}", failed.name, failed.messages.join("; ")).into())
        }
        None => Ok(()),
    }
}
