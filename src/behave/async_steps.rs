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

    /// Register a handler for the step `text_regex`, whatever keyword it is
    /// written with, or none: the way behave's `@step` matches.
    ///
    /// `text_regex` is the step text as a regex, without its keyword and
    /// without `^` or `$`; its capture groups go to the handler. It is
    /// [`register_regex`](Self::register_regex) with
    /// [`step_pattern`](crate::behave::step_pattern).
    ///
    /// # Panics
    ///
    /// Panics if `text_regex` is not a valid regular expression; use
    /// [`try_register_step`](Self::try_register_step) for one that comes from
    /// outside the program.
    pub fn register_step(&mut self, text_regex: &str, handler: AsyncStepHandler) {
        self.register_regex(&step_pattern(text_regex), handler);
    }

    /// Like [`register_step`](Self::register_step), but reports an invalid
    /// pattern instead of panicking.
    ///
    /// # Errors
    ///
    /// Returns the regex error if `text_regex` does not compile.
    pub fn try_register_step(
        &mut self,
        text_regex: &str,
        handler: AsyncStepHandler,
    ) -> Result<(), regex::Error> {
        self.try_register_regex(&step_pattern(text_regex), handler)
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

    /// Convenience: register common browser navigation steps, with or without
    /// a leading `Given`, `When`, `Then`, `And` or `But`.
    ///
    /// They are `I open "url"`, `I go back` and `I go forward`, and Python
    /// SeleniumBase's wording for the same, which its `--rec-behave` recorder
    /// writes: `Open "url"` (also `Open 'url'` and `Open URL "url"`), `Go back`
    /// and `Go forward`.
    pub fn register_navigation_steps(&mut self) {
        self.register_step(r#"I open "(.+?)""#, open_step());
        self.register_step(r#"Open(?: URL)? (?:"(.+?)"|'(.+?)')"#, open_step());
        self.register_step("(?:I go|Go) back", back_step());
        self.register_step("(?:I go|Go) forward", forward_step());
    }

    /// Convenience: register common element interaction steps, with or without
    /// a leading keyword.
    ///
    /// They are `I click "selector"` and `I type "text" into "selector"`, and
    /// Python SeleniumBase's wording for the same, which its `--rec-behave`
    /// recorder writes: `Click "selector"` (also `Click 'selector'` and `Click
    /// element "selector"`) and `Type "text" into "selector"` (also with
    /// single quotes, `Type text ...` and `in` for `into`).
    pub fn register_element_steps(&mut self) {
        self.register_step(r#"I click "(.+?)""#, click_step());
        self.register_step(r#"Click(?: element)? (?:"(.+?)"|'(.+?)')"#, click_step());
        self.register_step(r#"I type "(.+?)" into "(.+?)""#, type_step());
        self.register_step(
            r#"Type(?: text)? (?:"(.+?)"|'(.+?)') (?:into|in) (?:"(.+?)"|'(.+?)')"#,
            type_step(),
        );
    }

    /// Register a small default set of browser steps.
    pub fn register_common_browser_steps(&mut self) {
        self.register_navigation_steps();
        self.register_element_steps();
    }
}

/// The one capture a built-in step took, from whichever quote style matched.
///
/// A pattern with a double-quoted and a single-quoted choice has a group for
/// each; the one that did not take part is empty.
fn quoted(groups: &[String]) -> String {
    groups.concat()
}

/// The text and the selector of a typing step: half of the groups each, since
/// each side of Python's wording has one group per quote style.
fn text_and_selector(groups: &[String]) -> (String, String) {
    let (text, selector) = groups.split_at(groups.len() / 2);
    (quoted(text), quoted(selector))
}

fn open_step() -> AsyncStepHandler {
    Box::new(|bc, _step, caps| {
        let url = quoted(caps);
        Box::pin(async move {
            let mut sb = bc.lock().await;
            sb.open(&url).await.map_err(|e| format!("open failed: {e}"))
        })
    })
}

fn back_step() -> AsyncStepHandler {
    Box::new(|bc, _step, _caps| {
        Box::pin(async move {
            let sb = bc.lock().await;
            sb.go_back()
                .await
                .map_err(|e| format!("go_back failed: {e}"))
        })
    })
}

fn forward_step() -> AsyncStepHandler {
    Box::new(|bc, _step, _caps| {
        Box::pin(async move {
            let sb = bc.lock().await;
            sb.go_forward()
                .await
                .map_err(|e| format!("go_forward failed: {e}"))
        })
    })
}

fn click_step() -> AsyncStepHandler {
    Box::new(|bc, _step, caps| {
        let selector = quoted(caps);
        Box::pin(async move {
            let mut sb = bc.lock().await;
            sb.click(&selector)
                .await
                .map_err(|e| format!("click failed: {e}"))
        })
    })
}

fn type_step() -> AsyncStepHandler {
    Box::new(|bc, _step, caps| {
        let (text, selector) = text_and_selector(caps);
        Box::pin(async move {
            let mut sb = bc.lock().await;
            sb.type_text(&selector, &text)
                .await
                .map_err(|e| format!("type failed: {e}"))
        })
    })
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
    async fn a_top_level_alternation_is_anchored_as_a_whole() {
        let mut registry = registry();
        registry.register_step("yes|no", Box::new(|_, _, _| Box::pin(async { Ok(()) })));

        assert!(registry.has_handler("yes"));
        assert!(registry.has_handler("Given no"));
        assert!(!registry.has_handler("yes please"));
        assert!(!registry.has_handler("Given I said no"));
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

    #[tokio::test]
    async fn register_step_takes_any_keyword_or_none_and_passes_the_captures() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut registry = registry();
        let sink = Arc::clone(&seen);
        registry.register_step(
            r#"I open "(.+?)""#,
            Box::new(move |_, _, caps| {
                let (url, sink) = (caps[0].clone(), Arc::clone(&sink));
                Box::pin(async move {
                    sink.lock().unwrap().push(url);
                    Ok(())
                })
            }),
        );

        for step in [
            r#"Given I open "a""#,
            r#"And I open "b""#,
            r#"But I open "c""#,
            r#"I open "d""#,
        ] {
            registry.run(step).await.unwrap();
        }

        assert_eq!(*seen.lock().unwrap(), ["a", "b", "c", "d"]);
        assert!(!registry.has_handler(r#"Given I open the door"#));
        assert!(registry
            .try_register_step("(unclosed", failing("x"))
            .is_err());
    }

    #[test]
    fn the_built_in_steps_also_know_pythons_wording() {
        // What SeleniumBase's `--rec-behave` writes, with or without a keyword.
        let mut registry = registry();
        registry.register_common_browser_steps();

        for step in [
            r#"Given Open "https://x.test""#,
            "When Open 'https://x.test'",
            r#"Open URL "https://x.test""#,
            "And Open URL 'https://x.test'",
            "Then Go back",
            "But Go forward",
            r##"Click "#a""##,
            "When Click 'a.b'",
            r##"And Click element "#a""##,
            r##"Type text "hi" into "#q""##,
            "When Type text 'hi' into '#q'",
            r##"Type text "hi" into '#q'"##,
        ] {
            assert!(registry.has_handler(step), "{step}");
        }
        for step in ["Given Dance", "Open it", "Go sideways", "Click"] {
            assert!(!registry.has_handler(step), "{step}");
        }
    }

    #[test]
    fn a_built_in_step_reads_its_quote_style_from_the_groups_that_matched() {
        // `I type "hi" into "#q"` has one group each; Python's wording has a
        // group per quote style, and the other one is empty.
        let groups = |parts: &[&str]| parts.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>();

        assert_eq!(quoted(&groups(&["#a"])), "#a");
        assert_eq!(quoted(&groups(&["", "#a"])), "#a");
        assert_eq!(
            text_and_selector(&groups(&["hi", "#q"])),
            ("hi".to_owned(), "#q".to_owned())
        );
        assert_eq!(
            text_and_selector(&groups(&["", "hi", "#q", ""])),
            ("hi".to_owned(), "#q".to_owned())
        );
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
