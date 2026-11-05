//! Shutting a Pure CDP browser pool down when the process is told to stop,
//! against mocked browsers with a paused clock.
//! Run with `cargo test --features test-util`.

#![cfg(feature = "test-util")]

use std::future::pending;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use seleniumbase_rs::common::shutdown::{cleanup_within, Outcome, Shutdown, Signal};
use seleniumbase_rs::sb_cdp::{Browser, BrowserPool, PoolOptions};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::json;

/// Every browser the pool has launched.
type Fleet = Arc<Mutex<Vec<Browser>>>;

/// A pool of up to `browsers` mocked browsers, one tab-holding context each.
fn pool(fleet: &Fleet, browsers: usize) -> BrowserPool {
    let options = PoolOptions::builder()
        .max_browsers(browsers)
        .contexts_per_browser(1)
        .acquire_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let fleet = Arc::clone(fleet);
    BrowserPool::with_launcher(options, move || {
        let fleet = Arc::clone(&fleet);
        async move {
            let (browser, mock) = Browser::new_mocked();
            let contexts = Arc::new(AtomicUsize::new(0));
            let targets = Arc::new(AtomicUsize::new(0));
            mock.on("Target.createBrowserContext", move |_| {
                let id = contexts.fetch_add(1, Ordering::SeqCst);
                Ok(json!({ "browserContextId": format!("CTX{id}") }))
            });
            mock.on("Target.createTarget", move |_| {
                let id = targets.fetch_add(1, Ordering::SeqCst) + 10;
                Ok(json!({ "targetId": format!("T{id}") }))
            });
            fleet.lock().unwrap().push(browser.clone());
            Ok::<_, SeleniumBaseError>(browser)
        }
    })
}

#[tokio::test(start_paused = true)]
async fn a_signal_closes_every_pooled_browser_and_wakes_waiting_workers() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, 2);
    let (mut shutdown, trigger) = Shutdown::manual();

    // Two workers hold both browsers; a third waits for one to come free.
    let first = pool.acquire().await.unwrap();
    let second = pool.acquire().await.unwrap();
    let waiter = {
        let pool = pool.clone();
        tokio::spawn(async move { pool.acquire().await.map(drop) })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(fleet.lock().unwrap().iter().all(Browser::is_connected));

    // The process is told to stop while the workers are busy.
    trigger.fire(Signal::Terminate);
    let outcome = shutdown.race(pending::<()>()).await;
    assert_eq!(outcome, Outcome::Interrupted(Signal::Terminate));
    let closed = cleanup_within(Duration::from_secs(30), pool.close()).await;

    assert!(closed, "the pool did not close within the grace period");
    assert_eq!(fleet.lock().unwrap().len(), 2);
    assert!(
        fleet
            .lock()
            .unwrap()
            .iter()
            .all(|browser| !browser.is_connected()),
        "a browser was left running"
    );
    let waited = waiter.await.unwrap();
    assert!(
        matches!(waited, Err(SeleniumBaseError::BrowserDisconnected { .. })),
        "{waited:?}"
    );
    let after = pool.acquire().await.map(drop);
    assert!(matches!(
        after,
        Err(SeleniumBaseError::BrowserDisconnected { .. })
    ));
    drop((first, second));
}
