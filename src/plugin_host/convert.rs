//! Conversions between the plugin API's launch settings and this crate's own
//! types, shared by both engines.

use seleniumbase_rs_plugin::WebRtcPolicy as ApiPolicy;

use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::WebRtcPolicy;

/// This crate's WebRTC policy as the plugin API names it.
pub(crate) fn webrtc_to_api(policy: WebRtcPolicy) -> ApiPolicy {
    match policy {
        WebRtcPolicy::DisableNonProxiedUdp => ApiPolicy::DisableNonProxiedUdp,
        WebRtcPolicy::PublicAndPrivateInterfaces => ApiPolicy::DefaultPublicAndPrivateInterfaces,
        WebRtcPolicy::PublicInterfaceOnly => ApiPolicy::DefaultPublicInterfaceOnly,
    }
}

/// The policy a plugin set, as this crate applies it: `None` for Chrome's own
/// default, which needs no switch.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] for a policy newer than this crate
/// knows (the API's enum is `#[non_exhaustive]`).
pub(crate) fn webrtc_from_api(
    policy: ApiPolicy,
) -> Result<Option<WebRtcPolicy>, SeleniumBaseError> {
    match policy {
        ApiPolicy::Default => Ok(None),
        ApiPolicy::DisableNonProxiedUdp => Ok(Some(WebRtcPolicy::DisableNonProxiedUdp)),
        ApiPolicy::DefaultPublicAndPrivateInterfaces => {
            Ok(Some(WebRtcPolicy::PublicAndPrivateInterfaces))
        }
        ApiPolicy::DefaultPublicInterfaceOnly => Ok(Some(WebRtcPolicy::PublicInterfaceOnly)),
        other => Err(SeleniumBaseError::invalid_config(format!(
            "the WebRTC policy {other:?} a plugin set is not one this version of \
             seleniumbase-rs can apply"
        ))),
    }
}

/// The Chrome switch for the policy a plugin set, or `None` for Chrome's own
/// default.
///
/// # Errors
///
/// As [`webrtc_from_api`].
pub(crate) fn webrtc_switch(policy: ApiPolicy) -> Result<Option<&'static str>, SeleniumBaseError> {
    Ok(webrtc_from_api(policy)?.map(crate::sb_cdp::webrtc_chrome_flag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_policy_this_crate_has_survives_the_round_trip() {
        for policy in [
            WebRtcPolicy::DisableNonProxiedUdp,
            WebRtcPolicy::PublicAndPrivateInterfaces,
            WebRtcPolicy::PublicInterfaceOnly,
        ] {
            assert_eq!(
                webrtc_from_api(webrtc_to_api(policy)).unwrap(),
                Some(policy)
            );
        }
    }

    #[test]
    fn chromes_default_needs_no_switch() {
        assert_eq!(webrtc_from_api(ApiPolicy::Default).unwrap(), None);
    }
}
