//! The test hooks of the plugins (feature `plugins`): what a plugin hears when a test starts and
//! ends.
//!
//! The test lifecycle (`Sb` and `SbCdp`, feature `testing`) calls [`test_start`] once per attempt
//! of a test, after the browser is up and before the body runs, and [`test_end`] once per
//! attempt, after the body and before the browser closes, so the plugins see the page the test
//! left. Only plugins that hold
//! [`Capability::TestHooks`](seleniumbase_rs_plugin::Capability::TestHooks) are called, which the
//! WebDriver engine does not offer: the hooks run for tests on the pure CDP engine.
//!
//! A hook that fails is logged and never changes how a test ends. The one thing a plugin can
//! change is the verdict of a test that passed: [`TestVerdict::Fail`] makes it fail with the
//! plugins' messages, joined with `"; "`.

use seleniumbase_rs_plugin::{PageHandle, PluginSet, TestInfo, TestOutcome, TestVerdict};

/// Tells the plugins of `set` that `test` starts. A plugin whose hook fails is logged, and the
/// others are still told.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::plugin_api::{PluginSet, TestInfo};
/// use seleniumbase_rs::plugin_host::test_start;
///
/// # tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
/// test_start(&PluginSet::new(), &TestInfo::new("cart::pays")).await;
/// # });
/// ```
pub async fn test_start(set: &PluginSet, test: &TestInfo) {
    for error in set.on_test_start(test).await {
        tracing::event!(
            name: "plugin.test_start.failed",
            tracing::Level::WARN,
            test = %test.name,
            error = %error,
            "a plugin failed when the test {{test}} started: {{error}}",
        );
    }
}

/// Asks the plugins of `set` what they make of the finished `test`, while its `page` is still
/// open: [`TestVerdict::Fail`] with the messages of the plugins that fail it, joined with `"; "`,
/// or [`TestVerdict::Keep`]. A plugin whose hook fails is logged and has no say.
///
/// `outcome` is how the test ended before the plugins looked at it. The caller decides what a
/// verdict is worth: the test lifecycle applies [`TestVerdict::Fail`] to a test that passed.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::plugin_api::{PluginSet, TestInfo, TestOutcome, TestVerdict};
/// use seleniumbase_rs::plugin_host::test_end;
///
/// # tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
/// let verdict = test_end(
///     &PluginSet::new(),
///     &TestInfo::new("cart::pays"),
///     &TestOutcome::Passed,
///     None,
/// )
/// .await;
/// assert_eq!(verdict, TestVerdict::Keep);
/// # });
/// ```
pub async fn test_end(
    set: &PluginSet,
    test: &TestInfo,
    outcome: &TestOutcome,
    page: Option<&dyn PageHandle>,
) -> TestVerdict {
    let (verdict, failures) = set.on_test_end(test, outcome, page).await;
    for error in failures {
        tracing::event!(
            name: "plugin.test_end.failed",
            tracing::Level::WARN,
            test = %test.name,
            error = %error,
            "a plugin failed when the test {{test}} ended: {{error}}",
        );
    }
    verdict
}
