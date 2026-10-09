//! Answering proxy authentication challenges, tab by tab.
//!
//! Chrome ignores credentials written into a proxy address, so they have to
//! be supplied when the proxy asks (`Fetch.authRequired`). Different tabs can
//! sit behind different proxies, so the answer depends on the tab: this
//! registry maps a tab's protocol session to its credentials, falling back to
//! the ones the browser was launched with.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde_json::{json, Value};

use super::sync::locked;

/// A proxy username and password.
pub(super) type Credentials = (String, String);

#[derive(Default)]
pub(super) struct ProxyAuth {
    /// Credentials of the proxy the browser was launched with.
    launch: Option<Credentials>,
    /// Credentials of tabs in contexts with their own proxy.
    sessions: Mutex<HashMap<String, Credentials>>,
    /// Tabs whose paused requests an `Interception` is answering.
    intercepted: Mutex<HashSet<String>>,
    started: AtomicBool,
}

impl fmt::Debug for ProxyAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never the passwords.
        f.debug_struct("ProxyAuth")
            .field("launch", &self.launch.as_ref().map(|_| "<redacted>"))
            .field("sessions", &locked(&self.sessions).len())
            .finish_non_exhaustive()
    }
}

impl ProxyAuth {
    pub(super) fn new(launch: Option<Credentials>) -> Self {
        Self {
            launch,
            ..Self::default()
        }
    }

    pub(super) fn has_launch_credentials(&self) -> bool {
        self.launch.is_some()
    }

    /// The credentials for a tab: its own, else the launch proxy's.
    fn credentials(&self, session: &str) -> Option<Credentials> {
        locked(&self.sessions)
            .get(session)
            .cloned()
            .or_else(|| self.launch.clone())
    }

    /// Whether the tab has a proxy that wants a password.
    pub(super) fn handles_auth(&self, session: &str) -> bool {
        self.credentials(session).is_some()
    }

    pub(super) fn register(&self, session: &str, credentials: Credentials) {
        locked(&self.sessions).insert(session.to_owned(), credentials);
    }

    pub(super) fn forget(&self, session: &str) {
        locked(&self.sessions).remove(session);
        locked(&self.intercepted).remove(session);
    }

    /// Records whether an `Interception` is answering this tab's paused
    /// requests, so they are not continued here as well.
    pub(super) fn set_intercepted(&self, session: &str, on: bool) {
        let mut intercepted = locked(&self.intercepted);
        if on {
            intercepted.insert(session.to_owned());
        } else {
            intercepted.remove(session);
        }
    }

    /// `true` the first time only: whoever gets it starts the responder task.
    pub(super) fn claim_start(&self) -> bool {
        !self.started.swap(true, Ordering::AcqRel)
    }

    /// The command that answers a `Fetch` event from `session`, if any.
    pub(super) fn reply(
        &self,
        session: &str,
        method: &str,
        params: &Value,
    ) -> Option<(&'static str, Value)> {
        let request_id = params["requestId"].clone();
        match method {
            "Fetch.authRequired" => {
                let (username, password) = self.credentials(session)?;
                Some((
                    "Fetch.continueWithAuth",
                    json!({
                        "requestId": request_id,
                        "authChallengeResponse": {
                            "response": "ProvideCredentials",
                            "username": username,
                            "password": password,
                        },
                    }),
                ))
            }
            // Enabling auth handling pauses every request; let them go, unless
            // an interception has claimed this tab's requests.
            "Fetch.requestPaused"
                if self.credentials(session).is_some()
                    && !locked(&self.intercepted).contains(session) =>
            {
                Some(("Fetch.continueRequest", json!({ "requestId": request_id })))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds(user: &str) -> Credentials {
        (user.to_owned(), "pw".to_owned())
    }

    fn challenge() -> Value {
        json!({ "requestId": "R1" })
    }

    #[test]
    fn a_tab_with_its_own_credentials_gets_them() {
        let auth = ProxyAuth::new(Some(creds("launch")));
        auth.register("S1", creds("mine"));

        let (method, params) = auth
            .reply("S1", "Fetch.authRequired", &challenge())
            .unwrap();

        assert_eq!(method, "Fetch.continueWithAuth");
        assert_eq!(params["authChallengeResponse"]["username"], "mine");
        assert_eq!(
            params["authChallengeResponse"]["response"],
            "ProvideCredentials"
        );
        assert_eq!(params["requestId"], "R1");
    }

    #[test]
    fn other_tabs_fall_back_to_the_launch_credentials() {
        let auth = ProxyAuth::new(Some(creds("launch")));
        auth.register("S1", creds("mine"));

        let (_, params) = auth
            .reply("S2", "Fetch.authRequired", &challenge())
            .unwrap();

        assert_eq!(params["authChallengeResponse"]["username"], "launch");
    }

    #[test]
    fn a_tab_nobody_has_credentials_for_is_not_answered() {
        let auth = ProxyAuth::new(None);
        auth.register("S1", creds("mine"));

        assert!(auth
            .reply("S2", "Fetch.authRequired", &challenge())
            .is_none());
        assert!(auth
            .reply("S2", "Fetch.requestPaused", &challenge())
            .is_none());
        assert!(!auth.handles_auth("S2"));
        assert!(auth.handles_auth("S1"));
    }

    #[test]
    fn paused_requests_are_continued_unless_an_interception_owns_them() {
        let auth = ProxyAuth::new(Some(creds("launch")));

        let (method, params) = auth
            .reply("S1", "Fetch.requestPaused", &challenge())
            .unwrap();
        assert_eq!(
            (method, params),
            ("Fetch.continueRequest", json!({ "requestId": "R1" }))
        );

        auth.set_intercepted("S1", true);
        assert!(auth
            .reply("S1", "Fetch.requestPaused", &challenge())
            .is_none());
        assert!(
            auth.reply("S1", "Fetch.authRequired", &challenge())
                .is_some(),
            "the password is still supplied while intercepting"
        );

        auth.set_intercepted("S1", false);
        assert!(auth
            .reply("S1", "Fetch.requestPaused", &challenge())
            .is_some());
    }

    #[test]
    fn forgetting_a_tab_drops_its_credentials_and_its_interception() {
        let auth = ProxyAuth::new(None);
        auth.register("S1", creds("mine"));
        auth.set_intercepted("S1", true);

        auth.forget("S1");

        assert!(!auth.handles_auth("S1"));
        assert!(!locked(&auth.intercepted).contains("S1"));
    }

    #[test]
    fn unrelated_events_are_ignored() {
        let auth = ProxyAuth::new(Some(creds("launch")));
        assert!(auth
            .reply("S1", "Network.requestWillBeSent", &challenge())
            .is_none());
    }

    #[test]
    fn the_responder_is_started_once() {
        let auth = ProxyAuth::new(None);
        assert!(auth.claim_start());
        assert!(!auth.claim_start());
    }

    #[test]
    fn debug_output_never_shows_a_password() {
        let auth = ProxyAuth::new(Some(("user".into(), "hunter2-secret".into())));
        auth.register("S1", ("u2".into(), "other-secret".into()));
        let shown = format!("{auth:?}");
        assert!(
            !shown.contains("hunter2")
                && !shown.contains("other-secret")
                && !shown.contains("user"),
            "{shown}"
        );
    }
}
