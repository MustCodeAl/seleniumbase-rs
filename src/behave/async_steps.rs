use crate::behave::steps::{capture_groups, step_pattern};
use crate::BaseCase;
use regex::Regex;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

/// The [`BaseCase`] the steps of an [`AsyncStepRegistry`] share.
///
/// A step locks it for as long as it uses the browser:
/// `bc.lock().await.open(&url).await`.
pub type SharedBaseCase = Arc<Mutex<BaseCase>>;

/// What an async step handler returns: a future that settles with the step's
/// error message, if it failed.
///
/// It must be `Send` and cannot borrow from the handler's arguments, so copy
/// what it needs out of them before the `async` block.
pub type AsyncStepFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

/// Async step handler that can mutate a `BaseCase`.
///
/// It gets the shared `BaseCase`, the step text and the capture groups, and
/// must be `Send + Sync` so the registry can be shared between threads.
pub type AsyncStepHandler =
    Box<dyn Fn(SharedBaseCase, &str, &[String]) -> AsyncStepFuture + Send + Sync>;

struct RegisteredAsyncStep {
    pattern: Regex,
    handler: AsyncStepHandler,
}

/// Async behave step registry backed by a `BaseCase` instance.
///
/// It matches steps the way [`StepRegistry`](crate::behave::StepRegistry)
/// does: exact steps first, then regex patterns in registration order, and a
/// group that took no part in a match is passed as an empty string. Run a
/// whole feature with
/// [`run_feature_text_async`](crate::behave::run_feature_text_async), or a
/// file with [`run_feature_file_async`](crate::behave::run_feature_file_async).
///
/// The registry is `Send + Sync`, so it can sit in an `Arc` and a run can be
/// handed to `tokio::spawn`. Steps run one at a time and take the shared
/// `BaseCase` by locking it; [`into_base_case`](Self::into_base_case) hands it
/// back when the run is over.
pub struct AsyncStepRegistry {
    base_case: SharedBaseCase,
    exact: Vec<(String, AsyncStepHandler)>,
    regex: Vec<RegisteredAsyncStep>,
}

impl AsyncStepRegistry {
    /// Creates an empty registry whose handlers share `base_case`.
    pub fn new(base_case: BaseCase) -> Self {
        Self {
            base_case: Arc::new(Mutex::new(base_case)),
            exact: Vec::new(),
            regex: Vec::new(),
        }
    }

    /// Register an exact step string.
    ///
    /// Registering the same string again replaces the earlier handler, as it
    /// does in the sync registry.
    pub fn register(&mut self, pattern: impl Into<String>, handler: AsyncStepHandler) {
        let pattern = pattern.into();
        match self.exact.iter_mut().find(|(known, _)| *known == pattern) {
            Some(entry) => entry.1 = handler,
            None => self.exact.push((pattern, handler)),
        }
    }

    /// Register a regex step pattern with capture groups.
    ///
    /// # Panics
    ///
    /// Panics if `pattern` is not a valid regular expression; use
    /// [`try_register_regex`](Self::try_register_regex) for one that comes from
    /// outside the program.
    pub fn register_regex(&mut self, pattern: &str, handler: AsyncStepHandler) {
        if let Err(error) = self.try_register_regex(pattern, handler) {
            panic!("invalid step regex {pattern:?}: {error}");
        }
    }

    /// Like [`register_regex`](Self::register_regex), but reports an invalid
    /// pattern instead of panicking.
    pub fn try_register_regex(
        &mut self,
        pattern: &str,
        handler: AsyncStepHandler,
    ) -> Result<(), regex::Error> {
        let pattern = Regex::new(pattern)?;
        self.regex.push(RegisteredAsyncStep { pattern, handler });
        Ok(())
    }

    /// Whether [`run`](Self::run) would find a handler for `step`.
    pub fn has_handler(&self, step: &str) -> bool {
        let trimmed = step.trim();
        self.exact.iter().any(|(pattern, _)| pattern == trimmed)
            || self.regex.iter().any(|r| r.pattern.is_match(trimmed))
    }

    /// Run a step against the registry.
    pub async fn run(&self, step: &str) -> Result<(), String> {
        let trimmed = step.trim();
        for (pattern, handler) in &self.exact {
            if pattern == trimmed {
                return handler(Arc::clone(&self.base_case), trimmed, &[]).await;
            }
        }
        for registered in &self.regex {
            if let Some(captures) = registered.pattern.captures(trimmed) {
                let handler = &registered.handler;
                return handler(
                    Arc::clone(&self.base_case),
                    trimmed,
                    &capture_groups(&captures),
                )
                .await;
            }
        }
        Err(format!("no handler registered for step: {}", step))
    }

    /// Runs `f` on the underlying `BaseCase`, once no step is using it.
    pub async fn with_base<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut BaseCase) -> R,
    {
        f(&mut *self.base_case.lock().await)
    }

    /// The `BaseCase` the steps share, for code that needs to drive the browser
    /// between runs: `registry.base_case().lock().await`.
    pub fn base_case(&self) -> SharedBaseCase {
        Arc::clone(&self.base_case)
    }

    /// Takes the `BaseCase` out of the registry, for example to `quit` it after
    /// the run.
    ///
    /// # Errors
    ///
    /// Gives the registry back if something still holds the shared `BaseCase`,
    /// such as a [`base_case`](Self::base_case) handle you have not dropped.
    pub fn into_base_case(self) -> Result<BaseCase, Self> {
        let Self {
            base_case,
            exact,
            regex,
        } = self;
        match Arc::try_unwrap(base_case) {
            Ok(mutex) => Ok(mutex.into_inner()),
            Err(base_case) => Err(Self {
                base_case,
                exact,
                regex,
            }),
        }
    }

    /// Convenience: register common browser navigation steps: `I open "url"`,
    /// `I go back` and `I go forward`, with or without a leading `Given`,
    /// `When`, `Then`, `And` or `But`.
    #[allow(clippy::await_holding_refcell_ref)]
    pub fn register_navigation_steps(&mut self) {
        self.register_regex(
            &step_pattern(r#"I open "(.+?)""#),
            Box::new(|bc, _step, caps| {
                let url = caps[0].clone();
                Box::pin(async move {
                    bc.lock()
                        .await
                        .open(&url)
                        .await
                        .map_err(|e| format!("open failed: {}", e))
                })
            }),
        );
        self.register_regex(
            &step_pattern("I go back"),
            Box::new(|bc, _step, _caps| {
                Box::pin(async move {
                    bc.lock()
                        .await
                        .go_back()
                        .await
                        .map_err(|e| format!("go_back failed: {}", e))
                })
            }),
        );
        self.register_regex(
            &step_pattern("I go forward"),
            Box::new(|bc, _step, _caps| {
                Box::pin(async move {
                    bc.lock()
                        .await
                        .go_forward()
                        .await
                        .map_err(|e| format!("go_forward failed: {}", e))
                })
            }),
        );
    }

    /// Convenience: register common element interaction steps: `I click
    /// "selector"` and `I type "text" into "selector"`, with or without a
    /// leading keyword.
    #[allow(clippy::await_holding_refcell_ref)]
    pub fn register_element_steps(&mut self) {
        self.register_regex(
            &step_pattern(r#"I click "(.+?)""#),
            Box::new(|bc, _step, caps| {
                let selector = caps[0].clone();
                Box::pin(async move {
                    bc.lock()
                        .await
                        .click(&selector)
                        .await
                        .map_err(|e| format!("click failed: {}", e))
                })
            }),
        );
        self.register_regex(
            &step_pattern(r#"I type "(.+?)" into "(.+?)""#),
            Box::new(|bc, _step, caps| {
                let text = caps[0].clone();
                let selector = caps[1].clone();
                Box::pin(async move {
                    bc.lock()
                        .await
                        .type_text(&selector, &text)
                        .await
                        .map_err(|e| format!("type failed: {}", e))
                })
            }),
        );
    }

    /// Register a small default set of browser steps.
    pub fn register_common_browser_steps(&mut self) {
        self.register_navigation_steps();
        self.register_element_steps();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::behave::{run_feature_file_with_filter_async, run_feature_text_async, RunFilter};

    fn registry() -> AsyncStepRegistry {
        AsyncStepRegistry::new(BaseCase::without_session(Default::default()))
    }

    /// An exact-step handler that fails with `message`.
    fn failing(message: &'static str) -> AsyncStepHandler {
        Box::new(move |_, _, _| Box::pin(async move { Err(message.to_owned()) }))
    }

    #[tokio::test]
    async fn test_async_registry_no_match() {
        let bc = BaseCase::without_session(Default::default());
        let registry = AsyncStepRegistry::new(bc);
        assert!(registry.run("unknown").await.is_err());
    }

    #[tokio::test]
    async fn registering_a_step_again_replaces_it() {
        let mut registry = registry();
        registry.register("Given x", failing("first"));
        registry.register("Given x", failing("second"));
        assert_eq!(registry.run("Given x").await.unwrap_err(), "second");
    }

    #[tokio::test]
    async fn a_group_that_did_not_match_keeps_its_position() {
        let mut registry = registry();
        registry.register_regex(
            r#"^I click( twice)? on "(.+?)"$"#,
            Box::new(|_, _, caps| {
                let caps = caps.to_vec();
                Box::pin(async move { Err(caps.join("|")) })
            }),
        );
        assert_eq!(registry.run(r#"I click on "OK""#).await.unwrap_err(), "|OK");
        assert_eq!(
            registry.run(r#"I click twice on "OK""#).await.unwrap_err(),
            " twice|OK"
        );
    }

    #[test]
    fn has_handler_sees_exact_and_regex_steps() {
        let mut registry = registry();
        registry.register("Given exact", failing("x"));
        registry.register_regex(r"^Given re\d$", failing("x"));
        assert!(registry.has_handler("Given exact"));
        assert!(registry.has_handler("  Given re7  "));
        assert!(!registry.has_handler("Given other"));
        assert!(registry
            .try_register_regex("(unclosed", failing("x"))
            .is_err());
    }

    #[tokio::test]
    async fn a_feature_runs_against_the_async_registry() {
        let mut registry = registry();
        registry.register(
            "Given a cat",
            Box::new(|_, _, _| Box::pin(async { Ok(()) })),
        );
        registry.register("Then it purrs", failing("it hisses"));

        let results = run_feature_text_async(
            "@pets\nFeature: f\nScenario: ok\nGiven a cat\nAnd a cat\n\
             Scenario: broken\nGiven a cat\nThen it purrs\n",
            &registry,
            &RunFilter {
                include_tags: vec!["pets".into()],
                exclude_tags: vec![],
            },
        )
        .await;

        assert_eq!(results.len(), 2);
        assert!(results[0].passed, "{:?}", results[0].messages);
        assert_eq!(
            results[1].messages,
            ["step 'Then it purrs' failed: it hisses"]
        );
    }

    /// A handler that adds the text of the step it runs to `seen`.
    fn recording(seen: &Arc<std::sync::Mutex<Vec<String>>>) -> AsyncStepHandler {
        let seen = Arc::clone(seen);
        Box::new(move |_, step, _| {
            let (step, seen) = (step.to_owned(), Arc::clone(&seen));
            Box::pin(async move {
                seen.lock().unwrap().push(step);
                Ok(())
            })
        })
    }

    /// Compiled, never called: the registry can be shared between threads and
    /// a run can be moved to another task.
    #[expect(dead_code, reason = "exists only to be type-checked")]
    fn the_registry_and_its_runs_are_send(registry: &AsyncStepRegistry) {
        fn send<T: Send>(_: T) {}
        fn send_and_sync<T: Send + Sync>() {}
        send_and_sync::<AsyncStepRegistry>();
        send_and_sync::<AsyncStepHandler>();
        send(registry.run("Given x"));
        send(registry.with_base(|_| ()));
        send(run_feature_text_async("", registry, &RunFilter::all()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_run_can_be_spawned_on_a_multi_threaded_runtime() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut registry = registry();
        registry.register("Given a cat", recording(&seen));
        registry.register("Then it purrs", recording(&seen));
        let registry = Arc::new(registry);

        let shared = Arc::clone(&registry);
        let results = tokio::spawn(async move {
            run_feature_text_async(
                "Feature: f\nScenario: s\nGiven a cat\nThen it purrs\n",
                &shared,
                &RunFilter::all(),
            )
            .await
        })
        .await
        .unwrap();

        assert!(results[0].passed, "{:?}", results[0].messages);
        assert_eq!(*seen.lock().unwrap(), ["Given a cat", "Then it purrs"]);
    }

    #[tokio::test]
    async fn a_feature_file_with_a_background_and_an_outline_runs_against_the_async_registry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("count.feature");
        std::fs::write(
            &path,
            "@web\nFeature: counting\n\
             Background:\nGiven setup\n\
             Scenario Outline: add <n>\nWhen I add \"<n>\"\nAnd I add \"<n>\"\n\
             Examples:\n| n |\n| 1 |\n| 2 |\n\
             @wip\nScenario: unfinished\nWhen I add \"9\"\n",
        )
        .unwrap();
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut registry = registry();
        // `And` continues the `When` before it, as in the sync runner.
        registry.register_regex(r"^(?:Given|When) (.+)$", recording(&seen));

        let filter = RunFilter {
            include_tags: vec!["web".into()],
            exclude_tags: vec!["wip".into()],
        };
        let results = run_feature_file_with_filter_async(&path, &registry, &filter)
            .await
            .unwrap();

        assert_eq!(results.len(), 2, "{results:?}");
        assert!(results.iter().all(|r| r.passed), "{results:?}");
        assert_eq!(results[1].name, "add 2 (example 2)");
        assert_eq!(
            *seen.lock().unwrap(),
            [
                "Given setup",
                "When I add \"1\"",
                "When I add \"1\"",
                "Given setup",
                "When I add \"2\"",
                "When I add \"2\"",
            ]
        );
    }

    #[tokio::test]
    async fn the_base_case_comes_back_when_nothing_else_holds_it() {
        let registry = registry();
        let held = registry.base_case();
        let registry = registry
            .into_base_case()
            .err()
            .expect("a held BaseCase stays in the registry");

        assert_eq!(registry.with_base(|_| 42).await, 42);
        drop(held);
        assert!(registry.into_base_case().is_ok());
    }

    #[test]
    fn the_built_in_browser_steps_match_with_any_keyword() {
        // The runner looks a step up with its keyword, so a feature file's
        // `When I open "..."` must find the handler registered for `I open`.
        let mut registry = registry();
        registry.register_common_browser_steps();

        for step in [
            "I open \"https://x.test\"",
            "Given I open \"https://x.test\"",
            "When I go back",
            "And I go forward",
            "Then I click \"#a\"",
            "But I type \"hi\" into \"#q\"",
        ] {
            assert!(registry.has_handler(step), "{step}");
        }
        assert!(!registry.has_handler("Given I dance"));
    }
}
