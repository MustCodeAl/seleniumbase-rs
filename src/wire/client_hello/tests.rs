//! Tests of the `ClientHello` parser: typed fields, fragmentation, rejection of malformed input.

use super::fixtures::{
    key_share_body, length16, record_of_body, server_name_body, strings8, u16s, wrap_records,
    HelloBuilder, CHROME_CIPHERS, CHROME_SIGALGS, GREASE_CIPHER, GREASE_EXT_FIRST, GREASE_EXT_LAST,
    GREASE_GROUP, GREASE_VERSION, JA4_EXAMPLE_EXTENSIONS, RECORD_VERSION,
};
use super::{ext_id, is_grease, without_grease, ClientHello, ClientHelloError, MAX_CLIENT_HELLO};

/// Bytes before the handshake length is known: five record bytes and four handshake bytes.
const HEADERS_LEN: usize = 9;

fn parse(bytes: &[u8]) -> Result<ClientHello, ClientHelloError> {
    ClientHello::parse(bytes)
}

#[test]
fn parses_the_typed_fields_of_a_chrome_like_hello() {
    let hello = parse(&HelloBuilder::chrome_like().record()).expect("a valid hello parses");
    assert_eq!(hello.record_version, RECORD_VERSION);
    assert_eq!(hello.legacy_version, 0x0303);
    assert_eq!(hello.random, [0x42; 32]);
    assert_eq!(hello.session_id, vec![0x11; 32]);
    assert_eq!(hello.cipher_suites, CHROME_CIPHERS);
    assert_eq!(hello.compression_methods, vec![0]);
    assert_eq!(hello.extension_ids(), JA4_EXAMPLE_EXTENSIONS);
    assert_eq!(hello.sni.as_deref(), Some("example.com"));
    assert_eq!(hello.alpn, vec![b"h2".to_vec(), b"http/1.1".to_vec()]);
    assert_eq!(hello.supported_versions, vec![0x0304, 0x0303]);
    assert_eq!(hello.supported_groups, vec![0x001D, 0x0017, 0x0018]);
    assert_eq!(hello.key_share_groups, vec![0x001D]);
    assert_eq!(hello.point_formats, vec![0]);
    assert_eq!(hello.signature_algorithms, CHROME_SIGALGS);
    assert_eq!(hello.psk_modes, vec![1]);
    assert_eq!(hello.cert_compression, vec![2]);
    assert_eq!(hello.alps, vec![b"h2".to_vec()]);
    assert!(!hello.has_grease());
}

#[test]
fn decodes_the_remaining_typed_extensions() {
    let builder = HelloBuilder::new(
        &CHROME_CIPHERS,
        &[
            ext_id::RECORD_SIZE_LIMIT,
            ext_id::DELEGATED_CREDENTIAL,
            ext_id::ALPS,
        ],
    );
    let hello = parse(&builder.record()).expect("a valid hello parses");
    assert_eq!(hello.record_size_limit, Some(0x4001));
    assert_eq!(hello.delegated_credentials, vec![0x0403]);
    assert_eq!(hello.alps, vec![b"h2".to_vec()]);
    assert!(hello.has_extension(ext_id::ALPS));
    assert!(!hello.has_extension(ext_id::ALPN));
    assert_eq!(hello.extension(ext_id::ALPS).map(|e| e.data.len()), Some(5));
}

#[test]
fn grease_values_are_kept_and_detected() {
    let hello = parse(&HelloBuilder::chrome_like().with_grease().record()).expect("parses");
    assert!(hello.has_grease());
    assert_eq!(hello.cipher_suites.first(), Some(&GREASE_CIPHER));
    assert_eq!(hello.extension_ids().first(), Some(&GREASE_EXT_FIRST));
    assert_eq!(hello.extension_ids().last(), Some(&GREASE_EXT_LAST));
    assert_eq!(hello.supported_groups.first(), Some(&GREASE_GROUP));
    assert_eq!(hello.supported_versions.first(), Some(&GREASE_VERSION));
    assert_eq!(hello.key_share_groups, vec![GREASE_CIPHER, 0x001D]);
    assert_eq!(without_grease(&hello.cipher_suites), CHROME_CIPHERS);
    assert!(is_grease(GREASE_CIPHER) && !is_grease(0x1301) && !is_grease(0x0A1A));
}

#[test]
fn needed_len_is_none_until_both_headers_are_in_then_the_total() {
    let bytes = HelloBuilder::chrome_like().record();
    for len in 0..HEADERS_LEN {
        assert_eq!(
            ClientHello::needed_len(&bytes[..len]),
            Ok(None),
            "{len} bytes"
        );
    }
    for len in HEADERS_LEN..=bytes.len() {
        assert_eq!(
            ClientHello::needed_len(&bytes[..len]),
            Ok(Some(bytes.len())),
            "{len} bytes"
        );
    }
}

#[test]
fn every_truncation_of_a_valid_hello_is_a_truncation_error() {
    for chunk in [usize::MAX, 100, 7, 3, 1] {
        let bytes = HelloBuilder::chrome_like().fragmented(chunk);
        for len in 0..bytes.len() {
            match parse(&bytes[..len]) {
                Err(ClientHelloError::Truncated { need, have }) => {
                    assert_eq!(have, len, "chunk {chunk}");
                    assert!(need > len, "chunk {chunk}, {len} bytes");
                }
                other => panic!("chunk {chunk}, {len} bytes: {other:?}"),
            }
        }
        assert!(parse(&bytes).is_ok(), "chunk {chunk}");
    }
}

#[test]
fn a_hello_split_over_records_equals_the_single_record_hello() {
    let whole = parse(&HelloBuilder::chrome_like().record()).expect("parses");
    for chunk in [1, 2, 3, 4, 5, 50, 100, 600] {
        let bytes = HelloBuilder::chrome_like().fragmented(chunk);
        assert_eq!(parse(&bytes), Ok(whole.clone()), "chunk {chunk}");
        assert_eq!(
            ClientHello::needed_len(&bytes),
            Ok(Some(bytes.len())),
            "chunk {chunk}"
        );
    }
}

#[test]
fn needed_len_waits_for_the_next_record_header_of_a_fragmented_hello() {
    let builder = HelloBuilder::chrome_like();
    let bytes = builder.fragmented(100);
    let records = builder.message().len().div_ceil(100);
    let last_start = 105 * (records - 1);
    assert_eq!(ClientHello::needed_len(&bytes[..105]), Ok(None));
    assert_eq!(ClientHello::needed_len(&bytes[..last_start]), Ok(None));
    assert_eq!(ClientHello::needed_len(&bytes[..last_start + 4]), Ok(None));
    assert_eq!(
        ClientHello::needed_len(&bytes[..last_start + 5]),
        Ok(Some(bytes.len()))
    );
}

#[test]
fn bytes_after_the_hello_are_ignored() {
    let hello = HelloBuilder::chrome_like().record();
    let mut bytes = hello.clone();
    bytes.extend_from_slice(&[20, 3, 3, 0, 1, 1]);
    assert_eq!(parse(&bytes), parse(&hello));
    assert_eq!(ClientHello::needed_len(&bytes), Ok(Some(hello.len())));
}

#[test]
fn rejects_what_is_not_a_handshake_record() {
    let cases: [&[u8]; 4] = [
        &[23, 3, 3, 0, 5, 1, 0, 0, 1, 0],
        &[0x80, 0x0C, 0x01, 0x00, 0x02, 0, 0, 0, 0, 0],
        &[22, 4, 0, 0, 5, 1, 0, 0, 1, 0],
        &[22, 0, 3, 0, 5, 1, 0, 0, 1, 0],
    ];
    for bytes in cases {
        assert_eq!(
            parse(bytes),
            Err(ClientHelloError::NotHandshake),
            "{bytes:?}"
        );
        assert_eq!(
            ClientHello::needed_len(bytes),
            Err(ClientHelloError::NotHandshake)
        );
    }
}

#[test]
fn rejects_other_handshake_messages() {
    let mut message = HelloBuilder::chrome_like().message();
    message[0] = 2;
    let bytes = wrap_records(&message, usize::MAX);
    assert_eq!(parse(&bytes), Err(ClientHelloError::NotClientHello(2)));
}

#[test]
fn rejects_bad_record_lengths() {
    assert_eq!(parse(&[22, 3, 1, 0, 0]), Err(ClientHelloError::Length));
    assert_eq!(
        parse(&[22, 3, 1, 0x40, 0x01, 1]),
        Err(ClientHelloError::Length)
    );
}

#[test]
fn enforces_the_size_cap_on_the_declared_message_length() {
    let over = MAX_CLIENT_HELLO - 3;
    let header = |len: usize| {
        let bytes = u32::try_from(len).expect("fits").to_be_bytes();
        vec![22, 3, 1, 0, 4, 1, bytes[1], bytes[2], bytes[3]]
    };
    assert_eq!(parse(&header(over)), Err(ClientHelloError::TooLarge));
    assert_eq!(
        ClientHello::needed_len(&header(over)),
        Err(ClientHelloError::TooLarge)
    );
    assert_eq!(ClientHello::needed_len(&header(over - 1)), Ok(None));
}

/// A minimal body: version, random, empty session id, two ciphers, one compression method.
fn minimal_body() -> Vec<u8> {
    let mut body = vec![3, 3];
    body.extend_from_slice(&[7; 32]);
    body.push(0);
    body.extend(length16(&u16s(&[0x002F])));
    body.extend_from_slice(&[1, 0]);
    body
}

fn with_extensions(mut body: Vec<u8>, block: &[u8]) -> Vec<u8> {
    body.extend(length16(block));
    body
}

fn extension(id: u16, data: &[u8]) -> Vec<u8> {
    let mut out = id.to_be_bytes().to_vec();
    out.extend(length16(data));
    out
}

#[test]
fn a_hello_without_an_extension_block_parses() {
    let hello = parse(&record_of_body(&minimal_body())).expect("a TLS 1.2 style hello parses");
    assert_eq!(hello.cipher_suites, vec![0x002F]);
    assert!(hello.extensions.is_empty());
    assert!(hello.sni.is_none() && hello.alpn.is_empty());
}

#[test]
fn rejects_inconsistent_message_lengths() {
    let mut long_session = minimal_body();
    long_session[34] = 33;
    long_session.extend_from_slice(&[0; 40]);
    let mut odd_ciphers = minimal_body();
    odd_ciphers[36] = 1;
    let mut no_ciphers = vec![3, 3];
    no_ciphers.extend_from_slice(&[7; 32]);
    no_ciphers.extend_from_slice(&[0, 0, 0, 1, 0]);
    let missing_compression = minimal_body()[..minimal_body().len() - 2].to_vec();
    let short = vec![3, 3, 0, 0];
    let overlong_block = {
        let mut body = minimal_body();
        body.extend_from_slice(&[0, 9, 0, 0]);
        body
    };
    let trailing: Vec<u8> = with_extensions(minimal_body(), &[])
        .into_iter()
        .chain([9])
        .collect();
    let bad_inner = with_extensions(minimal_body(), &[0, 1, 0, 5, 1]);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("session id over 32 bytes", long_session),
        ("odd cipher suite vector", odd_ciphers),
        ("empty cipher suite vector", no_ciphers),
        ("no compression methods", missing_compression),
        ("body shorter than the random", short),
        ("extension block longer than the body", overlong_block),
        ("bytes after the extension block", trailing),
        ("extension longer than the block", bad_inner),
    ];
    for (label, body) in cases {
        assert_eq!(
            parse(&record_of_body(&body)),
            Err(ClientHelloError::Length),
            "{label}"
        );
    }
}

#[test]
fn rejects_malformed_extension_bodies_with_the_extension_id() {
    let cases: Vec<(u16, Vec<u8>)> = vec![
        (ext_id::SERVER_NAME, vec![0, 9, 0, 0, 1, b'a']),
        (ext_id::SERVER_NAME, length16(&[0, 0, 2, 0xFF, 0xFE])),
        (ext_id::SUPPORTED_GROUPS, length16(&[0, 1, 0])),
        (ext_id::EC_POINT_FORMATS, vec![2, 0]),
        (ext_id::SIGNATURE_ALGORITHMS, vec![0, 2, 4]),
        (ext_id::ALPN, length16(&[2, b'h'])),
        (ext_id::SUPPORTED_VERSIONS, vec![3, 3, 4, 3]),
        (ext_id::KEY_SHARE, length16(&[0, 0x1D, 0, 32, 1])),
        (ext_id::PSK_KEY_EXCHANGE_MODES, vec![]),
        (ext_id::COMPRESS_CERTIFICATE, vec![1, 0]),
        (ext_id::RECORD_SIZE_LIMIT, vec![1, 2, 3]),
        (ext_id::DELEGATED_CREDENTIAL, length16(&[4])),
        (ext_id::ALPS, length16(&[5, 1])),
    ];
    for (id, data) in cases {
        let body = with_extensions(minimal_body(), &extension(id, &data));
        assert_eq!(
            parse(&record_of_body(&body)),
            Err(ClientHelloError::Extension { id }),
            "extension {id:#06x}"
        );
    }
}

#[test]
fn the_first_occurrence_of_a_repeated_extension_wins() {
    let mut block = extension(ext_id::SERVER_NAME, &server_name_body("first.test"));
    block.extend(extension(
        ext_id::SERVER_NAME,
        &server_name_body("second.test"),
    ));
    block.extend(extension(ext_id::KEY_SHARE, &key_share_body(&[0x0017])));
    let hello = parse(&record_of_body(&with_extensions(minimal_body(), &block))).expect("parses");
    assert_eq!(hello.sni.as_deref(), Some("first.test"));
    assert_eq!(hello.extensions.len(), 3);
    assert_eq!(hello.key_share_groups, vec![0x0017]);
}

#[test]
fn unknown_extensions_are_kept_raw_and_the_rest_still_decodes() {
    let mut block = extension(0xCA34, &[1, 2, 3]);
    block.extend(extension(ext_id::ALPN, &length16(&strings8(&["h2"]))));
    let hello = parse(&record_of_body(&with_extensions(minimal_body(), &block))).expect("parses");
    assert_eq!(
        hello.extension(0xCA34).map(|e| e.data.clone()),
        Some(vec![1, 2, 3])
    );
    assert_eq!(hello.alpn, vec![b"h2".to_vec()]);
}

#[test]
fn only_host_names_feed_the_sni_field() {
    let mut names = vec![1];
    names.extend(length16(b"ignored"));
    names.push(0);
    names.extend(length16(b"host.test"));
    let block = extension(ext_id::SERVER_NAME, &length16(&names));
    let hello = parse(&record_of_body(&with_extensions(minimal_body(), &block))).expect("parses");
    assert_eq!(hello.sni.as_deref(), Some("host.test"));
}

/// A small deterministic generator, so the mutation tests are reproducible.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(bound).expect("small bound")).expect("fits")
    }
}

#[test]
fn every_single_byte_mutation_returns_without_panicking() {
    let valid = HelloBuilder::chrome_like().with_grease().record();
    for index in 0..valid.len() {
        for value in [0x00, 0x01, 0x7F, 0x80, 0xFF, valid[index] ^ 0x5A] {
            let mut bytes = valid.clone();
            bytes[index] = value;
            let _parsed = parse(&bytes);
            let _needed = ClientHello::needed_len(&bytes);
        }
    }
}

#[test]
fn seeded_multi_byte_mutations_never_panic() {
    let valid = HelloBuilder::chrome_like().fragmented(120);
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for _ in 0..4000 {
        let mut bytes = valid.clone();
        for _ in 0..=rng.below(4) {
            let at = rng.below(bytes.len());
            match rng.below(5) {
                0 => bytes[at] = u8::try_from(rng.below(256)).expect("a byte"),
                1 => bytes.truncate(at),
                2 => bytes.insert(at, 0xFF),
                3 => bytes
                    .splice(at..(at + 2).min(bytes.len()), [0xFF, 0xFF])
                    .for_each(drop),
                _ => bytes.extend_from_slice(&[22, 3, 3, 0xFF, 0xFF]),
            }
            if bytes.is_empty() {
                break;
            }
        }
        let _parsed = parse(&bytes);
        let _needed = ClientHello::needed_len(&bytes);
    }
}
