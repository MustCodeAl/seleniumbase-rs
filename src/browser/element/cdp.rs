//! The pure CDP form of an [`Element`](super::Element): a [`Locator`] under the
//! name it was found by.
//!
//! A locator finds again on every use, so this form never goes stale; a match
//! of `find_all` is pinned to its position with [`Locator::nth`]. What the
//! locator already does (the waits for the element, the real mouse and key
//! events) is not done twice here: each method is the locator's own, under
//! the name `Element` has for it.

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::browser::session::DialogWatch;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Key, Locator, Rect, SelectBy};
use crate::utils::selectors::{Selector, CHAIN_SEPARATOR};

/// An element of a page the pure CDP engine drives.
#[derive(Clone)]
pub(super) struct CdpElement {
    locator: Locator,
    /// How the element was found, for errors, `Debug` and `Display`.
    selector: Selector,
    /// The dialogs of the element's tab, for the actions that can open one.
    dialogs: Option<Arc<DialogWatch>>,
}

impl fmt::Debug for CdpElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Element")
            .field("selector", &self.selector.to_string())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CdpElement {
    /// The selector the element was found by.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.selector.fmt(f)
    }
}

impl CdpElement {
    /// The element `locator` finds, named `selector`.
    pub(super) fn new(
        locator: Locator,
        selector: Selector,
        dialogs: Option<Arc<DialogWatch>>,
    ) -> Self {
        Self {
            locator,
            selector,
            dialogs,
        }
    }

    /// Runs `action`, a command that can open a dialog, and lets it go as soon
    /// as one opens: the page stalls inside the command until the dialog is
    /// answered (see [`DialogWatch`]).
    async fn guarded(
        &self,
        action: impl Future<Output = Result<(), SeleniumBaseError>>,
    ) -> Result<(), SeleniumBaseError> {
        match &self.dialogs {
            Some(dialogs) => dialogs.unless_dialog(action).await.map(drop),
            None => action.await,
        }
    }

    /// This element under `name`, for one found by a selector that no longer
    /// matches anything (a temporary marker).
    pub(super) fn named(self, name: String) -> Self {
        Self {
            selector: Selector::css(name),
            ..self
        }
    }

    /// The locator the element looks again with.
    pub(super) fn locator(&self) -> &Locator {
        &self.locator
    }

    /// Runs `script` with the element as `arguments[0]` and `args` after it, in
    /// the frame the element is in, and answers what it returns.
    pub(super) async fn run_script(
        &self,
        script: &str,
        args: &[Value],
    ) -> Result<Value, SeleniumBaseError> {
        let args = serde_json::to_string(args)?;
        self.locator
            .with_element(&format!(
                "return (function() {{ {script}\n}}).apply(null, [el, ...{args}]);"
            ))
            .await
    }

    /// What `body` (a script that returns a string) answers for the element.
    async fn string_of(&self, body: &str) -> Result<String, SeleniumBaseError> {
        let value = self.locator.with_element(body).await?;
        Ok(value.as_str().unwrap_or_default().to_owned())
    }

    pub(super) async fn text(&self) -> Result<String, SeleniumBaseError> {
        self.locator.text().await
    }

    /// The text the browser renders for the element.
    pub(super) async fn rendered_text(&self) -> Result<String, SeleniumBaseError> {
        self.string_of("return el.innerText;").await
    }

    pub(super) async fn html(&self) -> Result<String, SeleniumBaseError> {
        self.locator.html().await
    }

    pub(super) async fn tag(&self) -> Result<String, SeleniumBaseError> {
        self.string_of("return el.tagName.toLowerCase();").await
    }

    pub(super) async fn attribute(&self, name: &str) -> Result<Option<String>, SeleniumBaseError> {
        self.locator.attribute(name).await
    }

    pub(super) async fn property(&self, name: &str) -> Result<Option<String>, SeleniumBaseError> {
        let name = serde_json::to_string(name)?;
        let value = self
            .locator
            .with_element(&format!(
                "const v = el[{name}]; return v === undefined || v === null ? null : String(v);"
            ))
            .await?;
        Ok(value.as_str().map(str::to_owned))
    }

    pub(super) async fn bounding_box(&self) -> Result<Rect, SeleniumBaseError> {
        self.locator.bounding_box().await
    }

    pub(super) async fn screenshot(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        self.locator.screenshot().await
    }

    pub(super) async fn is_visible(&self) -> Result<bool, SeleniumBaseError> {
        self.locator.is_visible().await
    }

    pub(super) async fn is_enabled(&self) -> Result<bool, SeleniumBaseError> {
        self.locator.is_enabled().await
    }

    pub(super) async fn is_selected(&self) -> Result<bool, SeleniumBaseError> {
        self.locator.is_selected().await
    }

    pub(super) async fn click(&self) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.click()).await
    }

    pub(super) async fn double_click(&self) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.double_click()).await
    }

    pub(super) async fn right_click(&self) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.right_click()).await
    }

    pub(super) async fn hover(&self) -> Result<(), SeleniumBaseError> {
        self.locator.hover().await
    }

    pub(super) async fn clear(&self) -> Result<(), SeleniumBaseError> {
        self.locator.clear().await
    }

    pub(super) async fn send_keys(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.send_keys(text)).await
    }

    pub(super) async fn type_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.type_text(text)).await
    }

    pub(super) async fn press(&self, key: Key) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.press(key)).await
    }

    pub(super) async fn select_option(&self, by: SelectBy<'_>) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.select_option(by)).await
    }

    pub(super) async fn submit(&self) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.submit()).await
    }

    pub(super) async fn scroll_into_view(&self) -> Result<(), SeleniumBaseError> {
        self.locator.scroll_into_view().await
    }

    pub(super) async fn drag_to(&self, target: &CdpElement) -> Result<(), SeleniumBaseError> {
        self.guarded(self.locator.drag_to(&target.locator)).await
    }

    /// The name of what `selector` finds inside this element: the chain.
    fn below(&self, selector: &Selector) -> Selector {
        Selector::css(format!("{self}{CHAIN_SEPARATOR}{selector}"))
    }

    /// The element `locator` finds if it matches now, else the error that
    /// names `name`. It does not wait: a found element is found once.
    async fn found(&self, locator: Locator, name: Selector) -> Result<Self, SeleniumBaseError> {
        if locator.exists().await? {
            Ok(Self::new(locator, name, self.dialogs.clone()))
        } else {
            Err(SeleniumBaseError::element_not_found(name.to_string()))
        }
    }

    pub(super) async fn find(&self, selector: Selector) -> Result<Self, SeleniumBaseError> {
        let name = self.below(&selector);
        self.found(self.locator.locator(selector), name).await
    }

    pub(super) async fn find_all(
        &self,
        selector: Selector,
    ) -> Result<Vec<Self>, SeleniumBaseError> {
        let name = self.below(&selector);
        Ok(self
            .locator
            .locator(selector)
            .all()
            .await?
            .into_iter()
            .map(|locator| Self::new(locator, name.clone(), self.dialogs.clone()))
            .collect())
    }

    pub(super) async fn parent(&self) -> Result<Self, SeleniumBaseError> {
        let name = self.below(&Selector::xpath(".."));
        self.found(self.locator.parent(), name).await
    }
}
