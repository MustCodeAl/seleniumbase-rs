//! Run a Gherkin feature file with step definitions written in Rust.
//!
//! Two runners are shown:
//!
//! 1. The sync [`StepRegistry`] and [`run_feature_file_with_filter`] parse a
//!    `.feature` file (Background, Scenario Outline with Examples, tags) and
//!    run every scenario. The steps are plain Rust closures that share state,
//!    so no browser is involved. This part always runs, offline.
//! 2. The [`AsyncStepRegistry`] owns a `BaseCase`, so its steps can drive a real
//!    browser. It has no feature-file runner of its own: you feed it step text.
//!    This part starts Chrome, so it only runs when you pass `--browser`.
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

// The async steps below borrow the shared `BaseCase` mutably across an await.
// That is sound here because the registry runs one step at a time.
#![expect(
    clippy::await_holding_refcell_ref,
    reason = "async steps run one at a time, so the BaseCase is never borrowed twice"
)]

mod local_site;

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::behave::{
    run_feature_file_with_filter, AsyncStepRegistry, RunFilter, StepRegistry,
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

/// Registers browser steps on a `BaseCase`, then runs them in order.
async fn run_steps_in_a_browser() -> Result<(), Box<dyn Error>> {
    let site = Site::serve_demo()?;
    let mut registry = AsyncStepRegistry::new(BaseCase::new(BrowserConfig::default()).await?);

    // `I open "..."`, `I go back`, `I click "..."` and `I type "..." into "..."`.
    registry.register_common_browser_steps();
    registry.register_regex(
        r#"^I verify the title is "(.+?)"$"#,
        Box::new(|bc, _step, captures| {
            let expected = captures[0].clone();
            Box::pin(async move {
                let title = bc
                    .borrow_mut()
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
    // The registry owns the BaseCase, so closing it is a step too.
    registry.register(
        "I close the browser",
        Box::new(|bc, _step, _captures| {
            Box::pin(async move {
                bc.borrow_mut()
                    .quit()
                    .await
                    .map_err(|error| format!("could not close the browser: {error}"))
            })
        }),
    );

    let steps = [
        format!(r#"I open "{}""#, site.url(DEMO_PATH)),
        r#"I verify the title is "Web Testing Page""#.to_owned(),
        r##"I type "behave" into "#myTextInput""##.to_owned(),
        r##"I click "#myButton""##.to_owned(),
    ];
    println!("\nBrowser-backed steps:");
    let mut failure = None;
    for step in &steps {
        match registry.run(step).await {
            Ok(()) => println!("  ok   {step}"),
            Err(error) => {
                failure = Some(format!("step `{step}` failed: {error}"));
                break;
            }
        }
    }

    // Close the browser even when a step failed.
    registry.run("I close the browser").await?;
    match failure {
        Some(message) => Err(message.into()),
        None => Ok(()),
    }
}
