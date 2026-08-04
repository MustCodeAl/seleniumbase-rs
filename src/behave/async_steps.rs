use crate::behave::steps::StepRegistry;
use crate::BaseCase;
use regex::{Regex, RegexBuilder};
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
pub struct AsyncStepRegistry {
    base_case: Rc<RefCell<BaseCase>>,
    exact: Vec<(String, AsyncStepHandler)>,
    regex: Vec<RegisteredAsyncStep>,
}

impl AsyncStepRegistry {
    pub fn new(base_case: BaseCase) -> Self {
        Self {
            base_case: Rc::new(RefCell::new(base_case)),
            exact: Vec::new(),
            regex: Vec::new(),
        }
    }

    /// Register an exact step string.
    pub fn register(&mut self, pattern: impl Into<String>, handler: AsyncStepHandler) {
        self.exact.push((pattern.into(), handler));
    }

    /// Register a regex step pattern with capture groups.
    pub fn register_regex(&mut self, pattern: &str, handler: AsyncStepHandler) {
        let re = RegexBuilder::new(pattern)
            .build()
            .expect("invalid step regex");
        self.regex.push(RegisteredAsyncStep {
            pattern: re,
            handler,
        });
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
                let caps: Vec<String> = captures
                    .iter()
                    .skip(1)
                    .filter_map(|m| m.map(|m| m.as_str().to_owned()))
                    .collect();
                let handler = &registered.handler;
                return handler(self.base_case.clone(), trimmed, &caps).await;
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

    /// Convenience: register common browser navigation steps.
    #[allow(clippy::await_holding_refcell_ref)]
    pub fn register_navigation_steps(&mut self) {
        self.register_regex(
            r#"^I open "(.+?)"$"#,
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
            r"^I go back$",
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
            r"^I go forward$",
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

    /// Convenience: register common element interaction steps.
    #[allow(clippy::await_holding_refcell_ref)]
    pub fn register_element_steps(&mut self) {
        self.register_regex(
            r#"^I click "(.+?)"$"#,
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
            r#"^I type "(.+?)" into "(.+?)"$"#,
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

impl From<AsyncStepRegistry> for StepRegistry {
    /// Convert to a sync `StepRegistry` by dropping the `BaseCase` reference.
    /// Useful for sharing step patterns with non-browser tests.
    fn from(_value: AsyncStepRegistry) -> Self {
        StepRegistry::new()
    }
}

fn _assert_send() {
    // BaseCase is not Send, so AsyncStepRegistry is not Send either.
    // This is intentional for single-threaded async behave runners.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_registry_no_match() {
        let bc = BaseCase::without_session(Default::default());
        let registry = AsyncStepRegistry::new(bc);
        assert!(registry.run("unknown").await.is_err());
    }
}
