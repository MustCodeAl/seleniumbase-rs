//! Deferred assertions: check now, fail later.
//!
//! A deferred assertion is a soft check. A failed one does not stop the test;
//! it is recorded, and every recorded failure is reported together by
//! [`DeferredAsserts::process`]. This is the Rust counterpart of SeleniumBase's
//! `deferred_assert_element`, `deferred_assert_text` and
//! `process_deferred_asserts`.
//!
//! # Checking now
//!
//! [`check_element`](DeferredAsserts::check_element),
//! [`check_element_present`](DeferredAsserts::check_element_present),
//! [`check_text`](DeferredAsserts::check_text),
//! [`check_exact_text`](DeferredAsserts::check_exact_text) and
//! [`check_non_empty_text`](DeferredAsserts::check_non_empty_text) look at the
//! page *when they are called*, which is what SeleniumBase does, and record
//! the URL they looked at with any failure. A test that visits several pages
//! can collect a check per page and see at the end which page each failure
//! came from. The `deferred_assert_*` methods of [`BaseCase`] are these checks.
//!
//! # Queuing
//!
//! [`add_element`](DeferredAsserts::add_element) and
//! [`add_text`](DeferredAsserts::add_text) only queue an assertion. It runs
//! inside [`process`](DeferredAsserts::process), against whichever page is
//! open *then*, which is rarely the page it was queued for. Prefer the
//! `check_*` methods when more than one page is involved.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::api::deferred::DeferredAsserts;
//! use seleniumbase_rs::{BaseCase, BrowserConfig};
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let mut sb = BaseCase::new(BrowserConfig::default()).await?;
//! let mut deferred = DeferredAsserts::default();
//!
//! sb.open("https://seleniumbase.io/demo_page").await?;
//! deferred.check_element(&mut sb, "h1").await;
//! deferred.check_text(&mut sb, "Demo Page", "h1").await;
//!
//! sb.open("https://seleniumbase.io/simple/login").await?;
//! deferred.check_element(&mut sb, "#username").await;
//!
//! // One error listing every failed check, with the page each one saw.
//! deferred.process(&mut sb).await?;
//! # Ok(())
//! # }
//! ```

use crate::api::base_case::BaseCase;
use crate::error::SeleniumBaseError;

/// A single deferred assertion.
#[derive(Clone, Debug)]
pub enum DeferredAssert {
    /// Assert that the selected element exists and is visible.
    Element(String),
    /// Assert that the selected element exists in the page, visible or not.
    ElementPresent(String),
    /// Assert that the selected element contains the expected text.
    Text(String, String),
    /// Assert that the text of the selected element is exactly the expected text.
    ExactText(String, String),
    /// Assert that the selected element has visible text that is not blank.
    NonEmptyText(String),
}

/// A deferred assertion that did not hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeferredFailure {
    /// The page the assertion was evaluated on, when it could be read.
    pub url: Option<String>,
    /// Why the assertion failed.
    pub message: String,
}

impl std::fmt::Display for DeferredFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.url {
            Some(url) => write!(f, "{} (at {url})", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// What deferred assertions need from the browser: the two checks, and where
/// the browser is. [`BaseCase`] is the implementation; the seam lets the
/// bookkeeping be tested without a browser.
trait AssertTarget {
    async fn assert_element(&mut self, css: &str) -> Result<(), SeleniumBaseError>;
    async fn assert_element_present(&mut self, css: &str) -> Result<(), SeleniumBaseError>;
    async fn assert_text(&mut self, text: &str, css: &str) -> Result<(), SeleniumBaseError>;
    async fn assert_exact_text(&mut self, text: &str, css: &str) -> Result<(), SeleniumBaseError>;
    async fn assert_non_empty_text(&mut self, css: &str) -> Result<(), SeleniumBaseError>;
    async fn current_url(&mut self) -> Option<String>;
}

/// How long a deferred check waits for its element, as Python's `MINI_TIMEOUT`.
const DEFERRED_WAIT_SECS: u64 = 2;

impl AssertTarget for BaseCase {
    async fn assert_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        BaseCase::wait_for_element_visible(self, css, DEFERRED_WAIT_SECS)
            .await
            .map_err(|error| match error {
                SeleniumBaseError::InvalidSelector(_) => error,
                _ => SeleniumBaseError::AssertionFailed(format!(
                    "element '{css}' was not visible after {DEFERRED_WAIT_SECS}s"
                )),
            })
    }

    async fn assert_element_present(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        BaseCase::wait_for_element_present(self, css, DEFERRED_WAIT_SECS)
            .await
            .map_err(|error| match error {
                SeleniumBaseError::InvalidSelector(_) => error,
                _ => SeleniumBaseError::AssertionFailed(format!(
                    "element '{css}' was not present after {DEFERRED_WAIT_SECS}s"
                )),
            })
    }

    async fn assert_text(&mut self, text: &str, css: &str) -> Result<(), SeleniumBaseError> {
        BaseCase::assert_text(self, text, css).await
    }

    async fn assert_exact_text(&mut self, text: &str, css: &str) -> Result<(), SeleniumBaseError> {
        BaseCase::assert_exact_text(self, text, css).await
    }

    async fn assert_non_empty_text(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        BaseCase::wait_for_non_empty_text(self, css, DEFERRED_WAIT_SECS)
            .await
            .map_err(|error| match error {
                SeleniumBaseError::InvalidSelector(_) => error,
                _ => SeleniumBaseError::AssertionFailed(format!(
                    "element '{css}' showed no text after {DEFERRED_WAIT_SECS}s"
                )),
            })
    }

    async fn current_url(&mut self) -> Option<String> {
        self.get_current_url().await.ok()
    }
}

/// Collection of deferred assertions: failures already recorded, and
/// assertions queued for [`process`](Self::process).
#[derive(Clone, Debug, Default)]
pub struct DeferredAsserts {
    items: Vec<DeferredAssert>,
    failures: Vec<DeferredFailure>,
}

impl DeferredAsserts {
    /// Queues an element-visibility assertion.
    ///
    /// It runs in [`process`](Self::process), on the page that is open then.
    /// See [`check_element`](Self::check_element) to check the current page.
    pub fn add_element(&mut self, css: &str) {
        self.items.push(DeferredAssert::Element(css.to_owned()));
    }

    /// Queues a text-content assertion.
    ///
    /// It runs in [`process`](Self::process), on the page that is open then.
    /// See [`check_text`](Self::check_text) to check the current page.
    pub fn add_text(&mut self, text: &str, css: &str) {
        self.items
            .push(DeferredAssert::Text(text.to_owned(), css.to_owned()));
    }

    /// Queues a presence assertion: `css` exists, visible or not.
    ///
    /// It runs in [`process`](Self::process), on the page that is open then.
    /// See [`check_element_present`](Self::check_element_present).
    pub fn add_element_present(&mut self, css: &str) {
        self.items
            .push(DeferredAssert::ElementPresent(css.to_owned()));
    }

    /// Queues an exact-text assertion: the text of `css` is `text`.
    ///
    /// It runs in [`process`](Self::process), on the page that is open then.
    /// See [`check_exact_text`](Self::check_exact_text).
    pub fn add_exact_text(&mut self, text: &str, css: &str) {
        self.items
            .push(DeferredAssert::ExactText(text.to_owned(), css.to_owned()));
    }

    /// Queues a non-empty-text assertion: `css` shows some text.
    ///
    /// It runs in [`process`](Self::process), on the page that is open then.
    /// See [`check_non_empty_text`](Self::check_non_empty_text).
    pub fn add_non_empty_text(&mut self, css: &str) {
        self.items
            .push(DeferredAssert::NonEmptyText(css.to_owned()));
    }

    /// Checks now that `css` exists and is visible on the current page.
    ///
    /// A failure is recorded, with the page's URL, rather than returned.
    /// Returns whether the check held.
    pub async fn check_element(&mut self, sb: &mut BaseCase, css: &str) -> bool {
        self.run_element(sb, css).await
    }

    /// Checks now that `css` contains `text` on the current page.
    ///
    /// A failure is recorded, with the page's URL, rather than returned.
    /// Returns whether the check held.
    pub async fn check_text(&mut self, sb: &mut BaseCase, text: &str, css: &str) -> bool {
        self.run_text(sb, text, css).await
    }

    /// Checks now that `css` exists in the current page, visible or not.
    ///
    /// A failure is recorded, with the page's URL, rather than returned.
    /// Returns whether the check held.
    pub async fn check_element_present(&mut self, sb: &mut BaseCase, css: &str) -> bool {
        self.run_element_present(sb, css).await
    }

    /// Checks now that the text of `css` is exactly `text` on the current page.
    ///
    /// A failure is recorded, with the page's URL, rather than returned.
    /// Returns whether the check held.
    pub async fn check_exact_text(&mut self, sb: &mut BaseCase, text: &str, css: &str) -> bool {
        self.run_exact_text(sb, text, css).await
    }

    /// Checks now that `css` has visible text that is not blank.
    ///
    /// A failure is recorded, with the page's URL, rather than returned.
    /// Returns whether the check held.
    pub async fn check_non_empty_text(&mut self, sb: &mut BaseCase, css: &str) -> bool {
        self.run_non_empty_text(sb, css).await
    }

    /// Records a failure found by something other than the `check_*` methods,
    /// such as a window comparison, to be reported by [`process`](Self::process).
    pub fn record_failure(&mut self, url: Option<String>, message: impl Into<String>) {
        self.failures.push(DeferredFailure {
            url,
            message: message.into(),
        });
    }

    /// The failures recorded so far, oldest first.
    pub fn failures(&self) -> &[DeferredFailure] {
        &self.failures
    }

    /// How many assertions are queued and not yet run.
    pub fn pending(&self) -> usize {
        self.items.len()
    }

    /// Runs the queued assertions, then reports every failure recorded since
    /// the last call, whether from a queued assertion or a `check_*` call.
    ///
    /// The failures are cleared, so a second call starts afresh.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] listing every failure,
    /// each with the URL of the page it was evaluated on.
    pub async fn process(&mut self, sb: &mut BaseCase) -> Result<(), SeleniumBaseError> {
        self.run_queued(sb).await;
        self.report()
    }

    async fn run_element(&mut self, page: &mut impl AssertTarget, css: &str) -> bool {
        match page.assert_element(css).await {
            Ok(()) => true,
            Err(error) => {
                self.record(page, error).await;
                false
            }
        }
    }

    async fn run_text(&mut self, page: &mut impl AssertTarget, text: &str, css: &str) -> bool {
        match page.assert_text(text, css).await {
            Ok(()) => true,
            Err(error) => {
                self.record(page, error).await;
                false
            }
        }
    }

    async fn run_element_present(&mut self, page: &mut impl AssertTarget, css: &str) -> bool {
        match page.assert_element_present(css).await {
            Ok(()) => true,
            Err(error) => {
                self.record(page, error).await;
                false
            }
        }
    }

    async fn run_exact_text(
        &mut self,
        page: &mut impl AssertTarget,
        text: &str,
        css: &str,
    ) -> bool {
        match page.assert_exact_text(text, css).await {
            Ok(()) => true,
            Err(error) => {
                self.record(page, error).await;
                false
            }
        }
    }

    async fn run_non_empty_text(&mut self, page: &mut impl AssertTarget, css: &str) -> bool {
        match page.assert_non_empty_text(css).await {
            Ok(()) => true,
            Err(error) => {
                self.record(page, error).await;
                false
            }
        }
    }

    async fn run_queued(&mut self, page: &mut impl AssertTarget) {
        for item in std::mem::take(&mut self.items) {
            match item {
                DeferredAssert::Element(css) => self.run_element(page, &css).await,
                DeferredAssert::ElementPresent(css) => self.run_element_present(page, &css).await,
                DeferredAssert::Text(text, css) => self.run_text(page, &text, &css).await,
                DeferredAssert::ExactText(text, css) => {
                    self.run_exact_text(page, &text, &css).await
                }
                DeferredAssert::NonEmptyText(css) => self.run_non_empty_text(page, &css).await,
            };
        }
    }

    async fn record(&mut self, page: &mut impl AssertTarget, error: SeleniumBaseError) {
        let url = page.current_url().await;
        self.record_failure(url, error.to_string());
    }

    fn report(&mut self) -> Result<(), SeleniumBaseError> {
        if self.failures.is_empty() {
            return Ok(());
        }
        let failures = std::mem::take(&mut self.failures);
        let lines: Vec<String> = failures.iter().map(ToString::to_string).collect();
        Err(SeleniumBaseError::AssertionFailed(format!(
            "{} deferred assertion(s) failed:\n{}",
            failures.len(),
            lines.join("\n")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A browser whose page can be changed under the assertions.
    struct FakePage {
        url: &'static str,
        /// The selectors present on each url, with their text.
        pages: Vec<(&'static str, Vec<(&'static str, &'static str)>)>,
        /// The selectors that are in the page but not displayed.
        hidden: Vec<&'static str>,
        log: Vec<String>,
    }

    impl FakePage {
        fn new() -> Self {
            Self {
                url: "https://a.test/",
                pages: vec![
                    (
                        "https://a.test/",
                        vec![
                            ("h1", "Welcome"),
                            ("#only-a", "A"),
                            ("#msg", "Welcome home"),
                            ("#empty", ""),
                            ("#hidden", "Secret"),
                        ],
                    ),
                    ("https://b.test/", vec![("h1", "Other"), ("#only-b", "B")]),
                ],
                hidden: vec!["#hidden"],
                log: Vec::new(),
            }
        }

        fn text_of(&self, css: &str) -> Option<&'static str> {
            self.elements()
                .iter()
                .find(|(selector, _)| *selector == css)
                .map(|(_, text)| *text)
        }

        fn elements(&self) -> &[(&'static str, &'static str)] {
            self.pages
                .iter()
                .find(|(url, _)| *url == self.url)
                .map_or(&[], |(_, elements)| elements.as_slice())
        }
    }

    impl AssertTarget for FakePage {
        async fn assert_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
            self.log.push(format!("element {css} on {}", self.url));
            if self.text_of(css).is_some() && !self.hidden.contains(&css) {
                Ok(())
            } else {
                Err(SeleniumBaseError::element_not_found(css))
            }
        }

        async fn assert_element_present(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
            self.log
                .push(format!("element present {css} on {}", self.url));
            if self.text_of(css).is_some() {
                Ok(())
            } else {
                Err(SeleniumBaseError::element_not_found(css))
            }
        }

        async fn assert_exact_text(
            &mut self,
            text: &str,
            css: &str,
        ) -> Result<(), SeleniumBaseError> {
            self.log
                .push(format!("exact text {text} in {css} on {}", self.url));
            match self.text_of(css) {
                Some(content) if content == text => Ok(()),
                _ => Err(SeleniumBaseError::AssertionFailed(format!(
                    "exact text '{text}' not found in {css}"
                ))),
            }
        }

        async fn assert_non_empty_text(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
            self.log
                .push(format!("non-empty text in {css} on {}", self.url));
            match self.text_of(css) {
                Some(content) if !content.trim().is_empty() && !self.hidden.contains(&css) => {
                    Ok(())
                }
                _ => Err(SeleniumBaseError::AssertionFailed(format!(
                    "{css} has no visible text"
                ))),
            }
        }

        async fn assert_text(&mut self, text: &str, css: &str) -> Result<(), SeleniumBaseError> {
            self.log
                .push(format!("text {text} in {css} on {}", self.url));
            match self
                .elements()
                .iter()
                .find(|(selector, _)| *selector == css)
            {
                Some((_, content)) if content.contains(text) => Ok(()),
                _ => Err(SeleniumBaseError::AssertionFailed(format!(
                    "text '{text}' not found in {css}"
                ))),
            }
        }

        async fn current_url(&mut self) -> Option<String> {
            Some(self.url.to_owned())
        }
    }

    #[tokio::test]
    async fn a_check_looks_at_the_page_it_is_called_on_not_the_page_open_at_process_time() {
        let mut page = FakePage::new();
        let mut deferred = DeferredAsserts::default();

        // On page A: #only-a is there, #only-b is not.
        assert!(deferred.run_element(&mut page, "#only-a").await);
        assert!(!deferred.run_element(&mut page, "#only-b").await);

        // Then the test moves to page B, where it is the other way round.
        page.url = "https://b.test/";
        assert!(deferred.run_element(&mut page, "#only-b").await);
        assert!(!deferred.run_element(&mut page, "#only-a").await);

        let failures = deferred.failures().to_vec();
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].url.as_deref(), Some("https://a.test/"));
        assert!(failures[0].message.contains("#only-b"), "{failures:?}");
        assert_eq!(failures[1].url.as_deref(), Some("https://b.test/"));
        assert!(failures[1].message.contains("#only-a"), "{failures:?}");
    }

    #[tokio::test]
    async fn queued_assertions_run_against_the_page_open_when_they_are_processed() {
        let mut page = FakePage::new();
        let mut deferred = DeferredAsserts::default();

        // Queued while on page A, where #only-a exists.
        deferred.add_element("#only-a");
        deferred.add_text("Welcome", "h1");
        assert_eq!(deferred.pending(), 2);
        assert!(page.log.is_empty(), "queuing must not touch the browser");

        // Processed on page B: both fail there, and say so.
        page.url = "https://b.test/";
        deferred.run_queued(&mut page).await;
        assert_eq!(deferred.pending(), 0);
        assert_eq!(deferred.failures().len(), 2);
        assert!(deferred
            .failures()
            .iter()
            .all(|failure| failure.url.as_deref() == Some("https://b.test/")));
    }

    #[tokio::test]
    async fn the_report_lists_every_failure_with_its_page_and_then_clears() {
        let mut page = FakePage::new();
        let mut deferred = DeferredAsserts::default();
        assert!(!deferred.run_element(&mut page, "#nope").await);
        page.url = "https://b.test/";
        assert!(!deferred.run_text(&mut page, "Missing", "h1").await);
        assert!(deferred.run_text(&mut page, "Other", "h1").await);

        let error = deferred.report().unwrap_err().to_string();
        assert!(error.contains("2 deferred assertion(s) failed:"), "{error}");
        assert!(error.contains("(at https://a.test/)"), "{error}");
        assert!(error.contains("(at https://b.test/)"), "{error}");
        assert!(
            error.find("a.test").unwrap() < error.find("b.test").unwrap(),
            "failures keep the order they happened in: {error}"
        );

        // Reported once; a second report has nothing left.
        assert!(deferred.failures().is_empty());
        assert!(deferred.report().is_ok());
    }

    #[tokio::test]
    async fn an_all_passing_run_reports_nothing() {
        let mut page = FakePage::new();
        let mut deferred = DeferredAsserts::default();
        deferred.add_element("h1");
        deferred.add_text("Welcome", "h1");
        deferred.run_queued(&mut page).await;
        assert!(deferred.run_element(&mut page, "#only-a").await);
        assert!(deferred.report().is_ok());
    }

    #[tokio::test]
    async fn a_hidden_element_is_present_but_not_visible() {
        let mut page = FakePage::new();
        let mut deferred = DeferredAsserts::default();
        assert!(deferred.run_element_present(&mut page, "#hidden").await);
        assert!(deferred.failures().is_empty());
        assert!(!deferred.run_element(&mut page, "#hidden").await);
        assert_eq!(deferred.failures().len(), 1);
        assert!(!deferred.run_element_present(&mut page, "#nope").await);
        assert_eq!(deferred.failures().len(), 2);
    }

    #[tokio::test]
    async fn exact_text_must_match_the_whole_text_not_a_part() {
        let mut page = FakePage::new();
        let mut deferred = DeferredAsserts::default();
        // "Welcome" is part of "Welcome home", which a contains-check accepts.
        assert!(!deferred.run_exact_text(&mut page, "Welcome", "#msg").await);
        assert!(deferred.run_text(&mut page, "Welcome", "#msg").await);
        assert!(
            deferred
                .run_exact_text(&mut page, "Welcome home", "#msg")
                .await
        );
        assert_eq!(deferred.failures().len(), 1, "{:?}", deferred.failures());
    }

    #[tokio::test]
    async fn non_empty_text_fails_for_an_empty_or_hidden_element() {
        let mut page = FakePage::new();
        let mut deferred = DeferredAsserts::default();
        assert!(deferred.run_non_empty_text(&mut page, "#msg").await);
        assert!(!deferred.run_non_empty_text(&mut page, "#empty").await);
        assert!(!deferred.run_non_empty_text(&mut page, "#hidden").await);
        assert_eq!(deferred.failures().len(), 2, "{:?}", deferred.failures());
    }

    #[tokio::test]
    async fn each_queued_variant_runs_its_own_check() {
        let mut page = FakePage::new();
        let mut deferred = DeferredAsserts::default();
        deferred.add_element_present("#hidden");
        deferred.add_exact_text("Welcome", "#msg");
        deferred.add_non_empty_text("#empty");
        assert_eq!(deferred.pending(), 3);
        deferred.run_queued(&mut page).await;
        // The presence check holds; the exact-text and non-empty checks fail.
        assert_eq!(deferred.failures().len(), 2, "{:?}", deferred.failures());
        assert!(page.log.iter().any(|line| line.starts_with("exact text")));
        assert!(page
            .log
            .iter()
            .any(|line| line.starts_with("non-empty text")));
    }

    #[test]
    fn a_failure_recorded_from_outside_is_reported_with_its_url() {
        let mut deferred = DeferredAsserts::default();
        deferred.record_failure(Some("https://a.test/".to_owned()), "window differs");
        deferred.record_failure(None, "no url here");
        let error = deferred.report().unwrap_err().to_string();
        assert!(error.contains("2 deferred assertion(s) failed:"), "{error}");
        assert!(
            error.contains("window differs (at https://a.test/)"),
            "{error}"
        );
        assert!(error.contains("no url here"), "{error}");
    }

    #[test]
    fn a_failure_without_a_url_prints_just_its_message() {
        let failure = DeferredFailure {
            url: None,
            message: "boom".to_owned(),
        };
        assert_eq!(failure.to_string(), "boom");
    }

    /// The futures must stay `Send`, as they were when `process` called
    /// `BaseCase` directly, so a test can be spawned onto a multi-thread runtime.
    /// This only has to compile.
    #[test]
    fn the_public_futures_are_send() {
        fn assert_send<T: Send>(_: &T) {}
        fn check(sb: &mut BaseCase, deferred: &mut DeferredAsserts) {
            assert_send(&deferred.process(sb));
        }
        let _: fn(&mut BaseCase, &mut DeferredAsserts) = check;
    }
}
