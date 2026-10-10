//! Playwright-backed browser session for a stealthy automation mode.
//!
//! This module is only available when the `playwright` feature is enabled. It
//! wraps the [`rustwright`](https://github.com/Skyvern-AI/rustwright) native
//! Rust CDP engine to launch Chromium with anti-detection arguments and
//! exposes a small, async API that mirrors the WebDriver-backed
//! [`BrowserSession`] where practical.
//!
//! # Driver installation
//!
//! `rustwright` bundles Chromium discovery and launch logic. On first launch it
//! may need to download a Chromium build. Ensure the build host can reach the
//! Chromium CDN, or point at a local Chromium with
//! [`BrowserConfig::browser_binary_path`] and
//! [`PlaywrightSession::launch_with_config`].
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "playwright")]
//! # async fn example() -> Result<(), seleniumbase_rs::error::SeleniumBaseError> {
//! use seleniumbase_rs::browser::playwright::PlaywrightSession;
//!
//! let mut session = PlaywrightSession::launch().await?;
//! session.goto("https://example.com").await?;
//! let text = session.get_text("h1").await?;
//! session.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! [`BrowserSession`]: crate::browser::session::BrowserSession
//! [`BrowserConfig::browser_binary_path`]: crate::browser::config::BrowserConfig::browser_binary_path

use std::path::Path;

use rustwright::{
    chromium, ActionOptions, ActionabilityError, Browser, Error as RwError, LaunchOptions, Page,
    ProxyOptions, ScreenshotOptions,
};
use serde_json::Value;

use crate::browser::config::BrowserConfig;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::Proxy;
use crate::stealth::fingerprint::Fingerprint;

/// Chrome switches every session starts with, on top of rustwright's own.
///
/// Nothing here weakens the browser: web security and site isolation stay on,
/// because a session visits pages the caller did not write. The sandbox is
/// handled in [`launch_options`], since it can only be left off where it
/// cannot start.
const STEALTH_ARGS: &[&str] = &[
    "--disable-blink-features=AutomationControlled",
    "--disable-infobars",
    "--disable-dev-shm-usage",
    "--window-size=1920,1080",
    "--start-maximized",
];

/// Whether the sandbox is left off by default: only on Linux, where
/// containers and root shells commonly cannot start it. Elsewhere Chrome
/// refuses nothing and shows a warning bar if the sandbox is off.
const SANDBOX_OFF_BY_DEFAULT: bool = cfg!(target_os = "linux");

/// A browser session backed by the native Rust `rustwright` CDP engine.
///
/// Holds ownership of the browser and active page. Call
/// [`PlaywrightSession::launch`] to create a session, then use the helper
/// methods to navigate and interact with pages.
pub struct PlaywrightSession {
    browser: Browser,
    page: Page,
    /// The script that presents the fingerprint, run again after every
    /// navigation: rustwright cannot register a script to run in each new
    /// document, so a script run once would only cover the page it ran in.
    fingerprint_script: Option<String>,
}

impl PlaywrightSession {
    /// Launches Chromium with stealth-oriented launch arguments.
    ///
    /// The browser is launched in headed mode by default so that human-like
    /// behavior is preserved. Use [`PlaywrightSession::launch_headless`] for
    /// headless execution.
    pub async fn launch() -> Result<Self, SeleniumBaseError> {
        Self::launch_with_config(&BrowserConfig::default().with_headless(false)).await
    }

    /// Launches Chromium in headless mode with stealth arguments.
    pub async fn launch_headless() -> Result<Self, SeleniumBaseError> {
        Self::launch_with_config(&BrowserConfig::default().with_headless(true)).await
    }

    /// Launches Chromium with a [`Fingerprint`] profile.
    pub async fn launch_with_fingerprint(fp: &Fingerprint) -> Result<Self, SeleniumBaseError> {
        let config = BrowserConfig::default()
            .with_headless(false)
            .with_fingerprint(fp.clone());
        Self::launch_with_config(&config).await
    }

    /// Launches Chromium headless with a [`Fingerprint`] profile.
    pub async fn launch_headless_with_fingerprint(
        fp: &Fingerprint,
    ) -> Result<Self, SeleniumBaseError> {
        let config = BrowserConfig::default()
            .with_headless(true)
            .with_fingerprint(fp.clone());
        Self::launch_with_config(&config).await
    }

    /// Launches Chromium as `config` describes.
    ///
    /// These settings are used: `headless`, `browser_binary_path`,
    /// `user_data_dir`, `proxy` (with its user name and password, if it has
    /// any), `user_agent`, `locale`, `extra_args` and `fingerprint`. The
    /// others belong to the WebDriver path and are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the proxy is not a
    /// proxy address, [`SeleniumBaseError::BrowserLaunch`] if Chromium cannot
    /// be started and [`SeleniumBaseError::Stealth`] if the fingerprint cannot
    /// be applied to the first page.
    pub async fn launch_with_config(config: &BrowserConfig) -> Result<Self, SeleniumBaseError> {
        let options = launch_options(config)?;
        let fingerprint_script = config
            .fingerprint
            .as_ref()
            .map(crate::stealth::evasions::bootstrap_script);

        let (browser, page) = tokio::task::spawn_blocking(move || {
            let browser = chromium().launch(options).map_err(|e| {
                let err =
                    SeleniumBaseError::browser_launch("chromium (playwright)", format!("{e}"));
                err.log_in_context("PlaywrightSession::new");
                err
            })?;
            let page = browser
                .new_page()
                .map_err(|e| SeleniumBaseError::playwright(format!("new page failed: {e}")))?;
            Ok::<_, SeleniumBaseError>((browser, page))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;

        let session = Self {
            browser,
            page,
            fingerprint_script,
        };
        if let Err(error) = session.apply_fingerprint().await {
            // Do not leave a browser running that is not the one that was asked for.
            let _ = session.close().await;
            return Err(error);
        }
        Ok(session)
    }

    /// Presents the fingerprint in the active page, if the session has one.
    async fn apply_fingerprint(&self) -> Result<(), SeleniumBaseError> {
        let Some(script) = self.fingerprint_script.as_deref() else {
            return Ok(());
        };
        self.evaluate(script).await.map(drop).map_err(|error| {
            SeleniumBaseError::stealth(format!("could not apply the fingerprint: {error}"))
        })
    }

    /// Creates a new page in the browser and activates it.
    pub async fn new_page(&mut self) -> Result<(), SeleniumBaseError> {
        let browser = self.browser.clone();
        self.page = tokio::task::spawn_blocking(move || {
            browser
                .new_page()
                .map_err(|e| SeleniumBaseError::playwright(format!("new page failed: {e}")))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        self.apply_fingerprint().await
    }

    /// Navigates the active page to `url`.
    ///
    /// If the session has a fingerprint, it is presented in the new page
    /// before this returns.
    pub async fn goto(&self, url: &str) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let url = url.to_owned();
        tokio::task::spawn_blocking(move || {
            page.goto(&url, rustwright::GotoOptions::default())
                .map_err(|e| SeleniumBaseError::navigation(url, format!("{e}")))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        self.apply_fingerprint().await
    }

    /// Clicks the element selected by `selector`.
    pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let selector = selector.to_owned();
        tokio::task::spawn_blocking(move || {
            page.click(&selector, ActionOptions::default())
                .map_err(|e| classify_playwright_element_error(&selector, "click", &e))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        Ok(())
    }

    /// Clears and types `text` into the element selected by `selector`.
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let selector = selector.to_owned();
        let text = text.to_owned();
        tokio::task::spawn_blocking(move || {
            page.fill(&selector, &text, ActionOptions::default())
                .map_err(|e| classify_playwright_element_error(&selector, "fill", &e))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        Ok(())
    }

    /// Returns the visible text of the element selected by `selector`.
    pub async fn get_text(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        let page = self.page.clone();
        let selector = selector.to_owned();
        let text = tokio::task::spawn_blocking(move || {
            page.text_content(&selector, ActionOptions::default())
                .map_err(|e| classify_playwright_element_error(&selector, "text_content", &e))
                .map(|text| text.unwrap_or_default())
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        Ok(text)
    }

    /// Evaluates `expression` in the active page and returns the JSON result.
    pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        let page = self.page.clone();
        let expression = expression.to_owned();
        let value = tokio::task::spawn_blocking(move || {
            page.evaluate(&expression, None, ActionOptions::default())
                .map_err(|e| SeleniumBaseError::playwright(format!("evaluate failed: {e}")))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        Ok(value)
    }

    /// Saves a screenshot of the active page to `path`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || {
            let options = ScreenshotOptions::default().path(path.to_string_lossy().to_string());
            page.screenshot(options)
                .map_err(|e| SeleniumBaseError::screenshot(format!("{e}")))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        Ok(())
    }

    /// Closes the browser and cleans up the session.
    pub async fn close(&self) -> Result<(), SeleniumBaseError> {
        let browser = self.browser.clone();
        tokio::task::spawn_blocking(move || {
            browser
                .close()
                .map_err(|e| SeleniumBaseError::playwright(format!("close failed: {e}")))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        Ok(())
    }
}

/// The Chrome switches for `config`: the stealth ones, then what the
/// fingerprint asks for, then the caller's own, so a later switch wins.
fn chrome_args(config: &BrowserConfig) -> Vec<String> {
    let mut args: Vec<String> = STEALTH_ARGS.iter().map(|arg| (*arg).to_owned()).collect();
    if SANDBOX_OFF_BY_DEFAULT {
        args.push("--disable-setuid-sandbox".to_owned());
    }
    if let Some(fingerprint) = config.fingerprint.as_ref() {
        args.extend(
            crate::stealth::evasions::launch_args(fingerprint)
                .into_iter()
                .filter(|arg| SANDBOX_OFF_BY_DEFAULT || !is_sandbox_off(arg)),
        );
    }
    if let Some(user_agent) = config.user_agent.as_deref() {
        args.push(format!("--user-agent={user_agent}"));
    }
    if let Some(locale) = config.locale.as_deref() {
        args.push(format!("--lang={locale}"));
    }
    args.extend(config.extra_args.iter().cloned());
    args
}

/// Whether `arg` turns the Chrome sandbox off.
fn is_sandbox_off(arg: &str) -> bool {
    matches!(arg, "--no-sandbox" | "--disable-setuid-sandbox")
}

/// The rustwright launch options for `config`.
fn launch_options(config: &BrowserConfig) -> Result<LaunchOptions, SeleniumBaseError> {
    let proxy = config.proxy.as_deref().map(proxy_options).transpose()?;
    Ok(LaunchOptions {
        headless: Some(config.headless),
        executable_path: config
            .browser_binary_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        args: chrome_args(config),
        user_data_dir: config.user_data_dir.clone(),
        chromium_sandbox: !SANDBOX_OFF_BY_DEFAULT,
        proxy,
        ..LaunchOptions::default()
    })
}

/// The proxy for `spec`, written `[scheme://][user:password@]host:port`.
fn proxy_options(spec: &str) -> Result<ProxyOptions, SeleniumBaseError> {
    let proxy = Proxy::parse(spec)?;
    let mut options = ProxyOptions::new(proxy.server());
    if let Some((username, password)) = proxy.credentials() {
        options.username = Some(username.to_owned());
        options.password = Some(password.to_owned());
    }
    Ok(options)
}

/// Maps a rustwright failure of an action on `selector` to the error callers
/// of every driver see.
fn classify_playwright_element_error(
    selector: &str,
    action: &str,
    error: &RwError,
) -> SeleniumBaseError {
    match error {
        // rustwright waits for the element, and says so when nothing matched.
        RwError::ActionTimeout(timeout) if timeout.to_string().ends_with("no element matched") => {
            SeleniumBaseError::element_not_found(selector)
        }
        RwError::ActionTimeout(_) | RwError::Timeout(_) => {
            SeleniumBaseError::wait_timeout(format!("{action} {selector}"), None)
        }
        // rustwright reports an element that is not in the page, whether it
        // never was or went away while it waited, as detached.
        RwError::Actionability(ActionabilityError::Detached) => {
            SeleniumBaseError::element_not_found(selector)
        }
        RwError::Actionability(reason) => {
            SeleniumBaseError::element_not_interactable(selector, reason.to_string())
        }
        RwError::ActionFailure(failure) => {
            classify_playwright_element_error(selector, action, failure.cause())
        }
        RwError::Closed
        | RwError::Disconnected
        | RwError::TargetClosed(_)
        | RwError::PageCrashed => SeleniumBaseError::browser_disconnected(error.to_string()),
        other => SeleniumBaseError::playwright(format!("{action} on '{selector}' failed: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::Fingerprint;

    fn config() -> BrowserConfig {
        BrowserConfig::default()
    }

    #[test]
    fn a_session_does_not_switch_off_web_security_or_site_isolation() {
        let args = chrome_args(&config());
        for forbidden in [
            "--disable-web-security",
            "--disable-site-isolation-trials",
            "--disable-features=IsolateOrigins,site-per-process",
        ] {
            assert!(
                !args.iter().any(|arg| arg == forbidden),
                "{forbidden}: {args:?}"
            );
        }
        assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_owned()));
    }

    #[test]
    fn the_sandbox_is_off_on_linux_only() {
        let options = launch_options(&config()).expect("valid options");
        let sandbox_flags = |args: &[String]| args.iter().filter(|arg| is_sandbox_off(arg)).count();
        assert_eq!(options.chromium_sandbox, !cfg!(target_os = "linux"));
        // A fingerprint's own switches obey the same rule.
        let with_fingerprint = config().with_fingerprint(Fingerprint::windows_desktop());
        let args = chrome_args(&with_fingerprint);
        if cfg!(target_os = "linux") {
            assert!(sandbox_flags(&args) > 0);
        } else {
            assert_eq!(sandbox_flags(&args), 0, "{args:?}");
        }
    }

    #[test]
    fn the_config_reaches_the_launch_options() {
        let config = config()
            .with_headless(true)
            .with_browser_binary_path("/opt/chromium/chrome")
            .with_user_data_dir("/tmp/profile")
            .with_user_agent("Agent/1.0")
            .with_locale("de-DE")
            .with_extra_args(["--mute-audio"]);
        let options = launch_options(&config).expect("valid options");
        assert_eq!(options.headless, Some(true));
        assert_eq!(
            options.executable_path.as_deref(),
            Some("/opt/chromium/chrome")
        );
        assert_eq!(options.user_data_dir.as_deref(), Some("/tmp/profile"));
        for expected in ["--user-agent=Agent/1.0", "--lang=de-DE", "--mute-audio"] {
            assert!(options.args.iter().any(|arg| arg == expected), "{expected}");
        }
        // The caller's own switches come last, so they win.
        assert_eq!(
            options.args.last().map(String::as_str),
            Some("--mute-audio")
        );
    }

    #[test]
    fn an_explicit_user_agent_beats_the_fingerprints() {
        let fingerprint = Fingerprint::windows_desktop();
        let config = config()
            .with_fingerprint(fingerprint)
            .with_user_agent("Chosen/1.0");
        let args = chrome_args(&config);
        let last_agent = args
            .iter()
            .rposition(|arg| arg.starts_with("--user-agent="));
        assert_eq!(
            last_agent.map(|index| args[index].as_str()),
            Some("--user-agent=Chosen/1.0")
        );
    }

    #[test]
    fn proxy_credentials_are_handed_to_the_engine_not_the_command_line() {
        let options =
            launch_options(&config().with_proxy("http://alice:s3cret@proxy.example:8080"))
                .expect("valid options");
        let proxy = options.proxy.expect("a proxy");
        assert_eq!(proxy.server, "http://proxy.example:8080");
        assert_eq!(proxy.username.as_deref(), Some("alice"));
        assert_eq!(proxy.password.as_deref(), Some("s3cret"));
        assert!(!options.args.iter().any(|arg| arg.contains("s3cret")));
    }

    #[test]
    fn a_proxy_without_credentials_has_none() {
        let proxy = proxy_options("socks5://127.0.0.1:1080").expect("a proxy");
        assert_eq!(proxy.server, "socks5://127.0.0.1:1080");
        assert!(proxy.username.is_none() && proxy.password.is_none());
    }

    #[test]
    fn an_empty_proxy_is_refused_before_a_browser_starts() {
        let error = launch_options(&config().with_proxy("http://")).expect_err("no server");
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
    }

    #[test]
    fn engine_failures_map_to_the_errors_other_drivers_return() {
        let classify = |error: RwError| classify_playwright_element_error("#go", "click", &error);
        assert!(matches!(
            classify(RwError::Timeout(30_000)),
            SeleniumBaseError::WaitTimeout(_)
        ));
        assert!(matches!(
            classify(RwError::Actionability(ActionabilityError::NotVisible)),
            SeleniumBaseError::ElementNotInteractable { .. }
        ));
        // An element that is not in the page is "not found", not "not interactable".
        assert!(matches!(
            classify(RwError::Actionability(ActionabilityError::Detached)),
            SeleniumBaseError::ElementNotFound { .. }
        ));
        assert!(matches!(
            classify(RwError::Closed),
            SeleniumBaseError::BrowserDisconnected { .. }
        ));
        assert!(matches!(
            classify(RwError::PageCrashed),
            SeleniumBaseError::BrowserDisconnected { .. }
        ));
        assert!(matches!(
            classify(RwError::Message("something else".to_owned())),
            SeleniumBaseError::Playwright(_)
        ));
    }
}
