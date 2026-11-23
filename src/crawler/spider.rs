//! Recursive web spider and link discovery engine.
//!
//! Provides [`Spider`] and [`SpiderConfig`] to recursively traverse web pages,
//! apply domain restrictions, enforce regex allow/block patterns, deduplicate URLs,
//! and respect robots.txt crawling policies.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::time::sleep;

use crate::error::SeleniumBaseError;
use crate::sb_cdp::{BrowserPool, Page};

use super::builder::CrawlerBuilder;
use super::engine::Crawler;
use super::item::CrawlItem;
use super::robots::{fetch_robots_txt, RobotsPolicy};

/// Configuration for recursive crawling with [`Spider`].
///
/// Controls allowed domains, regex filtering rules, traversal depth, deduplication,
/// and robots.txt compliance.
#[derive(Debug, Clone)]
pub struct SpiderConfig {
    allowed_domains: HashSet<String>,
    allow_patterns: Vec<regex::Regex>,
    block_patterns: Vec<regex::Regex>,
    max_depth: usize,
    max_pages: usize,
    user_agent: String,
    respect_robots_txt: bool,
}

impl Default for SpiderConfig {
    fn default() -> Self {
        Self {
            allowed_domains: HashSet::new(),
            allow_patterns: Vec::new(),
            block_patterns: Vec::new(),
            max_depth: 3,
            max_pages: 100,
            user_agent: "SeleniumBaseSpider".to_string(),
            respect_robots_txt: true,
        }
    }
}

impl SpiderConfig {
    /// Returns a new builder to configure `SpiderConfig`.
    #[must_use]
    pub fn builder() -> SpiderConfigBuilder {
        SpiderConfigBuilder::new()
    }

    /// Set of domains permitted for traversal.
    #[must_use]
    pub const fn allowed_domains(&self) -> &HashSet<String> {
        &self.allowed_domains
    }

    /// Whitelist regular expressions for permitted URLs.
    #[must_use]
    pub fn allow_patterns(&self) -> &[regex::Regex] {
        &self.allow_patterns
    }

    /// Blacklist regular expressions for forbidden URLs.
    #[must_use]
    pub fn block_patterns(&self) -> &[regex::Regex] {
        &self.block_patterns
    }

    /// Maximum link depth to traverse from starting URLs.
    #[must_use]
    pub const fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Maximum total pages to crawl across the run.
    #[must_use]
    pub const fn max_pages(&self) -> usize {
        self.max_pages
    }

    /// User-agent string reported for robots.txt checking.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Whether robots.txt rules are respected.
    #[must_use]
    pub const fn respect_robots_txt(&self) -> bool {
        self.respect_robots_txt
    }

    /// Evaluates whether a candidate URL should be crawled given its depth,
    /// visited history, domain rules, regex patterns, and robots policy.
    #[must_use]
    pub fn should_visit(
        &self,
        url_str: &str,
        depth: usize,
        visited: &HashSet<String>,
        robots_policy: Option<&RobotsPolicy<'_>>,
    ) -> bool {
        if depth > self.max_depth {
            return false;
        }

        if visited.contains(url_str) {
            return false;
        }

        let Ok(parsed) = url::Url::parse(url_str) else {
            return false;
        };

        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return false;
        }

        if !self.allowed_domains.is_empty() {
            let host = parsed.host_str().unwrap_or("");
            let allowed = self
                .allowed_domains
                .iter()
                .any(|d| host == d || host.ends_with(&format!(".{d}")));
            if !allowed {
                return false;
            }
        }

        for block in &self.block_patterns {
            if block.is_match(url_str) {
                return false;
            }
        }

        if !self.allow_patterns.is_empty()
            && !self
                .allow_patterns
                .iter()
                .any(|allow| allow.is_match(url_str))
        {
            return false;
        }

        if self.respect_robots_txt
            && let Some(policy) = robots_policy
            && !policy.is_allowed(&self.user_agent, url_str)
        {
            return false;
        }

        true
    }
}

/// Builder for constructing a [`SpiderConfig`].
#[derive(Debug, Clone, Default)]
pub struct SpiderConfigBuilder {
    allowed_domains: HashSet<String>,
    allow_patterns: Vec<regex::Regex>,
    block_patterns: Vec<regex::Regex>,
    max_depth: usize,
    max_pages: usize,
    user_agent: String,
    respect_robots_txt: bool,
}

impl SpiderConfigBuilder {
    /// Creates a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        let defaults = SpiderConfig::default();
        Self {
            allowed_domains: defaults.allowed_domains,
            allow_patterns: defaults.allow_patterns,
            block_patterns: defaults.block_patterns,
            max_depth: defaults.max_depth,
            max_pages: defaults.max_pages,
            user_agent: defaults.user_agent,
            respect_robots_txt: defaults.respect_robots_txt,
        }
    }

    /// Adds a domain to the allowed list (e.g. `"example.com"`).
    #[must_use]
    pub fn allowed_domain(mut self, domain: impl Into<String>) -> Self {
        self.allowed_domains.insert(domain.into());
        self
    }

    /// Adds multiple domains to the allowed list.
    #[must_use]
    pub fn allowed_domains<I, S>(mut self, domains: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for d in domains {
            self.allowed_domains.insert(d.into());
        }
        self
    }

    /// Adds an allow regex pattern. If configured, only matching URLs are crawled.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the pattern fails to compile.
    pub fn allow_pattern(mut self, pattern: &str) -> Result<Self, SeleniumBaseError> {
        let regex = regex::Regex::new(pattern)
            .map_err(|e| SeleniumBaseError::InvalidConfig(format!("invalid allow pattern: {e}")))?;
        self.allow_patterns.push(regex);
        Ok(self)
    }

    /// Adds a block regex pattern. URLs matching any block pattern are skipped.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the pattern fails to compile.
    pub fn block_pattern(mut self, pattern: &str) -> Result<Self, SeleniumBaseError> {
        let regex = regex::Regex::new(pattern)
            .map_err(|e| SeleniumBaseError::InvalidConfig(format!("invalid block pattern: {e}")))?;
        self.block_patterns.push(regex);
        Ok(self)
    }

    /// Appends a precompiled allow [`regex::Regex`].
    #[must_use]
    pub fn allow_regex(mut self, regex: regex::Regex) -> Self {
        self.allow_patterns.push(regex);
        self
    }

    /// Appends a precompiled block [`regex::Regex`].
    #[must_use]
    pub fn block_regex(mut self, regex: regex::Regex) -> Self {
        self.block_patterns.push(regex);
        self
    }

    /// Sets the maximum recursive crawling depth.
    #[must_use]
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Sets the maximum number of pages to crawl.
    #[must_use]
    pub fn max_pages(mut self, pages: usize) -> Self {
        self.max_pages = pages;
        self
    }

    /// Sets the user agent used for robots.txt checks.
    #[must_use]
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    /// Configures whether `robots.txt` rules should be respected.
    #[must_use]
    pub fn respect_robots_txt(mut self, respect: bool) -> Self {
        self.respect_robots_txt = respect;
        self
    }

    /// Validates and constructs the [`SpiderConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `max_pages` is 0.
    pub fn build(self) -> Result<SpiderConfig, SeleniumBaseError> {
        if self.max_pages == 0 {
            return Err(SeleniumBaseError::InvalidConfig(
                "Spider max_pages must be at least 1".to_string(),
            ));
        }

        Ok(SpiderConfig {
            allowed_domains: self.allowed_domains,
            allow_patterns: self.allow_patterns,
            block_patterns: self.block_patterns,
            max_depth: self.max_depth,
            max_pages: self.max_pages,
            user_agent: self.user_agent,
            respect_robots_txt: self.respect_robots_txt,
        })
    }
}

/// Recursive web spider engine.
///
/// Discovers URLs dynamically from visited pages up to configured depth and page limits,
/// filtering by domain and regex rules while deduplicating URLs.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::crawler::{Spider, SpiderConfig};
/// use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};
///
/// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let pool = BrowserPool::new(
///     LaunchOptions::builder().headless(true).build()?,
///     PoolOptions::builder().max_browsers(2).build()?,
/// );
///
/// let config = Spider::builder()
///     .allowed_domain("example.com")
///     .max_depth(2)
///     .max_pages(20)
///     .build()?;
///
/// let spider = Spider::new(config);
/// let items = spider.crawl(&pool, &["https://example.com"], |page, url| async move {
///     page.goto(&url).await?;
///     let title = page.title().await?;
///     Ok(title)
/// }).await;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Spider {
    config: SpiderConfig,
}

impl Spider {
    /// Creates a new spider with the given configuration.
    #[must_use]
    pub fn new(config: SpiderConfig) -> Self {
        Self { config }
    }

    /// Returns a [`SpiderConfigBuilder`] to build configuration.
    #[must_use]
    pub fn builder() -> SpiderConfigBuilder {
        SpiderConfigBuilder::new()
    }

    /// Returns a reference to the active spider configuration.
    #[must_use]
    pub const fn config(&self) -> &SpiderConfig {
        &self.config
    }

    /// Recursively crawls starting from `start_urls` using a default [`Crawler`].
    pub async fn crawl<T, F, Fut>(
        &self,
        pool: &BrowserPool,
        start_urls: &[impl AsRef<str>],
        visitor: F,
    ) -> Vec<CrawlItem<T>>
    where
        T: Send + 'static,
        F: Fn(Page, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<T, SeleniumBaseError>> + Send + 'static,
    {
        let default_crawler = CrawlerBuilder::default()
            .concurrency(4)
            .build()
            .unwrap_or_else(|_| Crawler::from_builder(CrawlerBuilder::default()));
        self.crawl_with_crawler(&default_crawler, pool, start_urls, visitor)
            .await
    }

    /// Recursively crawls starting from `start_urls` using custom [`Crawler`] engine settings.
    pub async fn crawl_with_crawler<T, F, Fut>(
        &self,
        crawler: &Crawler,
        pool: &BrowserPool,
        start_urls: &[impl AsRef<str>],
        visitor: F,
    ) -> Vec<CrawlItem<T>>
    where
        T: Send + 'static,
        F: Fn(Page, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<T, SeleniumBaseError>> + Send + 'static,
    {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut results: Vec<CrawlItem<T>> = Vec::new();
        let mut robots_cache: std::collections::HashMap<String, RobotsPolicy<'static>> =
            std::collections::HashMap::new();

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        let visitor = Arc::new(visitor);

        for seed in start_urls {
            if let Some(norm) = normalize_url(seed.as_ref(), None)
                && !visited.contains(&norm)
            {
                visited.insert(norm.clone());
                queue.push_back((norm, 0));
            }
        }

        while !queue.is_empty() && results.len() < self.config.max_pages {
            let remaining_capacity = self.config.max_pages.saturating_sub(results.len());
            let batch_size = remaining_capacity.min(crawler.concurrency()).max(1);

            let mut batch = Vec::with_capacity(batch_size);
            for _ in 0..batch_size {
                if let Some(item) = queue.pop_front() {
                    batch.push(item);
                } else {
                    break;
                }
            }

            if batch.is_empty() {
                break;
            }

            for (url, depth) in batch {
                if results.len() >= self.config.max_pages {
                    break;
                }

                if !self
                    .check_robots_allowed(&url, &http_client, &mut robots_cache)
                    .await
                {
                    continue;
                }

                if let Some(delay_ms) =
                    (1000u64).checked_div(u64::from(crawler.rate_limit_per_sec()))
                {
                    sleep(Duration::from_millis(delay_ms.max(1))).await;
                }

                let (item, discovered) =
                    Self::visit_page(pool, &url, Arc::clone(&visitor), crawler.timeout()).await;
                results.push(item);

                if depth < self.config.max_depth {
                    self.enqueue_discovered_links(
                        discovered,
                        depth,
                        &mut visited,
                        &mut queue,
                        &robots_cache,
                    );
                }
            }
        }

        results
    }

    async fn check_robots_allowed(
        &self,
        url: &str,
        client: &reqwest::Client,
        cache: &mut std::collections::HashMap<String, RobotsPolicy<'static>>,
    ) -> bool {
        if !self.config.respect_robots_txt {
            return true;
        }

        let Ok(parsed_url) = url::Url::parse(url) else {
            return true;
        };

        let host = parsed_url.host_str().unwrap_or("").to_string();
        if host.is_empty() {
            return true;
        }

        if !cache.contains_key(&host) {
            let base = format!("{}://{}", parsed_url.scheme(), host);
            let policy = match fetch_robots_txt(client, &base).await {
                Ok(txt) => RobotsPolicy::parse(&txt).into_owned(),
                Err(_) => RobotsPolicy::default(),
            };
            cache.insert(host.clone(), policy);
        }

        cache
            .get(&host)
            .is_none_or(|p| p.is_allowed(&self.config.user_agent, url))
    }

    async fn visit_page<T, F, Fut>(
        pool: &BrowserPool,
        url: &str,
        visitor: Arc<F>,
        timeout: Duration,
    ) -> (CrawlItem<T>, Vec<String>)
    where
        T: Send + 'static,
        F: Fn(Page, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<T, SeleniumBaseError>> + Send + 'static,
    {
        let start = Instant::now();
        let lease_res = pool.acquire().await;
        let lease = match lease_res {
            Ok(l) => l,
            Err(e) => {
                return (
                    CrawlItem::new(url.to_string(), Err(e), 1, start.elapsed()),
                    Vec::new(),
                );
            }
        };

        let page = lease.page();
        let visit_fut = visitor(page.clone(), url.to_string());

        let res = tokio::time::timeout(timeout, visit_fut).await;
        let discovered = Self::extract_page_links(page, url).await;

        lease.release().await;

        let outcome = match res {
            Ok(val) => val,
            Err(_) => Err(SeleniumBaseError::wait_timeout(
                format!("crawl timeout for '{url}'"),
                Some(timeout),
            )),
        };

        (
            CrawlItem::new(url.to_string(), outcome, 1, start.elapsed()),
            discovered,
        )
    }

    fn enqueue_discovered_links(
        &self,
        discovered: Vec<String>,
        depth: usize,
        visited: &mut HashSet<String>,
        queue: &mut VecDeque<(String, usize)>,
        robots_cache: &std::collections::HashMap<String, RobotsPolicy<'static>>,
    ) {
        for link in discovered {
            let link_host = url::Url::parse(&link)
                .ok()
                .and_then(|u| u.host_str().map(ToString::to_string))
                .unwrap_or_default();

            let policy_ref = robots_cache.get(&link_host);
            if self
                .config
                .should_visit(&link, depth + 1, visited, policy_ref)
            {
                visited.insert(link.clone());
                queue.push_back((link, depth + 1));
            }
        }
    }

    async fn extract_page_links(page: &Page, base_url: &str) -> Vec<String> {
        let js = "Array.from(document.querySelectorAll('a[href]'), a => a.href)";
        if let Ok(links) = page.evaluate_as::<Vec<String>>(js).await {
            let mut clean = Vec::new();
            for link in links {
                if let Some(norm) = normalize_url(&link, Some(base_url)) {
                    clean.push(norm);
                }
            }
            if !clean.is_empty() {
                return clean;
            }
        }

        if let Ok(html) = page.content().await {
            return extract_links_from_html(&html, Some(base_url));
        }

        Vec::new()
    }
}

/// Normalizes a candidate URL against an optional base URL.
///
/// Strips fragments (`#...`), ensures HTTP or HTTPS scheme, and returns an absolute URL.
#[must_use]
pub fn normalize_url(raw_url: &str, base_url: Option<&str>) -> Option<String> {
    let parsed = if let Some(base) = base_url {
        let base_parsed = url::Url::parse(base).ok()?;
        base_parsed.join(raw_url).ok()?
    } else {
        url::Url::parse(raw_url).ok()?
    };

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }

    let mut clean = parsed;
    clean.set_fragment(None);
    Some(clean.to_string())
}

/// Extracts all valid HTTP(S) links from an HTML document string.
///
/// Resolves relative URLs using `base_url` when provided.
#[must_use]
pub fn extract_links_from_html(html: &str, base_url: Option<&str>) -> Vec<String> {
    let mut links = Vec::new();
    let mut remaining = html;

    while let Some(start_idx) = remaining.find("href=") {
        let after_href = remaining[start_idx + 5..].trim_start();
        if after_href.is_empty() {
            break;
        }

        let first_char = after_href.chars().next().unwrap_or(' ');
        let (raw_href, next_pos) = if first_char == '"' || first_char == '\'' {
            let quote = first_char;
            let rest = &after_href[quote.len_utf8()..];
            if let Some(end_idx) = rest.find(quote) {
                (
                    &rest[..end_idx],
                    start_idx + 5 + quote.len_utf8() + end_idx + 1,
                )
            } else {
                break;
            }
        } else {
            let end_idx = after_href
                .find(|c: char| c.is_whitespace() || c == '>')
                .unwrap_or(after_href.len());
            let trimmed = after_href[..end_idx].trim_end_matches('/');
            (trimmed, start_idx + 5 + end_idx)
        };

        if let Some(norm) = normalize_url(raw_href, base_url) {
            links.push(norm);
        }

        if next_pos < remaining.len() {
            remaining = &remaining[next_pos..];
        } else {
            break;
        }
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spider_config_builder_and_defaults() {
        let config = SpiderConfig::builder()
            .allowed_domain("example.com")
            .max_depth(4)
            .max_pages(50)
            .allow_pattern(r"^https://example\.com/blog/.*")
            .unwrap()
            .block_pattern(r".*\?format=json$")
            .unwrap()
            .respect_robots_txt(false)
            .build()
            .unwrap();

        assert_eq!(config.max_depth(), 4);
        assert_eq!(config.max_pages(), 50);
        assert!(!config.respect_robots_txt());
        assert!(config.allowed_domains().contains("example.com"));
        assert_eq!(config.allow_patterns().len(), 1);
        assert_eq!(config.block_patterns().len(), 1);

        let invalid = SpiderConfig::builder().max_pages(0).build();
        assert!(invalid.is_err());
    }

    #[test]
    fn url_normalization_and_filtering() {
        assert_eq!(
            normalize_url("https://example.com/page#section", None),
            Some("https://example.com/page".to_string())
        );

        assert_eq!(
            normalize_url("/about/team", Some("https://example.com/company")),
            Some("https://example.com/about/team".to_string())
        );

        assert_eq!(normalize_url("javascript:void(0)", None), None);
        assert_eq!(normalize_url("mailto:info@example.com", None), None);
    }

    #[test]
    fn html_link_extraction() {
        let html = r#"
            <div>
                <a href="https://example.com/blog/1">Post 1</a>
                <a href="/blog/2">Post 2</a>
                <a href='https://example.com/about'>About</a>
                <a href="javascript:alert(1)">Invalid</a>
            </div>
        "#;

        let links = extract_links_from_html(html, Some("https://example.com"));
        assert_eq!(
            links,
            vec![
                "https://example.com/blog/1",
                "https://example.com/blog/2",
                "https://example.com/about",
            ]
        );
    }

    #[test]
    fn spider_should_visit_domain_restriction() {
        let config = SpiderConfig::builder()
            .allowed_domain("example.com")
            .build()
            .unwrap();

        let visited = HashSet::new();
        assert!(config.should_visit("https://example.com/page", 1, &visited, None));
        assert!(config.should_visit("https://sub.example.com/page", 1, &visited, None));
        assert!(!config.should_visit("https://other.com/page", 1, &visited, None));
    }

    #[test]
    fn spider_should_visit_regex_allow_and_block() {
        let config = SpiderConfig::builder()
            .allow_pattern(r"^https://example\.com/allowed/.*")
            .unwrap()
            .block_pattern(r".*\.pdf$")
            .unwrap()
            .build()
            .unwrap();

        let visited = HashSet::new();
        assert!(config.should_visit("https://example.com/allowed/article", 1, &visited, None));
        assert!(!config.should_visit("https://example.com/allowed/doc.pdf", 1, &visited, None));
        assert!(!config.should_visit("https://example.com/other/page", 1, &visited, None));
    }

    #[test]
    fn spider_deduplication_via_hashset() {
        let config = SpiderConfig::default();
        let mut visited = HashSet::new();
        visited.insert("https://example.com/visited".to_string());

        assert!(!config.should_visit("https://example.com/visited", 1, &visited, None));
        assert!(config.should_visit("https://example.com/new", 1, &visited, None));
    }

    #[test]
    fn spider_respects_robots_policy() {
        let config = SpiderConfig::builder()
            .user_agent("TestBot")
            .respect_robots_txt(true)
            .build()
            .unwrap();

        let robots = RobotsPolicy::parse("User-agent: TestBot\nDisallow: /admin\n");
        let visited = HashSet::new();

        assert!(!config.should_visit(
            "https://example.com/admin/settings",
            1,
            &visited,
            Some(&robots)
        ));
        assert!(config.should_visit("https://example.com/public", 1, &visited, Some(&robots)));

        let lenient_config = SpiderConfig::builder()
            .respect_robots_txt(false)
            .build()
            .unwrap();
        assert!(lenient_config.should_visit(
            "https://example.com/admin/settings",
            1,
            &visited,
            Some(&robots)
        ));
    }

    #[test]
    fn spider_max_depth_enforcement() {
        let config = SpiderConfig::builder().max_depth(2).build().unwrap();
        let visited = HashSet::new();

        assert!(config.should_visit("https://example.com/0", 0, &visited, None));
        assert!(config.should_visit("https://example.com/1", 1, &visited, None));
        assert!(config.should_visit("https://example.com/2", 2, &visited, None));
        assert!(!config.should_visit("https://example.com/3", 3, &visited, None));
    }

    #[test]
    fn unquoted_and_mixed_html_links() {
        let html = r#"
            <a href=https://example.com/unquoted>Unquoted</a>
            <a href='https://example.com/single'>Single</a>
            <a href="https://example.com/double">Double</a>
            <a href="/relative">Relative</a>
        "#;
        let links = extract_links_from_html(html, Some("https://example.com"));
        assert_eq!(
            links,
            vec![
                "https://example.com/unquoted",
                "https://example.com/single",
                "https://example.com/double",
                "https://example.com/relative",
            ]
        );
    }

    #[test]
    fn spider_precompiled_regex() {
        let allow_rx = regex::Regex::new(r".*/allowed/.*").unwrap();
        let block_rx = regex::Regex::new(r".*/blocked/.*").unwrap();

        let config = SpiderConfig::builder()
            .allow_regex(allow_rx)
            .block_regex(block_rx)
            .build()
            .unwrap();

        let visited = HashSet::new();
        assert!(config.should_visit("https://example.com/allowed/doc", 0, &visited, None));
        assert!(!config.should_visit("https://example.com/allowed/blocked/doc", 0, &visited, None));
    }
}
