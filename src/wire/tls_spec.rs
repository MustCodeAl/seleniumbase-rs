//! The TLS specification a client should reproduce: measured from a capture, or overlaid with a custom JA3.
//!
//! A [`TlsSpec`] holds a whole TLS profile as plain data: the ids a `ClientHello` carries and the
//! content of the extensions that have any. [`TlsSpec::from_client_hello`] records it from a
//! parsed hello. A caller can also layer a custom JA3, JA4 (as `JA4_r` or `JA4_ro`) or peetprint
//! over a base spec; each overlay replaces what its fingerprint carries, takes the rest from the
//! base, and then runs [`TlsSpec::check_reproducible`], so a spec `BoringSSL` cannot emit is
//! refused with a named [`SpecError`] before any client exists. The overlay result is also
//! recomputed and compared with the input, so an accepted input reproduces its own fingerprint.
//!
//! The lists never hold GREASE: the `grease` flag says `BoringSSL` adds its GREASE values. The
//! plan's per-extension flags (`ocsp_stapling`, `sct`, `session_ticket`, `renegotiation`,
//! `ech_grease`) are methods that read the extension list, so they cannot disagree with it.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::client_hello::{ext_id, is_grease, without_grease, ClientHello};
use super::ids::{
    BORINGSSL_CIPHERS, BORINGSSL_CLIENT_EXTENSIONS, BORINGSSL_GROUPS, BORINGSSL_SIGALGS,
    EC_POINT_UNCOMPRESSED, ML_DSA_SIGALGS, REFUSED_EXTENSIONS, SIGNATURE_ALGORITHMS,
    TLS12_LEGACY_VERSION, TLS13_CIPHERS,
};
use super::ja3::peetprint::{slots, PeetField, Slot};
use super::ja3::{Ja3, Peetprint};
use super::ja4::{Ja4, Ja4Facts, Ja4Order, Ja4Raw, Ja4Section};

/// Oldest protocol version `BoringSSL` offers.
const TLS_1_0: u16 = 0x0301;
/// TLS 1.3.
const TLS_1_3: u16 = 0x0304;
/// Longest ALPN protocol name (RFC 7301).
const MAX_ALPN_LEN: usize = 255;
/// Why an extension with no `BoringSSL` option is refused.
const UNKNOWN_EXTENSION: &str = "BoringSSL has no option that emits it";
/// Why a GREASE id in an extension list is refused.
const GREASE_EXTENSION: &str = "GREASE is added through the grease flag, not listed";
/// Name shown for a signature scheme the registry does not know.
const UNASSIGNED: &str = "unassigned";
/// GREASE extensions a `BoringSSL` hello carries: one first and one last.
const GREASE_EXTENSIONS: usize = 2;
/// Extensions whose body is data held by another field of the spec.
const CONTENT_EXTENSIONS: [u16; 11] = [
    ext_id::SUPPORTED_GROUPS,
    ext_id::SIGNATURE_ALGORITHMS,
    ext_id::ALPN,
    ext_id::SUPPORTED_VERSIONS,
    ext_id::KEY_SHARE,
    ext_id::PSK_KEY_EXCHANGE_MODES,
    ext_id::COMPRESS_CERTIFICATE,
    ext_id::RECORD_SIZE_LIMIT,
    ext_id::DELEGATED_CREDENTIAL,
    ext_id::ALPS,
    ext_id::ALPS_OLD,
];

/// Why a spec cannot be reproduced, or an overlay cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SpecError {
    /// The handshake version is not TLS 1.2 (771), the only value `BoringSSL` writes.
    #[error("handshake version {0} cannot be emitted; BoringSSL writes 771")]
    LegacyVersion(u16),
    /// `supported_versions` lists a version `BoringSSL` does not offer.
    #[error("protocol version {0:#06x} cannot be offered")]
    UnsupportedVersion(u16),
    /// The cipher suite list is empty.
    #[error("the spec offers no cipher suite")]
    NoCipherSuites,
    /// `BoringSSL` cannot send this cipher suite.
    #[error("cipher suite {0:#06x} cannot be emitted")]
    UnsupportedCipher(u16),
    /// The TLS 1.3 suites are not all present, or do not match `supported_versions`.
    #[error("the three TLS 1.3 suites must be present together, and only when TLS 1.3 is offered")]
    Tls13Suites,
    /// `BoringSSL` cannot send this extension.
    #[error("extension {id:#06x} cannot be emitted: {reason}")]
    UnsupportedExtension {
        /// The extension type.
        id: u16,
        /// Why it cannot be sent.
        reason: &'static str,
    },
    /// An extension is listed twice.
    #[error("extension {0:#06x} is listed twice")]
    DuplicateExtension(u16),
    /// `BoringSSL` cannot offer this supported group.
    #[error("supported group {0:#06x} cannot be offered")]
    UnsupportedGroup(u16),
    /// A key share is sent for a group that `supported_groups` does not offer.
    #[error("a key share for group {0:#06x} needs the group in supported_groups")]
    KeyShareNotOffered(u16),
    /// The point formats are not `[0]` with the extension, or empty without it.
    #[error("point formats {0:?} cannot be emitted; only the uncompressed format exists")]
    PointFormats(Vec<u8>),
    /// `BoringSSL` cannot send this signature scheme.
    #[error("signature algorithm {id:#06x} ({name}) cannot be emitted")]
    UnsupportedSigalg {
        /// The scheme id.
        id: u16,
        /// Its IANA name, or `unassigned`.
        name: &'static str,
    },
    /// An id occurs twice in a list where each may occur once.
    #[error("{list} lists {id:#06x} twice")]
    Duplicate {
        /// The list that repeats the id.
        list: &'static str,
        /// The repeated id.
        id: u16,
    },
    /// An extension is listed without its content, or the content has no extension.
    #[error("extension {0:#06x} and the data it carries do not agree")]
    ExtensionContent(u16),
    /// An ALPN protocol name is empty or longer than 255 bytes.
    #[error("`{0}` is not a valid ALPN protocol name")]
    Alpn(String),
    /// The recomputed `JA4_r` differs from the input in this section.
    #[error("the spec cannot reproduce the JA4 {0} section")]
    Ja4Mismatch(Ja4Section),
    /// No known ALPN name has this two-character `JA4` ALPN code.
    #[error("no known ALPN protocol has the JA4 code `{0}`")]
    Ja4Alpn(String),
    /// The recomputed peetprint differs from the input in this field.
    #[error("the spec cannot reproduce the peetprint {0} field")]
    PeetprintMismatch(PeetField),
    /// Two hellos meant to show one client differ in more than extension order.
    #[error("the hellos differ in their cipher suites or extension sets")]
    HellosDiffer,
}

/// A known difference between a measured browser hello and what `BoringSSL` can send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KnownGap {
    /// The `trust_anchors` extension (0xCA34), which needs a newer btls and wreq.
    TrustAnchorsExtension,
    /// The ML-DSA signature schemes (0x0904 to 0x0906).
    MlDsaSignatureAlgorithms,
}

/// The `application_settings` extension: the protocols it announces and the codepoint it uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alps {
    /// Protocol names, such as `h2`.
    pub protocols: Vec<String>,
    /// Whether the extension uses the new codepoint (0x44CD) rather than the old one (0x4469).
    pub new_codepoint: bool,
}

/// A whole TLS profile: what a `ClientHello` must carry for a client to look like this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsSpec {
    /// The handshake `client_version` (771).
    pub legacy_version: u16,
    /// Cipher suites in wire order, without GREASE.
    pub cipher_suites: Vec<u16>,
    /// Extension ids in wire order, without GREASE; SNI and ALPN are listed when sent.
    pub extensions: Vec<u16>,
    /// Whether the client shuffles the extension order on every connection, as Chrome does.
    #[serde(default)]
    pub permute_extensions: bool,
    /// Whether GREASE values are added (cipher, group, version, two extensions).
    #[serde(default)]
    pub grease: bool,
    /// Supported groups in preference order.
    #[serde(default)]
    pub supported_groups: Vec<u16>,
    /// Groups a key share is sent for, in order.
    #[serde(default)]
    pub key_share_groups: Vec<u16>,
    /// EC point formats: `[0]` when the extension is sent.
    #[serde(default)]
    pub point_formats: Vec<u8>,
    /// Signature algorithms in preference order.
    #[serde(default)]
    pub signature_algorithms: Vec<u16>,
    /// ALPN protocol names in preference order.
    #[serde(default)]
    pub alpn: Vec<String>,
    /// The `application_settings` extension, when sent.
    #[serde(default)]
    pub alps: Option<Alps>,
    /// `delegated_credential` signature schemes.
    #[serde(default)]
    pub delegated_credentials: Vec<u16>,
    /// `supported_versions`, highest first, without GREASE.
    #[serde(default)]
    pub supported_versions: Vec<u16>,
    /// `psk_key_exchange_modes`.
    #[serde(default)]
    pub psk_modes: Vec<u8>,
    /// `compress_certificate` algorithms.
    #[serde(default)]
    pub cert_compression: Vec<u16>,
    /// `record_size_limit`.
    #[serde(default)]
    pub record_size_limit: Option<u16>,
}

fn strings(items: &[Vec<u8>]) -> Vec<String> {
    items
        .iter()
        .map(|item| String::from_utf8_lossy(item).into_owned())
        .collect()
}

fn duplicate(list: &'static str, id: u16) -> SpecError {
    SpecError::Duplicate { list, id }
}

/// The first id that occurs twice in `ids`.
fn first_duplicate(ids: &[u16]) -> Option<u16> {
    let mut seen = BTreeSet::new();
    ids.iter().copied().find(|&id| !seen.insert(id))
}

impl TlsSpec {
    /// Records the profile a parsed hello shows; GREASE becomes the `grease` flag.
    ///
    /// The extension order is taken as observed; see [`TlsSpec::with_observed_order`] for
    /// detecting the per-connection shuffle Chrome applies.
    #[must_use]
    pub fn from_client_hello(hello: &ClientHello) -> Self {
        let new_codepoint = hello.has_extension(ext_id::ALPS);
        let alps = (new_codepoint || hello.has_extension(ext_id::ALPS_OLD)).then(|| Alps {
            protocols: strings(&hello.alps),
            new_codepoint,
        });
        Self {
            legacy_version: hello.legacy_version,
            cipher_suites: without_grease(&hello.cipher_suites),
            extensions: without_grease(&hello.extension_ids()),
            permute_extensions: false,
            grease: hello.has_grease(),
            supported_groups: without_grease(&hello.supported_groups),
            key_share_groups: without_grease(&hello.key_share_groups),
            point_formats: hello.point_formats.clone(),
            signature_algorithms: without_grease(&hello.signature_algorithms),
            alpn: strings(&hello.alpn),
            alps,
            delegated_credentials: hello.delegated_credentials.clone(),
            supported_versions: without_grease(&hello.supported_versions),
            psk_modes: hello.psk_modes.clone(),
            cert_compression: hello.cert_compression.clone(),
            record_size_limit: hello.record_size_limit,
        }
    }

    /// Compares a second hello of the same client: a changed extension order sets
    /// `permute_extensions`.
    ///
    /// # Errors
    ///
    /// Returns [`SpecError::HellosDiffer`] when the cipher suites or the set of extensions differ.
    pub fn with_observed_order(&self, other: &ClientHello) -> Result<Self, SpecError> {
        let extensions = without_grease(&other.extension_ids());
        let same_set = extensions.iter().collect::<BTreeSet<_>>()
            == self.extensions.iter().collect::<BTreeSet<_>>();
        if !same_set || without_grease(&other.cipher_suites) != self.cipher_suites {
            return Err(SpecError::HellosDiffer);
        }
        let mut spec = self.clone();
        spec.permute_extensions = self.permute_extensions || extensions != self.extensions;
        Ok(spec)
    }

    /// Whether the spec sends the extension with this id.
    #[must_use]
    pub fn has_extension(&self, id: u16) -> bool {
        self.extensions.contains(&id)
    }

    /// Whether `status_request` (OCSP stapling) is sent.
    #[must_use]
    pub fn ocsp_stapling(&self) -> bool {
        self.has_extension(ext_id::STATUS_REQUEST)
    }

    /// Whether `signed_certificate_timestamp` is sent.
    #[must_use]
    pub fn sct(&self) -> bool {
        self.has_extension(ext_id::SIGNED_CERT_TIMESTAMP)
    }

    /// Whether `session_ticket` is sent.
    #[must_use]
    pub fn session_ticket(&self) -> bool {
        self.has_extension(ext_id::SESSION_TICKET)
    }

    /// Whether `renegotiation_info` is sent.
    #[must_use]
    pub fn renegotiation(&self) -> bool {
        self.has_extension(ext_id::RENEGOTIATION_INFO)
    }

    /// Whether `encrypted_client_hello` is sent (as GREASE ECH).
    #[must_use]
    pub fn ech_grease(&self) -> bool {
        self.has_extension(ext_id::ENCRYPTED_CLIENT_HELLO)
    }

    /// Whether `extended_master_secret` is sent.
    #[must_use]
    pub fn extended_master_secret(&self) -> bool {
        self.has_extension(ext_id::EXTENDED_MASTER_SECRET)
    }

    /// The JA3 this spec produces (GREASE removed, extensions in the listed order).
    #[must_use]
    pub fn ja3(&self) -> Ja3 {
        Ja3 {
            version: self.legacy_version,
            ciphers: self.cipher_suites.clone(),
            extensions: self.extensions.clone(),
            groups: self.supported_groups.clone(),
            point_formats: self.point_formats.clone(),
        }
    }

    /// What JA4 reads from this spec.
    #[must_use]
    pub fn ja4_facts(&self) -> Ja4Facts {
        Ja4Facts {
            legacy_version: self.legacy_version,
            supported_versions: self.supported_versions.clone(),
            ciphers: self.cipher_suites.clone(),
            extensions: self.extensions.clone(),
            first_alpn: self.alpn.first().map(|name| name.as_bytes().to_vec()),
            signature_algorithms: self.signature_algorithms.clone(),
        }
    }

    /// The `JA4` this spec produces.
    #[must_use]
    pub fn ja4(&self) -> Ja4 {
        self.ja4_facts().ja4(Ja4Order::Sorted)
    }

    /// The unhashed JA4 in the given order.
    #[must_use]
    pub fn ja4_raw(&self, order: Ja4Order) -> Ja4Raw {
        self.ja4_facts().raw(order)
    }

    /// The peetprint this spec produces; with `grease` it carries the GREASE tokens `BoringSSL`
    /// adds (one each in versions, groups and ciphers, two in the extensions).
    #[must_use]
    pub fn peetprint(&self) -> Peetprint {
        Peetprint {
            versions: self.greased(&self.supported_versions, 1),
            alpn: self.alpn.clone(),
            groups: self.greased(&self.supported_groups, 1),
            signature_algorithms: slots(&self.signature_algorithms),
            psk_modes: self.psk_modes.clone(),
            cert_compression: self.cert_compression.clone(),
            ciphers: self.greased(&self.cipher_suites, 1),
            extensions: self.greased(&self.extensions, GREASE_EXTENSIONS),
        }
        .normalized()
    }

    fn greased(&self, ids: &[u16], count: usize) -> Vec<Slot> {
        let lead = if self.grease && !ids.is_empty() {
            count
        } else {
            0
        };
        std::iter::repeat_n(Slot::Grease, lead)
            .chain(slots(ids))
            .collect()
    }

    /// Removes the listed known gaps: the parts of a measured hello `BoringSSL` cannot send.
    #[must_use]
    pub fn without_known_gaps(&self, gaps: &[KnownGap]) -> Self {
        let mut spec = self.clone();
        for gap in gaps {
            match gap {
                KnownGap::TrustAnchorsExtension => {
                    spec.extensions.retain(|&id| id != ext_id::TRUST_ANCHORS);
                }
                KnownGap::MlDsaSignatureAlgorithms => {
                    spec.signature_algorithms
                        .retain(|id| !ML_DSA_SIGALGS.contains(id));
                    spec.delegated_credentials
                        .retain(|id| !ML_DSA_SIGALGS.contains(id));
                }
            }
        }
        spec
    }
}

impl TlsSpec {
    /// Checks the spec against what `BoringSSL` can emit (plan section 3.8).
    ///
    /// # Errors
    ///
    /// Returns the first [`SpecError`] found: the version, the cipher suites, the extensions, the
    /// groups, the point formats, the signature algorithms, then extension content.
    pub fn check_reproducible(&self) -> Result<(), SpecError> {
        self.check_version()?;
        self.check_ciphers()?;
        self.check_extensions()?;
        self.check_groups()?;
        self.check_point_formats()?;
        self.check_signature_algorithms()?;
        self.check_content()
    }

    fn check_version(&self) -> Result<(), SpecError> {
        if self.legacy_version != TLS12_LEGACY_VERSION {
            return Err(SpecError::LegacyVersion(self.legacy_version));
        }
        let mut seen = BTreeSet::new();
        for &version in &self.supported_versions {
            if !(TLS_1_0..=TLS_1_3).contains(&version) {
                return Err(SpecError::UnsupportedVersion(version));
            }
            if !seen.insert(version) {
                return Err(duplicate("supported_versions", version));
            }
        }
        Ok(())
    }

    fn check_ciphers(&self) -> Result<(), SpecError> {
        if self.cipher_suites.is_empty() {
            return Err(SpecError::NoCipherSuites);
        }
        let mut seen = BTreeSet::new();
        for &id in &self.cipher_suites {
            if !BORINGSSL_CIPHERS.contains(&id) {
                return Err(SpecError::UnsupportedCipher(id));
            }
            if !seen.insert(id) {
                return Err(duplicate("cipher_suites", id));
            }
        }
        let present = TLS13_CIPHERS.iter().filter(|id| seen.contains(id)).count();
        let complete = present == TLS13_CIPHERS.len();
        let wants_tls13 = self.supported_versions.contains(&TLS_1_3);
        if (present != 0 && !complete) || complete != wants_tls13 {
            return Err(SpecError::Tls13Suites);
        }
        Ok(())
    }

    fn check_extensions(&self) -> Result<(), SpecError> {
        let mut seen = BTreeSet::new();
        for &id in &self.extensions {
            let refused = REFUSED_EXTENSIONS
                .iter()
                .find(|&&(refused, _)| refused == id);
            let reason = if let Some(&(_, reason)) = refused {
                Some(reason)
            } else if is_grease(id) {
                Some(GREASE_EXTENSION)
            } else if !BORINGSSL_CLIENT_EXTENSIONS.contains(&id) {
                Some(UNKNOWN_EXTENSION)
            } else {
                None
            };
            if let Some(reason) = reason {
                return Err(SpecError::UnsupportedExtension { id, reason });
            }
            if !seen.insert(id) {
                return Err(SpecError::DuplicateExtension(id));
            }
        }
        Ok(())
    }

    fn check_groups(&self) -> Result<(), SpecError> {
        let mut offered = BTreeSet::new();
        for &id in &self.supported_groups {
            if !BORINGSSL_GROUPS.contains(&id) {
                return Err(SpecError::UnsupportedGroup(id));
            }
            if !offered.insert(id) {
                return Err(duplicate("supported_groups", id));
            }
        }
        if let Some(id) = first_duplicate(&self.key_share_groups) {
            return Err(duplicate("key_share_groups", id));
        }
        match self
            .key_share_groups
            .iter()
            .find(|id| !offered.contains(id))
        {
            Some(&id) => Err(SpecError::KeyShareNotOffered(id)),
            None => Ok(()),
        }
    }

    fn check_point_formats(&self) -> Result<(), SpecError> {
        let expected: &[u8] = if self.has_extension(ext_id::EC_POINT_FORMATS) {
            &[EC_POINT_UNCOMPRESSED]
        } else {
            &[]
        };
        if self.point_formats == expected {
            Ok(())
        } else {
            Err(SpecError::PointFormats(self.point_formats.clone()))
        }
    }

    fn check_signature_algorithms(&self) -> Result<(), SpecError> {
        let mut seen = BTreeSet::new();
        for &id in &self.signature_algorithms {
            check_sigalg(id)?;
            if !seen.insert(id) {
                return Err(duplicate("signature_algorithms", id));
            }
        }
        self.delegated_credentials
            .iter()
            .try_for_each(|&id| check_sigalg(id))
    }

    /// Whether the field behind `id` holds data; `None` for an extension with no body to hold.
    fn content_filled(&self, id: u16) -> Option<bool> {
        let alps = |new: bool| {
            self.alps
                .as_ref()
                .is_some_and(|alps| alps.new_codepoint == new && !alps.protocols.is_empty())
        };
        Some(match id {
            ext_id::SUPPORTED_GROUPS => !self.supported_groups.is_empty(),
            ext_id::SIGNATURE_ALGORITHMS => !self.signature_algorithms.is_empty(),
            ext_id::ALPN => !self.alpn.is_empty(),
            ext_id::SUPPORTED_VERSIONS => !self.supported_versions.is_empty(),
            ext_id::KEY_SHARE => !self.key_share_groups.is_empty(),
            ext_id::PSK_KEY_EXCHANGE_MODES => !self.psk_modes.is_empty(),
            ext_id::COMPRESS_CERTIFICATE => !self.cert_compression.is_empty(),
            ext_id::RECORD_SIZE_LIMIT => self.record_size_limit.is_some(),
            ext_id::DELEGATED_CREDENTIAL => !self.delegated_credentials.is_empty(),
            ext_id::ALPS => alps(true),
            ext_id::ALPS_OLD => alps(false),
            _ => return None,
        })
    }

    fn check_content(&self) -> Result<(), SpecError> {
        for id in CONTENT_EXTENSIONS {
            if self
                .content_filled(id)
                .is_some_and(|filled| filled != self.has_extension(id))
            {
                return Err(SpecError::ExtensionContent(id));
            }
        }
        match self
            .alpn
            .iter()
            .find(|name| name.is_empty() || name.len() > MAX_ALPN_LEN)
        {
            Some(name) => Err(SpecError::Alpn(name.clone())),
            None => Ok(()),
        }
    }
}

/// Refuses a signature scheme `BoringSSL` cannot send, naming it.
fn check_sigalg(id: u16) -> Result<(), SpecError> {
    if ML_DSA_SIGALGS.contains(&id) || !BORINGSSL_SIGALGS.contains(&id) {
        let name = SIGNATURE_ALGORITHMS.name(id).unwrap_or(UNASSIGNED);
        return Err(SpecError::UnsupportedSigalg { id, name });
    }
    Ok(())
}

mod overlay;

#[cfg(test)]
mod overlay_tests;
#[cfg(test)]
mod tests;
