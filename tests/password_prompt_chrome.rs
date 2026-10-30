//! Chrome's password prompts must not take the keyboard from the page.
//!
//! Logging in to saucedemo.com with its well-known password (`secret_sauce`)
//! makes Chrome check the password against its list of leaked ones and show a
//! "data breach" prompt, a piece of the browser's own interface. While that
//! prompt is up, Chrome sends the keys that `Input.dispatchKeyEvent` delivers to
//! it and not to the page, and answers the command at once as if all was well:
//! `type_text` returns `Ok` and the field stays empty, or keeps only the first
//! characters. Python SeleniumBase turns the check off with the profile
//! preference `profile.password_manager_leak_detection` (and the two other
//! password preferences); this crate does the same for the browsers it starts.
//!
//! Two kinds of test:
//!
//! - `*_starts_with_the_password_prompts_off` read the preferences of a browser
//!   that was just started, from `chrome://prefs-internals`. They need a Chrome
//!   and no network.
//! - `typing_lands_*_after_a_login_with_a_leaked_password` log in to
//!   saucedemo.com, give the prompt time to come up, and type into the checkout
//!   form. They need a Chrome and the network. Before the preferences were set,
//!   no key reached the page here at all, in either engine.
//!
//! All are `#[ignore]`d. Run them one binary at a time:
//!
//! ```text
//! cargo test --features "mcp-server test-util testing" --test password_prompt_chrome \
//!     -- --ignored --test-threads=1
//! ```

use std::time::Duration;

use seleniumbase_rs::api::runner::run_cdp_test;
use seleniumbase_rs::browser::options::WindowSize;
use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

/// How long the "data breach" prompt is given to come up, after the login that
/// triggers it. Chrome asks Google first, which takes a few hundred
/// milliseconds on a good connection.
const PROMPT_TIME: Duration = Duration::from_millis(1500);

fn cdp_options() -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1440, 1880)
        .build()
        .expect("the launch options are valid")
}

fn webdriver_config() -> BrowserConfig {
    BrowserConfig::default()
        .with_headless(true)
        .with_window_size(WindowSize::new(1440, 1880))
}

/// `chrome://prefs-internals` shows the browser's preferences as JSON, each
/// one as `{ "metadata": [...], "value": ... }`. This reads the value of the
/// two password preferences Chrome registers, in the order of
/// [`WANTED_PREFERENCES`]. (`profile.password_manager_enabled`, which Python
/// sets as well, is a name current Chrome no longer registers, so it is not
/// listed.)
const READ_PREFERENCES: &str = "(() => { \
    const prefs = JSON.parse(document.documentElement.innerText); \
    const value = (...path) => path.reduce((node, key) => node && node[key], prefs); \
    return JSON.stringify([ \
        value('profile', 'password_manager_leak_detection', 'value'), \
        value('credentials_enable_service', 'value') ]); \
    })()";

/// Both off: the leaked-password check and the offer to save a login.
const WANTED_PREFERENCES: &str = "[false,false]";

#[tokio::test]
#[ignore = "real headless Chrome; reads chrome://prefs-internals"]
async fn a_pure_cdp_browser_starts_with_the_password_prompts_off() -> Result<()> {
    run_cdp_test(cdp_options(), async |page| {
        page.goto("chrome://prefs-internals").await?;
        let values = page.evaluate(READ_PREFERENCES).await?;
        assert_eq!(values.as_str(), Some(WANTED_PREFERENCES));
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome and chromedriver; reads chrome://prefs-internals"]
async fn a_webdriver_browser_starts_with_the_password_prompts_off() -> Result<()> {
    run_browser_test(webdriver_config(), async |sb| {
        sb.open("chrome://prefs-internals").await?;
        let values = sb
            .execute_script(&format!("return {READ_PREFERENCES};"))
            .await?;
        assert_eq!(values.as_str(), Some(WANTED_PREFERENCES));
        Ok(())
    })
    .await
}

/// The values the three checkout fields hold, read by script.
const READ_FIELDS: &str = "JSON.stringify([document.querySelector('#first-name').value, \
    document.querySelector('#last-name').value, document.querySelector('#postal-code').value])";

const WANTED_FIELDS: &str = r#"["SeleniumBase","Automation","77123"]"#;

#[tokio::test]
#[ignore = "real headless Chrome; opens saucedemo.com"]
async fn typing_lands_in_a_pure_cdp_page_after_a_login_with_a_leaked_password() -> Result<()> {
    run_cdp_test(cdp_options(), async |page| {
        page.goto("https://www.saucedemo.com").await?;
        page.locator("#user-name")
            .type_text("standard_user")
            .await?;
        page.locator("#password")
            .type_text("secret_sauce\n")
            .await?;
        page.locator("div.inventory_list")
            .expect()
            .to_be_visible()
            .await?;
        page.locator("button[name*=\"backpack\"]").click().await?;
        page.locator("#shopping_cart_container a").click().await?;
        page.locator("button#checkout").click().await?;
        page.locator("input#first-name")
            .expect()
            .to_be_visible()
            .await?;
        tokio::time::sleep(PROMPT_TIME).await;
        page.locator("input#first-name")
            .type_text("SeleniumBase")
            .await?;
        page.locator("input#last-name")
            .type_text("Automation")
            .await?;
        page.locator("input#postal-code").type_text("77123").await?;
        let fields = page.evaluate(READ_FIELDS).await?;
        assert_eq!(fields.as_str(), Some(WANTED_FIELDS));
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome and chromedriver; opens saucedemo.com"]
async fn typing_lands_in_a_webdriver_page_after_a_login_with_a_leaked_password() -> Result<()> {
    run_browser_test(webdriver_config(), async |sb| {
        sb.open("https://www.saucedemo.com").await?;
        sb.type_text("#user-name", "standard_user").await?;
        sb.type_text("#password", "secret_sauce\n").await?;
        sb.assert_element("div.inventory_list").await?;
        sb.click("button[name*=\"backpack\"]").await?;
        sb.click("#shopping_cart_container a").await?;
        sb.click("button#checkout").await?;
        sb.assert_element("#first-name").await?;
        tokio::time::sleep(PROMPT_TIME).await;
        sb.type_text("#first-name", "SeleniumBase").await?;
        sb.type_text("#last-name", "Automation").await?;
        sb.type_text("#postal-code", "77123").await?;
        let fields = sb.execute_script(&format!("return {READ_FIELDS};")).await?;
        assert_eq!(fields.as_str(), Some(WANTED_FIELDS));
        Ok(())
    })
    .await
}
