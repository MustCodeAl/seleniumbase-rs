//! Bounds on the length of a cookie path and domain, whether the cookie comes from a header, from
//! CDP or from a file.

use super::*;

/// A host name of exactly `len` bytes made of labels of at most 63 bytes, under `.example`.
fn host_of(len: usize) -> String {
    const LABEL: usize = 63;
    let mut host = String::new();
    while host.len() + 1 + LABEL + ".example".len() < len {
        host.push_str(&"a".repeat(LABEL));
        host.push('.');
    }
    let rest = len - host.len() - ".example".len();
    host.push_str(&"b".repeat(rest));
    host.push_str(".example");
    assert_eq!(host.len(), len);
    host
}

#[test]
fn a_huge_request_path_cannot_fill_the_jar_through_default_paths() {
    let mut jar = CookieJar::new();
    let at = format!("https://example.com/{}x", "a/".repeat(200_000));
    for i in 0..MAX_COOKIES_PER_SITE {
        let outcome = store(&mut jar, &at, &format!("c{i}=1"));
        assert_eq!(outcome, ignored(IgnoreReason::TooLarge), "cookie {i}");
    }
    assert!(jar.is_empty());
}

#[test]
fn a_default_path_at_the_bound_is_kept_and_one_byte_more_is_refused() {
    let mut jar = CookieJar::new();
    let fits = format!("/{}", "a".repeat(MAX_PATH_BYTES - 1));
    let at = format!("https://example.com{fits}/x");
    assert_eq!(store(&mut jar, &at, "a=1"), StoreOutcome::Stored);
    assert_eq!(only(&jar).path, fits);
    let too_long = format!("/{}", "a".repeat(MAX_PATH_BYTES));
    let at = format!("https://example.com{too_long}/x");
    assert_eq!(store(&mut jar, &at, "b=1"), ignored(IgnoreReason::TooLarge));
    assert_eq!(jar.len(), 1);
}

#[test]
fn a_path_attribute_at_the_bound_is_kept_and_a_longer_one_falls_back_to_the_default() {
    let mut jar = CookieJar::new();
    let at = "https://example.com/app/page";
    let fits = format!("/{}", "p".repeat(MAX_PATH_BYTES - 1));
    assert_eq!(
        store(&mut jar, at, &format!("a=1; Path={fits}")),
        StoreOutcome::Stored
    );
    let longer = format!("/{}", "p".repeat(MAX_PATH_BYTES));
    assert_eq!(
        store(&mut jar, at, &format!("b=1; Path={longer}")),
        StoreOutcome::Stored
    );
    assert_eq!(jar.cookies()[0].path, fits);
    assert_eq!(jar.cookies()[1].path, "/app");
}

#[test]
fn a_host_name_over_the_dns_bound_is_refused() {
    let mut jar = CookieJar::new();
    let fits = host_of(MAX_DOMAIN_BYTES);
    assert_eq!(
        store(&mut jar, &format!("https://{fits}/"), "a=1"),
        StoreOutcome::Stored
    );
    let too_long = host_of(MAX_DOMAIN_BYTES + 1);
    assert_eq!(
        store(&mut jar, &format!("https://{too_long}/"), "a=1"),
        ignored(IgnoreReason::TooLarge)
    );
    assert_eq!(jar.len(), 1);
}

#[test]
fn a_jar_file_with_an_oversize_path_or_domain_is_refused() {
    let load = |cookie: StoredCookie| {
        CookieJar::try_from(JarFile {
            cookies: vec![cookie],
        })
    };
    let too_large = Err(JarError::Invalid {
        index: 0,
        reason: IgnoreReason::TooLarge,
    });
    let mut cookie = stored("a", "example.com");
    cookie.path = format!("/{}", "p".repeat(MAX_PATH_BYTES - 1));
    assert!(load(cookie.clone()).is_ok());
    cookie.path.push('p');
    assert_eq!(load(cookie), too_large);
    assert!(load(stored("a", &host_of(MAX_DOMAIN_BYTES))).is_ok());
    assert_eq!(load(stored("a", &host_of(MAX_DOMAIN_BYTES + 1))), too_large);
}

#[test]
fn from_cdp_leaves_out_an_oversize_path_or_domain() {
    let fits = Cookie::new("a", "1")
        .domain("example.com")
        .path(format!("/{}", "p".repeat(MAX_PATH_BYTES - 1)));
    assert_eq!(CookieJar::from_cdp(&[fits], NOW).len(), 1);
    let long_path = Cookie::new("a", "1")
        .domain("example.com")
        .path(format!("/{}", "p".repeat(MAX_PATH_BYTES)));
    let long_domain = Cookie::new("a", "1").domain(format!(".{}", host_of(MAX_DOMAIN_BYTES + 1)));
    assert!(CookieJar::from_cdp(&[long_path, long_domain], NOW).is_empty());
}
