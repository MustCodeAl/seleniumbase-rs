//! The DevTools client of a WebDriver session: protocol commands sent through the session's
//! driver (its `/goog/cdp/execute` passthrough), named and failing as the pure CDP engine's
//! [`Page::execute`](crate::sb_cdp::Page::execute) does.

use serde_json::{json, Value};
use thirtyfour::cdp::Cdp;
use thirtyfour::WebDriver;

use crate::error::SeleniumBaseError;
use crate::evasion::cdp::{warn_if_detectable, DEFAULT_DOMAIN_COMMANDS};

/// The DevTools client of a WebDriver session.
///
/// [`BrowserSession::devtools`](crate::BrowserSession::devtools) gives one in every mode. A
/// command the browser refuses is [`SeleniumBaseError::CdpDriver`] with the command's name and
/// the browser's message, as on the pure CDP engine. The driver of a browser without DevTools
/// (Firefox, Safari) refuses every command.
pub struct CdpClient {
    cdp: Cdp,
}

impl CdpClient {
    /// The client of `driver`'s session.
    pub(crate) fn new(driver: &WebDriver) -> Self {
        Self { cdp: driver.cdp() }
    }

    /// Sends `method` with `params` and returns the browser's answer, as
    /// [`Page::execute`](crate::sb_cdp::Page::execute) does. Pass `json!({})` for a command
    /// without parameters.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the browser refuses the command.
    pub async fn execute(&self, method: &str, params: Value) -> Result<Value, SeleniumBaseError> {
        warn_if_detectable(method);
        self.send(method, params).await
    }

    /// Sends a command the client itself chose, so without the warning about the detectable
    /// domains.
    async fn send(&self, method: &str, params: Value) -> Result<Value, SeleniumBaseError> {
        let answer: Value = self
            .cdp
            .send_raw(method, params)
            .await
            .map_err(|error| SeleniumBaseError::cdp_driver(format!("{method}: {error}")))?;
        Ok(answer)
    }

    /// Registers `script` to run on every new document
    /// (`Page.addScriptToEvaluateOnNewDocument`).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the browser refuses it.
    pub async fn add_init_script(&self, script: &str) -> Result<(), SeleniumBaseError> {
        self.send(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": script }),
        )
        .await?;
        Ok(())
    }

    /// Enables the Page and Network domains.
    ///
    /// The domains in [`DETECTABLE_DOMAINS`](crate::evasion::cdp::DETECTABLE_DOMAINS) are left
    /// off on purpose. The Runtime domain matters most: enabling it makes the browser report
    /// every console call and every new execution context to the client, and bot-detection
    /// scripts probe for exactly that traffic (for example by logging an object whose property
    /// getter fires only if a client is listening). Commands such as `Runtime.evaluate` work
    /// without it.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] for the first domain the browser refused to enable.
    pub async fn enable_default_domains(&self) -> Result<(), SeleniumBaseError> {
        for method in DEFAULT_DOMAIN_COMMANDS {
            self.send(method, json!({})).await?;
        }
        Ok(())
    }

    /// Clears the browser cache.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the browser refuses it.
    pub async fn clear_cache(&self) -> Result<(), SeleniumBaseError> {
        self.send("Network.clearBrowserCache", json!({})).await?;
        Ok(())
    }

    /// Clears every cookie of the browser.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the browser refuses it.
    pub async fn clear_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.send("Network.clearBrowserCookies", json!({})).await?;
        Ok(())
    }

    /// The cookies the browser would send to `url`, as the raw `Network.getCookies` result,
    /// `{"cookies": [...]}`. The URL is always sent: without `urls` the command also answers the
    /// cookies of every frame of the page, which WebDriver (and chromedriver) does not list.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the browser refuses it.
    pub async fn get_cookies(&self, url: &str) -> Result<Value, SeleniumBaseError> {
        self.send("Network.getCookies", json!({ "urls": [url] }))
            .await
    }

    /// Dispatches a left mouse click at `x`, `y` in the viewport.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] for the first event the browser refused, so a click
    /// that did not happen is not reported as one that did.
    pub async fn mouse_click(&self, x: f64, y: f64) -> Result<(), SeleniumBaseError> {
        for (kind, button) in [
            ("mouseMoved", None),
            ("mousePressed", Some("left")),
            ("mouseReleased", Some("left")),
        ] {
            let mut params = json!({ "type": kind, "x": x, "y": y });
            if let Some(button) = button {
                params["button"] = json!(button);
                params["clickCount"] = json!(1);
            }
            self.send("Input.dispatchMouseEvent", params).await?;
        }
        Ok(())
    }

    /// Inserts `text` as if typed, through the Input domain, with no key events.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the browser refuses it.
    pub async fn keyboard_insert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.send("Input.insertText", json!({ "text": text }))
            .await?;
        Ok(())
    }
}
