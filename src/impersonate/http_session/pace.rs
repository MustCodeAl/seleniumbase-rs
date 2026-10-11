//! Per-site pacing: at most so many requests to one site in a window.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::options::MAX_PACED_SITES;
use crate::sb_cdp::sync::locked;
use crate::utilities::retry::RateLimiter;

/// A rate limit per site, so that one busy site does not slow the others.
pub(super) struct SitePacer {
    calls: usize,
    window: Duration,
    limiters: Mutex<HashMap<String, Arc<RateLimiter>>>,
}

impl fmt::Debug for SitePacer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SitePacer")
            .field("calls", &self.calls)
            .field("window", &self.window)
            .field("sites", &locked(&self.limiters).len())
            .finish()
    }
}

impl SitePacer {
    /// At most `calls` requests to one site in `window`.
    pub(super) fn new(calls: usize, window: Duration) -> Self {
        Self {
            calls,
            window,
            limiters: Mutex::new(HashMap::new()),
        }
    }

    /// Waits until a request to `site` is allowed.
    pub(super) async fn acquire(&self, site: &str) {
        let limiter = self.limiter_for(site);
        limiter.acquire().await;
    }

    /// The limiter of `site`, made on first use. The lock is released before anyone waits. A
    /// scrape can touch an unbounded number of sites, so past [`MAX_PACED_SITES`] the table
    /// starts over: the sites it forgets are allowed a fresh burst.
    fn limiter_for(&self, site: &str) -> Arc<RateLimiter> {
        let mut limiters = locked(&self.limiters);
        if !limiters.contains_key(site) && limiters.len() >= MAX_PACED_SITES {
            limiters.clear();
        }
        Arc::clone(
            limiters
                .entry(site.to_owned())
                .or_insert_with(|| Arc::new(RateLimiter::new(self.calls, self.window))),
        )
    }

    /// How many sites are paced now.
    #[cfg(test)]
    pub(super) fn sites(&self) -> usize {
        locked(&self.limiters).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    const WINDOW: Duration = Duration::from_millis(400);

    #[tokio::test(start_paused = true)]
    async fn the_request_past_the_limit_waits_for_the_window() {
        let pacer = SitePacer::new(2, WINDOW);
        let start = Instant::now();
        pacer.acquire("shop.example").await;
        pacer.acquire("shop.example").await;
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "a burst is free"
        );
        pacer.acquire("shop.example").await;
        assert!(start.elapsed() >= WINDOW.saturating_sub(Duration::from_millis(50)));
    }

    #[tokio::test(start_paused = true)]
    async fn sites_are_paced_apart() {
        let pacer = SitePacer::new(1, WINDOW);
        let start = Instant::now();
        pacer.acquire("one.example").await;
        pacer.acquire("two.example").await;
        assert!(start.elapsed() < Duration::from_millis(50));
        assert_eq!(pacer.sites(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn the_table_of_sites_is_bounded() {
        let pacer = SitePacer::new(1, WINDOW);
        for index in 0..=MAX_PACED_SITES {
            pacer.acquire(&format!("site{index}.example")).await;
        }
        assert!(pacer.sites() <= MAX_PACED_SITES);
        assert!(pacer.sites() >= 1);
    }

    #[test]
    fn the_debug_view_counts_sites_only() {
        let pacer = SitePacer::new(3, WINDOW);
        let shown = format!("{pacer:?}");
        assert!(
            shown.contains("sites: 0") && shown.contains("calls: 3"),
            "{shown}"
        );
    }
}
