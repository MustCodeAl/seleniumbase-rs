//! The browser of one scenario, and how it is closed.
//!
//! rstest-bdd 0.6 runs a scenario's steps in the test's own runtime and drops
//! the scenario's fixtures at the end of the test function: `Drop` is the only
//! hook after the last step, it cannot await, and the harness path of the
//! crate cannot run a step that waits (see `docs/design/rstest_bdd_spike.md`).
//! So the lifecycle of [`Sb::run`](crate::testing::Sb::run) (evidence of a
//! failure, the debug prompts, the finalizers, the close) cannot wrap the
//! steps. It runs beside them instead:
//!
//! 1. [`Scenario::start`] starts a thread with a runtime of its own, which
//!    runs [`Lifecycle::run`] with a body that hands the open case to the
//!    fixture and waits for it to come back;
//! 2. the steps use the case from the test's runtime;
//! 3. when the fixture is dropped, [`Scenario`] sends the case back with the
//!    verdict (a panicking thread means a failed scenario) and joins the
//!    thread, which then does what the lifecycle does after a body: the
//!    evidence of a failure with the page still open, the prompts, the
//!    finalizers and the close.
//!
//! The browser's connections were made on the thread's runtime, which stays
//! alive until the join, so the clean-up never needs the test's runtime to
//! make progress.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Once};
use std::thread::JoinHandle;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::oneshot;

use crate::common::debug_prompt::PromptPage;
use crate::error::{Result, SeleniumBaseError};
use crate::testing::lifecycle::{
    say_on_stderr, Finalizer, Lifecycle, Plan, TerminalPrompter, TestCase,
};
use crate::testing::Deadline;

/// How to open the case of an attempt. It runs on the scenario's thread.
pub(crate) type Launch<C> = Box<dyn Fn(u32) -> Pin<Box<dyn Future<Output = Result<C>>>> + Send>;

/// A case coming back from the steps, with the verdict the scenario earned.
struct Back<C> {
    case: C,
    verdict: Result<()>,
}

/// What the scenario's thread gives the fixture: the open case and the way to
/// give it back, or why there is none.
type Handover<C> = Result<(C, oneshot::Sender<Back<C>>)>;

fn lost() -> SeleniumBaseError {
    SeleniumBaseError::TestLifecycle("the scenario's browser was lost".to_owned())
}

/// The case of an attempt as the lifecycle sees it. It is empty while the
/// steps have the case.
struct Slot<C> {
    inner: Option<C>,
}

impl<C> Slot<C> {
    fn case(&self) -> Result<&C> {
        self.inner.as_ref().ok_or_else(lost)
    }
}

#[async_trait]
impl<C: PromptPage> PromptPage for Slot<C> {
    async fn url(&self) -> Result<String> {
        self.case()?.url().await
    }

    async fn title(&self) -> Result<String> {
        self.case()?.title().await
    }

    async fn screenshot_png(&self) -> Result<Vec<u8>> {
        self.case()?.screenshot_png().await
    }

    async fn page_source(&self) -> Result<String> {
        self.case()?.page_source().await
    }

    async fn evaluate(&self, expression: &str) -> Result<Value> {
        self.case()?.evaluate(expression).await
    }
}

impl<C: TestCase> TestCase for Slot<C> {
    async fn close(&mut self) -> Result<()> {
        match self.inner.as_mut() {
            Some(case) => case.close().await,
            None => Ok(()),
        }
    }

    async fn prepare(&mut self, plan: &Plan) -> Result<()> {
        match self.inner.as_mut() {
            Some(case) => case.prepare(plan).await,
            None => Ok(()),
        }
    }

    fn name_test(&mut self, test_id: &str) {
        if let Some(case) = self.inner.as_mut() {
            case.name_test(test_id);
        }
    }

    fn attach_deadline(&mut self, deadline: Arc<Deadline>) {
        if let Some(case) = self.inner.as_mut() {
            case.attach_deadline(deadline);
        }
    }

    fn extend_clock(&mut self, paused: Duration) {
        if let Some(case) = self.inner.as_mut() {
            case.extend_clock(paused);
        }
    }
}

/// A scenario's open case, which the steps use and which closes it when
/// dropped.
pub(crate) struct Scenario<C: TestCase + 'static> {
    case: Option<C>,
    back: Option<oneshot::Sender<Back<C>>>,
    thread: Option<JoinHandle<Result<()>>>,
}

impl<C: TestCase + 'static> Scenario<C> {
    /// Opens the case of a scenario on a thread of its own and waits for it.
    ///
    /// # Errors
    ///
    /// Returns the launch error, or why the thread could not start.
    pub(crate) async fn start(
        plan: Plan,
        finalizers: Vec<Arc<dyn Finalizer>>,
        launch: Launch<C>,
    ) -> Result<Self> {
        watch_panics();
        let (ready, handover) = oneshot::channel::<Handover<C>>();
        let thread = std::thread::Builder::new()
            .name("sb-scenario".to_owned())
            .spawn(move || run_thread(&plan, &finalizers, &launch, ready))
            .map_err(|error| {
                SeleniumBaseError::TestLifecycle(format!(
                    "the scenario's browser thread did not start: {error}"
                ))
            })?;
        match handover.await {
            Ok(Ok((case, back))) => Ok(Self {
                case: Some(case),
                back: Some(back),
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                // The thread has nothing more to do once it has said why.
                let _ = thread.join();
                Err(error)
            }
            Err(_) => Err(match thread.join() {
                Err(_) => SeleniumBaseError::TestLifecycle(
                    "the scenario's browser thread panicked".to_owned(),
                ),
                Ok(_) => lost(),
            }),
        }
    }

    /// The case the steps use.
    pub(crate) fn case(&self) -> &C {
        self.case
            .as_ref()
            .expect("the case is only taken when the scenario is dropped")
    }

    pub(crate) fn case_mut(&mut self) -> &mut C {
        self.case
            .as_mut()
            .expect("the case is only taken when the scenario is dropped")
    }
}

impl<C: TestCase + 'static> Drop for Scenario<C> {
    fn drop(&mut self) {
        let failed = std::thread::panicking();
        if let (Some(case), Some(back)) = (self.case.take(), self.back.take()) {
            let verdict = if failed {
                Err(SeleniumBaseError::AssertionFailed(
                    take_last_panic().unwrap_or_else(|| "the scenario failed".to_owned()),
                ))
            } else {
                Ok(())
            };
            // A send that fails means the thread is gone; the case then
            // closes as it can when it is dropped.
            drop(back.send(Back { case, verdict }));
        }
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Ok(Err(error)) if !failed => {
                    say_on_stderr(&format!("warning: the scenario's browser: {error}\n"));
                }
                Ok(_) => {}
                Err(_) => say_on_stderr("warning: the scenario's browser thread panicked\n"),
            }
        }
    }
}

/// The scenario's thread: a runtime that runs the lifecycle of one attempt.
fn run_thread<C: TestCase>(
    plan: &Plan,
    finalizers: &[Arc<dyn Finalizer>],
    launch: &Launch<C>,
    ready: oneshot::Sender<Handover<C>>,
) -> Result<()> {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            drop(ready.send(Err(SeleniumBaseError::TestLifecycle(format!(
                "the scenario's runtime did not start: {error}"
            )))));
            return Ok(());
        }
    };
    runtime.block_on(async {
        let ready = RefCell::new(Some(ready));
        let lifecycle = Lifecycle {
            plan,
            prompter: &TerminalPrompter,
            finalizers,
            say: &say_on_stderr,
        };
        let result = lifecycle
            .run(
                async |attempt: u32| launch(attempt).await.map(|case| Slot { inner: Some(case) }),
                async |slot: &mut Slot<C>| {
                    let (Some(case), Some(ready)) = (slot.inner.take(), ready.borrow_mut().take())
                    else {
                        return Err(lost());
                    };
                    let (back, returned) = oneshot::channel::<Back<C>>();
                    if let Err(Ok((case, _))) = ready.send(Ok((case, back))) {
                        slot.inner = Some(case);
                        return Err(lost());
                    }
                    match returned.await {
                        Ok(back) => {
                            slot.inner = Some(back.case);
                            back.verdict
                        }
                        Err(_) => Err(lost()),
                    }
                },
            )
            .await;
        // A launch that failed never reached the body: say why.
        let unsent = ready.borrow_mut().take();
        match (unsent, result) {
            (Some(ready), Err(error)) => {
                drop(ready.send(Err(error)));
                Ok(())
            }
            (Some(ready), Ok(())) => {
                drop(ready.send(Err(lost())));
                Ok(())
            }
            (None, result) => result,
        }
    })
}

thread_local! {
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Has the message of the last panic of each thread kept, so the evidence of a
/// failed scenario can say why it failed. The earlier hook still runs.
fn watch_panics() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let earlier = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info
                .payload_as_str()
                .unwrap_or("a panic without a message")
                .to_owned();
            LAST_PANIC.with(|last| *last.borrow_mut() = Some(message));
            earlier(info);
        }));
    });
}

fn take_last_panic() -> Option<String> {
    LAST_PANIC.with(|last| last.borrow_mut().take())
}
