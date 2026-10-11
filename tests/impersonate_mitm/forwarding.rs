//! What reaches the origin through the proxy, and what the proxy refuses to send.

use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use futures_util::StreamExt as _;
use seleniumbase_rs::impersonate::mitm::{MitmOptions, MitmProxy};
use seleniumbase_rs::impersonate::tls_server::ALPN_HTTP1;
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use seleniumbase_rs::wire::header_spec::RequestShape;
use seleniumbase_rs::wire::ja4::Ja4;
use tokio::net::TcpStream;

use crate::support::{
    browser_client, der, exchange, measured_user_agent, navigation, new_ca, profile, proxy_auth,
    start_mitm, tunnel, Recorded, RecordingOrigin, StreamOrigin, ALPN_H2, CHUNK, PAGE, QUIET,
    WITHIN,
};

/// The host every test site is reached by.
const ORIGIN: &str = "origin.test";
/// The host of the streaming site; its leaf names it.
const STREAM: &str = "stream.test";

/// A recording site speaking HTTP/2 and HTTP/1.1, as [`site_speaking`] makes it.
async fn site_behind_proxy(
    options: MitmOptions,
) -> (RecordingOrigin, MitmProxy, CollectingEventSink, Vec<u8>) {
    site_speaking(&[ALPN_H2, ALPN_HTTP1], options).await
}

/// A recording site offering `alpn`, a proxy that trusts it and resolves [`ORIGIN`] to it, the
/// proxy's sink, and the DER of the proxy's CA.
async fn site_speaking(
    alpn: &[&[u8]],
    options: MitmOptions,
) -> (RecordingOrigin, MitmProxy, CollectingEventSink, Vec<u8>) {
    let origin = RecordingOrigin::start(alpn).await;
    let ca = new_ca();
    let options = options
        .ca(Arc::clone(&ca))
        .trust_upstream_root(origin.root())
        .resolve(ORIGIN, origin.addr())
        .allow_private_destinations(true);
    let (mitm, sink) = start_mitm(None, options).await;
    (origin, mitm, sink, der(&ca))
}

#[tokio::test]
async fn a_forwarded_request_carries_the_measured_ja4_and_akamai_string() {
    let (mut origin, mitm, sink, ca) = site_behind_proxy(MitmOptions::default()).await;
    let port = origin.addr().port();
    let mut tls = tunnel(&mitm, ORIGIN, port, &ca).await;

    let request = navigation(ORIGIN, port, &measured_user_agent(), &[]);
    let (head, body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 200);
    assert_eq!(body, PAGE);
    let seen = origin
        .next(WITHIN)
        .await
        .expect("the origin saw the request");
    let profile = profile();
    assert_eq!(seen.alpn.as_deref(), Some(ALPN_H2));
    assert_eq!(seen.hello.sni.as_deref(), Some(ORIGIN));
    assert_eq!(Ja4::of(&seen.hello), profile.tls_spec().ja4(), "JA4");
    let preface = seen.h2();
    assert_eq!(
        preface.akamai().expect("an Akamai string"),
        profile.http2_spec().akamai(),
        "Akamai string"
    );
    assert_eq!(
        preface.headers_priority,
        profile.http2_spec().headers_priority
    );
    let authority = format!("{ORIGIN}:{port}");
    assert_eq!(preface.header(":authority"), Some(authority.as_str()));
    assert_eq!(preface.header(":path"), Some("/page?q=1"));
    assert!(sink.events().iter().any(|event| matches!(
        event,
        ScraperEvent::Request { status: 200, url, .. } if url.as_str().contains("/page")
    )));
    assert_eq!(mitm.stats().forwarded, 1);
}

/// A made-up credential the browser leaks into a `Proxy-Authorization` inside the tunnel.
const LEAKED_PAIR: &str = "user:not-a-real-password";

/// What the browser adds beyond a navigation, after the leaked `Proxy-Authorization`:
/// hop-by-hop and proxy fields, a field the `Connection` header lists, and one ordinary header.
const HOP_FIELDS: [&str; 6] = [
    "Proxy-Connection: keep-alive",
    "Keep-Alive: timeout=5",
    "TE: gzip",
    "Connection: X-Hop",
    "X-Hop: drop-me",
    "sec-ch-ua: \"Chromium\";v=\"155\"",
];

/// The header names the origin must see for that request, in order.
const FORWARDED_NAMES: [&str; 10] = [
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "upgrade-insecure-requests",
    "user-agent",
    "accept",
    "sec-fetch-site",
    "sec-fetch-dest",
    "accept-encoding",
    "accept-language",
    "priority",
];

#[tokio::test]
async fn hop_by_hop_and_proxy_headers_never_reach_the_origin_and_the_order_is_kept() {
    let (mut origin, mitm, _sink, ca) = site_behind_proxy(MitmOptions::default()).await;
    let port = origin.addr().port();
    let mut tls = tunnel(&mitm, ORIGIN, port, &ca).await;

    let leaked = STANDARD.encode(LEAKED_PAIR);
    let auth_line = format!("Proxy-Authorization: Basic {leaked}");
    let mut extra = vec![auth_line.as_str()];
    extra.extend(HOP_FIELDS);
    let request = navigation(ORIGIN, port, &measured_user_agent(), &extra);
    let (head, _body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 200);
    let seen = origin
        .next(WITHIN)
        .await
        .expect("the origin saw the request");
    let preface = seen.h2();
    let names: Vec<&str> = preface
        .regular_headers()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(
        names, FORWARDED_NAMES,
        "the browser's order, the hop-by-hop fields gone, the measured priority last"
    );
    let measured = profile();
    let priority = measured
        .header_spec()
        .priority_for("document")
        .expect("a measured document priority");
    assert_eq!(preface.header("priority"), Some(priority));
    assert!(preface
        .headers
        .iter()
        .all(|field| !field.value.contains(&leaked)));
}

/// The measured spelling of a lower-case header name, or the name itself.
fn measured_spelling(lower: &str) -> String {
    let profile = profile();
    let spec = profile.header_spec();
    spec.headers(RequestShape::Document)
        .iter()
        .chain(spec.headers(RequestShape::Fetch))
        .find(|(name, _)| name.eq_ignore_ascii_case(lower))
        .map_or_else(|| lower.to_owned(), |(name, _)| name.clone())
}

#[tokio::test]
async fn an_http1_origin_gets_the_measured_spelling_and_no_priority_once_known() {
    let (mut origin, mitm, _sink, ca) = site_speaking(&[ALPN_HTTP1], MitmOptions::default()).await;
    let port = origin.addr().port();
    let mut seen = Vec::new();
    for _ in 0..2 {
        let mut tls = tunnel(&mitm, ORIGIN, port, &ca).await;
        let request = navigation(ORIGIN, port, &measured_user_agent(), &[]);
        let (head, _body) = exchange(&mut tls, &request).await.expect("an answer");
        assert_eq!(head.status(), 200);
        seen.push(origin.next(WITHIN).await.expect("the origin saw it"));
    }

    let Recorded::Http1(second) = &seen[1].request else {
        panic!("the origin speaks HTTP/1.1 only");
    };
    let names: Vec<&str> = second
        .headers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let mut expected = vec!["Host".to_owned()];
    expected.extend(
        FORWARDED_NAMES[1..9]
            .iter()
            .map(|name| measured_spelling(name)),
    );
    assert_eq!(
        names, expected,
        "Host first, then the browser's order in measured spelling"
    );
    assert!(
        second.header("priority").is_none(),
        "priority is HTTP/2-only"
    );
}

#[tokio::test]
async fn an_origin_with_an_untrusted_certificate_gets_an_empty_502() {
    let mut origin = RecordingOrigin::start(&[ALPN_H2, ALPN_HTTP1]).await;
    let ca = new_ca();
    let options = MitmOptions::default()
        .ca(Arc::clone(&ca))
        .trust_upstream_root(der(&new_ca()))
        .resolve(ORIGIN, origin.addr());
    let (mitm, sink) = start_mitm(None, options).await;
    let port = origin.addr().port();
    let mut tls = tunnel(&mitm, ORIGIN, port, &der(&ca)).await;

    let request = navigation(ORIGIN, port, &measured_user_agent(), &[]);
    let (head, body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 502);
    assert_eq!(head.content_length(), Some(0));
    assert!(body.is_empty());
    assert!(
        origin.next(QUIET).await.is_none(),
        "the origin served a request"
    );
    assert_eq!(mitm.stats().upstream_errors, 1);
    assert!(sink
        .events()
        .iter()
        .any(|event| matches!(event, ScraperEvent::Request { status: 502, .. })));
}

/// Sends `request` through a fresh tunnel and returns the status; the origin must see nothing.
async fn refused_status(request: impl Fn(u16) -> String, options: MitmOptions) -> u16 {
    let (mut origin, mitm, _sink, ca) = site_behind_proxy(options).await;
    let port = origin.addr().port();
    let mut tls = tunnel(&mitm, ORIGIN, port, &ca).await;
    let (head, body) = exchange(&mut tls, &request(port)).await.expect("an answer");
    assert!(body.is_empty(), "a refusal carries no detail");
    assert!(
        origin.next(QUIET).await.is_none(),
        "the request reached the origin"
    );
    assert_eq!(mitm.stats().refused, 1);
    head.status()
}

#[tokio::test]
async fn a_request_claiming_another_chrome_major_is_refused() {
    let agent = "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 Chrome/999.0.0.0 Safari/537.36";
    let status = refused_status(
        |port| navigation(ORIGIN, port, agent, &[]),
        MitmOptions::default(),
    )
    .await;
    assert_eq!(status, 502);
}

#[tokio::test]
async fn a_websocket_upgrade_is_refused() {
    let status = refused_status(
        |port| {
            navigation(
                ORIGIN,
                port,
                &measured_user_agent(),
                &["Upgrade: websocket", "Connection: Upgrade"],
            )
        },
        MitmOptions::default(),
    )
    .await;
    assert_eq!(status, 502);
}

#[tokio::test]
async fn a_host_header_naming_another_host_is_refused() {
    let status = refused_status(
        |port| navigation("other.test", port, &measured_user_agent(), &[]),
        MitmOptions::default(),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn an_announced_body_over_the_limit_is_refused() {
    let status = refused_status(
        |port| {
            format!(
                "POST /upload HTTP/1.1\r\nHost: {ORIGIN}:{port}\r\nContent-Length: 4096\r\n\r\n"
            )
        },
        MitmOptions::default().max_body_bytes(1024),
    )
    .await;
    assert_eq!(status, 413);
}

/// A streaming site, a proxy that trusts it, and a browser client for the proxy.
async fn stream_behind_proxy(
    total: usize,
    announce: bool,
    options: MitmOptions,
) -> (StreamOrigin, MitmProxy, wreq::Client) {
    let origin = StreamOrigin::start(true, total, announce).await;
    let ca = new_ca();
    let options = options
        .ca(Arc::clone(&ca))
        .trust_upstream_root(origin.root())
        .resolve(STREAM, origin.addr())
        .allow_private_destinations(true);
    let (mitm, _sink) = start_mitm(None, options).await;
    let client = browser_client(&mitm, &der(&ca));
    (origin, mitm, client)
}

#[tokio::test]
async fn a_64_mib_body_streams_through_without_being_held_back() {
    const TOTAL: usize = 64 * 1024 * 1024;
    let (origin, _mitm, client) = stream_behind_proxy(TOTAL, true, MitmOptions::default()).await;
    let url = format!("https://{STREAM}:{}/", origin.addr().port());

    let response = client.get(url).send().await.expect("an answer");
    assert_eq!(response.status(), 200);
    assert_eq!(response.content_length(), Some(TOTAL as u64));
    let mut chunks = response.bytes_stream();
    let mut received = 0;
    while received < CHUNK {
        let chunk = tokio::time::timeout(WITHIN, chunks.next())
            .await
            .expect("the first chunk arrives while the origin holds the rest back")
            .expect("the body goes on")
            .expect("bytes");
        received += chunk.len();
    }
    origin.release();
    while let Some(chunk) = chunks.next().await {
        received += chunk.expect("bytes").len();
    }
    assert_eq!(received, TOTAL);
}

/// A body four times the limit.
const OVERSIZED: usize = 4 * 1024 * 1024;
/// The body limit of the limit tests.
const LIMIT: usize = 1024 * 1024;

#[tokio::test]
async fn an_answer_announcing_more_than_the_limit_gets_an_empty_502() {
    let options = MitmOptions::default().max_body_bytes(LIMIT);
    let (origin, mitm, client) = stream_behind_proxy(OVERSIZED, true, options).await;
    origin.release();
    let url = format!("https://{STREAM}:{}/", origin.addr().port());

    let response = client.get(url).send().await.expect("an answer");

    assert_eq!(response.status(), 502);
    assert_eq!(response.bytes().await.expect("a body").len(), 0);
    assert_eq!(mitm.stats().upstream_errors, 1);
}

#[tokio::test]
async fn an_unannounced_body_past_the_limit_is_cut() {
    let options = MitmOptions::default().max_body_bytes(LIMIT);
    let (origin, _mitm, client) = stream_behind_proxy(OVERSIZED, false, options).await;
    origin.release();
    let url = format!("https://{STREAM}:{}/", origin.addr().port());

    let response = client.get(url).send().await.expect("an answer");
    assert_eq!(response.status(), 200);
    assert_eq!(response.content_length(), None, "the answer is chunked");
    let mut chunks = response.bytes_stream();
    let mut received = 0;
    let mut failed = false;
    while let Some(chunk) = chunks.next().await {
        match chunk {
            Ok(bytes) => received += bytes.len(),
            Err(_cut) => {
                failed = true;
                break;
            }
        }
    }
    assert!(failed, "the body ended cleanly");
    assert!(
        received <= LIMIT,
        "{received} bytes passed a {LIMIT}-byte limit"
    );
}

#[tokio::test]
async fn absolute_form_http_requests_are_forwarded_after_the_token_check() {
    let origin = StreamOrigin::start(false, 5, true).await;
    let options = MitmOptions::default()
        .resolve("plain.test", origin.addr())
        .allow_private_destinations(true);
    let (mitm, _sink) = start_mitm(None, options).await;
    let port = origin.addr().port();
    let request = |auth: Option<String>| {
        let auth = auth
            .map(|value| format!("Proxy-Authorization: {value}\r\n"))
            .unwrap_or_default();
        format!("GET http://plain.test:{port}/ HTTP/1.1\r\nHost: plain.test:{port}\r\n{auth}\r\n")
    };
    let mut tcp = TcpStream::connect(mitm.local_addr())
        .await
        .expect("reaches the proxy");

    let (head, _body) = exchange(&mut tcp, &request(None)).await.expect("an answer");
    assert_eq!(head.status(), 407);
    assert_eq!(origin.requests(), 0);

    let (head, body) = exchange(&mut tcp, &request(Some(proxy_auth(&mitm))))
        .await
        .expect("an answer");
    assert_eq!(head.status(), 200);
    assert_eq!(body, b"xxxxx");
    assert_eq!(origin.requests(), 1);
}
