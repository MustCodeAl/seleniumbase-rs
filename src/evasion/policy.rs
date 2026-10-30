//! The injection policy of a fingerprint: behaviour that is not a per-setting
//! value.
//!
//! A [`Fingerprint`](crate::evasion::fingerprint::Fingerprint) holds two kinds
//! of things. The settings (a time zone, a screen, a position) are what the
//! browser should *claim*, one [`Spoof`](crate::evasion::spoof::Spoof) each. The
//! [`StealthPolicy`] is *how* the claims are delivered and which protections run
//! beside them: whether `Function.prototype.toString` is masked, whether canvas
//! reads are noised, whether permissions are granted up front, which ports are
//! blocked.
//!
//! A policy preset such as [`StealthPolicy::balanced`] never changes a setting.
//! It is given to the fingerprint builder, in any order against the settings.
//!
//! ```
//! use seleniumbase_rs::evasion::fingerprint::{CanvasNoise, StealthPolicy};
//!
//! let policy = StealthPolicy::balanced().canvas_noise(CanvasNoise::Random);
//! assert_eq!(policy.canvas_noise, CanvasNoise::Random);
//! assert!(policy.chrome_runtime);
//! ```

use serde::{Deserialize, Serialize};

use super::fingerprint::{QuicMode, WebRtcPolicy};

/// How canvas reads (`toDataURL`, `toBlob`, `getImageData`) are noised.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum CanvasNoise {
    /// No noise: the browser's own canvas.
    #[default]
    #[serde(alias = "Disabled", alias = "Auto")]
    Natural,
    /// Deterministic noise from the fingerprint's seed, the same on every read
    /// and every launch.
    #[serde(alias = "Mask")]
    Persistent,
    /// Noise from a fresh seed each time the script is generated, so each
    /// browser context gets different noise.
    Random,
    /// Seeded noise on about one pixel in eight.
    Low,
}

impl CanvasNoise {
    /// Whether this adds noise at all.
    #[must_use]
    pub const fn adds_noise(self) -> bool {
        !matches!(self, Self::Natural)
    }

    /// How sparse the noise is: it touches one pixel in this many.
    #[must_use]
    pub const fn density(self) -> u32 {
        match self {
            Self::Low => 8,
            _ => 1,
        }
    }
}

/// How audio reads (`AudioBuffer`, `AnalyserNode`) are noised.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum AudioNoise {
    /// No noise: the browser's own audio.
    #[default]
    #[serde(alias = "Disabled", alias = "Auto")]
    Natural,
    /// Deterministic noise from the fingerprint's seed.
    #[serde(alias = "Mask")]
    Persistent,
    /// Noise from a fresh seed each time the script is generated.
    Random,
}

impl AudioNoise {
    /// Whether this adds noise at all.
    #[must_use]
    pub const fn adds_noise(self) -> bool {
        !matches!(self, Self::Natural)
    }
}

/// How a fingerprint's claims are delivered and which protections run with
/// them. See the [module documentation](self).
///
/// The fields can be read; a policy is changed by the methods of the same
/// names, which take and return the policy, or through the builder
/// ([`FingerprintBuilder::policy`](crate::evasion::fingerprint::FingerprintBuilder::policy),
/// [`with_policy`](crate::evasion::fingerprint::FingerprintBuilder::with_policy)
/// and its shortcuts).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::evasion::fingerprint::{CanvasNoise, StealthPolicy};
///
/// let policy = StealthPolicy::balanced()
///     .canvas_noise(CanvasNoise::Low)
///     .grant_permissions(true);
/// assert!(policy.native_tostring && policy.chrome_runtime);
/// assert_eq!(policy.canvas_noise, CanvasNoise::Low);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(default)]
#[non_exhaustive]
pub struct StealthPolicy {
    /// The WebRTC IP-handling policy the browser is launched with. `None`
    /// passes no policy flag unless the fingerprint sets WebRTC, which uses
    /// [`WebRtcPolicy::DisableNonProxiedUdp`].
    pub webrtc_policy: Option<WebRtcPolicy>,
    /// Canvas read-back noise.
    pub canvas_noise: CanvasNoise,
    /// Audio read-back noise.
    pub audio_noise: AudioNoise,
    /// Apply spoofing through CDP and launch arguments instead of JavaScript
    /// where possible. The browser's own implementation then answers with the
    /// fake value, so the spoof is invisible to `toString` and
    /// `getOwnPropertyNames`.
    pub native_spoofing: bool,
    /// Mask `Function.prototype.toString` so patched natives look native. On by
    /// default, because the other evasions rely on it.
    pub native_tostring: bool,
    /// Install the `window.chrome` runtime, app, `csi` and `loadTimes` stubs.
    pub chrome_runtime: bool,
    /// Apply the headless fixes: `matchMedia`, permissions consistency, page
    /// visibility and the hairline media query.
    pub headless_fixes: bool,
    /// Grant common browser permissions at start so permission prompts, a
    /// bot-detection signal, do not fire.
    pub grant_permissions: bool,
    /// Block the tracker and fingerprinting-script hosts.
    pub block_trackers: bool,
    /// The hosts to block when [`block_trackers`](Self::block_trackers) is on.
    /// Empty means the crate's default list.
    pub blocked_trackers: Vec<String>,
    /// Ports the browser may not reach on the loopback names.
    pub blocked_ports: Vec<u16>,
    /// How QUIC is handled.
    #[serde(alias = "quic_mode")]
    pub quic: QuicMode,
    /// Bypass Content-Security-Policy for the page context.
    pub disable_csp: bool,
    /// Force every closed shadow root open, so automation can reach into it. A
    /// page can detect this, so turn it off where that matters. On by default.
    pub open_shadow_roots: bool,
}

impl Default for StealthPolicy {
    /// Only what the other evasions rely on: `toString` masked and shadow roots
    /// opened. Nothing is noised, blocked or granted.
    fn default() -> Self {
        Self {
            webrtc_policy: None,
            canvas_noise: CanvasNoise::Natural,
            audio_noise: AudioNoise::Natural,
            native_spoofing: false,
            native_tostring: true,
            chrome_runtime: false,
            headless_fixes: false,
            grant_permissions: false,
            block_trackers: false,
            blocked_trackers: Vec::new(),
            blocked_ports: Vec::new(),
            quic: QuicMode::Natural,
            disable_csp: false,
            open_shadow_roots: true,
        }
    }
}

impl StealthPolicy {
    /// The policy of the built-in presets: the default, plus a WebRTC leak
    /// policy, seeded canvas and audio noise, the `window.chrome` stubs and the
    /// headless fixes.
    #[must_use]
    pub fn balanced() -> Self {
        Self {
            webrtc_policy: Some(WebRtcPolicy::DisableNonProxiedUdp),
            canvas_noise: CanvasNoise::Persistent,
            audio_noise: AudioNoise::Persistent,
            chrome_runtime: true,
            headless_fixes: true,
            ..Self::default()
        }
    }

    /// Delivers the claims natively (through CDP and launch arguments) wherever
    /// the browser can, so the page finds no JavaScript override to inspect.
    /// Nothing is noised and the JavaScript-only stubs stay off.
    #[must_use]
    pub fn native_only() -> Self {
        Self {
            webrtc_policy: Some(WebRtcPolicy::DisableNonProxiedUdp),
            native_spoofing: true,
            ..Self::default()
        }
    }

    /// Every protection: [`balanced`](Self::balanced), plus tracker blocking,
    /// granted permissions and QUIC off.
    #[must_use]
    pub fn max() -> Self {
        Self {
            block_trackers: true,
            grant_permissions: true,
            quic: QuicMode::Disabled,
            ..Self::balanced()
        }
    }

    /// Sets the WebRTC IP-handling policy; `None` passes no policy flag.
    #[must_use]
    pub fn webrtc_policy(mut self, policy: impl Into<Option<WebRtcPolicy>>) -> Self {
        self.webrtc_policy = policy.into();
        self
    }

    /// Sets the canvas noise.
    #[must_use]
    pub fn canvas_noise(mut self, noise: CanvasNoise) -> Self {
        self.canvas_noise = noise;
        self
    }

    /// Sets the audio noise.
    #[must_use]
    pub fn audio_noise(mut self, noise: AudioNoise) -> Self {
        self.audio_noise = noise;
        self
    }

    /// Switches native (CDP and launch argument) spoofing on or off.
    #[must_use]
    pub fn native_spoofing(mut self, on: bool) -> Self {
        self.native_spoofing = on;
        self
    }

    /// Switches the `Function.prototype.toString` mask on or off.
    #[must_use]
    pub fn native_tostring(mut self, on: bool) -> Self {
        self.native_tostring = on;
        self
    }

    /// Switches the `window.chrome` stubs on or off.
    #[must_use]
    pub fn chrome_runtime(mut self, on: bool) -> Self {
        self.chrome_runtime = on;
        self
    }

    /// Switches the headless fixes on or off.
    #[must_use]
    pub fn headless_fixes(mut self, on: bool) -> Self {
        self.headless_fixes = on;
        self
    }

    /// Grants common permissions at start, or not.
    #[must_use]
    pub fn grant_permissions(mut self, on: bool) -> Self {
        self.grant_permissions = on;
        self
    }

    /// Blocks the default tracker hosts, or stops blocking. Turning it off also
    /// drops a custom list.
    #[must_use]
    pub fn block_trackers(mut self, on: bool) -> Self {
        self.block_trackers = on;
        if !on {
            self.blocked_trackers.clear();
        }
        self
    }

    /// Blocks these hosts instead of the default ones. A non-empty list is
    /// itself the request to block them; an empty one is the default list.
    #[must_use]
    pub fn blocked_trackers(mut self, hosts: Vec<String>) -> Self {
        if !hosts.is_empty() {
            self.block_trackers = true;
        }
        self.blocked_trackers = hosts;
        self
    }

    /// Sets the ports the browser may not reach on the loopback names.
    #[must_use]
    pub fn blocked_ports(mut self, ports: Vec<u16>) -> Self {
        self.blocked_ports = ports;
        self
    }

    /// Sets how QUIC is handled.
    #[must_use]
    pub fn quic(mut self, mode: QuicMode) -> Self {
        self.quic = mode;
        self
    }

    /// Bypasses Content-Security-Policy, or not.
    #[must_use]
    pub fn disable_csp(mut self, on: bool) -> Self {
        self.disable_csp = on;
        self
    }

    /// Opens closed shadow roots, or not.
    #[must_use]
    pub fn open_shadow_roots(mut self, on: bool) -> Self {
        self.open_shadow_roots = on;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_policy_claims_nothing_but_the_tostring_mask_and_open_shadow_roots() {
        let policy = StealthPolicy::default();

        assert!(policy.native_tostring && policy.open_shadow_roots);
        assert_eq!(policy.canvas_noise, CanvasNoise::Natural);
        assert_eq!(policy.audio_noise, AudioNoise::Natural);
        assert_eq!(policy.webrtc_policy, None);
        assert!(!policy.native_spoofing && !policy.grant_permissions && !policy.block_trackers);
        assert!(!policy.chrome_runtime && !policy.headless_fixes && !policy.disable_csp);
        assert!(policy.blocked_ports.is_empty() && policy.blocked_trackers.is_empty());
    }

    #[test]
    fn balanced_adds_the_leak_policy_noise_and_the_stubs() {
        let policy = StealthPolicy::balanced();

        assert_eq!(
            policy.webrtc_policy,
            Some(WebRtcPolicy::DisableNonProxiedUdp)
        );
        assert_eq!(policy.canvas_noise, CanvasNoise::Persistent);
        assert_eq!(policy.audio_noise, AudioNoise::Persistent);
        assert!(policy.chrome_runtime && policy.headless_fixes && policy.native_tostring);
        assert!(!policy.native_spoofing && !policy.block_trackers);
    }

    #[test]
    fn native_only_delivers_natively_and_adds_no_javascript_stubs() {
        let policy = StealthPolicy::native_only();

        assert!(policy.native_spoofing);
        assert!(!policy.chrome_runtime && !policy.headless_fixes);
        assert_eq!(policy.canvas_noise, CanvasNoise::Natural);
    }

    #[test]
    fn max_is_balanced_plus_trackers_permissions_and_no_quic() {
        let policy = StealthPolicy::max();

        assert!(policy.block_trackers && policy.grant_permissions);
        assert_eq!(policy.quic, QuicMode::Disabled);
        assert_eq!(policy.canvas_noise, CanvasNoise::Persistent);
    }

    #[test]
    fn a_setter_changes_one_field_and_keeps_the_rest() {
        let policy = StealthPolicy::balanced().grant_permissions(true);

        assert!(policy.grant_permissions);
        assert_eq!(policy.canvas_noise, CanvasNoise::Persistent);
        assert_eq!(
            policy.webrtc_policy(None),
            StealthPolicy {
                webrtc_policy: None,
                grant_permissions: true,
                ..StealthPolicy::balanced()
            }
        );
    }

    #[test]
    fn a_tracker_list_is_a_request_to_block_and_turning_the_block_off_drops_it() {
        let listed = StealthPolicy::default().blocked_trackers(vec!["metrics.test".to_owned()]);
        assert!(listed.block_trackers);

        let off = listed.block_trackers(false);
        assert!(!off.block_trackers);
        assert!(off.blocked_trackers.is_empty());

        let empty = StealthPolicy::default().blocked_trackers(Vec::new());
        assert!(!empty.block_trackers, "an empty list asks for nothing");
    }

    #[test]
    fn noise_modes_say_whether_they_add_noise_and_how_dense() {
        assert!(!CanvasNoise::Natural.adds_noise());
        assert!(CanvasNoise::Persistent.adds_noise());
        assert!(CanvasNoise::Random.adds_noise() && CanvasNoise::Low.adds_noise());
        assert_eq!(CanvasNoise::Low.density(), 8);
        assert_eq!(CanvasNoise::Persistent.density(), 1);
        assert!(!AudioNoise::Natural.adds_noise() && AudioNoise::Random.adds_noise());
    }

    #[test]
    fn a_policy_saved_with_the_old_flag_words_still_loads() {
        let raw = r#"{
            "canvas_noise": "Mask",
            "quic_mode": "Disabled",
            "grant_permissions": true,
            "navigator_masking": "Custom"
        }"#;

        let policy: StealthPolicy = serde_json::from_str(raw).unwrap();

        assert_eq!(policy.canvas_noise, CanvasNoise::Persistent);
        assert_eq!(policy.quic, QuicMode::Disabled);
        assert!(policy.grant_permissions);
        assert!(
            policy.native_tostring,
            "a key the file lacks is the default"
        );
    }

    #[test]
    fn the_old_disabled_and_auto_canvas_words_are_natural() {
        for word in ["Disabled", "Natural", "Auto"] {
            let text = format!("\"{word}\"");
            assert_eq!(
                serde_json::from_str::<CanvasNoise>(&text).unwrap(),
                CanvasNoise::Natural
            );
        }
    }

    #[test]
    fn a_policy_round_trips_through_json() {
        let policy = StealthPolicy::max().blocked_ports(vec![9222]);

        let text = serde_json::to_string(&policy).unwrap();

        assert_eq!(
            serde_json::from_str::<StealthPolicy>(&text).unwrap(),
            policy
        );
    }
}
