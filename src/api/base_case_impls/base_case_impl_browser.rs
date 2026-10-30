// Browser / driver introspection helpers.

/// The `Name/version` token a `User-Agent` string gives for its browser, such
/// as `Chrome/139.0.7258.66`.
///
/// Edge is tried first because its user agent also says `Chrome/`.
fn product_from_user_agent(user_agent: &str) -> Option<String> {
    ["Edg/", "Firefox/", "Chrome/"].iter().find_map(|name| {
        let start = user_agent.find(name)?;
        let token = user_agent[start..].split_whitespace().next()?;
        (token.len() > name.len()).then(|| token.to_owned())
    })
}

/// The driver version in a session's capabilities, if it reports one.
///
/// ChromeDriver reports `chrome.chromedriverVersion` and Edge's driver
/// `msedge.msedgedriverVersion`, each as `152.0.7977.64 (commit hash)`; the
/// number before the first space is returned.
fn driver_version_from_capabilities(capabilities: &Value) -> Option<String> {
    [
        ("chrome", "chromedriverVersion"),
        ("msedge", "msedgedriverVersion"),
    ]
    .iter()
    .find_map(|(vendor, key)| capabilities.get(vendor)?.get(key)?.as_str())
    .and_then(|version| version.split_whitespace().next())
    .map(str::to_owned)
}

/// Whether `url` is one SeleniumBase's `is_valid_url` accepts: an `http` or
/// `https` URL with a host name, `localhost` or an IPv4 address (and an
/// optional port and path), or one of the browser schemes `about:`, `blob:`,
/// `chrome:`, `data:`, `edge:` and `file:`.
fn is_valid_url_text(url: &str) -> bool {
    // Python's pattern from `page_utils.is_valid_url`, case-insensitive.
    static HTTP_URL: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let http_url = HTTP_URL.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)^https?://",
            r"(?:(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+(?:[a-z]{2,6}\.?|[a-z0-9-]{2,}\.?)",
            r"|localhost",
            r"|[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3})",
            r"(?::[0-9]+)?",
            r"(?:/?|[/?]\S+)$",
        ))
        .expect("the URL pattern is valid")
    });
    http_url.is_match(url)
        || ["about:", "blob:", "chrome:", "data:", "edge:", "file:"]
            .iter()
            .any(|scheme| url.starts_with(scheme))
}

/// The version number in a browser product string: `126.0.6478.126` for
/// `Chrome/126.0.6478.126`. A string with no `/` is returned as it is.
fn version_number(product: &str) -> String {
    product.rsplit('/').next().unwrap_or(product).to_owned()
}

/// The first component of a dotted version: `126` for `126.0.6478.126`.
fn major_version(version: &str) -> String {
    version.split('.').next().unwrap_or(version).to_owned()
}

impl BaseCase {
    /// Returns true if the configured browser is Chromium-based.
    pub fn is_chromium(&self) -> bool {
        self.config.browser.is_chromium_based()
    }

    /// Whether the session can run DevTools Protocol commands.
    /// Corresponds to Python's `driver.is_cdp_mode_active()`.
    ///
    /// True for a session started in [`DriverMode::Cdp`](crate::DriverMode::Cdp)
    /// or [`DriverMode::Uc`](crate::DriverMode::Uc), which are the two that get
    /// a CDP client. Python reports true only after
    /// `activate_cdp_mode()`, once the browser is driven without a driver; this
    /// crate keeps that for the pure CDP engine in [`crate::sb_cdp`], where every
    /// session is in CDP mode.
    #[must_use]
    pub fn is_cdp_mode_active(&self) -> bool {
        self.config.is_cdp_enabled()
    }

    /// Whether the session runs in undetected (UC) mode.
    /// Corresponds to Python's `driver.is_uc_mode_active()`.
    #[must_use]
    pub fn is_uc_mode_active(&self) -> bool {
        self.config.is_uc_enabled()
    }

    /// Returns true if the WebDriver session is still responsive.
    pub async fn is_connected(&self) -> bool {
        self.execute_script("return document.readyState;").await.is_ok()
    }

    /// Returns true if `url` is one Python SeleniumBase's `is_valid_url`
    /// accepts: `http(s)://` with a host name, `localhost` or an IPv4 address,
    /// or an `about:`, `blob:`, `chrome:`, `data:`, `edge:` or `file:` URL.
    /// `mailto:`, `javascript:` and a bare `host:port` are not valid.
    pub fn is_valid_url(&self, url: &str) -> bool {
        is_valid_url_text(url)
    }

    /// Returns browser version information via CDP `Browser.getVersion`.
    async fn browser_version_info(&self) -> Result<serde_json::Value, SeleniumBaseError> {
        self.session
            .execute_cdp_with_params("Browser.getVersion", serde_json::json!({}))
            .await
    }

    /// Returns the browser product string (e.g. "Chrome/126.0.0").
    ///
    /// The browser is asked over CDP. A session without CDP (a Firefox session,
    /// or a Chrome one started without it) falls back to the version in the
    /// browser's user agent.
    ///
    /// # Errors
    ///
    /// Returns the CDP error if neither source gives a version, rather than an
    /// empty or made-up string.
    pub async fn get_browser_version(&self) -> Result<String, SeleniumBaseError> {
        match self.browser_version_info().await {
            Ok(info) => info["product"].as_str().map(str::to_owned).ok_or_else(|| {
                SeleniumBaseError::Unsupported("Browser.getVersion reported no product".to_owned())
            }),
            Err(cdp_error) => self
                .get_user_agent()
                .await
                .ok()
                .and_then(|user_agent| product_from_user_agent(&user_agent))
                .ok_or(cdp_error),
        }
    }

    /// The browser's version number, such as `126.0.6478.126`, without the
    /// product name that [`get_browser_version`](Self::get_browser_version)
    /// keeps. Corresponds to Python's `get_chrome_version`.
    ///
    /// # Errors
    ///
    /// See [`get_browser_version`](Self::get_browser_version).
    pub async fn get_chrome_version(&self) -> Result<String, SeleniumBaseError> {
        self.get_browser_version()
            .await
            .map(|product| version_number(&product))
    }

    /// The browser's major version, such as `126`. Corresponds to Python's
    /// `get_chromium_version`.
    ///
    /// # Errors
    ///
    /// See [`get_browser_version`](Self::get_browser_version).
    pub async fn get_chromium_version(&self) -> Result<String, SeleniumBaseError> {
        self.get_chrome_version()
            .await
            .map(|version| major_version(&version))
    }

    /// Returns the version of the driver this session is running on, such as
    /// `152.0.7977.64`.
    ///
    /// The version comes from the capabilities the driver returned when it
    /// created the session, so it is the version of the driver actually in use,
    /// not of whatever `chromedriver` is first on the `PATH`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] if the session reports no
    /// driver version, as a Firefox session does not.
    pub fn get_chromedriver_version(&self) -> Result<String, SeleniumBaseError> {
        let capabilities = serde_json::to_value(self.session.webdriver()?.capabilities())?;
        driver_version_from_capabilities(&capabilities).ok_or_else(|| {
            SeleniumBaseError::Unsupported(
                "this session's driver did not report a version".to_owned(),
            )
        })
    }

    /// Alias for `get_chromedriver_version`.
    pub fn get_chromium_driver_version(&self) -> Result<String, SeleniumBaseError> {
        self.get_chromedriver_version()
    }

    /// Returns true if the browser reports an online network state.
    pub async fn is_online(&self) -> Result<bool, SeleniumBaseError> {
        match self.execute_script("return navigator.onLine;").await? {
            Value::Bool(v) => Ok(v),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests_browser {
    use super::*;
    use crate::browser::config::{BrowserConfig, DriverMode};
    use crate::browser::session::BrowserSession;
    use serde_json::json;

    fn sb(mode: DriverMode) -> BaseCase {
        BaseCase::with_session(
            BrowserConfig::default().with_mode(mode),
            BrowserSession::disconnected(),
        )
    }

    #[test]
    fn the_mode_questions_follow_the_mode_the_session_was_started_in() {
        let plain = sb(DriverMode::WebDriver);
        assert!(!plain.is_cdp_mode_active() && !plain.is_uc_mode_active());

        let cdp = sb(DriverMode::Cdp);
        assert!(cdp.is_cdp_mode_active() && !cdp.is_uc_mode_active());

        // Undetected mode runs on the CDP client too.
        let uc = sb(DriverMode::Uc);
        assert!(uc.is_cdp_mode_active() && uc.is_uc_mode_active());
    }

    #[test]
    fn a_url_is_valid_only_when_python_would_accept_it() {
        for url in [
            "https://seleniumbase.io",
            "http://seleniumbase.io/demo_page?a=1",
            "HTTPS://EXAMPLE.COM/",
            "http://localhost",
            "http://localhost:8080/path",
            "http://127.0.0.1:9515",
            "https://a.b.example.co.uk/x/y",
            "about:blank",
            "blob:https://x/1",
            "chrome://version",
            "data:text/html,<p>x</p>",
            "edge://settings",
            "file:///tmp/a.html",
        ] {
            assert!(is_valid_url_text(url), "{url} should be valid");
        }
        for url in [
            "",
            "mailto:a@b.c",
            "javascript:alert(1)",
            "foo:bar",
            "ftp://example.com/file",
            "seleniumbase.io",
            "http://",
            "http://exa mple.com",
            "https://-bad.example.com",
            "localhost:8080",
        ] {
            assert!(!is_valid_url_text(url), "{url} should not be valid");
        }
    }

    #[test]
    fn the_version_loses_its_product_name_and_the_major_version_its_tail() {
        assert_eq!(version_number("Chrome/126.0.6478.126"), "126.0.6478.126");
        assert_eq!(version_number("HeadlessChrome/139.0.0.0"), "139.0.0.0");
        assert_eq!(version_number("Edg/139.0.3405.86"), "139.0.3405.86");
        assert_eq!(version_number("126.0.6478.126"), "126.0.6478.126");
        assert_eq!(major_version("126.0.6478.126"), "126");
        assert_eq!(major_version("126"), "126");
        // The two compose the way `get_chromium_version` uses them.
        assert_eq!(major_version(&version_number("Chrome/126.0.6478.126")), "126");
    }

    #[test]
    fn the_product_comes_out_of_the_user_agent() {
        let chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.7258.66 Safari/537.36";
        assert_eq!(product_from_user_agent(chrome).as_deref(), Some("Chrome/139.0.7258.66"));
        let headless = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/139.0.0.0 Safari/537.36";
        assert_eq!(product_from_user_agent(headless).as_deref(), Some("Chrome/139.0.0.0"));
        let edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36 Edg/139.0.3405.86";
        assert_eq!(product_from_user_agent(edge).as_deref(), Some("Edg/139.0.3405.86"));
        let firefox = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
        assert_eq!(product_from_user_agent(firefox).as_deref(), Some("Firefox/140.0"));
    }

    #[test]
    fn a_user_agent_without_a_known_browser_gives_no_version() {
        for user_agent in ["", "curl/8.4.0", "Mozilla/5.0 Chrome/", "Chrome"] {
            assert_eq!(product_from_user_agent(user_agent), None, "{user_agent:?}");
        }
    }

    #[test]
    fn the_driver_version_is_the_number_before_the_hash() {
        let chrome = json!({
            "browserName": "chrome",
            "chrome": { "chromedriverVersion": "152.0.7977.64 (506c834ecceaa943c5f41e6cfe7f68acb5c45346-refs/branch-heads/7977@{#1891})" }
        });
        assert_eq!(driver_version_from_capabilities(&chrome).as_deref(), Some("152.0.7977.64"));
        let edge = json!({ "msedge": { "msedgedriverVersion": "139.0.3405.86 (abc)" } });
        assert_eq!(driver_version_from_capabilities(&edge).as_deref(), Some("139.0.3405.86"));
    }

    #[test]
    fn a_session_that_reports_no_driver_version_is_not_given_one() {
        for capabilities in [
            json!({}),
            json!({ "browserName": "firefox", "moz:geckodriverVersion": "0.36.0" }),
            json!({ "chrome": {} }),
            json!({ "chrome": { "chromedriverVersion": "" } }),
            json!({ "chrome": { "chromedriverVersion": 152 } }),
        ] {
            assert_eq!(driver_version_from_capabilities(&capabilities), None, "{capabilities}");
        }
    }
}
