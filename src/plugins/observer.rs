//! Hooks that run around a test and can look at the page when it fails.
//!
//! A [`TestPlugin`] is told when a test starts, when it fails (with the page
//! still open, so it can take evidence) and when it finishes. The plugins that
//! ship with the crate save a screenshot and the page source on failure and
//! write a report; anything else is a few lines of your own.
//!
//! A plugin never changes how a test turns out. If one of them fails (a full
//! disk, a browser that has already gone away) the failure is logged and the
//! test's own result is returned untouched.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::plugins::observer::Plugins;
//! use seleniumbase_rs::plugins::page_source::PageSourceOnFailurePlugin;
//! use seleniumbase_rs::plugins::reports::ReportPlugin;
//! use seleniumbase_rs::plugins::screen_shots::ScreenshotOnFailurePlugin;
//! use seleniumbase_rs::BrowserConfig;
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let mut plugins = Plugins::new()
//!     .with(ScreenshotOnFailurePlugin::new("latest_logs"))
//!     .with(PageSourceOnFailurePlugin::new("latest_logs"))
//!     .with(ReportPlugin::json("latest_logs/report.json"));
//!
//! plugins
//!     .run_browser_test("login_works", BrowserConfig::default(), async |sb| {
//!         sb.open("https://example.com").await?;
//!         sb.assert_title("Example Domain").await
//!     })
//!     .await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::time::{Duration, Instant};

use async_trait::async_trait;

#[allow(deprecated, reason = "the shim keeps the old closure type")]
use crate::api::runner::BrowserTestFuture;
use crate::api::runner::{close_browser, drive, start_browser, CdpCase};
use crate::error::{Result, SeleniumBaseError};
use crate::sb_cdp::{Browser, LaunchOptions, Page};
use crate::{BaseCase, BrowserConfig};

/// What a plugin can look at when a test fails.
///
/// Implemented for [`BaseCase`] and for [`sb_cdp::Page`](crate::sb_cdp::Page),
/// so the same plugins work with either engine. Implement it for your own
/// page type to reuse them elsewhere.
#[async_trait]
pub trait PageEvidence: Sync {
    /// The current viewport as PNG bytes.
    async fn screenshot_png(&self) -> Result<Vec<u8>>;

    /// The current document's HTML.
    async fn page_source(&self) -> Result<String>;

    /// The current page's URL.
    ///
    /// The default is an [`Unsupported`](SeleniumBaseError::Unsupported)
    /// error, for page types that cannot tell; reports then leave it out.
    async fn current_url(&self) -> Result<String> {
        Err(SeleniumBaseError::unsupported(
            "this page cannot report its URL",
        ))
    }

    /// The current page's title. The default is an error, as for
    /// [`current_url`](Self::current_url).
    async fn page_title(&self) -> Result<String> {
        Err(SeleniumBaseError::unsupported(
            "this page cannot report its title",
        ))
    }
}

#[async_trait]
impl PageEvidence for BaseCase {
    async fn screenshot_png(&self) -> Result<Vec<u8>> {
        self.screenshot_as_png().await
    }

    async fn page_source(&self) -> Result<String> {
        self.get_page_source().await
    }

    // `get_current_url` and `get_title` need `&mut self`; asking the page
    // needs only a look.
    async fn current_url(&self) -> Result<String> {
        script_text(self, "return location.href;").await
    }

    async fn page_title(&self) -> Result<String> {
        script_text(self, "return document.title;").await
    }
}

/// The text a script returns, or an error if it returns something else.
async fn script_text(case: &BaseCase, script: &str) -> Result<String> {
    match case.execute_script(script).await? {
        serde_json::Value::String(text) => Ok(text),
        other => Err(SeleniumBaseError::parse(other.to_string(), "text")),
    }
}

#[async_trait]
impl PageEvidence for crate::sb_cdp::Page {
    async fn screenshot_png(&self) -> Result<Vec<u8>> {
        self.screenshot().await
    }

    async fn page_source(&self) -> Result<String> {
        self.evaluate_as("document.documentElement.outerHTML").await
    }

    async fn current_url(&self) -> Result<String> {
        self.url().await
    }

    async fn page_title(&self) -> Result<String> {
        self.title().await
    }
}

/// Code that runs around a test. Every hook is optional.
///
/// A hook that returns an error is logged and skipped; it cannot change the
/// test's result, and the remaining plugins still run.
#[async_trait]
pub trait TestPlugin: Send {
    /// The name used in log messages. Defaults to the type's name.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// A test is about to run.
    async fn test_started(&mut self, _test: &str) -> Result<()> {
        Ok(())
    }

    /// A test failed. The page is still open, so evidence can be taken.
    async fn test_failed(
        &mut self,
        _test: &str,
        _error: &SeleniumBaseError,
        _page: &dyn PageEvidence,
    ) -> Result<()> {
        Ok(())
    }

    /// A test is over, whether it passed or failed, and its browser is closed.
    async fn test_finished(
        &mut self,
        _test: &str,
        _elapsed: Duration,
        _outcome: &Result<()>,
    ) -> Result<()> {
        Ok(())
    }
}

/// A set of plugins that are told about each test, in the order they were added.
#[derive(Default)]
pub struct Plugins {
    plugins: Vec<Box<dyn TestPlugin>>,
}

impl fmt::Debug for Plugins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plugins")
            .field(
                "plugins",
                &self.plugins.iter().map(|p| p.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Plugins {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a plugin.
    #[must_use]
    pub fn with(mut self, plugin: impl TestPlugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// How many plugins are in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Whether the set has no plugins.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Tells every plugin that `test` is about to run.
    pub async fn started(&mut self, test: &str) {
        for plugin in &mut self.plugins {
            let result = plugin.test_started(test).await;
            log_failure(plugin.name(), "test_started", test, result);
        }
    }

    /// Tells every plugin that `test` failed with `error`, giving them `page`
    /// to take evidence from.
    ///
    /// Call the three hooks yourself to use plugins outside
    /// [`run_browser_test`](Self::run_browser_test), for example with the Pure
    /// CDP engine:
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use seleniumbase_rs::plugins::observer::Plugins;
    /// use seleniumbase_rs::plugins::screen_shots::ScreenshotOnFailurePlugin;
    /// use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
    ///
    /// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let browser = Browser::launch(LaunchOptions::default()).await?;
    /// let page = browser.default_page().await?;
    /// let mut plugins = Plugins::new().with(ScreenshotOnFailurePlugin::new("latest_logs"));
    ///
    /// plugins.started("checkout").await;
    /// let outcome: Result<(), seleniumbase_rs::SeleniumBaseError> = async {
    ///     page.goto("https://example.com").await?;
    ///     page.locator("#buy").click().await
    /// }
    /// .await;
    /// if let Err(error) = &outcome {
    ///     plugins.failed("checkout", error, &page).await; // screenshot taken now
    /// }
    /// plugins.finished("checkout", Duration::ZERO, &outcome).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn failed(&mut self, test: &str, error: &SeleniumBaseError, page: &dyn PageEvidence) {
        for plugin in &mut self.plugins {
            let result = plugin.test_failed(test, error, page).await;
            log_failure(plugin.name(), "test_failed", test, result);
        }
    }

    /// Tells every plugin that `test` is over and how it went.
    pub async fn finished(&mut self, test: &str, elapsed: Duration, outcome: &Result<()>) {
        for plugin in &mut self.plugins {
            let result = plugin.test_finished(test, elapsed, outcome).await;
            log_failure(plugin.name(), "test_finished", test, result);
        }
    }

    /// Runs a browser test like [`run_browser_test`](crate::run_browser_test),
    /// telling the plugins about it.
    ///
    /// The plugins see the page on failure before the browser is closed. The
    /// returned result is the test's own, combined with any error from closing
    /// the browser, exactly as `run_browser_test` returns it. A test that
    /// panics is a failure for the plugins too; the panic is raised again once
    /// they and the browser are done.
    ///
    /// # Errors
    ///
    /// Returns the test's error, the browser's launch or cleanup error, or both
    /// combined into [`SeleniumBaseError::TestLifecycle`].
    pub async fn run_browser_test<F>(
        &mut self,
        name: &str,
        config: BrowserConfig,
        test: F,
    ) -> Result<()>
    where
        F: AsyncFnOnce(&mut BaseCase) -> Result<()>,
    {
        self.started(name).await;
        let began = Instant::now();
        let mut sb = match start_browser(config).await {
            Ok(sb) => sb,
            Err(error) => {
                let outcome = Err(error);
                self.finished(name, began.elapsed(), &outcome).await;
                return outcome;
            }
        };
        let ended = drive(
            &mut sb,
            test,
            async |sb: &BaseCase, error: &SeleniumBaseError| self.failed(name, error, sb).await,
            close_browser,
        )
        .await;
        self.finished(name, began.elapsed(), ended.outcome()).await;
        ended.finish()
    }

    /// [`run_browser_test`](Self::run_browser_test) for a test written as a
    /// closure that returns a boxed future, the form it took before async
    /// closures. Migration: `|sb| Box::pin(async move { .. })` becomes
    /// `async |sb| { .. }`.
    ///
    /// # Errors
    ///
    /// As [`run_browser_test`](Self::run_browser_test).
    #[deprecated(
        since = "0.5.0",
        note = "use run_browser_test with an async closure: `async |sb| { .. }`"
    )]
    #[allow(deprecated, reason = "the old closure type is this function's bound")]
    pub async fn run_browser_test_boxed<F>(
        &mut self,
        name: &str,
        config: BrowserConfig,
        test: F,
    ) -> Result<()>
    where
        F: for<'a> FnOnce(&'a mut BaseCase) -> BrowserTestFuture<'a>,
    {
        self.run_browser_test(name, config, async move |sb: &mut BaseCase| test(sb).await)
            .await
    }

    /// Runs a test on the first tab of a Pure CDP browser, like
    /// [`run_cdp_test`](crate::api::runner::run_cdp_test), telling the
    /// plugins about it. The plugins see the tab on failure before the
    /// browser is closed.
    ///
    /// # Errors
    ///
    /// Returns the test's error, the browser's launch or close error, or both
    /// combined into [`SeleniumBaseError::TestLifecycle`].
    pub async fn run_cdp_test<F>(
        &mut self,
        name: &str,
        options: LaunchOptions,
        test: F,
    ) -> Result<()>
    where
        F: AsyncFnOnce(&Page) -> Result<()>,
    {
        self.started(name).await;
        let began = Instant::now();
        let browser = match Browser::launch(options).await {
            Ok(browser) => browser,
            Err(error) => {
                let outcome = Err(error);
                self.finished(name, began.elapsed(), &outcome).await;
                return outcome;
            }
        };
        self.run_cdp_test_on(name, began, browser, test).await
    }

    /// [`run_cdp_test`](Self::run_cdp_test) on a browser that is already
    /// running (or mocked).
    pub(crate) async fn run_cdp_test_on<F>(
        &mut self,
        name: &str,
        began: Instant,
        browser: Browser,
        test: F,
    ) -> Result<()>
    where
        F: AsyncFnOnce(&Page) -> Result<()>,
    {
        let mut case = match CdpCase::open(browser).await {
            Ok(case) => case,
            Err(error) => {
                let outcome = Err(error);
                self.finished(name, began.elapsed(), &outcome).await;
                return outcome;
            }
        };
        let ended = drive(
            &mut case,
            async |case: &mut CdpCase| test(&case.page).await,
            async |case: &CdpCase, error: &SeleniumBaseError| {
                self.failed(name, error, &case.page).await;
            },
            CdpCase::close,
        )
        .await;
        self.finished(name, began.elapsed(), ended.outcome()).await;
        ended.finish()
    }
}

fn log_failure(plugin: &str, hook: &str, test: &str, result: Result<()>) {
    if let Err(error) = result {
        tracing::warn!(plugin, hook, test, %error, "a test plugin failed; the test result is unchanged");
    }
}

/// A file name made from a test name: letters, digits, `-` and `_` only, so a
/// test called `../../etc/passwd` cannot write outside the plugin's directory.
pub(crate) fn file_stem(test: &str) -> String {
    crate::artifacts::safe_stem(test, "test")
}

/// Writes `bytes` to a new file in `dir` named after `test`, never replacing a
/// file that exists, and returns its path.
pub(crate) async fn write_evidence(
    dir: &std::path::Path,
    test: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<std::path::PathBuf> {
    let stem = format!(
        "{}_{}",
        file_stem(test),
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    crate::artifacts::write_new_file(dir, &stem, extension, bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_name_becomes_a_safe_file_stem() {
        assert_eq!(file_stem("login_works"), "login_works");
        assert_eq!(file_stem("tests::login works!"), "tests__login_works_");
        assert_eq!(file_stem("../../etc/passwd"), "______etc_passwd");
        assert_eq!(file_stem(""), "test");
        assert_eq!(file_stem(&"x".repeat(500)).len(), 80);
        assert!(!file_stem("a/b\\c:d").contains(['/', '\\', ':']));
    }

    /// A plugin that writes down which hooks ran.
    #[derive(Clone, Default)]
    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

    impl Recorder {
        fn note(&self, line: String) {
            self.0.lock().unwrap().push(line);
        }

        fn lines(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl TestPlugin for Recorder {
        async fn test_started(&mut self, test: &str) -> Result<()> {
            self.note(format!("started {test}"));
            Ok(())
        }

        async fn test_failed(
            &mut self,
            test: &str,
            _error: &SeleniumBaseError,
            _page: &dyn PageEvidence,
        ) -> Result<()> {
            self.note(format!("failed {test}"));
            Ok(())
        }

        async fn test_finished(
            &mut self,
            test: &str,
            _elapsed: Duration,
            outcome: &Result<()>,
        ) -> Result<()> {
            self.note(format!("finished {test} ok={}", outcome.is_ok()));
            Ok(())
        }
    }

    async fn cdp_run<F>(recorder: &Recorder, test: F) -> Result<()>
    where
        F: AsyncFnOnce(&Page) -> Result<()>,
    {
        let (browser, _mock) = Browser::new_mocked();
        Plugins::new()
            .with(recorder.clone())
            .run_cdp_test_on("t", Instant::now(), browser, test)
            .await
    }

    #[tokio::test]
    async fn a_passing_cdp_test_is_started_and_finished() {
        let recorder = Recorder::default();
        let outcome = cdp_run(&recorder, async |_: &Page| Ok(())).await;
        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(recorder.lines(), ["finished t ok=true"]);
    }

    #[tokio::test]
    async fn a_failing_cdp_test_shows_the_plugins_its_page_first() {
        let recorder = Recorder::default();
        let outcome = cdp_run(&recorder, async |_: &Page| {
            Err(SeleniumBaseError::AssertionFailed("boom".to_owned()))
        })
        .await;
        assert!(matches!(
            outcome,
            Err(SeleniumBaseError::AssertionFailed(_))
        ));
        assert_eq!(recorder.lines(), ["failed t", "finished t ok=false"]);
    }

    #[tokio::test]
    async fn a_skipped_cdp_test_is_not_a_failure_for_the_plugins() {
        let recorder = Recorder::default();
        let outcome = cdp_run(&recorder, async |_: &Page| {
            Err(SeleniumBaseError::Skipped("later".to_owned()))
        })
        .await;
        assert!(outcome.is_ok());
        assert_eq!(recorder.lines(), ["finished t ok=true"]);
    }

    #[tokio::test]
    async fn a_panicking_cdp_test_is_reported_and_then_panics_again() {
        use futures_util::FutureExt;
        let recorder = Recorder::default();
        let raised = std::panic::AssertUnwindSafe(cdp_run(&recorder, async |_: &Page| {
            panic!("assert failed in the test")
        }))
        .catch_unwind()
        .await;
        assert!(raised.is_err(), "the panic reaches the caller");
        assert_eq!(recorder.lines(), ["failed t", "finished t ok=false"]);
    }

    #[tokio::test]
    async fn evidence_files_never_replace_each_other() {
        let dir = tempfile::tempdir().unwrap();

        let first = write_evidence(dir.path(), "t", "png", b"one")
            .await
            .unwrap();
        let second = write_evidence(dir.path(), "t", "png", b"two")
            .await
            .unwrap();

        assert_ne!(first, second);
        assert_eq!(std::fs::read(first).unwrap(), b"one");
        assert_eq!(std::fs::read(second).unwrap(), b"two");
    }

    #[tokio::test]
    async fn the_directory_is_created_when_it_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b");

        let path = write_evidence(&nested, "t", "html", b"x").await.unwrap();

        assert!(path.starts_with(&nested));
    }
}
