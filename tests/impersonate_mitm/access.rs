//! Who may use the proxy, what it refuses instead of tunnelling, and its bounds.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use seleniumbase_rs::impersonate::mitm::{MitmOptions, MitmProxy, MIN_HEADER_BYTES, PRIME_HOST};
use seleniumbase_rs::impersonate::tls_server::{CaOptions, EphemeralCa};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

use crate::support::{
    closes, der, eventually, exchange, new_ca, proxy_auth, raw_connect, read_head, start_mitm,
    tls_client, tunnel, QUIET,
};

/// How long an upstream connection may take in the tests that send one to a name that does not
/// resolve.
const SHORT_CONNECT: Duration = Duration::from_secs(5);

/// The path of a proxy's priming page.
fn prime_path(mitm: &MitmProxy) -> String {
    mitm.prime_url()
        .strip_prefix(&format!("https://{PRIME_HOST}"))
        .expect("the priming page is on the priming host")
        .to_owned()
}

/// Writes a `CONNECT` on an open connection and reads the answer.
async fn connect_on(tcp: &mut TcpStream, target: &str, auth: &str) -> u16 {
    let request = format!(
        "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nProxy-Authorization: {auth}\r\n\r\n"
    );
    tcp.write_all(request.as_bytes()).await.expect("writes");
    read_head(tcp).await.expect("an answer").status()
}

#[tokio::test]
async fn a_connect_without_the_token_gets_407_with_a_basic_challenge() {
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;

    let (_tcp, head) = raw_connect(mitm.local_addr(), "origin.test:443", None).await;

    let head = head.expect("an answer");
    assert_eq!(head.status(), 407);
    assert_eq!(
        head.header("proxy-authenticate"),
        Some("Basic realm=\"seleniumbase-rs\"")
    );
    assert_eq!(head.content_length(), Some(0));
    assert_eq!(mitm.stats().unauthorized, 1);
    assert_eq!(mitm.stats().tunnels, 0);
}

#[tokio::test]
async fn a_wrong_token_gets_407_and_the_right_one_200_on_the_same_connection() {
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;
    let mut tcp = TcpStream::connect(mitm.local_addr())
        .await
        .expect("reaches the proxy");

    let wrong = "Basic c2I6MDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDA=";
    assert_eq!(connect_on(&mut tcp, "origin.test:443", wrong).await, 407);
    assert_eq!(
        connect_on(&mut tcp, "origin.test:443", &proxy_auth(&mitm)).await,
        200
    );
    assert_eq!(mitm.stats().unauthorized, 1);
}

#[tokio::test]
async fn a_target_that_cannot_be_intercepted_gets_an_empty_502_and_the_connection_closes() {
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;
    let auth = proxy_auth(&mitm);

    for target in ["*.wild.test:443", "origin.test:0"] {
        let (mut tcp, head) = raw_connect(mitm.local_addr(), target, Some(&auth)).await;
        let head = head.expect("an answer");
        assert_eq!(head.status(), 502, "{target}");
        assert_eq!(head.content_length(), Some(0), "{target}: no detail");
        assert!(
            closes(&mut tcp).await,
            "{target}: the connection stays open"
        );
    }
    assert_eq!(mitm.stats().tunnels, 0);
    assert_eq!(mitm.stats().refused, 2);
}

#[tokio::test]
async fn bytes_that_are_not_tls_after_connect_are_never_tunnelled() {
    let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("binds loopback");
    let addr = target.local_addr().expect("an address");
    let options = MitmOptions::default().resolve("plain.test", addr);
    let (mitm, _sink) = start_mitm(None, options).await;

    let (mut tcp, head) = raw_connect(
        mitm.local_addr(),
        &format!("plain.test:{}", addr.port()),
        Some(&proxy_auth(&mitm)),
    )
    .await;
    assert_eq!(head.expect("an answer").status(), 200);
    tcp.write_all(b"SSH-2.0-OpenSSH_9.9\r\n")
        .await
        .expect("writes");

    assert!(closes(&mut tcp).await, "the connection must end");
    let reached = tokio::time::timeout(QUIET, target.accept()).await;
    assert!(
        reached.is_err(),
        "the proxy opened a connection to the target"
    );
    assert_eq!(mitm.stats().tunnels, 0);
    assert_eq!(mitm.stats().refused, 1);
}

#[tokio::test]
async fn a_client_sni_naming_another_host_closes_the_tunnel() {
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;
    let (tcp, head) = raw_connect(mitm.local_addr(), "a.test:443", Some(&proxy_auth(&mitm))).await;
    assert_eq!(head.expect("an answer").status(), 200);

    // No verification on the client, so its side of the handshake can finish; the proxy
    // checks the SNI once its side is done and must drop the connection.
    if let Ok(mut tls) = tls_client(tcp, Some("b.test"), None).await {
        let answer = exchange(&mut tls, "GET / HTTP/1.1\r\nHost: a.test\r\n\r\n").await;
        assert!(
            answer.is_none(),
            "a request was served over a mismatched SNI"
        );
    }
    assert!(eventually(|| mitm.stats().refused == 1).await);
    assert_eq!(mitm.stats().tunnels, 0);
}

#[tokio::test]
async fn the_leaf_cache_stays_bounded() {
    let capacity = 4;
    let ca_options = CaOptions {
        leaf_capacity: capacity,
        ..CaOptions::default()
    };
    let ca = Arc::new(EphemeralCa::generate_at(ca_options, SystemTime::now()).expect("a CA"));
    let (mitm, _sink) = start_mitm(None, MitmOptions::default().ca(Arc::clone(&ca))).await;
    let root = der(&ca);

    for index in 0..10 {
        let _tls = tunnel(&mitm, &format!("h{index}.test"), 443, &root).await;
        assert!(ca.cached_leaves() <= capacity);
    }
    assert_eq!(ca.cached_leaves(), capacity);
    assert!(eventually(|| mitm.stats().tunnels == 10).await);
}

#[tokio::test]
async fn the_priming_page_is_answered_by_the_proxy_itself_behind_the_token() {
    let ca = new_ca();
    let (mitm, sink) = start_mitm(None, MitmOptions::default().ca(Arc::clone(&ca))).await;
    let path = prime_path(&mitm);
    let target = format!("{PRIME_HOST}:443");

    let (_tcp, head) = raw_connect(mitm.local_addr(), &target, None).await;
    assert_eq!(head.expect("an answer").status(), 407);

    let mut tls = tunnel(&mitm, PRIME_HOST, 443, &der(&ca)).await;
    let request = format!("GET {path} HTTP/1.1\r\nHost: {PRIME_HOST}\r\n\r\n");
    let (head, body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 200);
    assert_eq!(head.content_length(), Some(0));
    assert!(body.is_empty());
    assert_eq!(
        head.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(head.header("cache-control"), Some("no-store"));
    let stats = mitm.stats();
    assert_eq!(stats.primed, 1, "{stats:?}");
    assert_eq!(
        (stats.forwarded, stats.upstream_errors, stats.refused),
        (0, 0, 0),
        "nothing went upstream: {stats:?}"
    );
    assert!(sink.is_empty(), "not a site request: {:?}", sink.events());
}

#[tokio::test]
async fn the_priming_page_is_answered_over_plain_http_too_and_only_with_the_token() {
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;
    let path = prime_path(&mitm);
    let mut tcp = TcpStream::connect(mitm.local_addr())
        .await
        .expect("reaches the proxy");
    let line = format!("GET http://{PRIME_HOST}{path} HTTP/1.1\r\nHost: {PRIME_HOST}\r\n");

    let (refused, _) = exchange(&mut tcp, &format!("{line}\r\n"))
        .await
        .expect("an answer");
    let auth = format!("Proxy-Authorization: {}\r\n", proxy_auth(&mitm));
    let (answered, _) = exchange(&mut tcp, &format!("{line}{auth}\r\n"))
        .await
        .expect("an answer on the same connection");

    assert_eq!(refused.status(), 407);
    assert_eq!(answered.status(), 200);
    assert_eq!(mitm.stats().primed, 1, "{:?}", mitm.stats());
    assert_eq!(mitm.stats().forwarded, 0, "{:?}", mitm.stats());
}

#[tokio::test]
async fn the_priming_host_serves_its_page_only_and_nothing_for_it_goes_upstream() {
    let ca = new_ca();
    let options = MitmOptions::default()
        .ca(Arc::clone(&ca))
        .connect_timeout(SHORT_CONNECT);
    let (mitm, sink) = start_mitm(None, options).await;
    let path = prime_path(&mitm);
    let other_path = format!("/{}", "0".repeat(32));

    for request in [
        format!("GET {other_path} HTTP/1.1\r\nHost: {PRIME_HOST}\r\n\r\n"),
        format!("GET /favicon.ico HTTP/1.1\r\nHost: {PRIME_HOST}\r\n\r\n"),
        format!("GET {path}?x=1 HTTP/1.1\r\nHost: {PRIME_HOST}\r\n\r\n"),
        format!("POST {path} HTTP/1.1\r\nHost: {PRIME_HOST}\r\nContent-Length: 0\r\n\r\n"),
        format!(
            "GET {path} HTTP/1.1\r\nHost: {PRIME_HOST}\r\nConnection: Upgrade\r\n\
             Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
        ),
    ] {
        let mut tls = tunnel(&mitm, PRIME_HOST, 443, &der(&ca)).await;
        let (head, body) = exchange(&mut tls, &request).await.expect("an answer");
        assert_eq!(head.status(), 404, "{request}");
        assert!(body.is_empty(), "{request}");
    }
    // Not counted as refusals either: Chrome asks for the priming page's `favicon.ico` itself.
    let stats = mitm.stats();
    assert_eq!(
        (
            stats.primed,
            stats.refused,
            stats.forwarded,
            stats.upstream_errors
        ),
        (0, 0, 0, 0),
        "{stats:?}"
    );
    assert!(sink.is_empty(), "{:?}", sink.events());

    // The priming path on another host is an ordinary request: it goes upstream, where the
    // name does not resolve.
    let mut tls = tunnel(&mitm, "other.invalid", 443, &der(&ca)).await;
    let request = format!("GET {path} HTTP/1.1\r\nHost: other.invalid\r\n\r\n");
    let (head, _) = exchange(&mut tls, &request).await.expect("an answer");
    assert_eq!(head.status(), 502);
    assert_eq!(mitm.stats().upstream_errors, 1, "{:?}", mitm.stats());
    assert_eq!(mitm.stats().primed, 0, "{:?}", mitm.stats());
}

#[tokio::test]
async fn a_request_head_over_the_limit_is_refused() {
    let options = MitmOptions::default().max_header_bytes(MIN_HEADER_BYTES);
    let (mitm, _sink) = start_mitm(None, options).await;
    let mut tcp = TcpStream::connect(mitm.local_addr())
        .await
        .expect("reaches the proxy");

    let filler = "a".repeat(2 * MIN_HEADER_BYTES);
    let request = format!(
        "CONNECT origin.test:443 HTTP/1.1\r\nHost: origin.test:443\r\nX-Filler: {filler}\r\n\
         Proxy-Authorization: {}\r\n\r\n",
        proxy_auth(&mitm)
    );
    // The proxy may close while the head is still being written.
    let _written = tcp.write_all(request.as_bytes()).await;

    let head = read_head(&mut tcp).await;
    assert!(
        head.as_ref().is_none_or(|head| head.status() == 431),
        "{head:?}"
    );
    assert_eq!(mitm.stats().tunnels, 0);
}

#[tokio::test]
async fn connections_past_the_limit_wait_for_a_free_slot() {
    let options = MitmOptions::default().max_connections(2);
    let (mitm, _sink) = start_mitm(None, options).await;
    let addr = mitm.local_addr();
    let first = TcpStream::connect(addr).await.expect("first");
    let _second = TcpStream::connect(addr).await.expect("second");
    assert!(eventually(|| mitm.stats().connections == 2).await);

    let mut third = TcpStream::connect(addr)
        .await
        .expect("queued by the kernel");
    third
        .write_all(b"CONNECT origin.test:443 HTTP/1.1\r\nHost: origin.test:443\r\n\r\n")
        .await
        .expect("writes");
    let early = tokio::time::timeout(QUIET, read_head(&mut third)).await;
    assert!(
        early.is_err(),
        "a third connection was served past the limit"
    );

    drop(first);
    let head = read_head(&mut third)
        .await
        .expect("served once a slot frees");
    assert_eq!(head.status(), 407);
    assert_eq!(mitm.stats().connections, 3);
}

#[tokio::test]
async fn an_idle_connection_is_closed() {
    let options = MitmOptions::default().idle_timeout(Duration::from_millis(300));
    let (mitm, _sink) = start_mitm(None, options).await;
    let mut tcp = TcpStream::connect(mitm.local_addr())
        .await
        .expect("reaches the proxy");

    let started = Instant::now();
    assert!(closes(&mut tcp).await, "the idle connection stays open");
    assert!(started.elapsed() < Duration::from_secs(5));
}
