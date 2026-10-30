//! Check HTML for common structure and accessibility defects.
//!
//! `HtmlInspector::inspect` is a static check: it parses a string of HTML and
//! returns an issue for each defect (a missing `lang`, an image without `alt`,
//! an input without a label, duplicate ids, skipped heading levels, and so on).
//! Every issue has a stable `rule` name that is safe to allow-list in CI, and a
//! severity.
//!
//! The example inspects a deliberately flawed page and a clean one, and exits
//! with an error unless the first reports problems and the second reports none.
//! That part needs no browser.
//!
//! With `--browser` it then does the same on a live page: the pages are served
//! from `127.0.0.1`, and `BaseCase::inspect_html` (or `assert_no_html_issues`,
//! which fails the test directly) inspects the source of the page the browser
//! has open.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example html_inspector             # static check, no browser
//! cargo run --example html_inspector -- --browser # also inspect live pages
//! ```
//!
//! The browser part needs Chrome and a matching chromedriver on your `PATH`
//! (`sbrs doctor` checks both). Neither part needs the network.

mod local_site;

use local_site::{ensure, Site};
use seleniumbase_rs::api::html_inspector::{HtmlInspector, InspectionResult, InspectionSeverity};
use seleniumbase_rs::{run_browser_test, tokio, BaseCase, BrowserConfig, Result};

/// Everything wrong with this page is on purpose.
const FLAWED_PAGE: &str = r#"<html>
<head></head>
<body>
  <h1>Welcome</h1>
  <h3>This heading skips a level</h3>
  <img src="banner.png">
  <input id="email" type="text">
  <button></button>
  <div id="dup">first</div>
  <div id="dup">second</div>
  <a href="/more">click here</a>
</body>
</html>"#;

const CLEAN_PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Contact us</title>
</head>
<body>
  <a href="#maincontent">Skip to main content</a>
  <main id="maincontent">
    <h1>Contact us</h1>
    <img src="logo.png" alt="Company logo">
    <form>
      <label for="email">Email address</label>
      <input id="email" type="email">
      <button type="submit">Send message</button>
    </form>
    <p><a href="/docs">Read the documentation</a></p>
  </main>
</body>
</html>"##;

#[tokio::main]
async fn main() -> Result<()> {
    inspect_strings()?;

    if std::env::args().any(|arg| arg == "--browser") {
        let site = Site::serve(&[("/flawed", FLAWED_PAGE), ("/clean", CLEAN_PAGE)])?;
        let flawed_url = site.url("/flawed");
        let clean_url = site.url("/clean");
        run_browser_test(BrowserConfig::default(), async |sb| {
            inspect_live(sb, flawed_url, clean_url).await
        })
        .await?;
    } else {
        println!("\n(pass --browser to inspect the same pages in a live browser)");
    }
    Ok(())
}

/// Part 1: inspect HTML text, with no browser.
fn inspect_strings() -> Result<()> {
    let flawed = HtmlInspector::inspect(FLAWED_PAGE)?;
    print_report("flawed page", &flawed);
    ensure(
        flawed.has_errors(),
        "the flawed page has error-level issues",
    )?;

    let clean = HtmlInspector::inspect(CLEAN_PAGE)?;
    print_report("clean page", &clean);
    ensure(clean.is_clean(), "the clean page has no issues")
}

/// Part 2: inspect the page the browser has open.
async fn inspect_live(sb: &mut BaseCase, flawed_url: String, clean_url: String) -> Result<()> {
    sb.open(&flawed_url).await?;
    let flawed = sb.inspect_html().await?;
    print_report("live flawed page", &flawed);
    ensure(
        flawed.has_errors(),
        "the live flawed page has error-level issues",
    )?;
    ensure(
        sb.assert_no_html_issues().await.is_err(),
        "assert_no_html_issues fails on the flawed page",
    )?;

    sb.open(&clean_url).await?;
    sb.assert_no_html_issues().await?;
    println!("live clean page: assert_no_html_issues passed");
    Ok(())
}

fn print_report(name: &str, inspection: &InspectionResult) {
    if inspection.is_clean() {
        println!("{name}: no HTML inspector issues found.");
        return;
    }
    println!("{name}: {} issue(s)", inspection.issues.len());
    for issue in &inspection.issues {
        let severity = match issue.severity {
            InspectionSeverity::Error => "error",
            InspectionSeverity::Warning => "warning",
        };
        println!(
            "  [{severity}] {}: {} ({})",
            issue.rule,
            issue.message,
            issue.selector.as_deref().unwrap_or("whole document")
        );
    }
}
