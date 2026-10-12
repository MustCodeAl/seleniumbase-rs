//! The dual-mode `StealthSession` with the real Chrome on this machine.
//!
//! The story, end to end, against a loopback TLS shop that guards `/` with a script challenge:
//!
//! 1. The session asks over HTTP and meets the challenge.
//! 2. It starts the MITM proxy and a real Chrome behind it, with the same fingerprint and egress.
//! 3. Chrome runs the challenge page's script, which earns the clearance cookie and reloads.
//! 4. The cookies move from Chrome into the identity, Chrome is closed, and the session asks again
//!    over HTTP with the clearance. The shop sees one `ClientHello` shape and one user agent from
//!    both legs.
//!
//! The tests start Chrome, so they are ignored by default. Only one Chrome-driving test binary may
//! run on the machine at a time:
//!
//! ```text
//! cargo test --features impersonate --test stealth_session_chrome -- --ignored --test-threads=1
//! ```
#![cfg(feature = "impersonate")]

#[path = "support/measured_host.rs"]
mod measured_host;
#[path = "stealth_session/site.rs"]
mod site;

use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::impersonate::stealth_session::{StealthSession, StealthSessionBuilder};
use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::scrape::challenge::detect::ChallengeKind;
use seleniumbase_rs::scrape::challenge::mitigation::Mode;
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use seleniumbase_rs::scrape::passport::policy::{
    DemoteReason, EscalateReason, SessionPolicy, Transition,
};

use site::{ChallengeSite, CLEARANCE, DEFAULT_TTL_SECS, REAL_MARKER};

/// How long Chrome is given to run the challenge page's script.
const SOLVE_WITHIN: Duration = Duration::from_secs(30);

/// A builder wired to `site`, with Chrome headless.
fn builder(site: &ChallengeSite, sink: &CollectingEventSink) -> StealthSessionBuilder {
    StealthSession::builder()
        .launch_options(LaunchOptions::builder().headless(true))
        .http_options(site.http_options())
        .mitm_options(site.mitm_options())
        .with_event_sink(Arc::new(sink.clone()))
        .solve_timeout(SOLVE_WITHIN)
}

/// The modes the session went through, from its events.
fn changes(sink: &CollectingEventSink) -> Vec<(Mode, Mode, Transition)> {
    sink.events()
        .into_iter()
        .filter_map(|event| match event {
            ScraperEvent::ModeChanged { from, to, reason } => Some((from, to, reason)),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_challenge_is_passed_in_chrome_and_the_clearance_is_used_over_http() {
    if !measured_host::measured_or_skip(
        "a_challenge_is_passed_in_chrome_and_the_clearance_is_used_over_http",
    ) {
        return;
    }
    let site = ChallengeSite::start().await;
    let sink = CollectingEventSink::new();
    let mut session = builder(&site, &sink).build().await.expect("a session");

    let fetched = session.fetch(&site.url("/")).await.expect("a page");

    assert_eq!(fetched.status, 200);
    assert!(fetched.body.contains(REAL_MARKER), "{fetched:?}");
    assert_eq!(fetched.solved, Some(ChallengeKind::JsChallenge));
    assert_eq!((fetched.mode, fetched.attempts), (Mode::Http, 3));
    assert_eq!(session.mode(), Mode::Http);
    assert_eq!(
        changes(&sink),
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
    assert_origin_saw_one_browser(&site);
    assert_clearance_moved(&session, &site).await;
    let browser_requests = sink
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                ScraperEvent::Request {
                    mode: Mode::Browser,
                    ..
                }
            )
        })
        .count();
    assert!(
        browser_requests >= 3,
        "the proxy reports what Chrome asked: {browser_requests}"
    );
    session.close().await.expect("closes");
}

/// What the shop saw: the HTTP leg, then Chrome through the proxy, which ran the script that
/// earned the cookie, then the HTTP leg with the clearance (and, if Chrome got there before it
/// was closed, Chrome's reload with it), all with one `ClientHello` shape and one user agent.
fn assert_origin_saw_one_browser(site: &ChallengeSite) {
    let page: Vec<_> = site
        .seen()
        .into_iter()
        .filter(|seen| seen.path == "/" || seen.path == "/__solve")
        .collect();
    let paths: Vec<_> = page.iter().map(|seen| seen.path.as_str()).collect();
    assert_eq!(paths[..3], ["/", "/", "/__solve"], "{page:?}");
    assert!(paths[3..].iter().all(|path| *path == "/"), "{page:?}");
    assert!((4..=5).contains(&page.len()), "{page:?}");
    let token = site.token();
    assert!(
        page[..3].iter().all(|seen| !seen.carried(&token)),
        "{page:?}"
    );
    assert!(
        page[3..].iter().all(|seen| seen.carried(&token)),
        "{page:?}"
    );
    let (first, last) = (&page[0], &page[page.len() - 1]);
    for seen in &page {
        assert_eq!(
            seen.ja4, first.ja4,
            "one TLS fingerprint on every request: {seen:?}"
        );
        assert_eq!(
            seen.user_agent, first.user_agent,
            "one user agent: {seen:?}"
        );
    }
    assert_eq!(last.ja4, first.ja4);
}

/// The identity holds the clearance Chrome earned, with about the lifetime the shop gave it.
async fn assert_clearance_moved(session: &StealthSession, site: &ChallengeSite) {
    let identity = session.export_identity().await.expect("an identity");
    let names: Vec<_> = identity
        .cookies
        .cookies()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(names.contains(&CLEARANCE), "{names:?}");
    assert_eq!(identity.clearances.len(), 1);
    let left = identity.clearances[0]
        .expires_at
        .saturating_sub(identity.created_at);
    assert!(
        (DEFAULT_TTL_SECS - 120..=DEFAULT_TTL_SECS + 120).contains(&left),
        "about {DEFAULT_TTL_SECS} s of clearance, not {left}"
    );
    assert!(identity
        .cookies
        .cookies()
        .iter()
        .any(|c| c.value == site.token()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_clearance_near_its_end_is_taken_back_to_chrome_which_loads_the_page_with_it() {
    if !measured_host::measured_or_skip(
        "a_clearance_near_its_end_is_taken_back_to_chrome_which_loads_the_page_with_it",
    ) {
        return;
    }
    let site = ChallengeSite::start().await;
    let sink = CollectingEventSink::new();
    // An hour of margin against ten minutes of clearance: the next fetch is always "near the end".
    let policy = SessionPolicy {
        clearance_margin: Duration::from_secs(3600),
        ..SessionPolicy::default()
    };
    let mut session = builder(&site, &sink)
        .policy(policy)
        .build()
        .await
        .expect("a session");
    session.fetch(&site.url("/")).await.expect("a page");
    let solves = site.seen_at("/__solve").len();

    let again = session.fetch(&site.url("/")).await.expect("a page");

    assert_eq!(
        (again.mode, again.attempts, again.solved),
        (Mode::Browser, 1, None)
    );
    assert!(again.body.contains(REAL_MARKER), "{again:?}");
    assert_eq!(
        site.seen_at("/__solve").len(),
        solves,
        "Chrome had the clearance: no new solve"
    );
    let last = changes(&sink).pop().expect("a change");
    assert_eq!(last.0, Mode::Http);
    assert_eq!(last.1, Mode::Browser);
    assert!(
        matches!(
            last.2,
            Transition::Escalate(EscalateReason::ClearanceExpiring { remaining_secs })
                if remaining_secs <= DEFAULT_TTL_SECS
        ),
        "{last:?}"
    );
    session.demote().await.expect("back to HTTP");
    let over_http = session.fetch(&site.url("/")).await.expect("a page");
    assert_eq!(
        over_http.mode,
        Mode::Browser,
        "the policy sends it to Chrome again"
    );
    session.close().await.expect("closes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_new_profile_is_what_the_page_sees_and_the_proxy_stays() {
    if !measured_host::measured_or_skip("a_new_profile_is_what_the_page_sees_and_the_proxy_stays") {
        return;
    }
    let site = ChallengeSite::start().await;
    let sink = CollectingEventSink::new();
    let mut session = builder(&site, &sink)
        .seed(1)
        .build()
        .await
        .expect("a session");
    session.escalate().await.expect("escalates");
    let first = page_view(&session, &site).await;
    assert_eq!(
        first,
        expected_view(&session),
        "the page sees the identity's profile"
    );
    let seen_before = site.seen().len();

    session.rotate_profile().await.expect("rotates");

    let second = page_view(&session, &site).await;
    assert_eq!(
        second,
        expected_view(&session),
        "and the new one after the rotation"
    );
    assert_eq!(session.mode(), Mode::Browser);
    assert!(
        site.seen().len() > seen_before,
        "the new Chrome went through the proxy"
    );
    let agent = session.identity().fingerprint.user_agent.clone();
    assert_eq!(
        site.seen().last().and_then(|seen| seen.user_agent.clone()),
        agent
    );
    session.close().await.expect("closes");
}

/// What the page reports of the machine: cores, screen width and height.
type View = (Option<u64>, Option<u64>, Option<u64>);

/// Loads the unguarded page and reads the machine as the page sees it.
async fn page_view(session: &StealthSession, site: &ChallengeSite) -> View {
    let page = session.page().expect("Chrome is running");
    page.goto(site.url("/open")).await.expect("the page loads");
    let read = |script: &'static str| async move {
        page.evaluate_as::<Option<u64>>(script).await.ok().flatten()
    };
    (
        read("navigator.hardwareConcurrency").await,
        read("screen.width").await,
        read("screen.height").await,
    )
}

/// What the identity says the page should report.
fn expected_view(session: &StealthSession) -> View {
    let fingerprint = &session.identity().fingerprint;
    (
        fingerprint.hardware_concurrency.map(u64::from),
        fingerprint.screen_width.map(u64::from),
        fingerprint.screen_height.map(u64::from),
    )
}
