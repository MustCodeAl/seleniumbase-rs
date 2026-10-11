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

/// The part of an egress that may be stored, logged and serialized: where traffic goes, never how
/// to authenticate. Only this type is ever serialized.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EgressKey {
    /// `http`, `https`, `socks5` or `socks5h`.
    pub scheme: String,
    /// The host name or address, without brackets for IPv6 literals.
    pub host: String,
    /// The port, explicit or the scheme's default.
    pub port: u16,
    /// The country the egress is in, when the spec or the pool says so.
    pub country: Option<CountryCode>,
}

impl fmt::Display for EgressKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}:{}", self.scheme, self.host, self.port)
    }
}

/// A proxy the traffic leaves through. Its credentials live in the [`Proxy`] and in the spec kept
/// for launching; neither is printed or serialized.
#[derive(Clone)]
pub struct EgressRef {
    proxy: Proxy,
    key: EgressKey,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the browser launch path (WP13)")
    )]
    spec: Secret,
}

impl EgressRef {
    /// Parses `spec`, written as `scheme://[user:password@]host:port`, or as `host:port` for http.
    /// The credentials are taken out of the string here; nothing downstream sees them unless it
    /// asks for the proxy or the launch spec.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError::Scheme`] for a scheme other than http, https, socks5 or socks5h, and
    /// [`EgressError::Address`] when no host or port can be read. The address error never echoes
    /// the credentials.
    pub fn parse(spec: &str, country: Option<CountryCode>) -> Result<Self, EgressError> {
        let address = || EgressError::Address(RedactedUrl::new(spec));
        let full = if spec.contains("://") {
            spec.to_owned()
        } else {
            format!("{DEFAULT_SCHEME}://{spec}")
        };
        let url = url::Url::parse(&full).map_err(|_error| address())?;
        let scheme = url.scheme();
        if !SCHEMES.contains(&scheme) {
            return Err(EgressError::Scheme(scheme.to_owned()));
        }
        let host = url
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or_else(address)?;
        let port = url.port_or_known_default().ok_or_else(address)?;
        let proxy = Proxy::parse(&full).map_err(|_error| address())?;
        Ok(Self {
            proxy,
            key: EgressKey {
                scheme: scheme.to_owned(),
                host: host.to_owned(),
                port,
                country,
            },
            spec: Secret::new(full),
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
        expect(dead_code, reason = "used by the browser launch path (WP13)")
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

    #[test]
    fn credentials_leave_every_view_but_the_launch_spec() {
        let egress =
            EgressRef::parse("http://alice:s3cret@proxy.example:8080", None).expect("valid spec");
        let key_json = serde_json::to_string(egress.key()).unwrap_or_default();
        for view in [
            format!("{egress:?}"),
            format!("{egress}"),
            format!("{:?}", egress.proxy()),
            key_json,
        ] {
            assert!(!view.contains("s3cret"), "password in {view}");
            assert!(!view.contains("alice"), "user in {view}");
        }
        assert!(egress.proxy().has_credentials());
        assert!(egress.launch_spec().expose().contains("alice:s3cret"));
    }

    #[test]
    fn a_bare_host_and_port_default_to_http() {
        let egress = EgressRef::parse("proxy.example:8080", None).expect("valid spec");
        assert_eq!(egress.key().scheme, "http");
        assert_eq!(egress.key().host, "proxy.example");
        assert_eq!(egress.key().port, 8080);
    }

    #[test]
    fn https_takes_its_known_default_port() {
        let egress = EgressRef::parse("https://proxy.example", None).expect("valid spec");
        assert_eq!(egress.key().port, 443);
    }

    #[test]
    fn socks5h_is_accepted_with_an_explicit_port() {
        let egress = EgressRef::parse("socks5h://10.0.0.1:1080", None).expect("valid spec");
        assert_eq!(egress.key().scheme, "socks5h");
        assert_eq!(egress.key().host, "10.0.0.1");
    }

    #[test]
    fn a_scheme_outside_the_allowed_four_is_refused() {
        let err = EgressRef::parse("ftp://proxy.example:21", None).expect_err("refused");
        assert!(matches!(err, EgressError::Scheme(ref scheme) if scheme == "ftp"));
    }

    #[test]
    fn socks5_without_a_port_has_no_default_and_is_refused() {
        let err = EgressRef::parse("socks5://proxy.example", None).expect_err("refused");
        assert!(matches!(err, EgressError::Address(_)));
    }

    #[test]
    fn an_address_error_does_not_echo_the_password() {
        let err = EgressRef::parse("socks5://alice:s3cret@:1", None).expect_err("refused");
        assert!(!err.to_string().contains("s3cret"), "{err}");
    }

    #[test]
    fn the_country_travels_with_the_key() {
        let de = CountryCode::new("de").expect("valid");
        let egress = EgressRef::parse("proxy.example:8080", Some(de)).expect("valid spec");
        assert_eq!(egress.key().country, Some(de));
    }
}
