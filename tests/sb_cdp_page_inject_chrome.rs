//! `Page::add_script_tag`, `add_style_tag`, `add_init_script` and
//! `remove_init_script` against a real Chrome.
//!
//! A loopback server serves a page, a script, a stylesheet, and a 404 for
//! everything else. The test needs a Chrome or Chromium on the machine, so it
//! is ignored by default. Run it with:
//!
//! ```text
//! cargo test --test sb_cdp_page_inject_chrome -- --ignored
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, TagSource};

const HOME: &str = "<!doctype html><html><head><title>Inject</title>\
    <script>window.pageScriptRan = true;</script></head><body>Inject</body></html>";

/// Answers one connection.
fn answer(mut stream: TcpStream) {
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/");
    let (status, kind, body) = match path {
        "/" => ("200 OK", "text/html", HOME),
        "/lib.js" => ("200 OK", "text/javascript", "window.fromFile = 'loaded';"),
        "/style.css" => ("200 OK", "text/css", "body { margin-left: 33px; }"),
        _ => ("404 Not Found", "text/plain", "no such file"),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Starts the server and returns its `http://127.0.0.1:port` base.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let base = format!("http://{}", listener.local_addr().expect("bound address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            // A thread each: Chrome opens idle connections ahead of time.
            std::thread::spawn(move || answer(stream));
        }
    });
    base
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn tags_and_init_scripts_reach_the_documents_they_should() {
    let base = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.unwrap();
    page.goto(format!("{base}/")).await.unwrap();

    // Script tags: a file is waited for, a missing one and a throwing one are errors.
    page.add_script_tag(TagSource::url(format!("{base}/lib.js")))
        .await
        .unwrap();
    assert_eq!(page.evaluate("window.fromFile").await.unwrap(), "loaded");
    let missing = page
        .add_script_tag(TagSource::url(format!("{base}/missing.js")))
        .await
        .unwrap_err();
    assert!(missing.to_string().contains("did not load"), "{missing}");
    page.add_script_tag(TagSource::content("window.inline = 2;"))
        .await
        .unwrap();
    assert_eq!(page.evaluate("window.inline").await.unwrap(), 2);
    let thrown = page
        .add_script_tag(TagSource::content("throw new Error('inline boom');"))
        .await
        .unwrap_err();
    assert!(thrown.to_string().contains("inline boom"), "{thrown}");

    // Style tags: the file and the inline CSS both apply.
    page.add_style_tag(TagSource::url(format!("{base}/style.css")))
        .await
        .unwrap();
    assert_eq!(
        page.evaluate("getComputedStyle(document.body).marginLeft")
            .await
            .unwrap(),
        "33px"
    );
    page.add_style_tag(TagSource::content("body { margin-top: 44px; }"))
        .await
        .unwrap();
    assert_eq!(
        page.evaluate("getComputedStyle(document.body).marginTop")
            .await
            .unwrap(),
        "44px"
    );
    let missing = page
        .add_style_tag(TagSource::url(format!("{base}/missing.css")))
        .await
        .unwrap_err();
    assert!(missing.to_string().contains("did not load"), "{missing}");

    // An init script: not in the current document, in the next ones before
    // the page's own scripts, and gone once removed.
    let id = page
        .add_init_script("window.early = !window.pageScriptRan;")
        .await
        .unwrap();
    assert_eq!(
        page.evaluate("typeof window.early").await.unwrap(),
        "undefined"
    );
    page.goto(format!("{base}/")).await.unwrap();
    assert_eq!(page.evaluate("window.early").await.unwrap(), true);
    page.remove_init_script(&id).await.unwrap();
    page.goto(format!("{base}/")).await.unwrap();
    assert_eq!(
        page.evaluate("typeof window.early").await.unwrap(),
        "undefined"
    );

    browser.close().await.unwrap();
}
