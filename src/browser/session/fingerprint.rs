//! Applying a [`Fingerprint`](crate::Fingerprint) to a WebDriver session.

use serde_json::Value;

use crate::browser::config::BrowserConfig;
use crate::error::SeleniumBaseError;

use super::BrowserSession;

impl BrowserSession {
    /// Applies an optional [`Fingerprint`](crate::Fingerprint) to the active session.
    ///
    /// The script is [`session_script`](crate::evasion::evasions::session_script),
    /// the one the pure CDP engine runs. When CDP is available it is registered via
    /// `Page.addScriptToEvaluateOnNewDocument` and CDP overrides (screen size,
    /// geolocation, timezone, UA) are issued. Otherwise the script is executed
    /// directly in the current page as a fallback.
    pub async fn apply_fingerprint(&self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        let Some(fp) = config.fingerprint.as_ref() else {
            return Ok(());
        };

        let script = fingerprint_script(fp);

        if let Ok(cdp) = self.cdp_client() {
            cdp.add_init_script(&script).await?;
            for (method, params) in fingerprint_overrides(fp) {
                cdp.execute(&method, params).await.ok();
            }
        } else {
            let _ = self.webdriver()?.execute(&script, Vec::new()).await?;
        }
        Ok(())
    }
}

/// The init script that makes a WebDriver session match `fp`: the same one the
/// pure CDP engine runs, since both drive a real Chrome.
fn fingerprint_script(fp: &crate::evasion::fingerprint::Fingerprint) -> String {
    crate::evasion::evasions::session_script(fp)
}

/// The protocol commands, after the init script, that make a WebDriver session
/// match `fp`.
///
/// The `Proxy-Authorization` header a fingerprint with a proxy password can ask
/// for is left out: set through `Network.setExtraHTTPHeaders` it would reach
/// every site the browser visits, and hand the proxy's login to all of them.
pub(super) fn fingerprint_overrides(
    fp: &crate::evasion::fingerprint::Fingerprint,
) -> Vec<(String, Value)> {
    crate::evasion::evasions::safe_cdp_overrides(fp)
}

#[cfg(test)]
mod fingerprint_tests {
    use super::*;
    use crate::evasion::evasions::{raw_overrides, CREDENTIAL_HEADER_METHOD};
    use crate::evasion::fingerprint::{Fingerprint, OsType, ProxyConfig};

    fn with_proxy_login() -> Fingerprint {
        Fingerprint::randomized(OsType::Linux, 3)
            .to_builder()
            .proxy(ProxyConfig {
                r#type: "http".to_owned(),
                host: "proxy.example.com".to_owned(),
                port: 8080,
                username: Some("alice".to_owned()),
                password: Some("fake-proxy-password".to_owned()),
                save_traffic: false,
            })
            .build()
            .expect("the builder builds")
    }

    #[test]
    fn the_proxy_login_is_never_sent_to_every_site() {
        let fingerprint = with_proxy_login();
        // The source of the overrides does ask for the header.
        assert!(raw_overrides(&fingerprint).contains_key(CREDENTIAL_HEADER_METHOD));

        let sent = fingerprint_overrides(&fingerprint);

        assert!(sent
            .iter()
            .all(|(method, _)| method != CREDENTIAL_HEADER_METHOD));
        let everything = format!("{sent:?}");
        assert!(!everything.contains("Proxy-Authorization"), "{everything}");
        assert!(!everything.contains("fake-proxy-password"), "{everything}");
        assert!(
            !everything.contains("YWxpY2U6ZmFrZS1wcm94eS1wYXNzd29yZA"),
            "{everything}"
        );
    }

    #[test]
    fn a_webdriver_session_gets_the_same_script_as_the_pure_cdp_engine() {
        // Both drive a real Chrome, which answers for plugins, mime types, the
        // PDF viewer flag and the network report itself.
        let fingerprint = Fingerprint::randomized(OsType::Linux, 3);
        let script = fingerprint_script(&fingerprint);
        assert_eq!(
            script,
            crate::evasion::providers::session_registry(&fingerprint).bootstrap(
                &crate::evasion::providers::EvasionContext::new(&fingerprint)
            )
        );
        assert!(
            !script.contains("mimeTypes"),
            "a spoof Chrome does not need"
        );
    }

    #[test]
    fn the_other_overrides_are_still_sent_in_a_stable_order() {
        let fingerprint = with_proxy_login();
        let sent = fingerprint_overrides(&fingerprint);
        let methods: Vec<_> = sent.iter().map(|(method, _)| method.as_str()).collect();
        assert!(
            methods.contains(&"Network.setUserAgentOverride"),
            "{methods:?}"
        );
        assert!(
            methods.contains(&"Emulation.setTimezoneOverride"),
            "{methods:?}"
        );
        let mut sorted = methods.clone();
        sorted.sort_unstable();
        assert_eq!(methods, sorted);
    }
}
