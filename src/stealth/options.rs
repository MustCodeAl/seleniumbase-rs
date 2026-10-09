//! Chrome / Edge option helpers for undetected-chrome (UC) stealth profiles.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::browser::config::BrowserConfig;
use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint::Fingerprint;
use thirtyfour::common::capabilities::chromium::ChromiumLikeCapabilities;

/// Collection of browser launch options that reduce automation fingerprints.
#[derive(Clone, Default)]
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
    /// Optional anti-detection fingerprint profile.
    pub fingerprint: Option<Fingerprint>,
    /// Headers supplied to the CDP network reactor when intercepting requests.
    pub extra_headers: HashMap<String, String>,
    /// Extra Chromium/Edge command-line arguments supplied by integrations.
    pub extra_args: Vec<String>,
    /// Optional explicit path to a patched or custom browser binary.
    pub binary_path: Option<PathBuf>,
}

// Written by hand: the proxy URL may carry a password and the extra headers
// may carry credentials, and neither should reach a log.
impl std::fmt::Debug for StealthOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut header_names: Vec<&String> = self.extra_headers.keys().collect();
        header_names.sort();
        f.debug_struct("StealthOptions")
            .field("headless", &self.headless)
            .field("window_size", &self.window_size)
            .field("user_agent", &self.user_agent)
            .field("locale", &self.locale)
            .field("proxy", &self.proxy.as_deref().map(without_credentials))
            .field(
                "proxy_pac_url",
                &self.proxy_pac_url.as_deref().map(without_credentials),
            )
            .field("user_data_dir", &self.user_data_dir)
            .field("extension_dir", &self.extension_dir)
            .field("mobile", &self.mobile)
            .field("ad_block", &self.ad_block)
            .field("uc", &self.uc)
            .field("fingerprint", &self.fingerprint)
            .field("extra_header_names", &header_names)
            .field("extra_args", &self.extra_args)
            .field("binary_path", &self.binary_path)
            .finish()
    }
}

/// `url` with any `user:password@` removed, for showing in a log.
fn without_credentials(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").map_or(("", url), |(s, r)| (s, r));
    let host_start = rest.rfind('@').map_or(0, |at| at + 1);
    let rest = &rest[host_start..];
    if scheme.is_empty() {
        rest.to_owned()
    } else {
        format!("{scheme}://{rest}")
    }
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
            fingerprint: config.fingerprint.clone(),
            extra_headers: HashMap::new(),
            extra_args: config.extra_args.clone(),
            binary_path: config.browser_binary_path.clone(),
        }
    }
}

impl StealthOptions {
    /// Applies the options to a Chromium-like capabilities object.
    pub fn apply_to<C: ChromiumLikeCapabilities>(
        &self,
        caps: &mut C,
    ) -> Result<(), SeleniumBaseError> {
        // Baseline stability flags. The sandbox is only turned off where it
        // commonly cannot start (containers and root shells on Linux), and the
        // GPU only where there is no screen to draw on, because a browser with
        // no GPU reports a software renderer that pages can tell apart.
        caps.add_arg("--disable-dev-shm-usage")?;
        if cfg!(target_os = "linux") {
            caps.add_arg("--no-sandbox")?;
        }
        if self.headless {
            caps.add_arg("--disable-gpu")?;
            caps.add_arg("--headless=new")?;
        }

        let size =
            self.window_size
                .as_deref()
                .unwrap_or(if self.mobile { "390,844" } else { "1280,720" });
        caps.add_arg(&format!("--window-size={size}"))?;

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

        // A phone user agent only where the caller named none.
        if self.mobile && self.user_agent.is_none() {
            caps.add_arg("--user-agent=Mozilla/5.0 (Linux; Android 10; SM-G975F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36")?;
        }

        if let Some(binary) = self.binary_path.as_deref() {
            let binary_str = binary.display().to_string();
            caps.set_binary(&binary_str).map_err(|e| {
                SeleniumBaseError::invalid_config(format!(
                    "failed to set chrome binary to {binary_str}: {e}"
                ))
            })?;
        }

        if self.uc {
            apply_undetected_args(caps)?;
        }

        if let Some(fp) = self.fingerprint.as_ref() {
            for arg in crate::stealth::evasions::launch_args(fp) {
                caps.add_arg(&arg)?;
            }
        }

        for arg in &self.extra_args {
            if arg.starts_with('-') {
                caps.add_arg(arg)?;
            } else {
                caps.add_arg(&format!("--{arg}"))?;
            }
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

/// The standard undetected-chrome launch arguments.
const UC_ARGS: [&str; 17] = [
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

/// Adds the standard undetected-chrome launch arguments.
pub fn apply_undetected_args<C: ChromiumLikeCapabilities>(
    caps: &mut C,
) -> Result<(), SeleniumBaseError> {
    for arg in UC_ARGS {
        caps.add_arg(arg)?;
    }
    caps.add_exclude_switch("enable-automation")?;
    caps.add_experimental_option("useAutomationExtension", false)?;
    Ok(())
}

/// Returns the default list of undetected-chrome launch arguments.
pub fn default_uc_args() -> Vec<String> {
    UC_ARGS.iter().map(|s| (*s).to_owned()).collect()
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

    #[test]
    fn mobile_defaults_do_not_override_what_the_caller_set() {
        let own = StealthOptions {
            mobile: true,
            user_agent: Some("MyAgent/1.0".to_owned()),
            window_size: Some("500,900".to_owned()),
            ..Default::default()
        };
        let args = own.args().unwrap();
        assert_eq!(
            args.iter()
                .filter(|a| a.starts_with("--user-agent="))
                .count(),
            1,
            "{args:?}"
        );
        assert!(args.contains(&"--user-agent=MyAgent/1.0".to_owned()));
        assert_eq!(
            args.iter()
                .filter(|a| a.starts_with("--window-size="))
                .count(),
            1
        );
        assert!(args.contains(&"--window-size=500,900".to_owned()));

        let bare = StealthOptions {
            mobile: true,
            ..Default::default()
        };
        let args = bare.args().unwrap();
        assert!(args.contains(&"--window-size=390,844".to_owned()));
        assert!(args.iter().any(|a| a.contains("Android")));
    }

    #[test]
    fn gpu_stays_on_unless_headless() {
        let shown = StealthOptions::default().args().unwrap();
        assert!(!shown.contains(&"--disable-gpu".to_owned()));
        let headless = StealthOptions {
            headless: true,
            ..Default::default()
        };
        assert!(headless
            .args()
            .unwrap()
            .contains(&"--disable-gpu".to_owned()));
    }

    #[test]
    fn debug_output_hides_proxy_passwords_and_header_values() {
        let opts = StealthOptions {
            proxy: Some("http://user:hunter2@proxy.example:8080".to_owned()),
            extra_headers: [("Authorization".to_owned(), "Bearer s3cret".to_owned())].into(),
            ..Default::default()
        };
        let shown = format!("{opts:?}");
        assert!(
            !shown.contains("hunter2") && !shown.contains("user:"),
            "{shown}"
        );
        assert!(!shown.contains("s3cret"), "{shown}");
        assert!(shown.contains("http://proxy.example:8080"), "{shown}");
        assert!(shown.contains("Authorization"), "{shown}");
    }

    #[test]
    fn credentials_are_cut_from_urls_for_logs() {
        assert_eq!(without_credentials("socks5://a:b@h:1"), "socks5://h:1");
        assert_eq!(without_credentials("h:1"), "h:1");
        assert_eq!(without_credentials("a:b@h:1"), "h:1");
        // A password that itself contains an `@`.
        assert_eq!(without_credentials("http://a:p@ss@h:1"), "http://h:1");
    }
}
