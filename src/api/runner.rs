//! Rust-native browser test lifecycle helpers.

use std::{future::Future, pin::Pin};

use crate::{BaseCase, BrowserConfig, Result, SeleniumBaseError};

/// Boxed future returned by a browser test body.
pub type BrowserTestFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + 'a>>;

/// Runs a browser test and always attempts to close its browser session.
///
/// This helper is useful for generated and handwritten `#[tokio::test]` tests
/// because `Drop` cannot await WebDriver cleanup.
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
pub(crate) fn combine_outcomes(test_result: Result<()>, cleanup_result: Result<()>) -> Result<()> {
    match (test_result, cleanup_result) {
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
