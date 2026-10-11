//! The bounds on how a `ClientHello` may arrive: the records it spans and the work a slow reader
//! costs.

use super::fixtures::{HelloBuilder, CHROME_CIPHERS, RECORD_VERSION};
use super::{ext_id, ClientHello, ClientHelloError, MAX_HELLO_RECORDS};

/// A padded hello, long enough to span more than [`MAX_HELLO_RECORDS`] one-byte records.
fn long_hello() -> HelloBuilder {
    HelloBuilder::new(&CHROME_CIPHERS, &[ext_id::PADDING])
        .with_body(ext_id::PADDING, vec![0; 2 * MAX_HELLO_RECORDS])
}

/// `message` in exactly `count` handshake records: all but the last carry one byte each.
fn in_records(message: &[u8], count: usize) -> Vec<u8> {
    let (singles, last) = message.split_at(count - 1);
    let mut out = Vec::new();
    for part in singles.chunks(1).chain([last]) {
        out.push(22);
        out.extend_from_slice(&RECORD_VERSION.to_be_bytes());
        out.extend_from_slice(&u16::try_from(part.len()).expect("a record").to_be_bytes());
        out.extend_from_slice(part);
    }
    out
}

#[test]
fn a_hello_over_the_record_limit_is_refused_and_one_at_it_is_read() {
    let message = long_hello().message();
    let whole = ClientHello::parse(&long_hello().record()).expect("one record parses");
    let at_limit = in_records(&message, MAX_HELLO_RECORDS);
    // Not assert_eq: a failure would print the 2 KiB of padding twice.
    assert!(ClientHello::parse(&at_limit) == Ok(whole), "the same hello");
    assert_eq!(ClientHello::needed_len(&at_limit), Ok(Some(at_limit.len())));
    let over = in_records(&message, MAX_HELLO_RECORDS + 1);
    assert_eq!(
        ClientHello::parse(&over),
        Err(ClientHelloError::TooManyRecords)
    );
    assert_eq!(
        ClientHello::needed_len(&over),
        Err(ClientHelloError::TooManyRecords)
    );
}

#[test]
fn the_record_limit_is_found_as_soon_as_the_bytes_reach_it() {
    let over = in_records(&long_hello().message(), MAX_HELLO_RECORDS + 1);
    let record_len = 6;
    let limit_end = record_len * MAX_HELLO_RECORDS;
    // The last record header the reader can look at before it must give up is the 1025th.
    assert_eq!(
        ClientHello::needed_len(&over[..limit_end + 4]),
        Ok(None),
        "four of the five header bytes of the next record"
    );
    assert_eq!(
        ClientHello::needed_len(&over[..limit_end + 5]),
        Err(ClientHelloError::TooManyRecords)
    );
}

#[test]
fn a_slow_drip_reads_the_same_answer_at_every_length() {
    let message = HelloBuilder::chrome_like().message();
    let bytes = in_records(&message, 40);
    let mut known = None;
    for end in 0..=bytes.len() {
        match ClientHello::needed_len(&bytes[..end]) {
            Ok(Some(total)) => {
                assert_eq!(total, bytes.len(), "{end} bytes");
                known.get_or_insert(end);
            }
            Ok(None) => assert!(known.is_none(), "{end} bytes: the answer went back to none"),
            Err(error) => panic!("{end} bytes: {error}"),
        }
    }
    assert!(known.is_some_and(|end| end < bytes.len()));
}
