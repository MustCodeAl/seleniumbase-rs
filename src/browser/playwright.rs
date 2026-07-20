//! Playwright-backed browser session for a stealthy automation mode.
//!
//! This module is only available when the `playwright` feature is enabled. It
//! wraps the [`playwright-rs`](https://github.com/padamson/playwright-rust)
//! crate to launch Chromium with anti-detection arguments and exposes a small,
//! synchronous-feeling API that mirrors the WebDriver-backed [`BrowserSession`]
//! where practical.
//!
//! # Driver installation
//!
//! `playwright-rs` downloads the Playwright driver during its build script.
//! Make sure the build host can reach the Playwright CDN, or pre-install the
//! driver with `npx playwright install` and point `PLAYWRIGHT_DRIVER_PATH` at
//! it if the crate supports it.
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

use playwright_rs::api::LaunchOptions;
use playwright_rs::protocol::{Browser, Page};
use playwright_rs::Playwright;
use serde_json::Value;

use crate::error::SeleniumBaseError;

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

/// A browser session backed by the `playwright-rs` bindings.
///
/// Holds ownership of the Playwright runtime, browser, and active page. Call
/// [`PlaywrightSession::launch`] to create a session, then use the helper
/// methods to navigate and interact with pages.
pub struct PlaywrightSession {
    #[allow(dead_code)]
    playwright: Playwright,
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
        Self::launch_with_headless(false).await
    }

    /// Launches Chromium in headless mode with stealth arguments.
    pub async fn launch_headless() -> Result<Self, SeleniumBaseError> {
        Self::launch_with_headless(true).await
    }

    async fn launch_with_headless(headless: bool) -> Result<Self, SeleniumBaseError> {
        let playwright = Playwright::launch()
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("init failed: {e}")))?;

        let args: Vec<String> = STEALTH_ARGS.iter().map(|s| (*s).to_owned()).collect();
        let options = LaunchOptions::default().headless(headless).args(args);

        let browser = playwright
            .chromium()
            .launch_with_options(options)
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("launch failed: {e}")))?;

        let page = browser
            .new_page()
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("new page failed: {e}")))?;

        Ok(Self {
            playwright,
            browser,
            page,
        })
    }

    /// Creates a new page in the browser and activates it.
    pub async fn new_page(&mut self) -> Result<(), SeleniumBaseError> {
        self.page = self
            .browser
            .new_page()
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("new page failed: {e}")))?;
        Ok(())
    }

    /// Navigates the active page to `url`.
    pub async fn goto(&self, url: &str) -> Result<(), SeleniumBaseError> {
        self.page
            .goto(url, None)
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("goto failed: {e}")))?;
        Ok(())
    }

    /// Clicks the element selected by `selector`.
    pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let locator = self.page.locator(selector).await;
        locator
            .click(None)
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("click failed: {e}")))?;
        Ok(())
    }

    /// Clears and types `text` into the element selected by `selector`.
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let locator = self.page.locator(selector).await;
        locator
            .fill(text, None)
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("type_text failed: {e}")))?;
        Ok(())
    }

    /// Returns the visible text of the element selected by `selector`.
    pub async fn get_text(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        let locator = self.page.locator(selector).await;
        let text = locator
            .text_content()
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("get_text failed: {e}")))?;
        Ok(text.unwrap_or_default())
    }

    /// Evaluates `expression` in the active page and returns the JSON result.
    pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        let value: Value = self
            .page
            .evaluate::<(), Value>(expression, None)
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("evaluate failed: {e}")))?;
        Ok(value)
    }

    /// Saves a screenshot of the active page to `path`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        self.page
            .screenshot_to_file(path, None)
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("screenshot failed: {e}")))?;
        Ok(())
    }

    /// Closes the browser and cleans up the session.
    pub async fn close(&self) -> Result<(), SeleniumBaseError> {
        self.browser
            .close()
            .await
            .map_err(|e| SeleniumBaseError::Playwright(format!("close failed: {e}")))?;
        Ok(())
    }
}
