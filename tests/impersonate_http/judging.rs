//! A reply judged for the session policy: a challenge page, a clear page, a rate limit, and a
//! clearance about to end.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use seleniumbase_rs::impersonate::http_session::{Attempt, HttpError, HttpSession, Rules};
use seleniumbase_rs::scrape::challenge::detect::{ChallengeKind, Vendor};
use seleniumbase_rs::scrape::challenge::mitigation::{Action, Mode};
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use seleniumbase_rs::scrape::ledger::state::DomainState;
use seleniumbase_rs::scrape::passport::policy::{EscalateReason, Transition};
use url::Url;

use crate::site::{identity, now, Answer, Site};

const HOST: &str = "shop.test";
const CHALLENGE: &str = "<html><head><title>Just a moment...</title></head><body><div id=\"challenge-form\"></div></body></html>";

fn clocked(site: &Site, clock: &Arc<AtomicU64>, sink: &Arc<CollectingEventSink>) -> HttpSession {
    let reader = Arc::clone(clock);
    let options = site
        .options(&[HOST])
        .clock(Arc::new(move || reader.load(Ordering::SeqCst)))
        .event_sink(sink.clone());
    HttpSession::from_identity(&identity(), None, options).expect("a session")
}

#[tokio::test]
async fn a_challenge_page_asks_for_the_browser_and_says_so_in_events() {
    let site = Site::start(|_| {
        Answer::status(403)
            .header("content-type", "text/html")
            .body(CHALLENGE.as_bytes())
    })
    .await;
    let clock = Arc::new(AtomicU64::new(now()));
    let sink = Arc::new(CollectingEventSink::new());
    let session = clocked(&site, &clock, &sink);
    let _setup = sink.take();

    let fetched = session
        .fetch(
            &site.url(HOST, "/"),
            &Attempt::default(),
            &Rules::default(),
            &identity(),
        )
        .await
        .expect("a reply");

    assert_eq!(fetched.reply.status(), 403);
    assert!(!fetched.verdict.is_clear());
    assert_eq!(fetched.verdict.detection.kind, ChallengeKind::JsChallenge);
    assert_eq!(
        fetched.verdict.detection.evidence.vendor,
        Some(Vendor::Cloudflare)
    );
    assert_eq!(fetched.verdict.action, Action::Escalate);
    assert_eq!(
        fetched.verdict.transition,
        Transition::Escalate(EscalateReason::Challenge(ChallengeKind::JsChallenge))
    );
    let names: Vec<&str> = sink.events().iter().map(ScraperEvent::name).collect();
    assert_eq!(names, ["request", "challenge_detected", "action_chosen"]);
}

#[tokio::test]
async fn without_a_browser_the_challenge_is_retried_and_then_given_up() {
    let site = Site::start(|_| Answer::status(403).body(CHALLENGE.as_bytes())).await;
    let clock = Arc::new(AtomicU64::new(now()));
    let sink = Arc::new(CollectingEventSink::new());
    let session = clocked(&site, &clock, &sink);
    let alone = |number, previous_challenges| Attempt {
        number,
        previous_challenges,
        can_rotate_egress: false,
        can_escalate: false,
    };

    let first = session
        .fetch(
            &site.url(HOST, "/"),
            &alone(1, 0),
            &Rules::default(),
            &identity(),
        )
        .await
        .expect("a reply");
    let last = session
        .fetch(
            &site.url(HOST, "/"),
            &alone(3, 2),
            &Rules::default(),
            &identity(),
        )
        .await
        .expect("a reply");

    assert!(
        matches!(first.verdict.action, Action::Retry { .. }),
        "{:?}",
        first.verdict
    );
    assert_eq!(first.verdict.transition, Transition::Stay);
    assert!(
        matches!(last.verdict.action, Action::GiveUp(_)),
        "{:?}",
        last.verdict
    );
}

#[tokio::test]
async fn a_clear_page_proceeds_and_stays_in_http_mode() {
    let site = Site::start(|_| Answer::ok("<html><body>products</body></html>")).await;
    let clock = Arc::new(AtomicU64::new(now()));
    let sink = Arc::new(CollectingEventSink::new());
    let session = clocked(&site, &clock, &sink);
    let _setup = sink.take();

    let fetched = session
        .fetch(
            &site.url(HOST, "/"),
            &Attempt::default(),
            &Rules::default(),
            &identity(),
        )
        .await
        .expect("a reply");

    assert!(fetched.verdict.is_clear());
    assert_eq!(fetched.verdict.action, Action::Proceed);
    assert_eq!(fetched.verdict.transition, Transition::Stay);
    assert_eq!(sink.len(), 1, "only the request event");
}

#[tokio::test]
async fn a_rate_limit_waits_as_long_as_the_server_says() {
    let site = Site::start(|_| {
        Answer::status(429)
            .header("retry-after", "7")
            .body(b"slow down")
    })
    .await;
    let clock = Arc::new(AtomicU64::new(now()));
    let sink = Arc::new(CollectingEventSink::new());
    let session = clocked(&site, &clock, &sink);

    let fetched = session
        .fetch(
            &site.url(HOST, "/"),
            &Attempt::default(),
            &Rules::default(),
            &identity(),
        )
        .await
        .expect("a reply");

    assert_eq!(
        fetched.verdict.detection.kind,
        ChallengeKind::RateLimited {
            retry_after_secs: Some(7)
        }
    );
    assert_eq!(
        fetched.verdict.action,
        Action::Cooldown {
            until: clock.load(Ordering::SeqCst) + 7
        }
    );
    assert_eq!(fetched.verdict.transition, Transition::Stay);
}

/// An identity holding a clearance for `shop.test` that ends `lifetime` seconds after `at`.
fn cleared(at: u64, lifetime: u64) -> seleniumbase_rs::scrape::passport::identity::StealthIdentity {
    let mut base = identity();
    let url = Url::parse(&format!("https://{HOST}/")).expect("a url");
    let line = format!("cf_clearance=CLEARANCE; Path=/; Secure; HttpOnly; Max-Age={lifetime}");
    base.cookies.store(&url, &line, at);
    base.refresh_clearances(at);
    base
}

#[tokio::test]
async fn a_clearance_about_to_end_sends_the_session_to_the_browser_first() {
    let site = Site::start(|_| Answer::ok("")).await;
    let start = now();
    let clock = Arc::new(AtomicU64::new(start));
    let sink = Arc::new(CollectingEventSink::new());
    let base = cleared(start, 3600);
    let reader = Arc::clone(&clock);
    let options = site
        .options(&[HOST])
        .clock(Arc::new(move || reader.load(Ordering::SeqCst)))
        .event_sink(sink.clone());
    let session = HttpSession::from_identity(&base, None, options).expect("a session");
    let url = site.url(HOST, "/");
    let rules = Rules::default();

    let fresh = session
        .preflight(&url, &base, None, &rules)
        .expect("a verdict");
    clock.store(start + 3600 - 60, Ordering::SeqCst);
    let ending = session
        .preflight(&url, &base, None, &rules)
        .expect("a verdict");
    clock.store(start + 3600 + 5, Ordering::SeqCst);
    let ended = session
        .preflight(&url, &base, None, &rules)
        .expect("a verdict");

    assert_eq!(fresh, Transition::Stay);
    assert_eq!(
        ending,
        Transition::Escalate(EscalateReason::ClearanceExpiring { remaining_secs: 60 })
    );
    assert_eq!(
        ended,
        Transition::Escalate(EscalateReason::ClearanceExpiring { remaining_secs: 0 })
    );
    let expiring: Vec<ScraperEvent> = sink
        .events()
        .into_iter()
        .filter(|event| matches!(event, ScraperEvent::ClearanceExpiring { .. }))
        .collect();
    assert_eq!(expiring.len(), 2);
}

#[tokio::test]
async fn a_preflight_for_a_url_that_cannot_be_requested_is_an_error() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    for url in [
        "not a url",
        "ftp://shop.test/",
        "https://alice:s3cret@shop.test/",
    ] {
        let error = session
            .preflight(url, &identity(), None, &Rules::default())
            .expect_err(url);
        assert!(matches!(error, HttpError::Url { .. }), "{url}: {error}");
        assert!(!format!("{error} {error:?}").contains("s3cret"));
    }
}

#[tokio::test]
async fn a_site_that_needs_a_browser_sends_the_session_there_before_the_request() {
    let site = Site::start(|_| Answer::ok("")).await;
    let clock = Arc::new(AtomicU64::new(now()));
    let sink = Arc::new(CollectingEventSink::new());
    let session = clocked(&site, &clock, &sink);
    let mut state = DomainState::new(HOST);
    state.preferred_mode = Some(Mode::Browser);

    let transition = session
        .preflight(
            &site.url(HOST, "/"),
            &identity(),
            Some(&state),
            &Rules::default(),
        )
        .expect("a verdict");
    let plain = session
        .preflight(&site.url(HOST, "/"), &identity(), None, &Rules::default())
        .expect("a verdict");

    assert_eq!(
        transition,
        Transition::Escalate(EscalateReason::SiteNeedsBrowser)
    );
    assert_eq!(plain, Transition::Stay);
}

#[tokio::test]
async fn a_clearance_the_session_just_earned_counts_for_the_preflight() {
    let site =
        Site::start(|_| Answer::ok("").cookie("cf_clearance=NEW; Path=/; Secure; Max-Age=30"))
            .await;
    let clock = Arc::new(AtomicU64::new(now()));
    let sink = Arc::new(CollectingEventSink::new());
    let session = clocked(&site, &clock, &sink);
    let url = site.url(HOST, "/");
    let rules = Rules::default();
    let base = identity();

    session.get(&url).await.expect("a reply");
    let transition = session
        .preflight(&url, &base, None, &rules)
        .expect("a verdict");
    let exported = session.export_identity(&base);

    assert!(matches!(
        transition,
        Transition::Escalate(EscalateReason::ClearanceExpiring { .. })
    ));
    assert_eq!(exported.clearances.len(), 1, "{exported:?}");
    assert!(base.clearances.is_empty());
}
