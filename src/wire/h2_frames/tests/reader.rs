//! The bound on the frames before the first request, and the reader that parses each frame once.

use super::*;

/// `count` frames of an unknown type with no payload.
fn flood(count: usize) -> Vec<Vec<u8>> {
    std::iter::repeat_n(frame(0xaa, 0, 0, &[]), count).collect()
}

fn request() -> Vec<u8> {
    headers_frame(
        1,
        flag::END_HEADERS,
        None,
        &request_block(&Pseudo::ALL, &[]),
    )
}

const TOO_MANY: H2CaptureError = H2CaptureError::TooManyFrames {
    max: MAX_PREFACE_FRAMES,
};

#[test]
fn a_flood_of_empty_frames_before_the_request_is_refused() {
    let mut frames = vec![settings_frame(&SETTINGS)];
    frames.extend(flood(MAX_PREFACE_FRAMES));
    assert_eq!(error_of(&start(&frames)), TOO_MANY);
}

#[test]
fn the_request_may_be_the_last_frame_the_bound_allows() {
    let mut frames = vec![settings_frame(&SETTINGS)];
    frames.extend(flood(MAX_PREFACE_FRAMES - 2));
    frames.push(request());
    assert_eq!(frames.len(), MAX_PREFACE_FRAMES);
    assert_eq!(read(&start(&frames)).request_stream_id, 1);
    frames.insert(1, frame(0xaa, 0, 0, &[]));
    assert_eq!(error_of(&start(&frames)), TOO_MANY);
}

#[test]
fn a_reader_fed_one_byte_at_a_time_parses_each_frame_once() {
    let bytes = standard_bytes();
    let expected = read(&bytes);
    let mut reader = PrefaceReader::new();
    let (mut frames, mut consumed) = (0, 0);
    for end in 0..bytes.len() {
        assert_eq!(reader.read(&bytes[..end]), Ok(None), "{end} bytes");
        assert!(reader.frames_read() >= frames, "frames never go back");
        assert!(
            reader.bytes_read() >= consumed,
            "the offset never goes back"
        );
        assert!(reader.bytes_read() <= end, "only bytes that arrived");
        (frames, consumed) = (reader.frames_read(), reader.bytes_read());
    }
    assert_eq!(reader.read(&bytes), Ok(Some(expected)));
    assert_eq!(reader.frames_read(), standard_frames().len());
    assert_eq!(reader.bytes_read(), bytes.len());
}

#[test]
fn a_reader_that_returned_its_result_or_an_error_is_spent() {
    let bytes = standard_bytes();
    let mut reader = PrefaceReader::new();
    assert!(matches!(reader.read(&bytes), Ok(Some(_))));
    assert_eq!(reader.read(&bytes), Err(H2CaptureError::ReaderFinished));
    let mut refused = PrefaceReader::new();
    let text = b"GET / HTTP/1.1\r\n";
    assert_eq!(refused.read(text), Err(H2CaptureError::BadPreface));
    assert_eq!(refused.read(text), Err(H2CaptureError::ReaderFinished));
}

#[test]
fn the_reader_waits_for_the_whole_magic_before_it_counts_anything() {
    let mut reader = PrefaceReader::new();
    for end in 0..CLIENT_PREFACE.len() {
        assert_eq!(reader.read(&CLIENT_PREFACE[..end]), Ok(None));
        assert_eq!((reader.bytes_read(), reader.frames_read()), (0, 0));
    }
    assert_eq!(reader.read(CLIENT_PREFACE), Ok(None));
    assert_eq!(reader.bytes_read(), CLIENT_PREFACE.len());
}

#[test]
fn a_slow_flood_is_cut_off_at_the_frame_bound() {
    let mut frames = vec![settings_frame(&SETTINGS)];
    frames.extend(flood(MAX_PREFACE_FRAMES * 4));
    let bytes = start(&frames);
    let mut reader = PrefaceReader::new();
    let mut outcome = Ok(None);
    let mut calls = 0;
    for end in 0..=bytes.len() {
        outcome = reader.read(&bytes[..end]);
        calls += 1;
        if outcome.is_err() {
            break;
        }
    }
    assert_eq!(outcome, Err(TOO_MANY));
    assert_eq!(reader.frames_read(), MAX_PREFACE_FRAMES);
    assert!(
        calls <= start(&frames[..=MAX_PREFACE_FRAMES]).len() + 1,
        "the 65th frame ends the read as soon as it is whole: {calls} calls"
    );
}

#[test]
fn the_debug_output_shows_counters_and_no_bytes() {
    let mut reader = PrefaceReader::new();
    assert_eq!(
        reader.read(&standard_bytes()).map(|p| p.is_some()),
        Ok(true)
    );
    let shown = format!("{reader:?}");
    assert!(shown.contains("frames_read: 5") && shown.contains("finished: true"));
    assert!(!shown.contains("example.test"), "{shown}");
}
