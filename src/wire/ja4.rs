//! Computes `JA4` and `JA4_r` from a `ClientHello`, following `FoxIO`'s TLS-client definition.
//!
//! Only JA4 itself is implemented (the TLS client method, BSD-3-Clause); JA4H, JA4S, JA4X and the
//! other JA4+ methods carry a different licence and are not part of this crate. A JA4 string is
//! `a_b_c`: `a` holds the transport, the highest TLS version, whether SNI is present, the cipher
//! and extension counts and the first and last character of the first ALPN value; `b` is the first
//! 12 hex digits of the SHA-256 of the sorted cipher suites; `c` is the same hash over the sorted
//! extensions (without SNI and ALPN) followed by the signature algorithms in wire order. The
//! variants are `JA4_o` (original order, SNI and ALPN kept in the extension list), `JA4_r` (sorted,
//! unhashed) and `JA4_ro` (original order, unhashed). GREASE values are ignored everywhere.

use std::fmt;
use std::str::FromStr;

use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::client_hello::{ext_id, is_grease, without_grease, ClientHello};
use super::ja3::hex_lower;

/// Longest raw fingerprint string the parsers read.
pub const MAX_JA4_LEN: usize = 16 * 1024;

/// Separates the sections of a fingerprint.
const SECTION_SEPARATOR: char = '_';
/// Separates the ids inside a raw section.
const ID_SEPARATOR: char = ',';
/// Hex digits kept from the SHA-256 digest.
const HASH_HEX_CHARS: usize = 12;
/// Digest bytes behind those hex digits.
const HASH_BYTES: usize = HASH_HEX_CHARS / 2;
/// The hash written for an empty list.
const NO_HASH: &str = "000000000000";
/// The ALPN field of a hello with no usable ALPN value.
const NO_ALPN: &str = "00";
/// Highest value of a two-digit count.
const MAX_COUNT: usize = 99;
/// Characters in the `a` section.
const PART_A_LEN: usize = 10;
/// The transport letter of TLS over TCP.
const PROTOCOL_TCP: char = 't';
/// Transport letters `a` may start with: TCP, QUIC and DTLS.
const PROTOCOLS: &[u8] = b"tqd";
/// The SNI letter when the hello names a domain.
const SNI_DOMAIN: char = 'd';
/// The SNI letter when the hello carries no `server_name`.
const SNI_IP: char = 'i';
/// Hex digits in one raw id.
const ID_HEX_CHARS: usize = 4;

/// Two-character version codes of the `a` section.
const VERSION_CODES: [(u16, &str); 9] = [
    (0x0304, "13"),
    (0x0303, "12"),
    (0x0302, "11"),
    (0x0301, "10"),
    (0x0300, "s3"),
    (0x0002, "s2"),
    (0xFEFF, "d1"),
    (0xFEFD, "d2"),
    (0xFEFC, "d3"),
];

/// Whether the cipher and extension lists are sorted, as JA4 defines, or kept in wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ja4Order {
    /// Sorted ids, SNI and ALPN left out of the extension list: `JA4` and `JA4_r`.
    Sorted,
    /// Wire order, SNI and ALPN kept: `JA4_o` and `JA4_ro`.
    Original,
}

/// A section of a fingerprint, named in errors and mismatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ja4Section {
    /// The `a` section.
    A,
    /// The cipher suites (`b`).
    Ciphers,
    /// The extensions (`c`).
    Extensions,
    /// The signature algorithms (the tail of `c`).
    SignatureAlgorithms,
}

impl fmt::Display for Ja4Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::A => "a",
            Self::Ciphers => "ciphers",
            Self::Extensions => "extensions",
            Self::SignatureAlgorithms => "signature algorithms",
        })
    }
}

/// Why a JA4 string was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Ja4Error {
    /// The string has the wrong number of `_` separated sections.
    #[error("a JA4 string has 3 sections (4 when raw and signed), found {0}")]
    Sections(usize),
    /// The `a` section is not ten characters of the documented shape.
    #[error("`{0}` is not a valid JA4 `a` section")]
    PartA(String),
    /// A hashed section is not twelve lowercase hex digits.
    #[error("the {0} hash is not 12 lowercase hex digits")]
    Hash(Ja4Section),
    /// A raw id is not four hex digits.
    #[error("the {section} section holds `{token}`, which is not four hex digits")]
    Hex {
        /// The section that holds the token.
        section: Ja4Section,
        /// The offending text.
        token: String,
    },
    /// The string is longer than [`MAX_JA4_LEN`].
    #[error("the JA4 string is longer than 16 KiB")]
    TooLong,
}

/// The two-character code of a TLS version (`13` for 0x0304), or `00` when unknown.
#[must_use]
pub fn version_code(version: u16) -> &'static str {
    VERSION_CODES
        .iter()
        .find(|&&(known, _)| known == version)
        .map_or("00", |&(_, code)| code)
}

/// The version a two-character code stands for, or `None` for `00` and unknown codes.
#[must_use]
pub fn version_from_code(code: &str) -> Option<u16> {
    VERSION_CODES
        .iter()
        .find(|&&(_, known)| known == code)
        .map(|&(version, _)| version)
}

/// The ALPN field of the `a` section for the first ALPN value (`00` without one).
///
/// The first and last character of the value when both bytes are ASCII alphanumeric, else the
/// first and last hex digit of the whole value.
#[must_use]
pub fn alpn_code(first: Option<&[u8]>) -> String {
    let value = first.unwrap_or_default();
    let (Some(&head), Some(&tail)) = (value.first(), value.last()) else {
        return NO_ALPN.to_owned();
    };
    if head.is_ascii_alphanumeric() && tail.is_ascii_alphanumeric() {
        return [char::from(head), char::from(tail)].into_iter().collect();
    }
    let hex = hex_lower(value);
    hex.chars()
        .next()
        .into_iter()
        .chain(hex.chars().last())
        .collect()
}

/// The facts of a hello that JA4 reads, independent of how the hello was obtained.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ja4Facts {
    /// The handshake `client_version`, used when no `supported_versions` is present.
    pub legacy_version: u16,
    /// The `supported_versions` list; empty when the extension is absent.
    pub supported_versions: Vec<u16>,
    /// Cipher suites in wire order.
    pub ciphers: Vec<u16>,
    /// Every extension id in wire order, SNI and ALPN included.
    pub extensions: Vec<u16>,
    /// The first ALPN protocol name, when the hello offers ALPN.
    pub first_alpn: Option<Vec<u8>>,
    /// Signature algorithms in wire order; empty when the extension is absent.
    pub signature_algorithms: Vec<u16>,
}

impl Ja4Facts {
    /// The facts of a parsed hello.
    #[must_use]
    pub fn of(hello: &ClientHello) -> Self {
        Self {
            legacy_version: hello.legacy_version,
            supported_versions: hello.supported_versions.clone(),
            ciphers: hello.cipher_suites.clone(),
            extensions: hello.extension_ids(),
            first_alpn: hello.alpn.first().cloned(),
            signature_algorithms: hello.signature_algorithms.clone(),
        }
    }

    /// The `a` section, for example `t13d1516h2`.
    #[must_use]
    pub fn a(&self) -> String {
        let version = self
            .supported_versions
            .iter()
            .copied()
            .filter(|&version| !is_grease(version))
            .max()
            .unwrap_or(self.legacy_version);
        let sni = if self.extensions.contains(&ext_id::SERVER_NAME) {
            SNI_DOMAIN
        } else {
            SNI_IP
        };
        let ciphers = without_grease(&self.ciphers).len().min(MAX_COUNT);
        let extensions = without_grease(&self.extensions).len().min(MAX_COUNT);
        let alpn = alpn_code(self.first_alpn.as_deref());
        format!(
            "{PROTOCOL_TCP}{}{sni}{ciphers:02}{extensions:02}{alpn}",
            version_code(version)
        )
    }

    /// The unhashed fingerprint: `JA4_r` for [`Ja4Order::Sorted`], `JA4_ro` for the original order.
    #[must_use]
    pub fn raw(&self, order: Ja4Order) -> Ja4Raw {
        let mut ciphers = without_grease(&self.ciphers);
        let mut extensions = without_grease(&self.extensions);
        if order == Ja4Order::Sorted {
            ciphers.sort_unstable();
            extensions.retain(|&id| id != ext_id::SERVER_NAME && id != ext_id::ALPN);
            extensions.sort_unstable();
        }
        Ja4Raw {
            a: self.a(),
            ciphers,
            extensions,
            signature_algorithms: without_grease(&self.signature_algorithms),
        }
    }

    /// The hashed fingerprint: `JA4` for [`Ja4Order::Sorted`], `JA4_o` for the original order.
    #[must_use]
    pub fn ja4(&self, order: Ja4Order) -> Ja4 {
        self.raw(order).hashed()
    }
}

/// An unhashed fingerprint (`JA4_r` or `JA4_ro`): the `a` section and the id lists behind `b` and `c`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ja4Raw {
    /// The `a` section.
    pub a: String,
    /// Cipher suites, GREASE removed.
    pub ciphers: Vec<u16>,
    /// Extensions, GREASE removed; sorted and without SNI and ALPN in the `JA4_r` form.
    pub extensions: Vec<u16>,
    /// Signature algorithms in wire order.
    pub signature_algorithms: Vec<u16>,
}

impl Ja4Raw {
    /// Parses `a_ciphers_extensions[_signature_algorithms]` with ids as four hex digits.
    ///
    /// The string does not say whether it is sorted; the caller knows which variant it holds.
    ///
    /// # Errors
    ///
    /// Returns a [`Ja4Error`] naming the section that is malformed.
    pub fn parse(text: &str) -> Result<Self, Ja4Error> {
        if text.len() > MAX_JA4_LEN {
            return Err(Ja4Error::TooLong);
        }
        let sections: Vec<&str> = text.trim().split(SECTION_SEPARATOR).collect();
        let (a, ciphers, extensions, signature_algorithms) = match sections.as_slice() {
            &[a, ciphers, extensions] => (a, ciphers, extensions, ""),
            &[a, ciphers, extensions, signature_algorithms] => {
                (a, ciphers, extensions, signature_algorithms)
            }
            other => return Err(Ja4Error::Sections(other.len())),
        };
        if !valid_part_a(a) {
            return Err(Ja4Error::PartA(a.to_owned()));
        }
        Ok(Self {
            a: a.to_owned(),
            ciphers: ids(ciphers, Ja4Section::Ciphers)?,
            extensions: ids(extensions, Ja4Section::Extensions)?,
            signature_algorithms: ids(signature_algorithms, Ja4Section::SignatureAlgorithms)?,
        })
    }

    /// The hashed fingerprint of these lists, as JA4 defines `b` and `c`.
    #[must_use]
    pub fn hashed(&self) -> Ja4 {
        let ciphers = if self.ciphers.is_empty() {
            NO_HASH.to_owned()
        } else {
            truncated_sha256(&join_hex(&self.ciphers))
        };
        let extensions = if self.extensions.is_empty() {
            NO_HASH.to_owned()
        } else {
            truncated_sha256(&self.extension_text())
        };
        Ja4 {
            a: self.a.clone(),
            b: ciphers,
            c: extensions,
        }
    }

    /// The extension ids, then `_` and the signature algorithms when there are any.
    fn extension_text(&self) -> String {
        let mut text = join_hex(&self.extensions);
        if !self.signature_algorithms.is_empty() {
            text.push(SECTION_SEPARATOR);
            text.push_str(&join_hex(&self.signature_algorithms));
        }
        text
    }
}

impl fmt::Display for Ja4Raw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}_{}_{}",
            self.a,
            join_hex(&self.ciphers),
            join_hex(&self.extensions)
        )?;
        if self.signature_algorithms.is_empty() {
            return Ok(());
        }
        write!(f, "_{}", join_hex(&self.signature_algorithms))
    }
}

/// A hashed JA4 fingerprint, `a_b_c`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ja4 {
    /// The `a` section, ten characters.
    pub a: String,
    /// Twelve hex digits over the cipher suites.
    pub b: String,
    /// Twelve hex digits over the extensions and signature algorithms.
    pub c: String,
}

impl Ja4 {
    /// The `JA4` of a hello: sorted ciphers and extensions, hashed.
    #[must_use]
    pub fn of(hello: &ClientHello) -> Self {
        Ja4Facts::of(hello).ja4(Ja4Order::Sorted)
    }

    /// The `JA4_o` of a hello: wire order, hashed.
    #[must_use]
    pub fn of_original_order(hello: &ClientHello) -> Self {
        Ja4Facts::of(hello).ja4(Ja4Order::Original)
    }

    /// The `JA4_r` of a hello: sorted, unhashed.
    #[must_use]
    pub fn raw(hello: &ClientHello) -> String {
        Ja4Facts::of(hello).raw(Ja4Order::Sorted).to_string()
    }

    /// The `JA4_ro` of a hello: wire order, unhashed.
    #[must_use]
    pub fn raw_original_order(hello: &ClientHello) -> String {
        Ja4Facts::of(hello).raw(Ja4Order::Original).to_string()
    }

    /// Parses `a_b_c`.
    ///
    /// # Errors
    ///
    /// Returns a [`Ja4Error`] when the sections or their shapes are wrong.
    pub fn parse(text: &str) -> Result<Self, Ja4Error> {
        let sections: Vec<&str> = text.trim().split(SECTION_SEPARATOR).collect();
        let &[a, b, c] = sections.as_slice() else {
            return Err(Ja4Error::Sections(sections.len()));
        };
        if !valid_part_a(a) {
            return Err(Ja4Error::PartA(a.to_owned()));
        }
        for (hash, section) in [(b, Ja4Section::Ciphers), (c, Ja4Section::Extensions)] {
            if !valid_hash(hash) {
                return Err(Ja4Error::Hash(section));
            }
        }
        Ok(Self {
            a: a.to_owned(),
            b: b.to_owned(),
            c: c.to_owned(),
        })
    }

    /// Which parts differ from `other`.
    #[must_use]
    pub fn diff(&self, other: &Ja4) -> Ja4Diff {
        let a_fields = A_FIELDS
            .iter()
            .filter(|&&(_, start, end)| self.a.get(start..end) != other.a.get(start..end))
            .map(|&(field, _, _)| field)
            .collect();
        Ja4Diff {
            a_fields,
            b: self.b != other.b,
            c: self.c != other.c,
        }
    }
}

impl fmt::Display for Ja4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_{}_{}", self.a, self.b, self.c)
    }
}

impl FromStr for Ja4 {
    type Err = Ja4Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

/// A field of the `a` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ja4AField {
    /// The transport letter.
    Protocol,
    /// The TLS version code.
    Version,
    /// The SNI letter.
    Sni,
    /// The cipher suite count.
    CipherCount,
    /// The extension count.
    ExtensionCount,
    /// The ALPN characters.
    Alpn,
}

/// The fields of `a` with their character ranges.
const A_FIELDS: [(Ja4AField, usize, usize); 6] = [
    (Ja4AField::Protocol, 0, 1),
    (Ja4AField::Version, 1, 3),
    (Ja4AField::Sni, 3, 4),
    (Ja4AField::CipherCount, 4, 6),
    (Ja4AField::ExtensionCount, 6, 8),
    (Ja4AField::Alpn, 8, 10),
];

/// The difference between two fingerprints, part by part.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ja4Diff {
    /// The fields of `a` that differ.
    pub a_fields: Vec<Ja4AField>,
    /// Whether the cipher hash differs.
    pub b: bool,
    /// Whether the extension hash differs.
    pub c: bool,
}

impl Ja4Diff {
    /// Whether the fingerprints are equal.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.a_fields.is_empty() && !self.b && !self.c
    }
}

/// Whether `a` has the documented ten-character shape.
fn valid_part_a(a: &str) -> bool {
    let bytes = a.as_bytes();
    bytes.len() == PART_A_LEN
        && PROTOCOLS.contains(&bytes[0])
        && bytes[1].is_ascii_alphanumeric()
        && bytes[2].is_ascii_alphanumeric()
        && (bytes[3] == b'd' || bytes[3] == b'i')
        && bytes[4..8].iter().all(u8::is_ascii_digit)
        && bytes[8].is_ascii_alphanumeric()
        && bytes[9].is_ascii_alphanumeric()
}

/// Whether `hash` is twelve lowercase hex digits.
fn valid_hash(hash: &str) -> bool {
    hash.len() == HASH_HEX_CHARS
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Decodes a comma-separated list of four-digit hex ids; the empty text is the empty list.
fn ids(text: &str, section: Ja4Section) -> Result<Vec<u16>, Ja4Error> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    text.split(ID_SEPARATOR)
        .map(|token| {
            let hex = token.len() == ID_HEX_CHARS && token.bytes().all(|b| b.is_ascii_hexdigit());
            let parsed = if hex {
                u16::from_str_radix(token, 16).ok()
            } else {
                None
            };
            parsed.ok_or_else(|| Ja4Error::Hex {
                section,
                token: token.to_owned(),
            })
        })
        .collect()
}

/// Four lowercase hex digits per id, joined by commas.
fn join_hex(ids: &[u16]) -> String {
    let tokens: Vec<String> = ids.iter().map(|id| format!("{id:04x}")).collect();
    tokens.join(&ID_SEPARATOR.to_string())
}

/// The first 12 hex digits of the SHA-256 of `text`.
fn truncated_sha256(text: &str) -> String {
    let full = digest(&SHA256, text.as_bytes());
    hex_lower(&full.as_ref()[..HASH_BYTES])
}

#[cfg(test)]
mod tests;
