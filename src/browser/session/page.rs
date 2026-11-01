//! What a session asks of the page it is on: scripts, screenshots, alerts and
//! WebDriver cookies.

use std::path::Path;

use serde_json::Value;

use crate::error::SeleniumBaseError;

use super::BrowserSession;

impl BrowserSession {
    /// WebDriver interaction: `execute_async_script`.
    pub async fn execute_async_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.webdriver()?
            .execute_async(script, vec![])
            .await
            .map_err(SeleniumBaseError::WebDriver)
            .map(|ret| ret.json().clone())
    }

    /// Executes arbitrary JavaScript and returns the result.
    pub async fn execute_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        if let Some(page) = &self.external_page {
            return page
                .evaluate(format!("(function() {{\n{script}\n}}).call(window)"))
                .await;
        }
        let ret = self.webdriver()?.execute(script, Vec::new()).await?;
        Ok(ret.json().clone())
    }

    /// Captures a screenshot and writes it to `path`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        self.webdriver()?.screenshot(path).await?;
        Ok(())
    }

    /// Captures a screenshot and returns the raw PNG bytes.
    pub async fn screenshot_as_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        Ok(self.webdriver()?.screenshot_as_png().await?)
    }

    /// WebDriver interaction: `switch_to_alert_accept`.
    pub async fn switch_to_alert_accept(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .accept_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `switch_to_alert_dismiss`.
    pub async fn switch_to_alert_dismiss(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .dismiss_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `get_alert_text`.
    pub async fn get_alert_text(&self) -> Result<String, SeleniumBaseError> {
        self.webdriver()?
            .get_alert_text()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `type_alert_text`.
    pub async fn type_alert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .send_alert_text(text)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `add_cookie`.
    pub async fn add_cookie(&self, name: &str, value: &str) -> Result<(), SeleniumBaseError> {
        let cookie = thirtyfour::cookie::Cookie::new(name, value);
        self.webdriver()?
            .add_cookie(cookie)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// WebDriver interaction: `get_cookie`.
    pub async fn get_cookie(
        &self,
        name: &str,
    ) -> Result<thirtyfour::cookie::Cookie, SeleniumBaseError> {
        self.webdriver()?
            .get_named_cookie(name)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// WebDriver interaction: `delete_cookie`.
    pub async fn delete_cookie(&self, name: &str) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .delete_cookie(name)
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// WebDriver interaction: `delete_all_cookies`.
    pub async fn delete_all_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.webdriver()?
            .delete_all_cookies()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }
}
