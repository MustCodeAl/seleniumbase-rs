//! The navigation, context and tab hooks of the plugin API on the Pure CDP
//! engine, against a real headless Chrome and a local HTTP server.
//!
//! The plugin cancels a navigation to a forbidden address, hears the status of
//! the documents Chrome loads, is told about a real browser context, and
//! blocks a URL pattern in every tab. The test needs Chrome, so it is ignored
//! by default. Run it with:
//!
//! ```text
//! cargo test --features plugins --test sb_cdp_plugin_hooks_chrome -- --ignored
//! ```

#![cfg(feature = "plugins")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, ContextHandle, ContextId, Metadata, NavDecision, NavOutcome,
    Navigation, PageHandle, Plugin, PluginSet, Result as PluginResult,
};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::SeleniumBaseError;

type Log = Arc<Mutex<Vec<String>>>;

/// Answers `/missing` with a 404 and every other path with a 200.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let address = listener.local_addr().expect("its address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap_or(0);
            let head = String::from_utf8_lossy(&request[..read]);
            let status = if head.starts_with("GET /missing") {
                "404 Not Found"
            } else {
                "200 OK"
            };
            let body = "<html><body>hello</body></html>";
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    format!("http://{address}")
}

struct Witness {
    log: Log,
}

impl Witness {
    fn note(&self, entry: impl Into<String>) {
        self.log.lock().unwrap().push(entry.into());
    }
}

#[async_trait]
impl Plugin for Witness {
    fn name(&self) -> &str {
        "witness"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core)
            .capabilities([Capability::Contexts, Capability::Cdp("Network".into())])
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> PluginResult<()> {
        page.block_urls(&["*blocked*".to_owned()]).await
    }

    async fn before_navigate(
        &self,
        _page: &dyn PageHandle,
        navigation: &Navigation,
    ) -> PluginResult<NavDecision> {
        Ok(if navigation.url.starts_with("https://prod.example/") {
            NavDecision::Cancel("tests must not touch production".into())
        } else {
            NavDecision::Continue
        })
    }

    async fn after_navigate(
        &self,
        _page: &dyn PageHandle,
        outcome: &NavOutcome,
    ) -> PluginResult<()> {
        self.note(format!("navigated status={:?}", outcome.status));
        Ok(())
    }

    async fn on_context_created(&self, context: &dyn ContextHandle) -> PluginResult<()> {
        self.note(format!(
            "context created, id known: {}",
            !context.id().as_str().is_empty()
        ));
        Ok(())
    }

    async fn on_context_closed(&self, _id: &ContextId) -> PluginResult<()> {
        self.note("context closed");
        Ok(())
    }
}

#[tokio::test]
#[ignore = "needs a real Chrome"]
async fn the_hooks_work_against_a_real_chrome() -> Result<(), SeleniumBaseError> {
    let site = serve();
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(Witness {
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

    page.goto(format!("{site}/ok")).await?;
    page.goto(format!("{site}/missing")).await?;
    let cancelled = page.goto("https://prod.example/pay").await;
    assert!(
        matches!(&cancelled, Err(SeleniumBaseError::Plugin { plugin, .. }) if plugin == "witness"),
        "{cancelled:?}"
    );
    assert_eq!(
        page.url().await?,
        format!("{site}/missing"),
        "the tab did not move"
    );

    let context = browser.new_context().await?;
    let inside = context.new_page_at(format!("{site}/ok")).await?;
    let blocked: String = inside
        .evaluate_as(format!(
            "fetch('{site}/blocked').then(r => String(r.status)).catch(() => 'blocked')"
        ))
        .await?;
    assert_eq!(
        blocked, "blocked",
        "the plugin's pattern blocks the request in a new tab"
    );
    let allowed: String = inside
        .evaluate_as(format!("fetch('{site}/ok').then(r => String(r.status))"))
        .await?;
    assert_eq!(allowed, "200");
    context.dispose().await?;
    browser.close().await?;

    assert_eq!(
        log.lock().unwrap().clone(),
        [
            "navigated status=Some(200)",
            "navigated status=Some(404)",
            "context created, id known: true",
            "navigated status=Some(200)",
            "context closed",
        ]
    );
    Ok(())
}
