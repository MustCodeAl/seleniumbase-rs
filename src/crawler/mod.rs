//! High-throughput concurrent browser crawler and scraping pipeline.
//!
//! Provides bounded concurrency, rate limiting, automatic retry, and per-page context isolation
//! built directly on top of [`BrowserPool`](crate::sb_cdp::BrowserPool).
//!
//! Includes zero-copy [`RobotsPolicy`] evaluation, [`sitemap`] XML URL extraction,
//! and recursive [`Spider`] link discovery.

pub mod builder;
pub mod engine;
pub mod item;
pub mod robots;
pub mod sitemap;
pub mod spider;

#[doc(inline)]
pub use builder::CrawlerBuilder;
#[doc(inline)]
pub use engine::Crawler;
#[doc(inline)]
pub use item::CrawlItem;
#[doc(inline)]
pub use robots::{fetch_robots_txt, RobotsPolicy};
#[doc(inline)]
pub use sitemap::{fetch_sitemap_urls, parse_sitemap_urls};
#[doc(inline)]
pub use spider::{Spider, SpiderConfig, SpiderConfigBuilder};
