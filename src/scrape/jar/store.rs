//! An RFC 6265 cookie jar, with public-suffix checks through `psl`, that converts to and from CDP cookies.
//!
//! [`CookieJar::store`] applies the storage model of RFC 6265 section 5.3 with the RFC 6265bis
//! rules Chrome ships: a `Secure` cookie needs a secure origin and an insecure origin may not
//! overwrite or shadow one, the `__Secure-` and `__Host-` prefixes are enforced, `SameSite=None`
//! needs `Secure`, and a `Domain` on a public suffix or an IP address is refused. A path longer
//! than [`MAX_PATH_BYTES`] and a domain longer than [`MAX_DOMAIN_BYTES`] are refused, whether the
//! path is an attribute or the default path of the request URL. The jar holds at
//! most [`MAX_COOKIES_PER_SITE`] cookies and [`MAX_BYTES_PER_SITE`] bytes per site and
//! [`MAX_COOKIES`] cookies in all.
//! [`CookieJar::header_for`] builds the `Cookie` header of section 5.4. Times are seconds since the
//! Unix epoch, and every function that needs the time takes `now`.
//!
//! A secure origin is https or wss, or http or ws on a loopback host (`localhost`, `127.0.0.0/8`,
//! `::1`), which Chrome treats as trustworthy for cookies.

mod cdp;
mod rules;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use url::Url;

use super::set_cookie::{CookiePrefix, SetCookie, SetCookieError, MAX_ATTRIBUTE_VALUE_BYTES};
use crate::sb_cdp::{Cookie, SameSite};
use rules::{default_path, domain_match, path_match, site_key, validate, RequestOrigin};

pub use rules::site_of;

/// The most cookies one site may hold. A new cookie beyond it evicts another of the same site.
pub const MAX_COOKIES_PER_SITE: usize = 180;

/// The most cookies the jar holds in all. A new cookie beyond it evicts another.
pub const MAX_COOKIES: usize = 3000;

/// The most bytes the cookies of one site may hold in all, counting each cookie's name, value,
/// domain and path. A new cookie beyond it evicts others of the same site, like the count bound.
/// The jar as a whole is bounded by [`MAX_COOKIES`] cookies of at most 5,373 bytes each (a name
/// and value of 4096, a path of 1024 and a domain of 253).
pub const MAX_BYTES_PER_SITE: usize = 256 * 1024;

/// The most bytes a cookie path may hold, whether from the `Path` attribute or from the default
/// path of the request URL. It is the bound of an attribute value (RFC 6265bis).
pub const MAX_PATH_BYTES: usize = MAX_ATTRIBUTE_VALUE_BYTES;

/// The most bytes a cookie domain may hold: the longest DNS name, without a trailing dot.
pub const MAX_DOMAIN_BYTES: usize = 253;

/// The `Path` a `__Host-` cookie must be set with.
const ROOT_PATH: &str = "/";

/// The separator between the cookies of a `Cookie` header (RFC 6265 section 5.4 step 4).
const COOKIE_SEPARATOR: &str = "; ";

/// A cookie as the jar holds it (RFC 6265 section 5.3).
///
/// The value is a credential, so [`Debug`](fmt::Debug) hides it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCookie {
    /// The cookie name; empty for a nameless cookie.
    pub name: String,
    /// The cookie value.
    pub value: String,
    /// The host (when `host_only`) or the domain the cookie belongs to, in lower case, no leading dot.
    pub domain: String,
    /// Sent only to `domain` itself, not to its subdomains.
    pub host_only: bool,
    /// The path the cookie applies to; it starts with `/`.
    pub path: String,
    /// Expiry in seconds since the Unix epoch; `None` for a session cookie.
    pub expires: Option<u64>,
    /// Sent only to a secure origin.
    pub secure: bool,
    /// Hidden from page scripts.
    pub http_only: bool,
    /// The `SameSite` policy; `None` is the default, enforced as `Lax`.
    pub same_site: Option<SameSite>,
    /// When the cookie was first stored; an overwrite keeps it.
    pub created: u64,
    /// When the cookie was last stored or sent.
    pub last_access: u64,
}

impl StoredCookie {
    /// Whether the cookie has expired at `now`: it has an expiry and that time has come.
    #[must_use]
    pub fn is_expired(&self, now: u64) -> bool {
        self.expires.is_some_and(|at| at <= now)
    }

    /// The bytes the cookie counts for against [`MAX_BYTES_PER_SITE`]: name, value, domain, path.
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        self.name.len() + self.value.len() + self.domain.len() + self.path.len()
    }

    /// Whether `other` has the same name, domain, host-only flag and path, so one replaces the other.
    #[must_use]
    pub fn same_identity(&self, other: &Self) -> bool {
        self.name == other.name
            && self.domain == other.domain
            && self.host_only == other.host_only
            && self.path == other.path
    }

    /// Whether the cookie goes into a request to `origin` made in `context` (RFC 6265 section
    /// 5.4 step 1, RFC 6265bis section 5.8.3).
    fn is_sent_to(&self, origin: &RequestOrigin, context: SiteContext) -> bool {
        let host_matches = if self.host_only {
            origin.host == self.domain
        } else {
            domain_match(&origin.host, &self.domain)
        };
        host_matches
            && path_match(&origin.path, &self.path)
            && (origin.secure || !self.secure)
            && context.allows(self.same_site)
    }
}

impl fmt::Debug for StoredCookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredCookie")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("domain", &self.domain)
            .field("host_only", &self.host_only)
            .field("path", &self.path)
            .field("expires", &self.expires)
            .field("secure", &self.secure)
            .field("http_only", &self.http_only)
            .field("same_site", &self.same_site)
            .field("created", &self.created)
            .field("last_access", &self.last_access)
            .finish()
    }
}

/// Where a request comes from relative to its target, which decides the `SameSite` rules
/// (RFC 6265bis section 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteContext {
    /// The request comes from the target's own site: every cookie may be sent.
    SameSite {
        /// The request is a top-level navigation. It does not change which cookies are sent.
        top_level: bool,
    },
    /// The request comes from another site: `Strict` cookies are never sent, `Lax` and default
    /// cookies only with a top-level navigation by a safe method, `None` cookies always.
    CrossSite {
        /// The request is a top-level navigation by a safe method, such as `GET`.
        top_level_get: bool,
    },
}

impl SiteContext {
    /// Whether a cookie with `policy` may be sent in this context. No policy is enforced as `Lax`,
    /// as Chrome does.
    fn allows(self, policy: Option<SameSite>) -> bool {
        match self {
            Self::SameSite { .. } => true,
            Self::CrossSite { top_level_get } => match policy.unwrap_or(SameSite::Lax) {
                SameSite::Strict => false,
                SameSite::Lax => top_level_get,
                SameSite::None => true,
            },
        }
    }
}

/// What [`CookieJar::store`] did with a `Set-Cookie` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoreOutcome {
    /// A new cookie was stored.
    Stored,
    /// A cookie with the same name, domain and path was replaced; its creation time was kept.
    Replaced,
    /// The header expired the cookie; a stored cookie with the same name, domain and path is gone.
    Deleted,
    /// The header was refused, and no cookie was stored, replaced or removed.
    Ignored(IgnoreReason),
}

/// Why a cookie was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IgnoreReason {
    /// The header or cookie is malformed: a control character, no name and no value, a `;` or
    /// `=` where none may be, or a domain or path that is not a valid host or path.
    Malformed,
    /// The `Domain` attribute does not domain-match the request host.
    DomainMismatch,
    /// The `Domain` attribute is a public suffix other than the request host: a supercookie.
    PublicSuffix,
    /// The request host or the `Domain` attribute is an IP address and the two differ.
    IpLiteralDomain,
    /// A `Secure` cookie from an insecure origin, or an insecure origin that would overwrite or
    /// shadow a `Secure` cookie.
    SecureFromInsecure,
    /// A `__Host-` cookie that is not `Secure`, carries a `Domain`, or was not set with `Path=/`.
    HostPrefix,
    /// A `__Secure-` cookie that is not `Secure`.
    SecurePrefix,
    /// The name and value exceed [`MAX_NAME_VALUE_BYTES`](super::set_cookie::MAX_NAME_VALUE_BYTES),
    /// the path exceeds [`MAX_PATH_BYTES`], or the domain exceeds [`MAX_DOMAIN_BYTES`]. The path
    /// may be the default path of the request URL, which a site chooses.
    TooLarge,
    /// `SameSite=None` without `Secure`.
    SameSiteNoneInsecure,
    /// The request URL is not http, https, ws or wss, or it has no host.
    UnsupportedUrl,
}

impl From<SetCookieError> for IgnoreReason {
    fn from(error: SetCookieError) -> Self {
        match error {
            SetCookieError::ControlCharacter | SetCookieError::Empty => Self::Malformed,
            SetCookieError::TooLarge { .. } => Self::TooLarge,
            SetCookieError::NamelessPrefix(prefix) => prefix_reason(prefix),
        }
    }
}

/// The reason a cookie breaking the rules of `prefix` is refused for.
fn prefix_reason(prefix: CookiePrefix) -> IgnoreReason {
    match prefix {
        CookiePrefix::Secure => IgnoreReason::SecurePrefix,
        CookiePrefix::Host => IgnoreReason::HostPrefix,
    }
}

/// An RFC 6265 cookie store.
///
/// It serializes as `{"cookies": [...]}`; reading one back checks every cookie against the rules
/// [`CookieJar::store`] applies, so an edited file cannot smuggle in a cookie the jar would refuse.
/// [`Debug`](fmt::Debug) prints counts only.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "JarFile", into = "JarFile")]
pub struct CookieJar {
    cookies: Vec<StoredCookie>,
}

impl CookieJar {
    /// An empty jar.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many cookies the jar holds, expired ones not yet removed included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cookies.len()
    }

    /// Whether the jar holds no cookie.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// The cookies, in the order they were first stored.
    #[must_use]
    pub fn cookies(&self) -> &[StoredCookie] {
        &self.cookies
    }

    /// Stores the cookie of one `Set-Cookie` header received for `request_url` (RFC 6265 section
    /// 5.3 with RFC 6265bis section 5.7). An overwrite keeps the old creation time; an expired
    /// cookie removes the stored one. Expired cookies are purged, and when a bound is passed the
    /// cookie evicted is, among the others, an insecure one before a `Secure` one, then the least
    /// recently accessed, then the oldest.
    pub fn store(&mut self, request_url: &Url, set_cookie: &str, now: u64) -> StoreOutcome {
        let Some(origin) = RequestOrigin::of(request_url) else {
            return StoreOutcome::Ignored(IgnoreReason::UnsupportedUrl);
        };
        let built = SetCookie::parse(set_cookie)
            .map_err(IgnoreReason::from)
            .and_then(|parsed| build_cookie(&parsed, &origin, now));
        let cookie = match built {
            Ok(cookie) => cookie,
            Err(reason) => return StoreOutcome::Ignored(reason),
        };
        if !origin.secure && self.shadows_secure(&cookie, now) {
            return StoreOutcome::Ignored(IgnoreReason::SecureFromInsecure);
        }
        self.insert(cookie, now)
    }

    /// The `Cookie` header value for a request to `request_url` made in `context`, or `None` when
    /// no cookie applies or the URL cannot carry cookies (RFC 6265 section 5.4). Cookies with
    /// longer paths come first, then those created earlier. Expired cookies are purged, and every
    /// cookie sent has its last access set to `now`.
    pub fn header_for(
        &mut self,
        request_url: &Url,
        context: SiteContext,
        now: u64,
    ) -> Option<String> {
        let origin = RequestOrigin::of(request_url)?;
        self.remove_expired(now);
        let mut picked: Vec<usize> = (0..self.cookies.len())
            .filter(|&index| self.cookies[index].is_sent_to(&origin, context))
            .collect();
        if picked.is_empty() {
            return None;
        }
        // A stable sort: cookies created in the same second keep the order they were stored in.
        picked.sort_by(|&a, &b| {
            let (a, b) = (&self.cookies[a], &self.cookies[b]);
            b.path
                .len()
                .cmp(&a.path.len())
                .then(a.created.cmp(&b.created))
        });
        let mut header = String::new();
        for (position, &index) in picked.iter().enumerate() {
            let cookie = &mut self.cookies[index];
            cookie.last_access = now;
            if position > 0 {
                header.push_str(COOKIE_SEPARATOR);
            }
            if !cookie.name.is_empty() {
                header.push_str(&cookie.name);
                header.push('=');
            }
            header.push_str(&cookie.value);
        }
        Some(header)
    }

    /// A jar holding `cookies` as CDP reports them (`Network.getAllCookies`). A leading dot in the
    /// domain marks a domain cookie, no dot a host-only one, and `expires: None` a session cookie.
    /// A cookie the jar would refuse (see [`IgnoreReason`]), one without a domain, and an expired
    /// one are left out; a later cookie with the same name, domain and path replaces an earlier one.
    #[must_use]
    pub fn from_cdp(cookies: &[Cookie], now: u64) -> Self {
        let mut jar = Self::new();
        for cookie in cookies {
            if let Ok(stored) = cdp::from_cdp(cookie, now) {
                jar.insert(stored, now);
            }
        }
        jar
    }

    /// The unexpired cookies as CDP cookies, in the form `Network.getAllCookies` reports: a domain
    /// cookie's domain gets a leading dot, a host-only cookie's has none, and a session cookie has
    /// no expiry. Whether `Network.setCookies` keeps a host-only cookie host-only depends on how the
    /// caller sends it; a bare domain alone may be read as a `Domain` attribute.
    #[must_use]
    pub fn to_cdp(&self, now: u64) -> Vec<Cookie> {
        self.cookies
            .iter()
            .filter(|cookie| !cookie.is_expired(now))
            .map(cdp::to_cdp)
            .collect()
    }

    /// The latest expiry among the persistent cookies called `name` on `site`, a registrable domain
    /// or any host on it. `None` when there is no such cookie or only session cookies.
    #[must_use]
    pub fn expiry_of(&self, name: &str, site: &str) -> Option<u64> {
        let wanted = site_key(site);
        self.cookies
            .iter()
            .filter(|cookie| cookie.name == name && site_key(&cookie.domain) == wanted)
            .filter_map(|cookie| cookie.expires)
            .max()
    }

    /// Removes every cookie expired at `now` and returns how many there were.
    pub fn remove_expired(&mut self, now: u64) -> usize {
        let before = self.cookies.len();
        self.cookies.retain(|cookie| !cookie.is_expired(now));
        before - self.cookies.len()
    }

    /// RFC 6265bis section 5.7 step 16: whether a cookie from an insecure origin would overwrite or
    /// shadow a live `Secure` cookie of the same name whose domain and path overlap it.
    fn shadows_secure(&self, cookie: &StoredCookie, now: u64) -> bool {
        self.cookies.iter().any(|old| {
            old.secure
                && !old.is_expired(now)
                && old.name == cookie.name
                && (domain_match(&old.domain, &cookie.domain)
                    || domain_match(&cookie.domain, &old.domain))
                && path_match(&cookie.path, &old.path)
        })
    }

    /// Stores a checked cookie: replace, delete or add, then keep within the bounds.
    fn insert(&mut self, mut cookie: StoredCookie, now: u64) -> StoreOutcome {
        self.remove_expired(now);
        let old = self.cookies.iter().position(|c| c.same_identity(&cookie));
        if cookie.is_expired(now) {
            if let Some(index) = old {
                self.cookies.remove(index);
            }
            return StoreOutcome::Deleted;
        }
        if let Some(index) = old {
            cookie.created = self.cookies[index].created;
            self.cookies[index] = cookie;
            self.evict_beyond_bounds(index);
            return StoreOutcome::Replaced;
        }
        self.cookies.push(cookie);
        self.evict_beyond_bounds(self.cookies.len() - 1);
        StoreOutcome::Stored
    }

    /// Evicts until the site of the cookie at `keep`, which is never evicted, holds at most
    /// [`MAX_COOKIES_PER_SITE`] cookies and [`MAX_BYTES_PER_SITE`] bytes, and the jar at most
    /// [`MAX_COOKIES`] cookies. A replacement can grow a cookie, so `keep` is not always the last.
    fn evict_beyond_bounds(&mut self, mut keep: usize) {
        let Some(kept) = self.cookies.get(keep) else {
            return;
        };
        let site = site_key(&kept.domain);
        let on_site = |cookie: &StoredCookie| counts_for(&cookie.domain, &site);
        while self.site_over_bound(on_site) && self.evict_one(&mut keep, on_site) {}
        while self.cookies.len() > MAX_COOKIES && self.evict_one(&mut keep, |_| true) {}
    }

    /// Whether the cookies `on_site` hold more than [`MAX_COOKIES_PER_SITE`] cookies or
    /// [`MAX_BYTES_PER_SITE`] bytes.
    fn site_over_bound(&self, on_site: impl Fn(&StoredCookie) -> bool) -> bool {
        let (count, bytes) = self
            .cookies
            .iter()
            .filter(|&cookie| on_site(cookie))
            .fold((0_usize, 0_usize), |(count, bytes), cookie| {
                (count + 1, bytes + cookie.size_bytes())
            });
        count > MAX_COOKIES_PER_SITE || bytes > MAX_BYTES_PER_SITE
    }

    /// Removes the eviction victim among the `eligible` cookies and moves `keep` along with the
    /// cookies after it. `false` when no cookie other than `keep` is eligible.
    fn evict_one(&mut self, keep: &mut usize, eligible: impl Fn(&StoredCookie) -> bool) -> bool {
        let Some(victim) = self.eviction_victim(*keep, eligible) else {
            return false;
        };
        self.cookies.remove(victim);
        if victim < *keep {
            *keep -= 1;
        }
        true
    }

    /// The index of the cookie to evict first among the `eligible` ones other than the one at
    /// `keep`: an insecure cookie before a `Secure` one, then the least recently accessed, then the
    /// oldest, then the first stored.
    fn eviction_victim(
        &self,
        keep: usize,
        eligible: impl Fn(&StoredCookie) -> bool,
    ) -> Option<usize> {
        self.cookies
            .iter()
            .enumerate()
            .filter(|&(index, cookie)| index != keep && eligible(cookie))
            .min_by_key(|(_, cookie)| (cookie.secure, cookie.last_access, cookie.created))
            .map(|(index, _)| index)
    }
}

impl fmt::Debug for CookieJar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sites: HashSet<String> = self.cookies.iter().map(|c| site_key(&c.domain)).collect();
        f.debug_struct("CookieJar")
            .field("cookies", &self.cookies.len())
            .field("sites", &sites.len())
            .finish()
    }
}

/// Whether a cookie for `domain` counts against `site`. A cookie's site is a suffix of its domain at
/// a label boundary, so that cheap test runs first and the public-suffix lookup only after it.
fn counts_for(domain: &str, site: &str) -> bool {
    let candidate = domain
        .strip_suffix(site)
        .is_some_and(|head| head.is_empty() || head.ends_with('.'));
    candidate && site_key(domain) == site
}

/// Builds the cookie a parsed header asks for, for a request to `origin` (RFC 6265 section 5.3
/// steps 2 to 10, RFC 6265bis section 5.7), and checks it.
fn build_cookie(
    parsed: &SetCookie,
    origin: &RequestOrigin,
    now: u64,
) -> Result<StoredCookie, IgnoreReason> {
    if parsed.secure && !origin.secure {
        return Err(IgnoreReason::SecureFromInsecure);
    }
    let set_as_host = parsed.domain.is_none() && parsed.path.as_deref() == Some(ROOT_PATH);
    if parsed.prefix() == Some(CookiePrefix::Host) && !set_as_host {
        return Err(IgnoreReason::HostPrefix);
    }
    let (domain, host_only) = origin.resolve_domain(parsed.domain.as_deref())?;
    let path = match parsed.path.as_deref() {
        Some(path) if path.starts_with('/') => path.to_owned(),
        _ => default_path(&origin.path),
    };
    let cookie = StoredCookie {
        name: parsed.name.clone(),
        value: parsed.value.clone(),
        domain,
        host_only,
        path,
        expires: parsed.expiry(now),
        secure: parsed.secure,
        http_only: parsed.http_only,
        same_site: parsed.same_site,
        created: now,
        last_access: now,
    };
    validate(&cookie)?;
    Ok(cookie)
}

/// The serialized form of a [`CookieJar`].
#[derive(Serialize, Deserialize)]
struct JarFile {
    cookies: Vec<StoredCookie>,
}

impl From<CookieJar> for JarFile {
    fn from(jar: CookieJar) -> Self {
        Self {
            cookies: jar.cookies,
        }
    }
}

impl TryFrom<JarFile> for CookieJar {
    type Error = JarError;

    fn try_from(file: JarFile) -> Result<Self, JarError> {
        check_file(&file.cookies)?;
        Ok(Self {
            cookies: file.cookies,
        })
    }
}

/// Checks a serialized jar against what [`CookieJar::store`] guarantees: every cookie valid, no
/// two with the same identity, and the bounds kept.
fn check_file(cookies: &[StoredCookie]) -> Result<(), JarError> {
    let count = cookies.len();
    if count > MAX_COOKIES {
        return Err(JarError::TooMany { count });
    }
    let mut identities = HashSet::new();
    let mut per_site: HashMap<String, (usize, usize)> = HashMap::new();
    for (index, cookie) in cookies.iter().enumerate() {
        validate(cookie).map_err(|reason| JarError::Invalid { index, reason })?;
        let identity = (&cookie.name, &cookie.domain, cookie.host_only, &cookie.path);
        if !identities.insert(identity) {
            return Err(JarError::Duplicate { index });
        }
        let (held, bytes) = per_site.entry(site_key(&cookie.domain)).or_default();
        *held += 1;
        *bytes += cookie.size_bytes();
        if *held > MAX_COOKIES_PER_SITE {
            return Err(JarError::SiteFull { index });
        }
        if *bytes > MAX_BYTES_PER_SITE {
            return Err(JarError::SiteBytes { index });
        }
    }
    Ok(())
}

/// Why a serialized jar was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JarError {
    /// A stored cookie breaks a rule the jar enforces.
    #[error("stored cookie {index} is refused: {reason:?}")]
    Invalid {
        /// The position of the cookie in the file.
        index: usize,
        /// The rule it breaks.
        reason: IgnoreReason,
    },
    /// A stored cookie repeats the name, domain and path of an earlier one.
    #[error("stored cookie {index} repeats the name, domain and path of an earlier one")]
    Duplicate {
        /// The position of the repeated cookie in the file.
        index: usize,
    },
    /// The file holds more than [`MAX_COOKIES`] cookies.
    #[error("the jar holds {count} cookies; at most {max} are allowed", max = MAX_COOKIES)]
    TooMany {
        /// How many cookies the file holds.
        count: usize,
    },
    /// A site holds more than [`MAX_COOKIES_PER_SITE`] cookies.
    #[error("stored cookie {index} is one too many for its site; at most {max} are allowed", max = MAX_COOKIES_PER_SITE)]
    SiteFull {
        /// The position of the first cookie beyond the bound.
        index: usize,
    },
    /// A site holds more than [`MAX_BYTES_PER_SITE`] bytes of cookies.
    #[error("stored cookie {index} is one too many for the bytes its site may hold; at most {max} are allowed", max = MAX_BYTES_PER_SITE)]
    SiteBytes {
        /// The position of the first cookie beyond the bound.
        index: usize,
    },
}
