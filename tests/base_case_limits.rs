//! The test time limit and the default wait timeout of a `BaseCase`, which the
//! parity manifest maps `--time-limit`, `--timeout-multiplier` and
//! `set_default_timeout` to. None of this needs a browser.

use std::time::Duration;

use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

fn case() -> BaseCase {
    BaseCase::without_session(BrowserConfig::default())
}

fn is_time_limit_error(error: &SeleniumBaseError, seconds: &str) -> bool {
    matches!(error, SeleniumBaseError::TestLifecycle(message)
        if message.contains(&format!("exceeded the time limit of {seconds} second")))
}

#[tokio::test]
async fn a_test_that_has_run_past_its_time_limit_fails_when_it_opens_a_page() {
    let mut sb = case();
    sb.set_time_limit(1).unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;

    let error = sb.open("example.com").await.unwrap_err();

    assert!(
        is_time_limit_error(&error, "1"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn a_test_that_has_run_past_its_time_limit_fails_when_it_acts_on_an_element() {
    let mut sb = case();
    sb.set_time_limit(1).unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;

    // Both go through the readiness wait that every element action starts with.
    let click = sb.click("#go").await.unwrap_err();
    let type_text = sb.type_text("#name", "x").await.unwrap_err();

    assert!(
        is_time_limit_error(&click, "1"),
        "unexpected error: {click}"
    );
    assert!(
        is_time_limit_error(&type_text, "1"),
        "unexpected error: {type_text}"
    );
}

#[tokio::test]
async fn a_test_that_has_run_past_its_time_limit_fails_the_page_ready_wait() {
    let mut sb = case();
    sb.set_time_limit(1).unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;

    let error = sb.wait_for_ready_state_complete().await.unwrap_err();

    assert!(
        is_time_limit_error(&error, "1"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn a_test_inside_its_time_limit_is_left_alone() {
    let mut sb = case();
    sb.set_time_limit(60).unwrap();

    // An empty address is refused for being empty, not for the time.
    let error = sb.open("").await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn a_limit_of_zero_removes_the_limit() {
    let mut sb = case();
    sb.set_time_limit(1).unwrap();
    sb.set_time_limit(0).unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;

    let error = sb.open("").await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn the_time_limit_does_not_touch_the_default_wait() {
    let mut sb = case();

    sb.set_time_limit(5).unwrap();

    assert_eq!(sb.timeout_secs(), 10);
}

#[test]
fn set_default_timeout_changes_the_wait_both_ways_and_reset_puts_the_original_back() {
    let mut sb = case();
    assert_eq!(sb.timeout_secs(), 10);

    sb.set_default_timeout(25);
    assert_eq!(sb.timeout_secs(), 25, "longer than the default");
    sb.set_default_timeout(3);
    assert_eq!(sb.timeout_secs(), 3, "shorter than the default");

    sb.reset_default_timeout();
    assert_eq!(
        sb.timeout_secs(),
        10,
        "the value from before the first change"
    );
}

#[test]
fn set_default_timeout_keeps_to_the_range_python_allows() {
    let mut sb = case();

    sb.set_default_timeout(500);
    assert_eq!(sb.timeout_secs(), 60);
    sb.set_default_timeout(0);
    assert_eq!(sb.timeout_secs(), 1);
}

#[tokio::test]
async fn resetting_a_default_that_was_never_changed_leaves_the_wait_alone() {
    let mut sb = case();
    sb.set_timeout(7).await.unwrap();

    sb.reset_default_timeout();

    assert_eq!(sb.timeout_secs(), 7);
}
