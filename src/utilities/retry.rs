//! Retry helpers for transient WebDriver and network failures.

use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

/// A simple fixed/linear backoff retry policy.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the first).
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

    /// Delay before attempt `n` (1-indexed). The first attempt has no delay.
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        if attempt <= 1 {
            return Duration::ZERO;
        }
        let exp = (attempt - 1) as f64;
        let ms = self.base_delay.as_millis() as f64 * self.backoff_factor.powf(exp - 1.0);
        let ms = ms.min(self.max_delay.as_millis() as f64);
        Duration::from_millis(ms as u64).max(self.base_delay)
    }

    /// Retry an async operation until it succeeds or the policy is exhausted.
    ///
    /// `is_transient` decides whether a given error deserves another attempt.
    pub async fn retry<F, Fut, T, E>(
        &self,
        mut op: F,
        is_transient: impl Fn(&E) -> bool,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut last_err = None;
        for attempt in 1..=self.max_attempts {
            let delay = self.delay_for_attempt(attempt);
            if delay > Duration::ZERO {
                tokio::time::sleep(delay).await;
            }
            debug!(attempt, max_attempts = self.max_attempts, "retry operation");
            match op().await {
                Ok(value) => return Ok(value),
                Err(err) if is_transient(&err) && attempt < self.max_attempts => {
                    warn!(attempt, "transient failure; retrying");
                    last_err = Some(err);
                }
                Err(err) => return Err(err),
            }
        }
        Err(last_err.expect("at least one attempt"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn retry_succeeds_after_failures() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            backoff_factor: 1.0,
            max_delay: Duration::from_millis(10),
        };
        let mut attempts = 0;
        let result = policy
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
}
