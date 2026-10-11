//! The matching rules of RFC 6265 section 5.1, the public-suffix lookup, and the checks every
//! stored cookie passes wherever it came from.

use url::{Host, Url};

use super::super::set_cookie::{has_forbidden_control, CookiePrefix, MAX_NAME_VALUE_BYTES};
use super::{
    prefix_reason, IgnoreReason, StoredCookie, MAX_DOMAIN_BYTES, MAX_PATH_BYTES, ROOT_PATH,
};
use crate::sb_cdp::SameSite;

/// The URL schemes that carry cookies, each with whether it is secure on its own.
const COOKIE_SCHEMES: [(&str, bool); 4] = [
    ("http", false),
    ("https", true),
    ("ws", false),
    ("wss", true),
];

/// The loopback host name (RFC 6761 section 6.3); its subdomains are loopback too.
const LOCALHOST: &str = "localhost";

/// The prefix of a `localhost` subdomain.
const LOCALHOST_SUBDOMAIN: &str = ".localhost";

/// The registrable domain (`eTLD+1`) of `host` from the public suffix list.
///
/// Private suffixes such as `github.io` count as public suffixes. `None` for an IP address, for a
/// public suffix itself, and for an empty host. The host is compared in lower case; a trailing dot
/// is dropped from the answer.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::scrape::jar::store::site_of;
///
/// assert_eq!(site_of("www.example.co.uk").as_deref(), Some("example.co.uk"));
/// assert_eq!(site_of("co.uk"), None);
/// assert_eq!(site_of("192.168.0.1"), None);
/// ```
#[must_use]
pub fn site_of(host: &str) -> Option<String> {
    let host = host.to_ascii_lowercase();
    if host.is_empty() || is_ip_literal(&host) {
        return None;
    }
    let domain = psl::domain(host.as_bytes())?.trim();
    String::from_utf8(domain.as_bytes().to_vec()).ok()
}

/// The key cookies are counted by for the per-site bound: the site of `domain`, or `domain`
/// itself when it has none, such as an IP address or a host that is a public suffix.
pub(super) fn site_key(domain: &str) -> String {
    site_of(domain).unwrap_or_else(|| domain.to_ascii_lowercase())
}

/// Whether `host` is an IPv4 address or a bracketed IPv6 address, in any spelling the URL
/// standard accepts. Only a host that starts with `[` or whose last label starts with a digit is
/// parsed; no top-level domain starts with a digit.
pub(super) fn is_ip_literal(host: &str) -> bool {
    let last_label = host
        .trim_end_matches('.')
        .rsplit('.')
        .next()
        .unwrap_or_default();
    let may_be_ip = host.starts_with('[') || last_label.starts_with(|c: char| c.is_ascii_digit());
    may_be_ip && matches!(Host::parse(host), Ok(Host::Ipv4(_) | Host::Ipv6(_)))
}

/// Whether `domain` is a public suffix, or names no registrable domain at all, so that no cookie
/// may be scoped to it beyond the host itself (RFC 6265 section 5.3 step 5).
pub(super) fn is_public_suffix(domain: &str) -> bool {
    psl::domain(domain.as_bytes()).is_none()
}

/// The canonical spelling of a cookie domain given in lower case without its leading dot: an IP
/// address in its standard form, or an ASCII host name whose labels are not empty (one trailing
/// dot is allowed). `None` for anything else: non-ASCII text (RFC 6265bis), a `%` escape, or a
/// character no host may hold.
pub(super) fn canonical_domain(domain: &str) -> Option<String> {
    if domain.is_empty() || !domain.is_ascii() || domain.contains('%') {
        return None;
    }
    match Host::parse(domain).ok()? {
        Host::Domain(name) => {
            let labels = name.strip_suffix('.').unwrap_or(&name);
            let well_formed = labels.split('.').all(|label| !label.is_empty());
            well_formed.then_some(name)
        }
        address => Some(address.to_string()),
    }
}

/// Whether `string` domain-matches `domain` (RFC 6265 section 5.1.3): the two are identical, or
/// `string` is a host name, not an IP address, that ends with a dot followed by `domain`.
pub(super) fn domain_match(string: &str, domain: &str) -> bool {
    if string == domain {
        return true;
    }
    !domain.is_empty()
        && string
            .strip_suffix(domain)
            .is_some_and(|head| head.ends_with('.'))
        && !is_ip_literal(string)
}

/// Whether `request_path` path-matches `cookie_path` (RFC 6265 section 5.1.4): the two are
/// identical, or `cookie_path` is a prefix of `request_path` that ends with `/` or is followed by
/// `/` in it.
pub(super) fn path_match(request_path: &str, cookie_path: &str) -> bool {
    match request_path.strip_prefix(cookie_path) {
        Some("") => true,
        Some(rest) => cookie_path.ends_with('/') || rest.starts_with('/'),
        None => false,
    }
}

/// The default path of a cookie set by a request for `uri_path` (RFC 6265 section 5.1.4): the
/// path up to, not including, its last `/`, or `/` when that leaves nothing.
pub(super) fn default_path(uri_path: &str) -> String {
    if !uri_path.starts_with('/') {
        return ROOT_PATH.to_owned();
    }
    match uri_path.rfind('/') {
        Some(0) | None => ROOT_PATH.to_owned(),
        Some(last) => uri_path[..last].to_owned(),
    }
}

/// What the storage and retrieval rules need from a request URL.
pub(super) struct RequestOrigin {
    /// The canonical host: lower case, punycode, an IPv6 address in brackets.
    pub(super) host: String,
    /// The host is an IP address.
    pub(super) is_ip: bool,
    /// The scheme is secure, or the host is loopback.
    pub(super) secure: bool,
    /// The URL path, percent-encoded as the URL holds it.
    pub(super) path: String,
}

impl RequestOrigin {
    /// Reads `url`; `None` unless it is http, https, ws or wss with a host.
    pub(super) fn of(url: &Url) -> Option<Self> {
        let &(_, secure_scheme) = COOKIE_SCHEMES
            .iter()
            .find(|(scheme, _)| *scheme == url.scheme())?;
        let host = url.host()?;
        let text = host.to_string();
        if text.is_empty() {
            return None;
        }
        Some(Self {
            is_ip: !matches!(host, Host::Domain(_)),
            secure: secure_scheme || is_loopback(&host),
            host: text,
            path: url.path().to_owned(),
        })
    }

    /// Resolves the `Domain` attribute into the cookie domain and its host-only flag (RFC 6265
    /// section 5.3 steps 4 to 6). No attribute, or an empty one, makes a host-only cookie; so does
    /// an attribute that names the request host when that host is a public suffix or an IP address.
    pub(super) fn resolve_domain(
        &self,
        attribute: Option<&str>,
    ) -> Result<(String, bool), IgnoreReason> {
        let Some(raw) = attribute.filter(|domain| !domain.is_empty()) else {
            return Ok((self.host.clone(), true));
        };
        let domain = canonical_domain(raw).ok_or(IgnoreReason::Malformed)?;
        let narrow_only = if self.is_ip || is_ip_literal(&domain) {
            Some(IgnoreReason::IpLiteralDomain)
        } else if is_public_suffix(&domain) {
            Some(IgnoreReason::PublicSuffix)
        } else {
            None
        };
        match narrow_only {
            // An IP address or a public suffix may only name the request host itself.
            Some(_) if domain == self.host => Ok((domain, true)),
            Some(reason) => Err(reason),
            None if domain_match(&self.host, &domain) => Ok((domain, false)),
            None => Err(IgnoreReason::DomainMismatch),
        }
    }
}

/// Whether `host` is loopback: `localhost`, a subdomain of it, `127.0.0.0/8` or `::1`.
fn is_loopback(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(name) => *name == LOCALHOST || name.ends_with(LOCALHOST_SUBDOMAIN),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

/// Checks what every cookie in the jar satisfies, whether it came from a header, from CDP or
/// from a file: safe name and value text within the size bound, a canonical domain within
/// [`MAX_DOMAIN_BYTES`], a path that starts with `/` and fits [`MAX_PATH_BYTES`], no domain cookie
/// on a public suffix or an IP address, `SameSite=None` only with `Secure`, and the prefix rules.
pub(super) fn validate(cookie: &StoredCookie) -> Result<(), IgnoreReason> {
    check_text(&cookie.name, &cookie.value)?;
    check_scope(cookie)?;
    if cookie.same_site == Some(SameSite::None) && !cookie.secure {
        return Err(IgnoreReason::SameSiteNoneInsecure);
    }
    match CookiePrefix::of(&cookie.name) {
        Some(CookiePrefix::Secure) if !cookie.secure => Err(IgnoreReason::SecurePrefix),
        Some(CookiePrefix::Host)
            if !(cookie.secure && cookie.host_only && cookie.path == ROOT_PATH) =>
        {
            Err(IgnoreReason::HostPrefix)
        }
        _ => Ok(()),
    }
}

/// The name and value can be written into a `Cookie` header as they are, and fit the bound.
fn check_text(name: &str, value: &str) -> Result<(), IgnoreReason> {
    let unsafe_text = |text: &str| has_forbidden_control(text) || text.contains(';');
    if unsafe_text(name) || unsafe_text(value) || name.contains('=') {
        return Err(IgnoreReason::Malformed);
    }
    if name.is_empty() && value.is_empty() {
        return Err(IgnoreReason::Malformed);
    }
    if name.len() + value.len() > MAX_NAME_VALUE_BYTES {
        return Err(IgnoreReason::TooLarge);
    }
    match CookiePrefix::of(value) {
        Some(prefix) if name.is_empty() => Err(prefix_reason(prefix)),
        _ => Ok(()),
    }
}

/// The domain and the path fit their bounds; the domain is canonical and may carry a domain
/// cookie; the path is absolute.
fn check_scope(cookie: &StoredCookie) -> Result<(), IgnoreReason> {
    let domain_len = cookie
        .domain
        .strip_suffix('.')
        .unwrap_or(&cookie.domain)
        .len();
    if domain_len > MAX_DOMAIN_BYTES || cookie.path.len() > MAX_PATH_BYTES {
        return Err(IgnoreReason::TooLarge);
    }
    if canonical_domain(&cookie.domain).as_deref() != Some(cookie.domain.as_str()) {
        return Err(IgnoreReason::Malformed);
    }
    if !cookie.path.starts_with('/') || has_forbidden_control(&cookie.path) {
        return Err(IgnoreReason::Malformed);
    }
    if cookie.host_only {
        Ok(())
    } else if is_ip_literal(&cookie.domain) {
        Err(IgnoreReason::IpLiteralDomain)
    } else if is_public_suffix(&cookie.domain) {
        Err(IgnoreReason::PublicSuffix)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::humanize::Rng;

    #[test]
    fn site_of_uses_the_public_suffix_list() {
        assert_eq!(site_of("www.example.com").as_deref(), Some("example.com"));
        assert_eq!(
            site_of("a.b.example.co.uk").as_deref(),
            Some("example.co.uk")
        );
        assert_eq!(site_of("user.github.io").as_deref(), Some("user.github.io"));
        assert_eq!(site_of("WWW.EXAMPLE.COM").as_deref(), Some("example.com"));
        assert_eq!(site_of("www.example.com.").as_deref(), Some("example.com"));
        for no_site in [
            "com",
            "co.uk",
            "github.io",
            "",
            "127.0.0.1",
            "[::1]",
            "localhost",
        ] {
            assert_eq!(site_of(no_site), None, "{no_site:?}");
        }
    }

    #[test]
    fn ip_literals_are_recognised_in_every_spelling() {
        for ip in ["127.0.0.1", "10.1", "0x7f.0.0.1", "[::1]", "[2001:db8::1]"] {
            assert!(is_ip_literal(ip), "{ip}");
        }
        for name in [
            "example.com",
            "3com.com",
            "1.2.3.4.example",
            "localhost",
            "::1",
            "",
        ] {
            assert!(!is_ip_literal(name), "{name}");
        }
    }

    #[test]
    fn canonical_domain_refuses_what_no_host_may_be() {
        assert_eq!(
            canonical_domain("example.com").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            canonical_domain("example.com.").as_deref(),
            Some("example.com.")
        );
        assert_eq!(canonical_domain("127.1").as_deref(), Some("127.0.0.1"));
        assert_eq!(canonical_domain("[::0:1]").as_deref(), Some("[::1]"));
        assert_eq!(
            canonical_domain("xn--bcher-kva.example").as_deref(),
            Some("xn--bcher-kva.example")
        );
        for bad in [
            "",
            "bücher.example",
            "ex%61mple.com",
            "a..example.com",
            ".example.com",
            "a b.com",
            "a/b",
            "::1",
        ] {
            assert_eq!(canonical_domain(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn domain_match_follows_section_5_1_3() {
        assert!(domain_match("example.com", "example.com"));
        assert!(domain_match("www.example.com", "example.com"));
        assert!(domain_match("a.b.example.com", "example.com"));
        assert!(
            !domain_match("notexample.com", "example.com"),
            "a label boundary is needed"
        );
        assert!(!domain_match("example.com", "www.example.com"));
        assert!(!domain_match("example.com.evil.net", "example.com"));
        assert!(
            !domain_match("1.2.3.4", "2.3.4"),
            "an IP address only matches itself"
        );
        assert!(domain_match("1.2.3.4", "1.2.3.4"));
        assert!(
            !domain_match("example.com.", ""),
            "an empty domain matches nothing"
        );
    }

    #[test]
    fn path_match_follows_section_5_1_4() {
        assert!(path_match("/", "/"));
        assert!(path_match("/a/b", "/a"));
        assert!(path_match("/a/b", "/a/"));
        assert!(path_match("/a/", "/a"));
        assert!(!path_match("/ab", "/a"), "a segment boundary is needed");
        assert!(!path_match("/a", "/a/"));
        assert!(!path_match("/", "/a"));
        assert!(
            path_match("/A", "/A") && !path_match("/a", "/A"),
            "paths are case-sensitive"
        );
    }

    #[test]
    fn default_path_drops_the_last_segment() {
        assert_eq!(default_path(""), "/");
        assert_eq!(default_path("x"), "/");
        assert_eq!(default_path("/"), "/");
        assert_eq!(default_path("/page"), "/");
        assert_eq!(default_path("/app/page"), "/app");
        assert_eq!(default_path("/app/sub/"), "/app/sub");
    }

    #[test]
    fn origins_know_their_scheme_and_loopback() {
        let origin = |text: &str| RequestOrigin::of(&Url::parse(text).expect("test URL"));
        let secure = |text: &str| origin(text).map(|o| o.secure);
        assert_eq!(secure("https://example.com/"), Some(true));
        assert_eq!(secure("wss://example.com/"), Some(true));
        assert_eq!(secure("http://example.com/"), Some(false));
        assert_eq!(secure("ws://example.com/"), Some(false));
        for loopback in [
            "http://localhost/",
            "http://a.localhost/",
            "http://127.0.0.2/",
            "http://[::1]/",
        ] {
            assert_eq!(secure(loopback), Some(true), "{loopback}");
        }
        for refused in [
            "ftp://example.com/",
            "file:///etc/passwd",
            "data:text/plain,x",
            "mailto:a@b.c",
        ] {
            assert!(origin(refused).is_none(), "{refused}");
        }
        let ip = origin("http://[::1]:8080/a").expect("loopback URL");
        assert_eq!(
            (ip.host.as_str(), ip.is_ip, ip.path.as_str()),
            ("[::1]", true, "/a")
        );
    }

    /// Paths made of random segments, some of them odd.
    fn random_path(rng: &mut Rng) -> String {
        const SEGMENTS: [&str; 8] = ["a", "b", "", "..", "%2e", "x y", "é", "index.html"];
        let depth = rng.next_u64() % 6;
        let mut path = String::new();
        for _ in 0..depth {
            path.push('/');
            path.push_str(rng.pick(&SEGMENTS));
        }
        path
    }

    #[test]
    fn generated_paths_match_their_default_path_and_themselves() {
        let mut rng = Rng::new(7);
        for _ in 0..5_000 {
            let path = format!("{}/leaf", random_path(&mut rng));
            let default = default_path(&path);
            assert!(default.starts_with('/'), "{path}");
            assert!(path_match(&path, &default), "{path} vs {default}");
            assert!(path_match(&path, &path));
        }
    }

    #[test]
    fn generated_hosts_never_domain_match_a_foreign_domain() {
        const LABELS: [&str; 6] = ["a", "b", "example", "com", "co", "uk"];
        let mut rng = Rng::new(11);
        for _ in 0..5_000 {
            let host: Vec<&str> = (0..=rng.next_u64() % 4)
                .map(|_| *rng.pick(&LABELS))
                .collect();
            let host = host.join(".");
            let domain: Vec<&str> = (0..=rng.next_u64() % 3)
                .map(|_| *rng.pick(&LABELS))
                .collect();
            let domain = domain.join(".");
            let expected = host == domain || host.ends_with(&format!(".{domain}"));
            assert_eq!(domain_match(&host, &domain), expected, "{host} vs {domain}");
        }
    }
}
