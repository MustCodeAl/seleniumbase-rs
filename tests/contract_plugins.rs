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
//! * `the_imap_plugin_as_the_email_tool_uses_it` (feature `plugin-imap`): what the
//!   `wait_for_email` tool of `src/mcp/email.rs` imports of the `imap` plugin and the clock of the
//!   plugin API, the calls it makes, and a wait against a mailbox held in memory.

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
            Category, Clock, Configurable, ContextHandle, ContextId, ContextSettings, Descriptor,
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

/// The `imap` built-in plugin as the `wait_for_email` tool of `src/mcp/email.rs` uses it (feature
/// `plugin-imap`): the paths of its `use` lines, the calls it makes, and a wait that runs against
/// a mailbox held in memory, which is the part of the plugin the tool's own tests rely on.
#[cfg(feature = "plugin-imap")]
mod the_imap_plugin_as_the_email_tool_uses_it {
    use std::collections::HashSet;
    use std::future::{ready, Future};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use seleniumbase_rs_plugin::{async_trait, Clock, Configurable, PluginError, Timer};
    use seleniumbase_rs_plugins_builtin::imap::{
        wait_for, Found, Imap, ImapConfig, MailSource, Query, RawMail, DEFAULT_SINCE_SECS,
        MAX_TIMEOUT_SECS,
    };
    use serde_json::{json, Value};

    /// A clock that moves only when something sleeps: a sleep ends at once and adds its length.
    struct Stepped(AtomicU64);

    impl Timer for Stepped {
        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            let millis = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
            self.0.fetch_add(millis, Ordering::SeqCst);
            Box::pin(ready(()))
        }

        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    /// A mailbox that holds these messages.
    struct Inbox(Vec<RawMail>);

    #[async_trait]
    impl MailSource for Inbox {
        async fn fetch_new(
            &mut self,
            _since_secs: u64,
            seen: &HashSet<u32>,
        ) -> seleniumbase_rs_plugin::Result<Vec<RawMail>> {
            Ok(self
                .0
                .iter()
                .filter(|mail| !seen.contains(&mail.id))
                .cloned()
                .collect())
        }
    }

    fn mail(id: u32, from: &str, subject: &str, body: &str) -> RawMail {
        RawMail {
            id,
            bytes: format!("From: Shop <{from}>\r\nSubject: {subject}\r\n\r\n{body}\r\n")
                .into_bytes(),
        }
    }

    async fn waited(inbox: Vec<RawMail>, query: &Query) -> Result<Found, PluginError> {
        let clock = Clock::new(Arc::new(Stepped(AtomicU64::new(1_800_000_000_000))));
        wait_for(
            &mut Inbox(inbox),
            query,
            &clock,
            Duration::from_secs(1),
            Duration::from_secs(30),
        )
        .await
    }

    /// `build`: the plugin from the operator's options, and again with the wait a call asked for.
    fn the_tool_builds_the_plugin(
        options: &Value,
        requested: Option<u64>,
    ) -> Result<Imap, PluginError> {
        let configured = Imap::configure(options)?;
        let seconds = requested.map_or(configured.config().timeout_secs, |secs| {
            secs.min(MAX_TIMEOUT_SECS)
        });
        if seconds == configured.config().timeout_secs {
            return Ok(configured);
        }
        Imap::new(ImapConfig {
            timeout_secs: seconds,
            ..configured.config().clone()
        })
    }

    /// `wait_for_email`: the wait of the plugin, on the clock of the host.
    async fn the_tool_waits(imap: &Imap, query: &Query) -> Result<Found, PluginError> {
        let clock = Clock::new(Arc::new(Stepped(AtomicU64::new(0))));
        imap.wait_for_email(query, &clock).await
    }

    /// `request`: a call's arguments become a `Query`, with the limits the plugin names.
    fn the_tool_reads_a_query(input: &Value) -> Result<Query, PluginError> {
        let _: u64 = DEFAULT_SINCE_SECS;
        Query::parse(input)
    }

    /// `answer`: how the tool tells a failure from mail that did not come.
    fn how_the_tool_reads_a_plugin_error(error: PluginError) -> String {
        match error {
            PluginError::Config(message) | PluginError::Other(message) => message,
            PluginError::Timeout(after) => format!("{}s", after.as_secs()),
            other => other.to_string(),
        }
    }

    #[test]
    fn a_query_comes_from_the_arguments_of_a_call() {
        let query = the_tool_reads_a_query(&json!({
            "from": "no-reply@shop.example",
            "subject_contains": "verify",
            "since_secs": 120,
            "extract": "code",
        }))
        .expect("a query with a filter");
        assert_eq!(query.from.as_deref(), Some("no-reply@shop.example"));
        assert_eq!(query.subject_contains.as_deref(), Some("verify"));
        assert!(MAX_TIMEOUT_SECS >= 1);
    }

    #[tokio::test]
    async fn the_wait_finds_a_mail_and_reads_its_code() {
        let query = Query {
            from: Some("no-reply@shop.example".to_owned()),
            ..Query::default()
        };
        let inbox = vec![
            mail(1, "news@other.example", "Weekly", "Nothing here, 111111"),
            mail(
                2,
                "no-reply@shop.example",
                "Verify",
                "Your code is 482913. Thanks.",
            ),
        ];

        let found = waited(inbox, &query).await.expect("the mail is found");

        assert_eq!(found.from.as_deref(), Some("no-reply@shop.example"));
        assert_eq!(found.subject, "Verify");
        assert_eq!(found.code.as_deref(), Some("482913"));
        assert!(found.text.contains("Thanks"), "{}", found.text);
        let _: (Option<String>, Option<String>) = (found.date, found.link);
    }

    #[tokio::test]
    async fn mail_that_does_not_come_in_time_is_a_message_the_tool_can_tell_apart() {
        let query = Query {
            subject_contains: Some("verify".to_owned()),
            ..Query::default()
        };
        let inbox = vec![mail(1, "news@other.example", "Weekly", "code 111111")];

        let error = waited(inbox, &query).await.expect_err("no mail matches");

        // The tool answers `found: false` for this message and an error for every other one.
        let message = how_the_tool_reads_a_plugin_error(error);
        assert!(message.starts_with("imap: no email ("), "{message}");
        assert!(message.contains("within 30s"), "{message}");
    }
}
