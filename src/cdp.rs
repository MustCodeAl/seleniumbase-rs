use serde_json::{json, Value};
use thirtyfour::extensions::cdp::{ChromeDevTools, NetworkConditions};
use thirtyfour::session::handle::SessionHandle;

use crate::error::SeleniumBaseError;

#[derive(Clone)]
pub struct CdpClient {
    devtools: ChromeDevTools,
}

impl CdpClient {
    pub fn from_handle(handle: std::sync::Arc<SessionHandle>) -> Self {
        let devtools = ChromeDevTools::new(handle);
        Self { devtools }
    }

    pub async fn enable_default_domains(&self) -> Result<(), SeleniumBaseError> {
        self.execute("Page.enable").await?;
        self.execute("Network.enable").await?;
        Ok(())
    }

    pub async fn execute(&self, method: &str) -> Result<Value, SeleniumBaseError> {
        Ok(self.devtools.execute_cdp(method).await?)
    }

    pub async fn execute_with_params(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, SeleniumBaseError> {
        Ok(self
            .devtools
            .execute_cdp_with_params(method, params)
            .await?)
    }

    pub async fn set_network_conditions(
        &self,
        conditions: &NetworkConditions,
    ) -> Result<(), SeleniumBaseError> {
        self.devtools.set_network_conditions(conditions).await?;
        Ok(())
    }

    pub async fn clear_cache(&self) -> Result<(), SeleniumBaseError> {
        self.execute("Network.clearBrowserCache").await?;
        Ok(())
    }

    pub async fn clear_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.execute("Network.clearBrowserCookies").await?;
        Ok(())
    }

    pub async fn get_cookies(&self) -> Result<Value, SeleniumBaseError> {
        self.execute("Network.getCookies").await
    }

    pub async fn mouse_click(&self, x: f64, y: f64) -> Result<(), SeleniumBaseError> {
        // Mouse moved
        self.execute_with_params(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseMoved",
                "x": x,
                "y": y
            }),
        )
        .await?;
        // Mouse down
        self.execute_with_params(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mousePressed",
                "button": "left",
                "clickCount": 1,
                "x": x,
                "y": y
            }),
        )
        .await?;
        // Mouse up
        self.execute_with_params(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased",
                "button": "left",
                "clickCount": 1,
                "x": x,
                "y": y
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn keyboard_insert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.execute_with_params("Input.insertText", json!({ "text": text }))
            .await?;
        Ok(())
    }

    pub async fn add_init_script(&self, script_source: &str) -> Result<(), SeleniumBaseError> {
        self.execute_with_params(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": script_source }),
        )
        .await?;
        Ok(())
    }
}
