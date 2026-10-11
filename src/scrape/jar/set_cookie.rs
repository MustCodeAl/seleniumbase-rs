//! Parses one `Set-Cookie` header into a cookie, applying the RFC 6265 rules for dates, domains, paths and attributes.
//!
//! The parser follows RFC 6265 section 5.2 with the RFC 6265bis changes Chrome ships: a header that
//! carries a control character is refused whole, a pair without `=` is a nameless cookie, the name
//! and the value together may hold [`MAX_NAME_VALUE_BYTES`], and an attribute whose value is longer
//! than [`MAX_ATTRIBUTE_VALUE_BYTES`] is skipped. When an attribute repeats, the last valid one wins.
//! Nothing here knows the request URL or reads the clock: the store resolves the default path and
//! the `Domain` attribute, and [`SetCookie::expiry`] takes `now`.

mod date;

use std::fmt;

use crate::sb_cdp::SameSite;

pub use date::parse_cookie_date;

/// The most bytes the name and the value of one cookie may hold together (RFC 6265bis).
pub const MAX_NAME_VALUE_BYTES: usize = 4096;

/// The most bytes one attribute value may hold; a longer attribute is ignored (RFC 6265bis).
pub const MAX_ATTRIBUTE_VALUE_BYTES: usize = 1024;

/// Seconds in one day.
const SECONDS_PER_DAY: u64 = 86_400;

/// The longest lifetime a cookie may have, in days (RFC 6265bis caps `Expires` and `Max-Age`).
const MAX_LIFETIME_DAYS: u64 = 400;

/// The longest lifetime a cookie may have, in seconds after it is set.
pub const MAX_LIFETIME_SECS: u64 = MAX_LIFETIME_DAYS * SECONDS_PER_DAY;

/// The expiry of a cookie whose `Max-Age` is zero or negative, or whose `Expires` lies before the
/// Unix epoch: the earliest time there is, so the cookie is expired on arrival.
pub const EARLIEST_EXPIRY: u64 = 0;

/// The name prefix that obliges a cookie to be `Secure`. Matched without regard to ASCII case.
pub const SECURE_PREFIX: &str = "__Secure-";

/// The name prefix that obliges a cookie to be `Secure`, host-only and set with `Path=/`. Matched
/// without regard to ASCII case.
pub const HOST_PREFIX: &str = "__Host-";

/// The base of the digits in `Max-Age` and in date fields.
const DECIMAL: u32 = 10;

/// The attributes the parser knows; any other is skipped.
#[derive(Clone, Copy)]
enum Attribute {
    Expires,
    MaxAge,
    Domain,
    Path,
    Secure,
    HttpOnly,
    SameSite,
}

/// Attribute names, compared without regard to ASCII case.
const ATTRIBUTES: [(&str, Attribute); 7] = [
    ("expires", Attribute::Expires),
    ("max-age", Attribute::MaxAge),
    ("domain", Attribute::Domain),
    ("path", Attribute::Path),
    ("secure", Attribute::Secure),
    ("httponly", Attribute::HttpOnly),
    ("samesite", Attribute::SameSite),
];

/// `SameSite` values, compared without regard to ASCII case. Any other value is the default policy.
const SAME_SITE_POLICIES: [(&str, SameSite); 3] = [
    ("strict", SameSite::Strict),
    ("lax", SameSite::Lax),
    ("none", SameSite::None),
];

impl Attribute {
    fn named(name: &str) -> Option<Self> {
        ATTRIBUTES
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|&(_, attribute)| attribute)
    }
}

/// A cookie-name prefix that carries rules (RFC 6265bis section 4.1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookiePrefix {
    /// `__Secure-`: the cookie must be `Secure`.
    Secure,
    /// `__Host-`: the cookie must be `Secure`, host-only, and have the path `/`.
    Host,
}

impl CookiePrefix {
    /// The prefix `text` starts with, compared without regard to ASCII case as Chrome compares it.
    #[must_use]
    pub fn of(text: &str) -> Option<Self> {
        if starts_with_ignore_case(text, SECURE_PREFIX) {
            Some(Self::Secure)
        } else if starts_with_ignore_case(text, HOST_PREFIX) {
            Some(Self::Host)
        } else {
            None
        }
    }
}

fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    text.as_bytes()
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
}

/// One `Set-Cookie` header, parsed but not yet tied to a request.
///
/// The value is a credential, so [`Debug`](fmt::Debug) hides it.
#[derive(Clone, PartialEq, Eq)]
pub struct SetCookie {
    /// The cookie name; empty for a nameless cookie.
    pub name: String,
    /// The cookie value as sent; quotes are kept.
    pub value: String,
    /// The last valid `Expires`, in seconds since the Unix epoch; negative before 1970.
    pub expires: Option<i64>,
    /// The last valid `Max-Age`, in seconds, saturated at the bounds of `i64`.
    pub max_age: Option<i64>,
    /// The last non-empty `Domain`, in lower case with one leading dot removed; empty for `Domain=.`.
    pub domain: Option<String>,
    /// The last `Path`, as written.
    pub path: Option<String>,
    /// `Secure` was present.
    pub secure: bool,
    /// `HttpOnly` was present.
    pub http_only: bool,
    /// The last `SameSite`; `None` when it was absent or named no known policy.
    pub same_site: Option<SameSite>,
}

impl SetCookie {
    /// Parses one `Set-Cookie` header value (RFC 6265 section 5.2, as amended by RFC 6265bis).
    ///
    /// # Errors
    ///
    /// [`SetCookieError::ControlCharacter`] when the header holds a control character other than a
    /// tab, [`SetCookieError::Empty`] when it has neither a name nor a value,
    /// [`SetCookieError::TooLarge`] when the name and value exceed [`MAX_NAME_VALUE_BYTES`], and
    /// [`SetCookieError::NamelessPrefix`] for a nameless cookie whose value starts with a prefix,
    /// which a later reader could take for a prefixed name.
    pub fn parse(header: &str) -> Result<Self, SetCookieError> {
        if has_forbidden_control(header) {
            return Err(SetCookieError::ControlCharacter);
        }
        let (pair, attributes) = header.split_once(';').unwrap_or((header, ""));
        let (name, value) = match pair.split_once('=') {
            Some((name, value)) => (trim_whitespace(name), trim_whitespace(value)),
            None => ("", trim_whitespace(pair)),
        };
        check_pair(name, value)?;
        let mut cookie = Self {
            name: name.to_owned(),
            value: value.to_owned(),
            expires: None,
            max_age: None,
            domain: None,
            path: None,
            secure: false,
            http_only: false,
            same_site: None,
        };
        for attribute in attributes.split(';') {
            cookie.apply(attribute);
        }
        Ok(cookie)
    }

    /// When the cookie expires, in seconds since the Unix epoch, or `None` for a session cookie.
    ///
    /// `Max-Age` wins over `Expires`. A zero or negative `Max-Age`, or an `Expires` before the
    /// epoch, gives [`EARLIEST_EXPIRY`]. Every expiry is capped at [`MAX_LIFETIME_SECS`] after `now`.
    #[must_use]
    pub fn expiry(&self, now: u64) -> Option<u64> {
        let latest = now.saturating_add(MAX_LIFETIME_SECS);
        if let Some(delta) = self.max_age {
            return Some(if delta <= 0 {
                EARLIEST_EXPIRY
            } else {
                now.saturating_add(delta.unsigned_abs()).min(latest)
            });
        }
        self.expires
            .map(|at| u64::try_from(at).unwrap_or(EARLIEST_EXPIRY).min(latest))
    }

    /// The prefix the cookie name carries, if any.
    #[must_use]
    pub fn prefix(&self) -> Option<CookiePrefix> {
        CookiePrefix::of(&self.name)
    }

    /// Applies one `name=value` attribute (RFC 6265 sections 5.2.1 to 5.2.6, RFC 6265bis 5.6.7).
    fn apply(&mut self, attribute: &str) {
        let (name, value) = attribute.split_once('=').unwrap_or((attribute, ""));
        let (name, value) = (trim_whitespace(name), trim_whitespace(value));
        if value.len() > MAX_ATTRIBUTE_VALUE_BYTES {
            return;
        }
        let Some(kind) = Attribute::named(name) else {
            return;
        };
        match kind {
            Attribute::Expires => {
                if let Some(at) = parse_cookie_date(value) {
                    self.expires = Some(at);
                }
            }
            Attribute::MaxAge => {
                if let Some(delta) = parse_max_age(value) {
                    self.max_age = Some(delta);
                }
            }
            Attribute::Domain => {
                if !value.is_empty() {
                    let bare = value.strip_prefix('.').unwrap_or(value);
                    self.domain = Some(bare.to_ascii_lowercase());
                }
            }
            Attribute::Path => self.path = Some(value.to_owned()),
            Attribute::Secure => self.secure = true,
            Attribute::HttpOnly => self.http_only = true,
            Attribute::SameSite => self.same_site = parse_same_site(value),
        }
    }
}

/// Refuses a pair the store could never hold (RFC 6265bis section 5.6 steps 4 to 7).
fn check_pair(name: &str, value: &str) -> Result<(), SetCookieError> {
    if name.is_empty() && value.is_empty() {
        return Err(SetCookieError::Empty);
    }
    let len = name.len() + value.len();
    if len > MAX_NAME_VALUE_BYTES {
        return Err(SetCookieError::TooLarge { len });
    }
    if name.is_empty() {
        if let Some(prefix) = CookiePrefix::of(value) {
            return Err(SetCookieError::NamelessPrefix(prefix));
        }
    }
    Ok(())
}

/// Removes leading and trailing spaces and tabs, the whitespace RFC 6265 trims.
fn trim_whitespace(text: &str) -> &str {
    text.trim_matches([' ', '\t'])
}

/// `Max-Age` per RFC 6265 section 5.2.2: an optional `-` and digits, saturated at the `i64` bounds.
fn parse_max_age(value: &str) -> Option<i64> {
    let (negative, digits) = match value.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let magnitude = digits.bytes().fold(0_i64, |acc, b| {
        acc.saturating_mul(i64::from(DECIMAL))
            .saturating_add(i64::from(b - b'0'))
    });
    Some(if negative { -magnitude } else { magnitude })
}

fn parse_same_site(value: &str) -> Option<SameSite> {
    SAME_SITE_POLICIES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(value))
        .map(|&(_, policy)| policy)
}

impl fmt::Debug for SetCookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetCookie")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("expires", &self.expires)
            .field("max_age", &self.max_age)
            .field("domain", &self.domain)
            .field("path", &self.path)
            .field("secure", &self.secure)
            .field("http_only", &self.http_only)
            .field("same_site", &self.same_site)
            .finish()
    }
}

/// Whether `text` holds a control character a cookie may not carry: `%x00-08`, `%x0A-1F` or
/// `%x7F`. A horizontal tab is allowed.
#[must_use]
pub fn has_forbidden_control(text: &str) -> bool {
    text.bytes()
        .any(|b| matches!(b, 0x00..=0x08 | 0x0A..=0x1F | 0x7F))
}

/// Why a `Set-Cookie` header was refused as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SetCookieError {
    /// The header holds a control character other than a tab.
    #[error("the header holds a control character")]
    ControlCharacter,
    /// The cookie has neither a name nor a value.
    #[error("the cookie has neither a name nor a value")]
    Empty,
    /// The name and the value together exceed [`MAX_NAME_VALUE_BYTES`].
    #[error("the name and value take {len} bytes; at most {max} are allowed", max = MAX_NAME_VALUE_BYTES)]
    TooLarge {
        /// The bytes the name and value take together.
        len: usize,
    },
    /// A nameless cookie's value starts with a cookie prefix.
    #[error("a nameless cookie's value starts with the {0:?} prefix")]
    NamelessPrefix(CookiePrefix),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::humanize::Rng;

    const NOW: u64 = 1_700_000_000;

    fn parse(header: &str) -> SetCookie {
        SetCookie::parse(header).unwrap_or_else(|e| panic!("{header:?} should parse: {e}"))
    }

    #[test]
    fn reads_the_pair_and_every_attribute() {
        let c = parse(" sid = abc ; Path=/app; Domain=.Example.COM; Secure; HttpOnly; SameSite=Strict; Max-Age=60");
        assert_eq!((c.name.as_str(), c.value.as_str()), ("sid", "abc"));
        assert_eq!(c.path.as_deref(), Some("/app"));
        assert_eq!(c.domain.as_deref(), Some("example.com"));
        assert!(c.secure && c.http_only);
        assert_eq!(c.same_site, Some(SameSite::Strict));
        assert_eq!(c.max_age, Some(60));
        assert_eq!(c.expiry(NOW), Some(NOW + 60));
    }

    #[test]
    fn attribute_names_ignore_case_and_the_last_one_wins() {
        let c = parse(
            "a=1; PATH=/one; path=/two; DOMAIN=a.example; domain=b.example; sEcUrE; SAMESITE=lax",
        );
        assert_eq!(c.path.as_deref(), Some("/two"));
        assert_eq!(c.domain.as_deref(), Some("b.example"));
        assert!(c.secure);
        assert_eq!(c.same_site, Some(SameSite::Lax));
        let reset = parse("a=1; SameSite=None; SameSite=bogus");
        assert_eq!(
            reset.same_site, None,
            "an unknown policy resets to the default"
        );
    }

    #[test]
    fn a_flag_attribute_ignores_its_value_and_unknown_attributes_are_skipped() {
        let c = parse("a=1; Secure=false; HttpOnly=0; Partitioned; Priority=High");
        assert!(c.secure && c.http_only);
    }

    #[test]
    fn a_pair_without_equals_is_a_nameless_cookie() {
        let c = parse("just-a-value; Path=/");
        assert_eq!((c.name.as_str(), c.value.as_str()), ("", "just-a-value"));
        let c = parse("=v");
        assert_eq!((c.name.as_str(), c.value.as_str()), ("", "v"));
        let c = parse("n=");
        assert_eq!((c.name.as_str(), c.value.as_str()), ("n", ""));
        assert_eq!(
            parse("a=b=c").value,
            "b=c",
            "the value keeps later equals signs"
        );
    }

    #[test]
    fn an_empty_pair_is_refused() {
        for header in ["", "=", " = ", ";Path=/", "\t;"] {
            assert_eq!(
                SetCookie::parse(header),
                Err(SetCookieError::Empty),
                "{header:?}"
            );
        }
    }

    #[test]
    fn every_control_character_but_tab_refuses_the_header() {
        for byte in (0_u8..=0x1F).chain([0x7F]) {
            let header = format!("a=b{}c; Path=/", char::from(byte));
            let result = SetCookie::parse(&header);
            if byte == b'\t' {
                assert_eq!(result.map(|c| c.value), Ok("b\tc".to_owned()));
            } else {
                assert_eq!(
                    result,
                    Err(SetCookieError::ControlCharacter),
                    "byte {byte:#04x}"
                );
            }
        }
        assert_eq!(
            SetCookie::parse("a=b; Path=/\r\nSet-Cookie: evil=1"),
            Err(SetCookieError::ControlCharacter),
            "a header split in an attribute is refused too"
        );
    }

    #[test]
    fn name_and_value_may_hold_4096_bytes_together() {
        let fits = format!("{}={}", "n".repeat(96), "v".repeat(4000));
        assert_eq!(parse(&fits).value.len(), 4000);
        let over = format!("{}={}", "n".repeat(96), "v".repeat(4001));
        assert_eq!(
            SetCookie::parse(&over),
            Err(SetCookieError::TooLarge { len: 4097 })
        );
    }

    #[test]
    fn an_attribute_value_over_1024_bytes_is_ignored() {
        let long = format!("/{}", "p".repeat(MAX_ATTRIBUTE_VALUE_BYTES));
        let c = parse(&format!("a=1; Path=/kept; Path={long}"));
        assert_eq!(c.path.as_deref(), Some("/kept"));
        let at_limit = format!("/{}", "p".repeat(MAX_ATTRIBUTE_VALUE_BYTES - 1));
        assert_eq!(parse(&format!("a=1; Path={at_limit}")).path, Some(at_limit));
    }

    #[test]
    fn a_nameless_cookie_cannot_pose_as_a_prefixed_one() {
        assert_eq!(
            SetCookie::parse("__Host-sid=1; Secure; Path=/").map(|c| c.prefix()),
            Ok(Some(CookiePrefix::Host))
        );
        for (header, prefix) in [
            ("=__Host-sid=1", CookiePrefix::Host),
            ("__secure-sid", CookiePrefix::Secure),
            ("=__HOST-x", CookiePrefix::Host),
        ] {
            assert_eq!(
                SetCookie::parse(header),
                Err(SetCookieError::NamelessPrefix(prefix))
            );
        }
    }

    #[test]
    fn prefixes_match_without_regard_to_case() {
        assert_eq!(CookiePrefix::of("__secure-a"), Some(CookiePrefix::Secure));
        assert_eq!(CookiePrefix::of("__HOST-a"), Some(CookiePrefix::Host));
        assert_eq!(CookiePrefix::of("__Hos"), None);
        assert_eq!(CookiePrefix::of("_Host-a"), None);
        assert_eq!(CookiePrefix::of("é__Host-"), None);
    }

    #[test]
    fn the_domain_attribute_is_lowered_and_loses_one_leading_dot() {
        assert_eq!(
            parse("a=1; Domain=.WWW.Example.com").domain.as_deref(),
            Some("www.example.com")
        );
        assert_eq!(
            parse("a=1; Domain=..x.com").domain.as_deref(),
            Some(".x.com")
        );
        assert_eq!(parse("a=1; Domain=.").domain.as_deref(), Some(""));
        assert_eq!(
            parse("a=1; Domain=x.com; Domain=").domain.as_deref(),
            Some("x.com")
        );
    }

    #[test]
    fn max_age_wins_over_expires_in_either_order() {
        let expires = "Expires=Wed, 21 Oct 2037 07:28:00 GMT";
        for header in [
            format!("a=1; Max-Age=30; {expires}"),
            format!("a=1; {expires}; Max-Age=30"),
        ] {
            assert_eq!(parse(&header).expiry(NOW), Some(NOW + 30), "{header}");
        }
        assert_eq!(
            parse("a=1").expiry(NOW),
            None,
            "no expiry is a session cookie"
        );
    }

    #[test]
    fn zero_or_negative_max_age_expires_at_once() {
        for value in ["0", "-1", "-0", "-99999999999999999999999"] {
            assert_eq!(
                parse(&format!("a=1; Max-Age={value}")).expiry(NOW),
                Some(EARLIEST_EXPIRY)
            );
        }
    }

    #[test]
    fn a_malformed_max_age_is_ignored() {
        for value in ["", "abc", "+5", "1.5", "-", "5s", " "] {
            let c = parse(&format!("a=1; Max-Age=7; Max-Age={value}"));
            assert_eq!(c.max_age, Some(7), "Max-Age={value:?}");
        }
    }

    #[test]
    fn every_expiry_is_capped_at_400_days() {
        let cap = NOW + 400 * 86_400;
        assert_eq!(
            parse("a=1; Max-Age=999999999999999999999999").expiry(NOW),
            Some(cap)
        );
        assert_eq!(
            parse("a=1; Expires=Fri, 01 Jan 9999 00:00:00 GMT").expiry(NOW),
            Some(cap)
        );
        assert_eq!(
            parse("a=1; Max-Age=60").expiry(u64::MAX),
            Some(u64::MAX),
            "saturates"
        );
    }

    #[test]
    fn an_expires_before_the_epoch_is_the_earliest_time() {
        assert_eq!(
            parse("a=1; Expires=Thu, 01 Jan 1601 00:00:00 GMT").expiry(NOW),
            Some(0)
        );
        assert_eq!(
            parse("a=1; Expires=nonsense").expiry(NOW),
            None,
            "a bad date is ignored"
        );
    }

    #[test]
    fn has_forbidden_control_matches_the_rfc_set() {
        assert!(!has_forbidden_control("plain\ttext é"));
        assert!(has_forbidden_control("nul\0"));
        assert!(has_forbidden_control("del\u{7f}"));
        assert!(has_forbidden_control("lf\n"));
    }

    #[test]
    fn errors_name_what_was_wrong() {
        let too_large = SetCookieError::TooLarge { len: 5000 }.to_string();
        assert!(
            too_large.contains("5000") && too_large.contains("4096"),
            "{too_large}"
        );
        let nameless = SetCookieError::NamelessPrefix(CookiePrefix::Host).to_string();
        assert!(nameless.contains("Host"), "{nameless}");
    }

    #[test]
    fn debug_hides_the_value() {
        let c = parse("sid=s3cret-value; Path=/");
        assert!(!format!("{c:?}").contains("s3cret"));
    }

    /// Random text built from bytes that stress the parser: separators, controls, prefixes.
    fn random_header(rng: &mut Rng) -> String {
        const PIECES: [&str; 16] = [
            "a",
            "=",
            ";",
            " ",
            "\t",
            "\r",
            "\0",
            "__Host-",
            "__Secure-",
            "Path=/",
            "Domain=.x.com",
            "Max-Age=-1",
            "Expires=",
            "é",
            "\u{7f}",
            "SameSite=None",
        ];
        let len = rng.next_u64() % 24;
        (0..len).map(|_| *rng.pick(&PIECES)).collect()
    }

    #[test]
    fn generated_headers_never_yield_an_unsafe_cookie() {
        let mut rng = Rng::new(0x5e7_c00c1e);
        for _ in 0..20_000 {
            let header = random_header(&mut rng);
            let Ok(c) = SetCookie::parse(&header) else {
                continue;
            };
            assert!(
                !has_forbidden_control(&c.name) && !has_forbidden_control(&c.value),
                "{header:?}"
            );
            assert!(
                !c.name.contains([';', '=']) && !c.value.contains(';'),
                "{header:?}"
            );
            assert!(!c.name.is_empty() || !c.value.is_empty(), "{header:?}");
            assert!(c.name.len() + c.value.len() <= MAX_NAME_VALUE_BYTES);
            assert!(
                !c.name.is_empty() || CookiePrefix::of(&c.value).is_none(),
                "{header:?}"
            );
        }
    }
}
