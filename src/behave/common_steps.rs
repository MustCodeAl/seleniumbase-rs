use crate::behave::steps::{step_pattern, StepRegistry};
use std::cell::RefCell;
use std::rc::Rc;

/// Helpers for registering common Gherkin step definitions.
///
/// These steps do not require a browser and are useful for unit-testing
/// feature files or mixing with user-registered browser steps.
///
/// Most of them are placeholders that pass whatever they are given, so a
/// feature can be run before its real steps are written. The one that checks
/// anything is [`register_value_steps`](Self::register_value_steps).
pub struct CommonSteps;

impl CommonSteps {
    /// Register a step that always passes (useful for placeholders).
    ///
    /// `pattern` is matched against the step as the runner looks it up, with
    /// its keyword: `^Given I do nothing$`, not `^I do nothing$`.
    pub fn register_noop(registry: &mut StepRegistry, pattern: &str) {
        registry.register_regex(pattern, Box::new(|_, _| Ok(())));
    }

    /// Register the placeholder steps `the value should be "x"` and `the
    /// value should not be "x"`.
    ///
    /// **They always pass.** There is no value for them to compare with. Use
    /// [`register_value_steps`](Self::register_value_steps) for steps that
    /// really assert. Do not register both: the earlier registration wins.
    pub fn register_assertion_steps(registry: &mut StepRegistry) {
        registry.register_regex(
            &step_pattern(r#"the value should be "(.+?)""#),
            Box::new(|_, _| Ok(())),
        );
        registry.register_regex(
            &step_pattern(r#"the value should not be "(.+?)""#),
            Box::new(|_, _| Ok(())),
        );
    }

    /// Register `I wait N seconds` (or `1 second`). The step does not wait: it
    /// lets a feature be run quickly. A real test should wait with the
    /// framework's own sleep.
    pub fn register_delay_steps(registry: &mut StepRegistry) {
        registry.register_regex(
            &step_pattern(r"I wait (\d+) seconds?"),
            Box::new(|_, _| {
                // Delays in real browser tests should use the test framework's sleep.
                Ok(())
            }),
        );
    }

    /// Register steps that keep one value and check it:
    ///
    /// - `I set the value to "x"` stores `x`,
    /// - `the value should be "x"` fails unless the stored value is `x`,
    /// - `the value should not be "x"` fails if it is.
    ///
    /// The value starts empty. The returned handle is the same value, so a
    /// test can read it, or set it, outside the feature.
    ///
    /// ```
    /// use seleniumbase_rs::behave::{run_feature_text, CommonSteps, RunFilter, StepRegistry};
    ///
    /// let mut registry = StepRegistry::new();
    /// let value = CommonSteps::register_value_steps(&mut registry);
    /// let feature = "Feature: f\nScenario: s\nGiven I set the value to \"5\"\n\
    ///                Then the value should be \"5\"\nAnd the value should not be \"6\"\n";
    /// let results = run_feature_text(feature, &registry, &RunFilter::all());
    /// assert!(results[0].passed, "{:?}", results[0].messages);
    /// assert_eq!(*value.borrow(), "5");
    /// ```
    pub fn register_value_steps(registry: &mut StepRegistry) -> Rc<RefCell<String>> {
        let value = Rc::new(RefCell::new(String::new()));
        let store = Rc::clone(&value);
        registry.register_regex(
            &step_pattern(r#"I set the value to "(.*)""#),
            Box::new(move |_, caps| {
                *store.borrow_mut() = caps[0].clone();
                Ok(())
            }),
        );
        let current = Rc::clone(&value);
        registry.register_regex(
            &step_pattern(r#"the value should be "(.*)""#),
            Box::new(move |_, caps| {
                let actual = current.borrow();
                if *actual == caps[0] {
                    Ok(())
                } else {
                    Err(format!(
                        "the value is {:?}, expected {:?}",
                        *actual, caps[0]
                    ))
                }
            }),
        );
        let current = Rc::clone(&value);
        registry.register_regex(
            &step_pattern(r#"the value should not be "(.*)""#),
            Box::new(move |_, caps| {
                if *current.borrow() == caps[0] {
                    Err(format!(
                        "the value is {:?}, expected anything else",
                        caps[0]
                    ))
                } else {
                    Ok(())
                }
            }),
        );
        value
    }

    /// Register a small set of generic filler steps for feature-file smoke tests.
    ///
    /// Every step registered here passes without checking anything.
    pub fn register_all(registry: &mut StepRegistry) {
        Self::register_noop(registry, &step_pattern("I do nothing"));
        Self::register_noop(registry, &step_pattern("the system is ready"));
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

    #[test]
    fn the_steps_are_found_with_the_keyword_the_runner_gives_them() {
        let mut registry = StepRegistry::new();
        CommonSteps::register_all(&mut registry);

        let results = crate::behave::run_feature_text(
            "Feature: f\nScenario: s\nGiven the system is ready\nWhen I wait 2 seconds\n\
             And I do nothing\nThen the value should be \"x\"\n",
            &registry,
            &crate::behave::RunFilter::all(),
        );

        assert!(results[0].passed, "{:?}", results[0].messages);
    }

    #[test]
    fn the_wait_step_matches_a_number_of_seconds() {
        let mut registry = StepRegistry::new();
        CommonSteps::register_delay_steps(&mut registry);
        assert!(registry.run("I wait 5 seconds").is_ok());
        assert!(registry.run("I wait 1 second").is_ok());
        assert!(registry.run("I wait many seconds").is_err());
    }

    #[test]
    fn the_placeholder_assertions_never_fail() {
        let mut registry = StepRegistry::new();
        CommonSteps::register_assertion_steps(&mut registry);
        assert!(registry.run("the value should be \"anything\"").is_ok());
        assert!(registry.run("the value should not be \"anything\"").is_ok());
    }

    #[test]
    fn the_value_steps_really_compare() {
        let mut registry = StepRegistry::new();
        let value = CommonSteps::register_value_steps(&mut registry);

        assert!(
            registry.run("the value should be \"\"").is_ok(),
            "starts empty"
        );
        registry.run("I set the value to \"abc\"").unwrap();
        assert_eq!(*value.borrow(), "abc");
        assert!(registry.run("the value should be \"abc\"").is_ok());
        let error = registry.run("the value should be \"abd\"").unwrap_err();
        assert!(
            error.contains("\"abc\"") && error.contains("\"abd\""),
            "{error}"
        );
        assert!(registry.run("the value should not be \"abd\"").is_ok());
        assert!(registry.run("the value should not be \"abc\"").is_err());
    }
}
