//! The lifecycle of one test, from the run configuration to the closed browser.
//!
//! [`Lifecycle::run`] is what `Sb::run` and `SbCdp::run` do for every test. It is
//! generic over a [`TestCase`] (the open browser of one attempt), so the tests
//! of this module drive a fake one. Per attempt:
//!
//! 1. launch the case; a launch error is a failed attempt;
//! 2. [`TestCase::prepare`], then the `--trace` prompt;
//! 3. run the body, raced against the hard time limit
//!    ([`Deadline`](super::deadline)): on expiry the body is dropped and the
//!    attempt fails with Python's message;
//! 4. on a failure that is not a skip: the evidence in `latest_logs/<test id>/`
//!    ([`evidence`](super::evidence)), `=> Fail Page: <url>` for
//!    `--list-fail-page`, and on the last attempt the `--pdb` prompt;
//! 5. `--screenshot` for a pass, the `--final-debug` prompt, the
//!    [`Finalizer`]s, then the browser is closed;
//! 6. a failed attempt with attempts left prints `RERUN <test id> (attempt k of n)
//!    after: <error>`, waits `reruns_delay` and goes back to 1. A skip is never
//!    rerun, and a panic counts as a failure: the last one is raised again.
//!
//! # Seams for other parts of the crate
//!
//! The run recording of the reports, the plugin host's `test_end` and the
//! recorder's output belong to later lanes; they plug in here and nowhere else:
//! [`Finalizer`] sees every finished attempt with the page still open (register
//! it with `Sb::finalizer`), and [`TestCase::prepare`] runs before the body
//! (the recorder installs itself there when [`Plan::recorder`] is set).

use std::cell::Cell;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
#[cfg(feature = "plugins")]
use seleniumbase_rs_plugin::{PageHandle, PluginSet, TestInfo, TestOutcome, TestVerdict};
use tokio::time::Instant;

use super::deadline::{exceeded_message, Deadline};
use super::evidence::{self, Subject};
use crate::api::runner::{drive_with, Ended};
use crate::artifacts::{logs_root, LATEST_LOGS_DIR};
use crate::common::debug_prompt::{
    DebugPrompt, Failure, PromptKind, PromptOutcome, PromptPage, Scene, SourceSpot,
};
use crate::config::settings::Settings;
use crate::error::{Result, SeleniumBaseError};

/// How one attempt of a test ended, as a [`Finalizer`] sees it.
#[derive(Debug)]
#[non_exhaustive]
pub enum Outcome<'a> {
    /// The test passed.
    Passed,
    /// The test failed: an error, a panic or a time limit that ran out.
    Failed(&'a SeleniumBaseError),
    /// The test skipped itself; the error is the skip and holds its reason.
    Skipped(&'a SeleniumBaseError),
}

/// A finished attempt of a test, handed to every [`Finalizer`] before the
/// browser is closed.
#[derive(Debug)]
#[non_exhaustive]
pub struct AttemptEnd<'a> {
    /// The id of the test (`cart.checkout.renders`).
    pub test_id: &'a str,
    /// Which attempt this was, from 1.
    pub attempt: u32,
    /// How many attempts the test may have had: its reruns and the first run.
    pub attempts: u32,
    /// How it ended.
    pub outcome: Outcome<'a>,
    /// How long the attempt ran, a time spent at a prompt included.
    pub duration: Duration,
}

/// Something that wants to know how every attempt of a test ended, with the
/// page still open: the dashboard line, the results database, the recorder's
/// output.
///
/// A finalizer never changes how the test ends and has no error to return:
/// log what goes wrong. Register one with `Sb::finalizer` or
/// `SbCdp::finalizer`.
///
/// Per test a finalizer hears [`begin`](Finalizer::begin) once, then
/// [`finish`](Finalizer::finish) for every attempt, or
/// [`launch_failed`](Finalizer::launch_failed) for an attempt whose browser
/// did not start. An attempt that failed and will be run again has
/// `attempt < attempts`; the last attempt is the test's result.

#[async_trait]
pub trait Finalizer: Send + Sync {
    /// Called once per test, before its first attempt starts a browser: the
    /// run shows the test as running from here on.
    async fn begin(&self, _test_id: &str) {}

    /// Called once per attempt, after the evidence and before the browser closes.
    async fn finish(&self, end: &AttemptEnd<'_>, page: &dyn PromptPage);

    /// Called instead of [`finish`](Self::finish) for an attempt whose browser
    /// did not start, so there is no page to look at. `end.outcome` is the
    /// launch error as a failure.
    async fn launch_failed(&self, _end: &AttemptEnd<'_>) {}
}

/// The run configuration of one test, read once from the settings.
#[derive(Clone, Debug)]
pub(crate) struct Plan {
    /// The id of the test; it names the evidence folder.
    pub test_id: String,
    /// The name a report shows, and the first line of the info file; the id
    /// when the test has no other name.
    pub display: String,

    /// How many times a failed test is run again (`--reruns`).
    pub reruns: u32,
    /// How long to wait between attempts (`--reruns-delay`).
    pub reruns_delay: Duration,
    /// How long a test may run (`--time-limit`); `None` is no limit.
    pub time_limit: Option<Duration>,
    /// Prompt before the body (`--trace`).
    pub trace: bool,
    /// Prompt after a failed last attempt (`--pdb`).
    pub pdb: bool,
    /// Prompt after every test (`--final-debug`).
    pub final_debug: bool,
    /// Take a screenshot of a passing test too (`--screenshot`).
    pub save_screenshot: bool,
    /// Take no screenshot of a failure (`--no-screenshot`).
    pub no_screenshot: bool,
    /// Print `=> Fail Page: <url>` after a failure (`--list-fail-page`).
    pub list_fail_page: bool,
    /// Record the browser actions (`--rec`); the recorder installs itself in
    /// [`TestCase::prepare`].
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "read by the recorder's `TestCase::prepare` once it installs itself there"
        )
    )]
    pub recorder: bool,
    /// Where the folders of evidence go.
    pub logs_dir: PathBuf,
    /// The browser as configured, for the info file.
    pub browser: String,
    /// Whether the browser runs without a window.
    pub headless: bool,
    /// What drives the browser, for the info file.
    pub driver: &'static str,
    /// The `run` call that holds the test, for the source listing of a debug
    /// prompt. `None` when the test does not come through `Sb::run`.
    pub spot: Option<SourceSpot>,
}

impl Plan {
    /// The plan of the test `test_id` under `settings`; `driver` says what
    /// drives its browser.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a negative or
    /// non-finite `reruns_delay` or `time_limit`. A time limit of `0` is no
    /// limit, as in Python.
    pub fn from_settings(settings: &Settings, test_id: &str, driver: &'static str) -> Result<Self> {
        let test = &settings.test;
        let seconds = |name: &str, value: f64| {
            Duration::try_from_secs_f64(value).map_err(|_| {
                SeleniumBaseError::InvalidConfig(format!(
                    "{name}: expected a number of seconds, 0 or more, not {value}"
                ))
            })
        };
        Ok(Self {
            test_id: test_id.to_owned(),
            display: test_id.to_owned(),
            reruns: test.reruns,
            reruns_delay: seconds("reruns_delay", test.reruns_delay)?,
            time_limit: test
                .time_limit
                .map(|limit| seconds("time_limit", limit))
                .transpose()?
                .filter(|limit| !limit.is_zero()),
            trace: test.trace,
            pdb: test.pdb,
            final_debug: test.final_debug,
            save_screenshot: test.save_screenshot,
            no_screenshot: test.no_screenshot,
            list_fail_page: test.list_fail_page,
            recorder: test.recorder,
            logs_dir: logs_root().join(LATEST_LOGS_DIR),
            browser: settings.browser.clone(),
            headless: settings.headless || settings.headless1,
            spot: None,
            driver,
        })
    }

    /// How many attempts a test may have: its reruns and the first run.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.reruns.saturating_add(1)
    }
}

/// The plugins of an attempt's browser, and its tab, for the test hooks.
#[cfg(feature = "plugins")]
pub(crate) struct TestPlugins<'a> {
    /// The plugins the browser was launched with.
    pub set: &'a PluginSet,
    /// The tab the test ran on; it is still open when the hooks run.
    pub page: &'a dyn PageHandle,
}

/// The open browser of one attempt, as the lifecycle needs it.
///
/// The case is also the page the evidence and the prompts look at.
pub(crate) trait TestCase: PromptPage + Send {
    /// Closes the browser (or hands a reused one back).
    async fn close(&mut self) -> Result<()>;

    /// Called after the launch and before the prompt and body of an attempt.
    async fn prepare(&mut self, _plan: &Plan) -> Result<()> {
        Ok(())
    }

    /// Tells the case which test it runs.
    fn name_test(&mut self, _test_id: &str) {}

    /// Gives the case the deadline of the attempt, so a prompt it opens can
    /// suspend it.
    fn attach_deadline(&mut self, _deadline: Arc<Deadline>) {}

    /// Counts `paused`, the time a person spent at a prompt, out of the
    /// case's own time limit.
    fn extend_clock(&mut self, _paused: Duration) {}

    /// The plugins of the case's browser and the tab they may look at, when it has plugins.
    ///
    /// The lifecycle tells them when the attempt starts and ends (the plugin API's test hooks).
    /// A case on an engine that cannot host plugins keeps the default, `None`.
    #[cfg(feature = "plugins")]
    fn test_plugins(&self) -> Option<TestPlugins<'_>> {
        None
    }
}

/// Asks a person what to do at a debug prompt.
#[async_trait]
pub(crate) trait Prompter: Send + Sync {
    /// Pauses with `page` open until the person says to go on.
    async fn pause(
        &self,
        kind: PromptKind,
        test_id: &str,
        message: &str,
        page: &dyn PromptPage,
        scene: &Scene,
    ) -> Result<PromptOutcome>;
}

/// The prompter that talks to the person at the terminal. Where nobody can
/// answer (CI, a pipe, nextest) it logs a warning and goes on, so a flag left
/// in a command never hangs a run.
#[derive(Debug)]
pub(crate) struct TerminalPrompter;

#[async_trait]
impl Prompter for TerminalPrompter {
    async fn pause(
        &self,
        kind: PromptKind,
        test_id: &str,
        message: &str,
        page: &dyn PromptPage,
        scene: &Scene,
    ) -> Result<PromptOutcome> {
        let Some(mut prompt) = DebugPrompt::from_terminal() else {
            tracing::warn!(
                    prompt = kind.label(),
                    "debug prompt skipped: standard input is not a terminal (run with --test-threads 1 and --nocapture in a terminal to pause)"
                );
            return Ok(PromptOutcome::Continue);
        };
        prompt.pause_in(kind, test_id, message, page, scene).await
    }
}

/// Writes `text` to standard error without the capture of the test harness, so
/// a rerun or a fail page shows even in a test that passes.
pub(crate) fn say_on_stderr(text: &str) {
    // A closed standard error is nothing to fail a test over.
    let _ = std::io::stderr().lock().write_all(text.as_bytes());
}

/// How an attempt ended before the lifecycle looked at it.
enum Finished {
    /// The browser did not start.
    Launch(SeleniumBaseError),
    /// The test ran and its browser was closed; the flag says a person asked
    /// at the post-mortem prompt for another attempt.
    Ran(Ended, bool),
}

impl Finished {
    /// Whether a person asked at the post-mortem prompt to run the test again.
    fn retry(&self) -> bool {
        matches!(self, Self::Ran(_, true))
    }

    fn error(&self) -> Option<&SeleniumBaseError> {
        match self {
            Self::Launch(error) => Some(error),
            Self::Ran(ended, _) => ended.outcome().as_ref().err(),
        }
    }
}

/// Runs a test through its attempts. See the [module documentation](self).
pub(crate) struct Lifecycle<'a> {
    pub(crate) plan: &'a Plan,
    pub(crate) prompter: &'a dyn Prompter,
    pub(crate) finalizers: &'a [Arc<dyn Finalizer>],
    pub(crate) say: &'a (dyn Fn(&str) + Sync),
}

impl Lifecycle<'_> {
    /// Launches a case with `launch`, runs `body` on it, and does everything
    /// the [module documentation](self) lists, as often as the plan says.
    ///
    /// # Errors
    ///
    /// Returns the last attempt's error: the test's, the launch's, the time
    /// limit's, or the cleanup's. A panic of the last attempt is raised again.
    pub(crate) async fn run<C, L, B>(&self, launch: L, body: B) -> Result<()>
    where
        C: TestCase,
        L: AsyncFn(u32) -> Result<C>,
        B: AsyncFn(&mut C) -> Result<()>,
    {
        for finalizer in self.finalizers {
            finalizer.begin(&self.plan.test_id).await;
        }

        let mut attempts = self.plan.attempts();
        let mut attempt = 1;
        loop {
            let finished = Box::pin(self.attempt(attempt, &launch, &body)).await;
            if let (Some(error), true) = (finished.error(), attempt < attempts || finished.retry())
            {
                attempt += 1;
                // A retry asked for at the post-mortem prompt goes past the reruns.
                attempts = attempts.max(attempt);
                (self.say)(&format!(
                    "RERUN {} (attempt {attempt} of {attempts}) after: {error}\n",
                    self.plan.test_id
                ));
                // A panic of this attempt is dropped: the next one may pass.
                drop(finished);
                if !self.plan.reruns_delay.is_zero() {
                    tokio::time::sleep(self.plan.reruns_delay).await;
                }
                continue;
            }
            return match finished {
                Finished::Launch(error) => Err(error),
                Finished::Ran(ended, _) => ended.finish(),
            };
        }
    }

    async fn attempt<C, L, B>(&self, attempt: u32, launch: &L, body: &B) -> Finished
    where
        C: TestCase,
        L: AsyncFn(u32) -> Result<C>,
        B: AsyncFn(&mut C) -> Result<()>,
    {
        let plan = self.plan;
        let started = Instant::now();
        let mut case = match Box::pin(launch(attempt)).await {
            Ok(case) => case,
            Err(error) => {
                {
                    let end = AttemptEnd {
                        test_id: &plan.test_id,
                        attempt,
                        attempts: plan.attempts().max(attempt),
                        outcome: Outcome::Failed(&error),
                        duration: started.elapsed(),
                    };
                    for finalizer in self.finalizers {
                        finalizer.launch_failed(&end).await;
                    }
                }
                return Finished::Launch(error);
            }
        };
        case.name_test(&plan.test_id);
        let deadline = Deadline::new(plan.time_limit);
        case.attach_deadline(Arc::clone(&deadline));
        let last = attempt >= plan.attempts();
        // Set when a person types `r` at the post-mortem prompt.
        let retry = Cell::new(false);
        let ended = drive_with(
            &mut case,
            async |case: &mut C| {
                case.prepare(plan).await?;
                #[cfg(feature = "plugins")]
                self.plugins_test_start(&*case).await;

                if plan.trace {
                    self.pause_before_body(case, &deadline).await?;
                }
                // The body is boxed so that its future is not part of this one: a
                // long test otherwise makes rustc overflow its query depth limit
                // when it lays out the test's future.
                tokio::select! {
                    biased;
                    result = Box::pin(body(case)) => result,
                    () = deadline.expired() => Err(SeleniumBaseError::test_lifecycle(
                        exceeded_message(deadline.limit().unwrap_or_default()),
                    )),
                }
            },
            async |case: &C, error: &SeleniumBaseError| {
                self.take_failure(case, error, started, last, &retry).await;
            },
            async |case: &mut C, ended: &Result<()>| {
                self.finish(case, ended, started, attempt, &retry).await
            },
        )
        .await;
        Finished::Ran(ended, retry.get())
    }

    fn subject(&self, started: Instant) -> Subject<'_> {
        Subject {
            test_id: &self.plan.test_id,
            display: &self.plan.display,

            browser: &self.plan.browser,
            headless: self.plan.headless,
            driver: self.plan.driver,
            duration: started.elapsed(),
        }
    }

    /// The `--trace` prompt. The time spent there counts neither against the
    /// hard limit nor against the case's own.
    async fn pause_before_body<C: TestCase>(
        &self,
        case: &mut C,
        deadline: &Arc<Deadline>,
    ) -> Result<()> {
        let hold = deadline.suspend();
        let paused_at = Instant::now();
        let outcome = self
            .ask(
                &*case,
                PromptKind::Trace,
                "About to run the test body.",
                None,
            )
            .await;
        drop(hold);
        case.extend_clock(paused_at.elapsed());
        match outcome? {
            PromptOutcome::Continue | PromptOutcome::Retry => Ok(()),
            PromptOutcome::Quit => Err(SeleniumBaseError::test_lifecycle(
                "stopped at the debug prompt",
            )),
        }
    }

    async fn ask(
        &self,
        page: &dyn PromptPage,
        kind: PromptKind,
        message: &str,
        failure: Option<&SeleniumBaseError>,
    ) -> Result<PromptOutcome> {
        let mut scene = self.plan.spot.clone().map(Scene::at).unwrap_or_default();
        if let Some(error) = failure {
            scene = scene.with_failure(Failure::from_error(error));
        }
        self.prompter
            .pause(kind, &self.plan.test_id, message, page, &scene)
            .await
    }

    /// The evidence of a failed attempt, the fail page and the `--pdb` prompt.
    async fn take_failure<C: TestCase>(
        &self,
        case: &C,
        error: &SeleniumBaseError,
        started: Instant,
        last: bool,
        retry: &Cell<bool>,
    ) {
        let plan = self.plan;
        let taken = evidence::take_failure(
            &plan.logs_dir,
            &self.subject(started),
            case,
            error,
            !plan.no_screenshot,
        )
        .await;
        if let (true, Some(page)) = (plan.list_fail_page, &taken.last_page) {
            (self.say)(&format!("\n=> Fail Page: {page}\n"));
        }
        if plan.pdb && last {
            match self
                .ask(
                    case,
                    PromptKind::PostMortem,
                    "The test failed.",
                    Some(error),
                )
                .await
            {
                Ok(PromptOutcome::Retry) => retry.set(true),
                Ok(_) => {}
                Err(error) => tracing::warn!(%error, "the post-mortem prompt failed"),
            }
        }
    }

    /// What happens to every attempt before its browser closes, and the close.
    async fn finish<C: TestCase>(
        &self,
        case: &mut C,
        ended: &Result<()>,
        started: Instant,
        attempt: u32,
        retry: &Cell<bool>,
    ) -> Result<()> {
        let plan = self.plan;
        // The plugins have their say on a test that passed, while the page is still open. A
        // verdict that fails it makes it a failure like any other: evidence, the report line
        // and the rerun all see it.
        #[cfg(feature = "plugins")]
        let plugin_failure = self.plugin_verdict(&*case, ended).await;
        #[cfg(not(feature = "plugins"))]
        let plugin_failure: Option<SeleniumBaseError> = None;
        if let Some(error) = &plugin_failure {
            self.take_failure(&*case, error, started, attempt >= plan.attempts(), retry)
                .await;
        }
        {
            let outcome = match (&plugin_failure, ended) {
                (Some(error), _) => Outcome::Failed(error),
                (None, Ok(())) => Outcome::Passed,
                (None, Err(error)) if error.is_skipped() => Outcome::Skipped(error),
                (None, Err(error)) => Outcome::Failed(error),
            };
            if matches!(outcome, Outcome::Passed) && plan.save_screenshot && !plan.no_screenshot {
                evidence::take_pass(&plan.logs_dir, &self.subject(started), &*case).await;
            }
            if plan.final_debug {
                let message = "The test is over; the browser is still open.";
                if let Err(error) = self
                    .ask(&*case, PromptKind::FinalTrace, message, None)
                    .await
                {
                    tracing::warn!(%error, "the final prompt failed");
                }
            }
            let end = AttemptEnd {
                test_id: &plan.test_id,
                attempt,
                // A retry asked for at the prompt makes one more attempt than the plan allows.
                attempts: plan.attempts().max(attempt + u32::from(retry.get())),
                outcome,
                duration: started.elapsed(),
            };
            for finalizer in self.finalizers {
                finalizer.finish(&end, &*case).await;
            }
        }
        let closed = case.close().await;
        match plugin_failure {
            None => closed,
            Some(failure) => Err(match closed {
                Ok(()) => failure,
                Err(cleanup) => SeleniumBaseError::test_lifecycle(format!(
                    "{failure}; cleanup failed with '{cleanup}'"
                )),
            }),
        }
    }

    /// The test as the plugins name it.
    #[cfg(feature = "plugins")]
    fn test_info(&self) -> TestInfo {
        TestInfo::new(self.plan.display.clone())
    }

    /// Tells the plugins of the case's browser that the test starts.
    #[cfg(feature = "plugins")]
    async fn plugins_test_start<C: TestCase>(&self, case: &C) {
        if let Some(plugins) = case.test_plugins() {
            crate::plugin_host::test_start(plugins.set, &self.test_info()).await;
        }
    }

    /// Asks the plugins of the case's browser what they make of the finished attempt, with the
    /// page still open. The failure that turns a passed attempt into a failed one, when a plugin
    /// says so; a verdict on an attempt that already failed or skipped is only logged, because
    /// that attempt's own error is the better answer.
    #[cfg(feature = "plugins")]
    async fn plugin_verdict<C: TestCase>(
        &self,
        case: &C,
        ended: &Result<()>,
    ) -> Option<SeleniumBaseError> {
        let plugins = case.test_plugins()?;
        let outcome = match ended {
            Ok(()) => TestOutcome::Passed,
            Err(error) if error.is_skipped() => TestOutcome::Skipped,
            Err(error) => TestOutcome::Failed(error.to_string()),
        };
        let verdict = crate::plugin_host::test_end(
            plugins.set,
            &self.test_info(),
            &outcome,
            Some(plugins.page),
        )
        .await;
        match (verdict, ended) {
            (TestVerdict::Fail(message), Ok(())) => Some(SeleniumBaseError::AssertionFailed(
                format!("a plugin failed the test: {message}"),
            )),
            (TestVerdict::Fail(message), Err(_)) => {
                tracing::info!(
                    test = %self.plan.test_id,
                    %message,
                    "a plugin would fail the test, which did not pass anyway"
                );
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::panic::AssertUnwindSafe;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    use futures_util::FutureExt;
    use serde_json::{json, Value};

    use super::*;
    use crate::config::test_settings::TestSettings;
    use crate::core::log_helper::{BASIC_INFO_FILE, PAGE_SOURCE_FILE, SCREENSHOT_FILE};

    /// What happened, in order, across the cases of one test.
    #[derive(Default)]
    struct Events(Mutex<Vec<String>>);

    impl Events {
        fn push(&self, event: impl Into<String>) {
            self.0.lock().unwrap().push(event.into());
        }
        fn all(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
    }

    struct FakeCase {
        events: Arc<Events>,
        deadline: Option<Arc<Deadline>>,
        close_fails: bool,
    }

    #[async_trait]
    impl PromptPage for FakeCase {
        async fn url(&self) -> Result<String> {
            Ok("https://fake.test/page".to_owned())
        }
        async fn title(&self) -> Result<String> {
            Ok("Fake".to_owned())
        }
        async fn screenshot_png(&self) -> Result<Vec<u8>> {
            Ok(b"PNG".to_vec())
        }
        async fn page_source(&self) -> Result<String> {
            Ok("<html>fake</html>".to_owned())
        }
        async fn evaluate(&self, _expression: &str) -> Result<Value> {
            Ok(json!("Mozilla/5.0 Chrome/1.2.3"))
        }
    }

    impl TestCase for FakeCase {
        async fn close(&mut self) -> Result<()> {
            self.events.push("close");
            if self.close_fails {
                return Err(SeleniumBaseError::test_lifecycle(
                    "the browser would not quit",
                ));
            }
            Ok(())
        }
        async fn prepare(&mut self, plan: &Plan) -> Result<()> {
            self.events
                .push(format!("prepare recorder={}", plan.recorder));
            Ok(())
        }
        fn attach_deadline(&mut self, deadline: Arc<Deadline>) {
            self.deadline = Some(deadline);
        }
        fn extend_clock(&mut self, paused: Duration) {
            self.events.push(format!("clock +{}s", paused.as_secs()));
        }
    }

    /// Records the prompts it is asked for and answers `Quit` for one kind.
    struct ScriptedPrompter {
        asked: Mutex<Vec<PromptKind>>,
        quit_at: Option<PromptKind>,
        /// Asked to retry, the first time this kind of prompt comes.
        retry_at: Option<PromptKind>,
        /// The scene each prompt was shown.
        scenes: Mutex<Vec<(PromptKind, Scene)>>,
    }

    #[async_trait]
    impl Prompter for ScriptedPrompter {
        async fn pause(
            &self,
            kind: PromptKind,
            _test_id: &str,
            _message: &str,
            _page: &dyn PromptPage,
            scene: &Scene,
        ) -> Result<PromptOutcome> {
            let first = !self.asked.lock().unwrap().contains(&kind);
            self.asked.lock().unwrap().push(kind);
            self.scenes.lock().unwrap().push((kind, scene.clone()));
            // A person at the prompt takes five seconds.
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok(if self.quit_at == Some(kind) {
                PromptOutcome::Quit
            } else if self.retry_at == Some(kind) && first {
                PromptOutcome::Retry
            } else {
                PromptOutcome::Continue
            })
        }
    }

    /// Notes the outcome of every attempt it sees.
    struct Notes(Arc<Events>);

    #[async_trait]
    impl Finalizer for Notes {
        async fn finish(&self, end: &AttemptEnd<'_>, page: &dyn PromptPage) {
            let how = match end.outcome {
                Outcome::Passed => "passed",
                Outcome::Failed(_) => "failed",
                Outcome::Skipped(_) => "skipped",
            };
            let url = page.url().await.unwrap_or_default();
            self.0.push(format!(
                "finalizer {} {}/{} {how} {url}",
                end.test_id, end.attempt, end.attempts
            ));
        }
    }

    /// One test run through a [`Lifecycle`] with fakes.
    struct Rig {
        plan: Plan,
        events: Arc<Events>,
        said: Mutex<String>,
        prompter: ScriptedPrompter,
        finalizers: Vec<Arc<dyn Finalizer>>,
        close_fails: bool,
        launch_fails_first: bool,
        logs: tempfile::TempDir,
    }

    impl Rig {
        fn new() -> Self {
            let events = Arc::new(Events::default());
            let logs = tempfile::tempdir().unwrap();
            Self {
                plan: Plan {
                    test_id: "fake.test".to_owned(),
                    display: "fake::test".to_owned(),

                    reruns: 0,
                    reruns_delay: Duration::ZERO,
                    time_limit: None,
                    trace: false,
                    pdb: false,
                    final_debug: false,
                    save_screenshot: false,
                    no_screenshot: false,
                    list_fail_page: false,
                    recorder: false,
                    logs_dir: logs.path().to_owned(),
                    browser: "chrome".to_owned(),
                    headless: true,
                    driver: "Fake",
                    spot: None,
                },
                events: Arc::clone(&events),
                said: Mutex::new(String::new()),
                prompter: ScriptedPrompter {
                    asked: Mutex::new(Vec::new()),
                    quit_at: None,
                    retry_at: None,
                    scenes: Mutex::new(Vec::new()),
                },
                finalizers: vec![Arc::new(Notes(events)) as Arc<dyn Finalizer>],
                close_fails: false,
                launch_fails_first: false,
                logs,
            }
        }

        async fn run<B>(&self, body: B) -> Result<()>
        where
            B: AsyncFn(&mut FakeCase) -> Result<()>,
        {
            let say = |text: &str| self.said.lock().unwrap().push_str(text);
            let lifecycle = Lifecycle {
                plan: &self.plan,
                prompter: &self.prompter,
                finalizers: &self.finalizers,
                say: &say,
            };
            lifecycle
                .run(
                    async |attempt: u32| {
                        self.events.push(format!("launch {attempt}"));
                        if self.launch_fails_first && attempt == 1 {
                            return Err(SeleniumBaseError::BrowserLaunch {
                                binary: "chrome".to_owned(),
                                reason: "no display".to_owned(),
                            });
                        }
                        Ok(FakeCase {
                            events: Arc::clone(&self.events),
                            deadline: None,
                            close_fails: self.close_fails,
                        })
                    },
                    body,
                )
                .await
        }

        fn said(&self) -> String {
            self.said.lock().unwrap().clone()
        }

        fn prompts(&self) -> Vec<PromptKind> {
            self.prompter.asked.lock().unwrap().clone()
        }

        fn folder(&self) -> std::path::PathBuf {
            self.logs.path().join("fake.test")
        }

        fn files(&self) -> HashSet<String> {
            std::fs::read_dir(self.folder())
                .map(|entries| {
                    entries
                        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    fn fails(message: &str) -> Result<()> {
        Err(SeleniumBaseError::AssertionFailed(message.to_owned()))
    }

    fn names(files: &[&str]) -> HashSet<String> {
        files.iter().map(|name| (*name).to_owned()).collect()
    }

    #[tokio::test]
    async fn a_passing_test_runs_once_and_leaves_nothing() {
        let rig = Rig::new();
        let result = rig
            .run(async |case: &mut FakeCase| {
                case.events.push("body");
                Ok(())
            })
            .await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(
            rig.events.all(),
            [
                "launch 1",
                "prepare recorder=false",
                "body",
                "finalizer fake.test 1/1 passed https://fake.test/page",
                "close"
            ]
        );
        assert!(rig.files().is_empty());
        assert_eq!(rig.said(), "");
    }

    #[tokio::test]
    async fn a_failing_test_leaves_evidence_before_the_browser_closes() {
        let rig = Rig::new();
        let result = rig
            .run(async |_: &mut FakeCase| fails("the title is wrong"))
            .await;
        assert!(
            matches!(result, Err(SeleniumBaseError::AssertionFailed(_))),
            "{result:?}"
        );
        assert_eq!(
            rig.files(),
            names(&[SCREENSHOT_FILE, PAGE_SOURCE_FILE, BASIC_INFO_FILE])
        );
        let info = std::fs::read_to_string(rig.folder().join(BASIC_INFO_FILE)).unwrap();
        assert!(
            info.contains("Exception: assertion failed: the title is wrong"),
            "{info}"
        );
        assert!(info.contains("Last Page: https://fake.test/page"), "{info}");
        assert_eq!(rig.events.all().last().map(String::as_str), Some("close"));
    }

    #[tokio::test]
    async fn no_screenshot_leaves_the_other_files() {
        let mut rig = Rig::new();
        rig.plan.no_screenshot = true;
        let _ = rig.run(async |_: &mut FakeCase| fails("x")).await;
        assert_eq!(rig.files(), names(&[PAGE_SOURCE_FILE, BASIC_INFO_FILE]));
    }

    #[tokio::test]
    async fn a_screenshot_of_a_pass_is_taken_only_when_asked_for() {
        let mut rig = Rig::new();
        rig.run(async |_: &mut FakeCase| Ok(())).await.unwrap();
        assert!(rig.files().is_empty());
        rig.plan.save_screenshot = true;
        rig.run(async |_: &mut FakeCase| Ok(())).await.unwrap();
        assert_eq!(rig.files(), names(&[SCREENSHOT_FILE, PAGE_SOURCE_FILE]));
    }

    #[tokio::test]
    async fn no_screenshot_wins_over_save_screenshot() {
        let mut rig = Rig::new();
        rig.plan.save_screenshot = true;
        rig.plan.no_screenshot = true;
        rig.run(async |_: &mut FakeCase| Ok(())).await.unwrap();
        assert!(rig.files().is_empty());
    }

    #[tokio::test]
    async fn the_fail_page_is_printed_when_asked_for() {
        let mut rig = Rig::new();
        let _ = rig.run(async |_: &mut FakeCase| fails("x")).await;
        assert_eq!(rig.said(), "");
        rig.plan.list_fail_page = true;
        let _ = rig.run(async |_: &mut FakeCase| fails("x")).await;
        assert_eq!(rig.said(), "\n=> Fail Page: https://fake.test/page\n");
    }

    #[tokio::test]
    async fn a_panic_closes_the_browser_takes_evidence_and_is_raised_again() {
        let rig = Rig::new();
        let raised = AssertUnwindSafe(rig.run(async |_: &mut FakeCase| -> Result<()> {
            panic!("assertion failed in the test")
        }))
        .catch_unwind()
        .await;
        let payload = raised.expect_err("the panic is raised again");
        assert_eq!(
            payload.downcast_ref::<&str>().copied(),
            Some("assertion failed in the test")
        );
        assert_eq!(rig.events.all().last().map(String::as_str), Some("close"));
        assert!(rig.files().contains(BASIC_INFO_FILE));
    }

    #[tokio::test]
    async fn a_skip_is_ok_and_is_not_rerun_and_leaves_no_evidence() {
        let mut rig = Rig::new();
        rig.plan.reruns = 2;
        let result = rig
            .run(async |_: &mut FakeCase| Err(SeleniumBaseError::Skipped("no browser".to_owned())))
            .await;
        assert!(result.is_ok(), "{result:?}");
        let launches = rig
            .events
            .all()
            .iter()
            .filter(|e| e.starts_with("launch"))
            .count();
        assert_eq!(launches, 1);
        assert!(rig.files().is_empty());
        assert!(rig.events.all().iter().any(|e| e.contains("skipped")));
        assert_eq!(rig.said(), "");
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_test_is_run_again_after_the_delay() {
        let mut rig = Rig::new();
        rig.plan.reruns = 2;
        rig.plan.reruns_delay = Duration::from_millis(500);
        let runs = AtomicU32::new(0);
        let started = Instant::now();
        let result = rig
            .run(async |_: &mut FakeCase| {
                if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                    fails("flaky")
                } else {
                    Ok(())
                }
            })
            .await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(runs.load(Ordering::SeqCst), 2);
        assert_eq!(
            rig.said(),
            "RERUN fake.test (attempt 2 of 3) after: assertion failed: flaky\n"
        );
        assert_eq!(started.elapsed(), Duration::from_millis(500));
        let events = rig.events.all();
        assert_eq!(
            events.iter().filter(|e| *e == "close").count(),
            2,
            "{events:?}"
        );
        assert!(
            events.contains(&"finalizer fake.test 1/3 failed https://fake.test/page".to_owned())
        );
        assert!(
            events.contains(&"finalizer fake.test 2/3 passed https://fake.test/page".to_owned())
        );
    }

    #[tokio::test]
    async fn a_test_that_keeps_failing_returns_its_last_error() {
        let mut rig = Rig::new();
        rig.plan.reruns = 2;
        let runs = AtomicU32::new(0);
        let result = rig
            .run(async |_: &mut FakeCase| {
                fails(&format!("run {}", runs.fetch_add(1, Ordering::SeqCst)))
            })
            .await;
        let shown = result.unwrap_err().to_string();
        assert_eq!(shown, "assertion failed: run 2");
        assert_eq!(rig.said().matches("RERUN").count(), 2);
    }

    #[tokio::test]
    async fn a_panic_is_rerun_and_only_the_last_one_is_raised() {
        let mut rig = Rig::new();
        rig.plan.reruns = 1;
        let raised =
            AssertUnwindSafe(rig.run(async |_: &mut FakeCase| -> Result<()> { panic!("always") }))
                .catch_unwind()
                .await;
        assert!(raised.is_err());
        assert_eq!(rig.said().matches("RERUN").count(), 1);
    }

    #[tokio::test]
    async fn a_launch_error_is_a_failed_attempt() {
        let mut rig = Rig::new();
        rig.plan.reruns = 1;
        rig.launch_fails_first = true;
        let result = rig.run(async |_: &mut FakeCase| Ok(())).await;
        assert!(result.is_ok(), "{result:?}");
        assert!(rig.said().contains(
            "RERUN fake.test (attempt 2 of 2) after: failed to launch browser binary 'chrome'"
        ));
        assert_eq!(rig.events.all()[0], "launch 1");
    }

    /// Notes the start of a test and the attempts whose browser did not start.
    struct Starts(Arc<Events>);

    #[async_trait]
    impl Finalizer for Starts {
        async fn begin(&self, test_id: &str) {
            self.0.push(format!("begin {test_id}"));
        }
        async fn finish(&self, end: &AttemptEnd<'_>, _page: &dyn PromptPage) {
            self.0
                .push(format!("finish {}/{}", end.attempt, end.attempts));
        }
        async fn launch_failed(&self, end: &AttemptEnd<'_>) {
            let Outcome::Failed(error) = &end.outcome else {
                return;
            };
            self.0.push(format!(
                "launch failed {}/{}: {error}",
                end.attempt, end.attempts
            ));
        }
    }

    #[tokio::test]
    async fn a_finalizer_hears_the_start_once_and_every_launch_that_fails() {
        let mut rig = Rig::new();
        rig.plan.reruns = 1;
        rig.launch_fails_first = true;
        rig.finalizers = vec![Arc::new(Starts(Arc::clone(&rig.events))) as Arc<dyn Finalizer>];
        let result = rig.run(async |_: &mut FakeCase| Ok(())).await;
        assert!(result.is_ok(), "{result:?}");
        let events = rig.events.all();
        assert_eq!(events[0], "begin fake.test", "before the first launch");
        let heard: Vec<&String> = events
            .iter()
            .filter(|event| {
                event.starts_with("begin")
                    || event.starts_with("finish")
                    || event.starts_with("launch failed")
            })
            .collect();
        assert_eq!(heard.len(), 3, "{heard:?}");
        assert!(
            heard[1].starts_with("launch failed 1/2: failed to launch browser binary 'chrome'"),
            "{heard:?}"
        );
        assert_eq!(heard[2], "finish 2/2");
    }

    #[tokio::test]
    async fn a_failed_close_fails_a_passing_test() {
        let mut rig = Rig::new();
        rig.close_fails = true;
        let result = rig.run(async |_: &mut FakeCase| Ok(())).await;
        assert!(result.unwrap_err().to_string().contains("would not quit"));
    }

    #[tokio::test(start_paused = true)]
    async fn the_hard_time_limit_stops_a_test_that_never_waits_on_the_browser() {
        let mut rig = Rig::new();
        rig.plan.time_limit = Some(Duration::from_secs(1));
        let started = Instant::now();
        let result = rig
            .run(async |_: &mut FakeCase| {
                tokio::time::sleep(Duration::from_secs(3)).await;
                Ok(())
            })
            .await;
        assert!(result
            .unwrap_err()
            .to_string()
            .ends_with("This test has exceeded the time limit of 1 second!"));
        assert_eq!(started.elapsed(), Duration::from_secs(1));
        assert_eq!(rig.events.all().last().map(String::as_str), Some("close"));
        assert!(
            rig.files().contains(BASIC_INFO_FILE),
            "evidence of the timeout"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_test_that_beats_the_time_limit_passes() {
        let mut rig = Rig::new();
        rig.plan.time_limit = Some(Duration::from_secs(10));
        let result = rig
            .run(async |_: &mut FakeCase| {
                tokio::time::sleep(Duration::from_secs(3)).await;
                Ok(())
            })
            .await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn time_at_a_prompt_does_not_count_against_the_limit() {
        let mut rig = Rig::new();
        rig.plan.time_limit = Some(Duration::from_secs(1));
        let result = rig
            .run(async |case: &mut FakeCase| {
                let deadline = case.deadline.clone().expect("the case has the deadline");
                let hold = deadline.suspend();
                tokio::time::sleep(Duration::from_secs(60)).await;
                drop(hold);
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(())
            })
            .await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_limit_set_in_the_test_is_enforced_too() {
        let rig = Rig::new();
        let result = rig
            .run(async |case: &mut FakeCase| {
                let deadline = case.deadline.clone().unwrap();
                deadline.set_limit(Some(Duration::from_secs(2)));
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(())
            })
            .await;
        assert!(result
            .unwrap_err()
            .to_string()
            .ends_with("This test has exceeded the time limit of 2 seconds!"));
    }

    #[tokio::test(start_paused = true)]
    async fn the_debug_prompts_come_in_their_places() {
        let mut rig = Rig::new();
        rig.plan.trace = true;
        rig.plan.pdb = true;
        rig.plan.final_debug = true;
        rig.plan.time_limit = Some(Duration::from_secs(1));
        // The trace prompt takes five seconds of a one-second limit.
        let result = rig.run(async |_: &mut FakeCase| fails("x")).await;
        assert!(result.is_err());
        assert_eq!(
            rig.prompts(),
            [
                PromptKind::Trace,
                PromptKind::PostMortem,
                PromptKind::FinalTrace
            ]
        );
        assert!(rig.events.all().contains(&"clock +5s".to_owned()));
    }

    #[tokio::test(start_paused = true)]
    async fn a_pass_is_not_a_post_mortem() {
        let mut rig = Rig::new();
        rig.plan.pdb = true;
        rig.plan.final_debug = true;
        rig.run(async |_: &mut FakeCase| Ok(())).await.unwrap();
        assert_eq!(rig.prompts(), [PromptKind::FinalTrace]);
    }

    #[tokio::test(start_paused = true)]
    async fn the_post_mortem_is_only_for_the_last_attempt() {
        let mut rig = Rig::new();
        rig.plan.pdb = true;
        rig.plan.reruns = 1;
        let _ = rig.run(async |_: &mut FakeCase| fails("x")).await;
        assert_eq!(rig.prompts(), [PromptKind::PostMortem]);
    }

    #[tokio::test(start_paused = true)]
    async fn quitting_at_the_trace_prompt_fails_the_test_before_its_body() {
        let mut rig = Rig::new();
        rig.plan.trace = true;
        rig.prompter.quit_at = Some(PromptKind::Trace);
        let result = rig
            .run(async |case: &mut FakeCase| {
                case.events.push("body");
                Ok(())
            })
            .await;
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("stopped at the debug prompt"));
        assert!(!rig.events.all().contains(&"body".to_owned()));
        assert_eq!(rig.events.all().last().map(String::as_str), Some("close"));
    }

    #[tokio::test]
    async fn the_case_is_prepared_for_a_recorded_run() {
        let mut rig = Rig::new();
        rig.plan.recorder = true;
        rig.run(async |_: &mut FakeCase| Ok(())).await.unwrap();
        assert!(rig
            .events
            .all()
            .contains(&"prepare recorder=true".to_owned()));
    }

    #[tokio::test(start_paused = true)]
    async fn the_prompts_are_shown_the_run_call_and_the_post_mortem_the_failure() {
        let mut rig = Rig::new();
        rig.plan.trace = true;
        rig.plan.pdb = true;
        rig.plan.spot = Some(SourceSpot::new("tests/login.rs", 6, 5));

        let _ = rig
            .run(async |_: &mut FakeCase| {
                Err(SeleniumBaseError::ElementNotFound {
                    selector: "#go".to_owned(),
                    strategy: "css".to_owned(),
                })
            })
            .await;

        let scenes = rig.prompter.scenes.lock().unwrap();
        assert_eq!(scenes.len(), 2, "the trace and the post-mortem");
        let (trace_kind, trace) = &scenes[0];
        assert_eq!(*trace_kind, PromptKind::Trace);
        assert_eq!(trace.spot().map(SourceSpot::line), Some(6));
        assert!(trace.failure().is_none(), "nothing has failed yet");
        let (post_mortem_kind, post_mortem) = &scenes[1];
        assert_eq!(*post_mortem_kind, PromptKind::PostMortem);
        assert_eq!(post_mortem.spot().map(SourceSpot::line), Some(6));
        assert_eq!(
            post_mortem.failure().and_then(Failure::selector),
            Some("#go")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_test_that_does_not_come_through_run_has_no_place_to_list() {
        let mut rig = Rig::new();
        rig.plan.pdb = true;

        let _ = rig.run(async |_: &mut FakeCase| fails("x")).await;

        let scenes = rig.prompter.scenes.lock().unwrap();
        assert!(scenes[0].1.spot().is_none());
        assert!(scenes[0].1.failure().is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn r_at_the_post_mortem_prompt_runs_the_test_once_more_past_the_reruns() {
        let mut rig = Rig::new();
        rig.plan.pdb = true;
        rig.prompter.retry_at = Some(PromptKind::PostMortem);
        let runs = AtomicU32::new(0);

        let result = rig
            .run(async |_: &mut FakeCase| {
                if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                    fails("the first run")
                } else {
                    Ok(())
                }
            })
            .await;

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(runs.load(Ordering::SeqCst), 2);
        assert_eq!(
            rig.prompts(),
            [PromptKind::PostMortem],
            "the second run passed: no second post-mortem"
        );
        let said = rig.said.lock().unwrap().clone();
        assert!(
            said.starts_with("RERUN fake.test (attempt 2 of 2) after: "),
            "{said}"
        );
        let events = rig.events.all();
        assert!(
            events.contains(&"finalizer fake.test 1/2 failed https://fake.test/page".to_owned()),
            "the first attempt is not the last: {events:?}"
        );
        assert!(
            events.contains(&"finalizer fake.test 2/2 passed https://fake.test/page".to_owned()),
            "{events:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_retry_that_fails_again_is_a_post_mortem_again() {
        let mut rig = Rig::new();
        rig.plan.pdb = true;
        rig.prompter.retry_at = Some(PromptKind::PostMortem);

        let result = rig.run(async |_: &mut FakeCase| fails("still")).await;

        assert!(result.is_err());
        assert_eq!(
            rig.prompts(),
            [PromptKind::PostMortem, PromptKind::PostMortem],
            "the retry is the last attempt, so it ends at the prompt as well"
        );
    }

    fn settings_with(test: TestSettings) -> Settings {
        Settings {
            test,
            ..Settings::default()
        }
    }

    #[test]
    fn the_plan_reads_the_test_settings() {
        let settings = Settings {
            headless1: true,
            browser: "edge".to_owned(),
            test: TestSettings {
                reruns: 2,
                reruns_delay: 0.25,
                time_limit: Some(30.0),
                trace: true,
                pdb: true,
                final_debug: true,
                save_screenshot: true,
                no_screenshot: true,
                list_fail_page: true,
                recorder: true,
                ..TestSettings::default()
            },
            ..Settings::default()
        };
        let plan = Plan::from_settings(&settings, "a.b", "WebDriver").unwrap();
        assert_eq!(plan.test_id, "a.b");
        assert_eq!(plan.attempts(), 3);
        assert_eq!(plan.reruns_delay, Duration::from_millis(250));
        assert_eq!(plan.time_limit, Some(Duration::from_secs(30)));
        assert!(plan.trace && plan.pdb && plan.final_debug && plan.recorder);
        assert!(plan.save_screenshot && plan.no_screenshot && plan.list_fail_page);
        assert_eq!(plan.browser, "edge");
        assert!(plan.headless);
        assert_eq!(plan.logs_dir, logs_root().join(LATEST_LOGS_DIR));
        assert_eq!(plan.display, "a.b", "the id until the caller names it");
    }

    #[test]
    fn the_defaults_are_one_attempt_and_no_limit() {
        let plan = Plan::from_settings(&Settings::default(), "t", "WebDriver").unwrap();
        assert_eq!(plan.attempts(), 1);
        assert_eq!(plan.time_limit, None);
        assert_eq!(plan.reruns_delay, Duration::ZERO);
        assert!(!plan.trace && !plan.pdb && !plan.final_debug);
    }

    #[test]
    fn a_time_limit_of_zero_is_no_limit_and_a_negative_one_is_refused() {
        let zero = settings_with(TestSettings {
            time_limit: Some(0.0),
            ..TestSettings::default()
        });
        assert_eq!(
            Plan::from_settings(&zero, "t", "d").unwrap().time_limit,
            None
        );
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            let settings = settings_with(TestSettings {
                time_limit: Some(bad),
                ..TestSettings::default()
            });
            let error = Plan::from_settings(&settings, "t", "d").unwrap_err();
            assert!(error.to_string().contains("time_limit"), "{bad}: {error}");
            let settings = settings_with(TestSettings {
                reruns_delay: bad,
                ..TestSettings::default()
            });
            let error = Plan::from_settings(&settings, "t", "d").unwrap_err();
            assert!(error.to_string().contains("reruns_delay"), "{bad}: {error}");
        }
    }
}
