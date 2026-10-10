use crate::behave::steps::{capture_groups, step_pattern};
use crate::BaseCase;
use regex::Regex;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// Async step handler that can mutate a `BaseCase`.
pub type AsyncStepHandler = Box<
    dyn Fn(
        Rc<RefCell<BaseCase>>,
        &str,
        &[String],
    ) -> Pin<Box<dyn Future<Output = Result<(), String>>>>,
>;

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
/// [`run_feature_text_async`](crate::behave::run_feature_text_async).
pub struct AsyncStepRegistry {
    base_case: Rc<RefCell<BaseCase>>,
    exact: Vec<(String, AsyncStepHandler)>,
    regex: Vec<RegisteredAsyncStep>,
}

impl AsyncStepRegistry {
    /// Creates an empty registry whose handlers share `base_case`.
    pub fn new(base_case: BaseCase) -> Self {
        Self {
            base_case: Rc::new(RefCell::new(base_case)),
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
                return handler(self.base_case.clone(), trimmed, &[]).await;
            }
        }
        for registered in &self.regex {
            if let Some(captures) = registered.pattern.captures(trimmed) {
                let handler = &registered.handler;
                return handler(self.base_case.clone(), trimmed, &capture_groups(&captures)).await;
            }
        }
        Err(format!("no handler registered for step: {}", step))
    }

    /// Borrow the underlying `BaseCase` mutably.
    pub fn with_base<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut BaseCase) -> R,
    {
        f(&mut self.base_case.borrow_mut())
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
                    bc.borrow_mut()
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
                    bc.borrow_mut()
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
                    bc.borrow_mut()
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
                    bc.borrow_mut()
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
                    bc.borrow_mut()
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

    use crate::behave::{run_feature_text_async, RunFilter};

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
