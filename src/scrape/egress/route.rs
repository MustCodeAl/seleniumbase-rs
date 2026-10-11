//! Egress identities: a proxy spec parsed once, with its credentials kept out of every `Debug`, `Display` and serialized form.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::geo::CountryCode;
use super::redact::{RedactedUrl, Secret};
use crate::sb_cdp::Proxy;

/// The schemes a proxy may use. Any other scheme is refused when the spec is parsed.
const SCHEMES: [&str; 4] = ["http", "https", "socks5", "socks5h"];

/// The scheme assumed for a spec written without one, such as `proxy.example:8080`.
const DEFAULT_SCHEME: &str = "http";

/// What stands between the scheme and the authority of a URL.
const SCHEME_SEPARATOR: &str = "://";

/// The text before the last `@` of the authority of `url`, when there is any. It is taken from
/// the spelling the caller used, because [`Proxy`] wants the credentials as written.
fn userinfo_of(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once(SCHEME_SEPARATOR)?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let (userinfo, _) = authority.rsplit_once('@')?;
    (!userinfo.is_empty()).then_some(userinfo)
}

/// The part of an egress that may be stored, logged and serialized: where traffic goes, never how
/// to authenticate. Only this type is ever serialized.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EgressKey {
    /// `http`, `https`, `socks5` or `socks5h`.
    pub scheme: String,
    /// The host name or address, lower case, without brackets for IPv6 literals.
    pub host: String,
    /// The port, explicit or the scheme's default.
    pub port: u16,
    /// The country the egress is in, when the spec or the pool says so.
    pub country: Option<CountryCode>,
}

impl fmt::Display for EgressKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.contains(':') {
            write!(f, "{}://[{}]:{}", self.scheme, self.host, self.port)
        } else {
            write!(f, "{}://{}:{}", self.scheme, self.host, self.port)
        }
    }
}

/// A proxy the traffic leaves through. Its credentials live in the [`Proxy`] and in the spec kept
/// for launching; neither is printed or serialized, and this type does not implement `Serialize`.
#[derive(Clone)]
pub struct EgressRef {
    proxy: Proxy,
    key: EgressKey,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "read by launch_spec, which the browser launch path takes up next"
        )
    )]
    spec: Secret,
}

impl EgressRef {
    /// Parses `spec`, written as `scheme://[user:password@]host:port`, or as `host:port` for http.
    /// The credentials are taken out of the string here; nothing downstream sees them unless it
    /// asks for the proxy or the launch spec. They are passed on as written, not percent-decoded,
    /// as [`Proxy::parse`] takes them. The `socks5` schemes need a port; `http` and `https` default
    /// to 80 and 443. A spec with a path, query or fragment is refused.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError::Scheme`] for a scheme other than http, https, socks5 or socks5h, and
    /// [`EgressError::Address`] when no host or port can be read, or the spec carries more than an
    /// address. The address error never echoes the credentials.
    pub fn parse(spec: &str, country: Option<CountryCode>) -> Result<Self, EgressError> {
        let spec = spec.trim();
        let full = if spec.contains("://") {
            spec.to_owned()
        } else {
            format!("{DEFAULT_SCHEME}://{spec}")
        };
        let address = || EgressError::Address(RedactedUrl::new(&full));
        let url = url::Url::parse(&full).map_err(|_error| address())?;
        if url.cannot_be_a_base() {
            // Whatever came before the first colon was taken for a scheme; it may be a user name.
            return Err(EgressError::Address(RedactedUrl::placeholder()));
        }
        let scheme = url.scheme();
        if !SCHEMES.contains(&scheme) {
            return Err(EgressError::Scheme(scheme.to_owned()));
        }
        let has_path = !matches!(url.path(), "" | "/");
        if has_path || url.query().is_some() || url.fragment().is_some() {
            return Err(address());
        }
        let host = match url.host() {
            // `url` lower-cases the host of a special scheme (http, https) only; the host of
            // socks5 and socks5h is opaque and keeps its case.
            Some(url::Host::Domain(domain)) if !domain.is_empty() => domain.to_ascii_lowercase(),
            Some(url::Host::Ipv4(ip)) => ip.to_string(),
            Some(url::Host::Ipv6(ip)) => ip.to_string(),
            _ => return Err(address()),
        };
        let port = url.port_or_known_default().ok_or_else(address)?;
        let key = EgressKey {
            scheme: scheme.to_owned(),
            host,
            port,
            country,
        };
        // The address is rebuilt from the parsed parts, so a launch never sees the odd spelling the
        // spec was written in. The userinfo alone is kept as written.
        let address_text = key.to_string();
        let spec_text = match userinfo_of(&full) {
            Some(userinfo) => {
                let hostport = &address_text[key.scheme.len() + SCHEME_SEPARATOR.len()..];
                format!("{}{SCHEME_SEPARATOR}{userinfo}@{hostport}", key.scheme)
            }
            None => address_text,
        };
        let proxy = Proxy::parse(&spec_text).map_err(|_error| address())?;
        Ok(Self {
            proxy,
            key,
            spec: Secret::new(spec_text),
        })
    }

    /// The storable part of this egress.
    #[must_use]
    pub fn key(&self) -> &EgressKey {
        &self.key
    }

    /// The proxy for a browser context, such as `ContextOptions::proxy`.
    #[must_use]
    pub fn proxy(&self) -> &Proxy {
        &self.proxy
    }

    /// The spec a launch takes, credentials included. Crate-internal: nothing public exposes it.
    #[must_use]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "used by the browser launch path in the next work package"
        )
    )]
    pub(crate) fn launch_spec(&self) -> Secret {
        self.spec.clone()
    }
}

impl fmt::Debug for EgressRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EgressRef").field(&self.key).finish()
    }
}

impl fmt::Display for EgressRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.key)
    }
}

/// Why an egress spec was refused.
#[derive(Debug, thiserror::Error)]
pub enum EgressError {
    /// The scheme is not one a proxy may use.
    #[error("unsupported proxy scheme `{0}`; use http, https, socks5 or socks5h")]
    Scheme(String),
    /// No host or port could be read from the spec. The message holds the redacted spec.
    #[error("cannot read a host and port from {0}")]
    Address(RedactedUrl),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::humanize::Rng;

    /// Compiles only for a type that does not implement `Serialize`.
    trait AmbiguousIfSerialize<A> {
        fn check() {}
    }
    impl<T: ?Sized> AmbiguousIfSerialize<()> for T {}
    impl<T: ?Sized + Serialize> AmbiguousIfSerialize<u8> for T {}

    #[test]
    fn an_egress_cannot_be_serialized() {
        <EgressRef as AmbiguousIfSerialize<_>>::check();
        <Secret as AmbiguousIfSerialize<_>>::check();
    }

    #[test]
    fn credentials_leave_every_view_but_the_launch_spec() {
        let egress =
            EgressRef::parse("http://alice:s3cret@proxy.example:8080", None).expect("valid spec");
        let key_json = serde_json::to_string(egress.key()).expect("serializes");
        for view in [
            format!("{egress:?}"),
            format!("{egress:#?}"),
            format!("{egress}"),
            format!("{:?}", egress.proxy()),
            format!("{:?}", egress.launch_spec()),
            key_json,
        ] {
            assert!(!view.contains("s3cret"), "password in {view}");
            assert!(!view.contains("alice"), "user in {view}");
        }
        assert!(egress.proxy().has_credentials());
        assert!(egress.launch_spec().expose().contains("alice:s3cret"));
    }

    #[test]
    fn random_credentials_never_reach_a_view_in_a_seeded_run() {
        let mut rng = Rng::new(99);
        for _ in 0..64 {
            let user = format!("u{:x}", rng.next_u64());
            let pass = format!("p{:x}", rng.next_u64());
            let spec = format!("socks5h://{user}:{pass}@10.1.2.3:1080");
            let egress = EgressRef::parse(&spec, None).expect("valid spec");
            let json = serde_json::to_string(egress.key()).expect("serializes");
            let shown = format!(
                "{egress:?} {egress} {:?} {} {json}",
                egress.proxy(),
                egress.proxy()
            );
            assert!(!shown.contains(&user) && !shown.contains(&pass), "{shown}");
        }
    }

    #[test]
    fn a_bare_host_and_port_default_to_http() {
        let egress = EgressRef::parse("proxy.example:8080", None).expect("valid spec");
        assert_eq!(egress.key().scheme, "http");
        assert_eq!(egress.key().host, "proxy.example");
        assert_eq!(egress.key().port, 8080);
        assert_eq!(egress.proxy().server(), "http://proxy.example:8080");
    }

    #[test]
    fn the_http_schemes_take_their_default_ports() {
        let http = EgressRef::parse("http://proxy.example", None).expect("valid spec");
        let https = EgressRef::parse("https://proxy.example", None).expect("valid spec");
        assert_eq!((http.key().port, https.key().port), (80, 443));
    }

    #[test]
    fn socks5h_is_accepted_with_an_explicit_port() {
        let egress = EgressRef::parse("socks5h://10.0.0.1:1080", None).expect("valid spec");
        assert_eq!(egress.key().scheme, "socks5h");
        assert_eq!(egress.key().host, "10.0.0.1");
        assert_eq!(egress.key().port, 1080);
    }

    #[test]
    fn socks5_without_a_port_has_no_default_and_is_refused() {
        for spec in ["socks5://proxy.example", "socks5h://proxy.example"] {
            let err = EgressRef::parse(spec, None).expect_err("refused");
            assert!(matches!(err, EgressError::Address(_)), "{spec}");
        }
    }

    #[test]
    fn a_scheme_outside_the_allowed_four_is_refused() {
        for scheme in ["ftp", "ws", "socks4", "gopher"] {
            let err =
                EgressRef::parse(&format!("{scheme}://proxy.example:21"), None).expect_err("no");
            assert!(
                matches!(&err, EgressError::Scheme(found) if found == scheme),
                "{scheme}: {err}"
            );
        }
    }

    #[test]
    fn the_host_is_lower_cased() {
        let egress = EgressRef::parse("HTTP://Proxy.EXAMPLE:8080", None).expect("valid spec");
        assert_eq!(egress.key().host, "proxy.example");
        assert_eq!(egress.key().scheme, "http");
    }

    #[test]
    fn a_socks_host_is_lower_cased_too() {
        // `url` lower-cases only the hosts of special schemes (http, https);
        // for socks5 and socks5h the host is opaque and keeps its case.
        for scheme in ["socks5", "socks5h"] {
            let upper = EgressRef::parse(&format!("{scheme}://Proxy.EXAMPLE:1080"), None)
                .expect("valid spec");
            let lower = EgressRef::parse(&format!("{scheme}://proxy.example:1080"), None)
                .expect("valid spec");
            assert_eq!(upper.key().host, "proxy.example", "{scheme}");
            assert_eq!(upper.key(), lower.key(), "{scheme}");

            let upper = EgressRef::parse(&format!("{scheme}://al:pw@Proxy.EXAMPLE:1080"), None)
                .expect("valid spec");
            assert_eq!(
                upper.launch_spec().expose(),
                format!("{scheme}://al:pw@proxy.example:1080"),
                "{scheme}"
            );
        }
    }

    #[test]
    fn an_ipv6_host_loses_its_brackets_in_the_key_and_gets_them_back_when_shown() {
        let egress = EgressRef::parse("http://[2001:db8::1]:3128", None).expect("valid spec");
        assert_eq!(egress.key().host, "2001:db8::1");
        assert_eq!(egress.key().to_string(), "http://[2001:db8::1]:3128");
        assert_eq!(egress.proxy().server(), "http://[2001:db8::1]:3128");
        let again = EgressRef::parse("[::1]:8080", None).expect("valid spec");
        assert_eq!(again.key().host, "::1");
    }

    #[test]
    fn two_specs_for_one_address_have_one_key_whatever_their_credentials() {
        let plain = EgressRef::parse("proxy.example:8080", None).expect("valid spec");
        let a = EgressRef::parse("http://alice:one@proxy.example:8080", None).expect("valid");
        let b = EgressRef::parse("http://bob:two@proxy.example:8080", None).expect("valid");
        assert_eq!(plain.key(), a.key());
        assert_eq!(a.key(), b.key());
    }

    #[test]
    fn a_path_a_query_or_a_fragment_is_refused() {
        for spec in [
            "http://proxy.example:8080/pac",
            "http://proxy.example:8080/?x=1",
            "http://proxy.example:8080#f",
        ] {
            let err = EgressRef::parse(spec, None).expect_err("refused");
            assert!(matches!(err, EgressError::Address(_)), "{spec}");
        }
    }

    #[test]
    fn an_empty_spec_is_refused() {
        for spec in ["", "   ", "http://", "http://:8080"] {
            let err = EgressRef::parse(spec, None).expect_err("refused");
            assert!(matches!(err, EgressError::Address(_)), "{spec:?}");
        }
    }

    #[test]
    fn an_address_error_never_echoes_a_credential() {
        for spec in [
            "socks5://alice:s3cret@:1",
            "alice:s3cret@:1",
            "alice:s3cret@",
            "http://alice:s3cret@proxy.example:notaport",
            "http://alice:s3cret@proxy.example:8080/pac?token=s3cret",
            "alice:s3cret@host://x",
            "http://alice:s3cret@",
        ] {
            let err = EgressRef::parse(spec, None).expect_err("refused");
            let shown = format!("{err} {err:?}");
            assert!(
                !shown.contains("s3cret") && !shown.contains("alice"),
                "{spec}: {shown}"
            );
        }
    }

    #[test]
    fn the_country_travels_with_the_key() {
        let de = CountryCode::new("de").expect("valid");
        let egress = EgressRef::parse("proxy.example:8080", Some(de)).expect("valid spec");
        assert_eq!(egress.key().country, Some(de));
        let json = serde_json::to_string(egress.key()).expect("serializes");
        assert!(json.contains("\"country\":\"DE\""), "{json}");
        let back: EgressKey = serde_json::from_str(&json).expect("round trips");
        assert_eq!(&back, egress.key());
    }

    #[test]
    fn a_key_is_shown_without_its_country() {
        let de = CountryCode::new("de").expect("valid");
        let egress = EgressRef::parse("https://proxy.example", Some(de)).expect("valid spec");
        assert_eq!(egress.to_string(), "https://proxy.example:443");
    }
}
