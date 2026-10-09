//! Isolated browser contexts: separate cookies, storage and cache in one Chrome.
//!
//! A [`BrowserContext`] is what an incognito window is: tabs opened in it share
//! nothing with tabs in the default profile or in any other context, and
//! disposing it throws everything away. It is far cheaper than a second Chrome
//! process, which is why [`BrowserPool`](super::BrowserPool) hands one out per
//! lease.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::Browser;
//!
//! # async fn demo(browser: Browser) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let context = browser.new_context().await?;
//! let page = context.new_page(Some("https://example.com")).await?;
//! // ... cookies set here are invisible to every other tab ...
//! context.dispose().await?;
//! # Ok(())
//! # }
//! ```

use std::sync::{Arc, Mutex};

use serde_json::json;

use super::sync::locked;
use super::{Browser, Page};
use crate::error::SeleniumBaseError;

/// A set of tabs with their own cookies, storage and cache.
///
/// Cloning gives another handle to the same context. Dropping every handle
/// does not dispose the context; call [`dispose`](Self::dispose).
#[derive(Debug, Clone)]
pub struct BrowserContext {
    browser: Browser,
    id: Arc<str>,
    /// The tabs opened through this handle, so disposing can forget them.
    pages: Arc<Mutex<Vec<String>>>,
}

impl Browser {
    /// Creates an isolated [`BrowserContext`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    pub async fn new_context(&self) -> Result<BrowserContext, SeleniumBaseError> {
        let response = self
            .execute("Target.createBrowserContext", json!({}))
            .await?;
        let id = response["browserContextId"].as_str().ok_or_else(|| {
            SeleniumBaseError::cdp_driver(
                "Target.createBrowserContext returned no browserContextId",
            )
        })?;
        Ok(BrowserContext {
            browser: self.clone(),
            id: id.into(),
            pages: Arc::new(Mutex::new(Vec::new())),
        })
    }
}

impl BrowserContext {
    /// The DevTools id of the context.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The browser the context lives in.
    #[must_use]
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    /// Opens a tab in this context, optionally at `url`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the tab cannot be created,
    /// for example because the context was disposed.
    pub async fn new_page(&self, url: Option<impl AsRef<str>>) -> Result<Page, SeleniumBaseError> {
        // A fresh context has no window yet, and Chrome refuses to open a tab
        // without one, so the first tab brings its own; later tabs join it.
        let first = locked(&self.pages).is_empty();
        let page = self
            .browser
            .open_target(url.as_ref().map(AsRef::as_ref), first, Some(&self.id))
            .await?;
        locked(&self.pages).push(page.id().to_owned());
        Ok(page)
    }

    /// Closes every tab in the context and discards its cookies and storage.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses, for
    /// example because the context is already gone.
    pub async fn dispose(&self) -> Result<(), SeleniumBaseError> {
        let outcome = self
            .browser
            .execute(
                "Target.disposeBrowserContext",
                json!({ "browserContextId": &*self.id }),
            )
            .await
            .map(drop);
        for id in locked(&self.pages).drain(..) {
            self.browser.forget_page(&id);
        }
        outcome
    }
}
