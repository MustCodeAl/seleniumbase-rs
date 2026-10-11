//! The stealth identity: fingerprint, emulation, cookies and egress, serialized so that a later run can restore it.
//!
//! An identity is what a scrape session presents to a site: the browser fingerprint, the
//! measured TLS and HTTP/2 emulation it goes with, the cookies it earned, and the egress those
//! cookies were earned through. A clearance cookie is bound to the address that earned it, so an
//! identity is only used again through the same egress ([`StealthIdentity::check_egress`]).
//!
//! The identity holds cookies, which are credentials. Its [`Debug`](std::fmt::Debug) prints the
//! cookie count and the egress key only. Its fingerprint never holds a proxy: a proxy can carry a
//! password, so [`StealthIdentity::new`] clears it, serialization leaves it out even if it was set
//! afterwards, and [`StealthIdentity::verify`] refuses an identity that has one.
//!
//! This module is pure. The file the identity is saved to is written by
//! [`identity_file`](crate::scrape::runtime::identity_file).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize, Serializer};
use serde_json::error::Category;

use crate::scrape::challenge::detect::{clearance_cookie, Vendor};
use crate::scrape::egress::route::EgressKey;
use crate::scrape::jar::store::{site_of, CookieJar};
use crate::stealth::fingerprint::{Fingerprint, OsType};
use crate::wire::BrowserKind;

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;

/// The version of the identity layout. A file with another version is refused, not guessed at.
pub const IDENTITY_SCHEMA: u32 = 1;

/// The expiry recorded for a clearance cookie that has none (a session cookie): no known end.
pub const NO_EXPIRY: u64 = u64::MAX;

/// The measured browser whose TLS, HTTP/2 and headers an identity presents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmulationRef {
    /// Which browser was measured.
    #[serde(with = "browser_kind")]
    pub kind: BrowserKind,
    /// The browser's major version.
    pub major: u32,
    /// The operating system the measurement was made on.
    pub os: OsType,
}

/// Serde for [`BrowserKind`], which has none of its own. A kind this file does not know cannot be
/// written, so the compiler points here when `BrowserKind` gains a variant.
mod browser_kind {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::wire::BrowserKind;

    /// The spelling of [`BrowserKind::Chrome`].
    const CHROME: &str = "chrome";

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's `with` hands the field over by reference"
    )]
    pub(super) fn serialize<S: Serializer>(kind: &BrowserKind, out: S) -> Result<S::Ok, S::Error> {
        match kind {
            BrowserKind::Chrome => out.serialize_str(CHROME),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        input: D,
    ) -> Result<BrowserKind, D::Error> {
        let name = String::deserialize(input)?;
        match name.as_str() {
            CHROME => Ok(BrowserKind::Chrome),
            other => Err(D::Error::unknown_variant(other, &[CHROME])),
        }
    }
}

/// Proof that a site let the identity through: the vendor's clearance cookie is in the jar.
///
/// It records the cookie's name, never its value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clearance {
    /// The registrable domain the cookie belongs to.
    pub site: String,
    /// The vendor whose challenge was passed.
    pub vendor: Vendor,
    /// The name of the clearance cookie.
    pub cookie: String,
    /// When the cookie expires, in seconds since the Unix epoch; [`NO_EXPIRY`] for a session cookie.
    pub expires_at: u64,
}

/// A fingerprint, its emulation, and what the identity earned with them.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct StealthIdentity {
    /// The layout version, [`IDENTITY_SCHEMA`].
    pub schema: u32,
    /// The browser fingerprint. It never serializes a proxy.
    #[serde(serialize_with = "serialize_without_proxy")]
    pub fingerprint: Fingerprint,
    /// The measured browser the wire behaviour comes from.
    pub emulation: EmulationRef,
    /// The cookies the identity holds.
    pub cookies: CookieJar,
    /// The client hints each origin asked for with `Accept-CH`.
    #[serde(default)]
    pub accept_ch: BTreeMap<String, Vec<String>>,
    /// The egress that earned the clearances; `None` for a direct connection.
    #[serde(default)]
    pub egress: Option<EgressKey>,
    /// The clearances found in the jar by [`StealthIdentity::refresh_clearances`].
    #[serde(default)]
    pub clearances: Vec<Clearance>,
    /// When the identity was made, in seconds since the Unix epoch.
    pub created_at: u64,
}

/// Why an identity was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IdentityError {
    /// The identity is used through an egress other than the one that earned its clearances.
    #[error("the identity belongs to {} but the request leaves through {}", describe(.expected.as_ref()), describe(.found.as_ref()))]
    EgressMismatch {
        /// The egress the identity is bound to.
        expected: Option<EgressKey>,
        /// The egress the request would leave through.
        found: Option<EgressKey>,
    },
    /// The identity was written with a layout this version does not know.
    #[error("the identity has schema {0}, which this version cannot read")]
    Schema(u32),
    /// The fingerprint, or the identity around it, is not coherent.
    #[error("the identity is not coherent: {}", .0.join("; "))]
    Incoherent(Vec<String>),
    /// The identity claims another browser version than the one measured.
    #[error(
        "the identity claims Chrome {identity} but the emulation was measured on Chrome {measured}"
    )]
    EmulationMismatch {
        /// The major version the identity claims.
        identity: u32,
        /// The major version that was measured.
        measured: u32,
    },
}

/// The egress as text for a message: `no proxy` or `scheme://host:port`.
fn describe(egress: Option<&EgressKey>) -> String {
    egress.map_or_else(|| "no proxy".to_owned(), ToString::to_string)
}

/// In which way a JSON failure happened, without the text it met: that may be a cookie.
pub(crate) fn json_failure(error: &serde_json::Error) -> &'static str {
    match error.classify() {
        Category::Io => "the input could not be read",
        Category::Syntax => "the text is not valid JSON",
        Category::Data => "the data does not have the expected layout",
        Category::Eof => "the text ends too early",
    }
}

/// Serializes `fingerprint` without its proxy, which may hold a password.
fn serialize_without_proxy<S: Serializer>(
    fingerprint: &Fingerprint,
    out: S,
) -> Result<S::Ok, S::Error> {
    if fingerprint.proxy.is_none() {
        return fingerprint.serialize(out);
    }
    let mut clean = fingerprint.clone();
    clean.proxy = None;
    clean.serialize(out)
}

/// Refuses a fingerprint that [`Fingerprint::validate`] finds incoherent.
fn check_coherent(fingerprint: &Fingerprint) -> Result<(), IdentityError> {
    let report = fingerprint.validate();
    if report.is_coherent() {
        Ok(())
    } else {
        Err(IdentityError::Incoherent(report.errors))
    }
}

/// Refuses a fingerprint that does not come back equal from JSON. An infinite number is written
/// as `null`, which is read back as a missing value or not at all.
fn check_saveable(fingerprint: &Fingerprint) -> Result<(), IdentityError> {
    let refused = |why: &str| {
        IdentityError::Incoherent(vec![format!(
            "the fingerprint cannot be saved and read back: {why}"
        )])
    };
    let bytes = serde_json::to_vec(fingerprint).map_err(|error| refused(json_failure(&error)))?;
    let back: Fingerprint =
        serde_json::from_slice(&bytes).map_err(|error| refused(json_failure(&error)))?;
    if back == *fingerprint {
        Ok(())
    } else {
        Err(refused(
            "a value changes in JSON, such as a number that is not finite",
        ))
    }
}

/// Whether two egresses are the same exit: equal scheme, host and port, and countries that agree
/// where both are known.
fn same_egress(left: Option<&EgressKey>, right: Option<&EgressKey>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            let countries_agree = match (&left.country, &right.country) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            };
            left.scheme == right.scheme
                && left.host == right.host
                && left.port == right.port
                && countries_agree
        }
        _ => false,
    }
}

impl StealthIdentity {
    /// Makes an identity from a fingerprint. See the module documentation for what it refuses.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Incoherent`] when [`Fingerprint::validate`] reports an error, or
    /// when the fingerprint cannot be saved and read back as JSON (a battery time of infinity
    /// becomes `null`, for one).
    pub fn new(
        mut fingerprint: Fingerprint,
        emulation: EmulationRef,
        egress: Option<EgressKey>,
        now: u64,
    ) -> Result<Self, IdentityError> {
        check_coherent(&fingerprint)?;
        fingerprint.proxy = None;
        check_saveable(&fingerprint)?;
        Ok(Self {
            schema: IDENTITY_SCHEMA,
            fingerprint,
            emulation,
            cookies: CookieJar::new(),
            accept_ch: BTreeMap::new(),
            egress,
            clearances: Vec::new(),
            created_at: now,
        })
    }

    /// Checks an identity that was read from outside: its schema, its fingerprint, and that it
    /// carries no proxy.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Schema`] for another layout and [`IdentityError::Incoherent`] for a
    /// fingerprint that fails [`Fingerprint::validate`] or holds a proxy.
    pub fn verify(&self) -> Result<(), IdentityError> {
        if self.schema != IDENTITY_SCHEMA {
            return Err(IdentityError::Schema(self.schema));
        }
        let mut problems = self.fingerprint.validate().errors;
        if self.fingerprint.proxy.is_some() {
            problems.push("the fingerprint carries a proxy, which may hold a password".to_owned());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(IdentityError::Incoherent(problems))
        }
    }

    /// Records a [`Clearance`] for every live clearance cookie in the jar, replacing the old list.
    pub fn refresh_clearances(&mut self, now: u64) {
        let mut found: Vec<Clearance> = Vec::new();
        for cookie in self.cookies.cookies() {
            if cookie.is_expired(now) {
                continue;
            }
            let Some(vendor) = vendor_of_cookie(&cookie.name) else {
                continue;
            };
            let site = site_key(&cookie.domain);
            let expires_at = cookie.expires.unwrap_or(NO_EXPIRY);
            let same = |known: &&mut Clearance| known.site == site && known.cookie == cookie.name;
            match found.iter_mut().find(same) {
                Some(known) => known.expires_at = known.expires_at.max(expires_at),
                None => found.push(Clearance {
                    site,
                    vendor,
                    cookie: cookie.name.clone(),
                    expires_at,
                }),
            }
        }
        found.sort_by(|a, b| (&a.site, &a.cookie).cmp(&(&b.site, &b.cookie)));
        self.clearances = found;
    }

    /// The clearance for `site`, a registrable domain or any host on it. When several vendors
    /// cleared the site, the one that lasts longest. It may already be expired: compare
    /// [`Clearance::expires_at`] with the time.
    #[must_use]
    pub fn clearance_for(&self, site: &str) -> Option<&Clearance> {
        let wanted = site_key(site);
        self.clearances
            .iter()
            .filter(|clearance| clearance.site == wanted)
            .max_by_key(|clearance| clearance.expires_at)
    }

    /// Checks that a request leaves through the egress this identity belongs to.
    ///
    /// Scheme, host and port must be equal. The countries must be equal when both are known; an
    /// unknown country matches any. `None` is the direct connection and matches only `None`.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::EgressMismatch`] otherwise.
    pub fn check_egress(&self, egress: Option<&EgressKey>) -> Result<(), IdentityError> {
        if same_egress(self.egress.as_ref(), egress) {
            return Ok(());
        }
        Err(IdentityError::EgressMismatch {
            expected: self.egress.clone(),
            found: egress.cloned(),
        })
    }

    /// Checks that the browser measured now is the one the identity was made for.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::EmulationMismatch`] for another major version and
    /// [`IdentityError::Incoherent`] for another browser or operating system.
    pub fn check_emulation(&self, measured: &EmulationRef) -> Result<(), IdentityError> {
        let own = &self.emulation;
        if own.major != measured.major {
            return Err(IdentityError::EmulationMismatch {
                identity: own.major,
                measured: measured.major,
            });
        }
        if own.kind != measured.kind || own.os != measured.os {
            return Err(IdentityError::Incoherent(vec![format!(
                "the identity was made for {:?} on {:?}, but {:?} on {:?} was measured",
                own.kind, own.os, measured.kind, measured.os
            )]));
        }
        Ok(())
    }
}

impl fmt::Debug for StealthIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StealthIdentity")
            .field("cookies", &self.cookies.len())
            .field("egress", &self.egress)
            .finish_non_exhaustive()
    }
}

/// The vendors that have a clearance cookie, so that a cookie name can be traced to its vendor.
const VENDORS: [Vendor; 7] = [
    Vendor::Cloudflare,
    Vendor::DataDome,
    Vendor::PerimeterX,
    Vendor::Akamai,
    Vendor::Imperva,
    Vendor::Kasada,
    Vendor::AwsWaf,
];

/// The vendor whose clearance cookie is called `name`.
fn vendor_of_cookie(name: &str) -> Option<Vendor> {
    VENDORS
        .into_iter()
        .find(|&vendor| clearance_cookie(vendor) == Some(name))
}

/// The registrable domain of `host`, or `host` in lower case when it has none (an IP address).
fn site_key(host: &str) -> String {
    site_of(host).unwrap_or_else(|| host.to_ascii_lowercase())
}
