//! Logging in to a proxy from a WebDriver session.
//!
//! Chrome and Edge read no login from `--proxy-server`, and chromedriver has no
//! command to answer a proxy's password prompt. Python SeleniumBase solves it
//! with a generated browser extension, which Chrome now refuses to load from
//! the command line. This crate answers the prompt over the browser's DevTools
//! endpoint instead: the session's capabilities name it
//! (`goog:chromeOptions.debuggerAddress`, `ms:edgeOptions.debuggerAddress`),
//! and a [`ProxyAuthSidecar`] attaches to it, turns `Fetch` on in every tab and
//! answers the proxy's `407` with the login (`sb_cdp::Browser::connect_answering_proxy`).
//! The credentials go to a prompt that says it comes from a proxy and to nothing
//! else: a website's own `401` is left alone.
//!
//! A remote WebDriver server does not share its browser's DevTools endpoint, so
//! a proxy with a login is refused there before anything starts.

use std::net::IpAddr;

use serde_json::Value;

use crate::browser::config::BrowserConfig;
use crate::browser::options::{PacUrl, ProxyAddress};
use crate::error::SeleniumBaseError;
use crate::sb_cdp;

/// The proxy's user name and password.
pub(crate) type Credentials = (String, String);

/// Why a remote grid cannot be given a proxy with a login.
pub(crate) const REMOTE_GRID_LOGIN: &str = "a proxy with a username and password needs a browser \
     this machine starts: a remote WebDriver server does not share the browser's DevTools \
     endpoint; use the grid's proxy capability (--cap-string) or a proxy without a login";

/// The capability keys under which a Chromium driver reports the DevTools
/// address of the browser it started: Chrome's and Edge's.
const OPTION_KEYS: [&str; 2] = ["goog:chromeOptions", "ms:edgeOptions"];

/// The proxy login a session of `config` has to answer, or `None` when it
/// needs none.
///
/// Only a Chromium browser takes it (Firefox refuses a login when the
/// configuration is checked), from the proxy address, else from the PAC URL.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when the proxy or the PAC URL is not
/// valid, or when the WebDriver server is not on this machine: the message
/// never shows the password.
pub(crate) fn login_for(config: &BrowserConfig) -> Result<Option<Credentials>, SeleniumBaseError> {
    if !config.browser.is_chromium_based() {
        return Ok(None);
    }
    let own = |(user, password): (&str, &str)| (user.to_owned(), password.to_owned());
    let proxy = config
        .proxy
        .as_deref()
        .map(str::parse::<ProxyAddress>)
        .transpose()?
        .and_then(|proxy| proxy.credentials().map(own));
    let credentials = match proxy {
        Some(credentials) => Some(credentials),
        None => config
            .proxy_pac_url
            .as_deref()
            .map(str::parse::<PacUrl>)
            .transpose()?
            .and_then(|pac| pac.credentials().map(own)),
    };
    let Some(credentials) = credentials else {
        return Ok(None);
    };
    if config.is_default_webdriver_url() || server_is_local(&config.webdriver_url) {
        Ok(Some(credentials))
    } else {
        Err(SeleniumBaseError::invalid_config(REMOTE_GRID_LOGIN))
    }
}

/// Whether a WebDriver server at `url` runs on this machine, and so shares its
/// browser's DevTools endpoint.
fn server_is_local(url: &str) -> bool {
    let parsed = reqwest::Url::parse(url)
        .ok()
        .filter(|parsed| parsed.host_str().is_some())
        // `localhost:4444` has no scheme: the parser takes `localhost` for one.
        .or_else(|| reqwest::Url::parse(&format!("http://{url}")).ok());
    let Some(host) = parsed.as_ref().and_then(reqwest::Url::host_str) else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host.to_ascii_lowercase().ends_with(".localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The `host:port` of the browser's DevTools endpoint, from the capabilities
/// the driver reported for the new session.
pub(crate) fn debugger_address(capabilities: &Value) -> Option<String> {
    OPTION_KEYS.iter().find_map(|key| {
        capabilities[key]["debuggerAddress"]
            .as_str()
            .filter(|address| !address.is_empty())
            .map(str::to_owned)
    })
}

/// `host` and `port` of a `host:port` address (an IPv6 host may be in
/// brackets).
fn split_address(address: &str) -> Option<(&str, u16)> {
    let address = address
        .trim()
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let (host, port) = address.rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    (!host.is_empty()).then_some(())?;
    Some((host, port.parse().ok()?))
}

/// A DevTools client attached beside a WebDriver session, which answers the
/// browser's proxy prompts for as long as it lives.
///
/// Every request of every tab passes through the client while it is attached,
/// so it is closed with the session ([`close`](Self::close)); dropping it
/// closes its connection too, and the browser lets the requests it held go.
pub(crate) struct ProxyAuthSidecar {
    browser: sb_cdp::Browser,
}

impl ProxyAuthSidecar {
    /// Attaches to the browser at `debugger_address` and answers its proxy
    /// prompts with `credentials`.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the address is not `host:port` or
    /// the browser does not answer there. The message never shows the
    /// credentials.
    pub(crate) async fn attach(
        debugger_address: &str,
        credentials: Credentials,
    ) -> Result<Self, SeleniumBaseError> {
        let Some((host, port)) = split_address(debugger_address) else {
            return Err(SeleniumBaseError::cdp_driver(format!(
                "the browser's DevTools address {debugger_address:?} is not host:port, so its \
                 proxy cannot be logged in to"
            )));
        };
        let browser = sb_cdp::Browser::connect_answering_proxy(host, port, credentials)
            .await
            .map_err(|error| {
                SeleniumBaseError::cdp_driver(format!(
                    "the proxy login could not use the browser's DevTools endpoint at \
                     {debugger_address}: {error}"
                ))
            })?;
        Ok(Self { browser })
    }

    /// Detaches from the browser. Says so in the log when the connection had
    /// ended before the session did, since the proxy was not answered after
    /// that.
    pub(crate) async fn close(self) {
        if !self.browser.is_connected() {
            tracing::warn!(
                "the proxy login lost its connection to the browser before the session ended; \
                 the proxy's password prompts were not answered after that"
            );
        }
        // Only a connection is closed: the browser is chromedriver's.
        let _ = self.browser.close().await;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::browser::config::Browser;

    const PASSWORD: &str = "s3cret-proxy-password";

    fn login() -> BrowserConfig {
        BrowserConfig::default().with_proxy(format!("alice:{PASSWORD}@proxy.test:8080"))
    }

    #[test]
    fn the_address_is_read_from_chrome_options() {
        let capabilities = json!({
            "browserName": "chrome",
            "goog:chromeOptions": { "debuggerAddress": "localhost:51234" },
        });
        assert_eq!(
            debugger_address(&capabilities).as_deref(),
            Some("localhost:51234")
        );
    }

    #[test]
    fn the_address_is_read_from_edge_options() {
        let capabilities = json!({
            "browserName": "MicrosoftEdge",
            "ms:edgeOptions": { "debuggerAddress": "localhost:9333" },
        });
        assert_eq!(
            debugger_address(&capabilities).as_deref(),
            Some("localhost:9333")
        );
    }

    #[test]
    fn a_driver_that_names_no_address_gives_none() {
        assert_eq!(debugger_address(&json!({})), None);
        assert_eq!(debugger_address(&json!({ "goog:chromeOptions": {} })), None);
        assert_eq!(
            debugger_address(&json!({ "goog:chromeOptions": { "debuggerAddress": "" } })),
            None
        );
        assert_eq!(
            debugger_address(&json!({ "goog:chromeOptions": { "debuggerAddress": 9222 } })),
            None
        );
        assert_eq!(debugger_address(&Value::Null), None);
    }

    #[test]
    fn addresses_split_into_host_and_port() {
        assert_eq!(split_address("localhost:9222"), Some(("localhost", 9222)));
        assert_eq!(split_address("127.0.0.1:1"), Some(("127.0.0.1", 1)));
        assert_eq!(split_address("[::1]:9222"), Some(("::1", 9222)));
        assert_eq!(
            split_address("http://localhost:9222/"),
            Some(("localhost", 9222))
        );
        assert_eq!(split_address("localhost"), None);
        assert_eq!(split_address("localhost:port"), None);
        assert_eq!(split_address(":9222"), None);
        assert_eq!(split_address("localhost:70000"), None);
    }

    #[test]
    fn a_chromium_session_with_a_proxy_login_wants_the_sidecar() {
        for browser in [Browser::Chrome, Browser::Edge, Browser::Brave] {
            let mut config = login();
            config.browser = browser;
            assert_eq!(
                login_for(&config).expect("valid"),
                Some(("alice".to_owned(), PASSWORD.to_owned())),
                "{browser}"
            );
        }
    }

    #[test]
    fn a_pac_file_with_a_login_counts_too() {
        let config =
            BrowserConfig::default().with_proxy_pac_url("pac-user:pac-pass@http://pac.test/p.pac");
        assert_eq!(
            login_for(&config).expect("valid"),
            Some(("pac-user".to_owned(), "pac-pass".to_owned()))
        );
    }

    #[test]
    fn the_proxys_login_wins_over_the_pac_files() {
        let config = login().with_proxy_pac_url("pac-user:pac-pass@http://pac.test/p.pac");
        assert_eq!(
            login_for(&config).expect("valid").map(|(user, _)| user),
            Some("alice".to_owned())
        );
    }

    #[test]
    fn a_proxy_without_a_login_or_no_proxy_wants_nothing() {
        assert_eq!(login_for(&BrowserConfig::default()).expect("valid"), None);
        let plain = BrowserConfig::default().with_proxy("proxy.test:8080");
        assert_eq!(login_for(&plain).expect("valid"), None);
        let pac = BrowserConfig::default().with_proxy_pac_url("http://pac.test/p.pac");
        assert_eq!(login_for(&pac).expect("valid"), None);
    }

    #[test]
    fn a_browser_that_is_not_chromium_wants_nothing() {
        for browser in [Browser::Firefox, Browser::Safari, Browser::Remote] {
            let mut config = login();
            config.browser = browser;
            assert_eq!(login_for(&config).expect("valid"), None, "{browser}");
        }
    }

    #[test]
    fn a_remote_grid_with_a_login_is_refused_without_the_password() {
        let config = login().with_webdriver_url("http://USER:KEY@hub.browserstack.com/wd/hub");
        let error = login_for(&config).expect_err("refused").to_string();
        assert!(
            error.contains("needs a browser this machine starts"),
            "{error}"
        );
        assert!(error.contains("--cap-string"), "{error}");
        assert!(
            !error.contains(PASSWORD) && !error.contains("KEY"),
            "{error}"
        );
    }

    #[test]
    fn a_remote_grid_without_a_login_is_fine() {
        let config = BrowserConfig::default()
            .with_proxy("proxy.test:8080")
            .with_webdriver_url("http://hub.example.com:4444/wd/hub");
        assert_eq!(login_for(&config).expect("valid"), None);
    }

    #[test]
    fn servers_on_this_machine_are_local() {
        for url in [
            "http://localhost:4444",
            "http://localhost:9515/wd/hub",
            "http://127.0.0.1:4444",
            "http://127.0.0.5:4444",
            "http://[::1]:4444",
            "localhost:4444",
            "http://LOCALHOST:4444",
            "http://grid.localhost:4444",
        ] {
            assert!(server_is_local(url), "{url}");
        }
    }

    #[test]
    fn other_servers_are_remote() {
        for url in [
            "http://hub.browserstack.com/wd/hub",
            "https://USER:KEY@ondemand.saucelabs.com:443/wd/hub",
            "http://192.168.1.20:4444",
            "http://localhost.example.com:4444",
            "http://[2001:db8::1]:4444",
            "hub.example.com:4444",
        ] {
            assert!(!server_is_local(url), "{url}");
        }
    }

    #[test]
    fn a_local_server_on_another_port_may_be_given_the_login() {
        let config = login().with_webdriver_url("http://localhost:9515");
        assert!(login_for(&config).expect("valid").is_some());
    }

    #[test]
    fn an_invalid_proxy_is_refused_without_the_password() {
        let config = BrowserConfig::default().with_proxy(format!("alice:{PASSWORD}@proxy.test"));
        let error = login_for(&config).expect_err("no port").to_string();
        assert!(!error.contains(PASSWORD), "{error}");
    }

    #[tokio::test]
    async fn a_browser_that_does_not_answer_is_an_error_without_the_password() {
        let attached =
            ProxyAuthSidecar::attach("127.0.0.1:1", ("alice".to_owned(), PASSWORD.to_owned()))
                .await;
        let Err(error) = attached else {
            panic!("nothing listens on port 1");
        };
        let message = error.to_string();
        assert!(message.contains("127.0.0.1:1"), "{message}");
        assert!(!message.contains(PASSWORD), "{message}");
    }

    #[tokio::test]
    async fn an_address_that_is_not_host_and_port_is_an_error() {
        let attached =
            ProxyAuthSidecar::attach("nonsense", ("alice".to_owned(), PASSWORD.to_owned())).await;
        let Err(error) = attached else {
            panic!("not an address");
        };
        assert!(error.to_string().contains("not host:port"), "{error}");
    }
}
