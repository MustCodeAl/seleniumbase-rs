//! External Chromium attachment and Tauri discovery over real mock HTTP/WS sockets.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use seleniumbase_rs::api::cdp_driver::CdpDriver;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{BaseCase, BrowserConfig, Fingerprint};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

struct AttachServer {
    http: String,
    websocket: String,
    calls: Arc<Mutex<Vec<Value>>>,
    requests: Arc<Mutex<Vec<String>>>,
    tasks: Vec<JoinHandle<()>>,
}

impl AttachServer {
    async fn start(discovery: Option<Value>) -> Self {
        let ws_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let websocket = format!(
            "ws://{}/devtools/browser/tauri",
            ws_listener.local_addr().unwrap()
        );
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = calls.clone();
        let ws_task = tokio::spawn(async move {
            while let Ok((stream, _)) = ws_listener.accept().await {
                let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(Message::Text(text))) = socket.next().await {
                    let call: Value = serde_json::from_str(&text).unwrap();
                    let result = match call["method"].as_str().unwrap() {
                        "Target.getTargets" => json!({ "targetInfos": [
                            { "targetId": "T1", "type": "page", "url": "https://tauri.localhost/", "title": "Tauri" },
                            { "targetId": "T2", "type": "page", "url": "https://tauri.localhost/settings", "title": "Settings" }
                        ] }),
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
        });
        let http_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http = format!("http://{}", http_listener.local_addr().unwrap());
        let ws_url = websocket.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let paths = requests.clone();
        let http_task = tokio::spawn(async move {
            while let Ok((mut stream, _)) = http_listener.accept().await {
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    bytes.push(stream.read_u8().await.unwrap());
                }
                let request = String::from_utf8(bytes).unwrap();
                let path = request.split_whitespace().nth(1).unwrap();
                paths.lock().unwrap().push(path.to_owned());
                let body = if path == "/app-token/cdp" {
                    discovery.clone().unwrap_or(json!({ "value": { "cdpEnabled": false, "cdpPort": null, "cdpUrl": null } }))
                } else if path == "/json/version" {
                    json!({ "webSocketDebuggerUrl": ws_url, "User-Agent": "Chrome/155.0.0.0" })
                } else { json!({}) }.to_string();
                let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        Self {
            http,
            websocket,
            calls,
            requests,
            tasks: vec![ws_task, http_task],
        }
    }

    fn calls(&self) -> Vec<Value> {
        self.calls.lock().unwrap().clone()
    }
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

fn assert_identity(server: &AttachServer) {
    let calls = server.calls();
    let source = seleniumbase_rs::stealth::evasions::session_script(&fingerprint());
    for session in ["ST1", "ST2"] {
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
    assert_identity(&server);
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
async fn websocket_attachment_also_prepares_existing_windows() {
    let server = AttachServer::start(None).await;
    let options = LaunchOptions::builder()
        .cdp_url(&server.websocket)
        .fingerprint(&fingerprint())
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.unwrap();
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
        assert_eq!(*log.lock().unwrap(), ["browser", "page", "page"]);
        for session in ["ST1", "ST2"] {
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
            ["browser", "page", "page", "navigate", "close"]
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
