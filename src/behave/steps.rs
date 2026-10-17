//! Step definitions: the registry that maps the text of a Gherkin step to the
//! code that runs it.

use regex::{Captures, Regex};
use std::collections::HashMap;

/// Captured groups extracted from a matched step pattern.
pub type StepCaptures = Vec<String>;

/// A step handler that receives the full step and any regex captures.
pub type StepHandler = Box<dyn Fn(&str, &[String]) -> Result<(), String>>;

/// A regex for the step `text`, with or without a leading step keyword.
///
/// The runner looks a step up with its keyword (`Then the value should be
/// "5"`), so a pattern that starts at `^the value` would never match a step
/// run from a feature file. This one matches `Then the value ...`, `And the
/// value ...` and the bare `the value ...`. `text` is a regex and may hold
/// capture groups; it is not anchored, this adds the anchors around the whole
/// of it, so a top-level alternation such as `yes|no` matches `yes` or `no`
/// and nothing longer.
///
/// [`StepRegistry::register_step`] and
/// [`AsyncStepRegistry::register_step`](crate::behave::AsyncStepRegistry::register_step)
/// register a handler under this pattern; use `step_pattern` itself with
/// `register_regex` to combine it with other regex handling.
///
/// ```
/// use regex::Regex;
/// use seleniumbase_rs::behave::step_pattern;
///
/// let pattern = Regex::new(&step_pattern(r#"I open "(.+?)""#))?;
/// assert!(pattern.is_match(r#"Given I open "https://example.com""#));
/// assert!(pattern.is_match(r#"I open "https://example.com""#));
/// assert!(!pattern.is_match(r#"Given I opened "https://example.com""#));
/// # Ok::<(), regex::Error>(())
/// ```
pub fn step_pattern(text: &str) -> String {
    // `text` sits in a non-capturing group, so a top-level `a|b` is anchored
    // as a whole and the numbering of its own groups does not change.
    format!(r"^(?:(?:Given|When|Then|And|But|\*)\s+)?(?:{text})$")
}

/// The capture groups of a match, group 1 first.
///
/// A group that took no part in the match (an optional group, or the other
/// side of an alternation) is an empty string, not left out: `caps[n]` is
/// always the pattern's group `n + 1`, whichever way the step matched.
pub(crate) fn capture_groups(captures: &Captures<'_>) -> Vec<String> {
    captures
        .iter()
        .skip(1)
        .map(|group| group.map_or_else(String::new, |m| m.as_str().to_owned()))
        .collect()
}

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
    ///
    /// Registering the same string again replaces the earlier handler.
    pub fn register(&mut self, pattern: impl Into<String>, handler: StepHandler) {
        self.exact.insert(pattern.into(), handler);
    }

    /// Register a regex pattern with a handler. Capture groups are passed to the handler.
    ///
    /// Patterns are tried in the order they were registered, after the exact
    /// steps; the first match runs. A group that took no part in the match is
    /// passed as an empty string, so the position of a group never changes.
    ///
    /// # Panics
    ///
    /// Panics if `pattern` is not a valid regular expression. Use
    /// [`try_register_regex`](Self::try_register_regex) for a pattern that
    /// comes from outside the program.
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
        if let Err(error) = self.try_register_regex(pattern, handler) {
            panic!("invalid step regex {pattern:?}: {error}");
        }
    }

    /// Like [`register_regex`](Self::register_regex), but reports an invalid
    /// pattern instead of panicking.
    ///
    /// ```
    /// use seleniumbase_rs::behave::StepRegistry;
    ///
    /// let mut registry = StepRegistry::new();
    /// assert!(registry.try_register_regex("(unclosed", Box::new(|_, _| Ok(()))).is_err());
    /// assert!(registry.try_register_regex(r"^I wait$", Box::new(|_, _| Ok(()))).is_ok());
    /// ```
    pub fn try_register_regex(
        &mut self,
        pattern: &str,
        handler: StepHandler,
    ) -> Result<(), regex::Error> {
        let pattern = Regex::new(pattern)?;
        self.regex.push(RegisteredStep { pattern, handler });
        Ok(())
    }

    /// Register a handler for the step `text_regex`, whatever keyword it is
    /// written with, or none: the way behave's `@step` matches.
    ///
    /// `text_regex` is the step text as a regex, without its keyword and
    /// without `^` or `$`; its capture groups go to the handler. It is
    /// [`register_regex`](Self::register_regex) with [`step_pattern`].
    ///
    /// # Panics
    ///
    /// Panics if `text_regex` is not a valid regular expression; use
    /// [`try_register_step`](Self::try_register_step) for one that comes from
    /// outside the program.
    ///
    /// # Example
    /// ```
    /// use seleniumbase_rs::behave::{run_feature_text, RunFilter, StepRegistry};
    ///
    /// let mut registry = StepRegistry::new();
    /// registry.register_step(r#"I have (\d+) cats"#, Box::new(|_, caps| {
    ///     caps[0].parse::<u32>().map(|_| ()).map_err(|e| e.to_string())
    /// }));
    ///
    /// let results = run_feature_text(
    ///     "Feature: f\nScenario: s\nGiven I have 3 cats\nAnd I have 4 cats\n",
    ///     &registry,
    ///     &RunFilter::all(),
    /// );
    /// assert!(results[0].passed);
    /// ```
    pub fn register_step(&mut self, text_regex: &str, handler: StepHandler) {
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
        handler: StepHandler,
    ) -> Result<(), regex::Error> {
        self.try_register_regex(&step_pattern(text_regex), handler)
    }

    /// Find and run the handler for a step.
    ///
    /// Fails with `no handler registered for step: ...` when nothing matches,
    /// or with the handler's own error.
    pub fn run(&self, step: &str) -> Result<(), String> {
        let trimmed = step.trim();
        if let Some(handler) = self.exact.get(trimmed) {
            return handler(trimmed, &Vec::new());
        }
        for registered in &self.regex {
            if let Some(captures) = registered.pattern.captures(trimmed) {
                return (registered.handler)(trimmed, &capture_groups(&captures));
            }
        }
        Err(format!("no handler registered for step: {}", step))
    }

    /// Whether [`run`](Self::run) would find a handler for `step`.
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

    #[test]
    fn a_group_that_did_not_match_keeps_its_position() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut registry = StepRegistry::new();
        let sink = Rc::clone(&seen);
        registry.register_regex(
            r#"^I click( twice)? on "(.+?)"$"#,
            Box::new(move |_, caps| {
                sink.borrow_mut().push(caps.to_vec());
                Ok(())
            }),
        );

        registry.run(r#"I click on "OK""#).unwrap();
        registry.run(r#"I click twice on "OK""#).unwrap();

        // The selector is group 2 whether or not "twice" was said.
        assert_eq!(
            *seen.borrow(),
            [vec!["", "OK"], vec![" twice", "OK"]],
            "the optional group must not shift the selector into caps[0]"
        );
    }

    #[test]
    fn a_step_matches_whatever_keyword_it_is_written_with() {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut registry = StepRegistry::new();
        let sink = std::rc::Rc::clone(&seen);
        registry.register_step(
            r#"I open "(.+?)""#,
            Box::new(move |_, caps| {
                sink.borrow_mut().push(caps[0].clone());
                Ok(())
            }),
        );

        for step in [
            r#"Given I open "a""#,
            r#"When I open "b""#,
            r#"And I open "c""#,
            r#"* I open "d""#,
            r#"I open "e""#,
        ] {
            registry.run(step).unwrap();
        }
        assert_eq!(*seen.borrow(), ["a", "b", "c", "d", "e"]);
        assert!(!registry.has_handler(r#"Given I open the door"#));
        assert!(!registry.has_handler(r#"Maybe I open "f""#));
        assert!(registry
            .try_register_step("(unclosed", Box::new(|_, _| Ok(())))
            .is_err());
    }

    #[test]
    fn a_top_level_alternation_is_anchored_as_a_whole() {
        let mut registry = StepRegistry::new();
        registry.register_step("yes|no", Box::new(|_, _| Ok(())));

        assert!(registry.has_handler("yes"));
        assert!(registry.has_handler("Given no"));
        // `^` and `$` must bind both sides, not just the first and the last.
        assert!(!registry.has_handler("yes please"));
        assert!(!registry.has_handler("Given I said no"));
    }

    #[test]
    fn an_invalid_pattern_is_an_error_not_a_panic() {
        let mut registry = StepRegistry::new();
        assert!(registry
            .try_register_regex("(unclosed", Box::new(|_, _| Ok(())))
            .is_err());
        assert!(!registry.has_handler("(unclosed"));
    }

    #[test]
    #[should_panic(expected = "invalid step regex")]
    fn register_regex_still_panics_on_an_invalid_pattern() {
        StepRegistry::new().register_regex("(unclosed", Box::new(|_, _| Ok(())));
    }

    #[test]
    fn registering_a_step_again_replaces_it() {
        let mut registry = StepRegistry::new();
        registry.register("Given x", Box::new(|_, _| Err("first".into())));
        registry.register("Given x", Box::new(|_, _| Err("second".into())));
        assert_eq!(registry.run("Given x").unwrap_err(), "second");
    }

    #[test]
    fn exact_steps_win_over_regex_steps_and_regexes_run_in_order() {
        let mut registry = StepRegistry::new();
        registry.register_regex(r"^Given .*$", Box::new(|_, _| Err("regex one".into())));
        registry.register_regex(r"^Given x$", Box::new(|_, _| Err("regex two".into())));
        assert_eq!(registry.run("Given x").unwrap_err(), "regex one");
        registry.register("Given x", Box::new(|_, _| Err("exact".into())));
        assert_eq!(registry.run("Given x").unwrap_err(), "exact");
    }
}
