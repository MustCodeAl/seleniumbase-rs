//! A real-browser check that an Edge session starts msedgedriver by itself.
//!
//! The session downloads the msedgedriver for the installed Edge (unless one
//! that fits is in `downloaded_drivers/` or on the `PATH` already), starts it
//! with the orphan guard, opens a `data:` page in a headless Edge, reads its
//! title and quits; afterwards neither the driver nor that Edge may be left
//! running.
//!
//! Ignored by default: it needs Microsoft Edge, network access the first time,
//! and a Unix `pgrep`. Run it alone:
//!
//! ```bash
//! cargo test --test edge_msedgedriver -- --ignored
//! ```

use std::process::Command;
use std::time::{Duration, Instant};

use seleniumbase_rs::browser::session::BrowserSession;
use seleniumbase_rs::{Browser, BrowserConfig};

/// The ids of the running processes whose command line holds `pattern`.
fn processes_matching(pattern: &str) -> Vec<String> {
    let output = Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .expect("pgrep runs");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Waits up to ten seconds for no process to match `pattern`, and returns what
/// still does.
fn gone(pattern: &str) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = processes_matching(pattern);
        if left.is_empty() || Instant::now() >= deadline {
            return left;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Microsoft Edge and, the first time, network access"]
async fn an_edge_session_runs_on_an_msedgedriver_the_crate_starts() {
    let profile = tempfile::tempdir().expect("a profile folder");
    let marker = profile.path().to_str().expect("a UTF-8 path").to_owned();
    let drivers_before = processes_matching("msedgedriver --port=");

    let config = BrowserConfig::default()
        .with_browser(Browser::Edge)
        .with_headless(true)
        .with_user_data_dir(marker.clone());
    let mut session = BrowserSession::connect(config)
        .await
        .expect("an Edge session through a crate-started msedgedriver");

    let drivers_during = processes_matching("msedgedriver --port=");
    assert!(
        drivers_during.len() > drivers_before.len(),
        "no msedgedriver was started: before {drivers_before:?}, during {drivers_during:?}"
    );
    assert!(
        !processes_matching(&marker).is_empty(),
        "no Edge runs with the profile {marker}"
    );

    session
        .goto("data:text/html,<title>edge%20driver%20ok</title><p>hi</p>")
        .await
        .expect("the page opens");
    let title = session.driver().title().await.expect("the title is read");
    assert_eq!(title, "edge driver ok");

    session.quit().await.expect("the session quits");

    let edge_left = gone(&marker);
    assert!(edge_left.is_empty(), "Edge is still running: {edge_left:?}");
    let drivers_left: Vec<String> = gone("msedgedriver --port=")
        .into_iter()
        .filter(|pid| !drivers_before.contains(pid))
        .collect();
    assert!(
        drivers_left.is_empty(),
        "msedgedriver is still running: {drivers_left:?}"
    );
}
