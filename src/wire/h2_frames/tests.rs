//! Tests for the preface reader, on hand-built frame sequences.

use super::*;

mod framing;

/// Builds a frame; the arguments in these tests are always valid.
fn frame(kind: u8, flags: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
    encode_frame(kind, flags, stream, payload).expect("a valid frame")
}

fn settings_frame(entries: &[(u16, u32)]) -> Vec<u8> {
    let payload: Vec<u8> = entries
        .iter()
        .flat_map(|(id, value)| {
            let mut entry = id.to_be_bytes().to_vec();
            entry.extend(value.to_be_bytes());
            entry
        })
        .collect();
    frame(frame_type::SETTINGS, 0, 0, &payload)
}

fn window_update_frame(stream: u32, increment: u32) -> Vec<u8> {
    frame(
        frame_type::WINDOW_UPDATE,
        0,
        stream,
        &increment.to_be_bytes(),
    )
}

/// The five priority bytes; `weight` is the protocol weight, 1 to 256.
fn priority_bytes(exclusive: bool, depends_on: u32, weight: u16) -> Vec<u8> {
    let dependency = depends_on | if exclusive { 0x8000_0000 } else { 0 };
    let mut out = dependency.to_be_bytes().to_vec();
    out.push(u8::try_from(weight - 1).expect("weight 1 to 256"));
    out
}

fn priority_frame(stream: u32, exclusive: bool, depends_on: u32, weight: u16) -> Vec<u8> {
    frame(
        frame_type::PRIORITY,
        0,
        stream,
        &priority_bytes(exclusive, depends_on, weight),
    )
}

fn headers_frame(
    stream: u32,
    flags: u8,
    priority: Option<(bool, u32, u16)>,
    block: &[u8],
) -> Vec<u8> {
    let mut payload = Vec::new();
    let mut flags = flags;
    if let Some((exclusive, depends_on, weight)) = priority {
        flags |= flag::PRIORITY;
        payload.extend(priority_bytes(exclusive, depends_on, weight));
    }
    payload.extend_from_slice(block);
    frame(frame_type::HEADERS, flags, stream, &payload)
}

fn string_literal(text: &str, huffman_coded: bool) -> Vec<u8> {
    let (flag, body) = if huffman_coded {
        (0x80, huffman::encode(text.as_bytes()))
    } else {
        (0x00, text.as_bytes().to_vec())
    };
    let mut out = vec![flag | u8::try_from(body.len()).expect("short test string")];
    out.extend(body);
    out
}

/// A literal field with incremental indexing and a new name.
fn literal(name: &str, value: &str, huffman_coded: bool) -> Vec<u8> {
    let mut out = vec![0x40];
    out.extend(string_literal(name, huffman_coded));
    out.extend(string_literal(value, huffman_coded));
    out
}

fn pseudo_value(pseudo: Pseudo) -> &'static str {
    match pseudo {
        Pseudo::Method => "GET",
        Pseudo::Authority => "example.test",
        Pseudo::Scheme => "https",
        Pseudo::Path => "/index",
    }
}

/// A request header block: the pseudo-headers in `order`, then `extra`, alternating coding.
fn request_block(order: &[Pseudo], extra: &[(&str, &str)]) -> Vec<u8> {
    let mut block = Vec::new();
    let pseudo = order.iter().map(|p| (p.header_name(), pseudo_value(*p)));
    for (index, (name, value)) in pseudo.chain(extra.iter().copied()).enumerate() {
        block.extend(literal(name, value, index % 2 == 1));
    }
    block
}

const ORDER: [Pseudo; 4] = [
    Pseudo::Method,
    Pseudo::Scheme,
    Pseudo::Path,
    Pseudo::Authority,
];
const EXTRA: [(&str, &str); 3] = [
    ("user-agent", "TestAgent/1.0 (unit test)"),
    ("accept-language", "en-US,en;q=0.9"),
    ("x-test", "1"),
];
const SETTINGS: [(u16, u32); 3] = [(1, 4096), (2, 0), (4, 1_048_576)];

fn start(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = CLIENT_PREFACE.to_vec();
    for f in frames {
        bytes.extend_from_slice(f);
    }
    bytes
}

/// A client start with every kind of frame the reader reads, ending in a request on stream 7.
fn standard_frames() -> Vec<Vec<u8>> {
    vec![
        settings_frame(&SETTINGS),
        window_update_frame(0, 983_041),
        priority_frame(3, false, 0, 201),
        priority_frame(5, true, 3, 16),
        headers_frame(
            7,
            flag::END_HEADERS | flag::END_STREAM,
            Some((true, 0, 256)),
            &request_block(&ORDER, &EXTRA),
        ),
    ]
}

fn standard_bytes() -> Vec<u8> {
    start(&standard_frames())
}

fn read(bytes: &[u8]) -> H2Preface {
    read_preface(bytes)
        .expect("a valid start")
        .expect("a complete start")
}

fn error_of(bytes: &[u8]) -> H2CaptureError {
    read_preface(bytes).expect_err("an invalid start")
}

#[test]
fn reads_a_whole_client_start() {
    let bytes = standard_bytes();
    let preface = read(&bytes);
    assert_eq!(preface.settings, SETTINGS.to_vec());
    assert_eq!(preface.window_update, 983_041);
    assert_eq!(
        preface.priority_frames,
        vec![
            PriorityFrame {
                stream_id: 3,
                exclusive: false,
                depends_on: 0,
                weight: 201
            },
            PriorityFrame {
                stream_id: 5,
                exclusive: true,
                depends_on: 3,
                weight: 16
            },
        ]
    );
    assert_eq!(
        preface.headers_priority,
        Some(HeadersPriority {
            depends_on: 0,
            weight: 256,
            exclusive: true
        })
    );
    assert_eq!(preface.request_stream_id, 7);
    assert_eq!(preface.consumed, bytes.len());
    let names: Vec<&str> = preface.headers.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            ":method",
            ":scheme",
            ":path",
            ":authority",
            "user-agent",
            "accept-language",
            "x-test"
        ]
    );
    assert_eq!(
        preface.header("User-Agent"),
        Some("TestAgent/1.0 (unit test)")
    );
    assert_eq!(preface.header(":authority"), Some("example.test"));
    assert_eq!(preface.header("missing"), None);
    assert_eq!(preface.regular_headers().count(), 3);
}

#[test]
fn builds_the_akamai_string_of_a_capture() {
    let preface = read(&standard_bytes());
    assert_eq!(
        preface
            .akamai()
            .expect("a full pseudo-header set")
            .to_string(),
        "1:4096;2:0;4:1048576|983041|3:0:0:201,5:1:3:16|m,s,p,a"
    );
    assert_eq!(
        preface.pseudo_order(),
        Ok([
            Pseudo::Method,
            Pseudo::Scheme,
            Pseudo::Path,
            Pseudo::Authority
        ])
    );
}

#[test]
fn every_proper_prefix_asks_for_more_bytes() {
    let bytes = standard_bytes();
    for end in 0..bytes.len() {
        assert_eq!(
            read_preface(&bytes[..end]),
            Ok(None),
            "prefix of {end} bytes"
        );
    }
    assert!(matches!(read_preface(&bytes), Ok(Some(_))));
}

#[test]
fn bytes_after_the_request_are_left_alone() {
    let mut bytes = standard_bytes();
    let used = bytes.len();
    bytes.extend(frame(frame_type::DATA, 0, 7, b"body"));
    assert_eq!(read(&bytes).consumed, used);
}

#[test]
fn something_other_than_the_magic_is_refused_at_once() {
    assert_eq!(read_preface(&[]), Ok(None));
    assert_eq!(read_preface(&CLIENT_PREFACE[..23]), Ok(None));
    assert_eq!(
        read_preface(b"GET / HTTP/1.1\r\n"),
        Err(H2CaptureError::BadPreface)
    );
    assert_eq!(
        read_preface(&[0x16, 0x03, 0x01]),
        Err(H2CaptureError::BadPreface)
    );
    // One wrong byte late in an otherwise short input is found without waiting for more.
    let mut late = CLIENT_PREFACE[..20].to_vec();
    late[19] = b'X';
    assert_eq!(read_preface(&late), Err(H2CaptureError::BadPreface));
}

#[test]
fn an_oversized_frame_is_refused_before_its_payload_arrives() {
    let mut bytes = CLIENT_PREFACE.to_vec();
    bytes.extend([0x00, 0x40, 0x01, frame_type::SETTINGS, 0, 0, 0, 0, 0]); // 16385 bytes
    assert_eq!(
        error_of(&bytes),
        H2CaptureError::FrameTooLarge {
            len: 16_385,
            max: DEFAULT_MAX_FRAME_PAYLOAD
        }
    );
    let header = [0x00, 0x00, 0x65, frame_type::PING, 0, 0, 0, 0, 0]; // 101 bytes
    assert_eq!(
        read_frame(&header, 100),
        Err(H2CaptureError::FrameTooLarge { len: 101, max: 100 })
    );
    assert_eq!(
        read_frame(&header, 101),
        Ok(None),
        "within the limit: wait for the payload"
    );
}

#[test]
fn a_frame_at_the_limit_is_read() {
    let big = frame(0x42, 0, 0, &vec![0; DEFAULT_MAX_FRAME_PAYLOAD]);
    let (parsed, used) = read_frame(&big, DEFAULT_MAX_FRAME_PAYLOAD)
        .expect("within the limit")
        .expect("complete");
    assert_eq!(used, big.len());
    assert_eq!(
        parsed,
        Frame::Other {
            frame_type: 0x42,
            flags: 0,
            stream_id: 0,
            len: DEFAULT_MAX_FRAME_PAYLOAD
        }
    );
}

#[test]
fn a_header_block_split_over_continuation_frames_decodes_whole() {
    let block = request_block(&ORDER, &EXTRA);
    let (a, rest) = block.split_at(5);
    let (b, c) = rest.split_at(11);
    let bytes = start(&[
        settings_frame(&SETTINGS),
        headers_frame(1, 0, None, a),
        frame(frame_type::CONTINUATION, 0, 1, b),
        frame(frame_type::CONTINUATION, flag::END_HEADERS, 1, c),
    ]);
    let preface = read(&bytes);
    assert_eq!(preface.headers.len(), 7);
    assert_eq!(preface.headers_priority, None);
    assert_eq!(preface.request_stream_id, 1);
    for end in 0..bytes.len() {
        assert_eq!(
            read_preface(&bytes[..end]),
            Ok(None),
            "prefix of {end} bytes"
        );
    }
}

#[test]
fn a_header_block_must_not_be_interrupted() {
    let block = request_block(&Pseudo::ALL, &[]);
    let open = headers_frame(1, 0, None, &block[..4]);
    let after = |next: Vec<u8>| error_of(&start(&[settings_frame(&SETTINGS), open.clone(), next]));
    assert_eq!(
        after(frame(frame_type::PING, 0, 0, &[0; 8])),
        H2CaptureError::ExpectedContinuation {
            frame_type: frame_type::PING
        }
    );
    assert_eq!(
        after(frame(
            frame_type::CONTINUATION,
            flag::END_HEADERS,
            3,
            &block[4..]
        )),
        H2CaptureError::ExpectedContinuation {
            frame_type: frame_type::CONTINUATION
        }
    );
    assert_eq!(
        after(headers_frame(3, flag::END_HEADERS, None, &block)),
        H2CaptureError::ExpectedContinuation {
            frame_type: frame_type::HEADERS
        }
    );
}

#[test]
fn frames_that_cannot_come_first_are_refused() {
    let stray = |f: Vec<u8>| error_of(&start(&[settings_frame(&SETTINGS), f]));
    assert_eq!(
        stray(frame(
            frame_type::CONTINUATION,
            flag::END_HEADERS,
            1,
            &[0x82]
        )),
        H2CaptureError::UnexpectedFrame {
            frame_type: frame_type::CONTINUATION
        }
    );
    assert_eq!(
        stray(frame(frame_type::DATA, 0, 1, b"x")),
        H2CaptureError::UnexpectedFrame {
            frame_type: frame_type::DATA
        }
    );
    assert_eq!(
        stray(frame(
            frame_type::PUSH_PROMISE,
            flag::END_HEADERS,
            1,
            &[0; 4]
        )),
        H2CaptureError::UnexpectedFrame {
            frame_type: frame_type::PUSH_PROMISE
        }
    );
}

#[test]
fn irrelevant_frames_are_skipped_and_the_first_values_win() {
    let bytes = start(&[
        settings_frame(&SETTINGS),
        frame(frame_type::SETTINGS, flag::ACK, 0, &[]),
        window_update_frame(0, 983_041),
        window_update_frame(3, 77),
        window_update_frame(0, 5),
        settings_frame(&[(1, 1)]),
        frame(frame_type::PING, 0, 0, &[0; 8]),
        frame(frame_type::RST_STREAM, 0, 3, &[0; 4]),
        frame(frame_type::GOAWAY, 0, 0, &[0; 8]),
        frame(0xfa, 0, 0, b"unknown"),
        priority_frame(9, false, 0, 16),
        headers_frame(
            11,
            flag::END_HEADERS,
            None,
            &request_block(&Pseudo::ALL, &[]),
        ),
    ]);
    let preface = read(&bytes);
    assert_eq!(preface.settings, SETTINGS.to_vec());
    assert_eq!(preface.window_update, 983_041);
    assert_eq!(preface.priority_frames.len(), 1);
    assert_eq!(preface.request_stream_id, 11);
}

#[test]
fn a_request_without_any_settings_is_refused() {
    let bytes = start(&[headers_frame(
        1,
        flag::END_HEADERS,
        None,
        &request_block(&Pseudo::ALL, &[]),
    )]);
    assert_eq!(error_of(&bytes), H2CaptureError::MissingSettings);
}

#[test]
fn no_window_update_means_zero() {
    let bytes = start(&[
        settings_frame(&SETTINGS),
        headers_frame(
            1,
            flag::END_HEADERS,
            None,
            &request_block(&Pseudo::ALL, &[]),
        ),
    ]);
    let preface = read(&bytes);
    assert_eq!(preface.window_update, 0);
    assert_eq!(
        preface.akamai().expect("full set").to_string(),
        "1:4096;2:0;4:1048576|0|0|m,a,s,p"
    );
}

#[test]
fn a_header_block_over_the_limit_is_refused() {
    let chunk = vec![0_u8; DEFAULT_MAX_FRAME_PAYLOAD];
    let mut frames = vec![settings_frame(&SETTINGS), headers_frame(1, 0, None, &chunk)];
    for _ in 0..3 {
        frames.push(frame(frame_type::CONTINUATION, 0, 1, &chunk));
    }
    // Four fragments are exactly the limit; the fifth is over it.
    assert_eq!(read_preface(&start(&frames)), Ok(None));
    frames.push(frame(frame_type::CONTINUATION, 0, 1, &chunk));
    assert_eq!(
        error_of(&start(&frames)),
        H2CaptureError::HeaderBlockTooLarge
    );
}

#[test]
fn a_connection_that_never_sends_a_request_is_cut_off() {
    let mut frames = vec![settings_frame(&SETTINGS)];
    let ping = frame(frame_type::PING, 0, 0, &[0; 8]);
    frames.extend(std::iter::repeat_n(ping, MAX_PREFACE_BYTES / 17 + 2));
    assert_eq!(error_of(&start(&frames)), H2CaptureError::PrefaceTooLarge);
}

#[test]
fn pseudo_header_sets_that_are_not_a_permutation_are_named() {
    let with = |block: Vec<u8>| {
        read(&start(&[
            settings_frame(&SETTINGS),
            headers_frame(1, flag::END_HEADERS, None, &block),
        ]))
    };
    let missing = with(request_block(
        &[Pseudo::Method, Pseudo::Path, Pseudo::Scheme],
        &[],
    ));
    assert_eq!(missing.pseudo_order(), Err(H2CaptureError::PseudoOrder));
    let twice = with(request_block(
        &[Pseudo::Method, Pseudo::Method, Pseudo::Path, Pseudo::Scheme],
        &[],
    ));
    assert_eq!(twice.akamai(), Err(H2CaptureError::PseudoOrder));
    let mut late = request_block(
        &[Pseudo::Method, Pseudo::Path, Pseudo::Scheme],
        &[("x-a", "1")],
    );
    late.extend(literal(":authority", "example.test", false));
    assert_eq!(with(late).pseudo_order(), Err(H2CaptureError::PseudoOrder));
    let response = with(literal(":status", "200", false));
    assert_eq!(response.pseudo_order(), Err(H2CaptureError::PseudoOrder));
    let none = with(literal("x-a", "1", false));
    assert_eq!(none.pseudo_order(), Err(H2CaptureError::PseudoOrder));
}

#[test]
fn a_block_that_does_not_decode_surfaces_the_hpack_error() {
    let bytes = start(&[
        settings_frame(&SETTINGS),
        headers_frame(1, flag::END_HEADERS, None, &[0x80]),
    ]);
    assert_eq!(
        error_of(&bytes),
        H2CaptureError::Hpack(HpackError::IndexZero)
    );
}

#[test]
fn encoding_a_frame_checks_its_limits() {
    assert_eq!(
        encode_frame(frame_type::PING, 0, 0x8000_0000, &[]),
        Err(H2CaptureError::StreamIdRange(0x8000_0000))
    );
    assert_eq!(
        encode_frame(frame_type::DATA, 0, 1, &vec![0; MAX_FRAME_PAYLOAD + 1]),
        Err(H2CaptureError::FrameTooLarge {
            len: MAX_FRAME_PAYLOAD + 1,
            max: MAX_FRAME_PAYLOAD
        })
    );
    let bytes = encode_frame(0x42, 0xa5, 0x7fff_ffff, b"abc").expect("valid");
    assert_eq!(
        bytes[..FRAME_HEADER_LEN],
        [0, 0, 3, 0x42, 0xa5, 0x7f, 0xff, 0xff, 0xff]
    );
    assert_eq!(
        read_frame(&bytes, 16),
        Ok(Some((
            Frame::Other {
                frame_type: 0x42,
                flags: 0xa5,
                stream_id: MAX_STREAM_ID,
                len: 3
            },
            bytes.len()
        )))
    );
}

#[test]
fn a_cut_frame_is_incomplete_whatever_its_type() {
    let frames = [
        settings_frame(&SETTINGS),
        window_update_frame(0, 10),
        priority_frame(3, true, 1, 20),
        headers_frame(1, flag::END_HEADERS, Some((false, 0, 16)), &[0x82]),
        frame(frame_type::CONTINUATION, 0, 1, &[0x82]),
        frame(frame_type::PING, 0, 0, &[0; 8]),
    ];
    for bytes in &frames {
        for end in 0..bytes.len() {
            assert_eq!(
                read_frame(&bytes[..end], 100),
                Ok(None),
                "{end} of {}",
                bytes.len()
            );
        }
        assert!(matches!(read_frame(bytes, 100), Ok(Some(_))));
    }
}

/// A tiny deterministic generator for the mutation loop; no clock, no dependency.
struct XorShift(u64);

impl XorShift {
    fn step(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.step() % u64::try_from(bound).expect("small bound")).expect("fits")
    }
}

#[test]
fn mutated_client_starts_never_panic() {
    let base = standard_bytes();
    let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
    for _ in 0..6000 {
        let mut bytes = base.clone();
        for _ in 0..=rng.below(4) {
            let at = rng.below(bytes.len());
            match rng.below(3) {
                0 => bytes[at] = u8::try_from(rng.below(256)).expect("a byte"),
                1 => bytes.insert(at, u8::try_from(rng.below(256)).expect("a byte")),
                _ => {
                    bytes.remove(at);
                }
            }
        }
        let _outcome = read_preface(&bytes);
    }
}
