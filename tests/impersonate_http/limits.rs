//! Bounds: the size of a body after decompression, the time of a request, the pace per site, and
//! failures that must not leak what they were doing.

use std::io::Write as _;
use std::time::{Duration, Instant};

use seleniumbase_rs::impersonate::http_session::{HttpError, HttpOptions, HttpSession, Method};

use crate::site::{egress, fingerprint, identity, identity_of, measured_major, Answer, Site};

const HOST: &str = "shop.test";
const MIB: usize = 1024 * 1024;

/// `bytes` compressed with brotli, as a server sends it with `content-encoding: br`.
fn brotli(bytes: &[u8]) -> Vec<u8> {
    let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 5, 22);
    writer.write_all(bytes).expect("compresses");
    writer.into_inner()
}

fn limited(site: &Site, limit: usize) -> HttpSession {
    let options = site.options(&[HOST]).max_body_bytes(limit);
    HttpSession::from_identity(&identity(), None, options).expect("a session")
}

#[tokio::test]
async fn a_body_over_the_limit_fails_and_one_at_the_limit_does_not() {
    let site = Site::start(|seen| {
        let size = if seen.path() == "/exact" {
            MIB
        } else {
            MIB + 1
        };
        Answer::status(200).body(&vec![b'a'; size])
    })
    .await;
    let session = limited(&site, MIB);

    let exact = session
        .get(&site.url(HOST, "/exact"))
        .await
        .expect("at the limit");
    let error = session
        .get(&site.url(HOST, "/over"))
        .await
        .expect_err("over the limit");

    assert_eq!(exact.bytes().len(), MIB);
    assert!(
        matches!(error, HttpError::BodyTooLarge(limit) if limit == MIB),
        "{error}"
    );
}

#[tokio::test]
async fn a_head_reply_may_announce_a_body_it_does_not_send() {
    let site = Site::start(|_| Answer::status(200).body(&vec![b'a'; 2 * MIB])).await;
    let session = limited(&site, MIB);

    let reply = session
        .request(Method::HEAD, &site.url(HOST, "/"))
        .send()
        .await
        .expect("a reply");

    assert_eq!(reply.bytes().len(), 0);
    assert_eq!(reply.headers().get("content-length"), Some("2097152"));
}

#[tokio::test]
async fn a_compression_bomb_is_cut_by_its_size_after_decompression() {
    let bomb = brotli(&vec![0_u8; 16 * MIB]);
    assert!(
        bomb.len() < MIB,
        "the bomb is small on the wire: {}",
        bomb.len()
    );
    let site = Site::start(move |_| {
        Answer::status(200)
            .header("content-encoding", "br")
            .body(&bomb)
    })
    .await;
    let session = limited(&site, MIB);

    let error = session.get(&site.url(HOST, "/")).await.expect_err("a bomb");

    assert!(
        matches!(error, HttpError::BodyTooLarge(limit) if limit == MIB),
        "{error}"
    );
}

#[tokio::test]
async fn a_compressed_body_within_the_limit_is_decoded() {
    let packed = brotli(b"hello brotli");
    let site = Site::start(move |_| {
        Answer::status(200)
            .header("content-encoding", "br")
            .body(&packed)
    })
    .await;
    let session = site.session(&[HOST]);

    let reply = session.get(&site.url(HOST, "/")).await.expect("a reply");

    assert_eq!(reply.text().expect("text"), "hello brotli");
}

#[tokio::test]
async fn a_request_body_over_the_limit_is_refused_before_it_is_sent() {
    let site = Site::start(|_| Answer::ok("")).await;
    let options = site.options(&[HOST]).max_request_bytes(10);
    let session = HttpSession::from_identity(&identity(), None, options).expect("a session");

    let error = session
        .request(Method::POST, &site.url(HOST, "/"))
        .body(vec![0_u8; 11])
        .send()
        .await
        .expect_err("too large");

    assert!(matches!(error, HttpError::RequestTooLarge(10)), "{error}");
    assert!(site.seen().is_empty());
}

#[tokio::test]
async fn a_slow_server_times_out_with_the_url_redacted() {
    let site = Site::start(|_| Answer::ok("late").delay(Duration::from_secs(3))).await;
    let options = site.options(&[HOST]).timeout(Duration::from_millis(400));
    let session = HttpSession::from_identity(&identity(), None, options).expect("a session");

    let started = Instant::now();
    let error = session
        .get(&site.url(HOST, "/slow?token=hunter2&page=2"))
        .await
        .expect_err("a timeout");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(
        matches!(&error, HttpError::Request { kind, .. } if *kind == "timed out"),
        "{error}"
    );
    let shown = format!("{error} {error:?}");
    assert!(
        shown.contains("page=2") && !shown.contains("hunter2"),
        "{shown}"
    );
}

#[tokio::test]
async fn a_timeout_for_one_request_replaces_the_sessions() {
    let site = Site::start(|_| Answer::ok("late").delay(Duration::from_secs(3))).await;
    let session = site.session(&[HOST]);

    let error = session
        .request(Method::GET, &site.url(HOST, "/slow"))
        .timeout(Duration::from_millis(300))
        .send()
        .await
        .expect_err("a timeout");

    assert!(
        matches!(
            error,
            HttpError::Request {
                kind: "timed out",
                ..
            }
        ),
        "{error}"
    );
}

#[tokio::test]
async fn the_time_limit_covers_all_the_redirects_together() {
    let site = Site::start(|seen| match seen.path() {
        "/slow" => Answer::redirect(302, "/slower").delay(Duration::from_millis(300)),
        "/slower" => Answer::redirect(302, "/slowest").delay(Duration::from_millis(300)),
        _ => Answer::ok("done").delay(Duration::from_millis(300)),
    })
    .await;
    let options = site.options(&[HOST]).timeout(Duration::from_millis(700));
    let session = HttpSession::from_identity(&identity(), None, options).expect("a session");

    let error = session
        .get(&site.url(HOST, "/slow"))
        .await
        .expect_err("too slow in all");

    assert!(
        matches!(
            error,
            HttpError::Request {
                kind: "timed out",
                ..
            }
        ),
        "{error}"
    );
}

#[tokio::test]
async fn requests_to_one_site_are_paced_and_other_sites_are_not() {
    let site = Site::start(|_| Answer::ok("")).await;
    let options = site
        .options(&[HOST, "other.test"])
        .pacing(1, Duration::from_millis(700));
    let session = HttpSession::from_identity(&identity(), None, options).expect("a session");

    let started = Instant::now();
    session.get(&site.url(HOST, "/1")).await.expect("a reply");
    session
        .get(&site.url("other.test", "/1"))
        .await
        .expect("a reply");
    let apart = started.elapsed();
    session.get(&site.url(HOST, "/2")).await.expect("a reply");
    let together = started.elapsed();

    assert!(
        apart < Duration::from_millis(600),
        "two sites are paced apart: {apart:?}"
    );
    assert!(
        together >= Duration::from_millis(600),
        "the second request waited: {together:?}"
    );
}

#[tokio::test]
async fn every_hop_of_a_redirect_counts_for_the_pace() {
    let site = Site::start(|seen| match seen.path() {
        "/start" => Answer::redirect(302, "/end"),
        _ => Answer::ok(""),
    })
    .await;
    let options = site.options(&[HOST]).pacing(1, Duration::from_millis(500));
    let session = HttpSession::from_identity(&identity(), None, options).expect("a session");

    let started = Instant::now();
    session
        .get(&site.url(HOST, "/start"))
        .await
        .expect("a reply");

    assert!(
        started.elapsed() >= Duration::from_millis(400),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn a_server_the_client_does_not_trust_is_refused_and_nothing_is_read() {
    let site = Site::start(|_| Answer::ok("secret page")).await;
    let untrusted = HttpOptions::new().resolve(
        HOST,
        std::net::SocketAddr::from(([127, 0, 0, 1], site.port())),
    );
    let session = HttpSession::from_identity(&identity(), None, untrusted).expect("a session");

    let error = session
        .get(&site.url(HOST, "/"))
        .await
        .expect_err("untrusted");

    assert!(matches!(error, HttpError::Request { .. }), "{error}");
    assert!(site.seen().is_empty(), "no request reached the server");
}

#[tokio::test]
async fn a_refused_connection_names_the_failure_and_redacts_the_url() {
    let closed = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("binds");
    let port = closed.local_addr().expect("an address").port();
    drop(closed);
    let session =
        HttpSession::from_identity(&identity(), None, HttpOptions::new()).expect("a session");

    let error = session
        .get(&format!("https://127.0.0.1:{port}/?token=hunter2&page=2"))
        .await
        .expect_err("refused");

    assert!(matches!(&error, HttpError::Request { .. }), "{error}");
    let shown = format!("{error} {error:?}");
    assert!(
        shown.contains("page=2") && !shown.contains("hunter2"),
        "{shown}"
    );
}

#[tokio::test]
async fn a_proxy_that_cannot_be_reached_leaks_no_credentials() {
    let closed = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("binds");
    let port = closed.local_addr().expect("an address").port();
    drop(closed);
    let proxy = egress(&format!("http://alice:s3cret@127.0.0.1:{port}"));
    let major = measured_major();
    let base = identity_of(
        fingerprint(seleniumbase_rs::stealth::fingerprint::OsType::Macos, major),
        seleniumbase_rs::scrape::passport::identity::EmulationRef {
            kind: seleniumbase_rs::wire::BrowserKind::Chrome,
            major,
            os: seleniumbase_rs::stealth::fingerprint::OsType::Macos,
        },
        Some(proxy.key().clone()),
    );
    let session =
        HttpSession::from_identity(&base, Some(proxy), HttpOptions::new()).expect("a session");

    let error = session
        .get("https://shop.test/?token=hunter2")
        .await
        .expect_err("no proxy");

    let shown = format!("{error} {error:?} {session:?}");
    for secret in ["s3cret", "alice", "hunter2"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
    assert!(matches!(error, HttpError::Request { .. }), "{error}");
}
