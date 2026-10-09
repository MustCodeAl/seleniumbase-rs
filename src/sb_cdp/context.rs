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

use std::fmt;
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::browser::ContextTarget;
use super::proxy_auth::Credentials;
use super::sync::locked;
use super::{Browser, Page, Proxy};
use crate::error::SeleniumBaseError;

/// How a [`BrowserContext`] is set up.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::{ContextOptions, Proxy};
///
/// # fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let options = ContextOptions::new()
///     .proxy(Proxy::parse("alice:s3cret@proxy.example.com:8080")?)
///     .bypass("*.internal.example.com");
/// assert!(!format!("{options:?}").contains("s3cret"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextOptions {
    proxy: Option<Proxy>,
    bypass: Vec<String>,
}

impl ContextOptions {
    /// Default options: no proxy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Routes every tab of the context through `proxy`, whatever the rest of
    /// the browser uses. A proxy with a username and password is answered
    /// automatically, for this context's tabs only.
    #[must_use]
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Hosts that skip the proxy, such as `*.internal.example.com`. Repeat for
    /// several.
    #[must_use]
    pub fn bypass(mut self, host: impl Into<String>) -> Self {
        self.bypass.push(host.into());
        self
    }
}

/// A set of tabs with their own cookies, storage and cache.
///
/// Cloning gives another handle to the same context. Dropping every handle
/// does not dispose the context; call [`dispose`](Self::dispose).
#[derive(Clone)]
pub struct BrowserContext {
    browser: Browser,
    id: Arc<str>,
    /// The password of the context's proxy, if it has one.
    credentials: Option<Credentials>,
    /// The tabs opened through this handle, so disposing can forget them.
    pages: Arc<Mutex<Vec<String>>>,
}

impl fmt::Debug for BrowserContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never the proxy password.
        f.debug_struct("BrowserContext")
            .field("id", &self.id)
            .field(
                "proxy_credentials",
                &self.credentials.as_ref().map(|_| "<redacted>"),
            )
            .finish_non_exhaustive()
    }
}

impl Browser {
    /// Creates an isolated [`BrowserContext`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    pub async fn new_context(&self) -> Result<BrowserContext, SeleniumBaseError> {
        self.new_context_with(ContextOptions::default()).await
    }

    /// Creates an isolated [`BrowserContext`] set up as `options` describe,
    /// for example one whose tabs all go through their own proxy.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    pub async fn new_context_with(
        &self,
        options: ContextOptions,
    ) -> Result<BrowserContext, SeleniumBaseError> {
        let mut params = json!({});
        if let Some(proxy) = &options.proxy {
            params["proxyServer"] = json!(proxy.server());
            if !options.bypass.is_empty() {
                params["proxyBypassList"] = json!(options.bypass.join(";"));
            }
        }
        let response = self.execute("Target.createBrowserContext", params).await?;
        let id = response["browserContextId"].as_str().ok_or_else(|| {
            SeleniumBaseError::cdp_driver(
                "Target.createBrowserContext returned no browserContextId",
            )
        })?;
        Ok(BrowserContext {
            browser: self.clone(),
            id: id.into(),
            credentials: options
                .proxy
                .as_ref()
                .and_then(Proxy::credentials)
                .map(|(user, pass)| (user.to_owned(), pass.to_owned())),
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
            .open_target(
                url.as_ref().map(AsRef::as_ref),
                first,
                Some(ContextTarget {
                    id: &self.id,
                    credentials: self.credentials.as_ref(),
                }),
            )
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
