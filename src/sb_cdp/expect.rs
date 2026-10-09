//! Assertions that wait: [`Locator::expect`] and [`Page::expect`].
//!
//! An expectation re-checks until it holds or the timeout passes, so it is safe
//! to write straight after an action that changes the page asynchronously. A
//! failure says what was expected and what was seen last.
//!
//! ```no_run
//! # use seleniumbase_rs::sb_cdp::Page;
//! # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! page.expect().to_contain_url("/dashboard").await?;
//! page.locator("h1").expect().to_have_text("Welcome back").await?;
//! page.locator(".spinner").expect().not().to_be_visible().await?;
//! # Ok(())
//! # }
//! ```

use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use super::locator::Locator;
use super::page::Page;
use super::sync::locked;
use super::types::State;
use crate::error::SeleniumBaseError;

/// Re-runs `observe` until its verdict (flipped when `negate`) is `true`.
///
/// `observe` returns whether the thing holds plus a description of what was
/// seen, which goes into the failure message.
async fn retry_until<F, Fut>(
    page: &Page,
    subject: &str,
    expectation: &str,
    negate: bool,
    timeout: Duration,
    mut observe: F,
) -> Result<(), SeleniumBaseError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(bool, String), SeleniumBaseError>>,
{
    // A mutex rather than a `RefCell`, so the future stays `Send`.
    let last_seen = Mutex::new(String::from("nothing yet"));
    let satisfied = page
        .poll(timeout, || {
            let last_seen = &last_seen;
            let next = observe();
            async move {
                let (holds, seen) = next.await?;
                *locked(last_seen) = seen;
                Ok(holds != negate)
            }
        })
        .await?;
    if satisfied {
        Ok(())
    } else {
        let not = if negate { "not " } else { "" };
        let last_seen = last_seen
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Err(SeleniumBaseError::assertion_failed(format!(
            "expected {subject} {not}{expectation}, but after {timeout:?} it was {last_seen}"
        )))
    }
}

/// An assertion about a [`Locator`].
#[derive(Debug, Clone)]
pub struct LocatorExpect {
    locator: Locator,
    negate: bool,
    timeout: Duration,
}

impl LocatorExpect {
    pub(crate) fn new(locator: Locator) -> Self {
        let timeout = locator.timeout();
        Self {
            locator,
            negate: false,
            timeout,
        }
    }

    /// Inverts the next assertion: `.not().to_be_visible()` waits for hidden.
    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "reads as a fluent modifier in `expect().not().to_be_visible()`, not as the `!` operator"
    )]
    pub fn not(mut self) -> Self {
        self.negate = !self.negate;
        self
    }

    /// Waits up to `timeout` instead of the locator's default.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    async fn check_state(&self, state: State, expectation: &str) -> Result<(), SeleniumBaseError> {
        retry_until(
            self.locator.page(),
            &self.locator.to_string(),
            expectation,
            self.negate,
            self.timeout,
            || async {
                let holds = self.locator.is_in_state(state).await?;
                let seen = if self.locator.is_in_state(State::Visible).await? {
                    "visible"
                } else if self.locator.is_in_state(State::Present).await? {
                    "present but hidden"
                } else {
                    "absent"
                };
                Ok((holds, seen.to_owned()))
            },
        )
        .await
    }

    /// Passes when a match is visible.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if it does not hold in time.
    pub async fn to_be_visible(&self) -> Result<(), SeleniumBaseError> {
        self.check_state(State::Visible, "to be visible").await
    }

    /// Passes when no match is visible, including when none exists.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if it does not hold in time.
    pub async fn to_be_hidden(&self) -> Result<(), SeleniumBaseError> {
        self.check_state(State::Hidden, "to be hidden").await
    }

    /// Passes when a match exists in the DOM, visible or not.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if it does not hold in time.
    pub async fn to_exist(&self) -> Result<(), SeleniumBaseError> {
        self.check_state(State::Present, "to exist").await
    }

    async fn check_text(
        &self,
        expectation: String,
        matches: impl Fn(&str) -> bool,
    ) -> Result<(), SeleniumBaseError> {
        retry_until(
            self.locator.page(),
            &self.locator.to_string(),
            &expectation,
            self.negate,
            self.timeout,
            || async {
                if !self.locator.exists().await? {
                    return Ok((false, "missing".to_owned()));
                }
                let text = self.locator.with_timeout(Duration::ZERO).text().await?;
                Ok((matches(&text), format!("{:?}", text.trim())))
            },
        )
        .await
    }

    /// Passes when the visible text, trimmed, equals `expected`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] showing the actual text.
    pub async fn to_have_text(&self, expected: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        let expected = expected.as_ref().trim().to_owned();
        self.check_text(format!("to have text {expected:?}"), |text| {
            text.trim() == expected
        })
        .await
    }

    /// Passes when the visible text contains `fragment`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] showing the actual text.
    pub async fn to_contain_text(
        &self,
        fragment: impl AsRef<str>,
    ) -> Result<(), SeleniumBaseError> {
        let fragment = fragment.as_ref().to_owned();
        self.check_text(format!("to contain text {fragment:?}"), |text| {
            text.contains(&fragment)
        })
        .await
    }

    /// Passes when attribute `name` equals `value`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] showing the actual value.
    pub async fn to_have_attribute(
        &self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<(), SeleniumBaseError> {
        let (name, value) = (name.as_ref().to_owned(), value.as_ref().to_owned());
        retry_until(
            self.locator.page(),
            &self.locator.to_string(),
            &format!("to have attribute {name}={value:?}"),
            self.negate,
            self.timeout,
            || async {
                if !self.locator.exists().await? {
                    return Ok((false, "missing".to_owned()));
                }
                let actual = self
                    .locator
                    .with_timeout(Duration::ZERO)
                    .attribute(&name)
                    .await?;
                Ok((
                    actual.as_deref() == Some(value.as_str()),
                    format!("{actual:?}"),
                ))
            },
        )
        .await
    }

    /// Passes when a checkbox or radio button is checked.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if it does not hold in time.
    pub async fn to_be_checked(&self) -> Result<(), SeleniumBaseError> {
        retry_until(
            self.locator.page(),
            &self.locator.to_string(),
            "to be checked",
            self.negate,
            self.timeout,
            || async {
                if !self.locator.exists().await? {
                    return Ok((false, "missing".to_owned()));
                }
                let checked = self
                    .locator
                    .with_timeout(Duration::ZERO)
                    .is_checked()
                    .await?;
                Ok((
                    checked,
                    if checked { "checked" } else { "unchecked" }.to_owned(),
                ))
            },
        )
        .await
    }

    /// Passes when exactly `expected` elements match.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] showing the actual count.
    pub async fn to_have_count(&self, expected: usize) -> Result<(), SeleniumBaseError> {
        retry_until(
            self.locator.page(),
            &self.locator.to_string(),
            &format!("to match {expected} elements"),
            self.negate,
            self.timeout,
            || async {
                let count = self.locator.count().await?;
                Ok((count == expected, format!("{count} elements")))
            },
        )
        .await
    }
}

/// An assertion about a [`Page`].
#[derive(Debug, Clone)]
pub struct PageExpect {
    page: Page,
    negate: bool,
    timeout: Duration,
}

impl PageExpect {
    pub(crate) fn new(page: Page) -> Self {
        let timeout = page.timeout();
        Self {
            page,
            negate: false,
            timeout,
        }
    }

    /// Inverts the next assertion.
    #[must_use]
    #[expect(
        clippy::should_implement_trait,
        reason = "reads as a fluent modifier in `expect().not().to_have_title(..)`, not as the `!` operator"
    )]
    pub fn not(mut self) -> Self {
        self.negate = !self.negate;
        self
    }

    /// Waits up to `timeout` instead of the page's default.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    async fn check(
        &self,
        subject: Subject,
        expectation: String,
        matches: impl Fn(&str) -> bool,
    ) -> Result<(), SeleniumBaseError> {
        retry_until(
            &self.page,
            subject.label(),
            &expectation,
            self.negate,
            self.timeout,
            || async {
                let actual = match subject {
                    Subject::Title => self.page.title().await?,
                    Subject::Url => self.page.url().await?,
                };
                Ok((matches(&actual), format!("{actual:?}")))
            },
        )
        .await
    }

    /// Passes when the title equals `title`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] showing the actual title.
    pub async fn to_have_title(&self, title: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        let title = title.as_ref().to_owned();
        self.check(Subject::Title, format!("to equal {title:?}"), |actual| {
            actual == title
        })
        .await
    }

    /// Passes when the title contains `fragment`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] showing the actual title.
    pub async fn to_contain_title(
        &self,
        fragment: impl AsRef<str>,
    ) -> Result<(), SeleniumBaseError> {
        let fragment = fragment.as_ref().to_owned();
        self.check(
            Subject::Title,
            format!("to contain {fragment:?}"),
            |actual| actual.contains(&fragment),
        )
        .await
    }

    /// Passes when the URL equals `url`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] showing the actual URL.
    pub async fn to_have_url(&self, url: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        let url = url.as_ref().to_owned();
        self.check(Subject::Url, format!("to equal {url:?}"), |actual| {
            actual == url
        })
        .await
    }

    /// Passes when the URL contains `fragment`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] showing the actual URL.
    pub async fn to_contain_url(&self, fragment: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        let fragment = fragment.as_ref().to_owned();
        self.check(Subject::Url, format!("to contain {fragment:?}"), |actual| {
            actual.contains(&fragment)
        })
        .await
    }
}

/// What a [`PageExpect`] reads.
#[derive(Debug, Clone, Copy)]
enum Subject {
    Title,
    Url,
}

impl Subject {
    fn label(self) -> &'static str {
        match self {
            Self::Title => "the page title",
            Self::Url => "the page URL",
        }
    }
}

impl Page {
    /// Starts an assertion about this page's title or URL.
    #[must_use]
    pub fn expect(&self) -> PageExpect {
        PageExpect::new(self.clone())
    }
}
