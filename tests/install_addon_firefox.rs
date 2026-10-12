//! A real-browser check of `BaseCase::install_addon` and `uninstall_addon`.
//!
//! Chrome cannot answer this: only geckodriver has the `moz/addon` commands. The
//! unit tests beside the code assert the request each method sends to a mock
//! WebDriver; this one sends it to a real geckodriver and Firefox.
//!
//! The test is ignored by default. It needs a geckodriver that can read a folder
//! the test writes, so the easy way is the container in
//! `docker/firefox-check`, which holds Firefox and geckodriver and nothing of
//! this crate, with a host folder mounted at the same path inside it:
//!
//! ```bash
//! SHARE="$(mktemp -d)"
//! docker build --no-slim -t sbrs-firefox-check docker/firefox-check
//! docker run -d --rm --name sbrs-gecko --shm-size 512m -p 127.0.0.1:4444:4444 \
//!     -v "$SHARE:$SHARE:ro" sbrs-firefox-check \
//!     geckodriver --host 0.0.0.0 --port 4444 --allow-hosts 127.0.0.1 localhost
//! SB_WEBDRIVER_URL=http://127.0.0.1:4444 SB_ADDON_SHARE="$SHARE" \
//!     cargo test --test install_addon_firefox -- --ignored
//! docker stop sbrs-gecko && docker rmi sbrs-firefox-check
//! ```
//!
//! `SB_WEBDRIVER_URL` is where geckodriver listens and `SB_ADDON_SHARE` a folder
//! that geckodriver sees at the same path.

use std::path::Path;

use seleniumbase_rs::{BaseCase, Browser, BrowserConfig, SeleniumBaseError};

/// The id the add-on below gives itself, and so the id geckodriver answers with.
const ADDON_ID: &str = "sbrs-rust-check@example.org";

const MANIFEST: &str = r#"{
  "manifest_version": 2,
  "name": "sbrs install_addon test",
  "version": "1.0",
  "browser_specific_settings": { "gecko": { "id": "sbrs-rust-check@example.org" } }
}
"#;

fn firefox_config() -> BrowserConfig {
    let url = std::env::var("SB_WEBDRIVER_URL")
        .expect("SB_WEBDRIVER_URL names a geckodriver; see the module documentation");
    let mut config = BrowserConfig::default()
        .with_browser(Browser::Firefox)
        .with_webdriver_url(url);
    config.headless = true;
    config.auto_start_driver = false;
    config
}

/// An unpacked add-on in a folder both this test and geckodriver can read.
fn write_addon() -> String {
    let share = std::env::var("SB_ADDON_SHARE")
        .expect("SB_ADDON_SHARE names a folder geckodriver sees at the same path");
    let dir = Path::new(&share).join("sbrs-addon");
    std::fs::create_dir_all(&dir).expect("the add-on folder is created");
    std::fs::write(dir.join("manifest.json"), MANIFEST).expect("the manifest is written");
    dir.to_str().expect("a UTF-8 path").to_owned()
}

#[tokio::test]
#[ignore = "needs a real geckodriver and Firefox: see the module documentation"]
async fn a_real_geckodriver_installs_and_removes_an_add_on() {
    let folder = write_addon();
    let mut sb = BaseCase::new(firefox_config())
        .await
        .expect("geckodriver starts a Firefox session");

    // Refused here, before anything reaches geckodriver.
    let missing = sb.install_addon("/no/such/addon.xpi", true).await;
    assert!(
        matches!(missing, Err(SeleniumBaseError::InvalidConfig(_))),
        "{missing:?}"
    );

    let id = sb
        .install_addon(&folder, true)
        .await
        .expect("geckodriver installs the add-on");
    assert_eq!(id, ADDON_ID);

    sb.uninstall_addon(&id)
        .await
        .expect("geckodriver removes the add-on");
    // It is gone, so geckodriver refuses a second removal.
    let again = sb.uninstall_addon(&id).await;
    assert!(
        matches!(again, Err(SeleniumBaseError::WebDriver(_))),
        "{again:?}"
    );

    sb.quit().await.expect("the session ends");
}
