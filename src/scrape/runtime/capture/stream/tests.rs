//! Tests of the first-message readers, fed from memory.

use tokio::io::{duplex, DuplexStream};

use super::*;
use crate::wire::akamai::Pseudo;
use crate::wire::client_hello::fixtures::HelloBuilder;
use crate::wire::client_hello::ClientHelloError;
use crate::wire::h2_frames::CLIENT_PREFACE;

const WITHIN: Duration = Duration::from_secs(2);

/// A stream that delivers `bytes` in pieces of `chunk` bytes. With `keep_open` it then stays open
/// and silent; without it the stream ends.
fn feed(bytes: Vec<u8>, chunk: usize, keep_open: bool) -> DuplexStream {
    let (mut client, server) = duplex(64);
    tokio::spawn(async move {
        for piece in bytes.chunks(chunk) {
            if client.write_all(piece).await.is_err() {
                return;
            }
            tokio::task::yield_now().await;
        }
        if keep_open {
            std::future::pending::<()>().await;
        }
    });
    server
}

fn chrome_like_record() -> Vec<u8> {
    HelloBuilder::chrome_like().with_grease().record()
}

#[tokio::test]
async fn a_hello_in_one_record_is_returned_with_nothing_after_it() {
    let record = chrome_like_record();
    let mut sent = record.clone();
    sent.extend_from_slice(b"trailing bytes of a later record");
    let mut stream = feed(sent, 4096, false);

    let (raw, hello) = read_client_hello(&mut stream, WITHIN)
        .await
        .expect("a complete hello");

    assert_eq!(raw, record, "only the records of the hello are returned");
    assert_eq!(hello.sni.as_deref(), Some("example.com"));
    assert!(hello.has_grease());
}

#[tokio::test]
async fn a_hello_split_over_records_and_arriving_in_small_pieces_is_reassembled() {
    let record = HelloBuilder::chrome_like().fragmented(100);
    let mut stream = feed(record.clone(), 7, false);

    let (raw, hello) = read_client_hello(&mut stream, WITHIN)
        .await
        .expect("a fragmented hello");

    assert_eq!(raw, record);
    assert_eq!(hello.sni.as_deref(), Some("example.com"));
}

#[tokio::test]
async fn a_hello_cut_short_reports_how_much_arrived() {
    let record = chrome_like_record();
    let cut = record.get(..40).expect("the record is longer").to_vec();
    let mut stream = feed(cut, 16, false);

    let error = read_client_hello(&mut stream, WITHIN)
        .await
        .expect_err("the stream ends inside the hello");

    assert!(
        matches!(error, CaptureError::Closed { have: 40 }),
        "{error}"
    );
}

#[tokio::test]
async fn plain_http_is_not_a_hello() {
    let mut stream = feed(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n".to_vec(), 64, true);

    let error = read_client_hello(&mut stream, WITHIN)
        .await
        .expect_err("not TLS");

    assert!(
        matches!(error, CaptureError::Hello(ClientHelloError::NotHandshake)),
        "{error}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_silent_connection_times_out() {
    let mut stream = feed(Vec::new(), 1, true);

    let error = read_client_hello(&mut stream, Duration::from_millis(50))
        .await
        .expect_err("nothing is sent");

    assert!(
        matches!(
            error,
            CaptureError::Timeout {
                what: "a ClientHello"
            }
        ),
        "{error}"
    );
}

#[tokio::test]
async fn read_until_stops_a_stream_that_never_satisfies_the_reader() {
    let mut stream = feed(vec![b'a'; 10_000], 1000, true);

    let error = read_until(&mut stream, 3000, WITHIN, "nothing", |_buf: &[u8]| {
        Ok::<Option<()>, CaptureError>(None)
    })
    .await
    .expect_err("the limit is passed");

    assert!(
        matches!(error, CaptureError::TooLarge { limit: 3000 }),
        "{error}"
    );
}

#[tokio::test]
async fn an_http1_request_keeps_header_order_case_and_body() {
    let wire = b"POST /report?x=1 HTTP/1.1\r\nHost: 127.0.0.1:9\r\nuser-agent: Test/1\r\n\
                 Content-Length: 5\r\nAccept-Language:  en-US,en;q=0.9 \r\n\r\nhello"
        .to_vec();
    let mut stream = feed(wire, 9, true);

    let request = read_http1_request(&mut stream, WITHIN)
        .await
        .expect("a request");

    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/report?x=1");
    assert_eq!(request.path(), "/report");
    assert_eq!(request.version, "HTTP/1.1");
    let names: Vec<&str> = request.headers.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        ["Host", "user-agent", "Content-Length", "Accept-Language"]
    );
    assert_eq!(request.header("ACCEPT-LANGUAGE"), Some("en-US,en;q=0.9"));
    assert_eq!(request.body, b"hello");
}

#[tokio::test]
async fn an_http1_request_without_a_body_ends_at_the_blank_line() {
    let mut stream = feed(b"GET / HTTP/1.1\r\nhost: a\r\n\r\n".to_vec(), 64, true);

    let request = read_http1_request(&mut stream, WITHIN)
        .await
        .expect("a request");

    assert_eq!(request.body, Vec::<u8>::new());
    assert_eq!(request.header("missing"), None);
}

#[tokio::test]
async fn an_http1_head_that_never_ends_is_refused() {
    let mut stream = feed(vec![b'a'; MAX_HTTP_HEAD + 4096], 4096, true);

    let error = read_http1_request(&mut stream, WITHIN)
        .await
        .expect_err("the head is too long");

    assert!(
        matches!(
            error,
            CaptureError::TooLarge {
                limit: MAX_HTTP_HEAD
            }
        ),
        "{error}"
    );
}

#[tokio::test]
async fn an_http1_body_above_the_limit_is_refused_before_it_is_read() {
    let wire = format!(
        "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
        MAX_HTTP_BODY + 1
    );
    let mut stream = feed(wire.into_bytes(), 64, true);

    let error = read_http1_request(&mut stream, WITHIN)
        .await
        .expect_err("announced body is too large");

    assert!(
        matches!(
            error,
            CaptureError::TooLarge {
                limit: MAX_HTTP_BODY
            }
        ),
        "{error}"
    );
}

#[tokio::test]
async fn an_http1_body_cut_short_is_an_error() {
    let mut stream = feed(
        b"POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc".to_vec(),
        64,
        false,
    );

    let error = read_http1_request(&mut stream, WITHIN)
        .await
        .expect_err("the body is short");

    assert!(matches!(error, CaptureError::Closed { .. }), "{error}");
}

#[test]
fn malformed_http1_heads_are_refused_with_a_reason() {
    let cases: [(&[u8], &str); 9] = [
        (b"GET /\r\n\r\n", "request line"),
        (b"GET / HTTP/2.0\r\n\r\n", "request line"),
        (b"G ET / HTTP/1.1\r\n\r\n", "request line"),
        (b"GET / HTTP/1.1\r\nno colon here\r\n\r\n", "header line"),
        (b"GET / HTTP/1.1\r\n: empty-name\r\n\r\n", "header name"),
        (b"GET / HTTP/1.1\r\nbad name: v\r\n\r\n", "header name"),
        (b"GET / HTTP/1.1\r\nx: \xff\r\n\r\n", "header is not UTF-8"),
        (b"GET / HTTP/1.1\r\n folded: v\r\n\r\n", "header name"),
        (
            b"GET / HTTP/1.1\r\ntransfer-encoding: chunked\r\n\r\n",
            "chunked bodies",
        ),
    ];
    for (head, reason) in cases {
        let error = read_one(head).expect_err("malformed");
        assert!(
            matches!(&error, CaptureError::Http { reason: got } if *got == reason),
            "{head:?}: {error}"
        );
    }
}

#[test]
fn bad_content_lengths_are_refused() {
    for head in [
        &b"POST / HTTP/1.1\r\ncontent-length: -1\r\n\r\n"[..],
        b"POST / HTTP/1.1\r\ncontent-length: abc\r\n\r\n",
        b"POST / HTTP/1.1\r\ncontent-length: 1\r\ncontent-length: 1\r\n\r\n",
    ] {
        let error = read_one(head).expect_err("bad length");
        assert!(
            matches!(
                error,
                CaptureError::Http {
                    reason: "content-length"
                }
            ),
            "{error}"
        );
    }
}

#[test]
fn too_many_headers_are_refused() {
    let lines: Vec<String> = (0..=MAX_HTTP_HEADERS)
        .map(|index| format!("x-{index}: v"))
        .collect();
    let head = format!("GET / HTTP/1.1\r\n{}\r\n\r\n", lines.join("\r\n"));

    let error = read_one(head.as_bytes()).expect_err("too many");

    assert!(
        matches!(
            error,
            CaptureError::Http {
                reason: "too many headers"
            }
        ),
        "{error}"
    );
}

/// Parses a head through the same path the reader uses, without a stream.
fn read_one(head: &[u8]) -> Result<Http1Request, CaptureError> {
    let parsed = parse_head(head)?;
    Ok(Http1Request {
        method: parsed.method,
        target: parsed.target,
        version: parsed.version,
        headers: parsed.headers,
        body: Vec::new(),
    })
}

#[test]
fn debug_output_hides_header_values() {
    let request = Http1Request {
        method: "GET".to_owned(),
        target: "/".to_owned(),
        version: "HTTP/1.1".to_owned(),
        headers: vec![("Cookie".to_owned(), "session=hunter2".to_owned())],
        body: b"secret body".to_vec(),
    };

    let printed = format!("{request:?}");

    assert!(printed.contains("Cookie"));
    assert!(!printed.contains("hunter2"));
    assert!(!printed.contains("secret body"));
}

/// The bytes of a client that starts HTTP/2 with prior knowledge: preface, SETTINGS,
/// a connection `WINDOW_UPDATE` and a GET request.
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
    // :method GET, :scheme http, :path /, then :authority localhost as a literal.
    let mut block = vec![0x82, 0x86, 0x84, 0x41, 0x09];
    block.extend_from_slice(b"localhost");
    bytes.extend(encode_frame(frame_type::HEADERS, 0x05, 1, &block).expect("frame"));
    bytes
}

#[tokio::test]
async fn an_h2c_start_is_read_up_to_the_first_request() {
    let start = h2c_client_bytes();
    let mut sent = start.clone();
    sent.extend_from_slice(&[0; 9]);
    let mut stream = feed(sent, 11, true);

    let (raw, preface) = read_h2c_preface(&mut stream, WITHIN)
        .await
        .expect("an HTTP/2 start");

    assert_eq!(raw, start, "bytes after the request are not returned");
    assert_eq!(preface.settings, [(3, 100)]);
    assert_eq!(preface.window_update, 15_663_105);
    assert_eq!(preface.request_stream_id, 1);
    assert_eq!(
        preface.pseudo_order().expect("four pseudo-headers"),
        [
            Pseudo::Method,
            Pseudo::Scheme,
            Pseudo::Path,
            Pseudo::Authority
        ]
    );
}

#[tokio::test]
async fn http1_is_not_an_h2c_start() {
    let mut stream = feed(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n".to_vec(), 64, true);

    let error = read_h2c_preface(&mut stream, WITHIN)
        .await
        .expect_err("not HTTP/2");

    assert!(matches!(error, CaptureError::H2(_)), "{error}");
}

#[test]
fn the_h2c_reply_lets_a_client_finish() {
    let reply = h2c_reply(3).expect("a reply");

    let empty_settings = [0, 0, 0, frame_type::SETTINGS, 0, 0, 0, 0, 0];
    let settings_ack = [0, 0, 0, frame_type::SETTINGS, 1, 0, 0, 0, 0];
    let response = [0, 0, 1, frame_type::HEADERS, 0x05, 0, 0, 0, 3, 0x88];
    let mut expected = Vec::new();
    expected.extend_from_slice(&empty_settings);
    expected.extend_from_slice(&settings_ack);
    expected.extend_from_slice(&response);
    assert_eq!(reply, expected);
}

#[test]
fn the_h2c_reply_refuses_a_stream_id_above_31_bits() {
    let error = h2c_reply(u32::MAX).expect_err("too large");

    assert!(matches!(error, CaptureError::H2(_)), "{error}");
}

#[test]
fn an_http1_response_has_a_length_and_closes() {
    let response = http1_response(200, "OK", &[("Content-Type", "text/plain")], b"hi");

    assert_eq!(
        String::from_utf8(response).expect("ASCII"),
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\
         Connection: close\r\n\r\nhi"
    );
}
