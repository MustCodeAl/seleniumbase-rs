//! Overlays: custom JA3, JA4 and peetprint strings layered over a base `TlsSpec`.
//!
//! Each overlay replaces what its fingerprint carries and keeps the rest of the base, then runs
//! [`TlsSpec::check_reproducible`] and recomputes its own fingerprint to compare with the input.

use std::collections::BTreeSet;

use super::{duplicate, first_duplicate, SpecError, TlsSpec};
use crate::wire::client_hello::ext_id;
use crate::wire::ids::{ALPN_H2, ALPN_HTTP_1_1, EC_POINT_UNCOMPRESSED};
use crate::wire::ja3::peetprint::{ids as slot_ids, PeetField, Slot};
use crate::wire::ja3::{Ja3, Peetprint};
use crate::wire::ja4::{alpn_code, version_from_code, Ja4Order, Ja4Raw, Ja4Section, NO_ALPN};

/// ALPN name the `JA4` ALPN field may stand for besides the spec's own list.
const EXTRA_ALPN: &str = "http/1.0";

/// `base` order for the ids in `wanted`, then the other ids of `wanted` in ascending order.
fn merge_order(base: &[u16], wanted: &[u16]) -> Vec<u16> {
    let wanted_set: BTreeSet<u16> = wanted.iter().copied().collect();
    let base_set: BTreeSet<u16> = base.iter().copied().collect();
    let mut merged: Vec<u16> = base
        .iter()
        .copied()
        .filter(|id| wanted_set.contains(id))
        .collect();
    merged.extend(wanted_set.difference(&base_set));
    merged
}

/// `versions` up to `target`, with `target` included, highest first.
pub(super) fn versions_with_max(versions: &[u16], target: u16) -> Vec<u16> {
    let mut kept: Vec<u16> = versions.iter().copied().filter(|&v| v <= target).collect();
    if !kept.contains(&target) {
        kept.push(target);
    }
    kept.sort_unstable_by(|left, right| right.cmp(left));
    kept
}

/// Rejects repeated ciphers or extensions before a merge would hide them.
fn reject_duplicates(ciphers: &[u16], extensions: &[u16]) -> Result<(), SpecError> {
    if let Some(id) = first_duplicate(ciphers) {
        return Err(duplicate("cipher_suites", id));
    }
    first_duplicate(extensions).map_or(Ok(()), |id| Err(SpecError::DuplicateExtension(id)))
}

/// The fields of a JA4 `a` section that change the spec.
struct PartA {
    version: u16,
    sni: bool,
    alpn: String,
}

impl PartA {
    fn parse(a: &str) -> Result<Self, SpecError> {
        let mismatch = SpecError::Ja4Mismatch(Ja4Section::A);
        let version = a
            .get(1..3)
            .and_then(version_from_code)
            .ok_or_else(|| mismatch.clone())?;
        let alpn = a.get(8..10).ok_or(mismatch)?.to_owned();
        Ok(Self {
            version,
            sni: a.get(3..4) == Some("d"),
            alpn,
        })
    }
}

impl TlsSpec {
    /// Replaces the version, ciphers, extensions, groups and point formats with a custom JA3.
    ///
    /// Everything JA3 does not carry (signature algorithms, ALPN, key shares, versions and the
    /// other extension bodies) comes from `self`; content of extensions the JA3 omits is dropped.
    /// Extension permutation is turned off, so the JA3 on the wire is exact.
    ///
    /// # Errors
    ///
    /// Returns the [`SpecError`] that makes the result not reproducible.
    pub fn with_ja3(&self, ja3: &Ja3) -> Result<Self, SpecError> {
        let mut spec = self.clone();
        spec.legacy_version = ja3.version;
        spec.cipher_suites.clone_from(&ja3.ciphers);
        spec.extensions.clone_from(&ja3.extensions);
        spec.permute_extensions = false;
        spec.align_content();
        spec.supported_groups.clone_from(&ja3.groups);
        spec.point_formats.clone_from(&ja3.point_formats);
        spec.fix_key_shares();
        spec.check_reproducible()?;
        Ok(spec)
    }

    /// Replaces the signature algorithms.
    ///
    /// # Errors
    ///
    /// Returns [`SpecError::UnsupportedSigalg`] for a scheme `BoringSSL` cannot send, and
    /// [`SpecError::ExtensionContent`] when the list is empty or the extension is not sent.
    pub fn with_signature_algorithms(&self, ids: &[u16]) -> Result<Self, SpecError> {
        let mut spec = self.clone();
        spec.signature_algorithms = ids.to_vec();
        spec.check_reproducible()?;
        Ok(spec)
    }

    /// Replaces the ciphers, extensions and signature algorithms with a custom `JA4_r` (sorted)
    /// or `JA4_ro` (original order).
    ///
    /// The sorted form carries no order: the ids keep the order they have in `self`, new ones
    /// follow in ascending order, and `permute_extensions` is kept. The original-order form pins
    /// ciphers and extensions and turns permutation off. The TLS version and the first ALPN name
    /// come from the `a` section; groups and the other bodies come from `self`.
    ///
    /// # Errors
    ///
    /// Returns the [`SpecError`] that makes the result not reproducible, or
    /// [`SpecError::Ja4Mismatch`] when the result would not reproduce `raw`.
    pub fn with_ja4_raw(&self, raw: &Ja4Raw, order: Ja4Order) -> Result<Self, SpecError> {
        let part = PartA::parse(&raw.a)?;
        let mut spec = self.clone();
        if order == Ja4Order::Original {
            spec.cipher_suites.clone_from(&raw.ciphers);
            spec.extensions.clone_from(&raw.extensions);
            spec.permute_extensions = false;
        } else {
            reject_duplicates(&raw.ciphers, &raw.extensions)?;
            let mut wanted = raw.extensions.clone();
            wanted.extend(part.sni.then_some(ext_id::SERVER_NAME));
            wanted.extend((part.alpn != NO_ALPN).then_some(ext_id::ALPN));
            spec.cipher_suites = merge_order(&self.cipher_suites, &raw.ciphers);
            spec.extensions = merge_order(&self.extensions, &wanted);
        }
        spec.align_content();
        spec.signature_algorithms
            .clone_from(&raw.signature_algorithms);
        spec.apply_version(part.version);
        spec.apply_alpn(&part.alpn)?;
        spec.align_groups();
        spec.check_reproducible()?;
        spec.verify_ja4(raw, order)?;
        Ok(spec)
    }

    /// Replaces the ciphers, extensions, groups, versions, ALPN, signature algorithms, PSK modes
    /// and certificate compression with a custom peetprint.
    ///
    /// The extension field is sorted, so extension order comes from `self`. GREASE tokens set the
    /// `grease` flag, and a layout `BoringSSL` would not produce is refused.
    ///
    /// # Errors
    ///
    /// Returns the [`SpecError`] that makes the result not reproducible, or
    /// [`SpecError::PeetprintMismatch`] when the result would not reproduce `peet`.
    pub fn with_peetprint(&self, peet: &Peetprint) -> Result<Self, SpecError> {
        let ciphers = slot_ids(&peet.ciphers);
        let extensions = slot_ids(&peet.extensions);
        reject_duplicates(&ciphers, &extensions)?;
        let mut spec = self.clone();
        spec.cipher_suites = ciphers;
        spec.extensions = merge_order(&self.extensions, &extensions);
        spec.grease = [
            &peet.versions,
            &peet.groups,
            &peet.signature_algorithms,
            &peet.ciphers,
            &peet.extensions,
        ]
        .iter()
        .any(|list| list.contains(&Slot::Grease));
        spec.align_content();
        spec.supported_versions = slot_ids(&peet.versions);
        spec.alpn.clone_from(&peet.alpn);
        spec.supported_groups = slot_ids(&peet.groups);
        spec.signature_algorithms = slot_ids(&peet.signature_algorithms);
        spec.psk_modes.clone_from(&peet.psk_modes);
        spec.cert_compression.clone_from(&peet.cert_compression);
        spec.align_point_formats();
        spec.fix_key_shares();
        spec.check_reproducible()?;
        spec.verify_peetprint(peet)?;
        Ok(spec)
    }

    /// Clears the data of extensions the list no longer sends, and keeps ALPS on its codepoint.
    fn align_content(&mut self) {
        let present: BTreeSet<u16> = self.extensions.iter().copied().collect();
        let has = |id: u16| present.contains(&id);
        if !has(ext_id::SIGNATURE_ALGORITHMS) {
            self.signature_algorithms.clear();
        }
        if !has(ext_id::ALPN) {
            self.alpn.clear();
        }
        if !has(ext_id::SUPPORTED_VERSIONS) {
            self.supported_versions.clear();
        }
        if !has(ext_id::PSK_KEY_EXCHANGE_MODES) {
            self.psk_modes.clear();
        }
        if !has(ext_id::COMPRESS_CERTIFICATE) {
            self.cert_compression.clear();
        }
        if !has(ext_id::RECORD_SIZE_LIMIT) {
            self.record_size_limit = None;
        }
        if !has(ext_id::DELEGATED_CREDENTIAL) {
            self.delegated_credentials.clear();
        }
        let (new, old) = (has(ext_id::ALPS), has(ext_id::ALPS_OLD));
        if !new && !old {
            self.alps = None;
        } else if let Some(alps) = self.alps.as_mut() {
            alps.new_codepoint = new;
        }
    }

    /// Point formats follow extension 11: `[0]` when sent, none otherwise.
    fn align_point_formats(&mut self) {
        if !self.has_extension(ext_id::EC_POINT_FORMATS) {
            self.point_formats.clear();
        } else if self.point_formats.is_empty() {
            self.point_formats = vec![EC_POINT_UNCOMPRESSED];
        }
    }

    /// Groups, point formats and key shares follow their extensions.
    fn align_groups(&mut self) {
        if !self.has_extension(ext_id::SUPPORTED_GROUPS) {
            self.supported_groups.clear();
        }
        self.align_point_formats();
        self.fix_key_shares();
    }

    /// Key shares keep only offered groups; an empty set falls back to the first group.
    fn fix_key_shares(&mut self) {
        if !self.has_extension(ext_id::KEY_SHARE) {
            self.key_share_groups.clear();
            return;
        }
        self.key_share_groups
            .retain(|group| self.supported_groups.contains(group));
        if self.key_share_groups.is_empty() {
            self.key_share_groups.extend(self.supported_groups.first());
        }
    }

    /// Raises or lowers `supported_versions` so the highest version is the one `a` names.
    fn apply_version(&mut self, target: u16) {
        if self.has_extension(ext_id::SUPPORTED_VERSIONS) {
            self.supported_versions = versions_with_max(&self.supported_versions, target);
        }
    }

    /// Puts the ALPN name whose JA4 code is `code` first.
    fn apply_alpn(&mut self, code: &str) -> Result<(), SpecError> {
        if code == NO_ALPN || !self.has_extension(ext_id::ALPN) {
            return Ok(());
        }
        let pick = self
            .alpn
            .iter()
            .map(String::as_str)
            .chain([ALPN_H2, ALPN_HTTP_1_1, EXTRA_ALPN])
            .find(|name| alpn_code(Some(name.as_bytes())) == code)
            .ok_or_else(|| SpecError::Ja4Alpn(code.to_owned()))?
            .to_owned();
        self.alpn.retain(|name| *name != pick);
        self.alpn.insert(0, pick);
        Ok(())
    }

    /// Recomputes the JA4 and names the first section that differs from `raw`.
    fn verify_ja4(&self, raw: &Ja4Raw, order: Ja4Order) -> Result<(), SpecError> {
        let got = self.ja4_raw(order);
        let section = if got.a != raw.a {
            Ja4Section::A
        } else if got.ciphers != raw.ciphers {
            Ja4Section::Ciphers
        } else if got.extensions != raw.extensions {
            Ja4Section::Extensions
        } else if got.signature_algorithms != raw.signature_algorithms {
            Ja4Section::SignatureAlgorithms
        } else {
            return Ok(());
        };
        Err(SpecError::Ja4Mismatch(section))
    }

    /// Recomputes the peetprint and names the first field that differs from `peet`.
    fn verify_peetprint(&self, peet: &Peetprint) -> Result<(), SpecError> {
        let (got, want) = (self.peetprint(), peet.clone().normalized());
        let fields = [
            (PeetField::Versions, got.versions == want.versions),
            (PeetField::Alpn, got.alpn == want.alpn),
            (PeetField::Groups, got.groups == want.groups),
            (
                PeetField::SignatureAlgorithms,
                got.signature_algorithms == want.signature_algorithms,
            ),
            (PeetField::PskModes, got.psk_modes == want.psk_modes),
            (
                PeetField::CertCompression,
                got.cert_compression == want.cert_compression,
            ),
            (PeetField::Ciphers, got.ciphers == want.ciphers),
            (PeetField::Extensions, got.extensions == want.extensions),
        ];
        match fields.into_iter().find(|&(_, same)| !same) {
            Some((field, _)) => Err(SpecError::PeetprintMismatch(field)),
            None => Ok(()),
        }
    }
}
