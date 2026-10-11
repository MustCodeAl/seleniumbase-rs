//! The capture listeners against real loopback sockets, and the committed Chrome measurement
//! against its own raw bytes. None of it needs a browser.
#![cfg(feature = "scrape")]

use std::time::Duration;

use seleniumbase_rs::scrape::runtime::capture::hints::cross_check;
use seleniumbase_rs::scrape::runtime::capture::record::to_hex;
use seleniumbase_rs::scrape::runtime::capture::stream::{h2c_reply, http1_response};
use seleniumbase_rs::scrape::runtime::capture::{Capture, CaptureError, TlsMeasurement};
use seleniumbase_rs::stealth::fingerprint::{ClientHints, OsType};
use seleniumbase_rs::wire::client_hello::ClientHello;
use seleniumbase_rs::wire::h2_frames::{encode_frame, frame_type, CLIENT_PREFACE};
use seleniumbase_rs::wire::ja3::Ja3;
use seleniumbase_rs::wire::ja4::Ja4;
use seleniumbase_rs::wire::tls_spec::{KnownGap, TlsSpec};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const WITHIN: Duration = Duration::from_secs(5);

fn len16(body: &[u8]) -> Vec<u8> {
    let len = u16::try_from(body.len()).expect("a short body");
    let mut out = len.to_be_bytes().to_vec();
    out.extend_from_slice(body);
    out
}

fn extension(id: u16, body: &[u8]) -> Vec<u8> {
    let mut out = id.to_be_bytes().to_vec();
    out.extend(len16(body));
    out
}

/// A small but well-formed `ClientHello` in one record: two TLS 1.3 suites and the `server_name`,
/// `supported_versions` and ALPN extensions.
fn hello_bytes() -> Vec<u8> {
    let mut name = vec![0_u8];
    name.extend(len16(b"capture.test"));
    let mut alpn = vec![2, b'h', b'2', 8];
    alpn.extend_from_slice(b"http/1.1");
    let mut extensions = extension(0x0000, &len16(&name));
    extensions.extend(extension(0x002B, &[4, 3, 4, 3, 3]));
    extensions.extend(extension(0x0010, &len16(&alpn)));

    let mut body = vec![3, 3];
    body.extend_from_slice(&[0x42; 32]);
    body.push(0);
    body.extend(len16(&[0x13, 0x01, 0x13, 0x02]));
    body.extend_from_slice(&[1, 0]);
    body.extend(len16(&extensions));

    let len = u32::try_from(body.len())
        .expect("a short body")
        .to_be_bytes();
    let mut message = vec![1, len[1], len[2], len[3]];
    message.extend(body);
    let mut record = vec![22, 3, 1];
    record.extend(len16(&message));
    record
}

/// Connects to `port` and sends `bytes` a few at a time, as a slow client would.
async fn send_slowly(port: u16, bytes: Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    for piece in bytes.chunks(5) {
        stream.write_all(piece).await.expect("sends");
        tokio::task::yield_now().await;
    }
    // Keep the connection open until the listener has read what it needs.
    let mut sink = Vec::new();
    let _closed = tokio::time::timeout(WITHIN, stream.read_to_end(&mut sink)).await;
}

#[tokio::test]
async fn a_hello_sent_in_small_pieces_is_captured_byte_for_byte() {
    let capture = Capture::bind().await.expect("binds");
    let sent = hello_bytes();
    let client = tokio::spawn(send_slowly(capture.port(), sent.clone()));

    let captured = tokio::time::timeout(WITHIN, capture.accept_client_hello())
        .await
        .expect("a client connects")
        .expect("a hello");

    assert_eq!(captured.raw, sent);
    assert_eq!(captured.hello.sni.as_deref(), Some("capture.test"));
    assert_eq!(captured.hello.cipher_suites, [0x1301, 0x1302]);
    assert!(captured.peer.ip().is_loopback());
    let ja4 = Ja4::of(&captured.hello).to_string();
    assert!(ja4.starts_with("t13d0203h2_"), "{ja4}");
    client.await.expect("the client task ends");
}

#[tokio::test]
async fn the_listener_binds_loopback_only() {
    let capture = Capture::bind().await.expect("binds");

    assert!(capture.addr().ip().is_loopback());
    assert_ne!(capture.port(), 0);
}

#[tokio::test]
async fn silent_and_junk_connections_do_not_hide_a_real_hello() {
    let capture = Capture::bind().await.expect("binds");
    let port = capture.port();
    let silent = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    let mut junk = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    junk.write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .expect("sends");
    let client = tokio::spawn(send_slowly(port, hello_bytes()));

    let hellos = capture.collect_client_hellos(1, WITHIN).await;

    assert_eq!(hellos.len(), 1);
    assert_eq!(hellos.first().map(|hello| &hello.raw), Some(&hello_bytes()));
    drop(silent);
    client.abort();
}

#[tokio::test]
async fn collecting_gives_up_when_no_hello_arrives() {
    let capture = Capture::bind().await.expect("binds");

    let hellos = capture
        .collect_client_hellos(3, Duration::from_millis(100))
        .await;

    assert!(hellos.is_empty());
}

#[tokio::test]
async fn collecting_stops_when_the_caller_is_satisfied() {
    let capture = Capture::bind().await.expect("binds");
    let port = capture.port();
    let clients: Vec<_> = (0..3)
        .map(|_| tokio::spawn(send_slowly(port, hello_bytes())))
        .collect();

    let hellos = capture
        .collect_client_hellos_until(WITHIN, |so_far| so_far.len() >= 2)
        .await;

    assert_eq!(hellos.len(), 2);
    for client in clients {
        client.abort();
    }
}

/// The bytes of a client that starts HTTP/2 with prior knowledge.
fn h2c_client_bytes() -> Vec<u8> {
    let mut bytes = CLIENT_PREFACE.to_vec();
    bytes.extend(encode_frame(frame_type::SETTINGS, 0, 0, &[0, 3, 0, 0, 0, 100]).expect("frame"));
    bytes.extend(
        encode_frame(
            frame_type::WINDOW_UPDATE,
            0,
            0,
            &15_663_105_u32.to_be_bytes(),
        )
        .expect("frame"),
    );
    let mut block = vec![0x82, 0x86, 0x84, 0x41, 0x09];
    block.extend_from_slice(b"localhost");
    bytes.extend(encode_frame(frame_type::HEADERS, 0x05, 1, &block).expect("frame"));
    bytes
}

#[tokio::test]
async fn an_h2c_client_is_read_and_answered() {
    let capture = Capture::bind().await.expect("binds");
    let port = capture.port();
    let client = tokio::spawn(async move {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connects");
        stream.write_all(&h2c_client_bytes()).await.expect("sends");
        let mut reply = Vec::new();
        tokio::time::timeout(WITHIN, stream.read_to_end(&mut reply))
            .await
            .expect("the listener closes the connection")
            .expect("reads");
        reply
    });

    let captured = tokio::time::timeout(WITHIN, capture.accept_h2c())
        .await
        .expect("a client connects")
        .expect("an HTTP/2 start");

    assert_eq!(captured.raw, h2c_client_bytes());
    assert_eq!(
        captured
            .preface
            .akamai()
            .expect("four pseudo-headers")
            .to_string(),
        "3:100|15663105|0|m,s,p,a"
    );
    assert_eq!(
        client.await.expect("client task"),
        h2c_reply(1).expect("a reply")
    );
}

#[tokio::test]
async fn an_http1_request_is_read_and_answered() {
    let capture = Capture::bind().await.expect("binds");
    let port = capture.port();
    let client = tokio::spawn(async move {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connects");
        stream
            .write_all(b"GET /probe HTTP/1.1\r\nHost: capture\r\nUser-Agent: Test/1\r\n\r\n")
            .await
            .expect("sends");
        let mut reply = Vec::new();
        tokio::time::timeout(WITHIN, stream.read_to_end(&mut reply))
            .await
            .expect("the listener closes the connection")
            .expect("reads");
        reply
    });
    let response = http1_response(200, "OK", &[("Content-Type", "text/plain")], b"ok");

    let captured = tokio::time::timeout(WITHIN, capture.accept_http1(&response))
        .await
        .expect("a client connects")
        .expect("a request");

    assert_eq!(captured.request.path(), "/probe");
    let names: Vec<&str> = captured
        .request
        .headers
        .iter()
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(names, ["Host", "User-Agent"]);
    assert_eq!(client.await.expect("client task"), response);
}

#[tokio::test]
async fn a_connection_that_sends_nothing_times_out_without_a_hello() {
    let capture = Capture::bind()
        .await
        .expect("binds")
        .with_timeout(Duration::from_millis(100));
    let port = capture.port();
    let _silent = tokio::spawn(async move {
        let _stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connects");
        tokio::time::sleep(WITHIN).await;
    });

    let error = tokio::time::timeout(WITHIN, capture.accept_client_hello())
        .await
        .expect("the read timeout fires first")
        .expect_err("nothing was sent");

    assert!(matches!(error, CaptureError::Timeout { .. }), "{error}");
}

// The committed Chrome measurement. These tests read the files the capture tools wrote and check
// them against each other; none of them starts a browser.

/// The measured file of the Chrome the tools were last run against.
const MEASURED: &str = include_str!("../src/wire/measured/chrome-155-macos.json");
/// The hellos of the ordinary HTTPS connections, as the listener read them.
const PAGE_HELLOS: [&[u8]; 4] = [
    include_bytes!("fixtures/measured/chrome-155-macos.client_hello.1.bin"),
    include_bytes!("fixtures/measured/chrome-155-macos.client_hello.2.bin"),
    include_bytes!("fixtures/measured/chrome-155-macos.client_hello.3.bin"),
    include_bytes!("fixtures/measured/chrome-155-macos.client_hello.4.bin"),
];
/// The hello of the WebSocket connection.
const WEBSOCKET_HELLO: &[u8] =
    include_bytes!("fixtures/measured/chrome-155-macos.websocket_hello.bin");

fn measured() -> serde_json::Value {
    serde_json::from_str(MEASURED).expect("the measured file is JSON")
}

fn parse(raw: &[u8]) -> ClientHello {
    ClientHello::parse(raw).expect("the committed bytes are a ClientHello")
}

#[test]
fn the_committed_hellos_parse_and_show_a_browser() {
    for raw in PAGE_HELLOS.iter().copied().chain([WEBSOCKET_HELLO]) {
        let hello = parse(raw);

        assert!(hello.has_grease(), "Chrome sends GREASE");
        let ja4 = Ja4::of(&hello).to_string();
        assert!(ja4.starts_with("t13d"), "{ja4}");
        let name = hello.sni.as_deref().unwrap_or_default();
        assert!(name.ends_with(".capture.test"), "{name}");
    }
    let ja3: Vec<String> = PAGE_HELLOS
        .iter()
        .map(|raw| Ja3::of(&parse(raw)).to_string())
        .collect();
    let mut distinct = ja3.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert!(
        distinct.len() > 1,
        "Chrome shuffles, so the JA3 values differ"
    );
    let ja4: Vec<String> = PAGE_HELLOS
        .iter()
        .map(|raw| Ja4::of(&parse(raw)).to_string())
        .collect();
    assert!(ja4.windows(2).all(|pair| pair[0] == pair[1]), "{ja4:?}");
}

#[test]
fn the_json_evidence_is_the_committed_bytes_and_their_fingerprints() {
    let file = measured();
    let samples = file["capture"]["tls"]["samples"]
        .as_array()
        .expect("samples are listed");

    assert_eq!(samples.len(), PAGE_HELLOS.len());
    for (sample, raw) in samples.iter().zip(PAGE_HELLOS) {
        let hello = parse(raw);
        assert_eq!(sample["raw_hex"], to_hex(raw));
        assert_eq!(sample["ja3"], Ja3::of(&hello).to_string());
        assert_eq!(sample["ja4"], Ja4::of(&hello).to_string());
        assert_eq!(sample["ja4_r"], Ja4::raw(&hello));
    }
    let websocket = &file["capture"]["tls"]["websocket_sample"];
    assert_eq!(websocket["raw_hex"], to_hex(WEBSOCKET_HELLO));
    assert_eq!(
        websocket["ja4"],
        Ja4::of(&parse(WEBSOCKET_HELLO)).to_string()
    );
}

#[test]
fn the_committed_spec_is_what_the_committed_bytes_derive() {
    let file = measured();
    let pages: Vec<Vec<u8>> = PAGE_HELLOS.iter().map(|raw| raw.to_vec()).collect();

    let derived =
        TlsMeasurement::from_hellos(&pages, Some(WEBSOCKET_HELLO)).expect("one stable browser");

    let stored: TlsSpec = serde_json::from_value(file["tls"].clone()).expect("tls is a TlsSpec");
    assert_eq!(stored, derived.spec);
    let websocket: TlsSpec =
        serde_json::from_value(file["tls_websocket"].clone()).expect("a WebSocket spec");
    assert_eq!(Some(websocket), derived.websocket_spec);
    assert!(stored.grease && stored.permute_extensions);
    assert_eq!(stored.alpn, ["h2", "http/1.1"]);
    assert_eq!(
        derived.websocket_spec.map(|spec| spec.alpn),
        Some(vec!["http/1.1".to_owned()])
    );
}

#[test]
fn the_committed_spec_differs_from_what_boringssl_sends_by_exactly_the_known_gaps() {
    let file = measured();
    let stored: TlsSpec = serde_json::from_value(file["tls"].clone()).expect("tls is a TlsSpec");
    let gaps: Vec<KnownGap> = serde_json::from_value(file["provenance"]["known_gaps"].clone())
        .expect("the gaps are listed");

    assert_eq!(
        gaps,
        [
            KnownGap::TrustAnchorsExtension,
            KnownGap::MlDsaSignatureAlgorithms
        ]
    );
    assert!(stored.check_reproducible().is_err(), "the gaps are real");
    stored
        .without_known_gaps(&gaps)
        .check_reproducible()
        .expect("nothing but the known gaps stops a BoringSSL client");
}

#[test]
fn the_provenance_names_the_browser_the_tools_and_the_day() {
    let file = measured();
    let provenance = &file["provenance"];

    assert_eq!(file["kind"], "Chrome");
    assert_eq!(file["major"], 155);
    assert_eq!(
        file["os"],
        serde_json::to_value(OsType::Macos).expect("serializes")
    );
    assert_eq!(
        provenance["browser"],
        format!(
            "Chrome {}",
            file["full_version"].as_str().unwrap_or_default()
        )
    );
    assert!(file["full_version"]
        .as_str()
        .is_some_and(|v| v.starts_with("155.")));
    let tool = provenance["tool"].as_str().expect("a tool");
    assert!(
        tool.contains("capture_tls") && tool.contains("capture_hints"),
        "{tool}"
    );
    let day = provenance["captured_on"].as_str().expect("a date");
    assert_eq!(day.len(), 10, "{day}");
    assert!(day.starts_with("20") && day.as_bytes()[4] == b'-' && day.as_bytes()[7] == b'-');
}

#[test]
fn the_committed_hints_agree_with_the_request_headers_they_were_read_beside() {
    let file = measured();
    let hints: ClientHints =
        serde_json::from_value(file["client_hints"].clone()).expect("client hints");
    let headers: Vec<(String, String)> =
        serde_json::from_value(file["capture"]["hints"]["request_headers"].clone())
            .expect("the headers are listed");

    cross_check(&hints, &headers).expect("the page and the headers describe one browser");

    let user_agent = file["user_agent"].as_str().expect("a user agent");
    let sent = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("user-agent"))
        .map(|(_, value)| value.as_str());
    assert_eq!(sent, Some(user_agent));
    assert!(user_agent.contains("Chrome/155.0.0.0"), "{user_agent}");
    assert_eq!(hints.platform, "macOS");
    assert!(!hints.mobile);
    assert_eq!(hints.ua_full_version, file["full_version"]);
    let names: Vec<&str> = hints.brands.iter().map(|b| b.brand.as_str()).collect();
    assert!(
        names.contains(&"Google Chrome") && names.contains(&"Chromium"),
        "{names:?}"
    );
    assert_eq!(names.len(), 3, "two real brands and one decoy: {names:?}");
    assert!(hints
        .full_version_list
        .iter()
        .any(|b| b.version == hints.ua_full_version));
}

#[test]
fn the_first_visit_carries_only_the_low_entropy_hints() {
    let file = measured();
    let first: Vec<(String, String)> =
        serde_json::from_value(file["capture"]["hints"]["first_visit_headers"].clone())
            .expect("the headers are listed");
    let names: Vec<String> = first
        .iter()
        .map(|(name, _)| name.to_ascii_lowercase())
        .collect();

    for low in ["sec-ch-ua", "sec-ch-ua-mobile", "sec-ch-ua-platform"] {
        assert!(names.iter().any(|name| name == low), "{low}");
    }
    assert!(!names
        .iter()
        .any(|name| name == "sec-ch-ua-platform-version"));
    assert!(
        !names.iter().any(|name| name == "cookie"),
        "no cookie is ever recorded"
    );
}

#[tokio::test]
async fn the_real_chrome_hello_crosses_the_loopback_listener_unchanged() {
    let capture = Capture::bind().await.expect("binds");
    let sent = PAGE_HELLOS[0].to_vec();
    let client = tokio::spawn(send_slowly(capture.port(), sent.clone()));

    let captured = tokio::time::timeout(WITHIN, capture.accept_client_hello())
        .await
        .expect("a client connects")
        .expect("a hello");

    assert_eq!(captured.raw, sent);
    assert_eq!(captured.hello, parse(PAGE_HELLOS[0]));
    client.await.expect("the client task ends");
}
