//! The handles a [`PlaywrightSidecar`](crate::PlaywrightSidecar) can attach beside.
//!
//! Both of this project's engines are targets: the pure CDP [`Page`] and the WebDriver
//! [`BaseCase`]. A target tells the sidecar where its browser's DevTools endpoint is and
//! which tab it is on, so that the rustwright page is the page the target drives.

use std::future::Future;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{DevToolsEndpoint, Page};
use seleniumbase_rs::{BaseCase, SeleniumBaseError};

/// What a [`PlaywrightSidecar`](crate::PlaywrightSidecar) needs from its target.
///
/// Public only because the sealed trait names it; nothing outside the crate can reach it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// The DevTools endpoint of the target's browser.
    pub endpoint: DevToolsEndpoint,
    /// Which of the browser's tabs the target is on.
    pub hints: TabHints,
    /// How long the target waits for things by default; the attach waits as long.
    pub timeout: Duration,
}

/// What a target knows about its own tab. A hint a target cannot give is `None`.
///
/// Public only because the sealed trait names it; nothing outside the crate can reach it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabHints {
    /// The tab's DevTools target id: `Page::id`, or a chromedriver window handle.
    pub target_id: Option<String>,
    /// The URL the tab is on.
    pub url: Option<String>,
}

mod sealed {
    use std::future::Future;

    use super::Attachment;
    use seleniumbase_rs::SeleniumBaseError;

    /// The one method a target answers. It is `Sync` because the future holds `&self`
    /// across its awaits, which keeps `PlaywrightSidecar::attach` `Send`.
    pub trait Sealed: Sync {
        /// The endpoint, the tab hints and the timeout of this target.
        fn attachment(&self) -> impl Future<Output = Result<Attachment, SeleniumBaseError>> + Send;
    }
}

/// A handle of this project that is on one tab of a Chromium: an [`sb_cdp::Page`] or a
/// [`BaseCase`](seleniumbase_rs::BaseCase). [`PlaywrightSidecar::attach`] takes either.
///
/// The trait is sealed: it is implemented for those two types and cannot be implemented
/// elsewhere, because what a target must tell the sidecar is the crate's business.
///
/// [`sb_cdp::Page`]: seleniumbase_rs::sb_cdp::Page
/// [`PlaywrightSidecar::attach`]: crate::PlaywrightSidecar::attach
pub trait PlaywrightTarget: sealed::Sealed {}

impl PlaywrightTarget for Page {}
impl PlaywrightTarget for BaseCase {}

/// Asks `target` for its attachment. The sidecar's way into the sealed trait.
pub(crate) async fn attachment_of<T: PlaywrightTarget + ?Sized>(
    target: &T,
) -> Result<Attachment, SeleniumBaseError> {
    sealed::Sealed::attachment(target).await
}

impl sealed::Sealed for Page {
    fn attachment(&self) -> impl Future<Output = Result<Attachment, SeleniumBaseError>> + Send {
        async move {
            Ok(Attachment {
                endpoint: self.browser().devtools_endpoint(),
                hints: TabHints {
                    target_id: Some(self.id().to_owned()),
                    // A tab that cannot say its URL is still found by its id.
                    url: self.url().await.ok(),
                },
                timeout: self.timeout(),
            })
        }
    }
}

impl sealed::Sealed for BaseCase {
    fn attachment(&self) -> impl Future<Output = Result<Attachment, SeleniumBaseError>> + Send {
        async move {
            let endpoint = self.devtools_endpoint().await?;
            Ok(Attachment {
                endpoint,
                hints: TabHints {
                    // chromedriver's window handle may be the target id; when the driver
                    // answers none (a direct CDP attachment) the URL decides.
                    target_id: self.get_current_window_handle().await.ok(),
                    url: self.get_current_url().await.ok(),
                },
                timeout: Duration::from_secs(self.timeout_secs()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use seleniumbase_rs::json::json;
    use seleniumbase_rs::sb_cdp::Browser;
    use seleniumbase_rs::tokio;
    use seleniumbase_rs::BrowserConfig;

    use super::*;

    #[tokio::test]
    async fn a_page_gives_its_browsers_endpoint_its_id_its_url_and_its_timeout() {
        let (browser, mock) = Browser::new_mocked();
        mock.reply(
            "Runtime.evaluate",
            json!({ "result": { "value": "http://example.test/page" } }),
        );
        let page = browser
            .default_page()
            .await
            .expect("the mock has a first tab")
            .with_timeout(Duration::from_secs(7));

        let attachment = attachment_of(&page).await.expect("a page can be attached");

        // The mock's endpoint is what `Browser::http_url` says of it.
        assert_eq!(attachment.endpoint.http_url(), "http://127.0.0.1:0");
        assert_eq!(attachment.endpoint, browser.devtools_endpoint());
        assert_eq!(attachment.hints.target_id.as_deref(), Some(page.id()));
        assert_eq!(
            attachment.hints.url.as_deref(),
            Some("http://example.test/page")
        );
        assert_eq!(attachment.timeout, Duration::from_secs(7));
    }

    #[tokio::test]
    async fn a_case_without_a_session_cannot_be_attached() {
        let sb = BaseCase::without_session(BrowserConfig::default());

        let error = attachment_of(&sb)
            .await
            .expect_err("a case with no browser has no endpoint");

        assert!(
            matches!(error, SeleniumBaseError::SessionNotStarted),
            "{error:?}"
        );
    }
}
