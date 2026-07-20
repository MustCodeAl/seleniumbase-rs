use crate::api::base_case::BaseCase;
use crate::error::SeleniumBaseError;

#[derive(Clone, Debug)]
pub enum DeferredAssert {
    Element(String),
    Text(String, String),
}

#[derive(Clone, Debug, Default)]
pub struct DeferredAsserts {
    items: Vec<DeferredAssert>,
}

impl DeferredAsserts {
    pub fn add_element(&mut self, css: &str) {
        self.items.push(DeferredAssert::Element(css.to_owned()));
    }

    pub fn add_text(&mut self, text: &str, css: &str) {
        self.items
            .push(DeferredAssert::Text(text.to_owned(), css.to_owned()));
    }

    pub async fn process(&mut self, sb: &mut BaseCase) -> Result<(), SeleniumBaseError> {
        let mut failures = Vec::new();
        for item in &self.items {
            let result = match item {
                DeferredAssert::Element(css) => sb.assert_element(css).await,
                DeferredAssert::Text(text, css) => sb.assert_text(css, text).await,
            };
            if let Err(e) = result {
                failures.push(e.to_string());
            }
        }
        self.items.clear();

        if failures.is_empty() {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(format!(
                "{} deferred assertion(s) failed:\n{}",
                failures.len(),
                failures.join("\n")
            )))
        }
    }
}
