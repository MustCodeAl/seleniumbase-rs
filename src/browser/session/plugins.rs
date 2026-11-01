//! Browser plugins in a session (feature `plugins`): the hooks that run when
//! a session starts, reconnects and opens windows.

use serde_json::Value;

use crate::browser::config::BrowserConfig;
use crate::browser::plugin_hooks::{self, SessionPlugins};
use crate::error::SeleniumBaseError;

use super::emulation::fingerprint_blocklist;
use super::BrowserSession;

/// Browser plugins (feature `plugins`, ADR 004); see
/// [`BrowserConfig::with_plugins`].
impl BrowserSession {
    /// Gives the window the session is on the packages and `on_page_created`
    /// of its browser plugins (feature `plugins`).
    ///
    /// The session does this itself for its first window and for each window
    /// it opens with [`switch_to_new_window`](Self::switch_to_new_window).
    /// Call it after reaching a window some other way, such as a pop-up the
    /// page opened. It does nothing for a session without plugins.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Plugin`] for the first package or hook that
    /// fails.
    pub async fn apply_plugins_to_window(&self) -> Result<(), SeleniumBaseError> {
        match &self.plugins {
            Some(plugins) => plugins.prepare_window(self.webdriver()?).await,
            None => Ok(()),
        }
    }

    /// [`connect`](Self::connect) for a configuration with plugins.
    pub(super) async fn connect_with_plugins(
        config: BrowserConfig,
    ) -> Result<Self, SeleniumBaseError> {
        let config = plugin_hooks::before_launch(config).await?;
        let family = plugin_hooks::family_of(config.browser)?;
        let plugins = config.plugins().clone();
        let mut session = match Self::connect_new(config.clone()).await {
            Ok(session) => session,
            Err(error) => {
                // `before_launch` ran for a launch that ends here.
                plugin_hooks::close(&plugins).await;
                return Err(error);
            }
        };
        session.check_server_for(&plugins, family).await?;
        session
            .start_plugins(SessionPlugins::new(plugins, family), &config)
            .await?;
        Ok(session)
    }

    /// Checks `plugins` again against what the server says it is, now that
    /// it has answered: a Tauri window is an embedded webview, whatever
    /// browser the configuration named. A refusal ends the session and tells
    /// the plugins the launch ended, since `before_launch` already ran.
    async fn check_server_for(
        &mut self,
        plugins: &seleniumbase_rs_plugin::PluginSet,
        family: seleniumbase_rs_plugin::BrowserFamily,
    ) -> Result<(), SeleniumBaseError> {
        let capabilities =
            serde_json::to_value(self.webdriver()?.handle().capabilities()).unwrap_or(Value::Null);
        if let Err(error) = plugin_hooks::check_server(plugins, family, &capabilities) {
            if let Err(quit) = self.quit().await {
                tracing::warn!(
                    error = %quit,
                    "the browser of a session its plugins refused did not close"
                );
            }
            plugin_hooks::close(plugins).await;
            return Err(error);
        }
        Ok(())
    }

    /// Calls `on_browser` and prepares the first window, and keeps `plugins`
    /// for the rest of the session.
    ///
    /// # Errors
    ///
    /// The first hook that fails. The browser is closed then: one without
    /// what its plugins add is not handed out.
    async fn start_plugins(
        &mut self,
        plugins: SessionPlugins,
        config: &BrowserConfig,
    ) -> Result<(), SeleniumBaseError> {
        let mut blocked = plugin_hooks::blocked_urls_of(&self.ad_block);
        for url in fingerprint_blocklist(config) {
            if !blocked.contains(&url) {
                blocked.push(url);
            }
        }
        let plugins = plugins.with_blocked(blocked);
        let started = plugins.start(self.webdriver()?).await;
        self.plugins = Some(plugins);
        if let Err(error) = started {
            if let Err(quit) = self.quit().await {
                tracing::warn!(
                    error = %quit,
                    "the browser of a session its plugins stopped did not close"
                );
            }
            return Err(error);
        }
        Ok(())
    }

    /// [`reconnect`](Self::reconnect) for a session or configuration with
    /// plugins.
    pub(super) async fn reconnect_with_plugins(
        &mut self,
        config: &BrowserConfig,
    ) -> Result<(), SeleniumBaseError> {
        // Checked first, so a refusal leaves the session as it was.
        let prepared = plugin_hooks::before_launch(config.clone()).await?;
        if let Some(old) = self.plugins.take() {
            old.close().await;
        }
        if let Err(error) = self.reconnect_plain(&prepared).await {
            plugin_hooks::close(prepared.plugins()).await;
            return Err(error);
        }
        if prepared.plugins().is_empty() {
            return Ok(());
        }
        let family = plugin_hooks::family_of(prepared.browser)?;
        self.check_server_for(prepared.plugins(), family).await?;
        self.start_plugins(
            SessionPlugins::new(prepared.plugins().clone(), family),
            &prepared,
        )
        .await
    }
}
