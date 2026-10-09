//! The browser pool against mocked browsers, with a paused clock.
//! Run with `cargo test --features test-util`.

#![cfg(feature = "test-util")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, BrowserPool, MockCtrl, PoolOptions, Session};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};
use tokio::task::JoinSet;

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// Every browser the pool has launched, with the mock behind each.
#[derive(Clone, Default)]
struct Fleet {
    launched: Arc<Mutex<Vec<(Browser, MockCtrl)>>>,
    /// How many launches fail before one succeeds.
    failures: Arc<Mutex<usize>>,
}

impl Fleet {
    fn count(&self) -> usize {
        self.launched.lock().unwrap().len()
    }

    fn mock(&self, index: usize) -> MockCtrl {
        self.launched.lock().unwrap()[index].1.clone()
    }

    fn browser(&self, index: usize) -> Browser {
        self.launched.lock().unwrap()[index].0.clone()
    }

    fn total_calls(&self, method: &str) -> usize {
        self.launched
            .lock()
            .unwrap()
            .iter()
            .map(|(_, mock)| mock.calls_to(method).len())
            .sum()
    }
}

/// A browser whose every context and tab gets a fresh id.
fn mocked_browser() -> (Browser, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let contexts = Arc::new(AtomicUsize::new(0));
    let targets = Arc::new(AtomicUsize::new(0));
    mock.on("Target.createBrowserContext", move |_| {
        Ok(json!({ "browserContextId": format!("CTX{}", contexts.fetch_add(1, Ordering::SeqCst)) }))
    });
    mock.on("Target.createTarget", move |_| {
        Ok(json!({ "targetId": format!("T{}", targets.fetch_add(1, Ordering::SeqCst) + 10) }))
    });
    (browser, mock)
}

fn pool(fleet: &Fleet, options: PoolOptions) -> BrowserPool {
    let fleet = fleet.clone();
    BrowserPool::with_launcher(options, move || {
        let fleet = fleet.clone();
        async move {
            let fail = {
                let mut remaining = fleet.failures.lock().unwrap();
                let fail = *remaining > 0;
                *remaining = remaining.saturating_sub(1);
                fail
            };
            if fail {
                return Err(SeleniumBaseError::browser_launch("chrome", "no display"));
            }
            let (browser, mock) = mocked_browser();
            fleet.launched.lock().unwrap().push((browser.clone(), mock));
            Ok(browser)
        }
    })
}

fn options(browsers: usize, contexts: usize) -> PoolOptions {
    PoolOptions::builder()
        .max_browsers(browsers)
        .contexts_per_browser(contexts)
        .acquire_timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

#[tokio::test(start_paused = true)]
async fn nothing_launches_until_the_first_lease_and_a_browser_is_reused() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(2, 2));
    assert_eq!(fleet.count(), 0);

    let first = pool.acquire().await.unwrap();
    assert_eq!(fleet.count(), 1);
    first.release().await;
    let second = pool.acquire().await.unwrap();
    second.release().await;

    assert_eq!(
        fleet.count(),
        1,
        "a released browser is reused, not relaunched"
    );
}

#[tokio::test(start_paused = true)]
async fn leases_fill_one_browser_before_launching_the_next() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(3, 2));

    let a = pool.acquire().await.unwrap();
    let b = pool.acquire().await.unwrap();
    assert_eq!(fleet.count(), 1, "two contexts fit in one browser");
    let c = pool.acquire().await.unwrap();
    assert_eq!(fleet.count(), 2);

    let stats = pool.stats().await;
    assert_eq!((stats.browsers, stats.leased, stats.launched), (2, 3, 2));
    for lease in [a, b, c] {
        lease.release().await;
    }
}

#[tokio::test(start_paused = true)]
async fn many_workers_never_exceed_the_pools_capacity() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(2, 2));
    let current = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));

    let mut workers = JoinSet::new();
    for _ in 0..12 {
        let (pool, current, peak) = (pool.clone(), Arc::clone(&current), Arc::clone(&peak));
        workers.spawn(async move {
            let lease = pool.acquire().await.unwrap();
            let now = current.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            current.fetch_sub(1, Ordering::SeqCst);
            lease.release().await;
        });
    }
    while let Some(done) = workers.join_next().await {
        done.unwrap();
    }

    assert_eq!(
        peak.load(Ordering::SeqCst),
        4,
        "2 browsers x 2 contexts, fully used and never more"
    );
    assert_eq!(fleet.count(), 2);
    assert_eq!(pool.stats().await.leased, 0);
}

#[tokio::test(start_paused = true)]
async fn every_lease_gets_its_own_context_and_it_is_disposed_on_release() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 3));

    let a = pool.acquire().await.unwrap();
    let b = pool.acquire().await.unwrap();
    let c = pool.acquire().await.unwrap();
    let ids = [a.context().id(), b.context().id(), c.context().id()];
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        3,
        "{ids:?}"
    );
    assert_eq!(fleet.total_calls("Target.createBrowserContext"), 3);
    assert_eq!(fleet.total_calls("Target.disposeBrowserContext"), 0);

    a.release().await;
    b.release().await;
    c.release().await;
    assert_eq!(fleet.total_calls("Target.disposeBrowserContext"), 3);
}

#[tokio::test(start_paused = true)]
async fn the_tab_of_a_lease_is_opened_inside_its_context() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 1));

    let lease = pool.acquire().await.unwrap();
    let extra = lease.new_page(Some("https://example.com")).await.unwrap();

    let creations = fleet.mock(0).calls_to("Target.createTarget");
    assert_eq!(creations.len(), 2);
    assert!(creations
        .iter()
        .all(|call| call.params["browserContextId"] == lease.context().id()));
    assert_ne!(lease.page().id(), extra.id());
    assert_eq!(
        creations[0].params["newWindow"], true,
        "a context's first tab opens its window"
    );
    assert_eq!(
        creations[1].params["newWindow"], false,
        "later tabs join that window"
    );
    lease.release().await;
}

#[tokio::test(start_paused = true)]
async fn a_browser_is_retired_after_serving_its_quota_and_replaced() {
    let fleet = Fleet::default();
    let quota = PoolOptions::builder()
        .max_browsers(1)
        .contexts_per_browser(1)
        .retire_after(3)
        .build()
        .unwrap();
    let pool = pool(&fleet, quota);

    for _ in 0..7 {
        pool.acquire().await.unwrap().release().await;
    }

    assert_eq!(
        fleet.count(),
        3,
        "7 leases at 3 per browser need 3 browsers"
    );
    assert!(
        !fleet.browser(0).is_connected(),
        "a spent browser is closed"
    );
    assert!(!fleet.browser(1).is_connected());
    assert_eq!(pool.stats().await.retired, 2);
}

#[tokio::test(start_paused = true)]
async fn a_browser_that_died_is_replaced_on_the_next_lease() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 1));
    pool.acquire().await.unwrap().release().await;

    fleet.mock(0).disconnect(); // the process crashes

    let lease = pool.acquire().await.unwrap();
    assert_eq!(
        fleet.count(),
        2,
        "a dead browser must not be handed out again"
    );
    assert!(lease.browser().is_connected());
    lease.release().await;
}

#[tokio::test(start_paused = true)]
async fn waiting_gives_up_after_the_timeout_and_a_freed_slot_is_usable_again() {
    let fleet = Fleet::default();
    let tight = PoolOptions::builder()
        .max_browsers(1)
        .contexts_per_browser(1)
        .acquire_timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    let pool = pool(&fleet, tight);
    let holder = pool.acquire().await.unwrap();

    let error = pool.acquire().await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::WaitTimeout { .. }),
        "{error}"
    );
    assert!(error.to_string().contains("pool"), "{error}");

    holder.release().await;
    pool.acquire().await.unwrap().release().await;
}

#[tokio::test(start_paused = true)]
async fn a_waiting_worker_is_served_when_a_lease_ends() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 1));
    let holder = pool.acquire().await.unwrap();

    let waiter = {
        let pool = pool.clone();
        tokio::spawn(async move {
            pool.acquire()
                .await
                .map(|lease| lease.context().id().to_owned())
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !waiter.is_finished(),
        "the pool is full, so the worker waits"
    );

    holder.release().await;
    assert!(waiter.await.unwrap().is_ok());
}

#[tokio::test(start_paused = true)]
async fn dropping_a_lease_frees_its_slot_in_the_background() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 1));

    {
        let _lease = pool.acquire().await.unwrap();
    } // dropped without release()

    pool.acquire()
        .await
        .expect("the dropped lease's slot comes back")
        .release()
        .await;
    assert_eq!(
        fleet.total_calls("Target.disposeBrowserContext"),
        2,
        "both contexts were disposed"
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_launch_is_reported_and_does_not_leak_a_slot() {
    let fleet = Fleet::default();
    *fleet.failures.lock().unwrap() = 1;
    let pool = pool(&fleet, options(1, 1));

    let error = pool.acquire().await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::BrowserLaunch { .. }),
        "{error}"
    );

    // With one slot in total, a leaked permit would make this time out.
    pool.acquire()
        .await
        .expect("the slot was returned")
        .release()
        .await;
}

#[tokio::test(start_paused = true)]
async fn a_closed_pool_closes_its_browsers_and_refuses_new_leases() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(2, 2));
    pool.acquire().await.unwrap().release().await;

    pool.close().await;

    assert!(!fleet.browser(0).is_connected());
    let error = pool.acquire().await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::BrowserDisconnected { .. }),
        "{error}"
    );
    assert_eq!(pool.stats().await.browsers, 0);
}

#[tokio::test(start_paused = true)]
async fn closing_wakes_workers_that_were_waiting() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 1));
    let _holder = pool.acquire().await.unwrap();
    let waiter = {
        let pool = pool.clone();
        tokio::spawn(async move { pool.acquire().await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;

    pool.close().await;

    let result = waiter.await.unwrap();
    assert!(matches!(
        result,
        Err(SeleniumBaseError::BrowserDisconnected { .. })
    ));
}

fn session_cookie_reply() -> Value {
    json!({ "cookies": [{
        "name": "sid", "value": "abc", "domain": "a.test", "path": "/",
        "expires": -1, "httpOnly": true, "secure": true,
    }]})
}

#[tokio::test(start_paused = true)]
async fn a_session_saved_by_one_worker_is_loaded_by_another_in_a_different_browser() {
    let fleet = Fleet::default();
    // One context per browser, so two simultaneous leases use two browsers.
    let pool = pool(&fleet, options(2, 1));

    let alice = pool.acquire().await.unwrap();
    let bob = pool.acquire().await.unwrap();
    assert_eq!(fleet.count(), 2);

    let signed_in = fleet.mock(0);
    signed_in.reply("Network.getAllCookies", session_cookie_reply());
    signed_in.reply("Runtime.evaluate", value(json!("null"))); // about:blank has no origin
    alice.save_session("alice").await.unwrap();
    assert_eq!(pool.sessions().names(), ["alice"]);

    assert!(bob.load_session("alice").await.unwrap());
    let sent = fleet.mock(1).calls_to("Network.setCookies");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].params["cookies"][0]["name"], "sid");
    assert_eq!(sent[0].params["cookies"][0]["value"], "abc");
    alice.release().await;
    bob.release().await;
}

#[tokio::test(start_paused = true)]
async fn loading_an_unknown_session_changes_nothing() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 1));
    let lease = pool.acquire().await.unwrap();

    assert!(!lease.load_session("nobody").await.unwrap());

    assert_eq!(fleet.total_calls("Network.setCookies"), 0);
    lease.release().await;
}

#[tokio::test(start_paused = true)]
async fn expired_cookies_are_not_loaded() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 1));
    let lease = pool.acquire().await.unwrap();
    let stale = seleniumbase_rs::sb_cdp::Cookie::new("old", "x").expires(1.0);
    let fresh = seleniumbase_rs::sb_cdp::Cookie::new("new", "y");
    pool.sessions()
        .save("mixed", Session::new(vec![stale, fresh]));

    lease.load_session("mixed").await.unwrap();

    let sent = &fleet.mock(0).calls_to("Network.setCookies")[0].params["cookies"];
    let names: Vec<_> = sent
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["new"]);
    lease.release().await;
}

#[tokio::test(start_paused = true)]
async fn the_pool_and_a_lease_never_print_cookie_values() {
    let fleet = Fleet::default();
    let pool = pool(&fleet, options(1, 1));
    pool.sessions().save(
        "login",
        Session::new(vec![seleniumbase_rs::sb_cdp::Cookie::new(
            "sid",
            "hunter2-secret",
        )]),
    );
    let lease = pool.acquire().await.unwrap();

    let shown = format!("{pool:?} {lease:?} {:?}", pool.sessions());

    assert!(!shown.contains("hunter2"), "{shown}");
    lease.release().await;
}

#[test]
fn pool_options_reject_nonsense() {
    for builder in [
        PoolOptions::builder().max_browsers(0),
        PoolOptions::builder().contexts_per_browser(0),
        PoolOptions::builder().retire_after(0),
        PoolOptions::builder().max_browsers(1000),
        PoolOptions::builder().acquire_timeout(Duration::ZERO),
    ] {
        assert!(
            matches!(builder.build(), Err(SeleniumBaseError::InvalidConfig(_))),
            "{builder:?}"
        );
    }
    let ok = PoolOptions::builder()
        .max_browsers(3)
        .contexts_per_browser(5)
        .build()
        .unwrap();
    assert_eq!(
        (ok.max_browsers(), ok.contexts_per_browser(), ok.capacity()),
        (3, 5, 15)
    );
}
