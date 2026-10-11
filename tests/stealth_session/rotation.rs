//! What a rotation does to the Chrome that is running. The MITM proxy is shared by the old Chrome
//! and the new one, so the old one must be closed before the proxy is pointed at another egress
//! or another profile; a rotation whose new Chrome cannot start must leave a working session.

use std::sync::{Arc, Mutex, PoisonError};

use seleniumbase_rs::impersonate::stealth_session::StealthSession;
use seleniumbase_rs::scrape::challenge::mitigation::Mode;
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, EventSink, ScraperEvent};

use crate::browser::ScriptedLauncher;
use crate::world::{country, World};

/// Proxies that are never connected to, with passwords that must never be shown.
const US_PROXY: &str = "http://alice:s3cret@us.proxy.test:8080";
const DE_PROXY: &str = "http://bob:hunter2@de.proxy.test:8080";

/// Notes, each time the proxy is pointed at another egress, how many Chromes had been started
/// and how many of them were still connected at that moment.
struct SwapWitness {
    launcher: Arc<ScriptedLauncher>,
    forward: CollectingEventSink,
    swaps: Mutex<Vec<(usize, usize)>>,
}

impl SwapWitness {
    fn new(world: &World) -> Arc<Self> {
        Arc::new(Self {
            launcher: Arc::clone(&world.launcher),
            forward: world.sink.clone(),
            swaps: Mutex::new(Vec::new()),
        })
    }

    /// `(started, still connected)` at each swap, oldest first.
    fn swaps(&self) -> Vec<(usize, usize)> {
        self.swaps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl EventSink for SwapWitness {
    fn emit(&self, event: &ScraperEvent) {
        if matches!(event, ScraperEvent::EgressRotated { .. }) {
            let started = self.launcher.count();
            let connected = (0..started)
                .filter(|index| self.launcher.nth(*index).browser.is_connected())
                .count();
            self.swaps
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((started, connected));
        }
        self.forward.emit(event);
    }
}

async fn in_the_browser_with_two_proxies(
    world: &World,
    witness: &Arc<SwapWitness>,
) -> StealthSession {
    let mut session = world
        .session()
        .with_event_sink(Arc::clone(witness) as Arc<dyn EventSink>)
        .with_geo_proxies([(US_PROXY, country("US")), (DE_PROXY, country("DE"))])
        .expect("two proxies")
        .build()
        .await
        .expect("a session");
    session.escalate().await.expect("escalates");
    session
}

fn rotations(world: &World) -> Vec<(Option<String>, Option<String>)> {
    world
        .sink
        .events()
        .into_iter()
        .filter_map(|event| match event {
            ScraperEvent::EgressRotated { from, to } => Some((
                from.map(|key| key.to_string()),
                to.map(|key| key.to_string()),
            )),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_old_chrome_is_closed_before_the_proxy_is_pointed_at_the_new_egress() {
    let world = World::new().await;
    let witness = SwapWitness::new(&world);
    let mut session = in_the_browser_with_two_proxies(&world, &witness).await;

    session.rotate_egress().await.expect("rotates");

    assert_eq!(
        witness.swaps(),
        [(1, 0)],
        "when the proxy was pointed at the new egress, the old Chrome had been started and \
         was still connected, so it could send its cookies through the new address"
    );
    assert_eq!(world.launcher.count(), 2);
    assert_eq!(
        world.launcher.alive_at_launch(),
        [0, 0],
        "the new Chrome started while the old one was open"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_old_chrome_is_closed_before_the_proxy_takes_the_new_profile() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.escalate().await.expect("escalates");

    session.rotate_profile().await.expect("rotates");

    assert_eq!(world.launcher.count(), 2);
    assert_eq!(
        world.launcher.alive_at_launch(),
        [0, 0],
        "the old Chrome was open while the proxy took the new profile and the new Chrome started"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_old_chrome_is_closed_before_an_imported_identity_reaches_the_proxy() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.escalate().await.expect("escalates");
    let identity = session.export_identity().await.expect("an identity");

    session.import_identity(&identity).await.expect("imports");

    assert_eq!(world.launcher.alive_at_launch(), [0, 0]);
    assert!(session.page().is_some());
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rotation_whose_new_chrome_cannot_start_brings_the_old_egress_and_a_browser_back() {
    let world = World::new().await;
    let witness = SwapWitness::new(&world);
    let mut session = in_the_browser_with_two_proxies(&world, &witness).await;
    let from = session.egress().cloned().expect("the first egress");
    let seed = session.seed();
    world.launcher.fail_next_launches(1);

    let error = session
        .rotate_egress()
        .await
        .expect_err("the new Chrome cannot start");

    assert_eq!(session.egress().cloned(), Some(from.clone()));
    assert_eq!(session.seed(), seed);
    assert_eq!(session.mode(), Mode::Browser);
    assert!(session.page().is_some(), "the session has a browser again");
    assert_eq!(
        world.launcher.count(),
        2,
        "the old Chrome and the one that came back"
    );
    let (old, back) = (world.launcher.nth(0), world.launcher.nth(1));
    assert!(!old.browser.is_connected() && back.browser.is_connected());
    assert_eq!(old.proxy_server(), back.proxy_server());
    let rotated = rotations(&world);
    assert_eq!(rotated.len(), 2, "to the new egress and back: {rotated:?}");
    assert_eq!(rotated[1].1, Some(from.to_string()), "back to the old one");
    let shown = format!("{error} | {error:?} | {session:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("hunter2"),
        "{shown}"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn when_even_the_old_chrome_cannot_come_back_the_session_goes_on_over_http() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.escalate().await.expect("escalates");
    let seed = session.seed();
    world.launcher.fail_next_launches(2);

    session
        .rotate_profile()
        .await
        .expect_err("no Chrome starts");

    assert_eq!(session.seed(), seed, "the profile is the old one");
    assert_eq!(session.mode(), Mode::Http);
    assert!(session.http().is_some() && session.page().is_none());
    let fetched = session
        .fetch(&world.site.url("/"))
        .await
        .expect("the session still works");
    assert_eq!(
        (fetched.mode, fetched.attempts),
        (Mode::Http, 3),
        "a challenge over HTTP, a solve in a new browser, and the clearance used over HTTP"
    );
    assert_eq!(
        world.launcher.count(),
        2,
        "the first Chrome and the new one"
    );
    session.close().await.expect("closes");
}
