//! Huffman decoding of HPACK string literals (RFC 7541, section 5.2 and appendix B).
//!
//! The HPACK code is canonical: codes of one length ascend with the symbol value, and a
//! longer code starts where the shorter ones end. The length of each code is therefore all
//! that is needed. The decoding tables are derived from those lengths at compile time, and
//! the tests check them against the examples in RFC 7541 appendix C.

use thiserror::Error;

/// The 256 octets plus the end-of-string marker.
const SYMBOLS: usize = 257;
/// Index of the end-of-string symbol, which must never appear inside a string.
const EOS: usize = 256;
/// Length in bits of the longest code, the one of the end-of-string symbol.
const MAX_CODE_LEN: usize = 30;
/// Padding after the last symbol may be at most this many bits (RFC 7541, section 5.2).
const MAX_PADDING_BITS: usize = 7;

/// Length in bits of the code of each symbol (RFC 7541, appendix B), indexed by symbol.
const CODE_LENGTHS: [usize; SYMBOLS] = [
    13, 23, 28, 28, 28, 28, 28, 28, 28, 24, 30, 28, 28, 30, 28, 28, 28, 28, 28, 28, 28, 28, 30, 28,
    28, 28, 28, 28, 28, 28, 28, 28, 6, 10, 10, 12, 13, 6, 8, 11, 10, 10, 8, 11, 8, 6, 6, 6, 5, 5,
    5, 6, 6, 6, 6, 6, 6, 6, 7, 8, 15, 6, 12, 10, 13, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 8, 7, 8, 13, 19, 13, 14, 6, 15, 5, 6, 5, 6, 5, 6, 6, 6, 5, 7, 7, 6, 6,
    6, 5, 6, 7, 6, 5, 5, 6, 7, 7, 7, 7, 7, 15, 11, 14, 13, 28, 20, 22, 20, 20, 22, 22, 22, 23, 22,
    23, 23, 23, 23, 23, 24, 23, 24, 24, 22, 23, 24, 23, 23, 23, 23, 21, 22, 23, 22, 23, 23, 24, 22,
    21, 20, 22, 22, 23, 23, 21, 23, 22, 22, 24, 21, 22, 23, 23, 21, 21, 22, 21, 23, 22, 23, 23, 20,
    22, 22, 22, 23, 22, 22, 23, 26, 26, 20, 19, 22, 23, 22, 25, 26, 26, 26, 27, 27, 26, 24, 25, 19,
    21, 26, 27, 27, 26, 27, 24, 21, 21, 26, 26, 28, 27, 27, 27, 20, 24, 20, 21, 22, 21, 21, 23, 22,
    22, 25, 25, 24, 24, 26, 23, 26, 27, 26, 26, 27, 27, 27, 27, 27, 28, 27, 27, 27, 27, 27, 26, 30,
];

/// Canonical decoding tables, indexed by code length.
struct Tables {
    /// The numeric value of the first code of each length.
    first_code: [usize; MAX_CODE_LEN + 1],
    /// Position in `symbols` of the first symbol of each length.
    first_index: [usize; MAX_CODE_LEN + 1],
    /// How many codes have each length.
    count: [usize; MAX_CODE_LEN + 1],
    /// Symbols ordered by code length, then by symbol value.
    symbols: [usize; SYMBOLS],
}

static TABLES: Tables = build_tables();

const fn build_tables() -> Tables {
    let mut count = [0; MAX_CODE_LEN + 1];
    let mut symbol = 0;
    while symbol < SYMBOLS {
        count[CODE_LENGTHS[symbol]] += 1;
        symbol += 1;
    }
    let mut first_code = [0; MAX_CODE_LEN + 1];
    let mut first_index = [0; MAX_CODE_LEN + 1];
    let (mut code, mut index, mut len) = (0, 0, 1);
    while len <= MAX_CODE_LEN {
        first_code[len] = code;
        first_index[len] = index;
        code = (code + count[len]) << 1;
        index += count[len];
        len += 1;
    }
    let mut next = first_index;
    let mut symbols = [0; SYMBOLS];
    symbol = 0;
    while symbol < SYMBOLS {
        let len = CODE_LENGTHS[symbol];
        symbols[next[len]] = symbol;
        next[len] += 1;
        symbol += 1;
    }
    Tables {
        first_code,
        first_index,
        count,
        symbols,
    }
}

impl Tables {
    /// The symbol whose code is `code` written in exactly `len` bits, if there is one.
    fn lookup(&self, code: usize, len: usize) -> Option<usize> {
        let offset = code.checked_sub(*self.first_code.get(len)?)?;
        if offset >= *self.count.get(len)? {
            return None;
        }
        self.symbols
            .get(self.first_index.get(len)? + offset)
            .copied()
    }
}

/// Why a Huffman-coded string was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HuffmanError {
    /// The end-of-string symbol appeared inside the string.
    #[error("the end-of-string symbol appeared inside a Huffman-coded string")]
    EndOfString,
    /// The padding after the last symbol is longer than seven bits.
    #[error("Huffman padding is longer than 7 bits")]
    PaddingTooLong,
    /// The padding is not the most significant bits of the end-of-string symbol (all ones).
    #[error("Huffman padding is not made of ones")]
    PaddingNotOnes,
    /// Bits that match no code.
    #[error("Huffman data matches no code")]
    InvalidCode,
}

/// Decodes a Huffman-coded HPACK string literal.
///
/// # Errors
///
/// Returns a [`HuffmanError`] when the end-of-string symbol appears inside the data, when
/// the padding is longer than seven bits or is not all ones, or when the bits match no code.
pub fn decode(input: &[u8]) -> Result<Vec<u8>, HuffmanError> {
    let mut out = Vec::with_capacity(input.len().saturating_mul(2));
    let (mut code, mut len) = (0_usize, 0_usize);
    for byte in input {
        for shift in (0..8).rev() {
            code = (code << 1) | usize::from((byte >> shift) & 1);
            len += 1;
            match TABLES.lookup(code, len) {
                Some(EOS) => return Err(HuffmanError::EndOfString),
                Some(symbol) => {
                    out.push(u8::try_from(symbol).map_err(|_wide| HuffmanError::InvalidCode)?);
                    code = 0;
                    len = 0;
                }
                None if len >= MAX_CODE_LEN => return Err(HuffmanError::InvalidCode),
                None => {}
            }
        }
    }
    if len > MAX_PADDING_BITS {
        return Err(HuffmanError::PaddingTooLong);
    }
    if code != (1 << len) - 1 {
        return Err(HuffmanError::PaddingNotOnes);
    }
    Ok(out)
}

/// Encodes `input` with the HPACK code, padded with ones. Only the tests need it.
#[cfg(test)]
pub(super) fn encode(input: &[u8]) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::new();
    for byte in input {
        let symbol = usize::from(*byte);
        let len = CODE_LENGTHS[symbol];
        let rank = TABLES
            .symbols
            .iter()
            .position(|s| *s == symbol)
            .expect("every symbol has a place")
            - TABLES.first_index[len];
        let code = TABLES.first_code[len] + rank;
        bits.extend((0..len).rev().map(|shift| (code >> shift) & 1 == 1));
    }
    while bits.len() % 8 != 0 {
        bits.push(true);
    }
    bits.chunks(8)
        .map(|chunk| {
            chunk
                .iter()
                .fold(0_u8, |acc, bit| (acc << 1) | u8::from(*bit))
        })
        .collect()
}

#[cfg(test)]
mod tests {
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

    /// The Huffman-coded strings of RFC 7541 appendix C.4 and C.6, with their plain text.
    const RFC_VECTORS: &[(&str, &str)] = &[
        ("f1e3 c2e5 f23a 6ba0 ab90 f4ff", "www.example.com"),
        ("a8eb 1064 9cbf", "no-cache"),
        ("25a8 49e9 5ba9 7d7f", "custom-key"),
        ("25a8 49e9 5bb8 e8b4 bf", "custom-value"),
        ("6402", "302"),
        ("aec3 771a 4b", "private"),
        (
            "d07a be94 1054 d444 a820 0595 040b 8166 e082 a62d 1bff",
            "Mon, 21 Oct 2013 20:13:21 GMT",
        ),
        (
            "9d29 ad17 1863 c78f 0b97 c8e9 ae82 ae43 d3",
            "https://www.example.com",
        ),
        ("640e ff", "307"),
        (
            "d07a be94 1054 d444 a820 0595 040b 8166 e084 a62d 1bff",
            "Mon, 21 Oct 2013 20:13:22 GMT",
        ),
        ("9bd9 ab", "gzip"),
        (
            "94e7 821d d7f2 e6c7 b335 dfdf cd5b 3960 d5af 2708 7f36 72c1 ab27 0fb5 291f 9587 3160 65c0 03ed 4ee5 b106 3d50 07",
            "foo=ASDJKHQKBZXOQWEOPIUAXQWEOIU; max-age=3600; version=1",
        ),
    ];

    #[test]
    fn decodes_every_rfc_7541_appendix_c_string() {
        for (encoded, plain) in RFC_VECTORS {
            assert_eq!(
                decode(&hex(encoded)).expect(plain),
                plain.as_bytes(),
                "{plain}"
            );
        }
    }

    #[test]
    fn the_test_encoder_reproduces_the_rfc_bytes() {
        for (encoded, plain) in RFC_VECTORS {
            assert_eq!(encode(plain.as_bytes()), hex(encoded), "{plain}");
        }
    }

    #[test]
    fn the_code_is_complete_and_prefix_free() {
        // Kraft equality: a complete prefix code fills the whole code space exactly.
        let total: u64 = CODE_LENGTHS
            .iter()
            .map(|len| 1_u64 << (MAX_CODE_LEN - len))
            .sum();
        assert_eq!(total, 1_u64 << MAX_CODE_LEN);
        assert_eq!(TABLES.first_code[5], 0);
        assert_eq!(TABLES.lookup(0x1ff8, 13), Some(0));
        assert_eq!(
            TABLES.lookup((1 << MAX_CODE_LEN) - 1, MAX_CODE_LEN),
            Some(EOS)
        );
    }

    #[test]
    fn every_octet_round_trips() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(decode(&encode(&all)).expect("all octets"), all);
        for byte in 0..=255_u8 {
            assert_eq!(decode(&encode(&[byte])).expect("one octet"), vec![byte]);
        }
    }

    #[test]
    fn empty_input_is_an_empty_string() {
        assert_eq!(decode(&[]), Ok(Vec::new()));
    }

    #[test]
    fn the_end_of_string_symbol_is_refused() {
        assert_eq!(
            decode(&[0xff, 0xff, 0xff, 0xfc]),
            Err(HuffmanError::EndOfString)
        );
        // The same ones, but with a real symbol before them.
        assert_eq!(
            decode(&[0x1f, 0xff, 0xff, 0xff, 0xff]),
            Err(HuffmanError::EndOfString)
        );
    }

    #[test]
    fn padding_longer_than_seven_bits_is_refused() {
        // 'a' is 00011; 3 padding bits make 0x1f, a further byte of ones makes 11 padding bits.
        assert_eq!(decode(&[0x1f]), Ok(b"a".to_vec()));
        assert_eq!(decode(&[0x1f, 0xff]), Err(HuffmanError::PaddingTooLong));
    }

    #[test]
    fn padding_that_is_not_ones_is_refused() {
        assert_eq!(decode(&[0b0001_1000]), Err(HuffmanError::PaddingNotOnes));
        assert_eq!(decode(&[0b0001_1010]), Err(HuffmanError::PaddingNotOnes));
    }

    #[test]
    fn truncating_a_coded_string_never_panics() {
        for (encoded, _plain) in RFC_VECTORS {
            let bytes = hex(encoded);
            for end in 0..=bytes.len() {
                let _outcome = decode(&bytes[..end]);
            }
        }
    }

    #[test]
    fn single_bit_flips_never_panic() {
        for (encoded, _plain) in RFC_VECTORS {
            let bytes = hex(encoded);
            for at in 0..bytes.len() * 8 {
                let mut flipped = bytes.clone();
                flipped[at / 8] ^= 0x80 >> (at % 8);
                let _outcome = decode(&flipped);
            }
        }
    }
}
