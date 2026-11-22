//! Runtime configuration for script assembly.

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
