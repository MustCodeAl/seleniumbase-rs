//! The waits of a session: for an element to appear, be shown, be clickable,
//! go away, or contain a text.

use std::time::Duration;

use thirtyfour::prelude::{By, WebElement};

use crate::error::SeleniumBaseError;
use crate::utilities::retry::{poll_until, RetryPolicy};

use super::BrowserSession;

impl BrowserSession {
    /// Waits up to `timeout_secs` seconds for `locator` to match an element.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the locator if nothing
    /// matches in time.
    pub async fn wait_for_element(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async { self.find(locator.clone()).await.ok() },
        )
        .await
        .ok_or_else(|| timed_out(format!("element {locator}"), timeout_secs))
    }

    /// Waits up to `timeout` seconds for `locator` to become visible.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the locator if it is
    /// not visible in time.
    pub async fn wait_for_element_visible(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.find(locator.clone()).await {
                    Ok(element) if element.is_displayed().await.unwrap_or(false) => Some(element),
                    _ => None,
                }
            },
        )
        .await
        .ok_or_else(|| timed_out(format!("element {locator} to be visible"), timeout_secs))
    }

    /// Waits up to `timeout` seconds for `locator` to become clickable.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the locator if it is
    /// not clickable in time.
    pub async fn wait_for_element_clickable(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<WebElement, SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.find(locator.clone()).await {
                    Ok(element)
                        if element.is_displayed().await.unwrap_or(false)
                            && element.is_enabled().await.unwrap_or(false) =>
                    {
                        Some(element)
                    }
                    _ => None,
                }
            },
        )
        .await
        .ok_or_else(|| timed_out(format!("element {locator} to be clickable"), timeout_secs))
    }

    /// Waits up to `timeout` seconds for `locator` to be removed.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the locator if it is
    /// still there when the time is up.
    pub async fn wait_for_element_absent(
        &self,
        locator: By,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.find_all(locator.clone()).await {
                    Ok(elements) if elements.is_empty() => Some(()),
                    Err(_) => Some(()),
                    _ => None,
                }
            },
        )
        .await
        .ok_or_else(|| timed_out(format!("element {locator} to be absent"), timeout_secs))
    }

    /// Waits up to `timeout_secs` seconds for the element at `locator` to
    /// contain `expected_substring`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] naming the text and the
    /// locator if the text does not appear in time.
    pub async fn wait_for_text(
        &self,
        locator: By,
        expected_substring: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.webdriver()?;

        poll_until(
            Duration::from_secs(timeout_secs),
            RetryPolicy::webdriver(),
            || async {
                match self.find(locator.clone()).await {
                    Ok(element) => match shown_text(&element).await {
                        Ok(text) if text.contains(expected_substring) => Some(()),
                        _ => None,
                    },
                    Err(_) => None,
                }
            },
        )
        .await
        .ok_or_else(|| {
            timed_out(
                format!("text '{expected_substring}' in element {locator}"),
                timeout_secs,
            )
        })
    }
}

/// The error of a wait that ran out of time: a [`SeleniumBaseError::WaitTimeout`]
/// that names what was waited for and for how long.
fn timed_out(what: String, timeout_secs: u64) -> SeleniumBaseError {
    SeleniumBaseError::wait_timeout(
        format!("timed out waiting for {what}"),
        Some(Duration::from_secs(timeout_secs)),
    )
}

/// The text Python's text checks compare for `element`: the value of an
/// `<input>` or a `<textarea>`, whose rendered text is empty, and the rendered
/// text of any other element.
pub(crate) async fn shown_text(element: &WebElement) -> Result<String, SeleniumBaseError> {
    let tag = element.tag_name().await.unwrap_or_default();
    if tag.eq_ignore_ascii_case("input") || tag.eq_ignore_ascii_case("textarea") {
        return Ok(element.prop("value").await?.unwrap_or_default());
    }
    Ok(element.text().await?)
}

/// How the waits of a session end: with a `WaitTimeout` that names what was
/// waited for, against the mock WebDriver in `webdriver_support`.
#[cfg(test)]
mod wait_tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::api::base_case::webdriver_support::{element, MockWebDriver, Reply};
    use crate::browser::config::BrowserConfig;

    /// A WebDriver on which no element can be found, but `#present` can be
    /// listed.
    fn driver_without_elements() -> MockWebDriver {
        MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Error(404, "no such element"),
                ("POST", "/elements") => Reply::Value(json!([element("e1")])),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    async fn session(driver: &MockWebDriver) -> BrowserSession {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.auto_start_driver = false;
        BrowserSession::connect(config).await.expect("connects")
    }

    /// The message of the `WaitTimeout` in `result`; panics with what came
    /// instead.
    fn message_of_the_timeout<T: std::fmt::Debug>(result: Result<T, SeleniumBaseError>) -> String {
        match result {
            Err(SeleniumBaseError::WaitTimeout(message)) => message,
            other => panic!("expected a WaitTimeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn waiting_for_an_element_that_never_comes_is_a_wait_timeout_naming_it() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        let message = message_of_the_timeout(session.wait_for_element(By::Css("#a"), 0).await);

        assert!(
            message.contains("timed out waiting for element CSS(#a)"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn waiting_for_visibility_and_clickability_end_in_wait_timeouts_too() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        let visible =
            message_of_the_timeout(session.wait_for_element_visible(By::Css("#v"), 0).await);
        let clickable =
            message_of_the_timeout(session.wait_for_element_clickable(By::Css("#c"), 0).await);

        assert!(
            visible.contains("element CSS(#v) to be visible"),
            "{visible}"
        );
        assert!(
            clickable.contains("element CSS(#c) to be clickable"),
            "{clickable}"
        );
    }

    #[tokio::test]
    async fn waiting_for_an_element_to_go_away_ends_in_a_wait_timeout_naming_it() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        // The mock lists an element for every locator, so it never goes.
        let message =
            message_of_the_timeout(session.wait_for_element_absent(By::Css("#gone"), 0).await);

        assert!(
            message.contains("element CSS(#gone) to be absent"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn waiting_for_text_ends_in_a_wait_timeout_naming_the_text_and_the_element() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        let message =
            message_of_the_timeout(session.wait_for_text(By::Css("#t"), "Welcome", 0).await);

        assert!(message.contains("text 'Welcome'"), "{message}");
        assert!(message.contains("CSS(#t)"), "{message}");
    }

    #[tokio::test]
    async fn the_message_of_a_wait_timeout_says_how_long_the_wait_was() {
        let driver = driver_without_elements();
        let session = session(&driver).await;

        let error = session
            .wait_for_element(By::Css("#a"), 0)
            .await
            .expect_err("the element never comes");

        let text = error.to_string();
        assert!(text.starts_with("wait timeout: "), "{text}");
        assert!(text.ends_with("(after 0ms)"), "{text}");
    }
}
