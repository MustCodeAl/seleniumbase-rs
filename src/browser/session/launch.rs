//! Everything done before a WebDriver session exists: refusing what cannot
//! work, settling the headless variants and the debugging port, finding the
//! browser program, starting the driver and the virtual screen, and opening the
//! WebDriver session itself.

use std::time::Duration;

use thirtyfour::prelude::WebDriver;
use tracing::info;

use crate::browser::binaries;
use crate::browser::chrome_for_testing::{
    install_chrome_for_testing, install_chrome_headless_shell,
};
use crate::browser::chromium_snapshots::install_chromium;
use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::launcher::{launch_driver_for, DriverProcess};
use crate::browser::options::{BrowserSettings, DriverVersion, LaunchSwitch, ReleaseChannel};
use crate::browser::virtual_display::{self, XvfbProcess};
use crate::error::SeleniumBaseError;

use super::capabilities::session_capabilities;

pub(super) fn validate_mode_support(config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
    if config.is_cdp_enabled() && !config.browser.kind.is_chromium_based() {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "CDP/UC mode requires a Chromium-based browser, not {}",
            config.browser.kind
        )));
    }
    if config.is_uc_enabled()
        && (config.browser.kind == Browser::ChromeHeadlessShell || headless1_uses_shell(config))
    {
        return Err(SeleniumBaseError::InvalidConfig(
            "chrome-headless-shell cannot run the undetected (UC) mode".to_owned(),
        ));
    }
    Ok(())
}

/// Opens a WebDriver session, honoring
/// [`BrowserConfig::remote_webdriver_timeout_seconds`] when it is set.
async fn connect_with_caps(
    config: &BrowserConfig,
    url: &str,
    caps: impl Into<thirtyfour::Capabilities>,
) -> Result<WebDriver, SeleniumBaseError> {
    crate::install_tls_provider();
    let url = server_url_for_joins(url);
    let url = url.as_str();
    match config.session.remote_webdriver_timeout_seconds {
        Some(seconds) => Ok(WebDriver::builder(url, caps)
            .request_timeout(Duration::from_secs(seconds))
            .connect()
            .await?),
        None => Ok(WebDriver::new(url, caps).await?),
    }
}

/// `url` with a trailing slash on its path, the form a WebDriver client needs.
///
/// thirtyfour joins each command path (`session`, `session/{id}/url`, ...) onto
/// the server URL as a relative URL, so a last path segment without a slash is
/// replaced: `http://hub:4444/wd/hub` would send `POST /wd/session`, and a
/// server behind a path prefix (such as tauri-plugin-sbrs-automation's token)
/// would never be reached.
pub(crate) fn server_url_for_joins(url: &str) -> String {
    if url.ends_with('/') {
        url.to_owned()
    } else {
        format!("{url}/")
    }
}

async fn try_connect(config: &BrowserConfig, url: &str) -> Result<WebDriver, SeleniumBaseError> {
    crate::browser::downloads::create_folder_for(config.browser.kind);
    let caps = session_capabilities(config)?;
    connect_with_caps(config, url, caps).await
}

/// Moves and resizes the window of a running session, for the browsers that
/// cannot be given the geometry as a launch argument.
pub(super) async fn apply_window_rect(
    driver: &WebDriver,
    launch: &BrowserSettings,
) -> Result<(), SeleniumBaseError> {
    if launch.window_size.is_none() && launch.window_position.is_none() {
        return Ok(());
    }
    let current = driver.get_window_rect().await?;
    let (x, y) = launch
        .window_position
        .map_or((current.x, current.y), |p| (i64::from(p.x), i64::from(p.y)));
    let (width, height) = launch.window_size.map_or(
        (
            u32::try_from(current.width).unwrap_or(1),
            u32::try_from(current.height).unwrap_or(1),
        ),
        |s| (s.width, s.height),
    );
    driver.set_window_rect(x, y, width, height).await?;
    Ok(())
}

/// Whether `headless1` turns this configuration's Chrome into
/// `chrome-headless-shell`: only Chrome with no program of its own named has a
/// shell to stand for it.
fn headless1_uses_shell(config: &BrowserConfig) -> bool {
    config.browser.has(LaunchSwitch::Headless1)
        && config.browser.kind == Browser::Chrome
        && config.browser.executable_path.is_none()
}

/// Settles the headless variants of `config` before a browser is asked for.
///
/// `chrome-headless-shell` always runs headless, so a headed request is
/// overridden. `headless1` makes the session headless and, for Chrome, runs
/// `chrome-headless-shell` (downloaded when missing, as `--chs` does); another
/// Chromium browser has no such program and runs in its new headless mode, and
/// Firefox simply runs headless. (`headless2` is plain headless.)
fn apply_headless_variants(config: &mut BrowserConfig) {
    if config.browser.kind == Browser::ChromeHeadlessShell && !config.browser.headless {
        tracing::info!(
            "chrome-headless-shell only runs headless; the headed request is overridden"
        );
        config.browser.headless = true;
    }
    if !config.browser.has(LaunchSwitch::Headless1) {
        return;
    }
    config.browser.headless = true;
    if headless1_uses_shell(config) {
        config.browser.kind = Browser::ChromeHeadlessShell;
    } else if config.browser.kind.is_chromium_based()
        && config.browser.kind != Browser::ChromeHeadlessShell
    {
        tracing::info!(
            "headless1 needs chrome-headless-shell, which only replaces Chrome with no program of \
             its own named; {} runs in its new headless mode",
            config.browser.kind
        );
    }
}

/// Starts the virtual screen the settings ask for, if this session can use one.
///
/// Python SeleniumBase's `--xvfb` does the same on Linux and nothing elsewhere. A
/// session that connects to a driver somebody else started cannot have its
/// browser moved to another screen, so it gets none either.
pub(super) fn start_virtual_display(
    config: &BrowserConfig,
) -> Result<Option<XvfbProcess>, SeleniumBaseError> {
    let Some(screen) = config.browser.virtual_display else {
        return Ok(None);
    };
    if !virtual_display::wanted(
        Some(screen),
        config.browser.headless,
        cfg!(target_os = "linux"),
    ) {
        tracing::info!("a virtual display was asked for, but this session is headless or not on Linux; none is started");
        return Ok(None);
    }
    if !config.session.auto_start_driver || !config.is_default_webdriver_url() {
        tracing::warn!(
            "a virtual display only works with a driver this crate starts; none is started"
        );
        return Ok(None);
    }
    XvfbProcess::start(screen).map(Some)
}

/// Settles the port of Chrome's remote debugger before the browser starts.
///
/// A browser this crate starts on this machine gets the port that was asked
/// for when it is free. When it is busy, the default port (`with_remote_debug`)
/// gives way to a free one, and a named port (`with_remote_debug_port`) is an
/// error. A browser on a WebDriver server of its own cannot be checked from
/// here. Returns the port the session will have, or `None` for a browser with no
/// remote debugger.
pub(super) fn resolve_remote_debug(
    config: &mut BrowserConfig,
) -> Result<Option<u16>, SeleniumBaseError> {
    let Some(port) = config.browser.debugging_port else {
        return Ok(None);
    };
    if !config.browser.kind.is_chromium_based() {
        return Ok(None);
    }
    if !config.session.auto_start_driver || !config.is_default_webdriver_url() {
        return Ok(Some(port));
    }
    let port = crate::browser::launcher::choose_debug_port(
        port,
        config.browser.debugging_any_port,
        crate::browser::launcher::port_is_free,
        crate::browser::launcher::find_free_port,
    )?;
    config.browser.debugging_port = Some(port);
    info!("Chrome's remote debugger listens on localhost:{port}; open chrome://inspect");
    Ok(Some(port))
}

pub(super) async fn connect_driver(
    config: &BrowserConfig,
    display: Option<&str>,
) -> Result<(WebDriver, Option<DriverProcess>), SeleniumBaseError> {
    let mut config = config.clone();
    apply_headless_variants(&mut config);
    resolve_browser_binary(&mut config).await?;
    patch_chrome_binary_if_needed(&mut config).await?;

    let mut process: Option<DriverProcess> = None;
    let mut url = config.session.webdriver_url();

    if config.session.auto_start_driver && config.is_default_webdriver_url() {
        match try_connect(&config, &url).await {
            Ok(driver) => return Ok((driver, None)),
            Err(_) => {
                let launched = launch_driver_for(&config, display).await?;
                url = launched.url.clone();
                process = Some(launched);
            }
        }
    }

    let driver = try_connect(&config, &url).await?;
    Ok((driver, process))
}

/// Finds the program of a browser that is not Google Chrome and sets it as
/// the browser binary, unless the caller named one.
///
/// # Errors
///
/// Returns an error that says where it looked when Brave, Comet or Opera cannot
/// be found, or the download error when Chrome for Testing or
/// `chrome-headless-shell`, which are fetched when missing, cannot be.
async fn resolve_browser_binary(config: &mut BrowserConfig) -> Result<(), SeleniumBaseError> {
    let needs_program = matches!(
        config.browser.kind,
        Browser::Brave
            | Browser::Comet
            | Browser::Opera
            | Browser::Chromium
            | Browser::ChromeForTesting
            | Browser::ChromeHeadlessShell
    );
    if !needs_program || config.browser.executable_path.is_some() {
        return Ok(());
    }
    let proxy = config
        .browser
        .has(LaunchSwitch::ProxyForDriverDownload)
        .then_some(config.browser.proxy.server.as_deref())
        .flatten();
    let found = match binaries::locate(config.browser.kind) {
        Some(found) => found,
        // Chrome for Testing, chrome-headless-shell and Chromium are downloads, so a missing
        // one is fetched: the stable build of the first two, the newest snapshot of Chromium.
        None => match config.browser.kind {
            Browser::ChromeForTesting => {
                let stable = DriverVersion::Channel(ReleaseChannel::Stable);
                install_chrome_for_testing(&stable, proxy).await?
            }
            Browser::ChromeHeadlessShell => {
                let stable = DriverVersion::Channel(ReleaseChannel::Stable);
                install_chrome_headless_shell(&stable, proxy).await?
            }
            Browser::Chromium => install_chromium(None, proxy).await?,
            _ => {
                return Err(SeleniumBaseError::invalid_config(missing_browser_message(
                    config.browser.kind,
                )))
            }
        },
    };
    // A chromedriver drives only the Chrome of its own major version, and the Chromium found
    // is not the Chrome that is installed: the driver follows the Chromium.
    if config.browser.kind == Browser::Chromium && config.session.driver_version.is_none() {
        config.session.driver_version = Some(DriverVersion::MatchBrowser);
    }
    config.browser.executable_path = Some(found);
    Ok(())
}

/// What to tell the user when a browser's program cannot be found.
fn missing_browser_message(browser: Browser) -> String {
    match browser {
        Browser::ChromeForTesting | Browser::ChromeHeadlessShell => format!(
            "{browser} is not installed: nothing was found under {}/{}. Download it \
             there, or name its program with BrowserConfig::with_browser_binary_path.",
            binaries::DOWNLOADS_DIR,
            if browser == Browser::ChromeForTesting {
                binaries::CHROME_FOR_TESTING_DIR
            } else {
                binaries::CHROME_HEADLESS_SHELL_DIR
            },
        ),
        _ => format!(
            "{browser} was not found in its usual install locations or on the PATH. \
             Install it, or name its program with BrowserConfig::with_browser_binary_path."
        ),
    }
}

async fn patch_chrome_binary_if_needed(
    config: &mut BrowserConfig,
) -> Result<(), SeleniumBaseError> {
    if !config.native_spoofing_enabled() {
        return Ok(());
    }
    if !matches!(
        config.browser.kind,
        Browser::Chrome | Browser::Chromium | Browser::Edge
    ) {
        return Ok(());
    }
    if config.browser.executable_path.is_some() {
        return Ok(());
    }

    // A bad unrelated variable must not hide SB_CHROME_BIN.
    let runtime = crate::config::RuntimeConfig::from_env_lenient().0;
    let source = runtime
        .chrome_bin
        .or_else(crate::evasion::patcher::find_system_chrome)
        .ok_or_else(|| {
            SeleniumBaseError::invalid_config(
                "native_spoofing requires a Chrome/Chromium binary but none was found. \
                 Set SB_CHROME_BIN or BrowserConfig::browser_binary_path."
                    .to_owned(),
            )
        })?;

    let source_str = source.display().to_string();
    tracing::info!(source = %source_str, "native_spoofing enabled: patching chrome binary");

    let patched = tokio::task::spawn_blocking(move || {
        let mut patcher = crate::evasion::patcher::ChromeBinaryPatcher::new(source);
        if let Some(cache) = runtime.patch_cache_dir {
            patcher = patcher.with_cache_dir(cache);
        }
        patcher.patch(crate::evasion::patcher::EnginePatch::chrome_binary())
    })
    .await
    .map_err(|e| {
        SeleniumBaseError::patcher(source_str.clone(), format!("patch task panicked: {e}"))
    })??;

    tracing::info!(path = %patched.display(), "using patched chrome binary");
    config.browser.executable_path = Some(patched);
    Ok(())
}

#[cfg(test)]
mod server_url_tests {
    use super::server_url_for_joins;

    fn session_url(server: &str) -> String {
        let base = url::Url::parse(&server_url_for_joins(server)).expect("a valid URL");
        base.join("session").expect("a relative path").to_string()
    }

    #[test]
    fn a_path_prefix_survives_the_command_paths() {
        assert_eq!(
            session_url("http://hub:4444/wd/hub"),
            "http://hub:4444/wd/hub/session"
        );
        assert_eq!(
            session_url("http://127.0.0.1:4545/e2etoken"),
            "http://127.0.0.1:4545/e2etoken/session"
        );
    }

    #[test]
    fn urls_that_already_work_are_unchanged() {
        assert_eq!(
            session_url("http://localhost:4444"),
            "http://localhost:4444/session"
        );
        assert_eq!(
            session_url("http://hub:4444/wd/hub/"),
            "http://hub:4444/wd/hub/session"
        );
        assert_eq!(
            server_url_for_joins("http://hub:4444/wd/hub/"),
            "http://hub:4444/wd/hub/"
        );
    }
}

#[cfg(test)]
mod headless_variant_tests {
    use super::*;
    use crate::browser::config::DriverMode;

    fn settled(config: BrowserConfig) -> BrowserConfig {
        let mut config = config;
        apply_headless_variants(&mut config);
        config
    }

    #[test]
    fn headless1_runs_chrome_as_chrome_headless_shell() {
        let mut config = BrowserConfig::default()
            .with_headless(false)
            .with_headless1(true);
        config.browser.executable_path = None;
        let settled = settled(config);
        assert_eq!(settled.browser.kind, Browser::ChromeHeadlessShell);
        assert!(settled.browser.headless);
    }

    #[test]
    fn headless1_leaves_a_chrome_with_its_own_program_and_other_browsers_alone() {
        let own = BrowserConfig::default()
            .with_headless1(true)
            .with_browser_binary_path("/opt/chrome/chrome");
        assert_eq!(settled(own).browser.kind, Browser::Chrome);

        for browser in [Browser::Edge, Browser::Brave, Browser::Firefox] {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_headless(false)
                .with_headless1(true);
            let config = settled(config);
            assert_eq!(config.browser.kind, browser);
            assert!(config.browser.headless, "{browser}");
        }
    }

    #[test]
    fn headless2_and_a_plain_session_change_nothing() {
        let config = BrowserConfig::default()
            .with_headless(false)
            .with_headless2(true);
        let after = settled(config.clone());
        assert_eq!(after, config);
        assert!(after.browser.headless);
    }

    #[test]
    fn the_headless_shell_is_made_headless() {
        let shell = BrowserConfig::default()
            .with_browser(Browser::ChromeHeadlessShell)
            .with_headless(false);
        assert!(settled(shell).browser.headless);
    }

    #[test]
    fn headless1_cannot_run_the_undetected_mode_just_as_the_shell_cannot() {
        let mut config = BrowserConfig::default()
            .with_mode(DriverMode::Uc)
            .with_headless1(true);
        config.browser.executable_path = None;
        let error = validate_mode_support(&config).expect_err("refused");
        assert!(
            error
                .to_string()
                .contains("chrome-headless-shell cannot run"),
            "{error}"
        );
        let with_program = config.with_browser_binary_path("/opt/chrome/chrome");
        assert!(validate_mode_support(&with_program).is_ok());
    }
}

#[cfg(test)]
mod remote_debug_tests {
    use super::*;
    use crate::browser::session::BrowserSession;

    #[test]
    fn the_remote_debugger_port_is_settled_before_the_browser_starts() {
        let mut off = BrowserConfig::default();
        assert_eq!(resolve_remote_debug(&mut off).unwrap(), None);

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let busy = listener.local_addr().unwrap().port();

        // A port that was named and is in use is an error.
        let named = BrowserConfig::default().with_remote_debug_port(busy);
        let error = resolve_remote_debug(&mut named.clone())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("port {busy} is already in use")),
            "{error}"
        );

        // The default port gives way to a free one.
        let mut moving = BrowserConfig::default();
        moving.browser.debugging_port = Some(busy);
        moving.browser.debugging_any_port = true;
        let chosen = resolve_remote_debug(&mut moving).unwrap().unwrap();
        assert_ne!(chosen, busy);
        assert_eq!(moving.browser.debugging_port, Some(chosen));

        // A browser on a server of its own is not checked from here, and
        // Firefox has no such debugger.
        let mut elsewhere = named
            .clone()
            .with_webdriver_url("http://hub.test:4444/wd/hub");
        assert_eq!(resolve_remote_debug(&mut elsewhere).unwrap(), Some(busy));
        let mut firefox = named.with_browser(Browser::Firefox);
        assert_eq!(resolve_remote_debug(&mut firefox).unwrap(), None);
        drop(listener);
    }

    #[test]
    fn the_remote_debugger_address_is_localhost_and_its_port() {
        let mut session = BrowserSession::disconnected();
        assert_eq!(session.remote_debug_address(), None);
        session.remote_debug = Some(9222);
        assert_eq!(
            session.remote_debug_address().as_deref(),
            Some("localhost:9222")
        );
    }
}
