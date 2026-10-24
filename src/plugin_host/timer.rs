//! Time for the plugins a browser hosts.
//!
//! The plugin API crate has no async runtime. It runs the time limits of the
//! hooks (`Budgets`) only for a set that was given a timer, so the host gives
//! every set it launches a browser with one that sleeps on tokio.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs_plugin::{PluginSet, Timer};

/// A [`Timer`] over tokio's clock.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TokioTimer;

impl Timer for TokioTimer {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(tokio::time::sleep(duration))
    }
}

/// `plugins` with a timer, so its hooks get the time limits they declare.
///
/// A set that already has a timer (a program or a test gave it one) keeps it.
#[must_use]
pub(crate) fn timed(plugins: PluginSet) -> PluginSet {
    if plugins.clock().is_some() {
        plugins
    } else {
        plugins.with_timer(Arc::new(TokioTimer))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use seleniumbase_rs_plugin::{async_trait, Plugin, PluginSet, Result};

    use super::*;

    struct Idle;

    #[async_trait]
    impl Plugin for Idle {
        fn name(&self) -> &str {
            "idle"
        }

        async fn on_close(&self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_set_gets_a_timer_once() {
        let plain = PluginSet::new().with(Idle).unwrap();
        assert!(plain.clock().is_none());

        let timed_set = timed(plain);

        assert!(timed_set.clock().is_some());
        assert!(timed(timed_set).clock().is_some());
    }

    #[tokio::test]
    async fn the_timer_sleeps_on_tokio() {
        let started = tokio::time::Instant::now();
        TokioTimer.sleep(Duration::from_millis(20)).await;
        assert!(started.elapsed() >= Duration::from_millis(20));
    }
}
