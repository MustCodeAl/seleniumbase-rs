//! High-throughput concurrent browser crawler and scraping pipeline.
//!
//! Provides bounded concurrency, rate limiting, automatic retry, and per-page context isolation
//! built directly on top of [`BrowserPool`](crate::sb_cdp::BrowserPool).

pub mod builder;
pub mod engine;
pub mod item;

#[doc(inline)]
pub use builder::CrawlerBuilder;
#[doc(inline)]
pub use engine::Crawler;
#[doc(inline)]
pub use item::CrawlItem;
