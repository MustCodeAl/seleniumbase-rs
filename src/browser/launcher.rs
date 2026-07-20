use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::browser::downloader::download_chrome_driver;
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
pub async fn launch_chromedriver() -> Result<DriverProcess, SeleniumBaseError> {
    let port = find_free_port()?;
    let binary = ensure_chromedriver_binary().await?;
    let url = format!("http://127.0.0.1:{port}");

    let child = Command::new(&binary)
        .arg(format!("--port={port}"))
        .arg("--disable-dev-shm-usage")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to spawn chromedriver at {:?}: {}",
                binary, e
            ))
        })?;

    wait_for_port(port, Duration::from_secs(15)).await?;

    Ok(DriverProcess { url, child })
}

/// Find an unused TCP port on localhost.
fn find_free_port() -> Result<u16, SeleniumBaseError> {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = TcpListener::bind(addr).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to bind ephemeral port: {e}"))
    })?;
    let port = listener.local_addr().map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to read local address: {e}"))
    })?;
    drop(listener);
    Ok(port.port())
}

/// Poll the port until the driver accepts a TCP connection or timeout.
async fn wait_for_port(port: u16, timeout: Duration) -> Result<(), SeleniumBaseError> {
    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect(&addr).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(SeleniumBaseError::WaitTimeout(format!(
                "chromedriver did not start on port {port} within {:?}",
                timeout
            )));
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

    Err(SeleniumBaseError::InvalidConfig(
        "chromedriver binary not found after download".to_owned(),
    ))
}
