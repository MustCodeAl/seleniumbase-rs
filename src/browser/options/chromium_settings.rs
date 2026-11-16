//! Chromium command-line settings that have no switch of their own.

use serde::{Deserialize, Serialize};

/// The Chromium command-line settings that have no switch of their own.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChromiumSettings {
    /// Extra command-line arguments (`--chromium-arg`, `SB_CHROMIUM_ARG`), each written with
    /// its leading `--`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Chromium features to switch off (`--disable-features`), joined with the session's own
    /// into the one switch Chrome reads.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub disable_features: Vec<String>,
    /// Rules that map host names to addresses (`--host-resolver-rules`), sent as one
    /// Chromium switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_resolver_rules: Option<String>,
}
