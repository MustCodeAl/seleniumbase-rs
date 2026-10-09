//! Retrying, polling and rate limiting, in one place.
//!
//! Three things in this crate wait and try again, and they share this module
//! instead of each keeping its own loop:
//!
//! * [`RetryPolicy::retry`] and [`RetryPolicy::retry_blocking`] run an
//!   operation again after a transient failure, with exponential backoff. This
//!   is Python SeleniumBase's `retry_on_exception` decorator.
//! * [`poll_until`] asks a question over and over until it has an answer or a
//!   deadline passes. Every `wait_for_*` method is this shape.
//! * [`RateLimiter`] keeps a loop from calling something more than `n` times in
//!   a window. This is Python's `rate_limited` decorator.
//!
//! Rust has no decorators, so each is a plain value you call through.
//!
//! ```
//! use seleniumbase_rs::RetryPolicy;
//! use std::time::Duration;
//!
//! let policy = RetryPolicy {
//!     max_attempts: 3,
//!     base_delay: Duration::from_millis(1),
//!     backoff_factor: 2.0,
//!     max_delay: Duration::from_millis(10),
//! };
//! let mut calls = 0;
//! let result: Result<i32, &str> = policy.retry_blocking(
//!     || {
//!         calls += 1;
//!         if calls < 3 { Err("flaky") } else { Ok(calls) }
//!     },
//!     |_| true,
//! );
//! assert_eq!(result, Ok(3));
//! ```

use std::collections::VecDeque;
use std::future::Future;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// How often to try an operation, and how long to wait between tries.
///
/// The wait before the second attempt is [`base_delay`](Self::base_delay).
/// Each later wait is the previous one times
/// [`backoff_factor`](Self::backoff_factor), never more than
/// [`max_delay`](Self::max_delay). The first attempt is immediate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the first). Zero is treated as
    /// one: an operation always runs at least once.
    pub max_attempts: usize,
    /// Delay before the first retry.
    pub base_delay: Duration,
    /// Multiplier applied to the delay after each attempt.
    pub backoff_factor: f64,
    /// Maximum delay between attempts.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(200),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(5),
        }
    }
}

impl RetryPolicy {
    /// A conservative policy suitable for WebDriver element lookups.
    pub fn webdriver() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(250),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(3),
        }
    }

    /// A policy for network-level operations such as downloads.
    pub fn network() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(500),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(10),
        }
    }

    /// The defaults of Python SeleniumBase's `retry_on_exception`: six
    /// attempts, a one second pause that doubles up to 32 seconds.
    pub fn seleniumbase() -> Self {
        Self {
            max_attempts: 6,
            base_delay: Duration::from_secs(1),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(32),
        }
    }

    /// A policy that waits the same `interval` between every attempt and never
    /// gives up by itself. It is meant for [`poll_until`], whose deadline ends
    /// the loop.
    pub fn every(interval: Duration) -> Self {
        Self {
            max_attempts: usize::MAX,
            base_delay: interval,
            backoff_factor: 1.0,
            max_delay: interval,
        }
    }

    /// The number of attempts the policy allows, which is never less than one.
    pub fn attempts(&self) -> usize {
        self.max_attempts.max(1)
    }

    /// Delay before attempt `n` (1-indexed). The first attempt has no delay.
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        if attempt <= 1 {
            return Duration::ZERO;
        }
        let exponent = i32::try_from(attempt - 2).unwrap_or(i32::MAX);
        let nanos = self.base_delay.as_nanos() as f64 * self.backoff_factor.powi(exponent);
        let cap = self.max_delay.as_nanos() as f64;
        if !nanos.is_finite() || nanos >= cap {
            // A runaway factor (or NaN) is held at the cap, never beyond it.
            return self.max_delay;
        }
        Duration::from_nanos(nanos.max(0.0).round() as u64)
    }

    /// Retry an async operation until it succeeds or the policy is exhausted.
    ///
    /// `is_transient` decides whether a given error deserves another attempt;
    /// any other error is returned at once. The last error is returned when
    /// every attempt failed.
    ///
    /// ```
    /// # use seleniumbase_rs::{RetryPolicy, SeleniumBaseError};
    /// # async fn run() -> Result<(), SeleniumBaseError> {
    /// let page = RetryPolicy::network()
    ///     .retry(
    ///         || async { Ok::<_, SeleniumBaseError>("<html></html>") },
    ///         SeleniumBaseError::is_transient,
    ///     )
    ///     .await?;
    /// # let _ = page;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn retry<F, Fut, T, E>(
        &self,
        mut op: F,
        is_transient: impl Fn(&E) -> bool,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let attempts = self.attempts();
        let mut attempt = 1;
        loop {
            let delay = self.delay_for_attempt(attempt);
            if delay > Duration::ZERO {
                tokio::time::sleep(delay).await;
            }
            debug!(attempt, max_attempts = attempts, "retry operation");
            match op().await {
                Ok(value) => return Ok(value),
                Err(err) if attempt < attempts && is_transient(&err) => {
                    warn!(attempt, "transient failure; retrying");
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Like [`retry`](Self::retry), for an operation that blocks.
    ///
    /// It sleeps the calling thread between attempts, so do not call it from
    /// async code; use [`retry`](Self::retry) there.
    pub fn retry_blocking<F, T, E>(
        &self,
        mut op: F,
        is_transient: impl Fn(&E) -> bool,
    ) -> Result<T, E>
    where
        F: FnMut() -> Result<T, E>,
    {
        let attempts = self.attempts();
        let mut attempt = 1;
        loop {
            let delay = self.delay_for_attempt(attempt);
            if delay > Duration::ZERO {
                std::thread::sleep(delay);
            }
            debug!(attempt, max_attempts = attempts, "retry operation");
            match op() {
                Ok(value) => return Ok(value),
                Err(err) if attempt < attempts && is_transient(&err) => {
                    warn!(attempt, "transient failure; retrying");
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }
}

/// Polls `op` until it returns `Some` or the deadline passes.
///
/// Uses [`RetryPolicy`] backoff between attempts, capped so polling stays
/// responsive. Returns `None` when the timeout elapses. `op` always runs at
/// least once, even with a zero timeout.
///
/// ```
/// # use seleniumbase_rs::RetryPolicy;
/// # use seleniumbase_rs::utilities::retry::poll_until;
/// # use std::time::Duration;
/// # async fn run() {
/// let mut polls = 0;
/// let found = poll_until(
///     Duration::from_secs(5),
///     RetryPolicy::every(Duration::from_millis(50)),
///     || {
///         polls += 1;
///         let ready = polls >= 3;
///         async move { ready.then_some("ready") }
///     },
/// )
/// .await;
/// assert_eq!(found, Some("ready"));
/// # }
/// ```
pub async fn poll_until<T, F, Fut>(timeout: Duration, policy: RetryPolicy, mut op: F) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let deadline = Instant::now() + timeout;
    let mut attempt = 0usize;
    loop {
        attempt = attempt.saturating_add(1);
        if let Some(value) = op().await {
            return Some(value);
        }
        if Instant::now() >= deadline {
            return None;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let delay = policy
            .delay_for_attempt(attempt.saturating_add(1))
            .min(Duration::from_millis(500))
            .min(remaining);
        if delay > Duration::ZERO {
            tokio::time::sleep(delay).await;
        }
    }
}

/// Lets at most `max_calls` through in any `window`, making the rest wait.
///
/// It is a sliding window: a burst of `max_calls` goes through at once, and
/// the next call waits until the oldest of them is a full window old. The
/// limiter hands out start times, so one `RateLimiter` shared by several tasks
/// (behind an `Arc`) keeps them all to the limit.
///
/// Python's `rate_limited(max_per_second)` spaces calls evenly instead; use
/// [`RateLimiter::per_second`] with a small limit for the same effect.
///
/// ```
/// use seleniumbase_rs::utilities::retry::RateLimiter;
/// use std::time::Duration;
///
/// let limiter = RateLimiter::new(2, Duration::from_secs(1));
/// assert_eq!(limiter.reserve(), Duration::ZERO);
/// assert_eq!(limiter.reserve(), Duration::ZERO);
/// assert!(limiter.reserve() > Duration::from_millis(900));
/// ```
#[derive(Debug)]
pub struct RateLimiter {
    max_calls: usize,
    window: Duration,
    /// Start times of the last `max_calls` calls, oldest first. A start time
    /// can lie in the future: it is when that call was told to proceed.
    starts: Mutex<VecDeque<Instant>>,
}

impl RateLimiter {
    /// A limiter that allows `max_calls` per `window`. A `max_calls` of zero is
    /// treated as one.
    pub fn new(max_calls: usize, window: Duration) -> Self {
        let max_calls = max_calls.max(1);
        Self {
            max_calls,
            window,
            starts: Mutex::new(VecDeque::with_capacity(max_calls.min(1024))),
        }
    }

    /// A limiter that allows `max_calls` per second.
    pub fn per_second(max_calls: usize) -> Self {
        Self::new(max_calls, Duration::from_secs(1))
    }

    /// Books the next call and returns how long to wait before making it.
    ///
    /// The caller must wait that long; [`acquire`](Self::acquire) and
    /// [`acquire_blocking`](Self::acquire_blocking) do so.
    pub fn reserve(&self) -> Duration {
        self.reserve_at(Instant::now())
    }

    fn reserve_at(&self, now: Instant) -> Duration {
        // A poisoned lock only means another thread panicked between pushes;
        // the queue is still a valid list of times.
        let mut starts = self
            .starts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let start = if starts.len() < self.max_calls {
            now
        } else {
            // The call `max_calls` back must be a full window behind this one.
            starts
                .front()
                .map_or(now, |oldest| (*oldest + self.window).max(now))
        };
        starts.push_back(start);
        if starts.len() > self.max_calls {
            starts.pop_front();
        }
        start.saturating_duration_since(now)
    }

    /// Waits, without blocking the executor, until a call is allowed.
    pub async fn acquire(&self) {
        let wait = self.reserve();
        if wait > Duration::ZERO {
            tokio::time::sleep(wait).await;
        }
    }

    /// Blocks the calling thread until a call is allowed. Do not call it from
    /// async code; use [`acquire`](Self::acquire) there.
    pub fn acquire_blocking(&self) {
        let wait = self.reserve();
        if wait > Duration::ZERO {
            std::thread::sleep(wait);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quick(max_attempts: usize) -> RetryPolicy {
        RetryPolicy {
            max_attempts,
            base_delay: Duration::from_millis(1),
            backoff_factor: 1.0,
            max_delay: Duration::from_millis(10),
        }
    }

    #[test]
    fn delay_increases_with_backoff() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.delay_for_attempt(1), Duration::ZERO);
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
    }

    #[test]
    fn delay_is_capped_at_max() {
        let policy = RetryPolicy {
            max_attempts: 10,
            base_delay: Duration::from_secs(1),
            backoff_factor: 10.0,
            max_delay: Duration::from_secs(3),
        };
        assert_eq!(policy.delay_for_attempt(5), Duration::from_secs(3));
    }

    #[test]
    fn a_cap_below_the_base_delay_wins() {
        let policy = RetryPolicy {
            max_attempts: 4,
            base_delay: Duration::from_secs(5),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(1),
        };
        assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(1));
        assert_eq!(policy.delay_for_attempt(4), Duration::from_secs(1));
    }

    #[test]
    fn a_runaway_or_invalid_factor_never_overflows_or_panics() {
        for factor in [f64::INFINITY, f64::NAN, 1e300, -2.0] {
            let policy = RetryPolicy {
                backoff_factor: factor,
                ..RetryPolicy::default()
            };
            for attempt in [2, 3, 50, 5000, usize::MAX] {
                let delay = policy.delay_for_attempt(attempt);
                assert!(
                    delay <= policy.max_delay,
                    "{factor} at {attempt}: {delay:?}"
                );
            }
        }
    }

    #[test]
    fn a_shrinking_factor_shortens_the_delay() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(800),
            backoff_factor: 0.5,
            max_delay: Duration::from_secs(5),
        };
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(800));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
        assert_eq!(policy.delay_for_attempt(4), Duration::from_millis(200));
    }

    #[test]
    fn the_seleniumbase_policy_matches_the_python_defaults() {
        let policy = RetryPolicy::seleniumbase();
        assert_eq!(policy.attempts(), 6);
        assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(1));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(2));
        assert_eq!(policy.delay_for_attempt(7), Duration::from_secs(32));
    }

    #[test]
    fn zero_attempts_still_runs_the_operation_once() {
        let mut calls = 0;
        let result: Result<(), &str> = quick(0).retry_blocking(
            || {
                calls += 1;
                Err("nope")
            },
            |_| true,
        );
        assert_eq!(result, Err("nope"));
        assert_eq!(calls, 1);
    }

    #[test]
    fn blocking_retry_succeeds_after_failures() {
        let mut calls = 0;
        let result = quick(3).retry_blocking(
            || {
                calls += 1;
                if calls < 3 {
                    Err("transient")
                } else {
                    Ok("ok")
                }
            },
            |_| true,
        );
        assert_eq!(result, Ok("ok"));
        assert_eq!(calls, 3);
    }

    #[test]
    fn blocking_retry_returns_the_last_error_when_exhausted() {
        let mut calls = 0;
        let result: Result<(), String> = quick(3).retry_blocking(
            || {
                calls += 1;
                Err(format!("failure {calls}"))
            },
            |_| true,
        );
        assert_eq!(result, Err("failure 3".to_owned()));
    }

    #[test]
    fn blocking_retry_does_not_repeat_a_permanent_error() {
        let mut calls = 0;
        let result: Result<(), &str> = quick(5).retry_blocking(
            || {
                calls += 1;
                Err("fatal")
            },
            |error| *error != "fatal",
        );
        assert_eq!(result, Err("fatal"));
        assert_eq!(calls, 1);
    }

    #[test]
    fn blocking_retry_waits_the_backoff_between_attempts() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_millis(20),
            backoff_factor: 1.0,
            max_delay: Duration::from_millis(20),
        };
        let started = Instant::now();
        let _: Result<(), ()> = policy.retry_blocking(|| Err(()), |_| true);
        // Two retries, 20 ms each.
        assert!(started.elapsed() >= Duration::from_millis(40));
    }

    #[tokio::test]
    async fn retry_succeeds_after_failures() {
        let mut attempts = 0;
        let result = quick(3)
            .retry(
                || {
                    attempts += 1;
                    async move {
                        if attempts < 3 {
                            Err::<i32, &str>("transient")
                        } else {
                            Ok(42)
                        }
                    }
                },
                |_| true,
            )
            .await;
        assert_eq!(result, Ok(42));
        assert_eq!(attempts, 3);
    }

    #[tokio::test]
    async fn retry_stops_at_a_permanent_error() {
        let mut attempts = 0;
        let result = quick(5)
            .retry(
                || {
                    attempts += 1;
                    async { Err::<(), _>(crate::SeleniumBaseError::invalid_config("bad")) }
                },
                crate::SeleniumBaseError::is_transient,
            )
            .await;
        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }

    #[tokio::test]
    async fn retry_gives_up_after_the_last_attempt() {
        let mut attempts = 0;
        let result = quick(4)
            .retry(
                || {
                    attempts += 1;
                    async { Err::<(), _>(crate::SeleniumBaseError::wait_timeout("x", None)) }
                },
                crate::SeleniumBaseError::is_transient,
            )
            .await;
        assert!(result.is_err());
        assert_eq!(attempts, 4);
    }

    #[tokio::test(start_paused = true)]
    async fn poll_until_returns_the_first_answer() {
        let mut polls = 0;
        let found = poll_until(
            Duration::from_secs(5),
            RetryPolicy::every(Duration::from_millis(50)),
            || {
                polls += 1;
                let ready = polls == 3;
                async move { ready.then_some(polls) }
            },
        )
        .await;
        assert_eq!(found, Some(3));
    }

    #[tokio::test(start_paused = true)]
    async fn poll_until_runs_once_even_with_no_time() {
        let mut polls = 0;
        let found: Option<()> = poll_until(Duration::ZERO, RetryPolicy::default(), || {
            polls += 1;
            async { None }
        })
        .await;
        assert_eq!(found, None);
        assert_eq!(polls, 1);
    }

    #[tokio::test]
    async fn poll_until_gives_up_at_the_deadline() {
        let started = Instant::now();
        let found: Option<()> = poll_until(
            Duration::from_millis(60),
            RetryPolicy::every(Duration::from_millis(10)),
            || async { None },
        )
        .await;
        assert_eq!(found, None);
        assert!(started.elapsed() >= Duration::from_millis(60));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn the_rate_limiter_lets_a_burst_through_then_waits() {
        let limiter = RateLimiter::new(2, Duration::from_secs(1));
        let t0 = Instant::now();
        assert_eq!(limiter.reserve_at(t0), Duration::ZERO);
        assert_eq!(limiter.reserve_at(t0), Duration::ZERO);
        // The third call must wait until the first is a window old.
        assert_eq!(limiter.reserve_at(t0), Duration::from_secs(1));
        // The fourth waits for the second call's window, which ended at t0 + 1s,
        // but the third already holds that slot.
        assert_eq!(limiter.reserve_at(t0), Duration::from_secs(1));
        // Then the next two are pushed a window further out.
        assert_eq!(limiter.reserve_at(t0), Duration::from_secs(2));
    }

    #[test]
    fn the_rate_limiter_forgets_calls_older_than_the_window() {
        let limiter = RateLimiter::new(1, Duration::from_secs(1));
        let t0 = Instant::now();
        assert_eq!(limiter.reserve_at(t0), Duration::ZERO);
        assert_eq!(
            limiter.reserve_at(t0 + Duration::from_millis(400)),
            Duration::from_millis(600)
        );
        // A call long after both goes straight through.
        assert_eq!(
            limiter.reserve_at(t0 + Duration::from_secs(60)),
            Duration::ZERO
        );
    }

    #[test]
    fn the_rate_limiter_never_allows_more_than_the_limit_in_a_window() {
        let limiter = RateLimiter::new(3, Duration::from_secs(1));
        let t0 = Instant::now();
        let starts: Vec<Instant> = (0..30)
            .map(|call| {
                let now = t0 + Duration::from_millis(call * 7);
                now + limiter.reserve_at(now)
            })
            .collect();
        for (i, start) in starts.iter().enumerate() {
            let in_window = starts
                .iter()
                .filter(|other| **other <= *start && *start - **other < Duration::from_secs(1))
                .count();
            assert!(
                in_window <= 3,
                "call {i} has {in_window} calls in its window"
            );
        }
    }

    #[test]
    fn a_zero_limit_is_treated_as_one() {
        let limiter = RateLimiter::new(0, Duration::from_secs(1));
        let t0 = Instant::now();
        assert_eq!(limiter.reserve_at(t0), Duration::ZERO);
        assert_eq!(limiter.reserve_at(t0), Duration::from_secs(1));
    }

    #[test]
    fn blocking_acquire_waits_out_the_window() {
        let limiter = RateLimiter::new(2, Duration::from_millis(100));
        let started = Instant::now();
        limiter.acquire_blocking();
        limiter.acquire_blocking();
        limiter.acquire_blocking();
        assert!(started.elapsed() >= Duration::from_millis(95));
    }

    #[tokio::test(start_paused = true)]
    async fn async_acquire_waits_without_blocking_the_executor() {
        let limiter = RateLimiter::per_second(2);
        let started = tokio::time::Instant::now();
        limiter.acquire().await;
        limiter.acquire().await;
        assert!(started.elapsed() < Duration::from_millis(50));
        limiter.acquire().await;
        // The paused clock jumped to the end of the window.
        assert!(started.elapsed() >= Duration::from_millis(900));
    }

    #[tokio::test(start_paused = true)]
    async fn one_limiter_shared_by_tasks_keeps_them_all_to_the_limit() {
        let limiter = std::sync::Arc::new(RateLimiter::per_second(2));
        let started = tokio::time::Instant::now();
        let tasks: Vec<_> = (0..4)
            .map(|_| {
                let limiter = std::sync::Arc::clone(&limiter);
                tokio::spawn(async move {
                    limiter.acquire().await;
                    tokio::time::Instant::now()
                })
            })
            .collect();
        let mut done = Vec::new();
        for task in tasks {
            done.push(task.await.unwrap() - started);
        }
        done.sort();
        // Two at once, then two a window later.
        assert!(done[1] < Duration::from_millis(50), "{done:?}");
        assert!(done[2] >= Duration::from_millis(900), "{done:?}");
    }
}
