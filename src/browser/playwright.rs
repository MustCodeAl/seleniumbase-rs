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
//! Chromium CDN, or set a local Chromium executable with
//! `BrowserConfig::browser_binary_path`.
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

use std::path::Path;

use rustwright::{chromium, ActionOptions, Browser, LaunchOptions, Page, ScreenshotOptions};
use serde_json::Value;

use crate::error::SeleniumBaseError;
use crate::stealth::fingerprint::Fingerprint;

const STEALTH_ARGS: &[&str] = &[
    "--disable-blink-features=AutomationControlled",
    "--disable-infobars",
    "--disable-dev-shm-usage",
    "--disable-setuid-sandbox",
    "--no-sandbox",
    "--window-size=1920,1080",
    "--start-maximized",
    "--disable-web-security",
    "--disable-features=IsolateOrigins,site-per-process",
    "--disable-site-isolation-trials",
];

/// A browser session backed by the native Rust `rustwright` CDP engine.
///
/// Holds ownership of the browser and active page. Call
/// [`PlaywrightSession::launch`] to create a session, then use the helper
/// methods to navigate and interact with pages.
pub struct PlaywrightSession {
    browser: Browser,
    page: Page,
}

impl PlaywrightSession {
    /// Launches Chromium with stealth-oriented launch arguments.
    ///
    /// The browser is launched in headed mode by default so that human-like
    /// behavior is preserved. Use [`PlaywrightSession::launch_headless`] for
    /// headless execution.
    pub async fn launch() -> Result<Self, SeleniumBaseError> {
        Self::launch_with_options(false, None).await
    }

    /// Launches Chromium in headless mode with stealth arguments.
    pub async fn launch_headless() -> Result<Self, SeleniumBaseError> {
        Self::launch_with_options(true, None).await
    }

    /// Launches Chromium with a [`Fingerprint`] profile.
    pub async fn launch_with_fingerprint(fp: &Fingerprint) -> Result<Self, SeleniumBaseError> {
        Self::launch_with_options(false, Some(fp.clone())).await
    }

    /// Launches Chromium headless with a [`Fingerprint`] profile.
    pub async fn launch_headless_with_fingerprint(
        fp: &Fingerprint,
    ) -> Result<Self, SeleniumBaseError> {
        Self::launch_with_options(true, Some(fp.clone())).await
    }

    async fn launch_with_options(
        headless: bool,
        fingerprint: Option<Fingerprint>,
    ) -> Result<Self, SeleniumBaseError> {
        let mut args: Vec<String> = STEALTH_ARGS.iter().map(|s| (*s).to_owned()).collect();
        if let Some(fp) = fingerprint.as_ref() {
            args.extend(crate::stealth::evasions::launch_args(fp));
        }

        let options = LaunchOptions {
            headless: Some(headless),
            args,
            ..LaunchOptions::default()
        };

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

        if let Some(fp) = fingerprint {
            let script = crate::stealth::evasions::bootstrap_script(&fp);
            let _ = Self::evaluate_on_page(&page, &script).await;
        }

        Ok(Self { browser, page })
    }

    async fn evaluate_on_page(page: &Page, expression: &str) -> Result<Value, SeleniumBaseError> {
        let page = page.clone();
        let expression = expression.to_owned();
        let value = tokio::task::spawn_blocking(move || {
            page.evaluate(&expression, None, ActionOptions::default())
                .map_err(|e| SeleniumBaseError::playwright(format!("evaluate failed: {e}")))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        Ok(value)
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
        Ok(())
    }

    /// Navigates the active page to `url`.
    pub async fn goto(&self, url: &str) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let url = url.to_owned();
        tokio::task::spawn_blocking(move || {
            page.goto(&url, rustwright::GotoOptions::default())
                .map_err(|e| SeleniumBaseError::navigation(url, format!("{e}")))
        })
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))??;
        Ok(())
    }

    /// Clicks the element selected by `selector`.
    pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let selector = selector.to_owned();
        tokio::task::spawn_blocking(move || {
            page.click(&selector, ActionOptions::default())
                .map_err(|e| classify_playwright_element_error(&selector, "click", e.to_string()))
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
                .map_err(|e| classify_playwright_element_error(&selector, "fill", e.to_string()))
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
                .map_err(|e| {
                    classify_playwright_element_error(&selector, "text_content", e.to_string())
                })
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

fn classify_playwright_element_error(
    selector: &str,
    action: &str,
    msg: String,
) -> SeleniumBaseError {
    let lower = msg.to_lowercase();
    if lower.contains("timeout") || lower.contains("waiting") && lower.contains("failed") {
        SeleniumBaseError::wait_timeout(format!("{action} {selector}"), None)
    } else if lower.contains("not found") || lower.contains("no node") || lower.contains("selector")
    {
        SeleniumBaseError::element_not_found(selector)
    } else if lower.contains("not visible")
        || lower.contains("interactable")
        || lower.contains("hidden")
    {
        SeleniumBaseError::element_not_interactable(selector, msg)
    } else {
        SeleniumBaseError::playwright(format!("{action} on '{selector}' failed: {msg}"))
    }
}
