//! Run JavaScript in the page and read the results back into Rust.
//!
//! `execute_script` runs your code as a function body, so use `return` to send
//! a value back; the value arrives as a `serde_json::Value`, whatever its JSON
//! type. The example reads a string, a number and an object, changes the page
//! and reads the change back, and runs an async script. There is no `sleep`:
//! the effect of each script is asserted instead.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example js_execution
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), |sb| Box::pin(demo(sb, url))).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    // A string: `Value::as_str` gives it back without the JSON quotes that
    // `Value::to_string` would add.
    let title = sb.execute_script("return document.title").await?;
    println!("Page title from JS: {}", title.as_str().unwrap_or_default());

    // A number.
    let inputs = sb
        .execute_script("return document.querySelectorAll('input').length")
        .await?;
    println!("Inputs on the page: {inputs}");

    // An object comes back as JSON.
    let summary = sb
        .execute_script("return {heading: document.querySelector('h1').textContent, links: document.links.length}")
        .await?;
    println!("Summary: {summary}");
    ensure(
        summary["heading"] == "Demo Page",
        "the object carries the heading",
    )?;

    // Change the page, then read the change back.
    sb.execute_script("document.body.style.backgroundColor = 'yellow'")
        .await?;
    let background = sb
        .execute_script("return getComputedStyle(document.body).backgroundColor")
        .await?;
    ensure(
        background.as_str() == Some("rgb(255, 255, 0)"),
        "the body is yellow",
    )?;
    println!("Background is now {background}");

    // An async script reports its result by calling the callback, which is
    // always the last argument.
    let answer = sb
        .execute_async_script(
            "const done = arguments[arguments.length - 1]; setTimeout(() => done(6 * 7), 100);",
        )
        .await?;
    ensure(answer == 42, "the async script answers 42")?;
    println!("Async script answered {answer}");
    Ok(())
}
