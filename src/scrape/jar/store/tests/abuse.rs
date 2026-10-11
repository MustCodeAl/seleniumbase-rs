//! Abuse cases: every header the jar must refuse, and the scope tricks it must not fall for.

use super::*;

#[test]
fn a_supercookie_on_a_public_suffix_is_refused() {
    let mut jar = CookieJar::new();
    for (at, domain) in [
        ("https://www.example.com/", "com"),
        ("https://www.example.co.uk/", "co.uk"),
        ("https://www.example.co.uk/", ".CO.UK"),
        ("https://user.github.io/", "github.io"),
    ] {
        let outcome = store(&mut jar, at, &format!("a=1; Domain={domain}"));
        assert_eq!(
            outcome,
            ignored(IgnoreReason::PublicSuffix),
            "{at} {domain}"
        );
    }
    assert!(jar.is_empty());
    assert_eq!(
        store(&mut jar, "https://github.io/", "a=1; Domain=github.io"),
        StoreOutcome::Stored
    );
    assert!(
        only(&jar).host_only,
        "a suffix naming the host itself is host-only"
    );
    assert_eq!(header(&mut jar, "https://user.github.io/"), None);
}

#[test]
fn a_domain_the_host_does_not_match_is_refused() {
    let mut jar = CookieJar::new();
    for (at, domain) in [
        ("https://www.example.com/", "other.com"),
        ("https://example.com/", "www.example.com"),
        ("https://example.com/", "ample.com"),
        ("https://www.example.com/", "example.com.evil.net"),
    ] {
        let outcome = store(&mut jar, at, &format!("a=1; Domain={domain}"));
        assert_eq!(
            outcome,
            ignored(IgnoreReason::DomainMismatch),
            "{at} {domain}"
        );
    }
    assert!(jar.is_empty());
}

#[test]
fn ip_hosts_take_only_host_only_cookies() {
    let mut jar = CookieJar::new();
    let at = "http://192.168.0.1/";
    assert_eq!(
        store(&mut jar, at, "a=1; Domain=192.168.0.1"),
        StoreOutcome::Stored
    );
    assert!(only(&jar).host_only);
    for domain in ["168.0.1", "0.1", "example.com"] {
        let outcome = store(&mut jar, at, &format!("b=1; Domain={domain}"));
        assert_eq!(outcome, ignored(IgnoreReason::IpLiteralDomain), "{domain}");
    }
    let outcome = store(
        &mut jar,
        "https://example.com/",
        "c=1; Domain=93.184.216.34",
    );
    assert_eq!(outcome, ignored(IgnoreReason::IpLiteralDomain));
    assert_eq!(
        store(&mut jar, "http://127.0.0.1/", "d=1; Domain=127.1"),
        StoreOutcome::Stored
    );
    assert_eq!(
        store(&mut jar, "https://[::1]/", "e=1; Domain=[::0:1]"),
        StoreOutcome::Stored
    );
    assert_eq!(header(&mut jar, "https://[::1]/").as_deref(), Some("e=1"));
    assert_eq!(
        header(&mut jar, "http://127.0.0.1/").as_deref(),
        Some("d=1")
    );
    assert!(jar.cookies().iter().all(|c| c.host_only));
}

#[test]
fn hosts_and_domains_in_upper_case_are_lowered() {
    let mut jar = CookieJar::new();
    let outcome = store(
        &mut jar,
        "https://WWW.Example.COM/",
        "a=1; Domain=EXAMPLE.com",
    );
    assert_eq!(outcome, StoreOutcome::Stored);
    assert_eq!(only(&jar).domain, "example.com");
    assert_eq!(
        header(&mut jar, "https://Sub.EXAMPLE.Com/").as_deref(),
        Some("a=1")
    );
    assert_eq!(
        jar.expiry_of("a", "EXAMPLE.COM"),
        None,
        "a session cookie has no expiry"
    );
}

#[test]
fn a_domain_attribute_that_is_no_host_is_malformed() {
    let mut jar = CookieJar::new();
    for domain in [
        "exämple.com",
        "ex%61mple.com",
        "a..example.com",
        "exa mple.com",
        "a/b.com",
    ] {
        let outcome = store(
            &mut jar,
            "https://www.example.com/",
            &format!("a=1; Domain={domain}"),
        );
        assert_eq!(outcome, ignored(IgnoreReason::Malformed), "{domain}");
    }
    assert!(jar.is_empty());
}

#[test]
fn control_characters_refuse_the_cookie() {
    let mut jar = CookieJar::new();
    for header_text in [
        "a=b\u{1}",
        "a=b\r\nX-Injected: 1",
        "a\0=b",
        "a=b; Path=/\u{7f}",
    ] {
        let outcome = store(&mut jar, "https://example.com/", header_text);
        assert_eq!(outcome, ignored(IgnoreReason::Malformed), "{header_text:?}");
    }
    assert!(jar.is_empty());
}

#[test]
fn an_oversize_cookie_is_refused() {
    let mut jar = CookieJar::new();
    let big = format!("n={}", "v".repeat(4096));
    assert_eq!(
        store(&mut jar, "https://example.com/", &big),
        ignored(IgnoreReason::TooLarge)
    );
    let fits = format!("n={}", "v".repeat(4095));
    assert_eq!(
        store(&mut jar, "https://example.com/", &fits),
        StoreOutcome::Stored
    );
}

#[test]
fn a_secure_cookie_needs_a_secure_origin() {
    let mut jar = CookieJar::new();
    let outcome = store(&mut jar, "http://example.com/", "a=1; Secure");
    assert_eq!(outcome, ignored(IgnoreReason::SecureFromInsecure));
    assert_eq!(
        store(&mut jar, "https://example.com/", "a=1; Secure"),
        StoreOutcome::Stored
    );
    assert_eq!(
        header(&mut jar, "http://example.com/"),
        None,
        "not sent in the clear"
    );
    assert_eq!(
        header(&mut jar, "wss://example.com/").as_deref(),
        Some("a=1")
    );
    for loopback in ["http://localhost/", "http://127.0.0.1/", "http://[::1]/"] {
        let outcome = store(&mut jar, loopback, "s=1; Secure");
        assert_eq!(
            outcome,
            StoreOutcome::Stored,
            "loopback is trustworthy: {loopback}"
        );
    }
}

#[test]
fn an_insecure_origin_cannot_overwrite_or_shadow_a_secure_cookie() {
    let mut jar = CookieJar::new();
    let secure = "sid=good; Secure; Domain=example.com; Path=/";
    assert_eq!(
        store(&mut jar, "https://www.example.com/", secure),
        StoreOutcome::Stored
    );
    for attack in [
        "sid=evil",
        "sid=evil; Path=/deeper",
        "sid=evil; Domain=example.com",
    ] {
        let outcome = store(&mut jar, "http://www.example.com/", attack);
        assert_eq!(
            outcome,
            ignored(IgnoreReason::SecureFromInsecure),
            "{attack}"
        );
    }
    assert_eq!(
        store(&mut jar, "http://www.example.com/", "other=1"),
        StoreOutcome::Stored
    );
    let renewed = "sid=new; Secure; Domain=example.com; Path=/";
    assert_eq!(
        store(&mut jar, "https://www.example.com/", renewed),
        StoreOutcome::Replaced
    );
    assert_eq!(
        header(&mut jar, "https://www.example.com/").as_deref(),
        Some("sid=new; other=1")
    );
}

#[test]
fn the_secure_prefix_needs_secure() {
    let mut jar = CookieJar::new();
    let at = "https://example.com/";
    assert_eq!(
        store(&mut jar, at, "__Secure-a=1"),
        ignored(IgnoreReason::SecurePrefix)
    );
    assert_eq!(
        store(&mut jar, at, "__SECURE-a=1"),
        ignored(IgnoreReason::SecurePrefix)
    );
    assert_eq!(
        store(&mut jar, at, "__Secure-a=1; Secure"),
        StoreOutcome::Stored
    );
    let http = store(&mut jar, "http://example.com/", "__Secure-b=1; Secure");
    assert_eq!(http, ignored(IgnoreReason::SecureFromInsecure));
    assert_eq!(
        store(&mut jar, at, "=__Secure-c=1"),
        ignored(IgnoreReason::SecurePrefix)
    );
}

#[test]
fn the_host_prefix_needs_secure_root_path_and_no_domain() {
    let mut jar = CookieJar::new();
    let at = "https://www.example.com/app/page";
    for broken in [
        "__Host-a=1; Path=/",
        "__Host-a=1; Secure",
        "__Host-a=1; Secure; Path=/app",
        "__Host-a=1; Secure; Path=/; Domain=www.example.com",
        "__host-a=1; Secure; Path=/; Domain=example.com",
        "=__Host-a=1; Secure; Path=/",
    ] {
        assert_eq!(
            store(&mut jar, at, broken),
            ignored(IgnoreReason::HostPrefix),
            "{broken}"
        );
    }
    assert_eq!(
        store(&mut jar, at, "__Host-a=1; Secure; Path=/"),
        StoreOutcome::Stored
    );
    assert!(only(&jar).host_only);
}

#[test]
fn same_site_none_needs_secure() {
    let mut jar = CookieJar::new();
    let outcome = store(&mut jar, "https://example.com/", "a=1; SameSite=None");
    assert_eq!(outcome, ignored(IgnoreReason::SameSiteNoneInsecure));
    let outcome = store(
        &mut jar,
        "https://example.com/",
        "a=1; SameSite=None; Secure",
    );
    assert_eq!(outcome, StoreOutcome::Stored);
}

#[test]
fn path_traversal_in_path_never_widens_the_scope() {
    let mut jar = CookieJar::new();
    let at = "https://example.com/app/page";
    for header_text in [
        "a=1; Path=/../admin",
        "b=1; Path=/app/../",
        "c=1; Path=/app/%2e%2e/",
    ] {
        assert_eq!(
            store(&mut jar, at, header_text),
            StoreOutcome::Stored,
            "{header_text}"
        );
    }
    // URLs resolve dot segments before matching, so these literal paths match no request.
    for to in [
        "https://example.com/",
        "https://example.com/admin",
        "https://example.com/app/x",
        "https://example.com/app/%2e%2e/x",
    ] {
        assert_eq!(header(&mut jar, to), None, "{to}");
    }
    assert_eq!(store(&mut jar, at, "d=1; Path=admin"), StoreOutcome::Stored);
    assert_eq!(
        jar.cookies()[3].path,
        "/app",
        "a relative path falls back to the default"
    );
}
