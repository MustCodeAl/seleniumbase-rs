//! Builds an impersonating wreq client from a profile, with the custom JA3 and Akamai builder.
//!
//! An [`ImpersonationProfile`] is a browser the crate has measured, as data: the TLS and HTTP/2
//! specs it was measured with (minus what `BoringSSL` and wreq cannot send, which stays on
//! record in [`ImpersonationProfile::known_gaps`]), its header lists, and the identity a request
//! carries (`User-Agent`, `Sec-CH-UA*`, `Accept-Language`). [`impersonation_client`] turns it into
//! a [`wreq::ClientBuilder`]; the caller adds proxies, a trust store and timeouts, then builds.
//!
//! ```no_run
//! use seleniumbase_rs::impersonate::client::{
//!     impersonation_client, ImpersonationProfile, WireProtocol,
//! };
//! use seleniumbase_rs::wire::header_spec::RequestShape;
//! use seleniumbase_rs::wire::BrowserKind;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let profile = ImpersonationProfile::measured(BrowserKind::Chrome)?;
//! let client = impersonation_client(&profile)?.build()?;
//! let request = client.get("https://example.com/");
//! let request = profile.apply(request, RequestShape::Document, None, WireProtocol::Http2)?;
//! let response = request.send().await?;
//! # drop(response);
//! # Ok(())
//! # }
//! ```
//!
//! # What is measured, and what is refused
//!
//! Only a browser and system pair with a capture exists
//! ([`NotMeasured`](ImpersonateError::Emulation) otherwise); there is no fallback to another
//! system's values. [`ImpersonationProfile::custom`] layers caller-supplied inputs over a measured
//! base: a JA3, a `JA4_r` or `JA4_ro`, a peetprint, signature algorithms, an Akamai HTTP/2
//! fingerprint, header names, order and values, the `User-Agent`, `Accept-Language` and the
//! client hints. Each input is checked for what this client stack can emit; the rest is refused
//! with a named error before a client exists, and `tests/impersonate_wire.rs` proves on the wire
//! that an accepted input arrives as given.
//!
//! # Identity headers
//!
//! The identity headers (`User-Agent`, the client hints, `Accept-Language`, `Cookie`) are not part
//! of the TLS or HTTP/2 emulation. They are rendered per request with
//! [`ImpersonationProfile::apply`], in the order of the measured header list, so rotating an
//! identity never needs a new connection setup.

mod custom;
mod identity;

use thiserror::Error;
use wreq::{Emulation, Group, RequestBuilder};

use self::identity::{identity_for, user_agent_major, DEFAULT_LOCALE_TAG};
use super::convert::{self, ConvertError};
use crate::scrape::egress::geo::Locale;
use crate::stealth::fingerprint::{BrowserType, Fingerprint, OsType};
use crate::wire::akamai::AkamaiError;
use crate::wire::client_hello::ext_id;
use crate::wire::emulation::{self, Emulation as Measured, EmulationError, KnownGap};
use crate::wire::h2_spec::{H2Spec, SpecError as H2SpecError};
use crate::wire::header_spec::{HeaderIdentity, HeaderSpec, HeaderSpecError, RequestShape};
use crate::wire::ja3::{Ja3Error, PeetprintError};
use crate::wire::ja4::Ja4Error;
use crate::wire::tls_spec::{SpecError as TlsSpecError, TlsSpec};
use crate::wire::BrowserKind;

pub use self::custom::CustomProfileBuilder;

/// The header the HTTP/2 priority of a request hangs on.
const SEC_FETCH_DEST: &str = "sec-fetch-dest";
/// The name of the HTTP/2-only priority header.
const PRIORITY_HEADER: &str = "priority";
/// The header the language preference travels in; a cookie follows it.
const ACCEPT_LANGUAGE: &str = "accept-language";
/// The cookie header.
const COOKIE_HEADER: &str = "Cookie";
/// The header the transport adds on HTTP/1.1, which a browser writes first.
const HOST_HEADER: &str = "Host";

/// Why a profile or a client could not be built.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ImpersonateError {
    /// A custom JA3 is malformed.
    #[error("invalid JA3: {0}")]
    Ja3(#[from] Ja3Error),
    /// A custom `JA4_r` or `JA4_ro` is malformed.
    #[error("invalid JA4: {0}")]
    Ja4(#[from] Ja4Error),
    /// A custom peetprint is malformed.
    #[error("invalid peetprint: {0}")]
    Peetprint(#[from] PeetprintError),
    /// A custom Akamai HTTP/2 fingerprint is malformed.
    #[error("invalid Akamai fingerprint: {0}")]
    Akamai(#[from] AkamaiError),
    /// The TLS spec cannot be sent by `BoringSSL`.
    #[error("the TLS spec cannot be reproduced: {0}")]
    Tls(#[from] TlsSpecError),
    /// The HTTP/2 spec cannot be sent by wreq.
    #[error("the HTTP/2 spec cannot be reproduced: {0}")]
    Http2(#[from] H2SpecError),
    /// The measured tables have no entry for the browser and system, or are broken. This is the
    /// plan's `SpecError::NotMeasured`.
    #[error(transparent)]
    Emulation(#[from] EmulationError),
    /// A header spec or a header value is invalid.
    #[error("invalid headers: {0}")]
    Headers(#[from] HeaderSpecError),
    /// The spec cannot be turned into wreq options.
    #[error("cannot be emitted by this client stack: {0}")]
    Convert(#[from] ConvertError),
    /// The identity claims another browser version than the one the TLS and HTTP/2 come from.
    #[error("the identity claims Chrome {browser}, but the measured TLS and HTTP/2 are Chrome {measured}")]
    VersionMismatch {
        /// The major version the user agent claims.
        browser: u32,
        /// The major version that was measured.
        measured: u32,
    },
    /// The measured entry lacks a part the profile needs.
    #[error("the measured entry has no {what}; run the capture tools")]
    Incomplete {
        /// What is missing.
        what: &'static str,
    },
    /// A fingerprint cannot be turned into a profile.
    #[error("the fingerprint cannot be used: {0}")]
    Fingerprint(&'static str),
    /// A header named in an order is not in the list being reordered.
    #[error("header `{0}` is not in the list being reordered")]
    UnknownHeader(String),
    /// wreq refused to build the client.
    #[error("the client could not be built: {0}")]
    Build(String),
}

/// The protocol a request is expected to travel on.
///
/// HTTP/2 requests carry the `priority` header of the measured table; HTTP/1.1 requests do not,
/// because Chrome sends it on HTTP/2 only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireProtocol {
    /// HTTP/1.1.
    Http1,
    /// HTTP/2.
    Http2,
}

/// A measured browser as data: what to send on the wire and in the headers to look like it.
///
/// Build one with [`ImpersonationProfile::measured`], [`ImpersonationProfile::for_system`],
/// [`ImpersonationProfile::random`], [`ImpersonationProfile::from_fingerprint`] or
/// [`ImpersonationProfile::custom`]. `Debug` shows the specs and the identity but never a cookie:
/// a cookie is passed per request and never stored.
#[derive(Debug, Clone, PartialEq)]
pub struct ImpersonationProfile {
    /// Where the profile comes from, such as `chrome-155-macos`.
    source: String,
    /// The browser.
    kind: BrowserKind,
    /// The operating system the browser was measured on.
    os: OsType,
    /// The major version that was measured.
    major: u32,
    /// The TLS spec to send: the measured one without its known gaps, or a custom one.
    tls: TlsSpec,
    /// The HTTP/2 spec to send.
    http2: H2Spec,
    /// The SETTINGS wreq cannot send, dropped from the measured HTTP/2 start.
    dropped_settings: Vec<(u16, u32)>,
    /// The header lists.
    headers: HeaderSpec,
    /// The values of the identity placeholders.
    identity: HeaderIdentity,
    /// What the measured hello has and `BoringSSL` cannot send.
    known_gaps: Vec<KnownGap>,
}

impl ImpersonationProfile {
    /// The newest measured entry of a browser.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Emulation`] when the embedded tables are broken or hold no
    /// entry for `kind`, and the errors of [`ImpersonationProfile::from_measured`].
    pub fn measured(kind: BrowserKind) -> Result<Self, ImpersonateError> {
        let table = emulation::try_measured().map_err(Clone::clone)?;
        let entry = emulation::newest(table, kind).ok_or(ImpersonateError::Incomplete {
            what: "measurement for this browser",
        })?;
        Self::from_measured(entry)
    }

    /// The newest measured entry of a browser on one operating system.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Emulation`] with `NotMeasured` when the pair has no capture,
    /// and the errors of [`ImpersonationProfile::from_measured`].
    pub fn for_system(kind: BrowserKind, os: OsType) -> Result<Self, ImpersonateError> {
        Self::from_measured(emulation::get(kind, os)?)
    }

    /// A profile from one measured entry.
    ///
    /// The TLS spec loses the entry's known gaps, and the HTTP/2 spec loses the SETTINGS wreq
    /// cannot send; both are checked afterwards. The `Accept-Language` is the default locale's.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Incomplete`] when the entry has no HTTP/2 spec or no client
    /// hints yet, and the error of the check that fails.
    pub fn from_measured(entry: &Measured) -> Result<Self, ImpersonateError> {
        let http2 = entry.http2.as_ref().ok_or(ImpersonateError::Incomplete {
            what: "HTTP/2 spec",
        })?;
        let hints = entry
            .client_hints
            .as_ref()
            .ok_or(ImpersonateError::Incomplete {
                what: "client hints",
            })?;
        let (http2, dropped_settings) = http2.without_unsupported_settings();
        let accept_language = Locale::by_tag(DEFAULT_LOCALE_TAG).map(Locale::accept_language);
        let profile = Self {
            source: entry.file_stem(),
            kind: entry.kind,
            os: entry.os,
            major: entry.major,
            tls: entry.tls.without_known_gaps(&entry.provenance.known_gaps),
            http2,
            dropped_settings,
            headers: entry.headers.clone(),
            identity: identity_for(&entry.user_agent, accept_language.as_deref(), Some(hints)),
            known_gaps: entry.provenance.known_gaps.clone(),
        };
        profile.validate()?;
        Ok(profile)
    }

    /// A profile for a seeded random identity on a measured system.
    ///
    /// The seed picks one of the measured entries and the identity (screen, hardware, locale) of
    /// [`Fingerprint::randomized_with_major`], whose user agent claims the measured major.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Incomplete`] when nothing is measured, and the errors of
    /// [`ImpersonationProfile::from_fingerprint`].
    pub fn random(seed: u64) -> Result<Self, ImpersonateError> {
        let table = emulation::try_measured().map_err(Clone::clone)?;
        let count = u64::try_from(table.len()).unwrap_or(u64::MAX).max(1);
        let index = usize::try_from(seed % count).unwrap_or_default();
        let entry = table.get(index).ok_or(ImpersonateError::Incomplete {
            what: "measurement",
        })?;
        let fingerprint = Fingerprint::randomized_with_major(entry.os, seed, entry.major);
        Self::from_fingerprint(&fingerprint)
    }

    /// A profile for the identity of a fingerprint, riding the measured TLS and HTTP/2 of its
    /// operating system.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Emulation`] with `NotMeasured` for a system with no capture,
    /// [`ImpersonateError::Fingerprint`] for a fingerprint that is not Chromium or has no user
    /// agent, and [`ImpersonateError::VersionMismatch`] when the user agent claims a Chrome major
    /// other than the measured one: the TLS of Chrome 155 under a Chrome 153 user agent is a
    /// contradiction a detector can see.
    pub fn from_fingerprint(fingerprint: &Fingerprint) -> Result<Self, ImpersonateError> {
        if fingerprint.browser_type != BrowserType::Chromium {
            return Err(ImpersonateError::Fingerprint(
                "only Chromium identities have a measured TLS and HTTP/2",
            ));
        }
        let entry = emulation::get(BrowserKind::Chrome, fingerprint.os_type)?;
        let user_agent = fingerprint
            .user_agent
            .as_deref()
            .ok_or(ImpersonateError::Fingerprint("it has no user agent"))?;
        let claimed = user_agent_major(user_agent).ok_or(ImpersonateError::Fingerprint(
            "its user agent names no Chrome version",
        ))?;
        if claimed != entry.major {
            return Err(ImpersonateError::VersionMismatch {
                browser: claimed,
                measured: entry.major,
            });
        }
        let mut profile = Self::from_measured(entry)?;
        let language = fingerprint
            .accept_languages
            .as_deref()
            .or(fingerprint.languages.as_deref());
        let hints = fingerprint
            .client_hints
            .as_ref()
            .or(entry.client_hints.as_ref());
        let language = language.or(profile.identity.accept_language.as_deref());
        profile.identity = identity_for(user_agent, language, hints);
        Ok(profile)
    }

    /// The same profile with the `Accept-Language` of a locale.
    #[must_use]
    pub fn with_locale(mut self, locale: &Locale) -> Self {
        self.identity.accept_language = Some(locale.accept_language());
        self
    }

    /// A builder that layers custom inputs over a measured base.
    #[must_use]
    pub fn custom() -> CustomProfileBuilder {
        CustomProfileBuilder::new()
    }

    /// Checks that the specs can be sent and the headers are valid.
    ///
    /// # Errors
    ///
    /// Returns the first problem found, as a named error.
    pub fn validate(&self) -> Result<(), ImpersonateError> {
        self.tls.check_reproducible()?;
        convert::check_emittable(&self.tls)?;
        self.http2.check_reproducible()?;
        self.headers.validate()?;
        Ok(())
    }

    /// Where the profile comes from, such as `chrome-155-macos`.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The browser.
    #[must_use]
    pub fn kind(&self) -> BrowserKind {
        self.kind
    }

    /// The operating system the browser was measured on.
    #[must_use]
    pub fn os(&self) -> OsType {
        self.os
    }

    /// The major version that was measured.
    #[must_use]
    pub fn major(&self) -> u32 {
        self.major
    }

    /// The `User-Agent` a request carries.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        &self.identity.user_agent
    }

    /// The `Accept-Language` a request carries, when it has one.
    #[must_use]
    pub fn accept_language(&self) -> Option<&str> {
        self.identity.accept_language.as_deref()
    }

    /// The TLS spec the client sends.
    #[must_use]
    pub fn tls_spec(&self) -> &TlsSpec {
        &self.tls
    }

    /// The HTTP/2 spec the client sends.
    #[must_use]
    pub fn http2_spec(&self) -> &H2Spec {
        &self.http2
    }

    /// The header lists.
    #[must_use]
    pub fn header_spec(&self) -> &HeaderSpec {
        &self.headers
    }

    /// The values of the identity placeholders (a cookie is never part of it).
    #[must_use]
    pub fn identity(&self) -> &HeaderIdentity {
        &self.identity
    }

    /// What the measured hello has and `BoringSSL` cannot send: the difference between this
    /// client's hello and the browser's, which the wire test asserts and nothing else.
    #[must_use]
    pub fn known_gaps(&self) -> &[KnownGap] {
        &self.known_gaps
    }

    /// The SETTINGS of the measured HTTP/2 start that wreq cannot send and the profile dropped.
    #[must_use]
    pub fn dropped_settings(&self) -> &[(u16, u32)] {
        &self.dropped_settings
    }

    /// The headers of one request, in wire order, with the placeholders filled.
    ///
    /// `cookie` is the `Cookie` value of this request; it is not kept. When the header list has
    /// no `Cookie` slot (the measurement was made without cookies), the cookie goes right after
    /// `Accept-Language`, where Chrome writes it, or at the end when there is none. On HTTP/2 the
    /// list ends with the `priority` header of the measured table for the request's
    /// `Sec-Fetch-Dest`.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Headers`] when a resolved value is not a valid header value.
    pub fn headers_for(
        &self,
        shape: RequestShape,
        cookie: Option<&str>,
        wire: WireProtocol,
    ) -> Result<Vec<(String, String)>, ImpersonateError> {
        let mut identity = self.identity.clone();
        if let Some(cookie) = cookie {
            identity = identity.with_cookie(cookie);
        }
        let mut list = self.headers.render(shape, &identity)?;
        if let Some(cookie) = cookie {
            if !list
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(COOKIE_HEADER))
            {
                let after = list
                    .iter()
                    .position(|(name, _)| name.eq_ignore_ascii_case(ACCEPT_LANGUAGE))
                    .map_or(list.len(), |index| index + 1);
                list.insert(after, (COOKIE_HEADER.to_owned(), cookie.to_owned()));
            }
        }
        if wire == WireProtocol::Http2 {
            let destination = list
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(SEC_FETCH_DEST))
                .map(|(_, value)| value.as_str());
            if let Some(priority) = destination.and_then(|dest| self.headers.priority_for(dest)) {
                list.push((PRIORITY_HEADER.to_owned(), priority.to_owned()));
            }
        }
        Ok(list)
    }

    /// Sets the headers of one request on a wreq request: the rendered list in its order and
    /// spelling, in place of the client's default headers.
    ///
    /// # Errors
    ///
    /// Returns an [`ImpersonateError`] when a header cannot be rendered or sent.
    pub fn apply(
        &self,
        request: RequestBuilder,
        shape: RequestShape,
        cookie: Option<&str>,
        wire: WireProtocol,
    ) -> Result<RequestBuilder, ImpersonateError> {
        let list = self.headers_for(shape, cookie, wire)?;
        let leading: &[&'static str] = match wire {
            WireProtocol::Http1 => &[HOST_HEADER],
            WireProtocol::Http2 => &[],
        };
        let (headers, order) = convert::wreq_headers(&list, leading)?;
        Ok(request
            .default_headers(false)
            .headers(headers)
            .orig_headers(order))
    }
}

/// Starts a wreq client with the TLS, HTTP/2 and default headers of a profile.
///
/// The client announces `h2` and `http/1.1`, sends the profile's `ClientHello` and HTTP/2 start,
/// and carries the navigation headers by default; use [`ImpersonationProfile::apply`] to give a
/// request the headers of its own shape. Certificate verification stays on: trust a private
/// root with `tls_cert_store`, never by turning verification off. The caller sets proxies and
/// timeouts and builds the client.
///
/// # Errors
///
/// Returns an [`ImpersonateError`] when a spec cannot be sent by this client stack. That is the
/// same check a custom profile passed at build time, so it only fails for a profile whose specs
/// were changed afterwards.
pub fn impersonation_client(
    profile: &ImpersonationProfile,
) -> Result<wreq::ClientBuilder, ImpersonateError> {
    let tls = convert::tls_options(&profile.tls)?;
    let http2 = convert::http2_options(&profile.http2)?;
    let document = profile.headers_for(RequestShape::Document, None, WireProtocol::Http2)?;
    let (headers, order) = convert::wreq_headers(&document, &[])?;
    let emulation = Emulation::builder()
        .tls_options(tls)
        .http2_options(http2)
        .headers(headers)
        .orig_headers(order)
        .build(Group::default());
    Ok(wreq::Client::builder()
        .emulation(emulation)
        .tls_sni(profile.tls.has_extension(ext_id::SERVER_NAME)))
}

#[cfg(test)]
mod tests;
