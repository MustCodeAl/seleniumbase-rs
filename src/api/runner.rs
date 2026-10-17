//! Rust-native browser test lifecycle helpers.

use std::{future::Future, pin::Pin};

use crate::{BaseCase, BrowserConfig, Result, SeleniumBaseError};

/// Boxed future returned by a browser test body.
pub type BrowserTestFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + 'a>>;

/// Runs a browser test and always attempts to close its browser session.
///
/// This helper is useful for generated and handwritten `#[tokio::test]` tests
/// because `Drop` cannot await WebDriver cleanup.
///
/// A test that ends with [`BaseCase::skip_test`] counts as passed: libtest has
/// no way to report a skip, so the reason is logged and the test returns `Ok`.
pub async fn run_browser_test<F>(config: BrowserConfig, test: F) -> Result<()>
where
    F: for<'a> FnOnce(&'a mut BaseCase) -> BrowserTestFuture<'a>,
{
    let mut sb = BaseCase::new(config).await?;
    let test_result = test(&mut sb).await;
    let cleanup_result = sb.quit().await;
    combine_outcomes(test_result, cleanup_result)
}

/// The result of a test whose browser was then closed: the test's own error if
/// only it failed, the cleanup error if only that failed, and both if both did.
/// A skip is not a failure; see [`run_browser_test`].
///
/// Public only because [`sb_test!`](crate::sb_test) expands to a call of it.
#[doc(hidden)]
pub fn combine_outcomes(test_result: Result<()>, cleanup_result: Result<()>) -> Result<()> {
    match (test_result, cleanup_result) {
        (Err(skipped), Ok(())) if skipped.is_skipped() => {
            tracing::info!(reason = %skipped, "test skipped");
            Ok(())
        }
        (Ok(()), Ok(())) => Ok(()),
        (Err(test_error), Ok(())) => Err(test_error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(test_error), Err(cleanup_error)) => Err(SeleniumBaseError::TestLifecycle(format!(
            "test failed with '{test_error}'; cleanup failed with '{cleanup_error}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(message: &str) -> Result<()> {
        Err(SeleniumBaseError::AssertionFailed(message.to_owned()))
    }

    #[test]
    fn a_clean_test_and_cleanup_is_ok() {
        assert!(combine_outcomes(Ok(()), Ok(())).is_ok());
    }

    #[test]
    fn a_skipped_test_is_not_a_failure() {
        let skipped = Err(SeleniumBaseError::Skipped("no browser here".to_owned()));
        assert!(combine_outcomes(skipped, Ok(())).is_ok());
    }

    #[test]
    fn a_skipped_test_whose_cleanup_failed_still_fails() {
        let skipped = Err(SeleniumBaseError::Skipped("no browser here".to_owned()));
        let error = combine_outcomes(skipped, failure("stuck")).unwrap_err();
        assert!(error.to_string().contains("stuck"), "{error}");
    }

    #[test]
    fn the_test_error_wins_when_only_the_test_failed() {
        let error = combine_outcomes(failure("boom"), Ok(())).unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::AssertionFailed(_)),
            "{error}"
        );
    }

    #[test]
    fn the_cleanup_error_is_reported_when_only_cleanup_failed() {
        let error = combine_outcomes(Ok(()), failure("stuck")).unwrap_err();
        assert!(error.to_string().contains("stuck"), "{error}");
    }

    #[test]
    fn both_errors_are_kept_when_both_failed() {
        let error = combine_outcomes(failure("boom"), failure("stuck")).unwrap_err();
        let shown = error.to_string();
        assert!(
            matches!(error, SeleniumBaseError::TestLifecycle(_)),
            "{shown}"
        );
        assert!(shown.contains("boom") && shown.contains("stuck"), "{shown}");
    }
}
