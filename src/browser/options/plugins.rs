//! The browser plugins a configuration carries (feature `plugins`, ADR 004).

use seleniumbase_rs_plugin::PluginSet;

/// The browser plugins of a [`BrowserConfig`](crate::BrowserConfig): a
/// [`PluginSet`] that a configuration can hold, clone and compare.
///
/// Set it with [`BrowserConfig::with_plugins`](crate::BrowserConfig::with_plugins).
/// Two values are equal when their plugins have the same names in the same
/// order. A configuration file cannot name plugins: they are Rust values, so
/// this is left out when a configuration is written or read.
#[derive(Clone, Debug, Default)]
pub struct BrowserPlugins(PluginSet);

impl BrowserPlugins {
    /// The plugins, in the order their hooks run.
    #[must_use]
    pub fn set(&self) -> &PluginSet {
        &self.0
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<PluginSet> for BrowserPlugins {
    fn from(set: PluginSet) -> Self {
        Self(set)
    }
}

impl PartialEq for BrowserPlugins {
    fn eq(&self, other: &Self) -> bool {
        self.0.names() == other.0.names()
    }
}

#[cfg(test)]
mod tests {
    use seleniumbase_rs_plugin::{async_trait, Plugin};

    use super::*;

    struct Named(&'static str);

    #[async_trait]
    impl Plugin for Named {
        fn name(&self) -> &str {
            self.0
        }
    }

    fn of(names: &[&'static str]) -> BrowserPlugins {
        names
            .iter()
            .try_fold(PluginSet::new(), |set, name| set.with(Named(name)))
            .unwrap()
            .into()
    }

    #[test]
    fn plugins_compare_by_their_names_in_order() {
        assert_eq!(BrowserPlugins::default(), of(&[]));
        assert!(BrowserPlugins::default().is_empty());
        assert_eq!(of(&["a", "b"]), of(&["a", "b"]));
        assert_ne!(of(&["a", "b"]), of(&["b", "a"]));
        assert_ne!(of(&["a"]), of(&[]));
        assert_eq!(of(&["a", "b"]).set().names(), ["a", "b"]);
    }
}
