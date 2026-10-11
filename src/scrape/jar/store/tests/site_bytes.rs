//! The bytes of cookies one site may hold: a hostile site cannot fill the jar by sending many
//! large cookies, nor by growing the ones it has, and an edited file cannot hold more either.

use super::*;
use crate::scrape::jar::set_cookie::MAX_NAME_VALUE_BYTES;

/// A cookie value that, with a short name, takes nearly the whole name and value bound.
fn big_value() -> String {
    "v".repeat(MAX_NAME_VALUE_BYTES - 8)
}

fn held_by_jar(jar: &CookieJar) -> usize {
    jar.cookies().iter().map(StoredCookie::size_bytes).sum()
}

#[test]
fn the_bytes_a_site_may_hold_are_bounded_and_the_newest_cookie_stays() {
    let mut jar = CookieJar::new();
    let value = big_value();
    for (i, at) in (NOW..).take(MAX_COOKIES_PER_SITE).enumerate() {
        let header = format!("c{i:03}={value}");
        let outcome = store_at(&mut jar, "https://example.com/", &header, at);
        assert_eq!(outcome, StoreOutcome::Stored);
        assert!(held_by_jar(&jar) <= MAX_BYTES_PER_SITE, "after cookie {i}");
    }
    assert!(jar.len() < MAX_COOKIES_PER_SITE, "{jar:?}");
    assert_eq!(jar.cookies().last().expect("not empty").name, "c179");
    assert_ne!(jar.cookies()[0].name, "c000", "the oldest were evicted");
}

#[test]
fn growing_a_cookie_by_replacing_it_cannot_pass_the_site_bound() {
    let mut jar = CookieJar::new();
    fill(&mut jar, "example.com", "c", MAX_COOKIES_PER_SITE, NOW);
    let value = big_value();
    for i in 0..MAX_COOKIES_PER_SITE {
        let at = NOW + 1_000 + u64::try_from(i).expect("small");
        let header = format!("c{i}={value}");
        let outcome = store_at(&mut jar, "https://example.com/", &header, at);
        assert!(
            matches!(outcome, StoreOutcome::Replaced | StoreOutcome::Stored),
            "{outcome:?}"
        );
        assert!(held_by_jar(&jar) <= MAX_BYTES_PER_SITE, "after cookie {i}");
        let grown = jar.cookies().iter().find(|c| c.name == format!("c{i}"));
        assert!(grown.is_some_and(|c| c.value == value), "c{i} stays");
    }
}

#[test]
fn another_site_does_not_count_against_the_bytes_of_this_one() {
    let mut jar = CookieJar::new();
    let value = big_value();
    for i in 0..40 {
        store(&mut jar, "https://a.example/", &format!("c{i:03}={value}"));
        store(&mut jar, "https://b.example/", &format!("c{i:03}={value}"));
    }
    assert_eq!(jar.len(), 80, "40 cookies of 4 KiB are 160 KiB for each");
}

#[test]
fn a_jar_file_over_the_bytes_of_a_site_is_refused() {
    let heavy = |count: usize| -> Vec<StoredCookie> {
        (0..count)
            .map(|i| {
                let mut cookie = stored(&format!("c{i}"), "example.com");
                cookie.value = big_value();
                cookie
            })
            .collect()
    };
    assert!(CookieJar::try_from(JarFile { cookies: heavy(50) }).is_ok());
    let error = CookieJar::try_from(JarFile { cookies: heavy(80) }).expect_err("too heavy");
    assert!(
        matches!(error, JarError::SiteBytes { index } if index < 80),
        "{error}"
    );
}
