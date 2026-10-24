//! `sbrs plugin run fetcher` and `sbrs plugin run extractor` against a real headless Chrome and
//! a local HTTP server: the whole path from the command line through the built-in registry, the
//! task host and the browser, to the JSON lines on standard output.
//!
//! These tests start Chrome, so they are ignored by default. Run them one test binary at a time:
//!
//! ```text
//! cargo test -p seleniumbase-rs-cli --test cli_plugins_chrome -- --ignored --test-threads=1
//! ```

#![cfg(feature = "plugins-builtin")]

#[path = "support/sbrs.rs"]
mod sbrs;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Output;

use sbrs::{run, stderr, stdout};
use serde_json::{json, Value};

/// A page with a JSON-LD product, which the extractor finds without being told where to look.
const PRODUCT: &str = r#"<!doctype html><html><head><title>Widget Pro</title>
<script type="application/ld+json">
{"@context": "https://schema.org", "@type": "Product", "name": "Widget Pro",
 "offers": {"@type": "Offer", "price": "19.99", "priceCurrency": "USD"}}
</script></head><body><h1>Widget Pro</h1></body></html>"#;

/// Answers one request: the page, a text file, or a 404.
fn answer(mut stream: TcpStream) {
    let mut request = [0_u8; 4096];
    let read = stream.read(&mut request).unwrap_or(0);
    let request = String::from_utf8_lossy(&request[..read]);
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let (status, mime, body) = match path {
        "/" => ("200 OK", "text/html", "<html><body>home</body></html>"),
        "/product" => ("200 OK", "text/html", PRODUCT),
        "/hello.txt" => ("200 OK", "text/plain", "hello fetcher"),
        _ => ("404 Not Found", "text/plain", "no such page"),
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
}

/// A server on a free loopback port that answers every connection on a thread of its own, and
/// the origin to reach it by.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let address = listener.local_addr().expect("its address");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || answer(stream));
        }
    });
    format!("http://{address}")
}

/// Runs `sbrs --headless plugin run NAME --input input.json` in `dir` with `input` written to
/// the file.
fn plugin_run(dir: &Path, name: &str, input: &Value) -> Output {
    let file = dir.join("input.json");
    std::fs::write(&file, input.to_string()).expect("write the input");
    run(
        dir,
        &[],
        &[
            "--headless",
            "plugin",
            "run",
            name,
            "--input",
            file.to_str().expect("a path"),
        ],
    )
}

/// The JSON lines a task wrote to standard output.
fn lines(output: &Output) -> Vec<Value> {
    stdout(output)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{line}: {error}")))
        .collect()
}

#[test]
#[ignore = "starts a real Chrome"]
fn plugin_run_fetcher_fetches_the_urls_through_the_browser_and_writes_one_line_each() {
    let site = serve();
    let dir = tempfile::tempdir().expect("a folder");

    let output = plugin_run(
        dir.path(),
        "fetcher",
        &json!({ "urls": [format!("{site}/hello.txt"), format!("{site}/missing")] }),
    );

    assert!(output.status.success(), "{}", stderr(&output));
    let lines = lines(&output);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0]["index"], 0, "{lines:?}");
    assert_eq!(lines[0]["url"], format!("{site}/hello.txt"), "{lines:?}");
    assert_eq!(lines[0]["status"], 200, "{lines:?}");
    assert_eq!(lines[0]["text"], "hello fetcher", "{lines:?}");
    assert_eq!(lines[0]["truncated"], false, "{lines:?}");
    assert!(
        lines[0]["mime"]
            .as_str()
            .is_some_and(|mime| mime.starts_with("text/plain")),
        "{lines:?}"
    );
    assert_eq!(lines[1]["index"], 1, "{lines:?}");
    assert_eq!(lines[1]["status"], 404, "{lines:?}");
    let summary = stderr(&output);
    assert!(summary.contains("\"total\": 2"), "{summary}");
    assert!(summary.contains("\"ok\": 2"), "{summary}");
}

#[test]
#[ignore = "starts a real Chrome"]
fn plugin_run_extractor_finds_the_json_ld_product_of_a_page() {
    let site = serve();
    let dir = tempfile::tempdir().expect("a folder");

    let output = plugin_run(
        dir.path(),
        "extractor",
        &json!({ "urls": [format!("{site}/product")] }),
    );

    assert!(output.status.success(), "{}", stderr(&output));
    let lines = lines(&output);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["index"], 0, "{lines:?}");
    assert_eq!(lines[0]["title"], "Widget Pro", "{lines:?}");
    assert_eq!(lines[0]["schema"]["source"], "json-ld", "{lines:?}");
    assert_eq!(
        lines[0]["records"],
        json!([{ "name": "Widget Pro", "price": { "amount": 19.99, "currency": "USD" } }]),
        "{lines:?}"
    );
    let summary = stderr(&output);
    assert!(summary.contains("\"total\": 1"), "{summary}");
}
