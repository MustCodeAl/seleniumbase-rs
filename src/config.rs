use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Browser {
    #[default]
    Chrome,
    Chromium,
    Edge,
    Firefox,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum DriverMode {
    #[default]
    WebDriver,
    Cdp,
    Uc,
}

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
}
