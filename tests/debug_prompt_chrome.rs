//! The debug prompt against a real Chrome, on both engines.
//!
//! A scripted prompt (`DebugPrompt::from_reader`) takes the place of a person
//! at the terminal, so the commands run against the live page: `u`, `s`, `src`,
//! `js` and `c`. The tests need a Chrome (the WebDriver one also a
//! chromedriver), so they are `#[ignore]`d. Run them one at a time:
//!
//! ```text
//! cargo test --test debug_prompt_chrome -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::path::Path;

use seleniumbase_rs::common::debug_prompt::{
    DebugPrompt, PromptKind, PromptOutcome, PromptPage, ReaderInput,
};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{BaseCase, BrowserConfig};

const PAGE: &str =
    "<!doctype html><title>Debug Page</title><p id=\"hello\">hello from the page</p>";

/// Serves `PAGE` on a loopback port and returns its address.
fn serve() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                    PAGE.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    format!("http://{address}/")
}

type Scripted = DebugPrompt<ReaderInput<&'static [u8]>, Vec<u8>>;

fn scripted(logs: &Path) -> Scripted {
    DebugPrompt::from_reader(
        "u\nsrc\ns\njs document.title\njs 6 * 7\nc\n".as_bytes(),
        Vec::new(),
    )
    .with_logs_dir(logs)
}

/// Runs the script against `page` and checks what the prompt printed and saved.
async fn check(page: &dyn PromptPage, address: &str) {
    let logs = tempfile::tempdir().expect("a temporary folder");
    let mut prompt = scripted(logs.path());

    let outcome = prompt
        .pause(
            PromptKind::Breakpoint,
            "debug_prompt_chrome.real_page",
            "Paused on a real page.",
            page,
        )
        .await
        .expect("the pause runs");

    assert_eq!(outcome, PromptOutcome::Continue);
    let text = String::from_utf8_lossy(prompt.output()).into_owned();
    assert!(
        text.contains(&format!("page: {address}  \"Debug Page\"")),
        "{text}"
    );
    assert!(
        text.contains("\"Debug Page\"\n"),
        "js document.title: {text}"
    );
    assert!(text.contains("> 42\n"), "js 6 * 7: {text}");
    let folder = logs.path().join("debug_prompt_chrome.real_page");
    let png = std::fs::read(folder.join("breakpoint_1.png")).expect("a screenshot");
    assert!(
        png.starts_with(b"\x89PNG"),
        "a real PNG of {} bytes",
        png.len()
    );
    let source = std::fs::read_to_string(folder.join("page_source_1.html")).expect("the source");
    assert!(source.contains("hello from the page"), "{source}");
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn the_prompt_works_on_a_pure_cdp_page() {
    let address = serve();
    let browser = Browser::launch(
        LaunchOptions::builder()
            .headless(true)
            .build()
            .expect("options"),
    )
    .await
    .expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");
    page.goto(&address).await.expect("the page loads");

    check(&page, &address).await;

    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn the_prompt_works_on_a_webdriver_page() {
    let address = serve();
    let config = BrowserConfig {
        headless: true,
        ..BrowserConfig::default()
    };
    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.open(&address).await.expect("the page opens");

    check(&sb, &address).await;

    sb.quit().await.expect("Chrome closes");
}
