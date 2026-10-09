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

pub use crate::sb_cdp::Point;

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

    /// Returns the next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns an index in `0..len`, or `0` when `len` is `0`.
    ///
    /// The 64 random bits are reduced with a modulo, whose bias is far below
    /// anything observable for the small tables this is meant for.
    fn index(&mut self, len: usize) -> usize {
        let Some(bound) = u64::try_from(len).ok().filter(|bound| *bound > 0) else {
            return 0;
        };
        usize::try_from(self.next_u64() % bound).unwrap_or(0)
    }

    /// Picks one element of `items`, uniformly.
    ///
    /// Repeat an element to weight it. An empty array is rejected when the
    /// code is compiled, so this cannot panic at run time.
    ///
    /// ```
    /// use seleniumbase_rs::stealth::humanize::Rng;
    ///
    /// let mut rng = Rng::new(7);
    /// let colour = rng.pick(&["red", "green", "blue"]);
    /// assert!(["red", "green", "blue"].contains(colour));
    /// ```
    pub fn pick<'a, T, const N: usize>(&mut self, items: &'a [T; N]) -> &'a T {
        const { assert!(N > 0, "cannot pick from an empty array") };
        &items[self.index(N)]
    }

    /// Returns a float in `[0.0, 1.0)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Returns a float in `[min, max)`.
    pub fn range(&mut self, min: f64, max: f64) -> f64 {
        min + (max - min) * self.next_f64()
    }

    /// Returns `true` with probability `p` (clamped to `0.0..=1.0`).
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p.clamp(0.0, 1.0)
    }

    /// Returns a normally distributed float (Box–Muller transform).
    pub fn gaussian(&mut self, mean: f64, sigma: f64) -> f64 {
        // `1 - u` is in (0, 1], so the logarithm is finite.
        let u1 = 1.0 - self.next_f64();
        let u2 = self.next_f64();
        let standard = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        mean + sigma * standard
    }

    /// A generator seeded from the operating system's randomness.
    ///
    /// This is for variety, not secrecy: the stream is not suitable for keys.
    #[must_use]
    pub fn from_entropy() -> Self {
        use ring::rand::SecureRandom as _;
        let mut bytes = [0_u8; 8];
        // If the system source fails, fall back to the clock rather than panic.
        if ring::rand::SystemRandom::new().fill(&mut bytes).is_err() {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            #[allow(clippy::cast_possible_truncation)]
            return Self::new(nanos as u64);
        }
        Self::new(u64::from_le_bytes(bytes))
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

    #[test]
    fn next_u64_is_deterministic_for_seed() {
        let mut a = Rng::new(5);
        let mut b = Rng::new(5);
        let first: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let second: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(first, second);
        assert_ne!(first[0], first[1], "the stream must advance");
    }

    #[test]
    fn pick_reaches_every_element_and_nothing_else() {
        let items = ["a", "b", "c", "d"];
        let mut rng = Rng::new(11);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..200 {
            seen.insert(*rng.pick(&items));
        }
        assert_eq!(seen.len(), items.len());
    }

    #[test]
    fn pick_is_deterministic_for_seed() {
        let items = [1, 2, 3, 4, 5, 6, 7, 8];
        let draw = |seed| -> Vec<i32> {
            let mut rng = Rng::new(seed);
            (0..16).map(|_| *rng.pick(&items)).collect()
        };
        assert_eq!(draw(3), draw(3));
        assert_ne!(draw(3), draw(4));
    }

    #[test]
    fn pick_handles_edge_seeds_and_a_single_element() {
        for seed in [0, 1, u64::MAX] {
            let mut rng = Rng::new(seed);
            assert_eq!(*rng.pick(&[42]), 42);
            assert!([1, 2, 3].contains(rng.pick(&[1, 2, 3])));
        }
    }

    #[test]
    fn index_of_an_empty_range_is_zero() {
        let mut rng = Rng::new(9);
        assert_eq!(rng.index(0), 0);
        assert_eq!(rng.index(1), 0);
    }
}
