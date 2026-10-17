//! The convenience macros: shorthand for configuration, selectors,
//! fingerprints and the most common browser actions.
//!
//! Part 1 builds values with `selector!`, `fingerprint!`, `uc_config!`,
//! `cdp_config!` and `sb_config!`; it needs no browser.
//!
//! Part 2 drives a page with the action macros (`sb_open!`, `sb_fill_form!`,
//! `sb_click!`, `sb_assert_text!`, `sb_wait_for!`, ...). Each action macro has
//! two forms:
//!
//! * `sb_click!(sb, "#go")?` expands to the `Result` of the call, so you handle
//!   it with `?` like any other error.
//! * `sb_click!(sb, "#go", "click go")` adds a message and expands to
//!   `.expect("click go")`, which panics when the call fails. That is the form
//!   for `#[tokio::test]` bodies, where a panic is the test failure.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example macros_demo
//! ```
//!
//! Part 2 needs Chrome and a matching chromedriver on your `PATH` (`sbrs
//! doctor` checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{
    cdp_config, fingerprint, run_browser_test, sb_assert_text, sb_assert_title_contains,
    sb_assert_url_contains, sb_assert_visible, sb_click, sb_config, sb_fill_form, sb_hover, sb_js,
    sb_open, sb_screenshot, sb_scroll_to, sb_select, sb_wait_for, selector, uc_config, BaseCase,
    BrowserConfig, DriverMode, Result, SeleniumBaseError,
};

#[tokio::main]
async fn main() -> Result<()> {
    configuration_macros();

    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), async |sb| {
        action_macros(sb, url).await
    })
    .await
}

/// Part 1: macros that only build values.
fn configuration_macros() {
    println!("uc_config!()  -> mode {:?}", uc_config!().mode);
    println!("cdp_config!() -> mode {:?}", cdp_config!().mode);

    let custom = sb_config!(headless: true, mode: DriverMode::Cdp);
    println!(
        "sb_config!    -> mode {:?}, headless {}",
        custom.mode, custom.headless
    );

    let persona = fingerprint!(windows);
    println!(
        "fingerprint!  -> {} / {}",
        persona.os_type().platform(),
        persona.user_agent().as_str().unwrap_or("no user agent")
    );

    println!("selector!     -> {:?}", selector!(xpath, "//h1"));
}

/// Part 2: macros that drive the browser.
async fn action_macros(sb: &mut BaseCase, url: String) -> Result<()> {
    sb_open!(sb, &url)?;
    sb_assert_title_contains!(sb, "Web Testing")?; // `sb_assert_title!` wants the whole title

    sb_fill_form!(sb, "#username" => "alice", "#password" => "not-a-real-password")?;
    sb_click!(sb, "#submit")?;
    sb_assert_text!(sb, "Signed in as alice", "#status")?;

    sb_select!(sb, "#country", "Germany")?;
    sb_wait_for!(sb, "#late")?; // waits for the element the page adds later
    sb_hover!(sb, "nav .menu")?;
    sb_assert_visible!(sb, "#submenu")?;
    sb_scroll_to!(sb, "#footer")?;

    let title = sb_js!(sb, "return document.title")?;
    println!("title via sb_js!: {title}");

    let dir = tempfile::tempdir()?;
    let screenshot = dir.path().join("macros_demo.png");
    sb_screenshot!(sb, &screenshot)?;
    if !screenshot.is_file() {
        return Err(SeleniumBaseError::AssertionFailed(
            "sb_screenshot! wrote no file".to_owned(),
        ));
    }

    // The message form panics instead of returning an error.
    sb_assert_url_contains!(
        sb,
        "127.0.0.1",
        "the page is served from the loopback address"
    );
    println!("every macro did its job");
    Ok(())
}
