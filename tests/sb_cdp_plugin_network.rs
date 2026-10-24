//! The network router of the plugin hooks on the Pure CDP engine, against a
//! scripted browser, with no Chrome.
//!
//! A test pauses requests the way Chrome does (`Fetch.requestPaused`, at the
//! request stage and at the response stage) and reads back what the plugins
//! saw and which `Fetch` commands the mock browser was sent. Run with
//! `cargo test --features "test-util plugins" --test sb_cdp_plugin_network`.

#![cfg(all(feature = "plugins", feature = "test-util"))]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, Metadata, NetRule, NetworkInterest, Plugin, PluginSet,
    RequestDecision, RequestFilter, RequestView, ResourceKind, ResponseView,
    Result as PluginResult,
};
use seleniumbase_rs::sb_cdp::{
    Browser, ContextOptions, LaunchOptions, MockCtrl, Page, Proxy, Response, Rule,
};
use serde_json::{json, Value};

type Log = Arc<Mutex<Vec<String>>>;

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

/// A plugin with a network declaration, canned answers and a log of what it saw.
struct Net {
    name: &'static str,
    caps: Vec<Capability>,
    interest: NetworkInterest,
    decision: RequestDecision,
    hang: bool,
    log: Log,
}

impl Net {
    fn new(name: &'static str, log: &Log) -> Self {
        Self {
            name,
            caps: Vec::new(),
            interest: NetworkInterest::none(),
            decision: RequestDecision::Continue,
            hang: false,
            log: Arc::clone(log),
        }
    }

    fn caps(mut self, caps: impl IntoIterator<Item = Capability>) -> Self {
        self.caps.extend(caps);
        self
    }

    fn interest(mut self, interest: NetworkInterest) -> Self {
        self.interest = interest;
        self
    }

    fn decide(mut self, decision: RequestDecision) -> Self {
        self.decision = decision;
        self
    }

    fn hanging(mut self) -> Self {
        self.hang = true;
        self
    }
}

#[async_trait]
impl Plugin for Net {
    fn name(&self) -> &str {
        self.name
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capabilities(self.caps.clone())
    }

    fn network(&self) -> NetworkInterest {
        self.interest.clone()
    }

    async fn on_request(&self, request: &RequestView) -> PluginResult<RequestDecision> {
        let cookie = request.headers.get("Cookie").cloned().unwrap_or_default();
        self.log.lock().unwrap().push(format!(
            "{}:request {} cookie={cookie}",
            self.name, request.url
        ));
        if self.hang {
            std::future::pending::<()>().await;
        }
        Ok(self.decision.clone())
    }

    async fn on_response(&self, response: &ResponseView<'_>) -> PluginResult<()> {
        let body = response.body.map_or_else(
            || "none".to_owned(),
            |bytes| String::from_utf8_lossy(bytes).into_owned(),
        );
        self.log.lock().unwrap().push(format!(
            "{}:response {} {:?} body={body} truncated={} {:?}",
            self.name, response.status, response.mime, response.truncated, response.outcome
        ));
        Ok(())
    }
}

const INTERCEPT: [Capability; 1] = [Capability::NetIntercept];

fn intercept_all() -> NetworkInterest {
    NetworkInterest::none().intercept(RequestFilter::any())
}

async fn launch(plugins: PluginSet) -> (Browser, Page, MockCtrl) {
    capture();
    let mock = MockCtrl::new();
    mock.reply(
        "Target.createBrowserContext",
        json!({ "browserContextId": "CTX1" }),
    );
    let browser = Browser::launch_mocked(
        LaunchOptions::builder()
            .headless(true)
            .plugins(plugins)
            .build()
            .unwrap(),
        &mock,
    )
    .await
    .expect("the mocked launch");
    let page = browser.default_page().await.unwrap();
    (browser, page, mock)
}

/// Pauses a request at the request stage the way Chrome does.
fn pause(mock: &MockCtrl, id: &str, url: &str, kind: &str) {
    mock.emit(
        "Fetch.requestPaused",
        json!({
            "requestId": id,
            "resourceType": kind,
            "request": {
                "url": url,
                "method": "GET",
                "headers": { "Accept": "*/*", "Cookie": "sid=1" },
            },
        }),
        Some("S1"),
    );
}

/// Pauses a request at the response stage.
fn pause_response(mock: &MockCtrl, id: &str, url: &str, status: u16, content_type: &str) {
    mock.emit(
        "Fetch.requestPaused",
        json!({
            "requestId": id,
            "resourceType": "Fetch",
            "request": { "url": url, "method": "GET", "headers": { "Cookie": "sid=1" } },
            "responseStatusCode": status,
            "responseHeaders": [{ "name": "Content-Type", "value": content_type }],
        }),
        Some("S1"),
    );
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

fn only(mock: &MockCtrl, method: &str) -> Value {
    let calls = mock.calls_to(method);
    assert_eq!(calls.len(), 1, "expected one {method}, got {calls:?}");
    calls[0].params.clone()
}

fn header_names(command: &Value) -> Vec<String> {
    command["headers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|header| header["name"].as_str().unwrap().to_owned())
        .collect()
}

fn header(command: &Value, name: &str) -> Option<String> {
    command["headers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|header| header["name"] == name)
        .map(|header| header["value"].as_str().unwrap().to_owned())
}

// ----------------------------------------------------------------------
// Starting
// ----------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn plugins_with_no_network_interest_get_no_router() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(Net::new("quiet", &log).caps(INTERCEPT))
        .unwrap();

    let (_browser, _page, mock) = launch(plugins).await;

    assert!(mock.calls_to("Fetch.enable").is_empty());
}

#[tokio::test(start_paused = true)]
async fn the_patterns_are_the_union_of_what_the_plugins_declared() {
    let log = Log::default();
    let blocker = Net::new("blocker", &log)
        .caps(INTERCEPT)
        .interest(NetworkInterest::none().rule(NetRule::block(
            RequestFilter::any().url("https://ads.test/*"),
        )));
    let api = Net::new("api", &log)
        .caps([Capability::NetIntercept, Capability::NetObserve])
        .interest(
            NetworkInterest::none()
                .intercept(RequestFilter::any().url("https://api.test/*"))
                .observe(RequestFilter::any().url("*.json")),
        );
    let plugins = PluginSet::new().with(blocker).unwrap().with(api).unwrap();

    let (_browser, _page, mock) = launch(plugins).await;

    let enable = only(&mock, "Fetch.enable");
    assert_eq!(
        enable["patterns"],
        json!([
            { "urlPattern": "*.json", "requestStage": "Response" },
            { "urlPattern": "https://ads.test/*", "requestStage": "Request" },
            { "urlPattern": "https://api.test/*", "requestStage": "Request" },
        ])
    );
    assert_eq!(enable["handleAuthRequests"], false);
}

#[tokio::test(start_paused = true)]
async fn the_response_stage_is_paused_only_when_an_observe_filter_asks() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("gate", &log)
                .caps(INTERCEPT)
                .interest(intercept_all()),
        )
        .unwrap();

    let (_browser, _page, mock) = launch(plugins).await;

    assert_eq!(
        only(&mock, "Fetch.enable")["patterns"],
        json!([{ "urlPattern": "*", "requestStage": "Request" }])
    );
}

// ----------------------------------------------------------------------
// Request stage
// ----------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_request_nobody_objects_to_goes_on_untouched() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("gate", &log)
                .caps(INTERCEPT)
                .interest(intercept_all()),
        )
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    pause(&mock, "R1", "https://a.test/", "Document");
    settle().await;

    assert_eq!(
        only(&mock, "Fetch.continueRequest"),
        json!({ "requestId": "R1" })
    );
    assert_eq!(
        entries(&log),
        ["gate:request https://a.test/ cookie=[redacted]"]
    );
}

#[tokio::test(start_paused = true)]
async fn any_block_wins_whatever_the_order() {
    for blocker_first in [true, false] {
        let log = Log::default();
        let blocker = Net::new("blocker", &log)
            .caps(INTERCEPT)
            .interest(intercept_all())
            .decide(RequestDecision::Block("no".into()));
        let friend = Net::new("friend", &log)
            .caps(INTERCEPT)
            .interest(intercept_all());
        let plugins = if blocker_first {
            PluginSet::new()
                .with(blocker)
                .unwrap()
                .with(friend)
                .unwrap()
        } else {
            PluginSet::new()
                .with(friend)
                .unwrap()
                .with(blocker)
                .unwrap()
        };
        let (_browser, _page, mock) = launch(plugins).await;

        pause(&mock, "R1", "https://a.test/x", "Fetch");
        settle().await;

        let failed = only(&mock, "Fetch.failRequest");
        assert_eq!(failed["requestId"], "R1");
        assert_eq!(failed["errorReason"], "BlockedByClient");
        assert!(mock.calls_to("Fetch.continueRequest").is_empty());
    }
}

#[tokio::test(start_paused = true)]
async fn a_blocking_rule_fails_the_request_without_calling_the_plugin() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("shield", &log)
                .caps(INTERCEPT)
                .interest(NetworkInterest::none().rule(NetRule::block(
                    RequestFilter::any().kinds([ResourceKind::Image]),
                ))),
        )
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    pause(&mock, "R1", "https://a.test/logo.png", "Image");
    pause(&mock, "R2", "https://a.test/app.js", "Script");
    settle().await;

    assert_eq!(only(&mock, "Fetch.failRequest")["requestId"], "R1");
    assert_eq!(only(&mock, "Fetch.continueRequest")["requestId"], "R2");
    assert!(entries(&log).is_empty(), "rules never call the plugin");
}

#[tokio::test(start_paused = true)]
async fn the_headers_of_the_rules_are_added_to_the_request() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("agent", &log)
                .caps(INTERCEPT)
                .interest(NetworkInterest::none().rule(NetRule::set_headers(
                    RequestFilter::any().url("https://a.test/*"),
                    [("X-Run", "42"), ("Accept", "text/plain")],
                ))),
        )
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    pause(&mock, "R1", "https://a.test/", "Document");
    pause(&mock, "R2", "https://b.test/", "Document");
    settle().await;

    let calls = mock.calls_to("Fetch.continueRequest");
    let changed = calls
        .iter()
        .find(|c| c.params["requestId"] == "R1")
        .unwrap();
    assert_eq!(header(&changed.params, "X-Run").as_deref(), Some("42"));
    assert_eq!(
        header(&changed.params, "Accept").as_deref(),
        Some("text/plain"),
        "a rule's header replaces the request's own"
    );
    assert_eq!(
        header(&changed.params, "Cookie").as_deref(),
        Some("sid=1"),
        "the plugin's header list is raw, nothing is redacted on the wire"
    );
    let untouched = calls
        .iter()
        .find(|c| c.params["requestId"] == "R2")
        .unwrap();
    assert_eq!(untouched.params, json!({ "requestId": "R2" }));
}

#[tokio::test(start_paused = true)]
async fn plugins_see_sensitive_headers_only_with_the_capability() {
    let log = Log::default();
    let blind = Net::new("blind", &log)
        .caps(INTERCEPT)
        .interest(intercept_all());
    let trusted = Net::new("trusted", &log)
        .caps([Capability::NetIntercept, Capability::NetSensitive])
        .interest(intercept_all());
    let plugins = PluginSet::new().with(blind).unwrap().with(trusted).unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    pause(&mock, "R1", "https://a.test/", "Document");
    settle().await;

    let mut seen = entries(&log);
    seen.sort();
    assert_eq!(
        seen,
        [
            "blind:request https://a.test/ cookie=[redacted]",
            "trusted:request https://a.test/ cookie=sid=1",
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_plugin_that_never_decides_fails_the_request_instead_of_leaving_it_paused() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("stuck", &log)
                .caps(INTERCEPT)
                .interest(intercept_all())
                .hanging(),
        )
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    pause(&mock, "R1", "https://a.test/", "Document");
    tokio::time::sleep(Duration::from_secs(11)).await;

    let failed = only(&mock, "Fetch.failRequest");
    assert_eq!(failed["requestId"], "R1");
    assert_eq!(failed["errorReason"], "BlockedByClient");
}

// ----------------------------------------------------------------------
// The event
// ----------------------------------------------------------------------

/// The `reason` of each `cdp.plugin.request.failed` event.
#[derive(Clone, Default)]
struct Events(Arc<Mutex<Vec<String>>>);

impl Events {
    /// The recorded reasons that contain `needle`.
    fn reasons_like(&self, needle: &str) -> Vec<String> {
        let mut found = self.0.lock().unwrap().clone();
        found.retain(|reason| reason.contains(needle));
        found
    }
}

/// The subscriber of the whole test binary, set before the first router runs.
///
/// A subscriber set for one test only is not reliable here: the tests run on
/// several threads, and a callsite another thread reached first may already
/// be cached as uninteresting.
fn capture() -> &'static Events {
    static EVENTS: std::sync::OnceLock<Events> = std::sync::OnceLock::new();
    EVENTS.get_or_init(|| {
        let events = Events::default();
        // A second subscriber in the same binary is not wanted either.
        let _ = tracing::subscriber::set_global_default(events.clone());
        events
    })
}

struct Reason(Option<String>);

impl tracing::field::Visit for Reason {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "reason" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

impl tracing::Subscriber for Events {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        if event.metadata().name() == "cdp.plugin.request.failed" {
            let mut reason = Reason(None);
            event.record(&mut reason);
            self.0.lock().unwrap().push(reason.0.unwrap_or_default());
        }
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

#[tokio::test(start_paused = true)]
async fn a_block_raises_an_event_whose_reason_starts_with_the_plugin() {
    let events = capture();
    // One subscriber for the whole binary (see `capture`); other tests block
    // requests too, so this one looks for its own reason.
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("shield", &log)
                .caps(INTERCEPT)
                .interest(intercept_all())
                .decide(RequestDecision::Block("tracker-event".into())),
        )
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    pause(&mock, "R1", "https://t.test/pixel", "Image");
    settle().await;

    assert_eq!(
        events.reasons_like("tracker-event"),
        ["shield: tracker-event"],
        "one event, and the reason names the plugin first"
    );
    assert_eq!(only(&mock, "Fetch.failRequest")["requestId"], "R1");
}

// ----------------------------------------------------------------------
// The program's own rules
// ----------------------------------------------------------------------

fn gate_that_blocks(log: &Log) -> PluginSet {
    PluginSet::new()
        .with(
            Net::new("gate", log)
                .caps(INTERCEPT)
                .interest(intercept_all())
                .decide(RequestDecision::Block("closed".into())),
        )
        .unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_fulfilment_of_the_program_wins_and_the_plugins_are_not_asked() {
    let log = Log::default();
    let (_browser, page, mock) = launch(gate_that_blocks(&log)).await;
    let interception = page
        .intercept(vec![Rule::fulfill(Response::json(
            200,
            &json!({"plan": "pro"}),
        ))
        .url("*/api/account")])
        .await
        .unwrap();

    pause(&mock, "R1", "https://a.test/api/account", "Fetch");
    pause(&mock, "R2", "https://a.test/other", "Fetch");
    settle().await;

    assert_eq!(only(&mock, "Fetch.fulfillRequest")["requestId"], "R1");
    assert_eq!(
        only(&mock, "Fetch.failRequest")["requestId"],
        "R2",
        "what the program does not answer still meets the plugins"
    );
    assert_eq!(
        entries(&log),
        ["gate:request https://a.test/other cookie=[redacted]"]
    );
    let seen: Vec<_> = interception
        .log()
        .iter()
        .map(|seen| (seen.request.url.clone(), format!("{:?}", seen.outcome)))
        .collect();
    assert_eq!(
        seen,
        [
            (
                "https://a.test/api/account".to_owned(),
                "Fulfilled".to_owned()
            ),
            ("https://a.test/other".to_owned(), "Blocked".to_owned()),
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn an_interception_pauses_everything_and_stopping_it_narrows_the_patterns_again() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(Net::new("api", &log).caps(INTERCEPT).interest(
            NetworkInterest::none().intercept(RequestFilter::any().url("https://api.test/*")),
        ))
        .unwrap();
    let (_browser, page, mock) = launch(plugins).await;

    let interception = page
        .intercept(vec![Rule::block().url("*.png")])
        .await
        .unwrap();
    let during = mock.calls_to("Fetch.enable").pop().unwrap().params;
    interception.stop().await.unwrap();
    let after = mock.calls_to("Fetch.enable").pop().unwrap().params;

    assert_eq!(
        during["patterns"],
        json!([
            { "urlPattern": "*", "requestStage": "Request" },
        ])
    );
    assert_eq!(
        after["patterns"],
        json!([{ "urlPattern": "https://api.test/*", "requestStage": "Request" }])
    );
    assert!(
        mock.calls_to("Fetch.disable").is_empty(),
        "the router keeps Fetch on for the plugins"
    );
}

#[tokio::test(start_paused = true)]
async fn the_headers_of_a_program_rule_win_over_those_of_the_plugins() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("agent", &log)
                .caps(INTERCEPT)
                .interest(NetworkInterest::none().rule(NetRule::set_headers(
                    RequestFilter::any(),
                    [("X-Plugin", "p"), ("X-Both", "p"), ("X-Gone", "p")],
                ))),
        )
        .unwrap();
    let (_browser, page, mock) = launch(plugins).await;
    let _interception = page
        .intercept(vec![Rule::modify()
            .set_header("X-Both", "u")
            .remove_header("X-Gone")])
        .await
        .unwrap();

    pause(&mock, "R1", "https://a.test/", "Document");
    settle().await;

    let sent = only(&mock, "Fetch.continueRequest");
    let mut names = header_names(&sent);
    names.sort();
    assert_eq!(names, ["Accept", "Cookie", "X-Both", "X-Plugin"]);
    assert_eq!(header(&sent, "X-Both").as_deref(), Some("u"));
    assert_eq!(header(&sent, "X-Plugin").as_deref(), Some("p"));
}

#[tokio::test(start_paused = true)]
async fn a_later_interception_keeps_its_rules_when_an_earlier_one_stops() {
    let log = Log::default();
    let (_browser, page, mock) = launch(gate_that_blocks(&log)).await;
    let first = page
        .intercept(vec![Rule::block().url("*.gif")])
        .await
        .unwrap();
    let _second = page
        .intercept(vec![Rule::fulfill(Response::new(204, "")).url("*.png")])
        .await
        .unwrap();

    first.stop().await.unwrap();
    pause(&mock, "R1", "https://a.test/x.png", "Image");
    settle().await;

    assert_eq!(only(&mock, "Fetch.fulfillRequest")["requestId"], "R1");
}

// ----------------------------------------------------------------------
// Response stage
// ----------------------------------------------------------------------

fn observer(log: &Log, caps: impl IntoIterator<Item = Capability>, max_body: usize) -> Net {
    Net::new("watcher", log).caps(caps).interest(
        NetworkInterest::none().observe(RequestFilter::any().url("*.json").max_body(max_body)),
    )
}

#[tokio::test(start_paused = true)]
async fn an_observer_gets_the_status_the_type_and_the_body_up_to_its_limit() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(observer(
            &log,
            [Capability::NetObserve, Capability::NetBodies],
            5,
        ))
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;
    mock.reply(
        "Fetch.getResponseBody",
        json!({ "body": "aGVsbG8gd29ybGQ=", "base64Encoded": true }),
    );

    pause_response(
        &mock,
        "R1",
        "https://a.test/data.json",
        200,
        "application/json; charset=utf-8",
    );
    pause_response(&mock, "R2", "https://a.test/page.html", 200, "text/html");
    settle().await;

    assert_eq!(
        entries(&log),
        ["watcher:response 200 \"application/json\" body=hello truncated=true Ok"]
    );
    assert_eq!(
        only(&mock, "Fetch.getResponseBody")["requestId"],
        "R1",
        "the body of what no filter selects is not read"
    );
    let released: Vec<_> = mock
        .calls_to("Fetch.continueResponse")
        .iter()
        .map(|call| call.params["requestId"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(released, ["R1", "R2"], "every paused response goes on");
}

#[tokio::test(start_paused = true)]
async fn an_observer_without_the_bodies_capability_never_has_a_body_read_for_it() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(observer(&log, [Capability::NetObserve], 100))
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    pause_response(
        &mock,
        "R1",
        "https://a.test/data.json",
        404,
        "application/json",
    );
    settle().await;

    assert!(mock.calls_to("Fetch.getResponseBody").is_empty());
    assert_eq!(
        entries(&log),
        ["watcher:response 404 \"application/json\" body=none truncated=false Ok"]
    );
}

#[tokio::test(start_paused = true)]
async fn a_request_the_network_failed_is_reported_as_failed() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(observer(
            &log,
            [Capability::NetObserve, Capability::NetBodies],
            100,
        ))
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    mock.emit(
        "Fetch.requestPaused",
        json!({
            "requestId": "R1",
            "resourceType": "Fetch",
            "request": { "url": "https://a.test/data.json", "method": "GET", "headers": {} },
            "responseErrorReason": "ConnectionRefused",
        }),
        Some("S1"),
    );
    settle().await;

    assert!(mock.calls_to("Fetch.getResponseBody").is_empty());
    assert_eq!(
        entries(&log),
        ["watcher:response 0 \"\" body=none truncated=false Failed"]
    );
    assert_eq!(only(&mock, "Fetch.continueResponse")["requestId"], "R1");
}

#[tokio::test(start_paused = true)]
async fn a_blocked_request_is_reported_to_the_observers_with_who_blocked_it() {
    let log = Log::default();
    let blocker = Net::new("shield", &log)
        .caps(INTERCEPT)
        .interest(intercept_all())
        .decide(RequestDecision::Block("tracker".into()));
    let plugins = PluginSet::new()
        .with(blocker)
        .unwrap()
        .with(
            Net::new("watcher", &log)
                .caps([Capability::NetObserve])
                .interest(NetworkInterest::none().observe(RequestFilter::any())),
        )
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;

    pause(&mock, "R1", "https://t.test/pixel", "Image");
    settle().await;

    assert_eq!(only(&mock, "Fetch.failRequest")["requestId"], "R1");
    assert_eq!(
        entries(&log),
        [
            "shield:request https://t.test/pixel cookie=[redacted]",
            "watcher:response 0 \"\" body=none truncated=false Blocked { by: \"shield\" }",
        ]
    );
}

// ----------------------------------------------------------------------
// Supervision and the proxy
// ----------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_router_that_fell_behind_releases_what_it_may_have_missed() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("gate", &log)
                .caps(INTERCEPT)
                .interest(intercept_all()),
        )
        .unwrap();
    let (_browser, _page, mock) = launch(plugins).await;
    assert_eq!(mock.calls_to("Fetch.enable").len(), 1);

    // More than the event channel holds, with the router not running meanwhile.
    for number in 0..400 {
        pause(&mock, &format!("R{number}"), "https://a.test/", "Document");
    }
    settle().await;

    assert!(
        !mock.calls_to("Fetch.disable").is_empty(),
        "Fetch was turned off, which lets the waiting requests go"
    );
    assert!(
        mock.calls_to("Fetch.enable").len() >= 2,
        "and on again for the next ones"
    );
}

#[tokio::test(start_paused = true)]
async fn a_proxied_tab_with_a_router_is_answered_once_and_still_gets_its_password() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(
            Net::new("gate", &log)
                .caps(INTERCEPT)
                .interest(intercept_all()),
        )
        .unwrap();
    let (browser, _page, mock) = launch(plugins).await;
    let context = browser
        .new_context_with(
            ContextOptions::new().proxy(Proxy::parse("alice:s3cret@proxy.test:8080").unwrap()),
        )
        .await
        .unwrap();
    context.new_page().await.unwrap();

    let enable = mock.calls_to("Fetch.enable").pop().unwrap().params;
    assert_eq!(enable["handleAuthRequests"], true);
    assert_eq!(
        enable["patterns"],
        json!([{ "urlPattern": "*", "requestStage": "Request" }])
    );

    pause(&mock, "R1", "https://a.test/", "Document");
    mock.emit(
        "Fetch.authRequired",
        json!({
            "requestId": "AUTH1",
            "authChallenge": { "source": "Proxy", "origin": "http://proxy.test:8080" },
        }),
        Some("S1"),
    );
    settle().await;

    assert_eq!(
        only(&mock, "Fetch.continueRequest"),
        json!({ "requestId": "R1" }),
        "the router answers, the proxy responder leaves the request alone"
    );
    let auth = only(&mock, "Fetch.continueWithAuth");
    assert_eq!(auth["requestId"], "AUTH1");
    assert_eq!(
        auth["authChallengeResponse"]["response"],
        "ProvideCredentials"
    );
}
