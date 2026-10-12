//! Local WebDriver binary launcher and port discovery.
//!
//! This module starts the driver a browser needs (`chromedriver`, `geckodriver`
//! for Firefox, `msedgedriver` for Edge, and the drivers of Opera, Safari and
//! Internet Explorer) on a free port when the crate manages the driver's
//! lifecycle, downloading chromedriver, geckodriver and msedgedriver when none
//! is found.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::browser::binaries::{locate, DOWNLOADS_DIR};
use crate::browser::chrome_for_testing::install_chromedriver;
use crate::browser::config::{Browser, BrowserConfig};
use crate::browser::downloader::download_chrome_driver_for;
use crate::browser::driver_access::{killed_by_the_system, make_runnable, spawn_repairing};
use crate::browser::driver_install::{
    downloaded_driver, install_geckodriver, install_msedgedriver,
};
use crate::browser::driver_version::{binary_major, fit, Fit};
use crate::browser::options::{DriverVersion, LaunchSwitch};
use crate::common::orphan_guard::OrphanGuard;
use crate::error::SeleniumBaseError;
use crate::stealth::patcher::find_system_chrome;

/// A running WebDriver process launched by the crate.
///
/// The driver is ended when this value is dropped, so a session that failed to
/// start does not leave it running, on every system. On Unix the driver and the
/// browser it starts also end with this program when the program is killed
/// without quitting them.
pub struct DriverProcess {
    pub url: String,
    child: Child,
    _guard: OrphanGuard,
}

impl DriverProcess {
    /// The operating system's id of the driver process.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Best-effort kill of the underlying driver process.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for DriverProcess {
    fn drop(&mut self) {
        // Ending an exited process again only returns an error, ignored here.
        self.kill();
    }
}

#[cfg(all(test, unix))]
mod driver_process_tests {
    use super::*;

    fn running(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// A driver whose session failed to start is dropped, never quit; it must
    /// end too (Windows has no process-group watcher to do it).
    #[test]
    fn a_dropped_driver_process_is_ended() {
        let child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id();
        let driver = DriverProcess {
            url: "http://127.0.0.1:1".to_owned(),
            child,
            // A watcher with nothing enrolled, so only the drop can end `sleep`.
            _guard: OrphanGuard::start(),
        };
        assert!(running(pid));
        drop(driver);
        assert!(!running(pid), "the driver outlived its handle");
    }
}

/// Launch a local chromedriver on a free port and return its WebDriver URL.
///
/// If the system will not run the driver, it is repaired once and started
/// again: a missing execute permission is added, and on macOS the quarantine
/// flag is removed and a binary whose signature no longer matches (the system
/// kills it as it starts) is signed again. See
/// [`driver_access`](crate::browser::driver_access).
pub async fn launch_chromedriver() -> Result<DriverProcess, SeleniumBaseError> {
    let binary = ensure_chromedriver_binary().await?;
    launch_chromedriver_at(&binary, None).await
}

/// Launch the chromedriver at `binary` on a free port, with the repairs of
/// [`launch_chromedriver`].
///
/// `display` is the X display (such as `:99`) that the browser the driver starts
/// opens its window on; `None` leaves the environment as it is.
pub async fn launch_chromedriver_at(
    binary: &Path,
    display: Option<&str>,
) -> Result<DriverProcess, SeleniumBaseError> {
    let port = find_free_port()?;
    let url = format!("http://127.0.0.1:{port}");

    let binary_str = binary.display().to_string();
    let guard = OrphanGuard::start();
    let configure = |command: &mut std::process::Command| {
        command
            .arg(format!("--port={port}"))
            .arg("--disable-dev-shm-usage")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(display) = display {
            command.env("DISPLAY", display);
        }
        guard.enroll(command);
    };
    let mut child = spawn_repairing(binary, configure).inspect_err(|err| {
        err.log_in_context("launch_chromedriver");
    })?;

    let mut repaired = false;
    loop {
        match wait_for_port(port, Duration::from_secs(15), &binary_str, &mut child).await? {
            Waited::Ready => break,
            Waited::Exited(status)
                if cfg!(target_os = "macos") && !repaired && killed_by_the_system(status) =>
            {
                // macOS kills a driver whose signature or quarantine flag it
                // rejects. Fix the file and start it once more.
                repaired = true;
                make_runnable(binary, true)?;
                child = spawn_repairing(binary, configure)?;
            }
            Waited::Exited(status) => {
                let err = SeleniumBaseError::browser_launch(
                    binary_str,
                    format!("chromedriver exited as it started ({status})"),
                );
                err.log_in_context("launch_chromedriver");
                return Err(err);
            }
        }
    }

    Ok(DriverProcess {
        url,
        child,
        _guard: guard,
    })
}

/// How waiting for the driver's port ended.
enum Waited {
    /// The driver accepts connections.
    Ready,
    /// The driver process ended first.
    Exited(ExitStatus),
}

/// Find an unused TCP port on localhost.
fn find_free_port() -> Result<u16, SeleniumBaseError> {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = TcpListener::bind(addr).map_err(|e| {
        SeleniumBaseError::browser_launch("127.0.0.1:0", format!("bind failed: {e}"))
    })?;
    let port = listener.local_addr().map_err(|e| {
        SeleniumBaseError::browser_launch("127.0.0.1:0", format!("local_addr failed: {e}"))
    })?;
    drop(listener);
    Ok(port.port())
}

/// Poll the port until the driver accepts a TCP connection, its process ends,
/// or the timeout passes.
async fn wait_for_port(
    port: u16,
    timeout: Duration,
    binary: &str,
    child: &mut Child,
) -> Result<Waited, SeleniumBaseError> {
    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect(&addr).is_ok() {
            return Ok(Waited::Ready);
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Ok(Waited::Exited(status));
        }
        if Instant::now() >= deadline {
            let err = SeleniumBaseError::browser_launch(
                binary.to_owned(),
                format!("the driver did not accept connections on {addr} within {timeout:?}"),
            );
            err.log_in_context("wait_for_port");
            return Err(err);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Return the path to a chromedriver binary that matches the installed Chrome.
///
/// A chromedriver only drives the Chrome of its own major version. A driver in
/// `downloaded_drivers/` is used as it is when its version matches, or when
/// either version cannot be read. When the versions differ, the driver for the
/// installed Chrome is downloaded in its place, falling back to the latest
/// stable one, and to the old driver only when nothing could be downloaded.
async fn ensure_chromedriver_binary() -> Result<PathBuf, SeleniumBaseError> {
    let name = if cfg!(windows) {
        "chromedriver.exe"
    } else {
        "chromedriver"
    };
    let existing = PathBuf::from("downloaded_drivers").join(name);
    let browser = match find_system_chrome() {
        Some(chrome) => binary_major(&chrome).await,
        None => None,
    };

    if existing.exists() {
        match fit(binary_major(&existing).await, browser) {
            Fit::Mismatch { driver, browser } => {
                tracing::warn!(
                    driver,
                    browser,
                    path = %existing.display(),
                    "chromedriver {driver} cannot drive Chrome {browser}; fetching the matching driver"
                );
            }
            Fit::Matches | Fit::Unknown => return Ok(existing),
        }
    }

    let attempts: &[Option<u32>] = if browser.is_some() {
        &[browser, None]
    } else {
        &[None]
    };
    let mut last_error = None;
    for &major in attempts {
        match download_chrome_driver_for(major).await {
            Ok(path) if path.exists() => return Ok(path),
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, ?major, "chromedriver download failed");
                last_error = Some(error);
            }
        }
    }
    if existing.exists() {
        tracing::warn!("using the chromedriver on disk although its version does not match");
        return Ok(existing);
    }
    Err(last_error.unwrap_or_else(|| {
        SeleniumBaseError::browser_launch(
            "chromedriver",
            "binary not found in downloaded_drivers/ and download returned nothing",
        )
    }))
}

/// The program that automates a browser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriverProgram {
    /// `chromedriver`, for Chrome and every browser built on it that does not
    /// ship a driver of its own.
    Chromedriver,
    /// `geckodriver`, for Firefox.
    Geckodriver,
    /// `msedgedriver`, for Microsoft Edge.
    Msedgedriver,
    /// `operadriver`, Opera's build of chromedriver.
    Operadriver,
    /// `safaridriver`, which comes with macOS.
    Safaridriver,
    /// `IEDriverServer`, for Internet Explorer on Windows.
    InternetExplorerDriver,
}

impl DriverProgram {
    /// The driver `browser` needs: geckodriver for Firefox, msedgedriver for
    /// Edge, and chromedriver for Chrome and the Chromium browsers that have no
    /// driver of their own.
    #[must_use]
    pub const fn for_browser(browser: Browser) -> Self {
        match browser {
            Browser::Firefox => Self::Geckodriver,
            Browser::Edge => Self::Msedgedriver,
            Browser::Opera => Self::Operadriver,
            Browser::Safari => Self::Safaridriver,
            Browser::InternetExplorer => Self::InternetExplorerDriver,
            _ => Self::Chromedriver,
        }
    }

    /// The file names the driver goes by, most usual first.
    #[must_use]
    pub const fn executable_names(self) -> &'static [&'static str] {
        match self {
            Self::Chromedriver => &["chromedriver", "chromedriver.exe"],
            Self::Geckodriver => &["geckodriver", "geckodriver.exe"],
            Self::Msedgedriver => &["msedgedriver", "msedgedriver.exe"],
            Self::Operadriver => &["operadriver", "operadriver.exe"],
            Self::Safaridriver => &["safaridriver"],
            Self::InternetExplorerDriver => &["IEDriverServer.exe", "IEDriverServer"],
        }
    }

    /// The arguments that make the driver listen on `port`.
    ///
    /// geckodriver is also given `--websocket-port 0`, so the system picks the
    /// WebDriver BiDi port: its default, 9222, would let only one Firefox run at
    /// a time.
    #[must_use]
    pub fn port_args(self, port: u16) -> Vec<String> {
        match self {
            Self::Safaridriver => vec!["-p".to_owned(), port.to_string()],
            Self::Geckodriver => vec![
                "--port".to_owned(),
                port.to_string(),
                "--websocket-port".to_owned(),
                "0".to_owned(),
            ],
            _ => vec![format!("--port={port}")],
        }
    }

    /// Where the operating system keeps the driver, if it comes with it.
    fn system_location(self) -> Option<&'static str> {
        (self == Self::Safaridriver).then_some("/usr/bin/safaridriver")
    }
}

/// Finds a driver program: in `downloads`, then where the system keeps it, then
/// on the `PATH`.
pub(crate) fn find_driver_program(
    program: DriverProgram,
    downloads: &Path,
    is_file: &dyn Fn(&Path) -> bool,
    on_path: &dyn Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let names = program.executable_names();
    names
        .iter()
        .map(|name| downloads.join(name))
        .chain(program.system_location().map(PathBuf::from))
        .find(|path| is_file(path))
        .or_else(|| names.iter().find_map(|name| on_path(name)))
}

/// Finds a downloaded driver program: right in `downloads`, or in the newest
/// `<name>-<version>/` folder there; then on the `PATH`.
pub(crate) fn find_downloaded_or_on_path(
    program: DriverProgram,
    downloads: &Path,
    on_path: &dyn Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let names = program.executable_names();
    downloaded_driver(downloads, names[0], names)
        .or_else(|| names.iter().find_map(|name| on_path(name)))
}

/// The proxy downloads go through: the session's, when
/// [`with_proxy_for_driver_download`](BrowserConfig::with_proxy_for_driver_download)
/// asks for it.
fn download_proxy(config: &BrowserConfig) -> Option<String> {
    config
        .launch
        .has(LaunchSwitch::ProxyForDriverDownload)
        .then(|| config.proxy.clone())
        .flatten()
}

/// The geckodriver a Firefox session runs: one in `downloaded_drivers/`, else
/// the one on the `PATH`, else the newest release, downloaded.
///
/// One geckodriver drives a range of Firefox versions, so no version is
/// compared.
async fn ensure_geckodriver(config: &BrowserConfig) -> Result<PathBuf, SeleniumBaseError> {
    let found = find_downloaded_or_on_path(
        DriverProgram::Geckodriver,
        Path::new(DOWNLOADS_DIR),
        &|name| which::which(name).ok(),
    );
    match found {
        Some(found) => Ok(found),
        None => install_geckodriver(None, download_proxy(config).as_deref()).await,
    }
}

/// The msedgedriver an Edge session runs.
///
/// msedgedriver only drives the Edge of its own major version. A version named
/// with [`with_driver_version`](BrowserConfig::with_driver_version) is fetched
/// as [`install_msedgedriver`] says. Otherwise the driver in
/// `downloaded_drivers/` or on the `PATH` is used when it fits the installed
/// Edge (or when either version cannot be read), and the driver for that Edge
/// is downloaded when it does not, or when there is none; a driver that does
/// not fit is the last resort when nothing could be downloaded.
async fn ensure_msedgedriver(config: &BrowserConfig) -> Result<PathBuf, SeleniumBaseError> {
    let edge = match config
        .browser_binary_path
        .clone()
        .or_else(|| locate(Browser::Edge))
    {
        Some(program) => binary_major(&program).await,
        None => None,
    };
    let proxy = download_proxy(config);
    if let Some(version) = &config.launch.driver_version {
        return install_msedgedriver(version, edge, proxy.as_deref()).await;
    }
    let found = find_downloaded_or_on_path(
        DriverProgram::Msedgedriver,
        Path::new(DOWNLOADS_DIR),
        &|name| which::which(name).ok(),
    );
    let stale = match found {
        Some(found) => match fit(binary_major(&found).await, edge) {
            Fit::Mismatch { driver, browser } => {
                tracing::warn!(
                    driver,
                    browser,
                    path = %found.display(),
                    "msedgedriver {driver} cannot drive Edge {browser}; fetching the matching driver"
                );
                Some(found)
            }
            _ => return Ok(found),
        },
        None => None,
    };
    match install_msedgedriver(&DriverVersion::MatchBrowser, edge, proxy.as_deref()).await {
        Ok(path) => Ok(path),
        Err(error) => match stale {
            Some(stale) => {
                tracing::warn!(%error, "using the msedgedriver found although its version does not match");
                Ok(stale)
            }
            None => Err(error),
        },
    }
}

/// Which chromedriver a session runs.
#[derive(Clone, Eq, PartialEq)]
pub(crate) enum ChromedriverPlan {
    /// The one in `downloaded_drivers/` when it fits the installed Chrome,
    /// otherwise a download for that Chrome.
    Default,
    /// The build `version` names, downloaded when it is not on disk.
    Pinned {
        /// Which build.
        version: DriverVersion,
        /// The browser whose version [`DriverVersion::MatchBrowser`] follows;
        /// the installed Chrome when unset.
        browser: Option<PathBuf>,
        /// The proxy to download through.
        proxy: Option<String>,
    },
}

// By hand: the proxy may carry a password.
impl std::fmt::Debug for ChromedriverPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => f.write_str("Default"),
            Self::Pinned {
                version,
                browser,
                proxy,
            } => f
                .debug_struct("Pinned")
                .field("version", version)
                .field("browser", browser)
                .field("proxy", &proxy.as_ref().map(|_| "<redacted>"))
                .finish(),
        }
    }
}

/// Which chromedriver `config` calls for.
///
/// A driver version named with
/// [`with_driver_version`](BrowserConfig::with_driver_version) is pinned. Brave,
/// Comet, Chrome for Testing and `chrome-headless-shell` follow the version of
/// their own program, since the Chrome that is installed is not the browser they
/// run. A proxy for downloads
/// ([`with_proxy_for_driver_download`](BrowserConfig::with_proxy_for_driver_download))
/// also needs a download of our own, to put the proxy in front of it.
pub(crate) fn chromedriver_plan(config: &BrowserConfig) -> ChromedriverPlan {
    let proxy = download_proxy(config);
    let runs_its_own_browser = matches!(
        config.browser,
        Browser::Brave | Browser::Comet | Browser::ChromeForTesting | Browser::ChromeHeadlessShell
    );
    let version = config.launch.driver_version.clone().or_else(|| {
        (runs_its_own_browser || proxy.is_some()).then_some(DriverVersion::MatchBrowser)
    });
    match version {
        None => ChromedriverPlan::Default,
        Some(version) => ChromedriverPlan::Pinned {
            version,
            browser: config.browser_binary_path.clone(),
            proxy,
        },
    }
}

/// Starts the driver `config.browser` needs on a free port.
///
/// Chromedriver is the one in `downloaded_drivers/`, or a download, as
/// [`chromedriver_plan`] says. geckodriver (Firefox) and msedgedriver (Edge)
/// are looked for in `downloaded_drivers/` and on the `PATH`, and downloaded
/// when missing; msedgedriver is also replaced when it does not fit the
/// installed Edge. Operadriver, safaridriver and IEDriverServer are only looked
/// for, in `downloaded_drivers/`, where the system keeps them, and on the
/// `PATH`; nothing is downloaded for them. `display` is the X display (such as
/// `:99`) the browser opens its window on.
///
/// # Errors
///
/// Returns an error that names the driver when it cannot be found, or when it
/// does not start.
pub async fn launch_driver_for(
    config: &BrowserConfig,
    display: Option<&str>,
) -> Result<DriverProcess, SeleniumBaseError> {
    let program = DriverProgram::for_browser(config.browser);
    if program == DriverProgram::Chromedriver {
        let binary = match chromedriver_plan(config) {
            ChromedriverPlan::Default => ensure_chromedriver_binary().await?,
            ChromedriverPlan::Pinned {
                version,
                browser,
                proxy,
            } => {
                let program = browser.or_else(find_system_chrome);
                let major = match &program {
                    Some(program) => binary_major(program).await,
                    None => None,
                };
                install_chromedriver(&version, major, proxy.as_deref()).await?
            }
        };
        return launch_chromedriver_at(&binary, display).await;
    }
    let binary = match program {
        DriverProgram::Geckodriver => ensure_geckodriver(config).await?,
        DriverProgram::Msedgedriver => ensure_msedgedriver(config).await?,
        _ => {
            let name = program.executable_names()[0];
            find_driver_program(
                program,
                Path::new(DOWNLOADS_DIR),
                &|path| path.is_file(),
                &|name| which::which(name).ok(),
            )
            .ok_or_else(|| {
                SeleniumBaseError::browser_launch(
                    name,
                    format!(
                        "{name} was not found in {DOWNLOADS_DIR}/, where the system keeps it, \
                         or on the PATH; put it in one of them, or start it and name its URL \
                         with BrowserConfig::with_webdriver_url"
                    ),
                )
            })?
        }
    };
    launch_driver_binary(&binary, |port| program.port_args(port), display).await
}

/// Starts `binary` with the arguments `args` gives for a free port, and waits
/// until it accepts connections.
///
/// The driver and the browser it starts get the [`OrphanGuard`], and the
/// repairs of [`launch_chromedriver`] apply: on macOS a driver the system kills
/// as it starts is signed again and started once more.
async fn launch_driver_binary(
    binary: &Path,
    args: impl Fn(u16) -> Vec<String>,
    display: Option<&str>,
) -> Result<DriverProcess, SeleniumBaseError> {
    let port = find_free_port()?;
    let url = format!("http://127.0.0.1:{port}");
    let binary_str = binary.display().to_string();
    let arguments = args(port);
    let guard = OrphanGuard::start();
    let configure = |command: &mut std::process::Command| {
        command
            .args(&arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(display) = display {
            command.env("DISPLAY", display);
        }
        guard.enroll(command);
    };
    let mut child = spawn_repairing(binary, configure)?;
    let mut repaired = false;
    loop {
        match wait_for_port(port, Duration::from_secs(15), &binary_str, &mut child).await? {
            Waited::Ready => break,
            Waited::Exited(status)
                if cfg!(target_os = "macos") && !repaired && killed_by_the_system(status) =>
            {
                repaired = true;
                make_runnable(binary, true)?;
                child = spawn_repairing(binary, configure)?;
            }
            Waited::Exited(status) => {
                return Err(SeleniumBaseError::browser_launch(
                    binary_str,
                    format!("the driver exited as it started ({status})"),
                ))
            }
        }
    }
    Ok(DriverProcess {
        url,
        child,
        _guard: guard,
    })
}

#[cfg(test)]
mod driver_program_tests {
    use super::*;

    #[test]
    fn each_browser_gets_its_own_driver() {
        for (browser, program) in [
            (Browser::Chrome, DriverProgram::Chromedriver),
            (Browser::Chromium, DriverProgram::Chromedriver),
            (Browser::Firefox, DriverProgram::Geckodriver),
            (Browser::Edge, DriverProgram::Msedgedriver),
            (Browser::Brave, DriverProgram::Chromedriver),
            (Browser::Comet, DriverProgram::Chromedriver),
            (Browser::ChromeForTesting, DriverProgram::Chromedriver),
            (Browser::ChromeHeadlessShell, DriverProgram::Chromedriver),
            (Browser::Opera, DriverProgram::Operadriver),
            (Browser::Safari, DriverProgram::Safaridriver),
            (
                Browser::InternetExplorer,
                DriverProgram::InternetExplorerDriver,
            ),
        ] {
            assert_eq!(DriverProgram::for_browser(browser), program, "{browser}");
        }
    }

    #[test]
    fn a_plain_session_keeps_the_default_driver() {
        assert_eq!(
            chromedriver_plan(&BrowserConfig::default()),
            ChromedriverPlan::Default
        );
        // A proxy alone changes nothing: it is for the session, not the download.
        let proxied = BrowserConfig::default().with_proxy("u:p@proxy.test:8080");
        assert_eq!(chromedriver_plan(&proxied), ChromedriverPlan::Default);
        // The switch with no proxy to use changes nothing either.
        let switch_only = BrowserConfig::default().with_proxy_for_driver_download(true);
        assert_eq!(chromedriver_plan(&switch_only), ChromedriverPlan::Default);
    }

    #[test]
    fn a_driver_version_pins_the_driver() {
        let config = BrowserConfig::default().with_driver_version("126".parse().unwrap());
        assert_eq!(
            chromedriver_plan(&config),
            ChromedriverPlan::Pinned {
                version: DriverVersion::Milestone(126),
                browser: None,
                proxy: None,
            }
        );
    }

    #[test]
    fn browsers_with_their_own_program_follow_the_version_of_that_program() {
        for browser in [
            Browser::Brave,
            Browser::Comet,
            Browser::ChromeForTesting,
            Browser::ChromeHeadlessShell,
        ] {
            let config = BrowserConfig::default()
                .with_browser(browser)
                .with_browser_binary_path("/opt/browser/program");
            assert_eq!(
                chromedriver_plan(&config),
                ChromedriverPlan::Pinned {
                    version: DriverVersion::MatchBrowser,
                    browser: Some("/opt/browser/program".into()),
                    proxy: None,
                },
                "{browser}"
            );
        }
        // An explicit version still wins.
        let pinned = BrowserConfig::default()
            .with_browser(Browser::Brave)
            .with_driver_version(DriverVersion::Milestone(120));
        assert!(matches!(
            chromedriver_plan(&pinned),
            ChromedriverPlan::Pinned {
                version: DriverVersion::Milestone(120),
                ..
            }
        ));
    }

    #[test]
    fn the_session_proxy_is_used_for_downloads_only_when_asked_and_never_printed() {
        let config = BrowserConfig::default()
            .with_proxy("user:hunter2@proxy.test:8080")
            .with_proxy_for_driver_download(true);
        let plan = chromedriver_plan(&config);
        assert_eq!(
            plan,
            ChromedriverPlan::Pinned {
                version: DriverVersion::MatchBrowser,
                browser: None,
                proxy: Some("user:hunter2@proxy.test:8080".into()),
            }
        );
        let shown = format!("{plan:?}");
        assert!(!shown.contains("hunter2"), "{shown}");
    }

    #[test]
    fn the_port_is_given_the_way_each_driver_takes_it() {
        assert_eq!(DriverProgram::Safaridriver.port_args(4723), ["-p", "4723"]);
        assert_eq!(DriverProgram::Operadriver.port_args(9515), ["--port=9515"]);
        assert_eq!(
            DriverProgram::InternetExplorerDriver.port_args(1),
            ["--port=1"]
        );
        assert_eq!(DriverProgram::Msedgedriver.port_args(9515), ["--port=9515"]);
        // geckodriver takes the port as a separate word, and its BiDi port is
        // left to the system so two Firefox sessions can run at once.
        assert_eq!(
            DriverProgram::Geckodriver.port_args(4444),
            ["--port", "4444", "--websocket-port", "0"]
        );
    }

    #[test]
    fn firefox_and_edge_drivers_go_by_their_own_names() {
        assert_eq!(
            DriverProgram::Geckodriver.executable_names(),
            ["geckodriver", "geckodriver.exe"]
        );
        assert_eq!(
            DriverProgram::Msedgedriver.executable_names(),
            ["msedgedriver", "msedgedriver.exe"]
        );
    }

    #[test]
    fn a_downloaded_driver_wins_over_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let on_path = |name: &str| {
            (name == "geckodriver").then(|| PathBuf::from("/opt/homebrew/bin/geckodriver"))
        };
        // Nothing downloaded: the PATH answers.
        assert_eq!(
            find_downloaded_or_on_path(DriverProgram::Geckodriver, dir.path(), &on_path),
            Some("/opt/homebrew/bin/geckodriver".into())
        );
        assert_eq!(
            find_downloaded_or_on_path(DriverProgram::Msedgedriver, dir.path(), &on_path),
            None
        );
        // A download in its versioned folder wins.
        let folder = dir.path().join("geckodriver-v0.37.1");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("geckodriver"), b"x").unwrap();
        assert_eq!(
            find_downloaded_or_on_path(DriverProgram::Geckodriver, dir.path(), &on_path),
            Some(folder.join("geckodriver"))
        );
    }

    #[test]
    fn the_download_proxy_is_the_session_proxy_only_when_asked() {
        let proxied = BrowserConfig::default().with_proxy("u:p@proxy.test:8080");
        assert_eq!(download_proxy(&proxied), None);
        let asked = proxied.with_proxy_for_driver_download(true);
        assert_eq!(
            download_proxy(&asked).as_deref(),
            Some("u:p@proxy.test:8080")
        );
    }

    #[test]
    fn a_download_wins_over_the_system_copy_and_the_path_is_last() {
        let downloads = Path::new("/d");
        let on_path =
            |name: &str| (name == "operadriver").then(|| PathBuf::from("/bin/operadriver"));
        let has = |wanted: &'static str| move |path: &Path| path == Path::new(wanted);

        assert_eq!(
            find_driver_program(
                DriverProgram::Operadriver,
                downloads,
                &has("/d/operadriver"),
                &on_path
            ),
            Some("/d/operadriver".into())
        );
        assert_eq!(
            find_driver_program(DriverProgram::Operadriver, downloads, &|_| false, &on_path),
            Some("/bin/operadriver".into())
        );
        // safaridriver is where macOS keeps it.
        assert_eq!(
            find_driver_program(
                DriverProgram::Safaridriver,
                downloads,
                &has("/usr/bin/safaridriver"),
                &|_| None
            ),
            Some("/usr/bin/safaridriver".into())
        );
        assert_eq!(
            find_driver_program(
                DriverProgram::InternetExplorerDriver,
                downloads,
                &|_| false,
                &|_| None
            ),
            None
        );
    }

    #[tokio::test]
    async fn a_driver_that_cannot_be_found_is_an_error_that_names_it() {
        // Neither driver exists in this crate's working directory or, on a
        // machine without Opera, on the PATH.
        if which::which("operadriver").is_ok() {
            return;
        }
        let config = BrowserConfig::default().with_browser(Browser::Opera);
        let Err(error) = launch_driver_for(&config, None).await else {
            panic!("operadriver was found where none should be");
        };
        let text = error.to_string();
        assert!(text.contains("operadriver"), "{text}");
        assert!(text.contains("with_webdriver_url"), "{text}");
    }
}
