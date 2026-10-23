//! The built-in steps (`seleniumbase_rs::testing::bdd::steps`) used the way a
//! user's test crate uses them: a feature file, a scenario function and no
//! step code, from a binary that only links the library.
//!
//! Scenarios run on a mock WebDriver server, so no browser starts. The wording
//! tests read `tests/fixtures/bdd/python_steps.tsv`, the 669 wordings of the
//! 98 actions of SeleniumBase 4.56.2's `sbase/steps.py` (written once with
//! `python3 -I`, one `action<TAB>wording` per line).
#![cfg(feature = "bdd")]

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use base64::prelude::*;
use rstest::fixture;
use rstest_bdd::{find_step, StepKeyword};
use rstest_bdd_macros::{scenario, then};
use seleniumbase_rs::testing::bdd::steps::{builtin_steps, link, matching_actions};
use seleniumbase_rs::testing::bdd::SbCase;
use seleniumbase_rs::Settings;
use serde_json::{json, Value};

/// The key W3C WebDriver gives an element reference.
const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";

/// The wordings of `sbase/steps.py`: `action<TAB>wording`.
const PYTHON_STEPS: &str = include_str!("fixtures/bdd/python_steps.tsv");

/// A request the mock WebDriver received.
#[derive(Clone, Debug)]
struct Seen {
    method: String,
    /// Relative to the session: `/title`, not `/session/mock/title`.
    path: String,
    body: Value,
}

/// A minimal W3C WebDriver on a loopback port that remembers what it was asked.
struct Mock {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Mock {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let address = listener.local_addr().expect("its address");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let page = Arc::new(Mutex::new(String::from("about:blank")));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let (log, page) = (Arc::clone(&log), Arc::clone(&page));
                std::thread::spawn(move || answer(stream, &log, &page));
            }
        });
        Self {
            url: format!("http://{address}"),
            seen,
        }
    }

    /// The requests of `"METHOD /path"` so far.
    fn requests(&self, request: &str) -> Vec<Seen> {
        let (method, path) = request.split_once(' ').expect("METHOD /path");
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|seen| seen.method == method && seen.path == path)
            .cloned()
            .collect()
    }
}

enum Reply {
    Value(Value),
    Error(u16, &'static str),
}

fn find(body: &Value, many: bool) -> Reply {
    let selector = body["value"].as_str().unwrap_or_default();
    if selector.contains("missing") {
        return if many {
            Reply::Value(json!([]))
        } else {
            Reply::Error(404, "no such element")
        };
    }
    let id = if selector.contains("hidden") {
        "e-hidden"
    } else {
        "e1"
    };
    let element = json!({ ELEMENT_KEY: id });
    Reply::Value(if many { json!([element]) } else { element })
}

/// What the mock answers: a page called "Mock Shop" that says "Hello World".
fn reply(request: &Seen, page: &Mutex<String>) -> Reply {
    let parts: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
    let value = match (request.method.as_str(), parts.as_slice()) {
        ("POST", ["url"]) => {
            let url = request.body["url"].as_str().unwrap_or_default();
            *page.lock().unwrap() = url.to_owned();
            Value::Null
        }
        ("GET", ["url"]) => json!(page.lock().unwrap().clone()),
        ("GET", ["title"]) => json!("Mock Shop"),
        ("GET", ["source"]) => json!("<html><body>Hello World</body></html>"),
        ("GET", ["screenshot"]) => json!(BASE64_STANDARD.encode(b"\x89PNG-from-the-mock")),
        ("POST", ["element"]) => return find(&request.body, false),
        ("POST", ["elements"]) => return find(&request.body, true),
        ("GET", ["element", id, "displayed"]) => json!(*id != "e-hidden"),
        ("GET", ["element", _, "enabled"]) => json!(true),
        ("GET", ["element", _, "selected"]) => json!(false),
        ("GET", ["element", _, "text"]) => json!("Hello World"),
        ("GET", ["element", _, "attribute" | "property", _]) => json!("btn"),
        ("GET", ["element", _, "rect"]) => json!({"x": 0, "y": 0, "width": 80, "height": 20}),
        ("POST", ["window", "new"]) => json!({"handle": "h2", "type": "tab"}),
        ("GET", ["window", "handles"]) => json!(["h1", "h2"]),
        ("GET", ["window"]) => json!("h1"),
        _ => Value::Null,
    };
    Reply::Value(value)
}

fn answer(mut stream: TcpStream, seen: &Mutex<Vec<Seen>>, page: &Mutex<String>) {
    let Some((method, target, body)) = read_request(&mut stream) else {
        return;
    };
    let path = target
        .strip_prefix("/session/mock")
        .unwrap_or(&target)
        .to_owned();
    let request = Seen { method, path, body };
    let reply = match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/session") => Reply::Value(json!({ "sessionId": "mock", "capabilities": {} })),
        ("POST", "/timeouts") | ("DELETE", "") => Reply::Value(Value::Null),
        _ => reply(&request, page),
    };
    seen.lock().unwrap().push(request);
    let (status, value) = match reply {
        Reply::Value(value) => (200, value),
        Reply::Error(status, code) => (
            status,
            json!({ "error": code, "message": code, "stacktrace": "" }),
        ),
    };
    let body = json!({ "value": value }).to_string();
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json; charset=utf-8\r\n\
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

/// The mocks, by the name of the test thread that started them.
static MOCKS: Mutex<Vec<(String, Arc<Mock>)>> = Mutex::new(Vec::new());

fn thread_name() -> String {
    std::thread::current().name().unwrap_or("?").to_owned()
}

fn mock() -> Arc<Mock> {
    let owner = thread_name();
    let mocks = MOCKS.lock().unwrap();
    let (_, mock) = mocks
        .iter()
        .rev()
        .find(|(who, _)| *who == owner)
        .expect("this test started a mock WebDriver");
    Arc::clone(mock)
}

/// The browser of a scenario: the `sb` of the steps, on the mock. A user's
/// test crate gets the real one from `seleniumbase_rs::testing::bdd::sb`.
#[fixture]
async fn sb() -> SbCase {
    let mock = Arc::new(Mock::start());
    MOCKS
        .lock()
        .unwrap()
        .push((thread_name(), Arc::clone(&mock)));
    let settings = Settings {
        webdriver_url: Some(mock.url.clone()),
        headless: true,
        ..Settings::default()
    };
    SbCase::launch_with(settings)
        .await
        .expect("the mock WebDriver accepts a session")
}

/// Python's escapes in a text, as the built-in steps read them.
fn unescape(text: &str) -> String {
    text.replace("\\\\", "\\")
        .replace("\\t", "\t")
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\'", "'")
}

/// Whether a string anywhere in `value` contains `needle`.
fn mentions(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Array(items) => items.iter().any(|item| mentions(item, needle)),
        Value::Object(fields) => fields.values().any(|field| mentions(field, needle)),
        _ => false,
    }
}

// Steps of the test crate itself, beside the built-in ones.

#[then("the driver received {request:string} {n:u32} times")]
fn the_driver_received_n_times(request: String, n: u32) {
    let seen = mock().requests(&request).len();
    assert_eq!(seen, n as usize, "requests {request}");
}

#[then("the driver received {request:string} at least {n:u32} times")]
fn the_driver_received_at_least(request: String, n: u32) {
    let seen = mock().requests(&request).len();
    assert!(seen >= n as usize, "{seen} requests {request}, not {n}");
}

#[then("the driver received {request:string} with {text:string}")]
fn the_driver_received_with(request: String, text: String) {
    let needle = unescape(&text);
    let requests = mock().requests(&request);
    assert!(
        requests
            .iter()
            .any(|request| mentions(&request.body, &needle)),
        "no request {request} mentions {needle:?}: {requests:?}"
    );
}

#[scenario(path = "tests/features/bdd_builtin.feature", name = "Navigation")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn navigation(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}

#[scenario(path = "tests/features/bdd_builtin.feature", name = "Elements")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn elements(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}

#[scenario(path = "tests/features/bdd_builtin.feature", name = "Asserts")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn asserts(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}

#[scenario(
    path = "tests/features/bdd_builtin.feature",
    name = "A wrong title fails"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[should_panic(expected = "Step failed")]
async fn a_wrong_title_fails(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}

#[scenario(path = "tests/features/bdd_builtin.feature", name = "Deferred asserts")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_asserts(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}

#[scenario(
    path = "tests/features/bdd_builtin.feature",
    name = "A deferred assert fails at the end"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[should_panic(expected = "Step failed")]
async fn a_deferred_assert_fails_at_the_end(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}

#[scenario(path = "tests/features/bdd_builtin.feature", name = "Frames")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frames(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}

#[scenario(
    path = "tests/features/bdd_builtin.feature",
    name = "Pages saved and sleeping"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pages_saved_and_sleeping(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}

/// `(action, wording)` of every line of the fixture.
fn python_wordings() -> Vec<(&'static str, &'static str)> {
    PYTHON_STEPS
        .lines()
        .map(|line| line.split_once('\t').expect("action<TAB>wording"))
        .collect()
}

/// A step text for a Python wording: its placeholders get a value that fits.
fn sample(wording: &str) -> String {
    let mut text = String::new();
    let mut rest = wording;
    while let Some(open) = rest.find('{') {
        let close = open + rest[open..].find('}').expect("a closed placeholder");
        text.push_str(&rest[..open]);
        let name = &rest[open + 1..close];
        text.push_str(match name {
            "seconds" => "2",
            "px" => "3",
            "py" => "4",
            "option" => "text",
            "attribute" => "class",
            other => {
                return sample(&wording.replace(&format!("{{{other}}}"), &format!("x-{other}")))
            }
        });
        rest = &rest[close + 1..];
    }
    text.push_str(rest);
    // A feature file's steps end where their line does.
    text.trim_end().to_owned()
}

#[test]
fn the_builtin_actions_are_the_98_of_python() {
    let python: BTreeSet<&str> = python_wordings()
        .iter()
        .map(|&(action, _)| action)
        .collect();
    let ours: BTreeSet<&str> = builtin_steps().map(|(action, _)| action).collect();
    assert_eq!(python.len(), 98);
    assert_eq!(
        python.symmetric_difference(&ours).collect::<Vec<_>>(),
        Vec::<&&str>::new(),
        "only in Python, or only here"
    );
}

#[test]
fn every_python_wording_matches_exactly_one_builtin_step() {
    let wordings = python_wordings();
    assert_eq!(wordings.len(), 669, "the wordings of steps.py");
    let wrong: Vec<String> = wordings
        .iter()
        .filter_map(|&(action, wording)| {
            let text = sample(wording);
            let found = matching_actions(&text);
            (found != [action])
                .then(|| format!("{wording:?} ({text}) matched {found:?}, not {action}"))
        })
        .collect();
    assert!(
        wrong.is_empty(),
        "{} wordings:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn every_python_wording_is_registered_for_given_when_and_then() {
    link();
    let missing: Vec<String> = python_wordings()
        .iter()
        .flat_map(|&(_, wording)| {
            let text = sample(wording);
            [StepKeyword::Given, StepKeyword::When, StepKeyword::Then]
                .into_iter()
                .filter(|&keyword| find_step(keyword, text.as_str().into()).is_none())
                .map(|keyword| format!("{keyword:?} {text}"))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        missing.is_empty(),
        "{} steps:\n{}",
        missing.len(),
        missing.join("\n")
    );
}
