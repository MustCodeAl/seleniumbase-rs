//! The settings a launch is asked for, kept in layers, and the one rule that turns them into
//! the options a launch reads.
//!
//! [`LaunchOptionsBuilder`](super::LaunchOptionsBuilder) writes the settings of the caller into
//! one [`Layer`]. [`browser_config`](super::LaunchOptionsBuilder::browser_config) fills a second
//! with what a [`BrowserConfig`](crate::BrowserConfig) says, and a fingerprint implies a third.
//! The layers are merged when the options are built, by one rule that does not depend on the
//! order of the calls:
//!
//! > A value set on the builder wins; else the value of the `BrowserConfig`; else the
//! > fingerprint's; else the default.
//!
//! The lists of values (arguments, extensions, preferences, features) are added together, the
//! config's first and the builder's after it, so the builder's arguments come last and Chrome
//! keeps them. A switch the builder turns on or off is decided by the builder.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value};

use super::{
    bridge, proxy_of, BrowserSource, LaunchOptions, ProfileMode, Proxy, Redacted,
    DEFAULT_STARTUP_TIMEOUT,
};
use crate::browser::options::{
    chrome_preferences, emulated_phone, DeviceMetrics, LaunchSwitch, PacUrl, ProxyBypassList,
    VirtualDisplay, MOBILE_USER_AGENT,
};
use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::{Dimension, Fingerprint, WebRtcPolicy};
use crate::sb_cdp::identity::StealthPlan;

/// A fingerprint the layers and the options built from them share. Its `Debug` is short: the
/// fingerprint is long, and the plan made from it is what the options show.
#[derive(Clone)]
pub(super) struct SharedFingerprint(pub(super) Arc<Fingerprint>);

impl fmt::Debug for SharedFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Fingerprint(..)")
    }
}

/// A proxy as the caller wrote it, and whether it is read as Python reads one (when it came from
/// a `BrowserConfig`) or as the pure CDP engine does.
#[derive(Debug, Clone)]
pub(super) struct ProxySpec {
    pub(super) text: Redacted,
    pub(super) checked: bool,
}

/// The settings of one layer. Each is unset until something sets it, so that layers can be
/// merged (`Layer::over`).
#[derive(Debug, Clone, Default)]
pub(super) struct Layer {
    pub(super) url: Option<String>,
    pub(super) cdp_url: Option<Redacted>,
    pub(super) cdp_target: Option<String>,
    pub(super) headless: Option<bool>,
    pub(super) use_chromium: Option<bool>,
    pub(super) executable_path: Option<PathBuf>,
    pub(super) ad_block: Option<bool>,
    pub(super) proxy: Option<ProxySpec>,
    pub(super) user_data_dir: Option<PathBuf>,
    pub(super) user_agent: Option<String>,
    pub(super) lang: Option<String>,
    pub(super) window_size: Option<(u32, u32)>,
    pub(super) window_position: Option<(i32, i32)>,
    pub(super) webrtc: Option<WebRtcPolicy>,
    pub(super) shield_webrtc: Option<bool>,
    pub(super) fingerprint: Option<SharedFingerprint>,
    pub(super) mobile: Option<bool>,
    pub(super) device_metrics: Option<DeviceMetrics>,
    pub(super) proxy_bypass_list: Option<ProxyBypassList>,
    pub(super) proxy_pac_url: Option<Redacted>,
    pub(super) virtual_display: Option<VirtualDisplay>,
    pub(super) startup_timeout: Option<Duration>,
    pub(super) debugging_port: Option<u16>,
    #[cfg(feature = "plugins")]
    pub(super) plugins: Option<seleniumbase_rs_plugin::PluginSet>,
    pub(super) extra_args: Vec<String>,
    pub(super) extensions: Vec<PathBuf>,
    pub(super) prefs: Map<String, Value>,
    /// The switches this layer turns on or off, which a lower layer cannot undo.
    pub(super) switches: BTreeMap<LaunchSwitch, bool>,
    pub(super) disable_features: Vec<String>,
    pub(super) enable_features: Vec<String>,
    pub(super) host_resolver_rules: Vec<String>,
    /// Plugins named rather than built, added to the set by `resolve`.
    #[cfg(feature = "plugins")]
    pub(super) selection: crate::config::plugins::PluginSelection,
}

/// `lower`, then `upper`, as one list.
fn joined<T>(mut lower: Vec<T>, upper: Vec<T>) -> Vec<T> {
    lower.extend(upper);
    lower
}

impl Layer {
    /// This layer over `lower`: each setting is this layer's when it has one, else `lower`'s.
    /// The lists of both are kept, `lower`'s first.
    pub(super) fn over(self, lower: Self) -> Self {
        let mut prefs = lower.prefs;
        prefs.extend(self.prefs);
        let mut switches = lower.switches;
        switches.extend(self.switches);
        #[cfg(feature = "plugins")]
        let mut selection = lower.selection;
        #[cfg(feature = "plugins")]
        selection.extend(&self.selection);
        Self {
            url: self.url.or(lower.url),
            cdp_url: self.cdp_url.or(lower.cdp_url),
            cdp_target: self.cdp_target.or(lower.cdp_target),
            headless: self.headless.or(lower.headless),
            use_chromium: self.use_chromium.or(lower.use_chromium),
            executable_path: self.executable_path.or(lower.executable_path),
            ad_block: self.ad_block.or(lower.ad_block),
            proxy: self.proxy.or(lower.proxy),
            user_data_dir: self.user_data_dir.or(lower.user_data_dir),
            user_agent: self.user_agent.or(lower.user_agent),
            lang: self.lang.or(lower.lang),
            window_size: self.window_size.or(lower.window_size),
            window_position: self.window_position.or(lower.window_position),
            webrtc: self.webrtc.or(lower.webrtc),
            shield_webrtc: self.shield_webrtc.or(lower.shield_webrtc),
            fingerprint: self.fingerprint.or(lower.fingerprint),
            mobile: self.mobile.or(lower.mobile),
            device_metrics: self.device_metrics.or(lower.device_metrics),
            proxy_bypass_list: self.proxy_bypass_list.or(lower.proxy_bypass_list),
            proxy_pac_url: self.proxy_pac_url.or(lower.proxy_pac_url),
            virtual_display: self.virtual_display.or(lower.virtual_display),
            startup_timeout: self.startup_timeout.or(lower.startup_timeout),
            debugging_port: self.debugging_port.or(lower.debugging_port),
            #[cfg(feature = "plugins")]
            plugins: self.plugins.or(lower.plugins),
            extra_args: joined(lower.extra_args, self.extra_args),
            extensions: joined(lower.extensions, self.extensions),
            prefs,
            switches,
            disable_features: joined(lower.disable_features, self.disable_features),
            enable_features: joined(lower.enable_features, self.enable_features),
            host_resolver_rules: joined(lower.host_resolver_rules, self.host_resolver_rules),
            #[cfg(feature = "plugins")]
            selection,
        }
    }

    /// What a fingerprint implies for a launch: its user agent, language, window size, WebRTC
    /// policy and proxy, as the settings that are lowest in the rule.
    fn of_fingerprint(fingerprint: &Fingerprint) -> Self {
        Self {
            user_agent: fingerprint.user_agent().value().cloned(),
            lang: fingerprint.locale().value().cloned(),
            window_size: fingerprint
                .screen()
                .value()
                .map(|size| (size.width, size.height)),
            // Only a fingerprint that masks WebRTC passes the policy flag, which a policy, an
            // address or turning WebRTC off makes it; an empty one leaves Chrome's own
            // behaviour alone.
            webrtc: fingerprint
                .is_masked(Dimension::WebRtc)
                .then(|| fingerprint.policy().webrtc_policy.unwrap_or_default()),
            proxy: proxy_of(fingerprint).map(|url| ProxySpec {
                text: Redacted(url),
                checked: false,
            }),
            ..Self::default()
        }
    }

    /// Turns one of the library's on/off launch settings on or off in this layer. See
    /// [`LaunchOptionsBuilder::switch`](super::LaunchOptionsBuilder::switch) for which ones the
    /// pure CDP engine uses.
    pub(super) fn set_switch(&mut self, switch: LaunchSwitch, on: bool) {
        match switch {
            LaunchSwitch::Incognito
            | LaunchSwitch::Guest
            | LaunchSwitch::NoSandbox
            | LaunchSwitch::BlockImages
            | LaunchSwitch::DisableJavaScript
            | LaunchSwitch::DisableCookies
            | LaunchSwitch::DoNotTrack
            | LaunchSwitch::ExternalPdf
            | LaunchSwitch::DisableCsp
            | LaunchSwitch::DarkMode
            | LaunchSwitch::Devtools
            | LaunchSwitch::DisableWebSecurity
            | LaunchSwitch::Swiftshader
            | LaunchSwitch::Maximize
            | LaunchSwitch::DisableGpu => {
                self.switches.insert(switch, on);
            }
            LaunchSwitch::Enable3dApis => {}
            other => {
                tracing::debug!("{other:?} is not used by the pure CDP engine");
            }
        }
    }
}

/// The browser one layer asks for: a binary, Chromium, or (when asked not to) Chrome. The two
/// settings go together, so a layer that names a browser replaces the one a lower layer names.
fn browser_of(layer: &Layer) -> Option<BrowserSource> {
    match (&layer.executable_path, layer.use_chromium) {
        (Some(path), _) => Some(BrowserSource::Path(path.clone())),
        (None, Some(true)) => Some(BrowserSource::Chromium),
        (None, Some(false)) => Some(BrowserSource::Chrome),
        (None, None) => None,
    }
}

/// The layers of a launch: what the builder was given, and what its `browser_config` read.
/// A fingerprint is one of the settings of either.
#[derive(Debug, Clone, Default)]
pub(super) struct Layers {
    pub(super) set: Layer,
    pub(super) config: Layer,
}

/// What the layers say once they are merged, and what follows from it.
struct View {
    /// The layers merged by the rule, the fingerprint's included.
    merged: Layer,
    /// The builder's and the config's layers merged: what was asked for by name.
    asked: Layer,
    fingerprint: Option<SharedFingerprint>,
    /// Whether the phone is asked for by name (`mobile`, a screen of the builder, a config's
    /// `mobile`), and not switched off.
    mobile: bool,
    /// The screen of the phone, unless it is switched off.
    metrics: Option<DeviceMetrics>,
    source: BrowserSource,
}

impl View {
    fn of(layers: &Layers) -> Self {
        let asked = layers.set.clone().over(layers.config.clone());
        let fingerprint = asked.fingerprint.clone();
        let merged = match &fingerprint {
            Some(fingerprint) => asked.clone().over(Layer::of_fingerprint(&fingerprint.0)),
            None => asked.clone(),
        };
        // `mobile(false)` of the builder turns the phone off whatever else asks for one.
        let switched_off = asked.mobile == Some(false);
        let metrics = if switched_off {
            None
        } else {
            asked.device_metrics
        };
        let mobile =
            !switched_off && (asked.mobile == Some(true) || layers.set.device_metrics.is_some());
        let source = browser_of(&layers.set)
            .or_else(|| browser_of(&layers.config))
            .unwrap_or(BrowserSource::Chrome);
        Self {
            merged,
            asked,
            fingerprint,
            mobile,
            metrics,
            source,
        }
    }

    /// Whether the phone is on, with a screen of its own or the default one.
    fn phone(&self) -> bool {
        self.mobile || self.metrics.is_some()
    }

    /// Whether the switch is on in the merged layers.
    fn switch_on(&self, switch: LaunchSwitch) -> Option<bool> {
        self.merged.switches.get(&switch).copied()
    }

    fn check_phone_and_fingerprint(&self) -> Result<(), SeleniumBaseError> {
        if self.fingerprint.is_some() && self.phone() {
            return Err(SeleniumBaseError::invalid_config(
                "mobile emulation and a fingerprint both set the screen and the user agent; \
                 use one of them",
            ));
        }
        Ok(())
    }

    /// Refuses what a browser that is only attached to cannot be given.
    ///
    /// The browser is the app's. A proxy would need a launch flag (and a login would turn on
    /// request interception in every tab with nothing to answer it, hanging the app's own
    /// requests). A user agent or locale given as a launch flag would be dropped without a
    /// word, so it is only taken when a fingerprint (or a phone, for the user agent) applies it
    /// to the tabs that are prepared.
    fn check_attachable(&self) -> Result<(), SeleniumBaseError> {
        if self.merged.proxy.is_some() || self.merged.proxy_pac_url.is_some() {
            return Err(SeleniumBaseError::invalid_config(
                "a proxy cannot be set on an attached browser: it belongs to the app, whose \
                 network settings are not changed (this covers a PAC file and the proxy of a \
                 fingerprint); set the proxy where the app starts its browser",
            ));
        }
        let unapplied_agent = self.asked.user_agent.is_some() && !self.phone();
        if self.fingerprint.is_none() && (unapplied_agent || self.asked.lang.is_some()) {
            return Err(SeleniumBaseError::invalid_config(
                "a user agent or locale needs a fingerprint on an attached browser: they are \
                 launch flags, which cannot reach a running app, and a fingerprint applies \
                 both to the windows it prepares",
            ));
        }
        Ok(())
    }

    /// Refuses the combinations of the profile mode and of the browser that mean nothing.
    fn check_modes(&self, set: &Layer) -> Result<(), SeleniumBaseError> {
        if self.switch_on(LaunchSwitch::Incognito) == Some(true)
            && self.switch_on(LaunchSwitch::Guest) == Some(true)
        {
            return Err(SeleniumBaseError::invalid_config(
                "incognito and guest mode cannot be combined",
            ));
        }
        if set.use_chromium == Some(true) && set.executable_path.is_some() {
            return Err(SeleniumBaseError::invalid_config(
                "use_chromium and an explicit executable path are mutually exclusive",
            ));
        }
        Ok(())
    }

    /// The plugin set with the plugins named (`plugins_named`, `plugin_selection`) added after
    /// the ones given.
    #[cfg(feature = "plugins")]
    fn plugins(&self) -> Result<seleniumbase_rs_plugin::PluginSet, SeleniumBaseError> {
        let given = self.merged.plugins.clone().unwrap_or_default();
        if self.merged.selection.is_empty() {
            return Ok(given);
        }
        crate::plugin_host::add_selection(
            given,
            &crate::plugin_host::builtin_registry(),
            &self.merged.selection,
        )
    }
}

/// What the switches say: the profile mode, whether the sandbox is off (unset, `SB_NO_SANDBOX`
/// decides), and the other switches that are on. The three that have fields of their own are
/// not in the set.
fn profile_of(
    switches: BTreeMap<LaunchSwitch, bool>,
) -> (ProfileMode, bool, BTreeSet<LaunchSwitch>) {
    let on = |switch: LaunchSwitch| switches.get(&switch).copied();
    let mode = if on(LaunchSwitch::Incognito) == Some(true) {
        ProfileMode::Incognito
    } else if on(LaunchSwitch::Guest) == Some(true) {
        ProfileMode::Guest
    } else {
        ProfileMode::Standard
    };
    let no_sandbox =
        on(LaunchSwitch::NoSandbox).unwrap_or_else(|| std::env::var_os("SB_NO_SANDBOX").is_some());
    let rest = switches
        .into_iter()
        .filter(|(switch, on)| {
            *on && !matches!(
                switch,
                LaunchSwitch::Incognito | LaunchSwitch::Guest | LaunchSwitch::NoSandbox
            )
        })
        .map(|(switch, _)| switch)
        .collect();
    (mode, no_sandbox, rest)
}

/// What reading the layers turns into text that can fail to parse, and the plugins.
#[derive(Default)]
struct Parsed {
    proxy: Option<Proxy>,
    pac: Option<PacUrl>,
    #[cfg(feature = "plugins")]
    plugins: Option<seleniumbase_rs_plugin::PluginSet>,
}

impl Layers {
    /// The fingerprint of the builder or else the config's.
    pub(super) fn fingerprint(&self) -> Option<&Fingerprint> {
        self.set
            .fingerprint
            .as_ref()
            .or(self.config.fingerprint.as_ref())
            .map(|shared| shared.0.as_ref())
    }

    /// Checks the combination and merges the layers into the options a launch reads.
    pub(super) fn resolve(self) -> Result<LaunchOptions, SeleniumBaseError> {
        let view = View::of(&self);
        view.check_phone_and_fingerprint()?;
        if view.merged.cdp_url.is_some() {
            view.check_attachable()?;
        }
        let pac = view
            .merged
            .proxy_pac_url
            .as_ref()
            .map(|text| text.0.parse::<PacUrl>())
            .transpose()?;
        view.check_modes(&self.set)?;
        let proxy = view
            .merged
            .proxy
            .as_ref()
            .map(|spec| bridge::parse_proxy(&spec.text.0, spec.checked))
            .transpose()?;
        let parsed = Parsed {
            proxy,
            pac,
            #[cfg(feature = "plugins")]
            plugins: Some(view.plugins()?),
        };
        Ok(self.assemble(view, parsed))
    }

    /// The options of layers nothing has been set on: no combination to check.
    pub(super) fn assemble_default() -> LaunchOptions {
        let layers = Self::default();
        let view = View::of(&layers);
        layers.assemble(view, Parsed::default())
    }

    /// Merges the layers into options, without checking the combination.
    fn assemble(self, view: View, parsed: Parsed) -> LaunchOptions {
        let View {
            merged,
            fingerprint,
            mobile,
            metrics,
            source,
            ..
        } = view;
        let Layer {
            url,
            cdp_url,
            cdp_target,
            headless,
            ad_block,
            user_data_dir,
            user_agent,
            lang,
            window_size,
            window_position,
            webrtc,
            shield_webrtc,
            proxy_bypass_list,
            virtual_display,
            startup_timeout,
            debugging_port,
            extra_args,
            extensions,
            prefs,
            switches,
            disable_features,
            mut enable_features,
            host_resolver_rules,
            ..
        } = merged;

        // The phone, when one is asked for, and the agent the browser starts with.
        let device = emulated_phone(mobile, metrics, user_agent.as_deref())
            .map(|(metrics, agent)| (metrics, agent.map(str::to_owned)));
        let user_agent = user_agent.or_else(|| mobile.then(|| MOBILE_USER_AGENT.to_owned()));

        // The three that have fields of their own are not in the set.
        let (mode, no_sandbox, switches) = profile_of(switches);

        // What the caller sets wins over what a switch implies.
        let mut merged_prefs = chrome_preferences(&switches);
        merged_prefs.extend(prefs);
        if switches.contains(&LaunchSwitch::DarkMode)
            && !enable_features
                .iter()
                .any(|feature| feature == "WebContentsForceDark")
        {
            enable_features.push("WebContentsForceDark".to_owned());
        }

        LaunchOptions {
            url,
            cdp_url,
            cdp_target,
            headless: headless.or_else(|| virtual_display.map(|_| false)),
            source,
            mode,
            ad_block: ad_block.unwrap_or(false),
            proxy: parsed.proxy,
            user_data_dir,
            user_agent,
            lang,
            window_size,
            window_position,
            no_sandbox,
            webrtc,
            shield_webrtc: shield_webrtc.unwrap_or(false),
            identity: fingerprint.map(|shared| Arc::new(StealthPlan::new(shared.0.as_ref()))),
            extra_args,
            extensions,
            prefs: merged_prefs,
            host_resolver_rules,
            disable_features,
            enable_features,
            disable_quic: false,
            switches,
            device,
            proxy_bypass_list: proxy_bypass_list.unwrap_or_default(),
            proxy_pac_url: parsed.pac,
            virtual_display,
            #[cfg(feature = "plugins")]
            plugins: parsed.plugins.unwrap_or_default(),
            startup_timeout: startup_timeout.unwrap_or(DEFAULT_STARTUP_TIMEOUT),
            debugging_port: debugging_port.unwrap_or(0),
            given: Arc::new(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::browser::config::{Browser as BrowserKind, BrowserConfig};
    use crate::evasion::fingerprint::{OsType, ProxyConfig};
    use crate::sb_cdp::LaunchOptionsBuilder;

    fn built(builder: LaunchOptionsBuilder) -> LaunchOptions {
        builder.build().unwrap()
    }

    fn chromium_config() -> BrowserConfig {
        BrowserConfig {
            browser: BrowserKind::Chromium,
            ..BrowserConfig::default()
        }
    }

    fn fingerprint(seed: u64) -> Fingerprint {
        Fingerprint::randomized(OsType::Linux, seed)
    }

    #[test]
    fn a_browser_the_builder_names_replaces_the_one_the_config_names() {
        let config = chromium_config();
        let path_config = BrowserConfig {
            browser_binary_path: Some("/opt/config/chrome".into()),
            ..BrowserConfig::default()
        };
        assert!(matches!(
            built(LaunchOptions::builder()).source,
            BrowserSource::Chrome
        ));
        assert!(matches!(
            built(LaunchOptions::builder().browser_config(&config)).source,
            BrowserSource::Chromium
        ));
        // A path of the builder beats Chromium of the config, before or after it.
        for builder in [
            LaunchOptions::builder()
                .executable_path("/opt/builder/chrome")
                .browser_config(&config),
            LaunchOptions::builder()
                .browser_config(&config)
                .executable_path("/opt/builder/chrome"),
        ] {
            assert!(matches!(
                built(builder).source,
                BrowserSource::Path(path) if path == Path::new("/opt/builder/chrome")
            ));
        }
        // Chromium of the builder beats the path of the config, and so does Chrome.
        for chromium in [true, false] {
            let expected = |source: &BrowserSource| {
                matches!(source, BrowserSource::Chromium) == chromium
                    && matches!(source, BrowserSource::Chrome) != chromium
            };
            for builder in [
                LaunchOptions::builder()
                    .use_chromium(chromium)
                    .browser_config(&path_config),
                LaunchOptions::builder()
                    .browser_config(&path_config)
                    .use_chromium(chromium),
            ] {
                assert!(expected(&built(builder).source));
            }
        }
        // Chrome of the builder keeps Chrome over the config's Chromium.
        assert!(matches!(
            built(
                LaunchOptions::builder()
                    .browser_config(&config)
                    .use_chromium(false)
            )
            .source,
            BrowserSource::Chrome
        ));
    }

    #[test]
    fn chromium_and_a_path_are_refused_together_only_when_the_builder_gives_both() {
        assert!(LaunchOptions::builder()
            .use_chromium(true)
            .executable_path("/opt/chrome")
            .build()
            .is_err());
        // Chromium of the config and the builder's path are two layers, not a conflict.
        assert!(LaunchOptions::builder()
            .executable_path("/opt/chrome")
            .browser_config(&chromium_config())
            .build()
            .is_ok());
    }

    #[test]
    fn a_switch_is_decided_by_the_builder_when_it_says_anything() {
        let mut config = BrowserConfig::default();
        config.launch.set(LaunchSwitch::Incognito, true);
        config.launch.set(LaunchSwitch::BlockImages, true);
        config.launch.set(LaunchSwitch::NoSandbox, true);
        let from_config = built(LaunchOptions::builder().browser_config(&config));
        assert!(matches!(from_config.mode, ProfileMode::Incognito));
        assert!(from_config.no_sandbox);
        assert!(from_config.switches.contains(&LaunchSwitch::BlockImages));

        for builder in [
            LaunchOptions::builder()
                .incognito(false)
                .no_sandbox(false)
                .switch(LaunchSwitch::BlockImages, false)
                .browser_config(&config),
            LaunchOptions::builder()
                .browser_config(&config)
                .incognito(false)
                .no_sandbox(false)
                .switch(LaunchSwitch::BlockImages, false),
        ] {
            let options = built(builder);
            assert!(matches!(options.mode, ProfileMode::Standard));
            assert!(!options.no_sandbox);
            assert!(options.switches.is_empty());
            assert!(options.prefs.is_empty());
        }
    }

    #[test]
    fn incognito_and_guest_are_refused_whichever_layer_gives_them() {
        let mut config = BrowserConfig::default();
        config.launch.set(LaunchSwitch::Incognito, true);
        assert!(LaunchOptions::builder()
            .guest(true)
            .browser_config(&config)
            .build()
            .is_err());
        assert!(LaunchOptions::builder()
            .browser_config(&config)
            .guest(true)
            .build()
            .is_err());
        // Unless the builder turns the config's mode off.
        let options = built(
            LaunchOptions::builder()
                .guest(true)
                .incognito(false)
                .browser_config(&config),
        );
        assert!(matches!(options.mode, ProfileMode::Guest));
    }

    #[test]
    fn the_phone_is_on_for_a_screen_or_mobile_and_off_for_mobile_false() {
        let screen = DeviceMetrics::new(360, 640, 2.0);
        let config = BrowserConfig::default().with_mobile(true);

        // A screen of the builder turns the phone on, with the phone's agent.
        let phone = built(LaunchOptions::builder().device_metrics(screen));
        assert_eq!(phone.device, Some((screen, Some(MOBILE_USER_AGENT.into()))));
        assert_eq!(phone.user_agent.as_deref(), Some(MOBILE_USER_AGENT));

        // A config's `mobile` makes the default phone, with the screen of the builder if any.
        let default_phone = built(LaunchOptions::builder().browser_config(&config));
        assert!(default_phone.device.is_some());
        let own_screen = built(
            LaunchOptions::builder()
                .browser_config(&config)
                .device_metrics(screen),
        );
        assert_eq!(own_screen.device.map(|(metrics, _)| metrics), Some(screen));

        // `mobile(false)` is final, before or after everything else.
        for builder in [
            LaunchOptions::builder()
                .mobile(false)
                .device_metrics(screen)
                .browser_config(&config),
            LaunchOptions::builder()
                .browser_config(&config)
                .device_metrics(screen)
                .mobile(false),
        ] {
            let options = built(builder);
            assert!(options.device.is_none());
            assert_eq!(options.user_agent, None);
        }
    }

    #[test]
    fn a_fingerprint_of_either_layer_is_refused_with_a_phone_unless_it_is_switched_off() {
        let config = BrowserConfig {
            mobile: true,
            fingerprint: Some(fingerprint(3)),
            ..BrowserConfig::default()
        };
        assert!(LaunchOptions::builder()
            .browser_config(&config)
            .build()
            .is_err());
        assert!(LaunchOptions::builder()
            .mobile(true)
            .fingerprint(&fingerprint(3))
            .build()
            .is_err());
        assert!(LaunchOptions::builder()
            .browser_config(&config)
            .mobile(false)
            .build()
            .is_ok());
    }

    #[test]
    fn the_builders_fingerprint_is_the_one_the_options_keep() {
        let config = BrowserConfig {
            fingerprint: Some(fingerprint(1)),
            ..BrowserConfig::default()
        };
        let own = fingerprint(2);
        for builder in [
            LaunchOptions::builder()
                .fingerprint(&own)
                .browser_config(&config),
            LaunchOptions::builder()
                .browser_config(&config)
                .fingerprint(&own),
        ] {
            assert_eq!(built(builder).fingerprint(), Some(&own));
        }
        let kept = built(LaunchOptions::builder().browser_config(&config));
        assert_eq!(kept.fingerprint(), config.fingerprint.as_ref());
    }

    #[test]
    fn an_unset_headless_follows_the_virtual_display_and_a_set_one_does_not() {
        let display = VirtualDisplay::default();
        let config = BrowserConfig::default().with_headless(true);
        assert_eq!(
            built(LaunchOptions::builder().virtual_display(display)).headless,
            Some(false)
        );
        // The config always says headless or headed, so the display only matters without one.
        for builder in [
            LaunchOptions::builder()
                .virtual_display(display)
                .browser_config(&config),
            LaunchOptions::builder()
                .browser_config(&config)
                .virtual_display(display),
        ] {
            assert_eq!(built(builder).headless, Some(true));
        }
        assert_eq!(
            built(
                LaunchOptions::builder()
                    .headless(true)
                    .virtual_display(display)
            )
            .headless,
            Some(true)
        );
    }

    #[test]
    fn an_attached_browser_is_judged_on_what_was_asked_for_by_name() {
        let print = fingerprint(4);
        // A user agent needs a fingerprint, from either layer, in any order.
        let with_print = BrowserConfig {
            fingerprint: Some(print.clone()),
            ..BrowserConfig::default()
        };
        assert!(LaunchOptions::builder()
            .cdp_url("http://127.0.0.1:9222")
            .user_agent("Agent/1")
            .build()
            .is_err());
        assert!(LaunchOptions::builder()
            .cdp_url("http://127.0.0.1:9222")
            .user_agent("Agent/1")
            .fingerprint(&print)
            .build()
            .is_ok());
        assert!(LaunchOptions::builder()
            .user_agent("Agent/1")
            .browser_config(&with_print)
            .cdp_url("http://127.0.0.1:9222")
            .build()
            .is_ok());
        // The fingerprint's own user agent, language and window size are not asked for by name.
        assert!(LaunchOptions::builder()
            .cdp_url("http://127.0.0.1:9222")
            .fingerprint(&print)
            .build()
            .is_ok());
        // A proxy is refused whoever gives it, a fingerprint's included.
        let proxied = print
            .to_builder()
            .proxy(ProxyConfig {
                r#type: "http".to_owned(),
                host: "proxy.example".to_owned(),
                port: 8080,
                ..ProxyConfig::default()
            })
            .build()
            .unwrap();
        assert!(LaunchOptions::builder()
            .cdp_url("http://127.0.0.1:9222")
            .fingerprint(&proxied)
            .build()
            .is_err());
    }

    #[test]
    fn a_proxy_is_checked_as_python_checks_it_only_where_the_config_gave_it() {
        let bad = BrowserConfig {
            proxy: Some("ftp://proxy.example:21".to_owned()),
            ..BrowserConfig::default()
        };
        assert!(LaunchOptions::builder()
            .browser_config(&bad)
            .build()
            .is_err());
        // The builder's proxy replaces the config's, and is read as the engine reads one.
        for builder in [
            LaunchOptions::builder()
                .proxy("ftp://own.example:21")
                .browser_config(&bad),
            LaunchOptions::builder()
                .browser_config(&bad)
                .proxy("ftp://own.example:21"),
        ] {
            let options = built(builder);
            assert_eq!(
                options.proxy.as_ref().map(Proxy::server),
                Some("ftp://own.example:21")
            );
        }
    }

    #[test]
    fn the_port_and_the_timeout_of_the_builder_win_over_the_config() {
        let mut config = BrowserConfig::default();
        config.launch.remote_debug_port = Some(9333);
        config.launch.remote_debug_any_port = false;
        assert_eq!(
            built(LaunchOptions::builder().browser_config(&config)).debugging_port,
            9333
        );
        for builder in [
            LaunchOptions::builder()
                .debugging_port(9444)
                .browser_config(&config),
            LaunchOptions::builder()
                .browser_config(&config)
                .debugging_port(9444),
        ] {
            assert_eq!(built(builder).debugging_port, 9444);
        }
        assert_eq!(
            built(LaunchOptions::builder()).startup_timeout,
            DEFAULT_STARTUP_TIMEOUT
        );
        assert_eq!(
            built(LaunchOptions::builder().startup_timeout(Duration::from_secs(3))).startup_timeout,
            Duration::from_secs(3)
        );
    }

    #[test]
    fn the_lists_of_both_layers_are_kept_the_configs_first() {
        let mut config = BrowserConfig {
            locale: Some("fr_FR".to_owned()),
            extension_dir: Some("/ext/one, /ext/two".to_owned()),
            ..BrowserConfig::default()
        };
        config.launch.disable_features = vec!["FromConfig".to_owned()];
        let options = built(
            LaunchOptions::builder()
                .disable_features(["FromBuilder"])
                .extension("/ext/three")
                .browser_config(&config)
                .preference("intl.accept_languages", "xx"),
        );
        assert_eq!(options.disable_features, ["FromConfig", "FromBuilder"]);
        assert_eq!(
            options.extensions,
            ["/ext/one", "/ext/two", "/ext/three"].map(PathBuf::from)
        );
        // The preference the config's locale implies yields to the one the builder sets.
        assert_eq!(options.prefs["intl.accept_languages"], "xx");
        assert_eq!(options.lang.as_deref(), Some("fr-FR"));
    }

    #[test]
    fn the_options_show_no_secret_and_no_whole_fingerprint() {
        let print = fingerprint(5)
            .to_builder()
            .proxy(ProxyConfig {
                r#type: "http".to_owned(),
                host: "proxy.example".to_owned(),
                port: 8080,
                username: Some("alice".to_owned()),
                password: Some("s3cret".to_owned()),
                ..ProxyConfig::default()
            })
            .build()
            .unwrap();
        let config = BrowserConfig {
            proxy: Some("bob:hunter2@config.example:9090".to_owned()),
            ..BrowserConfig::default()
        };
        for builder in [
            LaunchOptions::builder().fingerprint(&print),
            LaunchOptions::builder().browser_config(&config),
        ] {
            let shown = format!("{builder:?} {:?}", built(builder.clone()));
            for secret in ["s3cret", "alice", "hunter2", "bob"] {
                assert!(!shown.contains(secret), "{secret} in {shown}");
            }
        }
        let shown = format!("{:?}", built(LaunchOptions::builder().fingerprint(&print)));
        assert!(shown.contains("Fingerprint(..)"), "{shown}");
        assert!(!shown.contains("navigator"), "{shown}");
    }

    #[test]
    fn default_options_are_the_ones_nothing_was_set_for() {
        let profile = Path::new("/tmp/profile");
        assert_eq!(
            LaunchOptions::default().browser_args(profile),
            built(LaunchOptions::builder()).browser_args(profile)
        );
    }
}
