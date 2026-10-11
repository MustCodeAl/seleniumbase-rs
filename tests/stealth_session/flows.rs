//! The fetch loop and the changes of mode: every transition of the state machine, with the HTTP leg
//! real (a session against a loopback TLS shop) and the browser leg scripted.

use seleniumbase_rs::impersonate::stealth_session::StealthError;
use seleniumbase_rs::scrape::challenge::detect::ChallengeKind;
use seleniumbase_rs::scrape::challenge::mitigation::Mode;
use seleniumbase_rs::scrape::error::ScrapeError;
use seleniumbase_rs::scrape::ledger::events::ScraperEvent;
use seleniumbase_rs::scrape::ledger::state::StateStore;
use seleniumbase_rs::scrape::passport::policy::{
    DemoteReason, EscalateReason, SessionPolicy, Transition,
};

use crate::site::{CLEARANCE, HOST, REAL_MARKER};
use crate::world::World;

/// The events of a session that met a challenge over HTTP, solved it in the browser and finished
/// over HTTP, in the order the session emits them: the first is the HTTP leg starting.
const FULL_FLOW: [&str; 14] = [
    "identity_restored",
    "request",
    "challenge_detected",
    "action_chosen",
    "identity_exported",
    "identity_restored",
    "mode_changed",
    "challenge_detected",
    "action_chosen",
    "identity_exported",
    "identity_restored",
    "mode_changed",
    "request",
    "challenge_solved",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unguarded_page_is_fetched_over_http_and_no_browser_is_started() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    assert_eq!(session.mode(), Mode::Http);

    let fetched = session
        .fetch(&world.site.url("/open"))
        .await
        .expect("a page");

    assert_eq!(fetched.status, 200);
    assert_eq!((fetched.mode, fetched.attempts), (Mode::Http, 1));
    assert!(fetched.body.contains(REAL_MARKER));
    assert_eq!(fetched.solved, None);
    assert_eq!(world.launcher.count(), 0, "no browser is needed");
    assert!(world.changes().is_empty());
    let state = world
        .store
        .get(HOST)
        .await
        .expect("a store")
        .expect("a state");
    assert_eq!(
        (state.successes, state.preferred_mode),
        (1, Some(Mode::Http))
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_challenge_over_http_goes_to_the_browser_and_the_clearance_comes_back_to_http() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");

    let fetched = session.fetch(&world.site.url("/")).await.expect("a page");

    assert_eq!(fetched.status, 200);
    assert!(fetched.body.contains(REAL_MARKER));
    assert_eq!(fetched.solved, Some(ChallengeKind::JsChallenge));
    assert_eq!((fetched.mode, fetched.attempts), (Mode::Http, 3));
    assert_eq!(session.mode(), Mode::Http, "the session went back to HTTP");
    assert_eq!(world.names(), FULL_FLOW);
    assert_eq!(
        world.changes(),
        [
            (
                Mode::Http,
                Mode::Browser,
                Transition::Escalate(EscalateReason::Challenge(ChallengeKind::JsChallenge))
            ),
            (
                Mode::Browser,
                Mode::Http,
                Transition::Demote(DemoteReason::ChallengeSolved)
            ),
        ]
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_browser_runs_behind_the_proxy_and_is_closed_when_the_session_goes_back() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");

    session.fetch(&world.site.url("/")).await.expect("a page");

    assert_eq!(world.launcher.count(), 1);
    let browser = world.launcher.nth(0);
    let server = browser.proxy_server().expect("the browser has a proxy");
    assert!(server.starts_with("http://127.0.0.1:"), "{server}");
    assert!(browser
        .options
        .proxy()
        .is_some_and(|proxy| proxy.has_credentials()));
    assert!(!browser.browser.is_connected(), "the browser was closed");
    assert!(browser.is_cleared());
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_site_sees_the_clearance_on_the_request_after_the_solve_and_one_fingerprint() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");

    session.fetch(&world.site.url("/")).await.expect("a page");

    // The browser is scripted, so the only traffic is the session's own HTTP leg.
    let seen = world.site.seen();
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_eq!(seen[0].cookie, None, "the first request had no clearance");
    assert!(seen[1].carried(&world.site.token()), "{seen:?}");
    assert_eq!(seen[0].ja4, seen[1].ja4, "one ClientHello shape");
    assert_eq!(seen[0].user_agent, seen[1].user_agent);
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn what_the_site_learns_is_kept_and_a_solved_site_is_fetched_over_http_next_time() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");

    session.fetch(&world.site.url("/")).await.expect("a page");
    let again = session.fetch(&world.site.url("/")).await.expect("a page");

    assert_eq!(
        (again.mode, again.attempts, again.solved),
        (Mode::Http, 1, None)
    );
    assert_eq!(
        world.launcher.count(),
        1,
        "the clearance is used, not earned again"
    );
    let state = world
        .store
        .get(HOST)
        .await
        .expect("a store")
        .expect("a state");
    assert_eq!(state.challenges, 2, "one over HTTP, one in the browser");
    assert_eq!(state.preferred_mode, Some(Mode::Http));
    assert_eq!(state.consecutive_challenges, 0);
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_session_that_prefers_the_browser_starts_in_it_and_stays_after_a_solve() {
    let world = World::new().await;
    let policy = SessionPolicy {
        prefer: Mode::Browser,
        ..SessionPolicy::default()
    };
    let mut session = world
        .session()
        .policy(policy)
        .build()
        .await
        .expect("a session");
    assert_eq!(session.mode(), Mode::Browser);
    assert_eq!(world.launcher.count(), 1);

    let fetched = session.fetch(&world.site.url("/")).await.expect("a page");

    assert_eq!((fetched.mode, fetched.attempts), (Mode::Browser, 2));
    assert_eq!(fetched.solved, Some(ChallengeKind::JsChallenge));
    assert!(fetched.body.contains(REAL_MARKER));
    assert!(
        world.changes().is_empty(),
        "a session that prefers the browser has nowhere to go"
    );
    assert_eq!(world.launcher.count(), 1);
    assert!(
        world.site.seen().is_empty(),
        "the browser is scripted: no HTTP request"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn escalate_and_demote_by_hand_say_why_and_do_nothing_twice() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");

    session.escalate().await.expect("escalates");
    session.escalate().await.expect("already there");
    assert_eq!((session.mode(), world.launcher.count()), (Mode::Browser, 1));
    assert!(session.page().is_some() && session.http().is_none());

    session.demote().await.expect("demotes");
    session.demote().await.expect("already there");
    assert_eq!(session.mode(), Mode::Http);
    assert!(session.http().is_some() && session.page().is_none());
    assert!(!world.launcher.last().browser.is_connected());
    assert_eq!(
        world.changes(),
        [
            (
                Mode::Http,
                Mode::Browser,
                Transition::Escalate(EscalateReason::Manual)
            ),
            (
                Mode::Browser,
                Mode::Http,
                Transition::Demote(DemoteReason::Manual)
            ),
        ]
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_browser_that_died_does_not_trap_the_session_in_it() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.escalate().await.expect("escalates");
    world
        .launcher
        .last()
        .browser
        .close()
        .await
        .expect("the browser goes away");

    let error = session
        .fetch(&world.site.url("/"))
        .await
        .expect_err("nobody to load the page");
    assert!(matches!(error, StealthError::Scrape(_)), "{error:?}");
    session
        .demote()
        .await
        .expect("back to HTTP with the last known identity");

    assert_eq!(session.mode(), Mode::Http);
    let fetched = session
        .fetch(&world.site.url("/open"))
        .await
        .expect("a page");
    assert_eq!(fetched.status, 200);
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_clearance_near_its_end_sends_the_next_fetch_to_the_browser() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.fetch(&world.site.url("/")).await.expect("a page");
    assert_eq!(world.launcher.count(), 1);

    // 600 s of clearance, a margin of 120 s: still good at 200 s left.
    world.advance(400);
    let early = session.fetch(&world.site.url("/")).await.expect("a page");
    assert_eq!((early.mode, world.launcher.count()), (Mode::Http, 1));

    // 115 s left: the policy does not wait for the site to refuse.
    world.advance(85);
    let late = session.fetch(&world.site.url("/")).await.expect("a page");

    assert_eq!(
        (late.mode, late.attempts, late.solved),
        (Mode::Browser, 1, None)
    );
    assert_eq!(world.launcher.count(), 2);
    assert!(world
        .launcher
        .last()
        .imported_names()
        .contains(&CLEARANCE.to_owned()));
    let last = world.changes().last().copied().expect("a change");
    assert_eq!(
        last,
        (
            Mode::Http,
            Mode::Browser,
            Transition::Escalate(EscalateReason::ClearanceExpiring {
                remaining_secs: 115
            })
        )
    );
    let expiring = world
        .sink
        .events()
        .into_iter()
        .filter(|event| matches!(event, ScraperEvent::ClearanceExpiring { .. }))
        .count();
    assert_eq!(expiring, 1);
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fresh_challenge_after_a_clearance_goes_to_the_browser_again() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.fetch(&world.site.url("/")).await.expect("a page");

    world.site.revoke();
    let again = session.fetch(&world.site.url("/")).await.expect("a page");

    assert_eq!((again.mode, again.attempts), (Mode::Http, 3));
    assert_eq!(again.solved, Some(ChallengeKind::JsChallenge));
    assert_eq!(
        world.launcher.count(),
        2,
        "a second browser earned the new clearance"
    );
    assert_eq!(world.changes().len(), 4);
    let last = world.site.seen().pop().expect("a request");
    assert!(
        last.carried(&world.site.token()),
        "the new clearance, not the old one"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_challenge_that_cannot_be_solved_gives_up_after_the_attempts_allowed() {
    let world = World::new().await;
    world.launcher.never_solve();
    let mut session = world
        .session()
        .solve_timeout(crate::world::BRIEF_SOLVE)
        .build()
        .await
        .expect("a session");

    let error = session
        .fetch(&world.site.url("/"))
        .await
        .expect_err("nothing clears");

    assert!(
        matches!(
            error,
            StealthError::Scrape(ScrapeError::Unsolved {
                kind: ChallengeKind::JsChallenge,
                attempts: 4,
                ..
            })
        ),
        "{error:?}"
    );
    assert_eq!(
        session.mode(),
        Mode::Browser,
        "no clearance, so nothing to carry over HTTP"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rate_limited_site_is_left_alone_until_the_wait_is_over() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    let url = world.site.url("/limited");
    let start = world.clock.load(std::sync::atomic::Ordering::SeqCst);

    let first = session.fetch(&url).await.expect_err("rate limited");
    let until = match first {
        StealthError::Scrape(ScrapeError::CoolingDown { until, .. }) => until,
        other => panic!("expected a cooldown, got {other:?}"),
    };
    assert_eq!(until, start + 120, "the Retry-After the site named");
    let second = session.fetch(&url).await.expect_err("still cooling down");
    assert!(matches!(
        second,
        StealthError::Scrape(ScrapeError::CoolingDown { .. })
    ));
    assert_eq!(
        world.site.seen_at("/limited").len(),
        1,
        "the site was not asked again"
    );

    world.advance(121);
    let third = session.fetch(&url).await.expect_err("limited again");
    assert!(matches!(
        third,
        StealthError::Scrape(ScrapeError::CoolingDown { .. })
    ));
    assert_eq!(
        world.site.seen_at("/limited").len(),
        2,
        "the wait was over, so it asked"
    );
    assert!(world.names().contains(&"cooldown_started"));
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_browser_that_is_not_the_measured_chrome_is_refused_and_the_session_stays_as_it_was() {
    let world = World::new().await;
    let drifted = crate::world::measured_major() + 1;
    world
        .launcher
        .set_product(&format!("Chrome/{drifted}.0.1.0"));
    let mut session = world.session().build().await.expect("a session");

    let error = session
        .escalate()
        .await
        .expect_err("the Chrome has drifted");

    assert!(
        matches!(
            error,
            StealthError::Impersonate(
                seleniumbase_rs::impersonate::client::ImpersonateError::VersionMismatch {
                    browser,
                    ..
                }
            ) if browser == drifted
        ),
        "{error:?}"
    );
    assert_eq!(session.mode(), Mode::Http);
    assert!(
        !world.launcher.last().browser.is_connected(),
        "the browser was closed"
    );
    assert!(world.changes().is_empty());
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closing_the_session_closes_the_browser_and_the_proxy() {
    let world = World::new().await;
    let mut session = world.session().build().await.expect("a session");
    session.escalate().await.expect("escalates");
    let launched = world.launcher.last();
    let server = launched.proxy_server().expect("a proxy");
    let address = server.trim_start_matches("http://").to_owned();
    assert!(
        tokio::net::TcpStream::connect(&address).await.is_ok(),
        "the proxy listens"
    );

    session.close().await.expect("closes");

    assert!(!launched.browser.is_connected());
    assert!(
        tokio::net::TcpStream::connect(&address).await.is_err(),
        "the proxy stopped listening"
    );
}
