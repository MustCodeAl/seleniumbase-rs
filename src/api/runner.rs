//! Rust-native browser test lifecycle helpers.
//!
//! [`run_browser_test`] drives a WebDriver [`BaseCase`] and [`run_cdp_test`] a
//! Pure CDP [`Page`]. Both take an async closure and share one lifecycle (see
//! `drive`): the test runs, a failure is handed to whoever wants evidence, the
//! browser is closed whatever happened, and a panic in the test is re-raised
//! only after that.

use std::any::Any;
use std::panic::{resume_unwind, AssertUnwindSafe};

use futures_util::FutureExt;

use crate::sb_cdp::{Browser, LaunchOptions, Page};
use crate::{BaseCase, BrowserConfig, Result, SeleniumBaseError};

/// Runs a browser test and always attempts to close its browser session.
///
/// The test is an async closure, `async |sb| { .. }`, or an `async fn` that
/// takes `&mut BaseCase`. This helper is useful for generated and handwritten
/// `#[tokio::test]` tests because `Drop` cannot await WebDriver cleanup.
///
/// ```no_run
/// use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};
///
/// # async fn demo() -> Result<()> {
/// run_browser_test(BrowserConfig::default(), async |sb| {
///     sb.open("https://example.com").await?;
///     sb.assert_title("Example Domain").await
/// })
/// .await
/// # }
/// ```
///
/// The browser is closed when the test panics too, and the panic is then
/// raised again, so a failed `assert!` still fails the test.
///
/// A test that ends with [`BaseCase::skip_test`] counts as passed: libtest has
/// no way to report a skip, so the reason is logged and the test returns `Ok`.
///
/// The run settings of the configuration ([`BrowserConfig::with_run_settings`])
/// are the case's from the start, as they are for [`BaseCase::new`].
///
/// # Errors
///
/// Returns the test's error, the browser's launch or cleanup error, or both
/// combined into [`SeleniumBaseError::TestLifecycle`].
pub async fn run_browser_test<F>(config: BrowserConfig, test: F) -> Result<()>
where
    F: AsyncFnOnce(&mut BaseCase) -> Result<()>,
{
    let mut sb = BaseCase::new(config).await?;
    drive(&mut sb, test, no_evidence, close_browser)
        .await
        .finish()
}

/// Launches a Pure CDP browser, runs `test` on its first tab and always closes
/// the browser, the way [`run_browser_test`] does for WebDriver.
///
/// ```no_run
/// use seleniumbase_rs::api::runner::run_cdp_test;
/// use seleniumbase_rs::sb_cdp::LaunchOptions;
/// use seleniumbase_rs::Result;
///
/// # async fn demo() -> Result<()> {
/// run_cdp_test(LaunchOptions::default(), async |page| {
///     page.goto("https://example.com").await?;
///     page.locator("h1").click().await
/// })
/// .await
/// # }
/// ```
///
/// # Errors
///
/// Returns the test's error, the browser's launch or close error, or both
/// combined into [`SeleniumBaseError::TestLifecycle`].
pub async fn run_cdp_test<F>(options: LaunchOptions, test: F) -> Result<()>
where
    F: AsyncFnOnce(&Page) -> Result<()>,
{
    let browser = Browser::launch(options).await?;
    run_cdp_test_on(browser, test).await
}

/// [`run_cdp_test`] on a browser that is already running (or mocked).
pub(crate) async fn run_cdp_test_on<F>(browser: Browser, test: F) -> Result<()>
where
    F: AsyncFnOnce(&Page) -> Result<()>,
{
    let mut case = CdpCase::open(browser).await?;
    drive(
        &mut case,
        async |case: &mut CdpCase| test(&case.page).await,
        no_evidence,
        CdpCase::close,
    )
    .await
    .finish()
}

/// A Pure CDP browser and the tab a test runs on.
pub(crate) struct CdpCase {
    pub(crate) browser: Browser,
    pub(crate) page: Page,
}

impl CdpCase {
    /// The first tab of `browser`; the browser is closed if there is none.
    pub(crate) async fn open(browser: Browser) -> Result<Self> {
        match browser.default_page().await {
            Ok(page) => Ok(Self { browser, page }),
            Err(error) => Err(match browser.close().await {
                Ok(()) => error,
                Err(cleanup) => SeleniumBaseError::TestLifecycle(format!(
                    "no tab to test: '{error}'; cleanup failed with '{cleanup}'"
                )),
            }),
        }
    }

    pub(crate) async fn close(&mut self) -> Result<()> {
        self.browser.close().await
    }
}

pub(crate) async fn close_browser(sb: &mut BaseCase) -> Result<()> {
    sb.quit().await
}

/// The hook for a runner that takes no evidence of a failure.
pub(crate) async fn no_evidence<C>(_case: &C, _error: &SeleniumBaseError) {}

/// How a test ended, after its browser was closed.
pub(crate) struct Ended {
    outcome: Result<()>,
    panic: Option<Box<dyn Any + Send>>,
}

impl Ended {
    /// The test's own result combined with the cleanup's. A test that panicked
    /// is an error here, so a caller can report it before re-raising.
    pub(crate) fn outcome(&self) -> &Result<()> {
        &self.outcome
    }

    /// The result, or the test's panic raised again. When the browser could
    /// not be closed after a panic, the cleanup error is logged first: the
    /// unwind can carry only the panic.
    pub(crate) fn finish(self) -> Result<()> {
        match self.panic {
            Some(payload) => {
                if let Err(error) = &self.outcome {
                    tracing::warn!(%error, "the test panicked; its lifecycle result is dropped by the unwind");
                }
                resume_unwind(payload)
            }
            None => self.outcome,
        }
    }
}

/// The one test lifecycle: runs `test` on `case`, hands a failure that is not
/// a skip to `evidence` while the browser is still open, then always runs
/// `close`.
///
/// A panic in `test` is caught so `close` still runs; it counts as a failure
/// for `evidence` and is raised again by [`Ended::finish`].
pub(crate) async fn drive_with<C>(
    case: &mut C,
    test: impl AsyncFnOnce(&mut C) -> Result<()>,
    evidence: impl AsyncFnOnce(&C, &SeleniumBaseError),
    close: impl AsyncFnOnce(&mut C, &Result<()>) -> Result<()>,
) -> Ended {
    // Each closure's future is boxed, so it is not laid out inside this one:
    // nested in a test's future they made rustc overflow its query depth limit.
    let running = Box::pin(test(&mut *case));
    let (test_result, panic) = match AssertUnwindSafe(running).catch_unwind().await {
        Ok(result) => (result, None),
        Err(payload) => {
            let error = SeleniumBaseError::TestLifecycle(format!(
                "the test panicked: {}",
                panic_message(payload.as_ref())
            ));
            (Err(error), Some(payload))
        }
    };
    if let Err(error) = &test_result {
        // A skipped test did not fail: nothing to take evidence of.
        if !error.is_skipped() {
            Box::pin(evidence(&*case, error)).await;
        }
    }
    let cleanup_result = Box::pin(close(&mut *case, &test_result)).await;
    Ended {
        outcome: combine_outcomes(test_result, cleanup_result),
        panic,
    }
}

/// [`drive_with`] for a runner whose `close` does not look at how the test
/// ended.
pub(crate) async fn drive<C>(
    case: &mut C,
    test: impl AsyncFnOnce(&mut C) -> Result<()>,
    evidence: impl AsyncFnOnce(&C, &SeleniumBaseError),
    close: impl AsyncFnOnce(&mut C) -> Result<()>,
) -> Ended {
    drive_with(
        case,
        test,
        evidence,
        async |case: &mut C, _ended: &Result<()>| close(case).await,
    )
    .await
}

/// The message of a panic payload, when it is text.
fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(not text)")
}

/// The result of a test whose browser was then closed: the test's own error if
/// only it failed, the cleanup error if only that failed, and both if both did.
/// A skip is not a failure; see [`run_browser_test`].
pub(crate) fn combine_outcomes(test_result: Result<()>, cleanup_result: Result<()>) -> Result<()> {
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

    /// A case that only remembers what happened to it.
    #[derive(Default)]
    struct Log(Vec<&'static str>);

    async fn closed(log: &mut Log) -> Result<()> {
        log.0.push("closed");
        Ok(())
    }

    async fn saw(log: &Log, error: &SeleniumBaseError) {
        assert!(!error.is_skipped());
        let _ = log;
        EVIDENCE.with(|seen| *seen.borrow_mut() += 1);
    }

    thread_local! {
        static EVIDENCE: std::cell::RefCell<u32> = const { std::cell::RefCell::new(0) };
    }

    fn evidence_taken() -> u32 {
        EVIDENCE.with(|seen| std::mem::take(&mut *seen.borrow_mut()))
    }

    #[tokio::test]
    async fn a_passing_test_closes_the_browser_and_takes_no_evidence() {
        evidence_taken();
        let mut log = Log::default();
        let ended = drive(
            &mut log,
            async |log: &mut Log| {
                log.0.push("ran");
                Ok(())
            },
            saw,
            closed,
        )
        .await;
        assert!(ended.outcome().is_ok());
        assert_eq!(log.0, ["ran", "closed"]);
        assert_eq!(evidence_taken(), 0);
    }

    #[tokio::test]
    async fn a_failing_test_gives_evidence_before_the_browser_closes() {
        evidence_taken();
        let mut log = Log::default();
        let ended = drive(&mut log, async |_: &mut Log| failure("boom"), saw, closed).await;
        assert!(ended.outcome().is_err());
        assert_eq!(log.0, ["closed"]);
        assert_eq!(evidence_taken(), 1);
    }

    #[tokio::test]
    async fn a_skipped_test_is_ok_and_gives_no_evidence() {
        evidence_taken();
        let mut log = Log::default();
        let ended = drive(
            &mut log,
            async |_: &mut Log| Err(SeleniumBaseError::Skipped("no browser".to_owned())),
            saw,
            closed,
        )
        .await;
        assert!(ended.finish().is_ok());
        assert_eq!(log.0, ["closed"]);
        assert_eq!(evidence_taken(), 0);
    }

    #[tokio::test]
    async fn a_panicking_test_still_closes_the_browser_and_then_panics_again() {
        evidence_taken();
        let mut log = Log::default();
        let ended = drive(
            &mut log,
            async |_: &mut Log| -> Result<()> { panic!("assertion failed in the test") },
            saw,
            closed,
        )
        .await;
        assert_eq!(log.0, ["closed"], "the browser is closed first");
        assert_eq!(evidence_taken(), 1, "a panic is a failure for the plugins");
        let shown = ended.outcome().as_ref().unwrap_err().to_string();
        assert!(shown.contains("assertion failed in the test"), "{shown}");
        let raised = std::panic::catch_unwind(AssertUnwindSafe(|| ended.finish()));
        let payload = raised.expect_err("the panic is raised again");
        assert_eq!(
            panic_message(payload.as_ref()),
            "assertion failed in the test"
        );
    }

    #[tokio::test]
    async fn a_cdp_test_runs_on_the_first_tab_and_closes_the_browser() {
        let (browser, mock) = Browser::new_mocked();
        mock.reply(
            "Runtime.evaluate",
            serde_json::json!({ "result": { "value": "Example" } }),
        );
        let outcome = run_cdp_test_on(browser, async |page: &Page| {
            assert_eq!(page.title().await?, "Example");
            Ok(())
        })
        .await;
        assert!(outcome.is_ok(), "{outcome:?}");
    }

    #[tokio::test]
    async fn a_cdp_test_that_fails_returns_its_error() {
        let (browser, _mock) = Browser::new_mocked();
        let outcome = run_cdp_test_on(browser, async |_: &Page| failure("boom")).await;
        assert!(
            matches!(outcome, Err(SeleniumBaseError::AssertionFailed(_))),
            "{outcome:?}"
        );
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
