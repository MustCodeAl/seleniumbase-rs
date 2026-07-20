use regex::{Regex, RegexBuilder};
use std::collections::HashMap;

/// Captured groups extracted from a matched step pattern.
pub type StepCaptures = Vec<String>;

/// A step handler that receives the full step and any regex captures.
pub type StepHandler = Box<dyn Fn(&str, &[String]) -> Result<(), String>>;

struct RegisteredStep {
    pattern: Regex,
    handler: StepHandler,
}

/// Registry mapping step patterns (exact strings or regexes) to handlers.
pub struct StepRegistry {
    exact: HashMap<String, StepHandler>,
    regex: Vec<RegisteredStep>,
}

impl StepRegistry {
    pub fn new() -> Self {
        Self {
            exact: HashMap::new(),
            regex: Vec::new(),
        }
    }

    /// Register an exact step string with a handler.
    pub fn register(&mut self, pattern: impl Into<String>, handler: StepHandler) {
        self.exact.insert(pattern.into(), handler);
    }

    /// Register a regex pattern with a handler. Capture groups are passed to the handler.
    ///
    /// # Example
    /// ```
    /// use seleniumbase_rs::behave::steps::StepRegistry;
    /// let mut registry = StepRegistry::new();
    /// registry.register_regex(
    ///     r#"^I enter "(.+?)" into "(.+?)"$"#,
    ///     Box::new(|step, caps| {
    ///         assert_eq!(caps.len(), 2);
    ///         Ok(())
    ///     }),
    /// );
    /// registry.run(r#"I enter "hello" into "world""#).unwrap();
    /// ```
    pub fn register_regex(&mut self, pattern: &str, handler: StepHandler) {
        let re = RegexBuilder::new(pattern)
            .build()
            .expect("invalid step regex");
        self.regex.push(RegisteredStep {
            pattern: re,
            handler,
        });
    }

    /// Find and run the handler for a step.
    pub fn run(&self, step: &str) -> Result<(), String> {
        let trimmed = step.trim();
        if let Some(handler) = self.exact.get(trimmed) {
            return handler(trimmed, &Vec::new());
        }
        for registered in &self.regex {
            if let Some(captures) = registered.pattern.captures(trimmed) {
                let caps: Vec<String> = captures
                    .iter()
                    .skip(1)
                    .filter_map(|m| m.map(|m| m.as_str().to_owned()))
                    .collect();
                return (registered.handler)(trimmed, &caps);
            }
        }
        Err(format!("no handler registered for step: {}", step))
    }

    pub fn has_handler(&self, step: &str) -> bool {
        let trimmed = step.trim();
        if self.exact.contains_key(trimmed) {
            return true;
        }
        self.regex.iter().any(|r| r.pattern.is_match(trimmed))
    }
}

impl Default for StepRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let mut registry = StepRegistry::new();
        registry.register("Given one", Box::new(|_, _| Ok(())));
        assert!(registry.run("Given one").is_ok());
        assert!(registry.run("Given two").is_err());
    }

    #[test]
    fn test_regex_match_with_captures() {
        let mut registry = StepRegistry::new();
        registry.register_regex(
            r#"^I enter "(.+?)" into "(.+?)"$"#,
            Box::new(|_, caps| {
                assert_eq!(caps[0], "hello");
                assert_eq!(caps[1], "world");
                Ok(())
            }),
        );
        assert!(registry.run(r#"I enter "hello" into "world""#).is_ok());
    }

    #[test]
    fn test_has_handler() {
        let mut registry = StepRegistry::new();
        registry.register_regex(r#"^click "(.+?)"$"#, Box::new(|_, _| Ok(())));
        assert!(registry.has_handler(r#"click "Submit""#));
        assert!(!registry.has_handler("unknown step"));
    }
}
