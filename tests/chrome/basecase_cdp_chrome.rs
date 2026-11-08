//! `BaseCase` on a direct DevTools attachment (`BrowserConfig::with_cdp_url`), against a real
//! headless Chrome: the pure CDP engine runs the whole `BaseCase` API, with no WebDriver.
//!
//! A browser is started with the pure CDP engine and a case attaches to it, as it would to the
//! stealthy browser or to an app. The page is `tests/fixtures/basecase_cdp.html`, served from a
//! loopback port: a form, a hidden element, buttons that open `alert`, `confirm` and `prompt`,
//! a box that answers the mouse, an `<iframe>` and a link to a second page. The design is
//! `docs/design/basecase_over_cdp.md`.
//!
//! The tests need a Chrome, so they are ignored by default; run the module on its own:
//!
//! ```text
//! SB_HEADLESS=true cargo test --features "mcp-server test-util testing bdd" --test chrome \
//!     basecase_cdp_chrome:: -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use seleniumbase_rs::json::json;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{BaseCase, BrowserConfig, Cookie, Result};

const PAGE: &str = include_str!("../fixtures/basecase_cdp.html");

/// Answers one connection with the page, whatever the path.
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

/// Starts a headless browser, attaches a case to it, runs `test` with the case and the URL of
/// the page, and closes both.
async fn run(test: impl AsyncFnOnce(&mut BaseCase, &str) -> Result<()>) -> Result<()> {
    let url = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()?;
    let browser = Browser::launch(options).await?;
    let mut sb = BaseCase::new(BrowserConfig::default().with_cdp_url(browser.http_url())).await?;
    sb.set_timeout(5).await?;
    let outcome = test(&mut sb, &url).await;
    let _ = sb.quit().await;
    browser.close().await?;
    outcome
}

#[tokio::test]
#[ignore = "real headless Chrome"]
#[serial_test::serial(chrome)]
async fn a_case_reads_the_page_and_moves_through_its_history() -> Result<()> {
    run(async |sb, url| {
        sb.open(url).await?;
        assert_eq!(sb.get_title().await?, "BaseCase over CDP");
        sb.assert_title("BaseCase over CDP").await?;
        assert_eq!(sb.get_text("#title").await?, "BaseCase over CDP");
        assert_eq!(sb.get_text("#greeting").await?, "Hello world");
        assert_eq!(
            sb.get_text("#name").await?,
            "Ada",
            "an input's text is its value"
        );
        assert_eq!(
            sb.get_attribute("#name", "data-kind").await?.as_deref(),
            Some("person")
        );
        assert!(sb.get_page_source().await?.contains("id=\"greeting\""));
        sb.assert_text("Hello", "#greeting").await?;
        sb.assert_element("#docs").await?;

        // A hidden element is there, not visible, until a click shows it.
        assert!(sb.is_element_present("#hidden").await?);
        assert!(!sb.is_element_visible("#hidden").await?);
        sb.click("#reveal").await?;
        sb.wait_for_element_visible("#hidden", 3).await?;
        sb.assert_text("Now shown", "#hidden").await?;

        // A link, the history and a reload.
        sb.click_link("Docs link").await?;
        sb.wait_for_title("Second", 5).await?;
        assert!(sb.get_current_url().await?.ends_with("/?page=2"));
        sb.go_back().await?;
        sb.wait_for_title("BaseCase", 5).await?;
        sb.go_forward().await?;
        sb.wait_for_title("Second", 5).await?;
        sb.refresh().await?;
        sb.wait_for_title("Second", 5).await?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome"]
#[serial_test::serial(chrome)]
async fn a_case_types_selects_checks_and_submits() -> Result<()> {
    run(async |sb, url| {
        sb.open(url).await?;
        sb.type_text("#name", "Grace").await?;
        assert_eq!(sb.get_text("#name").await?, "Grace");
        sb.add_text("#notes", " and more").await?;
        assert_eq!(sb.get_text("#notes").await?, "First note and more");
        sb.select_option_by_text("#color", "Green").await?;
        assert_eq!(
            sb.get_attribute("#color", "value").await?.as_deref(),
            Some("g")
        );

        sb.check_if_unchecked("#agree").await?;
        assert!(sb.is_checked("#agree").await?);
        sb.uncheck_if_checked("#agree").await?;
        assert!(!sb.is_checked("#agree").await?);

        sb.submit("#name").await?;
        sb.assert_text("submitted Grace", "#result").await?;

        sb.set_value("#name", "By script").await?;
        assert_eq!(sb.get_text("#name").await?, "By script");
        sb.press_keys("#name", "!").await?;
        assert_eq!(sb.get_text("#name").await?, "By script!");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome"]
#[serial_test::serial(chrome)]
async fn a_case_answers_the_alerts_a_click_opens() -> Result<()> {
    run(async |sb, url| {
        sb.open(url).await?;

        // The page stalls inside `alert()`; the click is let go, as WebDriver lets it go.
        sb.click("#alert").await?;
        assert!(sb.is_alert_present().await?);
        assert_eq!(sb.switch_to_alert().await?.text().await?, "Careful!");
        sb.accept_alert().await?;
        assert!(!sb.is_alert_present().await?);
        sb.assert_text("after alert", "#result").await?;

        sb.click("#confirm").await?;
        sb.dismiss_alert().await?;
        sb.assert_text("no", "#result").await?;
        sb.click("#confirm").await?;
        sb.accept_alert().await?;
        sb.assert_text("yes", "#result").await?;

        sb.click("#prompt").await?;
        let prompt = sb.switch_to_alert().await?;
        prompt.send_keys("Ada").await?;
        prompt.accept().await?;
        sb.assert_text("hello Ada", "#result").await?;

        // No alert is open, and nothing waits for one that is not coming.
        assert!(sb.accept_alert().await.is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome"]
#[serial_test::serial(chrome)]
async fn a_case_enters_a_frame_and_leaves_it() -> Result<()> {
    run(async |sb, url| {
        sb.open(url).await?;
        sb.switch_to_frame("#frame").await?;
        assert_eq!(sb.get_text("#inner").await?, "Inside");
        sb.click("#inbtn").await?;
        sb.assert_text("clicked inside", "#inner").await?;
        // Scripts run in the frame the case is in.
        assert_eq!(
            sb.execute_script("return document.querySelector('h2').id;")
                .await?,
            json!("inner")
        );
        sb.switch_to_default_content().await?;
        assert!(!sb.is_element_present("#inner").await?);
        assert_eq!(sb.get_text("#title").await?, "BaseCase over CDP");

        sb.frame_switch("#frame")
            .run(async |sb| sb.assert_text("clicked inside", "#inner").await)
            .await?;
        assert!(!sb.is_element_present("#inner").await?);
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome"]
#[serial_test::serial(chrome)]
async fn a_case_opens_switches_and_closes_tabs_and_resizes_the_window() -> Result<()> {
    run(async |sb, url| {
        sb.open(url).await?;
        let first = sb.get_current_window_handle().await?;

        sb.open_new_tab().await?;
        assert_eq!(sb.get_all_window_handles().await?.len(), 2);
        assert_ne!(sb.get_current_window_handle().await?, first);
        sb.open(&format!("{url}?page=2")).await?;
        assert_eq!(sb.get_title().await?, "Second page");

        sb.switch_to_window(first.as_str()).await?;
        assert_eq!(sb.get_title().await?, "BaseCase over CDP");
        sb.switch_to_newest_window().await?;
        assert_eq!(sb.get_title().await?, "Second page");
        sb.close_window().await?;
        sb.switch_to_default_window().await?;
        assert_eq!(sb.get_all_window_handles().await?, [first]);
        assert_eq!(sb.get_title().await?, "BaseCase over CDP");

        sb.set_window_size(900, 700).await?;
        let (_, _, width, height) = sb.get_window_rect().await?;
        assert_eq!((width, height), (900, 700));
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome"]
#[serial_test::serial(chrome)]
async fn a_case_reads_writes_and_saves_cookies() -> Result<()> {
    run(async |sb, url| {
        sb.open(url).await?;
        let cookies = sb.get_cookies().await?;
        assert!(
            cookies
                .iter()
                .any(|cookie| cookie.name == "visited" && cookie.value == "yes"),
            "{cookies:?}"
        );
        assert!(sb.get_cookie_string().await?.contains("visited=yes"));

        sb.add_cookie(&Cookie::new("theme", "dark")).await?;
        assert_eq!(
            sb.get_cookie("theme").await?.expect("the cookie").value,
            "dark"
        );

        let path = std::env::temp_dir().join(format!(
            "sbrs-cdp-chrome-cookies-{}.json",
            std::process::id()
        ));
        sb.save_cookies_to_path(&path).await?;
        sb.clear_all_cookies().await?;
        assert!(sb.get_cookie("theme").await?.is_none());
        sb.load_cookies_from_path(&path, Default::default()).await?;
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            sb.get_cookie("theme").await?.expect("loaded again").value,
            "dark"
        );

        sb.delete_cookie("theme").await?;
        assert!(sb.get_cookie("theme").await?.is_none());
        assert!(sb.get_cookie("visited").await?.is_some());
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome"]
#[serial_test::serial(chrome)]
async fn a_case_runs_scripts_takes_a_screenshot_and_uses_the_mouse() -> Result<()> {
    run(async |sb, url| {
        sb.open(url).await?;
        assert_eq!(sb.execute_script("return 6 * 7;").await?, json!(42));
        assert_eq!(
            sb.execute_async_script(
                "var done = arguments[arguments.length - 1]; setTimeout(() => done('late'), 50);"
            )
            .await?,
            json!("late")
        );
        let png = sb.screenshot_as_png().await?;
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "a PNG");

        sb.hover("#hover-box").await?;
        sb.assert_text("hovered", "#hover-box").await?;
        sb.double_click("#hover-box").await?;
        sb.assert_text("double clicked", "#hover-box").await?;
        sb.context_click("#hover-box").await?;
        sb.assert_text("context menu", "#hover-box").await?;
        Ok(())
    })
    .await
}
