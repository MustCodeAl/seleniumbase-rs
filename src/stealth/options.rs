//! Chrome / Edge option helpers for undetected-chrome (UC) stealth profiles.

use std::collections::HashMap;

use crate::browser::config::BrowserConfig;
use crate::error::SeleniumBaseError;
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;

/// Collection of browser launch options that reduce automation fingerprints.
#[derive(Clone, Debug, Default)]
pub struct StealthOptions {
    pub headless: bool,
    pub window_size: Option<String>,
    pub user_agent: Option<String>,
    pub locale: Option<String>,
    pub proxy: Option<String>,
    pub proxy_pac_url: Option<String>,
    pub user_data_dir: Option<String>,
    pub extension_dir: Option<String>,
    pub mobile: bool,
    pub ad_block: bool,
    pub uc: bool,
    /// Headers supplied to the CDP network reactor when intercepting requests.
    pub extra_headers: HashMap<String, String>,
}

impl From<&BrowserConfig> for StealthOptions {
    fn from(config: &BrowserConfig) -> Self {
        Self {
            headless: config.headless,
            window_size: None,
            user_agent: config.user_agent.clone(),
            locale: config.locale.clone(),
            proxy: config.proxy.clone(),
            proxy_pac_url: config.proxy_pac_url.clone(),
            user_data_dir: config.user_data_dir.clone(),
            extension_dir: config.extension_dir.clone(),
            mobile: config.mobile,
            ad_block: config.ad_block,
            uc: config.is_uc_enabled(),
            extra_headers: HashMap::new(),
        }
    }
}

impl StealthOptions {
    /// Applies the options to a Chromium-like capabilities object.
    pub fn apply_to<C: ChromiumLikeCapabilities>(
        &self,
        caps: &mut C,
    ) -> Result<(), SeleniumBaseError> {
        // Baseline stability flags.
        caps.add_arg("--disable-gpu")?;
        caps.add_arg("--disable-dev-shm-usage")?;
        caps.add_arg("--no-sandbox")?;

        let size = self
            .window_size
            .clone()
            .unwrap_or_else(|| "1280,720".to_owned());
        caps.add_arg(&format!("--window-size={size}"))?;

        if self.headless {
            caps.add_arg("--headless=new")?;
        }

        if self.ad_block {
            caps.add_arg("--blink-settings=imagesEnabled=false")?;
        }

        if let Some(locale) = self.locale.as_deref() {
            caps.add_arg(&format!("--lang={locale}"))?;
        }

        if let Some(user_agent) = self.user_agent.as_deref() {
            caps.add_arg(&format!("--user-agent={user_agent}"))?;
        }

        if let Some(proxy) = self.proxy.as_deref() {
            caps.add_arg(&format!("--proxy-server={proxy}"))?;
        }

        if let Some(pac_url) = self.proxy_pac_url.as_deref() {
            caps.add_arg(&format!("--proxy-pac-url={pac_url}"))?;
        }

        if let Some(user_data_dir) = self.user_data_dir.as_deref() {
            caps.add_arg(&format!("--user-data-dir={user_data_dir}"))?;
        }

        if let Some(extension_dir) = self.extension_dir.as_deref() {
            caps.add_arg(&format!("--load-extension={extension_dir}"))?;
        }

        if self.mobile {
            caps.add_arg("--user-agent=Mozilla/5.0 (Linux; Android 10; SM-G975F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36")?;
            caps.add_arg("--window-size=390,844")?;
        }

        if self.uc {
            apply_undetected_args(caps)?;
        }

        Ok(())
    }

    /// Returns the full list of Chrome/Edge launch arguments that these options
    /// would produce.
    pub fn args(&self) -> Result<Vec<String>, SeleniumBaseError> {
        use thirtyfour::BrowserCapabilitiesHelper;
        use thirtyfour::DesiredCapabilities;
        let mut caps = DesiredCapabilities::chrome();
        self.apply_to(&mut caps)?;
        Ok(caps.args())
    }
}

/// Adds the standard undetected-chrome launch arguments.
pub fn apply_undetected_args<C: ChromiumLikeCapabilities>(
    caps: &mut C,
) -> Result<(), SeleniumBaseError> {
    let args = [
        "--disable-blink-features=AutomationControlled",
        "--disable-infobars",
        "--disable-popup-blocking",
        "--no-first-run",
        "--disable-notifications",
        "--disable-background-networking",
        "--disable-client-side-phishing-detection",
        "--disable-default-apps",
        "--disable-prompt-on-repost",
        "--disable-sync",
        "--disable-translate",
        "--metrics-recording-only",
        "--no-default-browser-check",
        "--password-store=basic",
        "--use-mock-keychain",
        "--disable-search-engine-choice-screen",
        "--safebrowsing-disable-download-protection",
    ];
    for arg in args {
        caps.add_arg(arg)?;
    }
    caps.add_exclude_switch("enable-automation")?;
    caps.add_experimental_option("useAutomationExtension", false)?;
    Ok(())
}

/// Returns the default list of undetected-chrome launch arguments.
pub fn default_uc_args() -> Vec<String> {
    [
        "--disable-blink-features=AutomationControlled",
        "--disable-infobars",
        "--disable-popup-blocking",
        "--no-first-run",
        "--disable-notifications",
        "--disable-background-networking",
        "--disable-client-side-phishing-detection",
        "--disable-default-apps",
        "--disable-prompt-on-repost",
        "--disable-sync",
        "--disable-translate",
        "--metrics-recording-only",
        "--no-default-browser-check",
        "--password-store=basic",
        "--use-mock-keychain",
        "--disable-search-engine-choice-screen",
        "--safebrowsing-disable-download-protection",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use thirtyfour::BrowserCapabilitiesHelper;
    use thirtyfour::DesiredCapabilities;

    #[test]
    fn uc_args_contain_automation_switch() {
        let args = default_uc_args();
        assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_owned()));
    }

    #[test]
    fn apply_to_adds_stealth_args() {
        let mut caps = DesiredCapabilities::chrome();
        let opts = StealthOptions {
            uc: true,
            window_size: Some("1920,1080".to_owned()),
            ..Default::default()
        };
        opts.apply_to(&mut caps).unwrap();
        let args = caps.args();
        assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_owned()));
        assert!(args.contains(&"--window-size=1920,1080".to_owned()));
        assert!(!args.iter().any(|a| a.contains("enable-automation")));
    }
}
