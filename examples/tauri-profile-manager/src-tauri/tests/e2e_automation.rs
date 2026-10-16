//! Drives the app's real window with `BaseCase`, over the WebDriver endpoint
//! that `tauri-plugin-sbrs-automation` serves inside the app (feature `e2e`).
//!
//! The test starts the app Cargo built for it, waits for the endpoint, then
//! reads the title, finds the profile list, creates a profile through the
//! dialog, filters the list by typing in the search box, adds a folder, and
//! reads and resizes the window. Then it adds a few controls of its own to the
//! page (the app has no dialogs, frames or double clicks) to answer confirm
//! and prompt dialogs, enter a same-origin frame (and be refused a sandboxed
//! one), double-click, hover and press keys through W3C actions, and take
//! screenshots on macOS (elsewhere the plugin answers `unsupported operation`
//! for them, which the test checks). It stops the app when it ends, also when
//! it fails.
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
/// Controls the test adds to the page, fixed at its top left corner: a
/// button that counts clicks, one for a confirm and one for a prompt, a hover
/// target, a form, a same-origin frame and a sandboxed (cross-origin) one.
const CONTROLS: &str = r#"
    const area = document.createElement('div');
    area.id = 'e2e';
    area.style.cssText = 'position:fixed;top:0;left:0;z-index:2147483647;background:#fff;padding:8px';
    area.innerHTML = `
      <button id="e2e-twice" type="button">Twice</button> <span id="e2e-clicks"></span>
      <button id="e2e-confirm" type="button">Confirm</button>
      <button id="e2e-prompt" type="button">Prompt</button> <span id="e2e-answer"></span>
      <span id="e2e-hover">Hover me</span>
      <form id="e2e-form"><input id="e2e-field"></form> <span id="e2e-submitted"></span>
      <iframe id="e2e-frame" srcdoc="<p id=inside>Inside the frame</p><button id=frame-alert>Alert</button>"></iframe>
      <iframe id="e2e-sealed" sandbox srcdoc="<p>Sealed</p>"></iframe>`;
    document.body.append(area);
    const show = (id, text) => { document.getElementById(id).textContent = text; };
    const clicks = [];
    const twice = document.getElementById('e2e-twice');
    twice.addEventListener('click', (event) => { clicks.push(`click${event.detail}`); show('e2e-clicks', clicks.join(' ')); });
    twice.addEventListener('dblclick', () => { clicks.push('dblclick'); show('e2e-clicks', clicks.join(' ')); });
    document.getElementById('e2e-confirm').addEventListener('click', () => {
      show('e2e-answer', `confirm ${window.confirm('Delete the E2E profile?')}`);
    });
    document.getElementById('e2e-prompt').addEventListener('click', () => {
      show('e2e-answer', `prompt ${window.prompt('Your name?', 'nobody')}`);
    });
    document.getElementById('e2e-hover').addEventListener('mouseover', () => show('e2e-hover', 'Hovered'));
    document.getElementById('e2e-form').addEventListener('submit', (event) => {
      event.preventDefault();
      show('e2e-submitted', `submitted [${document.getElementById('e2e-field').value}]`);
    });
"#;

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

    // The window's own rectangle (the page alone would say 0 x 0): at least
    // the 800 x 600 of tauri.conf.json, and it can be resized.
    let (_, _, width, height) = sb.get_window_rect().await?;
    assert!(
        width >= 800 && height >= 600,
        "the window is {width} x {height}"
    );
    sb.set_window_size(900, 700).await?;
    let (_, _, width, height) = sb.get_window_rect().await?;
    assert_eq!((width, height), (900, 700));

    sb.execute_script(CONTROLS).await?;
    sb.wait_for_element_visible("#e2e-twice", 10).await?;
    dialogs(sb).await?;
    frames(sb).await?;
    actions(sb).await?;
    screenshots(sb).await
}

/// Dialogs answer at once (they cannot wait for the client): with the answer
/// set before, else confirm and prompt are dismissed. The dialog's text stays
/// readable until it is accepted or dismissed.
async fn dialogs(sb: &mut BaseCase) -> Result<(), Box<dyn Error>> {
    // No answer set: the confirm is dismissed, and its text can be read.
    sb.click("#e2e-confirm").await?;
    sb.wait_for_text("#e2e-answer", "confirm false", 10).await?;
    assert_eq!(sb.get_alert_text().await?, "Delete the E2E profile?");
    sb.accept_alert().await?;
    assert!(
        sb.get_alert_text().await.is_err(),
        "the dialog was accepted, yet its text is still there"
    );

    // Accepted before it opens: the confirm answers true.
    sb.accept_alert().await?;
    sb.click("#e2e-confirm").await?;
    sb.wait_for_text("#e2e-answer", "confirm true", 10).await?;
    sb.dismiss_alert().await?;

    // A prompt's text, sent and accepted before it opens.
    sb.type_alert_text("Ada").await?;
    sb.accept_alert().await?;
    sb.click("#e2e-prompt").await?;
    sb.wait_for_text("#e2e-answer", "prompt Ada", 10).await?;
    assert_eq!(sb.get_alert_text().await?, "Your name?");
    sb.accept_alert().await?;

    // And with nothing set, the prompt is dismissed.
    sb.click("#e2e-prompt").await?;
    sb.wait_for_text("#e2e-answer", "prompt null", 10).await?;
    sb.dismiss_alert().await?;
    Ok(())
}

/// Finding, scripts, clicks and dialogs inside a same-origin frame; a
/// sandboxed frame has an origin of its own and cannot be entered.
async fn frames(sb: &mut BaseCase) -> Result<(), Box<dyn Error>> {
    sb.switch_to_frame("#e2e-frame").await?;
    sb.assert_text("#inside", "Inside the frame").await?;
    // The script runs in the frame: its `document` is the frame's.
    let found = sb
        .execute_script(
            "document.getElementById('frame-alert').addEventListener('click', () => alert('From the frame'));
             return document.getElementById('inside').textContent;",
        )
        .await?;
    assert_eq!(found, "Inside the frame");
    sb.click("#frame-alert").await?;
    assert_eq!(sb.get_alert_text().await?, "From the frame");
    sb.accept_alert().await?;
    sb.switch_to_default_content().await?;
    assert!(
        !sb.is_element_present("#inside").await?,
        "the top page sees the frame's element"
    );
    sb.assert_element("#e2e-twice").await?;

    let sealed = sb.switch_to_frame("#e2e-sealed").await;
    let refusal = sealed
        .expect_err("a sandboxed frame was entered")
        .to_string();
    assert!(
        refusal.contains("cross-origin"),
        "unexpected refusal: {refusal}"
    );
    eprintln!("a sandboxed frame is refused: {refusal}");
    sb.assert_element("#e2e-twice").await?;
    Ok(())
}

/// W3C actions as DOM events: a double click, a hover, and keys typed into
/// the focused field, Enter submitting its form.
async fn actions(sb: &mut BaseCase) -> Result<(), Box<dyn Error>> {
    sb.double_click("#e2e-twice").await?;
    assert_eq!(sb.get_text("#e2e-clicks").await?, "click1 click2 dblclick");

    sb.hover("#e2e-hover").await?;
    sb.wait_for_text("#e2e-hover", "Hovered", 10).await?;

    sb.type_text("#e2e-field", "E2E").await?;
    sb.press_key("space").await?;
    sb.press_key("enter").await?;
    sb.wait_for_text("#e2e-submitted", "submitted [E2E ]", 10)
        .await?;
    Ok(())
}

/// The PNG's width and height, from its header.
#[cfg(target_os = "macos")]
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    let signature = b"\x89PNG\r\n\x1a\n";
    if png.len() < 24 || &png[..8] != signature || &png[12..16] != b"IHDR" {
        return None;
    }
    let number = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
    Some((number(16), number(20)))
}

/// Off macOS the plugin takes no screenshots yet: it says so.
#[cfg(not(target_os = "macos"))]
async fn screenshots(sb: &mut BaseCase) -> Result<(), Box<dyn Error>> {
    let refusal = sb
        .screenshot_as_png()
        .await
        .expect_err("a screenshot was taken off macOS")
        .to_string();
    assert!(
        refusal.contains("macOS only"),
        "unexpected refusal: {refusal}"
    );
    eprintln!("screenshots are refused off macOS: {refusal}");
    Ok(())
}

/// A screenshot of the whole view, and one of an element cut out of it.
#[cfg(target_os = "macos")]
async fn screenshots(sb: &mut BaseCase) -> Result<(), Box<dyn Error>> {
    let page = sb.screenshot_as_png().await?;
    let (page_width, page_height) = png_size(&page).ok_or("the page screenshot is not a PNG")?;
    assert!(
        page_width >= 900 && page_height >= 600,
        "the page screenshot is {page_width} x {page_height}"
    );
    let button = sb.find_element("#e2e-twice").await?;
    let element = button.screenshot_as_png().await?;
    let (width, height) = png_size(&element).ok_or("the element screenshot is not a PNG")?;
    assert!(
        width > 0 && height > 0 && width < page_width && height < page_height,
        "the element screenshot is {width} x {height}"
    );
    eprintln!("screenshots: the page {page_width} x {page_height}, the button {width} x {height}");
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
