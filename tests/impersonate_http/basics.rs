//! A plain request: the identity it carries, the headers a caller may and may not set, a body, the
//! events, and the protocol the origin settles on.

use std::sync::Arc;

use seleniumbase_rs::impersonate::http_session::{
    HttpError, HttpOptions, HttpSession, Method, ReplyProtocol,
};
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use seleniumbase_rs::wire::header_spec::RequestShape;

use crate::site::{identity, Answer, Site};

const HOST: &str = "shop.test";

#[tokio::test]
async fn a_get_returns_the_status_the_headers_and_the_body() {
    let site = Site::start(|_| Answer::ok("hello").header("x-n", "1")).await;
    let session = site.session(&[HOST]);

    let reply = session
        .get(&site.url(HOST, "/a?page=2"))
        .await
        .expect("a reply");

    assert_eq!(reply.status(), 200);
    assert!(reply.is_success());
    assert_eq!(reply.text().expect("text"), "hello");
    assert_eq!(reply.headers().get("X-N"), Some("1"));
    assert_eq!(reply.protocol(), ReplyProtocol::Http1);
    assert!(reply.redirects().is_empty());
    assert_eq!(reply.url().path(), "/a");
    let seen = site.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        (seen[0].method.as_str(), seen[0].target.as_str()),
        ("GET", "/a?page=2")
    );
    assert_eq!(seen[0].host, HOST);
}

#[tokio::test]
async fn the_request_carries_the_identity_of_the_profile_and_no_cookie() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    session.get(&site.url(HOST, "/")).await.expect("a reply");

    let seen = &site.seen()[0];
    let profile = session.profile();
    assert_eq!(seen.header("user-agent"), Some(profile.user_agent()));
    assert!(seen
        .header("sec-ch-ua")
        .is_some_and(|hints| hints.contains("Chrome")));
    assert_eq!(seen.header("accept-language"), profile.accept_language());
    assert!(seen.cookie().is_none());
    let document = profile
        .headers_for(
            RequestShape::Document,
            None,
            seleniumbase_rs::impersonate::client::WireProtocol::Http1,
        )
        .expect("the headers");
    for (name, _) in document {
        assert!(
            seen.header(&name).is_some(),
            "the profile lists `{name}` but the request lacks it"
        );
    }
}

#[tokio::test]
async fn a_caller_header_replaces_the_profiles_value_or_is_added() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    session
        .request(Method::GET, &site.url(HOST, "/"))
        .header("accept", "application/json")
        .header("X-Custom", "1")
        .send()
        .await
        .expect("a reply");

    let seen = &site.seen()[0];
    assert_eq!(seen.header("accept"), Some("application/json"));
    assert_eq!(seen.header("x-custom"), Some("1"));
    assert_eq!(
        seen.headers
            .iter()
            .filter(|(name, _)| name == "accept")
            .count(),
        1,
        "replaced, not doubled"
    );
}

#[tokio::test]
async fn a_header_the_profile_owns_is_refused_before_anything_is_sent() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    for name in [
        "User-Agent",
        "Sec-CH-UA",
        "sec-ch-ua-platform",
        "Accept-Language",
        "Cookie",
        "Host",
    ] {
        let error = session
            .request(Method::GET, &site.url(HOST, "/"))
            .header(name, "x")
            .send()
            .await
            .expect_err(name);
        assert!(
            matches!(error, HttpError::ReservedHeader(_)),
            "{name}: {error}"
        );
    }

    assert!(site.seen().is_empty(), "nothing was sent");
}

#[tokio::test]
async fn a_header_with_a_line_break_is_refused_and_its_value_is_not_echoed() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    let error = session
        .request(Method::GET, &site.url(HOST, "/"))
        .header("x-a", "ok\r\nx-injected: s3cret")
        .send()
        .await
        .expect_err("refused");

    assert!(matches!(error, HttpError::InvalidHeader(_)));
    assert!(!format!("{error} {error:?}").contains("s3cret"));
    assert!(site.seen().is_empty());
}

#[tokio::test]
async fn a_post_sends_its_body_with_the_headers_of_a_script_request() {
    let site = Site::start(|_| Answer::ok("{}")).await;
    let session = site.session(&[HOST]);

    session
        .request(Method::POST, &site.url(HOST, "/api"))
        .json(&serde_json::json!({ "q": "shoes" }))
        .send()
        .await
        .expect("a reply");

    let seen = &site.seen()[0];
    assert_eq!(seen.method, "POST");
    assert_eq!(seen.header("content-type"), Some("application/json"));
    assert_eq!(seen.body, br#"{"q":"shoes"}"#);
    let fetch = session
        .profile()
        .headers_for(
            RequestShape::Fetch,
            None,
            seleniumbase_rs::impersonate::client::WireProtocol::Http1,
        )
        .expect("the headers");
    let (_, dest) = fetch
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("sec-fetch-dest"))
        .expect("the fetch list names its destination");
    assert_eq!(seen.header("sec-fetch-dest"), Some(dest.as_str()));
}

#[tokio::test]
async fn a_form_is_urlencoded() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    session
        .request(Method::POST, &site.url(HOST, "/login"))
        .form(&[("user", "a b"), ("pw", "x&y")])
        .send()
        .await
        .expect("a reply");

    let seen = &site.seen()[0];
    assert_eq!(
        seen.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(seen.body, b"user=a+b&pw=x%26y");
}

#[tokio::test]
async fn an_explicit_shape_changes_the_headers() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    session
        .request(Method::GET, &site.url(HOST, "/"))
        .shape(RequestShape::Fetch)
        .send()
        .await
        .expect("a reply");
    session.get(&site.url(HOST, "/")).await.expect("a reply");

    let seen = site.seen();
    assert_ne!(
        seen[0].header("sec-fetch-mode"),
        seen[1].header("sec-fetch-mode"),
        "a script request and a navigation differ"
    );
}

#[tokio::test]
async fn the_request_event_is_emitted_with_the_url_redacted() {
    let site = Site::start(|_| Answer::ok("")).await;
    let sink = Arc::new(CollectingEventSink::new());
    let options = site.options(&[HOST]).event_sink(sink.clone());
    let session = HttpSession::from_identity(&identity(), None, options).expect("a session");

    session
        .get(&site.url(HOST, "/p?token=hunter2&page=2"))
        .await
        .expect("a reply");

    let events = sink.events();
    let request = events
        .iter()
        .find(|event| matches!(event, ScraperEvent::Request { .. }))
        .expect("a request event");
    let shown = format!("{request:?}");
    assert!(
        shown.contains("page=2") && !shown.contains("hunter2"),
        "{shown}"
    );
    assert!(matches!(request, ScraperEvent::Request { status: 200, .. }));
}

#[tokio::test]
async fn the_protocol_an_origin_settled_on_is_remembered() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    session.get(&site.url(HOST, "/")).await.expect("a reply");
    session.get(&site.url(HOST, "/")).await.expect("a reply");

    let seen = site.seen();
    assert!(
        seen[1].header("priority").is_none(),
        "HTTP/1.1 has no priority header once the origin is known to speak it"
    );
}

#[tokio::test]
async fn a_url_with_credentials_or_another_scheme_is_refused_without_echoing_it() {
    let site = Site::start(|_| Answer::ok("")).await;
    let session = site.session(&[HOST]);

    for url in [
        "https://alice:s3cret@shop.test/",
        "ftp://shop.test/",
        "file:///etc/passwd",
        "not a url",
    ] {
        let error = session.get(url).await.expect_err(url);
        assert!(matches!(error, HttpError::Url { .. }), "{url}: {error}");
        let shown = format!("{error} {error:?}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
    }
    for method in ["CONNECT", "TRACE"] {
        let method = Method::from_bytes(method.as_bytes()).expect("a method");
        let error = session
            .request(method, &site.url(HOST, "/"))
            .send()
            .await
            .expect_err("forbidden");
        assert!(matches!(error, HttpError::ForbiddenMethod(_)));
    }
    assert!(site.seen().is_empty());
}

#[tokio::test]
async fn a_session_clone_shares_the_cookies() {
    let site = Site::start(|seen| {
        if seen.path() == "/set" {
            Answer::ok("").cookie("sid=1")
        } else {
            Answer::ok("")
        }
    })
    .await;
    let session = site.session(&[HOST]);
    let clone = session.clone();

    session.get(&site.url(HOST, "/set")).await.expect("a reply");
    clone.get(&site.url(HOST, "/after")).await.expect("a reply");

    assert_eq!(site.seen()[1].cookie(), Some("sid=1"));
    assert_eq!(clone.cookie_count(), 1);
}

#[tokio::test]
async fn a_plain_http_url_is_requested_with_the_headers_of_http1() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("binds");
    let port = listener.local_addr().expect("an address").port();
    let server = tokio::spawn(async move {
        let (mut stream, _peer) = listener.accept().await.expect("a client");
        let mut head = Vec::new();
        let mut buf = [0_u8; 1024];
        while !head.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buf).await.expect("reads");
            assert!(read > 0, "the client closed early");
            head.extend_from_slice(&buf[..read]);
        }
        let answer = b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok";
        stream.write_all(answer).await.expect("answers");
        String::from_utf8_lossy(&head).to_ascii_lowercase()
    });
    let session =
        HttpSession::from_identity(&identity(), None, HttpOptions::new()).expect("a session");

    let reply = session
        .get(&format!("http://127.0.0.1:{port}/plain"))
        .await
        .expect("a reply");
    let head = server.await.expect("the server task");

    assert_eq!(reply.text().expect("text"), "ok");
    assert_eq!(reply.protocol(), ReplyProtocol::Http1);
    assert!(
        head.starts_with(&format!(
            "get /plain http/1.1\r\nhost: 127.0.0.1:{port}\r\n"
        )),
        "{head}"
    );
    assert!(
        !head.contains("\r\npriority:"),
        "plain HTTP has no priority header: {head}"
    );
}

#[test]
fn the_session_its_replies_and_its_futures_can_cross_threads() {
    fn send_sync<T: Send + Sync>() {}
    fn is_send<T: Send>(_: &T) {}
    send_sync::<HttpSession>();
    send_sync::<HttpOptions>();
    send_sync::<HttpError>();
    send_sync::<seleniumbase_rs::impersonate::http_session::HttpReply>();
    let session =
        HttpSession::from_identity(&identity(), None, HttpOptions::new()).expect("a session");
    let future = session.get("https://shop.test/");
    is_send(&future);
}

#[test]
fn bad_options_are_refused_before_a_session_exists() {
    let options = HttpOptions::new().max_redirects(1000);
    let error = HttpSession::from_identity(&identity(), None, options).expect_err("refused");
    assert!(matches!(error, HttpError::Options(_)));
}
