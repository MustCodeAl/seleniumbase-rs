//! Resilient auto-healing locators with prioritized fallback strategies.
//!
//! Eliminates brittle test failures by allowing a list of candidate selectors
//! in order of priority. The first candidate that successfully resolves is cached
//! for subsequent actions, and selector drift is logged via `tracing::warn!`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{Locator, Page};
use crate::error::SeleniumBaseError;
use crate::utils::selectors::Selector;

/// A resilient, self-healing locator that tries fallback selectors if the primary selector fails.
///
/// Automatically caches the successful selector index in memory so subsequent
/// interactions avoid re-testing failed candidates.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::Page;
///
/// # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// // Primary is data-testid; fallback to id; fallback to button text
/// let btn = page.resilient_locator(&[
///     "[data-testid='submit-order']",
///     "#submit-btn",
///     "button:has-text('Place Order')",
/// ]);
///
/// btn.click().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ResilientLocator {
    page: Page,
    candidates: Vec<Selector>,
    cached_index: Arc<AtomicUsize>,
}

impl ResilientLocator {
    /// Creates a new `ResilientLocator` with a list of candidate selectors.
    ///
    /// The first candidate is treated as the primary selector.
    #[must_use]
    pub fn new(page: Page, candidates: Vec<Selector>) -> Self {
        Self {
            page,
            candidates,
            cached_index: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the candidate selectors in priority order.
    #[must_use]
    pub fn candidates(&self) -> &[Selector] {
        &self.candidates
    }

    /// Returns the currently active / cached selector.
    #[must_use]
    pub fn active_selector(&self) -> Option<&Selector> {
        let idx = self.cached_index.load(Ordering::Relaxed);
        self.candidates.get(idx).or_else(|| self.candidates.first())
    }

    async fn execute_with_fallback<T, F, Fut>(&self, action: F) -> Result<T, SeleniumBaseError>
    where
        F: Fn(Locator) -> Fut,
        Fut: Future<Output = Result<T, SeleniumBaseError>>,
    {
        if self.candidates.is_empty() {
            return Err(SeleniumBaseError::InvalidSelector(
                "no candidate selectors provided".to_string(),
            ));
        }

        let cached_idx = self.cached_index.load(Ordering::Relaxed);

        // 1. Try cached index first
        if let Some(sel) = self.candidates.get(cached_idx) {
            let loc = self.page.locator(sel);
            match action(loc).await {
                Ok(val) => return Ok(val),
                Err(err) => {
                    tracing::debug!(
                        "ResilientLocator: cached candidate [{}] '{}' failed: {:?}",
                        cached_idx,
                        sel,
                        err
                    );
                }
            }
        }

        // 2. Iterate remaining candidates in order
        let mut first_error = None;
        for (idx, sel) in self.candidates.iter().enumerate() {
            if idx == cached_idx {
                continue;
            }

            let loc = self.page.locator(sel);
            match action(loc).await {
                Ok(val) => {
                    tracing::warn!(
                        "ResilientLocator self-healed: primary '{}' not found, fell back to [{}] '{}'",
                        self.candidates[0],
                        idx,
                        sel
                    );
                    self.cached_index.store(idx, Ordering::Relaxed);
                    return Ok(val);
                }
                Err(err) => {
                    if first_error.is_none() {
                        first_error = Some(err);
                    }
                }
            }
        }

        Err(first_error.unwrap_or_else(|| {
            SeleniumBaseError::InvalidSelector(
                "all resilient locator candidates failed".to_string(),
            )
        }))
    }

    /// Clicks the first matching candidate element.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if none of the candidate selectors succeed.
    pub async fn click(&self) -> Result<(), SeleniumBaseError> {
        self.execute_with_fallback(|loc| async move { loc.click().await })
            .await
    }

    /// Types text into the first matching candidate element.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if none of the candidate selectors succeed.
    pub async fn type_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.execute_with_fallback(|loc| async move { loc.type_text(text).await })
            .await
    }

    /// Moves the pointer over the first matching candidate element.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if none of the candidate selectors succeed.
    pub async fn hover(&self) -> Result<(), SeleniumBaseError> {
        self.execute_with_fallback(|loc| async move { loc.hover().await })
            .await
    }

    /// Reads the inner text of the first matching candidate element.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if none of the candidate selectors succeed.
    pub async fn text(&self) -> Result<String, SeleniumBaseError> {
        self.execute_with_fallback(|loc| async move { loc.text().await })
            .await
    }

    /// Returns `true` if any candidate element is currently visible.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if querying fails.
    pub async fn is_visible(&self) -> Result<bool, SeleniumBaseError> {
        for (idx, sel) in self.candidates.iter().enumerate() {
            let loc = self.page.locator(sel);
            match loc.is_visible().await {
                Ok(true) => {
                    self.cached_index.store(idx, Ordering::Relaxed);
                    return Ok(true);
                }
                Ok(false) => {}
                Err(err) => {
                    tracing::debug!(
                        "ResilientLocator::is_visible candidate [{idx}] '{sel}' check error: {err}"
                    );
                }
            }
        }
        Ok(false)
    }
}
