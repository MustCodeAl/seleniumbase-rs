//! Finding elements through shadow roots, for the WebDriver engine.
//!
//! A selector such as `my-app::shadow .form::shadow input` names an element
//! that sits inside two shadow roots. WebDriver's find commands never cross a
//! shadow boundary, so [`find`] and [`find_all`] cut the selector at each
//! `::shadow` (see [`shadow_chain`](crate::utils::shadow::shadow_chain)),
//! look for the first piece in the document, and look for each of the others
//! in the shadow root of the element the piece before it found. That is what
//! Python SeleniumBase does (`element.shadow_root`, then
//! `shadow_root.find_element`), and like Python it enters closed roots too,
//! because the browser driver hands them out.
//!
//! Every lookup of the session goes through here, so every method that takes a
//! selector reads the combinator: clicking, typing, reading text and
//! attributes, and the waits and assertions that are built on them. A
//! selector without the combinator is one ordinary `find`.

use std::sync::Arc;

use http::Method;
use thirtyfour::common::command::FormatRequestData;
use thirtyfour::common::command::Selector as WireSelector;
use thirtyfour::common::types::SessionId;
use thirtyfour::error::{WebDriverError, WebDriverErrorInner};
use thirtyfour::prelude::{By, WebDriver, WebElement};
use thirtyfour::RequestData;

use crate::error::SeleniumBaseError;
use crate::utils::selectors::{Selector, SelectorBuf};
use crate::utils::shadow::{is_shadow_selector, shadow_chain};

/// What the wire calls a CSS locator.
const CSS: &str = "css selector";

/// The pieces of `by` if it is a CSS selector that crosses shadow roots, and
/// `None` for any other locator.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidSelector`] if it crosses a shadow root but
/// cannot be cut, for instance because it ends on one.
pub(crate) fn pieces(by: &By) -> Result<Option<Vec<String>>, SeleniumBaseError> {
    let wire: WireSelector = by.clone().into();
    if &*wire.name != CSS || !is_shadow_selector(&wire.query) {
        return Ok(None);
    }
    shadow_chain(&wire.query).map(Some)
}

/// W3C WebDriver's Get Element Shadow Root, which thirtyfour has no command
/// for: `GET /session/{id}/element/{element}/shadow`.
#[derive(Debug)]
struct GetShadowRoot(String);

impl FormatRequestData for GetShadowRoot {
    fn format_request(&self, session_id: &SessionId) -> RequestData {
        RequestData::new(
            Method::GET,
            format!("session/{session_id}/element/{}/shadow", self.0),
        )
    }
}

/// The shadow root of `host`, as the browser driver gives it. That is how
/// Python SeleniumBase reaches it (`element.shadow_root`), and unlike a script
/// that reads `host.shadowRoot` it also gives a closed root. A driver that
/// does not know the command gets the script, which is what thirtyfour does.
async fn shadow_root_of(
    driver: &WebDriver,
    host: &WebElement,
) -> Result<WebElement, WebDriverError> {
    let asked = async {
        let reply = driver
            .cmd(GetShadowRoot(host.element_id().to_string()))
            .await?;
        WebElement::from_json(reply.value_json()?, Arc::clone(driver))
    };
    match asked.await {
        Ok(root) => Ok(root),
        Err(_) => host.get_shadow_root().await,
    }
}

/// Whether the driver said that nothing matches.
fn is_missing(error: &WebDriverError) -> bool {
    matches!(error.as_inner(), WebDriverErrorInner::NoSuchElement(_))
}

/// How far a walk down the pieces of a selector got.
enum Walk<'a> {
    /// The shadow root that holds the last piece, and that piece.
    Reached(WebElement, &'a str),
    /// A host before the last piece is not in the page, or has no shadow root;
    /// the selector up to that host.
    Short(String),
}

/// The shadow root that holds the last piece of `pieces`, and that piece.
///
/// `pieces` has at least two pieces: the last one is looked for in the root
/// and the others lead to the host of that root.
async fn last_root<'a>(
    driver: &WebDriver,
    pieces: &'a [String],
) -> Result<Walk<'a>, SeleniumBaseError> {
    let empty = || SeleniumBaseError::InvalidSelector("empty shadow selector".to_owned());
    let (last, hosts) = pieces.split_last().ok_or_else(empty)?;
    let (first, inner) = hosts.split_first().ok_or_else(empty)?;
    let mut so_far = first.clone();
    let mut host = match driver.find(Selector::auto(first).to_by()?).await {
        Ok(element) => element,
        Err(error) if is_missing(&error) => return Ok(Walk::Short(so_far)),
        Err(other) => return Err(other.into()),
    };
    for piece in inner {
        let Ok(root) = shadow_root_of(driver, &host).await else {
            return Ok(Walk::Short(so_far));
        };
        so_far = format!("{so_far}::shadow {piece}");
        host = match root.find(By::Css(piece)).await {
            Ok(element) => element,
            Err(error) if is_missing(&error) => return Ok(Walk::Short(so_far)),
            Err(other) => return Err(other.into()),
        };
    }
    Ok(match shadow_root_of(driver, &host).await {
        Ok(root) => Walk::Reached(root, last),
        Err(_) => Walk::Short(so_far),
    })
}

/// The WebDriver locator that looks `selector` up: the one conversion from
/// this crate's selector to a `thirtyfour` one, which the session and an
/// [`Element`](crate::Element) look elements up with.
///
/// A CSS selector that crosses a shadow root stays whole, for [`find`] and
/// [`find_all`] to cut; one that cannot be cut is refused here, before anything
/// waits. The kind was decided when the string became a [`SelectorBuf`], so a
/// CSS selector is taken as CSS here, and `:contains(..)` is not read again.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidSelector`] if the text is empty or blank, or if
/// a CSS selector crosses a shadow root but cannot be cut.
pub(crate) fn by_of(selector: &SelectorBuf) -> Result<By, SeleniumBaseError> {
    let filled = |text: &str| !text.trim().is_empty();
    match selector {
        SelectorBuf::Css(css) if is_shadow_selector(css) => {
            shadow_chain(css)?;
            Ok(By::Css(css.clone()))
        }
        SelectorBuf::Css(css) if filled(css) => Ok(By::Css(css.clone())),
        SelectorBuf::XPath(xpath) if filled(xpath) => Ok(By::XPath(xpath.clone())),
        SelectorBuf::LinkText(text) if filled(text) => Ok(By::LinkText(text.clone())),
        SelectorBuf::PartialLinkText(text) if filled(text) => Ok(By::PartialLinkText(text.clone())),
        _ => Err(SeleniumBaseError::InvalidSelector(
            "selector value cannot be empty".to_owned(),
        )),
    }
}

/// `selector` as a [`SelectorBuf`], once it is known that [`by_of`] accepts
/// it: for the waits, which look it up again and again and must refuse a
/// selector that can never match at once, not after the timeout.
///
/// # Errors
///
/// As [`by_of`].
pub(crate) fn checked(selector: impl Into<SelectorBuf>) -> Result<SelectorBuf, SeleniumBaseError> {
    let selector = selector.into();
    by_of(&selector)?;
    Ok(selector)
}

/// The first element `by` names: with a selector that crosses shadow roots,
/// the first match in the innermost root.
///
/// # Errors
///
/// An error if a host has no shadow root, if nothing matches, or if the
/// selector cannot be cut (see [`pieces`]).
pub(crate) async fn find(driver: &WebDriver, by: By) -> Result<WebElement, SeleniumBaseError> {
    let Some(pieces) = pieces(&by)? else {
        return Ok(driver.find(by).await?);
    };
    match last_root(driver, &pieces).await? {
        Walk::Reached(root, last) => Ok(root.find(By::Css(last)).await?),
        Walk::Short(so_far) => Err(SeleniumBaseError::element_not_found(format!(
            "{so_far} (not in the page, or without a shadow root)"
        ))),
    }
}

/// Every element `by` names: with a selector that crosses shadow roots, all
/// the matches in the innermost root (the hosts before it are the first
/// match of each piece).
///
/// # Errors
///
/// An error if a host has no shadow root, or if the selector cannot be cut
/// (see [`pieces`]). Nothing matching is an empty list.
pub(crate) async fn find_all(
    driver: &WebDriver,
    by: By,
) -> Result<Vec<WebElement>, SeleniumBaseError> {
    let Some(pieces) = pieces(&by)? else {
        return Ok(driver.find_all(by).await?);
    };
    match last_root(driver, &pieces).await? {
        Walk::Reached(root, last) => Ok(root.find_all(By::Css(last)).await?),
        Walk::Short(_) => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_css_locator_that_crosses_a_root_is_cut() {
        let cut = pieces(&By::Css("app ::shadow .form::shadow input")).unwrap();
        assert_eq!(cut.unwrap(), ["app", ".form", "input"]);

        assert!(pieces(&By::Css("div > a")).unwrap().is_none());
        assert!(pieces(&By::XPath("//a/parent::shadow-host"))
            .unwrap()
            .is_none());
        assert!(pieces(&By::LinkText("a ::shadow b")).unwrap().is_none());
        assert!(pieces(&By::PartialLinkText("a ::shadow b"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_locator_that_cannot_be_cut_is_an_error() {
        let error = pieces(&By::Css("app ::shadow")).unwrap_err().to_string();
        assert!(error.contains("cannot end on a shadow root"), "{error}");
    }

    /// The `Debug` text of the locator `selector` becomes.
    fn locator(selector: SelectorBuf) -> String {
        format!("{:?}", by_of(&selector).unwrap())
    }

    #[test]
    fn a_selector_becomes_the_locator_of_its_kind() {
        assert_eq!(
            locator(SelectorBuf::css("#a")),
            format!("{:?}", By::Css("#a"))
        );
        assert_eq!(
            locator(SelectorBuf::xpath("//a")),
            format!("{:?}", By::XPath("//a"))
        );
        assert_eq!(
            locator(SelectorBuf::link_text("Go")),
            format!("{:?}", By::LinkText("Go"))
        );
        assert_eq!(
            locator(SelectorBuf::partial_link_text("G")),
            format!("{:?}", By::PartialLinkText("G"))
        );
        // The kind was decided when the string became a selector: a CSS
        // selector is looked up as CSS, and `:contains(..)` is not read again.
        assert_eq!(
            locator(SelectorBuf::css("a:contains('x')")),
            format!("{:?}", By::Css("a:contains('x')"))
        );
        assert_eq!(
            locator(SelectorBuf::from("app::shadow a")),
            format!("{:?}", By::Css("app::shadow a"))
        );
    }

    #[test]
    fn a_selector_that_can_never_match_is_refused_before_anything_is_looked_up() {
        for selector in [
            SelectorBuf::css(""),
            SelectorBuf::xpath(" "),
            SelectorBuf::link_text(""),
            SelectorBuf::partial_link_text("\t"),
            SelectorBuf::css("app ::shadow"),
        ] {
            assert!(
                matches!(by_of(&selector), Err(SeleniumBaseError::InvalidSelector(_))),
                "{selector:?}"
            );
            assert!(checked(selector.clone()).is_err(), "{selector:?}");
        }
        assert_eq!(checked("#a").unwrap(), SelectorBuf::css("#a"));
    }
}
