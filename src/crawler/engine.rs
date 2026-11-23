//! High-throughput concurrent browser crawler engine.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::error::SeleniumBaseError;
use crate::sb_cdp::{BrowserPool, Page};

use super::builder::CrawlerBuilder;
use super::item::CrawlItem;

/// High-throughput concurrent web crawler powered by Tokio and [`BrowserPool`].
///
/// Features bounded concurrency, per-page context isolation, automatic retry
/// with backoff, and rate limiting.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::crawler::Crawler;
/// use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};
///
/// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let pool = BrowserPool::new(
///     LaunchOptions::builder().headless(true).build()?,
///     PoolOptions::builder().max_browsers(2).build()?,
/// );
///
/// let crawler = Crawler::builder().concurrency(4).build()?;
/// let urls = vec!["https://example.com/page1", "https://example.com/page2"];
///
/// let items = crawler.run(&pool, &urls, |page, url| async move {
///     page.goto(&url).await?;
///     let title = page.title().await?;
///     Ok(title)
/// }).await;
///
/// for item in items {
///     if let Ok(title) = item.result() {
///         println!("Crawled {}: {}", item.url(), title);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Crawler {
    concurrency: usize,
    rate_limit_per_sec: u32,
    retry_attempts: usize,
    timeout: Duration,
}

impl Crawler {
    /// Returns a [`CrawlerBuilder`] to configure a new crawler.
    #[must_use]
    pub fn builder() -> CrawlerBuilder {
        CrawlerBuilder::new()
    }

    pub(crate) fn from_builder(builder: CrawlerBuilder) -> Self {
        Self {
            concurrency: builder.concurrency,
            rate_limit_per_sec: builder.rate_limit_per_sec,
            retry_attempts: builder.retry_attempts,
            timeout: builder.timeout,
        }
    }

    /// Maximum concurrent worker tasks.
    #[must_use]
    pub const fn concurrency(&self) -> usize {
        self.concurrency
    }

    /// Rate limit in requests per second (0 means unthrottled).
    #[must_use]
    pub const fn rate_limit_per_sec(&self) -> u32 {
        self.rate_limit_per_sec
    }

    /// Number of retry attempts on failure.
    #[must_use]
    pub const fn retry_attempts(&self) -> usize {
        self.retry_attempts
    }

    /// Per-page navigation and execution timeout.
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Concurrently crawls a list of URLs using browsers leased from `pool`.
    ///
    /// Runs `visitor(page, url)` on each target with bounded concurrency and automatic retries.
    pub async fn run<T, F, Fut>(
        &self,
        pool: &BrowserPool,
        urls: &[impl AsRef<str>],
        visitor: F,
    ) -> Vec<CrawlItem<T>>
    where
        T: Send + 'static,
        F: Fn(Page, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, SeleniumBaseError>> + Send + 'static,
    {
        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let visitor = Arc::new(visitor);
        let timeout = self.timeout;
        let retry_attempts = self.retry_attempts;

        let mut handles = Vec::with_capacity(urls.len());

        for target in urls {
            let url = target.as_ref().to_string();
            let sem = Arc::clone(&semaphore);
            let vis = Arc::clone(&visitor);
            let pool_clone = pool.clone();

            if let Some(delay_ms) = (1000u64).checked_div(u64::from(self.rate_limit_per_sec)) {
                let delay = Duration::from_millis(delay_ms.max(1));
                sleep(delay).await;
            }

            let handle = tokio::spawn(async move {
                let _permit = match sem.acquire().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        return CrawlItem::new(
                            url,
                            Err(SeleniumBaseError::InvalidConfig(
                                "semaphore closed".to_string(),
                            )),
                            0,
                            Duration::ZERO,
                        );
                    }
                };

                let start = Instant::now();
                let mut attempt = 0;
                let mut last_err = None;

                while attempt <= retry_attempts {
                    attempt += 1;

                    let lease_res = pool_clone.acquire().await;
                    let lease = match lease_res {
                        Ok(l) => l,
                        Err(e) => {
                            last_err = Some(e);
                            continue;
                        }
                    };

                    let page = lease.page();

                    let crawl_work = vis(page.clone(), url.clone());
                    let res = tokio::time::timeout(timeout, crawl_work).await;

                    lease.release().await;

                    match res {
                        Ok(Ok(val)) => {
                            return CrawlItem::new(url, Ok(val), attempt, start.elapsed());
                        }
                        Ok(Err(err)) => {
                            last_err = Some(err);
                        }
                        Err(_) => {
                            last_err = Some(SeleniumBaseError::wait_timeout(
                                format!("crawl timeout for '{url}'"),
                                Some(timeout),
                            ));
                        }
                    }

                    if attempt <= retry_attempts {
                        let backoff = Duration::from_millis((150 * (1 << attempt)).min(2000));
                        sleep(backoff).await;
                    }
                }

                let final_err = last_err.unwrap_or_else(|| {
                    SeleniumBaseError::InvalidConfig("crawl attempts exhausted".to_string())
                });

                CrawlItem::new(url, Err(final_err), attempt, start.elapsed())
            });

            handles.push(handle);
        }

        let mut items = Vec::with_capacity(handles.len());
        for handle in handles {
            if let Ok(item) = handle.await {
                items.push(item);
            }
        }

        items
    }
}
