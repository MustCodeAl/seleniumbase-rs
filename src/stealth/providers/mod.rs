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
//!    [`EvasionContext::escape`] and the placeholder helpers to substitute
//!    values from the fingerprint.
//! 3. Gate inclusion with [`EvasionProvider::applies`] and order it with
//!    [`EvasionProvider::priority`].
//! 4. Register it, either in [`default_registry`] for a built-in or via
//!    [`EvasionRegistry::register_provider`] at runtime.
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
    pub fn escape(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
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
        format!(
            "(() => {{\n  'use strict';\n{body}}})();",
            body = indent(&body)
        )
    }
}

fn indent(s: &str) -> String {
    s.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
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
}
