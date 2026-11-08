//! How a session ends and starts over: `reconnect`, `quit`, giving a reused
//! browser back, and the cleanup when a session is dropped.

use std::time::Duration;

use tracing::{info, instrument};

use crate::browser::config::BrowserConfig;
use crate::browser::devtools::CdpClient;
use crate::browser::proxy_auth;
use crate::browser::virtual_display::XvfbProcess;
use crate::error::SeleniumBaseError;

use super::emulation::{ad_block_commands, mobile_commands};
use super::launch::connect_driver;
use super::BrowserSession;

impl BrowserSession {
    /// Lets go of the driver without ending its session: the browser stays
    /// open for the next test.
    fn keep_browser_open(&mut self) {
        // The next lease starts a login client of its own.
        self.proxy_login = None;
        self.cdp = None;
        if let Some(driver) = self.driver.take() {
            let _ = driver.leak();
        }
    }

    /// Disconnects the current WebDriver session and creates a fresh one.
    ///
    /// This is useful in undetected (UC) mode to obtain a clean session handle.
    /// When the new session started its own chromedriver, the previous
    /// chromedriver process is stopped, so reconnecting does not leak one each time.
    ///
    /// With browser plugins (feature `plugins`), the old browser's plugins
    /// hear it close, and the new browser goes through the hooks as
    /// [`connect`](Self::connect) describes, `before_launch` starting again
    /// from `config`.
    ///
    /// A direct CDP attachment (`cdp_url` or `cdp_discovery`) is attached to
    /// again, at the same endpoint: the old attachment is detached, the app
    /// keeps running, and no browser or driver is ever started.
    pub async fn reconnect(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        if self.external_page.is_some() {
            return self.reconnect_cdp(config).await;
        }
        #[cfg(feature = "plugins")]
        if self.plugins.is_some() || !config.plugins().is_empty() {
            return self.reconnect_with_plugins(config).await;
        }
        self.reconnect_plain(config).await
    }

    /// [`reconnect`](Self::reconnect) for a direct CDP attachment: the old
    /// attachment is let go, its sessions detached with the app left running,
    /// and the endpoint of `config` is attached to again. The browser is the
    /// app's and there is no driver, so nothing is ever launched.
    ///
    /// A reconnect that fails leaves the session without a page, as a
    /// WebDriver reconnect that fails leaves it without a driver.
    async fn reconnect_cdp(&mut self, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
        if config.cdp_url.is_none() && !config.cdp_discovery {
            return Err(SeleniumBaseError::invalid_config(
                "a direct CDP attachment reconnects to the endpoint of its configuration: \
                     set cdp_url or cdp_discovery",
            ));
        }
        // The old window is let go first, so the plugins hear it close before
        // the new attachment starts and no window is prepared twice.
        if let Some(page) = self.external_page.take() {
            page.browser().close().await?;
        }
        let mut fresh = Self::connect_cdp(config).await?;
        self.external_page = fresh.external_page.take();
        Ok(())
    }

    /// [`reconnect`](Self::reconnect) without running plugin hooks.
    pub(super) async fn reconnect_plain(
        &mut self,
        config: &BrowserConfig,
    ) -> Result<(), SeleniumBaseError> {
        // Worked out before anything is torn down, so a bad list leaves the
        // session as it was.
        let ad_block = ad_block_commands(config)?;
        proxy_auth::login_for(config)?;
        if let Some(sidecar) = self.proxy_login.take() {
            sidecar.close().await;
        }
        if let Some(driver) = self.driver.take() {
            driver.quit().await?;
        }
        // A reused browser has just been closed: it leaves the registry, which
        // stops its driver. The new session is this test's own.
        if let Some(lease) = self.reuse.take() {
            lease.discard();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let display = self.virtual_display.as_ref().map(XvfbProcess::display);
        let (driver, process) = connect_driver(config, display).await?;
        self.driver = Some(driver);
        if let Some(process) = process
            && let Some(mut previous) = self.driver_process.replace(process)
        {
            previous.kill();
        }
        self.cdp = if config.is_cdp_enabled() {
            self.driver.as_ref().map(CdpClient::new)
        } else {
            None
        };
        self.ad_block = ad_block;
        self.mobile = mobile_commands(config);
        self.begin_proxy_login(config).await?;
        self.initialize_mode(config).await?;
        Ok(())
    }

    /// Closes the browser session and any auto-started driver process.
    ///
    /// A session on a browser kept for reuse gives the browser back instead,
    /// open, for the next test.
    #[instrument(skip(self))]
    pub async fn quit(&mut self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.take() {
            return page.browser().close().await;
        }
        if let Some(lease) = self.reuse.take() {
            info!("giving the browser back for reuse");
            self.keep_browser_open();
            drop(lease);
            return Ok(());
        }
        info!("quitting browser session");
        if let Some(sidecar) = self.proxy_login.take() {
            sidecar.close().await;
        }
        // The plugins hear it while the browser still runs.
        #[cfg(feature = "plugins")]
        if let Some(plugins) = self.plugins.take() {
            plugins.close().await;
        }
        if let Some(driver) = self.driver.take() {
            driver.quit().await?;
        }
        if let Some(mut process) = self.driver_process.take() {
            process.kill();
        }
        // The browser is gone, so its screen can be too.
        self.virtual_display = None;
        Ok(())
    }
}

impl Drop for BrowserSession {
    /// Best-effort cleanup when a session is dropped without an explicit
    /// `quit()` call. This improves disposability (Twelve-Factor IX) by
    /// reducing leaked browser/driver processes in short-lived scripts.
    fn drop(&mut self) {
        if self.reuse.is_some() {
            // The browser stays open; the lease, dropped with the session,
            // gives it back.
            self.keep_browser_open();
            return;
        }
        if self.driver.is_none() {
            return;
        }
        let driver = self.driver.take();
        let process = self.driver_process.take();
        let virtual_display = self.virtual_display.take();
        #[cfg(feature = "plugins")]
        let plugins = self.plugins.take();
        // `drop` cannot await, so the cleanup runs in the background. It is
        // registered so a program that exits next can wait for it with
        // `common::shutdown::drain_cleanups`.
        crate::common::shutdown::spawn_cleanup(async move {
            #[cfg(feature = "plugins")]
            if let Some(plugins) = plugins {
                plugins.close().await;
            }
            if let Some(driver) = driver {
                let _ = driver.quit().await;
            }
            if let Some(mut process) = process {
                process.kill();
            }
            drop(virtual_display);
        });
    }
}

/// `BrowserSession::quit` against the mock WebDriver in `webdriver_support` (no
/// browser): what it sends, and what it does when it is called again or when
/// there is no driver left to close.
#[cfg(test)]
mod quit_tests {
    use serde_json::json;

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply};

    /// A WebDriver that answers every command.
    fn driver() -> MockWebDriver {
        MockWebDriver::start(json!({}), |_| Reply::Value(json!({})))
    }

    /// A session on `driver` that starts no driver of its own.
    async fn session_on(driver: &MockWebDriver) -> BrowserSession {
        let mut config = BrowserConfig::default().with_webdriver_url(driver.url());
        config.session.auto_start_driver = false;
        BrowserSession::connect(config)
            .await
            .expect("the mock WebDriver accepts a session")
    }

    /// How many times the session was deleted: the mock records
    /// `DELETE /session/mock` with the empty path.
    fn deletes(driver: &MockWebDriver) -> usize {
        driver
            .requests("")
            .iter()
            .filter(|request| request.method == "DELETE")
            .count()
    }

    #[tokio::test]
    async fn quit_deletes_the_webdriver_session_and_leaves_none() {
        let mock = driver();
        let mut session = session_on(&mock).await;
        assert_eq!(session.session_id().as_deref(), Some("mock"));
        assert_eq!(deletes(&mock), 0);

        session.quit().await.expect("the session ends");

        assert_eq!(deletes(&mock), 1);
        assert_eq!(session.session_id(), None);
        assert!(matches!(
            session.webdriver(),
            Err(SeleniumBaseError::SessionNotStarted)
        ));
    }

    #[tokio::test]
    async fn quitting_twice_deletes_the_session_once() {
        let mock = driver();
        let mut session = session_on(&mock).await;

        session.quit().await.expect("the session ends");
        session
            .quit()
            .await
            .expect("a second quit has nothing left to close");

        assert_eq!(deletes(&mock), 1);
    }

    #[tokio::test]
    async fn a_session_with_no_driver_quits_without_error_every_time() {
        let mut session = BrowserSession::disconnected();

        session.quit().await.expect("nothing to close");
        session.quit().await.expect("still nothing to close");

        assert_eq!(session.session_id(), None);
    }
}
