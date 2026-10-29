//! The DevTools endpoint of a running browser, and the client configs other
//! DevTools tools need to attach to it.
//!
//! A browser this crate launched listens for the DevTools Protocol on
//! `127.0.0.1`. Other clients can share it: Google's `chrome-devtools-mcp`
//! attaches with `--browserUrl`, Microsoft's Playwright MCP with
//! `--cdp-endpoint`, and both accept the HTTP address `http://127.0.0.1:PORT`.
//! [`DevToolsEndpoint`] holds the addresses and writes those configs.

use crate::error::SeleniumBaseError;
use serde_json::{json, Value};

/// Resolves the existing attach path, including Tauri's authenticated discovery.
pub(super) async fn resolve(
    address: &str,
    timeout: std::time::Duration,
) -> Result<(DevToolsEndpoint, Value), SeleniumBaseError> {
    crate::install_tls_provider();
    let mut url = parse_address(address)?;
    if matches!(url.scheme(), "ws" | "wss") {
        let http_scheme = if url.scheme() == "wss" {
            "https"
        } else {
            "http"
        };
        let mut http = url.clone();
        http.set_scheme(http_scheme)
            .map_err(|()| SeleniumBaseError::cdp_driver("invalid debugger scheme"))?;
        http.set_path("");
        http.set_query(None);
        return Ok((
            DevToolsEndpoint::new(http.as_str(), url.as_str()),
            json!({}),
        ));
    }
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(|_error| {
            SeleniumBaseError::cdp_driver("cannot build debugger discovery client")
        })?;
    // A non-root path identifies the plugin's token URL. Never send that token
    // in a version probe, a redirect, a proxy request, or an error message.
    let discovered = !matches!(url.path(), "" | "/" | "/json/version");
    if discovered {
        url = discover_tauri(&client, &url).await?;
    }
    let mut version_url = url.clone();
    version_url.set_path("/json/version");
    let version = get_json(&client, &version_url, "DevTools version probe").await?;
    let ws = version["webSocketDebuggerUrl"]
        .as_str()
        .ok_or_else(|| SeleniumBaseError::cdp_driver("no webSocketDebuggerUrl advertised"))?;
    let websocket = parse_address(ws)?;
    if !matches!(websocket.scheme(), "ws" | "wss") {
        return Err(SeleniumBaseError::cdp_driver(
            "debugger did not advertise a WebSocket",
        ));
    }
    // The plugin vouched for one loopback endpoint. What that endpoint
    // advertises must stay on it: the same port, on this machine, in the clear.
    if discovered
        && (websocket.scheme() != "ws"
            || !is_loopback(&websocket)
            || websocket.port_or_known_default() != url.port_or_known_default())
    {
        return Err(SeleniumBaseError::cdp_driver(
            "Tauri advertised a debugger WebSocket outside its loopback CDP endpoint",
        ));
    }
    url.set_path("");
    Ok((
        DevToolsEndpoint::new(url.as_str(), websocket.as_str()),
        version,
    ))
}

/// Validates an address without putting a discovery token into diagnostics.
fn parse_address(address: &str) -> Result<url::Url, SeleniumBaseError> {
    let url = url::Url::parse(address)
        .map_err(|_error| SeleniumBaseError::invalid_config("invalid CDP endpoint URL"))?;
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() == Some(0)
    {
        return Err(SeleniumBaseError::invalid_config(
            "CDP requires an HTTP or WebSocket endpoint without credentials or a fragment",
        ));
    }
    Ok(url)
}

/// Fetches local debugging metadata. Query strings (and their tokens) stay private.
async fn get_json(
    client: &reqwest::Client,
    url: &url::Url,
    step: &str,
) -> Result<Value, SeleniumBaseError> {
    let location = format!(
        "{}:{}",
        url.host_str().unwrap_or_default(),
        url.port_or_known_default().unwrap_or_default()
    );
    // A `reqwest` error prints the full URL, token included, so none is
    // passed on: the step, the address and the cause are named instead.
    let failed =
        |cause: &str| SeleniumBaseError::cdp_driver(format!("{step} at {location} {cause}"));
    let mut response = client.get(url.clone()).send().await.map_err(|error| {
        failed(if error.is_timeout() {
            "timed out"
        } else if error.is_connect() {
            "could not connect"
        } else {
            "failed"
        })
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(failed(&format!("answered HTTP {status}")));
    }
    let too_large = || {
        failed(&format!(
            "answered with a body that is too large (over {MAX_REPLY_BYTES} bytes)"
        ))
    };
    if response
        .content_length()
        .is_some_and(|length| length > MAX_REPLY_BYTES)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_error| failed("broke off while answering"))?
    {
        if (body.len() + chunk.len()) as u64 > MAX_REPLY_BYTES {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_error| failed("answered with invalid JSON"))
}

/// The plugin only advertises its loopback CDP endpoint; follow no arbitrary URL.
async fn discover_tauri(
    client: &reqwest::Client,
    base: &url::Url,
) -> Result<url::Url, SeleniumBaseError> {
    if !is_loopback(base) || base.scheme() != "http" || base.query().is_some() {
        return Err(SeleniumBaseError::invalid_config(
            "Tauri CDP discovery requires a loopback HTTP plugin URL",
        ));
    }
    let mut discovery = base.clone();
    discovery.set_path(&format!("{}/cdp", base.path().trim_end_matches('/')));
    let reply = get_json(client, &discovery, "Tauri discovery").await?;
    let value = &reply["value"];
    if value["cdpEnabled"] != true {
        return Err(SeleniumBaseError::cdp_driver(
            "Tauri CDP is disabled; configure Chromium debugging in the app first",
        ));
    }
    let endpoint = value["cdpUrl"]
        .as_str()
        .ok_or_else(|| SeleniumBaseError::cdp_driver("Tauri did not advertise cdpUrl"))?;
    let endpoint = parse_address(endpoint)?;
    if !is_loopback(&endpoint)
        || endpoint.scheme() != "http"
        || !matches!(endpoint.path(), "" | "/")
        || endpoint.query().is_some()
        || value["cdpPort"].as_u64() != endpoint.port().map(u64::from)
    {
        return Err(SeleniumBaseError::cdp_driver(
            "Tauri advertised an invalid loopback CDP endpoint",
        ));
    }
    Ok(endpoint)
}

/// Whether `url` names this machine, in the spellings the plugin uses.
fn is_loopback(url: &url::Url) -> bool {
    matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
}

/// The most a discovery or version reply may weigh. Both are a few hundred
/// bytes; the limit keeps a hostile or broken endpoint from filling memory.
const MAX_REPLY_BYTES: u64 = 1 << 20;

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
