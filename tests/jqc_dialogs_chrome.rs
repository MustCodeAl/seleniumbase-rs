//! Real-browser checks of the jquery-confirm dialogs: `get_jqc_button_input`,
//! `get_jqc_text_input`, `get_jqc_form_inputs` and `set_jqc_theme`.
//!
//! A dialog waits for a person, so each test plays one: a script that runs in
//! the page, waits for the box to appear and then clicks a button, presses a
//! key or types into the input. The browser is headless, which would make the
//! calls return at once without opening anything, so the case is told that
//! somebody watches (`BaseCase::with_somebody_watching`, in the `test-util`
//! feature).
//!
//! These tests are ignored by default; they need Chrome and a network that can
//! reach cdnjs, from where the page loads jQuery and jquery-confirm. Run them
//! locally with:
//!
//! ```bash
//! cargo test --test jqc_dialogs_chrome --features test-util -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

#![cfg(feature = "test-util")]

use seleniumbase_rs::api::jqc::{JqcColor, JqcTheme};
use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A page with nothing in it but a paragraph.
const PAGE: &str = "data:text/html,<html><head><title>dialogs</title></head><body>\
    <p id=\"here\">hello</p></body></html>";

/// Clicks the second button of a dialog.
const CLICK_SECOND: &str = "document.querySelectorAll('.jconfirm-buttons button')[1].click();";

/// Presses the `n` key, which the `No` button answers to.
const PRESS_N: &str = "var box = jQuery('.jconfirm-box'); box.trigger(jQuery.Event('keydown', { which: 78 })); box.trigger(jQuery.Event('keyup', { which: 78 }));";

/// Types into the input and submits the form, as Enter in the field does.
const TYPE_AND_SUBMIT: &str = "jQuery('.jconfirm-box input.jqc_input').val('typed text'); \
    jQuery('form.jqc_form').trigger('submit');";

/// Types into the input and clicks the second button.
const TYPE_AND_CLICK_SECOND: &str = "jQuery('.jconfirm-box input.jqc_input').val('typed text'); \
    document.querySelectorAll('.jconfirm-buttons button')[1].click();";

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

/// A case in a headless browser that behaves as if somebody watches, on a page.
async fn watched_case() -> BaseCase {
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser")
        .with_somebody_watching();
    // A dialog nobody answers waits for ever; the time limit ends it.
    sb.set_time_limit(90).expect("set the limit");
    sb.open(PAGE).await.expect("open the page");
    sb
}

/// Starts the person: once the dialog box and its buttons are in the page, the
/// script records what the box looks like (`window.__jqcSeen`), waits a moment
/// for jquery-confirm to settle and then runs `action`.
async fn arm_the_person(sb: &BaseCase, action: &str) {
    let script = format!(
        "window.__jqcSeen = null; \
         var timer = setInterval(function () {{ \
             var box = document.querySelector('.jconfirm-box'); \
             if (!box || !document.querySelector('.jconfirm-buttons button')) {{ return; }} \
             clearInterval(timer); \
             window.__jqcSeen = {{ \
                 classes: document.querySelector('.jconfirm').className + ' ' + box.className, \
                 width: box.style.width }}; \
             setTimeout(function () {{ {action} }}, 1000); \
         }}, 20); \
         return true;"
    );
    sb.execute_script(&script)
        .await
        .expect("the person is ready");
}

/// What the person saw of the box.
async fn seen(sb: &BaseCase) -> serde_json::Value {
    sb.execute_script("return window.__jqcSeen;")
        .await
        .expect("read what the person saw")
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn a_clicked_button_is_the_answer() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    arm_the_person(&sb, CLICK_SECOND).await;

    let answer = sb
        .get_jqc_button_input("Pick <b>one</b>", [("First", "green"), ("Second", "red")])
        .await
        .expect("the dialog is answered");

    assert_eq!(answer, "Second");
    // The call left nothing open behind it.
    let open = sb
        .execute_script("return jconfirm.instances.length;")
        .await
        .expect("count the dialogs");
    assert_eq!(open, serde_json::json!(0));
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn a_key_clicks_the_button_it_belongs_to() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    arm_the_person(&sb, PRESS_N).await;

    let answer = sb
        .get_jqc_button_input("Sure?", ["Yes", "No"])
        .await
        .expect("the dialog is answered");

    assert_eq!(answer, "No");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn the_text_of_the_input_is_the_answer_and_the_form_submits() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    arm_the_person(&sb, TYPE_AND_SUBMIT).await;

    let text = sb
        .get_jqc_text_input("Your name?")
        .await
        .expect("the dialog is answered");

    assert_eq!(text, "typed text");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn a_form_answers_with_the_text_and_the_button() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    arm_the_person(&sb, TYPE_AND_CLICK_SECOND).await;

    let (text, button) = sb
        .get_jqc_form_inputs("Why?", [("Send", "green"), ("Cancel", "red")])
        .await
        .expect("the dialog is answered");

    assert_eq!((text.as_str(), button.as_str()), ("typed text", "Cancel"));
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn the_stored_theme_and_a_calls_own_options_style_the_box() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    sb.set_jqc_theme("material", Some("red"), Some("52%"))
        .expect("a valid theme");
    arm_the_person(&sb, CLICK_SECOND).await;

    sb.get_jqc_button_input("Styled", ["One", "Two"])
        .await
        .expect("the dialog is answered");
    let stored = seen(&sb).await;
    let classes = stored["classes"].as_str().expect("the box was seen");
    assert!(classes.contains("jconfirm-material"), "{classes}");
    assert!(classes.contains("jconfirm-type-red"), "{classes}");
    assert_eq!(stored["width"], "52%");

    // The options of one call win for that call, and the next call is back to
    // the stored theme.
    arm_the_person(&sb, CLICK_SECOND).await;
    sb.get_jqc_button_input("Own", ["One", "Two"])
        .theme(JqcTheme::Modern)
        .color(JqcColor::Green)
        .options([("width", "450")])
        .await
        .expect("the dialog is answered");
    let own = seen(&sb).await;
    let classes = own["classes"].as_str().expect("the box was seen");
    assert!(classes.contains("jconfirm-modern"), "{classes}");
    assert!(classes.contains("jconfirm-type-green"), "{classes}");
    assert_eq!(own["width"], "450px");

    sb.reset_jqc_theme();
    arm_the_person(&sb, CLICK_SECOND).await;
    sb.get_jqc_button_input("Reset", ["One", "Two"])
        .await
        .expect("the dialog is answered");
    let reset = seen(&sb).await;
    let classes = reset["classes"].as_str().expect("the box was seen");
    assert!(classes.contains("jconfirm-bootstrap"), "{classes}");
    assert!(classes.contains("jconfirm-type-blue"), "{classes}");
    assert_eq!(reset["width"], "38%");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to cdnjs"]
async fn nobody_answering_ends_with_the_time_limit() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    sb.set_time_limit(1).expect("set the limit");

    let error = sb
        .get_jqc_button_input("Anyone there?", ["OK"])
        .await
        .expect_err("nobody answers, so the time limit ends the wait");

    let SeleniumBaseError::TestLifecycle(message) = &error else {
        panic!("expected a time limit error, got {error:?}");
    };
    assert!(message.contains("time limit"), "{message}");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn in_a_headless_browser_the_dialogs_do_not_open() {
    let _guard = BROWSER_LOCK.lock().await;
    // No `with_somebody_watching`: nobody watches a headless browser.
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(PAGE).await.expect("open the page");

    let button = sb
        .get_jqc_button_input("Sure?", ["First", "Last"])
        .await
        .expect("answered at once");
    assert_eq!(button, "Last");
    assert_eq!(sb.get_jqc_text_input("Name?").await.unwrap(), "");
    assert_eq!(
        sb.get_jqc_form_inputs("Why?", ["A", "B"]).await.unwrap(),
        (String::new(), "B".to_owned())
    );
    // Not even the library was loaded into the page.
    let loaded = sb
        .execute_script("return typeof window.jconfirm;")
        .await
        .expect("the script runs");
    assert_eq!(loaded, serde_json::json!("undefined"));
    sb.quit().await.expect("quit");
}
