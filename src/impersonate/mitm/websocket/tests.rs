//! The WebSocket rules on their own: what counts as a handshake, the header rewrite both ways,
//! and the byte copy with its bounds, over in-memory pipes.

use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Version};
use tokio::io::{duplex, AsyncReadExt as _, AsyncWriteExt as _, DuplexStream};
use tokio::sync::watch;

use super::{
    ask, handshake_headers, pump, switched_to_websocket, switching_protocols, Ask, Bounds, Ended,
};
use crate::impersonate::mitm::MitmError;

/// How long a test waits for anything in memory.
const WITHIN: Duration = Duration::from_secs(5);
/// The idle limit of the idle test.
const SHORT_IDLE: Duration = Duration::from_millis(200);
/// A pipe large enough for every test write.
const PIPE: usize = 64 * 1024;

/// A header map in the order given.
fn headers(list: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in list {
        map.append(
            HeaderName::from_bytes(name.as_bytes()).expect("a header name"),
            HeaderValue::from_str(value).expect("a header value"),
        );
    }
    map
}

/// The headers of a handshake as Chrome writes them, plus `extra` at the end.
fn handshake(extra: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut list = vec![
        ("host", "ws.test"),
        ("connection", "Upgrade"),
        ("pragma", "no-cache"),
        ("cache-control", "no-cache"),
        ("user-agent", "Mozilla/5.0 Chrome/155.0.0.0"),
        ("upgrade", "websocket"),
        ("origin", "https://ws.test"),
        ("sec-websocket-version", "13"),
        ("accept-encoding", "gzip, deflate, br, zstd"),
        ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
        (
            "sec-websocket-extensions",
            "permessage-deflate; client_max_window_bits",
        ),
    ];
    list.extend_from_slice(extra);
    headers(&list)
}

/// The status of a refusal, or `None` when the request was taken.
fn refused(method: &Method, version: Version, map: &HeaderMap) -> Option<(StatusCode, MitmError)> {
    ask(method, version, map).err()
}

#[test]
fn an_ordinary_request_asks_for_nothing() {
    let map = headers(&[("host", "a.test"), ("connection", "keep-alive")]);
    assert_eq!(
        ask(&Method::GET, Version::HTTP_11, &map).ok(),
        Some(Ask::Plain)
    );
    assert_eq!(
        ask(&Method::POST, Version::HTTP_11, &HeaderMap::new()).ok(),
        Some(Ask::Plain)
    );
}

#[test]
fn chromes_handshake_is_a_websocket() {
    assert_eq!(
        ask(&Method::GET, Version::HTTP_11, &handshake(&[])).ok(),
        Some(Ask::WebSocket)
    );
}

#[test]
fn the_upgrade_tokens_are_read_without_regard_to_case_and_in_a_list() {
    let map = headers(&[
        ("connection", "keep-alive, UPGRADE"),
        ("upgrade", " WebSocket "),
        ("content-length", "0"),
    ]);
    assert_eq!(
        ask(&Method::GET, Version::HTTP_11, &map).ok(),
        Some(Ask::WebSocket)
    );
}

#[test]
fn half_an_upgrade_is_malformed() {
    let cases = [
        headers(&[("connection", "Upgrade")]),
        headers(&[("connection", "keep-alive, upgrade")]),
        headers(&[("upgrade", "websocket")]),
        headers(&[("upgrade", "websocket"), ("connection", "keep-alive")]),
    ];
    for map in cases {
        let refusal = refused(&Method::GET, Version::HTTP_11, &map);
        assert!(
            matches!(
                refusal,
                Some((StatusCode::BAD_REQUEST, MitmError::MalformedUpgrade(_)))
            ),
            "{map:?}: {refusal:?}"
        );
    }
}

#[test]
fn an_upgrade_to_anything_but_websocket_is_not_intercepted() {
    let cases = [
        headers(&[("connection", "Upgrade"), ("upgrade", "h2c")]),
        headers(&[("connection", "Upgrade"), ("upgrade", "TLS/1.2")]),
        headers(&[("connection", "Upgrade"), ("upgrade", "websocket, h2c")]),
        headers(&[("connection", "Upgrade"), ("upgrade", "websocket/13")]),
        headers(&[
            ("connection", "Upgrade"),
            ("upgrade", "websocket"),
            ("upgrade", "websocket"),
        ]),
        headers(&[("connection", "Upgrade"), ("upgrade", "")]),
        headers(&[("upgrade", "h2c")]),
    ];
    for map in cases {
        let refusal = refused(&Method::GET, Version::HTTP_11, &map);
        assert!(
            matches!(
                refusal,
                Some((StatusCode::BAD_GATEWAY, MitmError::NotIntercepted(_)))
            ),
            "{map:?}: {refusal:?}"
        );
    }
}

#[test]
fn a_handshake_that_is_not_a_bodiless_http11_get_is_malformed() {
    let cases = [
        (Method::POST, Version::HTTP_11, handshake(&[])),
        (Method::CONNECT, Version::HTTP_11, handshake(&[])),
        (Method::GET, Version::HTTP_10, handshake(&[])),
        (
            Method::GET,
            Version::HTTP_11,
            handshake(&[("content-length", "5")]),
        ),
        (
            Method::GET,
            Version::HTTP_11,
            handshake(&[("content-length", "nope")]),
        ),
        (
            Method::GET,
            Version::HTTP_11,
            handshake(&[("transfer-encoding", "chunked")]),
        ),
    ];
    for (method, version, map) in cases {
        let refusal = refused(&method, version, &map);
        assert!(
            matches!(
                refusal,
                Some((StatusCode::BAD_REQUEST, MitmError::MalformedUpgrade(_)))
            ),
            "{method} {version:?}: {refusal:?}"
        );
    }
}

#[test]
fn the_handshake_keeps_the_browsers_order_and_only_the_upgrade_pair_of_hop_fields() {
    let map = headers(&[
        ("host", "ws.test"),
        ("connection", "keep-alive, Upgrade, X-Hop"),
        ("proxy-authorization", "Basic c2I6dG9rZW4="),
        ("pragma", "no-cache"),
        ("x-hop", "drop-me"),
        ("keep-alive", "timeout=5"),
        ("proxy-connection", "keep-alive"),
        ("te", "gzip"),
        ("upgrade", "WebSocket"),
        ("sec-websocket-version", "13"),
        ("cookie", "session=1"),
        ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
    ]);
    let kept = handshake_headers(&map);
    let pairs: Vec<(&str, &str)> = kept
        .iter()
        .map(|(name, value)| (name.as_str(), value.to_str().expect("text")))
        .collect();
    assert_eq!(
        pairs,
        [
            ("connection", "Upgrade"),
            ("pragma", "no-cache"),
            ("upgrade", "websocket"),
            ("sec-websocket-version", "13"),
            ("cookie", "session=1"),
            ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
        ]
    );
}

#[test]
fn a_repeated_connection_field_yields_one_connection_upgrade() {
    let map = headers(&[
        ("connection", "Upgrade"),
        ("connection", "keep-alive"),
        ("upgrade", "websocket"),
    ]);
    let kept = handshake_headers(&map);
    let names: Vec<&str> = kept.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["connection", "upgrade"]);
}

#[test]
fn an_upstream_101_must_switch_to_websocket() {
    let good = [
        headers(&[("upgrade", "websocket"), ("connection", "Upgrade")]),
        headers(&[
            ("Upgrade", "WebSocket"),
            ("connection", "keep-alive, upgrade"),
        ]),
    ];
    for map in good {
        assert!(switched_to_websocket(&map), "{map:?}");
    }
    let bad = [
        HeaderMap::new(),
        headers(&[("upgrade", "websocket")]),
        headers(&[("connection", "Upgrade")]),
        headers(&[("upgrade", "h2c"), ("connection", "Upgrade")]),
        headers(&[
            ("upgrade", "websocket"),
            ("upgrade", "h2c"),
            ("connection", "Upgrade"),
        ]),
    ];
    for map in bad {
        assert!(!switched_to_websocket(&map), "{map:?}");
    }
}

#[test]
fn the_browser_gets_the_upstreams_101_without_hop_fields_or_a_length() {
    let upstream = headers(&[
        ("upgrade", "websocket"),
        ("connection", "Upgrade, X-Private"),
        ("x-private", "1"),
        ("keep-alive", "timeout=5"),
        ("content-length", "0"),
        ("sec-websocket-accept", "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
        (
            "sec-websocket-extensions",
            "permessage-deflate; client_max_window_bits=15",
        ),
        ("set-cookie", "a=1"),
    ]);
    let response = switching_protocols(&upstream);
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    assert_eq!(response.version(), Version::HTTP_11);
    let headers = response.headers();
    assert_eq!(
        headers.get("sec-websocket-accept"),
        Some(&HeaderValue::from_static("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="))
    );
    assert!(headers.contains_key("sec-websocket-extensions"));
    assert!(headers.contains_key("set-cookie"));
    assert_eq!(
        headers.get("connection"),
        Some(&HeaderValue::from_static("Upgrade"))
    );
    assert_eq!(
        headers.get("upgrade"),
        Some(&HeaderValue::from_static("websocket"))
    );
    for gone in ["x-private", "keep-alive", "content-length"] {
        assert!(!headers.contains_key(gone), "{gone} reached the browser");
    }
    assert_eq!(headers.get_all("connection").iter().count(), 1);
}

// ===== The copy =====

/// A relay between two pipes: returns the browser's and the upstream's far ends, and the
/// running relay.
fn relay(
    bounds: Bounds,
) -> (
    DuplexStream,
    DuplexStream,
    watch::Sender<bool>,
    tokio::task::JoinHandle<super::Relayed>,
) {
    let (browser, browser_end) = duplex(PIPE);
    let (upstream, upstream_end) = duplex(PIPE);
    let (stop, mut stopped) = watch::channel(false);
    let task =
        tokio::spawn(async move { pump(browser_end, upstream_end, bounds, &mut stopped).await });
    (browser, upstream, stop, task)
}

/// Generous bounds.
fn roomy() -> Bounds {
    Bounds {
        idle: Duration::from_secs(60),
        max_bytes: 1024 * 1024,
    }
}

/// Reads exactly `len` bytes.
async fn read_exactly(stream: &mut DuplexStream, len: usize) -> Vec<u8> {
    let mut buf = vec![0_u8; len];
    tokio::time::timeout(WITHIN, stream.read_exact(&mut buf))
        .await
        .expect("in time")
        .expect("the bytes");
    buf
}

/// Reads until the end of the stream; `true` when it ended in time.
async fn ends(stream: &mut DuplexStream) -> bool {
    let mut sink = Vec::new();
    tokio::time::timeout(WITHIN, stream.read_to_end(&mut sink))
        .await
        .is_ok()
}

#[tokio::test]
async fn bytes_cross_both_ways_and_a_close_ends_the_relay() {
    let (mut browser, mut upstream, _stop, task) = relay(roomy());
    browser.write_all(b"from the page").await.expect("writes");
    assert_eq!(read_exactly(&mut upstream, 13).await, b"from the page");
    upstream.write_all(b"from the site").await.expect("writes");
    assert_eq!(read_exactly(&mut browser, 13).await, b"from the site");

    browser.shutdown().await.expect("closes");
    assert!(ends(&mut upstream).await, "the close reached the site");
    upstream.shutdown().await.expect("closes");
    let relayed = tokio::time::timeout(WITHIN, task)
        .await
        .expect("the relay ends")
        .expect("no panic");
    assert_eq!(relayed.ended, Ended::Closed);
    assert_eq!((relayed.to_upstream, relayed.to_browser), (13, 13));
}

#[tokio::test]
async fn a_direction_past_the_byte_limit_ends_the_relay_without_passing_the_excess() {
    let bounds = Bounds {
        idle: Duration::from_secs(60),
        max_bytes: 100,
    };
    let (mut browser, mut upstream, _stop, task) = relay(bounds);
    browser.write_all(&[7_u8; 60]).await.expect("writes");
    assert_eq!(read_exactly(&mut upstream, 60).await, vec![7_u8; 60]);
    browser.write_all(&[8_u8; 4096]).await.expect("writes");

    let relayed = tokio::time::timeout(WITHIN, task)
        .await
        .expect("the relay ends")
        .expect("no panic");
    assert_eq!(relayed.ended, Ended::TooLarge);
    let mut rest = Vec::new();
    let _read = tokio::time::timeout(WITHIN, upstream.read_to_end(&mut rest)).await;
    assert!(
        rest.is_empty(),
        "{} bytes past the limit reached the site",
        rest.len()
    );
    assert!(ends(&mut browser).await, "the page side was closed");
}

#[tokio::test]
async fn an_idle_relay_is_closed() {
    let bounds = Bounds {
        idle: SHORT_IDLE,
        max_bytes: 1024,
    };
    let (mut browser, mut upstream, _stop, task) = relay(bounds);
    browser.write_all(b"once").await.expect("writes");
    assert_eq!(read_exactly(&mut upstream, 4).await, b"once");

    let relayed = tokio::time::timeout(WITHIN, task)
        .await
        .expect("the relay ends")
        .expect("no panic");
    assert_eq!(relayed.ended, Ended::Idle);
    assert!(ends(&mut browser).await, "the page side was closed");
    assert!(ends(&mut upstream).await, "the site side was closed");
}

#[tokio::test]
async fn traffic_keeps_a_relay_from_going_idle() {
    let bounds = Bounds {
        idle: SHORT_IDLE,
        max_bytes: 1024 * 1024,
    };
    let (mut browser, mut upstream, _stop, task) = relay(bounds);
    for _ in 0..6 {
        tokio::time::sleep(SHORT_IDLE / 2).await;
        browser.write_all(b"ping").await.expect("writes");
        assert_eq!(read_exactly(&mut upstream, 4).await, b"ping");
    }
    assert!(!task.is_finished(), "the relay closed while bytes moved");
    task.abort();
}

#[tokio::test]
async fn a_stopping_proxy_ends_the_relay() {
    let (mut browser, mut upstream, stop, task) = relay(roomy());
    browser.write_all(b"hi").await.expect("writes");
    assert_eq!(read_exactly(&mut upstream, 2).await, b"hi");

    stop.send_replace(true);
    let relayed = tokio::time::timeout(WITHIN, task)
        .await
        .expect("the relay ends")
        .expect("no panic");
    assert_eq!(relayed.ended, Ended::Stopped);
    assert!(ends(&mut browser).await, "the page side was closed");
    assert!(ends(&mut upstream).await, "the site side was closed");
}
