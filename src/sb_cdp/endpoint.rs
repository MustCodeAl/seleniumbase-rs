//! The DevTools endpoint of a running browser, and the client configs other
//! DevTools tools need to attach to it.
//!
//! A browser this crate launched listens for the DevTools Protocol on
//! `127.0.0.1`. Other clients can share it: Google's `chrome-devtools-mcp`
//! attaches with `--browserUrl`, Microsoft's Playwright MCP with
//! `--cdp-endpoint`, and both accept the HTTP address `http://127.0.0.1:PORT`.
//! [`DevToolsEndpoint`] holds the addresses and writes those configs.

use serde_json::{json, Value};

/// The `chrome-devtools-mcp` release (npm, by Google) the generated configs run.
///
/// Pinned rather than `@latest`, so a config keeps running the code it was
/// written for.
pub const CHROME_DEVTOOLS_MCP_VERSION: &str = "1.11.0";

/// The `@playwright/mcp` release (npm, by Microsoft) the generated configs run.
pub const PLAYWRIGHT_MCP_VERSION: &str = "0.0.83";

/// Where a browser's DevTools Protocol endpoint listens.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::DevToolsEndpoint;
///
/// let endpoint = DevToolsEndpoint::new(
///     "http://127.0.0.1:9222",
///     "ws://127.0.0.1:9222/devtools/browser/0a1b",
/// );
/// assert_eq!(endpoint.version_url(), "http://127.0.0.1:9222/json/version");
/// let config = endpoint.chrome_devtools_mcp_config();
/// assert_eq!(config["mcpServers"]["chrome-devtools"]["command"], "npx");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevToolsEndpoint {
    http_url: String,
    websocket_url: String,
}

impl DevToolsEndpoint {
    /// An endpoint with the HTTP address `http_url` (such as
    /// `http://127.0.0.1:9222`) and the browser WebSocket address
    /// `websocket_url`. A trailing `/` on the HTTP address is dropped.
    #[must_use]
    pub fn new(http_url: impl Into<String>, websocket_url: impl Into<String>) -> Self {
        let mut http_url = http_url.into();
        while http_url.ends_with('/') {
            http_url.pop();
        }
        Self {
            http_url,
            websocket_url: websocket_url.into(),
        }
    }

    /// The HTTP address, such as `http://127.0.0.1:9222`: what
    /// `chrome-devtools-mcp --browserUrl` and Playwright's `--cdp-endpoint` take.
    #[must_use]
    pub fn http_url(&self) -> &str {
        &self.http_url
    }

    /// The browser-level WebSocket address, such as
    /// `ws://127.0.0.1:9222/devtools/browser/<id>`: what
    /// `chrome-devtools-mcp --wsEndpoint` and Puppeteer's `browserWSEndpoint` take.
    #[must_use]
    pub fn websocket_url(&self) -> &str {
        &self.websocket_url
    }

    /// The address of the endpoint's version document (`/json/version`), which
    /// names the browser and repeats the WebSocket address.
    #[must_use]
    pub fn version_url(&self) -> String {
        format!("{}/json/version", self.http_url)
    }

    /// The arguments that start Google's `chrome-devtools-mcp` attached to this
    /// browser: `--browserUrl`, with Google's usage statistics and its
    /// Chrome UX Report lookups (which send page URLs to Google) turned off.
    #[must_use]
    pub fn chrome_devtools_mcp_args(&self) -> Vec<String> {
        vec![
            "-y".to_owned(),
            format!("chrome-devtools-mcp@{CHROME_DEVTOOLS_MCP_VERSION}"),
            format!("--browserUrl={}", self.http_url),
            "--no-usage-statistics".to_owned(),
            "--no-performance-crux".to_owned(),
        ]
    }

    /// The arguments that start Microsoft's Playwright MCP attached to this
    /// browser over the DevTools Protocol (`--cdp-endpoint`).
    #[must_use]
    pub fn playwright_mcp_args(&self) -> Vec<String> {
        vec![
            "-y".to_owned(),
            format!("@playwright/mcp@{PLAYWRIGHT_MCP_VERSION}"),
            format!("--cdp-endpoint={}", self.http_url),
        ]
    }

    /// A client config (`mcpServers`, the form Claude Desktop, Cursor and
    /// others read) that runs `chrome-devtools-mcp` against this browser.
    #[must_use]
    pub fn chrome_devtools_mcp_config(&self) -> Value {
        json!({
            "mcpServers": {
                "chrome-devtools": {
                    "command": "npx",
                    "args": self.chrome_devtools_mcp_args(),
                }
            }
        })
    }

    /// A client config that runs Playwright MCP against this browser.
    #[must_use]
    pub fn playwright_mcp_config(&self) -> Value {
        json!({
            "mcpServers": {
                "playwright": {
                    "command": "npx",
                    "args": self.playwright_mcp_args(),
                }
            }
        })
    }

    /// The `claude mcp add` commands that register both servers with Claude
    /// Code, one per line.
    #[must_use]
    pub fn claude_code_commands(&self) -> Vec<String> {
        vec![
            format!(
                "claude mcp add chrome-devtools -- npx {}",
                self.chrome_devtools_mcp_args().join(" ")
            ),
            format!(
                "claude mcp add playwright -- npx {}",
                self.playwright_mcp_args().join(" ")
            ),
        ]
    }

    /// Everything above as one JSON object: the addresses, both client
    /// configs and the Claude Code commands.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "http_url": self.http_url,
            "websocket_url": self.websocket_url,
            "version_url": self.version_url(),
            "mcp_configs": {
                "chrome_devtools_mcp": self.chrome_devtools_mcp_config(),
                "playwright_mcp": self.playwright_mcp_config(),
            },
            "claude_code": self.claude_code_commands(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint() -> DevToolsEndpoint {
        DevToolsEndpoint::new(
            "http://127.0.0.1:9333/",
            "ws://127.0.0.1:9333/devtools/browser/abc",
        )
    }

    #[test]
    fn the_http_address_loses_a_trailing_slash() {
        assert_eq!(endpoint().http_url(), "http://127.0.0.1:9333");
        assert_eq!(
            endpoint().version_url(),
            "http://127.0.0.1:9333/json/version"
        );
        assert_eq!(
            endpoint().websocket_url(),
            "ws://127.0.0.1:9333/devtools/browser/abc"
        );
    }

    #[test]
    fn chrome_devtools_mcp_attaches_by_browser_url_with_google_telemetry_off() {
        assert_eq!(
            endpoint().chrome_devtools_mcp_config(),
            json!({
                "mcpServers": {
                    "chrome-devtools": {
                        "command": "npx",
                        "args": [
                            "-y",
                            "chrome-devtools-mcp@1.11.0",
                            "--browserUrl=http://127.0.0.1:9333",
                            "--no-usage-statistics",
                            "--no-performance-crux"
                        ]
                    }
                }
            })
        );
    }

    #[test]
    fn playwright_mcp_attaches_by_cdp_endpoint() {
        assert_eq!(
            endpoint().playwright_mcp_config(),
            json!({
                "mcpServers": {
                    "playwright": {
                        "command": "npx",
                        "args": ["-y", "@playwright/mcp@0.0.83", "--cdp-endpoint=http://127.0.0.1:9333"]
                    }
                }
            })
        );
    }

    #[test]
    fn claude_code_gets_one_command_per_server() {
        assert_eq!(
            endpoint().claude_code_commands(),
            [
                "claude mcp add chrome-devtools -- npx -y chrome-devtools-mcp@1.11.0 \
                 --browserUrl=http://127.0.0.1:9333 --no-usage-statistics --no-performance-crux",
                "claude mcp add playwright -- npx -y @playwright/mcp@0.0.83 \
                 --cdp-endpoint=http://127.0.0.1:9333",
            ]
        );
    }

    #[test]
    fn the_json_form_holds_the_addresses_and_both_configs() {
        let all = endpoint().to_json();
        assert_eq!(all["http_url"], "http://127.0.0.1:9333");
        assert_eq!(
            all["websocket_url"],
            "ws://127.0.0.1:9333/devtools/browser/abc"
        );
        assert_eq!(
            all["mcp_configs"]["chrome_devtools_mcp"],
            endpoint().chrome_devtools_mcp_config()
        );
        assert_eq!(
            all["mcp_configs"]["playwright_mcp"],
            endpoint().playwright_mcp_config()
        );
        assert_eq!(all["claude_code"].as_array().map(Vec::len), Some(2));
    }
}
