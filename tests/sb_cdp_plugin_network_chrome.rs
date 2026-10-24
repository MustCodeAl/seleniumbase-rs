//! The network router of the plugin hooks on the Pure CDP engine, against a
//! real headless Chrome and a local HTTP server.
//!
//! One plugin blocks a URL pattern, adds a header to every request to the
//! server, blocks another URL from its `on_request` hook and watches the
//! responses of one path. The test checks what reached the server (so the
//! blocked requests did not, and the header did), what the page got back (so
//! the responses the plugin watched arrive intact), what the plugin saw, and
//! that an interception of the program works beside the plugin on the same
//! tab. It needs Chrome, so it is ignored by default. Run it with:
//!
//! ```text
//! cargo test --features plugins --test sb_cdp_plugin_network_chrome -- --ignored
//! ```

#![cfg(feature = "plugins")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, Metadata, NetRule, NetworkInterest, Plugin, PluginSet,
    RequestDecision, RequestFilter, RequestView, ResponseView, Result as PluginResult,
};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Response, Rule};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::json;

type Log = Arc<Mutex<Vec<String>>>;

/// Answers every path with a small body. `/echo` says which value of the
/// header `X-Sbrs` it received. Every path asked for is written down.
fn serve() -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let address = listener.local_addr().expect("its address");
    let asked = Log::default();
    let record = Arc::clone(&asked);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let record = Arc::clone(&record);
            std::thread::spawn(move || {
                let mut request = [0_u8; 4096];
                let read = stream.read(&mut request).unwrap_or(0);
                let head = String::from_utf8_lossy(&request[..read]).into_owned();
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let header = head
                    .lines()
                    .map(str::to_ascii_lowercase)
                    .find_map(|line| line.strip_prefix("x-sbrs:").map(|v| v.trim().to_owned()))
                    .unwrap_or_default();
                record.lock().unwrap().push(path.clone());
                let (kind, body) = match path.as_str() {
                    "/echo" => ("application/json", format!("{{\"x_sbrs\":\"{header}\"}}")),
                    "/data.json" => ("application/json", "{\"hello\":\"world\"}".to_owned()),
                    _ => ("text/html", "<html><body>hello</body></html>".to_owned()),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    (format!("http://{address}"), asked)
}

/// Blocks, adds a header, blocks from its hook, and watches `/data.json`.
struct Probe {
    log: Log,
}

#[async_trait]
impl Plugin for Probe {
    fn name(&self) -> &str {
        "probe"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capabilities([
            Capability::NetIntercept,
            Capability::NetObserve,
            Capability::NetBodies,
        ])
    }

    fn network(&self) -> NetworkInterest {
        NetworkInterest::none()
            .rule(NetRule::block(RequestFilter::any().url("*/blocked*")))
            .rule(NetRule::set_headers(
                RequestFilter::any().url("http://127.0.0.1*"),
                [("X-Sbrs", "yes")],
            ))
            .intercept(RequestFilter::any().url("*/hook*"))
            .observe(RequestFilter::any().url("*/data.json").max_body(8))
    }

    async fn on_request(&self, request: &RequestView) -> PluginResult<RequestDecision> {
        self.log
            .lock()
            .unwrap()
            .push(format!("request {}", request.url));
        Ok(RequestDecision::Block("the hook said no".into()))
    }

    async fn on_response(&self, response: &ResponseView<'_>) -> PluginResult<()> {
        let body = String::from_utf8_lossy(response.body.unwrap_or_default()).into_owned();
        self.log.lock().unwrap().push(format!(
            "response {} {} {body} truncated={} {:?}",
            response.status, response.mime, response.truncated, response.outcome
        ));
        Ok(())
    }
}

/// What the page's `fetch` of `url` gives: the body, or `failed`.
async fn fetched(
    page: &seleniumbase_rs::sb_cdp::Page,
    url: &str,
) -> Result<String, SeleniumBaseError> {
    page.evaluate_as(format!(
        "fetch('{url}').then(r => r.text()).catch(() => 'failed')"
    ))
    .await
}

fn count(asked: &Log, path: &str) -> usize {
    asked.lock().unwrap().iter().filter(|p| *p == path).count()
}

#[tokio::test]
#[ignore = "needs a real Chrome"]
async fn the_router_works_against_a_real_chrome() -> Result<(), SeleniumBaseError> {
    let (site, asked) = serve();
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(Probe {
            log: Arc::clone(&log),
        })
        .expect("the plugin joins the set");
    let browser = Browser::launch(
        LaunchOptions::builder()
            .headless(true)
            .plugins(plugins)
            .build()?,
    )
    .await?;
    let page = browser.default_page().await?;
    page.goto(format!("{site}/page")).await?;
    assert_eq!(
        page.url().await?,
        format!("{site}/page"),
        "the page loaded through the router"
    );

    // The header of the rule reaches the server.
    assert_eq!(
        fetched(&page, &format!("{site}/echo")).await?,
        "{\"x_sbrs\":\"yes\"}"
    );

    // A blocking rule and the hook fail the request before the server hears of it.
    assert_eq!(fetched(&page, &format!("{site}/blocked")).await?, "failed");
    assert_eq!(fetched(&page, &format!("{site}/hook")).await?, "failed");
    assert_eq!(count(&asked, "/blocked"), 0);
    assert_eq!(count(&asked, "/hook"), 0);

    // The watched response arrives intact, and the plugin saw its start.
    assert_eq!(
        fetched(&page, &format!("{site}/data.json")).await?,
        "{\"hello\":\"world\"}"
    );

    // An interception of the program on the same tab answers first, and the
    // plugin's header is still added to what it lets through.
    let interception = page
        .intercept(vec![Rule::fulfill(Response::json(
            200,
            &json!({"stub": true}),
        ))
        .url("*/stubbed")])
        .await?;
    assert_eq!(
        fetched(&page, &format!("{site}/stubbed")).await?,
        "{\"stub\":true}"
    );
    assert_eq!(count(&asked, "/stubbed"), 0, "the server never saw it");
    assert_eq!(
        fetched(&page, &format!("{site}/echo")).await?,
        "{\"x_sbrs\":\"yes\"}"
    );
    interception.stop().await?;
    assert_eq!(
        fetched(&page, &format!("{site}/echo")).await?,
        "{\"x_sbrs\":\"yes\"}",
        "the plugin's rules outlive the interception"
    );
    browser.close().await?;

    let seen = log.lock().unwrap().clone();
    assert!(
        seen.contains(&format!("request {site}/hook")),
        "the hook was asked: {seen:?}"
    );
    assert!(
        seen.contains(&"response 200 application/json {\"hello\" truncated=true Ok".to_owned()),
        "the observer got the type and the first eight bytes: {seen:?}"
    );
    Ok(())
}
