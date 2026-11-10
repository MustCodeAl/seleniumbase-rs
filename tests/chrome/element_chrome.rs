//! `get_text` of a field and a search from an `Element`, against a real
//! headless Chrome.
//!
//! The page is `tests/fixtures/element_find.html`, served from a loopback
//! port: a form with an input, a textarea and a labelled input, and a
//! container with a "Save" button inside it and another "Save" button outside
//! it. Python SeleniumBase gives the value of an `input` or a `textarea` for
//! `get_text`; a search from an element stays inside it, and the XPath of
//! `SelectorBuf::role`, `text`, `label` and `containing_text` is written for
//! that. The tests need Chrome and chromedriver, so they are ignored by
//! default; run the binary on its own:
//!
//! ```text
//! cargo test --features "mcp-server test-util testing" --test chrome element_chrome \
//!     -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use seleniumbase_rs::utils::selectors::SelectorBuf;
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

const PAGE: &str = include_str!("../fixtures/element_find.html");

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

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
#[serial_test::serial(chrome)]
async fn get_text_of_an_input_and_a_textarea_is_what_they_hold() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;

        // What the markup says first, then what was typed: the rendered text of an
        // `input` is empty, and a `textarea`'s own text is the markup's, so only the
        // value is what Python's `get_text` gives.
        assert_eq!(sb.text("#name").await?, "Ada Lovelace");
        assert_eq!(sb.text("#notes").await?, "First note");
        sb.type_text("#name", "Grace Hopper").await?;
        sb.type_text("#notes", "Typed note").await?;
        assert_eq!(sb.text("#name").await?, "Grace Hopper");
        assert_eq!(sb.text("#notes").await?, "Typed note");
        assert_eq!(sb.text_of_element("#name").await?, "Grace Hopper");

        // Anything else gives its rendered text.
        assert_eq!(sb.text("h1").await?, "Order");
        assert_eq!(sb.text("#inside").await?, "Save");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
#[serial_test::serial(chrome)]
async fn the_text_checks_read_the_value_of_a_field() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;
        sb.type_text("#name", "Grace Hopper").await?;
        sb.type_text("#notes", "Typed note").await?;

        // The driver renders nothing for an `input`, so only its value shows the text
        // that Python's `find_text` and `find_non_empty_text` look for.
        sb.find_text("Grace", "#name").await?;
        sb.find_exact_text("Grace Hopper", "#name").await?;
        sb.find_non_empty_text("#name").await?;
        sb.find_text("Typed", "#notes").await?;
        assert!(sb.is_non_empty_text_visible("#name").await?);
        sb.assert_non_empty_text("#name").await?;
        sb.wait_for_non_empty_text("#name", 1).await?;
        sb.find_exact_text("Grace", "#name").await.unwrap_err();
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
#[serial_test::serial(chrome)]
async fn a_role_or_text_selector_from_an_element_finds_only_what_is_inside_it() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;

        // From the page, `.//` is the whole document: both buttons.
        let save = SelectorBuf::role("button", Some("Save"), false);
        assert_eq!(sb.find_elements(&save.to_string()).await?.len(), 2);

        // From the container, only the one inside it.
        let panel = sb.find_element("#panel").await?;
        let inside = panel.find(save.clone()).await?;
        assert_eq!(inside.attribute("id").await?.as_deref(), Some("inside"));
        assert_eq!(panel.find_all(save).await?.len(), 1);

        for selector in [
            SelectorBuf::containing_text("Save", Some("button")),
            SelectorBuf::containing_text("Save", None),
            SelectorBuf::text("Save", false),
        ] {
            let name = selector.to_string();
            let found = panel.find_all(selector).await?;
            assert_eq!(found.len(), 1, "{name}");
            assert_eq!(found[0].attribute("id").await?.as_deref(), Some("inside"));
        }

        // A label names its control from anywhere in the document.
        let form = sb.find_element("#order").await?;
        let email = form.find(SelectorBuf::label("Email", false)).await?;
        assert_eq!(email.attribute("id").await?.as_deref(), Some("email"));
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
#[serial_test::serial(chrome)]
async fn a_union_with_a_branch_from_the_root_is_refused_and_one_from_the_element_works(
) -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;
        let panel = sb.find_element("#panel").await?;

        // `//button` would match the button outside the container too.
        let error = panel.find(".//p | //button").await.unwrap_err();
        assert!(error.to_string().contains("'//button'"), "{error}");
        assert!(panel.find_all("(.//p | //button)[1]").await.is_err());

        let both = panel.find_all(".//p | .//button").await?;
        assert_eq!(both.len(), 2);
        Ok(())
    })
    .await
}
