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
//! let page = context.new_page_at("https://example.com").await?;
//! // ... cookies set here are invisible to every other tab ...
//! context.dispose().await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;
#[cfg(feature = "plugins")]
use std::sync::atomic::{AtomicBool, Ordering};
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

/// How plugins see the options of a context (feature `plugins`).
#[cfg(feature = "plugins")]
impl ContextOptions {
    /// The settings `before_context` shows the plugins: the proxy as its server
    /// only, so a password in the options is not shown to them.
    pub(super) fn settings(&self) -> seleniumbase_rs_plugin::ContextSettings {
        let mut settings = seleniumbase_rs_plugin::ContextSettings::new();
        settings.proxy = self.proxy.as_ref().map(|proxy| proxy.server().to_owned());
        settings.bypass.clone_from(&self.bypass);
        settings
    }

    /// These options with the settings the plugins returned. A proxy they left
    /// as it was keeps its password.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] when a plugin set a proxy that
    /// cannot be parsed.
    pub(super) fn with_settings(
        mut self,
        settings: seleniumbase_rs_plugin::ContextSettings,
    ) -> Result<Self, SeleniumBaseError> {
        if settings.proxy.as_deref() != self.proxy.as_ref().map(Proxy::server) {
            self.proxy = settings
                .proxy
                .as_deref()
                .map(Proxy::parse)
                .transpose()
                .map_err(|error| {
                    // The proxy itself is not shown: it may carry a password.
                    SeleniumBaseError::invalid_config(format!(
                        "the proxy a plugin set for a context is not valid ({error})"
                    ))
                })?;
        }
        self.bypass = settings.bypass;
        Ok(self)
    }

    /// The options for a context a plugin asked a browser handle to make.
    ///
    /// # Errors
    ///
    /// As [`with_settings`](Self::with_settings).
    pub(super) fn from_settings(
        settings: seleniumbase_rs_plugin::ContextSettings,
    ) -> Result<Self, SeleniumBaseError> {
        Self::default().with_settings(settings)
    }
}

/// The proxy a context routes its tabs through, and the hosts that skip it.
///
/// Kept by the browser so that plain HTTP requests made on behalf of a tab can
/// take the same route. Its `Debug` output never shows the credentials.
#[derive(Debug, Clone)]
pub struct ContextRoute {
    /// The proxy the context's tabs go through.
    pub proxy: Proxy,
    /// The hosts that skip the proxy.
    pub bypass: Vec<String>,
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
    /// Set once the plugins have been told the context is gone, so that
    /// every handle to it tells them once.
    #[cfg(feature = "plugins")]
    gone: Arc<AtomicBool>,
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
    /// With plugins (feature `plugins`), each plugin that holds the `contexts`
    /// capability may change the proxy first (`before_context`) and is told
    /// when the context exists (`on_context_created`); when one of them
    /// stops it, the context is disposed of and the error is returned.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses, and
    /// [`SeleniumBaseError::Plugin`] when a plugin refuses.
    pub async fn new_context_with(
        &self,
        options: ContextOptions,
    ) -> Result<BrowserContext, SeleniumBaseError> {
        #[cfg(feature = "plugins")]
        let options = super::plugin_hooks::before_context(self, options).await?;
        let context = self.create_context(options).await?;
        #[cfg(feature = "plugins")]
        super::plugin_hooks::on_context_created(self, &context).await?;
        Ok(context)
    }

    /// Creates an isolated [`BrowserContext`] set up as `options` describe,
    /// for example one whose tabs all go through their own proxy.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    async fn create_context(
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
        if let Some(proxy) = &options.proxy {
            locked(&self.inner().context_routes).insert(
                id.to_owned(),
                ContextRoute {
                    proxy: proxy.clone(),
                    bypass: options.bypass.clone(),
                },
            );
        }
        Ok(BrowserContext {
            browser: self.clone(),
            id: id.into(),
            credentials: options
                .proxy
                .as_ref()
                .and_then(Proxy::credentials)
                .map(|(user, pass)| (user.to_owned(), pass.to_owned())),
            pages: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "plugins")]
            gone: Arc::new(AtomicBool::new(false)),
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

    /// Opens a blank tab in this context.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the tab cannot be created,
    /// for example because the context was disposed.
    pub async fn new_page(&self) -> Result<Page, SeleniumBaseError> {
        self.open_page(None).await
    }

    /// Opens a tab in this context and navigates it to `url`, which is opened
    /// over HTTPS if it has no scheme.
    ///
    /// # Errors
    ///
    /// As [`new_page`](Self::new_page), and [`SeleniumBaseError::Navigation`]
    /// if the page cannot be loaded.
    pub async fn new_page_at(&self, url: impl AsRef<str>) -> Result<Page, SeleniumBaseError> {
        self.open_page(Some(url.as_ref())).await
    }

    async fn open_page(&self, url: Option<&str>) -> Result<Page, SeleniumBaseError> {
        // A fresh context has no window yet, and Chrome refuses to open a tab
        // without one, so the first tab brings its own; later tabs join it.
        let first = locked(&self.pages).is_empty();
        let page = self
            .browser
            .open_target(
                url,
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
        locked(&self.browser.inner().context_routes).remove(&*self.id);
        // Told once, when the browser confirms the context is gone.
        #[cfg(feature = "plugins")]
        if outcome.is_ok() && !self.gone.swap(true, Ordering::SeqCst) {
            super::plugin_hooks::on_context_closed(&self.browser, &self.id).await;
        }

        outcome
    }
}

impl Browser {
    /// The own proxy of the context with this id, if it was created with one.
    #[must_use]
    pub fn context_route(&self, id: &str) -> Option<ContextRoute> {
        locked(&self.inner().context_routes).get(id).cloned()
    }
}

#[cfg(test)]
mod route_tests {
    use super::*;

    fn route_of(browser: &Browser, context: &BrowserContext) -> Option<ContextRoute> {
        browser.context_route(context.id())
    }

    #[tokio::test]
    async fn a_context_with_a_proxy_has_a_route_until_it_is_disposed() {
        let (browser, mock) = Browser::new_mocked();
        mock.reply(
            "Target.createBrowserContext",
            serde_json::json!({ "browserContextId": "C1" }),
        );
        let options = ContextOptions::new()
            .proxy(Proxy::parse("alice:s3cret@proxy.example:8080").expect("a valid proxy"))
            .bypass("*.internal.example");

        let context = browser
            .new_context_with(options)
            .await
            .expect("the context");

        let route = route_of(&browser, &context).expect("the context has a route");
        assert_eq!(route.proxy.server(), "proxy.example:8080");
        assert!(route.proxy.has_credentials());
        assert_eq!(route.bypass, ["*.internal.example"]);
        let shown = format!("{route:?}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
        context.dispose().await.expect("the context is disposed");
        assert!(
            route_of(&browser, &context).is_none(),
            "the route goes with the context"
        );
    }

    #[tokio::test]
    async fn a_context_without_a_proxy_has_no_route() {
        let (browser, mock) = Browser::new_mocked();
        mock.reply(
            "Target.createBrowserContext",
            serde_json::json!({ "browserContextId": "C1" }),
        );

        let context = browser.new_context().await.expect("the context");

        assert!(route_of(&browser, &context).is_none());
    }

    #[tokio::test]
    async fn routes_are_kept_apart_by_context() {
        let (browser, mock) = Browser::new_mocked();
        let made = std::sync::atomic::AtomicUsize::new(0);
        mock.on("Target.createBrowserContext", move |_| {
            let id = made.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            Ok(serde_json::json!({ "browserContextId": format!("C{id}") }))
        });
        let one = ContextOptions::new().proxy(Proxy::parse("one.example:1").expect("valid"));
        let two = ContextOptions::new().proxy(Proxy::parse("two.example:2").expect("valid"));

        let first = browser.new_context_with(one).await.expect("the first");
        let second = browser.new_context_with(two).await.expect("the second");

        assert_eq!(
            route_of(&browser, &first).expect("route").proxy.server(),
            "one.example:1"
        );
        assert_eq!(
            route_of(&browser, &second).expect("route").proxy.server(),
            "two.example:2"
        );
    }
}
