//! A minimal W3C WebDriver server on a loopback port, for tests that run the
//! real `sbase` binary and need to see what it sent to its browser.
//!
//! It answers the new-session, delete-session and a few element commands
//! itself and returns `null` for every other command. Every request is
//! recorded, so a test can read the capabilities `sbase` asked for. No browser
//! or driver is involved.

// A test that includes this file uses only some of what it offers.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

/// The key W3C WebDriver gives an element reference.
const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";

/// The id of the one session the server hands out.
const SESSION: &str = "fake";

/// A request the server received.
#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    /// Relative to the session: `/title`, not `/session/fake/title`.
    pub path: String,
    pub body: Value,
}

/// The running server. Its thread ends with the test process.
pub struct FakeWebDriver {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl FakeWebDriver {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let address = listener.local_addr().expect("bound address");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let log = Arc::clone(&log);
                std::thread::spawn(move || answer(stream, &log));
            }
        });
        Self {
            url: format!("http://{address}"),
            seen,
        }
    }

    /// The address to give `sbase --webdriver`.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Every request received so far, oldest first.
    pub fn requests(&self) -> Vec<Seen> {
        self.seen.lock().expect("request log").clone()
    }

    /// The command-line arguments of the Chrome that the first new-session
    /// request asked for, if it asked for one.
    pub fn chrome_args(&self) -> Option<Vec<String>> {
        let requests = self.requests();
        let session = requests
            .iter()
            .find(|request| request.method == "POST" && request.path == "/session")?;
        let args = &session.body["capabilities"]["alwaysMatch"]["goog:chromeOptions"]["args"];
        Some(
            args.as_array()?
                .iter()
                .filter_map(|arg| arg.as_str().map(str::to_owned))
                .collect(),
        )
    }

    /// Whether the Chrome that was asked for runs without a window.
    pub fn asked_for_headless(&self) -> bool {
        self.chrome_args()
            .is_some_and(|args| args.iter().any(|arg| arg.starts_with("--headless")))
    }
}

fn answer(mut stream: TcpStream, seen: &Mutex<Vec<Seen>>) {
    let Some((method, target, body)) = read_request(&mut stream) else {
        return;
    };
    let path = target
        .strip_prefix(&format!("/session/{SESSION}"))
        .unwrap_or(&target)
        .to_owned();
    let value = match (method.as_str(), path.as_str()) {
        ("POST", "/session") => {
            json!({ "sessionId": SESSION, "capabilities": { "browserName": "chrome" } })
        }
        ("POST", "/element") => json!({ ELEMENT_KEY: "e1" }),
        ("GET", "/element/e1/displayed" | "/element/e1/enabled") => json!(true),
        ("GET", "/title") => json!("Fake page"),
        ("GET", "/url") => json!("https://example.test/"),
        _ => Value::Null,
    };
    seen.lock()
        .expect("request log")
        .push(Seen { method, path, body });
    let body = json!({ "value": value }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// The method, target and JSON body of the request on `stream`.
fn read_request(stream: &mut TcpStream) -> Option<(String, String, Value)> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
        let read = stream.read(&mut chunk).ok().filter(|read| *read > 0)?;
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut request_line = head.lines().next()?.split(' ');
    let method = request_line.next()?.to_owned();
    let target = request_line.next()?.to_owned();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    while buffer.len() < header_end + length {
        let read = stream.read(&mut chunk).ok().filter(|read| *read > 0)?;
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body =
        serde_json::from_slice(&buffer[header_end..header_end + length]).unwrap_or(Value::Null);
    Some((method, target, body))
}
