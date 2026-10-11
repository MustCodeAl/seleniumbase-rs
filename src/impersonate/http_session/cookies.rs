//! The cookies of a session: the RFC 6265 jar, and what changed in it since the browser last saw it.
//!
//! The jar is `scrape::jar::store::CookieJar`; this module wraps it for use from many tasks and
//! remembers which cookies a response set or removed, so that
//! [`HttpSession::sync_cookies_back`](super::HttpSession::sync_cookies_back) writes only those to
//! the browser and leaves what the page changed alone. A cookie value never appears in `Debug`.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Mutex;

use serde_json::{json, Value};
use url::Url;
use wreq::Method;

use crate::sb_cdp::sync::locked;
use crate::sb_cdp::SameSite;
use crate::scrape::jar::store::{CookieJar, SiteContext, StoreOutcome, StoredCookie};
use crate::wire::header_spec::RequestShape;

/// The most changes remembered for the next sync. Past it the oldest is forgotten.
pub(super) const MAX_PENDING_CHANGES: usize = 4096;
/// The most `Set-Cookie` headers of one response that are read.
pub(super) const MAX_SET_COOKIES_PER_RESPONSE: usize = 128;

/// Which cookie, without its value.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct CookieKey {
    name: String,
    domain: String,
    host_only: bool,
    path: String,
}

impl CookieKey {
    fn of(cookie: &StoredCookie) -> Self {
        Self {
            name: cookie.name.clone(),
            domain: cookie.domain.clone(),
            host_only: cookie.host_only,
            path: cookie.path.clone(),
        }
    }

    fn matches(&self, cookie: &StoredCookie) -> bool {
        self.name == cookie.name
            && self.domain == cookie.domain
            && self.host_only == cookie.host_only
            && self.path == cookie.path
    }

    /// The domain as the browser spells it: a leading dot for a domain cookie.
    fn browser_domain(&self) -> String {
        if self.host_only {
            self.domain.clone()
        } else {
            format!(".{}", self.domain)
        }
    }

    /// The parameters of `Network.deleteCookies` for this cookie.
    pub(super) fn delete_params(&self) -> Value {
        json!({
            "name": self.name,
            "domain": self.browser_domain(),
            "path": self.path,
        })
    }
}

impl fmt::Debug for CookieKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieKey")
            .field("name", &self.name)
            .field("domain", &self.domain)
            .field("host_only", &self.host_only)
            .field("path", &self.path)
            .finish()
    }
}

/// What a response did to the jar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Absorbed {
    /// Headers that stored or replaced a cookie.
    pub(super) stored: usize,
    /// Headers that removed a cookie.
    pub(super) deleted: usize,
    /// Headers the jar refused: a public suffix, another host, a bad prefix, a bound.
    pub(super) ignored: usize,
}

/// The changes since the last sync.
#[derive(Default)]
pub(super) struct Pending {
    /// Cookies to write, with their current values.
    pub(super) cookies: Vec<StoredCookie>,
    /// Cookies to remove.
    pub(super) gone: Vec<CookieKey>,
}

impl fmt::Debug for Pending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pending")
            .field("cookies", &self.cookies.len())
            .field("gone", &self.gone.len())
            .finish()
    }
}

#[derive(Default)]
struct State {
    jar: CookieJar,
    set: Vec<CookieKey>,
    gone: Vec<CookieKey>,
}

/// A jar shared by the requests of a session.
pub(super) struct SessionJar {
    state: Mutex<State>,
}

impl fmt::Debug for SessionJar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = locked(&self.state);
        f.debug_struct("SessionJar")
            .field("cookies", &state.jar.len())
            .field("pending", &(state.set.len() + state.gone.len()))
            .finish()
    }
}

/// Which cookies a request may carry, by where it comes from.
///
/// A request none of whose redirects changed site is same-site. Once a redirect has crossed to
/// another site the chain is cross-site: `Strict` cookies stay home, and `Lax` and default
/// cookies travel only on a navigation by a safe method.
pub(super) fn context_for(shape: RequestShape, method: &Method, crossed_site: bool) -> SiteContext {
    let navigation = shape == RequestShape::Document;
    if crossed_site {
        SiteContext::CrossSite {
            top_level_get: navigation && (method == Method::GET || method == Method::HEAD),
        }
    } else {
        SiteContext::SameSite {
            top_level: navigation,
        }
    }
}

impl SessionJar {
    pub(super) fn new(jar: CookieJar) -> Self {
        Self {
            state: Mutex::new(State {
                jar,
                ..State::default()
            }),
        }
    }

    /// The `Cookie` header value for a request to `url`.
    pub(super) fn header_for(&self, url: &Url, context: SiteContext, now: u64) -> Option<String> {
        locked(&self.state).jar.header_for(url, context, now)
    }

    /// Stores the cookies of the `Set-Cookie` headers of a response from `url`, and notes what
    /// changed in the jar.
    pub(super) fn absorb(&self, url: &Url, lines: &[&str], now: u64) -> Absorbed {
        let mut absorbed = Absorbed::default();
        if lines.is_empty() {
            return absorbed;
        }
        let mut state = locked(&self.state);
        let before = state.jar.cookies().to_vec();
        for line in lines.iter().take(MAX_SET_COOKIES_PER_RESPONSE) {
            match state.jar.store(url, line, now) {
                StoreOutcome::Stored | StoreOutcome::Replaced => absorbed.stored += 1,
                StoreOutcome::Deleted => absorbed.deleted += 1,
                StoreOutcome::Ignored(_) => absorbed.ignored += 1,
            }
        }
        let after = state.jar.cookies().to_vec();
        note_changes(&mut state, &before, &after);
        absorbed
    }

    /// A copy of the jar.
    pub(super) fn snapshot(&self) -> CookieJar {
        locked(&self.state).jar.clone()
    }

    /// How many cookies the jar holds.
    pub(super) fn len(&self) -> usize {
        locked(&self.state).jar.len()
    }

    /// Takes the changes since the last sync: the unexpired cookies to write, with their current
    /// values, and the cookies to remove. The note of changes is cleared.
    pub(super) fn take_pending(&self, now: u64) -> Pending {
        let mut state = locked(&self.state);
        let set = std::mem::take(&mut state.set);
        let gone = std::mem::take(&mut state.gone);
        let cookies = set
            .iter()
            .filter_map(|key| {
                state
                    .jar
                    .cookies()
                    .iter()
                    .find(|cookie| key.matches(cookie))
            })
            .filter(|cookie| !cookie.is_expired(now))
            .cloned()
            .collect();
        Pending { cookies, gone }
    }

    /// Puts changes back after a sync that failed, ahead of anything that arrived meanwhile.
    pub(super) fn requeue(&self, pending: &Pending) {
        let mut state = locked(&self.state);
        let newer_set = std::mem::take(&mut state.set);
        let newer_gone = std::mem::take(&mut state.gone);
        for cookie in &pending.cookies {
            note_set(&mut state, CookieKey::of(cookie));
        }
        for key in &pending.gone {
            note_gone(&mut state, key.clone());
        }
        for key in newer_set {
            note_set(&mut state, key);
        }
        for key in newer_gone {
            note_gone(&mut state, key);
        }
    }
}

/// Notes the cookies that differ between two lists of the jar.
fn note_changes(state: &mut State, before: &[StoredCookie], after: &[StoredCookie]) {
    let old: HashMap<CookieKey, &StoredCookie> = before
        .iter()
        .map(|cookie| (CookieKey::of(cookie), cookie))
        .collect();
    let mut kept: HashSet<CookieKey> = HashSet::with_capacity(after.len());
    for cookie in after {
        let key = CookieKey::of(cookie);
        let changed = old.get(&key).is_none_or(|earlier| differs(earlier, cookie));
        kept.insert(key.clone());
        if changed {
            note_set(state, key);
        }
    }
    for cookie in before {
        let key = CookieKey::of(cookie);
        if !kept.contains(&key) {
            note_gone(state, key);
        }
    }
}

/// Whether the browser would see a difference between two states of one cookie.
fn differs(earlier: &StoredCookie, now: &StoredCookie) -> bool {
    earlier.value != now.value
        || earlier.expires != now.expires
        || earlier.secure != now.secure
        || earlier.http_only != now.http_only
        || earlier.same_site != now.same_site
}

fn note_set(state: &mut State, key: CookieKey) {
    state.gone.retain(|known| *known != key);
    state.set.retain(|known| *known != key);
    if state.set.len() >= MAX_PENDING_CHANGES {
        state.set.remove(0);
    }
    state.set.push(key);
}

fn note_gone(state: &mut State, key: CookieKey) {
    state.set.retain(|known| *known != key);
    state.gone.retain(|known| *known != key);
    if state.gone.len() >= MAX_PENDING_CHANGES {
        state.gone.remove(0);
    }
    state.gone.push(key);
}

/// The parameters that store `cookie` in the browser with `Network.setCookies`.
///
/// A host-only cookie is given by URL and no domain, so the browser keeps it host-only; a domain
/// cookie is given by domain.
pub(super) fn set_params(cookie: &StoredCookie) -> Value {
    let mut params = json!({
        "name": cookie.name,
        "value": cookie.value,
        "path": cookie.path,
        "secure": cookie.secure,
        "httpOnly": cookie.http_only,
    });
    if cookie.host_only {
        let scheme = if cookie.secure { "https" } else { "http" };
        let host = if cookie.domain.contains(':') {
            format!("[{}]", cookie.domain)
        } else {
            cookie.domain.clone()
        };
        params["url"] = json!(format!("{scheme}://{host}{}", cookie.path));
    } else {
        params["domain"] = json!(format!(".{}", cookie.domain));
    }
    if let Some(expires) = cookie.expires {
        params["expires"] = json!(seconds(expires));
    }
    if let Some(same_site) = cookie.same_site {
        params["sameSite"] = json!(match same_site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None",
        });
    }
    params
}

/// Whole seconds as the protocol's number. A time a clock reads is far below 2^53.
#[expect(
    clippy::cast_precision_loss,
    reason = "an expiry is at most 400 days after a time a clock reads, far below 2^53 seconds"
)]
fn seconds(at: u64) -> f64 {
    at as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn url(raw: &str) -> Url {
        Url::parse(raw).expect("a test URL")
    }

    fn jar_with(lines: &[(&str, &str)]) -> SessionJar {
        let jar = SessionJar::new(CookieJar::new());
        for (at, line) in lines {
            jar.absorb(&url(at), &[*line], NOW);
        }
        jar
    }

    #[test]
    fn a_response_cookie_is_sent_back_to_its_host_only() {
        let jar = jar_with(&[("https://a.test/", "sid=1; Path=/")]);
        let same = SiteContext::SameSite { top_level: true };
        assert_eq!(
            jar.header_for(&url("https://a.test/x"), same, NOW)
                .as_deref(),
            Some("sid=1")
        );
        assert_eq!(
            jar.header_for(&url("https://sub.a.test/"), same, NOW),
            None,
            "host-only"
        );
        assert_eq!(jar.header_for(&url("https://b.test/"), same, NOW), None);
    }

    #[test]
    fn a_public_suffix_domain_is_ignored_and_counted() {
        let jar = SessionJar::new(CookieJar::new());
        let absorbed = jar.absorb(
            &url("https://shop.co.uk/"),
            &["a=1; Domain=co.uk", "b=2; Domain=.github.io", "c=3"],
            NOW,
        );
        assert_eq!(absorbed.stored, 1);
        assert_eq!(absorbed.ignored, 2);
        assert_eq!(jar.len(), 1);
    }

    #[test]
    fn a_cookie_for_another_host_is_ignored() {
        let jar = SessionJar::new(CookieJar::new());
        let absorbed = jar.absorb(&url("https://a.test/"), &["x=1; Domain=b.test"], NOW);
        assert_eq!((absorbed.stored, absorbed.ignored), (0, 1));
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn set_cookie_headers_beyond_the_cap_are_not_read() {
        let jar = SessionJar::new(CookieJar::new());
        let lines: Vec<String> = (0..MAX_SET_COOKIES_PER_RESPONSE + 20)
            .map(|index| format!("c{index}=v"))
            .collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let absorbed = jar.absorb(&url("https://a.test/"), &refs, NOW);
        assert_eq!(absorbed.stored, MAX_SET_COOKIES_PER_RESPONSE);
    }

    #[test]
    fn the_site_context_depends_on_the_shape_the_method_and_the_chain() {
        let get = Method::GET;
        let post = Method::POST;
        assert_eq!(
            context_for(RequestShape::Document, &get, false),
            SiteContext::SameSite { top_level: true }
        );
        assert_eq!(
            context_for(RequestShape::Fetch, &post, false),
            SiteContext::SameSite { top_level: false }
        );
        assert_eq!(
            context_for(RequestShape::Document, &get, true),
            SiteContext::CrossSite {
                top_level_get: true
            }
        );
        assert_eq!(
            context_for(RequestShape::Document, &post, true),
            SiteContext::CrossSite {
                top_level_get: false
            }
        );
        assert_eq!(
            context_for(RequestShape::Fetch, &get, true),
            SiteContext::CrossSite {
                top_level_get: false
            }
        );
    }

    #[test]
    fn a_cross_site_chain_keeps_strict_cookies_home() {
        let jar = jar_with(&[
            ("https://a.test/", "strict=1; SameSite=Strict; Secure"),
            ("https://a.test/", "lax=1; SameSite=Lax; Secure"),
            ("https://a.test/", "none=1; SameSite=None; Secure"),
        ]);
        let to = url("https://a.test/x");
        let cross_get = SiteContext::CrossSite {
            top_level_get: true,
        };
        let cross_post = SiteContext::CrossSite {
            top_level_get: false,
        };
        assert_eq!(
            jar.header_for(&to, cross_get, NOW).as_deref(),
            Some("lax=1; none=1")
        );
        assert_eq!(
            jar.header_for(&to, cross_post, NOW).as_deref(),
            Some("none=1")
        );
    }

    #[test]
    fn what_a_response_set_and_removed_is_noted_for_the_browser() {
        let jar = jar_with(&[("https://a.test/", "keep=1"), ("https://a.test/", "drop=1")]);
        jar.take_pending(NOW);
        jar.absorb(
            &url("https://a.test/"),
            &["new=2; Domain=a.test; Path=/p", "drop=; Max-Age=0"],
            NOW,
        );
        let pending = jar.take_pending(NOW);
        let names: Vec<&str> = pending.cookies.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["new"]);
        assert_eq!(pending.gone.len(), 1);
        assert_eq!(pending.gone[0].delete_params()["name"], "drop");
        assert_eq!(
            pending.gone[0].delete_params()["domain"],
            "a.test",
            "host-only: no dot"
        );
        let again = jar.take_pending(NOW);
        assert!(
            again.cookies.is_empty() && again.gone.is_empty(),
            "the note is cleared"
        );
    }

    #[test]
    fn a_cookie_set_again_unchanged_is_not_noted() {
        let jar = jar_with(&[("https://a.test/", "same=1")]);
        jar.take_pending(NOW);
        jar.absorb(&url("https://a.test/"), &["same=1"], NOW);
        assert_eq!(jar.take_pending(NOW).cookies.len(), 0);
        jar.absorb(&url("https://a.test/"), &["same=2"], NOW);
        assert_eq!(jar.take_pending(NOW).cookies.len(), 1);
    }

    #[test]
    fn a_cookie_removed_and_set_again_is_only_set() {
        let jar = jar_with(&[("https://a.test/", "c=1")]);
        jar.take_pending(NOW);
        jar.absorb(&url("https://a.test/"), &["c=; Max-Age=0"], NOW);
        jar.absorb(&url("https://a.test/"), &["c=3"], NOW);
        let pending = jar.take_pending(NOW);
        assert_eq!(pending.cookies.len(), 1);
        assert_eq!(pending.gone.len(), 0);
    }

    #[test]
    fn a_failed_sync_requeues_the_changes_ahead_of_newer_ones() {
        let jar = jar_with(&[("https://a.test/", "one=1")]);
        let pending = jar.take_pending(NOW);
        jar.absorb(&url("https://a.test/"), &["two=2"], NOW);
        jar.requeue(&pending);
        let names: Vec<String> = jar
            .take_pending(NOW)
            .cookies
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["one", "two"]);
    }

    fn key(index: usize) -> CookieKey {
        CookieKey {
            name: format!("c{index}"),
            domain: "a.test".to_owned(),
            host_only: true,
            path: "/".to_owned(),
        }
    }

    #[test]
    fn the_note_of_changes_is_bounded_and_forgets_the_oldest() {
        let mut state = State::default();
        for index in 0..MAX_PENDING_CHANGES + 10 {
            note_set(&mut state, key(index));
            note_gone(&mut state, key(index + MAX_PENDING_CHANGES * 2));
        }
        assert_eq!(state.set.len(), MAX_PENDING_CHANGES);
        assert_eq!(state.gone.len(), MAX_PENDING_CHANGES);
        assert!(!state.set.contains(&key(0)), "the oldest was forgotten");
        assert!(state.set.contains(&key(MAX_PENDING_CHANGES + 9)));
    }

    #[test]
    fn host_only_cookies_are_given_to_the_browser_by_url_and_domain_cookies_by_domain() {
        let jar = jar_with(&[
            (
                "https://a.test/",
                "host=1; Secure; HttpOnly; SameSite=Lax; Max-Age=60",
            ),
            ("https://a.test/", "wide=1; Domain=a.test; Path=/p"),
        ]);
        let pending = jar.take_pending(NOW);
        let host = set_params(&pending.cookies[0]);
        assert_eq!(host["url"], "https://a.test/");
        assert!(host.get("domain").is_none());
        assert_eq!(host["secure"], true);
        assert_eq!(host["httpOnly"], true);
        assert_eq!(host["sameSite"], "Lax");
        assert_eq!(host["expires"].as_f64(), Some(1_800_000_060.0));
        let wide = set_params(&pending.cookies[1]);
        assert_eq!(wide["domain"], ".a.test");
        assert!(wide.get("url").is_none() && wide.get("expires").is_none());
        assert_eq!(wide["path"], "/p");
    }

    #[test]
    fn nothing_secret_reaches_debug() {
        let jar = jar_with(&[("https://a.test/", "sid=s3cret")]);
        let shown = format!("{jar:?} {:?}", jar.take_pending(NOW));
        assert!(!shown.contains("s3cret"), "{shown}");
        let key = CookieKey::of(&jar.snapshot().cookies()[0]);
        assert!(!format!("{key:?}").contains("s3cret"));
    }

    #[test]
    fn an_expired_cookie_is_not_given_to_the_browser() {
        let jar = jar_with(&[("https://a.test/", "short=1; Max-Age=10")]);
        let pending = jar.take_pending(NOW + 11);
        assert_eq!(pending.cookies.len(), 0);
    }
}
