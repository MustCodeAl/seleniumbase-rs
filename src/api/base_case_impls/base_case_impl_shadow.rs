// Shadow DOM piercing helpers for BaseCase.

use crate::utils::shadow::{
    build_shadow_attribute, build_shadow_click, build_shadow_query, build_shadow_text,
    build_shadow_type, split_shadow_selector,
};

impl BaseCase {
    /// Returns true if the selector contains the `::shadow` piercing combinator.
    pub fn is_shadow_selector(&self, selector: &str) -> bool {
        selector.contains("::shadow")
    }

    /// Evaluates a shadow-piercing selector and returns the raw JS element (or null).
    async fn query_shadow(&self, selector: &str) -> Result<Value, SeleniumBaseError> {
        let fragments = split_shadow_selector(selector);
        if fragments.is_empty() {
            return Err(SeleniumBaseError::InvalidSelector(
                "empty shadow selector".to_owned(),
            ));
        }
        self.execute_script(&build_shadow_query(&fragments)).await
    }

    /// Clicks an element inside one or more shadow roots.
    pub async fn shadow_click(&self, selector: &str) -> Result<(), SeleniumBaseError> {
        let fragments = split_shadow_selector(selector);
        if fragments.is_empty() {
            return Err(SeleniumBaseError::InvalidSelector(
                "empty shadow selector".to_owned(),
            ));
        }
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
        let fragments = split_shadow_selector(selector);
        if fragments.is_empty() {
            return Err(SeleniumBaseError::InvalidSelector(
                "empty shadow selector".to_owned(),
            ));
        }
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
        let fragments = split_shadow_selector(selector);
        if fragments.is_empty() {
            return Err(SeleniumBaseError::InvalidSelector(
                "empty shadow selector".to_owned(),
            ));
        }
        let result = self.execute_script(&build_shadow_text(&fragments)).await?;
        Ok(result.as_str().unwrap_or("").to_owned())
    }

    /// Returns an attribute value of an element inside shadow roots.
    pub async fn shadow_get_attribute(
        &self,
        selector: &str,
        attribute: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let fragments = split_shadow_selector(selector);
        if fragments.is_empty() {
            return Err(SeleniumBaseError::InvalidSelector(
                "empty shadow selector".to_owned(),
            ));
        }
        let result = self
            .execute_script(&build_shadow_attribute(&fragments, attribute))
            .await?;
        match result {
            Value::String(s) if !s.is_empty() => Ok(Some(s)),
            _ => Ok(None),
        }
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

    /// Asserts that text appears inside a shadow-pierced element.
    pub async fn assert_shadow_text(
        &self,
        selector: &str,
        text: &str,
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
    // Unit tests for the helper logic live in crate::utils::shadow.
    // The following tests ensure BaseCase-level selector detection works
    // without a live browser.

    #[test]
    fn shadow_selector_detection() {
        // We cannot instantiate BaseCase here without async, but we can test
        // the split helper directly through the public utils module.
        let parts = crate::utils::shadow::split_shadow_selector("app ::shadow input");
        assert_eq!(parts, vec!["app", "input"]);
    }
}
