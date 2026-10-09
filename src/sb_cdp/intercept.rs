//! Rule-based interception of a page's network requests.
//!
//! [`Page::intercept`] pauses every request the page makes and answers it
//! according to the first [`Rule`] that matches: block it, serve a made-up
//! response instead of going to the network, or send it on with a changed
//! header, URL, method or body. Requests no rule matches go through untouched.
//!
//! Interception is per page: other tabs are not affected. (For a header
//! override across the whole browser, see
//! [`CdpReactor`](crate::stealth::reactor::CdpReactor).)
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{Page, ResourceType, Response, Rule};
//!
//! # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let interception = page
//!     .intercept(vec![
//!         // No images or fonts: faster pages, less bandwidth.
//!         Rule::block().resource_type(ResourceType::Image).resource_type(ResourceType::Font),
//!         // Stub the API so the page is deterministic.
//!         Rule::fulfill(Response::json(200, &serde_json::json!({"plan": "pro"})))
//!             .url("*/api/account"),
//!         // Tell the server who is asking.
//!         Rule::modify().set_header("X-Test-Run", "42").url("https://example.com/*"),
//!     ])
//!     .await?;
//!
//! page.goto("https://example.com").await?;
//! println!("{} requests seen", interception.log().len());
//! interception.stop().await?;
//! # Ok(())
//! # }
//! ```

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use super::sync::locked;
use super::Page;
use crate::error::SeleniumBaseError;

/// The most requests an [`Interception`] remembers.
const LOG_LIMIT: usize = 10_000;

/// What kind of resource a request is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceType {
    /// An HTML document, including frames.
    Document,
    /// A stylesheet.
    Stylesheet,
    /// An image.
    Image,
    /// Audio or video.
    Media,
    /// A web font.
    Font,
    /// A script.
    Script,
    /// An `XMLHttpRequest`.
    Xhr,
    /// A `fetch()` call.
    Fetch,
    /// A WebSocket handshake.
    WebSocket,
    /// Anything else.
    Other,
}

impl ResourceType {
    fn from_protocol(name: &str) -> Self {
        match name {
            "Document" => Self::Document,
            "Stylesheet" => Self::Stylesheet,
            "Image" => Self::Image,
            "Media" => Self::Media,
            "Font" => Self::Font,
            "Script" => Self::Script,
            "XHR" => Self::Xhr,
            "Fetch" => Self::Fetch,
            "WebSocket" => Self::WebSocket,
            _ => Self::Other,
        }
    }
}

/// A request the page made, as it was when paused.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// The full URL.
    pub url: String,
    /// The HTTP method.
    pub method: String,
    /// What the request is for.
    pub resource_type: ResourceType,
    /// The request headers.
    pub headers: BTreeMap<String, String>,
}

/// How a request was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// Failed before it reached the network.
    Blocked,
    /// Answered with a made-up response.
    Fulfilled,
    /// Sent on with changes.
    Modified,
    /// Sent on unchanged.
    Continued,
}

/// A response to serve in place of the network's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// A response with this status and body, and no headers.
    #[must_use]
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    /// A JSON response.
    #[must_use]
    pub fn json(status: u16, value: &Value) -> Self {
        Self::new(status, value.to_string()).header("Content-Type", "application/json")
    }

    /// An HTML response.
    #[must_use]
    pub fn html(status: u16, html: impl Into<String>) -> Self {
        Self::new(status, html.into()).header("Content-Type", "text/html; charset=utf-8")
    }

    /// Adds a header.
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// What a [`Rule`] does to a request it matches.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Block,
    Fulfill(Response),
    Modify(Changes),
}

/// Changes to make to a request on its way to the network.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Changes {
    set_headers: BTreeMap<String, String>,
    remove_headers: Vec<String>,
    url: Option<String>,
    method: Option<String>,
    body: Option<Vec<u8>>,
}

/// A condition on requests and what to do with those that meet it.
///
/// A rule with no condition matches every request. Conditions combine with
/// "and"; giving a resource type several times means "any of these".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    action: Action,
    url: Option<String>,
    methods: Vec<String>,
    types: Vec<ResourceType>,
}

impl Rule {
    fn with(action: Action) -> Self {
        Self {
            action,
            url: None,
            methods: Vec::new(),
            types: Vec::new(),
        }
    }

    /// Fails matching requests before they reach the network.
    #[must_use]
    pub fn block() -> Self {
        Self::with(Action::Block)
    }

    /// Answers matching requests with `response`, never touching the network.
    #[must_use]
    pub fn fulfill(response: Response) -> Self {
        Self::with(Action::Fulfill(response))
    }

    /// Sends matching requests on with the changes added by
    /// [`set_header`](Self::set_header) and its siblings.
    #[must_use]
    pub fn modify() -> Self {
        Self::with(Action::Modify(Changes::default()))
    }

    /// Only requests whose whole URL matches `glob`, where `*` matches any run
    /// of characters and `?` any one character.
    #[must_use]
    pub fn url(mut self, glob: impl Into<String>) -> Self {
        self.url = Some(glob.into());
        self
    }

    /// Only requests using this HTTP method (case-insensitive). Repeat for
    /// several.
    #[must_use]
    pub fn method(mut self, method: impl Into<String>) -> Self {
        self.methods.push(method.into().to_ascii_uppercase());
        self
    }

    /// Only requests for this kind of resource. Repeat for several.
    #[must_use]
    pub fn resource_type(mut self, kind: ResourceType) -> Self {
        self.types.push(kind);
        self
    }

    fn changes(mut self, change: impl FnOnce(&mut Changes)) -> Self {
        if let Action::Modify(changes) = &mut self.action {
            change(changes);
        }
        self
    }

    /// For a [`modify`](Self::modify) rule: sets a request header.
    #[must_use]
    pub fn set_header(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.changes(|c| {
            c.set_headers.insert(name.into(), value.into());
        })
    }

    /// For a [`modify`](Self::modify) rule: removes a request header.
    #[must_use]
    pub fn remove_header(self, name: impl Into<String>) -> Self {
        self.changes(|c| c.remove_headers.push(name.into()))
    }

    /// For a [`modify`](Self::modify) rule: sends the request to another URL.
    #[must_use]
    pub fn redirect_to(self, url: impl Into<String>) -> Self {
        self.changes(|c| c.url = Some(url.into()))
    }

    /// For a [`modify`](Self::modify) rule: changes the HTTP method.
    #[must_use]
    pub fn change_method(self, method: impl Into<String>) -> Self {
        self.changes(|c| c.method = Some(method.into()))
    }

    /// For a [`modify`](Self::modify) rule: replaces the request body.
    #[must_use]
    pub fn replace_body(self, body: impl Into<Vec<u8>>) -> Self {
        self.changes(|c| c.body = Some(body.into()))
    }

    /// Whether the rule applies to `request`.
    #[must_use]
    pub fn matches(&self, request: &Request) -> bool {
        self.url
            .as_deref()
            .is_none_or(|glob| glob_match(glob, &request.url))
            && (self.methods.is_empty()
                || self.methods.contains(&request.method.to_ascii_uppercase()))
            && (self.types.is_empty() || self.types.contains(&request.resource_type))
    }

    fn outcome(&self) -> Outcome {
        match self.action {
            Action::Block => Outcome::Blocked,
            Action::Fulfill(_) => Outcome::Fulfilled,
            Action::Modify(_) => Outcome::Modified,
        }
    }
}

/// Whether `text` matches `pattern`, where `*` matches any run of characters
/// (including none) and `?` any single character.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // Where the last `*` was, and how much text it has swallowed so far.
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

/// One request an [`Interception`] saw, and what it did.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    /// The request.
    pub request: Request,
    /// How it was answered.
    pub outcome: Outcome,
}

/// A running interception; see the [module docs](crate::sb_cdp::intercept).
///
/// Stops when [`stop`](Self::stop)ped or dropped.
#[derive(Debug)]
pub struct Interception {
    page: Page,
    task: Option<JoinHandle<()>>,
    log: Arc<Mutex<Vec<Seen>>>,
}

impl Page {
    /// Starts answering this page's requests according to `rules`.
    ///
    /// Rules are tried in order and the first match wins. A page can have one
    /// interception at a time; starting another replaces the first.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    pub async fn intercept(&self, rules: Vec<Rule>) -> Result<Interception, SeleniumBaseError> {
        // Listen before enabling, so no request is missed.
        let mut events = self.events();
        self.execute(
            "Fetch.enable",
            json!({
                "patterns": [{ "urlPattern": "*" }],
                "handleAuthRequests": self.browser().proxy_has_credentials(),
            }),
        )
        .await?;

        let log = Arc::new(Mutex::new(Vec::new()));
        let page = self.clone();
        let task_log = Arc::clone(&log);
        let task = tokio::spawn(async move {
            while let Some(event) = events.next().await {
                if event.method != "Fetch.requestPaused" {
                    continue;
                }
                let Some(request_id) = event.params["requestId"].as_str() else {
                    continue;
                };
                let request =
                    parse_request(&event.params["request"], &event.params["resourceType"]);
                let rule = rules.iter().find(|rule| rule.matches(&request));
                let outcome = rule.map_or(Outcome::Continued, Rule::outcome);

                let answered = match rule.map(|rule| &rule.action) {
                    Some(Action::Block) => {
                        page.execute(
                            "Fetch.failRequest",
                            json!({ "requestId": request_id, "errorReason": "BlockedByClient" }),
                        )
                        .await
                    }
                    Some(Action::Fulfill(response)) => {
                        page.execute("Fetch.fulfillRequest", fulfill_params(request_id, response))
                            .await
                    }
                    Some(Action::Modify(changes)) => {
                        page.execute(
                            "Fetch.continueRequest",
                            modify_params(request_id, &request, changes),
                        )
                        .await
                    }
                    None => {
                        page.execute("Fetch.continueRequest", json!({ "requestId": request_id }))
                            .await
                    }
                };
                if let Err(error) = answered {
                    tracing::warn!(url = %request.url, %error, "could not answer an intercepted request");
                }

                let mut seen = locked(&task_log);
                if seen.len() < LOG_LIMIT {
                    seen.push(Seen { request, outcome });
                }
            }
        });

        Ok(Interception {
            page: self.clone(),
            task: Some(task),
            log,
        })
    }
}

impl Interception {
    /// Every request seen so far and how it was answered, oldest first (up to
    /// ten thousand).
    #[must_use]
    pub fn log(&self) -> Vec<Seen> {
        locked(&self.log).clone()
    }

    /// Stops intercepting; later requests go straight to the network.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    pub async fn stop(mut self) -> Result<(), SeleniumBaseError> {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.page
            .execute("Fetch.disable", json!({}))
            .await
            .map(drop)
    }
}

impl Drop for Interception {
    fn drop(&mut self) {
        let Some(task) = self.task.take() else { return };
        task.abort();
        let page = self.page.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                // The page may already be gone; there is nothing to report to.
                let _ = page.execute("Fetch.disable", json!({})).await;
            });
        }
    }
}

fn parse_request(request: &Value, resource_type: &Value) -> Request {
    Request {
        url: request["url"].as_str().unwrap_or_default().to_owned(),
        method: request["method"].as_str().unwrap_or("GET").to_owned(),
        resource_type: ResourceType::from_protocol(resource_type.as_str().unwrap_or_default()),
        headers: request["headers"]
            .as_object()
            .map(|headers| {
                headers
                    .iter()
                    .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn fulfill_params(request_id: &str, response: &Response) -> Value {
    json!({
        "requestId": request_id,
        "responseCode": response.status,
        "responseHeaders": response
            .headers
            .iter()
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect::<Vec<_>>(),
        "body": base64::engine::general_purpose::STANDARD.encode(&response.body),
    })
}

fn modify_params(request_id: &str, request: &Request, changes: &Changes) -> Value {
    // `continueRequest` replaces the header list, so start from the original.
    let mut headers: BTreeMap<String, String> = request
        .headers
        .iter()
        .filter(|(name, _)| {
            !changes
                .remove_headers
                .iter()
                .any(|gone| gone.eq_ignore_ascii_case(name))
                && !changes
                    .set_headers
                    .keys()
                    .any(|set| set.eq_ignore_ascii_case(name))
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    headers.extend(changes.set_headers.clone());

    let mut params = json!({
        "requestId": request_id,
        "headers": headers
            .iter()
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect::<Vec<_>>(),
    });
    if let Some(url) = &changes.url {
        params["url"] = json!(url);
    }
    if let Some(method) = &changes.method {
        params["method"] = json!(method);
    }
    if let Some(body) = &changes.body {
        params["postData"] = json!(base64::engine::general_purpose::STANDARD.encode(body));
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(url: &str, method: &str, kind: ResourceType) -> Request {
        Request {
            url: url.to_owned(),
            method: method.to_owned(),
            resource_type: kind,
            headers: BTreeMap::from([("Accept".to_owned(), "*/*".to_owned())]),
        }
    }

    #[test]
    fn globs_match_stars_questions_and_literals() {
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything at all"));
        assert!(glob_match("https://a.test/*", "https://a.test/x/y.png"));
        assert!(glob_match("*.png", "https://a.test/x.png"));
        assert!(glob_match("*/api/*/items", "https://a.test/api/v2/items"));
        assert!(glob_match("a?c", "abc"));
        assert!(glob_match("*a*b*", "xxaYYbzz"));
        assert!(!glob_match("*.png", "https://a.test/x.png?v=1"));
        assert!(!glob_match("a?c", "ac"));
        assert!(!glob_match("abc", "abcd"));
        assert!(!glob_match("", "x"));
        assert!(glob_match("", ""));
    }

    #[test]
    fn a_glob_with_many_stars_does_not_blow_up() {
        let pattern = "*a".repeat(30) + "b";
        let text = "a".repeat(200);
        assert!(
            !glob_match(&pattern, &text),
            "must finish quickly and not match"
        );
    }

    #[test]
    fn a_rule_with_no_conditions_matches_everything() {
        let rule = Rule::block();
        assert!(rule.matches(&request("https://a.test/", "GET", ResourceType::Document)));
        assert!(rule.matches(&request("data:,", "POST", ResourceType::Other)));
    }

    #[test]
    fn conditions_combine_with_and_and_repeated_ones_with_or() {
        let rule = Rule::block()
            .url("https://a.test/*")
            .resource_type(ResourceType::Image)
            .resource_type(ResourceType::Font);
        assert!(rule.matches(&request("https://a.test/x", "GET", ResourceType::Image)));
        assert!(rule.matches(&request("https://a.test/x", "GET", ResourceType::Font)));
        assert!(!rule.matches(&request("https://a.test/x", "GET", ResourceType::Script)));
        assert!(!rule.matches(&request("https://b.test/x", "GET", ResourceType::Image)));
    }

    #[test]
    fn methods_match_case_insensitively() {
        let rule = Rule::block().method("post");
        assert!(rule.matches(&request("https://a.test/", "POST", ResourceType::Fetch)));
        assert!(!rule.matches(&request("https://a.test/", "GET", ResourceType::Fetch)));
    }

    #[test]
    fn resource_types_come_from_the_protocol_names() {
        for (name, kind) in [
            ("Document", ResourceType::Document),
            ("XHR", ResourceType::Xhr),
            ("Fetch", ResourceType::Fetch),
            ("Image", ResourceType::Image),
            ("Ping", ResourceType::Other),
            ("", ResourceType::Other),
        ] {
            assert_eq!(ResourceType::from_protocol(name), kind, "{name}");
        }
    }

    #[test]
    fn a_request_is_read_from_the_paused_event() {
        let parsed = parse_request(
            &json!({ "url": "https://a.test/x", "method": "PUT", "headers": { "X-A": "1", "Bad": 5 } }),
            &json!("XHR"),
        );
        assert_eq!(parsed.url, "https://a.test/x");
        assert_eq!(parsed.method, "PUT");
        assert_eq!(parsed.resource_type, ResourceType::Xhr);
        assert_eq!(
            parsed.headers.len(),
            1,
            "non-string header values are ignored"
        );
        assert_eq!(parse_request(&json!({}), &json!(null)).method, "GET");
    }

    #[test]
    fn a_fulfilled_response_carries_status_headers_and_a_base64_body() {
        let response = Response::json(201, &json!({"ok": true})).header("X-Mock", "1");
        let params = fulfill_params("R1", &response);
        assert_eq!(params["requestId"], "R1");
        assert_eq!(params["responseCode"], 201);
        let headers = params["responseHeaders"].as_array().unwrap();
        assert!(headers
            .iter()
            .any(|h| h["name"] == "Content-Type" && h["value"] == "application/json"));
        assert!(headers.iter().any(|h| h["name"] == "X-Mock"));
        let body = base64::engine::general_purpose::STANDARD
            .decode(params["body"].as_str().unwrap())
            .unwrap();
        assert_eq!(body, br#"{"ok":true}"#);
    }

    #[test]
    fn modifying_keeps_the_original_headers_and_applies_the_changes() {
        let rule = Rule::modify()
            .set_header("accept", "text/plain")
            .set_header("X-New", "1")
            .remove_header("Cookie")
            .redirect_to("https://b.test/")
            .change_method("POST")
            .replace_body("hi");
        let Action::Modify(changes) = &rule.action else {
            panic!("a modify rule")
        };
        let mut original = request("https://a.test/", "GET", ResourceType::Fetch);
        original.headers.insert("Cookie".into(), "sid=1".into());
        original.headers.insert("Keep".into(), "yes".into());

        let params = modify_params("R2", &original, changes);

        let headers: BTreeMap<String, String> = params["headers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| {
                (
                    h["name"].as_str().unwrap().to_owned(),
                    h["value"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(headers.get("Keep").map(String::as_str), Some("yes"));
        assert_eq!(
            headers.get("accept").map(String::as_str),
            Some("text/plain"),
            "set replaces, case-insensitively"
        );
        assert!(!headers.contains_key("Accept"));
        assert!(!headers.contains_key("Cookie"));
        assert_eq!(headers.get("X-New").map(String::as_str), Some("1"));
        assert_eq!(params["url"], "https://b.test/");
        assert_eq!(params["method"], "POST");
        assert_eq!(
            params["postData"],
            base64::engine::general_purpose::STANDARD.encode("hi")
        );
    }

    #[test]
    fn change_methods_do_nothing_on_other_kinds_of_rule() {
        let rule = Rule::block()
            .set_header("X", "1")
            .redirect_to("https://x.test/");
        assert_eq!(rule, Rule::block());
    }

    #[test]
    fn each_rule_reports_its_outcome() {
        assert_eq!(Rule::block().outcome(), Outcome::Blocked);
        assert_eq!(
            Rule::fulfill(Response::new(200, "")).outcome(),
            Outcome::Fulfilled
        );
        assert_eq!(Rule::modify().outcome(), Outcome::Modified);
    }
}
