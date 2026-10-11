//! The times a stored cookie may carry: a loaded file is held to what `store` and `header_for`
//! guarantee, and a clock that steps back does not make the jar unloadable.

use super::*;

fn load(cookie: StoredCookie) -> Result<CookieJar, JarError> {
    CookieJar::try_from(JarFile {
        cookies: vec![cookie],
    })
}

#[test]
fn a_jar_file_cannot_outlive_the_longest_lifetime() {
    let mut cookie = stored("a", "example.com");
    cookie.expires = Some(NOW + MAX_LIFETIME_SECS);
    assert!(load(cookie.clone()).is_ok(), "the cap itself is allowed");
    cookie.expires = Some(NOW + MAX_LIFETIME_SECS + 1);
    assert_eq!(load(cookie), Err(JarError::Lifetime { index: 0 }));
    let mut cookie = stored("a", "example.com");
    cookie.expires = Some(u64::MAX);
    assert_eq!(load(cookie), Err(JarError::Lifetime { index: 0 }));
}

#[test]
fn a_jar_file_cannot_carry_times_no_clock_produced() {
    let mut cookie = stored("a", "example.com");
    cookie.created = NOW + 1;
    assert_eq!(load(cookie), Err(JarError::Clock { index: 0 }));
    let mut cookie = stored("a", "example.com");
    cookie.last_access = MAX_TIMESTAMP_SECS;
    assert!(load(cookie.clone()).is_ok(), "the last second of 9999");
    cookie.last_access = MAX_TIMESTAMP_SECS + 1;
    assert_eq!(load(cookie), Err(JarError::Clock { index: 0 }));
    let mut cookie = stored("a", "example.com");
    cookie.last_access = u64::MAX;
    assert_eq!(load(cookie), Err(JarError::Clock { index: 0 }));
}

#[test]
fn a_clock_that_steps_back_leaves_a_jar_that_still_loads() {
    let mut jar = CookieJar::new();
    let at = url("https://example.com/");
    jar.store(&at, "a=1; Max-Age=999999999", NOW + 10);
    jar.store(&at, "b=1; Max-Age=999999999", NOW + 10);
    assert!(jar.header_for(&at, SAME, NOW + 5).is_some());
    assert!(
        jar.cookies().iter().all(|c| c.last_access == NOW + 10),
        "sending never lowers the last access"
    );
    let outcome = jar.store(&at, "b=2; Max-Age=999999999", NOW + 3);
    assert_eq!(outcome, StoreOutcome::Replaced);
    let json = serde_json::to_string(&jar).expect("serializes");
    let back: CookieJar = serde_json::from_str(&json).expect("a jar the library wrote loads");
    assert_eq!(back, jar);
    assert!(
        jar.cookies().iter().all(|c| c.created <= c.last_access),
        "{jar:?}"
    );
}
