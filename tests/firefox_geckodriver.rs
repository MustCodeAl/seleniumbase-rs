//! Real-driver checks of Firefox sessions: the crate starts the geckodriver it
//! finds, and a headless Firefox takes the `moz:firefoxOptions` it is sent.
//!
//! Both are ignored by default; run them one at a time.
//!
//! `the_crate_starts_the_geckodriver_it_finds` needs only a geckodriver (in
//! `downloaded_drivers/` or on the `PATH`; it downloads one otherwise), not
//! Firefox: it starts the driver through the launcher, asks its `/status`, and
//! checks the process is gone after `kill`.
//!
//! `a_headless_firefox_session_takes_its_options` needs a geckodriver with a
//! Firefox, such as the container in `docker/firefox-check`:
//!
//! ```bash
//! docker build --no-slim -t sbrs-firefox-check docker/firefox-check
//! docker run -d --rm --name sbrs-gecko --shm-size 512m -p 127.0.0.1:4444:4444 \
//!     sbrs-firefox-check geckodriver --host 0.0.0.0 --port 4444 \
//!     --allow-hosts 127.0.0.1 localhost
//! SB_WEBDRIVER_URL=http://127.0.0.1:4444 \
//!     cargo test --test firefox_geckodriver -- --ignored a_headless_firefox
//! docker stop sbrs-gecko && docker rmi sbrs-firefox-check
//! ```
//!
//! Publish the port geckodriver listens on as the same port on the host:
//! geckodriver refuses a request whose `Host` header names another port.

use std::process::Command;

use seleniumbase_rs::browser::launcher::launch_driver_for;
use seleniumbase_rs::browser::session::BrowserSession;
use seleniumbase_rs::{Browser, BrowserConfig};

/// Whether a process whose command line holds `pattern` runs.
fn running(pattern: &str) -> bool {
    Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .is_ok_and(|output| !output.stdout.is_empty())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a geckodriver, or network access to download one"]
async fn the_crate_starts_the_geckodriver_it_finds() {
    let config = BrowserConfig::default().with_browser(Browser::Firefox);
    let mut driver = launch_driver_for(&config, None)
        .await
        .expect("geckodriver is found and started");
    seleniumbase_rs::install_tls_provider();
    let status: serde_json::Value = reqwest::get(format!("{}/status", driver.url))
        .await
        .expect("geckodriver answers")
        .json()
        .await
        .expect("a JSON status");
    assert!(status["value"].get("ready").is_some(), "{status}");

    let port = driver.url.rsplit(':').next().expect("a port").to_owned();
    let pattern = format!("geckodriver --port {port}");
    // Windows has no `pgrep`; there the /status answer above is the check.
    if cfg!(unix) {
        assert!(running(&pattern), "no {pattern}");
    }
    driver.kill();
    if cfg!(unix) {
        assert!(!running(&pattern), "{pattern} is still running");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs SB_WEBDRIVER_URL to name a geckodriver with Firefox; see the module docs"]
async fn a_headless_firefox_session_takes_its_options() {
    let Ok(url) = std::env::var("SB_WEBDRIVER_URL") else {
        eprintln!(
            "skipped: set SB_WEBDRIVER_URL to a geckodriver with Firefox (see the module docs)"
        );
        return;
    };
    let mut config = BrowserConfig::default()
        .with_browser(Browser::Firefox)
        .with_webdriver_url(url)
        .with_headless(true)
        .with_user_agent("sbrs-firefox-check/1.0")
        .with_locale("fr");
    config.auto_start_driver = false;
    let mut session = BrowserSession::connect(config)
        .await
        .expect("a Firefox session");

    session
        .goto("data:text/html,<title>firefox%20driver%20ok</title><p>hi</p>")
        .await
        .expect("the page opens");
    let title = session.driver().title().await.expect("the title is read");
    let seen = session
        .driver()
        .execute(
            "return [navigator.userAgent, navigator.language, navigator.webdriver]",
            Vec::new(),
        )
        .await
        .expect("the script runs")
        .json()
        .clone();
    session.quit().await.expect("the session quits");

    assert_eq!(title, "firefox driver ok");
    assert_eq!(seen[0], "sbrs-firefox-check/1.0", "{seen}");
    assert_eq!(seen[1], "fr", "{seen}");
    assert_eq!(seen[2], true, "{seen}");
}
