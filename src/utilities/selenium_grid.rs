//! Selenium Grid addresses and status.
//!
//! A Selenium Grid is a hub that hands browser sessions to nodes. Tests reach
//! it through its WebDriver URL (`BrowserConfig::webdriver_url`). This module
//! covers the client side of that:
//!
//! - [`GridUrl`] parses and builds Grid addresses (the Python `--server`,
//!   `--port` and `--protocol` options become one value);
//! - [`GridStatus`] reads the Grid's `/status` reply, and
//!   [`wait_until_ready`] polls it, so a test run can wait for a Grid that is
//!   still starting.
//!
//! Starting and stopping a Grid hub or node is in
//! [`grid_server`](super::grid_server).
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::utilities::selenium_grid::GridUrl;
//! use seleniumbase_rs::BrowserConfig;
//!
//! let grid: GridUrl = "grid.example.com".parse()?;
//! assert_eq!(grid.to_string(), "http://grid.example.com:4444");
//!
//! let config = BrowserConfig::default().with_webdriver_url(grid.to_string());
//! assert_eq!(config.webdriver_url, "http://grid.example.com:4444");
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::fmt;
use std::str::FromStr;
use std::time::{Duration, Instant};

use serde_json::Value;
use url::{Host, Url};

use crate::error::SeleniumBaseError;

/// The port a Selenium Grid hub listens on unless told otherwise.
pub const DEFAULT_HUB_PORT: u16 = 4444;

/// The URL scheme of a Grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scheme {
    /// Plain HTTP.
    Http,
    /// HTTP over TLS.
    Https,
}

impl Scheme {
    /// The scheme as it is written in a URL.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    const fn default_port(self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https => 443,
        }
    }
}

/// The address of a Selenium Grid.
///
/// Parsed from `host`, `host:port` or a full `http(s)://host[:port][/path]`
/// URL. Without a scheme the Grid is assumed to be plain HTTP on port 4444,
/// the port a hub listens on; with a scheme and no port, the scheme's own
/// port is used. A URL may not carry a user name, password, query or fragment.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utilities::selenium_grid::GridUrl;
///
/// let grid: GridUrl = "https://grid.example.com/wd/hub".parse()?;
/// assert_eq!(grid.port(), 443);
/// assert_eq!(grid.path(), "/wd/hub");
/// assert_eq!(grid.status_url(), "https://grid.example.com/status");
///
/// let local = GridUrl::new("localhost", 4444);
/// assert_eq!(local.to_string(), "http://localhost:4444");
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridUrl {
    scheme: Scheme,
    host: String,
    port: u16,
    path: String,
}

impl GridUrl {
    /// A plain-HTTP Grid at `host` and `port`, with no path.
    ///
    /// `host` is used as given; use [`FromStr`] to have it checked.
    #[must_use]
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            scheme: Scheme::Http,
            host: host.into(),
            port,
            path: "/".to_owned(),
        }
    }

    /// The same Grid over HTTPS (or back to HTTP).
    #[must_use]
    pub fn with_scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = scheme;
        self
    }

    /// The same Grid with a different path, such as `/wd/hub` for a Grid 3
    /// server. A missing leading slash is added.
    #[must_use]
    pub fn with_path(mut self, path: &str) -> Self {
        self.path = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        };
        self
    }

    /// The scheme.
    #[must_use]
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// The host name or address, without brackets for IPv6.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The path; `/` when the address has none.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The scheme, host and port, with no path: `http://host:4444`.
    #[must_use]
    pub fn origin(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == self.scheme.default_port() {
            format!("{}://{host}", self.scheme.as_str())
        } else {
            format!("{}://{host}:{}", self.scheme.as_str(), self.port)
        }
    }

    /// Where a Grid 4 hub or node reports whether it is ready.
    #[must_use]
    pub fn status_url(&self) -> String {
        format!("{}/status", self.origin())
    }

    /// The Grid's web console (Grid 4).
    #[must_use]
    pub fn ui_url(&self) -> String {
        format!("{}/ui", self.origin())
    }

    /// Whether the Grid is on this machine (`localhost` or a loopback address).
    #[must_use]
    pub fn is_local(&self) -> bool {
        matches!(self.host.as_str(), "localhost" | "127.0.0.1" | "::1")
    }
}

impl fmt::Display for GridUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.origin())?;
        if self.path != "/" {
            f.write_str(&self.path)?;
        }
        Ok(())
    }
}

impl FromStr for GridUrl {
    type Err = SeleniumBaseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        // An address with an '@' may carry a password; do not echo it.
        let shown = if text.contains('@') {
            "the address".to_owned()
        } else {
            format!("{text:?}")
        };
        let invalid = |reason: &str| {
            SeleniumBaseError::invalid_config(format!(
                "{shown} is not a Selenium Grid address: {reason}"
            ))
        };
        if text.is_empty() {
            return Err(invalid("it is empty"));
        }
        let has_scheme = text.contains("://");
        let url = if has_scheme {
            Url::parse(text)
        } else {
            Url::parse(&format!("http://{text}"))
        }
        .map_err(|error| invalid(&error.to_string()))?;

        let scheme = match url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            other => {
                return Err(invalid(&format!(
                    "the scheme must be http or https, not {other}"
                )))
            }
        };
        if !url.username().is_empty() || url.password().is_some() {
            return Err(invalid(
                "a user name or password in the address is not supported",
            ));
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(invalid("it has a query or fragment"));
        }
        let host = match url.host() {
            Some(Host::Domain(domain)) if !domain.is_empty() => domain.to_owned(),
            Some(Host::Ipv4(address)) => address.to_string(),
            Some(Host::Ipv6(address)) => address.to_string(),
            _ => return Err(invalid("it has no host")),
        };
        let port = url.port().unwrap_or_else(|| {
            // `Url` hides a port equal to the scheme's default, so ":80" on a
            // scheme-less address would look absent; look at the text.
            let authority = text.split(['/', '?', '#']).next().unwrap_or(text);
            let wrote_port_80 = !has_scheme
                && authority
                    .rsplit_once(':')
                    .is_some_and(|(before, port)| port == "80" && !before.ends_with(':'));
            if wrote_port_80 {
                80
            } else if has_scheme {
                scheme.default_port()
            } else {
                DEFAULT_HUB_PORT
            }
        });
        Ok(Self {
            scheme,
            host,
            port,
            path: url.path().to_owned(),
        })
    }
}

impl From<GridUrl> for String {
    fn from(grid: GridUrl) -> Self {
        grid.to_string()
    }
}

/// Builds a Grid URL from a host, a port and an optional path.
///
/// The path defaults to `/wd/hub`, which every Grid generation accepts. A path
/// written without its leading slash gets one.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utilities::selenium_grid::grid_url;
///
/// assert_eq!(grid_url("hub", 4444, None), "http://hub:4444/wd/hub");
/// assert_eq!(grid_url("hub", 4444, Some("custom")), "http://hub:4444/custom");
/// ```
#[must_use]
pub fn grid_url(host: &str, port: u16, path: Option<&str>) -> String {
    GridUrl::new(host, port)
        .with_path(path.unwrap_or("/wd/hub"))
        .to_string()
}

/// The URL of a Grid on this machine on the default port.
#[must_use]
pub fn local_grid_url() -> String {
    grid_url("localhost", DEFAULT_HUB_PORT, None)
}

/// Splits a Grid URL into its host, port and path.
///
/// Returns `None` if `url` is not a valid Grid address; see [`GridUrl`] for
/// what is accepted. The scheme is dropped, so prefer [`GridUrl`] when it
/// matters.
#[must_use]
pub fn parse_grid_url(url: &str) -> Option<(String, u16, String)> {
    let grid: GridUrl = url.parse().ok()?;
    Some((grid.host, grid.port, grid.path))
}

/// What a Grid reports at `/status`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridStatus {
    /// Whether the Grid can take new sessions.
    pub ready: bool,
    /// The Grid's own description of its state.
    pub message: String,
    /// How many nodes are registered with a hub (zero for a node's own status).
    pub nodes: usize,
    /// How many browser slots those nodes offer.
    pub slots: usize,
    /// How many of the slots hold a session.
    pub busy_slots: usize,
}

impl GridStatus {
    /// Reads the JSON a Grid 4 hub or node returns from `/status`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Parse`] if the JSON has no `value.ready`
    /// boolean, which means it did not come from a Selenium Grid.
    pub fn from_json(json: &Value) -> Result<Self, SeleniumBaseError> {
        let value = &json["value"];
        let ready = value["ready"]
            .as_bool()
            .ok_or_else(|| SeleniumBaseError::Parse {
                input: json.to_string(),
                expected: "a Selenium Grid /status reply with value.ready".to_owned(),
            })?;
        let nodes = value["nodes"].as_array().map_or(&[][..], Vec::as_slice);
        let slots = nodes
            .iter()
            .filter_map(|node| node["slots"].as_array())
            .flatten();
        let (total, busy) = slots.fold((0, 0), |(total, busy), slot| {
            (total + 1, busy + usize::from(!slot["session"].is_null()))
        });
        Ok(Self {
            ready,
            message: value["message"].as_str().unwrap_or_default().to_owned(),
            nodes: nodes.len(),
            slots: total,
            busy_slots: busy,
        })
    }
}

/// Asks the Grid at `grid` for its status once.
///
/// # Errors
///
/// Returns an error if the Grid cannot be reached within five seconds, answers
/// with an HTTP error, or answers with something that is not a Grid status.
pub async fn fetch_status(grid: &GridUrl) -> Result<GridStatus, SeleniumBaseError> {
    let url = grid.status_url();
    let mut client = reqwest::Client::builder().timeout(Duration::from_secs(5));
    if grid.is_local() {
        // A proxy in the environment is for the outside world.
        client = client.no_proxy();
    }
    let client = client
        .build()
        .map_err(|error| SeleniumBaseError::network(&url, 0, error.to_string()))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| SeleniumBaseError::network(&url, 0, error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(SeleniumBaseError::network(
            &url,
            status.as_u16(),
            status.to_string(),
        ));
    }
    let json: Value = response
        .json()
        .await
        .map_err(|error| SeleniumBaseError::network(&url, status.as_u16(), error.to_string()))?;
    GridStatus::from_json(&json)
}

/// Polls the Grid's status until it is ready.
///
/// A Grid that is not up yet, or is up but not ready, is polled again after
/// `interval`; this is how a test run waits for a Grid that is still starting.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::WaitTimeout`] if the Grid is not ready after
/// `timeout`.
pub async fn wait_until_ready(
    grid: &GridUrl,
    timeout: Duration,
    interval: Duration,
) -> Result<GridStatus, SeleniumBaseError> {
    let deadline = Instant::now() + timeout;
    loop {
        let last_problem = match fetch_status(grid).await {
            Ok(status) if status.ready => return Ok(status),
            Ok(status) => format!("not ready: {}", status.message),
            Err(error) => error.to_string(),
        };
        if Instant::now() >= deadline {
            return Err(SeleniumBaseError::WaitTimeout(format!(
                "the Selenium Grid at {} was not ready after {timeout:?} ({last_problem})",
                grid.origin()
            )));
        }
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn grid(text: &str) -> GridUrl {
        text.parse().unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    #[test]
    fn a_bare_host_is_http_on_the_hub_port() {
        let g = grid("grid.example.com");
        assert_eq!(
            (g.scheme(), g.host(), g.port(), g.path()),
            (Scheme::Http, "grid.example.com", 4444, "/")
        );
        assert_eq!(g.to_string(), "http://grid.example.com:4444");
    }

    #[test]
    fn host_and_port_without_a_scheme_keep_the_port_even_when_it_is_80() {
        assert_eq!(grid("hub:5555").port(), 5555);
        assert_eq!(grid("hub:80").port(), 80);
        assert_eq!(grid("hub:80").to_string(), "http://hub");
        assert_eq!(grid("hub:80/wd/hub").port(), 80);
    }

    #[test]
    fn a_scheme_without_a_port_uses_the_schemes_port() {
        assert_eq!(grid("http://hub").port(), 80);
        let secure = grid("https://hub.example.com/wd/hub");
        assert_eq!(secure.port(), 443);
        assert_eq!(secure.scheme(), Scheme::Https);
        assert_eq!(secure.to_string(), "https://hub.example.com/wd/hub");
    }

    #[test]
    fn a_full_url_keeps_its_port_and_path() {
        let g = grid("http://localhost:4444/wd/hub");
        assert_eq!(
            (g.host(), g.port(), g.path()),
            ("localhost", 4444, "/wd/hub")
        );
        assert_eq!(g.to_string(), "http://localhost:4444/wd/hub");
        assert_eq!(
            grid("http://localhost:4444/").to_string(),
            "http://localhost:4444"
        );
    }

    #[test]
    fn ipv6_hosts_are_written_in_brackets_and_stored_without() {
        let g = grid("http://[::1]:4444/wd/hub");
        assert_eq!(g.host(), "::1");
        assert_eq!(g.to_string(), "http://[::1]:4444/wd/hub");
        assert!(g.is_local());
        assert_eq!(
            grid("[2001:db8::1]:5555").origin(),
            "http://[2001:db8::1]:5555"
        );
    }

    #[test]
    fn host_names_are_lower_cased() {
        assert_eq!(
            grid("HTTP://Grid.Example.COM:4444").host(),
            "grid.example.com"
        );
    }

    #[test]
    fn what_is_not_a_grid_address_is_refused() {
        for bad in [
            "",
            "   ",
            "ftp://hub:4444",
            "http://",
            "http://hub:99999",
            "http://hub:port",
            "http://user:pw@hub:4444",
            "http://user@hub",
            "http://hub:4444/wd/hub?x=1",
            "http://hub:4444/#frag",
            "exa mple.com",
        ] {
            assert!(bad.parse::<GridUrl>().is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn credentials_are_never_echoed_in_the_error() {
        let error = "http://alice:hunter2@hub:4444"
            .parse::<GridUrl>()
            .unwrap_err();
        let shown = error.to_string();
        assert!(shown.contains("not supported"), "{shown}");
        assert!(
            !shown.contains("hunter2") && !shown.contains("alice"),
            "{shown}"
        );
    }

    #[test]
    fn builders_change_one_part() {
        let g = GridUrl::new("hub", 4444)
            .with_scheme(Scheme::Https)
            .with_path("wd/hub");
        assert_eq!(g.to_string(), "https://hub:4444/wd/hub");
        assert_eq!(g.status_url(), "https://hub:4444/status");
        assert_eq!(g.ui_url(), "https://hub:4444/ui");
        assert!(!g.is_local());
    }

    #[test]
    fn a_grid_url_converts_into_the_webdriver_url_string() {
        let url: String = GridUrl::new("hub", 4444).into();
        assert_eq!(url, "http://hub:4444");
    }

    #[test]
    fn the_free_helpers_agree_with_the_type() {
        assert_eq!(grid_url("hub", 4444, None), "http://hub:4444/wd/hub");
        assert_eq!(grid_url("hub", 80, Some("/x")), "http://hub/x");
        assert_eq!(local_grid_url(), "http://localhost:4444/wd/hub");
        assert_eq!(
            parse_grid_url("http://localhost:4444/wd/hub"),
            Some(("localhost".to_owned(), 4444, "/wd/hub".to_owned()))
        );
        assert_eq!(
            parse_grid_url("hub"),
            Some(("hub".to_owned(), 4444, "/".to_owned()))
        );
        assert_eq!(parse_grid_url("not a url"), None);
    }

    #[test]
    fn a_hub_status_is_summarised() {
        let status = GridStatus::from_json(&json!({
            "value": {
                "ready": true,
                "message": "Selenium Grid ready.",
                "nodes": [
                    { "id": "n1", "slots": [
                        { "session": null },
                        { "session": { "sessionId": "s1" } }
                    ]},
                    { "id": "n2", "slots": [ { "session": null } ] }
                ]
            }
        }))
        .unwrap();
        assert_eq!(
            status,
            GridStatus {
                ready: true,
                message: "Selenium Grid ready.".to_owned(),
                nodes: 2,
                slots: 3,
                busy_slots: 1,
            }
        );
    }

    #[test]
    fn a_status_without_nodes_is_still_a_status() {
        let status = GridStatus::from_json(&json!({
            "value": { "ready": false, "message": "Selenium Grid not ready." }
        }))
        .unwrap();
        assert!(!status.ready);
        assert_eq!((status.nodes, status.slots, status.busy_slots), (0, 0, 0));
    }

    #[test]
    fn json_that_is_not_a_grid_status_is_refused() {
        for json in [
            json!({}),
            json!({ "value": {} }),
            json!({ "value": { "ready": "yes" } }),
            json!([]),
        ] {
            assert!(GridStatus::from_json(&json).is_err(), "{json}");
        }
    }

    /// Serves `/status` on a loopback port: each reply is the next of
    /// `replies` (the last one repeats), as `(HTTP status line, body)`.
    async fn serve_status(replies: Vec<(&'static str, String)>) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut served = 0;
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0_u8; 2048];
                let _ = socket.read(&mut buffer).await;
                let (line, body) = &replies[served.min(replies.len() - 1)];
                served += 1;
                let reply = format!(
                    "HTTP/1.1 {line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        port
    }

    fn status_body(ready: bool) -> String {
        json!({ "value": { "ready": ready, "message": if ready { "ready" } else { "starting" } } })
            .to_string()
    }

    #[tokio::test]
    async fn a_ready_grid_is_reported_ready() {
        let port = serve_status(vec![("200 OK", status_body(true))]).await;
        let status = fetch_status(&GridUrl::new("127.0.0.1", port))
            .await
            .unwrap();
        assert!(status.ready);
    }

    #[tokio::test]
    async fn an_http_error_from_the_grid_is_an_error() {
        let port = serve_status(vec![("503 Service Unavailable", "{}".to_owned())]).await;
        let error = fetch_status(&GridUrl::new("127.0.0.1", port))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("503"), "{error}");
    }

    #[tokio::test]
    async fn a_reply_that_is_not_a_grid_status_is_an_error() {
        let port = serve_status(vec![("200 OK", "<html>hello</html>".to_owned())]).await;
        assert!(fetch_status(&GridUrl::new("127.0.0.1", port))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn waiting_returns_once_the_grid_becomes_ready() {
        let port = serve_status(vec![
            ("200 OK", status_body(false)),
            ("503 Service Unavailable", "{}".to_owned()),
            ("200 OK", status_body(true)),
        ])
        .await;

        let status = wait_until_ready(
            &GridUrl::new("127.0.0.1", port),
            Duration::from_secs(10),
            Duration::from_millis(20),
        )
        .await
        .unwrap();

        assert!(status.ready);
    }

    #[tokio::test]
    async fn waiting_gives_up_and_says_why() {
        let port = serve_status(vec![("200 OK", status_body(false))]).await;

        let error = wait_until_ready(
            &GridUrl::new("127.0.0.1", port),
            Duration::from_millis(200),
            Duration::from_millis(20),
        )
        .await
        .unwrap_err();

        assert!(
            matches!(error, SeleniumBaseError::WaitTimeout(_)),
            "{error}"
        );
        assert!(error.to_string().contains("not ready: starting"), "{error}");
    }

    #[tokio::test]
    async fn waiting_for_a_grid_that_is_not_listening_times_out() {
        // Bind and drop to learn a port nothing listens on.
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            listener.local_addr().unwrap().port()
        };
        let error = wait_until_ready(
            &GridUrl::new("127.0.0.1", port),
            Duration::from_millis(150),
            Duration::from_millis(20),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::WaitTimeout(_)),
            "{error}"
        );
    }
}
