//! Human-paced input against a scripted browser, with no Chrome.
//!
//! The clock is paused, so the seconds a person takes to type pass instantly
//! while `tokio::time::Instant` still measures them.
//! Run with `cargo test --features test-util`.

#![cfg(feature = "test-util")]

use seleniumbase_rs::sb_cdp::{Browser, MockCtrl, Page, Point};
use seleniumbase_rs::stealth::behavior::Behavior;
use serde_json::{json, Value};
use tokio::time::Instant;

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// A 1000x800 viewport holding a 200x50 element centred at (500, 400).
async fn page_with_element() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("window.innerWidth") {
            value(json!([1000.0, 800.0]))
        } else if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 500.0, "y": 400.0, "covered": false }))
        } else if expression.contains("__sbcdp.info(") {
            value(json!({
                "tag": "button", "text": "", "html": "", "attributes": {}, "visible": true,
                "rect": { "x": 400.0, "y": 375.0, "width": 200.0, "height": 50.0 },
            }))
        } else if expression.contains(".length > 0") || expression.contains(".some(") {
            value(json!(true))
        } else {
            value(Value::Null)
        })
    });
    (browser.default_page().await.unwrap(), mock)
}

fn mouse_events(mock: &MockCtrl, kind: &str) -> Vec<Point> {
    mock.calls_to("Input.dispatchMouseEvent")
        .into_iter()
        .filter(|call| call.params["type"] == kind)
        .map(|call| {
            Point::new(
                call.params["x"].as_f64().unwrap(),
                call.params["y"].as_f64().unwrap(),
            )
        })
        .collect()
}

/// What the typed keys leave in a text field.
fn typed_result(mock: &MockCtrl) -> String {
    let mut field = String::new();
    for call in mock.calls_to("Input.dispatchKeyEvent") {
        // Named keys such as Backspace go out as `rawKeyDown`.
        if !matches!(call.params["type"].as_str(), Some("keyDown" | "rawKeyDown")) {
            continue;
        }
        match call.params["key"].as_str().unwrap() {
            "Backspace" => {
                field.pop();
            }
            "Enter" => field.push('\n'),
            "Tab" => field.push('\t'),
            key => field.push_str(key),
        }
    }
    field
}

fn seeded(seed: u64) -> Behavior {
    Behavior::builder().seed(seed).build().unwrap()
}

#[tokio::test(start_paused = true)]
async fn moving_the_pointer_sends_a_timed_curved_path_that_ends_on_the_target() {
    let (page, mock) = page_with_element().await;
    let person = page.human(Behavior::builder().overshoot(0.0).seed(3).build().unwrap());
    let target = Point::new(800.0, 120.0);

    let started = Instant::now();
    person.move_to(target, 80.0).await.unwrap();
    let took = started.elapsed();

    let moves = mouse_events(&mock, "mouseMoved");
    assert!(
        moves.len() > 15,
        "a human path has many samples, got {}",
        moves.len()
    );
    assert_eq!(*moves.last().unwrap(), target);
    assert!(
        took.as_millis() >= 150,
        "a long movement takes time, took {took:?}"
    );

    // Curved: some sample is well off the straight line from first to last.
    let (a, b) = (moves[1], *moves.last().unwrap());
    let length = (b.x - a.x).hypot(b.y - a.y);
    let off_line = moves
        .iter()
        .map(|p| ((b.x - a.x) * (a.y - p.y) - (a.x - p.x) * (b.y - a.y)).abs() / length)
        .fold(0.0, f64::max);
    assert!(
        off_line > 3.0,
        "the path should bow away from a straight line, by {off_line:.1}px"
    );
}

#[tokio::test(start_paused = true)]
async fn the_pointer_stays_where_it_was_left_between_movements() {
    let (page, mock) = page_with_element().await;
    let person = page.human(Behavior::builder().overshoot(0.0).seed(8).build().unwrap());
    let first = Point::new(200.0, 600.0);

    person.move_to(first, 60.0).await.unwrap();
    let after_first = mouse_events(&mock, "mouseMoved").len();
    person
        .move_to(Point::new(900.0, 100.0), 60.0)
        .await
        .unwrap();

    let moves = mouse_events(&mock, "mouseMoved");
    let second_start = moves[after_first];
    let gap = (second_start.x - first.x).hypot(second_start.y - first.y);
    assert!(
        gap < 40.0,
        "the second movement starts near where the first ended, {gap:.1}px away"
    );
}

#[tokio::test(start_paused = true)]
async fn a_clone_shares_the_pointer_with_the_original() {
    let (page, mock) = page_with_element().await;
    let person = page.human(Behavior::builder().overshoot(0.0).seed(8).build().unwrap());
    let twin = person.clone();
    let first = Point::new(150.0, 150.0);

    person.move_to(first, 60.0).await.unwrap();
    let after_first = mouse_events(&mock, "mouseMoved").len();
    twin.move_to(Point::new(700.0, 500.0), 60.0).await.unwrap();

    let start = mouse_events(&mock, "mouseMoved")[after_first];
    assert!((start.x - first.x).hypot(start.y - first.y) < 40.0);
}

#[tokio::test(start_paused = true)]
async fn clicks_land_inside_the_element_but_not_always_at_its_centre() {
    let mut presses = Vec::new();
    for seed in 0..20 {
        let (page, mock) = page_with_element().await;
        let person = page.human(seeded(seed));

        person.click(&page.locator("button")).await.unwrap();

        let pressed = mouse_events(&mock, "mousePressed");
        let released = mouse_events(&mock, "mouseReleased");
        assert_eq!(pressed.len(), 1, "seed {seed}");
        assert_eq!(
            pressed, released,
            "the button is released where it was pressed"
        );
        presses.push(pressed[0]);
    }
    assert!(presses
        .iter()
        .all(|p| (400.0..=600.0).contains(&p.x) && (375.0..=425.0).contains(&p.y)));
    let distinct: std::collections::BTreeSet<_> = presses
        .iter()
        .map(|p| (p.x.to_bits(), p.y.to_bits()))
        .collect();
    assert!(
        distinct.len() >= 15,
        "clicks should differ, got {} distinct of 20",
        distinct.len()
    );
    let centred = presses
        .iter()
        .filter(|p| **p == Point::new(500.0, 400.0))
        .count();
    assert_eq!(centred, 0, "no click should hit the exact centre");
}

#[tokio::test(start_paused = true)]
async fn a_click_holds_the_button_for_a_moment() {
    let (page, mock) = page_with_element().await;
    let person = page.human(seeded(1));

    person.click(&page.locator("button")).await.unwrap();

    let events = mock.calls_to("Input.dispatchMouseEvent");
    let pressed_at = events
        .iter()
        .position(|c| c.params["type"] == "mousePressed")
        .unwrap();
    let released_at = events
        .iter()
        .position(|c| c.params["type"] == "mouseReleased")
        .unwrap();
    assert!(released_at > pressed_at);
    assert_eq!(events[pressed_at].params["button"], "left");
}

#[tokio::test(start_paused = true)]
async fn typing_sends_every_character_in_order_at_a_human_pace() {
    let (page, mock) = page_with_element().await;
    let person = page.human(seeded(2));
    let text = "hello, world";

    let started = Instant::now();
    person.type_focused(text).await.unwrap();
    let took = started.elapsed();

    assert_eq!(typed_result(&mock), text);
    let mean_ms = took.as_secs_f64() * 1000.0 / text.chars().count() as f64;
    assert!(
        (120.0..600.0).contains(&mean_ms),
        "about 55 wpm is ~218 ms a key, got {mean_ms:.0}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_faster_typist_finishes_sooner() {
    let text = "the same sentence for both typists";
    let mut times = Vec::new();
    for wpm in [30.0, 120.0] {
        let (page, _mock) = page_with_element().await;
        let person = page.human(Behavior::builder().typing_wpm(wpm).seed(4).build().unwrap());
        let started = Instant::now();
        person.type_focused(text).await.unwrap();
        times.push(started.elapsed());
    }
    assert!(times[1] * 2 < times[0], "{times:?}");
}

#[tokio::test(start_paused = true)]
async fn typing_into_an_element_clicks_it_first() {
    let (page, mock) = page_with_element().await;
    let person = page.human(seeded(5));

    person
        .type_text(&page.locator("input"), "abc")
        .await
        .unwrap();

    let pressed = mouse_events(&mock, "mousePressed");
    assert_eq!(pressed.len(), 1, "the field is clicked to focus it");
    assert_eq!(typed_result(&mock), "abc");
    let calls = mock.calls();
    let click_at = calls
        .iter()
        .position(|c| c.method == "Input.dispatchMouseEvent" && c.params["type"] == "mousePressed")
        .unwrap();
    let key_at = calls
        .iter()
        .position(|c| c.method == "Input.dispatchKeyEvent")
        .unwrap();
    assert!(click_at < key_at, "the click comes before the first key");
}

#[tokio::test(start_paused = true)]
async fn typos_are_deleted_so_the_field_holds_the_intended_text() {
    let text = "The quick brown fox jumps over the lazy dog";
    let mut deletions = 0;
    for seed in 0..10 {
        let (page, mock) = page_with_element().await;
        let behavior = Behavior::builder()
            .typo_rate(0.15)
            .seed(seed)
            .build()
            .unwrap();
        page.human(behavior).type_focused(text).await.unwrap();

        assert_eq!(typed_result(&mock), text, "seed {seed}");
        deletions += mock
            .calls_to("Input.dispatchKeyEvent")
            .iter()
            .filter(|c| c.params["key"] == "Backspace" && c.params["type"] != "keyUp")
            .count();
    }
    assert!(
        deletions > 10,
        "a 15% typo rate should produce corrections, got {deletions}"
    );
}

#[tokio::test(start_paused = true)]
async fn newlines_and_tabs_press_their_keys() {
    let (page, mock) = page_with_element().await;

    page.human(seeded(6)).type_focused("a\tb\nc").await.unwrap();

    assert_eq!(typed_result(&mock), "a\tb\nc");
}

#[tokio::test(start_paused = true)]
async fn scrolling_sends_wheel_notches_that_add_up_to_the_distance() {
    let (page, mock) = page_with_element().await;
    let person = page.human(seeded(7));

    let started = Instant::now();
    person.scroll_by(-700.0).await.unwrap();
    let took = started.elapsed();

    let wheels: Vec<f64> = mock
        .calls_to("Input.dispatchMouseEvent")
        .iter()
        .filter(|c| c.params["type"] == "mouseWheel")
        .map(|c| c.params["deltaY"].as_f64().unwrap())
        .collect();
    assert!(
        wheels.len() >= 6,
        "a long scroll is several notches, got {}",
        wheels.len()
    );
    assert!((wheels.iter().sum::<f64>() + 700.0).abs() < 1e-6);
    assert!(wheels.iter().all(|dy| *dy < 0.0 && dy.abs() <= 120.0));
    assert!(took.as_millis() >= 300, "{took:?}");
}

#[tokio::test(start_paused = true)]
async fn a_missing_element_is_reported_without_moving_the_pointer() {
    let (browser, mock) = Browser::new_mocked();
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(
            if expression.contains("resolve(") && expression.contains(".length === 0") {
                value(json!(true))
            } else if expression.contains("resolve(") && expression.contains(".length") {
                value(json!(0))
            } else {
                value(Value::Null)
            },
        )
    });
    let page = browser
        .default_page()
        .await
        .unwrap()
        .with_timeout(std::time::Duration::from_millis(100));

    let error = page
        .human(seeded(1))
        .click(&page.locator("#nope"))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("#nope"), "{error}");
    assert!(mouse_events(&mock, "mouseMoved").is_empty());
}
