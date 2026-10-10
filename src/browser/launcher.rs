//! Local WebDriver binary launcher and port discovery.
//!
//! This module is responsible for starting a local `chromedriver` process on a
//! free port when the crate is configured to manage its own driver lifecycle.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::browser::downloader::download_chrome_driver;
use crate::browser::driver_access::{killed_by_the_system, make_runnable, spawn_repairing};
use crate::error::SeleniumBaseError;

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

/// Return the path to the chromedriver binary, downloading it if necessary.
async fn ensure_chromedriver_binary() -> Result<PathBuf, SeleniumBaseError> {
    let candidate_names = if cfg!(windows) {
        vec!["chromedriver.exe"]
    } else {
        vec!["chromedriver"]
    };

    let dest_dir = PathBuf::from("downloaded_drivers");
    for name in &candidate_names {
        let path = dest_dir.join(name);
        if path.exists() {
            return Ok(path);
        }
    }

    let downloaded = download_chrome_driver().await?;
    if downloaded.exists() {
        return Ok(downloaded);
    }

    Err(SeleniumBaseError::browser_launch(
        "chromedriver",
        "binary not found in PATH or downloaded_drivers/ and download returned nothing",
    ))
}
