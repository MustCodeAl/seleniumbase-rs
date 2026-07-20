use serde::{Deserialize, Serialize};

/// Supported browser types.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Browser {
    #[default]
    Chrome,
    Chromium,
    Edge,
    Firefox,
}

/// Driver execution mode.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum DriverMode {
    #[default]
    WebDriver,
    Cdp,
    Uc,
}

/// Configuration for a browser session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BrowserConfig {
    pub webdriver_url: String,
    pub browser: Browser,
    pub headless: bool,
    pub mode: DriverMode,
    pub user_agent: Option<String>,
    pub locale: Option<String>,
    pub ad_block: bool,
    pub proxy: Option<String>,
    pub proxy_pac_url: Option<String>,
    pub user_data_dir: Option<String>,
    pub extension_dir: Option<String>,
    pub start_page: Option<String>,
    pub reuse_session: bool,
    pub mobile: bool,
    pub threads: Option<usize>,
    pub auto_start_driver: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            webdriver_url: "http://localhost:4444".to_owned(),
            browser: Browser::Chrome,
            headless: true,
            mode: DriverMode::WebDriver,
            user_agent: None,
            locale: None,
            ad_block: false,
            proxy: None,
            proxy_pac_url: None,
            user_data_dir: None,
            extension_dir: None,
            start_page: None,
            reuse_session: false,
            mobile: false,
            threads: None,
            auto_start_driver: true,
        }
    }
}

impl BrowserConfig {
    pub fn with_mode(mut self, mode: DriverMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn is_cdp_enabled(&self) -> bool {
        self.mode == DriverMode::Cdp || self.mode == DriverMode::Uc
    }

    pub fn is_uc_enabled(&self) -> bool {
        self.mode == DriverMode::Uc
    }

    pub fn is_default_webdriver_url(&self) -> bool {
        self.webdriver_url == "http://localhost:4444" || self.webdriver_url.is_empty()
    }
}
