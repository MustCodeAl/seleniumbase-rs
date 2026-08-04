//! Deferred assertion collection and processing.
//!
//! Test cases can queue assertions while continuing execution, then flush them
//! all at once with [`DeferredAsserts::process`]. This is useful for soft
//! validations that should not stop the test on first failure.

use crate::api::base_case::BaseCase;
use crate::error::SeleniumBaseError;

/// A single deferred assertion.
#[derive(Clone, Debug)]
pub enum DeferredAssert {
    /// Assert that the selected element exists and is visible.
    Element(String),
    /// Assert that the selected element contains the expected text.
    Text(String, String),
}

/// Collection of deferred assertions queued for later processing.
#[derive(Clone, Debug, Default)]
pub struct DeferredAsserts {
    items: Vec<DeferredAssert>,
}

impl DeferredAsserts {
    /// Queues an element-visibility assertion.
    pub fn add_element(&mut self, css: &str) {
        self.items.push(DeferredAssert::Element(css.to_owned()));
    }

    /// Queues a text-content assertion.
    pub fn add_text(&mut self, text: &str, css: &str) {
        self.items
            .push(DeferredAssert::Text(text.to_owned(), css.to_owned()));
    }

    /// Runs all queued assertions and reports any failures together.
    pub async fn process(&mut self, sb: &mut BaseCase) -> Result<(), SeleniumBaseError> {
        let mut failures = Vec::new();
        for item in std::mem::take(&mut self.items) {
            let result = match item {
                DeferredAssert::Element(css) => sb.assert_element(&css).await,
                DeferredAssert::Text(text, css) => sb.assert_text(&css, &text).await,
            };
            if let Err(e) = result {
                failures.push(e.to_string());
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(format!(
                "{} deferred assertion(s) failed:\n{}",
                failures.len(),
                failures.join("\n")
            )))
        }
    }
}
