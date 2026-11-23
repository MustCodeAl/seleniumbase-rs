//! Builder for configuring concurrent [`Crawler`](super::Crawler) instances.

use super::engine::Crawler;
use crate::error::SeleniumBaseError;
use std::time::Duration;

/// Builder for constructing a [`Crawler`](super::Crawler).
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use seleniumbase_rs::crawler::Crawler;
///
/// let crawler = Crawler::builder()
///     .concurrency(8)
///     .rate_limit_per_sec(10)
///     .retry_attempts(3)
///     .timeout(Duration::from_secs(20))
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct CrawlerBuilder {
    pub(crate) concurrency: usize,
    pub(crate) rate_limit_per_sec: u32,
    pub(crate) retry_attempts: usize,
    pub(crate) timeout: Duration,
}

impl Default for CrawlerBuilder {
    fn default() -> Self {
        Self {
            concurrency: 4,
            rate_limit_per_sec: 0,
            retry_attempts: 2,
            timeout: Duration::from_secs(30),
        }
    }
}

impl CrawlerBuilder {
    /// Creates a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of concurrent pages/contexts to run simultaneously.
    ///
    /// Must be greater than 0. Defaults to 4.
    #[must_use]
    pub fn concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency;
        self
    }

    /// Sets an optional rate limit in maximum requests dispatched per second.
    ///
    /// Set to 0 for unlimited throughput. Defaults to 0.
    #[must_use]
    pub fn rate_limit_per_sec(mut self, per_sec: u32) -> Self {
        self.rate_limit_per_sec = per_sec;
        self
    }

    /// Number of retry attempts on navigation or extraction failures.
    ///
    /// Defaults to 2.
    #[must_use]
    pub fn retry_attempts(mut self, retries: usize) -> Self {
        self.retry_attempts = retries;
        self
    }

    /// Per-page navigation and execution timeout.
    ///
    /// Defaults to 30 seconds.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Validates configuration and constructs a [`Crawler`](super::Crawler).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `concurrency` is 0.
    pub fn build(self) -> Result<Crawler, SeleniumBaseError> {
        if self.concurrency == 0 {
            return Err(SeleniumBaseError::InvalidConfig(
                "Crawler concurrency must be at least 1".to_string(),
            ));
        }

        Ok(Crawler::from_builder(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults_and_validation() {
        let b = CrawlerBuilder::default();
        let crawler = b.build().unwrap();
        assert_eq!(crawler.concurrency(), 4);
        assert_eq!(crawler.rate_limit_per_sec(), 0);
        assert_eq!(crawler.retry_attempts(), 2);

        let invalid = CrawlerBuilder::default().concurrency(0).build();
        assert!(invalid.is_err());
    }
}
