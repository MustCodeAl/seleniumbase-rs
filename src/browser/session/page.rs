//! What a session asks of the page it is on: scripts, screenshots, alerts and
//! WebDriver cookies.

use std::path::Path;

use serde_json::{Value, json};

use crate::cookie::Cookie;
use crate::error::{DriverErrorKind, SeleniumBaseError, driver_kind};

use super::BrowserSession;

impl BrowserSession {
    /// WebDriver interaction: `execute_async_script`.
    pub async fn execute_async_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            // The callback is the last argument, as WebDriver passes it: a promise
            // the script resolves.
            let expression = format!(
                "new Promise((resolve, reject) => {{ try {{ (function() {{\n{script}\n}}).call(window, resolve); }} catch (error) {{ reject(error); }} }})"
            );
            return Box::pin(self.cdp_evaluate(&page, expression)).await;
        }
        self.webdriver()?
            .execute_async(script, vec![])
            .await
            .map_err(SeleniumBaseError::from)
            .map(|ret| ret.json().clone())
    }

    /// Executes arbitrary JavaScript and returns the result.
    pub async fn execute_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return Box::pin(
                self.cdp_evaluate(&page, format!("(function() {{\n{script}\n}}).call(window)")),
            )
            .await;
        }
        let ret = self.webdriver()?.execute(script, Vec::new()).await?;
        Ok(ret.json().clone())
    }

    /// Captures a screenshot and writes it to `path`.
    pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            std::fs::write(path, page.screenshot().await?)?;
            return Ok(());
        }
        self.webdriver()?.screenshot(path).await?;
        Ok(())
    }

    /// Captures a screenshot and returns the raw PNG bytes.
    pub async fn screenshot_as_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page.screenshot().await;
        }
        Ok(self.webdriver()?.screenshot_as_png().await?)
    }

    /// WebDriver interaction: `switch_to_alert_accept`.
    pub async fn switch_to_alert_accept(&self) -> Result<(), SeleniumBaseError> {
        if let Some(dialogs) = self.external_page.dialogs() {
            return dialogs.accept().await;
        }
        self.webdriver()?
            .accept_alert()
            .await
            .map_err(SeleniumBaseError::from)
    }

    /// WebDriver interaction: `switch_to_alert_dismiss`.
    pub async fn switch_to_alert_dismiss(&self) -> Result<(), SeleniumBaseError> {
        if let Some(dialogs) = self.external_page.dialogs() {
            return dialogs.dismiss().await;
        }
        self.webdriver()?
            .dismiss_alert()
            .await
            .map_err(SeleniumBaseError::from)
    }

    /// The text of the open alert. Python's `get_alert_text`.
    pub async fn alert_text(&self) -> Result<String, SeleniumBaseError> {
        if let Some(dialogs) = self.external_page.dialogs() {
            return dialogs.text();
        }
        self.webdriver()?
            .get_alert_text()
            .await
            .map_err(SeleniumBaseError::from)
    }

    /// WebDriver interaction: `type_alert_text`.
    pub async fn type_alert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        if let Some(dialogs) = self.external_page.dialogs() {
            return dialogs.send_text(text);
        }
        self.webdriver()?
            .send_alert_text(text)
            .await
            .map_err(SeleniumBaseError::from)
    }

    /// Adds `cookie` to the browser through WebDriver, for the page the session is on.
    ///
    /// An empty `domain` is left out, so the browser uses the page's; the expiry is cut to
    /// whole seconds. [`BaseCase::add_cookie`](crate::BaseCase::add_cookie) also moves a domain
    /// that is not the page's and applies Python's `expiry` argument.
    ///
    /// # Errors
    ///
    /// Returns the driver's error if it refuses the cookie.
    pub async fn add_cookie(&self, cookie: &Cookie) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return Self::add_cdp_cookie(&page, cookie).await;
        }
        self.webdriver()?
            .add_cookie(cookie.to_webdriver())
            .await
            .map_err(SeleniumBaseError::from)?;
        Ok(())
    }

    /// The cookie of the page called `name`, or `None` when the page has no such cookie, as
    /// Python's `get_cookie` answers.
    ///
    /// # Errors
    ///
    /// Returns the driver's error for any failure other than a missing cookie.
    pub async fn cookie(&self, name: &str) -> Result<Option<Cookie>, SeleniumBaseError> {
        if self.external_page.is_some() || self.cdp_client().is_ok() {
            let cookies = self.cookies().await?;
            return Ok(cookies.into_iter().find(|cookie| cookie.name == name));
        }
        match self.webdriver()?.get_named_cookie(name).await {
            Ok(cookie) => Ok(Some(Cookie::from_webdriver(&cookie))),
            // Python's `get_cookie` answers `None` for a cookie that is not there.
            Err(error) if driver_kind(&error) == DriverErrorKind::CookieNotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// WebDriver interaction: `delete_cookie`.
    pub async fn delete_cookie(&self, name: &str) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            let url = page.url().await?;
            page.execute("Network.deleteCookies", json!({ "name": name, "url": url }))
                .await?;
            return Ok(());
        }
        self.webdriver()?
            .delete_cookie(name)
            .await
            .map_err(SeleniumBaseError::from)?;
        Ok(())
    }

    /// WebDriver interaction: `delete_all_cookies`.
    pub async fn delete_all_cookies(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            // The cookies of the page's address, as WebDriver deletes them.
            for cookie in self.cookies().await? {
                page.execute(
                    "Network.deleteCookies",
                    json!({ "name": cookie.name, "domain": cookie.domain, "path": cookie.path }),
                )
                .await?;
            }
            return Ok(());
        }
        self.webdriver()?
            .delete_all_cookies()
            .await
            .map_err(SeleniumBaseError::from)?;
        Ok(())
    }

    /// Adds `cookie` through DevTools to the page of a direct CDP attachment.
    ///
    /// A cookie with no domain is given the page's address, as WebDriver gives
    /// it the page's domain.
    async fn add_cdp_cookie(
        page: &crate::sb_cdp::Page,
        cookie: &Cookie,
    ) -> Result<(), SeleniumBaseError> {
        let mut protocol = cookie.to_protocol();
        if cookie.domain.is_empty() {
            protocol["url"] = json!(page.url().await?);
        }
        let answer = page.execute("Network.setCookie", protocol).await?;
        if answer["success"].as_bool() == Some(false) {
            return Err(SeleniumBaseError::cdp_driver(format!(
                "the browser refused the cookie '{}'",
                cookie.name
            )));
        }
        Ok(())
    }
}
