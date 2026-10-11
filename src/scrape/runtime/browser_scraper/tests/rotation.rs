//! Tests of the rotations, of identities and sessions, and of what the scraper must never do.

use serde_json::json;

use super::rig::{country, real_page, widget_challenge, Rig, NOW, SEED};
use crate::scrape::challenge::detect::Vendor;
use crate::scrape::egress::geo::GeoError;
use crate::scrape::error::ScrapeError;
use crate::scrape::ledger::events::ScraperEvent;
use crate::scrape::passport::identity::{IdentityError, NO_EXPIRY};
use crate::scrape::runtime::browser_scraper::launcher::{claimed_major, host_os};
use crate::stealth::humanize::Rng;
use crate::wire::BrowserKind;

const URL: &str = "https://shop.example/";

/// The domains whose `enable` command a page can feel, and which nothing here may send.
const FORBIDDEN_DOMAINS: [&str; 5] = ["Runtime", "DOM", "Log", "Debugger", "Profiler"];

fn two_proxies(rig: &Rig) -> crate::scrape::runtime::browser_scraper::BrowserScraperBuilder {
    rig.builder()
        .with_geo_proxies([
            ("http://ualpha:s3cret@de.proxy.example:8080", country("DE")),
            ("http://ubeta:hunter2@fr.proxy.example:8080", country("FR")),
        ])
        .expect("valid specs")
}

#[tokio::test(start_paused = true)]
async fn rotating_the_profile_draws_another_one_for_the_same_machine_and_starts_clean() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    scraper.fetch(URL).await.expect("the page is fetched");
    let before = scraper.fingerprint().clone();

    scraper.rotate_profile().await.expect("the profile rotates");

    assert_eq!(scraper.seed(), Rng::new(SEED).next_u64());
    let after = scraper.fingerprint();
    assert_ne!(after, &before);
    assert_eq!(after.os_type, before.os_type);
    assert_eq!(
        claimed_major(after),
        Some(155),
        "it claims the Chrome that runs"
    );
    assert!(
        !rig.launcher.nth(1).browser.is_connected(),
        "the old browser is closed"
    );
    let new = rig.launcher.nth(2);
    assert!(new.browser.is_connected());
    assert_eq!(new.options.user_agent, after.user_agent);
    assert!(
        new.mock.calls_to("Network.setCookies").is_empty(),
        "nothing carries over"
    );
    assert_eq!(rig.event_names().last(), Some(&"profile_rotated"));
    let known = crate::scrape::ledger::state::StateStore::get(&rig.store, "shop.example")
        .await
        .expect("the store reads")
        .expect("the site is still known");
    assert_eq!(known.successes, 1);
}

#[tokio::test(start_paused = true)]
async fn a_profile_that_cannot_start_leaves_the_old_browser_in_place() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    let page = scraper.page().id().to_owned();
    let seed = scraper.seed();
    rig.launcher
        .on_launch(|_, mock| mock.fail("Target.getTargets", "no tabs"));

    let error = scraper
        .rotate_profile()
        .await
        .expect_err("the new browser has no tab");

    assert!(matches!(error, ScrapeError::Browser(_)), "{error:?}");
    assert_eq!(scraper.seed(), seed);
    assert_eq!(scraper.page().id(), page);
    assert!(
        rig.launcher.nth(1).browser.is_connected(),
        "the old browser still serves"
    );
    assert!(
        !rig.launcher.nth(2).browser.is_connected(),
        "the new one was closed"
    );
}

#[tokio::test(start_paused = true)]
async fn rotating_to_another_country_restarts_the_browser_in_the_language_and_zone_of_that_country()
{
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = two_proxies(&rig).build().await.expect("the scraper builds");
    let before = scraper.fingerprint().clone();

    scraper.rotate_egress().await.expect("the egress rotates");

    assert_eq!(
        scraper.egress().map(|key| key.host.as_str()),
        Some("fr.proxy.example")
    );
    assert_eq!(
        rig.launcher.count(),
        3,
        "a probe, the first browser, the browser for the new country"
    );
    assert!(
        !rig.launcher.nth(1).browser.is_connected(),
        "the old browser is closed"
    );
    let new = rig.launcher.nth(2);
    assert!(new.browser.is_connected());
    assert_eq!(new.options.lang.as_deref(), Some("fr-FR"));
    let created = new.mock.calls_to("Target.createBrowserContext");
    assert_eq!(created.len(), 1);
    assert_eq!(
        created[0].params["proxyServer"],
        "http://fr.proxy.example:8080"
    );
    let after = scraper.fingerprint();
    assert_eq!(after.locale.as_deref(), Some("fr-FR"));
    assert_eq!(after.timezone.as_deref(), Some("Europe/Paris"));
    assert_eq!(scraper.seed(), SEED, "it is the same profile, moved");
    assert_eq!(after.os_type, before.os_type);
    assert_eq!(after.hardware_concurrency, before.hardware_concurrency);
    assert_eq!(after.user_agent, before.user_agent);
    let event = rig.sink.events().pop().expect("an event");
    assert!(matches!(
        &event,
        ScraperEvent::EgressRotated { from: Some(from), to: Some(to) }
            if from.host == "de.proxy.example" && to.host == "fr.proxy.example"
    ));
}

#[tokio::test(start_paused = true)]
async fn rotating_within_a_country_only_opens_a_context_behind_the_next_proxy() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig
        .builder()
        .with_geo_proxies([
            ("http://one.proxy.example:8080", country("DE")),
            ("http://two.proxy.example:8080", country("DE")),
        ])
        .expect("valid specs")
        .build()
        .await
        .expect("the scraper builds");
    let kept = rig.launcher.last();
    let before = scraper.fingerprint().clone();

    scraper.rotate_egress().await.expect("the egress rotates");

    assert_eq!(rig.launcher.count(), 2, "the browser is not started again");
    assert_eq!(
        scraper.egress().map(|key| key.host.as_str()),
        Some("two.proxy.example")
    );
    let created = kept.mock.calls_to("Target.createBrowserContext");
    assert_eq!(created.len(), 2);
    assert_eq!(
        created[1].params["proxyServer"],
        "http://two.proxy.example:8080"
    );
    let gone = kept.mock.calls_to("Target.disposeBrowserContext");
    assert_eq!(
        gone.len(),
        1,
        "the context of the first egress goes with its cookies"
    );
    assert_eq!(scraper.fingerprint(), &before);
    assert!(kept.browser.is_connected());
}

#[tokio::test(start_paused = true)]
async fn a_browser_that_cannot_start_for_the_new_country_leaves_the_scraper_as_it_was() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = two_proxies(&rig).build().await.expect("the scraper builds");
    let page = scraper.page().id().to_owned();
    let before = scraper.fingerprint().clone();
    rig.launcher
        .on_launch(|_, mock| mock.fail("Target.createBrowserContext", "no contexts"));

    let error = scraper
        .rotate_egress()
        .await
        .expect_err("the new browser has no context");

    assert!(matches!(error, ScrapeError::Browser(_)), "{error:?}");
    assert_eq!(
        scraper.egress().map(|key| key.host.as_str()),
        Some("de.proxy.example")
    );
    assert_eq!(scraper.page().id(), page);
    assert_eq!(scraper.fingerprint(), &before);
    assert!(rig.launcher.nth(1).browser.is_connected());
    assert!(!rig.launcher.nth(2).browser.is_connected());
}

#[tokio::test(start_paused = true)]
async fn rotating_with_no_other_egress_is_an_error_and_changes_nothing() {
    let rig = Rig::new(vec![real_page()]);
    let mut without_pool = rig.builder().build().await.expect("the scraper builds");
    let one = Rig::new(vec![real_page()]);
    let mut single = one
        .builder()
        .with_geo_proxies([("http://de.proxy.example:8080", country("DE"))])
        .expect("a valid spec")
        .build()
        .await
        .expect("the scraper builds");
    let page = single.page().id().to_owned();

    let none = without_pool.rotate_egress().await.expect_err("no pool");
    let alone = single.rotate_egress().await.expect_err("one egress");

    assert!(matches!(none, ScrapeError::NoEgressAvailable));
    assert!(matches!(alone, ScrapeError::NoEgressAvailable));
    assert_eq!(single.page().id(), page);
    assert_eq!(
        single.egress().map(|key| key.host.as_str()),
        Some("de.proxy.example")
    );
    assert_eq!(
        one.launcher
            .last()
            .mock
            .calls_to("Target.createBrowserContext")
            .len(),
        1
    );
}

#[tokio::test(start_paused = true)]
async fn an_egress_whose_country_has_no_locale_is_refused_before_the_switch() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig
        .builder()
        .with_geo_proxies([
            ("http://de.proxy.example:8080", country("DE")),
            ("http://zz.proxy.example:8080", country("ZZ")),
        ])
        .expect("valid specs")
        .build()
        .await
        .expect("the scraper builds");

    let error = scraper.rotate_egress().await.expect_err("ZZ has no locale");

    assert!(
        matches!(error, ScrapeError::Geo(GeoError::NoLocale(_))),
        "{error:?}"
    );
    assert_eq!(
        scraper.egress().map(|key| key.host.as_str()),
        Some("de.proxy.example")
    );
    assert_eq!(
        rig.launcher
            .last()
            .mock
            .calls_to("Target.createBrowserContext")
            .len(),
        1
    );
}

/// A scraper that passed a widget challenge, so that it holds a clearance cookie.
async fn scraper_with_a_clearance(
    rig: &Rig,
) -> crate::scrape::runtime::browser_scraper::BrowserScraper {
    rig.site.show_widget(true);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    scraper.fetch(URL).await.expect("the challenge is passed");
    scraper
}

#[tokio::test(start_paused = true)]
async fn an_identity_carries_the_fingerprint_the_cookies_and_the_clearance() {
    let rig = Rig::new(vec![widget_challenge(), real_page()]);
    let scraper = scraper_with_a_clearance(&rig).await;

    let identity = scraper
        .export_identity()
        .await
        .expect("the identity exports");

    assert_eq!(&identity.fingerprint, scraper.fingerprint());
    assert_eq!(identity.cookies.len(), 1);
    assert_eq!(identity.emulation.kind, BrowserKind::Chrome);
    assert_eq!(identity.emulation.major, 155);
    assert_eq!(identity.emulation.os, host_os());
    assert_eq!(identity.egress, None);
    assert_eq!(identity.created_at, NOW);
    let clearance = identity.clearance_for("shop.example").expect("a clearance");
    assert_eq!(clearance.vendor, Vendor::Cloudflare);
    assert_eq!(clearance.cookie, "cf_clearance");
    assert_eq!(clearance.expires_at, NO_EXPIRY);
    assert!(matches!(
        rig.sink.events().pop(),
        Some(ScraperEvent::IdentityExported { cookies: 1 })
    ));
    assert!(!format!("{identity:?}").contains("fresh-token"));
}

#[tokio::test(start_paused = true)]
async fn an_identity_is_restored_in_a_scraper_with_another_profile() {
    let source = Rig::new(vec![widget_challenge(), real_page()]);
    let identity = scraper_with_a_clearance(&source)
        .await
        .export_identity()
        .await
        .expect("the identity exports");
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().seed(SEED + 5).build().await.expect("builds");
    assert_ne!(scraper.fingerprint(), &identity.fingerprint);

    scraper
        .import_identity(&identity)
        .await
        .expect("the identity imports");

    assert_eq!(scraper.fingerprint(), &identity.fingerprint);
    assert_eq!(
        rig.launcher.count(),
        3,
        "a probe, the first browser, one started with the identity"
    );
    assert!(!rig.launcher.nth(1).browser.is_connected());
    let set = rig.launcher.last().mock.calls_to("Network.setCookies");
    assert_eq!(set.len(), 1);
    assert_eq!(set[0].params["cookies"][0]["name"], "cf_clearance");
    assert!(matches!(
        rig.sink.events().pop(),
        Some(ScraperEvent::IdentityRestored { cookies: 1 })
    ));
}

#[tokio::test(start_paused = true)]
async fn an_identity_of_the_profile_in_use_needs_no_new_browser() {
    let rig = Rig::new(vec![widget_challenge(), real_page()]);
    let mut scraper = scraper_with_a_clearance(&rig).await;
    let identity = scraper
        .export_identity()
        .await
        .expect("the identity exports");
    let launched = rig.launcher.count();

    scraper
        .import_identity(&identity)
        .await
        .expect("the identity imports");

    assert_eq!(rig.launcher.count(), launched);
}

#[tokio::test(start_paused = true)]
async fn an_identity_of_another_egress_is_refused_and_nothing_is_set() {
    let source = Rig::new(vec![real_page()]);
    let behind_proxy = two_proxies(&source).build().await.expect("builds");
    let identity = behind_proxy
        .export_identity()
        .await
        .expect("the identity exports");
    assert!(identity.egress.is_some());
    let rig = Rig::new(vec![real_page()]);
    let mut direct = rig.builder().build().await.expect("builds");

    let error = direct
        .import_identity(&identity)
        .await
        .expect_err("another egress");

    assert!(
        matches!(
            &error,
            ScrapeError::Identity(IdentityError::EgressMismatch { .. })
        ),
        "{error:?}"
    );
    assert_eq!(rig.launcher.count(), 2, "no browser was started for it");
    assert_eq!(
        rig.launcher
            .last()
            .mock
            .calls_to("Network.setCookies")
            .len(),
        0,
        "no cookie was set"
    );
}

#[tokio::test(start_paused = true)]
async fn an_identity_for_another_chrome_or_layout_is_refused() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().build().await.expect("builds");
    let identity = scraper
        .export_identity()
        .await
        .expect("the identity exports");

    let mut old_chrome = identity.clone();
    old_chrome.emulation.major = 150;
    let mut future_layout = identity;
    future_layout.schema = 9;

    let chrome = scraper
        .import_identity(&old_chrome)
        .await
        .expect_err("another Chrome");
    let layout = scraper
        .import_identity(&future_layout)
        .await
        .expect_err("another layout");

    assert!(
        matches!(
            chrome,
            ScrapeError::Identity(IdentityError::EmulationMismatch {
                identity: 150,
                measured: 155
            })
        ),
        "{chrome:?}"
    );
    assert!(
        matches!(layout, ScrapeError::Identity(IdentityError::Schema(9))),
        "{layout:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn the_version_of_chrome_is_not_checked_when_the_running_one_is_unknown() {
    let source = Rig::new(vec![real_page()]);
    let mut identity = source
        .builder()
        .build()
        .await
        .expect("builds")
        .export_identity()
        .await
        .expect("the identity exports");
    identity.emulation.major = 150;
    let rig = Rig::with_product(vec![real_page()], "");
    let mut scraper = rig.builder().seed(SEED + 3).build().await.expect("builds");

    scraper
        .import_identity(&identity)
        .await
        .expect("nothing to compare the version with");

    assert_eq!(scraper.fingerprint(), &identity.fingerprint);
}

#[tokio::test(start_paused = true)]
async fn a_session_is_saved_and_loaded_with_its_local_storage() {
    let rig = Rig::new(vec![real_page()]);
    rig.site.set_local_storage(json!({ "theme": "dark" }));
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    scraper.fetch(URL).await.expect("the page is fetched");

    scraper
        .save_session("login")
        .await
        .expect("the session saves");
    let loaded = scraper
        .load_session("login")
        .await
        .expect("the session loads");
    let missing = scraper
        .load_session("nobody")
        .await
        .expect("nothing to load");

    let saved = scraper.sessions().load("login").expect("it was saved");
    assert_eq!(saved.origin(), Some("https://shop.example"));
    assert_eq!(
        saved.local_storage().get("theme").map(String::as_str),
        Some("dark")
    );
    assert!(loaded && !missing);
    let wrote = rig
        .launcher
        .last()
        .mock
        .calls_to("Runtime.evaluate")
        .iter()
        .any(|call| {
            call.params["expression"]
                .as_str()
                .is_some_and(|expression| {
                    expression.contains("setItem") && expression.contains("dark")
                })
        });
    assert!(wrote, "the storage of the origin is put back");
}

#[tokio::test(start_paused = true)]
async fn no_flow_switches_on_a_domain_a_page_can_feel() {
    let rig = Rig::new(vec![widget_challenge(), real_page()]);
    rig.site.show_widget(true);
    let mut scraper = two_proxies(&rig).build().await.expect("the scraper builds");

    scraper.fetch(URL).await.expect("the challenge is passed");
    scraper.rotate_egress().await.expect("the egress rotates");
    scraper.rotate_profile().await.expect("the profile rotates");
    let identity = scraper
        .export_identity()
        .await
        .expect("the identity exports");
    scraper
        .import_identity(&identity)
        .await
        .expect("the identity imports");
    scraper
        .save_session("login")
        .await
        .expect("the session saves");

    let methods = rig.launcher.all_methods();
    assert!(
        methods.iter().any(|method| method == "Page.enable"),
        "the flow really ran"
    );
    for domain in FORBIDDEN_DOMAINS {
        let method = format!("{domain}.enable");
        assert!(!methods.contains(&method), "{method} was sent");
    }
}

#[tokio::test(start_paused = true)]
async fn no_secret_reaches_a_log_an_event_a_state_an_error_or_the_browser() {
    let rig = Rig::new(vec![widget_challenge(), real_page()]);
    rig.site.show_widget(true);
    let mut scraper = two_proxies(&rig).build().await.expect("the scraper builds");

    let fetched = scraper
        .fetch("https://shop.example/?token=hunter3")
        .await
        .expect("the challenge is passed");
    scraper.rotate_egress().await.expect("the egress rotates");
    let identity = scraper
        .export_identity()
        .await
        .expect("the identity exports");

    let events = serde_json::to_string(&rig.sink.events()).expect("events serialize");
    let state = serde_json::to_string(
        &crate::scrape::ledger::state::StateStore::get(&rig.store, "shop.example")
            .await
            .expect("the store reads"),
    )
    .expect("state serializes");
    let printed = format!(
        "{scraper:?} {fetched:?} {identity:?} {events} {state} {:?}",
        rig.sink.events()
    );
    for secret in [
        "s3cret",
        "hunter2",
        "hunter3",
        "ualpha",
        "ubeta",
        "fresh-token",
    ] {
        assert!(!printed.contains(secret), "{secret} in {printed}");
    }
    let sent: Vec<String> = (0..rig.launcher.count())
        .flat_map(|index| rig.launcher.nth(index).mock.calls())
        .map(|call| call.params.to_string())
        .collect();
    for secret in ["s3cret", "hunter2", "ualpha", "ubeta"] {
        assert!(
            !sent.iter().any(|params| params.contains(secret)),
            "{secret} was sent to the browser"
        );
    }
}
