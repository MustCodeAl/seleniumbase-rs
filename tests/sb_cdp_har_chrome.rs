//! `Page::start_har` against a real Chrome.
//!
//! A loopback server serves a page that pulls a stylesheet, a PNG, a missing
//! image and a `fetch` POST that sets a cookie, behind a redirect. The test
//! records the visit with bodies, saves the HAR, reads it back as JSON and
//! checks the entries. It needs a Chrome or Chromium on the machine, so it is
//! ignored by default. Run it alone:
//!
//! ```text
//! cargo test --features test-util --test sb_cdp_har_chrome -- --ignored --nocapture
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use base64::Engine as _;
use seleniumbase_rs::sb_cdp::{Browser, HarOptions, LaunchOptions};
use serde_json::Value;

const HOME: &str = r#"<!doctype html><html><head><title>HAR</title>
<link rel="stylesheet" href="/style.css"></head><body><h1>Recorded</h1>
<img src="/pixel.png"><img src="/missing.png">
<script>
fetch('/api/items?id=7', { method: 'POST', body: '{"a":1}', headers: { 'Content-Type': 'application/json' } })
  .then(r => r.text()).then(t => { document.title = 'done ' + t; });
</script></body></html>"#;

/// A 1 x 1 transparent PNG.
const PIXEL: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

/// Answers one connection.
fn answer(mut stream: TcpStream) {
    let mut buffer = [0_u8; 16384];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/")
        .to_owned();
    let pixel = base64::engine::general_purpose::STANDARD
        .decode(PIXEL)
        .unwrap();
    let (status, headers, body): (&str, &str, Vec<u8>) = match path.as_str() {
        "/old" => ("302 Found", "Location: /\r\n", Vec::new()),
        "/" => ("200 OK", "Content-Type: text/html\r\n", HOME.into()),
        "/style.css" => (
            "200 OK",
            "Content-Type: text/css\r\n",
            b"h1 { color: teal; }".to_vec(),
        ),
        "/pixel.png" => ("200 OK", "Content-Type: image/png\r\n", pixel),
        "/api/items?id=7" => (
            "200 OK",
            "Content-Type: application/json\r\nSet-Cookie: session=xyz; Path=/; HttpOnly\r\n",
            br#"{"items":[1,2]}"#.to_vec(),
        ),
        _ => (
            "404 Not Found",
            "Content-Type: text/plain\r\n",
            b"no such file".to_vec(),
        ),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

/// Starts the server and returns its `http://127.0.0.1:port` base.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let base = format!("http://{}", listener.local_addr().expect("bound address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            std::thread::spawn(move || answer(stream));
        }
    });
    base
}

fn entry<'a>(entries: &'a [Value], url: &str) -> &'a Value {
    entries
        .iter()
        .find(|entry| entry["request"]["url"] == url)
        .unwrap_or_else(|| panic!("no entry for {url}"))
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_visit_is_recorded_as_a_har_that_reads_back() {
    let base = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .unwrap()
        .with_timeout(Duration::from_secs(15));

    let recorder = page
        .start_har_with(HarOptions::new().content(true))
        .await
        .expect("recording starts");
    page.goto(format!("{base}/old"))
        .await
        .expect("the page opens");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !page.title().await.unwrap().starts_with("done") {
        assert!(Instant::now() < deadline, "the fetch never finished");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let har = recorder.finish().await.expect("the recording finishes");
    browser.close().await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("visit.har");
    har.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let json: Value = serde_json::from_str(&text).expect("the HAR is valid JSON");
    let log = &json["log"];
    let entries = log["entries"].as_array().unwrap();
    for entry in entries {
        eprintln!(
            "{} {} {} {} time={} {}",
            entry["request"]["method"],
            entry["request"]["url"],
            entry["response"]["status"],
            entry["_resourceType"],
            entry["time"],
            entry["timings"],
        );
    }
    eprintln!("browser: {}", log["browser"]);
    eprintln!("pages: {}", log["pages"]);

    assert_eq!(log["version"], "1.2");
    assert!(log["browser"]["name"].as_str().unwrap().contains("Chrome"));
    let first_page = &log["pages"][0];
    assert_eq!(first_page["title"], format!("{base}/old"));
    assert!(first_page["pageTimings"]["onLoad"].as_f64().unwrap() > 0.0);

    let old = entry(entries, &format!("{base}/old"));
    assert_eq!(old["response"]["status"], 302);
    assert_eq!(old["response"]["redirectURL"], "/");
    let home = entry(entries, &format!("{base}/"));
    assert_eq!(home["response"]["status"], 200);
    assert_eq!(home["_resourceType"], "document");
    assert_eq!(home["response"]["content"]["mimeType"], "text/html");
    assert!(home["response"]["content"]["text"]
        .as_str()
        .unwrap()
        .contains("<h1>Recorded</h1>"));
    assert_eq!(home["pageref"], "page_1");
    assert_eq!(home["serverIPAddress"], "127.0.0.1");
    assert!(home["startedDateTime"].as_str().unwrap().ends_with('Z'));
    for phase in [
        "blocked", "dns", "connect", "send", "wait", "receive", "ssl",
    ] {
        assert!(home["timings"][phase].is_number(), "{phase}");
    }
    assert!(home["time"].as_f64().unwrap() > 0.0);

    let css = entry(entries, &format!("{base}/style.css"));
    assert_eq!(css["response"]["content"]["text"], "h1 { color: teal; }");
    let pixel = entry(entries, &format!("{base}/pixel.png"));
    assert_eq!(pixel["response"]["content"]["encoding"], "base64");
    assert_eq!(pixel["response"]["content"]["text"], PIXEL);
    assert_eq!(pixel["_resourceType"], "image");
    let missing = entry(entries, &format!("{base}/missing.png"));
    assert_eq!(missing["response"]["status"], 404);

    let api = entry(entries, &format!("{base}/api/items?id=7"));
    assert_eq!(api["request"]["method"], "POST");
    assert_eq!(api["request"]["postData"]["text"], r#"{"a":1}"#);
    assert_eq!(api["request"]["postData"]["mimeType"], "application/json");
    assert_eq!(api["request"]["queryString"][0]["name"], "id");
    assert_eq!(api["request"]["queryString"][0]["value"], "7");
    assert_eq!(api["response"]["status"], 200);
    assert_eq!(api["response"]["cookies"][0]["name"], "session");
    assert_eq!(api["response"]["cookies"][0]["httpOnly"], true);
    assert_eq!(api["response"]["content"]["text"], r#"{"items":[1,2]}"#);
}
