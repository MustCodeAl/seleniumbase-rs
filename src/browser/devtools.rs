//! The DevTools client of a WebDriver session: protocol commands sent through the session's
//! driver (its `/goog/cdp/execute` passthrough), named and failing as the pure CDP engine's
//! [`Page::execute`](crate::sb_cdp::Page::execute) does.

use serde_json::{Value, json};
use thirtyfour::WebDriver;
use thirtyfour::cdp::Cdp;

use crate::error::SeleniumBaseError;

/// The DevTools client of a WebDriver session.
///
/// [`BrowserSession::devtools`](crate::BrowserSession::devtools) gives one in every mode. A
/// command the browser refuses is [`SeleniumBaseError::CdpDriver`] with the command's name and
/// the browser's message, as on the pure CDP engine. The driver of a browser without DevTools
/// (Firefox, Safari) refuses every command.
pub struct CdpClient {
    cdp: Cdp,
}

impl std::fmt::Debug for CdpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdpClient").finish_non_exhaustive()
    }
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
    /// The detectable domains (such as `Runtime` and `Debugger`) are left
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
    pub async fn cookies(&self, url: &str) -> Result<Value, SeleniumBaseError> {
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

/// The commands `CdpClient::enable_default_domains` sends, in order.
///
/// None of the [`DETECTABLE_DOMAINS`] may be added: each is an automation tell.
pub(crate) const DEFAULT_DOMAIN_COMMANDS: [&str; 2] = ["Page.enable", "Network.enable"];

/// The protocol domains whose `enable` command makes the browser show a page
/// that something is driving it.
pub const DETECTABLE_DOMAINS: [&str; 5] = ["Runtime", "DOM", "Log", "Debugger", "Profiler"];

/// Whether `method` is the `enable` command of a domain in
/// [`DETECTABLE_DOMAINS`].
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::devtools::is_detectable_enable;
///
/// assert!(is_detectable_enable("Runtime.enable"));
/// assert!(!is_detectable_enable("Runtime.evaluate"));
/// assert!(!is_detectable_enable("Page.enable"));
/// ```
#[must_use]
pub fn is_detectable_enable(method: &str) -> bool {
    method
        .strip_suffix(".enable")
        .is_some_and(|domain| DETECTABLE_DOMAINS.contains(&domain))
}

/// Says so in the log when a caller sends one of the detectable `enable`
/// commands. It is still sent: a person who asks for it by name gets it.
pub(crate) fn warn_if_detectable(method: &str) {
    if is_detectable_enable(method) {
        tracing::warn!(
            method,
            "this command makes the browser show that it is driven; most pages do not need it"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_DOMAIN_COMMANDS, DETECTABLE_DOMAINS, is_detectable_enable};

    #[test]
    fn the_default_domains_never_include_runtime() {
        for command in DEFAULT_DOMAIN_COMMANDS {
            assert!(
                !command.starts_with("Runtime."),
                "{command} would make the automation detectable"
            );
        }
    }

    #[test]
    fn the_default_domains_include_none_of_the_detectable_ones() {
        for command in DEFAULT_DOMAIN_COMMANDS {
            assert!(
                !is_detectable_enable(command),
                "{command} would make the automation detectable"
            );
        }
        for domain in DETECTABLE_DOMAINS {
            assert!(
                DEFAULT_DOMAIN_COMMANDS
                    .iter()
                    .all(|command| !command.starts_with(&format!("{domain}."))),
                "{domain} is on by default"
            );
        }
    }

    #[test]
    fn exactly_the_enable_commands_of_the_five_domains_are_detectable() {
        for domain in ["Runtime", "DOM", "Log", "Debugger", "Profiler"] {
            assert!(
                is_detectable_enable(&format!("{domain}.enable")),
                "{domain}"
            );
        }
        for method in [
            "Runtime.evaluate",
            "Runtime.disable",
            "DOM.getDocument",
            "Page.enable",
            "Network.enable",
            "Fetch.enable",
            "Emulation.setDeviceMetricsOverride",
            "Page.addScriptToEvaluateOnNewDocument",
            "enable",
            ".enable",
            "Runtime.enabled",
            "NotRuntime.enable",
        ] {
            assert!(!is_detectable_enable(method), "{method}");
        }
    }

    #[test]
    fn the_default_domains_are_page_then_network() {
        assert_eq!(DEFAULT_DOMAIN_COMMANDS, ["Page.enable", "Network.enable"]);
    }
}
