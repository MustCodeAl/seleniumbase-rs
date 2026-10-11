//! WebSocket through the proxy: the handshake and the echo, what the origin sees (the measured
//! WebSocket JA4, the browser's order with only the upgrade pair of hop fields), and the inputs
//! the proxy must refuse or cut short.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt as _, StreamExt as _};
use seleniumbase_rs::impersonate::mitm::{MitmOptions, MitmProxy};
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::emulation::{self, Emulation};
use seleniumbase_rs::wire::ja4::{Ja4, Ja4AField};
use seleniumbase_rs::wire::BrowserKind;
use tokio::io::AsyncWriteExt as _;
use tokio::net::TcpStream;
use tokio_btls::SslStream;
use tokio_tungstenite::tungstenite::Message;

use crate::echo::{
    accept_for, client_frame, read_frame, websocket_handshake, EchoOrigin, BINARY, CLOSE, KEY,
    REFUSED_BODY, REFUSED_PATH, TEXT,
};
use crate::support::{
    closes, der, eventually, exchange, measured_user_agent, new_ca, proxy_auth, read_head,
    start_mitm, tunnel, Head, QUIET, WITHIN,
};

/// The host the echo site is reached by.
const WS: &str = "ws.test";
/// The path of an accepted handshake.
const ECHO_PATH: &str = "/echo";
/// The measured file, with the evidence its specs were derived from.
const CHROME_FILE: &str = include_str!("../../src/wire/measured/chrome-155-macos.json");

/// An echo site, a proxy that trusts it and resolves [`WS`] to it (loopback allowed), the
/// proxy's sink, and the DER of the proxy's CA.
async fn echo_behind_proxy(
    options: MitmOptions,
) -> (EchoOrigin, MitmProxy, CollectingEventSink, Vec<u8>) {
    let origin = EchoOrigin::start().await;
    let ca = new_ca();
    let options = options
        .ca(Arc::clone(&ca))
        .trust_upstream_root(origin.root())
        .resolve(WS, origin.addr())
        .allow_private_destinations(true);
    let (mitm, sink) = start_mitm(None, options).await;
    (origin, mitm, sink, der(&ca))
}

/// Opens a tunnel to the echo site and sends `request` in it; returns the stream and the head
/// of the answer.
async fn send_head(
    mitm: &MitmProxy,
    ca: &[u8],
    port: u16,
    request: &str,
) -> (SslStream<TcpStream>, Head) {
    let mut tls = tunnel(mitm, WS, port, ca).await;
    tls.write_all(request.as_bytes()).await.expect("writes");
    tls.flush().await.expect("flushes");
    let head = read_head(&mut tls).await.expect("an answer");
    (tls, head)
}

/// A Chrome handshake for [`ECHO_PATH`] through a fresh tunnel, which must be switched.
async fn open_echo(mitm: &MitmProxy, ca: &[u8], port: u16) -> SslStream<TcpStream> {
    let request = websocket_handshake(WS, port, ECHO_PATH, &measured_user_agent(), &[]);
    let (tls, head) = send_head(mitm, ca, port, &request).await;
    assert_eq!(head.status(), 101, "{head:?}");
    assert!(head.rest.is_empty(), "no frame arrives before one is sent");
    tls
}

/// Sends one masked frame and reads the frame that comes back.
async fn round_trip(tls: &mut SslStream<TcpStream>, opcode: u8, payload: &[u8]) -> (u8, Vec<u8>) {
    tls.write_all(&client_frame(opcode, payload))
        .await
        .expect("writes");
    tls.flush().await.expect("flushes");
    read_frame(tls).await.expect("a frame back")
}

#[test]
fn the_accept_helper_matches_the_rfc_sample() {
    assert_eq!(accept_for(KEY), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}

#[tokio::test]
async fn a_websocket_echoes_through_the_proxy_with_the_accept_key_untouched() {
    let (mut origin, mitm, sink, ca) = echo_behind_proxy(MitmOptions::default()).await;
    let port = origin.addr().port();
    let request = websocket_handshake(WS, port, ECHO_PATH, &measured_user_agent(), &[]);
    let (mut tls, head) = send_head(&mitm, &ca, port, &request).await;

    assert_eq!(head.status(), 101);
    assert_eq!(
        head.header("sec-websocket-accept"),
        Some(accept_for(KEY).as_str())
    );
    assert!(head
        .header("upgrade")
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket")));
    assert!(head
        .header("connection")
        .is_some_and(|value| value.eq_ignore_ascii_case("upgrade")));
    origin
        .next(WITHIN)
        .await
        .expect("the origin took the handshake");

    let text = b"hello through the proxy";
    assert_eq!(
        round_trip(&mut tls, TEXT, text).await,
        (TEXT, text.to_vec())
    );
    // Over 64 KiB, so the frame carries the 64-bit length.
    let large: Vec<u8> = (0..70_000_u32)
        .map(|n| u8::try_from(n % 251).expect("under 251"))
        .collect();
    assert_eq!(round_trip(&mut tls, BINARY, &large).await, (BINARY, large));
    assert_eq!(round_trip(&mut tls, CLOSE, &[0x03, 0xe8]).await.0, CLOSE);
    assert!(closes(&mut tls).await, "the relay ends after the close");

    let stats = mitm.stats();
    assert_eq!((stats.websockets, stats.forwarded), (1, 1), "{stats:?}");
    assert!(sink.events().iter().any(|event| matches!(
        event,
        ScraperEvent::Request { status: 101, url, .. } if url.as_str().contains(ECHO_PATH)
    )));
    assert!(eventually(|| origin.closed() == 1).await);
}

#[tokio::test]
async fn an_independent_websocket_client_completes_the_handshake_through_the_proxy() {
    let (origin, mitm, _sink, ca) = echo_behind_proxy(MitmOptions::default()).await;
    let port = origin.addr().port();
    let tls = tunnel(&mitm, WS, port, &ca).await;
    let url = format!("wss://{WS}:{port}{ECHO_PATH}");

    let (mut socket, response) = tokio_tungstenite::client_async(url.as_str(), tls)
        .await
        .expect("tungstenite accepts the handshake, Sec-WebSocket-Accept included");
    assert_eq!(response.status().as_u16(), 101);
    socket
        .send(Message::text("checked by tungstenite"))
        .await
        .expect("sends");
    let echoed = tokio::time::timeout(WITHIN, socket.next())
        .await
        .expect("in time")
        .expect("a message")
        .expect("valid");
    assert_eq!(echoed, Message::text("checked by tungstenite"));
    socket.close(None).await.expect("closes");
}

fn chrome() -> &'static Emulation {
    emulation::get(BrowserKind::Chrome, OsType::Macos).expect("Chrome on macOS is measured")
}

#[tokio::test]
async fn the_upstream_hello_is_the_measured_websocket_hello_minus_exactly_the_known_gaps() {
    let (mut origin, mitm, _sink, ca) = echo_behind_proxy(MitmOptions::default()).await;
    let port = origin.addr().port();
    let _tls = open_echo(&mitm, &ca, port).await;
    let seen = origin
        .next(WITHIN)
        .await
        .expect("the origin took the handshake");

    let measured = chrome();
    let websocket = measured.tls_websocket.as_ref().expect("a WebSocket hello");
    let expected = websocket.without_known_gaps(&measured.provenance.known_gaps);
    let wire = Ja4::of(&seen.hello);
    eprintln!("the WebSocket JA4 on the wire: {wire}");
    assert_eq!(
        wire,
        expected.ja4(),
        "the measured WebSocket JA4 without its gaps"
    );
    assert_ne!(
        wire,
        measured
            .tls
            .without_known_gaps(&measured.provenance.known_gaps)
            .ja4(),
        "not the ordinary hello"
    );
    assert_eq!(seen.hello.alpn, [b"http/1.1".to_vec()], "HTTP/1.1 only");
    assert_eq!(seen.alpn.as_deref(), Some(&b"http/1.1"[..]));
    assert_eq!(seen.hello.sni.as_deref(), Some(WS));

    // The hello as Chrome sent it differs in exactly what the gaps say.
    let diff = wire.diff(&websocket.ja4());
    assert_eq!(diff.a_fields, [Ja4AField::ExtensionCount]);
    assert!(!diff.b, "the cipher hash equals the measured one");
    assert!(diff.c, "the extension hash differs by exactly the gaps");

    // And the measured spec is the one in the evidence.
    let file: serde_json::Value = serde_json::from_str(CHROME_FILE).expect("valid JSON");
    let evidence = file["capture"]["tls"]["websocket_sample"]["ja4"]
        .as_str()
        .expect("a WebSocket JA4 in the evidence");
    assert_eq!(websocket.ja4().to_string(), evidence);
    assert!(evidence.starts_with("t13d1516h1_"), "{evidence}");
}

#[tokio::test]
async fn the_handshake_reaches_the_origin_in_the_browsers_order_with_only_the_upgrade_pair() {
    let (mut origin, mitm, _sink, ca) = echo_behind_proxy(MitmOptions::default()).await;
    let port = origin.addr().port();
    let leaked = "Proxy-Authorization: Basic dXNlcjpub3QtYS1yZWFsLXBhc3N3b3Jk";
    let request = websocket_handshake(
        WS,
        port,
        ECHO_PATH,
        &measured_user_agent(),
        &[
            leaked,
            "Keep-Alive: timeout=5",
            "Proxy-Connection: keep-alive",
            "X-Hop: drop-me",
            "Connection: X-Hop",
            "Cookie: session=abc",
        ],
    );
    let (_tls, head) = send_head(&mitm, &ca, port, &request).await;
    assert_eq!(head.status(), 101);
    let seen = origin
        .next(WITHIN)
        .await
        .expect("the origin took the handshake");

    let names: Vec<String> = seen
        .request
        .headers
        .iter()
        .map(|(name, _)| name.to_ascii_lowercase())
        .collect();
    assert_eq!(
        names,
        [
            "host",
            "connection",
            "pragma",
            "cache-control",
            "user-agent",
            "upgrade",
            "origin",
            "sec-websocket-version",
            "accept-encoding",
            "accept-language",
            "sec-websocket-key",
            "sec-websocket-extensions",
            "cookie",
        ],
        "Host first, then the browser's order without the hop-by-hop and proxy fields"
    );
    let spelled: Vec<&str> = seen
        .request
        .headers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    for chrome_spelling in [
        "Host",
        "Connection",
        "Upgrade",
        "Sec-WebSocket-Version",
        "Sec-WebSocket-Key",
        "Sec-WebSocket-Extensions",
    ] {
        assert!(
            spelled.contains(&chrome_spelling),
            "{chrome_spelling} in {spelled:?}"
        );
    }
    assert_eq!(seen.request.header("connection"), Some("Upgrade"));
    assert_eq!(seen.request.header("upgrade"), Some("websocket"));
    assert_eq!(seen.request.header("sec-websocket-key"), Some(KEY));
    assert_eq!(seen.request.method, "GET");
    assert_eq!(seen.request.target, ECHO_PATH);
    assert_eq!(seen.request.version, "HTTP/1.1");
    assert!(seen.request.header("priority").is_none(), "HTTP/2-only");
}

/// Sends `request` through a fresh tunnel and returns the answer's status; the origin must see
/// no connection at all.
async fn refused_status(request: impl Fn(u16) -> String, options: MitmOptions) -> u16 {
    let (origin, mitm, _sink, ca) = echo_behind_proxy(options).await;
    let port = origin.addr().port();
    let mut tls = tunnel(&mitm, WS, port, &ca).await;
    let (head, body) = exchange(&mut tls, &request(port)).await.expect("an answer");
    assert!(body.is_empty(), "a refusal carries no detail");
    tokio::time::sleep(QUIET).await;
    assert_eq!(origin.connections(), 0, "the proxy reached the origin");
    let stats = mitm.stats();
    assert_eq!((stats.refused, stats.websockets), (1, 0), "{stats:?}");
    head.status()
}

/// A request head from its lines.
fn head_of(lines: &[String]) -> String {
    format!("{}\r\n\r\n", lines.join("\r\n"))
}

/// A Chrome handshake sent with `method`, with the fields named in `drop` (lower case) left out
/// and the lines of `add` appended.
fn handshake_lines(port: u16, method: &str, drop: &[&str], add: &[&str]) -> String {
    let base = websocket_handshake(WS, port, ECHO_PATH, &measured_user_agent(), &[]);
    let mut lines: Vec<String> = base
        .trim_end()
        .split("\r\n")
        .filter(|line| {
            !drop
                .iter()
                .any(|name| line.to_ascii_lowercase().starts_with(&format!("{name}:")))
        })
        .map(str::to_owned)
        .collect();
    lines[0] = format!("{method} {ECHO_PATH} HTTP/1.1");
    lines.extend(add.iter().map(|line| (*line).to_owned()));
    head_of(&lines)
}

#[tokio::test]
async fn connection_upgrade_without_an_upgrade_header_is_refused() {
    let status = refused_status(
        |port| handshake_lines(port, "GET", &["upgrade"], &[]),
        MitmOptions::default(),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn an_upgrade_header_without_connection_upgrade_is_refused() {
    let status = refused_status(
        |port| handshake_lines(port, "GET", &["connection"], &[]),
        MitmOptions::default(),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn an_upgrade_to_another_protocol_is_refused() {
    for protocol in ["h2c", "TLS/1.2", "websocket, h2c"] {
        let line = format!("Upgrade: {protocol}");
        let status = refused_status(
            |port| handshake_lines(port, "GET", &["upgrade"], &[line.as_str()]),
            MitmOptions::default(),
        )
        .await;
        assert_eq!(status, 502, "{protocol}");
    }
}

#[tokio::test]
async fn a_handshake_with_another_method_or_a_body_is_refused() {
    let post = refused_status(
        |port| handshake_lines(port, "POST", &[], &[]),
        MitmOptions::default(),
    )
    .await;
    assert_eq!(post, 400);
    let with_body = refused_status(
        |port| handshake_lines(port, "GET", &[], &["Content-Length: 4"]) + "ping",
        MitmOptions::default(),
    )
    .await;
    assert_eq!(with_body, 400);
}

#[tokio::test]
async fn a_handshake_claiming_another_chrome_major_or_naming_another_host_is_refused() {
    let agent = "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 Chrome/999.0.0.0 Safari/537.36";
    let mismatch = refused_status(
        |port| websocket_handshake(WS, port, ECHO_PATH, agent, &[]),
        MitmOptions::default(),
    )
    .await;
    assert_eq!(mismatch, 502);
    let other_host = refused_status(
        |port| websocket_handshake("other.test", port, ECHO_PATH, &measured_user_agent(), &[]),
        MitmOptions::default(),
    )
    .await;
    assert_eq!(other_host, 400);
}

#[tokio::test]
async fn an_oversize_frame_ends_the_relay_and_never_reaches_the_origin_whole() {
    const LIMIT: u64 = 1024;
    let options = MitmOptions::default().max_websocket_bytes(LIMIT);
    let (origin, mitm, _sink, ca) = echo_behind_proxy(options).await;
    let mut tls = open_echo(&mitm, &ca, origin.addr().port()).await;
    assert_eq!(
        round_trip(&mut tls, TEXT, b"small").await,
        (TEXT, b"small".to_vec())
    );

    tls.write_all(&client_frame(BINARY, &[0x5a; 4096]))
        .await
        .expect("writes");
    tls.flush().await.expect("flushes");

    assert!(closes(&mut tls).await, "the proxy closed the page's side");
    assert!(
        eventually(|| origin.closed() == 1).await,
        "and the origin's"
    );
    let received = u64::try_from(origin.received()).expect("fits");
    assert!(
        received <= LIMIT,
        "{received} bytes reached the origin past a limit of {LIMIT}"
    );
}

#[tokio::test]
async fn an_idle_websocket_is_closed_on_both_sides() {
    let options = MitmOptions::default().websocket_idle_timeout(Duration::from_millis(300));
    let (origin, mitm, _sink, ca) = echo_behind_proxy(options).await;
    let mut tls = open_echo(&mitm, &ca, origin.addr().port()).await;
    assert_eq!(
        round_trip(&mut tls, TEXT, b"once").await,
        (TEXT, b"once".to_vec())
    );

    let started = Instant::now();
    assert!(closes(&mut tls).await, "the idle connection was closed");
    assert!(started.elapsed() < WITHIN);
    assert!(
        eventually(|| origin.closed() == 1).await,
        "on the origin's side too"
    );
}

#[tokio::test]
async fn the_origin_refusing_the_upgrade_reaches_the_browser_as_an_ordinary_answer() {
    let (origin, mitm, _sink, ca) = echo_behind_proxy(MitmOptions::default()).await;
    let port = origin.addr().port();
    let mut tls = tunnel(&mitm, WS, port, &ca).await;
    let request = websocket_handshake(WS, port, REFUSED_PATH, &measured_user_agent(), &[]);
    let (head, body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 403);
    assert_eq!(body, REFUSED_BODY);
    assert_eq!(origin.handshakes(), 1);
    let stats = mitm.stats();
    assert_eq!((stats.websockets, stats.forwarded), (0, 1), "{stats:?}");
}

#[tokio::test]
async fn without_the_switch_a_name_resolving_to_loopback_is_never_reached() {
    let origin = EchoOrigin::start().await;
    let ca = new_ca();
    let options = MitmOptions::default()
        .ca(Arc::clone(&ca))
        .trust_upstream_root(origin.root())
        .resolve(WS, origin.addr());
    let (mitm, _sink) = start_mitm(None, options).await;
    let port = origin.addr().port();
    let request = websocket_handshake(WS, port, ECHO_PATH, &measured_user_agent(), &[]);
    let mut tls = tunnel(&mitm, WS, port, &der(&ca)).await;
    let (head, _body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 502);
    tokio::time::sleep(QUIET).await;
    assert_eq!(origin.connections(), 0, "the proxy connected to loopback");
    assert_eq!(mitm.stats().upstream_errors, 1, "{:?}", mitm.stats());
}

#[tokio::test]
async fn a_websocket_handshake_outside_a_tunnel_is_refused() {
    let (origin, mitm, _sink, _ca) = echo_behind_proxy(MitmOptions::default()).await;
    let port = origin.addr().port();
    let mut tcp = TcpStream::connect(mitm.local_addr())
        .await
        .expect("reaches the proxy");
    let request = websocket_handshake(WS, port, ECHO_PATH, &measured_user_agent(), &[]).replacen(
        &format!("GET {ECHO_PATH} HTTP/1.1"),
        &format!(
            "GET http://{WS}:{port}{ECHO_PATH} HTTP/1.1\r\nProxy-Authorization: {}",
            proxy_auth(&mitm)
        ),
        1,
    );
    let (head, body) = exchange(&mut tcp, &request).await.expect("an answer");

    assert_eq!(head.status(), 502);
    assert!(body.is_empty());
    tokio::time::sleep(QUIET).await;
    assert_eq!(origin.connections(), 0);
}

#[tokio::test]
async fn stopping_the_proxy_closes_an_open_websocket_at_once() {
    let (origin, mitm, _sink, ca) = echo_behind_proxy(MitmOptions::default()).await;
    let mut tls = open_echo(&mitm, &ca, origin.addr().port()).await;
    assert_eq!(
        round_trip(&mut tls, TEXT, b"before").await,
        (TEXT, b"before".to_vec())
    );

    let grace = Duration::from_secs(5);
    let started = Instant::now();
    mitm.shutdown(grace).await;
    assert!(
        started.elapsed() < grace,
        "the relay held the shutdown for its whole grace"
    );
    assert!(closes(&mut tls).await, "the page's side was closed");
    assert!(
        eventually(|| origin.closed() == 1).await,
        "and the origin's"
    );
}
