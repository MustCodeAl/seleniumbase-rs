// Browser / driver introspection helpers.

impl BaseCase {
    /// Returns true if the configured browser is Chromium-based.
    pub fn is_chromium(&self) -> bool {
        matches!(
            self.config.browser,
            crate::browser::config::Browser::Chrome
                | crate::browser::config::Browser::Chromium
                | crate::browser::config::Browser::Edge
        )
    }

    /// Returns true if the WebDriver session is still responsive.
    pub async fn is_connected(&self) -> bool {
        self.execute_script("return document.readyState;").await.is_ok()
    }

    /// Returns true if `url` is a valid absolute URL.
    pub fn is_valid_url(&self, url: &str) -> bool {
        reqwest::Url::parse(url).is_ok()
    }

    /// Returns browser version information via CDP `Browser.getVersion`.
    async fn browser_version_info(&self) -> Result<serde_json::Value, SeleniumBaseError> {
        self.session
            .execute_cdp_with_params("Browser.getVersion", serde_json::json!({}))
            .await
    }

    /// Returns the browser product string (e.g. "Chrome/126.0.0").
    pub async fn get_browser_version(&self) -> Result<String, SeleniumBaseError> {
        match self.browser_version_info().await {
            Ok(v) => Ok(v["product"].as_str().unwrap_or("unknown").to_owned()),
            Err(_) => Ok(self.get_user_agent().await.unwrap_or_default()),
        }
    }

    /// Alias for `get_browser_version` focused on Chrome/Chromium.
    pub async fn get_chrome_version(&self) -> Result<String, SeleniumBaseError> {
        self.get_browser_version().await
    }

    /// Alias for `get_chrome_version`.
    pub async fn get_chromium_version(&self) -> Result<String, SeleniumBaseError> {
        self.get_chrome_version().await
    }

    /// Returns the chromedriver executable version by running `chromedriver --version`.
    pub fn get_chromedriver_version(&self) -> Result<String, SeleniumBaseError> {
        let output = std::process::Command::new("chromedriver")
            .arg("--version")
            .output()
            .map_err(|e| SeleniumBaseError::InvalidConfig(format!("chromedriver not found: {e}")))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
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
