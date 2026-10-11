//! Tests of the fetch loop: what a fetch does with a real page, a challenge, a block and a rate limit.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant;

use super::rig::{block, country, real_page, script_challenge, widget_challenge, Rig, NOW};
use crate::scrape::challenge::detect::{ChallengeKind, Detection, Evidence};
use crate::scrape::challenge::mitigation::{Action, GiveUpReason};
use crate::scrape::egress::pool::{EgressOutcome, DEFAULT_QUARANTINE_AFTER};
use crate::scrape::egress::route::EgressRef;
use crate::scrape::error::ScrapeError;
use crate::scrape::ledger::events::ScraperEvent;
use crate::scrape::ledger::state::{DomainState, StateError, StateStore};
use crate::scrape::runtime::browser_scraper::fetch::{give_up, Step, Target};
use crate::scrape::runtime::cdp::testkit::Served;
use crate::scrape::runtime::cdp::PageSnapshot;

const URL: &str = "https://shop.example/";

/// How many mouse events of `kind` the browser launched last received.
fn mouse_events(rig: &Rig, kind: &str) -> usize {
    rig.launcher
        .last()
        .mock
        .calls_to("Input.dispatchMouseEvent")
        .iter()
        .filter(|call| call.params["type"] == kind)
        .count()
}

async fn state_of(rig: &Rig, site: &str) -> DomainState {
    rig.store
        .get(site)
        .await
        .expect("the store reads")
        .expect("the site was recorded")
}

#[tokio::test(start_paused = true)]
async fn a_real_page_is_returned_at_the_first_load() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");

    let fetched = scraper.fetch(URL).await.expect("the page is fetched");

    assert_eq!((fetched.status, fetched.attempts), (200, 1));
    assert_eq!(fetched.solved, None);
    assert_eq!(fetched.detection.kind, ChallengeKind::None);
    assert_eq!(fetched.url, URL);
    assert!(fetched.headers.iter().any(|(name, _)| name == "server"));
    assert!(fetched.body.contains("<html") || fetched.body.contains("<HTML"));
    assert_eq!(rig.event_names(), ["request", "action_chosen"]);
    let state = state_of(&rig, "shop.example").await;
    assert_eq!(
        (
            state.successes,
            state.challenges,
            state.consecutive_challenges
        ),
        (1, 0, 0)
    );
    assert_eq!(state.last_seen, NOW);
}

#[tokio::test(start_paused = true)]
async fn a_widget_challenge_is_clicked_and_the_page_is_loaded_again_with_the_clearance() {
    let rig = Rig::new(vec![widget_challenge(), real_page()]);
    rig.site.show_widget(true);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");

    let fetched = scraper.fetch(URL).await.expect("the challenge is passed");

    assert_eq!(fetched.attempts, 2);
    assert_eq!(fetched.solved, Some(ChallengeKind::Interactive));
    assert!(rig.site.is_cleared());
    assert_eq!(
        mouse_events(&rig, "mousePressed"),
        1,
        "the widget is clicked once"
    );
    assert!(
        mouse_events(&rig, "mouseWheel") >= 1,
        "the pointer arrives on the page like a person's before the click"
    );
    assert_eq!(
        rig.event_names(),
        [
            "request",
            "challenge_detected",
            "action_chosen",
            "request",
            "action_chosen",
            "challenge_solved"
        ]
    );
    let solved = rig.sink.events().pop().expect("an event");
    assert!(matches!(
        solved,
        ScraperEvent::ChallengeSolved {
            kind: ChallengeKind::Interactive,
            attempts: 2,
            ..
        }
    ));
    let state = state_of(&rig, "shop.example").await;
    assert_eq!(
        (
            state.successes,
            state.challenges,
            state.consecutive_challenges
        ),
        (1, 1, 0)
    );
    assert_eq!(state.last_challenge, Some(ChallengeKind::Interactive));
}

#[tokio::test(start_paused = true)]
async fn a_script_challenge_is_waited_for_without_a_click() {
    let rig = Rig::new(vec![script_challenge(), real_page()]);
    rig.site.clear_at_poll(3);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");

    let fetched = scraper.fetch(URL).await.expect("the challenge is passed");

    assert_eq!(fetched.solved, Some(ChallengeKind::JsChallenge));
    assert_eq!(fetched.attempts, 2);
    assert_eq!(
        mouse_events(&rig, "mousePressed"),
        0,
        "a script challenge needs no click"
    );
}

#[tokio::test(start_paused = true)]
async fn a_challenge_that_never_clears_is_unsolved_after_the_attempts_allowed() {
    let rig = Rig::new(vec![script_challenge()]);
    let mut scraper = rig
        .builder()
        .with_max_challenge_attempts(3)
        .build()
        .await
        .expect("the scraper builds");

    let error = scraper.fetch(URL).await.expect_err("it is never passed");

    assert!(
        matches!(
            &error,
            ScrapeError::Unsolved {
                kind: ChallengeKind::JsChallenge,
                attempts: 3,
                ..
            }
        ),
        "{error:?}"
    );
    let state = state_of(&rig, "shop.example").await;
    assert_eq!((state.challenges, state.consecutive_challenges), (3, 3));
    let chosen: Vec<Action> = rig
        .sink
        .events()
        .into_iter()
        .filter_map(|event| match event {
            ScraperEvent::ActionChosen { action, .. } => Some(action),
            _ => None,
        })
        .collect();
    assert_eq!(
        chosen,
        [
            Action::Solve,
            Action::Solve,
            Action::GiveUp(GiveUpReason::AttemptsExhausted)
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn repeated_challenges_change_the_egress_and_then_the_profile() {
    let rig = Rig::new(vec![script_challenge()]);
    let mut scraper = rig
        .builder()
        .with_geo_proxies([
            ("http://de.proxy.example:8080", country("DE")),
            ("http://fr.proxy.example:8080", country("FR")),
        ])
        .expect("valid specs")
        .with_max_challenge_attempts(4)
        .build()
        .await
        .expect("the scraper builds");
    let first_seed = scraper.seed();

    let error = scraper.fetch(URL).await.expect_err("it is never passed");

    assert!(
        matches!(&error, ScrapeError::Unsolved { attempts: 4, .. }),
        "{error:?}"
    );
    let events = rig.sink.events();
    let rotated = events
        .iter()
        .find_map(|event| match event {
            ScraperEvent::EgressRotated { from, to } => Some((from.clone(), to.clone())),
            _ => None,
        })
        .expect("the egress was rotated");
    assert_eq!(
        rotated.0.map(|key| key.host),
        Some("de.proxy.example".to_owned())
    );
    assert_eq!(
        rotated.1.map(|key| key.host),
        Some("fr.proxy.example".to_owned())
    );
    assert!(events
        .iter()
        .any(|event| matches!(event, ScraperEvent::ProfileRotated { from_seed, .. } if *from_seed == first_seed)));
    assert_eq!(
        rig.launcher.count(),
        4,
        "a probe, the first browser, one for the other country, one for the new profile"
    );
    for old in [1, 2] {
        assert!(
            !rig.launcher.nth(old).browser.is_connected(),
            "browser #{old} is closed"
        );
    }
    assert!(rig.launcher.nth(3).browser.is_connected());
    assert_eq!(
        scraper.egress().map(|key| key.host.as_str()),
        Some("fr.proxy.example")
    );
    assert_eq!(scraper.fingerprint().locale.as_deref(), Some("fr-FR"));
    assert_ne!(scraper.seed(), first_seed);
    let state = state_of(&rig, "shop.example").await;
    assert_eq!(
        state.challenges, 4,
        "what is known of the site survives the rotations"
    );
}

#[tokio::test(start_paused = true)]
async fn a_block_without_another_egress_starts_a_cooldown_the_next_fetch_respects() {
    let rig = Rig::new(vec![block()]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");

    let error = scraper.fetch(URL).await.expect_err("the site blocks");
    let until = NOW + 900;
    assert!(
        matches!(&error, ScrapeError::CoolingDown { site, until: at } if site == "shop.example" && *at == until),
        "{error:?}"
    );
    let state = state_of(&rig, "shop.example").await;
    assert_eq!((state.blocks, state.cooldown_until), (1, Some(until)));
    assert!(rig.event_names().contains(&"cooldown_started"));

    let again = scraper
        .fetch(URL)
        .await
        .expect_err("the site is left alone");
    assert!(matches!(again, ScrapeError::CoolingDown { .. }));
    assert_eq!(
        rig.site.navigations().len(),
        1,
        "the site was not requested again"
    );

    rig.advance(901);
    scraper.fetch(URL).await.expect_err("it blocks again");
    assert_eq!(
        rig.site.navigations().len(),
        2,
        "after the cooldown it is tried again"
    );
}

#[tokio::test(start_paused = true)]
async fn a_block_with_another_egress_moves_there_and_tries_again() {
    let rig = Rig::new(vec![block(), real_page()]);
    let mut scraper = rig
        .builder()
        .with_geo_proxies([
            ("http://de.proxy.example:8080", country("DE")),
            ("http://fr.proxy.example:8080", country("FR")),
        ])
        .expect("valid specs")
        .build()
        .await
        .expect("the scraper builds");

    let fetched = scraper.fetch(URL).await.expect("the second egress gets in");

    assert_eq!(fetched.attempts, 2);
    assert_eq!(
        fetched.egress.map(|key| key.host),
        Some("fr.proxy.example".to_owned())
    );
    assert_eq!(
        rig.launcher.count(),
        3,
        "the French egress needed a new browser"
    );
    assert_eq!(rig.launcher.last().options.lang.as_deref(), Some("fr-FR"));
    assert_eq!(
        scraper.fingerprint().timezone.as_deref(),
        Some("Europe/Paris")
    );
    let state = state_of(&rig, "shop.example").await;
    assert_eq!(
        state.last_egress.map(|key| key.host),
        Some("fr.proxy.example".to_owned())
    );
}

#[tokio::test(start_paused = true)]
async fn a_block_with_every_other_egress_out_of_rotation_gives_up_instead_of_looping() {
    let rig = Rig::new(vec![block()]);
    let mut scraper = rig
        .builder()
        .with_geo_proxies([
            ("http://de.proxy.example:8080", country("DE")),
            ("http://fr.proxy.example:8080", country("FR")),
        ])
        .expect("valid specs")
        .build()
        .await
        .expect("the scraper builds");
    let france = EgressRef::parse("http://fr.proxy.example:8080", Some(country("FR")))
        .expect("a valid spec");
    let pool = scraper.egress.pool.as_ref().expect("a pool");
    for _ in 0..DEFAULT_QUARANTINE_AFTER {
        pool.report(france.key(), EgressOutcome::Blocked, NOW);
    }

    let error = scraper
        .fetch(URL)
        .await
        .expect_err("there is nowhere to go");

    assert!(
        matches!(
            &error,
            ScrapeError::GaveUp {
                reason: GiveUpReason::NoEgressLeft,
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(rig.launcher.count(), 2, "no browser was started for it");
}

#[tokio::test(start_paused = true)]
async fn a_rate_limit_waits_as_long_as_the_site_asked() {
    let limited = Served::new(429, "slow down").header("Retry-After", "5");
    let rig = Rig::new(vec![limited]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");

    let error = scraper.fetch(URL).await.expect_err("the site limits");

    assert!(
        matches!(&error, ScrapeError::CoolingDown { until, .. } if *until == NOW + 5),
        "{error:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_page_that_will_not_load_is_a_navigation_error_and_the_pool_hears_of_it() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    rig.launcher.last().mock.reply(
        "Page.navigate",
        serde_json::json!({ "errorText": "net::ERR_CONNECTION_REFUSED" }),
    );

    let error = scraper
        .fetch("https://alice:s3cret@shop.example/?token=hunter2")
        .await
        .expect_err("the page does not load");

    assert!(
        matches!(&error, ScrapeError::Navigation { .. }),
        "{error:?}"
    );
    let shown = format!("{error} {error:?}");
    for secret in ["alice", "s3cret", "hunter2"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
}

#[tokio::test(start_paused = true)]
async fn a_site_in_cooldown_in_the_store_is_not_requested() {
    let rig = Rig::new(vec![real_page()]);
    let mut remembered = DomainState::new("shop.example");
    remembered.start_cooldown(NOW + 100);
    rig.store.put(remembered).await.expect("the store writes");
    let mut scraper = rig.builder().build().await.expect("the scraper builds");

    let error = scraper.fetch(URL).await.expect_err("left alone");

    assert!(matches!(error, ScrapeError::CoolingDown { until, .. } if until == NOW + 100));
    assert_eq!(rig.site.navigations().len(), 0);
}

#[tokio::test(start_paused = true)]
async fn fetches_of_one_site_are_paced_and_other_sites_are_not() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig
        .builder()
        .per_site_rate(1, Duration::from_millis(400))
        .build()
        .await
        .expect("the scraper builds");
    let start = Instant::now();

    scraper.fetch(URL).await.expect("the first fetch");
    scraper
        .fetch("https://other.example/")
        .await
        .expect("another site");
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "another site is not slowed"
    );
    scraper.fetch(URL).await.expect("the second fetch");

    assert!(
        start.elapsed() >= Duration::from_millis(350),
        "{:?}",
        start.elapsed()
    );
}

#[tokio::test(start_paused = true)]
async fn a_scraper_left_idle_for_a_week_still_works() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    scraper.fetch(URL).await.expect("the first fetch");

    tokio::time::advance(Duration::from_hours(7 * 24)).await;

    let kept = rig.launcher.last();
    assert!(kept.browser.is_connected(), "nothing closed the browser");
    scraper
        .fetch(URL)
        .await
        .expect("the fetch after the idle week");
    assert!(
        !rig.launcher
            .all_methods()
            .iter()
            .any(|method| method == "Browser.close"),
        "the scraper never closes a browser it was not told to"
    );
    scraper.close().await.expect("the scraper closes");
    assert!(!kept.browser.is_connected());
}

#[tokio::test(start_paused = true)]
async fn solving_a_page_that_is_not_a_challenge_changes_nothing() {
    let rig = Rig::new(vec![real_page()]);
    rig.site.show_widget(true);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    scraper.fetch(URL).await.expect("the page is fetched");

    let detection = scraper.solve_challenge().await.expect("nothing to solve");

    assert_eq!(detection.kind, ChallengeKind::None);
    assert_eq!(mouse_events(&rig, "mousePressed"), 0);
}

fn snapshot(kind: ChallengeKind) -> PageSnapshot {
    PageSnapshot {
        detection: Detection {
            kind,
            evidence: Evidence::default(),
        },
        body: String::new(),
    }
}

#[tokio::test(start_paused = true)]
async fn a_retry_waits_the_time_it_was_given_and_asks_for_another_load() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    let target = Target::parse(URL).expect("a valid URL");
    let mut state = DomainState::new("shop.example");
    let start = Instant::now();

    let step = scraper
        .act(
            Action::Retry {
                after: Duration::from_secs(5),
            },
            &target,
            &mut state,
            &snapshot(ChallengeKind::JsChallenge),
            1,
        )
        .await
        .expect("a retry is not an error");

    assert!(matches!(step, Step::Again));
    assert!(start.elapsed() >= Duration::from_secs(5));
}

#[tokio::test(start_paused = true)]
async fn an_action_the_loop_does_not_know_ends_it_instead_of_being_guessed_at() {
    let rig = Rig::new(vec![real_page()]);
    let mut scraper = rig.builder().build().await.expect("the scraper builds");
    let target = Target::parse(URL).expect("a valid URL");
    let mut state = DomainState::new("shop.example");

    let result = scraper
        .act(
            Action::Escalate,
            &target,
            &mut state,
            &snapshot(ChallengeKind::JsChallenge),
            2,
        )
        .await;

    assert!(
        matches!(
            result,
            Err(ScrapeError::Unsolved {
                kind: ChallengeKind::JsChallenge,
                attempts: 2,
                ..
            })
        ),
        "{:?}",
        result.err()
    );
    assert_eq!(rig.site.navigations().len(), 0, "nothing was loaded for it");
}

#[test]
fn a_policy_that_stops_is_an_unsolved_challenge_only_when_a_challenge_stood() {
    let target = Target::parse(URL).expect("a valid URL");
    let stop = |reason: GiveUpReason, kind: ChallengeKind| {
        give_up(&target, &snapshot(kind).detection, reason, 3)
    };
    let limited = ChallengeKind::RateLimited {
        retry_after_secs: None,
    };

    for kind in [ChallengeKind::JsChallenge, ChallengeKind::Interactive] {
        let unsolved = stop(GiveUpReason::AttemptsExhausted, kind);
        assert!(
            matches!(unsolved, ScrapeError::Unsolved { attempts: 3, .. }),
            "{unsolved:?}"
        );
    }
    for (reason, kind) in [
        (GiveUpReason::AttemptsExhausted, limited),
        (GiveUpReason::Blocked, ChallengeKind::Blocked),
        (GiveUpReason::NoEgressLeft, ChallengeKind::JsChallenge),
    ] {
        let gave_up = stop(reason, kind);
        assert!(
            matches!(&gave_up, ScrapeError::GaveUp { reason: found, .. } if *found == reason),
            "{gave_up:?}"
        );
    }
}

/// A store that reads fine and cannot write.
struct ReadOnlyStore;

#[async_trait]
impl StateStore for ReadOnlyStore {
    async fn get(&self, _site: &str) -> Result<Option<DomainState>, StateError> {
        Ok(None)
    }

    async fn put(&self, _state: DomainState) -> Result<(), StateError> {
        Err(StateError::TooLarge {
            path: PathBuf::from("state.json"),
            limit: 0,
        })
    }

    async fn remove(&self, _site: &str) -> Result<(), StateError> {
        Ok(())
    }

    async fn sites(&self) -> Result<Vec<String>, StateError> {
        Ok(Vec::new())
    }
}

#[tokio::test(start_paused = true)]
async fn a_store_that_cannot_write_fails_a_fetch_that_worked_but_not_the_reason_it_failed() {
    let worked = Rig::new(vec![real_page()]);
    let mut scraper = worked
        .builder()
        .with_state_store(Arc::new(ReadOnlyStore))
        .build()
        .await
        .expect("the scraper builds");
    let saved = scraper
        .fetch(URL)
        .await
        .expect_err("the state cannot be saved");
    assert!(matches!(saved, ScrapeError::State(_)), "{saved:?}");

    let failed = Rig::new(vec![script_challenge()]);
    let mut scraper = failed
        .builder()
        .with_state_store(Arc::new(ReadOnlyStore))
        .with_max_challenge_attempts(1)
        .build()
        .await
        .expect("the scraper builds");
    let reason = scraper.fetch(URL).await.expect_err("the challenge stands");
    assert!(matches!(reason, ScrapeError::Unsolved { .. }), "{reason:?}");
}
