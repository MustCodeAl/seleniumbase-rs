//! Unit tests for human typing timing, Bézier mouse movements, and BaseCase human actions.

use seleniumbase_rs::evasion::humanize::{bezier_mouse_path, keystroke_delays, Point};

#[test]
fn keystroke_delays_within_human_pace_range() {
    let text = "Hello, world! This is a human typing test.";
    let min_ms = 50;
    let max_ms = 120;
    let seed = 42;
    let delays = keystroke_delays(text, min_ms, max_ms, seed);

    assert_eq!(delays.len(), text.chars().count());

    for (ch, delay) in text.chars().zip(&delays) {
        assert!(
            *delay >= min_ms,
            "delay {delay} for char '{ch}' was below min {min_ms}"
        );
        // Space or punctuation can add natural pause, but base is clamped
        let allowed_max = if ch.is_whitespace() {
            max_ms + (max_ms - min_ms)
        } else {
            max_ms
        };
        assert!(
            *delay <= allowed_max,
            "delay {delay} for char '{ch}' exceeded allowed max {allowed_max}"
        );
    }

    // Verify there is natural jitter (values are not all identical)
    let first = delays[0];
    let has_variation = delays.iter().any(|d| *d != first);
    assert!(has_variation, "keystroke delays should have natural jitter");
}

#[test]
fn bezier_mouse_path_geometry_and_curve() {
    let start = Point::new(100.0, 100.0);
    let end = Point::new(600.0, 450.0);
    let steps = 24;
    let seed = 12345;

    let path = bezier_mouse_path(start, end, steps, seed);
    assert_eq!(path.len(), steps as usize);
    assert_eq!(path[0], start);
    assert_eq!(path[steps as usize - 1], end);

    // Verify points do not lie strictly on a straight line (they follow a curve)
    let mut max_deviation = 0.0_f64;
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let line_len = (dx * dx + dy * dy).sqrt();

    for pt in &path[1..steps as usize - 1] {
        // Distance from point to line (start -> end)
        let numerator = (dy * pt.x - dx * pt.y + end.x * start.y - end.y * start.x).abs();
        let dist = numerator / line_len;
        if dist > max_deviation {
            max_deviation = dist;
        }
    }

    assert!(
        max_deviation > 2.0,
        "bezier path must exhibit realistic curve deviation from linear segment, got {max_deviation}"
    );
}

#[test]
fn human_typing_deterministic_with_same_seed() {
    let text = "Consistency test";
    let a = keystroke_delays(text, 50, 120, 999);
    let b = keystroke_delays(text, 50, 120, 999);
    assert_eq!(a, b);

    let c = keystroke_delays(text, 50, 120, 1000);
    assert_ne!(a, c);
}
