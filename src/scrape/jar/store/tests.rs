//! Behaviour tests of the cookie jar, plus properties checked over generated input. The abuse
//! cases live in `abuse`, the path and domain bounds in `bounds`.

mod abuse;
mod bounds;

use super::super::set_cookie::MAX_LIFETIME_SECS;
use super::rules::is_public_suffix;
use super::*;
use crate::stealth::humanize::Rng;

const NOW: u64 = 1_700_000_000;
const SAME: SiteContext = SiteContext::SameSite { top_level: true };

fn url(text: &str) -> Url {
    Url::parse(text).unwrap_or_else(|e| panic!("bad test URL {text}: {e}"))
}

fn store_at(jar: &mut CookieJar, at: &str, header: &str, now: u64) -> StoreOutcome {
    jar.store(&url(at), header, now)
}

fn store(jar: &mut CookieJar, at: &str, header: &str) -> StoreOutcome {
    store_at(jar, at, header, NOW)
}

fn header(jar: &mut CookieJar, at: &str) -> Option<String> {
    jar.header_for(&url(at), SAME, NOW)
}

fn ignored(reason: IgnoreReason) -> StoreOutcome {
    StoreOutcome::Ignored(reason)
}

fn only(jar: &CookieJar) -> &StoredCookie {
    assert_eq!(jar.len(), 1, "{jar:?}");
    &jar.cookies()[0]
}

#[test]
fn a_host_only_cookie_goes_to_its_host_alone() {
    let mut jar = CookieJar::new();
    assert_eq!(
        store(&mut jar, "https://www.example.com/", "a=1"),
        StoreOutcome::Stored
    );
    let cookie = only(&jar);
    assert!(cookie.host_only);
    assert_eq!(
        (cookie.domain.as_str(), cookie.path.as_str()),
        ("www.example.com", "/")
    );
    assert_eq!(
        header(&mut jar, "https://www.example.com/x").as_deref(),
        Some("a=1")
    );
    assert_eq!(header(&mut jar, "https://sub.www.example.com/"), None);
    assert_eq!(header(&mut jar, "https://example.com/"), None);
}

#[test]
fn a_domain_cookie_reaches_subdomains_but_not_look_alikes() {
    let mut jar = CookieJar::new();
    let outcome = store(
        &mut jar,
        "https://www.example.com/",
        "a=1; Domain=example.com",
    );
    assert_eq!(outcome, StoreOutcome::Stored);
    assert!(!only(&jar).host_only);
    for to in [
        "https://example.com/",
        "https://a.b.example.com/",
        "https://www.example.com/",
    ] {
        assert_eq!(header(&mut jar, to).as_deref(), Some("a=1"), "{to}");
    }
    for to in [
        "https://notexample.com/",
        "https://example.com.evil.net/",
        "https://example.org/",
    ] {
        assert_eq!(header(&mut jar, to), None, "{to}");
    }
}

#[test]
fn the_default_path_comes_from_the_request() {
    let mut jar = CookieJar::new();
    assert_eq!(
        store(&mut jar, "https://example.com/app/page", "a=1"),
        StoreOutcome::Stored
    );
    assert_eq!(only(&jar).path, "/app");
    for to in ["https://example.com/app", "https://example.com/app/x/y"] {
        assert_eq!(header(&mut jar, to).as_deref(), Some("a=1"), "{to}");
    }
    for to in ["https://example.com/", "https://example.com/application"] {
        assert_eq!(header(&mut jar, to), None, "{to}");
    }
}

#[test]
fn expired_cookies_are_purged_and_deletions_remove() {
    let mut jar = CookieJar::new();
    let at = "https://example.com/";
    assert_eq!(
        store(&mut jar, at, "short=1; Max-Age=10"),
        StoreOutcome::Stored
    );
    assert_eq!(store(&mut jar, at, "keep=1"), StoreOutcome::Stored);
    let later = jar.header_for(&url(at), SAME, NOW + 10);
    assert_eq!(
        later.as_deref(),
        Some("keep=1"),
        "expired at its expiry time"
    );
    assert_eq!(jar.len(), 1, "header_for purged the expired cookie");
    assert_eq!(
        store(&mut jar, at, "keep=1; Max-Age=0"),
        StoreOutcome::Deleted
    );
    assert!(jar.is_empty());
    assert_eq!(
        store(
            &mut jar,
            at,
            "gone=1; Expires=Thu, 01 Jan 1970 00:00:01 GMT"
        ),
        StoreOutcome::Deleted
    );
    assert!(jar.is_empty(), "an expired cookie is never stored");
    store(&mut jar, at, "x=1; Max-Age=5");
    store(&mut jar, at, "y=1; Max-Age=50");
    assert_eq!(jar.remove_expired(NOW + 5), 1);
    assert_eq!(jar.remove_expired(NOW + 5), 0);
}

#[test]
fn an_overwrite_keeps_the_creation_time() {
    let mut jar = CookieJar::new();
    let at = "https://example.com/";
    assert_eq!(store_at(&mut jar, at, "a=1", NOW), StoreOutcome::Stored);
    assert_eq!(store_at(&mut jar, at, "b=2", NOW + 5), StoreOutcome::Stored);
    assert_eq!(
        store_at(&mut jar, at, "a=3; HttpOnly", NOW + 10),
        StoreOutcome::Replaced
    );
    let a = &jar.cookies()[0];
    assert_eq!(
        (a.value.as_str(), a.created, a.last_access, a.http_only),
        ("3", NOW, NOW + 10, true)
    );
    assert_eq!(header(&mut jar, at).as_deref(), Some("a=3; b=2"));
    assert_eq!(
        store(&mut jar, at, "a=4; Path=/x"),
        StoreOutcome::Stored,
        "another path is another cookie"
    );
}

#[test]
fn the_cookie_header_puts_longer_paths_first_then_older_cookies() {
    let mut jar = CookieJar::new();
    let at = "https://example.com/a/b/c";
    for (offset, header_text) in [
        (0, "root=1; Path=/"),
        (1, "deep_old=1; Path=/a/b"),
        (2, "mid=1; Path=/a"),
        (3, "deep_new=1; Path=/a/b"),
    ] {
        assert_eq!(
            store_at(&mut jar, at, header_text, NOW + offset),
            StoreOutcome::Stored
        );
    }
    let sent = header(&mut jar, at);
    assert_eq!(
        sent.as_deref(),
        Some("deep_old=1; deep_new=1; mid=1; root=1")
    );
    assert!(
        jar.cookies().iter().all(|c| c.last_access == NOW),
        "sending updates last access"
    );
}

#[test]
fn a_nameless_cookie_is_sent_as_its_value() {
    let mut jar = CookieJar::new();
    assert_eq!(
        store(&mut jar, "https://example.com/", "token"),
        StoreOutcome::Stored
    );
    assert_eq!(
        store(&mut jar, "https://example.com/", "x=1"),
        StoreOutcome::Stored
    );
    assert_eq!(
        header(&mut jar, "https://example.com/").as_deref(),
        Some("token; x=1")
    );
}

#[test]
fn same_site_follows_the_request_context() {
    let mut jar = CookieJar::new();
    let at = "https://example.com/";
    for header_text in [
        "strict=1; SameSite=Strict",
        "lax=1; SameSite=Lax",
        "default=1",
        "none=1; SameSite=None; Secure",
    ] {
        assert_eq!(store(&mut jar, at, header_text), StoreOutcome::Stored);
    }
    let mut sent = |context| jar.header_for(&url(at), context, NOW);
    let all = Some("strict=1; lax=1; default=1; none=1".to_owned());
    assert_eq!(sent(SiteContext::SameSite { top_level: false }), all);
    let navigation = sent(SiteContext::CrossSite {
        top_level_get: true,
    });
    assert_eq!(navigation.as_deref(), Some("lax=1; default=1; none=1"));
    let subresource = sent(SiteContext::CrossSite {
        top_level_get: false,
    });
    assert_eq!(subresource.as_deref(), Some("none=1"));
}

#[test]
fn urls_that_cannot_carry_cookies_are_refused() {
    let mut jar = CookieJar::new();
    for at in [
        "ftp://example.com/",
        "file:///tmp/x",
        "data:text/plain,x",
        "chrome://settings/",
    ] {
        assert_eq!(
            store(&mut jar, at, "a=1"),
            ignored(IgnoreReason::UnsupportedUrl),
            "{at}"
        );
        assert_eq!(header(&mut jar, at), None);
    }
}

/// Stores `count` cookies called `{tag}{i}` on `host`, one second apart from `start`.
fn fill(jar: &mut CookieJar, host: &str, tag: &str, count: usize, start: u64) {
    for (i, at) in (start..).take(count).enumerate() {
        let outcome = store_at(jar, &format!("https://{host}/"), &format!("{tag}{i}=1"), at);
        assert_eq!(outcome, StoreOutcome::Stored);
    }
}

#[test]
fn the_per_site_bound_evicts_insecure_least_recently_used_cookies_first() {
    let mut jar = CookieJar::new();
    store(&mut jar, "https://example.com/", "guard=1; Secure");
    fill(
        &mut jar,
        "www.example.com",
        "c",
        MAX_COOKIES_PER_SITE - 1,
        NOW + 1,
    );
    fill(&mut jar, "other.org", "o", 3, NOW);
    let at = NOW + 2_000;
    assert_eq!(
        store_at(&mut jar, "https://www.example.com/", "c0=again", at),
        StoreOutcome::Replaced
    );
    assert_eq!(
        store_at(&mut jar, "https://example.com/", "new=1", at),
        StoreOutcome::Stored
    );
    let names: Vec<&str> = jar.cookies().iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| !n.starts_with('o')).count(),
        MAX_COOKIES_PER_SITE
    );
    assert!(
        names.contains(&"guard"),
        "a secure cookie outlives older insecure ones"
    );
    assert!(names.contains(&"c0"), "a recently written cookie stays");
    assert!(
        !names.contains(&"c1"),
        "the least recently accessed insecure cookie goes"
    );
    assert!(names.contains(&"new") && names.iter().filter(|n| n.starts_with('o')).count() == 3);
}

#[test]
fn the_total_bound_holds_and_never_evicts_the_newest() {
    let mut jar = CookieJar::new();
    let mut stored = 0;
    let mut start = NOW;
    for site in 0.. {
        let count = MAX_COOKIES_PER_SITE.min(MAX_COOKIES + 1 - stored);
        if count == 0 {
            break;
        }
        fill(&mut jar, &format!("s{site}.example"), "k", count, start);
        stored += count;
        start += 1_000;
    }
    assert_eq!(jar.len(), MAX_COOKIES);
    let newest = jar.cookies().last().expect("not empty");
    assert_eq!(
        (newest.domain.as_str(), newest.name.as_str()),
        ("s16.example", "k120")
    );
    let oldest_gone = !jar
        .cookies()
        .iter()
        .any(|c| c.domain == "s0.example" && c.name == "k0");
    assert!(
        oldest_gone,
        "the least recently accessed cookie of all was evicted"
    );
}

#[test]
fn cdp_cookies_round_trip() {
    let mut jar = CookieJar::new();
    let at = "https://www.example.com/app/x";
    store(
        &mut jar,
        at,
        "host=1; Max-Age=60; HttpOnly; SameSite=Strict",
    );
    store(
        &mut jar,
        at,
        "dom=2; Domain=example.com; Path=/; Secure; SameSite=None",
    );
    let cdp = jar.to_cdp(NOW);
    assert_eq!(cdp[0].domain, "www.example.com");
    assert_eq!(cdp[0].expires, Some(1_700_000_060.0));
    assert_eq!(cdp[1].domain, ".example.com");
    assert_eq!(cdp[1].expires, None);
    let back = CookieJar::from_cdp(&cdp, NOW);
    assert_eq!(back, jar, "created and last access are NOW on both sides");
    assert!(
        jar.to_cdp(NOW + 60).iter().all(|c| c.name != "host"),
        "expired cookies are left out"
    );
}

#[test]
fn from_cdp_leaves_out_unsafe_cookies() {
    let good = Cookie::new("ok", "1").domain("example.com");
    let bad = [
        Cookie::new("a", "x; injected=1").domain("example.com"),
        Cookie::new("a=b", "1").domain("example.com"),
        Cookie::new("a", "line\r\nbreak").domain("example.com"),
        Cookie::new("a", "1").domain(".com"),
        Cookie::new("a", "1").domain(".co.uk"),
        Cookie::new("a", "1").domain(".10.0.0.1"),
        Cookie::new("a", "1"),
        Cookie::new("a", "1").domain("example.com").path("relative"),
        Cookie::new("__Host-a", "1")
            .domain(".example.com")
            .secure(true),
        Cookie::new("__Secure-a", "1").domain("example.com"),
        Cookie::new("a", "1")
            .domain("example.com")
            .same_site(SameSite::None),
        Cookie::new("a", "1")
            .domain("example.com")
            .expires(f64::NAN),
        Cookie::new("a", "1").domain("example.com").expires(1.0),
        Cookie::new("a", "v".repeat(4096)).domain("example.com"),
        Cookie::new("", "").domain("example.com"),
        Cookie::new("", "__Host-x")
            .domain("example.com")
            .secure(true),
    ];
    for cookie in &bad {
        assert!(
            CookieJar::from_cdp(std::slice::from_ref(cookie), NOW).is_empty(),
            "{cookie:?}"
        );
    }
    let mut all = bad.to_vec();
    all.push(good);
    let jar = CookieJar::from_cdp(&all, NOW);
    assert_eq!(jar.len(), 1);
    let far = Cookie::new("far", "1").domain("example.com").expires(1e300);
    let capped = CookieJar::from_cdp(&[far], NOW);
    assert_eq!(only(&capped).expires, Some(NOW + MAX_LIFETIME_SECS));
    let unrooted = Cookie::new("p", "1").domain("EXAMPLE.com").path("");
    let rooted = CookieJar::from_cdp(&[unrooted], NOW);
    assert_eq!(
        (only(&rooted).path.as_str(), only(&rooted).domain.as_str()),
        ("/", "example.com"),
        "an empty path is the root and the domain is lowered"
    );
}

/// A valid host-only cookie called `name` on `host`, for building jar files by hand.
fn stored(name: &str, host: &str) -> StoredCookie {
    StoredCookie {
        name: name.to_owned(),
        value: "1".to_owned(),
        domain: host.to_owned(),
        host_only: true,
        path: "/".to_owned(),
        expires: None,
        secure: false,
        http_only: false,
        same_site: None,
        created: NOW,
        last_access: NOW,
    }
}

#[test]
fn a_jar_file_beyond_the_bounds_is_refused() {
    let one_site: Vec<StoredCookie> = (0..=MAX_COOKIES_PER_SITE)
        .map(|i| stored(&format!("c{i}"), "example.com"))
        .collect();
    let error = CookieJar::try_from(JarFile { cookies: one_site }).expect_err("site full");
    assert_eq!(
        error,
        JarError::SiteFull {
            index: MAX_COOKIES_PER_SITE
        }
    );
    let everywhere: Vec<StoredCookie> = (0..=MAX_COOKIES)
        .map(|i| stored("c", &format!("s{i}.example")))
        .collect();
    let error = CookieJar::try_from(JarFile {
        cookies: everywhere,
    })
    .expect_err("too many");
    assert_eq!(
        error,
        JarError::TooMany {
            count: MAX_COOKIES + 1
        }
    );
    assert!(error.to_string().contains("3001"), "{error}");
    let fits: Vec<StoredCookie> = (0..MAX_COOKIES_PER_SITE)
        .map(|i| stored(&format!("c{i}"), "example.com"))
        .collect();
    assert!(CookieJar::try_from(JarFile { cookies: fits }).is_ok());
}

#[test]
fn a_jar_survives_serde_and_refuses_a_tampered_file() {
    let mut jar = CookieJar::new();
    store(
        &mut jar,
        "https://www.example.com/",
        "a=1; Domain=example.com; Max-Age=60",
    );
    store(&mut jar, "https://www.example.com/", "b=2");
    let json = serde_json::to_string(&jar).expect("serializes");
    let back: CookieJar = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back, jar);
    for (from, to) in [
        ("\"value\":\"2\"", "\"value\":\"2\\r\\nX: y\""),
        ("\"domain\":\"example.com\"", "\"domain\":\"com\""),
        ("\"name\":\"b\"", "\"name\":\"__Host-b\""),
        (
            "\"domain\":\"www.example.com\"",
            "\"domain\":\"WWW.example.com\"",
        ),
        (
            "\"path\":\"/\",\"expires\":null",
            "\"path\":\"\",\"expires\":null",
        ),
    ] {
        let tampered = json.replacen(from, to, 1);
        assert_ne!(tampered, json, "the fixture must change: {from}");
        assert!(
            serde_json::from_str::<CookieJar>(&tampered).is_err(),
            "{to}"
        );
    }
    let doubled = json
        .replacen("\"name\":\"b\"", "\"name\":\"a\"", 1)
        .replacen("\"host_only\":true", "\"host_only\":false", 1)
        .replacen(
            "\"domain\":\"www.example.com\"",
            "\"domain\":\"example.com\"",
            1,
        );
    let error = serde_json::from_str::<CookieJar>(&doubled).expect_err("duplicate");
    assert!(error.to_string().contains("repeats"), "{error}");
}

#[test]
fn debug_prints_counts_and_never_a_value() {
    let mut jar = CookieJar::new();
    store(&mut jar, "https://example.com/", "sid=s3cret-value");
    store(&mut jar, "https://example.org/", "sid=s3cret-other");
    let shown = format!("{jar:?} {:?}", jar.cookies());
    assert!(!shown.contains("s3cret"), "{shown}");
    assert!(format!("{jar:?}").contains("cookies: 2, sites: 2"));
}

#[test]
fn expiry_of_reports_the_latest_persistent_expiry_on_a_site() {
    let mut jar = CookieJar::new();
    store(
        &mut jar,
        "https://www.example.com/",
        "clear=1; Max-Age=3600; Domain=example.com",
    );
    store(&mut jar, "https://www.example.com/", "clear=2; Max-Age=60");
    store(&mut jar, "https://www.example.com/", "session=1");
    assert_eq!(jar.expiry_of("clear", "example.com"), Some(NOW + 3600));
    assert_eq!(
        jar.expiry_of("clear", "deep.www.example.com"),
        Some(NOW + 3600)
    );
    assert_eq!(jar.expiry_of("clear", "example.org"), None);
    assert_eq!(jar.expiry_of("session", "example.com"), None);
    assert_eq!(jar.expiry_of("missing", "example.com"), None);
}

/// A random request URL and `Set-Cookie` header built from pieces that stress the rules.
fn random_exchange(rng: &mut Rng) -> (String, String) {
    const SCHEMES: [&str; 3] = ["https", "http", "wss"];
    const HOSTS: [&str; 7] = [
        "www.example.com",
        "example.com",
        "a.b.example.co.uk",
        "user.github.io",
        "10.0.0.1",
        "[::1]",
        "localhost",
    ];
    const DOMAINS: [&str; 10] = [
        "",
        "; Domain=example.com",
        "; Domain=com",
        "; Domain=co.uk",
        "; Domain=github.io",
        "; Domain=10.0.0.1",
        "; Domain=.EXAMPLE.com",
        "; Domain=example.co.uk",
        "; Domain=0.0.1",
        "; Domain=x%2e.com",
    ];
    const EXTRAS: [&str; 10] = [
        "",
        "; Secure",
        "; Path=/",
        "; Path=/a",
        "; Max-Age=0",
        "; Max-Age=5",
        "; SameSite=None",
        "; HttpOnly",
        "; Expires=Wed, 21 Oct 2037 07:28:00 GMT",
        "; Path=/../x",
    ];
    const NAMES: [&str; 6] = ["a", "b", "__Host-h", "__Secure-s", "", "a\u{1}"];
    let host = rng.pick(&HOSTS);
    let at = format!("{}://{host}/a/b", rng.pick(&SCHEMES));
    let name = rng.pick(&NAMES);
    let pair = if name.is_empty() {
        "v".to_owned()
    } else {
        format!("{name}=v")
    };
    let header_text = format!(
        "{pair}{}{}{}",
        rng.pick(&DOMAINS),
        rng.pick(&EXTRAS),
        rng.pick(&EXTRAS)
    );
    (at, header_text)
}

/// The invariants a jar holds after any sequence of stores once `header_for` has run at `now`,
/// which purges expired cookies (purging is lazy: an ignored header leaves the jar untouched).
fn check_invariants(jar: &CookieJar, now: u64) {
    for cookie in jar.cookies() {
        assert_eq!(validate(cookie), Ok(()), "{cookie:?}");
        assert!(!cookie.is_expired(now), "{cookie:?}");
        assert!(
            cookie.host_only || !is_public_suffix(&cookie.domain),
            "{cookie:?}"
        );
        if let Some(at) = cookie.expires {
            assert!(at <= now + MAX_LIFETIME_SECS, "{cookie:?}");
        }
    }
    assert!(jar.len() <= MAX_COOKIES);
}

#[test]
fn generated_exchanges_keep_every_invariant() {
    let mut rng = Rng::new(0x00C0_0C1E);
    let mut jar = CookieJar::new();
    for step in 0..20_000_u64 {
        let now = NOW + step / 10;
        let (at, header_text) = random_exchange(&mut rng);
        let origin_host = url(&at).host_str().expect("host").to_owned();
        let outcome = jar.store(&url(&at), &header_text, now);
        if outcome == StoreOutcome::Stored {
            // A new cookie is pushed last and is never the one evicted.
            let latest = jar.cookies().last().expect("stored");
            let reachable = if latest.host_only {
                origin_host == latest.domain
            } else {
                domain_match(&origin_host, &latest.domain)
            };
            assert!(reachable, "{at} {header_text:?} -> {latest:?}");
        }
        if let Some(sent) = jar.header_for(&url(&at), SAME, now) {
            assert!(!sent.contains(['\r', '\n', '\0']), "{sent:?}");
        }
        check_invariants(&jar, now);
    }
    let json = serde_json::to_string(&jar).expect("serializes");
    assert_eq!(
        serde_json::from_str::<CookieJar>(&json).expect("deserializes"),
        jar
    );
}
