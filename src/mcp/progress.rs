//! Progress notifications for tool calls that take a while.
//!
//! A client that wants progress puts a `progressToken` in the `_meta` of its
//! `tools/call`. The host then hands the tool a [`Progress`] it can
//! [`report`](Progress::report) to, and each report becomes a
//! `notifications/progress` message. Without a token a [`Progress`] does
//! nothing, so a handler reports unconditionally.

use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rmcp::model::{ProgressNotificationParam, ProgressToken};
use rmcp::service::{Peer, RoleServer};

/// One report: how far a call has got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressUpdate {
    /// Work done so far, in whatever unit the tool counts. It grows with
    /// every report that is sent.
    pub progress: u32,
    /// Work to do in all, when the tool knows.
    pub total: Option<u32>,
    /// What the call is doing now; empty for none.
    pub message: String,
}

type SendFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

/// Where the reports of one call go.
///
/// The host builds one from the client's progress token. Tests build one that
/// collects the updates ([`ProgressSink::new`]).
#[derive(Clone)]
pub struct ProgressSink {
    send: Arc<dyn Fn(ProgressUpdate) -> SendFuture + Send + Sync>,
}

impl std::fmt::Debug for ProgressSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressSink").finish_non_exhaustive()
    }
}

impl ProgressSink {
    /// A sink that passes every update to `send`.
    ///
    /// A failure `send` returns is logged and never reaches the tool.
    pub fn new<F, Fut, E>(send: F) -> Self
    where
        F: Fn(ProgressUpdate) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
        E: Display,
    {
        Self {
            send: Arc::new(move |update| {
                let sending = send(update);
                Box::pin(async move { sending.await.map_err(|error| error.to_string()) })
            }),
        }
    }

    /// A sink that notifies `peer`, the client, under the client's `token`.
    pub(crate) fn to_peer(peer: Peer<RoleServer>, token: ProgressToken) -> Self {
        Self::new(move |update| {
            let peer = peer.clone();
            let mut params =
                ProgressNotificationParam::new(token.clone(), f64::from(update.progress));
            if let Some(total) = update.total {
                params = params.with_total(f64::from(total));
            }
            if !update.message.is_empty() {
                params = params.with_message(update.message);
            }
            async move { peer.notify_progress(params).await }
        })
    }
}

struct Active {
    sink: ProgressSink,
    /// One more than the largest progress value sent; 0 before the first.
    sent: AtomicU64,
}

/// What a tool reports its progress to.
///
/// A cheap handle: clones share the one call's state. The default one has no
/// client behind it and ignores every report.
#[derive(Clone, Default)]
pub struct Progress {
    active: Option<Arc<Active>>,
}

impl std::fmt::Debug for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Progress")
            .field("listening", &self.active.is_some())
            .finish()
    }
}

impl Progress {
    /// A handle that reports to `sink`.
    #[must_use]
    pub fn new(sink: ProgressSink) -> Self {
        Self {
            active: Some(Arc::new(Active {
                sink,
                sent: AtomicU64::new(0),
            })),
        }
    }

    /// Whether a client asked for progress on this call.
    #[must_use]
    pub fn is_listening(&self) -> bool {
        self.active.is_some()
    }

    /// Tells the client how far the call has got.
    ///
    /// `current` is the work done so far and `total` the work to do in all,
    /// when known. The protocol wants the numbers to grow, so a report whose
    /// `current` does not exceed one already sent is dropped. Without a
    /// progress token this does nothing, and a notification that cannot be
    /// sent is logged: reporting never fails the call.
    pub async fn report(&self, current: u32, total: Option<u32>, message: impl Into<String>) {
        let Some(active) = &self.active else { return };
        let next = u64::from(current) + 1;
        if active.sent.fetch_max(next, Ordering::SeqCst) >= next {
            tracing::debug!(
                current,
                "a progress report that does not advance was dropped"
            );
            return;
        }
        let update = ProgressUpdate {
            progress: current,
            total,
            message: message.into(),
        };
        if let Err(error) = (active.sink.send)(update).await {
            tracing::warn!(%error, "a progress notification could not be sent");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn collecting() -> (Progress, Arc<Mutex<Vec<ProgressUpdate>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let sink = ProgressSink::new(move |update| {
            log.lock().unwrap().push(update);
            std::future::ready(Ok::<(), String>(()))
        });
        (Progress::new(sink), seen)
    }

    #[tokio::test]
    async fn reports_reach_the_sink_in_order() {
        let (progress, seen) = collecting();

        progress.report(0, Some(2), "launching").await;
        progress.report(1, Some(2), "ready").await;

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(
            seen[0],
            ProgressUpdate {
                progress: 0,
                total: Some(2),
                message: "launching".to_owned()
            }
        );
        assert_eq!(seen[1].progress, 1);
    }

    #[tokio::test]
    async fn a_report_that_does_not_advance_is_dropped() {
        let (progress, seen) = collecting();

        progress.report(3, None, "a").await;
        progress.report(3, None, "same").await;
        progress.report(2, None, "back").await;
        progress.report(4, None, "b").await;

        let messages: Vec<_> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|update| update.message.clone())
            .collect();
        assert_eq!(messages, ["a", "b"]);
    }

    #[tokio::test]
    async fn clones_share_one_count() {
        let (progress, seen) = collecting();
        let other = progress.clone();

        progress.report(1, None, "x").await;
        other.report(1, None, "y").await;

        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn without_a_client_reporting_does_nothing() {
        let progress = Progress::default();

        progress.report(1, Some(1), "ignored").await;

        assert!(!progress.is_listening());
    }

    #[tokio::test]
    async fn a_sink_that_fails_does_not_fail_the_report() {
        let sink = ProgressSink::new(|_| std::future::ready(Err::<(), _>("the pipe is closed")));
        let progress = Progress::new(sink);

        progress.report(1, None, "lost").await;

        assert!(progress.is_listening());
    }
}
