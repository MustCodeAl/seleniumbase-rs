//! Chrome DevTools Protocol (CDP) helpers.
//!
//! This module wraps the `thirtyfour` CDP extensions with typed helpers for
//! common commands such as injecting scripts, clearing state, and dispatching
//! input events. It is used by stealth and automation internals.

#![allow(deprecated)]

use std::sync::Arc;
use thirtyfour::extensions::cdp::ChromeDevTools;

/// The commands [`CdpClient::enable_default_domains`] sends, in order.
///
/// `Runtime.enable` must never be added: it is a well-known automation tell
/// (see the method's documentation).
const DEFAULT_DOMAIN_COMMANDS: [&str; 2] = ["Page.enable", "Network.enable"];

/// Thin wrapper around [`ChromeDevTools`] for issuing CDP commands.
#[allow(deprecated)]
pub struct CdpClient {
    pub dev_tools: ChromeDevTools,
}

impl CdpClient {
    /// Builds a CDP client from a shared session handle.
    pub fn from_handle(handle: Arc<thirtyfour::session::handle::SessionHandle>) -> Self {
        Self {
            dev_tools: ChromeDevTools::new(handle),
        }
    }

    /// Registers `script` to run on every new document via
    /// `Page.addScriptToEvaluateOnNewDocument`.
    pub async fn add_init_script(
        &self,
        script: &str,
    ) -> Result<(), crate::error::SeleniumBaseError> {
        let params = serde_json::json!({
            "source": script,
        });
        self.dev_tools
            .execute_cdp_with_params("Page.addScriptToEvaluateOnNewDocument", params)
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        Ok(())
    }

    /// Sends a CDP command with parameters, discarding the response.
    pub async fn send_command(
        &self,
        cmd: &str,
        params: serde_json::Value,
    ) -> Result<(), crate::error::SeleniumBaseError> {
        self.dev_tools
            .execute_cdp_with_params(cmd, params)
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        Ok(())
    }

    /// Enables the Page and Network CDP domains.
    ///
    /// The Runtime domain is deliberately left off. Enabling it makes the
    /// browser report every console call and every new execution context to the
    /// client, and bot-detection scripts probe for exactly that traffic (for
    /// example by logging an object whose property getter fires only if a
    /// client is listening). Commands such as `Runtime.evaluate` work without
    /// it.
    ///
    /// # Errors
    ///
    /// Returns the first domain the browser refused to enable.
    pub async fn enable_default_domains(&self) -> Result<(), crate::error::SeleniumBaseError> {
        for domain in DEFAULT_DOMAIN_COMMANDS {
            self.dev_tools.execute_cdp(domain).await.map_err(|e| {
                crate::error::SeleniumBaseError::Unsupported(format!("{domain}: {e}"))
            })?;
        }
        Ok(())
    }

    /// Executes a CDP command and returns the JSON response.
    pub async fn execute(
        &self,
        method: &str,
    ) -> Result<serde_json::Value, crate::error::SeleniumBaseError> {
        self.dev_tools
            .execute_cdp(method)
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))
    }

    /// Executes a CDP command with JSON parameters and returns the response.
    pub async fn execute_with_params(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, crate::error::SeleniumBaseError> {
        self.dev_tools
            .execute_cdp_with_params(method, params)
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))
    }

    /// Clears the browser cache.
    pub async fn clear_cache(&self) -> Result<(), crate::error::SeleniumBaseError> {
        self.dev_tools
            .execute_cdp("Network.clearBrowserCache")
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        Ok(())
    }

    /// Clears all browser cookies.
    pub async fn clear_cookies(&self) -> Result<(), crate::error::SeleniumBaseError> {
        self.dev_tools
            .execute_cdp("Network.clearBrowserCookies")
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        Ok(())
    }

    /// Returns all cookies as a JSON value.
    pub async fn get_cookies(&self) -> Result<serde_json::Value, crate::error::SeleniumBaseError> {
        self.dev_tools
            .execute_cdp("Network.getCookies")
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))
    }

    /// Dispatches a left mouse click at the given coordinates via CDP.
    ///
    /// # Errors
    ///
    /// Returns the error of the first event the browser refused, so a click
    /// that did not happen is not reported as one that did.
    pub async fn mouse_click(&self, x: f64, y: f64) -> Result<(), crate::error::SeleniumBaseError> {
        for (kind, button) in [
            ("mouseMoved", None),
            ("mousePressed", Some("left")),
            ("mouseReleased", Some("left")),
        ] {
            let mut params = serde_json::json!({ "type": kind, "x": x, "y": y });
            if let Some(button) = button {
                params["button"] = serde_json::json!(button);
                params["clickCount"] = serde_json::json!(1);
            }
            self.dev_tools
                .execute_cdp_with_params("Input.dispatchMouseEvent", params)
                .await
                .map_err(|e| {
                    crate::error::SeleniumBaseError::Unsupported(format!("{kind}: {e}"))
                })?;
        }
        Ok(())
    }

    /// Inserts `text` as if typed via the CDP Input domain.
    pub async fn keyboard_insert_text(
        &self,
        text: &str,
    ) -> Result<(), crate::error::SeleniumBaseError> {
        let params = serde_json::json!({
            "text": text
        });
        self.dev_tools
            .execute_cdp_with_params("Input.insertText", params)
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        Ok(())
    }

    /// Applies network throttling/latency conditions via CDP.
    pub async fn set_network_conditions(
        &self,
        conditions: thirtyfour::extensions::cdp::NetworkConditions,
    ) -> Result<(), crate::error::SeleniumBaseError> {
        self.dev_tools
            .set_network_conditions(&conditions)
            .await
            .map_err(|e| crate::error::SeleniumBaseError::Unsupported(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_DOMAIN_COMMANDS;

    #[test]
    fn the_default_domains_never_include_runtime() {
        // `Runtime.enable` makes the browser emit the console and
        // execution-context events that bot-detection scripts probe for.
        for command in DEFAULT_DOMAIN_COMMANDS {
            assert!(
                !command.starts_with("Runtime."),
                "{command} would make the automation detectable"
            );
        }
    }

    #[test]
    fn the_default_domains_are_page_then_network() {
        assert_eq!(DEFAULT_DOMAIN_COMMANDS, ["Page.enable", "Network.enable"]);
    }
}
