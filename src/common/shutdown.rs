//! Graceful shutdown (Twelve-Factor IX: Disposability).
//!
//! A process that owns a browser must close it when it is told to stop.
//! Orchestrators stop processes with SIGTERM, and a terminal sends SIGINT on
//! Ctrl-C; with the default disposition both end the process on the spot and
//! leave Chrome and its driver running. The pieces here turn those signals
//! into something an async program can wait on:
//!
//! * [`Shutdown`] listens for SIGINT and SIGTERM (Ctrl-C on Windows).
//!   [`run_until_shutdown`] races a piece of work against it.
//! * [`cleanup_within`] bounds the cleanup that follows by a grace period,
//!   normally [`timeout_from_env`].
//! * [`spawn_cleanup`] and [`drain_cleanups`] cover cleanup that starts in a
//!   `Drop` implementation, where nothing can be awaited: the cleanup is
//!   spawned, and the program waits for it before it exits.
//!
//! ```no_run
//! use seleniumbase_rs::common::shutdown::{
//!     cleanup_within, run_until_shutdown, timeout_from_env, Outcome,
//! };
//!
//! # async fn serve() {}
//! # async fn close_browsers() {}
//! # async fn run() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! match run_until_shutdown(serve()).await? {
//!     Outcome::Completed(()) => {}
//!     Outcome::Interrupted(signal) => eprintln!("{signal} received; closing browsers"),
//! }
//! cleanup_within(timeout_from_env(), close_browsers()).await;
//! # Ok(())
//! # }
//! ```
//!
//! Once a handler is installed the signal no longer ends the process by
//! itself, so whatever installs one must keep waiting on it.

use std::fmt;
use std::future::{pending, Future};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::error::SeleniumBaseError;

/// The environment variable that sets the grace period, in whole seconds.
///
/// [`RuntimeConfig`](crate::RuntimeConfig) reads the same variable.
pub const TIMEOUT_VAR: &str = "SB_SHUTDOWN_TIMEOUT_SECS";

/// The grace period when [`TIMEOUT_VAR`] is unset or unusable.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A request to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Signal {
    /// SIGINT, or Ctrl-C.
    Interrupt,
    /// SIGTERM.
    Terminate,
}

impl Signal {
    /// The exit status a shell reports for a process ended by this signal:
    /// 128 plus the signal number.
    #[must_use]
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Interrupt => 130,
            Self::Terminate => 143,
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Interrupt => "SIGINT",
            Self::Terminate => "SIGTERM",
        })
    }
}

/// A source of [`Signal`]s: the process's own, or one a test fires by hand.
#[derive(Debug)]
pub struct Shutdown {
    source: Source,
}

#[derive(Debug)]
enum Source {
    Os(Os),
    Manual(mpsc::Receiver<Signal>),
}

impl Shutdown {
    /// Starts listening for SIGINT and SIGTERM (Ctrl-C on Windows).
    ///
    /// Call it inside a Tokio runtime, and before the work it guards starts,
    /// so a signal arriving early is not lost. Each `Shutdown` hears every
    /// signal; two of them in one process do not take signals from each other.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] if the operating system
    /// refuses the handler.
    pub fn install() -> Result<Self, SeleniumBaseError> {
        Ok(Self {
            source: Source::Os(Os::install()?),
        })
    }

    /// A `Shutdown` that fires only when the returned [`ShutdownTrigger`] says
    /// so. It lets tests, and programs with their own stop button, drive the
    /// same code that real signals drive.
    #[must_use]
    pub fn manual() -> (Self, ShutdownTrigger) {
        let (sender, receiver) = mpsc::channel(4);
        (
            Self {
                source: Source::Manual(receiver),
            },
            ShutdownTrigger { sender },
        )
    }

    /// Waits for the next signal.
    ///
    /// A manual `Shutdown` whose trigger was dropped never fires.
    pub async fn recv(&mut self) -> Signal {
        match &mut self.source {
            Source::Os(os) => os.recv().await,
            Source::Manual(receiver) => match receiver.recv().await {
                Some(signal) => signal,
                None => pending().await,
            },
        }
    }

    /// Runs `work`, unless a signal comes first. In that case `work` is
    /// dropped, which cancels it at its next await point, and the signal is
    /// returned.
    pub async fn race<F: Future>(&mut self, work: F) -> Outcome<F::Output> {
        tokio::select! {
            output = work => Outcome::Completed(output),
            signal = self.recv() => Outcome::Interrupted(signal),
        }
    }
}

/// Fires a [`Shutdown::manual`].
#[derive(Debug, Clone)]
pub struct ShutdownTrigger {
    sender: mpsc::Sender<Signal>,
}

impl ShutdownTrigger {
    /// Delivers `signal`. It is dropped if the `Shutdown` is gone or four
    /// signals are already waiting.
    pub fn fire(&self, signal: Signal) {
        let _ = self.sender.try_send(signal);
    }
}

/// How a raced piece of work ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome<T> {
    /// The work finished first, with this result.
    Completed(T),
    /// A signal arrived first and the work was dropped.
    Interrupted(Signal),
}

#[cfg(unix)]
#[derive(Debug)]
struct Os {
    interrupt: tokio::signal::unix::Signal,
    terminate: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl Os {
    fn install() -> Result<Self, SeleniumBaseError> {
        use tokio::signal::unix::{signal, SignalKind};
        let handler = |kind, name| {
            signal(kind).map_err(|e| {
                SeleniumBaseError::Unsupported(format!("failed to install {name} handler: {e}"))
            })
        };
        Ok(Self {
            interrupt: handler(SignalKind::interrupt(), "SIGINT")?,
            terminate: handler(SignalKind::terminate(), "SIGTERM")?,
        })
    }

    async fn recv(&mut self) -> Signal {
        // A stream that reports its end (the runtime is going away) is not a
        // request to stop.
        async fn next(stream: &mut tokio::signal::unix::Signal) {
            if stream.recv().await.is_none() {
                pending::<()>().await;
            }
        }
        tokio::select! {
            () = next(&mut self.interrupt) => Signal::Interrupt,
            () = next(&mut self.terminate) => Signal::Terminate,
        }
    }
}

#[cfg(not(unix))]
#[derive(Debug)]
struct Os;

#[cfg(not(unix))]
impl Os {
    fn install() -> Result<Self, SeleniumBaseError> {
        Ok(Self)
    }

    async fn recv(&mut self) -> Signal {
        match tokio::signal::ctrl_c().await {
            Ok(()) => Signal::Interrupt,
            Err(_) => pending().await,
        }
    }
}

/// Waits for the first SIGINT or SIGTERM.
///
/// The handlers are installed when this is first polled. To guard work that is
/// already running, install a [`Shutdown`] first instead.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Unsupported`] if the handler cannot be
/// installed.
pub async fn wait_for_shutdown_signal() -> Result<Signal, SeleniumBaseError> {
    Ok(Shutdown::install()?.recv().await)
}

/// Runs `work` until it finishes or the process is asked to stop.
///
/// The handlers are installed before `work` is first polled. When a signal wins
/// the race, `work` is dropped; follow up with [`cleanup_within`].
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Unsupported`] if the handlers cannot be
/// installed, in which case `work` has not started.
pub async fn run_until_shutdown<F: Future>(
    work: F,
) -> Result<Outcome<F::Output>, SeleniumBaseError> {
    let mut shutdown = Shutdown::install()?;
    Ok(shutdown.race(work).await)
}

/// Runs `cleanup` for at most `grace`. Returns `true` if it finished in time.
///
/// A cleanup that overruns is dropped and a warning is logged: shutdown must
/// end even when a browser will not close.
pub async fn cleanup_within<F: Future<Output = ()>>(grace: Duration, cleanup: F) -> bool {
    let finished = tokio::time::timeout(grace, cleanup).await.is_ok();
    if !finished {
        warn!(?grace, "shutdown cleanup did not finish in time; giving up");
    }
    finished
}

/// The grace period to give cleanup: [`TIMEOUT_VAR`], else [`DEFAULT_TIMEOUT`].
///
/// A value that is not a whole number of seconds is logged and ignored. Unlike
/// [`RuntimeConfig::from_env`](crate::RuntimeConfig::from_env), a bad value in
/// some other variable does not change the answer.
#[must_use]
pub fn timeout_from_env() -> Duration {
    let value = std::env::var(TIMEOUT_VAR).ok();
    parse_timeout(value.as_deref()).unwrap_or_else(|error| {
        warn!(%error, "ignoring {TIMEOUT_VAR}; using the default grace period");
        DEFAULT_TIMEOUT
    })
}

/// Reads a grace period given in seconds. `None` and blank text give
/// [`DEFAULT_TIMEOUT`]; `0` means no grace at all.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if the text is not a whole
/// number of seconds.
pub fn parse_timeout(value: Option<&str>) -> Result<Duration, SeleniumBaseError> {
    let Some(text) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(DEFAULT_TIMEOUT);
    };
    text.parse::<u64>().map(Duration::from_secs).map_err(|e| {
        SeleniumBaseError::invalid_config(format!(
            "{TIMEOUT_VAR} must be a whole number of seconds, got {text:?}: {e}"
        ))
    })
}

/// Cleanup tasks spawned from `Drop`, kept so [`drain_cleanups`] can wait for
/// them. It is process-wide because a `Drop` has no way to be handed a
/// registry, and the thing waiting for them is the end of the process.
static CLEANUPS: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

fn cleanups() -> MutexGuard<'static, Vec<JoinHandle<()>>> {
    // A panic while holding the list cannot leave it in a bad state.
    CLEANUPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Starts `cleanup` in the background and remembers it, for use in `Drop`.
///
/// Returns `false`, dropping `cleanup` unrun, when there is no Tokio runtime to
/// run it on. Pair it with [`drain_cleanups`] before the program exits;
/// without that, a cleanup still running when the runtime shuts down is lost.
pub fn spawn_cleanup<F>(cleanup: F) -> bool
where
    F: Future<Output = ()> + Send + 'static,
{
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return false;
    };
    let handle = runtime.spawn(cleanup);
    let mut pending = cleanups();
    pending.retain(|task| !task.is_finished());
    pending.push(handle);
    true
}

/// Waits up to `grace` for every cleanup from [`spawn_cleanup`].
///
/// Returns how many had not finished when the time ran out.
pub async fn drain_cleanups(grace: Duration) -> usize {
    let tasks = std::mem::take(&mut *cleanups());
    debug!(count = tasks.len(), "waiting for shutdown cleanups");
    let deadline = tokio::time::Instant::now() + grace;
    let mut unfinished = 0;
    for task in tasks {
        match tokio::time::timeout_at(deadline, task).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => warn!(%error, "a shutdown cleanup panicked"),
            Err(_) => unfinished += 1,
        }
    }
    if unfinished > 0 {
        warn!(
            unfinished,
            ?grace,
            "shutdown cleanups did not finish in time"
        );
    }
    unfinished
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;

    #[test]
    fn a_missing_or_blank_timeout_is_the_default() {
        assert_eq!(parse_timeout(None).unwrap(), DEFAULT_TIMEOUT);
        assert_eq!(parse_timeout(Some("")).unwrap(), DEFAULT_TIMEOUT);
        assert_eq!(parse_timeout(Some("   ")).unwrap(), DEFAULT_TIMEOUT);
    }

    #[test]
    fn a_timeout_is_a_whole_number_of_seconds() {
        assert_eq!(parse_timeout(Some("5")).unwrap(), Duration::from_secs(5));
        assert_eq!(
            parse_timeout(Some(" 12 ")).unwrap(),
            Duration::from_secs(12)
        );
        assert_eq!(parse_timeout(Some("0")).unwrap(), Duration::ZERO);
    }

    #[test]
    fn a_bad_timeout_is_reported_with_its_name_and_text() {
        for bad in ["soon", "-1", "1.5", "5s", "99999999999999999999999"] {
            let message = parse_timeout(Some(bad)).unwrap_err().to_string();
            assert!(message.contains(TIMEOUT_VAR), "{message}");
            assert!(message.contains(bad), "{message}");
        }
    }

    #[test]
    fn the_default_matches_the_runtime_config_default() {
        assert_eq!(
            DEFAULT_TIMEOUT,
            crate::RuntimeConfig::default().shutdown_timeout
        );
    }

    #[test]
    fn signals_have_shell_style_exit_codes_and_names() {
        assert_eq!(Signal::Interrupt.exit_code(), 130);
        assert_eq!(Signal::Terminate.exit_code(), 143);
        assert_eq!(Signal::Interrupt.to_string(), "SIGINT");
        assert_eq!(Signal::Terminate.to_string(), "SIGTERM");
    }

    #[tokio::test]
    async fn a_manual_shutdown_delivers_what_the_trigger_fires() {
        let (mut shutdown, trigger) = Shutdown::manual();
        trigger.fire(Signal::Terminate);
        assert_eq!(shutdown.recv().await, Signal::Terminate);
        trigger.clone().fire(Signal::Interrupt);
        assert_eq!(shutdown.recv().await, Signal::Interrupt);
    }

    #[tokio::test(start_paused = true)]
    async fn a_manual_shutdown_without_a_trigger_never_fires() {
        let (mut shutdown, trigger) = Shutdown::manual();
        drop(trigger);
        let heard = tokio::time::timeout(Duration::from_secs(3600), shutdown.recv()).await;
        assert!(heard.is_err());
    }

    #[tokio::test]
    async fn work_that_finishes_first_is_reported_complete() {
        let (mut shutdown, _trigger) = Shutdown::manual();
        let outcome = shutdown.race(async { 7 }).await;
        assert_eq!(outcome, Outcome::Completed(7));
    }

    #[tokio::test]
    async fn a_signal_cancels_the_work_it_interrupts() {
        struct Flag(Arc<AtomicUsize>);
        impl Drop for Flag {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(AtomicUsize::new(0));
        let (mut shutdown, trigger) = Shutdown::manual();
        trigger.fire(Signal::Terminate);
        let guard = Flag(Arc::clone(&dropped));
        let outcome = shutdown
            .race(async move {
                let _guard = guard;
                pending::<()>().await;
            })
            .await;
        assert_eq!(outcome, Outcome::Interrupted(Signal::Terminate));
        assert_eq!(
            dropped.load(Ordering::SeqCst),
            1,
            "the work was not dropped"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn cleanup_that_finishes_in_time_is_reported_so() {
        let done = cleanup_within(Duration::from_secs(5), async {
            tokio::time::sleep(Duration::from_secs(1)).await;
        })
        .await;
        assert!(done);
    }

    #[tokio::test(start_paused = true)]
    async fn cleanup_that_overruns_is_abandoned() {
        let started = tokio::time::Instant::now();
        let done = cleanup_within(Duration::from_secs(5), pending::<()>()).await;
        assert!(!done);
        assert_eq!(started.elapsed(), Duration::from_secs(5));
    }

    #[tokio::test(start_paused = true)]
    async fn drained_cleanups_are_awaited_and_stragglers_are_counted() {
        // Other tests share the list, so work only with tasks made here and
        // drain them all in one step.
        let finished = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let finished = Arc::clone(&finished);
            assert!(spawn_cleanup(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                finished.fetch_add(1, Ordering::SeqCst);
            }));
        }
        let stuck = spawn_cleanup(pending::<()>());
        assert!(stuck);

        let unfinished = drain_cleanups(Duration::from_secs(10)).await;
        assert_eq!(finished.load(Ordering::SeqCst), 3);
        assert!(unfinished >= 1, "the stuck cleanup should be reported");
    }

    #[test]
    fn a_cleanup_cannot_be_spawned_without_a_runtime() {
        assert!(!spawn_cleanup(async {}));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_process_hears_sigint_and_sigterm() {
        // The only test that installs the real handlers: every installed
        // `Shutdown` hears every signal, so a second one would see these.
        fn send(signal: &str) {
            let status = std::process::Command::new("kill")
                .arg(format!("-{signal}"))
                .arg(std::process::id().to_string())
                .status()
                .expect("run kill");
            assert!(status.success(), "kill -{signal} failed");
        }

        let mut shutdown = Shutdown::install().expect("install handlers");
        send("INT");
        let first = tokio::time::timeout(Duration::from_secs(10), shutdown.recv())
            .await
            .expect("SIGINT was not delivered");
        assert_eq!(first, Signal::Interrupt);

        send("TERM");
        let second = tokio::time::timeout(Duration::from_secs(10), shutdown.recv())
            .await
            .expect("SIGTERM was not delivered");
        assert_eq!(second, Signal::Terminate);
    }
}
