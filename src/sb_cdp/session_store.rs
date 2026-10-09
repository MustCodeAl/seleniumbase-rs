//! Browser sessions kept in memory and shared between workers.
//!
//! A [`Session`] is what makes a browser "logged in": its cookies, and the
//! local storage of the page it was captured on. A [`SessionStore`] holds named
//! sessions for any number of workers. One worker signs in and saves; the
//! others load and are signed in, in their own browsers, without repeating the
//! login.
//!
//! The store lives in memory only. Nothing is written to disk, and its `Debug`
//! output never shows a cookie value.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::sb_cdp::{Cookie, Session, SessionStore};
//!
//! let store = SessionStore::new();
//! store.save("alice", Session::new(vec![Cookie::new("sid", "s3cret")]));
//!
//! let shared = store.clone(); // another worker's handle to the same store
//! assert_eq!(shared.load("alice").unwrap().cookies().len(), 1);
//! assert!(!format!("{store:?}").contains("s3cret"));
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use super::sync::locked;
use super::Cookie;

/// A browser's login state at one moment.
#[derive(Clone, Default, PartialEq)]
pub struct Session {
    cookies: Vec<Cookie>,
    origin: Option<String>,
    local_storage: BTreeMap<String, String>,
}

impl Session {
    /// A session holding these cookies.
    #[must_use]
    pub fn new(cookies: Vec<Cookie>) -> Self {
        Self {
            cookies,
            origin: None,
            local_storage: BTreeMap::new(),
        }
    }

    /// Adds the local storage of `origin`, such as `https://example.com`.
    #[must_use]
    pub fn with_local_storage(
        mut self,
        origin: impl Into<String>,
        entries: BTreeMap<String, String>,
    ) -> Self {
        self.origin = Some(origin.into());
        self.local_storage = entries;
        self
    }

    /// The cookies.
    #[must_use]
    pub fn cookies(&self) -> &[Cookie] {
        &self.cookies
    }

    /// The origin the local storage was captured from, if any.
    #[must_use]
    pub fn origin(&self) -> Option<&str> {
        self.origin.as_deref()
    }

    /// The local storage entries of [`origin`](Self::origin).
    #[must_use]
    pub fn local_storage(&self) -> &BTreeMap<String, String> {
        &self.local_storage
    }

    /// The session without cookies that have expired by `now` (Unix seconds).
    /// Session cookies, which have no expiry, are kept.
    #[must_use]
    pub fn without_expired(mut self, now: f64) -> Self {
        self.cookies
            .retain(|cookie| cookie.expires.is_none_or(|expires| expires > now));
        self
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Neither cookie values nor storage values are secrets to log.
        f.debug_struct("Session")
            .field("cookies", &self.cookies.len())
            .field("origin", &self.origin)
            .field("local_storage_keys", &self.local_storage.len())
            .finish()
    }
}

/// Named [`Session`]s shared between workers.
///
/// Cloning gives another handle to the same store. It is `Send` and `Sync`.
#[derive(Clone, Default)]
pub struct SessionStore {
    sessions: Arc<Mutex<BTreeMap<String, Session>>>,
}

impl SessionStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `session` under `name`, replacing any earlier one.
    pub fn save(&self, name: impl Into<String>, session: Session) {
        locked(&self.sessions).insert(name.into(), session);
    }

    /// A copy of the session saved as `name`.
    #[must_use]
    pub fn load(&self, name: &str) -> Option<Session> {
        locked(&self.sessions).get(name).cloned()
    }

    /// Removes and returns the session saved as `name`.
    #[allow(
        clippy::must_use_candidate,
        reason = "removing is the point; the old session is a bonus"
    )]
    pub fn remove(&self, name: &str) -> Option<Session> {
        locked(&self.sessions).remove(name)
    }

    /// The names of the saved sessions, in order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        locked(&self.sessions).keys().cloned().collect()
    }

    /// How many sessions are saved.
    #[must_use]
    pub fn len(&self) -> usize {
        locked(&self.sessions).len()
    }

    /// Whether no session is saved.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        locked(&self.sessions).is_empty()
    }

    /// Removes every session.
    pub fn clear(&self) {
        locked(&self.sessions).clear();
    }

    /// Drops cookies that have expired by `now` (Unix seconds) from every
    /// session, and returns how many were dropped.
    #[allow(
        clippy::must_use_candidate,
        reason = "pruning is the point; the count is a bonus"
    )]
    pub fn prune_expired(&self, now: f64) -> usize {
        let mut sessions = locked(&self.sessions);
        let mut dropped = 0;
        for session in sessions.values_mut() {
            let before = session.cookies.len();
            session
                .cookies
                .retain(|cookie| cookie.expires.is_none_or(|expires| expires > now));
            dropped += before - session.cookies.len();
        }
        dropped
    }
}

impl fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionStore")
            .field("sessions", &self.names())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(name: &str, expires: Option<f64>) -> Cookie {
        let cookie = Cookie::new(name, "value");
        match expires {
            Some(at) => cookie.expires(at),
            None => cookie,
        }
    }

    #[test]
    fn a_saved_session_can_be_loaded_listed_and_removed() {
        let store = SessionStore::new();
        assert!(store.is_empty());
        store.save("b", Session::new(vec![cookie("one", None)]));
        store.save("a", Session::default());

        assert_eq!(store.names(), ["a", "b"], "names come back in order");
        assert_eq!(store.len(), 2);
        assert_eq!(store.load("b").unwrap().cookies().len(), 1);
        assert!(store.load("missing").is_none());
        assert_eq!(store.remove("b").unwrap().cookies().len(), 1);
        assert!(store.load("b").is_none());
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn saving_again_replaces_the_session() {
        let store = SessionStore::new();
        store.save("x", Session::new(vec![cookie("old", None)]));
        store.save(
            "x",
            Session::new(vec![cookie("new", None), cookie("newer", None)]),
        );

        assert_eq!(store.len(), 1);
        assert_eq!(store.load("x").unwrap().cookies().len(), 2);
    }

    #[test]
    fn a_loaded_session_is_a_copy_so_workers_do_not_alias_each_other() {
        let store = SessionStore::new();
        store.save("x", Session::new(vec![cookie("a", None)]));
        let mut mine = store.load("x").unwrap();
        mine =
            mine.with_local_storage("https://a.test", BTreeMap::from([("k".into(), "v".into())]));

        assert_eq!(mine.origin(), Some("https://a.test"));
        assert!(store.load("x").unwrap().origin().is_none());
    }

    #[test]
    fn clones_share_one_store() {
        let store = SessionStore::new();
        let other = store.clone();
        store.save("shared", Session::default());

        assert!(other.load("shared").is_some());
    }

    #[test]
    fn expired_cookies_are_pruned_and_session_cookies_stay() {
        let store = SessionStore::new();
        store.save(
            "x",
            Session::new(vec![
                cookie("expired", Some(100.0)),
                cookie("live", Some(9_999.0)),
                cookie("session", None),
            ]),
        );
        store.save("y", Session::new(vec![cookie("expired", Some(5.0))]));

        assert_eq!(store.prune_expired(1_000.0), 2);

        let names: Vec<_> = store
            .load("x")
            .unwrap()
            .cookies()
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(names, ["live", "session"]);
        assert_eq!(store.load("y").unwrap().cookies().len(), 0);
    }

    #[test]
    fn a_session_can_drop_its_own_expired_cookies() {
        let session = Session::new(vec![cookie("old", Some(1.0)), cookie("fresh", Some(50.0))]);
        assert_eq!(session.without_expired(10.0).cookies().len(), 1);
    }

    #[test]
    fn debug_output_never_shows_a_value() {
        let store = SessionStore::new();
        let secret = Cookie::new("sid", "hunter2-secret");
        let session = Session::new(vec![secret]).with_local_storage(
            "https://a.test",
            BTreeMap::from([("token".into(), "tok-secret".into())]),
        );
        store.save("login", session.clone());

        let shown = format!("{store:?} {session:?}");
        assert!(
            !shown.contains("hunter2") && !shown.contains("tok-secret"),
            "{shown}"
        );
        assert!(shown.contains("login"));
    }

    #[test]
    fn many_threads_can_save_and_load_at_once() {
        let store = SessionStore::new();
        let handles: Vec<_> = (0..8)
            .map(|worker| {
                let store = store.clone();
                std::thread::spawn(move || {
                    for round in 0..50 {
                        store.save(
                            format!("w{worker}"),
                            Session::new(vec![cookie(&format!("c{round}"), None)]),
                        );
                        assert!(store.load(&format!("w{worker}")).is_some());
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(store.len(), 8);
    }
}
