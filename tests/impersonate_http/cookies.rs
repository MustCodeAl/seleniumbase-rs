//! Cookies: what the jar refuses, what it sends back, expiry on the injected clock, and the
//! identity's cookies going in and coming out.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use seleniumbase_rs::impersonate::http_session::{HttpError, HttpSession, Method};
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use url::Url;

use crate::site::{identity, now, Answer, Site};

const HOST: &str = "shop.test";

#[tokio::test]
async fn a_cookie_for_a_public_suffix_is_refused_and_a_host_cookie_is_kept() {
    let site = Site::start(|seen| match seen.path() {
        "/set" => Answer::ok("")
            .cookie("super=1; Domain=co.uk")
            .cookie("host=2"),
        _ => Answer::ok(""),
    })
    .await;
    let session = site.session(&["shop.co.uk", "other.co.uk"]);

    session
        .get(&site.url("shop.co.uk", "/set"))
        .await
        .expect("a reply");
    session
        .get(&site.url("shop.co.uk", "/again"))
        .await
        .expect("a reply");
    session
        .get(&site.url("other.co.uk", "/other"))
        .await
        .expect("a reply");

    let seen = site.seen();
    assert_eq!(
        seen[1].cookie(),
        Some("host=2"),
        "only the host cookie came back"
    );
    assert_eq!(
        seen[2].cookie(),
        None,
        "the supercookie was not stored for the other site"
    );
    assert_eq!(session.cookie_count(), 1);
}

#[tokio::test]
async fn a_cookie_for_a_hosting_suffix_is_refused() {
    let site = Site::start(|_| Answer::ok("").cookie("x=1; Domain=github.io")).await;
    let session = site.session(&["alice.github.io", "bob.github.io"]);

    session
        .get(&site.url("alice.github.io", "/"))
        .await
        .expect("a reply");
    session
        .get(&site.url("bob.github.io", "/"))
        .await
        .expect("a reply");

    assert!(site.seen()[1].cookie().is_none());
    assert_eq!(session.cookie_count(), 0);
}

#[tokio::test]
async fn a_cookie_for_another_host_is_refused() {
    let site = Site::start(|seen| match seen.host.as_str() {
        "evil.test" => Answer::ok("").cookie("x=1; Domain=victim.test"),
        _ => Answer::ok(""),
    })
    .await;
    let session = site.session(&["evil.test", "victim.test"]);

    session
        .get(&site.url("evil.test", "/"))
        .await
        .expect("a reply");
    session
        .get(&site.url("victim.test", "/"))
        .await
        .expect("a reply");

    assert!(site.seen()[1].cookie().is_none());
    assert_eq!(session.cookie_count(), 0);
}

#[tokio::test]
async fn a_cookie_expires_on_the_clock_the_session_reads() {
    let clock = Arc::new(AtomicU64::new(now()));
    let site = Site::start(|seen| match seen.path() {
        "/set" => Answer::ok("")
            .cookie("short=1; Max-Age=60")
            .cookie("session=2"),
        _ => Answer::ok(""),
    })
    .await;
    let reader = Arc::clone(&clock);
    let options = site
        .options(&[HOST])
        .clock(Arc::new(move || reader.load(Ordering::SeqCst)));
    let session = HttpSession::from_identity(&identity(), None, options).expect("a session");

    session.get(&site.url(HOST, "/set")).await.expect("a reply");
    session
        .get(&site.url(HOST, "/soon"))
        .await
        .expect("a reply");
    clock.fetch_add(61, Ordering::SeqCst);
    session
        .get(&site.url(HOST, "/later"))
        .await
        .expect("a reply");

    let seen = site.seen();
    assert_eq!(seen[1].cookie(), Some("short=1; session=2"));
    assert_eq!(
        seen[2].cookie(),
        Some("session=2"),
        "the short cookie expired"
    );
}

#[tokio::test]
async fn a_cookie_removed_by_the_server_is_no_longer_sent() {
    let site = Site::start(|seen| match seen.path() {
        "/set" => Answer::ok("").cookie("gone=1"),
        "/clear" => Answer::ok("").cookie("gone=; Max-Age=0"),
        _ => Answer::ok(""),
    })
    .await;
    let session = site.session(&[HOST]);

    session.get(&site.url(HOST, "/set")).await.expect("a reply");
    session
        .get(&site.url(HOST, "/clear"))
        .await
        .expect("a reply");
    session
        .get(&site.url(HOST, "/after"))
        .await
        .expect("a reply");

    let seen = site.seen();
    assert_eq!(seen[1].cookie(), Some("gone=1"));
    assert_eq!(seen[2].cookie(), None);
}

#[tokio::test]
async fn the_cookies_of_the_identity_are_sent_and_the_session_exports_the_new_ones() {
    let site = Site::start(|seen| match seen.path() {
        "/login" => Answer::ok("").cookie("session=NEW; Max-Age=3600"),
        _ => Answer::ok(""),
    })
    .await;
    let mut base = identity();
    let url = Url::parse(&site.url(HOST, "/")).expect("a url");
    base.cookies.store(&url, "carried=OLD; Max-Age=3600", now());
    let sink = Arc::new(CollectingEventSink::new());
    let options = site.options(&[HOST]).event_sink(sink.clone());
    let session = HttpSession::from_identity(&base, None, options).expect("a session");

    session
        .get(&site.url(HOST, "/login"))
        .await
        .expect("a reply");
    let exported = session.export_identity(&base);

    assert_eq!(site.seen()[0].cookie(), Some("carried=OLD"));
    assert_eq!(exported.cookies.len(), 2);
    assert_eq!(base.cookies.len(), 1, "the base identity is not changed");
    let events = sink.events();
    assert!(events.contains(&ScraperEvent::IdentityRestored { cookies: 1 }));
    assert!(events.contains(&ScraperEvent::IdentityExported { cookies: 2 }));
    let session_again =
        HttpSession::from_identity(&exported, None, site.options(&[HOST])).expect("a session");
    session_again
        .get(&site.url(HOST, "/check"))
        .await
        .expect("a reply");
    let sent = site.seen()[1]
        .cookie()
        .expect("cookies were sent")
        .to_owned();
    assert!(
        sent.contains("carried=OLD") && sent.contains("session=NEW"),
        "the exported identity carries both: {sent}"
    );
}

#[tokio::test]
async fn no_debug_view_shows_a_cookie_value() {
    let site = Site::start(|_| Answer::ok("body").cookie("sid=TOPSECRET; Max-Age=3600")).await;
    let session = site.session(&[HOST]);

    let reply = session.get(&site.url(HOST, "/")).await.expect("a reply");
    let exported = session.export_identity(&identity());

    for shown in [
        format!("{session:?}"),
        format!("{reply:?}"),
        format!("{:?}", reply.headers()),
        format!("{exported:?}"),
        format!("{:?}", session.request(Method::GET, &site.url(HOST, "/"))),
    ] {
        assert!(!shown.contains("TOPSECRET"), "{shown}");
    }
}

#[tokio::test]
async fn a_cookie_header_set_by_the_caller_is_refused_so_the_jar_decides() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    let error = session
        .request(Method::GET, &site.url(HOST, "/"))
        .header("Cookie", "sid=stolen")
        .send()
        .await
        .expect_err("refused");

    assert!(matches!(error, HttpError::ReservedHeader(_)));
    assert!(site.seen().is_empty());
}

#[tokio::test]
async fn a_secure_cookie_set_over_https_is_sent_back_over_https() {
    let site = Site::start(|seen| match seen.path() {
        "/set" => Answer::ok("")
            .cookie("__Host-id=1; Secure; Path=/")
            .cookie("__Secure-t=2; Secure"),
        _ => Answer::ok(""),
    })
    .await;
    let session = site.session(&[HOST]);

    session.get(&site.url(HOST, "/set")).await.expect("a reply");
    session.get(&site.url(HOST, "/x")).await.expect("a reply");

    assert_eq!(site.seen()[1].cookie(), Some("__Host-id=1; __Secure-t=2"));
}
