//! Conversion between [`StoredCookie`] and the CDP [`Cookie`] that `Network.getAllCookies` and
//! `Network.setCookies` carry.

use super::super::set_cookie::MAX_LIFETIME_SECS;
use super::rules::{canonical_domain, validate};
use super::{IgnoreReason, StoredCookie, ROOT_PATH};
use crate::sb_cdp::Cookie;

/// The marker CDP puts before the domain of a domain cookie.
const DOMAIN_COOKIE_DOT: char = '.';

/// The largest whole number of seconds an `f64` holds exactly: 2^53.
const MAX_EXACT_SECONDS: f64 = 9_007_199_254_740_992.0;

/// The CDP form of `cookie`: a domain cookie's domain gets a leading dot, a session cookie has no
/// expiry.
pub(super) fn to_cdp(cookie: &StoredCookie) -> Cookie {
    let domain = if cookie.host_only {
        cookie.domain.clone()
    } else {
        format!("{DOMAIN_COOKIE_DOT}{}", cookie.domain)
    };
    Cookie {
        name: cookie.name.clone(),
        value: cookie.value.clone(),
        domain,
        path: cookie.path.clone(),
        expires: cookie.expires.map(seconds_to_f64),
        http_only: cookie.http_only,
        secure: cookie.secure,
        same_site: cookie.same_site,
    }
}

/// The stored form of a CDP cookie, created and last accessed at `now`. The domain is lowered
/// and must be a valid host; an empty path becomes `/`; the expiry is capped at
/// [`MAX_LIFETIME_SECS`] after `now`. The cookie must then pass the checks every stored cookie
/// passes.
pub(super) fn from_cdp(cookie: &Cookie, now: u64) -> Result<StoredCookie, IgnoreReason> {
    let (bare, host_only) = match cookie.domain.strip_prefix(DOMAIN_COOKIE_DOT) {
        Some(rest) => (rest, false),
        None => (cookie.domain.as_str(), true),
    };
    let domain = canonical_domain(&bare.to_ascii_lowercase()).ok_or(IgnoreReason::Malformed)?;
    let expires = match cookie.expires {
        Some(seconds) => {
            let at = seconds_from_f64(seconds).ok_or(IgnoreReason::Malformed)?;
            Some(at.min(now.saturating_add(MAX_LIFETIME_SECS)))
        }
        None => None,
    };
    let path = if cookie.path.is_empty() {
        ROOT_PATH.to_owned()
    } else {
        cookie.path.clone()
    };
    let stored = StoredCookie {
        name: cookie.name.clone(),
        value: cookie.value.clone(),
        domain,
        host_only,
        path,
        expires,
        secure: cookie.secure,
        http_only: cookie.http_only,
        same_site: cookie.same_site,
        created: now,
        last_access: now,
    };
    validate(&stored)?;
    Ok(stored)
}

#[allow(
    clippy::cast_precision_loss,
    reason = "expiries are at most 400 days after a time a clock reads (a loaded jar is held to the same, see MAX_TIMESTAMP_SECS), far below 2^53 seconds, which f64 holds exactly"
)]
fn seconds_to_f64(seconds: u64) -> f64 {
    seconds as f64
}

/// Whole seconds from a CDP expiry: the fraction is dropped and a time before the epoch is the
/// epoch. `None` for NaN or an infinity.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is finite and clamped to 0..=2^53 first, where the cast is exact"
)]
fn seconds_from_f64(seconds: f64) -> Option<u64> {
    seconds
        .is_finite()
        .then(|| seconds.clamp(0.0, MAX_EXACT_SECONDS).floor() as u64)
}
