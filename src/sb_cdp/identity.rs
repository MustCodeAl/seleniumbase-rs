//! Giving a tab a browser identity from a [`Fingerprint`].
//!
//! A fingerprint says what the browser claims to be: its user agent, language,
//! time zone, screen, location and the hardware a page can probe. Two things
//! make a tab match it. A script, run before any page script, rewrites what the
//! page can read; and a set of protocol overrides changes what the browser
//! itself reports and sends. [`Page::apply_fingerprint`] does both for one tab,
//! and [`LaunchOptionsBuilder::fingerprint`](super::LaunchOptionsBuilder::fingerprint)
//! does it for every tab a browser opens.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//! use seleniumbase_rs::{Fingerprint, OsType};
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let fingerprint = Fingerprint::randomized(OsType::Windows, 42);
//! let browser = Browser::launch(
//!     LaunchOptions::builder().fingerprint(&fingerprint).build()?,
//! )
//! .await?;
//! let page = browser.default_page().await?;
//! page.goto("https://example.com").await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;

use serde_json::{json, Value};

use super::Page;
use crate::error::SeleniumBaseError;
use crate::stealth::evasions::{bootstrap_script, cdp_overrides};
use crate::stealth::fingerprint::Fingerprint;

/// The override that adds a `Proxy-Authorization` header to every request.
///
/// A fingerprint can ask for it, but a header added this way goes to every
/// site the tab visits, which would hand the proxy's password to the sites.
/// The tab's proxy is answered through the proxy's own challenge instead.
const CREDENTIAL_HEADER: &str = "Network.setExtraHTTPHeaders";

/// The override that grants permissions. It is browser-wide, so it needs the
/// context of the tab it is for and is sent separately.
const PERMISSIONS: &str = "Browser.grantPermissions";

/// What it takes to make a tab match a fingerprint, worked out once.
#[derive(Clone)]
pub(crate) struct Identity {
    /// Commands for the tab itself, in the order they are sent.
    tab: Vec<(String, Value)>,
    /// Permissions to grant in the tab's browser context, if any.
    permissions: Option<Vec<String>>,
}

impl fmt::Debug for Identity {
    /// Counts instead of printing: the bootstrap script is thousands of lines.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("tab_commands", &self.tab.len())
            .field("permissions", &self.permissions.as_ref().map(Vec::len))
            .finish()
    }
}

impl Identity {
    pub(crate) fn of(fingerprint: &Fingerprint) -> Self {
        let mut tab = vec![
            (
                "Page.addScriptToEvaluateOnNewDocument".to_owned(),
                json!({ "source": bootstrap_script(fingerprint) }),
            ),
            // Some overrides, such as the blocked URLs, only take effect once
            // the network domain is on.
            ("Network.enable".to_owned(), json!({})),
        ];
        let mut overrides: Vec<(String, Value)> = cdp_overrides(fingerprint)
            .into_iter()
            .filter(|(method, _)| method != CREDENTIAL_HEADER)
            .collect();
        // A map has no order; sorting makes the commands, and so the tests and
        // any trace of them, the same on every run.
        overrides.sort_by(|a, b| a.0.cmp(&b.0));

        let mut permissions = None;
        for (method, params) in overrides {
            if method == PERMISSIONS {
                permissions = params["permissions"].as_array().map(|names| {
                    names
                        .iter()
                        .filter_map(|name| name.as_str().map(str::to_owned))
                        .collect()
                });
            } else {
                tab.push((method, params));
            }
        }
        Self { tab, permissions }
    }

    /// The commands for the tab's own protocol session, in order.
    pub(crate) fn tab_commands(&self) -> &[(String, Value)] {
        &self.tab
    }

    /// The parameters of the permission grant for a tab in `context`, if the
    /// fingerprint asks for one.
    pub(crate) fn permission_grant(&self, context: Option<&str>) -> Option<Value> {
        let names = self
            .permissions
            .as_ref()
            .filter(|names| !names.is_empty())?;
        let mut params = json!({ "permissions": names });
        if let Some(context) = context {
            params["browserContextId"] = json!(context);
        }
        Some(params)
    }
}

impl Page {
    /// Makes this tab match `fingerprint`, for the documents it loads from now
    /// on.
    ///
    /// Installs the fingerprint's script to run before any page script, then
    /// applies the protocol overrides it recommends: user agent and Client
    /// Hints, language, time zone, screen size, location, and so on. The page
    /// that is open now keeps its old identity until it navigates or reloads.
    ///
    /// The `Proxy-Authorization` header a fingerprint with a proxy password can
    /// ask for is never sent: it would reach every site the tab visits.
    /// Permissions the fingerprint grants go to this tab's own browser context,
    /// not to the browser's default one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] naming the command the browser
    /// refused. Commands already applied stay applied.
    pub async fn apply_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<(), SeleniumBaseError> {
        let identity = Identity::of(fingerprint);
        for (method, params) in identity.tab_commands() {
            self.execute(method, params.clone())
                .await
                .map_err(|error| refused(method, &error))?;
        }
        if let Some(grant) = identity.permission_grant(self.browser_context_id().await?.as_deref())
        {
            self.browser()
                .execute(PERMISSIONS, grant)
                .await
                .map_err(|error| refused(PERMISSIONS, &error))?;
        }
        Ok(())
    }

    /// The browser context this tab lives in, if the browser says.
    async fn browser_context_id(&self) -> Result<Option<String>, SeleniumBaseError> {
        let info = self
            .browser()
            .execute("Target.getTargetInfo", json!({ "targetId": self.id() }))
            .await?;
        Ok(info["targetInfo"]["browserContextId"]
            .as_str()
            .map(str::to_owned))
    }
}

/// The error for a command the browser refused while applying an identity.
pub(super) fn refused(method: &str, error: &SeleniumBaseError) -> SeleniumBaseError {
    SeleniumBaseError::cdp_driver(format!(
        "applying the fingerprint failed at {method}: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::fingerprint::{OsType, ProxyConfig};

    fn methods(identity: &Identity) -> Vec<&str> {
        identity
            .tab_commands()
            .iter()
            .map(|(m, _)| m.as_str())
            .collect()
    }

    #[test]
    fn the_script_comes_first_then_the_network_domain_then_the_overrides() {
        let identity = Identity::of(&Fingerprint::randomized(OsType::Windows, 7));

        let methods = methods(&identity);

        assert_eq!(methods[0], "Page.addScriptToEvaluateOnNewDocument");
        assert_eq!(methods[1], "Network.enable");
        assert!(methods.contains(&"Network.setUserAgentOverride"));
        assert!(methods.contains(&"Emulation.setTimezoneOverride"));
        assert!(methods.contains(&"Emulation.setLocaleOverride"));
    }

    #[test]
    fn the_overrides_are_in_a_stable_order() {
        let fingerprint = Fingerprint::randomized(OsType::Macos, 99);

        let first: Vec<_> = methods(&Identity::of(&fingerprint))
            .into_iter()
            .map(str::to_owned)
            .collect();
        let again: Vec<_> = methods(&Identity::of(&fingerprint))
            .into_iter()
            .map(str::to_owned)
            .collect();

        assert_eq!(first, again);
        let overrides = &first[2..];
        let mut sorted = overrides.to_vec();
        sorted.sort();
        assert_eq!(overrides, sorted, "overrides are sorted by command name");
    }

    #[test]
    fn the_proxy_password_is_never_turned_into_a_header() {
        let mut fingerprint = Fingerprint::randomized(OsType::Linux, 3);
        fingerprint.proxy = Some(ProxyConfig {
            r#type: "http".to_owned(),
            host: "proxy.example.com".to_owned(),
            port: 8080,
            username: Some("alice".to_owned()),
            password: Some("fake-proxy-password".to_owned()),
            save_traffic: false,
        });
        // The WebDriver path would send it; confirm the source does ask for it.
        assert!(cdp_overrides(&fingerprint).contains_key(CREDENTIAL_HEADER));

        let identity = Identity::of(&fingerprint);

        assert!(!methods(&identity).contains(&CREDENTIAL_HEADER));
        let everything = format!(
            "{:?}{:?}",
            identity.tab_commands(),
            identity.permission_grant(None)
        );
        assert!(!everything.contains("fake-proxy-password"));
        assert!(!everything.contains("Proxy-Authorization"));
    }

    #[test]
    fn permissions_are_held_back_for_the_tabs_own_context() {
        let mut fingerprint = Fingerprint::randomized(OsType::Windows, 5);
        fingerprint.flags.grant_permissions = true;

        let identity = Identity::of(&fingerprint);

        assert!(
            !methods(&identity).contains(&PERMISSIONS),
            "not sent to the tab session"
        );
        let grant = identity.permission_grant(Some("CTX1")).expect("a grant");
        assert_eq!(grant["browserContextId"], "CTX1");
        assert!(grant["permissions"]
            .as_array()
            .is_some_and(|p| !p.is_empty()));
        assert!(identity
            .permission_grant(None)
            .unwrap()
            .get("browserContextId")
            .is_none());
    }

    #[test]
    fn no_grant_is_made_unless_the_fingerprint_asks() {
        let mut fingerprint = Fingerprint::randomized(OsType::Windows, 5);
        fingerprint.flags.grant_permissions = false;

        assert!(Identity::of(&fingerprint)
            .permission_grant(Some("CTX1"))
            .is_none());
    }

    #[test]
    fn debug_output_stays_short_and_secret_free() {
        let shown = format!(
            "{:?}",
            Identity::of(&Fingerprint::randomized(OsType::Windows, 1))
        );

        assert!(shown.len() < 200, "{shown}");
        assert!(shown.contains("tab_commands"));
    }
}

#[cfg(test)]
mod launch_tests {
    use std::path::Path;

    use super::super::LaunchOptions;
    use crate::stealth::fingerprint::{OsType, ProxyConfig, ProxyMaskingMode};

    use super::*;

    fn args(builder: super::super::LaunchOptionsBuilder) -> Vec<String> {
        builder
            .build()
            .unwrap()
            .browser_args(Path::new("/tmp/profile"))
    }

    #[test]
    fn a_fingerprint_sets_the_launch_arguments_it_can() {
        let fingerprint = Fingerprint::randomized(OsType::Windows, 42);

        let args = args(LaunchOptions::builder().fingerprint(&fingerprint));

        let ua = fingerprint.user_agent.as_deref().unwrap();
        assert!(args.contains(&format!("--user-agent={ua}")));
        let locale = fingerprint.locale.as_deref().unwrap();
        assert!(args.contains(&format!("--lang={locale}")));
        let (w, h) = (
            fingerprint.screen_width.unwrap(),
            fingerprint.screen_height.unwrap(),
        );
        assert!(args.contains(&format!("--window-size={w},{h}")));
        assert!(
            args.iter()
                .any(|a| a.starts_with("--force-webrtc-ip-handling-policy=")),
            "{args:?}"
        );
    }

    #[test]
    fn a_setting_made_before_the_fingerprint_wins_and_one_made_after_replaces_it() {
        let fingerprint = Fingerprint::randomized(OsType::Linux, 9);

        let before = args(
            LaunchOptions::builder()
                .user_agent("mine/1.0")
                .fingerprint(&fingerprint),
        );
        let after = args(
            LaunchOptions::builder()
                .fingerprint(&fingerprint)
                .user_agent("mine/2.0"),
        );

        assert!(before.contains(&"--user-agent=mine/1.0".to_owned()));
        assert!(after.contains(&"--user-agent=mine/2.0".to_owned()));
        for list in [&before, &after] {
            assert_eq!(
                list.iter()
                    .filter(|a| a.starts_with("--user-agent="))
                    .count(),
                1,
                "only one user agent is passed"
            );
        }
    }

    #[test]
    fn free_form_chrome_flags_in_a_fingerprint_are_not_passed_on() {
        let mut fingerprint = Fingerprint::randomized(OsType::Linux, 9);
        fingerprint
            .cmd_params
            .insert("renderer-cmd-prefix".into(), "/bin/sh -c evil".into());

        let args = args(LaunchOptions::builder().fingerprint(&fingerprint));

        assert!(
            !args
                .iter()
                .any(|a| a.contains("renderer-cmd-prefix") || a.contains("evil")),
            "{args:?}"
        );
    }

    #[test]
    fn the_fingerprints_proxy_is_used_only_when_it_asks_and_none_was_set() {
        let mut fingerprint = Fingerprint::randomized(OsType::Linux, 2);
        fingerprint.proxy = Some(ProxyConfig {
            r#type: "http".to_owned(),
            host: "fp-proxy.example.com".to_owned(),
            port: 3128,
            username: Some("alice".to_owned()),
            password: Some("fake-fp-password".to_owned()),
            save_traffic: false,
        });
        fingerprint.flags.proxy_masking = ProxyMaskingMode::Custom;

        let from_fingerprint = LaunchOptions::builder()
            .fingerprint(&fingerprint)
            .build()
            .unwrap();
        let explicit = LaunchOptions::builder()
            .proxy("explicit.example.com:8080")
            .fingerprint(&fingerprint)
            .build()
            .unwrap();
        fingerprint.flags.proxy_masking = ProxyMaskingMode::Disabled;
        let not_asked = LaunchOptions::builder()
            .fingerprint(&fingerprint)
            .build()
            .unwrap();

        assert!(from_fingerprint
            .proxy()
            .unwrap()
            .server()
            .contains("fp-proxy.example.com"));
        assert!(explicit
            .proxy()
            .unwrap()
            .server()
            .contains("explicit.example.com"));
        assert!(not_asked.proxy().is_none());
    }

    #[test]
    fn neither_the_builder_nor_the_options_print_a_proxy_password() {
        let mut fingerprint = Fingerprint::randomized(OsType::Linux, 2);
        fingerprint.proxy = Some(ProxyConfig {
            r#type: "http".to_owned(),
            host: "fp-proxy.example.com".to_owned(),
            port: 3128,
            username: Some("alice".to_owned()),
            password: Some("fake-fp-password".to_owned()),
            save_traffic: false,
        });
        fingerprint.flags.proxy_masking = ProxyMaskingMode::Custom;
        let builder = LaunchOptions::builder()
            .proxy("bob:fake-explicit-password@host.example.com:8080")
            .fingerprint(&fingerprint);

        let shown = format!("{builder:?}");
        let options = builder.build().unwrap();
        let shown_options = format!("{options:?}");

        for text in [&shown, &shown_options] {
            assert!(!text.contains("fake-explicit-password"), "{text}");
            assert!(!text.contains("fake-fp-password"), "{text}");
        }
    }
}
