//! How `BaseCase` (WebDriver) and the pure CDP engine read and click form
//! fields, against a real headless Chrome.
//!
//! The page is `tests/fixtures/form_fields.html`, served from a loopback port:
//! an input and a textarea, a select, radio buttons, TodoMVC-style checkboxes
//! at zero opacity, a range input whose page listens for the mouse, a form
//! that Enter submits, and a button that appears after a moment. Each test
//! checks what Python SeleniumBase does with the same page. They need Chrome
//! (and chromedriver for the `BaseCase` ones), so they are ignored by default;
//! run the binary on its own:
//!
//! ```text
//! cargo test --features "mcp-server test-util testing" --test form_fields_chrome \
//!     -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

const PAGE: &str = include_str!("fixtures/form_fields.html");

/// Answers one connection with the page.
fn answer(mut stream: TcpStream) {
    let mut buffer = [0_u8; 4096];
    let _ = stream.read(&mut buffer);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
        PAGE.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Serves the page on a loopback port and returns its URL.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let url = format!("http://{}/", listener.local_addr().expect("bound address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            // A thread each: Chrome opens idle connections ahead of time.
            std::thread::spawn(move || answer(stream));
        }
    });
    url
}

/// The WebDriver engine, headless.
fn headless() -> BrowserConfig {
    BrowserConfig::default().with_headless(true)
}

/// The pure CDP engine, headless.
fn cdp_options() -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .expect("the launch options are valid")
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn text_assertions_read_the_value_of_an_input_and_a_textarea() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(1).await?;
        sb.type_text("#name", "This is Automated").await?;
        sb.type_text("#notes", "Testing Time!\n").await?;

        sb.assert_text("This is Automated", "#name").await?;
        sb.assert_exact_text("This is Automated", "#name").await?;
        sb.assert_text("Testing Time!", "#notes").await?;
        sb.assert_exact_text("Testing Time!", "#notes").await?;
        sb.assert_text("Plain text", "#plain").await?;

        assert!(sb.assert_text("Plain text", "#name").await.is_err());
        assert!(sb.assert_exact_text("Automated", "#name").await.is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn cdp_expectations_read_the_value_of_an_input_and_a_textarea() -> Result<()> {
    let url = serve();
    let browser = Browser::launch(cdp_options()).await?;
    let page = browser.default_page().await?;
    page.goto(url).await?;
    page.locator("#name").type_text("This is Automated").await?;
    page.locator("#notes").type_text("Testing Time!\n").await?;

    page.locator("#name")
        .expect()
        .to_contain_text("This is Automated")
        .await?;
    page.locator("#name")
        .expect()
        .to_have_text("This is Automated")
        .await?;
    page.locator("#notes")
        .expect()
        .to_contain_text("Testing Time!")
        .await?;
    page.locator("#plain")
        .expect()
        .to_have_text("Plain text")
        .await?;
    let wrong = page
        .locator("#name")
        .expect()
        .timeout(Duration::from_millis(300))
        .to_contain_text("Plain text")
        .await;
    assert!(wrong.is_err());
    assert_eq!(page.locator("#name").text().await?, "This is Automated");

    browser.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn get_attribute_falls_back_to_the_dom_property() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(1).await?;

        // A select has no value attribute: Selenium gives the chosen value.
        let chosen = sb.get_attribute("#pick", "value").await?;
        assert_eq!(chosen.as_deref(), Some("v2"));
        sb.assert_attribute("#pick", "value", "v2").await?;
        assert!(sb.assert_attribute("#pick", "value", "v1").await.is_err());

        // An input's value attribute is what the page wrote; its value is
        // what was typed.
        sb.type_text("#name", "typed").await?;
        let typed = sb.get_attribute("#name", "value").await?;
        assert_eq!(typed.as_deref(), Some("typed"));

        // An attribute the element has is what the call returns.
        assert_eq!(
            sb.get_attribute("#pick", "id").await?.as_deref(),
            Some("pick")
        );
        // A boolean property that is false is no attribute, as in Selenium.
        let unselected = sb
            .get_attribute("#pick option[value=\"v1\"]", "selected")
            .await?;
        assert_eq!(unselected, None);
        assert_eq!(sb.get_attribute("#pick", "data-missing").await?, None);
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn get_attribute_gives_what_a_field_holds_now_not_the_value_of_its_markup() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(1).await?;

        // Before any typing the property is the markup's text.
        assert_eq!(
            sb.get_attribute("#prefilled", "value").await?.as_deref(),
            Some("")
        );
        assert_eq!(
            sb.get_attribute("#greeting", "value").await?.as_deref(),
            Some("Hello")
        );

        // After typing, the markup still says value="" and value="Hello"; the
        // driver's attribute command keeps answering with those, Selenium's
        // `get_attribute` with what the field holds.
        sb.type_text("#prefilled", "typed").await?;
        sb.type_text("#greeting", "Goodbye").await?;
        assert_eq!(
            sb.get_attribute("#prefilled", "value").await?.as_deref(),
            Some("typed")
        );
        assert_eq!(
            sb.get_attribute("#greeting", "value").await?.as_deref(),
            Some("Goodbye")
        );
        sb.assert_attribute("#prefilled", "value", "typed").await?;
        assert!(sb
            .assert_attribute("#prefilled", "value", "")
            .await
            .is_err());

        // `checked` is in the markup of the second box; once the user clears
        // it the property says no, and so does `get_attribute`.
        let walk = "ul.todo-list li:nth-child(2) input.toggle";
        assert_eq!(
            sb.get_attribute(walk, "checked").await?.as_deref(),
            Some("true")
        );
        sb.uncheck_if_checked(walk).await?;
        assert_eq!(sb.get_attribute(walk, "checked").await?, None);
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn get_select_options_gives_the_text_the_value_or_the_index_of_each_option() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        assert_eq!(
            sb.get_select_options("#pick").await?,
            ["One", "Two", "Three"]
        );
        let values = sb.get_select_options("#pick").attribute("value").await?;
        assert_eq!(values, ["v1", "v2", "v3"]);
        let indexes = sb.get_select_options("#pick").attribute("index").await?;
        assert_eq!(indexes, ["0", "1", "2"]);
        assert!(sb
            .get_select_options("#pick")
            .attribute("title")
            .await
            .is_err());
        // Not a select.
        sb.set_timeout(1).await?;
        assert!(sb.get_select_options("#name").await.is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn checkboxes_at_zero_opacity_or_not_displayed_can_be_checked_and_unchecked() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(2).await?;
        let first = "ul.todo-list li:nth-child(1) input.toggle";
        let second = "ul.todo-list li:nth-child(2) input.toggle";

        // TodoMVC's toggle is at zero opacity: Python raises the opacity a
        // little and clicks it.
        sb.check_if_unchecked(first).await?;
        assert!(sb.is_checked(first).await?);
        // Already checked: no second click.
        sb.check_if_unchecked(first).await?;
        sb.uncheck_if_checked(second).await?;
        assert!(!sb.is_checked(second).await?);
        // A box that is not displayed at all is clicked with a script.
        sb.check_if_unchecked("#hidden-box").await?;
        assert!(sb.is_checked("#hidden-box").await?);

        assert_eq!(
            sb.get_text("#clicked").await?,
            "toggle0;toggle1;hidden-box;"
        );
        // The opacity is put back to what the stylesheet says.
        let opacity = sb
            .execute_script(
                "return document.querySelector('ul.todo-list input.toggle').style.opacity;",
            )
            .await?;
        assert_eq!(opacity, "");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn js_click_reads_contains_and_xpath_selectors_and_waits_for_the_element() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(4).await?;

        // `:contains()` and XPath: document.querySelector reads neither.
        sb.js_click("nav a:contains(\"Coffee Cart\")").await?;
        sb.js_click("//a[@id='coffee']").await?;
        // A button the page adds after a moment: js_click waits for it.
        assert!(!sb.is_element_present("#late").await?);
        sb.js_click("#late").await?;
        // An element that is not displayed can be clicked with a script.
        sb.js_click("#covered").await?;

        assert_eq!(
            sb.get_text("#clicked").await?,
            "coffee;coffee;late;covered;"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn set_value_moves_the_mouse_over_a_range_input_and_presses_enter_for_a_newline() -> Result<()>
{
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(2).await?;

        // The page's progress bar follows the slider on `mousemove` only.
        sb.assert_element("progress[value=\"50\"]").await?;
        sb.set_value("input#slider", "100").await?;
        sb.assert_element("progress[value=\"100\"]").await?;

        // A trailing newline presses Enter in the field: the form is submitted
        // with the text, not with the newline.
        sb.set_value("#search", "query\n").await?;
        sb.assert_exact_text("submitted:query", "#submitted")
            .await?;
        Ok(())
    })
    .await
}

// The tests of the other form-field behaviours follow.
