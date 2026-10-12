//! Unit tests of the proxy's own rules: options, the token, the credentials check, targets,
//! the header rewrite, the Chrome switches and the launch line.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use http::{HeaderMap, HeaderName, HeaderValue, Uri};

use super::connect::ProxyCredentials;
use super::forward::{
    claimed_chrome_major, forwarded_request_headers, forwarded_response_headers, Origin,
};
use super::upstream::{egress_proxy, websocket_spec};
use super::{
    chrome_args_for, hex, random_token, MitmError, MitmOptions, MitmProxy,
    DEFAULT_MAX_WEBSOCKET_BYTES, DEFAULT_WEBSOCKET_IDLE_TIMEOUT, DISABLE_QUIC_ARG, MAX_CONNECTIONS,
    MAX_HEADERS, MAX_HEADER_BYTES, MIN_HEADER_BYTES, PRIME_HOST, PROXY_EVERYTHING_ARG, PROXY_USER,
};
use crate::impersonate::client::ImpersonationProfile;
use crate::impersonate::tls_server::EphemeralCa;
use crate::sb_cdp::LaunchOptions;
use crate::scrape::egress::redact::Secret;
use crate::scrape::egress::route::EgressRef;
use crate::scrape::ledger::events::NoopEventSink;
use crate::wire::{emulation, BrowserKind};

/// A header map in the order given.
fn headers(list: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in list {
        map.append(
            HeaderName::from_bytes(name.as_bytes()).expect("a header name"),
            HeaderValue::from_str(value).expect("a header value"),
        );
    }
    map
}

/// The names of a header list, in order.
fn names(list: &[(HeaderName, HeaderValue)]) -> Vec<&str> {
    list.iter().map(|(name, _)| name.as_str()).collect()
}

fn basic(user: &str, password: &str) -> String {
    format!("Basic {}", STANDARD.encode(format!("{user}:{password}")))
}

#[test]
fn default_options_are_valid() {
    MitmOptions::default()
        .validate()
        .expect("the defaults hold");
}

#[test]
fn options_outside_their_bounds_are_refused() {
    let cases = [
        MitmOptions::default().max_connections(0),
        MitmOptions::default().max_connections(MAX_CONNECTIONS + 1),
        MitmOptions::default().max_header_bytes(MIN_HEADER_BYTES - 1),
        MitmOptions::default().max_header_bytes(MAX_HEADER_BYTES + 1),
        MitmOptions::default().max_headers(0),
        MitmOptions::default().max_headers(MAX_HEADERS + 1),
        MitmOptions::default().max_body_bytes(0),
        MitmOptions::default().idle_timeout(Duration::ZERO),
        MitmOptions::default().handshake_timeout(Duration::ZERO),
        MitmOptions::default().connect_timeout(Duration::ZERO),
        MitmOptions::default().read_timeout(Duration::ZERO),
        MitmOptions::default().websocket_idle_timeout(Duration::ZERO),
        MitmOptions::default().max_websocket_bytes(0),
        MitmOptions::default().resolve(" ", "127.0.0.1:1".parse().expect("an address")),
    ];
    for options in cases {
        assert!(
            matches!(options.validate(), Err(MitmError::InvalidOptions(_))),
            "{options:?}"
        );
    }
}

#[test]
fn the_websocket_bounds_have_defaults_and_show_in_debug() {
    let shown = format!("{:?}", MitmOptions::default());
    assert!(
        shown.contains(&format!(
            "websocket_idle_timeout: {DEFAULT_WEBSOCKET_IDLE_TIMEOUT:?}"
        )),
        "{shown}"
    );
    assert!(
        shown.contains(&format!(
            "max_websocket_bytes: {DEFAULT_MAX_WEBSOCKET_BYTES}"
        )),
        "{shown}"
    );
    MitmOptions::default()
        .websocket_idle_timeout(Duration::from_millis(1))
        .max_websocket_bytes(1)
        .validate()
        .expect("the smallest bounds hold");
}

#[test]
fn the_websocket_hello_is_the_measured_one_without_its_gaps_and_offers_http1_only() {
    let profile = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
    let spec = websocket_spec(&profile).expect("a WebSocket hello is measured");
    let entry = emulation::get(profile.kind(), profile.os()).expect("the entry");
    let measured = entry.tls_websocket.as_ref().expect("measured");
    assert_eq!(
        spec,
        measured.without_known_gaps(&entry.provenance.known_gaps)
    );
    assert_eq!(spec.alpn, ["http/1.1".to_owned()]);
    assert_ne!(
        spec.ja4(),
        profile.tls_spec().ja4(),
        "the WebSocket hello is not the ordinary one"
    );
}

#[test]
fn options_debug_counts_roots_and_never_prints_a_key() {
    let ca = Arc::new(EphemeralCa::generate().expect("a CA"));
    let options = MitmOptions::default()
        .trust_upstream_root(vec![0x30, 0x82, 0x01])
        .ca(Arc::clone(&ca));
    let shown = format!("{options:?}");
    assert!(shown.contains("upstream_roots: 1"), "{shown}");
    assert!(shown.contains("<redacted>"), "{shown}");
}

#[test]
fn the_token_is_128_random_bits_in_hex() {
    let first = random_token().expect("a token");
    let second = random_token().expect("a token");
    assert_eq!(first.expose().len(), 32);
    assert!(first.expose().bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(first.expose(), second.expose());
    assert_eq!(format!("{first:?}"), "<redacted>");
}

#[test]
fn hex_is_lower_case_and_two_digits_per_byte() {
    assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
}

#[test]
fn credentials_accept_only_the_exact_basic_value() {
    let token = Secret::new("0123456789abcdef0123456789abcdef");
    let credentials = ProxyCredentials::new(PROXY_USER, &token);
    let good = basic(PROXY_USER, token.expose());
    assert!(credentials.accepts(&headers(&[("proxy-authorization", &good)])));

    let refused = [
        String::new(),
        basic(PROXY_USER, "0123456789abcdef0123456789abcdee"),
        basic("other", token.expose()),
        good.replacen("Basic", "basic", 1),
        format!("{good} "),
        format!("Bearer {}", token.expose()),
    ];
    for value in refused {
        let map = if value.is_empty() {
            HeaderMap::new()
        } else {
            headers(&[("proxy-authorization", &value)])
        };
        assert!(!credentials.accepts(&map), "accepted {value:?}");
    }
    assert!(
        !credentials.accepts(&headers(&[("authorization", &good)])),
        "only Proxy-Authorization counts"
    );
}

#[test]
fn connect_targets_parse_names_addresses_and_ports() {
    let cases = [
        ("example.test:443", "https://example.test/"),
        ("EXAMPLE.test:8443", "https://example.test:8443/"),
        ("127.0.0.1:443", "https://127.0.0.1/"),
        ("[::1]:9443", "https://[::1]:9443/"),
    ];
    for (target, url) in cases {
        let uri: Uri = target.parse().expect("an authority-form target");
        let (origin, _host) = Origin::from_connect(&uri).expect(target);
        assert_eq!(origin.url("/"), url, "{target}");
    }
}

#[test]
fn connect_targets_without_a_port_or_with_port_zero_are_refused() {
    for target in ["example.test", "example.test:0", "http://example.test:443/"] {
        let uri: Uri = target.parse().expect("a URI");
        assert!(
            matches!(Origin::from_connect(&uri), Err(MitmError::BadTarget(_))),
            "{target}"
        );
    }
}

#[test]
fn connect_targets_with_user_information_are_refused() {
    let uri: Uri = "user@example.test:443"
        .parse()
        .expect("hyper accepts userinfo");
    assert!(matches!(
        Origin::from_connect(&uri),
        Err(MitmError::BadTarget(_))
    ));
}

#[test]
fn absolute_form_takes_http_only() {
    let plain: Uri = "http://plain.test:8080/a?b=1".parse().expect("a URI");
    let origin = Origin::from_absolute(&plain).expect("http is taken");
    assert_eq!(origin.url("/a?b=1"), "http://plain.test:8080/a?b=1");
    assert_eq!(origin.key(), "http://plain.test:8080");
    for refused in ["https://secure.test/", "/relative"] {
        let uri: Uri = refused.parse().expect("a URI");
        assert!(
            matches!(Origin::from_absolute(&uri), Err(MitmError::BadTarget(_))),
            "{refused}"
        );
    }
}

#[test]
fn the_host_header_must_name_the_origin() {
    let uri: Uri = "origin.test:443".parse().expect("a target");
    let (origin, _host) = Origin::from_connect(&uri).expect("an origin");
    for good in ["origin.test", "ORIGIN.test", "origin.test:443"] {
        assert!(
            origin.accepts_host_header(&HeaderValue::from_static(good)),
            "{good}"
        );
    }
    for bad in [
        "other.test",
        "origin.test:8443",
        "user@origin.test",
        "origin.test:0",
        "",
    ] {
        assert!(
            !origin.accepts_host_header(&HeaderValue::from_static(bad)),
            "{bad}"
        );
    }
}

#[test]
fn hop_by_hop_proxy_and_listed_headers_are_dropped_and_the_order_kept() {
    let request = headers(&[
        ("host", "origin.test"),
        ("connection", "keep-alive, X-Hop"),
        ("sec-ch-ua", "\"Chromium\";v=\"155\""),
        ("x-hop", "1"),
        ("proxy-authorization", "Basic c2I6dG9rZW4="),
        ("user-agent", "Mozilla/5.0"),
        ("proxy-connection", "keep-alive"),
        ("keep-alive", "timeout=5"),
        ("te", "gzip"),
        ("trailer", "x-end"),
        ("transfer-encoding", "chunked"),
        ("upgrade", "websocket"),
        ("accept", "*/*"),
        ("cookie", "a=1"),
    ]);
    let kept = forwarded_request_headers(&request);
    assert_eq!(
        names(&kept),
        ["sec-ch-ua", "user-agent", "accept", "cookie"]
    );
}

#[test]
fn te_trailers_is_the_one_te_that_goes_upstream() {
    let kept = forwarded_request_headers(&headers(&[("te", "Trailers"), ("accept", "*/*")]));
    assert_eq!(names(&kept), ["te", "accept"]);
}

#[test]
fn response_hop_by_hop_headers_are_dropped() {
    let response = headers(&[
        ("content-type", "text/html"),
        ("connection", "close, x-private"),
        ("x-private", "1"),
        ("keep-alive", "timeout=5"),
        ("proxy-authenticate", "Basic"),
        ("content-encoding", "br"),
        ("content-length", "10"),
    ]);
    let kept = forwarded_response_headers(&response);
    let names: Vec<&str> = kept.keys().map(HeaderName::as_str).collect();
    assert_eq!(
        names,
        ["content-type", "content-encoding", "content-length"]
    );
}

#[test]
fn a_length_that_came_with_chunking_is_dropped_both_ways() {
    let smuggling = headers(&[
        ("content-length", "4"),
        ("transfer-encoding", "chunked"),
        ("accept", "*/*"),
    ]);
    assert_eq!(names(&forwarded_request_headers(&smuggling)), ["accept"]);
    let kept = forwarded_response_headers(&smuggling);
    assert_eq!(
        kept.keys().map(HeaderName::as_str).collect::<Vec<_>>(),
        ["accept"]
    );

    let plain = headers(&[("content-length", "4"), ("accept", "*/*")]);
    assert_eq!(
        names(&forwarded_request_headers(&plain)),
        ["content-length", "accept"]
    );
}

#[test]
fn the_claimed_chrome_major_is_read_from_the_user_agent() {
    let agent = |value: &str| headers(&[("user-agent", value)]);
    let chrome = "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 Chrome/155.0.0.0 Safari/537.36";
    assert_eq!(claimed_chrome_major(&agent(chrome)), Some(155));
    assert_eq!(
        claimed_chrome_major(&agent("Mozilla/5.0 HeadlessChrome/154.0.1.2")),
        Some(154)
    );
    assert_eq!(claimed_chrome_major(&agent("curl/8.7.1")), None);
    assert_eq!(claimed_chrome_major(&agent("Chrome/")), None);
    assert_eq!(claimed_chrome_major(&HeaderMap::new()), None);
}

#[test]
fn chrome_args_pin_the_leaf_only_and_cover_loopback_and_quic() {
    let ca = EphemeralCa::generate().expect("a CA");
    let args = chrome_args_for(&ca);
    let [ca_pin, leaf_pin] = ca.spki_pins();
    assert_eq!(
        args,
        [
            format!("--ignore-certificate-errors-spki-list={leaf_pin}"),
            PROXY_EVERYTHING_ARG.to_owned(),
            DISABLE_QUIC_ARG.to_owned(),
        ]
    );
    assert!(args.iter().all(|arg| !arg.contains(&ca_pin)));
    assert!(!args.iter().any(|arg| arg == "--ignore-certificate-errors"));
}

#[tokio::test]
async fn the_launch_line_has_the_proxy_and_switches_but_never_the_token() {
    let profile = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
    let mitm = MitmProxy::start(
        profile,
        None,
        MitmOptions::default(),
        Arc::new(NoopEventSink),
    )
    .await
    .expect("starts");
    let spec = mitm.proxy_spec();
    let token = spec
        .expose()
        .rsplit_once('@')
        .and_then(|(userinfo, _)| userinfo.rsplit_once(':'))
        .map(|(_, token)| token.to_owned())
        .expect("a token in the spec");
    let options = mitm
        .configure_launch(LaunchOptions::builder().headless(true))
        .build()
        .expect("valid launch options");
    let args = options.browser_args(Path::new("/tmp/sb-mitm-unit"));

    let server = format!("--proxy-server=http://127.0.0.1:{}", mitm.port());
    assert!(args.contains(&server), "{args:?}");
    for wanted in mitm.chrome_args() {
        assert!(args.contains(&wanted), "{wanted} missing from {args:?}");
    }
    assert!(
        args.iter().all(|arg| !arg.contains(&token)),
        "the token is on the command line"
    );
    assert!(mitm.local_addr().ip().is_loopback());
    for shown in [
        format!("{mitm:?}"),
        format!("{:?}", mitm.browser_proxy()),
        format!("{spec:?}"),
        format!("{spec}"),
    ] {
        assert!(!shown.contains(&token), "token in {shown}");
    }
    mitm.shutdown(Duration::from_secs(5)).await;
}

/// A proxy with the measured profile and the default options.
async fn started() -> MitmProxy {
    let profile = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
    MitmProxy::start(
        profile,
        None,
        MitmOptions::default(),
        Arc::new(NoopEventSink),
    )
    .await
    .expect("starts")
}

#[tokio::test]
async fn the_priming_page_is_on_the_priming_host_under_a_random_path_without_the_token() {
    let (mitm, other) = (started().await, started().await);
    let url = mitm.prime_url();
    let token = mitm
        .proxy_spec()
        .expose()
        .rsplit_once('@')
        .and_then(|(userinfo, _)| userinfo.rsplit_once(':'))
        .map(|(_, token)| token.to_owned())
        .expect("a token in the spec");

    let path = url
        .strip_prefix(&format!("https://{PRIME_HOST}/"))
        .expect("https on the priming host");
    assert_eq!(path.len(), 32, "{url}");
    assert!(
        path.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{url}"
    );
    assert!(!url.contains(&token), "the token is in the priming URL");
    assert_ne!(url, other.prime_url(), "two proxies share a priming path");
    assert!(
        !format!("{mitm:?}").contains(path),
        "Debug shows the priming path"
    );
    mitm.shutdown(Duration::from_secs(5)).await;
    other.shutdown(Duration::from_secs(5)).await;
}

#[tokio::test]
async fn the_launch_opens_the_priming_page_unless_it_has_a_start_page_already() {
    let mitm = started().await;

    let primed = mitm
        .configure_launch(LaunchOptions::builder())
        .build()
        .expect("valid launch options");
    let kept = mitm
        .configure_launch(LaunchOptions::builder().url("https://example.com/"))
        .build()
        .expect("valid launch options");
    let replaced = mitm
        .configure_launch(LaunchOptions::builder())
        .url("https://example.org/")
        .build()
        .expect("valid launch options");

    assert_eq!(primed.url.as_deref(), Some(mitm.prime_url().as_str()));
    assert_eq!(kept.url.as_deref(), Some("https://example.com/"));
    assert_eq!(replaced.url.as_deref(), Some("https://example.org/"));
    mitm.shutdown(Duration::from_secs(5)).await;
}

#[tokio::test]
async fn invalid_options_never_bind() {
    let profile = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
    let started = MitmProxy::start(
        profile,
        None,
        MitmOptions::default().max_connections(0),
        Arc::new(NoopEventSink),
    )
    .await;
    assert!(matches!(started, Err(MitmError::InvalidOptions(_))));
}

#[test]
fn a_socks5_egress_is_given_to_the_client_as_socks5h_so_names_resolve_at_the_proxy() {
    for spec in [
        "socks5://alice:s3cret@10.0.0.1:1080",
        "socks5h://alice:s3cret@10.0.0.1:1080",
        "socks5://10.0.0.1:1080",
    ] {
        let egress = EgressRef::parse(spec, None).expect(spec);
        let shown = format!("{:?}", egress_proxy(&egress).expect(spec));
        assert!(shown.contains("socks5h://"), "{spec}: {shown}");
        assert!(!shown.contains("socks5://"), "{spec}: {shown}");
    }
}

#[test]
fn an_http_egress_keeps_its_credentials_out_of_the_proxy_address() {
    // The client prints the proxy address in its trace logs.
    for spec in [
        "http://alice:s3cret@proxy.example:8080",
        "https://alice:s3cret@proxy.example:8443",
    ] {
        let egress = EgressRef::parse(spec, None).expect(spec);
        let shown = format!("{:?}", egress_proxy(&egress).expect(spec));
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{spec}: {shown}"
        );
    }
}

#[test]
fn every_scheme_an_egress_may_have_makes_a_proxy() {
    for spec in [
        "http://alice:s3cret@proxy.example:8080",
        "https://proxy.example:8443",
        "socks5://alice:s3cret@10.0.0.1:1080",
        "socks5h://10.0.0.1:1080",
        "http://[2001:db8::1]:3128",
        "socks5://[2001:db8::1]:1080",
    ] {
        let egress = EgressRef::parse(spec, None).expect(spec);
        assert!(egress_proxy(&egress).is_ok(), "{spec}");
    }
}
