//! The header names, values and order per request shape, with the HTTP/2 `priority` header set per destination.
//!
//! A [`HeaderSpec`] says which headers a request carries, in which order, with which
//! capitalisation, for two request shapes: a top-level navigation ([`RequestShape::Document`])
//! and a script-issued request ([`RequestShape::Fetch`]). Most values are literal. The ones
//! that belong to the caller's identity are placeholders, resolved at request time from a
//! [`HeaderIdentity`]: the `User-Agent`, `Accept-Language`, the client hints and the cookie.
//! Because the caller supplies them, any of them can be changed without touching the spec,
//! and any header can be replaced by name with [`HeaderIdentity::with_override`] or
//! [`HeaderSpec::with_header`].
//!
//! A spec is built from a captured header list ([`HeaderSpec::from_captured`]), or written by
//! hand as plain data and checked with [`HeaderSpec::validate`]. Names keep the capitalisation
//! they were captured with (HTTP/1 shows it); an HTTP/2 sender lowercases them.
//!
//! The HTTP/2 `priority` header is not part of the lists. It depends on what the request is
//! for, so it is looked up by the `Sec-Fetch-Dest` value with [`HeaderSpec::priority_for`]
//! and goes last in the header block.
//!
//! ```
//! use seleniumbase_rs::wire::header_spec::{HeaderIdentity, HeaderSpec, RequestShape};
//!
//! let captured = [("Accept", "*/*"), ("User-Agent", "Agent/1.0"), ("Host", "example.test")];
//! let spec = HeaderSpec::from_captured(&captured, &captured)?;
//! let identity = HeaderIdentity::new("Agent/2.0");
//! let headers = spec.render(RequestShape::Fetch, &identity)?;
//! // `Host` belongs to the connection; the user agent is the caller's.
//! assert_eq!(
//!     headers,
//!     [("Accept".to_owned(), "*/*".to_owned()), ("User-Agent".to_owned(), "Agent/2.0".to_owned())]
//! );
//! # Ok::<(), seleniumbase_rs::wire::header_spec::HeaderSpecError>(())
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The most headers one list may hold.
pub const MAX_HEADERS: usize = 128;
/// The longest header name, in bytes.
pub const MAX_NAME_LEN: usize = 256;
/// The longest header value, in bytes.
pub const MAX_VALUE_LEN: usize = 8192;
/// The most `Sec-Fetch-Dest` values that can carry their own `priority`.
pub const MAX_PRIORITY_DESTINATIONS: usize = 32;

/// Headers that belong to the connection, not to a profile: the transport sets them.
///
/// [`HeaderSpec::from_captured`] drops them from captures; [`HeaderSpec::validate`] refuses them.
pub const TRANSPORT_HEADERS: [&str; 9] = [
    "host",
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authorization",
    "transfer-encoding",
    "upgrade",
    "content-length",
    "te",
];

/// What a request is, for the purpose of choosing its headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RequestShape {
    /// A top-level navigation.
    Document,
    /// A request made by a script (`fetch`, `XMLHttpRequest`).
    Fetch,
}

/// The value of one header in a [`HeaderSpec`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeaderValue {
    /// Exactly this text.
    Literal(String),
    /// The identity's `User-Agent`.
    UserAgent,
    /// The identity's `Sec-CH-UA`; the header is left out when the identity has none.
    SecChUa,
    /// The identity's `Sec-CH-UA-Mobile`; left out when the identity has none.
    SecChUaMobile,
    /// The identity's `Sec-CH-UA-Platform`; left out when the identity has none.
    SecChUaPlatform,
    /// The identity's `Accept-Language`; left out when the identity has none.
    AcceptLanguage,
    /// The identity's cookie; left out when the identity has none.
    Cookie,
}

/// The header names, their order and their values, per request shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderSpec {
    /// Headers of a navigation, in order.
    #[serde(default)]
    pub document: Vec<(String, HeaderValue)>,
    /// Headers of a script-issued request, in order.
    #[serde(default)]
    pub fetch: Vec<(String, HeaderValue)>,
    /// The HTTP/2 `priority` header value for each `Sec-Fetch-Dest` value.
    #[serde(default)]
    pub priority_by_dest: Vec<(String, String)>,
}

/// The caller's values for the placeholders of a [`HeaderSpec`].
///
/// The cookie is a secret, so `Debug` shows it and the override values only as `<redacted>`.
#[derive(Clone, PartialEq, Eq)]
pub struct HeaderIdentity {
    /// The `User-Agent` value.
    pub user_agent: String,
    /// The `Accept-Language` value, when the request has one.
    pub accept_language: Option<String>,
    /// The `Sec-CH-UA` value, when the request has one.
    pub sec_ch_ua: Option<String>,
    /// The `Sec-CH-UA-Mobile` value, when the request has one.
    pub sec_ch_ua_mobile: Option<String>,
    /// The `Sec-CH-UA-Platform` value, when the request has one.
    pub sec_ch_ua_platform: Option<String>,
    /// The `Cookie` value, when the request has one.
    pub cookie: Option<String>,
    /// Header values that replace whatever the spec says, by case-insensitive name.
    pub overrides: Vec<(String, String)>,
}

/// Why a header spec or a header value was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HeaderSpecError {
    /// A header name that is empty, too long or not an HTTP token.
    #[error("`{0}` is not a valid header name")]
    InvalidName(String),
    /// A header value with control characters, edge whitespace, or too long.
    #[error("the value of header `{name}` is not a valid header value")]
    InvalidValue {
        /// The header the value belongs to.
        name: String,
    },
    /// A pseudo-header, which belongs to the HTTP/2 spec.
    #[error("`{0}` is a pseudo-header; pseudo-headers belong to the HTTP/2 spec")]
    PseudoHeader(String),
    /// A connection-level header, or `priority`, which has its own table.
    #[error("`{0}` is set by the transport or by the priority table, not by the header lists")]
    ReservedHeader(String),
    /// The same header twice in one list.
    #[error("header `{0}` appears more than once")]
    DuplicateName(String),
    /// The same `Sec-Fetch-Dest` twice in the priority table.
    #[error("destination `{0}` appears more than once in the priority table")]
    DuplicateDestination(String),
    /// A list longer than [`MAX_HEADERS`].
    #[error("more than {} headers", MAX_HEADERS)]
    TooManyHeaders,
    /// A priority table longer than [`MAX_PRIORITY_DESTINATIONS`].
    #[error("more than {} priority destinations", MAX_PRIORITY_DESTINATIONS)]
    TooManyDestinations,
    /// A captured `priority` header in a list that has no `Sec-Fetch-Dest` to key it by.
    #[error("a captured priority header has no Sec-Fetch-Dest to belong to")]
    PriorityWithoutDestination,
}

impl fmt::Debug for HeaderIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted = |value: &Option<String>| value.as_ref().map(|_| "<redacted>");
        f.debug_struct("HeaderIdentity")
            .field("user_agent", &self.user_agent)
            .field("accept_language", &self.accept_language)
            .field("sec_ch_ua", &self.sec_ch_ua)
            .field("sec_ch_ua_mobile", &self.sec_ch_ua_mobile)
            .field("sec_ch_ua_platform", &self.sec_ch_ua_platform)
            .field("cookie", &redacted(&self.cookie))
            .field(
                "overrides",
                &self
                    .overrides
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl HeaderIdentity {
    /// An identity with a user agent and nothing else.
    #[must_use]
    pub fn new(user_agent: impl Into<String>) -> Self {
        Self {
            user_agent: user_agent.into(),
            accept_language: None,
            sec_ch_ua: None,
            sec_ch_ua_mobile: None,
            sec_ch_ua_platform: None,
            cookie: None,
            overrides: Vec::new(),
        }
    }

    /// Sets the `Accept-Language` value.
    #[must_use]
    pub fn with_accept_language(mut self, value: impl Into<String>) -> Self {
        self.accept_language = Some(value.into());
        self
    }

    /// Sets the three client-hint headers a browser sends on every request.
    #[must_use]
    pub fn with_client_hints(
        mut self,
        brands: impl Into<String>,
        mobile: impl Into<String>,
        platform: impl Into<String>,
    ) -> Self {
        self.sec_ch_ua = Some(brands.into());
        self.sec_ch_ua_mobile = Some(mobile.into());
        self.sec_ch_ua_platform = Some(platform.into());
        self
    }

    /// Sets the `Cookie` value.
    #[must_use]
    pub fn with_cookie(mut self, value: impl Into<String>) -> Self {
        self.cookie = Some(value.into());
        self
    }

    /// Replaces the value of the header `name` (any capitalisation) wherever a spec has it.
    #[must_use]
    pub fn with_override(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.overrides.push((name.into(), value.into()));
        self
    }

    /// The identity a captured header list shows, or `None` when it has no `User-Agent`.
    #[must_use]
    pub fn from_captured<N: AsRef<str>, V: AsRef<str>>(headers: &[(N, V)]) -> Option<Self> {
        let mut identity = Self::new(String::new());
        let mut has_user_agent = false;
        for (name, value) in headers {
            let value = value.as_ref().to_owned();
            match name.as_ref().to_ascii_lowercase().as_str() {
                "user-agent" => {
                    identity.user_agent = value;
                    has_user_agent = true;
                }
                "accept-language" => identity.accept_language = Some(value),
                "sec-ch-ua" => identity.sec_ch_ua = Some(value),
                "sec-ch-ua-mobile" => identity.sec_ch_ua_mobile = Some(value),
                "sec-ch-ua-platform" => identity.sec_ch_ua_platform = Some(value),
                "cookie" => identity.cookie = Some(value),
                _ => {}
            }
        }
        has_user_agent.then_some(identity)
    }

    /// The override for the header `name`; the last one given wins.
    fn override_for(&self, name: &str) -> Option<&str> {
        self.overrides
            .iter()
            .rev()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The value a placeholder stands for, or `None` when the identity has no such value.
    fn resolve<'a>(&'a self, value: &'a HeaderValue) -> Option<&'a str> {
        match value {
            HeaderValue::Literal(text) => Some(text),
            HeaderValue::UserAgent => Some(&self.user_agent),
            HeaderValue::SecChUa => self.sec_ch_ua.as_deref(),
            HeaderValue::SecChUaMobile => self.sec_ch_ua_mobile.as_deref(),
            HeaderValue::SecChUaPlatform => self.sec_ch_ua_platform.as_deref(),
            HeaderValue::AcceptLanguage => self.accept_language.as_deref(),
            HeaderValue::Cookie => self.cookie.as_deref(),
        }
    }
}

impl HeaderSpec {
    /// Builds a spec from the headers of one captured navigation and one captured fetch.
    ///
    /// Pseudo-headers and the transport headers in [`TRANSPORT_HEADERS`] are dropped. The
    /// identity headers become placeholders. A `priority` header (an HTTP/2 capture) moves to
    /// the priority table, keyed by the same request's `Sec-Fetch-Dest`.
    ///
    /// # Errors
    ///
    /// Returns a [`HeaderSpecError`] when a name or value is invalid, a header repeats, or a
    /// `priority` header has no `Sec-Fetch-Dest` to belong to.
    pub fn from_captured<N: AsRef<str>, V: AsRef<str>>(
        document: &[(N, V)],
        fetch: &[(N, V)],
    ) -> Result<Self, HeaderSpecError> {
        let mut priority_by_dest = Vec::new();
        let document = captured_list(document, &mut priority_by_dest)?;
        let fetch = captured_list(fetch, &mut priority_by_dest)?;
        let spec = Self {
            document,
            fetch,
            priority_by_dest,
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Checks a spec written by hand or read from a file.
    ///
    /// # Errors
    ///
    /// Returns the first problem found: an invalid or duplicate name, a pseudo-header, a
    /// reserved header, a bad value, or a list or table that is too long.
    pub fn validate(&self) -> Result<(), HeaderSpecError> {
        validate_list(&self.document)?;
        validate_list(&self.fetch)?;
        validate_priority_table(&self.priority_by_dest)
    }

    /// The header list of a request shape.
    #[must_use]
    pub fn headers(&self, shape: RequestShape) -> &[(String, HeaderValue)] {
        match shape {
            RequestShape::Document => &self.document,
            RequestShape::Fetch => &self.fetch,
        }
    }

    /// The headers to send, in order, with the placeholders filled from `identity`.
    ///
    /// A placeholder the identity has no value for leaves its header out.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderSpecError::InvalidValue`] when a resolved value is not a valid header
    /// value.
    pub fn render(
        &self,
        shape: RequestShape,
        identity: &HeaderIdentity,
    ) -> Result<Vec<(String, String)>, HeaderSpecError> {
        let mut out = Vec::new();
        for (name, value) in self.headers(shape) {
            let resolved = identity
                .override_for(name)
                .or_else(|| identity.resolve(value));
            let Some(text) = resolved else { continue };
            validate_value(name, text)?;
            out.push((name.clone(), text.to_owned()));
        }
        Ok(out)
    }

    /// The `priority` header value for requests whose `Sec-Fetch-Dest` is `dest`.
    #[must_use]
    pub fn priority_for(&self, dest: &str) -> Option<&str> {
        self.priority_by_dest
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(dest))
            .map(|(_, value)| value.as_str())
    }

    /// A copy with the header `name` set to `value` for `shape`: replaced in place (keeping
    /// its capitalisation and position) when the list has it, appended when it does not.
    ///
    /// # Errors
    ///
    /// Returns a [`HeaderSpecError`] when the name or value is invalid or the result would
    /// not validate.
    pub fn with_header(
        &self,
        shape: RequestShape,
        name: &str,
        value: HeaderValue,
    ) -> Result<Self, HeaderSpecError> {
        validate_name(name)?;
        let mut next = self.clone();
        let list = match shape {
            RequestShape::Document => &mut next.document,
            RequestShape::Fetch => &mut next.fetch,
        };
        match list
            .iter_mut()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
        {
            Some(entry) => entry.1 = value,
            None => list.push((name.to_owned(), value)),
        }
        next.validate()?;
        Ok(next)
    }

    /// A copy without the header `name` (any capitalisation) in the list of `shape`.
    #[must_use]
    pub fn without_header(&self, shape: RequestShape, name: &str) -> Self {
        let mut next = self.clone();
        let list = match shape {
            RequestShape::Document => &mut next.document,
            RequestShape::Fetch => &mut next.fetch,
        };
        list.retain(|(known, _)| !known.eq_ignore_ascii_case(name));
        next
    }

    /// A copy with the `priority` header for `dest` set to `value`.
    ///
    /// # Errors
    ///
    /// Returns a [`HeaderSpecError`] when the destination or value is invalid or the table
    /// would be too long.
    pub fn with_priority(&self, dest: &str, value: &str) -> Result<Self, HeaderSpecError> {
        let mut next = self.clone();
        match next
            .priority_by_dest
            .iter_mut()
            .find(|(known, _)| known.eq_ignore_ascii_case(dest))
        {
            Some(entry) => entry.1 = value.to_owned(),
            None => next
                .priority_by_dest
                .push((dest.to_owned(), value.to_owned())),
        }
        next.validate()?;
        Ok(next)
    }
}

/// The placeholder for a lowercase header name that carries the caller's identity.
fn placeholder_for(lower_name: &str) -> Option<HeaderValue> {
    match lower_name {
        "user-agent" => Some(HeaderValue::UserAgent),
        "accept-language" => Some(HeaderValue::AcceptLanguage),
        "sec-ch-ua" => Some(HeaderValue::SecChUa),
        "sec-ch-ua-mobile" => Some(HeaderValue::SecChUaMobile),
        "sec-ch-ua-platform" => Some(HeaderValue::SecChUaPlatform),
        "cookie" => Some(HeaderValue::Cookie),
        _ => None,
    }
}

/// One captured list as spec entries; a `priority` header goes to `priority_table` instead.
fn captured_list<N: AsRef<str>, V: AsRef<str>>(
    raw: &[(N, V)],
    priority_table: &mut Vec<(String, String)>,
) -> Result<Vec<(String, HeaderValue)>, HeaderSpecError> {
    let mut entries = Vec::new();
    let mut destination: Option<String> = None;
    let mut priority: Option<String> = None;
    for (name, value) in raw {
        let (name, value) = (name.as_ref(), value.as_ref());
        let lower = name.to_ascii_lowercase();
        if name.starts_with(':') || TRANSPORT_HEADERS.contains(&lower.as_str()) {
            continue;
        }
        match lower.as_str() {
            "priority" if priority.is_some() => {
                return Err(HeaderSpecError::DuplicateName(lower));
            }
            "priority" => {
                priority = Some(value.to_owned());
                continue;
            }
            "sec-fetch-dest" => destination = Some(value.to_owned()),
            _ => {}
        }
        let kept =
            placeholder_for(&lower).unwrap_or_else(|| HeaderValue::Literal(value.to_owned()));
        entries.push((name.to_owned(), kept));
    }
    if let Some(priority) = priority {
        let destination = destination.ok_or(HeaderSpecError::PriorityWithoutDestination)?;
        record_priority(priority_table, destination, priority)?;
    }
    Ok(entries)
}

/// Adds a destination's priority; the same destination with the same value is one entry.
fn record_priority(
    table: &mut Vec<(String, String)>,
    destination: String,
    priority: String,
) -> Result<(), HeaderSpecError> {
    match table
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(&destination))
    {
        Some((_, known)) if *known == priority => Ok(()),
        Some(_) => Err(HeaderSpecError::DuplicateDestination(destination)),
        None => {
            table.push((destination, priority));
            Ok(())
        }
    }
}

fn is_token(text: &str) -> bool {
    const SPECIALS: &[u8] = b"!#$%&'*+-.^_`|~";
    !text.is_empty()
        && text.len() <= MAX_NAME_LEN
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || SPECIALS.contains(&b))
}

fn validate_name(name: &str) -> Result<(), HeaderSpecError> {
    if name.starts_with(':') {
        Err(HeaderSpecError::PseudoHeader(name.to_owned()))
    } else if is_token(name) {
        Ok(())
    } else {
        Err(HeaderSpecError::InvalidName(name.to_owned()))
    }
}

/// A value is text without control characters or edge whitespace, within the size limit.
fn validate_value(name: &str, value: &str) -> Result<(), HeaderSpecError> {
    let edge_space = |c: char| c == ' ' || c == '\t';
    let bad = value.len() > MAX_VALUE_LEN
        || value.chars().any(|c| c.is_control() && c != '\t')
        || value.starts_with(edge_space)
        || value.ends_with(edge_space);
    if bad {
        Err(HeaderSpecError::InvalidValue {
            name: name.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_list(list: &[(String, HeaderValue)]) -> Result<(), HeaderSpecError> {
    if list.len() > MAX_HEADERS {
        return Err(HeaderSpecError::TooManyHeaders);
    }
    let mut seen: Vec<String> = Vec::with_capacity(list.len());
    for (name, value) in list {
        validate_name(name)?;
        let lower = name.to_ascii_lowercase();
        if lower == "priority" || TRANSPORT_HEADERS.contains(&lower.as_str()) {
            return Err(HeaderSpecError::ReservedHeader(lower));
        }
        if seen.contains(&lower) {
            return Err(HeaderSpecError::DuplicateName(lower));
        }
        if let HeaderValue::Literal(text) = value {
            validate_value(name, text)?;
        }
        seen.push(lower);
    }
    Ok(())
}

fn validate_priority_table(table: &[(String, String)]) -> Result<(), HeaderSpecError> {
    if table.len() > MAX_PRIORITY_DESTINATIONS {
        return Err(HeaderSpecError::TooManyDestinations);
    }
    for (index, (destination, value)) in table.iter().enumerate() {
        if !is_token(destination) {
            return Err(HeaderSpecError::InvalidName(destination.clone()));
        }
        let earlier = table.get(..index).unwrap_or_default();
        if earlier
            .iter()
            .any(|(known, _)| known.eq_ignore_ascii_case(destination))
        {
            return Err(HeaderSpecError::DuplicateDestination(destination.clone()));
        }
        validate_value("priority", value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
