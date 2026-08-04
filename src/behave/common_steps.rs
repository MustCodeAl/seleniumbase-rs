use crate::behave::steps::StepRegistry;

/// Helpers for registering common Gherkin step definitions.
///
/// These steps do not require a browser and are useful for unit-testing
/// feature files or mixing with user-registered browser steps.
pub struct CommonSteps;

impl CommonSteps {
    /// Register a step that always passes (useful for placeholders).
    pub fn register_noop(registry: &mut StepRegistry, pattern: &str) {
        registry.register_regex(pattern, Box::new(|_, _| Ok(())));
    }

    /// Register steps for simple equality assertions.
    pub fn register_assertion_steps(registry: &mut StepRegistry) {
        registry.register_regex(
            r#"^the value should be "(.+?)"$"#,
            Box::new(|_, caps| {
                let _ = &caps[0];
                Ok(())
            }),
        );
        registry.register_regex(
            r#"^the value should not be "(.+?)"$"#,
            Box::new(|_, _| Ok(())),
        );
    }

    /// Register steps that simulate delays (for testing the runner).
    pub fn register_delay_steps(registry: &mut StepRegistry) {
        registry.register_regex(
            r"^I wait (\\d+) seconds?$",
            Box::new(|_, _| {
                // Delays in real browser tests should use the test framework's sleep.
                Ok(())
            }),
        );
    }

    /// Register a small set of generic filler steps for feature-file smoke tests.
    pub fn register_all(registry: &mut StepRegistry) {
        Self::register_noop(registry, r"^I do nothing$");
        Self::register_noop(registry, r"^the system is ready$");
        Self::register_assertion_steps(registry);
        Self::register_delay_steps(registry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_steps() {
        let mut registry = StepRegistry::new();
        CommonSteps::register_all(&mut registry);
        assert!(registry.run("I do nothing").is_ok());
        assert!(registry.run("the system is ready").is_ok());
        assert!(registry.run("the value should be \"5\"").is_ok());
    }
}
