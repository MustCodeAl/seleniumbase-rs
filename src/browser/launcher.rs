//! Local WebDriver binary launcher and port discovery.
//!
//! This module is responsible for starting a local `chromedriver` process on a
//! free port when the crate is configured to manage its own driver lifecycle.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

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
