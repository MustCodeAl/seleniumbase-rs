//! An ordered collection of [`EvasionProvider`]s.

use super::builtin;
use super::context::EvasionContext;
use super::EvasionProvider;
use crate::evasion::fingerprint::Fingerprint;

/// An ordered collection of [`EvasionProvider`]s.
#[derive(Default)]
pub struct EvasionRegistry {
    providers: Vec<Box<dyn EvasionProvider>>,
}

impl std::fmt::Debug for EvasionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvasionRegistry")
            .field("providers", &self.provider_names())
            .finish()
    }
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
/// use seleniumbase_rs::evasion::fingerprint::Fingerprint;
/// use seleniumbase_rs::evasion::providers::{session_registry, EvasionContext};
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
