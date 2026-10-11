//! A small driver for a Chrome that is already running with remote debugging.
//!
//! [`CdpDriver`] attaches to a browser by the host and port of its DevTools
//! endpoint, picks a tab, and drives that tab with a handful of calls:
//! navigate, find, click, type, evaluate and screenshot.
//!
//! The browser's own WebSocket (the one `/json/version` advertises) only
//! understands browser-level commands such as `Target.getTargets`. Page,
//! DOM, Runtime and Input commands are answered only on a *session* attached
//! to a tab. A driver that sends them on the bare browser socket gets "method
//! not found" for every call, so `CdpDriver` is a thin layer over the
//! [`sb_cdp`](crate::sb_cdp) engine, which attaches to the tab and addresses
//! each command to its session.
//!
//! New code should use [`sb_cdp::Browser`](crate::sb_cdp::Browser) and
//! [`sb_cdp::Page`](crate::sb_cdp::Page) directly: they cover much more
//! (locators that wait, trusted input, cookies, interception, ...). This type
//! is kept for code written against the small `CdpDriver` surface.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::api::cdp_driver::CdpDriver;
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! // Chrome started with `--remote-debugging-port=9222`.
//! let mut driver = CdpDriver::new();
//! driver.start_async("127.0.0.1", 9222).await?;
//! driver.get("https://seleniumbase.io/simple/login").await?;
//! driver.type_text("#username", "demo_user").await?;
//! driver.click("button").await?;
//! let title = driver.evaluate("document.title").await?;
//! println!("{}", title["value"]);
//! driver.stop().await?;
//! # Ok(())
//! # }
//! ```

use std::path::Path;

use serde_json::{json, Value};

use crate::api::cdp_page::CdpNode;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Browser, Page};

/// A minimal CDP driver for a browser that is already running.
///
/// See the [module documentation](self) for what it does and when to prefer
/// [`sb_cdp`](crate::sb_cdp) instead.
#[derive(Debug, Default)]
pub struct CdpDriver {
    /// The browser the driver is attached to.
    browser: Option<Browser>,
    /// The tab every command is sent to.
    page: Option<Page>,
}

impl CdpDriver {
    /// Creates a disconnected [`CdpDriver`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches to the browser whose DevTools endpoint is at `host:port`.
    ///
    /// The driver then works on the browser's first tab, opening a blank one
    /// if there is none. Starting an already started driver attaches it
    /// afresh. The browser keeps running after [`stop`](Self::stop).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the endpoint cannot be
    /// reached, does not describe a DevTools browser, or no tab can be
    /// attached to.
    pub async fn start_async(&mut self, host: &str, port: u16) -> Result<(), SeleniumBaseError> {
        let browser = Browser::connect(host, port).await?;
        self.attach(browser).await
    }

    /// Drives a browser you already hold, for example one from
    /// [`Browser::launch`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if no tab can be attached to.
    pub async fn attach(&mut self, browser: Browser) -> Result<(), SeleniumBaseError> {
        let page = browser.default_page().await?;
        self.page = Some(page);
        self.browser = Some(browser);
        Ok(())
    }

    /// Detaches from the browser.
    ///
    /// A browser that [`start_async`](Self::start_async) attached to keeps
    /// running. One handed over by [`attach`](Self::attach) is closed only if
    /// that handle launched it.
    ///
    /// # Errors
    ///
    /// Never fails in practice; the `Result` leaves room for cleanup errors.
    pub async fn stop(&mut self) -> Result<(), SeleniumBaseError> {
        self.page = None;
        if let Some(browser) = self.browser.take() {
            browser.close().await?;
        }
        Ok(())
    }

    /// Whether the driver is attached to a browser.
    pub fn is_started(&self) -> bool {
        self.page.is_some()
    }

    /// Builds a CDP command payload. Exposed for tests.
    pub fn build_command(&self, id: u64, method: &str, params: Value) -> Value {
        json!({"id": id, "method": method, "params": params})
    }

    /// The tab commands are sent to.
    fn page(&self) -> Result<&Page, SeleniumBaseError> {
        self.page
            .as_ref()
            .ok_or(SeleniumBaseError::SessionNotStarted)
    }

    /// Navigates to `url` and waits for the page to load.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] before
    /// [`start_async`](Self::start_async), or an error from the browser.
    pub async fn get(&self, url: &str) -> Result<(), SeleniumBaseError> {
        self.page()?.goto(url).await
    }

    /// Finds the first element matching the CSS `selector`.
    ///
    /// The node ids in the result belong to the document as it is now:
    /// navigating invalidates them.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches, and
    /// [`SeleniumBaseError::CdpDriver`] if the browser rejects the selector.
    pub async fn find(&self, selector: &str) -> Result<CdpNode, SeleniumBaseError> {
        let page = self.page()?;
        // `DOM.querySelector` needs the id of the node to search from. Ids are
        // handed out per document, so the root is asked for each time.
        let document = page.execute("DOM.getDocument", json!({"depth": 0})).await?;
        let root = document["root"]["nodeId"]
            .as_i64()
            .ok_or_else(|| SeleniumBaseError::cdp_driver("DOM.getDocument returned no root"))?;
        let found = page
            .execute(
                "DOM.querySelector",
                json!({"nodeId": root, "selector": selector}),
            )
            .await?;
        // CDP answers a selector that matches nothing with node id 0.
        CdpNode::from_query_result(&found, selector)
            .filter(|node| node.node_id != 0)
            .ok_or_else(|| SeleniumBaseError::element_not_found(selector))
    }

    /// Clicks the centre of the first element matching `selector`.
    ///
    /// Waits for the element to be visible, scrolls it into view and clicks
    /// with real mouse events.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] or
    /// [`SeleniumBaseError::ElementNotInteractable`] if the element does not
    /// become ready in time.
    pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        self.page()?.locator(selector).click().await
    }

    /// Focuses the element, clears it, and inserts `text`.
    ///
    /// # Errors
    ///
    /// See [`click`](Self::click).
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.page()?.locator(selector).fill(text).await
    }

    /// Evaluates `expression` and returns the raw CDP result object, such as
    /// `{"type": "number", "value": 2}`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the script throws.
    pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        let response = self
            .page()?
            .execute(
                "Runtime.evaluate",
                json!({"expression": expression, "returnByValue": true}),
            )
            .await?;
        if let Some(exception) = response.get("exceptionDetails") {
            return Err(SeleniumBaseError::cdp_driver(format!(
                "JS exception: {exception}"
            )));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Captures a PNG screenshot and writes it to `path`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser cannot capture,
    /// or [`SeleniumBaseError::Io`] if the file cannot be written.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        let bytes = self.page()?.screenshot().await?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    /// A driver attached to a mocked browser.
    async fn attached() -> (CdpDriver, crate::sb_cdp::MockCtrl) {
        let (browser, mock) = Browser::new_mocked();
        let mut driver = CdpDriver::new();
        driver.attach(browser).await.expect("attaches");
        (driver, mock)
    }

    #[test]
    fn build_command_payload() {
        let driver = CdpDriver::new();
        let payload = driver.build_command(7, "Runtime.evaluate", json!({"expression": "1+1"}));
        assert_eq!(payload["id"], 7);
        assert_eq!(payload["method"], "Runtime.evaluate");
        assert_eq!(payload["params"]["expression"], "1+1");
    }

    #[tokio::test]
    async fn a_driver_that_was_not_started_says_so() {
        let driver = CdpDriver::new();
        assert!(!driver.is_started());
        assert!(matches!(
            driver.get("http://example.test/").await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            driver.evaluate("1").await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        assert!(matches!(
            driver.find("a").await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
    }

    #[tokio::test]
    async fn page_commands_go_to_the_tab_session_not_the_browser_socket() {
        let (driver, mock) = attached().await;
        assert!(driver.is_started());

        driver.get("http://example.test/").await.unwrap();
        driver.evaluate("1 + 1").await.unwrap();

        // The mocked tab is attached as session "S1". A command sent without
        // a session reaches only the browser, which has no Page or Runtime
        // domain.
        for method in ["Page.navigate", "Runtime.evaluate"] {
            let calls = mock.calls_to(method);
            assert!(!calls.is_empty(), "{method} was sent");
            assert!(
                calls
                    .iter()
                    .all(|call| call.session_id.as_deref() == Some("S1")),
                "{method} must be addressed to the tab session: {calls:?}"
            );
        }
    }

    #[tokio::test]
    async fn find_searches_from_the_document_root_it_was_given() {
        let (driver, mock) = attached().await;
        mock.reply("DOM.getDocument", json!({"root": {"nodeId": 42}}));
        mock.on("DOM.querySelector", |params| {
            // Node ids are handed out per document; only the real root works.
            if params["nodeId"] == 42 && params["selector"] == "#go" {
                Ok(json!({"nodeId": 7}))
            } else {
                Err("Could not find node with given id".to_owned())
            }
        });

        let node = driver.find("#go").await.unwrap();
        assert_eq!(node.node_id, 7);
        assert_eq!(node.selector.as_deref(), Some("#go"));
        let queries = mock.calls_to("DOM.querySelector");
        assert_eq!(queries[0].session_id.as_deref(), Some("S1"));
    }

    #[tokio::test]
    async fn find_reports_a_selector_that_matches_nothing_as_not_found() {
        let (driver, mock) = attached().await;
        mock.reply("DOM.getDocument", json!({"root": {"nodeId": 1}}));
        // CDP answers "no match" with node id 0, not with an error.
        mock.reply("DOM.querySelector", json!({"nodeId": 0}));

        let error = driver.find("#missing").await.unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::ElementNotFound { selector, .. } if selector == "#missing"),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn find_passes_on_a_browser_error_for_a_bad_selector() {
        let (driver, mock) = attached().await;
        mock.reply("DOM.getDocument", json!({"root": {"nodeId": 1}}));
        mock.fail(
            "DOM.querySelector",
            "DOM.querySelector: not a valid selector",
        );

        let error = driver.find("[").await.unwrap_err().to_string();
        assert!(error.contains("not a valid selector"), "{error}");
    }

    #[tokio::test]
    async fn evaluate_returns_the_result_object_and_turns_an_exception_into_an_error() {
        let (driver, mock) = attached().await;
        mock.reply(
            "Runtime.evaluate",
            json!({"result": {"type": "number", "value": 2}}),
        );
        let result = driver.evaluate("1 + 1").await.unwrap();
        assert_eq!(result["value"], 2);
        assert_eq!(result["type"], "number");

        mock.reply(
            "Runtime.evaluate",
            json!({
                "result": {"type": "object"},
                "exceptionDetails": {"text": "Uncaught", "exception": {"description": "boom"}}
            }),
        );
        let error = driver
            .evaluate("throw new Error('boom')")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("boom"), "{error}");
    }

    #[tokio::test]
    async fn screenshot_writes_the_decoded_png() {
        let (driver, mock) = attached().await;
        let png = b"\x89PNG not really";
        mock.reply(
            "Page.captureScreenshot",
            json!({"data": base64::prelude::BASE64_STANDARD.encode(png)}),
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.png");
        driver.screenshot(&path).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), png);
        assert_eq!(
            mock.calls_to("Page.captureScreenshot")[0]
                .session_id
                .as_deref(),
            Some("S1")
        );
    }

    #[tokio::test]
    async fn stop_detaches_and_later_calls_fail() {
        let (mut driver, _mock) = attached().await;
        driver.stop().await.unwrap();
        assert!(!driver.is_started());
        assert!(matches!(
            driver.get("http://example.test/").await,
            Err(SeleniumBaseError::SessionNotStarted)
        ));
        // Stopping twice is harmless.
        driver.stop().await.unwrap();
    }

    /// Connects to a real Chrome by its debugging port and drives a page.
    ///
    /// Run with `cargo test --lib cdp_driver -- --ignored`.
    #[tokio::test]
    #[ignore = "launches a real Chrome"]
    async fn drives_a_real_chrome_it_connected_to_by_port() {
        use crate::sb_cdp::LaunchOptions;

        let browser = Browser::launch(
            LaunchOptions::builder()
                .headless(true)
                .no_sandbox(std::env::var_os("CI").is_some())
                .build()
                .unwrap(),
        )
        .await
        .expect("Chrome starts");
        let port = browser.debugging_port();

        let mut driver = CdpDriver::new();
        driver
            .start_async("127.0.0.1", port)
            .await
            .expect("attaches to the running Chrome by port");
        // `%2B` is a plus sign; a bare one would be read as part of the URL.
        driver
            .get(
                "data:text/html,<title>Driver</title><input id=txt>\
                 <button id=go onclick=\"document.title='clicked:'%2Bdocument.getElementById('txt').value\">Go</button>",
            )
            .await
            .unwrap();

        assert_eq!(driver.evaluate("1 + 1").await.unwrap()["value"], 2);
        assert_eq!(
            driver.evaluate("document.title").await.unwrap()["value"],
            "Driver"
        );
        assert!(driver.find("#go").await.unwrap().node_id > 0);
        assert!(matches!(
            driver.find("#nothing-here").await,
            Err(SeleniumBaseError::ElementNotFound { .. })
        ));

        driver.type_text("#txt", "ada").await.unwrap();
        driver.click("#go").await.unwrap();
        assert_eq!(
            driver.evaluate("document.title").await.unwrap()["value"],
            "clicked:ada"
        );

        let dir = tempfile::tempdir().unwrap();
        let shot = dir.path().join("shot.png");
        driver.screenshot(&shot).await.unwrap();
        assert!(std::fs::read(&shot).unwrap().starts_with(b"\x89PNG"));

        driver.stop().await.unwrap();
        // Detaching leaves the browser the driver did not start running.
        assert!(browser.is_connected());
        browser.close().await.unwrap();
    }
}
