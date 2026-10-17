// Shadow DOM piercing helpers for BaseCase.

use crate::utils::shadow::{
    build_shadow_attribute, build_shadow_click, build_shadow_query, build_shadow_text,
    build_shadow_type, split_shadow_selector,
};

/// The fragments of a shadow-piercing `selector`, one per shadow root.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidSelector`] if the selector is blank.
fn shadow_fragments(selector: &str) -> Result<Vec<String>, SeleniumBaseError> {
    let fragments = split_shadow_selector(selector);
    if fragments.is_empty() {
        return Err(SeleniumBaseError::InvalidSelector(
            "empty shadow selector".to_owned(),
        ));
    }
    Ok(fragments)
}

/// The attribute a shadow-attribute script returned. The script gives `''`
/// for an attribute that is missing, so an empty value is reported as absent.
fn shadow_attribute_value(result: Value) -> Option<String> {
    match result {
        Value::String(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

impl BaseCase {
    /// Returns true if the selector contains the `::shadow` piercing combinator.
    pub fn is_shadow_selector(&self, selector: &str) -> bool {
        selector.contains("::shadow")
    }

    /// Evaluates a shadow-piercing selector and returns the raw JS element (or null).
    async fn query_shadow(&self, selector: &str) -> Result<Value, SeleniumBaseError> {
        let fragments = shadow_fragments(selector)?;
        self.execute_script(&build_shadow_query(&fragments)).await
    }

    /// Clicks an element inside one or more shadow roots.
    pub async fn shadow_click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let fragments = shadow_fragments(selector)?;
        let result = self.execute_script(&build_shadow_click(&fragments)).await?;
        if result.as_bool() != Some(true) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "shadow element not found or not clickable for selector '{}'",
                selector
            )));
        }
        Ok(())
    }

    /// Types text into an element inside one or more shadow roots.
    pub async fn shadow_type(
        &self,
        selector: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let fragments = shadow_fragments(selector)?;
        let result = self
            .execute_script(&build_shadow_type(&fragments, text))
            .await?;
        if result.as_bool() != Some(true) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "shadow element not found for selector '{}'",
                selector
            )));
        }
        Ok(())
    }

    /// Returns the text content of an element inside shadow roots.
    pub async fn shadow_get_text(&self, selector: &str) -> Result<String, SeleniumBaseError> {
        let fragments = shadow_fragments(selector)?;
        let result = self.execute_script(&build_shadow_text(&fragments)).await?;
        Ok(result.as_str().unwrap_or("").to_owned())
    }

    /// Returns an attribute value of an element inside shadow roots.
    pub async fn shadow_get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let fragments = shadow_fragments(selector)?;
        let result = self
            .execute_script(&build_shadow_attribute(&fragments, attribute))
            .await?;
        Ok(shadow_attribute_value(result))
    }

    /// Asserts that a shadow-pierced element exists.
    pub async fn assert_shadow_element(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        match self.query_shadow(selector).await? {
            Value::Null => Err(SeleniumBaseError::AssertionFailed(format!(
                "shadow element '{}' not found",
                selector
            ))),
            _ => Ok(()),
        }
    }

    /// Asserts that text appears inside a shadow-pierced element. The text
    /// comes first and the selector second, like `assert_text`.
    pub async fn assert_shadow_text(
        &self,
        text: &str,
        selector: &str,
    ) -> Result<(), SeleniumBaseError> {
        let actual = self.shadow_get_text(selector).await?;
        if !actual.contains(text) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected shadow element '{}' to contain '{}', got '{}'",
                selector, text, actual
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_shadow {
    use super::*;

    #[test]
    fn a_selector_is_cut_at_each_shadow_root() {
        assert_eq!(shadow_fragments("app ::shadow input").unwrap(), ["app", "input"]);
        assert_eq!(
            shadow_fragments("my-app ::shadow .form ::shadow input").unwrap(),
            ["my-app", ".form", "input"]
        );
        assert_eq!(shadow_fragments("input").unwrap(), ["input"]);
    }

    #[test]
    fn a_blank_selector_is_rejected_before_any_script_runs() {
        for selector in ["", "   ", "::shadow", " ::shadow ::shadow "] {
            let error = shadow_fragments(selector).unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidSelector(m) if m == "empty shadow selector"),
                "{selector:?}"
            );
        }
    }

    #[test]
    fn selector_detection_matches_what_the_splitter_cuts_at() {
        let sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        assert!(sb.is_shadow_selector("app ::shadow input"));
        assert!(!sb.is_shadow_selector("app input"));
    }

    #[test]
    fn a_missing_attribute_is_none_and_a_present_one_is_its_value() {
        assert_eq!(shadow_attribute_value(Value::String("x".into())), Some("x".to_owned()));
        assert_eq!(shadow_attribute_value(Value::String(String::new())), None);
        assert_eq!(shadow_attribute_value(Value::Null), None);
    }
}
