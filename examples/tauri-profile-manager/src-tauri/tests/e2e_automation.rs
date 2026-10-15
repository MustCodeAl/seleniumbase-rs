//! Drives the app's real window with `BaseCase`, over the WebDriver endpoint
//! that `tauri-plugin-sbrs-automation` serves inside the app (feature `e2e`).
//!
//! The test starts the app Cargo built for it, waits for the endpoint, then
//! reads the title, finds the profile list, creates a profile through the
//! dialog, filters the list by typing in the search box and adds a folder. It
//! stops the app when it ends, also when it fails.
//!
//! It opens the app's window, so it is ignored by default. Run it with:
//!
//! ```text
//! cd examples/tauri-profile-manager/src-tauri
//! cargo test --features e2e --test e2e_automation -- --ignored
//! ```
//!
//! The app gets a home folder of its own (`HOME`) and a vault passphrase of
//! its own (`SB_PROFILE_PASSPHRASE`), so the real profile vault and the
//! keychain are never touched. That is why the test is for macOS and Linux
//! only: on Windows the app-data folder does not follow `HOME`. Ports 4545
//! (the plugin) and 45001 (the app's REST API) must be free. Clicks and typing
//! are DOM events in the page; the mouse and the keyboard are not used.
#![cfg(unix)]

use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use seleniumbase_rs::{BaseCase, BrowserConfig};

/// The secret at the start of every URL of the plugin's endpoint.
const TOKEN: &str = "e2etoken";
/// The port of the plugin's endpoint.
const PORT: u16 = 4545;
/// The WebDriver server URL. It ends with `/` on purpose: the client joins
/// each command path (`session`, `session/<id>/title`, ...) to it as a
/// relative URL, which would replace a last segment `e2etoken` that has no
/// slash after it.
const WEBDRIVER_URL: &str = "http://127.0.0.1:4545/e2etoken/";
/// A profile card in the list.
const CARD: &str = "#profile-list li.profile-card";
/// The window's title, from `src/index.html`.
const TITLE: &str = "Profiles — SeleniumBase Profile Manager";

/// The app, started for this test and stopped when this is dropped.
struct App {
    child: Child,
}

impl App {
    /// Starts the app with the automation endpoint on, a home folder of its
    /// own in `home`, and a passphrase of its own for the new vault there.
    fn start(home: &Path) -> std::io::Result<Self> {
        let child = Command::new(env!("CARGO_BIN_EXE_tauri-profile-manager"))
            .env("SBRS_AUTOMATION", "1")
            .env("SBRS_AUTOMATION_TOKEN", TOKEN)
            .env("SBRS_AUTOMATION_PORT", PORT.to_string())
            .env("HOME", home)
            .env("XDG_DATA_HOME", home.join("data"))
            .env("SB_PROFILE_PASSPHRASE", "e2e test vault, not a secret")
            .stdin(Stdio::null())
            .spawn()?;
        Ok(Self { child })
    }

    /// Waits until the endpoint answers its status, failing at once if the
    /// app exits first.
    fn wait_until_served(&mut self, timeout: Duration) -> Result<(), String> {
        let started = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    return Err(format!("the app exited before it was ready: {status}"))
                }
                Ok(None) => {}
                Err(error) => return Err(format!("cannot watch the app: {error}")),
            }
            if endpoint_is_ready() {
                return Ok(());
            }
            if started.elapsed() > timeout {
                return Err(format!(
                    "the app did not serve http://127.0.0.1:{PORT}/{TOKEN}/status in {timeout:?}"
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Only the process this test started.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Whether `GET /<token>/status` answers `ready`.
fn endpoint_is_ready() -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", PORT)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let request =
        format!("GET /{TOKEN}/status HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    answer.starts_with("HTTP/1.1 200") && answer.contains("\"ready\":true")
}

/// Waits until the list shows `count` profile cards.
async fn wait_for_cards(sb: &mut BaseCase, count: usize) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    loop {
        let shown = sb.find_elements(CARD).await?.len();
        if shown == count {
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(10) {
            return Err(format!("expected {count} profile cards, the list shows {shown}").into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// What the test does in the window.
async fn drive(sb: &mut BaseCase) -> Result<(), Box<dyn Error>> {
    assert_eq!(sb.get_title().await?, TITLE);
    sb.assert_text("h1", "Profile Manager").await?;

    // A new vault starts with sample profiles; the list fills in after load.
    sb.wait_for_element_visible(CARD, 30).await?;
    let before = sb.find_elements(CARD).await?.len();
    assert!(before > 0, "the profile list is empty");

    // A new WebDriver profile, through the dialog.
    sb.click("#open-create").await?;
    sb.type_text("#profile-name", "E2E Profile").await?;
    sb.type_text("#container-url", "http://localhost:4444")
        .await?;
    sb.click("#save-profile").await?;
    sb.wait_for_text("#status", "Created E2E Profile", 10)
        .await?;
    sb.wait_for_text("#profile-list", "E2E Profile", 10).await?;
    wait_for_cards(sb, before + 1).await?;

    // Typing in the search box leaves only the new profile.
    sb.type_text("#profile-search", "E2E").await?;
    wait_for_cards(sb, 1).await?;
    sb.assert_text(CARD, "E2E Profile").await?;

    // A new folder; the window sends it to the app's REST API.
    sb.type_text("#new-folder", "E2E Folder").await?;
    sb.click("#folder-form button[type='submit']").await?;
    sb.wait_for_text("#folder-list", "E2E Folder", 10).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "starts the app and opens its window: cargo test --features e2e --test e2e_automation -- --ignored"]
async fn the_profile_window_is_driven_over_webdriver() -> Result<(), Box<dyn Error>> {
    assert!(
        !endpoint_is_ready(),
        "something already serves port {PORT}; stop it first"
    );
    let home = tempfile::tempdir()?;
    let mut app = App::start(home.path())?;
    // The first start derives the vault key, which takes a while in a debug build.
    app.wait_until_served(Duration::from_secs(120))?;

    let config = BrowserConfig::default().with_webdriver_url(WEBDRIVER_URL);
    let mut sb = BaseCase::new(config).await?;
    let outcome = drive(&mut sb).await;
    sb.quit().await?;
    drop(app);
    outcome
}
