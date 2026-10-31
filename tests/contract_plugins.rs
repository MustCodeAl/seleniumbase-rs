//! The contract with `seleniumbase-rs-plugins`, the repository of the plugin API
//! (`seleniumbase-rs-plugin`) and the built-in plugins (`seleniumbase-rs-plugins-builtin`).
//!
//! This direction is the reverse of the others: this crate depends on the plugins repository, by
//! git and a commit (`Cargo.toml`, the two `seleniumbase-rs-plugin*` lines), and the plugins
//! repository imports nothing from here. The contract is therefore what this crate uses of
//! those crates, written as this crate uses it:
//!
//! * `imports`: every `seleniumbase_rs_plugin::` and `seleniumbase_rs_plugins_builtin::` path
//!   that `src/` and `crates/cli/src` name. `scripts/check-consumers.sh plugins` fails, naming
//!   the path, when the sources use one that is not here, so a new use of the plugin API cannot
//!   go unpinned.
//! * `a_plugin_as_the_host_sees_it`: a plugin written against the trait, put into a set and given
//!   to both launch paths (`LaunchOptionsBuilder::plugins`, `BrowserConfig::with_plugins`).
//!
//! `just contracts` also checks the pin itself: both lines of `Cargo.toml` carry the same git URL
//! and the same `rev` (two revisions would be two `seleniumbase-rs-plugin` crates, and the
//! `Plugin` of a built-in would not be the one this crate calls), and that `rev` is an ancestor of
//! the plugins repository's `main`, so the commit this crate builds against is one that was kept.
//!
//! Feature `plugins` (`plugin-shield` for the built-in registry); the workspace build turns both
//! on through the command line's `plugins-builtin`.

#![cfg(feature = "plugins")]
#![allow(dead_code, unused_imports, clippy::unused_async)]

use std::sync::Arc;

use serde_json::{json, Value};

mod imports {
    //! The plugin API as `src/` imports it.

    const _: () = {
        use seleniumbase_rs_plugin::{
            async_trait, BrowserFamily, BrowserHandle, BrowserId, Capability, CapabilitySet,
            Category, Configurable, ContextHandle, ContextId, ContextSettings, Descriptor,
            Enforcement, Engine, HostInfo, HostKind, LaunchContext, LaunchSettings,
            LoadedExtension, Metadata, NavDecision, NavOrigin, NavOutcome, Navigation, NetRule,
            NetworkInterest, NoPrompt, Outcome, Package, PageHandle, PageId, PathScope, Phase,
            Plugin, PluginError, PluginSet, PluginSpec, ProfileView, Prompt, ProtocolEvent,
            Registry, RequestDecision, RequestFilter, RequestView, Requirements, ResourceKind,
            ResponseView, Result, Seals, Sink, Task, TaskContext, TestInfo, TestOutcome,
            TestVerdict, Timer, WebRtcPolicy,
        };
    };

    /// The variants, constructors and methods `src/` names by their whole path.
    async fn paths_in_code(
        plugin: &dyn seleniumbase_rs_plugin::Plugin,
        page: &dyn seleniumbase_rs_plugin::PageHandle,
        browser: &dyn seleniumbase_rs_plugin::BrowserHandle,
    ) -> seleniumbase_rs_plugin::Result<()> {
        let _ = seleniumbase_rs_plugin::BrowserFamily::Chromium;
        let _ = seleniumbase_rs_plugin::Engine::Cdp;
        let _ = seleniumbase_rs_plugin::Category::Core;
        let _ = seleniumbase_rs_plugin::NavOrigin::Mcp;
        let _ = seleniumbase_rs_plugin::WebRtcPolicy::DisableNonProxiedUdp;
        let _ = seleniumbase_rs_plugin::WebRtcPolicy::DefaultPublicInterfaceOnly;
        let _ = seleniumbase_rs_plugin::Capability::Cdp("Network".into());
        let _ = seleniumbase_rs_plugin::Capability::NetObserve;
        let _ = seleniumbase_rs_plugin::Capability::TestHooks;
        let _ = seleniumbase_rs_plugin::ContextSettings::new();
        let _ = seleniumbase_rs_plugin::PluginSet::new();
        let _ = seleniumbase_rs_plugin::Metadata::new(seleniumbase_rs_plugin::Category::Core);
        let _ = seleniumbase_rs_plugin::PluginError::other("a failure");
        let _ = seleniumbase_rs_plugin::PluginError::unsupported("a thing");
        let _: Option<std::sync::Arc<dyn seleniumbase_rs_plugin::Plugin>> =
            seleniumbase_rs_plugin::Plugin::per_launch(plugin);
        let _: serde_json::Value =
            seleniumbase_rs_plugin::PageHandle::send(page, "Page.enable", serde_json::json!({}))
                .await?;
        let _: serde_json::Value = seleniumbase_rs_plugin::BrowserHandle::send(
            browser,
            "Browser.getVersion",
            serde_json::json!({}),
        )
        .await?;
        Ok(())
    }

    /// `src/plugin_host/mod.rs`, `builtin_registry`.
    #[cfg(feature = "plugin-shield")]
    fn the_builtin_registry() {
        let _: fn() -> seleniumbase_rs_plugin::Registry = seleniumbase_rs_plugins_builtin::registry;
    }
}

mod a_plugin_as_the_host_sees_it {
    use std::sync::Arc;

    use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
    use seleniumbase_rs::BrowserConfig;
    use seleniumbase_rs_plugin::{
        async_trait, BrowserFamily, Capability, Category, Configurable, Descriptor, Engine,
        LaunchContext, Metadata, PageHandle, Plugin, PluginError, PluginSet, Prompt, Registry,
        Sink, Task, TaskContext,
    };
    use serde_json::{json, Value};

    /// As `src/plugin_host/mod.rs` tests one: a plugin built from `{"mode": "..."}`.
    struct Dummy {
        mode: String,
    }

    #[async_trait]
    impl Plugin for Dummy {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn metadata(&self) -> Metadata {
            Self::describe()
        }
    }

    impl Configurable for Dummy {
        const NAME: &'static str = "dummy";

        fn describe() -> Metadata {
            Metadata::new(Category::Core).capability(Capability::LaunchArgs)
        }

        fn configure(options: &Value) -> seleniumbase_rs_plugin::Result<Self> {
            let mode = match options {
                Value::Null => "default".to_owned(),
                Value::Object(map) if map.keys().all(|key| key == "mode") => map
                    .get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or("default")
                    .to_owned(),
                other => {
                    return Err(PluginError::Config(format!("dummy: unexpected {other}")));
                }
            };
            Ok(Self { mode })
        }
    }

    /// A plugin that offers a task, as `sbrs plugin run` and the MCP server run one.
    struct Worker;

    #[async_trait]
    impl Plugin for Worker {
        fn name(&self) -> &str {
            Self::NAME
        }

        fn metadata(&self) -> Metadata {
            Self::describe()
        }

        fn task(&self) -> Option<&dyn Task> {
            Some(self)
        }
    }

    impl Configurable for Worker {
        const NAME: &'static str = "worker";

        fn describe() -> Metadata {
            Metadata::new(Category::Core).capability(Capability::Task)
        }

        fn configure(_options: &Value) -> seleniumbase_rs_plugin::Result<Self> {
            Ok(Self)
        }
    }

    #[async_trait]
    impl Task for Worker {
        async fn run(&self, cx: &TaskContext<'_>) -> seleniumbase_rs_plugin::Result<Value> {
            cx.sink.record(&json!({ "input": cx.input }))?;
            Ok(json!({ "done": true }))
        }
    }

    /// The sink and the prompt the command line and the MCP server hand to a task.
    struct Lines;

    impl Sink for Lines {
        fn record(&self, _line: &Value) -> seleniumbase_rs_plugin::Result<()> {
            Ok(())
        }
    }

    struct Nobody;

    #[async_trait]
    impl Prompt for Nobody {
        async fn wait(
            &self,
            _message: &str,
            _page: &dyn PageHandle,
        ) -> seleniumbase_rs_plugin::Result<()> {
            Err(PluginError::unsupported("waiting for a person"))
        }
    }

    #[test]
    fn a_set_holds_its_plugins_in_order_and_checks_a_launch() {
        let dummy = Dummy::configure(&Value::Null).expect("a plugin without options");
        assert_eq!(dummy.mode, "default");
        let set = PluginSet::new()
            .with(dummy)
            .expect("the first plugin")
            .with(Worker)
            .expect("a second one");
        assert_eq!(set.names(), ["dummy", "worker"]);
        assert!(!set.is_empty());
        let context = LaunchContext {
            engine: Engine::Cdp,
            family: BrowserFamily::Chromium,
            headless: true,
            connecting: false,
        };
        assert!(set.check(&context).is_ok());
        assert!(PluginSet::new().is_empty());
    }

    #[test]
    fn a_registry_builds_a_plugin_from_its_descriptor() {
        let registry = Registry::new()
            .with(Descriptor::of::<Dummy>())
            .with(Descriptor::of::<Worker>());
        let _: Registry = registry;
        let refused = Dummy::configure(&json!({ "other": 1 }))
            .err()
            .expect("refused");
        assert!(refused.to_string().contains("dummy"), "{refused}");
    }

    /// `LaunchOptionsBuilder::plugins` and `BrowserConfig::with_plugins`, the two launch paths.
    #[test]
    fn both_launch_paths_take_the_set() {
        let set = PluginSet::new().with(Worker).expect("a plugin");
        LaunchOptions::builder()
            .plugins(set.clone())
            .build()
            .expect("options with plugins build");
        let _config: BrowserConfig = BrowserConfig::default().with_plugins(set);
        // The re-export is the same crate: one `PluginSet` type, not two.
        let again: seleniumbase_rs::plugin_api::PluginSet = PluginSet::new();
        let _ = again;
    }

    async fn the_attach_path(set: PluginSet) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
        let browser = Browser::connect_with_plugins("127.0.0.1", 9222, set).await?;
        browser.close().await
    }

    fn a_plugin_error_becomes_the_crates_error(
        error: PluginError,
    ) -> seleniumbase_rs::SeleniumBaseError {
        error.into()
    }
}
