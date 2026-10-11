//! A real-browser check of `BaseCase::check_window`.
//!
//! Python SeleniumBase saves the structure of the page as the baseline and
//! compares that on later runs: the tags (level 1), with the names of their
//! attributes (level 2), with the attribute values (level 3). The screenshot
//! is kept for a person to look at and is not compared.
//!
//! This test is ignored by default. Run it locally with Chrome installed:
//!
//! ```bash
//! cargo test --test check_window_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

use std::path::PathBuf;

use seleniumbase_rs::api::run_settings::RunSettings;
use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The name of the test below: `check_window` keeps its baselines in a folder
/// named after the running test.
const TEST: &str = "check_window_compares_the_structure_of_the_page_and_not_its_pixels";

const PAGE: &str = "data:text/html,<html><head><title>Home</title></head><body>\
    <div id=\"box\" class=\"a b\"><p id=\"t\">Hello</p></div></body></html>";

fn test_config() -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

fn baseline_dir() -> PathBuf {
    PathBuf::from("visual_baseline").join(TEST)
}

/// Runs `script` in the page.
async fn run(sb: &BaseCase, script: &str) {
    sb.execute_script(script).await.expect("the script runs");
}

/// Whether `check_window(name, level)` fails with a difference.
async fn differs(sb: &mut BaseCase, level: f64) -> bool {
    match sb.check_window("home", level).await {
        Ok(()) => false,
        Err(SeleniumBaseError::AssertionFailed(_)) => true,
        Err(other) => panic!("level {level} failed for another reason: {other:?}"),
    }
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn check_window_compares_the_structure_of_the_page_and_not_its_pixels() {
    let _guard = BROWSER_LOCK.lock().await;
    let _ = std::fs::remove_dir_all(baseline_dir());
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(PAGE).await.expect("open the page");

    // The first check saves the baseline, at any level.
    sb.check_window("home", 3.0)
        .await
        .expect("the first check saves the baseline");
    let saved = baseline_dir().join("home");
    for file in ["baseline.png", "page_url.txt", "tags_level_1.txt"] {
        assert!(saved.join(file).is_file(), "{file} is saved");
    }
    let read = |file: &str| std::fs::read_to_string(saved.join(file)).expect(file);
    assert_eq!(read("tags_level_1.txt"), r#"[["div"],["p"]]"#);
    assert_eq!(
        read("tags_level_2.txt"),
        r#"[["div",["class","id"]],["p",["id"]]]"#
    );
    assert_eq!(
        read("tags_level_3.txt"),
        r#"[["div",[["class",["a","b"]],["id","box"]]],["p",[["id","t"]]]]"#
    );
    assert!(read("page_url.txt").starts_with("data:text/html"));

    // The same page matches, and so does one that only looks different.
    assert!(!differs(&mut sb, 3.0).await);
    run(
        &sb,
        "document.getElementById('t').textContent = 'Changed'; \
         document.body.style.background = 'red'; return 1;",
    )
    .await;
    assert!(
        !differs(&mut sb, 3.0).await,
        "text and colour are no difference"
    );
    assert!(
        saved.join("latest.png").is_file(),
        "the new picture is kept"
    );

    // Each level sees what it compares and the ones below it do not.
    run(
        &sb,
        "document.getElementById('box').id = 'crate'; return 1;",
    )
    .await;
    assert!(!differs(&mut sb, 1.0).await);
    assert!(!differs(&mut sb, 2.0).await);
    assert!(differs(&mut sb, 3.0).await, "a new attribute value");

    run(
        &sb,
        "document.getElementById('t').setAttribute('data-x', '1'); return 1;",
    )
    .await;
    assert!(!differs(&mut sb, 1.0).await);
    assert!(differs(&mut sb, 2.0).await, "a new attribute name");

    run(
        &sb,
        "document.getElementById('crate').appendChild(document.createElement('span')); return 1;",
    )
    .await;
    assert!(differs(&mut sb, 1.0).await, "a new tag");

    // Level 0 is a dry run, and a level that is not 0 to 3 is refused.
    assert!(!differs(&mut sb, 0.0).await, "a dry run does not fail");
    assert!(matches!(
        sb.check_window("home", 4.0).await,
        Err(SeleniumBaseError::InvalidConfig(_))
    ));

    // A reset makes the page as it is now the baseline.
    sb.set_run_settings(RunSettings::default().with_visual_baseline_reset(true));
    sb.check_window("home", 3.0).await.expect("reset");
    sb.set_run_settings(RunSettings::default());
    assert!(
        !differs(&mut sb, 3.0).await,
        "the reset page is the baseline"
    );

    sb.quit().await.expect("quit");
    let _ = std::fs::remove_dir_all(baseline_dir());
}
