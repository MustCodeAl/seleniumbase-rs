//! A browser plugin (ADR 004) on the WebDriver engine, against a real Chrome
//! driven through chromedriver.
//!
//! The plugin sets a preference before the browser starts, reads the browser's
//! version through the DevTools channel, and gives each window a time zone, a
//! package and an init script: the first one, and one `open_new_window`
//! opens. The test needs Chrome and chromedriver, so it is ignored by default.
//! Run it with:
//!
//! ```text
//! cargo test --features plugins --test webdriver_plugins_chrome -- --ignored
//! ```

#![cfg(feature = "plugins")]

use std::sync::{Arc, Mutex};

use seleniumbase_rs::plugin_api::{
    async_trait, BrowserHandle, LaunchSettings, Package, PageHandle, Plugin, PluginSet, Result,
};
use seleniumbase_rs::{BaseCase, BrowserConfig};
use serde_json::json;

type Log = Arc<Mutex<Vec<String>>>;

/// Germany, through every hook.
struct German {
    log: Log,
}

impl German {
    fn note(&self, entry: impl Into<String>) {
        self.log.lock().unwrap().push(entry.into());
    }
}

#[async_trait]
impl Plugin for German {
    fn name(&self) -> &str {
        "german"
    }

    fn packages(&self) -> Vec<Package> {
        vec![Package::script("window.packaged = 'yes';").everywhere()]
    }

    async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()> {
        launch
            .prefs
            .insert("intl.accept_languages".to_owned(), json!("de-DE,de"));
        Ok(())
    }

    async fn on_browser(&self, browser: &dyn BrowserHandle) -> Result<()> {
        let version = browser.send("Browser.getVersion", json!({})).await?;
        self.note(version["product"].as_str().unwrap_or_default());
        Ok(())
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        page.send(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "Europe/Berlin" }),
        )
        .await?;
        page.add_init_script("window.planet = 'Erde';").await?;
        let sum = page.evaluate("1 + 1").await?;
        self.note(format!("evaluate {sum}"));
        Ok(())
    }

    async fn on_close(&self) -> Result<()> {
        self.note("on_close");
        Ok(())
    }
}

const PAGE: &str =
    "data:text/html,<html><head><title>Plugins</title></head><body>Hallo</body></html>";

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_plugin_reaches_chrome_through_webdriver_and_its_devtools_channel() {
    let log = Log::default();
    let plugins = PluginSet::new()
        .with(German {
            log: Arc::clone(&log),
        })
        .unwrap();
    let mut config = BrowserConfig::default()
        .with_headless(true)
        .with_plugins(plugins);
    config.auto_start_driver = true;

    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.open(PAGE).await.unwrap();

    let script = |expression: &str| format!("return {expression};");
    assert_eq!(
        sb.execute_script(&script("window.planet")).await.unwrap(),
        "Erde",
        "the init script ran in the document opened after it was added"
    );
    assert_eq!(
        sb.execute_script(&script("window.packaged")).await.unwrap(),
        "yes"
    );
    assert_eq!(
        sb.execute_script(&script("Intl.DateTimeFormat().resolvedOptions().timeZone"))
            .await
            .unwrap(),
        "Europe/Berlin"
    );
    assert_eq!(
        sb.execute_script(&script("navigator.languages[0]"))
            .await
            .unwrap(),
        "de-DE",
        "the preference before_launch set reached Chrome"
    );

    // A window BaseCase opens gets the page hooks on its own target.
    sb.open_new_window().await.unwrap();
    sb.open(PAGE).await.unwrap();
    assert_eq!(
        sb.execute_script(&script("window.planet")).await.unwrap(),
        "Erde",
        "the init script reached the new window"
    );
    assert_eq!(
        sb.execute_script(&script("Intl.DateTimeFormat().resolvedOptions().timeZone"))
            .await
            .unwrap(),
        "Europe/Berlin"
    );
    sb.quit().await.unwrap();

    let seen = log.lock().unwrap().clone();
    assert!(
        seen[0].starts_with("HeadlessChrome/") || seen[0].starts_with("Chrome/"),
        "{seen:?}"
    );
    assert_eq!(seen[1..3], ["evaluate 2", "evaluate 2"], "two windows");
    assert_eq!(seen.last().map(String::as_str), Some("on_close"));
}
