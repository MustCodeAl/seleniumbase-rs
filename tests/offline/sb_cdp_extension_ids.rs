//! The ids Chrome gives the extensions a launch loads, against a scripted
//! browser with no Chrome.
//!
//! A launch loads each unpacked extension folder over the protocol
//! (`Extensions.loadUnpacked`), and the answer names the extension. The
//! browser keeps those ids so the code that started it, and later the plugin
//! API, can tell which extension is which.

#![cfg(feature = "test-util")]

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, MockCtrl};
use serde_json::json;

#[tokio::test]
async fn the_ids_chrome_gives_the_loaded_extensions_are_kept_in_load_order() {
    let mock = MockCtrl::new();
    mock.on("Extensions.loadUnpacked", |params| {
        let path = params["path"].as_str().unwrap_or_default();
        let folder = path.rsplit('/').next().unwrap_or_default();
        Ok(json!({ "id": format!("id-of-{folder}") }))
    });
    let options = LaunchOptions::builder()
        .headless(true)
        .extension("/opt/ext/a")
        .extension("/opt/ext/b")
        .build()
        .unwrap();

    let browser = Browser::launch_mocked(options, &mock).await.unwrap();

    assert_eq!(browser.extension_ids(), ["id-of-a", "id-of-b"]);
    assert_eq!(mock.calls_to("Extensions.loadUnpacked").len(), 2);
}

#[tokio::test]
async fn a_clone_of_the_browser_sees_the_same_ids() {
    let mock = MockCtrl::new();
    mock.reply("Extensions.loadUnpacked", json!({ "id": "only-one" }));
    let options = LaunchOptions::builder()
        .headless(true)
        .extension("/opt/ext/a")
        .build()
        .unwrap();

    let browser = Browser::launch_mocked(options, &mock).await.unwrap();

    assert_eq!(browser.clone().extension_ids(), ["only-one"]);
}

#[tokio::test]
async fn a_launch_without_extensions_has_no_ids() {
    let (browser, _mock) = Browser::new_mocked();
    assert!(browser.extension_ids().is_empty());

    let mock = MockCtrl::new();
    let browser = Browser::launch_mocked(LaunchOptions::default(), &mock)
        .await
        .unwrap();
    assert!(browser.extension_ids().is_empty());
}

#[tokio::test]
async fn an_answer_without_an_id_is_not_an_id_and_does_not_fail_the_launch() {
    // The default answer of the mock is an empty object.
    let mock = MockCtrl::new();
    let options = LaunchOptions::builder()
        .headless(true)
        .extension("/opt/ext/a")
        .build()
        .unwrap();

    let browser = Browser::launch_mocked(options, &mock).await.unwrap();

    assert!(browser.extension_ids().is_empty());
}
