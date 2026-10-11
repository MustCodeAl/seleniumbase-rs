//! Tests for the HPACK decoder, including the examples of RFC 7541 appendix C.

use super::*;

fn hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("ascii");
            u8::from_str_radix(pair, 16).expect("hex")
        })
        .collect()
}

fn fields(pairs: &[(&str, &str)]) -> Vec<HeaderField> {
    pairs
        .iter()
        .map(|(n, v)| HeaderField::new(*n, *v))
        .collect()
}

fn reader(bytes: &[u8]) -> Reader<'_> {
    Reader {
        block: bytes,
        pos: 0,
    }
}

/// Encodes a string literal; Huffman-coded when `huffman_coded`. Test strings are short.
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

/// A literal with incremental indexing and a new name.
fn indexed_literal(name: &str, value: &str) -> Vec<u8> {
    let mut out = vec![0x40];
    out.extend(string_literal(name, false));
    out.extend(string_literal(value, false));
    out
}

// RFC 7541 appendix C.1: integer representation.

#[test]
fn integers_follow_appendix_c1() {
    assert_eq!(reader(&[0x0a]).integer(5), Ok(10));
    assert_eq!(reader(&hex("1f 9a 0a")).integer(5), Ok(1337));
    assert_eq!(reader(&hex("7f 00")).integer(7), Ok(127));
    assert_eq!(reader(&hex("7e")).integer(7), Ok(126));
}

#[test]
fn integer_errors_are_named() {
    assert_eq!(reader(&[]).integer(7), Err(HpackError::Truncated));
    assert_eq!(reader(&hex("1f 9a")).integer(5), Err(HpackError::Truncated));
    assert_eq!(
        reader(&hex("1f ff ff ff ff ff 01")).integer(5),
        Err(HpackError::IntegerOverflow)
    );
    // Five continuation bytes are the most that fit; the fifth may still end the number.
    assert!(reader(&hex("1f ff ff ff ff 0f")).integer(5).is_ok());
}

// RFC 7541 appendix C.2: header field representation examples.

#[test]
fn c2_1_literal_with_incremental_indexing() {
    let mut d = HpackDecoder::default();
    let block = hex("400a 6375 7374 6f6d 2d6b 6579 0d63 7573 746f 6d2d 6865 6164 6572");
    assert_eq!(
        d.decode(&block),
        Ok(fields(&[("custom-key", "custom-header")]))
    );
    assert_eq!((d.dynamic_len(), d.dynamic_size()), (1, 55));
}

#[test]
fn c2_2_literal_without_indexing() {
    let mut d = HpackDecoder::default();
    let block = hex("040c 2f73 616d 706c 652f 7061 7468");
    assert_eq!(d.decode(&block), Ok(fields(&[(":path", "/sample/path")])));
    assert_eq!(d.dynamic_len(), 0);
}

#[test]
fn c2_3_literal_never_indexed() {
    let mut d = HpackDecoder::default();
    let block = hex("1008 7061 7373 776f 7264 0673 6563 7265 74");
    assert_eq!(d.decode(&block), Ok(fields(&[("password", "secret")])));
    assert_eq!(d.dynamic_len(), 0);
}

#[test]
fn c2_4_indexed_field() {
    let mut d = HpackDecoder::default();
    assert_eq!(d.decode(&[0x82]), Ok(fields(&[(":method", "GET")])));
}

// RFC 7541 appendix C.3 and C.4: three requests, without and with Huffman coding.

fn request_sequence(blocks: [&str; 3]) {
    let expected: [Vec<(&str, &str)>; 3] = [
        vec![
            (":method", "GET"),
            (":scheme", "http"),
            (":path", "/"),
            (":authority", "www.example.com"),
        ],
        vec![
            (":method", "GET"),
            (":scheme", "http"),
            (":path", "/"),
            (":authority", "www.example.com"),
            ("cache-control", "no-cache"),
        ],
        vec![
            (":method", "GET"),
            (":scheme", "https"),
            (":path", "/index.html"),
            (":authority", "www.example.com"),
            ("custom-key", "custom-value"),
        ],
    ];
    let sizes = [57, 110, 164];
    let mut d = HpackDecoder::default();
    for ((block, want), size) in blocks.iter().zip(&expected).zip(sizes) {
        assert_eq!(d.decode(&hex(block)), Ok(fields(want)), "{block}");
        assert_eq!(d.dynamic_size(), size, "{block}");
    }
    assert_eq!(d.dynamic_len(), 3);
}

#[test]
fn c3_requests_without_huffman_coding() {
    request_sequence([
        "8286 8441 0f77 7777 2e65 7861 6d70 6c65 2e63 6f6d",
        "8286 84be 5808 6e6f 2d63 6163 6865",
        "8287 85bf 400a 6375 7374 6f6d 2d6b 6579 0c63 7573 746f 6d2d 7661 6c75 65",
    ]);
}

#[test]
fn c4_requests_with_huffman_coding() {
    request_sequence([
        "8286 8441 8cf1 e3c2 e5f2 3a6b a0ab 90f4 ff",
        "8286 84be 5886 a8eb 1064 9cbf",
        "8287 85bf 4088 25a8 49e9 5ba9 7d7f 8925 a849 e95b b8e8 b4bf",
    ]);
}

// RFC 7541 appendix C.5 and C.6: three responses with a 256-byte table.

fn response_sequence(blocks: [&str; 3]) {
    let date = |s: &'static str| ("date", s);
    let expected: [Vec<(&str, &str)>; 3] = [
        vec![
            (":status", "302"),
            ("cache-control", "private"),
            date("Mon, 21 Oct 2013 20:13:21 GMT"),
            ("location", "https://www.example.com"),
        ],
        vec![
            (":status", "307"),
            ("cache-control", "private"),
            date("Mon, 21 Oct 2013 20:13:21 GMT"),
            ("location", "https://www.example.com"),
        ],
        vec![
            (":status", "200"),
            ("cache-control", "private"),
            date("Mon, 21 Oct 2013 20:13:22 GMT"),
            ("location", "https://www.example.com"),
            ("content-encoding", "gzip"),
            (
                "set-cookie",
                "foo=ASDJKHQKBZXOQWEOPIUAXQWEOIU; max-age=3600; version=1",
            ),
        ],
    ];
    let sizes = [222, 222, 215];
    let mut d = HpackDecoder::new(256);
    for ((block, want), size) in blocks.iter().zip(&expected).zip(sizes) {
        assert_eq!(d.decode(&hex(block)), Ok(fields(want)), "{block}");
        assert_eq!(d.dynamic_size(), size, "{block}");
    }
}

#[test]
fn c5_responses_without_huffman_coding() {
    response_sequence([
        "4803 3330 3258 0770 7269 7661 7465 611d 4d6f 6e2c 2032 3120 4f63 7420 3230 3133 2032 303a 3133 3a32 3120 474d 546e 1768 7474 7073 3a2f 2f77 7777 2e65 7861 6d70 6c65 2e63 6f6d",
        "4803 3330 37c1 c0bf",
        "88c1 611d 4d6f 6e2c 2032 3120 4f63 7420 3230 3133 2032 303a 3133 3a32 3220 474d 54c0 5a04 677a 6970 7738 666f 6f3d 4153 444a 4b48 514b 425a 584f 5157 454f 5049 5541 5851 5745 4f49 553b 206d 6178 2d61 6765 3d33 3630 303b 2076 6572 7369 6f6e 3d31",
    ]);
}

#[test]
fn c6_responses_with_huffman_coding() {
    response_sequence([
        "4882 6402 5885 aec3 771a 4b61 96d0 7abe 9410 54d4 44a8 2005 9504 0b81 66e0 82a6 2d1b ff6e 919d 29ad 1718 63c7 8f0b 97c8 e9ae 82ae 43d3",
        "4883 640e ffc1 c0bf",
        "88c1 6196 d07a be94 1054 d444 a820 0595 040b 8166 e084 a62d 1bff c05a 839b d9ab 77ad 94e7 821d d7f2 e6c7 b335 dfdf cd5b 3960 d5af 2708 7f36 72c1 ab27 0fb5 291f 9587 3160 65c0 03ed 4ee5 b106 3d50 07",
    ]);
}

// Behaviour beyond the RFC examples.

#[test]
fn index_zero_and_missing_entries_are_errors() {
    let mut d = HpackDecoder::default();
    assert_eq!(d.decode(&[0x80]), Err(HpackError::IndexZero));
    assert_eq!(d.decode(&[0xbe]), Err(HpackError::IndexOutOfRange(62)));
    assert_eq!(
        d.decode(&[0xff, 0x00]),
        Err(HpackError::IndexOutOfRange(127))
    );
    // A literal that names a missing entry.
    assert_eq!(
        d.decode(&[0x7e, 0x00]),
        Err(HpackError::IndexOutOfRange(62))
    );
}

#[test]
fn size_update_must_open_the_block_and_stay_within_the_limit() {
    let mut d = HpackDecoder::new(100);
    // Update to 0 empties the table; two updates at the start are fine.
    assert_eq!(
        d.decode(&indexed_literal("a", "b")),
        Ok(fields(&[("a", "b")]))
    );
    assert_eq!(d.dynamic_len(), 1);
    let mut block = vec![0x20, 0x3f, 0x45]; // size 0, then size 100 (31 + 69)
    block.push(0x82);
    assert_eq!(d.decode(&block), Ok(fields(&[(":method", "GET")])));
    assert_eq!(d.dynamic_len(), 0, "the update to 0 evicted the entry");
    assert_eq!(
        d.decode(&[0x3f, 0x46]), // 31 + 70 = 101
        Err(HpackError::SizeUpdateTooLarge {
            requested: 101,
            max: 100
        })
    );
    assert_eq!(
        d.decode(&[0x82, 0x20]),
        Err(HpackError::SizeUpdateAfterField)
    );
}

/// A dynamic table size update block for `size` (RFC 7541, section 6.3: 5-bit prefix `001`).
fn size_update(size: usize) -> Vec<u8> {
    const PREFIX_MAX: usize = 31;
    if size < PREFIX_MAX {
        return vec![0x20 | u8::try_from(size).expect("below 31")];
    }
    let mut out = vec![0x20 | 0x1f];
    let mut rest = size - PREFIX_MAX;
    while rest >= 0x80 {
        out.push(u8::try_from(rest & 0x7f).expect("seven bits") | 0x80);
        rest >>= 7;
    }
    out.push(u8::try_from(rest).expect("below 128"));
    out
}

#[test]
fn a_decoder_never_allows_its_peer_a_table_above_the_ceiling() {
    for asked in [MAX_TABLE_SIZE, MAX_TABLE_SIZE + 1, usize::MAX] {
        let mut d = HpackDecoder::new(asked);
        assert_eq!(d.decode(&size_update(MAX_TABLE_SIZE)), Ok(Vec::new()));
        assert_eq!(
            d.decode(&size_update(MAX_TABLE_SIZE + 1)),
            Err(HpackError::SizeUpdateTooLarge {
                requested: MAX_TABLE_SIZE + 1,
                max: MAX_TABLE_SIZE
            }),
            "asked for {asked}"
        );
    }
    let mut d = HpackDecoder::new(MAX_TABLE_SIZE - 1);
    assert_eq!(
        d.decode(&size_update(MAX_TABLE_SIZE)),
        Err(HpackError::SizeUpdateTooLarge {
            requested: MAX_TABLE_SIZE,
            max: MAX_TABLE_SIZE - 1
        }),
        "a request below the ceiling is kept as it is"
    );
}

#[test]
fn an_entry_larger_than_the_table_empties_it() {
    let mut d = HpackDecoder::new(64);
    assert_eq!(d.decode(&indexed_literal("a", "b")).map(|f| f.len()), Ok(1));
    assert_eq!(d.dynamic_len(), 1);
    let big = "x".repeat(40);
    assert_eq!(
        d.decode(&indexed_literal("n", &big)).map(|f| f.len()),
        Ok(1)
    );
    assert_eq!(d.dynamic_len(), 0);
    assert_eq!(d.dynamic_size(), 0);
}

#[test]
fn eviction_drops_the_oldest_entries_first() {
    let mut d = HpackDecoder::new(100);
    for name in ["a", "b", "c"] {
        let _fields = d.decode(&indexed_literal(name, "1")).expect("decodes");
    }
    // Three entries of 34 bytes do not fit in 100: the oldest went.
    assert_eq!(d.dynamic_len(), 2);
    assert_eq!(d.decode(&[0xbe]), Ok(fields(&[("c", "1")])));
    assert_eq!(d.decode(&[0xbf]), Ok(fields(&[("b", "1")])));
    assert_eq!(d.decode(&[0xc0]), Err(HpackError::IndexOutOfRange(64)));
}

#[test]
fn huffman_strings_decode_and_bad_ones_are_named() {
    let mut d = HpackDecoder::default();
    let mut block = vec![0x00];
    block.extend(string_literal("x-custom", true));
    block.extend(string_literal("va/lue", true));
    assert_eq!(d.decode(&block), Ok(fields(&[("x-custom", "va/lue")])));
    let eos = [0x00, 0x84, 0xff, 0xff, 0xff, 0xfc, 0x00];
    assert_eq!(
        d.decode(&eos),
        Err(HpackError::Huffman(HuffmanError::EndOfString))
    );
}

#[test]
fn invalid_utf8_is_refused() {
    let mut d = HpackDecoder::default();
    assert_eq!(
        d.decode(&[0x00, 0x01, 0xff, 0x00]),
        Err(HpackError::NotUtf8)
    );
}

#[test]
fn a_length_beyond_the_block_is_truncation() {
    let mut d = HpackDecoder::default();
    assert_eq!(d.decode(&[0x00, 0x05, b'a']), Err(HpackError::Truncated));
    assert_eq!(d.decode(&[0x00, 0x7f]), Err(HpackError::Truncated));
    assert_eq!(
        d.decode(&hex("00 7f ff ff ff ff 0f")),
        Err(HpackError::Truncated)
    );
}

#[test]
fn a_reference_bomb_hits_the_decoded_size_limit() {
    // One 4000-byte entry in the table, then 40 references to it: 160 KB decoded.
    let mut d = HpackDecoder::default();
    let value = "v".repeat(4000);
    let mut block = vec![0x40, 0x01, b'n'];
    block.extend(long_length(value.len()));
    block.extend(value.bytes());
    block.extend(std::iter::repeat_n(0xbe, 40));
    assert_eq!(d.decode(&block), Err(HpackError::HeaderListTooLarge));
}

/// A 7-bit-prefix length for `len` of 127 or more.
fn long_length(len: usize) -> Vec<u8> {
    let mut rest = len - 127;
    let mut out = vec![0x7f];
    while rest >= 128 {
        out.push(u8::try_from(rest % 128).expect("below 128") | 0x80);
        rest /= 128;
    }
    out.push(u8::try_from(rest).expect("below 128"));
    out
}

#[test]
fn too_many_fields_hit_the_field_limit() {
    let mut d = HpackDecoder::default();
    let block = vec![0x82; MAX_HEADER_FIELDS + 1];
    assert_eq!(d.decode(&block), Err(HpackError::HeaderListTooLarge));
    assert_eq!(
        d.decode(&block[1..]).map(|f| f.len()),
        Ok(MAX_HEADER_FIELDS)
    );
}

#[test]
fn truncating_every_rfc_block_never_panics() {
    let blocks = [
        "8286 8441 8cf1 e3c2 e5f2 3a6b a0ab 90f4 ff",
        "8287 85bf 4088 25a8 49e9 5ba9 7d7f 8925 a849 e95b b8e8 b4bf",
        "88c1 6196 d07a be94 1054 d444 a820 0595 040b 8166 e084 a62d 1bff c05a 839b d9ab 77ad 94e7 821d d7f2 e6c7 b335 dfdf cd5b 3960 d5af 2708 7f36 72c1 ab27 0fb5 291f 9587 3160 65c0 03ed 4ee5 b106 3d50 07",
    ];
    for text in blocks {
        let bytes = hex(text);
        for end in 0..=bytes.len() {
            let mut d = HpackDecoder::new(256);
            let _outcome = d.decode(&bytes[..end]);
        }
    }
}

#[test]
fn a_cut_inside_a_field_is_truncation_not_a_shorter_block() {
    let bytes = hex("8286 8441 8cf1 e3c2 e5f2 3a6b a0ab 90f4 ff");
    let mut d = HpackDecoder::default();
    assert_eq!(
        d.decode(&bytes[..bytes.len() - 1]),
        Err(HpackError::Truncated)
    );
}

#[test]
fn mutated_blocks_never_panic() {
    let base = hex("8287 85bf 4088 25a8 49e9 5ba9 7d7f 8925 a849 e95b b8e8 b4bf");
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..4000 {
        let mut block = base.clone();
        for _ in 0..=(next() % 3) {
            let at = usize::try_from(next() % 64).expect("small") % block.len();
            block[at] = u8::try_from(next() % 256).expect("a byte");
        }
        let mut d = HpackDecoder::new(256);
        let _outcome = d.decode(&block);
    }
}

#[test]
fn the_static_table_matches_the_rfc_anchors() {
    assert_eq!(STATIC_TABLE.len(), STATIC_LEN);
    assert_eq!(STATIC_TABLE[0], (":authority", ""));
    assert_eq!(STATIC_TABLE[1], (":method", "GET"));
    assert_eq!(STATIC_TABLE[15], ("accept-encoding", "gzip, deflate"));
    assert_eq!(STATIC_TABLE[57], ("user-agent", ""));
    assert_eq!(STATIC_TABLE[60], ("www-authenticate", ""));
}

#[test]
fn pseudo_fields_are_recognised() {
    assert!(HeaderField::new(":path", "/").is_pseudo());
    assert!(!HeaderField::new("path", "/").is_pseudo());
}
