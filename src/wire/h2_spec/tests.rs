//! Tests for `H2Spec`: building from captures and strings, and every `SpecError` case.

use super::super::h2_frames::{encode_frame, frame_type};
use super::*;

const STANDARD: &str = "1:4096;2:0;3:100;4:1048576|983041|3:0:0:201,5:1:3:16|m,s,p,a";

fn akamai(text: &str) -> Akamai {
    Akamai::parse(text).expect("a valid Akamai string")
}

fn spec(text: &str) -> H2Spec {
    H2Spec::from_akamai(&akamai(text)).expect("a sendable spec")
}

/// A client start the way a capture tool would see it, with plain-text header fields.
fn client_start(
    settings: &[(u16, u32)],
    window: u32,
    priority: &[u8],
    order: &[Pseudo],
    request_stream: u32,
) -> Vec<u8> {
    let mut bytes = crate::wire::h2_frames::CLIENT_PREFACE.to_vec();
    let entries: Vec<u8> = settings
        .iter()
        .flat_map(|(id, value)| {
            let mut e = id.to_be_bytes().to_vec();
            e.extend(value.to_be_bytes());
            e
        })
        .collect();
    bytes.extend(encode_frame(frame_type::SETTINGS, 0, 0, &entries).expect("frame"));
    if window > 0 {
        bytes.extend(
            encode_frame(frame_type::WINDOW_UPDATE, 0, 0, &window.to_be_bytes()).expect("frame"),
        );
    }
    bytes.extend_from_slice(priority);
    let mut block = Vec::new();
    for pseudo in order {
        block.extend([
            0x00,
            u8::try_from(pseudo.header_name().len()).expect("short"),
        ]);
        block.extend(pseudo.header_name().bytes());
        block.extend([1, b'x']);
    }
    let mut payload = vec![0, 0, 0, 0, 255];
    payload.extend(block);
    bytes.extend(encode_frame(frame_type::HEADERS, 0x24, request_stream, &payload).expect("frame"));
    bytes
}

fn priority_frame_bytes(stream: u32, depends_on: u32, weight_byte: u8) -> Vec<u8> {
    let mut payload = depends_on.to_be_bytes().to_vec();
    payload.push(weight_byte);
    encode_frame(frame_type::PRIORITY, 0, stream, &payload).expect("frame")
}

#[test]
fn a_capture_is_recorded_field_by_field() {
    let mut priority = priority_frame_bytes(3, 0, 200);
    priority.extend(priority_frame_bytes(5, 3, 15));
    let bytes = client_start(
        &[(1, 4096), (2, 0), (4, 1_048_576)],
        983_041,
        &priority,
        &Pseudo::ALL,
        15,
    );
    let spec = H2Spec::from_capture(&bytes).expect("a valid capture");
    assert_eq!(spec.settings, vec![(1, 4096), (2, 0), (4, 1_048_576)]);
    assert_eq!(spec.window_update, 983_041);
    assert_eq!(spec.priority_frames.len(), 2);
    assert_eq!(spec.priority_frames[0].weight, 201);
    assert_eq!(
        spec.headers_priority,
        Some(HeadersPriority {
            depends_on: 0,
            weight: 256,
            exclusive: false
        })
    );
    assert_eq!(spec.pseudo_order, Pseudo::ALL);
    assert_eq!(spec.initial_stream_id, Some(15));
    assert_eq!(spec.check_reproducible(), Ok(()));
}

#[test]
fn a_first_request_on_stream_one_leaves_the_initial_stream_unset() {
    let bytes = client_start(&[(4, 65_535)], 0, &[], &Pseudo::ALL, 1);
    let spec = H2Spec::from_capture(&bytes).expect("valid");
    assert_eq!(spec.initial_stream_id, None);
    assert_eq!(spec.window_update, 0);
}

#[test]
fn a_capture_keeps_what_the_stack_cannot_send() {
    let settings = [(0x0a0a, 1), (1, 4096), (4, 1_048_576)];
    let bytes = client_start(&settings, 0, &[], &Pseudo::ALL, 1);
    let spec = H2Spec::from_capture(&bytes).expect("recorded, not judged");
    assert_eq!(spec.settings[0], (0x0a0a, 1));
    assert_eq!(
        spec.check_reproducible(),
        Err(SpecError::UnsupportedSetting(0x0a0a))
    );
    let (clean, dropped) = spec.without_unsupported_settings();
    assert_eq!(dropped, vec![(0x0a0a, 1)]);
    assert_eq!(clean.settings, vec![(1, 4096), (4, 1_048_576)]);
    assert_eq!(clean.check_reproducible(), Ok(()));
    let (same, none) = clean.without_unsupported_settings();
    assert_eq!((same, none), (clean, Vec::new()));
}

#[test]
fn capture_errors_are_distinguished() {
    let bytes = client_start(&[(4, 1)], 0, &[], &Pseudo::ALL, 1);
    for end in 0..bytes.len() {
        assert_eq!(
            H2Spec::from_capture(&bytes[..end]),
            Err(SpecError::Incomplete),
            "{end}"
        );
    }
    assert!(matches!(
        H2Spec::from_capture(b"GET / HTTP/1.1\r\n\r\n"),
        Err(SpecError::Capture(H2CaptureError::BadPreface))
    ));
    let two = [Pseudo::Method, Pseudo::Path];
    let missing = client_start(&[(4, 1)], 0, &[], &two, 1);
    assert_eq!(H2Spec::from_capture(&missing), Err(SpecError::PseudoOrder));
}

#[test]
fn a_standard_string_makes_a_sendable_spec_that_reads_back_the_same() {
    let s = spec(STANDARD);
    assert_eq!(s.headers_priority, None);
    assert_eq!(
        s.initial_stream_id,
        Some(7),
        "after the highest PRIORITY stream, 5"
    );
    assert_eq!(s.akamai().to_string(), STANDARD);
    assert_eq!(s.setting(4), Some(1_048_576));
    assert_eq!(s.setting(5), None);
}

#[test]
fn a_string_without_priority_frames_uses_the_default_stream() {
    assert_eq!(spec("4:65535|0|0|m,a,s,p").initial_stream_id, None);
}

#[test]
fn overlaying_a_string_keeps_the_headers_priority_of_the_base() {
    let mut base = spec(STANDARD);
    base.headers_priority = Some(HeadersPriority {
        depends_on: 0,
        weight: 256,
        exclusive: true,
    });
    let over = base
        .with_akamai(&akamai(
            "1:65536;4:131072|12517377|3:0:0:201,5:0:0:101,7:0:0:1|m,p,a,s",
        ))
        .expect("sendable");
    assert_eq!(over.headers_priority, base.headers_priority);
    assert_eq!(over.initial_stream_id, Some(9));
    assert_eq!(over.settings, vec![(1, 65_536), (4, 131_072)]);
    assert_eq!(over.window_update, 12_517_377);
    assert_eq!(
        over.pseudo_order,
        [
            Pseudo::Method,
            Pseudo::Path,
            Pseudo::Authority,
            Pseudo::Scheme
        ]
    );
    let none = base
        .with_akamai(&akamai("4:65535|0|0|m,a,s,p"))
        .expect("sendable");
    assert_eq!(
        none.initial_stream_id, None,
        "no PRIORITY frames, so the default stream"
    );
}

#[test]
fn overlaying_an_unsendable_string_fails_and_leaves_the_base_alone() {
    let base = spec(STANDARD);
    let before = base.clone();
    assert_eq!(
        base.with_akamai(&akamai("1:1;4:1;2570:5|0|0|m,a,s,p")),
        Err(SpecError::UnsupportedSetting(2570))
    );
    assert_eq!(base, before);
}

#[test]
fn settings_the_stack_cannot_send_are_named() {
    for id in [0_u16, 7, 10, 0x0a0a, 0x2a2a, 65_535] {
        let a = Akamai {
            settings: vec![(4, 1), (id, 1)],
            ..akamai(STANDARD)
        };
        assert_eq!(
            H2Spec::from_akamai(&a),
            Err(SpecError::UnsupportedSetting(id))
        );
    }
}

#[test]
fn a_repeated_setting_is_refused_not_dropped() {
    let a = Akamai {
        settings: vec![(4, 1), (1, 2), (4, 3)],
        ..akamai(STANDARD)
    };
    assert_eq!(H2Spec::from_akamai(&a), Err(SpecError::DuplicateSetting(4)));
}

#[test]
fn setting_values_follow_rfc_9113() {
    use setting_id as sid;
    let ok = [
        (sid::HEADER_TABLE_SIZE, u32::MAX),
        (sid::ENABLE_PUSH, 0),
        (sid::ENABLE_PUSH, 1),
        (sid::MAX_CONCURRENT_STREAMS, u32::MAX),
        (sid::INITIAL_WINDOW_SIZE, 0),
        (sid::INITIAL_WINDOW_SIZE, 2_147_483_647),
        (sid::MAX_FRAME_SIZE, 16_384),
        (sid::MAX_FRAME_SIZE, 16_777_215),
        (sid::MAX_HEADER_LIST_SIZE, u32::MAX),
        (sid::ENABLE_CONNECT_PROTOCOL, 1),
        (sid::NO_RFC7540_PRIORITIES, 1),
    ];
    let bad = [
        (sid::ENABLE_PUSH, 2),
        (sid::INITIAL_WINDOW_SIZE, 2_147_483_648),
        (sid::MAX_FRAME_SIZE, 16_383),
        (sid::MAX_FRAME_SIZE, 16_777_216),
        (sid::ENABLE_CONNECT_PROTOCOL, 2),
        (sid::NO_RFC7540_PRIORITIES, 2),
    ];
    // The window setting is always present, so it is the base entry unless under test.
    let settings_with = |id: u16, value: u32| {
        if id == sid::INITIAL_WINDOW_SIZE {
            vec![(id, value)]
        } else {
            vec![(sid::INITIAL_WINDOW_SIZE, 1), (id, value)]
        }
    };
    for (id, value) in ok {
        let a = Akamai {
            settings: settings_with(id, value),
            ..akamai(STANDARD)
        };
        assert_eq!(H2Spec::from_akamai(&a).map(|_| ()), Ok(()), "{id}:{value}");
    }
    for (id, value) in bad {
        let a = Akamai {
            settings: settings_with(id, value),
            ..akamai(STANDARD)
        };
        assert_eq!(
            H2Spec::from_akamai(&a),
            Err(SpecError::SettingValue { id, value }),
            "{id}:{value}"
        );
    }
}

#[test]
fn a_setting_the_stack_always_sends_cannot_be_left_out() {
    for text in ["|0|0|m,a,s,p", "1:4096;2:0|0|0|m,a,s,p"] {
        assert_eq!(
            H2Spec::from_akamai(&akamai(text)),
            Err(SpecError::SettingAlwaysSent(4)),
            "{text}"
        );
    }
}

#[test]
fn the_connection_window_may_not_pass_its_maximum() {
    let with = |window| {
        H2Spec::from_akamai(&Akamai {
            window_update: window,
            ..akamai(STANDARD)
        })
    };
    assert!(with(MAX_WINDOW_UPDATE).is_ok());
    assert_eq!(
        with(MAX_WINDOW_UPDATE + 1),
        Err(SpecError::WindowUpdate(MAX_WINDOW_UPDATE + 1))
    );
    assert_eq!(
        with(MAX_STREAM_ID),
        Err(SpecError::WindowUpdate(MAX_STREAM_ID))
    );
    assert_eq!(MAX_WINDOW_UPDATE, 2_147_418_112);
}

/// Checks PRIORITY frames alone: the first-request stream id is left at its default.
fn priority_reason(frames: Vec<PriorityFrame>) -> Result<(), SpecError> {
    let spec = H2Spec {
        priority_frames: frames,
        initial_stream_id: None,
        ..spec(STANDARD)
    };
    spec.check_reproducible()
}

fn frame(stream_id: u32, exclusive: bool, depends_on: u32, weight: u16) -> PriorityFrame {
    PriorityFrame {
        stream_id,
        exclusive,
        depends_on,
        weight,
    }
}

#[test]
fn priority_frames_the_stack_would_drop_or_that_are_invalid_are_named() {
    let cases = [
        (frame(0, false, 0, 16), "stream id 0 is the connection"),
        (frame(4, false, 0, 16), "client streams have odd ids"),
        (frame(3, false, 3, 16), "a stream cannot depend on itself"),
        (frame(3, false, 0, 0), "weight outside 1 to 256"),
        (frame(3, false, 0, 257), "weight outside 1 to 256"),
        (
            frame(3, false, 2_147_483_648, 16),
            "dependency above 2^31 - 1",
        ),
        (
            frame(2_147_483_649, false, 0, 16),
            "stream id above 2^31 - 1",
        ),
    ];
    for (bad, reason) in cases {
        assert_eq!(
            priority_reason(vec![frame(1, false, 0, 16), bad]),
            Err(SpecError::Priority {
                stream: bad.stream_id,
                reason
            }),
            "{bad:?}"
        );
    }
    assert_eq!(
        priority_reason(vec![
            frame(3, false, 0, 16),
            frame(5, false, 1, 16),
            frame(3, true, 0, 32)
        ]),
        Err(SpecError::Priority {
            stream: 3,
            reason: "more than one PRIORITY frame for the stream"
        })
    );
}

#[test]
fn priority_frame_bounds_are_accepted() {
    assert_eq!(
        priority_reason(vec![
            frame(1, true, 2_147_483_647, 1),
            frame(2_147_483_647, false, 0, 256)
        ]),
        Ok(())
    );
}

#[test]
fn too_many_priority_frames_are_refused() {
    let at_cap: Vec<PriorityFrame> = (0..64).map(|i| frame(2 * i + 1, false, 0, 16)).collect();
    assert_eq!(priority_reason(at_cap.clone()), Ok(()));
    let mut over = at_cap;
    over.push(frame(129, false, 0, 16));
    assert_eq!(
        priority_reason(over),
        Err(SpecError::Priority {
            stream: 129,
            reason: "too many PRIORITY frames"
        })
    );
}

#[test]
fn headers_priority_must_be_in_range() {
    let mut s = spec(STANDARD);
    for ok in [(0, 1), (0, 256), (2_147_483_647, 100)] {
        s.headers_priority = Some(HeadersPriority {
            depends_on: ok.0,
            weight: ok.1,
            exclusive: true,
        });
        assert_eq!(s.check_reproducible(), Ok(()), "{ok:?}");
    }
    for (depends_on, weight, reason) in [
        (0, 0, "weight outside 1 to 256"),
        (0, 257, "weight outside 1 to 256"),
        (2_147_483_648, 16, "dependency above 2^31 - 1"),
    ] {
        s.headers_priority = Some(HeadersPriority {
            depends_on,
            weight,
            exclusive: false,
        });
        assert_eq!(
            s.check_reproducible(),
            Err(SpecError::HeadersPriority { reason })
        );
    }
}

#[test]
fn the_pseudo_order_must_be_a_permutation() {
    let mut s = spec(STANDARD);
    s.pseudo_order = [Pseudo::Method, Pseudo::Method, Pseudo::Scheme, Pseudo::Path];
    assert_eq!(s.check_reproducible(), Err(SpecError::PseudoOrder));
}

#[test]
fn the_initial_stream_must_be_an_odd_client_stream() {
    let mut s = spec(STANDARD);
    for ok in [1, 7, 15, 2_147_483_647] {
        s.initial_stream_id = Some(ok);
        assert_eq!(s.check_reproducible(), Ok(()), "{ok}");
    }
    for bad in [0, 2, 8, 2_147_483_648, u32::MAX] {
        s.initial_stream_id = Some(bad);
        assert_eq!(
            s.check_reproducible(),
            Err(SpecError::InitialStreamId(bad)),
            "{bad}"
        );
    }
    let near_the_top = Akamai {
        priority_frames: vec![frame(2_147_483_647, false, 0, 16)],
        ..akamai(STANDARD)
    };
    assert_eq!(
        H2Spec::from_akamai(&near_the_top),
        Err(SpecError::InitialStreamId(2_147_483_649))
    );
}

#[test]
fn problems_are_reported_in_a_fixed_order() {
    // Settings first, then the window, the PRIORITY frames, HEADERS, pseudo, stream id.
    let mut s = spec(STANDARD);
    s.settings.push((7, 1));
    s.window_update = u32::MAX;
    assert_eq!(
        s.check_reproducible(),
        Err(SpecError::UnsupportedSetting(7))
    );
    s.settings.pop();
    assert_eq!(
        s.check_reproducible(),
        Err(SpecError::WindowUpdate(u32::MAX))
    );
}

#[test]
fn json_round_trip_and_defaults() {
    let mut s = spec(STANDARD);
    s.headers_priority = Some(HeadersPriority {
        depends_on: 0,
        weight: 256,
        exclusive: true,
    });
    let json = serde_json::to_string(&s).expect("serialises");
    assert_eq!(
        serde_json::from_str::<H2Spec>(&json).expect("deserialises"),
        s
    );
    let bare = r#"{"settings":[[4,65535]],"window_update":0,"pseudo_order":["method","authority","scheme","path"]}"#;
    let parsed: H2Spec = serde_json::from_str(bare).expect("defaults fill the optional fields");
    assert_eq!(
        (
            parsed.priority_frames.len(),
            parsed.headers_priority,
            parsed.initial_stream_id
        ),
        (0, None, None)
    );
    assert!(!serde_json::to_string(&parsed)
        .expect("serialises")
        .contains("headers_priority"));
    let typo = r#"{"settings":[],"window_update":0,"pseudo_order":["method","authority","scheme","path"],"windows_update":1}"#;
    assert!(
        serde_json::from_str::<H2Spec>(typo).is_err(),
        "unknown fields are errors"
    );
}

#[test]
fn the_plan_name_is_an_alias() {
    let s: Http2Spec = spec(STANDARD);
    assert_eq!(s, spec(STANDARD));
}
