//! The proxy a browser uses.

use serde::{Deserialize, Serialize};

use super::ProxyBypassList;

/// The proxy of a browser: the server, a proxy auto-config script, and the hosts that skip
/// the proxy.
///
/// Both engines read it, so it belongs to the [`BrowserSettings`] they share.
///
/// [`BrowserSettings`]: super::BrowserSettings
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProxySettings {
    /// The proxy server, written `[scheme://][user:password@]host:port` (`--proxy`,
    /// `SB_PROXY`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// A proxy auto-config script the browser fetches to choose its proxy
    /// (`--proxy-pac-url`, `SB_PROXY_PAC_URL`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pac_url: Option<String>,
    /// Hosts that skip the proxy (`--proxy-bypass-list`, `SB_PROXY_BYPASS_LIST`).
    #[serde(skip_serializing_if = "ProxyBypassList::is_empty")]
    pub bypass_list: ProxyBypassList,
}

// Written by hand: the server and the script's address may carry a password.
impl std::fmt::Debug for ProxySettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::browser::options::chromium::without_credentials;
        f.debug_struct("ProxySettings")
            .field("server", &self.server.as_deref().map(without_credentials))
            .field("pac_url", &self.pac_url.as_deref().map(without_credentials))
            .field("bypass_list", &self.bypass_list)
            .finish()
    }
}
