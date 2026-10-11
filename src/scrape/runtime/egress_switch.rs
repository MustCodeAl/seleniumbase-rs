//! The egress switch trait and its `ContextProxySwitch` implementation, which changes egress by opening a new browser context.
//!
//! A browser context has its own cookies, storage and cache, and can have its own proxy, so a new
//! context behind a new proxy is a clean visitor from a new address without starting another
//! Chrome. That is also what an egress change needs: a clearance cookie belongs to the address that
//! earned it, so the cookies of the old egress must not travel to the new one.

use std::fmt;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::sb_cdp::{Browser, BrowserContext, ContextOptions, Page};
use crate::scrape::egress::route::EgressRef;
use crate::scrape::error::ScrapeError;

#[cfg(test)]
mod tests;

/// Changes where a browser's traffic leaves from.
///
/// An implementation gives back a tab that goes through the new egress and has none of the state
/// of the old one. The shipped implementation is [`ContextProxySwitch`].
#[async_trait]
pub trait EgressSwitch: Send + Sync {
    /// Opens a tab in `browser` that leaves through `egress`, or directly when `egress` is `None`.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when the browser refuses. The tab the caller held before is
    /// left as it was.
    async fn switch(
        &self,
        browser: &Browser,
        egress: Option<&EgressRef>,
    ) -> Result<Page, ScrapeError>;
}

/// An [`EgressSwitch`] that opens a new browser context with its own proxy and drops the one it
/// opened before.
///
/// The new context is made first, so a failure leaves the old one in place. A proxy with a user
/// name and password is answered for the tabs of its context only. The browser is not restarted.
#[derive(Default)]
pub struct ContextProxySwitch {
    current: Mutex<Option<BrowserContext>>,
}

impl ContextProxySwitch {
    /// A switch that has opened no context yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl fmt::Debug for ContextProxySwitch {
    /// Shows the context id only, which is not a secret; the proxy is never printed.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let open = self
            .current
            .try_lock()
            .ok()
            .and_then(|current| current.as_ref().map(|context| context.id().to_owned()));
        f.debug_struct("ContextProxySwitch")
            .field("context", &open)
            .finish()
    }
}

#[async_trait]
impl EgressSwitch for ContextProxySwitch {
    async fn switch(
        &self,
        browser: &Browser,
        egress: Option<&EgressRef>,
    ) -> Result<Page, ScrapeError> {
        let options = egress.map_or_else(ContextOptions::new, |egress| {
            ContextOptions::new().proxy(egress.proxy().clone())
        });
        let context = browser.new_context_with(options).await?;
        let page = match context.new_page(None::<&str>).await {
            Ok(page) => page,
            Err(error) => {
                dispose(&context).await;
                return Err(error.into());
            }
        };
        let previous = self.current.lock().await.replace(context);
        if let Some(previous) = previous {
            dispose(&previous).await;
        }
        Ok(page)
    }
}

/// Throws a context away. One that is already gone is no loss, so a failure is only logged.
async fn dispose(context: &BrowserContext) {
    if let Err(error) = context.dispose().await {
        tracing::debug!(%error, "a browser context could not be disposed");
    }
}
