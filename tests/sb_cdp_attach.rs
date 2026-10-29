//! External Chromium attachment and Tauri discovery over real mock HTTP/WS sockets.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use seleniumbase_rs::api::cdp_driver::CdpDriver;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{BaseCase, BrowserConfig, BrowserSession, Fingerprint, SeleniumBaseError};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

struct AttachServer {
    http: String,
    websocket: String,
    calls: Arc<Mutex<Vec<Value>>>,
    requests: Arc<Mutex<Vec<String>>>,
    /// How many DevTools WebSocket connections the server accepted.
    connections: Arc<AtomicUsize>,

    tasks: Vec<JoinHandle<()>>,
}

impl AttachServer {
    async fn start(discovery: Option<Value>) -> Self {
        Self::start_advertising(discovery, None).await
    }

    /// A server whose `/json/version` advertises `advertised` instead of its
    /// own browser WebSocket, as a misbehaving or hostile debugger would.
    async fn start_advertising(discovery: Option<Value>, advertised: Option<String>) -> Self {
        // One port serves both, as a real DevTools endpoint does.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let http = format!("http://{address}");
        let websocket = format!("ws://{address}/devtools/browser/tauri");
        let port = address.port().to_string();
        let ws_url = advertised.map_or_else(
            || websocket.clone(),
            |url| url.replace("{http_port}", &port),
        );
        let calls = Arc::new(Mutex::new(Vec::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let (recorded, paths, counted) = (calls.clone(), requests.clone(), connections.clone());
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let (discovery, ws_url) = (discovery.clone(), ws_url.clone());
                let (recorded, paths, counted) = (recorded.clone(), paths.clone(), counted.clone());
                tokio::spawn(async move {
                    if wants_websocket(&stream).await {
                        counted.fetch_add(1, Ordering::SeqCst);
                        serve_websocket(stream, recorded).await;
                    } else {
                        serve_http(stream, paths, discovery, ws_url).await;
                    }
                });
            }
        });
        Self {
            http,
            websocket,
            calls,
            requests,
            connections,
            tasks: vec![task],
        }
    }

    fn calls(&self) -> Vec<Value> {
        self.calls.lock().unwrap().clone()
    }
}

/// Whether the request waiting on `stream` asks for a WebSocket upgrade. The
/// request is only peeked at, so the handshake or the HTTP reply still reads it.
async fn wants_websocket(stream: &TcpStream) -> bool {
    let mut buffer = [0_u8; 2048];
    for _ in 0..200 {
        let read = stream.peek(&mut buffer).await.unwrap_or(0);
        let head = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
        if head.contains("\r\n\r\n") {
            return head.contains("upgrade: websocket");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    false
}

/// Answers the browser-level DevTools commands of one WebSocket connection,
/// recording each.
async fn serve_websocket(stream: TcpStream, recorded: Arc<Mutex<Vec<Value>>>) {
    let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
    while let Some(Ok(Message::Text(text))) = socket.next().await {
        let call: Value = serde_json::from_str(&text).unwrap();
        let result = match call["method"].as_str().unwrap() {
            "Target.getTargets" => json!({ "targetInfos": [
                { "targetId": "T1", "type": "page", "url": "https://tauri.localhost/", "title": "Tauri" },
                { "targetId": "T2", "type": "page", "url": "https://tauri.localhost/settings", "title": "Settings" }
            ] }),
            "Target.createTarget" => json!({ "targetId": "T3" }),
            "Target.attachToTarget" => {
                json!({ "sessionId": format!("S{}", call["params"]["targetId"].as_str().unwrap()) })
            }
            "Target.getTargetInfo" => {
                json!({ "targetInfo": { "browserContextId": "app" } })
            }
            "Page.addScriptToEvaluateOnNewDocument" => json!({ "identifier": "init" }),
            "Runtime.evaluate" => {
                json!({ "result": { "type": "string", "value": "Tauri window" } })
            }
            "Browser.getVersion" => json!({ "userAgent": "Chrome/155.0.0.0" }),
            _ => json!({}),
        };
        recorded.lock().unwrap().push(call.clone());
        let mut reply = json!({ "id": call["id"], "result": result });
        if let Some(session) = call.get("sessionId") {
            reply["sessionId"] = session.clone();
        }
        if socket
            .send(Message::Text(reply.to_string().into()))
            .await
            .is_err()
        {
            break;
        }
    }
}

/// Answers one HTTP request: the plugin's `/app-token/cdp` discovery and the
/// debugger's `/json/version`.
async fn serve_http(
    mut stream: TcpStream,
    paths: Arc<Mutex<Vec<String>>>,
    discovery: Option<Value>,
    ws_url: String,
) {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        match stream.read_u8().await {
            Ok(byte) => bytes.push(byte),
            Err(_) => return,
        }
    }
    let request = String::from_utf8(bytes).unwrap();
    let path = request.split_whitespace().nth(1).unwrap();
    paths.lock().unwrap().push(path.to_owned());
    let body = if path == "/app-token/cdp" {
        discovery
            .unwrap_or(json!({ "value": { "cdpEnabled": false, "cdpPort": null, "cdpUrl": null } }))
    } else if path == "/json/version" {
        json!({ "webSocketDebuggerUrl": ws_url, "User-Agent": "Chrome/155.0.0.0" })
    } else {
        json!({})
    }
    .to_string();
    let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = stream.write_all(reply.as_bytes()).await;
}

impl Drop for AttachServer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn fingerprint() -> Fingerprint {
    Fingerprint::windows_desktop()
        .to_builder()
        .native_spoofing(true)
        .build()
        .unwrap()
}

/// The host app's second window (target `T2`, session `ST2`) is not the one the
/// automation selected, so nothing is sent to it, and nothing attaches to it.
fn assert_second_window_untouched(server: &AttachServer) {
    let calls = server.calls();
    assert!(
        !calls.iter().any(|call| call["sessionId"] == "ST2"
            || (call["method"] == "Target.attachToTarget" && call["params"]["targetId"] == "T2")),
        "the app's other window was touched: {calls:#?}"
    );
}

fn assert_identity(server: &AttachServer) {
    let calls = server.calls();
    let source = seleniumbase_rs::stealth::evasions::session_script(&fingerprint());
    for session in ["ST1"] {
        assert!(
            calls.iter().any(
                |call| call["method"] == "Page.addScriptToEvaluateOnNewDocument"
                    && call["params"]["source"] == source
                    && call["params"]["runImmediately"] == true
                    && call["sessionId"] == session
            ),
            "existing tab {session} must receive the provider registry script immediately"
        );
        for method in [
            "Network.setUserAgentOverride",
            "Emulation.setDeviceMetricsOverride",
            "Emulation.setTimezoneOverride",
            "Emulation.setLocaleOverride",
        ] {
            assert!(
                calls
                    .iter()
                    .any(|call| call["method"] == method && call["sessionId"] == session),
                "missing {method} on {session}"
            );
        }
    }
    assert_second_window_untouched(server);

    assert!(!calls
        .iter()
        .any(|call| call["method"] == "Target.createTarget" || call["method"] == "Browser.close"));
}

#[tokio::test]
async fn attaches_without_launching_and_reuses_the_existing_driver() {
    let server = AttachServer::start(None).await;
    let options = LaunchOptions::builder()
        .cdp_url(&server.http)
        .startup_timeout(Duration::from_secs(2))
        .executable_path("/no-browser-is-installed")
        .fingerprint(&fingerprint())
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.unwrap();
    assert!(
        server.calls().is_empty(),
        "attaching must send nothing to the app"
    );
    let mut driver = CdpDriver::new();
    driver.attach(browser.clone()).await.unwrap();
    assert_eq!(
        driver.evaluate("document.title").await.unwrap(),
        "Tauri window"
    );
    driver.stop().await.unwrap();
    browser.close().await.unwrap();
    assert_identity(&server);
}

#[tokio::test]
async fn websocket_attachment_prepares_only_the_window_that_is_selected() {
    let server = AttachServer::start(None).await;
    let options = LaunchOptions::builder()
        .cdp_url(&server.websocket)
        .fingerprint(&fingerprint())
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.unwrap();
    assert!(
        server
            .calls()
            .iter()
            .all(|call| call["method"] == "Browser.getVersion"),
        "only the browser's version is read"
    );
    browser.default_page().await.unwrap();
    assert_identity(&server);
    browser.close().await.unwrap();
    assert!(server.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn base_case_discovers_the_plugin_and_uses_the_pure_cdp_session() {
    let debugger = AttachServer::start(None).await;
    let port = url::Url::parse(&debugger.http).unwrap().port().unwrap();
    let plugin = AttachServer::start(Some(json!({ "value": {
        "cdpEnabled": true, "cdpPort": port, "cdpUrl": debugger.http
    } })))
    .await;
    let config = BrowserConfig::default()
        .with_webdriver_url(format!("{}/app-token/", plugin.http))
        .with_cdp_discovery(true)
        .with_fingerprint(fingerprint());
    let mut case = BaseCase::new(config).await.unwrap();
    assert!(case.is_cdp_mode_active());
    assert!(case.is_connected().await);
    assert_eq!(
        case.execute_script("return document.title;").await.unwrap(),
        "Tauri window"
    );
    case.activate_cdp_mode().await.unwrap();
    assert_eq!(case.cdp_get_title().await.unwrap(), "Tauri window");
    case.cdp_open("https://tauri.localhost/next").await.unwrap();
    case.quit().await.unwrap();
    assert_identity(&debugger);
    assert_eq!(*plugin.requests.lock().unwrap(), ["/app-token/cdp"]);
}

#[tokio::test]
async fn discovery_refuses_disabled_and_non_loopback_endpoints_without_leaking_tokens() {
    for discovery in [
        json!({ "value": { "cdpEnabled": false, "cdpPort": null, "cdpUrl": null } }),
        json!({ "value": { "cdpEnabled": true, "cdpPort": 9222, "cdpUrl": "http://example.com:9222" } }),
        json!({ "value": { "cdpEnabled": true, "cdpPort": 1, "cdpUrl": "http://127.0.0.1:9222" } }),
    ] {
        let plugin = AttachServer::start(Some(discovery)).await;
        let options = LaunchOptions::builder()
            .cdp_url(format!("{}/app-token/", plugin.http))
            .build()
            .unwrap();
        let error = Browser::launch(options).await.unwrap_err().to_string();
        assert!(!error.contains("app-token"), "{error}");
        assert!(plugin.calls().is_empty());
    }
}

#[test]
fn config_is_additive_and_passed_through_the_existing_builder() {
    let mut old = serde_json::to_value(BrowserConfig::default()).unwrap();
    old.as_object_mut().unwrap().remove("cdp_url");
    old.as_object_mut().unwrap().remove("cdp_discovery");
    let old: BrowserConfig = serde_json::from_value(old).unwrap();
    assert!(old.cdp_url.is_none() && !old.cdp_discovery);
    let config = old.with_cdp_url("ws://127.0.0.1:9222/devtools/browser/app-token");
    let options = LaunchOptions::builder()
        .browser_config(&config)
        .build()
        .unwrap();
    assert_eq!(options.cdp_url(), config.cdp_url.as_deref());
    assert!(!format!("{options:?}").contains("app-token"));
    assert!(!format!("{config:?}").contains("app-token"));
    let discovery = BrowserConfig::default()
        .with_webdriver_url("http://127.0.0.1:4445/app-token/")
        .with_cdp_discovery(true);
    assert!(!format!("{discovery:?}").contains("app-token"));
}

fn sent(server: &AttachServer, method: &str) -> Vec<Value> {
    server
        .calls()
        .into_iter()
        .filter(|call| call["method"] == method)
        .collect()
}

fn attached_options(server: &AttachServer) -> LaunchOptions {
    LaunchOptions::builder()
        .cdp_url(&server.http)
        .fingerprint(&fingerprint())
        .build()
        .unwrap()
}

/// The reply of a Tauri plugin that points at `debugger`.
fn tauri_discovery(debugger: &AttachServer) -> Value {
    let port = url::Url::parse(&debugger.http).unwrap().port().unwrap();
    json!({ "value": { "cdpEnabled": true, "cdpPort": port, "cdpUrl": debugger.http } })
}

/// A loopback HTTP server that answers every request with `response` as it is.
async fn canned_http(response: Vec<u8>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                match stream.read_u8().await {
                    Ok(byte) => request.push(byte),
                    Err(_) => break,
                }
            }
            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        }
    });
    (base, task)
}

#[tokio::test]
async fn attaching_leaves_every_window_of_the_app_alone_until_one_is_picked() {
    let server = AttachServer::start(None).await;
    let browser = Browser::launch(attached_options(&server)).await.unwrap();
    assert!(server.calls().is_empty(), "{:#?}", server.calls());

    // Listing the windows reads them and prepares none.
    assert_eq!(browser.list_pages().await.unwrap().len(), 2);
    assert!(sent(&server, "Target.attachToTarget").is_empty());

    // Picking one by its id prepares that one and no other.
    browser.page("T2").await.unwrap();
    let attached = sent(&server, "Target.attachToTarget");
    assert_eq!(attached.len(), 1, "{attached:#?}");
    assert_eq!(attached[0]["params"]["targetId"], "T2");
    assert!(!server.calls().iter().any(|call| call["sessionId"] == "ST1"));
    browser.close().await.unwrap();
}

#[tokio::test]
async fn a_tab_the_automation_opens_is_prepared_and_the_apps_windows_are_not() {
    let server = AttachServer::start(None).await;
    let browser = Browser::launch(attached_options(&server)).await.unwrap();
    let _page = browser.new_page().await.unwrap();

    let calls = server.calls();
    assert!(
        calls
            .iter()
            .any(|call| call["method"] == "Network.setUserAgentOverride"
                && call["sessionId"] == "ST3"),
        "the new tab gets the fingerprint: {calls:#?}"
    );
    assert!(
        !calls
            .iter()
            .any(|call| call["sessionId"] == "ST1" || call["sessionId"] == "ST2"),
        "the app's windows were touched: {calls:#?}"
    );
    browser.close().await.unwrap();
}

#[tokio::test]
async fn closing_an_attached_browser_detaches_its_sessions_and_leaves_the_app_open() {
    let server = AttachServer::start(None).await;
    let browser = Browser::launch(attached_options(&server)).await.unwrap();
    let page = browser.default_page().await.unwrap();
    let _opened = browser.new_page().await.unwrap();
    browser.close().await.unwrap();

    let mut detached: Vec<String> = sent(&server, "Target.detachFromTarget")
        .iter()
        .map(|call| call["params"]["sessionId"].as_str().unwrap().to_owned())
        .collect();
    detached.sort();
    assert_eq!(detached, ["ST1", "ST3"]);
    for method in ["Browser.close", "Target.closeTarget"] {
        assert!(sent(&server, method).is_empty(), "{method} was sent");
    }
    assert!(!browser.is_connected());

    // Every handle of the closed browser fails, instead of driving the app.
    let error = browser.new_page().await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::BrowserDisconnected { .. }),
        "{error:?}"
    );
    let error = page.title().await.unwrap_err().to_string();
    assert!(error.contains("closed"), "{error}");
    // A second close is harmless.
    browser.close().await.unwrap();
}

#[test]
fn a_proxy_cannot_be_set_on_an_attached_browser() {
    let url = "http://127.0.0.1:9222";
    for builder in [
        LaunchOptions::builder()
            .cdp_url(url)
            .proxy("http://alice:hunter2@proxy.example:3128"),
        LaunchOptions::builder()
            .cdp_url(url)
            .proxy("http://proxy.example:3128"),
        LaunchOptions::builder()
            .cdp_url(url)
            .proxy_pac_url("http://pac.example/proxy.pac"),
    ] {
        let error = builder.build().unwrap_err();
        let text = error.to_string();
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)) && text.contains("proxy"),
            "{text}"
        );
        assert!(
            !text.contains("hunter2") && !text.contains("alice"),
            "{text}"
        );
    }
    // The same settings are fine for a browser the library launches.
    LaunchOptions::builder()
        .proxy("http://alice:hunter2@proxy.example:3128")
        .build()
        .unwrap();
}

#[tokio::test]
async fn base_case_refuses_a_proxy_on_an_attached_app_before_it_connects() {
    let server = AttachServer::start(None).await;
    let config = BrowserConfig::default()
        .with_cdp_url(&server.http)
        .with_proxy("alice:hunter2@proxy.example:3128");
    let error = BaseCase::new(config)
        .await
        .err()
        .expect("a proxy is refused");
    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "{error:?}"
    );
    assert!(!error.to_string().contains("hunter2"), "{error}");
    assert!(server.calls().is_empty());
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn a_user_agent_or_locale_needs_a_fingerprint_on_an_attached_browser() {
    let url = "http://127.0.0.1:9222";
    for builder in [
        LaunchOptions::builder()
            .cdp_url(url)
            .user_agent("Custom/1.0"),
        LaunchOptions::builder().cdp_url(url).lang("de-DE"),
    ] {
        let error = builder.build().unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::InvalidConfig(text) if text.contains("fingerprint")),
            "{error:?}"
        );
    }
    // A fingerprint applies its own user agent and locale to the tabs it prepares.
    LaunchOptions::builder()
        .cdp_url(url)
        .fingerprint(&fingerprint())
        .build()
        .unwrap();
    // A browser the library launches takes both as command-line flags.
    LaunchOptions::builder()
        .user_agent("Custom/1.0")
        .lang("de-DE")
        .build()
        .unwrap();
}

#[tokio::test]
async fn base_case_refuses_a_user_agent_on_an_attached_app_before_it_connects() {
    let server = AttachServer::start(None).await;
    let mut config = BrowserConfig::default().with_cdp_url(&server.http);
    config.user_agent = Some("Custom/1.0".to_owned());
    let error = BaseCase::new(config).await.err().expect("refused");
    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "{error:?}"
    );
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn reconnect_attaches_to_the_same_endpoint_and_starts_no_driver() {
    let server = AttachServer::start(None).await;
    let mut config = BrowserConfig::default()
        .with_cdp_url(&server.http)
        .with_fingerprint(fingerprint());
    // Were a driver started, it would be a local chromedriver: not here.
    config.auto_start_driver = false;
    let mut case = BaseCase::new(config).await.unwrap();
    assert_eq!(server.connections.load(Ordering::SeqCst), 1);

    case.reconnect().await.unwrap();

    assert_eq!(server.connections.load(Ordering::SeqCst), 2);
    assert!(case.is_cdp_mode_active());
    assert_eq!(case.cdp_get_title().await.unwrap(), "Tauri window");
    // The first attachment was let go before the second began.
    assert!(!sent(&server, "Target.detachFromTarget").is_empty());
    case.quit().await.unwrap();
    assert!(!sent(&server, "Target.detachFromTarget").is_empty());
}

#[tokio::test]
async fn a_webdriver_call_on_a_cdp_attachment_is_an_error_not_a_panic() {
    let server = AttachServer::start(None).await;
    let config = BrowserConfig::default().with_cdp_url(&server.http);
    let mut session = BrowserSession::connect(config.clone()).await.unwrap();
    for error in [
        session.back().await.unwrap_err(),
        session.forward().await.unwrap_err(),
        session.refresh().await.unwrap_err(),
        session.page_source().await.unwrap_err(),
        session.click(thirtyfour::By::Css("#go")).await.unwrap_err(),
    ] {
        assert!(
            matches!(&error, SeleniumBaseError::Unsupported(text) if text.contains("CDP")),
            "{error:?}"
        );
    }
    let mut case = BaseCase::with_session(config, session);
    let error = case.click("#go").await.unwrap_err();
    assert!(
        !matches!(error, SeleniumBaseError::SessionNotStarted),
        "{error:?}"
    );
    case.quit().await.unwrap();
}

#[tokio::test]
async fn a_discovered_debugger_must_advertise_its_own_loopback_endpoint() {
    let elsewhere = AttachServer::start(None).await;
    for advertised in [
        // Another loopback debugger.
        elsewhere.websocket.clone(),
        // The discovered port, on a host that is not this machine.
        "ws://example.com:{http_port}/devtools/browser/x".to_owned(),
    ] {
        let debugger = AttachServer::start_advertising(None, Some(advertised)).await;
        let plugin = AttachServer::start(Some(tauri_discovery(&debugger))).await;
        let options = LaunchOptions::builder()
            .cdp_url(format!("{}/app-token/", plugin.http))
            .startup_timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let error = Browser::launch(options).await.unwrap_err().to_string();
        assert!(error.contains("outside"), "{error}");
        assert!(!error.contains("app-token"), "{error}");
    }
    assert_eq!(elsewhere.connections.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_oversized_discovery_reply_is_refused_not_buffered() {
    const HEAD: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n";
    // One reply announces a huge body and sends none; the other sends an
    // endless-looking one with no length at all.
    let announced = format!("{HEAD}Content-Length: 200000000\r\nConnection: close\r\n\r\n");
    let mut unannounced = format!("{HEAD}Connection: close\r\n\r\n{{\"pad\":\"").into_bytes();
    unannounced.extend(std::iter::repeat_n(b' ', 3 << 20));
    for reply in [announced.into_bytes(), unannounced] {
        let (base, task) = canned_http(reply).await;
        for address in [base.clone(), format!("{base}/app-token/")] {
            let options = LaunchOptions::builder()
                .cdp_url(address)
                .startup_timeout(Duration::from_secs(2))
                .build()
                .unwrap();
            let error = tokio::time::timeout(Duration::from_secs(10), Browser::launch(options))
                .await
                .expect("the size limit ends the attach")
                .unwrap_err()
                .to_string();
            assert!(error.contains("too large"), "{error}");
            assert!(!error.contains("app-token"), "{error}");
        }
        task.abort();
    }
}

#[tokio::test]
async fn discovery_errors_name_the_failing_step_and_the_http_status() {
    let reply = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    let (base, task) = canned_http(reply.to_vec()).await;
    for (address, step) in [
        (format!("{base}/app-token/"), "Tauri discovery"),
        (base.clone(), "version probe"),
    ] {
        let options = LaunchOptions::builder()
            .cdp_url(address)
            .startup_timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let error = Browser::launch(options).await.unwrap_err().to_string();
        assert!(error.contains(step), "{error}");
        assert!(error.contains("404"), "{error}");
        assert!(!error.contains("app-token"), "{error}");
    }
    task.abort();
}

#[tokio::test]
async fn a_window_is_picked_by_its_target_id_or_by_part_of_its_url() {
    let server = AttachServer::start(None).await;
    let browser = Browser::launch(attached_options(&server)).await.unwrap();

    browser.select_page("/settings").await.unwrap();
    let attached = sent(&server, "Target.attachToTarget");
    assert_eq!(attached.len(), 1, "{attached:#?}");
    assert_eq!(attached[0]["params"]["targetId"], "T2");

    // A target id takes precedence over URL text, and a window already picked
    // is not attached to again.
    browser.select_page("T2").await.unwrap();
    assert_eq!(sent(&server, "Target.attachToTarget").len(), 1);
    assert_second_window_is_the_only_one_touched(&server);

    let error = browser.select_page("nothing-has-this").await.unwrap_err();
    assert!(
        matches!(&error, SeleniumBaseError::CdpDriver(text) if text.contains("nothing-has-this")),
        "{error:?}"
    );
    let error = browser.select_page("").await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "{error:?}"
    );
    browser.close().await.unwrap();
}

/// Nothing was sent to the app's first window (target `T1`, session `ST1`).
fn assert_second_window_is_the_only_one_touched(server: &AttachServer) {
    let calls = server.calls();
    assert!(
        !calls.iter().any(|call| call["sessionId"] == "ST1"
            || (call["method"] == "Target.attachToTarget" && call["params"]["targetId"] == "T1")),
        "the app's first window was touched: {calls:#?}"
    );
}

#[tokio::test]
async fn base_case_drives_the_window_named_in_the_configuration() {
    let server = AttachServer::start(None).await;
    let config = BrowserConfig::default()
        .with_cdp_url(&server.http)
        .with_cdp_target("/settings")
        .with_fingerprint(fingerprint());
    let mut case = BaseCase::new(config).await.unwrap();
    assert_eq!(case.cdp_get_title().await.unwrap(), "Tauri window");
    assert_second_window_is_the_only_one_touched(&server);
    assert!(sent(&server, "Network.setUserAgentOverride")
        .iter()
        .all(|call| call["sessionId"] == "ST2"));
    case.quit().await.unwrap();
}

#[test]
fn the_window_to_drive_survives_a_round_trip_through_serde_and_the_builder() {
    let config = BrowserConfig::default()
        .with_cdp_url("http://127.0.0.1:9222")
        .with_cdp_target("/settings");
    let again: BrowserConfig =
        serde_json::from_value(serde_json::to_value(&config).unwrap()).unwrap();
    assert_eq!(again.cdp_target.as_deref(), Some("/settings"));
    let options = LaunchOptions::builder()
        .browser_config(&again)
        .build()
        .unwrap();
    assert_eq!(options.cdp_target(), Some("/settings"));
    // A configuration written before the setting existed has none.
    let mut old = serde_json::to_value(BrowserConfig::default()).unwrap();
    assert!(old.as_object_mut().unwrap().remove("cdp_target").is_none());
    let old: BrowserConfig = serde_json::from_value(old).unwrap();
    assert_eq!(old.cdp_target, None);
}

#[tokio::test]
async fn closing_resets_what_the_automation_set_for_the_whole_browser() {
    let server = AttachServer::start(None).await;
    let browser = Browser::launch(attached_options(&server)).await.unwrap();
    let downloads = tempfile::tempdir().unwrap();
    browser.grant_all_permissions().await.unwrap();
    browser.set_download_dir(downloads.path()).await.unwrap();
    assert!(sent(&server, "Browser.resetPermissions").is_empty());

    browser.close().await.unwrap();

    assert_eq!(sent(&server, "Browser.resetPermissions").len(), 1);
    let behaviour = sent(&server, "Browser.setDownloadBehavior");
    assert_eq!(behaviour.len(), 2, "{behaviour:#?}");
    assert_eq!(behaviour[1]["params"]["behavior"], "default");
}

#[cfg(feature = "plugins")]
mod plugins {
    use super::*;
    use seleniumbase_rs::plugin_api::{
        async_trait, BrowserHandle, LaunchSettings, NavDecision, Navigation, Package, PageHandle,
        Plugin, PluginSet, Requirements, Result as PluginResult,
    };

    struct AttachPlugin(Arc<Mutex<Vec<&'static str>>>);

    #[async_trait]
    impl Plugin for AttachPlugin {
        fn name(&self) -> &str {
            "tauri-attach-test"
        }
        fn packages(&self) -> Vec<Package> {
            vec![Package::script("window.tauriPluginStealth = true;").everywhere()]
        }
        async fn before_launch(&self, _settings: &mut LaunchSettings) -> PluginResult<()> {
            panic!("an external app must not run launch hooks");
        }
        async fn on_browser(&self, _browser: &dyn BrowserHandle) -> PluginResult<()> {
            self.0.lock().unwrap().push("browser");
            Ok(())
        }
        async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
            self.0.lock().unwrap().push("page");
            page.add_init_script("window.tauriPluginHook = true;").await
        }
        async fn before_navigate(
            &self,
            _page: &dyn PageHandle,
            _navigation: &Navigation,
        ) -> PluginResult<NavDecision> {
            self.0.lock().unwrap().push("navigate");
            Ok(NavDecision::Cancel(
                "navigation rejected by the attached app's plugin".into(),
            ))
        }
        async fn on_close(&self) -> PluginResult<()> {
            self.0.lock().unwrap().push("close");
            Ok(())
        }
    }

    #[tokio::test]
    async fn base_case_passes_plugins_to_the_existing_cdp_host_with_stealth() {
        let server = AttachServer::start(None).await;
        let log = Arc::new(Mutex::new(Vec::new()));
        let set = PluginSet::new().with(AttachPlugin(log.clone())).unwrap();
        let config = BrowserConfig::default()
            .with_cdp_url(&server.http)
            .with_fingerprint(fingerprint())
            .with_plugins(set);
        let mut case = BaseCase::new(config).await.unwrap();
        assert_identity(&server);
        assert_eq!(*log.lock().unwrap(), ["browser", "page"]);
        for session in ["ST1"] {
            for needle in ["tauriPluginStealth", "tauriPluginHook"] {
                let calls = server.calls();
                for (method, field) in [
                    ("Page.addScriptToEvaluateOnNewDocument", "source"),
                    ("Runtime.evaluate", "expression"),
                ] {
                    assert!(
                        calls.iter().any(|call| call["method"] == method
                            && call["sessionId"] == session
                            && call["params"][field]
                                .as_str()
                                .is_some_and(|source| source.contains(needle))),
                        "{needle} {method} on {session}"
                    );
                }
            }
        }
        let error = case.cdp_open("https://blocked.example/").await.unwrap_err();
        assert!(matches!(
            error,
            seleniumbase_rs::SeleniumBaseError::Plugin { .. }
        ));
        assert!(!server
            .calls()
            .iter()
            .any(|call| call["method"] == "Page.navigate"));
        case.quit().await.unwrap();
        assert_eq!(
            *log.lock().unwrap(),
            ["browser", "page", "navigate", "close"]
        );
        assert_identity(&server);
    }

    struct LaunchOnly;
    #[async_trait]
    impl Plugin for LaunchOnly {
        fn name(&self) -> &str {
            "tauri-launch-only-test"
        }
        fn requirements(&self) -> Requirements {
            Requirements::LAUNCH_ONLY
        }
    }

    #[tokio::test]
    async fn attaching_preserves_the_plugin_launch_only_guard() {
        let server = AttachServer::start(None).await;
        let options = LaunchOptions::builder()
            .cdp_url(&server.http)
            .plugins(PluginSet::new().with(LaunchOnly).unwrap())
            .build()
            .unwrap();
        let error = Browser::launch(options).await.unwrap_err();
        assert!(matches!(
            error,
            seleniumbase_rs::SeleniumBaseError::Plugin { .. }
        ));
        assert!(server.calls().is_empty());
    }
}
