//! The builder that layers caller-supplied inputs over a measured base.

use super::identity::with_hints;
use super::{ImpersonateError, ImpersonationProfile};
use crate::impersonate::convert;
use crate::stealth::fingerprint::{ClientHints, OsType};
use crate::wire::akamai::Akamai;
use crate::wire::header_spec::{HeaderSpec, HeaderValue, RequestShape};
use crate::wire::ja3::{Ja3, Peetprint};
use crate::wire::ja4::{Ja4Order, Ja4Raw};
use crate::wire::tls_spec::{SpecError as TlsSpecError, TlsSpec};
use crate::wire::BrowserKind;

/// One change to the header lists, applied in the order it was asked for.
#[derive(Debug, Clone)]
enum HeaderEdit {
    /// Sets a header: replaced in place when the list has it, appended when it does not.
    Set {
        shape: RequestShape,
        name: String,
        value: HeaderValue,
    },
    /// Removes a header.
    Remove { shape: RequestShape, name: String },
    /// Puts the named headers first, in this order.
    Order {
        shape: RequestShape,
        names: Vec<String>,
    },
    /// Sets the HTTP/2 `priority` value for a `Sec-Fetch-Dest`.
    Priority { destination: String, value: String },
    /// Replaces both lists and the priority table.
    Replace(HeaderSpec),
}

/// Builds an [`ImpersonationProfile`] from a measured base and caller-supplied inputs.
///
/// Every input is optional and layered over the base, which is the newest measured Chrome unless
/// [`CustomProfileBuilder::base`] or [`CustomProfileBuilder::base_on`] says otherwise. What an
/// input does not carry comes from the base: a JA3 has no signature algorithms, an Akamai
/// fingerprint has no HEADERS priority, a header list has no TLS.
///
/// The inputs that need the base to be judged ([`ja3`](Self::ja3), [`ja4_r`](Self::ja4_r),
/// [`ja4_ro`](Self::ja4_ro), [`peetprint`](Self::peetprint),
/// [`signature_algorithms`](Self::signature_algorithms), [`akamai`](Self::akamai)) return the
/// named error at once. The identity and header inputs are checked in
/// [`build`](Self::build). Nothing is sent before a client exists, and a custom JA3 turns
/// extension permutation off, so its JA3 arrives exactly as given.
///
/// ```
/// use seleniumbase_rs::impersonate::client::ImpersonationProfile;
///
/// # fn demo() -> Result<(), Box<dyn std::error::Error>> {
/// let profile = ImpersonationProfile::custom()
///     .akamai("1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p")?
///     .user_agent("Mozilla/5.0 (custom)")
///     .build()?;
/// assert_eq!(profile.user_agent(), "Mozilla/5.0 (custom)");
/// # Ok(())
/// # }
/// # demo().unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct CustomProfileBuilder {
    /// The browser whose measurement is the base.
    kind: BrowserKind,
    /// The system of the base, or the newest measured one.
    os: Option<OsType>,
    /// The base with the TLS and HTTP/2 overlays applied; resolved on first use.
    profile: Option<ImpersonationProfile>,
    /// A `User-Agent` that replaces the base's.
    user_agent: Option<String>,
    /// An `Accept-Language` that replaces the base's.
    accept_language: Option<String>,
    /// Client hints that replace the base's.
    client_hints: Option<ClientHints>,
    /// Header changes, in order.
    header_edits: Vec<HeaderEdit>,
}

impl Default for CustomProfileBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomProfileBuilder {
    /// A builder over the newest measured Chrome.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kind: BrowserKind::Chrome,
            os: None,
            profile: None,
            user_agent: None,
            accept_language: None,
            client_hints: None,
            header_edits: Vec::new(),
        }
    }

    /// Takes the newest measurement of `kind` as the base. Call it first: it discards the TLS
    /// and HTTP/2 inputs given before it.
    #[must_use]
    pub fn base(mut self, kind: BrowserKind) -> Self {
        self.kind = kind;
        self.os = None;
        self.profile = None;
        self
    }

    /// Takes the measurement of `kind` on `os` as the base; a pair with no capture is refused by
    /// the next input that needs the base, or by [`build`](Self::build). Call it first.
    #[must_use]
    pub fn base_on(mut self, kind: BrowserKind, os: OsType) -> Self {
        self.kind = kind;
        self.os = Some(os);
        self.profile = None;
        self
    }

    /// The base profile, loaded on first use.
    fn resolved(&mut self) -> Result<&mut ImpersonationProfile, ImpersonateError> {
        if self.profile.is_none() {
            let base = match self.os {
                Some(os) => ImpersonationProfile::for_system(self.kind, os)?,
                None => ImpersonationProfile::measured(self.kind)?,
            };
            self.profile = Some(base);
        }
        self.profile.as_mut().ok_or(ImpersonateError::Incomplete {
            what: "base profile",
        })
    }

    /// Applies a TLS overlay to the base and checks the result against this client stack.
    fn tls_overlay(
        mut self,
        overlay: impl FnOnce(&TlsSpec) -> Result<TlsSpec, TlsSpecError>,
    ) -> Result<Self, ImpersonateError> {
        let profile = self.resolved()?;
        profile.tls = overlay(&profile.tls)?;
        convert::check_emittable(&profile.tls)?;
        Ok(self)
    }

    /// Replaces the version, ciphers, extensions, groups and point formats with a JA3.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Ja3`] for a malformed string and the named TLS or
    /// client-stack error for a fingerprint this client cannot send.
    pub fn ja3(self, ja3: &str) -> Result<Self, ImpersonateError> {
        let ja3 = Ja3::parse(ja3)?;
        self.tls_overlay(|base| base.with_ja3(&ja3))
    }

    /// Replaces the ciphers, extensions and signature algorithms with a `JA4_r` (sorted ids).
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Ja4`] for a malformed string and the named error for a
    /// fingerprint this client cannot send.
    pub fn ja4_r(self, ja4_r: &str) -> Result<Self, ImpersonateError> {
        let raw = Ja4Raw::parse(ja4_r)?;
        self.tls_overlay(|base| base.with_ja4_raw(&raw, Ja4Order::Sorted))
    }

    /// Replaces the ciphers, extensions and signature algorithms with a `JA4_ro` (wire order).
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Ja4`] for a malformed string and the named error for a
    /// fingerprint this client cannot send.
    pub fn ja4_ro(self, ja4_ro: &str) -> Result<Self, ImpersonateError> {
        let raw = Ja4Raw::parse(ja4_ro)?;
        self.tls_overlay(|base| base.with_ja4_raw(&raw, Ja4Order::Original))
    }

    /// Replaces the ciphers, extensions, groups, versions, ALPN, signature algorithms, PSK modes
    /// and certificate compression with a peetprint.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Peetprint`] for a malformed string and the named error for a
    /// fingerprint this client cannot send.
    pub fn peetprint(self, peetprint: &str) -> Result<Self, ImpersonateError> {
        let peet = Peetprint::parse(peetprint)?;
        self.tls_overlay(|base| base.with_peetprint(&peet))
    }

    /// Replaces the signature algorithms.
    ///
    /// # Errors
    ///
    /// Returns the named error for a scheme `BoringSSL` cannot send.
    pub fn signature_algorithms(self, ids: &[u16]) -> Result<Self, ImpersonateError> {
        self.tls_overlay(|base| base.with_signature_algorithms(ids))
    }

    /// Replaces the HTTP/2 SETTINGS, connection window, PRIORITY frames and pseudo-header order
    /// with an Akamai fingerprint. The HEADERS priority fields stay the base's.
    ///
    /// # Errors
    ///
    /// Returns [`ImpersonateError::Akamai`] for a malformed string and the named HTTP/2 error
    /// for a SETTINGS id, value, frame or window wreq cannot send.
    pub fn akamai(mut self, akamai: &str) -> Result<Self, ImpersonateError> {
        let akamai = Akamai::parse(akamai)?;
        let profile = self.resolved()?;
        profile.http2 = profile.http2.with_akamai(&akamai)?;
        profile.dropped_settings.clear();
        Ok(self)
    }

    /// Sets the `User-Agent`.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Sets the `Accept-Language`.
    #[must_use]
    pub fn accept_language(mut self, accept_language: impl Into<String>) -> Self {
        self.accept_language = Some(accept_language.into());
        self
    }

    /// Sets the client hints (`Sec-CH-UA`, `Sec-CH-UA-Mobile`, `Sec-CH-UA-Platform`).
    #[must_use]
    pub fn client_hints(mut self, hints: ClientHints) -> Self {
        self.client_hints = Some(hints);
        self
    }

    /// Sets a header of one request shape: replaced in place (keeping its position and
    /// spelling) when the list has it, appended when it does not.
    #[must_use]
    pub fn header(
        mut self,
        shape: RequestShape,
        name: impl Into<String>,
        value: HeaderValue,
    ) -> Self {
        self.header_edits.push(HeaderEdit::Set {
            shape,
            name: name.into(),
            value,
        });
        self
    }

    /// Removes a header of one request shape.
    #[must_use]
    pub fn without_header(mut self, shape: RequestShape, name: impl Into<String>) -> Self {
        self.header_edits.push(HeaderEdit::Remove {
            shape,
            name: name.into(),
        });
        self
    }

    /// Puts the named headers of one request shape first, in this order; the others follow in
    /// their old order. A name that is not in the list is refused by [`build`](Self::build).
    #[must_use]
    pub fn header_order<I, S>(mut self, shape: RequestShape, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.header_edits.push(HeaderEdit::Order {
            shape,
            names: names.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// Sets the HTTP/2 `priority` header value for requests with this `Sec-Fetch-Dest`.
    #[must_use]
    pub fn priority(mut self, destination: impl Into<String>, value: impl Into<String>) -> Self {
        self.header_edits.push(HeaderEdit::Priority {
            destination: destination.into(),
            value: value.into(),
        });
        self
    }

    /// Replaces both header lists and the priority table.
    #[must_use]
    pub fn headers(mut self, spec: HeaderSpec) -> Self {
        self.header_edits.push(HeaderEdit::Replace(spec));
        self
    }

    /// Applies the identity and header inputs, checks every spec, and returns the profile.
    ///
    /// # Errors
    ///
    /// Returns the named error of the first check that fails: the base is not measured, a header
    /// is invalid or unknown, or a spec cannot be sent.
    pub fn build(mut self) -> Result<ImpersonationProfile, ImpersonateError> {
        self.resolved()?;
        let edits = std::mem::take(&mut self.header_edits);
        let mut profile = self.profile.take().ok_or(ImpersonateError::Incomplete {
            what: "base profile",
        })?;
        profile.source = format!("custom over {}", profile.source);
        let mut headers = std::mem::take(&mut profile.headers);
        for edit in edits {
            headers = apply_edit(headers, edit)?;
        }
        profile.headers = headers;
        if let Some(hints) = &self.client_hints {
            profile.identity = with_hints(profile.identity, hints);
        }
        if let Some(language) = self.accept_language {
            profile.identity.accept_language = Some(language);
        }
        if let Some(user_agent) = self.user_agent {
            profile.identity.user_agent = user_agent;
        }
        profile.validate()?;
        Ok(profile)
    }
}

/// Applies one header change to a spec.
fn apply_edit(spec: HeaderSpec, edit: HeaderEdit) -> Result<HeaderSpec, ImpersonateError> {
    Ok(match edit {
        HeaderEdit::Set { shape, name, value } => spec.with_header(shape, &name, value)?,
        HeaderEdit::Remove { shape, name } => spec.without_header(shape, &name),
        HeaderEdit::Order { shape, names } => reorder(spec, shape, &names)?,
        HeaderEdit::Priority { destination, value } => spec.with_priority(&destination, &value)?,
        HeaderEdit::Replace(replacement) => replacement,
    })
}

/// Puts `names` first in the list of `shape`, then the headers they do not name.
fn reorder(
    mut spec: HeaderSpec,
    shape: RequestShape,
    names: &[String],
) -> Result<HeaderSpec, ImpersonateError> {
    let list = spec.headers(shape).to_vec();
    let mut ordered: Vec<(String, HeaderValue)> = Vec::with_capacity(list.len());
    for wanted in names {
        let entry = list
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
            .ok_or_else(|| ImpersonateError::UnknownHeader(wanted.clone()))?;
        if !ordered
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(wanted))
        {
            ordered.push(entry.clone());
        }
    }
    for entry in &list {
        if !ordered
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(&entry.0))
        {
            ordered.push(entry.clone());
        }
    }
    match shape {
        RequestShape::Document => spec.document = ordered,
        RequestShape::Fetch => spec.fetch = ordered,
    }
    Ok(spec)
}
