use serde::{Deserialize, Serialize};

/// WebRTC IP handling policy for Chromium (`Emulation.setWebRTCIPHandlingPolicy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WebRtcPolicy {
    /// Disable non-proxied UDP (strictest; recommended when using a proxy).
    #[default]
    DisableNonProxiedUdp,
    /// Allow default public and private interfaces.
    PublicAndPrivateInterfaces,
    /// Allow only the default public interface.
    PublicInterfaceOnly,
}
