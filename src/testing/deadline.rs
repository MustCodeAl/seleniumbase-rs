//! The hard time limit of one test attempt.
//!
//! `--time-limit` (`SB_TIME_LIMIT`, `test.time_limit`) says how long a test
//! may run. [`BaseCase::set_time_limit`](crate::BaseCase::set_time_limit) fails
//! the test at its next open or wait; a [`Deadline`] stops a test that does
//! neither, such as one stuck in a loop of its own: the lifecycle races the test
//! against [`Deadline::expired`] and drops the test when it wins.
//!
//! The deadline is shared (`Arc`) with the `BaseCase` the test drives, so time
//! a person spends at a prompt does not count: a [`Suspension`] holds the clock
//! while `breakpoint()`, `--trace` or `--pdb` waits for an answer.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::Instant;

/// How long a test attempt may run, shared between the lifecycle that enforces
/// it and the case that suspends it.
#[derive(Debug)]
pub(crate) struct Deadline {
    state: Mutex<State>,
    changed: Notify,
}

#[derive(Debug)]
struct State {
    started: Instant,
    limit: Option<Duration>,
    /// Time already spent suspended and resumed.
    banked: Duration,
    /// How many [`Suspension`]s are alive.
    suspended: u32,
    /// When the oldest live suspension began.
    suspended_since: Option<Instant>,
}

impl State {
    /// How long the attempt has run, not counting the time it was suspended.
    fn elapsed(&self, now: Instant) -> Duration {
        let running = now.saturating_duration_since(self.started);
        let paused = self
            .suspended_since
            .map_or(Duration::ZERO, |since| now.saturating_duration_since(since));
        running.saturating_sub(self.banked + paused)
    }
}

impl Deadline {
    /// A deadline that starts now; `None` is no limit.
    pub(crate) fn new(limit: Option<Duration>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                started: Instant::now(),
                limit,
                banked: Duration::ZERO,
                suspended: 0,
                suspended_since: None,
            }),
            changed: Notify::new(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The limit, if there is one.
    pub(crate) fn limit(&self) -> Option<Duration> {
        self.lock().limit
    }

    /// Replaces the limit (`set_time_limit` in the test); `None` removes it.
    pub(crate) fn set_limit(&self, limit: Option<Duration>) {
        self.lock().limit = limit;
        self.changed.notify_waiters();
    }

    /// How long the attempt has run without the time it was suspended.
    #[cfg(test)]
    pub(crate) fn elapsed(&self) -> Duration {
        self.lock().elapsed(Instant::now())
    }

    /// Stops the clock until the returned guard is dropped. Suspensions nest:
    /// the clock runs again when the last one ends.
    pub(crate) fn suspend(self: &Arc<Self>) -> Suspension {
        {
            let mut state = self.lock();
            if state.suspended == 0 {
                state.suspended_since = Some(Instant::now());
            }
            state.suspended += 1;
        }
        self.changed.notify_waiters();
        Suspension {
            deadline: Arc::clone(self),
        }
    }

    fn resume(&self) {
        {
            let mut state = self.lock();
            state.suspended = state.suspended.saturating_sub(1);
            if state.suspended == 0 {
                if let Some(since) = state.suspended_since.take() {
                    state.banked += Instant::now().saturating_duration_since(since);
                }
            }
        }
        self.changed.notify_waiters();
    }

    /// Completes when the attempt has run for its limit. Never completes while
    /// there is no limit or the clock is suspended; a change of either wakes it
    /// to look again.
    pub(crate) async fn expired(&self) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            // Registered before the state is read, so a change in between is not lost.
            changed.as_mut().enable();
            let left = {
                let state = self.lock();
                match state.limit {
                    Some(limit) if state.suspended == 0 => {
                        let elapsed = state.elapsed(Instant::now());
                        if elapsed >= limit {
                            return;
                        }
                        Some(limit - elapsed)
                    }
                    _ => None,
                }
            };
            match left {
                Some(left) => {
                    tokio::select! {
                        () = tokio::time::sleep(left) => {}
                        () = &mut changed => {}
                    }
                }
                None => changed.await,
            }
        }
    }
}

/// The clock of a [`Deadline`] is stopped while this is alive.
#[derive(Debug)]
pub(crate) struct Suspension {
    deadline: Arc<Deadline>,
}

impl Drop for Suspension {
    fn drop(&mut self) {
        self.deadline.resume();
    }
}

/// Python's message for a test that ran past its limit: `This test has exceeded
/// the time limit of 1 second!`, `... 30 seconds!`.
pub(crate) fn exceeded_message(limit: Duration) -> String {
    let shown = if limit.subsec_nanos() == 0 {
        limit.as_secs().to_string()
    } else {
        limit.as_secs_f64().to_string()
    };
    let plural = if limit == Duration::from_secs(1) {
        ""
    } else {
        "s"
    };
    format!("This test has exceeded the time limit of {shown} second{plural}!")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[tokio::test(start_paused = true)]
    async fn a_deadline_expires_when_its_limit_is_reached() {
        let started = Instant::now();
        let deadline = Deadline::new(Some(SECOND));
        let early = tokio::time::timeout(Duration::from_millis(900), deadline.expired()).await;
        assert!(early.is_err(), "not yet");
        deadline.expired().await;
        assert_eq!(started.elapsed(), SECOND);
    }

    #[tokio::test(start_paused = true)]
    async fn a_suspended_clock_does_not_run_and_the_limit_is_pushed_back() {
        let started = Instant::now();
        let deadline = Deadline::new(Some(SECOND));
        let guard = deadline.suspend();
        let held = tokio::time::timeout(Duration::from_secs(60), deadline.expired()).await;
        assert!(held.is_err(), "a suspended deadline never expires");
        assert_eq!(deadline.elapsed(), Duration::ZERO);
        drop(guard);
        deadline.expired().await;
        assert_eq!(started.elapsed(), Duration::from_secs(61));
    }

    #[tokio::test(start_paused = true)]
    async fn suspensions_nest() {
        let deadline = Deadline::new(Some(SECOND));
        let outer = deadline.suspend();
        let inner = deadline.suspend();
        tokio::time::sleep(Duration::from_secs(5)).await;
        drop(inner);
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(
            deadline.elapsed(),
            Duration::ZERO,
            "still held by the outer one"
        );
        drop(outer);
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(deadline.elapsed(), Duration::from_millis(400));
    }

    #[tokio::test(start_paused = true)]
    async fn a_deadline_without_a_limit_waits_for_one() {
        let started = Instant::now();
        let deadline = Deadline::new(None);
        let nothing = tokio::time::timeout(Duration::from_secs(3600), deadline.expired()).await;
        assert!(nothing.is_err());
        let ((), ()) = tokio::join!(
            async {
                tokio::time::sleep(Duration::from_secs(2)).await;
                deadline.set_limit(Some(SECOND));
            },
            deadline.expired()
        );
        // The hour of waiting counts as running time, so the new limit was already past.
        assert_eq!(started.elapsed(), Duration::from_secs(3600 + 2));
        assert_eq!(deadline.limit(), Some(SECOND));
    }

    #[tokio::test(start_paused = true)]
    async fn removing_the_limit_stops_the_wait() {
        let deadline = Deadline::new(Some(SECOND));
        let ((), ()) = tokio::join!(
            async {
                tokio::time::sleep(Duration::from_millis(500)).await;
                deadline.set_limit(None);
            },
            async {
                let waited = tokio::time::timeout(Duration::from_secs(60), deadline.expired());
                assert!(waited.await.is_err(), "no limit, no expiry");
            }
        );
        assert_eq!(deadline.limit(), None);
    }

    #[test]
    fn the_message_is_pythons() {
        assert_eq!(
            exceeded_message(SECOND),
            "This test has exceeded the time limit of 1 second!"
        );
        assert_eq!(
            exceeded_message(Duration::from_secs(30)),
            "This test has exceeded the time limit of 30 seconds!"
        );
        assert_eq!(
            exceeded_message(Duration::from_millis(500)),
            "This test has exceeded the time limit of 0.5 seconds!"
        );
        assert_eq!(
            exceeded_message(Duration::from_secs_f64(2.0)),
            "This test has exceeded the time limit of 2 seconds!"
        );
    }
}
