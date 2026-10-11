//! Identities, profiles and egresses: export and restore, rotating the profile and the egress in
//! both modes, and that no credential reaches a `Debug`, an event or a saved identity.

use seleniumbase_rs::impersonate::stealth_session::{StealthError, StealthSession};
use seleniumbase_rs::scrape::challenge::mitigation::Mode;
use seleniumbase_rs::scrape::error::ScrapeError;
use seleniumbase_rs::scrape::ledger::events::ScraperEvent;
use seleniumbase_rs::scrape::ledger::state::StateStore;
use seleniumbase_rs::scrape::passport::identity::{IdentityError, StealthIdentity};

use crate::site::{CLEARANCE, HOST};
use crate::world::{country, World};

/// A proxy that is never connected to, with a password that must never be shown.
const US_PROXY: &str = "http://alice:s3cret@us.proxy.test:8080";
const DE_PROXY: &str = "http://bob:hunter2@de.proxy.test:8080";

async fn with_two_proxies(world: &World) -> StealthSession {
    world
        .session()
        .with_geo_proxies([(US_PROXY, country("US")), (DE_PROXY, country("DE"))])
        .expect("two proxies")
        .build()
        .await
        .expect("a session")
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
async fn an_identity_taken_after_a_solve_holds_the_clearance_and_starts_a_session_without_a_challenge(
) {
    let world = World::new().await;
    let mut first = world.session().build().await.expect("a session");
    first.fetch(&world.site.url("/")).await.expect("a page");
    let identity = first.export_identity().await.expect("an identity");
    first.close().await.expect("closes");

    assert_eq!(identity.clearances.len(), 1, "{identity:?}");
    assert_eq!(identity.clearances[0].cookie, CLEARANCE);
    assert_eq!(identity.clearances[0].site, HOST);
    let saved = serde_json::to_string(&identity).expect("serializes");
    let loaded: StealthIdentity = serde_json::from_str(&saved).expect("reads back");
    assert_eq!(loaded, identity);

    let mut second = world
        .session()
        .restore(loaded)
        .build()
        .await
        .expect("a session");
    let launched = world.launcher.count();
    let fetched = second.fetch(&world.site.url("/")).await.expect("a page");

    assert_eq!(
        (fetched.mode, fetched.attempts, fetched.solved),
        (Mode::Http, 1, None)
    );
    assert_eq!(
        world.launcher.count(),
        launched,
        "no browser: the clearance was enough"
    );
    second.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restored_identity_of_another_egress_is_refused() {
    let world = World::new().await;
    let direct = world.session().build().await.expect("a session");
    let identity = direct.export_identity().await.expect("an identity");
    direct.close().await.expect("closes");

    let error = world
        .session()
        .with_geo_proxies([(US_PROXY, country("US"))])
        .expect("a proxy")
        .restore(identity)
        .build()
        .await
        .expect_err("the identity was made without a proxy");

    assert!(
        matches!(
            error,
            StealthError::Scrape(ScrapeError::Identity(IdentityError::EgressMismatch { .. }))
        ),
        "{error:?}"
    );
    let shown = format!("{error} | {error:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_restored_identity_picks_its_egress_from_the_pool_whatever_the_order() {
    let world = World::new().await;
    let only_de = world
        .session()
        .with_geo_proxies([(DE_PROXY, country("DE"))])
        .expect("a proxy")
        .build()
        .await
        .expect("a session");
    let identity = only_de.export_identity().await.expect("an identity");
    let key = identity
        .egress
        .clone()
        .expect("the identity names its egress");
    only_de.close().await.expect("closes");

    let restored = world
        .session()
        .with_geo_proxies([(US_PROXY, country("US")), (DE_PROXY, country("DE"))])
        .expect("two proxies")
        .restore(identity)
        .build()
        .await
        .expect("a session");

    assert_eq!(restored.egress(), Some(&key), "not the first of the pool");
    let elsewhere = world
        .session()
        .with_geo_proxies([(US_PROXY, country("US"))])
        .expect("a proxy")
        .restore(restored.export_identity().await.expect("an identity"))
        .build()
        .await;
    assert!(
        elsewhere.is_err(),
        "the egress of the identity is not in this pool"
    );
    restored.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn importing_an_identity_needs_the_egress_in_use() {
    let world = World::new().await;
    let mut session = with_two_proxies(&world).await;
    let first = session.export_identity().await.expect("an identity");
    session.rotate_egress().await.expect("rotates");

    let error = session
        .import_identity(&first)
        .await
        .expect_err("another egress");
    assert!(
        matches!(
            error,
            StealthError::Scrape(ScrapeError::Identity(IdentityError::EgressMismatch { .. }))
        ),
        "{error:?}"
    );
    let current = session.export_identity().await.expect("an identity");
    session
        .import_identity(&current)
        .await
        .expect("the egress in use");
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rotating_the_profile_over_http_draws_a_new_one_and_drops_what_the_old_one_earned() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.fetch(&world.site.url("/")).await.expect("a page");
    let before = session.identity().clone();
    let seed = session.seed();
    assert!(!before.clearances.is_empty());

    session.rotate_profile().await.expect("rotates");

    let after = session.identity();
    assert_ne!(after.fingerprint, before.fingerprint);
    assert_ne!(session.seed(), seed);
    assert!(after.cookies.is_empty() && after.clearances.is_empty());
    assert_eq!(
        after.emulation, before.emulation,
        "the same measured Chrome"
    );
    assert!(world
        .sink
        .events()
        .iter()
        .any(|event| matches!(event, ScraperEvent::ProfileRotated { from_seed, to_seed } if *from_seed != *to_seed)));
    let state = world.store.get(HOST).await.expect("a store");
    assert!(state.is_some(), "what is known of the site stays");
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_profile_earns_its_own_clearance_through_the_same_proxy() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.fetch(&world.site.url("/")).await.expect("a page");
    session.rotate_profile().await.expect("rotates");

    let fetched = session.fetch(&world.site.url("/")).await.expect("a page");

    assert_eq!(
        (fetched.mode, fetched.attempts),
        (Mode::Http, 3),
        "no cookie, so a challenge"
    );
    assert_eq!(world.launcher.count(), 2);
    let (first, second) = (world.launcher.nth(0), world.launcher.nth(1));
    assert_eq!(
        first.proxy_server(),
        second.proxy_server(),
        "the proxy and its port outlive the browser"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rotating_the_profile_in_the_browser_starts_a_new_chrome_behind_the_same_proxy() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.escalate().await.expect("escalates");
    let before = session.identity().fingerprint.clone();

    session.rotate_profile().await.expect("rotates");

    assert_eq!(session.mode(), Mode::Browser);
    assert_ne!(session.identity().fingerprint, before);
    assert_eq!(world.launcher.count(), 2);
    let (old, new) = (world.launcher.nth(0), world.launcher.nth(1));
    assert_eq!(old.proxy_server(), new.proxy_server());
    assert!(!old.browser.is_connected() && new.browser.is_connected());
    assert!(session.identity().cookies.is_empty());
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rotating_the_egress_over_http_moves_the_identity_and_the_language() {
    let world = World::new().await;
    let mut session = with_two_proxies(&world).await;
    let from = session.egress().cloned().expect("the first egress");
    let language = session.identity().fingerprint.locale.clone();

    session.rotate_egress().await.expect("rotates");

    let to = session.egress().cloned().expect("the next egress");
    assert_ne!(from, to);
    assert_eq!(session.identity().egress, Some(to.clone()));
    assert_ne!(
        session.identity().fingerprint.locale,
        language,
        "the country's language"
    );
    assert!(session.identity().cookies.is_empty());
    assert_eq!(
        rotations(&world),
        [(Some(from.to_string()), Some(to.to_string()))],
        "reported once, with keys and no credentials"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rotating_the_egress_in_the_browser_points_the_proxy_at_it_and_reports_once() {
    let world = World::new().await;
    let mut session = with_two_proxies(&world).await;
    session.escalate().await.expect("escalates");
    let from = session.egress().cloned().expect("the first egress");

    session.rotate_egress().await.expect("rotates");

    let to = session.egress().cloned().expect("the next egress");
    assert_eq!(session.mode(), Mode::Browser);
    assert_eq!(
        world.launcher.count(),
        2,
        "a new Chrome in the language of the new country"
    );
    assert_eq!(
        rotations(&world),
        [(Some(from.to_string()), Some(to.to_string()))],
        "the proxy reported it, and the session did not report it again"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_another_egress_there_is_nothing_to_rotate_to() {
    let world = World::new().await;
    let mut direct = world.session().build().await.expect("a session");
    let mut single = world
        .session()
        .with_geo_proxies([(US_PROXY, country("US"))])
        .expect("a proxy")
        .build()
        .await
        .expect("a session");
    let before = single.egress().cloned();

    for session in [&mut direct, &mut single] {
        let error = session.rotate_egress().await.expect_err("nowhere to go");
        assert!(
            matches!(error, StealthError::Scrape(ScrapeError::NoEgressAvailable)),
            "{error:?}"
        );
    }

    assert_eq!(single.egress().cloned(), before, "the session is as it was");
    assert!(rotations(&world).is_empty());
    direct.close().await.expect("closes");
    single.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_credential_reaches_a_debug_an_event_or_a_saved_identity() {
    let world = World::new().await;
    let mut session = with_two_proxies(&world).await;
    session.escalate().await.expect("escalates");
    session.rotate_egress().await.expect("rotates");
    let identity = session.export_identity().await.expect("an identity");

    let events = serde_json::to_string(&world.sink.events()).expect("events serialize");
    let saved = serde_json::to_string(&identity).expect("an identity serializes");
    let shown = format!(
        "{session:?} | {identity:?} | {:?} | {:?}",
        world.launcher.last().options,
        world.launcher.nth(0).options
    );
    for text in [&events, &saved, &shown] {
        for secret in ["alice", "s3cret", "bob", "hunter2"] {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
    }
    session.close().await.expect("closes");
}
