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
    /// Deterministic noise seed (see [`Fingerprint::seed_value`]).
    pub seed: u64,
    /// Runtime assembly configuration.
    pub config: EvasionConfig,
}

impl<'a> EvasionContext<'a> {
    /// Builds a context from a fingerprint, deriving the seed from it.
    pub fn new(fingerprint: &'a Fingerprint) -> Self {
        Self {
            seed: fingerprint.seed_value(),
            fingerprint,
            config: EvasionConfig::default(),
        }
    }

    /// Builds a context with an explicit config.
    pub fn with_config(fingerprint: &'a Fingerprint, config: EvasionConfig) -> Self {
        Self {
            seed: fingerprint.seed_value(),
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
    /// Defaults to `true`; most providers gate on `fingerprint.flags`.
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
}
