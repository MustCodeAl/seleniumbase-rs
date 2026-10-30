//! Driving a page over a DevTools channel the caller carries
//! (`Browser::over_channel`), as an app does with the in-process channel of
//! CEF or WebView2. No Chrome is started: a task stands in for the embedder's
//! channel. It takes every command the transport hands over, records it, and
//! answers through the inbox the way a page's DevTools agent would.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, CdpInbox, CdpRequest, StealthPlan, TransportError};
use seleniumbase_rs::{Fingerprint, OsType, SeleniumBaseError};
use serde_json::{json, Value};
use tokio::sync::mpsc;

/// What the stand-in agent says to one command: `None` never answers it.
type Answer = Option<Result<Value, String>>;

struct Channel {
    browser: Browser,
    inbox: CdpInbox,
    log: Arc<Mutex<Vec<CdpRequest>>>,
}

impl Channel {
    /// Every command handed over so far, oldest first.
    fn sent(&self) -> Vec<CdpRequest> {
        self.log.lock().unwrap().clone()
    }

    fn methods(&self) -> Vec<String> {
        self.sent()
            .into_iter()
            .map(|request| request.method)
            .collect()
    }
}

/// A browser over a channel whose far end answers with `answer`.
fn channel_with(
    mut answer: impl FnMut(&CdpRequest, &CdpInbox) -> Answer + Send + 'static,
) -> Channel {
    let (outgoing, mut incoming) = mpsc::unbounded_channel::<CdpRequest>();
    let (browser, inbox) =
        Browser::over_channel(move |request: CdpRequest| -> Result<(), TransportError> {
            outgoing
                .send(request)
                .map_err(|_| "the agent is gone".into())
        });
    let log = Arc::new(Mutex::new(Vec::new()));
    let (agent_inbox, agent_log) = (inbox.clone(), Arc::clone(&log));
    tokio::spawn(async move {
        while let Some(request) = incoming.recv().await {
            agent_log.lock().unwrap().push(request.clone());
            if let Some(outcome) = answer(&request, &agent_inbox) {
                agent_inbox.respond(request.id, outcome);
            }
        }
    });
    Channel {
        browser,
        inbox,
        log,
    }
}

/// A page's agent: navigations load at once, scripts read a finished page,
/// and the commands a page agent does not have are refused as Chromium
/// refuses them.
fn page_agent(request: &CdpRequest, inbox: &CdpInbox) -> Answer {
    let method = request.method.as_str();
    Some(match method {
        "Page.navigate" | "Page.reload" => {
            inbox.event("Page.loadEventFired", json!({ "timestamp": 1.0 }), None);
            Ok(json!({ "frameId": "F1", "loaderId": "L1" }))
        }
        "Runtime.evaluate" => {
            let expression = request.params["expression"].as_str().unwrap_or_default();
            let value = if expression.contains("location.href") {
                json!({ "url": "tauri://localhost/", "title": "My App" })
            } else {
                json!("complete")
            };
            Ok(json!({ "result": { "type": "object", "value": value } }))
        }
        _ if method.starts_with("Target.") || method.starts_with("Browser.") => {
            Err(format!("'{method}' wasn't found"))
        }
        _ => Ok(json!({})),
    })
}

#[tokio::test]
async fn the_page_is_prepared_over_the_channel_without_targets_or_sessions() {
    let channel = channel_with(page_agent);

    let page = channel.browser.default_page().await.unwrap();

    let sent = channel.sent();
    assert!(
        sent.iter().all(|request| request.session_id.is_none()),
        "the channel reaches one page, so nothing names a session"
    );
    let methods = channel.methods();
    assert!(
        methods.iter().all(|method| !method.starts_with("Target.")),
        "{methods:?}"
    );
    for wanted in ["Page.enable", "Page.addScriptToEvaluateOnNewDocument"] {
        assert!(methods.iter().any(|method| method == wanted), "{methods:?}");
    }
    assert_eq!(page.id(), channel.browser.list_pages().await.unwrap()[0].id);
    assert!(channel.browser.is_connected());
}

#[tokio::test]
async fn the_channels_page_is_listed_with_its_address_and_title() {
    let channel = channel_with(page_agent);

    let pages = channel.browser.list_pages().await.unwrap();

    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].url, "tauri://localhost/");
    assert_eq!(pages[0].title, "My App");
}

#[tokio::test]
async fn goto_waits_for_the_load_event_the_channel_reports() {
    let channel = channel_with(page_agent);
    let page = channel.browser.default_page().await.unwrap();

    tokio::time::timeout(
        Duration::from_secs(5),
        page.goto("https://example.com/login"),
    )
    .await
    .expect("the load event ends the wait")
    .unwrap();

    let navigate = channel
        .sent()
        .into_iter()
        .find(|request| request.method == "Page.navigate")
        .expect("a navigation was sent");
    assert_eq!(navigate.params["url"], "https://example.com/login");
    assert_eq!(navigate.session_id, None);
}

#[tokio::test]
async fn the_stealth_overrides_reach_the_page_in_order_and_unchanged() {
    let channel = channel_with(page_agent);
    let plan = StealthPlan::new(&Fingerprint::randomized(OsType::Windows, 21));

    for (method, params) in plan.overrides() {
        channel
            .browser
            .execute(method, params.clone())
            .await
            .unwrap();
    }

    let sent: Vec<(String, Value)> = channel
        .sent()
        .into_iter()
        .map(|request| (request.method, request.params))
        .collect();
    let planned: Vec<(String, Value)> = plan
        .overrides()
        .map(|(method, params)| (method.to_owned(), params.clone()))
        .collect();
    assert_eq!(sent, planned);
    assert!(planned
        .iter()
        .any(|(method, _)| method == "Network.setUserAgentOverride"));
}

#[tokio::test]
async fn a_command_the_agent_refuses_fails_with_its_message() {
    let channel = channel_with(|request, inbox| {
        if request.method == "Emulation.setTimezoneOverride" {
            Some(Err("Invalid timezone ID".to_owned()))
        } else {
            page_agent(request, inbox)
        }
    });

    let error = channel
        .browser
        .execute(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "Mars/Base" }),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::CdpDriver(_)),
        "{error:?}"
    );
    let shown = error.to_string();
    assert!(shown.contains("Emulation.setTimezoneOverride"), "{shown}");
    assert!(shown.contains("Invalid timezone ID"), "{shown}");
}

#[tokio::test]
async fn closing_the_inbox_fails_a_waiting_command_at_once() {
    let channel = channel_with(|_, _| None);
    let browser = channel.browser.clone();
    let waiting = tokio::spawn(async move { browser.execute("Page.enable", json!({})).await });
    while channel.sent().is_empty() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    channel.inbox.close();

    let outcome = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .expect("closing does not leave the command to its timeout")
        .unwrap();
    assert!(
        matches!(outcome, Err(SeleniumBaseError::BrowserDisconnected { .. })),
        "{outcome:?}"
    );
    assert!(!channel.browser.is_connected());
    let later = channel.browser.execute("Page.enable", json!({})).await;
    assert!(later.is_err());
    assert_eq!(channel.sent().len(), 1, "nothing goes out after the close");
}

#[tokio::test]
async fn a_transport_that_cannot_hand_over_fails_the_command() {
    let (browser, _inbox) = Browser::over_channel(|_: CdpRequest| -> Result<(), TransportError> {
        Err("the webview was destroyed".into())
    });

    let error = browser.execute("Page.enable", json!({})).await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::BrowserDisconnected { .. }),
        "{error:?}"
    );
    let shown = error.to_string();
    assert!(shown.contains("Page.enable"), "{shown}");
    assert!(shown.contains("the webview was destroyed"), "{shown}");
}

#[tokio::test]
async fn raw_messages_are_taken_as_a_websocket_would_carry_them() {
    let (outgoing, mut incoming) = mpsc::unbounded_channel::<CdpRequest>();
    let (browser, inbox) =
        Browser::over_channel(move |request: CdpRequest| -> Result<(), TransportError> {
            outgoing.send(request).map_err(|_| "gone".into())
        });
    let mut events = browser.events();
    let agent = inbox.clone();
    tokio::spawn(async move {
        while let Some(request) = incoming.recv().await {
            let reply = format!(
                r#"{{"id":{},"result":{{"product":"Chrome/140"}}}}"#,
                request.id
            );
            agent.deliver(&reply).unwrap();
            agent
                .deliver(r#"{"method":"Page.frameNavigated","params":{"frame":{"id":"F1"}}}"#)
                .unwrap();
        }
    });

    let answer = browser
        .execute("Browser.getVersion", json!({}))
        .await
        .unwrap();

    assert_eq!(answer["product"], "Chrome/140");
    let event = events
        .wait_for("Page.frameNavigated", Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(event.params["frame"]["id"], "F1");
    assert!(inbox.deliver("{ not json").is_err());
}

#[tokio::test]
async fn opening_another_tab_is_refused_without_asking_the_browser() {
    let channel = channel_with(page_agent);

    let error = channel.browser.new_page().await.unwrap_err();

    assert!(error.to_string().contains("channel"), "{error}");
    assert!(
        !channel
            .methods()
            .iter()
            .any(|method| method == "Target.createTarget"),
        "{:?}",
        channel.methods()
    );
}

#[tokio::test]
async fn closing_the_browser_detaches_nothing_and_ends_the_channel() {
    let channel = channel_with(page_agent);
    let _page = channel.browser.default_page().await.unwrap();

    channel.browser.close().await.unwrap();

    let methods = channel.methods();
    assert!(
        methods
            .iter()
            .all(|method| !method.starts_with("Target.") && !method.starts_with("Browser.")),
        "{methods:?}"
    );
    assert!(!channel.inbox.is_open());
    assert!(!channel.browser.is_connected());
}
