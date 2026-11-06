//! Starting a session: `connect`, a new browser, a leased one, and a direct
//! CDP attachment, with the proxy login that goes with them.

use serde_json::Value;
use thirtyfour::prelude::WebDriver;
use tracing::{info, instrument};

use crate::browser::config::BrowserConfig;
use crate::browser::options::LaunchSwitch;
use crate::browser::proxy_auth::{self, ProxyAuthSidecar};
use crate::browser::reuse::{self, Lease, ReuseKey, SessionAddress};
use crate::browser::virtual_display::XvfbProcess;
use crate::error::SeleniumBaseError;
use crate::evasion::cdp::CdpClient;

use super::emulation::{ad_block_commands, mobile_commands};
use super::launch::{
    apply_window_rect, connect_driver, resolve_remote_debug, start_virtual_display,
    validate_mode_support,
};
use super::BrowserSession;

impl BrowserSession {
    /// Starts a browser for `config` and connects to it, or, with
    /// [`BrowserConfig::reuse_session`] on, attaches to a browser kept for
    /// reuse (see [`crate::browser::reuse`]).
    ///
    /// With browser plugins (feature `plugins`,
    /// `BrowserConfig::with_plugins`) the set is checked and `before_launch`
    /// runs before the browser is asked for; once it runs, `on_browser` and,
    /// for its window, the packages and `on_page_created`. A refusal or a hook
    /// that fails stops the session with [`SeleniumBaseError::Plugin`] and
    /// closes a browser that was started.
    #[instrument(skip(config))]
    pub async fn connect(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        if config.cdp_url.is_some() || config.cdp_discovery {
            return Self::connect_cdp(&config).await;
        }
        #[cfg(feature = "plugins")]
        if !config.plugins().is_empty() {
            return Self::connect_with_plugins(config).await;
        }
        if config.session.reuse {
            return reuse::connect(config).await;
        }
        Self::connect_new(config).await
    }

    /// Starts a browser for `config` and connects to it, whatever its reuse
    /// setting says.
    pub(crate) async fn connect_new(mut config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        if config.cdp_url.is_some() || config.cdp_discovery {
            return Self::connect_cdp(&config).await;
        }
        info!(browser = ?config.browser.kind, headless = config.browser.headless, "connecting browser session");
        let (remote_debug, ad_block) = refuse_what_cannot_work(&mut config)?;
        let virtual_display = start_virtual_display(&config)?;
        let (driver, driver_process) =
            connect_driver(&config, virtual_display.as_ref().map(XvfbProcess::display)).await?;
        place_window(&driver, &config).await?;
        let cdp = if config.is_cdp_enabled() {
            Some(CdpClient::from_handle(driver.handle().clone()))
        } else {
            None
        };

        let mut session = Self {
            driver: Some(driver),
            cdp,
            external_page: None,
            driver_process,
            virtual_display,
            mobile: mobile_commands(&config),
            remote_debug,
            proxy_login: None,
            ad_block,
            reuse: None,
            #[cfg(feature = "plugins")]
            plugins: None,
        };
        // The proxy must be answerable before the first page loads.
        if let Err(error) = session.begin_proxy_login(&config).await {
            let _ = session.quit().await;
            return Err(error);
        }
        session.initialize_mode(&config).await?;
        info!("browser session connected");
        Ok(session)
    }

    /// A session on a browser leased for reuse, which was set up when it
    /// started and has just been cleaned: its mode is not set up again.
    pub(crate) fn attached(
        config: &BrowserConfig,
        driver: WebDriver,
        lease: Lease,
    ) -> Result<Self, SeleniumBaseError> {
        let ad_block = match ad_block_commands(config) {
            Ok(commands) => commands,
            Err(error) => {
                // The browser stays open for the next test.
                let _ = driver.leak();
                return Err(error);
            }
        };
        let cdp = config
            .is_cdp_enabled()
            .then(|| CdpClient::from_handle(driver.handle().clone()));
        Ok(Self {
            driver: Some(driver),
            cdp,
            external_page: None,
            driver_process: None,
            virtual_display: None,
            ad_block,
            reuse: Some(lease),
            mobile: mobile_commands(config),
            remote_debug: config.browser.debugging_port,
            proxy_login: None,
            #[cfg(feature = "plugins")]
            plugins: None,
        })
    }

    /// Keeps the browser of this new session for reuse under `key`, held by
    /// this session. The registry takes the driver process and the virtual
    /// display, which live as long as the browser.
    pub(crate) fn share(&mut self, key: ReuseKey) {
        let Some(driver) = self.driver.as_ref() else {
            return;
        };
        let handle = driver.handle();
        let address = SessionAddress {
            server_url: handle.server_url().to_string(),
            session_id: handle.session_id().to_string(),
            capabilities: serde_json::to_value(handle.capabilities()).unwrap_or(Value::Null),
        };
        self.reuse = Some(reuse::register(
            key,
            address,
            self.driver_process.take(),
            self.virtual_display.take(),
        ));
    }

    /// [`attached`](Self::attached), then the proxy login of `config`, if it has
    /// one, read from the `capabilities` the browser was created with.
    ///
    /// A browser kept for reuse was set up by the session that started it, but
    /// the client that answers a proxy's password prompts lives as long as one
    /// session, so each lease starts its own.
    pub(crate) async fn attached_with_login(
        config: &BrowserConfig,
        driver: WebDriver,
        lease: Lease,
        capabilities: &Value,
    ) -> Result<Self, SeleniumBaseError> {
        let mut session = Self::attached(config, driver, lease)?;
        session.begin_proxy_login_with(config, capabilities).await?;
        Ok(session)
    }

    /// Starts answering the proxy's password prompts when `config` has a proxy
    /// with a login, reading the browser's DevTools address from the session's
    /// capabilities.
    pub(super) async fn begin_proxy_login(
        &mut self,
        config: &BrowserConfig,
    ) -> Result<(), SeleniumBaseError> {
        let capabilities = self.driver.as_ref().map_or(Value::Null, |driver| {
            serde_json::to_value(driver.handle().capabilities()).unwrap_or(Value::Null)
        });
        self.begin_proxy_login_with(config, &capabilities).await
    }

    /// [`begin_proxy_login`](Self::begin_proxy_login) with the capabilities
    /// given.
    async fn begin_proxy_login_with(
        &mut self,
        config: &BrowserConfig,
        capabilities: &Value,
    ) -> Result<(), SeleniumBaseError> {
        let Some(credentials) = proxy_auth::login_for(config)? else {
            return Ok(());
        };
        let address = proxy_auth::debugger_address(capabilities).ok_or_else(|| {
            SeleniumBaseError::cdp_driver(
                "the browser did not report a DevTools address (debuggerAddress), so its proxy \
                 cannot be logged in to",
            )
        })?;
        self.proxy_login = Some(ProxyAuthSidecar::attach(&address, credentials).await?);
        Ok(())
    }

    /// Connects through the pure engine's existing config, identity and plugin bridge.
    pub(super) async fn connect_cdp(config: &BrowserConfig) -> Result<Self, SeleniumBaseError> {
        config.validate()?;
        if config.session.reuse || config.recorder_mode() {
            return Err(SeleniumBaseError::invalid_config(
                "direct CDP attachment does not support WebDriver session reuse or Recorder Mode",
            ));
        }
        let options = crate::sb_cdp::LaunchOptions::builder()
            .browser_config(config)
            .build()?;
        let target = options.cdp_target().map(str::to_owned);

        let browser = crate::sb_cdp::Browser::launch(options).await?;
        // Only the window that is picked is prepared: the app's others are
        // left as they are.
        let picked = match &target {
            Some(target) => browser.select_page(target).await,
            None => browser.default_page().await,
        };
        let page = match picked {
            Ok(page) => page,
            Err(error) => {
                browser.close().await?;
                return Err(error);
            }
        };
        let mut session = Self::disconnected();
        session.external_page = Some(page);
        Ok(session)
    }
}

/// DevTools commands, each with its parameters, sent to a window.
type DevtoolsCommands = Vec<(&'static str, Value)>;

/// Everything that can be refused is, before a browser starts: the settings,
/// the mode, a proxy login a remote grid cannot be given, the remote debugging
/// port, and an ad-block list the browser cannot be given.
///
/// Returns the port the session will have for Chrome's remote debugger and the
/// DevTools commands of its ad block.
fn refuse_what_cannot_work(
    config: &mut BrowserConfig,
) -> Result<(Option<u16>, DevtoolsCommands), SeleniumBaseError> {
    config.validate()?;
    validate_mode_support(config)?;
    // A remote grid cannot be given a proxy login: refused before a browser starts.
    proxy_auth::login_for(config)?;
    let remote_debug = resolve_remote_debug(config)?;
    // Worked out first, so a list the browser cannot be given is refused
    // before a browser starts.
    let ad_block = ad_block_commands(config)?;
    Ok((remote_debug, ad_block))
}

/// Gives the window of a running session the geometry the settings ask for,
/// where the browser cannot be given it at launch.
async fn place_window(driver: &WebDriver, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
    // Chromium takes its window geometry as launch arguments; the
    // others have to be told once they are running.
    if config.browser.kind.is_chromium_based() {
        return Ok(());
    }
    apply_window_rect(driver, &config.browser).await?;
    // Chromium is given `--start-maximized`; the others are asked once
    // they are running, and only when they have a window.
    if config.browser.has(LaunchSwitch::Maximize) && !config.browser.headless {
        driver.maximize_window().await?;
    }
    Ok(())
}
