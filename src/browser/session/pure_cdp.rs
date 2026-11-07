//! What a session does on a direct DevTools attachment (`BrowserConfig::with_cdp_url`):
//! the methods that have a CDP form answer through the pure CDP engine's
//! [`Page`] instead of a WebDriver.
//!
//! The design, group by group, is `docs/design/basecase_over_cdp.md`.

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::browser::element::Element;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Locator, Page};
use crate::utils::selectors::SelectorBuf;

use super::BrowserSession;

/// The tab a direct CDP attachment drives.
///
/// It changes when the session switches tabs, which WebDriver's window switch
/// does through `&self`, so it sits behind a lock: handed out as a clone and
/// never held across an `.await`.
#[derive(Default)]
pub(super) struct PageSlot(Mutex<Option<Page>>);

impl PageSlot {
    fn locked(&self) -> MutexGuard<'_, Option<Page>> {
        // The slot is only read and replaced, so a panic while it was held
        // cannot have left it half-written.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The tab, if the session is a direct CDP attachment.
    pub(super) fn get(&self) -> Option<Page> {
        self.locked().clone()
    }

    /// Makes `page` the tab the session drives.
    pub(super) fn set(&self, page: Option<Page>) {
        *self.locked() = page;
    }

    /// Takes the tab away, leaving the session with none.
    pub(super) fn take(&self) -> Option<Page> {
        self.locked().take()
    }

    /// Whether the session is a direct CDP attachment.
    pub(super) fn is_some(&self) -> bool {
        self.locked().is_some()
    }
}

impl BrowserSession {
    /// A session on `page`, a tab of a browser the pure CDP engine drives, as a
    /// direct attachment makes one: no WebDriver, so `BaseCase` over it runs
    /// through the pure CDP engine. For tests that script the browser
    /// ([`Browser::new_mocked`](crate::sb_cdp::Browser::new_mocked)).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn attach_page(page: Page) -> Self {
        let session = Self::disconnected();
        session.external_page.set(Some(page));
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

    /// The handles of the open windows and tabs, in the order the browser lists
    /// them. On a direct CDP attachment a handle is the tab's target id.
    ///
    /// # Errors
    ///
    /// The engine's error, or [`SeleniumBaseError::SessionNotStarted`] after
    /// [`quit`](Self::quit).
    pub async fn window_handles(&self) -> Result<Vec<String>, SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            let tabs = page.browser().list_pages().await?;
            return Ok(tabs.into_iter().map(|tab| tab.id).collect());
        }
        let handles = self.webdriver()?.windows().await?;
        Ok(handles.iter().map(ToString::to_string).collect())
    }

    /// The handle of the window or tab the session is on.
    ///
    /// # Errors
    ///
    /// See [`window_handles`](Self::window_handles).
    pub async fn window_handle(&self) -> Result<String, SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return Ok(page.id().to_string());
        }
        Ok(self.webdriver()?.window().await?.to_string())
    }

    /// Makes `page` the tab a direct CDP attachment drives, brought to the
    /// front, at its top document.
    pub(crate) async fn switch_to_tab(&self, page: Page) -> Result<(), SeleniumBaseError> {
        page.bring_to_front().await?;
        self.external_page.set(Some(page));
        self.leave_frames();
        Ok(())
    }

    /// The position and size of the window, as `(x, y, width, height)`.
    ///
    /// # Errors
    ///
    /// The engine's error.
    pub async fn window_rect(&self) -> Result<(i64, i64, i64, i64), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            let bounds = page.window().bounds().await?;
            return Ok((bounds.x, bounds.y, bounds.width, bounds.height));
        }
        let rect = self.webdriver()?.get_window_rect().await?;
        Ok((rect.x, rect.y, rect.width, rect.height))
    }

    /// Moves and resizes the window.
    ///
    /// # Errors
    ///
    /// The engine's error.
    pub async fn set_window_rect(
        &self,
        x: i64,
        y: i64,
        width: u32,
        height: u32,
    ) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page
                .window()
                .set_bounds(x, y, i64::from(width), i64::from(height))
                .await;
        }
        self.webdriver()?
            .set_window_rect(x, y, width, height)
            .await?;
        Ok(())
    }

    /// Minimizes the window.
    ///
    /// # Errors
    ///
    /// The engine's error.
    pub async fn minimize_window(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page.window().minimize().await;
        }
        self.webdriver()?.minimize_window().await?;
        Ok(())
    }

    /// Makes the window fullscreen.
    ///
    /// # Errors
    ///
    /// The engine's error.
    pub async fn fullscreen_window(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page.window().fullscreen().await;
        }
        self.webdriver()?.fullscreen_window().await?;
        Ok(())
    }

    /// Closes the window or tab the session is on. The session has none after
    /// it, until it switches to another.
    ///
    /// # Errors
    ///
    /// The engine's error.
    pub async fn close_window(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page.close().await;
        }
        self.webdriver()?.close_window().await?;
        Ok(())
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
