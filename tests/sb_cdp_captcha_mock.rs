//! The CAPTCHA solver against a scripted browser, with no Chrome.
//!
//! The clock is paused, so the seconds the solver waits (for a widget to stand
//! still, for a result, between two presses) pass at once while
//! `tokio::time::Instant` still measures them. The page is a script of answers
//! to `Runtime.evaluate`; the pointer events are the real ones the `Page` sends,
//! read back from the mock. What the solver does with a real page is in
//! `sb_cdp_captcha_chrome.rs`. Run with `cargo test --features test-util`.

#![cfg(feature = "test-util")]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::evasion::behavior::Behavior;
use seleniumbase_rs::sb_cdp::{Browser, Captcha, Evidence, MockCtrl, Page, Point, SolveOptions};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};
use tokio::time::Instant;

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// A page with a 300x65 Turnstile at (100, 200). Its response field fills once
/// the widget has been pressed `presses_needed` times; the returned counter
/// is how many presses the browser has received.
async fn page_with_widget(presses_needed: u32, fields: u32) -> (Page, MockCtrl, Arc<AtomicU32>) {
    let (browser, mock) = Browser::new_mocked();
    let presses = Arc::new(AtomicU32::new(0));

    let counted = Arc::clone(&presses);
    mock.on("Input.dispatchMouseEvent", move |params| {
        if params["type"] == "mousePressed" {
            counted.fetch_add(1, Ordering::SeqCst);
        }
        Ok(json!({}))
    });

    let seen = Arc::clone(&presses);
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("pending: pending() }") {
            value(json!({
                "widget": { "kind": 0, "x": 100.0, "y": 200.0, "width": 300.0, "height": 65.0,
                            "inView": true },
                "pending": false,
            }))
        } else if expression.contains("const k = DATA.kinds") {
            value(json!({
                "fields": fields,
                "filled": fields > 0 && seen.load(Ordering::SeqCst) >= presses_needed,
                "success": false, "widget": true, "pending": false, "epoch": "doc",
            }))
        } else if expression.contains("__sbCaptcha = Math.random") {
            value(json!("doc"))
        } else if expression.contains("window.innerWidth") {
            value(json!([1000.0, 800.0]))
        } else {
            value(Value::Null)
        })
    });
    (browser.default_page().await.unwrap(), mock, presses)
}

fn seeded() -> Behavior {
    Behavior::builder().seed(21).build().unwrap()
}

fn options() -> SolveOptions {
    SolveOptions::new().behavior(seeded())
}

#[tokio::test(start_paused = true)]
async fn a_widget_that_needs_two_presses_is_pressed_twice_and_then_reported_cleared() {
    let (page, _mock, presses) = page_with_widget(2, 1).await;

    let solved = page
        .solve_captcha_with(&options())
        .await
        .unwrap()
        .expect("a widget");

    assert_eq!(solved.captcha, Captcha::Turnstile);
    assert_eq!(solved.attempts, 2);
    assert_eq!(solved.evidence, Some(Evidence::Token));
    assert_eq!(presses.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn a_widget_that_never_clears_fails_with_a_typed_error_after_three_presses() {
    let (page, _mock, presses) = page_with_widget(u32::MAX, 1).await;
    let started = Instant::now();

    let error = page.solve_captcha().await.unwrap_err();

    match &error {
        SeleniumBaseError::Captcha {
            captcha,
            attempts,
            reason,
        } => {
            assert_eq!(captcha, "Cloudflare Turnstile");
            assert_eq!(*attempts, 3);
            assert!(reason.contains("response field"), "{reason}");
        }
        other => panic!("expected a CAPTCHA error, got {other:?}"),
    }
    assert_eq!(presses.load(Ordering::SeqCst), 3);
    // Three waits of five seconds for a result, and the waits between presses.
    assert!(
        started.elapsed() >= Duration::from_secs(16),
        "took {:?}",
        started.elapsed()
    );
    assert!(error.hint().unwrap().contains("verify_timeout"));
}

#[tokio::test(start_paused = true)]
async fn a_widget_that_is_already_clear_is_not_pressed() {
    let (page, mock, presses) = page_with_widget(0, 1).await;

    let solved = page
        .solve_captcha_with(&options())
        .await
        .unwrap()
        .expect("a widget");

    assert_eq!(solved.attempts, 0);
    assert_eq!(solved.evidence, Some(Evidence::Token));
    assert_eq!(presses.load(Ordering::SeqCst), 0);
    assert!(mock.calls_to("Input.dispatchMouseEvent").is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_page_with_no_response_field_is_pressed_once_and_reported_unverified() {
    let (page, _mock, presses) = page_with_widget(u32::MAX, 0).await;

    let solved = page
        .solve_captcha_with(&options())
        .await
        .unwrap()
        .expect("a widget");

    assert_eq!(solved.attempts, 1);
    assert_eq!(solved.evidence, None);
    assert_eq!(
        presses.load(Ordering::SeqCst),
        1,
        "a second press can reset it"
    );
    assert!(solved.to_string().contains("nothing on the page shows"));
}

#[tokio::test(start_paused = true)]
async fn the_pointer_starts_in_the_middle_of_the_viewport_and_travels_to_the_widget() {
    let (page, mock, _presses) = page_with_widget(1, 1).await;

    page.solve_captcha_with(&options()).await.unwrap();

    let moves: Vec<Point> = mock
        .calls_to("Input.dispatchMouseEvent")
        .into_iter()
        .filter(|call| call.params["type"] == "mouseMoved")
        .map(|call| {
            Point::new(
                call.params["x"].as_f64().unwrap(),
                call.params["y"].as_f64().unwrap(),
            )
        })
        .collect();
    let start = moves[0];
    // A resting pointer is somewhere in the middle (30 to 70 percent) of a
    // 1000x800 viewport, not in a corner and not on the widget.
    assert!((300.0..=700.0).contains(&start.x), "starts at {start:?}");
    assert!((240.0..=560.0).contains(&start.y), "starts at {start:?}");
    assert!(
        moves.len() > 10,
        "the pointer travelled in {} moves",
        moves.len()
    );
    // No jump: consecutive moves are close, unlike a pointer put on its target.
    let biggest = moves
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .fold(0.0, f64::max);
    assert!(biggest < 150.0, "the biggest jump was {biggest:.0}px");
}

#[tokio::test(start_paused = true)]
async fn the_press_and_the_release_are_a_moment_apart() {
    let (page, mock, _presses) = page_with_widget(1, 1).await;

    let started = Instant::now();
    page.solve_captcha_with(&options()).await.unwrap();
    let calls = mock.calls_to("Input.dispatchMouseEvent");
    let kinds: Vec<&str> = calls
        .iter()
        .map(|call| call.params["type"].as_str().unwrap())
        .collect();
    let down = kinds
        .iter()
        .position(|kind| *kind == "mousePressed")
        .unwrap();
    assert_eq!(kinds[down + 1], "mouseReleased", "released right after");
    assert_eq!(
        kinds[down - 1],
        "mouseMoved",
        "the pointer was on the box first"
    );
    assert!(started.elapsed() >= Duration::from_millis(300));
}

#[tokio::test(start_paused = true)]
async fn click_at_presses_near_the_aim_and_remembers_where_the_pointer_is() {
    let (page, mock, _presses) = page_with_widget(1, 1).await;
    let person = page.human(seeded());
    let aim = Point::new(640.0, 500.0);

    let landed = person.click_at(aim, 12.0).await.unwrap();

    assert!((landed.x - aim.x).abs() <= 6.0 && (landed.y - aim.y).abs() <= 6.0);
    let before = mock.calls_to("Input.dispatchMouseEvent").len();
    let second = Point::new(200.0, 150.0);
    person.click_at(second, 12.0).await.unwrap();
    let calls = mock.calls_to("Input.dispatchMouseEvent");
    // The next path starts from where the last click ended, not from a corner.
    let first_move = &calls[before].params;
    let (x, y) = (
        first_move["x"].as_f64().unwrap(),
        first_move["y"].as_f64().unwrap(),
    );
    assert!(
        (x - landed.x).hypot(y - landed.y) < 80.0,
        "the second path starts at ({x}, {y}), far from {landed:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_drag_holds_the_button_from_the_handle_to_the_end() {
    let (page, mock, _presses) = page_with_widget(1, 1).await;
    let person = page.human(seeded());

    person
        .drag(Point::new(100.0, 300.0), Point::new(420.0, 300.0), 12.0)
        .await
        .unwrap();

    let calls = mock.calls_to("Input.dispatchMouseEvent");
    let kinds: Vec<&str> = calls
        .iter()
        .map(|call| call.params["type"].as_str().unwrap())
        .collect();
    let down = kinds
        .iter()
        .position(|kind| *kind == "mousePressed")
        .unwrap();
    let up = kinds
        .iter()
        .position(|kind| *kind == "mouseReleased")
        .unwrap();
    assert!(down < up);
    let held = &calls[down + 1..up];
    assert!(held.len() > 5, "dragged in {} moves", held.len());
    assert!(held.iter().all(|call| call.params["buttons"] == 1));
    assert_eq!(calls[up].params["x"].as_f64(), Some(420.0));
}
