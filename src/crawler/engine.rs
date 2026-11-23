//! High-throughput concurrent browser crawler engine.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::error::SeleniumBaseError;
use crate::sb_cdp::{BrowserPool, Page};

use super::builder::CrawlerBuilder;
use super::item::CrawlItem;
use super::robots::{fetch_robots_txt, RobotsPolicy};
use super::sitemap::fetch_sitemap_urls;
use super::spider::Spider;

/// High-throughput concurrent web crawler powered by Tokio and [`BrowserPool`].
///
/// Features bounded concurrency, per-page context isolation, automatic retry
/// with backoff, rate limiting, and integrated robots/sitemap support.
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
    user_agent: Option<String>,
    respect_robots_txt: bool,
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
            user_agent: builder.user_agent,
            respect_robots_txt: builder.respect_robots_txt,
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

    /// Custom user-agent string configured for this crawler, if any.
    #[must_use]
    pub fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }

    /// Whether robots.txt restrictions are respected.
    #[must_use]
    pub const fn respect_robots_txt(&self) -> bool {
        self.respect_robots_txt
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

        let mut robots_cache: HashMap<String, RobotsPolicy<'static>> = HashMap::new();
        let http_client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        let mut handles = Vec::with_capacity(urls.len());

        for target in urls {
            let url = target.as_ref().to_string();

            if self.respect_robots_txt
                && let Ok(parsed_url) = url::Url::parse(&url)
            {
                let host = parsed_url.host_str().unwrap_or("").to_string();
                if !host.is_empty() && !robots_cache.contains_key(&host) {
                    let base = format!("{}://{}", parsed_url.scheme(), host);
                    let policy = match fetch_robots_txt(&http_client, &base).await {
                        Ok(txt) => RobotsPolicy::parse(&txt).into_owned(),
                        Err(_) => RobotsPolicy::default(),
                    };
                    robots_cache.insert(host.clone(), policy);
                }

                if let Some(policy) = robots_cache.get(&host) {
                    let ua = self.user_agent.as_deref().unwrap_or("SeleniumBaseCrawler");
                    if !policy.is_allowed(ua, &url) {
                        handles.push(tokio::spawn(async move {
                            CrawlItem::new(
                                url.clone(),
                                Err(SeleniumBaseError::InvalidConfig(format!(
                                    "URL blocked by robots.txt: {url}"
                                ))),
                                0,
                                Duration::ZERO,
                            )
                        }));
                        continue;
                    }
                }
            }

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

                crawl_worker_loop(pool_clone, url, vis, timeout, retry_attempts).await
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

    /// Discovers target URLs from an XML sitemap and crawls them concurrently.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if fetching or parsing the sitemap fails.
    pub async fn crawl_sitemap<T, F, Fut>(
        &self,
        pool: &BrowserPool,
        sitemap_url: &str,
        visitor: F,
    ) -> Result<Vec<CrawlItem<T>>, SeleniumBaseError>
    where
        T: Send + 'static,
        F: Fn(Page, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, SeleniumBaseError>> + Send + 'static,
    {
        let http_client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .unwrap_or_default();

        let urls = fetch_sitemap_urls(&http_client, sitemap_url).await?;
        Ok(self.run(pool, &urls, visitor).await)
    }

    /// Recursively crawls pages according to [`SpiderConfig`](super::spider::SpiderConfig)
    /// rules using this crawler engine.
    pub async fn crawl_spider<T, F, Fut>(
        &self,
        pool: &BrowserPool,
        spider: &Spider,
        start_urls: &[impl AsRef<str>],
        visitor: F,
    ) -> Vec<CrawlItem<T>>
    where
        T: Send + 'static,
        F: Fn(Page, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, SeleniumBaseError>> + Send + 'static,
    {
        spider
            .crawl_with_crawler(self, pool, start_urls, visitor)
            .await
    }
}

async fn crawl_worker_loop<T, F, Fut>(
    pool: BrowserPool,
    url: String,
    vis: Arc<F>,
    timeout: Duration,
    retry_attempts: usize,
) -> CrawlItem<T>
where
    T: Send + 'static,
    F: Fn(Page, String) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<T, SeleniumBaseError>> + Send + 'static,
{
    let start = Instant::now();
    let mut attempt = 0;
    let mut last_err = None;

    while attempt <= retry_attempts {
        attempt += 1;

        let lease_res = pool.acquire().await;
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
}
