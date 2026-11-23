//! Results and telemetry produced by the crawler.

use crate::error::SeleniumBaseError;
use std::time::Duration;

/// The result of crawling an individual URL.
#[derive(Debug)]
pub struct CrawlItem<T> {
    url: String,
    result: Result<T, SeleniumBaseError>,
    attempts: usize,
    duration: Duration,
}

impl<T> CrawlItem<T> {
    /// Creates a new `CrawlItem`.
    #[must_use]
    pub fn new(
        url: String,
        result: Result<T, SeleniumBaseError>,
        attempts: usize,
        duration: Duration,
    ) -> Self {
        Self {
            url,
            result,
            attempts,
            duration,
        }
    }

    /// The target URL that was crawled.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The extraction outcome.
    pub fn result(&self) -> Result<&T, &SeleniumBaseError> {
        self.result.as_ref()
    }

    /// Consumes the item and returns the inner result.
    pub fn into_result(self) -> Result<T, SeleniumBaseError> {
        self.result
    }

    /// Total number of attempts taken.
    #[must_use]
    pub const fn attempts(&self) -> usize {
        self.attempts
    }

    /// Total duration elapsed during the crawl of this URL.
    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }

    /// Returns `true` if the crawl succeeded.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Returns `true` if the crawl encountered an error.
    #[must_use]
    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }
}
