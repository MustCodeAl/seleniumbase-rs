//! The wreq client of a session: the measured TLS and HTTP/2, redirects off, the egress as the
//! proxy, and a private trust root when the options name one.

use base64::Engine as _;
use wreq::header::HeaderValue;
use wreq::redirect::Policy;
use wreq::{NoProxy, Proxy};

use super::error::HttpError;
use super::options::HttpOptions;
use crate::impersonate::client::{impersonation_client, ImpersonationProfile};
use crate::scrape::egress::route::EgressRef;

/// The notation of the `no_proxy` list for a host that skips the proxy.
fn no_proxy_list(bypass: &[String]) -> String {
    bypass
        .iter()
        .map(|host| host.trim())
        .filter(|host| !host.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// The scheme wreq must use for an egress. A `socks5` proxy resolves host names in the browser's
/// way, at the proxy: wreq's `socks5` resolves them here and leaks the lookup, so it gets the
/// `socks5h` scheme.
fn proxy_scheme(scheme: &str) -> &str {
    if scheme == "socks5" {
        "socks5h"
    } else {
        scheme
    }
}

/// The `Proxy-Authorization` value of a user name and password, marked sensitive.
fn basic_auth(user: &str, password: &str) -> Result<HeaderValue, HttpError> {
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
    let mut value =
        HeaderValue::from_str(&format!("Basic {token}")).map_err(|_error| HttpError::Client {
            kind: "the proxy credentials were refused",
        })?;
    value.set_sensitive(true);
    Ok(value)
}

/// The wreq proxy for an egress.
///
/// The credentials of an HTTP or HTTPS proxy travel in a sensitive `Proxy-Authorization` header
/// and never in the proxy URI, which the HTTP client prints in its trace logs. A SOCKS proxy takes
/// them in the URI, because its handshake has no header; its URI is never logged by this crate.
///
/// # Errors
///
/// Returns [`HttpError::Client`] when wreq refuses the proxy. The reason never names the proxy.
pub(super) fn wreq_proxy(egress: &EgressRef, bypass: &[String]) -> Result<Proxy, HttpError> {
    let key = egress.key();
    let host = if key.host.contains(':') {
        format!("[{}]", key.host)
    } else {
        key.host.clone()
    };
    let address = format!("{}://{host}:{}", proxy_scheme(&key.scheme), key.port);
    let mut proxy = Proxy::all(address.as_str()).map_err(|_error| HttpError::Client {
        kind: "the proxy address was refused",
    })?;
    if let Some((user, password)) = egress.proxy().credentials() {
        proxy = if key.scheme.starts_with("socks") {
            proxy.basic_auth(user, password)
        } else {
            proxy.custom_http_auth(basic_auth(user, password)?)
        };
    }
    Ok(proxy.no_proxy(NoProxy::from_string(&no_proxy_list(bypass))))
}

/// Builds the client of a session.
///
/// Certificate verification stays on. Redirects are off, because the session follows them itself
/// to treat cookies and headers per hop. The environment's proxy settings are never read.
///
/// # Errors
///
/// Returns [`HttpError::Impersonate`] when the profile cannot be sent, and [`HttpError::Client`]
/// when the proxy or the client is refused.
pub(super) fn build_client(
    profile: &ImpersonationProfile,
    egress: Option<&EgressRef>,
    options: &HttpOptions,
) -> Result<wreq::Client, HttpError> {
    let mut builder = impersonation_client(profile)?
        .redirect(Policy::none())
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .zstd(true)
        .connect_timeout(options.connect_timeout.min(options.timeout));
    builder = match egress {
        Some(egress) => builder.proxy(wreq_proxy(egress, &options.bypass)?),
        None => builder.no_proxy(),
    };
    if let Some(store) = &options.trust {
        builder = builder.tls_cert_store(store.clone());
    }
    for (host, addr) in &options.resolve {
        builder = builder.resolve(host.clone(), *addr);
    }
    builder.build().map_err(|_error| HttpError::Client {
        kind: "the client was refused",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::BrowserKind;

    #[test]
    fn the_bypass_list_is_comma_separated_without_blanks() {
        let list = no_proxy_list(&[" a.test ".to_owned(), String::new(), ".b.test".to_owned()]);
        assert_eq!(list, "a.test,.b.test");
        assert_eq!(no_proxy_list(&[]), "");
    }

    #[test]
    fn socks5_resolves_at_the_proxy() {
        assert_eq!(proxy_scheme("socks5"), "socks5h");
        assert_eq!(proxy_scheme("socks5h"), "socks5h");
        assert_eq!(proxy_scheme("http"), "http");
        assert_eq!(proxy_scheme("https"), "https");
    }

    #[test]
    fn every_scheme_an_egress_may_have_makes_a_proxy() {
        for spec in [
            "http://alice:s3cret@proxy.example:8080",
            "https://alice:s3cret@proxy.example:8443",
            "socks5://alice:s3cret@10.0.0.1:1080",
            "socks5h://alice:s3cret@10.0.0.1:1080",
            "http://[2001:db8::1]:3128",
        ] {
            let egress = EgressRef::parse(spec, None).expect("a spec");
            assert!(
                wreq_proxy(&egress, &["skip.test".to_owned()]).is_ok(),
                "{spec}"
            );
        }
    }

    #[test]
    fn an_http_proxys_credentials_stay_out_of_its_uri_so_trace_logs_cannot_print_them() {
        for spec in [
            "http://alice:s3cret@proxy.example:8080",
            "https://alice:s3cret@proxy.example:8443",
        ] {
            let egress = EgressRef::parse(spec, None).expect("a spec");
            let proxy = wreq_proxy(&egress, &[]).expect(spec);
            let shown = format!("{proxy:?}");
            assert!(
                !shown.contains("s3cret") && !shown.contains("alice"),
                "{spec}: {shown}"
            );
        }
    }

    #[test]
    fn the_proxy_authorization_is_the_basic_scheme_and_sensitive() {
        let value = basic_auth("alice", "s3cret").expect("a value");
        assert_eq!(value.to_str().expect("ascii"), "Basic YWxpY2U6czNjcmV0");
        assert!(value.is_sensitive());
        assert!(!format!("{value:?}").contains("YWxpY2U"));
    }

    #[test]
    fn a_client_is_built_for_the_measured_profile_with_and_without_a_proxy() {
        let profile = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
        let options = HttpOptions::new();
        assert!(build_client(&profile, None, &options).is_ok());
        let egress =
            EgressRef::parse("http://alice:s3cret@proxy.example:8080", None).expect("spec");
        assert!(build_client(&profile, Some(&egress), &options).is_ok());
    }
}
