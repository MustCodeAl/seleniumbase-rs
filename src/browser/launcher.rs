//! Local WebDriver binary launcher and port discovery.
//!
//! This module is responsible for starting a local `chromedriver` process on a
//! free port when the crate is configured to manage its own driver lifecycle.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::browser::config::Browser;
use crate::browser::downloader::download_chrome_driver_for;
use crate::browser::driver_access::{killed_by_the_system, make_runnable, spawn_repairing};
use crate::browser::driver_version::{binary_major, fit, Fit};
use crate::error::SeleniumBaseError;
use crate::stealth::patcher::find_system_chrome;

/// A running WebDriver process launched by the crate.
pub struct DriverProcess {
    pub url: String,
    child: Child,
}

impl DriverProcess {
    /// Best-effort kill of the underlying chromedriver process.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
    let port = find_free_port()?;
    let binary = ensure_chromedriver_binary().await?;
    let url = format!("http://127.0.0.1:{port}");

    let binary_str = binary.display().to_string();
    let configure = |command: &mut std::process::Command| {
        command
            .arg(format!("--port={port}"))
            .arg("--disable-dev-shm-usage")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    };
    let mut child = spawn_repairing(&binary, configure).inspect_err(|err| {
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
                make_runnable(&binary, true)?;
                child = spawn_repairing(&binary, configure)?;
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

    Ok(DriverProcess { url, child })
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
                format!("chromedriver did not accept connections on {addr} within {timeout:?}"),
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
    /// `operadriver`, Opera's build of chromedriver.
    Operadriver,
    /// `safaridriver`, which comes with macOS.
    Safaridriver,
    /// `IEDriverServer`, for Internet Explorer on Windows.
    InternetExplorerDriver,
}

impl DriverProgram {
    /// The driver `browser` needs. Edge and Firefox are given chromedriver too,
    /// as they always were here; run them against a driver you started and
    /// named with [`BrowserConfig::with_webdriver_url`](crate::BrowserConfig::with_webdriver_url).
    #[must_use]
    pub const fn for_browser(browser: Browser) -> Self {
        match browser {
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
            Self::Operadriver => &["operadriver", "operadriver.exe"],
            Self::Safaridriver => &["safaridriver"],
            Self::InternetExplorerDriver => &["IEDriverServer.exe", "IEDriverServer"],
        }
    }

    /// The arguments that make the driver listen on `port`.
    #[must_use]
    pub fn port_args(self, port: u16) -> Vec<String> {
        match self {
            Self::Safaridriver => vec!["-p".to_owned(), port.to_string()],
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

/// Starts the driver `browser` needs on a free port.
///
/// Chromedriver goes through [`launch_chromedriver`], which downloads one if
/// there is none. Operadriver, safaridriver and IEDriverServer are only looked
/// for, in `downloaded_drivers/`, where the system keeps them, and on the
/// `PATH`; nothing is downloaded for them.
///
/// # Errors
///
/// Returns an error that names the driver when it cannot be found, or when it
/// does not start.
pub async fn launch_driver_for(browser: Browser) -> Result<DriverProcess, SeleniumBaseError> {
    let program = DriverProgram::for_browser(browser);
    if program == DriverProgram::Chromedriver {
        return launch_chromedriver().await;
    }
    let name = program.executable_names()[0];
    let binary = find_driver_program(
        program,
        Path::new(crate::browser::binaries::DOWNLOADS_DIR),
        &|path| path.is_file(),
        &|name| which::which(name).ok(),
    )
    .ok_or_else(|| {
        SeleniumBaseError::browser_launch(
            name,
            format!(
                "{name} was not found in {}/, where the system keeps it, or on the PATH; \
                 put it in one of them, or start it and name its URL with \
                 BrowserConfig::with_webdriver_url",
                crate::browser::binaries::DOWNLOADS_DIR
            ),
        )
    })?;
    launch_driver_binary(&binary, |port| program.port_args(port)).await
}

/// Starts `binary` with the arguments `args` gives for a free port, and waits
/// until it accepts connections.
async fn launch_driver_binary(
    binary: &Path,
    args: impl Fn(u16) -> Vec<String>,
) -> Result<DriverProcess, SeleniumBaseError> {
    let port = find_free_port()?;
    let url = format!("http://127.0.0.1:{port}");
    let binary_str = binary.display().to_string();
    let arguments = args(port);
    let mut child = spawn_repairing(binary, |command| {
        command
            .args(&arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    })?;
    match wait_for_port(port, Duration::from_secs(15), &binary_str, &mut child).await? {
        Waited::Ready => Ok(DriverProcess { url, child }),
        Waited::Exited(status) => Err(SeleniumBaseError::browser_launch(
            binary_str,
            format!("the driver exited as it started ({status})"),
        )),
    }
}

#[cfg(test)]
mod driver_program_tests {
    use super::*;

    #[test]
    fn each_browser_gets_its_own_driver() {
        for (browser, program) in [
            (Browser::Chrome, DriverProgram::Chromedriver),
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
    fn the_port_is_given_the_way_each_driver_takes_it() {
        assert_eq!(DriverProgram::Safaridriver.port_args(4723), ["-p", "4723"]);
        assert_eq!(DriverProgram::Operadriver.port_args(9515), ["--port=9515"]);
        assert_eq!(
            DriverProgram::InternetExplorerDriver.port_args(1),
            ["--port=1"]
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
        let error = launch_driver_for(Browser::Opera).await.unwrap_err();
        let text = error.to_string();
        assert!(text.contains("operadriver"), "{text}");
        assert!(text.contains("with_webdriver_url"), "{text}");
    }
}
