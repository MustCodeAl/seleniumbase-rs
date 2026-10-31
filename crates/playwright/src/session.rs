//! A Playwright-style session attached to a browser that is already running.
//!
//! [`PlaywrightSession`] holds [`rustwright`]'s handles to one browser and one
//! active page, and offers the usual page actions as `async` methods: rustwright's calls
//! block, so each one runs on Tokio's blocking pool. Everything else rustwright can do is
//! reached through [`PlaywrightSession::browser`], [`PlaywrightSession::page`] and
//! [`PlaywrightSession::with_page`].
//!
//! The session never starts a browser. It attaches to one, over the DevTools Protocol
//! (`connect_over_cdp`), which is how Python SeleniumBase's *Stealthy Playwright Mode*
//! reaches its stealthy Chrome: the stealth comes from the browser that was launched, and
//! the attached client inherits it.

use std::fmt;
use std::path::Path;

use rustwright::{
    chromium, ActionOptions, ActionabilityError, Browser, ConnectOptions, Error as RwError,
    GotoOptions, Page, ScreenshotOptions,
};
use seleniumbase_rs::json::Value;
use seleniumbase_rs::sb_cdp;
use seleniumbase_rs::tokio;
use seleniumbase_rs::SeleniumBaseError;

/// A [`rustwright`] client attached to a running Chromium, with one active page.
///
/// Create it with [`attach`](Self::attach) for a browser started through
/// [`sb_cdp`], or with [`connect_over_cdp`](Self::connect_over_cdp) for any other
/// DevTools endpoint, such as the one a chromedriver session reports.
///
/// The attached page keeps whatever fingerprint, proxy and launch switches the browser was
/// started with; the session changes none of them. [`close`](Self::close) only detaches, so a
/// browser that another handle owns is never shut down from here.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
/// use seleniumbase_rs_playwright::PlaywrightSession;
///
/// # async fn demo() -> seleniumbase_rs::Result<()> {
/// let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
/// let session = PlaywrightSession::attach(&browser).await?;
/// session.goto("https://example.com").await?;
/// println!("{}", session.get_text("h1").await?);
/// session.close().await?;
/// browser.close().await?;
/// # Ok(())
/// # }
/// ```
pub struct PlaywrightSession {
    browser: Browser,
    page: Page,
}

// rustwright's handles are not `Debug`, so the session reports what it can tell.
impl fmt::Debug for PlaywrightSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaywrightSession")
            .field("connected", &self.is_connected())
            .finish_non_exhaustive()
    }
}

impl PlaywrightSession {
    /// Attaches rustwright to a browser launched through [`sb_cdp`], using the HTTP address
    /// of its DevTools endpoint ([`sb_cdp::Browser::http_url`]).
    ///
    /// The session works on the first page the browser has open, or on a new one if it has
    /// none. Open the page you want with the CDP API first, as Python SeleniumBase does for
    /// an authenticated proxy, and the session starts on that page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Playwright`] if nothing answers at the endpoint or the
    /// browser cannot be controlled.
    pub async fn attach(browser: &sb_cdp::Browser) -> Result<Self, SeleniumBaseError> {
        Self::connect_over_cdp(&browser.http_url()).await
    }

    /// Attaches to a Chromium that is already running, through its DevTools endpoint: an
    /// `http://host:port` address or a `ws://` URL. A chromedriver session's
    /// `debuggerAddress` is a bare `host:port`; put `http://` in front of it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Playwright`] if nothing answers at `endpoint` or the
    /// browser cannot be controlled.
    pub async fn connect_over_cdp(endpoint: &str) -> Result<Self, SeleniumBaseError> {
        Self::connect_with(ConnectOptions::new(endpoint)).await
    }

    /// Attaches as `options` say, for a timeout or request headers that
    /// [`connect_over_cdp`](Self::connect_over_cdp) does not set.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Playwright`] if the attach fails or times out.
    pub async fn connect_with(options: ConnectOptions) -> Result<Self, SeleniumBaseError> {
        let (browser, page) = blocking(move || {
            let browser = chromium()
                .connect_over_cdp(options)
                .map_err(|e| SeleniumBaseError::playwright(format!("attach failed: {e}")))?;
            let existing = browser
                .pages()
                .map_err(|e| SeleniumBaseError::playwright(format!("listing pages failed: {e}")))?
                .into_iter()
                .next();
            let page = match existing {
                Some(page) => page,
                None => browser
                    .new_page()
                    .map_err(|e| SeleniumBaseError::playwright(format!("new page failed: {e}")))?,
            };
            Ok((browser, page))
        })
        .await?;
        Ok(Self { browser, page })
    }

    /// The rustwright browser this session is attached to.
    #[must_use]
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    /// The active rustwright page. It is a cheap handle: clone it to move it elsewhere.
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// Whether the connection to the browser is still open.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.browser.is_connected()
    }

    /// Runs `work` with the active page on Tokio's blocking pool and returns its result.
    ///
    /// rustwright's page calls block the thread they run on; this is the way to use any of
    /// them from async code without stalling the runtime.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Playwright`] with the text of the rustwright error if
    /// `work` fails.
    pub async fn with_page<T, F>(&self, work: F) -> Result<T, SeleniumBaseError>
    where
        F: FnOnce(Page) -> Result<T, RwError> + Send + 'static,
        T: Send + 'static,
    {
        let page = self.page.clone();
        blocking(move || {
            work(page).map_err(|e| SeleniumBaseError::playwright(format!("page call failed: {e}")))
        })
        .await
    }

    /// Opens a new page in the browser and makes it the active one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Playwright`] if the page cannot be created.
    pub async fn new_page(&mut self) -> Result<(), SeleniumBaseError> {
        let browser = self.browser.clone();
        self.page = blocking(move || {
            browser
                .new_page()
                .map_err(|e| SeleniumBaseError::playwright(format!("new page failed: {e}")))
        })
        .await?;
        Ok(())
    }

    /// Navigates the active page to `url`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Navigation`] if the page cannot be loaded.
    pub async fn goto(&self, url: &str) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let url = url.to_owned();
        blocking(move || {
            page.goto(&url, GotoOptions::default())
                .map_err(|e| SeleniumBaseError::navigation(url, format!("{e}")))
                .map(drop)
        })
        .await
    }

    /// Clicks the element selected by `selector`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches,
    /// [`SeleniumBaseError::ElementNotInteractable`] if the element cannot be clicked and
    /// [`SeleniumBaseError::WaitTimeout`] if it does not become clickable in time.
    pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let selector = selector.to_owned();
        blocking(move || {
            page.click(&selector, ActionOptions::default())
                .map_err(|e| classify_element_error(&selector, "click", &e))
        })
        .await
    }

    /// Clears and types `text` into the element selected by `selector`.
    ///
    /// # Errors
    ///
    /// The errors of [`click`](Self::click).
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let selector = selector.to_owned();
        let text = text.to_owned();
        blocking(move || {
            page.fill(&selector, &text, ActionOptions::default())
                .map_err(|e| classify_element_error(&selector, "fill", &e))
        })
        .await
    }

    /// Returns the text content of the element selected by `selector`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if nothing matches and
    /// [`SeleniumBaseError::WaitTimeout`] if the element does not appear in time.
    pub async fn get_text(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        let page = self.page.clone();
        let selector = selector.to_owned();
        blocking(move || {
            page.text_content(&selector, ActionOptions::default())
                .map_err(|e| classify_element_error(&selector, "text_content", &e))
                .map(Option::unwrap_or_default)
        })
        .await
    }

    /// Evaluates `expression` in the active page and returns the JSON result.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Playwright`] if the script throws.
    pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        let page = self.page.clone();
        let expression = expression.to_owned();
        blocking(move || {
            page.evaluate(&expression, None, ActionOptions::default())
                .map_err(|e| SeleniumBaseError::playwright(format!("evaluate failed: {e}")))
        })
        .await
    }

    /// Saves a screenshot of the active page to `path`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Screenshot`] if the page cannot be captured or the file
    /// cannot be written.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        let page = self.page.clone();
        let path = path.to_string_lossy().into_owned();
        blocking(move || {
            page.screenshot(ScreenshotOptions::default().path(path))
                .map_err(|e| SeleniumBaseError::screenshot(format!("{e}")))
                .map(drop)
        })
        .await
    }

    /// Closes this client's connection. The browser it attached to keeps running: it is
    /// shut down by whoever started it (for a browser from [`sb_cdp`], its own `close`).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Playwright`] if the connection does not close cleanly.
    pub async fn close(&self) -> Result<(), SeleniumBaseError> {
        let browser = self.browser.clone();
        blocking(move || {
            browser
                .close()
                .map_err(|e| SeleniumBaseError::playwright(format!("close failed: {e}")))
        })
        .await
    }
}

/// Runs `work` on Tokio's blocking pool, where rustwright's blocking calls belong.
///
/// A task that panics or is cancelled is reported as a [`SeleniumBaseError::Playwright`].
async fn blocking<T, F>(work: F) -> Result<T, SeleniumBaseError>
where
    F: FnOnce() -> Result<T, SeleniumBaseError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))?
}

/// Maps a rustwright failure of an action on `selector` to the error callers of every
/// driver see.
fn classify_element_error(selector: &str, action: &str, error: &RwError) -> SeleniumBaseError {
    match error {
        // rustwright waits for the element, and says so when nothing matched.
        RwError::ActionTimeout(timeout) if timeout.to_string().ends_with("no element matched") => {
            SeleniumBaseError::element_not_found(selector)
        }
        RwError::ActionTimeout(_) | RwError::Timeout(_) => {
            SeleniumBaseError::wait_timeout(format!("{action} {selector}"), None)
        }
        // rustwright reports an element that is not in the page, whether it never was or
        // went away while it waited, as detached.
        RwError::Actionability(ActionabilityError::Detached) => {
            SeleniumBaseError::element_not_found(selector)
        }
        RwError::Actionability(reason) => {
            SeleniumBaseError::element_not_interactable(selector, reason.to_string())
        }
        RwError::ActionFailure(failure) => {
            classify_element_error(selector, action, failure.cause())
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
    use std::time::Duration;

    use super::*;

    #[test]
    fn engine_failures_map_to_the_errors_other_drivers_return() {
        let classify = |error: RwError| classify_element_error("#go", "click", &error);
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

    #[tokio::test]
    async fn an_endpoint_nothing_answers_at_is_reported_as_a_playwright_error() {
        // Port 1 is reserved and nothing listens there, so the attach is refused at once;
        // the short timeout keeps a slow refusal from holding the test.
        let options = ConnectOptions::new("http://127.0.0.1:1").timeout(Duration::from_millis(500));

        let error = PlaywrightSession::connect_with(options)
            .await
            .expect_err("there is no browser to attach to");

        assert!(
            matches!(&error, SeleniumBaseError::Playwright(text) if text.starts_with("attach failed")),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn blocking_work_that_panics_is_an_error_not_a_hang() {
        let outcome: Result<(), SeleniumBaseError> =
            blocking(|| panic!("rustwright blew up")).await;

        assert!(
            matches!(&outcome, Err(SeleniumBaseError::Playwright(text)) if text.contains("blocking task failed")),
            "{outcome:?}"
        );
    }
}
