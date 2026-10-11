//! The per-process certificate authority and the leaves it issues.

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use btls::asn1::{Asn1Integer, Asn1Time};
use btls::bn::BigNum;
use btls::ec::{EcGroup, EcKey};
use btls::hash::MessageDigest;
use btls::nid::Nid;
use btls::pkey::{HasPublic, PKey, PKeyRef, Private};
use btls::ssl::SslRef;
use btls::x509::extension::{
    AuthorityKeyIdentifier, BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName,
    SubjectKeyIdentifier,
};
use btls::x509::{X509Builder, X509Extension, X509Name, X509NameRef, X509Ref, X509};
use url::Host;

use super::cache::LeafCache;
use super::host::LeafName;
use super::{CaOptions, TlsServerError, CLOCK_SKEW_ALLOWANCE};
use crate::sb_cdp::sync::locked;

/// The organization in every subject this module writes.
const ORGANIZATION: &str = "seleniumbase-rs";
/// The CA's common name; it says what the CA is for.
const CA_COMMON_NAME: &str = "seleniumbase-rs ephemeral loopback CA";
/// The longest commonName X.509 allows (RFC 5280 `ub-common-name`); longer hosts get no CN.
const MAX_COMMON_NAME_LEN: usize = 64;
/// Random bytes in a serial number (RFC 5280 allows up to 20 octets).
const SERIAL_BYTES: usize = 16;
/// Clears the sign bit of the first serial byte, so the serial is positive.
const SERIAL_SIGN_MASK: u8 = 0x7f;
/// Sets the next bit, so the serial keeps its full length.
const SERIAL_LENGTH_BIT: u8 = 0x40;
/// The version field of an X.509 v3 certificate (the field counts from zero).
const X509_VERSION_3: i32 = 2;
/// The Chrome switch that trusts a certificate chain by the SHA-256 of an SPKI in it.
const CHROME_SPKI_SWITCH: &str = "--ignore-certificate-errors-spki-list";
/// What `Debug` prints in place of key material.
const REDACTED: &str = "<redacted>";

/// A certificate authority that lives in memory for one process, for loopback TLS only.
///
/// P-256 key, critical `CA:TRUE, pathlen:0`, critical `keyCertSign, cRLSign`, a random 128-bit
/// serial and a short validity ([`CaOptions::ca_validity`], 7 days by default). One shared P-256
/// key is certified by every leaf. Neither key is ever written to disk or printed: `Debug`
/// shows the SPKI pins and the validity only. Share it between connections with an `Arc`.
pub struct EphemeralCa {
    /// The self-signed CA certificate.
    pub(super) cert: X509,
    /// The CA's signing key.
    pub(super) key: PKey<Private>,
    /// The key every leaf certifies.
    pub(super) leaf_key: PKey<Private>,
    /// Base64 SHA-256 SPKI pins of the CA key and of the leaf key.
    pins: [String; 2],
    /// The lifetimes and cache bound this CA was created with.
    options: CaOptions,
    /// The CA certificate's validity window.
    validity: Validity,
    /// Issued leaves by host.
    leaves: Mutex<LeafCache<Arc<Leaf>>>,
}

impl EphemeralCa {
    /// Creates a CA with [`CaOptions::default`], valid from now.
    ///
    /// # Errors
    ///
    /// [`TlsServerError::Ssl`] when `BoringSSL` cannot generate the keys or sign the
    /// certificate, [`TlsServerError::ClockOutOfRange`] when the system clock is unusable.
    pub fn generate() -> Result<Self, TlsServerError> {
        Self::generate_at(CaOptions::default(), SystemTime::now())
    }

    /// Creates a CA with `options`, valid from `now` (backdated by [`CLOCK_SKEW_ALLOWANCE`]).
    ///
    /// # Errors
    ///
    /// [`TlsServerError::InvalidOptions`] when `options` break their bounds,
    /// [`TlsServerError::ClockOutOfRange`] when `now` cannot be put in a certificate,
    /// [`TlsServerError::Ssl`] when `BoringSSL` cannot generate the keys or sign.
    pub fn generate_at(options: CaOptions, now: SystemTime) -> Result<Self, TlsServerError> {
        options.validate()?;
        let validity = Validity::for_ca(unix_seconds(now)?, &options)?;
        let key = new_p256_key()?;
        let leaf_key = new_p256_key()?;
        let cert = build_ca_cert(&key, validity)?;
        let pins = [spki_pin(&key)?, spki_pin(&leaf_key)?];
        Ok(Self {
            cert,
            key,
            leaf_key,
            pins,
            options,
            validity,
            leaves: Mutex::new(LeafCache::new(options.leaf_capacity)),
        })
    }

    /// The pins of the two public keys: base64 of the SHA-256 of the `SubjectPublicKeyInfo` of
    /// the CA key and of the shared leaf key, in that order.
    ///
    /// Chrome needs the leaf pin only (see [`EphemeralCa::chrome_spki_arg`]). Do not put the CA
    /// pin in Chrome's list: the server never sends the CA certificate, and a pinned key is a key
    /// that any chain carrying a certificate with it passes with.
    #[must_use]
    pub fn spki_pins(&self) -> [String; 2] {
        self.pins.clone()
    }

    /// The Chrome switch for this CA's leaves:
    /// `--ignore-certificate-errors-spki-list=<leaf pin>`. Chrome honours it only together with
    /// `--user-data-dir`.
    ///
    /// # What the switch trusts
    ///
    /// Chrome hashes the `SubjectPublicKeyInfo` of every certificate in the chain the server
    /// presents. If one hash is in the list, Chrome accepts the connection and runs no other
    /// check: no signature, host name, validity or revocation. So the switch trusts every
    /// chain that contains a certificate with the leaf public key, whoever sends it and whether
    /// or not they hold the private key. The public key is not secret: it is in every
    /// handshake and in [`Leaf::certificate`]. The pin is safe only while every connection of
    /// that Chrome profile ends at this server, so the proxy that uses it must keep all of
    /// Chrome's traffic, loopback hosts and QUIC included, on the MITM, and nobody may log
    /// or export a leaf certificate. This follows the Chromium source
    /// (`services/network/ignore_errors_cert_verifier.cc`); it was not measured on Chrome 155.
    ///
    /// Only the leaf key is pinned and the server presents the leaf alone, so the CA key is not
    /// a second way in.
    #[must_use]
    pub fn chrome_spki_arg(&self) -> String {
        format!("{CHROME_SPKI_SWITCH}={}", self.leaf_spki_pin())
    }

    /// The pin of the shared leaf key: base64 of the SHA-256 of its `SubjectPublicKeyInfo`.
    #[must_use]
    pub fn leaf_spki_pin(&self) -> &str {
        &self.pins[1]
    }

    /// The self-signed CA certificate.
    #[must_use]
    pub fn certificate(&self) -> &X509Ref {
        &self.cert
    }

    /// The lifetimes and cache bound this CA was created with.
    #[must_use]
    pub fn options(&self) -> CaOptions {
        self.options
    }

    /// The end of the CA's validity, in seconds since the Unix epoch.
    #[must_use]
    pub fn not_after_unix(&self) -> i64 {
        self.validity.not_after
    }

    /// How many leaves the cache holds now.
    #[must_use]
    pub fn cached_leaves(&self) -> usize {
        locked(&self.leaves).count()
    }

    /// The leaf for `host` at `now`: the cached one while it is valid for longer than
    /// [`CaOptions::leaf_renew_margin`], otherwise a newly issued one.
    ///
    /// The leaf names exactly `host` in its SAN (a DNS name or an IP address), has the EKU
    /// `serverAuth`, certifies the shared leaf key and expires after
    /// [`CaOptions::leaf_validity`] or with the CA, whichever comes first. The cache holds
    /// [`CaOptions::leaf_capacity`] leaves and evicts the least recently used one.
    ///
    /// # Errors
    ///
    /// [`TlsServerError::InvalidHost`] for a wildcard, an empty label or a name longer than
    /// 253 bytes, [`TlsServerError::CaExpired`] when the CA expires within the renew margin,
    /// [`TlsServerError::ClockOutOfRange`] for an unusable `now`, [`TlsServerError::Ssl`] when
    /// signing fails.
    pub fn leaf_for(
        &self,
        host: &Host<String>,
        now: SystemTime,
    ) -> Result<Arc<Leaf>, TlsServerError> {
        let name = LeafName::from_host(host)?;
        let now = unix_seconds(now)?;
        let margin = seconds(self.options.leaf_renew_margin)?;
        let key = name.cache_key();
        let mut cache = locked(&self.leaves);
        if let Some(leaf) = cache
            .get(&key)
            .filter(|leaf| leaf.validity.is_fresh(now, margin))
        {
            return Ok(leaf);
        }
        let leaf = Arc::new(self.issue(name, now)?);
        cache.insert(key, Arc::clone(&leaf));
        Ok(leaf)
    }

    /// Signs a new leaf for `name`, valid from `now`.
    fn issue(&self, name: LeafName, now: i64) -> Result<Leaf, TlsServerError> {
        let validity = Validity::for_leaf(now, self.validity.not_after, &self.options)?;
        let cert = build_leaf_cert(self, &name, validity)?;
        Ok(Leaf {
            name,
            cert,
            issuer: self.cert.clone(),
            key: self.leaf_key.clone(),
            validity,
        })
    }
}

impl fmt::Debug for EphemeralCa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EphemeralCa")
            .field("spki_pins", &self.pins)
            .field("not_before", &self.validity.not_before)
            .field("not_after", &self.validity.not_after)
            .field("options", &self.options)
            .field("keys", &REDACTED)
            .finish_non_exhaustive()
    }
}

/// One leaf certificate for one CONNECT host, signed by an [`EphemeralCa`].
///
/// It holds a reference to the shared leaf key for the handshake; `Debug` never shows it.
pub struct Leaf {
    /// The host the SAN names.
    name: LeafName,
    /// The leaf certificate.
    cert: X509,
    /// The CA certificate that signed the leaf. It is never sent: the server presents the leaf alone.
    issuer: X509,
    /// The shared leaf key.
    pub(super) key: PKey<Private>,
    /// The leaf's validity window.
    validity: Validity,
}

impl Leaf {
    /// The leaf certificate.
    #[must_use]
    pub fn certificate(&self) -> &X509Ref {
        &self.cert
    }

    /// The CA certificate that signed it. The server does not send it: the chain it presents
    /// holds the leaf alone, so a client must trust the CA by other means.
    #[must_use]
    pub fn issuer(&self) -> &X509Ref {
        &self.issuer
    }

    /// The host the SAN names: a lowercase DNS name or an IP address.
    #[must_use]
    pub fn host(&self) -> String {
        self.name.to_string()
    }

    /// The start of the leaf's validity, in seconds since the Unix epoch.
    #[must_use]
    pub fn not_before_unix(&self) -> i64 {
        self.validity.not_before
    }

    /// The end of the leaf's validity, in seconds since the Unix epoch.
    #[must_use]
    pub fn not_after_unix(&self) -> i64 {
        self.validity.not_after
    }

    /// The validated host, for the SNI check.
    pub(super) fn name(&self) -> &LeafName {
        &self.name
    }

    /// Sets the certificate and the key on one connection. No chain is added: the leaf goes
    /// alone, so no certificate other than the leaf carries a key Chrome has pinned.
    pub(super) fn install(&self, ssl: &mut SslRef) -> Result<(), TlsServerError> {
        ssl.set_certificate(&self.cert)?;
        ssl.set_private_key(&self.key)?;
        Ok(())
    }
}

impl fmt::Debug for Leaf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Leaf")
            .field("host", &self.name.to_string())
            .field("not_before", &self.validity.not_before)
            .field("not_after", &self.validity.not_after)
            .field("key", &REDACTED)
            .finish_non_exhaustive()
    }
}

/// The Chrome SPKI pin of a certificate: base64 of the SHA-256 of its `SubjectPublicKeyInfo`.
///
/// # Errors
///
/// [`TlsServerError::Ssl`] when `BoringSSL` cannot read or encode the public key.
pub fn spki_pin_of(cert: &X509Ref) -> Result<String, TlsServerError> {
    let public = cert.public_key()?;
    spki_pin(&public)
}

/// A validity window in seconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Validity {
    /// The first second the certificate is valid.
    pub(super) not_before: i64,
    /// The last second the certificate is valid.
    pub(super) not_after: i64,
}

impl Validity {
    /// The CA window: from `now` minus the skew allowance, for `ca_validity`.
    fn for_ca(now: i64, options: &CaOptions) -> Result<Self, TlsServerError> {
        Ok(Self {
            not_before: shift(now, CLOCK_SKEW_ALLOWANCE, i64::checked_sub)?,
            not_after: shift(now, options.ca_validity, i64::checked_add)?,
        })
    }

    /// A leaf window from `now`, capped at the CA's end; refused when the CA ends within the
    /// renew margin.
    pub(super) fn for_leaf(
        now: i64,
        ca_not_after: i64,
        options: &CaOptions,
    ) -> Result<Self, TlsServerError> {
        if ca_not_after.saturating_sub(now) <= seconds(options.leaf_renew_margin)? {
            return Err(TlsServerError::CaExpired);
        }
        Ok(Self {
            not_before: shift(now, CLOCK_SKEW_ALLOWANCE, i64::checked_sub)?,
            not_after: shift(now, options.leaf_validity, i64::checked_add)?.min(ca_not_after),
        })
    }

    /// Whether a certificate with this window may still be served at `now` without renewal.
    pub(super) fn is_fresh(&self, now: i64, margin: i64) -> bool {
        self.not_before <= now && now.saturating_add(margin) < self.not_after
    }
}

/// `now` moved by `by` in the direction of `op`.
fn shift(now: i64, by: Duration, op: fn(i64, i64) -> Option<i64>) -> Result<i64, TlsServerError> {
    op(now, seconds(by)?).ok_or(TlsServerError::ClockOutOfRange)
}

/// Whole seconds of `duration` as a signed count.
fn seconds(duration: Duration) -> Result<i64, TlsServerError> {
    i64::try_from(duration.as_secs())
        .ok()
        .ok_or(TlsServerError::ClockOutOfRange)
}

/// Seconds since the Unix epoch.
pub(super) fn unix_seconds(now: SystemTime) -> Result<i64, TlsServerError> {
    let since_epoch = now
        .duration_since(UNIX_EPOCH)
        .ok()
        .ok_or(TlsServerError::ClockOutOfRange)?;
    seconds(since_epoch)
}

/// A fresh P-256 key.
fn new_p256_key() -> Result<PKey<Private>, TlsServerError> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;
    Ok(PKey::from_ec_key(EcKey::generate(&group)?)?)
}

/// Base64 of the SHA-256 of the DER `SubjectPublicKeyInfo` of `key`.
fn spki_pin<T: HasPublic>(key: &PKeyRef<T>) -> Result<String, TlsServerError> {
    let spki = key.public_key_to_der()?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &spki);
    Ok(base64::engine::general_purpose::STANDARD.encode(digest.as_ref()))
}

/// A random positive serial number of [`SERIAL_BYTES`] bytes.
fn random_serial() -> Result<Asn1Integer, TlsServerError> {
    let mut bytes = [0_u8; SERIAL_BYTES];
    btls::rand::rand_bytes(&mut bytes)?;
    if let Some(first) = bytes.first_mut() {
        *first = (*first & SERIAL_SIGN_MASK) | SERIAL_LENGTH_BIT;
    }
    Ok(BigNum::from_slice(&bytes)?.to_asn1_integer()?)
}

/// An ASN.1 time for `unix` seconds.
#[allow(
    clippy::useless_conversion,
    reason = "time_t is i64 on 64-bit targets and i32 on some 32-bit ones"
)]
fn asn1_time(unix: i64) -> Result<Asn1Time, TlsServerError> {
    let time = unix
        .try_into()
        .ok()
        .ok_or(TlsServerError::ClockOutOfRange)?;
    Ok(Asn1Time::from_unix(time)?)
}

/// A distinguished name from `(field, value)` pairs.
fn distinguished_name(entries: &[(Nid, &str)]) -> Result<X509Name, TlsServerError> {
    let mut name = X509Name::builder()?;
    for &(field, value) in entries {
        name.append_entry_by_nid(field, value)?;
    }
    Ok(name.build())
}

/// A v3 certificate builder with serial, names, key and validity set.
fn certificate_builder(
    subject: &X509NameRef,
    issuer: &X509NameRef,
    key: &PKeyRef<Private>,
    validity: Validity,
) -> Result<X509Builder, TlsServerError> {
    let serial = random_serial()?;
    let not_before = asn1_time(validity.not_before)?;
    let not_after = asn1_time(validity.not_after)?;
    let mut builder = X509::builder()?;
    builder.set_version(X509_VERSION_3)?;
    builder.set_serial_number(&serial)?;
    builder.set_subject_name(subject)?;
    builder.set_issuer_name(issuer)?;
    builder.set_pubkey(key)?;
    builder.set_not_before(&not_before)?;
    builder.set_not_after(&not_after)?;
    Ok(builder)
}

/// The self-signed CA certificate for `key`.
fn build_ca_cert(key: &PKeyRef<Private>, validity: Validity) -> Result<X509, TlsServerError> {
    let subject = distinguished_name(&[
        (Nid::ORGANIZATIONNAME, ORGANIZATION),
        (Nid::COMMONNAME, CA_COMMON_NAME),
    ])?;
    let mut builder = certificate_builder(&subject, &subject, key, validity)?;
    let constraints = BasicConstraints::new().critical().ca().pathlen(0).build()?;
    let usage = KeyUsage::new()
        .critical()
        .key_cert_sign()
        .crl_sign()
        .build()?;
    let key_id = SubjectKeyIdentifier::new().build(&builder.x509v3_context(None, None))?;
    append_extensions(&mut builder, &[constraints, usage, key_id])?;
    builder.sign(key, MessageDigest::sha256())?;
    Ok(builder.build())
}

/// A leaf for `name`, certifying the shared leaf key, signed by `ca`.
fn build_leaf_cert(
    ca: &EphemeralCa,
    name: &LeafName,
    validity: Validity,
) -> Result<X509, TlsServerError> {
    let subject = leaf_subject(name)?;
    let mut builder =
        certificate_builder(&subject, ca.cert.subject_name(), &ca.leaf_key, validity)?;
    let constraints = BasicConstraints::new().critical().build()?;
    let usage = KeyUsage::new().critical().digital_signature().build()?;
    let extended_usage = ExtendedKeyUsage::new().server_auth().build()?;
    let (san, authority_key_id) = {
        let context = builder.x509v3_context(Some(&*ca.cert), None);
        let san = subject_alt_name(name).build(&context)?;
        let authority_key_id = AuthorityKeyIdentifier::new().keyid(false).build(&context)?;
        (san, authority_key_id)
    };
    append_extensions(
        &mut builder,
        &[constraints, usage, extended_usage, san, authority_key_id],
    )?;
    builder.sign(&ca.key, MessageDigest::sha256())?;
    Ok(builder.build())
}

/// Appends `extensions` to `builder` in order.
fn append_extensions(
    builder: &mut X509Builder,
    extensions: &[X509Extension],
) -> Result<(), TlsServerError> {
    for extension in extensions {
        builder.append_extension(extension)?;
    }
    Ok(())
}

/// The leaf subject: the organization, plus the host as commonName when it fits.
fn leaf_subject(name: &LeafName) -> Result<X509Name, TlsServerError> {
    let host = name.to_string();
    if host.len() <= MAX_COMMON_NAME_LEN {
        distinguished_name(&[
            (Nid::ORGANIZATIONNAME, ORGANIZATION),
            (Nid::COMMONNAME, &host),
        ])
    } else {
        distinguished_name(&[(Nid::ORGANIZATIONNAME, ORGANIZATION)])
    }
}

/// The SAN naming exactly `name`.
fn subject_alt_name(name: &LeafName) -> SubjectAlternativeName {
    let mut san = SubjectAlternativeName::new();
    match name {
        LeafName::Dns(dns) => san.dns(dns),
        LeafName::Ip(ip) => san.ip(&ip.to_string()),
    };
    san
}
