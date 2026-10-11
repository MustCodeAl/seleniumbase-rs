//! Parses and renders JA3 strings with GREASE removed, so that a custom JA3 can be checked and replayed.
//!
//! JA3 is the Salesforce TLS client fingerprint: five comma-separated fields (handshake version,
//! cipher suites, extensions, supported groups, point formats), each a dash-separated list of
//! decimal ids with GREASE removed, hashed with MD5. The MD5 here is a small pure function because
//! JA3 is defined with it; nothing in this crate uses it for security. The [`peetprint`] module
//! holds the sibling format that also records signature algorithms.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::client_hello::{without_grease, ClientHello};

pub use peetprint::{Peetprint, PeetprintError};

/// Longest JA3 string [`Ja3::parse`] reads.
pub const MAX_JA3_LEN: usize = 16 * 1024;

/// Separates the five fields.
const FIELD_SEPARATOR: char = ',';
/// Separates the values inside one field.
const VALUE_SEPARATOR: char = '-';

/// One of the five fields of a JA3 string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ja3Field {
    /// The handshake `client_version`.
    Version,
    /// The cipher suites.
    Ciphers,
    /// The extension ids.
    Extensions,
    /// The supported groups (elliptic curves).
    Groups,
    /// The EC point formats.
    PointFormats,
}

impl fmt::Display for Ja3Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Version => "version",
            Self::Ciphers => "ciphers",
            Self::Extensions => "extensions",
            Self::Groups => "groups",
            Self::PointFormats => "point formats",
        })
    }
}

/// Why a JA3 string was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Ja3Error {
    /// The string does not have exactly five comma-separated fields.
    #[error("a JA3 string has 5 comma-separated fields, found {0}")]
    FieldCount(usize),
    /// A value is not a decimal number that fits its field.
    #[error("the JA3 {field} field holds `{token}`, which is not a valid number")]
    Number {
        /// The field that holds the value.
        field: Ja3Field,
        /// The offending text.
        token: String,
    },
    /// A field that must hold at least one value is empty (after GREASE removal).
    #[error("the JA3 {0} field is empty")]
    Empty(Ja3Field),
    /// The string is longer than [`MAX_JA3_LEN`].
    #[error("the JA3 string is longer than 16 KiB")]
    TooLong,
}

/// A JA3 fingerprint: the five fields, GREASE removed, in wire order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ja3 {
    /// The handshake `client_version` (771 for TLS 1.2 and 1.3 clients).
    pub version: u16,
    /// Cipher suite ids.
    pub ciphers: Vec<u16>,
    /// Extension ids, in the order the client sent them.
    pub extensions: Vec<u16>,
    /// Supported group ids.
    pub groups: Vec<u16>,
    /// EC point format ids.
    pub point_formats: Vec<u8>,
}

impl Ja3 {
    /// The JA3 of a parsed hello: GREASE removed everywhere, extensions in wire order.
    #[must_use]
    pub fn of(hello: &ClientHello) -> Self {
        Self {
            version: hello.legacy_version,
            ciphers: without_grease(&hello.cipher_suites),
            extensions: without_grease(&hello.extension_ids()),
            groups: without_grease(&hello.supported_groups),
            point_formats: hello.point_formats.clone(),
        }
    }

    /// Parses a JA3 string such as `771,4865-4866,0-23,29-23,0`.
    ///
    /// GREASE ids in the input are dropped, as the fingerprint is defined without them. The
    /// version and the ciphers must be present; the other lists may be empty.
    ///
    /// # Errors
    ///
    /// Returns a [`Ja3Error`] naming the field and the token when the text is not a JA3 string.
    pub fn parse(text: &str) -> Result<Self, Ja3Error> {
        if text.len() > MAX_JA3_LEN {
            return Err(Ja3Error::TooLong);
        }
        let fields: Vec<&str> = text.trim().split(FIELD_SEPARATOR).collect();
        let &[version, ciphers, extensions, groups, point_formats] = fields.as_slice() else {
            return Err(Ja3Error::FieldCount(fields.len()));
        };
        if version.is_empty() {
            return Err(Ja3Error::Empty(Ja3Field::Version));
        }
        let ciphers = without_grease(&numbers(ciphers, Ja3Field::Ciphers)?);
        if ciphers.is_empty() {
            return Err(Ja3Error::Empty(Ja3Field::Ciphers));
        }
        Ok(Self {
            version: decimal(version).ok_or_else(|| number_error(Ja3Field::Version, version))?,
            ciphers,
            extensions: without_grease(&numbers(extensions, Ja3Field::Extensions)?),
            groups: without_grease(&numbers(groups, Ja3Field::Groups)?),
            point_formats: numbers(point_formats, Ja3Field::PointFormats)?,
        })
    }

    /// The lowercase hex MD5 of the JA3 string, the form JA3 databases list.
    #[must_use]
    pub fn hash(&self) -> String {
        md5_hex(self.to_string().as_bytes())
    }
}

impl fmt::Display for Ja3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},{},{},{}",
            self.version,
            join(&self.ciphers),
            join(&self.extensions),
            join(&self.groups),
            join(&self.point_formats),
        )
    }
}

impl FromStr for Ja3 {
    type Err = Ja3Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

fn number_error(field: Ja3Field, token: &str) -> Ja3Error {
    Ja3Error::Number {
        field,
        token: token.to_owned(),
    }
}

/// A non-empty run of ASCII digits that fits `T`; signs and spaces are refused.
fn decimal<T: FromStr>(token: &str) -> Option<T> {
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    token.parse().ok()
}

/// A dash-separated list of numbers; the empty text is the empty list.
fn numbers<T: FromStr>(text: &str, field: Ja3Field) -> Result<Vec<T>, Ja3Error> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    text.split(VALUE_SEPARATOR)
        .map(|token| decimal(token).ok_or_else(|| number_error(field, token)))
        .collect()
}

fn join<T: fmt::Display>(items: &[T]) -> String {
    let texts: Vec<String> = items.iter().map(ToString::to_string).collect();
    texts.join("-")
}

/// Block size of MD5 in bytes.
const MD5_BLOCK: usize = 64;
/// Message length modulo the block size at which the length field starts.
const MD5_LENGTH_OFFSET: usize = 56;
/// Initial state of MD5 (RFC 1321, section 3.3).
const MD5_INIT: [u32; 4] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476];
/// Per-round left rotations (RFC 1321, section 3.4).
const MD5_SHIFTS: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];
/// The sine table: the integer part of `2^32 * abs(sin(i + 1))` (RFC 1321, section 3.4).
const MD5_CONSTANTS: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

/// The MD5 digest of `data` (RFC 1321).
fn md5(data: &[u8]) -> [u8; 16] {
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % MD5_BLOCK != MD5_LENGTH_OFFSET {
        message.push(0);
    }
    let bit_len = u64::try_from(data.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    message.extend_from_slice(&bit_len.to_le_bytes());
    let mut state = MD5_INIT;
    let (blocks, _) = message.as_chunks::<MD5_BLOCK>();
    for block in blocks {
        md5_block(&mut state, block);
    }
    let mut digest = [0_u8; 16];
    let (lanes, _) = digest.as_chunks_mut::<4>();
    for (out, word) in lanes.iter_mut().zip(state) {
        *out = word.to_le_bytes();
    }
    digest
}

/// Folds one 64-byte block into the MD5 state.
fn md5_block(state: &mut [u32; 4], block: &[u8; MD5_BLOCK]) {
    let mut words = [0_u32; 16];
    let (chunks, _) = block.as_chunks::<4>();
    for (word, bytes) in words.iter_mut().zip(chunks) {
        *word = u32::from_le_bytes(*bytes);
    }
    let [mut a, mut b, mut c, mut d] = *state;
    for (round, (&constant, &shift)) in MD5_CONSTANTS.iter().zip(&MD5_SHIFTS).enumerate() {
        let (mix, word_index) = match round / 16 {
            0 => ((b & c) | (!b & d), round),
            1 => ((d & b) | (!d & c), (5 * round + 1) % 16),
            2 => (b ^ c ^ d, (3 * round + 5) % 16),
            _ => (c ^ (b | !d), (7 * round) % 16),
        };
        let rotated = mix
            .wrapping_add(a)
            .wrapping_add(constant)
            .wrapping_add(words[word_index])
            .rotate_left(shift);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(rotated);
    }
    for (slot, lane) in state.iter_mut().zip([a, b, c, d]) {
        *slot = slot.wrapping_add(lane);
    }
}

/// Lowercase hex digits.
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Lowercase hex encoding of `bytes`.
pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(HEX_DIGITS[usize::from(byte & 0x0F)]));
    }
    out
}

/// The lowercase hex MD5 of `data`.
///
/// MD5 is broken as a hash. It is here only because JA3 and the peetprint hash are defined with
/// it; do not use it for anything that needs collision resistance.
#[must_use]
pub fn md5_hex(data: &[u8]) -> String {
    hex_lower(&md5(data))
}

pub mod peetprint {
    //! The peetprint: a JA3-like TLS fingerprint that also records signature algorithms.
    //!
    //! The format is that of a public TLS inspection service, read from its description. Eight
    //! fields joined by `|`, each a `-` separated list: supported versions, ALPN protocols
    //! (`1.0`, `1.1`, `2`), supported groups, signature algorithms, PSK key exchange modes,
    //! certificate compression algorithms, cipher suites and the extensions. GREASE ids are
    //! written as the word `GREASE`. The public description says only that the extension list is
    //! sorted; this module sorts it by the text of each token. Parsing accepts any order, and
    //! the spec layer compares extension lists as sets, so the sort rule never decides whether a
    //! custom peetprint is accepted.

    use std::fmt;
    use std::str::FromStr;

    use serde::{Deserialize, Serialize};
    use thiserror::Error;

    use super::{md5_hex, VALUE_SEPARATOR};
    use crate::wire::client_hello::{without_grease, ClientHello};
    use crate::wire::ids::is_grease;

    /// Fields in a peetprint.
    const PEET_FIELDS: usize = 8;
    /// Separates the eight fields.
    const PEET_FIELD_SEPARATOR: char = '|';
    /// The word that stands for any GREASE id.
    const GREASE_TOKEN: &str = "GREASE";
    /// ALPN protocol names and the short forms a peetprint writes.
    const ALPN_FORMS: [(&str, &str); 3] = [("h2", "2"), ("http/1.1", "1.1"), ("http/1.0", "1.0")];
    /// Prefix of an ALPN protocol with no short form, followed by its hex bytes.
    const ALPN_HEX_PREFIX: char = 'x';
    /// Longest peetprint string [`Peetprint::parse`] reads.
    pub const MAX_PEETPRINT_LEN: usize = 16 * 1024;

    /// A list entry: a GREASE value or a concrete id.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Slot {
        /// Any GREASE value (RFC 8701).
        Grease,
        /// A concrete id.
        Id(u16),
    }

    impl Slot {
        /// The slot for a wire id: GREASE ids collapse into [`Slot::Grease`].
        #[must_use]
        pub fn from_id(id: u16) -> Self {
            if is_grease(id) {
                Self::Grease
            } else {
                Self::Id(id)
            }
        }

        fn token(self) -> String {
            match self {
                Self::Grease => GREASE_TOKEN.to_owned(),
                Self::Id(id) => id.to_string(),
            }
        }
    }

    /// One of the eight fields of a peetprint.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum PeetField {
        /// Supported TLS versions.
        Versions,
        /// ALPN protocols.
        Alpn,
        /// Supported groups.
        Groups,
        /// Signature algorithms.
        SignatureAlgorithms,
        /// PSK key exchange modes.
        PskModes,
        /// Certificate compression algorithms.
        CertCompression,
        /// Cipher suites.
        Ciphers,
        /// Extensions.
        Extensions,
    }

    impl fmt::Display for PeetField {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(match self {
                Self::Versions => "versions",
                Self::Alpn => "alpn",
                Self::Groups => "groups",
                Self::SignatureAlgorithms => "signature algorithms",
                Self::PskModes => "psk modes",
                Self::CertCompression => "certificate compression",
                Self::Ciphers => "ciphers",
                Self::Extensions => "extensions",
            })
        }
    }

    /// Why a peetprint string was refused.
    #[derive(Debug, Clone, PartialEq, Eq, Error)]
    pub enum PeetprintError {
        /// The string does not have exactly eight `|` separated fields.
        #[error("a peetprint has 8 fields separated by `|`, found {0}")]
        FieldCount(usize),
        /// A token is neither a number that fits the field, `GREASE`, nor a known ALPN form.
        #[error("the peetprint {field} field holds `{token}`, which is not a valid value")]
        Token {
            /// The field that holds the token.
            field: PeetField,
            /// The offending text.
            token: String,
        },
        /// The string is longer than [`MAX_PEETPRINT_LEN`].
        #[error("the peetprint is longer than 16 KiB")]
        TooLong,
    }

    /// A parsed peetprint.
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct Peetprint {
        /// Supported TLS versions as sent.
        pub versions: Vec<Slot>,
        /// ALPN protocol names (`h2`, `http/1.1`, ...) as sent.
        pub alpn: Vec<String>,
        /// Supported groups as sent.
        pub groups: Vec<Slot>,
        /// Signature algorithms as sent.
        pub signature_algorithms: Vec<Slot>,
        /// PSK key exchange modes as sent.
        pub psk_modes: Vec<u8>,
        /// Certificate compression algorithms as sent.
        pub cert_compression: Vec<u16>,
        /// Cipher suites as sent.
        pub ciphers: Vec<Slot>,
        /// Extensions; [`Peetprint::of`] sorts them, parsing keeps the given order.
        pub extensions: Vec<Slot>,
    }

    impl Peetprint {
        /// The peetprint of a parsed hello.
        #[must_use]
        pub fn of(hello: &ClientHello) -> Self {
            let alpn = hello
                .alpn
                .iter()
                .map(|name| String::from_utf8_lossy(name).into_owned())
                .collect();
            Self {
                versions: slots(&hello.supported_versions),
                alpn,
                groups: slots(&hello.supported_groups),
                signature_algorithms: slots(&hello.signature_algorithms),
                psk_modes: hello.psk_modes.clone(),
                cert_compression: hello.cert_compression.clone(),
                ciphers: slots(&hello.cipher_suites),
                extensions: slots(&hello.extension_ids()),
            }
            .normalized()
        }

        /// The same peetprint with the extension list sorted by token text.
        #[must_use]
        pub fn normalized(mut self) -> Self {
            self.extensions.sort_by_key(|slot| slot.token());
            self
        }

        /// The extension ids without GREASE, in the stored order.
        #[must_use]
        pub fn extension_ids(&self) -> Vec<u16> {
            ids(&self.extensions)
        }

        /// The lowercase hex MD5 of the string, the peetprint hash.
        #[must_use]
        pub fn hash(&self) -> String {
            md5_hex(self.to_string().as_bytes())
        }

        /// Parses a peetprint string.
        ///
        /// # Errors
        ///
        /// Returns a [`PeetprintError`] naming the field and the token that failed.
        pub fn parse(text: &str) -> Result<Self, PeetprintError> {
            if text.len() > MAX_PEETPRINT_LEN {
                return Err(PeetprintError::TooLong);
            }
            let fields: Vec<&str> = text.trim().split(PEET_FIELD_SEPARATOR).collect();
            let &[versions, alpn, groups, sigalgs, psk, compression, ciphers, extensions] =
                fields.as_slice()
            else {
                return Err(PeetprintError::FieldCount(fields.len()));
            };
            Ok(Self {
                versions: list(versions, PeetField::Versions, slot)?,
                alpn: list(alpn, PeetField::Alpn, alpn_name)?,
                groups: list(groups, PeetField::Groups, slot)?,
                signature_algorithms: list(sigalgs, PeetField::SignatureAlgorithms, slot)?,
                psk_modes: list(psk, PeetField::PskModes, number)?,
                cert_compression: list(compression, PeetField::CertCompression, number)?,
                ciphers: list(ciphers, PeetField::Ciphers, slot)?,
                extensions: list(extensions, PeetField::Extensions, slot)?,
            })
        }
    }

    impl fmt::Display for Peetprint {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let alpn: Vec<String> = self.alpn.iter().map(|name| alpn_token(name)).collect();
            let fields = [
                join_slots(&self.versions),
                alpn.join("-"),
                join_slots(&self.groups),
                join_slots(&self.signature_algorithms),
                join_numbers(&self.psk_modes),
                join_numbers(&self.cert_compression),
                join_slots(&self.ciphers),
                join_slots(&self.extensions),
            ];
            debug_assert_eq!(fields.len(), PEET_FIELDS);
            f.write_str(&fields.join(&PEET_FIELD_SEPARATOR.to_string()))
        }
    }

    impl FromStr for Peetprint {
        type Err = PeetprintError;

        fn from_str(text: &str) -> Result<Self, Self::Err> {
            Self::parse(text)
        }
    }

    /// GREASE-collapsed slots of a list of wire ids.
    #[must_use]
    pub fn slots(values: &[u16]) -> Vec<Slot> {
        values.iter().map(|&id| Slot::from_id(id)).collect()
    }

    /// The concrete ids of a slot list, GREASE dropped.
    #[must_use]
    pub fn ids(values: &[Slot]) -> Vec<u16> {
        let raw: Vec<u16> = values
            .iter()
            .filter_map(|slot| match slot {
                Slot::Id(id) => Some(*id),
                Slot::Grease => None,
            })
            .collect();
        without_grease(&raw)
    }

    fn join_slots(values: &[Slot]) -> String {
        let tokens: Vec<String> = values.iter().map(|slot| slot.token()).collect();
        tokens.join("-")
    }

    fn join_numbers<T: fmt::Display>(values: &[T]) -> String {
        let tokens: Vec<String> = values.iter().map(ToString::to_string).collect();
        tokens.join("-")
    }

    fn alpn_token(name: &str) -> String {
        if let Some((_, short)) = ALPN_FORMS.iter().find(|(long, _)| *long == name) {
            return (*short).to_owned();
        }
        let hex = super::hex_lower(name.as_bytes());
        format!("{ALPN_HEX_PREFIX}{hex}")
    }

    /// Splits a field into tokens and converts each; the empty field is the empty list.
    fn list<T>(
        text: &str,
        field: PeetField,
        convert: fn(&str) -> Option<T>,
    ) -> Result<Vec<T>, PeetprintError> {
        if text.is_empty() {
            return Ok(Vec::new());
        }
        text.split(VALUE_SEPARATOR)
            .map(|token| {
                convert(token).ok_or_else(|| PeetprintError::Token {
                    field,
                    token: token.to_owned(),
                })
            })
            .collect()
    }

    fn number<T: FromStr>(token: &str) -> Option<T> {
        if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        token.parse().ok()
    }

    fn slot(token: &str) -> Option<Slot> {
        if token == GREASE_TOKEN {
            return Some(Slot::Grease);
        }
        number::<u16>(token).map(Slot::from_id)
    }

    fn alpn_name(token: &str) -> Option<String> {
        if let Some((long, _)) = ALPN_FORMS.iter().find(|(_, short)| *short == token) {
            return Some((*long).to_owned());
        }
        let hex = token.strip_prefix(ALPN_HEX_PREFIX)?;
        if hex.is_empty() || hex.len() % 2 != 0 {
            return None;
        }
        let (pairs, _) = hex.as_bytes().as_chunks::<2>();
        let bytes: Option<Vec<u8>> = pairs.iter().copied().map(hex_byte).collect();
        String::from_utf8(bytes?).ok()
    }

    /// The byte two hex digits stand for; signs and other characters are refused.
    fn hex_byte(pair: [u8; 2]) -> Option<u8> {
        let high = char::from(pair[0]).to_digit(16)?;
        let low = char::from(pair[1]).to_digit(16)?;
        u8::try_from(high * 16 + low).ok()
    }
}

#[cfg(test)]
mod tests;
