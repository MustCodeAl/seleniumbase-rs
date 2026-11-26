---
title: "Autonomous Web Crawling & Spidering"
sidebarTitle: "Web Crawling & Spidering"
description: "Concurrent web crawling with Tokio and BrowserPool, RFC 9309 robots.txt compliance, XML sitemaps, and recursive depth-bounded spidering."
---

Web scraping at scale requires balancing throughput, polite adherence to server guidelines, bounded memory consumption, and browser context isolation.

<Note>

Since v0.10.0, crawling, `robots.txt` evaluation, XML sitemap parsing, and spidering live in `seleniumbase-rs-plugin-crawler` inside [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins) (enabled via `features = ["plugin-crawler"]` or directly as a crate dependency; see the [crawler Plugin](/plugins/crawler) reference).

</Note>

`seleniumbase-rs-plugin-crawler` provides crawling and spidering utilities designed for headless browser workloads:

- **Concurrent Crawler (`Crawler`, `CrawlerBuilder`)**: Manages bounded concurrent browser contexts, rate limiting, and automatic retry backoff on top of [`BrowserPool`](/tutorials/pure_cdp).
- **XML Sitemap Discovery (`sitemap`)**: Zero-dependency extraction of URL sets and sitemap indexes from XML documents.
- **RFC 9309 `robots.txt` Compliance (`RobotsPolicy`)**: Standardized parser for `User-agent`, `Allow`, `Disallow`, `Crawl-delay`, and `Sitemap` directives.
- **Recursive Spider (`Spider`, `SpiderConfig`)**: Breadth-first web crawler with link extraction, depth boundaries, domain fencing, and URL deduplication via `HashSet`.

<Info>

**What you will learn**

- How to crawl hundreds of URLs concurrently with bounded browser leases.
- How to discover target endpoints from `sitemap.xml` files.
- How to evaluate `robots.txt` policies before dispatching requests.
- How to run recursive spidering campaigns with domain boundaries and link deduplication.

</Info>

<Visibility for="agents">

Summary for AI exports: Since v0.10.0, crawling moved out of `seleniumbase-rs` core into `seleniumbase-rs-plugin-crawler` (see `/plugins/crawler`). It provides `CrawlerPlugin` (`[plugins.crawler]`), self-contained RFC 9309 `robots` evaluation, XML `sitemap::parse_sitemap_urls`, and breadth-first crawling over pure CDP.

</Visibility>

---

## The Concurrent Crawler Pipeline

The `Crawler` engine executes a list of target URLs across a [`BrowserPool`](/tutorials/pure_cdp). Each worker thread leases an isolated context and tab from the pool, navigates to the URL, and executes a user-provided closure.

```rust
use std::time::Duration;
use seleniumbase_rs_plugin_crawler::{Crawler, CrawlItem};
use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};
use seleniumbase_rs::{tokio, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize an isolated browser pool
    let pool = BrowserPool::new(
        LaunchOptions::builder().headless(true).build()?,
        PoolOptions::builder().max_browsers(2).build()?,
    );

    // 2. Configure crawler concurrency and rate limits
    let crawler = Crawler::builder()
        .concurrency(4)                   // Up to 4 concurrent page contexts
        .rate_limit_per_sec(5)             // Max 5 page requests per second
        .retry_attempts(2)                 // Automatic retry on network glitch
        .timeout(Duration::from_secs(20))  // Per-page timeout
        .build()?;

    let target_urls = vec![
        "https://example.com/products/1",
        "https://example.com/products/2",
        "https://example.com/products/3",
    ];

    // 3. Execute extraction closure
    let items: Vec<CrawlItem<String>> = crawler.run(&pool, &target_urls, |page, url| async move {
        page.goto(&url).await?;
        let heading = page.locator("h1").text().await?;
        Ok(heading)
    }).await;

    // 4. Process aggregated results
    for item in items {
        match item.result() {
            Ok(heading) => println!("Success: {} -> {}", item.url(), heading),
            Err(e) => eprintln!("Failed: {} -> {}", item.url(), e),
        }
    }

    pool.close().await?;
    Ok(())
}
```

---

## XML Sitemap Discovery (`sitemap`)

Many sites publish complete indices of their content in standard XML sitemaps. `seleniumbase_rs_plugin_crawler::sitemap` extracts all `<loc>` entries without pulling in heavyweight XML libraries.

```rust
use seleniumbase_rs_plugin_crawler::sitemap::parse_sitemap_urls;
use seleniumbase_rs::{tokio, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::new();

    // Fetch and parse sitemap URLs automatically over HTTP
    let urls = fetch_sitemap_urls(&client, "https://example.com/sitemap.xml").await?;
    println!("Found {} target URLs from sitemap", urls.len());

    // Or parse raw sitemap XML content directly
    let raw_xml = r#"
        <?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url><loc>https://example.com/docs/intro</loc></url>
            <url><loc>https://example.com/docs/install</loc></url>
        </urlset>
    "#;
    let parsed_urls = parse_sitemap_urls(raw_xml);
    assert_eq!(parsed_urls.len(), 2);

    Ok(())
}
```

---

## RFC 9309 `robots.txt` Compliance (`RobotsPolicy`)

Well-behaved scrapers respect website crawl permissions. `RobotsPolicy` evaluates whether a path is accessible for a specific user-agent, respects crawl delays, and discovers linked sitemaps:

```rust
use seleniumbase_rs_plugin_crawler::RobotsPolicy;
use seleniumbase_rs::{tokio, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::new();

    // Parse robots.txt string directly (zero-copy RFC 9309 compliance)
    let robots_txt = r#"
        User-agent: *
        Disallow: /admin/
        Allow: /admin/public/
        Crawl-delay: 2.5
        Sitemap: https://example.com/sitemap.xml
    "#;
    let policy = RobotsPolicy::parse(robots_txt);

    let ua = "SeleniumBaseBot";

    // Check path permissions
    if policy.is_allowed(ua, "/admin/public/index.html") {
        println!("Path is permitted for crawling.");
    }

    if !policy.is_allowed(ua, "/admin/dashboard") {
        println!("Admin path is disallowed by robots.txt; skipping.");
    }

    // Inspect crawl delay if specified by the site
    if let Some(delay) = policy.crawl_delay(ua) {
        println!("Recommended crawl delay: {:?}", delay);
    }

    // Read sitemaps advertised in robots.txt
    for sitemap_url in policy.sitemaps() {
        println!("Advertised sitemap: {}", sitemap_url);
    }

    Ok(())
}
```

---

## Recursive Depth-Bounded Spidering (`Spider`)

When endpoint URLs are not known upfront, `Spider` drives an autonomous breadth-first exploration of links starting from seed URLs.

Features:
- **Depth Boundaries (`max_depth`)**: Limits recursive depth to prevent runaway spiders.
- **Domain Fencing (`allowed_domains`)**: Restricts crawling strictly to target domains (rejects external tracking or social media links).
- **URL Deduplication**: Internal `HashSet` tracks visited pages and normalizes URLs (stripping URL fragment hashes like `#section`).
- **Pattern Whitelists & Blacklists**: Include or exclude URL substrings (`allow_patterns`, `block_patterns`).

### End-to-End Spidering Example

```rust
use std::time::Duration;
use seleniumbase_rs_plugin_crawler::{Crawler, RobotsPolicy, Spider, SpiderConfig};
use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};
use seleniumbase_rs::{tokio, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = BrowserPool::new(
        LaunchOptions::builder().headless(true).build()?,
        PoolOptions::builder().max_browsers(2).build()?,
    );

    let crawler = Crawler::builder()
        .concurrency(4)
        .timeout(Duration::from_secs(15))
        .build()?;

    // 1. Configure the spider frontier
    let config = SpiderConfig {
        max_depth: 2,
        allowed_domains: vec!["example.com".to_string()],
        allow_patterns: vec!["/blog/".to_string()],
        block_patterns: vec!["/logout".to_string(), "/search".to_string()],
        robots_policy: None, // Or pass Some(policy)
        user_agent: "SeleniumBaseBot/1.0".to_string(),
    };

    let mut spider = Spider::new(config);
    spider.seed("https://example.com/blog/");

    // 2. Run recursive crawl over the spider's discovered link graph
    let results = crawler.crawl_spider(&pool, &mut spider, |page, url, depth| async move {
        page.goto(&url).await?;

        // Extract data and capture page HTML for discovering outgoing links
        let title = page.title().await?;
        let html = page.content().await?;

        println!("Crawled [depth {}]: {} ({})", depth, url, title);

        // Return (extracted_item, Option<html_for_link_extraction>)
        Ok((title, Some(html)))
    }).await;

    println!("Total pages visited: {}", results.len());

    pool.close().await?;
    Ok(())
}
```
