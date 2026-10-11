//! The capture listeners against real loopback sockets, and the committed Chrome measurement
//! against its own raw bytes. None of it needs a browser.
#![cfg(feature = "scrape")]

use std::time::Duration;

use seleniumbase_rs::scrape::runtime::capture::stream::{h2c_reply, http1_response};
use seleniumbase_rs::scrape::runtime::capture::{Capture, CaptureError};
use seleniumbase_rs::wire::h2_frames::{encode_frame, frame_type, CLIENT_PREFACE};
use seleniumbase_rs::wire::ja4::Ja4;
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
