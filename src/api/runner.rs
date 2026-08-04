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

    match (test_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(test_error), Ok(())) => Err(test_error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(test_error), Err(cleanup_error)) => Err(SeleniumBaseError::TestLifecycle(format!(
            "test failed with '{test_error}'; cleanup failed with '{cleanup_error}'"
        ))),
    }
}
