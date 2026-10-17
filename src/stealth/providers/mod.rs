//! Provider/plugin architecture for anti-detection JavaScript evasions.
//!
//! Each detectable browser dimension is spoofed by an [`EvasionProvider`]. A
//! provider is a small, self-contained unit that returns a JavaScript snippet
//! for a given [`EvasionContext`] (the session fingerprint plus a deterministic
//! seed and runtime config). An [`EvasionRegistry`] holds an ordered list of
//! providers and assembles them into a single bootstrap script.
//!
//! # Adding a new evasion
//!
//! 1. Implement [`EvasionProvider`] for a unit struct in
//!    [`builtin`](crate::stealth::providers::builtin) (or your own crate).
//! 2. Return the JavaScript from [`EvasionProvider::script`]; use
//!    [`EvasionContext::escape`] for text and [`EvasionContext::number`] for
//!    numbers when substituting values from the fingerprint. Patch
//!    prototypes rather than instances, and start the snippet with
//!    [`SB_PREAMBLE`] to make replacements look native (see below).
//! 3. Gate inclusion with [`EvasionProvider::applies`] and order it with
//!    [`EvasionProvider::priority`].
//! 4. Register it, either in [`default_registry`] for a built-in or via
//!    [`EvasionRegistry::register_provider`] at runtime.
//!
//! # Looking native
//!
//! A page can ask a replaced function for its source, name, length and
//! properties, and a getter for its name. [`SB_PREAMBLE`] puts a constant `sb`
//! in scope with three helpers that answer like the browser would:
//!
//! - `sb.native(fn, name, like)` returns a stand-in for a method: it has the
//!   given `name`, the length of `like` (the original), no `prototype`, and its
//!   source prints as `[native code]`;
//! - `sb.ctor(fn, name)` marks a constructor the same way;
//! - `sb.getter(object, property, fn)` replaces an accessor's getter, keeping
//!   the property's other attributes, with a native-looking `get <property>`.
//!
//! The helpers are installed once, by the `native_tostring` provider, in the
//! scope of the assembled script, not on `window`. Without it, or when a snippet
//! runs on its own, `sb` falls back to plain functions that mark nothing.
//!
//! # Example
//!
//! ```rust
//! use seleniumbase_rs::stealth::fingerprint::Fingerprint;
//! use seleniumbase_rs::stealth::providers::{default_registry, EvasionContext};
//!
//! let fp = Fingerprint::windows_desktop();
//! let ctx = EvasionContext::new(&fp);
//! let script = default_registry().bootstrap(&ctx);
//! assert!(script.contains("webdriver"));
//! ```

pub mod builtin;

use crate::stealth::fingerprint::Fingerprint;

/// JavaScript that puts the shared `sb` helpers in scope for a snippet.
///
/// Put it on the first line of a snippet's function body. See
/// [Looking native](self#looking-native).
pub const SB_PREAMBLE: &str = "const sb = (typeof __sb !== 'undefined' && __sb) || { \
native: function (f) { return f; }, ctor: function (f) { return f; }, \
getter: function (o, p, f) { Object.defineProperty(o, p, { get: f, configurable: true }); } };";

/// Runtime configuration for script assembly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvasionConfig {
    /// Wrap every provider's snippet in its own `try/catch` so a single failing
    /// evasion cannot break the others (or the host page).
    pub isolate_failures: bool,
    /// Emit a `// provider: <name>` comment before each snippet.
    pub annotate: bool,
}

impl Default for EvasionConfig {
    fn default() -> Self {
        Self {
            isolate_failures: true,
            annotate: true,
        }
    }
}

/// Context handed to every provider when generating its script.
#[derive(Clone, Debug)]
pub struct EvasionContext<'a> {
    /// The session fingerprint being spoofed.
    pub fingerprint: &'a Fingerprint,
    /// Deterministic noise seed (see [`Fingerprint::seed`]).
    pub seed: u64,
    /// Runtime assembly configuration.
    pub config: EvasionConfig,
}

impl<'a> EvasionContext<'a> {
    /// Builds a context from a fingerprint, deriving the seed from it.
    pub fn new(fingerprint: &'a Fingerprint) -> Self {
        Self {
            seed: fingerprint.seed(),
            fingerprint,
            config: EvasionConfig::default(),
        }
    }

    /// Builds a context with an explicit config.
    pub fn with_config(fingerprint: &'a Fingerprint, config: EvasionConfig) -> Self {
        Self {
            seed: fingerprint.seed(),
            fingerprint,
            config,
        }
    }

    /// Escapes a string for safe interpolation inside single-quoted JS.
    ///
    /// Backslashes, single quotes, line breaks and the Unicode line and
    /// paragraph separators are escaped, so the text cannot end the literal or
    /// the line it is on.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::stealth::providers::EvasionContext;
    ///
    /// assert_eq!(EvasionContext::escape("it's"), "it\\'s");
    /// ```
    pub fn escape(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029")
    }

    /// Formats a number as a JavaScript numeric expression.
    ///
    /// Rust prints an infinite `f64` as `inf`, which JavaScript reads as an
    /// undefined variable. This prints `Infinity`, `-Infinity` and `NaN`, which
    /// it does understand, and finite values as they are.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::stealth::providers::EvasionContext;
    ///
    /// assert_eq!(EvasionContext::number(1.5), "1.5");
    /// assert_eq!(EvasionContext::number(f64::INFINITY), "Infinity");
    /// assert_eq!(EvasionContext::number(f64::NEG_INFINITY), "-Infinity");
    /// assert_eq!(EvasionContext::number(f64::NAN), "NaN");
    /// ```
    pub fn number(value: f64) -> String {
        if value.is_nan() {
            "NaN".to_owned()
        } else if value.is_infinite() {
            if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned()
        } else {
            value.to_string()
        }
    }
}

/// A single anti-detection evasion.
///
/// Providers must be cheap to construct and side-effect free: they only build
/// JavaScript strings. Detection-relevant ordering is expressed through
/// [`priority`](EvasionProvider::priority).
pub trait EvasionProvider: Send + Sync {
    /// Stable, unique identifier (used by tooling and the MCP server).
    fn name(&self) -> &str;

    /// Lower runs earlier. The shared prelude/native-`toString` machinery uses
    /// small values so later providers can rely on it. Defaults to `100`.
    fn priority(&self) -> i32 {
        100
    }

    /// Whether this provider should be included for the given fingerprint.
    /// Defaults to `true`; most providers gate on [`Fingerprint::is_masked`].
    fn applies(&self, fingerprint: &Fingerprint) -> bool {
        let _ = fingerprint;
        true
    }

    /// Returns the JavaScript snippet, or `None` to contribute nothing.
    fn script(&self, ctx: &EvasionContext) -> Option<String>;
}

/// An ordered collection of [`EvasionProvider`]s.
#[derive(Default)]
pub struct EvasionRegistry {
    providers: Vec<Box<dyn EvasionProvider>>,
}

impl EvasionRegistry {
    /// Creates an empty registry.
    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Registers a provider and returns `&mut self` for chaining.
    pub fn register_provider(&mut self, provider: Box<dyn EvasionProvider>) -> &mut Self {
        self.providers.push(provider);
        self
    }

    /// Returns all registered providers sorted by ascending priority (ties keep
    /// insertion order, since the sort is stable).
    pub fn ordered(&self) -> Vec<&dyn EvasionProvider> {
        let mut refs: Vec<&dyn EvasionProvider> =
            self.providers.iter().map(|p| p.as_ref()).collect();
        refs.sort_by_key(|p| p.priority());
        refs
    }

    /// Returns the names of every registered provider, ordered by priority.
    pub fn provider_names(&self) -> Vec<String> {
        self.ordered()
            .into_iter()
            .map(|p| p.name().to_owned())
            .collect()
    }

    /// Returns the names of providers that apply to the given fingerprint.
    pub fn applicable_names(&self, fingerprint: &Fingerprint) -> Vec<String> {
        self.ordered()
            .into_iter()
            .filter(|p| p.applies(fingerprint))
            .map(|p| p.name().to_owned())
            .collect()
    }

    /// Returns `(name, script)` pairs for every applicable provider that emits a
    /// snippet, in execution order.
    pub fn scripts(&self, ctx: &EvasionContext) -> Vec<(String, String)> {
        self.ordered()
            .into_iter()
            .filter(|p| p.applies(ctx.fingerprint))
            .filter_map(|p| p.script(ctx).map(|s| (p.name().to_owned(), s)))
            .collect()
    }

    /// Assembles a single IIFE bootstrap script from all applicable providers.
    ///
    /// The providers' snippets are copied in unchanged, one after another, in
    /// priority order. The script declares the `__sb` variable that
    /// [`SB_PREAMBLE`] looks for in its own scope, so the assembly adds nothing
    /// to the page's global object.
    pub fn bootstrap(&self, ctx: &EvasionContext) -> String {
        let mut body = String::new();
        for (name, script) in self.scripts(ctx) {
            if ctx.config.annotate {
                body.push_str(&format!("// provider: {name}\n"));
            }
            if ctx.config.isolate_failures {
                body.push_str("try {\n");
                body.push_str(&script);
                body.push_str("\n} catch (e) {}\n");
            } else {
                body.push_str(&script);
                body.push('\n');
            }
        }
        format!("(() => {{\n  'use strict';\n  let __sb;\n{body}}})();")
    }
}

/// Returns a registry populated with every built-in provider.
pub fn default_registry() -> EvasionRegistry {
    let mut registry = EvasionRegistry::empty();
    for provider in builtin::all() {
        registry.register_provider(provider);
    }
    registry
}

/// The built-in providers [`session_registry`] leaves out, by
/// [`name`](EvasionProvider::name).
///
/// Chrome already has `navigator.plugins`, `navigator.mimeTypes` and
/// `navigator.connection`, and answers for them itself, so a replaced getter
/// is one more thing a page can find and a real Chrome does not have.
pub const SESSION_OMITTED_PROVIDERS: [&str; 2] = ["plugins", "connection"];

/// Returns the registry for a browser session that drives a real Chrome.
///
/// It is [`default_registry`] without the providers named in
/// [`SESSION_OMITTED_PROVIDERS`]: the assembled script never mentions
/// `plugins`, `mimeTypes`, `pdfViewerEnabled` or `connection`, so the browser's
/// own lists, PDF viewer flag and network information are what a page reads.
/// [`default_registry`] is unchanged, for callers that want those spoofs, such
/// as a browser that really lacks them.
///
/// The fingerprint is accepted so that a later rule can depend on the persona
/// without changing callers; today every persona gets the same set.
///
/// # Examples
///
/// ```rust
/// use seleniumbase_rs::stealth::fingerprint::Fingerprint;
/// use seleniumbase_rs::stealth::providers::{session_registry, EvasionContext};
///
/// let fp = Fingerprint::windows_desktop();
/// let script = session_registry(&fp).bootstrap(&EvasionContext::new(&fp));
/// assert!(script.contains("webdriver"));
/// assert!(!script.contains("mimeTypes"));
/// ```
#[must_use]
pub fn session_registry(_fingerprint: &Fingerprint) -> EvasionRegistry {
    let mut registry = EvasionRegistry::empty();
    for provider in builtin::all() {
        if !SESSION_OMITTED_PROVIDERS.contains(&provider.name()) {
            registry.register_provider(provider);
        }
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy {
        name: &'static str,
        priority: i32,
    }

    impl EvasionProvider for Dummy {
        fn name(&self) -> &str {
            self.name
        }
        fn priority(&self) -> i32 {
            self.priority
        }
        fn script(&self, _ctx: &EvasionContext) -> Option<String> {
            Some(format!("/* {} */", self.name))
        }
    }

    #[test]
    fn registry_orders_by_priority() {
        let mut reg = EvasionRegistry::empty();
        reg.register_provider(Box::new(Dummy {
            name: "late",
            priority: 200,
        }));
        reg.register_provider(Box::new(Dummy {
            name: "early",
            priority: 1,
        }));
        assert_eq!(reg.provider_names(), vec!["early", "late"]);
    }

    #[test]
    fn escape_handles_quotes() {
        assert_eq!(EvasionContext::escape("a'b\\c"), "a\\'b\\\\c");
    }

    #[test]
    fn escape_handles_every_character_that_ends_a_line() {
        assert_eq!(
            EvasionContext::escape("a\nb\rc\u{2028}d\u{2029}e"),
            "a\\nb\\rc\\u2028d\\u2029e"
        );
    }

    #[test]
    fn number_never_prints_a_rust_only_spelling() {
        assert_eq!(EvasionContext::number(0.0), "0");
        assert_eq!(EvasionContext::number(-2.5), "-2.5");
        assert_eq!(EvasionContext::number(1e21), "1000000000000000000000");
        assert_eq!(EvasionContext::number(f64::INFINITY), "Infinity");
        assert_eq!(EvasionContext::number(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(EvasionContext::number(f64::NAN), "NaN");
    }

    struct Template;

    impl EvasionProvider for Template {
        fn name(&self) -> &str {
            "template"
        }
        fn script(&self, _ctx: &EvasionContext) -> Option<String> {
            Some("window.t = `one\n  two\n    three`;".to_owned())
        }
    }

    #[test]
    fn bootstrap_copies_a_snippet_in_unchanged() {
        // Re-indenting the script would add spaces inside this template
        // literal and change the string the page sees.
        let mut reg = EvasionRegistry::empty();
        reg.register_provider(Box::new(Template));
        let fp = Fingerprint::windows_desktop();
        let config = EvasionConfig {
            isolate_failures: false,
            annotate: false,
        };

        let script = reg.bootstrap(&EvasionContext::with_config(&fp, config));

        assert!(
            script.contains("window.t = `one\n  two\n    three`;"),
            "{script}"
        );
    }

    #[test]
    fn bootstrap_declares_the_helper_scope_and_nothing_global() {
        let fp = Fingerprint::windows_desktop();
        let script = default_registry().bootstrap(&EvasionContext::new(&fp));

        assert!(script.starts_with("(() => {\n  'use strict';\n  let __sb;\n"));
        assert!(script.ends_with("})();"));
        assert!(!script.contains("window.__sb"));
        assert!(
            script.contains("__sb = {"),
            "native_tostring fills the scope"
        );
    }

    #[test]
    fn the_preamble_falls_back_to_plain_functions() {
        assert!(SB_PREAMBLE.starts_with("const sb = "));
        assert!(SB_PREAMBLE.contains("typeof __sb !== 'undefined'"));
        assert!(SB_PREAMBLE.contains("native:") && SB_PREAMBLE.contains("getter:"));
        assert!(SB_PREAMBLE.contains("ctor:"));
        assert!(
            !SB_PREAMBLE.contains('\n'),
            "it is one line, to sit on one line of a snippet"
        );
    }

    #[test]
    fn default_registry_has_every_builtin_once() {
        let registry = default_registry();
        let names = registry.provider_names();
        assert_eq!(names.len(), builtin::all().len());
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }

    type Setter = fn(FingerprintBuilder) -> FingerprintBuilder;

    /// One setter per masking dimension, with the providers it turns on.
    fn setters() -> Vec<(&'static str, Setter, &'static [&'static str])> {
        vec![
            (
                "user_agent",
                |b| b.user_agent("Mozilla/5.0"),
                &["navigator_props"],
            ),
            (
                "hardware_concurrency",
                |b| b.hardware_concurrency(8),
                &["navigator_props"],
            ),
            ("locale", |b| b.locale("de-DE"), &["localization"]),
            ("timezone", |b| b.timezone("Europe/Berlin"), &["timezone"]),
            ("screen", |b| b.screen(1440, 900), &["window_geometry"]),
            ("webgl", |b| b.webgl(("Vendor", "Renderer")), &["webgl"]),
            (
                "geolocation",
                |b| b.geolocation((52.5, 13.4)),
                &["geolocation"],
            ),
            (
                "geolocation denied",
                |b| b.geolocation(Spoof::Off),
                &["geolocation"],
            ),
            (
                "media_devices",
                |b| b.media_device_counts(1, 1, 1),
                &["media_devices"],
            ),
            ("fonts", |b| b.fonts(["Arial"]), &["fonts"]),
            (
                "webrtc",
                |b| b.webrtc(("203.0.113.9", "10.0.0.2")),
                &["webrtc"],
            ),
            ("webrtc off", |b| b.webrtc(Spoof::Off), &["webrtc"]),
            (
                "canvas_noise",
                |b| b.canvas_noise(CanvasNoise::Persistent),
                &["canvas_noise"],
            ),
            (
                "audio_noise",
                |b| b.audio_noise(AudioNoise::Persistent),
                &["audio_noise"],
            ),
            (
                "battery",
                |b| b.battery(BatteryProfile::default()),
                &["battery"],
            ),
            (
                "connection",
                |b| b.connection(ConnectionProfile::default()),
                &["connection"],
            ),
            (
                "speech_voices",
                |b| b.speech_voices(SpeechVoice::default_set()),
                &["speech"],
            ),
            (
                "client_hints",
                |b| b.client_hints(ClientHints::default()),
                &["client_hints"],
            ),
            (
                "blocked_trackers",
                |b| b.blocked_trackers(vec!["tracker.test".to_owned()]),
                &["tracker_block"],
            ),
        ]
    }

    #[test]
    fn a_setting_given_to_the_builder_turns_its_own_provider_on() {
        let registry = default_registry();
        for (name, set, providers) in setters() {
            // No policy at all, then each policy given before and after the setting.
            let policies = [
                None,
                Some(StealthPolicy::default()),
                Some(StealthPolicy::balanced()),
                Some(StealthPolicy::native_only()),
                Some(StealthPolicy::max()),
            ];
            for policy in policies {
                for policy_first in [true, false] {
                    let mut builder = Fingerprint::builder();
                    if let (Some(policy), true) = (&policy, policy_first) {
                        builder = builder.policy(policy.clone());
                    }
                    builder = set(builder);
                    if let (Some(policy), false) = (&policy, policy_first) {
                        builder = builder.policy(policy.clone());
                    }
                    let fp = builder.build().expect("the builder builds");
                    let applicable = registry.applicable_names(&fp);
                    for provider in providers {
                        // Under native spoofing the protocol delivers a position, a
                        // time zone and a language, so their scripts stay out.
                        if fp.policy().native_spoofing
                            && ["timezone", "localization", "client_hints"].contains(provider)
                            || fp.policy().native_spoofing
                                && *provider == "geolocation"
                                && fp.geolocation().is_value()
                        {
                            continue;
                        }
                        assert!(
                            applicable.contains(&(*provider).to_owned()),
                            "{name}: {provider} is off (policy {policy:?}, policy first: {policy_first})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_policy_alone_turns_on_only_the_providers_that_have_no_setting() {
        let registry = default_registry();
        let fp = Fingerprint::builder()
            .policy(StealthPolicy::max())
            .build()
            .expect("the builder builds");
        let applicable = registry.applicable_names(&fp);
        for name in [
            "chrome_runtime",
            "headless",
            "tracker_block",
            "canvas_noise",
            "audio_noise",
            "webrtc",
        ] {
            assert!(applicable.contains(&name.to_owned()), "{name}");
        }
        for name in [
            "navigator_props",
            "webgl",
            "window_geometry",
            "timezone",
            "battery",
            "client_hints",
        ] {
            assert!(
                !applicable.contains(&name.to_owned()),
                "{name} needs a setting"
            );
        }
    }

    #[test]
    fn inapplicable_providers_are_left_out() {
        let fp = Fingerprint::windows_desktop();
        let registry = default_registry();
        let applicable = registry.applicable_names(&fp);
        assert!(applicable.contains(&"webdriver".to_owned()));
        assert!(
            !applicable.contains(&"tracker_block".to_owned()),
            "off by default"
        );
        assert!(!applicable.contains(&"iframe".to_owned()));
    }

    // ------------------------------------------------------------------
    // Hardening: what a session leaves to the browser
    // ------------------------------------------------------------------

    use crate::stealth::fingerprint::{
        AudioNoise, BatteryProfile, BrandVersion, CanvasNoise, ClientHints, ConnectionProfile,
        FingerprintBuilder, SpeechVoice, Spoof, StealthPolicy,
    };

    /// The names a page reads to find a replaced list, flag or network report
    /// that Chrome would have answered for itself, as whole words:
    /// `chrome.loadTimes()` has a `connectionInfo` field, which is not one.
    fn browser_owned() -> regex::Regex {
        regex::Regex::new(r"\b(?:plugins|mimeTypes|pdfViewerEnabled|connection)\b").unwrap()
    }

    /// The first browser-owned name in `script`, with the text before it.
    fn names_browser_owned(script: &str) -> Option<String> {
        let found = browser_owned().find(script)?;
        let before = script.get(found.start().saturating_sub(60)..found.start());
        Some(format!("{} after {before:?}", found.as_str()))
    }

    /// A fingerprint that makes every provider that can apply, apply.
    fn everything() -> Fingerprint {
        Fingerprint::windows_desktop()
            .to_builder()
            .policy(StealthPolicy::max())
            .audio_noise(AudioNoise::Persistent)
            .media_device_counts(1, 1, 1)
            .fonts(["Arial"])
            .geolocation((1.0, 2.0))
            .webrtc(("203.0.113.9", "10.0.0.2"))
            .client_hints(ClientHints {
                brands: vec![BrandVersion::new("Chromium", "124")],
                full_version_list: vec![BrandVersion::new("Chromium", "124.0.0.0")],
                platform: "Windows".to_owned(),
                ..ClientHints::default()
            })
            .build()
            .expect("the builder builds")
    }

    #[test]
    fn the_session_registry_is_the_default_one_without_plugins_and_connection() {
        let fp = everything();

        let session = session_registry(&fp).provider_names();

        let expected: Vec<String> = default_registry()
            .provider_names()
            .into_iter()
            .filter(|name| !SESSION_OMITTED_PROVIDERS.contains(&name.as_str()))
            .collect();
        assert_eq!(session, expected, "the same providers in the same order");
        assert_eq!(session.len(), builtin::all().len() - 2);
        assert!(!session.contains(&"plugins".to_owned()));
        assert!(!session.contains(&"connection".to_owned()));
    }

    #[test]
    fn the_default_registry_still_has_the_providers_a_session_leaves_out() {
        let fp = everything();
        let script = default_registry().bootstrap(&EvasionContext::new(&fp));

        for word in ["plugins", "mimeTypes", "connection"] {
            assert!(script.contains(word), "{word} is the default's to spoof");
        }
    }

    #[test]
    fn the_session_script_never_names_what_chrome_answers_for_itself() {
        let fp = everything();

        let script = session_registry(&fp).bootstrap(&EvasionContext::new(&fp));

        assert_eq!(names_browser_owned(&script), None);
    }

    /// The providers that never apply and so never emit a script.
    const INERT: [&str; 2] = ["prepare_stack_trace", "iframe"];

    /// What every session provider must satisfy, whatever it spoofs.
    ///
    /// Its snippet is a self-running function, so it can be run on its own and
    /// leaves nothing behind but what it patches. It names none of the surfaces
    /// Chrome answers for itself, leaves no marker unfilled and no helper on
    /// `window`, and is the same text every time it is asked for.
    fn audit(name: &str) {
        let fp = everything();
        let ctx = EvasionContext::new(&fp);
        let registry = session_registry(&fp);
        let provider = registry
            .ordered()
            .into_iter()
            .find(|provider| provider.name() == name)
            .unwrap_or_else(|| panic!("{name} is not in the session registry"));

        let Some(script) = provider.script(&ctx) else {
            assert!(INERT.contains(&name), "{name} gave no script");
            assert!(
                !provider.applies(&fp),
                "{name} is inert, so it never applies"
            );
            return;
        };

        assert!(!INERT.contains(&name), "{name} is listed as inert");
        assert!(script.starts_with("(function"), "{name} is a snippet");
        assert!(script.trim_end().ends_with("})();"), "{name} runs itself");
        assert_eq!(names_browser_owned(&script), None, "{name}");
        assert!(
            !script.contains("window.__sb"),
            "{name} leaves a helper on window"
        );
        assert!(
            !regex::Regex::new(r"@[A-Z][A-Z_]*@")
                .unwrap()
                .is_match(&script),
            "{name} has an unfilled marker"
        );
        assert_eq!(
            provider.script(&ctx).as_deref(),
            Some(script.as_str()),
            "{name} gives the same script for the same input"
        );
    }

    /// One test per session provider, named after it.
    macro_rules! audit_each {
        ($($test:ident => $name:literal),+ $(,)?) => {
            /// The providers the audit tests below cover.
            const AUDITED: &[&str] = &[$($name),+];
            $(
                #[test]
                fn $test() {
                    audit($name);
                }
            )+
        };
    }

    audit_each! {
        native_tostring_passes_the_audit => "native_tostring",
        webdriver_passes_the_audit => "webdriver",
        cdp_markers_passes_the_audit => "cdp_markers",
        chrome_runtime_passes_the_audit => "chrome_runtime",
        navigator_props_passes_the_audit => "navigator_props",
        permissions_passes_the_audit => "permissions",
        window_geometry_passes_the_audit => "window_geometry",
        webgl_passes_the_audit => "webgl",
        canvas_noise_passes_the_audit => "canvas_noise",
        audio_noise_passes_the_audit => "audio_noise",
        webrtc_passes_the_audit => "webrtc",
        battery_passes_the_audit => "battery",
        media_devices_passes_the_audit => "media_devices",
        fonts_passes_the_audit => "fonts",
        speech_passes_the_audit => "speech",
        bluetooth_passes_the_audit => "bluetooth",
        headless_passes_the_audit => "headless",
        prepare_stack_trace_passes_the_audit => "prepare_stack_trace",
        client_hints_passes_the_audit => "client_hints",
        media_codecs_passes_the_audit => "media_codecs",
        timezone_passes_the_audit => "timezone",
        localization_passes_the_audit => "localization",
        geolocation_passes_the_audit => "geolocation",
        hairline_passes_the_audit => "hairline",
        iframe_passes_the_audit => "iframe",
        attach_shadow_passes_the_audit => "attach_shadow",
        tracker_block_passes_the_audit => "tracker_block",
    }

    #[test]
    fn every_session_provider_has_an_audit_test() {
        let fp = everything();

        let mut names = session_registry(&fp).provider_names();
        names.sort_unstable();
        let mut audited: Vec<String> = AUDITED.iter().map(|name| (*name).to_owned()).collect();
        audited.sort_unstable();

        assert_eq!(
            names, audited,
            "add a line to audit_each! for a new provider"
        );
    }

    #[test]
    fn noise_is_a_pure_function_of_the_seed_and_the_position() {
        let with_seed = |seed: u64| {
            everything()
                .to_builder()
                .seed(seed)
                .build()
                .expect("the builder builds")
        };
        let fp = with_seed(41);
        let noisy = [
            &builtin::CanvasNoiseProvider as &dyn EvasionProvider,
            &builtin::AudioNoiseProvider,
        ];

        for provider in noisy {
            let name = provider.name();
            let script = provider.script(&EvasionContext::new(&fp)).unwrap();
            for source in [
                "Math.random",
                "Date.now",
                "performance.now",
                "getRandomValues",
            ] {
                assert!(!script.contains(source), "{name} reads {source}");
            }
            assert!(script.contains("hash(index)"), "{name} hashes the position");

            let again = provider.script(&EvasionContext::new(&fp)).unwrap();
            assert_eq!(script, again, "{name}: same seed, same script");
            let reseeded = with_seed(42);
            let other = provider.script(&EvasionContext::new(&reseeded)).unwrap();
            assert_ne!(script, other, "{name}: the seed is what changes the noise");
        }
    }

    #[test]
    fn client_hints_wait_for_a_secure_context() {
        let fp = everything();
        let script = builtin::ClientHintsProvider
            .script(&EvasionContext::new(&fp))
            .unwrap();

        let guard = script
            .find("if (!window.isSecureContext)")
            .expect("the snippet checks the context");
        let define = script
            .find("'userAgentData'")
            .expect("the snippet defines the property");
        assert!(guard < define, "the check comes before the property exists");
    }

    #[test]
    fn navigator_props_leave_the_pdf_viewer_flag_to_the_browser() {
        let fp = everything();
        let script = builtin::NavigatorPropsProvider
            .script(&EvasionContext::new(&fp))
            .unwrap();

        assert!(!script.contains("pdfViewerEnabled"));
        assert!(script.contains("hardwareConcurrency"), "the rest is intact");
    }
}
