//! Per-site pacing: at most so many fetches of one site in a window.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::sb_cdp::sync::locked;
use crate::utilities::retry::RateLimiter;

/// How many sites are paced at once. A scrape can touch an unbounded number of sites, so past this
/// the table starts over: the sites it forgets are allowed a fresh burst.
pub(super) const MAX_PACED_SITES: usize = 4096;

/// A rate limit per site, so that one busy site does not slow the others.
pub(super) struct SiteLimits {
    calls: usize,
    window: Duration,
    limiters: Mutex<HashMap<String, Arc<RateLimiter>>>,
}

impl SiteLimits {
    /// At most `calls` fetches of one site in `window`.
    pub(super) fn new(calls: usize, window: Duration) -> Self {
        Self {
            calls,
            window,
            limiters: Mutex::new(HashMap::new()),
        }
    }

    /// Waits until a fetch of `site` is allowed.
    pub(super) async fn acquire(&self, site: &str) {
        let limiter = self.limiter_for(site);
        limiter.acquire().await;
    }

    /// The limiter of `site`, made on first use. The lock is released before anyone waits.
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
    async fn the_call_past_the_limit_waits_for_the_window() {
        let limits = SiteLimits::new(2, WINDOW);
        let start = Instant::now();

        limits.acquire("shop.example").await;
        limits.acquire("shop.example").await;
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "a burst is free"
        );
        limits.acquire("shop.example").await;

        assert!(start.elapsed() >= WINDOW.saturating_sub(Duration::from_millis(50)));
    }

    #[tokio::test(start_paused = true)]
    async fn sites_are_paced_apart() {
        let limits = SiteLimits::new(1, WINDOW);
        let start = Instant::now();

        limits.acquire("one.example").await;
        limits.acquire("two.example").await;

        assert!(start.elapsed() < Duration::from_millis(50));
        assert_eq!(limits.sites(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn the_table_of_sites_is_bounded() {
        let limits = SiteLimits::new(1, WINDOW);

        for index in 0..=MAX_PACED_SITES {
            limits.acquire(&format!("site{index}.example")).await;
        }

        assert!(limits.sites() <= MAX_PACED_SITES);
        assert!(limits.sites() >= 1);
    }
}
