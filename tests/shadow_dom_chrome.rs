//! `::shadow` selectors in both engines, against a real headless Chrome.
//!
//! The page is `tests/fixtures/shadow_dom.html`, served from a loopback port:
//! open shadow roots three levels deep, a host that only slots its content,
//! a shadow tree that arrives after a moment, a status that changes after a
//! moment, and a closed root. Python SeleniumBase reads `host::shadow inner`
//! in every method that takes a selector (`click`, `type`, `get_text`,
//! `assert_element`, `assert_text`, `wait_for_element_visible`, ...), waits for
//! the element, and gives the rendered text. The tests are ignored by default
//! because they need Chrome (and chromedriver for the `BaseCase` ones); run the
//! binary on its own:
//!
//! ```text
//! cargo test --features "mcp-server test-util testing" --test shadow_dom_chrome \
//!     -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, State};
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

const PAGE: &str = include_str!("fixtures/shadow_dom.html");

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
async fn the_regular_methods_read_a_shadow_selector() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;

        sb.assert_element("app-shell::shadow #title").await?;
        sb.assert_element_present("app-shell::shadow #go").await?;
        sb.assert_text("App shell", "app-shell::shadow #title")
            .await?;
        // Python also reads the combinator with a space before it.
        sb.assert_exact_text("App shell", "app-shell ::shadow #title")
            .await?;
        assert_eq!(sb.get_text("app-shell::shadow #title").await?, "App shell");

        sb.type_text("app-shell::shadow #name", "Ada").await?;
        sb.click("app-shell::shadow #go").await?;
        sb.assert_exact_text("Hello, Ada", "app-shell::shadow #greeting")
            .await?;
        assert_eq!(
            sb.get_attribute("app-shell::shadow #go", "data-kind")
                .await?
                .as_deref(),
            Some("primary")
        );
        sb.assert_attribute("app-shell::shadow #go", "data-kind", "primary")
            .await?;

        assert!(sb.is_element_present("app-shell::shadow #go").await?);
        assert!(sb.is_element_visible("app-shell::shadow #go").await?);
        assert!(
            !sb.is_element_visible("app-shell::shadow #hidden-button")
                .await?
        );
        assert!(
            sb.is_element_present("app-shell::shadow #hidden-button")
                .await?
        );
        assert!(!sb.is_element_present("app-shell::shadow #nope").await?);
        assert!(
            sb.is_text_visible("Hello", "app-shell::shadow #greeting")
                .await?
        );
        sb.wait_for_element_visible("app-shell::shadow #go", 2)
            .await?;
        sb.wait_for_element_present("app-shell::shadow #hidden-button", 2)
            .await?;

        // The light DOM is untouched by all of this.
        sb.assert_exact_text("Shadow DOM fixture", "#light").await?;
        // A host with no shadow root, and an element that is not there, fail.
        sb.set_timeout(1).await?;
        assert!(sb.click("#light::shadow #go").await.is_err());
        assert!(sb.click("app-shell::shadow #nope").await.is_err());
        assert!(sb.get_text("nope-host::shadow #go").await.is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_chain_crosses_three_shadow_roots() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;
        let two = "app-shell::shadow user-card::shadow span.badge";
        let three = "app-shell::shadow user-card::shadow badge-pill::shadow b.pill";

        sb.assert_exact_text("Admin", two).await?;
        sb.assert_exact_text("Gold", three).await?;
        assert_eq!(sb.get_text(three).await?, "Gold");
        assert_eq!(
            sb.get_attribute(three, "data-tier").await?.as_deref(),
            Some("gold")
        );
        sb.type_text("app-shell::shadow user-card::shadow input.note", "hi")
            .await?;
        assert_eq!(
            sb.get_attribute("app-shell::shadow user-card::shadow input.note", "value")
                .await?
                .as_deref(),
            Some("hi")
        );
        assert_eq!(
            sb.find_elements("app-shell::shadow li.item").await?.len(),
            2
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_container_that_only_slots_its_content_gives_its_rendered_text() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;
        // Python's get_text reads what the browser shows, slotted text
        // included; the container's own textContent is empty.
        assert_eq!(
            sb.get_text("fancy-tabs::shadow #panels").await?,
            "Content Panel 1"
        );
        sb.assert_exact_text("Content Panel 1", "fancy-tabs::shadow #panels")
            .await?;
        assert_eq!(
            sb.shadow_get_text("fancy-tabs::shadow #panels").await?,
            "Content Panel 1"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn the_checks_wait_for_content_that_arrives_late() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(5).await?;
        // The status says "Loading" for the first 800 ms, and the shadow root
        // of the late host does not exist before then.
        sb.assert_text("Ready", "app-shell::shadow #status").await?;
        sb.assert_element("late-host::shadow #late").await?;
        sb.assert_exact_text("Arrived", "late-host::shadow #late")
            .await?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn the_shadow_helpers_wait_with_the_timeout_of_the_case() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(5).await?;
        sb.assert_shadow_text("Ready", "app-shell::shadow #status")
            .await?;
        sb.assert_shadow_element("late-host::shadow #late").await?;

        // And they give up when the timeout is out.
        sb.set_timeout(1).await?;
        let started = std::time::Instant::now();
        assert!(sb
            .assert_shadow_text("Never", "app-shell::shadow #status")
            .await
            .is_err());
        assert!(started.elapsed() >= Duration::from_millis(900));
        assert!(sb
            .assert_shadow_element("app-shell::shadow #nope")
            .await
            .is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn the_shadow_helpers_still_act_on_the_page() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;
        sb.shadow_type("app-shell ::shadow #name", "Grace").await?;
        sb.shadow_click("app-shell ::shadow #go").await?;
        assert_eq!(
            sb.shadow_get_text("app-shell ::shadow #greeting").await?,
            "Hello, Grace"
        );
        assert_eq!(
            sb.shadow_get_attribute("app-shell ::shadow user-card ::shadow .badge", "data-role")
                .await?
                .as_deref(),
            Some("admin")
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_selector_that_ends_on_a_shadow_root_is_refused_at_once() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(5).await?;
        let started = std::time::Instant::now();
        let error = sb
            .get_text("app-shell ::shadow")
            .await
            .expect_err("a selector must end inside a shadow root")
            .to_string();
        assert!(error.contains("cannot end on a shadow root"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(3), "it waited");
        assert!(sb.click("app-shell::shadow").await.is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_closed_root_is_reached_the_way_python_reaches_it() -> Result<()> {
    let url = serve();
    run_browser_test(headless(), async |sb| {
        sb.open(&url).await?;
        sb.set_timeout(3).await?;
        // WebDriver's Get Element Shadow Root gives a closed root too.
        sb.assert_exact_text("Closed text", "closed-host::shadow #secret")
            .await?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn the_cdp_engine_reads_a_shadow_selector() -> Result<()> {
    let url = serve();
    let browser = Browser::launch(cdp_options()).await?;
    let page = browser.default_page().await?;
    page.goto(url).await?;

    let title = page.locator("app-shell::shadow #title");
    title.wait_for(State::Visible).await?;
    assert_eq!(title.text().await?, "App shell");
    assert_eq!(
        page.locator("app-shell ::shadow #title").text().await?,
        "App shell"
    );

    page.locator("app-shell::shadow #name")
        .type_text("Ada")
        .await?;
    page.locator("app-shell::shadow #go").click().await?;
    page.locator("app-shell::shadow #greeting")
        .expect()
        .to_have_text("Hello, Ada")
        .await?;
    assert_eq!(
        page.locator("app-shell::shadow #go")
            .attribute("data-kind")
            .await?
            .as_deref(),
        Some("primary")
    );

    let three = page.locator("app-shell::shadow user-card::shadow badge-pill::shadow b.pill");
    assert_eq!(three.text().await?, "Gold");
    assert_eq!(page.locator("app-shell::shadow li.item").count().await?, 2);

    // The rendered text of a container that only slots its content.
    assert_eq!(
        page.locator("fancy-tabs::shadow #panels").text().await?,
        "Content Panel 1"
    );

    // A step of a chain reads the combinator too.
    assert_eq!(
        page.locator("app-shell")
            .locator("user-card::shadow span.badge")
            .count()
            .await?,
        0,
        "user-card is inside the shadow root of app-shell, not in its light DOM"
    );
    assert_eq!(
        page.locator("app-shell::shadow user-card")
            .locator("span.badge")
            .count()
            .await?,
        0,
        "a plain step does not enter a shadow root by itself"
    );

    // Waits: the shadow root of the late host appears after a moment.
    page.locator("late-host::shadow #late")
        .wait_for(State::Visible)
        .await?;
    page.locator("app-shell::shadow #status")
        .expect()
        .to_have_text("Ready")
        .await?;

    assert!(!page.locator("app-shell::shadow #nope").exists().await?);
    assert!(!page.locator("#light::shadow #go").exists().await?);
    assert!(
        !page
            .locator("app-shell::shadow #hidden-button")
            .is_visible()
            .await?
    );
    assert!(
        page.locator("app-shell::shadow #hidden-button")
            .exists()
            .await?
    );

    // A selector that ends on a shadow root is refused.
    let error = page
        .locator("app-shell ::shadow")
        .text()
        .await
        .expect_err("a selector must end inside a shadow root")
        .to_string();
    assert!(error.contains("cannot end on a shadow root"), "{error}");

    browser.close().await?;
    Ok(())
}
