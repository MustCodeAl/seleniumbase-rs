//! Humanization helpers for input timing and pointer movement.
//!
//! These utilities produce human-like mouse paths and keystroke delays so
//! automated interactions do not exhibit the perfectly-linear, zero-latency
//! signatures that bot detectors look for. They are deterministic when seeded,
//! which keeps tests reproducible.
//!
//! # Example
//!
//! ```rust
//! use seleniumbase_rs::stealth::humanize::{bezier_mouse_path, Point};
//!
//! let path = bezier_mouse_path(Point::new(0.0, 0.0), Point::new(100.0, 40.0), 16, 7);
//! assert_eq!(path.len(), 16);
//! assert_eq!(path[0], Point::new(0.0, 0.0));
//! ```

/// A 2D point in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A small deterministic PRNG (SplitMix64) used for jitter.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a float in `[0.0, 1.0)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Returns a float in `[min, max)`.
    pub fn range(&mut self, min: f64, max: f64) -> f64 {
        min + (max - min) * self.next_f64()
    }
}

/// Generates a cubic Bézier mouse path from `start` to `end`.
///
/// Two control points are derived from the endpoints with seeded jitter so the
/// arc looks natural. Returns exactly `steps` points (>= 2) including both
/// endpoints.
pub fn bezier_mouse_path(start: Point, end: Point, steps: u32, seed: u64) -> Vec<Point> {
    let steps = steps.max(2);
    let mut rng = Rng::new(seed);

    let dx = end.x - start.x;
    let dy = end.y - start.y;
    // Offset control points perpendicular-ish to the travel direction.
    let jitter = |rng: &mut Rng| rng.range(-0.25, 0.25);
    let c1 = Point::new(
        start.x + dx * 0.3 + dy * jitter(&mut rng),
        start.y + dy * 0.3 - dx * jitter(&mut rng),
    );
    let c2 = Point::new(
        start.x + dx * 0.7 + dy * jitter(&mut rng),
        start.y + dy * 0.7 - dx * jitter(&mut rng),
    );

    let mut path = Vec::with_capacity(steps as usize);
    for i in 0..steps {
        let t = i as f64 / (steps - 1) as f64;
        let mt = 1.0 - t;
        // Cubic Bézier.
        let x = mt * mt * mt * start.x
            + 3.0 * mt * mt * t * c1.x
            + 3.0 * mt * t * t * c2.x
            + t * t * t * end.x;
        let y = mt * mt * mt * start.y
            + 3.0 * mt * mt * t * c1.y
            + 3.0 * mt * t * t * c2.y
            + t * t * t * end.y;
        if i == 0 {
            path.push(start);
        } else if i == steps - 1 {
            path.push(end);
        } else {
            path.push(Point::new(x, y));
        }
    }
    path
}

/// Produces a per-character keystroke delay (milliseconds) for `text`.
///
/// Delays vary between `min_ms` and `max_ms`, with a small extra pause after
/// whitespace to mimic natural typing cadence.
pub fn keystroke_delays(text: &str, min_ms: u64, max_ms: u64, seed: u64) -> Vec<u64> {
    let (min_ms, max_ms) = if min_ms <= max_ms {
        (min_ms, max_ms)
    } else {
        (max_ms, min_ms)
    };
    let mut rng = Rng::new(seed);
    text.chars()
        .map(|ch| {
            let base = rng.range(min_ms as f64, max_ms as f64);
            let extra = if ch.is_whitespace() {
                rng.range(0.0, (max_ms - min_ms) as f64)
            } else {
                0.0
            };
            (base + extra).round() as u64
        })
        .collect()
}

/// Returns a single humanized delay in `[min_ms, max_ms]`.
pub fn keystroke_delay(min_ms: u64, max_ms: u64, seed: u64) -> u64 {
    let (min_ms, max_ms) = if min_ms <= max_ms {
        (min_ms, max_ms)
    } else {
        (max_ms, min_ms)
    };
    let mut rng = Rng::new(seed);
    rng.range(min_ms as f64, max_ms as f64).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_keeps_endpoints_and_count() {
        let start = Point::new(10.0, 20.0);
        let end = Point::new(200.0, 90.0);
        let path = bezier_mouse_path(start, end, 20, 42);
        assert_eq!(path.len(), 20);
        assert_eq!(path[0], start);
        assert_eq!(path[19], end);
    }

    #[test]
    fn path_is_deterministic_for_seed() {
        let a = bezier_mouse_path(Point::new(0.0, 0.0), Point::new(50.0, 50.0), 10, 7);
        let b = bezier_mouse_path(Point::new(0.0, 0.0), Point::new(50.0, 50.0), 10, 7);
        assert_eq!(a, b);
    }

    #[test]
    fn keystroke_delays_within_bounds() {
        let delays = keystroke_delays("hello world", 40, 180, 99);
        assert_eq!(delays.len(), "hello world".chars().count());
        for d in &delays {
            // Whitespace can add up to (max-min) extra.
            assert!(*d >= 40 && *d <= 180 + (180 - 40));
        }
    }

    #[test]
    fn keystroke_delay_handles_swapped_bounds() {
        let d = keystroke_delay(180, 40, 1);
        assert!((40..=180).contains(&d));
    }
}
