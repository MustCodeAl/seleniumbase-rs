use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::browser::config::{Browser, BrowserConfig, DriverMode};
use crate::error::SeleniumBaseError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub browser: String,
    pub headless: bool,
    pub timeout_seconds: u64,
    pub screenshot_dir: String,
    pub proxy: Option<String>,
    pub proxy_pac_url: Option<String>,
    pub window_width: u32,
    pub window_height: u32,
    pub user_data_dir: Option<String>,
    pub extension_dir: Option<String>,
    pub locale: Option<String>,
    pub user_agent: Option<String>,
    pub mode: Option<String>,
    pub ad_block: bool,
    pub reuse_session: bool,
    pub mobile: bool,
    pub threads: Option<usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            browser: "chrome".to_owned(),
            headless: false,
            timeout_seconds: 30,
            screenshot_dir: "screenshots".to_owned(),
            proxy: None,
            proxy_pac_url: None,
            window_width: 1920,
            window_height: 1080,
            user_data_dir: None,
            extension_dir: None,
            locale: None,
            user_agent: None,
            mode: None,
            ad_block: false,
            reuse_session: false,
            mobile: false,
            threads: None,
        }
    }
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to read settings: {e}"))
        })?;
        if path.extension().is_some_and(|ext| ext == "toml") {
            toml::from_str(&content).map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("failed to parse TOML settings: {e}"))
            })
        } else {
            serde_json::from_str(&content).map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("failed to parse settings: {e}"))
            })
        }
    }

    pub fn from_toml<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let content = fs::read_to_string(path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to read TOML settings: {e}"))
        })?;
        toml::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse TOML settings: {e}"))
        })
    }

    pub fn from_env() -> Result<Self, SeleniumBaseError> {
        let settings = Self::default();
        Self::apply_env_overrides(settings)
    }

    /// Loads the global config file if one exists.
    ///
    /// Searches, in order: `sbase_config.toml`, `.sbase_config.toml`, `.sbase_config`.
    pub fn load_global() -> Result<Self, SeleniumBaseError> {
        let candidates = ["sbase_config.toml", ".sbase_config.toml", ".sbase_config"];
        for candidate in &candidates {
            if Path::new(candidate).is_file() {
                return Self::from_file(candidate);
            }
        }
        Ok(Self::default())
    }

    pub fn load<P: AsRef<Path>>(path: Option<P>) -> Result<Self, SeleniumBaseError> {
        let settings = match path {
            Some(p) => Self::from_file(p)?,
            None => Self::load_global()?,
        };
        Self::apply_env_overrides(settings)
    }

    /// Converts these settings into a `BrowserConfig` used by `BaseCase`.
    pub fn to_browser_config(&self) -> BrowserConfig {
        let browser = match self.browser.to_lowercase().as_str() {
            "chromium" => Browser::Chromium,
            "edge" => Browser::Edge,
            "firefox" => Browser::Firefox,
            _ => Browser::Chrome,
        };
        let mode = self
            .mode
            .as_deref()
            .map(|m| match m.to_lowercase().as_str() {
                "cdp" => DriverMode::Cdp,
                "uc" => DriverMode::Uc,
                _ => DriverMode::WebDriver,
            })
            .unwrap_or(DriverMode::WebDriver);
        BrowserConfig {
            webdriver_url: "http://localhost:4444".to_owned(),
            browser,
            headless: self.headless,
            mode,
            user_agent: self.user_agent.clone(),
            locale: self.locale.clone(),
            ad_block: self.ad_block,
            proxy: self.proxy.clone(),
            proxy_pac_url: self.proxy_pac_url.clone(),
            user_data_dir: self.user_data_dir.clone(),
            extension_dir: self.extension_dir.clone(),
            reuse_session: self.reuse_session,
            mobile: self.mobile,
            threads: self.threads,
            auto_start_driver: true,
            start_page: None,
        }
    }

    fn apply_env_overrides(mut settings: Self) -> Result<Self, SeleniumBaseError> {
        if let Ok(v) = std::env::var("SB_BROWSER") {
            settings.browser = v;
        }
        if let Ok(v) = std::env::var("SB_HEADLESS") {
            settings.headless = parse_bool(&v)?;
        }
        if let Ok(v) = std::env::var("SB_TIMEOUT") {
            settings.timeout_seconds = v
                .parse()
                .map_err(|e| SeleniumBaseError::InvalidConfig(format!("SB_TIMEOUT: {e}")))?;
        }
        if let Ok(v) = std::env::var("SB_SCREENSHOT_DIR") {
            settings.screenshot_dir = v;
        }
        if let Ok(v) = std::env::var("SB_PROXY") {
            settings.proxy = Some(v);
        }
        if let Ok(v) = std::env::var("SB_WINDOW_WIDTH") {
            settings.window_width = v
                .parse()
                .map_err(|e| SeleniumBaseError::InvalidConfig(format!("SB_WINDOW_WIDTH: {e}")))?;
        }
        if let Ok(v) = std::env::var("SB_WINDOW_HEIGHT") {
            settings.window_height = v
                .parse()
                .map_err(|e| SeleniumBaseError::InvalidConfig(format!("SB_WINDOW_HEIGHT: {e}")))?;
        }
        if let Ok(v) = std::env::var("SB_USER_DATA_DIR") {
            settings.user_data_dir = Some(v);
        }
        if let Ok(v) = std::env::var("SB_EXTENSION_DIR") {
            settings.extension_dir = Some(v);
        }
        if let Ok(v) = std::env::var("SB_LOCALE") {
            settings.locale = Some(v);
        }
        if let Ok(v) = std::env::var("SB_USER_AGENT") {
            settings.user_agent = Some(v);
        }
        if let Ok(v) = std::env::var("SB_MODE") {
            settings.mode = Some(v);
        }
        if let Ok(v) = std::env::var("SB_AD_BLOCK") {
            settings.ad_block = parse_bool(&v)?;
        }
        if let Ok(v) = std::env::var("SB_REUSE_SESSION") {
            settings.reuse_session = parse_bool(&v)?;
        }
        if let Ok(v) = std::env::var("SB_MOBILE") {
            settings.mobile = parse_bool(&v)?;
        }
        if let Ok(v) = std::env::var("SB_THREADS") {
            settings.threads = Some(
                v.parse()
                    .map_err(|e| SeleniumBaseError::InvalidConfig(format!("SB_THREADS: {e}")))?,
            );
        }
        if let Ok(v) = std::env::var("SB_PROXY_PAC_URL") {
            settings.proxy_pac_url = Some(v);
        }
        Ok(settings)
    }
}

fn parse_bool(value: &str) -> Result<bool, SeleniumBaseError> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(SeleniumBaseError::InvalidConfig(format!(
            "cannot parse bool: {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_settings_are_sensible() {
        let s = Settings::default();
        assert_eq!(s.browser, "chrome");
        assert!(!s.headless);
        assert_eq!(s.timeout_seconds, 30);
        assert_eq!(s.screenshot_dir, "screenshots");
        assert_eq!(s.window_width, 1920);
        assert_eq!(s.window_height, 1080);
        assert!(s.proxy.is_none());
    }

    #[test]
    fn settings_from_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let json = r#"{"browser":"firefox","headless":true,"timeout_seconds":60,"screenshot_dir":"shots","proxy":"http://proxy:8080","window_width":1280,"window_height":720,"user_data_dir":null}"#;
        tmp.write_all(json.as_bytes()).unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.browser, "firefox");
        assert!(s.headless);
        assert_eq!(s.timeout_seconds, 60);
        assert_eq!(s.screenshot_dir, "shots");
        assert_eq!(s.proxy, Some("http://proxy:8080".to_string()));
        assert_eq!(s.window_width, 1280);
        assert_eq!(s.window_height, 720);
    }

    #[test]
    fn settings_from_toml() {
        let mut tmp = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        let toml = r#"
browser = "edge"
headless = true
proxy = "http://proxy:8080"
mode = "uc"
mobile = true
threads = 4
"#;
        tmp.write_all(toml.as_bytes()).unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.browser, "edge");
        assert!(s.headless);
        assert_eq!(s.proxy, Some("http://proxy:8080".to_string()));
        assert_eq!(s.mode, Some("uc".to_string()));
        assert!(s.mobile);
        assert_eq!(s.threads, Some(4));
    }

    #[test]
    fn to_browser_config_maps_fields() {
        let s = Settings {
            browser: "firefox".to_string(),
            headless: true,
            mode: Some("cdp".to_string()),
            mobile: true,
            threads: Some(2),
            proxy_pac_url: Some("http://proxy/proxy.pac".to_string()),
            ..Default::default()
        };
        let config = s.to_browser_config();
        assert!(matches!(
            config.browser,
            crate::browser::config::Browser::Firefox
        ));
        assert!(config.headless);
        assert!(matches!(config.mode, DriverMode::Cdp));
        assert!(config.mobile);
        assert_eq!(config.threads, Some(2));
        assert_eq!(
            config.proxy_pac_url,
            Some("http://proxy/proxy.pac".to_string())
        );
    }

    #[test]
    fn parse_bool_values() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("YES").unwrap());
        assert!(parse_bool("1").unwrap());
        assert!(!parse_bool("false").unwrap());
        assert!(!parse_bool("OFF").unwrap());
        assert!(parse_bool("not-a-bool").is_err());
    }
}
