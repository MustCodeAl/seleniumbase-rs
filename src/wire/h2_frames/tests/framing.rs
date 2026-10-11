//! Frame-level tests: each frame type on its own, well formed and malformed.

use super::*;

fn framing_error(bytes: &[u8]) -> H2CaptureError {
    read_frame(bytes, DEFAULT_MAX_FRAME_PAYLOAD).expect_err("a malformed frame")
}

#[test]
fn malformed_settings_are_named() {
    let size = |len| H2CaptureError::FrameSize {
        frame_type: frame_type::SETTINGS,
        len,
    };
    assert_eq!(
        framing_error(&frame(frame_type::SETTINGS, 0, 0, &[0; 5])),
        size(5)
    );
    assert_eq!(
        framing_error(&frame(frame_type::SETTINGS, 0, 0, &[0; 7])),
        size(7)
    );
    assert_eq!(
        framing_error(&frame(frame_type::SETTINGS, flag::ACK, 0, &[0; 6])),
        size(6)
    );
    assert_eq!(
        framing_error(&frame(frame_type::SETTINGS, 0, 1, &[])),
        H2CaptureError::StreamId {
            frame_type: frame_type::SETTINGS,
            stream_id: 1
        }
    );
}

#[test]
fn settings_keep_their_order_and_repeats() {
    let bytes = settings_frame(&[(4, 9), (1, 2), (4, 8), (0x0a0a, 7)]);
    let (parsed, _) = read_frame(&bytes, 100).expect("valid").expect("complete");
    assert_eq!(
        parsed,
        Frame::Settings {
            ack: false,
            settings: vec![(4, 9), (1, 2), (4, 8), (0x0a0a, 7)]
        }
    );
    let ack = frame(frame_type::SETTINGS, flag::ACK, 0, &[]);
    let (parsed, _) = read_frame(&ack, 100).expect("valid").expect("complete");
    assert_eq!(
        parsed,
        Frame::Settings {
            ack: true,
            settings: Vec::new()
        }
    );
}

#[test]
fn malformed_window_updates_are_named() {
    let size = |len| H2CaptureError::FrameSize {
        frame_type: frame_type::WINDOW_UPDATE,
        len,
    };
    assert_eq!(
        framing_error(&frame(frame_type::WINDOW_UPDATE, 0, 0, &[0; 3])),
        size(3)
    );
    assert_eq!(
        framing_error(&frame(frame_type::WINDOW_UPDATE, 0, 0, &[0; 5])),
        size(5)
    );
    assert_eq!(
        framing_error(&window_update_frame(0, 0)),
        H2CaptureError::ZeroWindowIncrement
    );
    // Only the reserved bit set: the increment is zero once it is masked.
    assert_eq!(
        framing_error(&window_update_frame(0, 0x8000_0000)),
        H2CaptureError::ZeroWindowIncrement
    );
    let (parsed, _) = read_frame(&window_update_frame(9, 0xffff_ffff), 100)
        .expect("valid")
        .expect("complete");
    assert_eq!(
        parsed,
        Frame::WindowUpdate {
            stream_id: 9,
            increment: MAX_STREAM_ID
        }
    );
}

#[test]
fn malformed_priority_frames_are_named() {
    assert_eq!(
        framing_error(&frame(frame_type::PRIORITY, 0, 3, &[0; 4])),
        H2CaptureError::FrameSize {
            frame_type: frame_type::PRIORITY,
            len: 4
        }
    );
    assert_eq!(
        framing_error(&priority_frame(0, false, 0, 16)),
        H2CaptureError::StreamId {
            frame_type: frame_type::PRIORITY,
            stream_id: 0
        }
    );
}

#[test]
fn priority_weights_are_the_wire_byte_plus_one() {
    for (wire, weight) in [(0_u8, 1_u16), (15, 16), (255, 256)] {
        let mut payload = 3_u32.to_be_bytes().to_vec();
        payload.push(wire);
        let bytes = frame(frame_type::PRIORITY, 0, 5, &payload);
        let (parsed, _) = read_frame(&bytes, 100).expect("valid").expect("complete");
        assert_eq!(
            parsed,
            Frame::Priority(PriorityFrame {
                stream_id: 5,
                exclusive: false,
                depends_on: 3,
                weight
            })
        );
    }
    let bytes = priority_frame(5, true, 3, 16);
    let (parsed, _) = read_frame(&bytes, 100).expect("valid").expect("complete");
    assert!(matches!(
        parsed,
        Frame::Priority(PriorityFrame {
            exclusive: true,
            depends_on: 3,
            ..
        })
    ));
}

#[test]
fn malformed_headers_frames_are_named() {
    let size = |len| H2CaptureError::FrameSize {
        frame_type: frame_type::HEADERS,
        len,
    };
    assert_eq!(
        framing_error(&frame(frame_type::HEADERS, flag::END_HEADERS, 0, &[0x82])),
        H2CaptureError::StreamId {
            frame_type: frame_type::HEADERS,
            stream_id: 0
        }
    );
    assert_eq!(
        framing_error(&frame(frame_type::HEADERS, flag::PADDED, 1, &[])),
        size(0)
    );
    assert_eq!(
        framing_error(&frame(frame_type::HEADERS, flag::PRIORITY, 1, &[0; 4])),
        size(4)
    );
    // Pad length 5 with only 3 bytes of data after it.
    assert_eq!(
        framing_error(&frame(frame_type::HEADERS, flag::PADDED, 1, &[5, 1, 2, 3])),
        H2CaptureError::BadPadding
    );
    // Padding that would reach back into the priority fields.
    let mut payload = vec![3];
    payload.extend(priority_bytes(false, 0, 16));
    assert_eq!(
        framing_error(&frame(
            frame_type::HEADERS,
            flag::PADDED | flag::PRIORITY,
            1,
            &payload[..6]
        )),
        H2CaptureError::BadPadding
    );
    assert_eq!(
        framing_error(&frame(
            frame_type::CONTINUATION,
            flag::END_HEADERS,
            0,
            &[0x82]
        )),
        H2CaptureError::StreamId {
            frame_type: frame_type::CONTINUATION,
            stream_id: 0
        }
    );
}

#[test]
fn padded_headers_with_priority_yield_the_bare_fragment() {
    let block = request_block(&Pseudo::ALL, &[]);
    let mut payload = vec![3];
    payload.extend(priority_bytes(true, 9, 33));
    payload.extend(&block);
    payload.extend([0, 0, 0]);
    let flags = flag::PADDED | flag::PRIORITY | flag::END_HEADERS;
    let bytes = frame(frame_type::HEADERS, flags, 1, &payload);
    let (parsed, _) = read_frame(&bytes, 4096).expect("valid").expect("complete");
    assert_eq!(
        parsed,
        Frame::Headers {
            stream_id: 1,
            end_stream: false,
            end_headers: true,
            priority: Some(HeadersPriority {
                depends_on: 9,
                weight: 33,
                exclusive: true
            }),
            fragment: block,
        }
    );
}
