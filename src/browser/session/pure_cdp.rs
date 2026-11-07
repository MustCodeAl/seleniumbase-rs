//! What a session does on a direct DevTools attachment (`BrowserConfig::with_cdp_url`):
//! the methods that have a CDP form answer through the pure CDP engine's
//! [`Page`] instead of a WebDriver.
//!
//! The design, group by group, is `docs/design/basecase_over_cdp.md`.

use crate::browser::element::Element;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Locator, Page};
use crate::utils::selectors::SelectorBuf;

use super::BrowserSession;

impl BrowserSession {
    /// A session on `page`, a tab of a browser the pure CDP engine drives, as a
    /// direct attachment makes one: no WebDriver, so `BaseCase` over it runs
    /// through the pure CDP engine. For tests that script the browser
    /// ([`Browser::new_mocked`](crate::sb_cdp::Browser::new_mocked)).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn attach_page(page: Page) -> Self {
        let mut session = Self::disconnected();
        session.external_page = Some(page);
        session
    }

    /// Fails at once, as the first WebDriver command would, for a session with
    /// no browser to ask: one after [`quit`](Self::quit), or one a failed
    /// reconnect left. A direct CDP attachment has a browser; its page is
    /// the engine.
    pub(super) fn ensure_started(&self) -> Result<(), SeleniumBaseError> {
        if self.external_page.is_some() {
            return Ok(());
        }
        self.webdriver().map(|_| ())
    }

    /// The locator for `selector` on `page`.
    pub(super) fn cdp_locator(&self, page: &Page, selector: SelectorBuf) -> Locator {
        match self.frame_scope() {
            Some(frame) => frame.locator(selector),
            None => page.locator(selector),
        }
    }

    /// The frames entered so far, outermost first.
    fn entered_frames(&self) -> std::sync::MutexGuard<'_, Vec<Locator>> {
        // A panic while the list was held cannot leave it half-written: it is
        // only pushed to, popped and cleared.
        self.frames
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The locator of the frame the session is in, or `None` at the top
    /// document: what a search starts from.
    pub(super) fn frame_scope(&self) -> Option<Locator> {
        self.entered_frames().last().cloned()
    }

    /// Back at the top document.
    pub(super) fn leave_frames(&self) {
        self.entered_frames().clear();
    }

    /// Switches into the frame `element` is, as WebDriver's `switch_to.frame`
    /// does: the session's later searches, scripts and clicks happen inside it.
    ///
    /// # Errors
    ///
    /// The driver's error if the element is not a frame; on a direct CDP
    /// attachment, [`SeleniumBaseError::InvalidSelector`] for an element that
    /// is neither an `<iframe>` nor a `<frame>`.
    pub(crate) async fn enter_frame_element(
        &self,
        element: &Element,
    ) -> Result<(), SeleniumBaseError> {
        if let Some(raw) = element.as_webdriver_element() {
            return raw
                .clone()
                .enter_frame()
                .await
                .map_err(SeleniumBaseError::WebDriver);
        }
        let tag = element.tag().await?;
        let Some(locator) = element
            .as_locator()
            .filter(|_| tag == "iframe" || tag == "frame")
        else {
            return Err(SeleniumBaseError::InvalidSelector(format!(
                "'{element}' is a <{tag}>, not an <iframe> or a <frame>, so the session cannot \
                     switch into it"
            )));
        };
        self.entered_frames().push(locator.clone());
        Ok(())
    }

    /// Switches to the frame that holds the one the session is in.
    ///
    /// # Errors
    ///
    /// The driver's error, or [`SeleniumBaseError::SessionNotStarted`] after
    /// [`quit`](Self::quit).
    pub async fn switch_to_parent_frame(&self) -> Result<(), SeleniumBaseError> {
        if self.external_page.is_some() {
            self.entered_frames().pop();
            return Ok(());
        }
        self.webdriver()?.enter_parent_frame().await?;
        Ok(())
    }

    /// The first element `selector` matches on `page`, as of now.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::ElementNotFound`] if nothing matches,
    /// [`SeleniumBaseError::InvalidSelector`] if the selector cannot match.
    pub(super) async fn cdp_find(
        &self,
        page: &Page,
        selector: SelectorBuf,
    ) -> Result<Element, SeleniumBaseError> {
        let locator = self.cdp_locator(page, selector.clone());
        if locator.exists().await? {
            Ok(Element::cdp(locator, selector))
        } else {
            Err(SeleniumBaseError::element_not_found(selector.to_string()))
        }
    }

    /// Every element `selector` matches on `page`, as of now, each pinned to
    /// its position; none is an empty list.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidSelector`] if the selector cannot match.
    pub(super) async fn cdp_find_all(
        &self,
        page: &Page,
        selector: SelectorBuf,
    ) -> Result<Vec<Element>, SeleniumBaseError> {
        let locators = self.cdp_locator(page, selector.clone()).all().await?;
        Ok(locators
            .into_iter()
            .map(|locator| Element::cdp(locator, selector.clone()))
            .collect())
    }
}
