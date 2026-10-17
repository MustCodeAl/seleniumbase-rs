//! Real-browser checks that `BaseCase` methods do what the Python method of the
//! same name does (see `parity/api.toml`).
//!
//! These tests are ignored by default. Run them locally with Chrome installed:
//!
//! ```bash
//! cargo test --test parity_honesty_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

async fn open(page: &str) -> BaseCase {
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(page).await.expect("open the page");
    sb
}

const FORM_PAGE: &str = "data:text/html,<html><head><title>Form</title></head><body>\
    <form onsubmit=\"document.title='submitted';return false;\">\
    <input id=\"q\" type=\"text\"><textarea id=\"t\"></textarea></form>\
    <div id=\"d\">old</div><a href=\"one\">one</a><a href=\"two\">two</a><a href=\"three\">three</a>\
    </body></html>";

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_start_page_is_opened_when_the_test_case_starts() {
    let _guard = BROWSER_LOCK.lock().await;
    let config = test_config().with_start_page(FORM_PAGE);

    let mut sb = BaseCase::new(config).await.expect("connect to browser");

    assert_eq!(sb.get_title().await.expect("title"), "Form");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn press_keys_types_text_into_the_element_and_presses_enter_for_a_trailing_newline() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = open(FORM_PAGE).await;

    sb.press_keys("#q", "rust").await.expect("type into #q");
    assert_eq!(sb.get_value("#q").await.expect("value"), "rust");
    assert_eq!(sb.get_title().await.expect("title"), "Form", "no Enter yet");

    sb.press_keys("#q", "!\n")
        .await
        .expect("type and press Enter");
    assert_eq!(sb.get_value("#q").await.expect("value"), "rust!");
    assert_eq!(sb.get_title().await.expect("title"), "submitted");

    sb.press_key("tab").await.expect("a named key still works");
    assert!(sb.press_key("nonsense").await.is_err());
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn select_all_returns_the_matching_elements_like_find_elements() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = open(FORM_PAGE).await;

    let links = sb.select_all("a").await.expect("select_all");
    assert_eq!(links.len(), 3);
    assert!(sb.select_all("#nope").await.expect("no match").is_empty());
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn set_text_sets_the_value_of_fields_and_the_text_of_other_elements() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = open(FORM_PAGE).await;

    sb.set_text("#q", "ann").await.expect("input");
    sb.set_text("#t", "two\nlines").await.expect("textarea");
    sb.set_text("#d", "new").await.expect("div");
    assert_eq!(sb.get_value("#q").await.expect("value"), "ann");
    assert_eq!(sb.get_value("#t").await.expect("value"), "two\nlines");
    assert_eq!(sb.get_text("#d").await.expect("text"), "new");

    sb.set_text_content("#q", "bob").await.expect("alias");
    assert_eq!(sb.get_value("#q").await.expect("value"), "bob");

    sb.set_timeout(1).await.expect("shorten the wait");
    assert!(
        sb.set_text("#nope", "x").await.is_err(),
        "a missing element is an error"
    );
    sb.quit().await.expect("quit");
}

const PAGE_A: &str = "data:text/html,<html><head><title>A</title></head><body>\
    <p id=\"only-a\">Welcome home</p><p id=\"hidden\" style=\"display:none\">Secret</p>\
    <p id=\"empty\"></p></body></html>";
const PAGE_B: &str = "data:text/html,<html><head><title>B</title></head><body>\
    <p id=\"only-b\">B</p></body></html>";

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn deferred_asserts_look_at_the_page_they_were_made_on() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = open(PAGE_A).await;

    // Right on page A, wrong on page B if they were only queued.
    sb.deferred_assert_element("#only-a").await.expect("record");
    sb.deferred_assert_text("Welcome", "#only-a")
        .await
        .expect("record");
    sb.deferred_assert_element_present("#hidden")
        .await
        .expect("record");
    sb.open(PAGE_B).await.expect("open page B");
    sb.deferred_assert_element("#only-b").await.expect("record");
    sb.process_deferred_asserts()
        .await
        .expect("each check held on the page it was made on");

    // The variants that used to check the wrong thing.
    sb.open(PAGE_A).await.expect("open page A");
    sb.deferred_assert_exact_text("Welcome", "#only-a")
        .await
        .expect("record");
    sb.deferred_assert_non_empty_text("#empty")
        .await
        .expect("record");
    sb.deferred_assert_element("#hidden").await.expect("record");
    let error = sb.process_deferred_asserts().await.expect_err(
        "an exact-text check of part of the text, an empty element and a hidden one fail",
    );
    let message = error.to_string();
    assert!(
        message.contains("3 deferred assertion(s) failed"),
        "{message}"
    );
    assert!(message.contains("(at data:text/html"), "{message}");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn switch_to_frame_of_element_enters_the_frame_that_holds_the_element() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = open(
        "data:text/html,<html><body><p id=\"here\">top</p>\
         <iframe id=\"f\" srcdoc=\"<button id='b'>inside</button>\"></iframe></body></html>",
    )
    .await;

    assert_eq!(
        sb.switch_to_frame_of_element("#here")
            .await
            .expect("in this document"),
        None
    );
    let frame = sb
        .switch_to_frame_of_element("#b")
        .await
        .expect("the button is in the iframe");
    assert_eq!(frame.as_deref(), Some("f"));
    assert_eq!(sb.get_text("#b").await.expect("text"), "inside");
    sb.switch_to_default_content().await.expect("back out");

    sb.set_timeout(1).await.expect("shorten the wait");
    assert!(sb.switch_to_frame_of_element("#nowhere").await.is_err());
    assert_eq!(sb.get_text("#here").await.expect("still at the top"), "top");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn select_option_by_index_fires_change_and_refuses_a_missing_option() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = open(
        "data:text/html,<html><body><select id=\"s\" \
         onchange=\"document.title='changed '+this.selectedIndex\">\
         <option>a</option><option>b</option><option>c</option></select></body></html>",
    )
    .await;

    sb.set_timeout(1).await.expect("shorten the wait");
    assert!(sb.select_option_by_index("#s", 99).await.is_err());
    assert_eq!(
        sb.get_title().await.expect("title"),
        "",
        "no change event for a miss"
    );

    sb.select_option_by_index("#s", 2)
        .await
        .expect("a real option");
    assert_eq!(sb.get_title().await.expect("title"), "changed 2");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn find_text_waits_for_visible_text_and_ignores_hidden_text() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = open(
        "data:text/html,<html><body><div style=\"display:none\">hidden-needle</div>\
         <p>visible</p><script>setTimeout(function(){var b=document.createElement('b');\
         b.id='late';b.textContent='late-needle';document.body.append(b)},600)</script>\
         </body></html>",
    )
    .await;

    sb.set_timeout(1).await.expect("shorten the wait");
    assert!(
        sb.find_text("hidden-needle", "body").await.is_err(),
        "hidden text is not found"
    );

    sb.set_timeout(5).await.expect("lengthen the wait");
    let late = sb
        .find_text("late-needle", "body")
        .await
        .expect("found once it appears");
    assert_eq!(late.tag_name().await.expect("tag"), "body");
    let exact = sb
        .find_exact_text("visible", "p")
        .await
        .expect("exact text");
    assert_eq!(exact.tag_name().await.expect("tag"), "p");
    sb.find_non_empty_text("body").await.expect("some text");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn bring_to_front_and_highlight_change_and_restore_the_element_style() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = open(
        "data:text/html,<html><body><button id=\"b\" style=\"position:relative\">b</button></body></html>",
    )
    .await;

    sb.bring_to_front("#b").await.expect("raise the button");
    let z = sb
        .execute_script("return document.querySelector('#b').style.zIndex;")
        .await
        .expect("z");
    assert_eq!(z, "999999");

    sb.highlight("#b").await.expect("highlight");
    let during = sb
        .execute_script("return document.querySelector('#b').style.outline;")
        .await
        .expect("outline");
    assert!(
        during.as_str().unwrap_or("").contains("magenta"),
        "{during}"
    );
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let after = sb
        .execute_script("return document.querySelector('#b').style.outline;")
        .await
        .expect("outline");
    assert_eq!(after, "", "the outline is restored");
    sb.quit().await.expect("quit");
}
