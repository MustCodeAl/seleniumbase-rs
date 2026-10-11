//! The capture site against real loopback TLS clients, and the server name reader on fragmented hellos.

use std::pin::Pin;
use std::time::Duration;

use btls::ssl::{SslAlert, SslConnector, SslMethod, SslVerifyError, SslVerifyMode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_btls::SslStream;

use super::{h2_headers, h2_start};
use crate::impersonate::capture_h2::probes::{DOCUMENT, DOCUMENT_BODY, FETCH};
use crate::impersonate::capture_h2::site::{
    h2_reply, peek_server_name, Protocol, Seen, Site, ALPN_H2,
};
use crate::impersonate::capture_h2::CaptureH2Error;
use crate::impersonate::tls_server::{spki_pin_of, ALPN_HTTP1};
use crate::scrape::runtime::capture::CaptureError;
use crate::wire::h2_frames::hpack::HpackDecoder;
use crate::wire::h2_frames::{read_frame, Frame, DEFAULT_MAX_FRAME_PAYLOAD};

/// A first ordinary hello captured from a real Chrome; its server name is `h0.capture.test`.
const FIRST_HELLO: &[u8] =
    include_bytes!("../../../../tests/fixtures/measured/chrome-155-macos.client_hello.1.bin");
/// The server name inside [`FIRST_HELLO`].
const FIRST_HELLO_NAME: &str = "h0.capture.test";
/// How long a test waits for the network.
const WITHIN: Duration = Duration::from_secs(10);
/// A browser's ALPN offer: HTTP/2 first, then HTTP/1.1.
const BROWSER_ALPN: &[u8] = b"\x02h2\x08http/1.1";
/// An offer that has no HTTP/2.
const HTTP1_ONLY_ALPN: &[u8] = b"\x08http/1.1";

/// A client that trusts a chain only when one of its certificates has one of `pins`, the way
/// Chrome's `--ignore-certificate-errors-spki-list` does.
fn pinned_connector(pins: Vec<String>, alpn: &[u8]) -> SslConnector {
    let mut builder = SslConnector::builder(SslMethod::tls()).expect("connector builder");
    builder.set_alpn_protos(alpn).expect("client ALPN offer");
    builder.set_custom_verify_callback(SslVerifyMode::PEER, move |ssl| {
        let refuse = SslVerifyError::Invalid(SslAlert::BAD_CERTIFICATE);
        let chain = ssl.peer_cert_chain().ok_or(refuse)?;
        if chain
            .iter()
            .any(|cert| spki_pin_of(cert).is_ok_and(|pin| pins.contains(&pin)))
        {
            Ok(())
        } else {
            Err(refuse)
        }
    });
    builder.build()
}

/// Connects to the site on `port`, asking for `host`.
async fn connect(
    port: u16,
    host: &str,
    alpn: &[u8],
    pins: Vec<String>,
) -> Result<SslStream<TcpStream>, btls::ssl::Error> {
    let ssl = pinned_connector(pins, alpn)
        .configure()
        .expect("client configuration")
        .use_server_name_indication(true)
        .into_ssl(host)
        .expect("client session");
    let tcp = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect tcp");
    let mut tls = SslStream::new(ssl, tcp).expect("client stream");
    Pin::new(&mut tls).connect().await?;
    Ok(tls)
}

/// Everything the server sends before it closes.
async fn read_all(tls: &mut SslStream<TcpStream>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let outcome = tokio::time::timeout(WITHIN, tls.read_to_end(&mut bytes)).await;
    assert!(outcome.is_ok(), "the server closed the connection in time");
    bytes
}

/// The frames in `bytes`, in order.
fn frames(mut bytes: &[u8]) -> Vec<Frame> {
    let mut found = Vec::new();
    while let Some((frame, used)) = read_frame(bytes, DEFAULT_MAX_FRAME_PAYLOAD).expect("a frame") {
        found.push(frame);
        bytes = bytes.get(used..).unwrap_or_default();
    }
    assert!(bytes.is_empty(), "no partial frame is left over");
    found
}

fn as_pairs(headers: &[(String, String)]) -> Vec<(&str, &str)> {
    headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect()
}

#[tokio::test]
async fn an_http2_navigation_is_recorded_and_answered_with_a_page() {
    let site = Site::bind(Protocol::H2, WITHIN).await.expect("binds");
    let (port, pins) = (site.port(), site.ca().spki_pins().to_vec());
    let headers = h2_headers(DOCUMENT, port);
    let request = h2_start(
        &[(1, 4096), (4, 1_048_576)],
        &as_pairs(&headers),
        Some((0, 255)),
    );
    let sent = request.clone();
    let client = tokio::spawn(async move {
        let mut tls = connect(port, &DOCUMENT.host(), BROWSER_ALPN, pins)
            .await
            .expect("the handshake succeeds");
        assert_eq!(tls.ssl().selected_alpn_protocol(), Some(ALPN_H2));
        tls.write_all(&sent).await.expect("writes the request");
        read_all(&mut tls).await
    });

    let observed = site.collect_until(WITHIN, |seen| !seen.is_empty()).await;
    let reply = client.await.expect("the client finishes");

    assert_eq!(observed.len(), 1);
    let one = observed.first().expect("an observation");
    assert_eq!(one.probe, DOCUMENT);
    let Seen::H2 { raw, preface } = &one.seen else {
        panic!("an HTTP/2 observation, got {:?}", one.seen);
    };
    assert_eq!(raw, &request, "the bytes are the connection start, exactly");
    assert_eq!(
        preface
            .headers
            .iter()
            .map(|f| (f.name.clone(), f.value.clone()))
            .collect::<Vec<_>>(),
        headers
    );
    assert_eq!(reply, h2_reply(1, Some(DOCUMENT_BODY)).expect("builds"));
    assert_eq!(site.ca().cached_leaves(), 1, "one leaf, for the one host");
}

#[tokio::test]
async fn an_http1_request_is_recorded_with_its_header_order_and_case() {
    let site = Site::bind(Protocol::Http1, WITHIN).await.expect("binds");
    let (port, pins) = (site.port(), site.ca().spki_pins().to_vec());
    let client = tokio::spawn(async move {
        let mut tls = connect(port, &FETCH.host(), BROWSER_ALPN, pins)
            .await
            .expect("the handshake succeeds");
        assert_eq!(tls.ssl().selected_alpn_protocol(), Some(ALPN_HTTP1));
        let head = format!(
            "GET /probe HTTP/1.1\r\nHost: {}:{port}\r\nConnection: keep-alive\r\n\
             sec-ch-ua: \"Stand-in\"\r\nUser-Agent: Stand-in/1.0\r\nAccept: */*\r\n\r\n",
            FETCH.host()
        );
        tls.write_all(head.as_bytes()).await.expect("writes");
        read_all(&mut tls).await
    });

    let observed = site.collect_until(WITHIN, |seen| !seen.is_empty()).await;
    let reply = client.await.expect("the client finishes");

    let Some(Seen::Http1 { request }) = observed.first().map(|one| &one.seen) else {
        panic!("an HTTP/1 observation, got {observed:?}");
    };
    assert_eq!(observed.first().map(|one| one.probe), Some(FETCH));
    assert_eq!(
        (request.method.as_str(), request.target.as_str()),
        ("GET", "/probe")
    );
    let names: Vec<&str> = request.headers.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        ["Host", "Connection", "sec-ch-ua", "User-Agent", "Accept"]
    );
    let text = String::from_utf8_lossy(&reply);
    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "{text}");
    assert!(
        text.contains("Content-Length: 0"),
        "only the navigation gets a page: {text}"
    );
}

#[tokio::test]
async fn the_navigation_gets_an_html_page_over_http1() {
    let site = Site::bind(Protocol::Http1, WITHIN).await.expect("binds");
    let (port, pins) = (site.port(), site.ca().spki_pins().to_vec());
    let client = tokio::spawn(async move {
        let mut tls = connect(port, &DOCUMENT.host(), HTTP1_ONLY_ALPN, pins)
            .await
            .expect("the handshake succeeds");
        tls.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .expect("writes");
        read_all(&mut tls).await
    });

    let _observed = site.collect_until(WITHIN, |seen| !seen.is_empty()).await;
    let reply = client.await.expect("the client finishes");

    let text = String::from_utf8_lossy(&reply);
    assert!(text.contains("Content-Type: text/html"), "{text}");
    assert!(
        text.ends_with(&String::from_utf8_lossy(DOCUMENT_BODY).into_owned()),
        "{text}"
    );
}

#[tokio::test]
async fn a_name_outside_the_probe_table_gets_no_certificate() {
    let site = Site::bind(Protocol::H2, WITHIN).await.expect("binds");
    let (port, pins) = (site.port(), site.ca().spki_pins().to_vec());
    let client = tokio::spawn(async move {
        connect(port, "evil.example", BROWSER_ALPN, pins)
            .await
            .map(|_tls| ())
    });

    let observed = site
        .collect_until(Duration::from_millis(800), |_| false)
        .await;

    assert!(
        observed.is_empty(),
        "nothing is recorded for a name outside the table"
    );
    assert!(
        client.await.expect("the client finishes").is_err(),
        "the handshake never completes"
    );
    assert_eq!(
        site.ca().cached_leaves(),
        0,
        "the CA never signed for that name"
    );
}

#[tokio::test]
async fn a_client_without_the_pass_protocol_is_refused() {
    let site = Site::bind(Protocol::H2, WITHIN).await.expect("binds");
    let (port, pins) = (site.port(), site.ca().spki_pins().to_vec());
    let client = tokio::spawn(async move {
        connect(port, &DOCUMENT.host(), HTTP1_ONLY_ALPN, pins)
            .await
            .map(|_tls| ())
    });

    let observed = site
        .collect_until(Duration::from_millis(800), |_| false)
        .await;

    assert!(
        observed.is_empty(),
        "an HTTP/1.1-only client is not an HTTP/2 measurement"
    );
    assert!(client.await.expect("the client finishes").is_err());
}

#[tokio::test]
async fn a_connection_that_says_nothing_is_dropped_after_the_timeout() {
    let site = Site::bind(Protocol::H2, Duration::from_millis(150))
        .await
        .expect("binds");
    let silent = TcpStream::connect(("127.0.0.1", site.port()))
        .await
        .expect("connects");

    let observed = site
        .collect_until(Duration::from_millis(700), |_| false)
        .await;

    assert!(observed.is_empty(), "a silent connection records nothing");
    drop(silent);
}

#[tokio::test]
async fn collecting_stops_at_once_when_nothing_is_wanted() {
    let site = Site::bind(Protocol::H2, WITHIN).await.expect("binds");

    let observed = site.collect_until(WITHIN, |_| true).await;

    assert!(
        observed.is_empty(),
        "nothing was wanted, so nothing was read"
    );
    assert!(format!("{site:?}").contains("H2"));
}

/// A listener, a writer that sends `bytes` in `chunk`-sized pieces, and the accepted side.
async fn fed(
    bytes: &'static [u8],
    chunk: usize,
) -> (TcpStream, tokio::task::JoinHandle<TcpStream>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let addr = listener.local_addr().expect("address");
    let writer = tokio::spawn(async move {
        let mut tcp = TcpStream::connect(addr).await.expect("connects");
        for piece in bytes.chunks(chunk) {
            tcp.write_all(piece).await.expect("writes");
            tcp.flush().await.expect("flushes");
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
        tcp
    });
    let (server_side, _) = listener.accept().await.expect("accepts");
    (server_side, writer)
}

#[tokio::test]
async fn the_server_name_is_read_from_a_hello_that_arrives_in_pieces_and_nothing_is_consumed() {
    let (mut server_side, writer) = fed(FIRST_HELLO, 100).await;

    let name = peek_server_name(&server_side, WITHIN)
        .await
        .expect("the name");
    let _client = writer.await.expect("the writer finishes");

    assert_eq!(name, FIRST_HELLO_NAME);
    let mut again = vec![0_u8; FIRST_HELLO.len()];
    server_side
        .read_exact(&mut again)
        .await
        .expect("reads it all");
    assert_eq!(
        again, FIRST_HELLO,
        "the hello is still there for the handshake"
    );
}

#[tokio::test]
async fn bytes_that_are_not_a_hello_are_refused() {
    let (server_side, writer) = fed(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n", 64).await;

    let error = peek_server_name(&server_side, WITHIN)
        .await
        .expect_err("not TLS");
    let _client = writer.await.expect("the writer finishes");

    assert!(
        matches!(error, CaptureH2Error::Capture(CaptureError::Hello(_))),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_closed_or_silent_connection_is_an_error() {
    let (server_side, writer) = fed(b"", 1).await;
    drop(writer.await.expect("the writer finishes"));
    let closed = peek_server_name(&server_side, WITHIN)
        .await
        .expect_err("closed");
    assert!(
        matches!(
            closed,
            CaptureH2Error::Capture(CaptureError::Closed { have: 0 })
        ),
        "{closed:?}"
    );

    let (silent_side, writer) = fed(b"", 1).await;
    let silent = peek_server_name(&silent_side, Duration::from_millis(100))
        .await
        .expect_err("silent");
    let _client = writer.await.expect("the writer finishes");
    assert!(
        matches!(
            silent,
            CaptureH2Error::Capture(CaptureError::Timeout { .. })
        ),
        "{silent:?}"
    );
}

#[tokio::test]
async fn a_hello_without_a_server_name_is_refused() {
    let (server_side, writer) = fed(HELLO_WITHOUT_NAME, 16).await;

    let error = peek_server_name(&server_side, WITHIN)
        .await
        .expect_err("no name");
    let _client = writer.await.expect("the writer finishes");

    assert!(matches!(error, CaptureH2Error::NoServerName), "{error:?}");
}

/// A well-formed `ClientHello` in one record with `supported_versions` and no `server_name`:
/// TLS 1.2, 32 random bytes, no session id, one cipher suite, null compression, one extension.
const HELLO_WITHOUT_NAME: &[u8] = &[
    0x16, 0x03, 0x01, 0x00, 0x36, // record: handshake, TLS 1.0, 54 bytes
    0x01, 0x00, 0x00, 0x32, // ClientHello, 50 bytes
    0x03, 0x03, // client version
    0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42,
    0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42,
    0x42, // random
    0x00, // session id length
    0x00, 0x02, 0x13, 0x01, // one cipher suite
    0x01, 0x00, // null compression
    0x00, 0x07, // extensions: 7 bytes
    0x00, 0x2b, 0x00, 0x03, 0x02, 0x03, 0x04, // supported_versions: TLS 1.3
];

#[test]
fn the_reply_is_two_settings_frames_and_a_status_200_page() {
    let reply = h2_reply(1, Some(DOCUMENT_BODY)).expect("builds");

    let all = frames(&reply);

    assert_eq!(all.len(), 4, "{all:?}");
    assert!(matches!(&all[0], Frame::Settings { ack: false, settings } if settings.is_empty()));
    assert!(matches!(&all[1], Frame::Settings { ack: true, .. }));
    let Frame::Headers {
        stream_id,
        end_stream,
        end_headers,
        fragment,
        ..
    } = &all[2]
    else {
        panic!("HEADERS, got {:?}", all[2]);
    };
    assert_eq!((*stream_id, *end_stream, *end_headers), (1, false, true));
    let fields = HpackDecoder::new(4096).decode(fragment).expect("decodes");
    let fields: Vec<(&str, &str)> = fields
        .iter()
        .map(|f| (f.name.as_str(), f.value.as_str()))
        .collect();
    assert_eq!(
        fields,
        [
            (":status", "200"),
            ("content-type", "text/html; charset=utf-8")
        ]
    );
    assert!(
        matches!(&all[3], Frame::Other { frame_type: 0, flags: 1, stream_id: 1, len }
        if *len == DOCUMENT_BODY.len())
    );
}

#[test]
fn a_reply_without_a_body_ends_the_stream_in_its_headers() {
    let reply = h2_reply(7, None).expect("builds");

    let all = frames(&reply);

    assert_eq!(all.len(), 3, "{all:?}");
    let Frame::Headers {
        stream_id,
        end_stream,
        fragment,
        ..
    } = &all[2]
    else {
        panic!("HEADERS, got {:?}", all[2]);
    };
    assert_eq!(
        (*stream_id, *end_stream, fragment.as_slice()),
        (7, true, &[0x88_u8][..])
    );
    assert!(matches!(
        h2_reply(u32::MAX, None),
        Err(CaptureH2Error::H2(_))
    ));
}
