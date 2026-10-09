//! A model of how a person uses a mouse and a keyboard.
//!
//! Bot detectors look for input that is too regular: a pointer that travels in
//! a straight line at constant speed, lands on the exact centre of a button,
//! and a keyboard that never varies its rhythm. [`Humanizer`] produces input
//! that does not have those tells:
//!
//! - **Pointer paths** are curved, follow a bell-shaped speed profile (slow
//!   start, fast middle, slow arrival), take as long as Fitts's law says a
//!   movement of that distance to a target of that size takes, carry a faint
//!   hand tremor that fades out on arrival, and sometimes overshoot a distant
//!   target and correct.
//! - **Typing** has Gaussian inter-key intervals with a slowly drifting
//!   rhythm, longer pauses at word and sentence boundaries, occasional
//!   hesitations, and, if you ask for it, typos that are noticed and
//!   corrected.
//!
//! Everything here is pure: it returns plans (positions and waits) and does no
//! I/O. [`Page::human`](crate::sb_cdp::Page::human) plays a plan out on a real
//! page. With a seed the plans are reproducible, which keeps tests exact.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::sb_cdp::Point;
//! use seleniumbase_rs::stealth::behavior::{Behavior, Humanizer};
//!
//! # fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let behavior = Behavior::builder().typing_wpm(70.0).seed(7).build()?;
//! let mut person = Humanizer::new(behavior);
//!
//! let path = person.mouse_path(Point::new(0.0, 0.0), Point::new(400.0, 120.0), 80.0);
//! assert_eq!(path.last().map(|step| step.at), Some(Point::new(400.0, 120.0)));
//!
//! let keys = person.typing_plan("hello world");
//! assert_eq!(keys.len(), "hello world".chars().count());
//! # Ok(())
//! # }
//! ```

use std::f64::consts::PI;
use std::time::Duration;

use super::fingerprint::HumanizeConfig;
use super::humanize::Rng;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::{Point, Rect};

/// Time between pointer samples, close to a display frame.
const SAMPLE_MS: f64 = 12.0;

/// Fitts's law: movement time is `A + B * log2(distance / width + 1)`.
const FITTS_A_MS: f64 = 120.0;
const FITTS_B_MS: f64 = 150.0;
const MIN_MOVE_MS: f64 = 120.0;
const MAX_MOVE_MS: f64 = 2500.0;

/// Only a movement at least this long is ever overshot.
const OVERSHOOT_MIN_DISTANCE: f64 = 200.0;

/// Typical amplitude of hand tremor, in pixels.
const TREMOR_PX: f64 = 0.7;

/// How strongly one tremor sample carries into the next.
const TREMOR_MEMORY: f64 = 0.8;

/// How long a wheel notch scrolls, in pixels.
const WHEEL_NOTCH_PX: (f64, f64) = (80.0, 120.0);

/// Typing speed and rhythm.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Typing {
    mean_ms: f64,
    sigma_ms: f64,
    typo_rate: f64,
}

/// Pointer speed and habits.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pointer {
    speed: f64,
    overshoot: f64,
}

/// How a simulated person types and moves a mouse.
///
/// Build one with [`Behavior::builder`]; the default is a 55 words-per-minute
/// typist with an ordinary pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behavior {
    typing: Typing,
    pointer: Pointer,
    seed: Option<u64>,
}

impl Default for Behavior {
    fn default() -> Self {
        Self::builder().build().unwrap_or(Self {
            typing: Typing {
                mean_ms: 12_000.0 / 55.0,
                sigma_ms: 12_000.0 / 55.0 * 0.35,
                typo_rate: 0.0,
            },
            pointer: Pointer {
                speed: 1.0,
                overshoot: 0.2,
            },
            seed: None,
        })
    }
}

impl Behavior {
    /// A builder with the default typist and pointer.
    #[must_use]
    pub fn builder() -> BehaviorBuilder {
        BehaviorBuilder::default()
    }

    /// The nearest behaviour to a fingerprint's [`HumanizeConfig`]: keystroke
    /// delays centred between its bounds, with a spread that keeps nearly all
    /// of them inside.
    #[must_use]
    pub fn from_config(config: &HumanizeConfig) -> Self {
        let (low, high) = if config.min_keystroke_delay_ms <= config.max_keystroke_delay_ms {
            (config.min_keystroke_delay_ms, config.max_keystroke_delay_ms)
        } else {
            (config.max_keystroke_delay_ms, config.min_keystroke_delay_ms)
        };
        #[allow(clippy::cast_precision_loss)]
        let (low, high) = (low as f64, high as f64);
        let mean = ((low + high) / 2.0).max(20.0);
        let sigma = ((high - low) / 6.0).max(1.0);
        let mut behavior = Self::default();
        behavior.typing.mean_ms = mean;
        behavior.typing.sigma_ms = sigma;
        behavior
    }

    /// The mean time between keystrokes, in milliseconds.
    #[must_use]
    pub fn mean_keystroke_ms(&self) -> f64 {
        self.typing.mean_ms
    }
}

/// Builds a [`Behavior`], checking the values in [`build`](Self::build).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BehaviorBuilder {
    wpm: f64,
    variability: f64,
    typo_rate: f64,
    pointer_speed: f64,
    overshoot: f64,
    seed: Option<u64>,
}

impl Default for BehaviorBuilder {
    fn default() -> Self {
        Self {
            wpm: 55.0,
            variability: 0.35,
            typo_rate: 0.0,
            pointer_speed: 1.0,
            overshoot: 0.2,
            seed: None,
        }
    }
}

impl BehaviorBuilder {
    /// Typing speed in words per minute (five characters to a word).
    #[must_use]
    pub fn typing_wpm(mut self, wpm: f64) -> Self {
        self.wpm = wpm;
        self
    }

    /// How much keystroke intervals vary, as a fraction of their mean.
    #[must_use]
    pub fn typing_variability(mut self, fraction: f64) -> Self {
        self.variability = fraction;
        self
    }

    /// The chance, per letter, of a typo that is noticed and corrected.
    ///
    /// Off by default: a typo is only safe where a correction is harmless.
    #[must_use]
    pub fn typo_rate(mut self, probability: f64) -> Self {
        self.typo_rate = probability;
        self
    }

    /// Pointer speed relative to a typical user: 2.0 moves twice as fast.
    #[must_use]
    pub fn pointer_speed(mut self, factor: f64) -> Self {
        self.pointer_speed = factor;
        self
    }

    /// The chance that a long movement overshoots its target and corrects.
    #[must_use]
    pub fn overshoot(mut self, probability: f64) -> Self {
        self.overshoot = probability;
        self
    }

    /// Makes every plan reproducible.
    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Checks the values and builds the behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the typing speed is not
    /// in `5..=400` words per minute, the variability is not in `0.0..=1.5`,
    /// the typo rate is not in `0.0..=0.2`, the pointer speed is not in
    /// `0.1..=5.0`, or the overshoot chance is not in `0.0..=1.0`.
    pub fn build(self) -> Result<Behavior, SeleniumBaseError> {
        let check = |name: &str, value: f64, low: f64, high: f64| {
            if value.is_finite() && (low..=high).contains(&value) {
                Ok(())
            } else {
                Err(SeleniumBaseError::InvalidConfig(format!(
                    "{name} must be between {low} and {high}, got {value}"
                )))
            }
        };
        check("typing_wpm", self.wpm, 5.0, 400.0)?;
        check("typing_variability", self.variability, 0.0, 1.5)?;
        check("typo_rate", self.typo_rate, 0.0, 0.2)?;
        check("pointer_speed", self.pointer_speed, 0.1, 5.0)?;
        check("overshoot", self.overshoot, 0.0, 1.0)?;

        let mean_ms = 12_000.0 / self.wpm;
        Ok(Behavior {
            typing: Typing {
                mean_ms,
                sigma_ms: mean_ms * self.variability,
                typo_rate: self.typo_rate,
            },
            pointer: Pointer {
                speed: self.pointer_speed,
                overshoot: self.overshoot,
            },
            seed: self.seed,
        })
    }
}

/// One sample of a pointer movement: wait, then be at `at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// Where the pointer is.
    pub at: Point,
    /// How long to wait before moving there.
    pub wait: Duration,
}

/// What a keystroke does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyAction {
    /// Types a character.
    Type(char),
    /// Deletes the character before the cursor.
    Backspace,
}

/// One planned keystroke: wait, then act.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keystroke {
    /// What to do.
    pub action: KeyAction,
    /// How long to wait before doing it.
    pub wait: Duration,
}

/// One notch of a mouse-wheel scroll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelNotch {
    /// Pixels to scroll; negative scrolls up.
    pub dy: f64,
    /// How long to wait before this notch.
    pub wait: Duration,
}

/// Plans pointer movements and typing for one simulated person.
///
/// It carries the random state, so successive plans differ even for the same
/// request, as a person's would.
#[derive(Debug, Clone)]
pub struct Humanizer {
    behavior: Behavior,
    rng: Rng,
}

impl Humanizer {
    /// A person who behaves as `behavior` describes. Without a seed, the
    /// randomness comes from the operating system.
    #[must_use]
    pub fn new(behavior: Behavior) -> Self {
        let rng = behavior.seed.map_or_else(Rng::from_entropy, Rng::new);
        Self { behavior, rng }
    }

    /// The behaviour being simulated.
    #[must_use]
    pub fn behavior(&self) -> &Behavior {
        &self.behavior
    }

    /// A pointer movement from `from` to `to`, aiming at a target
    /// `target_width` pixels across (a bigger target is reached faster).
    ///
    /// The last step is exactly `to`. A movement shorter than a pixel is a
    /// single step.
    pub fn mouse_path(&mut self, from: Point, to: Point, target_width: f64) -> Vec<Step> {
        let distance = distance(from, to);
        if distance < 1.0 {
            return vec![Step {
                at: to,
                wait: Duration::ZERO,
            }];
        }
        let pointer = self.behavior.pointer;
        let direction = unit(from, to);

        let overshoots = distance >= OVERSHOOT_MIN_DISTANCE && self.rng.chance(pointer.overshoot);
        let aim = if overshoots {
            let past = (distance * self.rng.range(0.03, 0.08)).min(16.0);
            let sideways = self.rng.range(-0.4, 0.4) * past;
            let normal = normal(direction);
            Point::new(
                to.x + direction.x * past + normal.x * sideways,
                to.y + direction.y * past + normal.y * sideways,
            )
        } else {
            to
        };

        let time_ms =
            movement_time_ms(distance, target_width, pointer.speed) * self.rng.range(0.9, 1.15);
        let mut steps = self.stroke(from, aim, time_ms, true);

        if overshoots {
            // Notice the miss, then correct with a short, straight movement.
            let reaction = self.rng.range(40.0, 120.0);
            let correction_ms = self.rng.range(90.0, 220.0) / pointer.speed;
            let mut fix = self.stroke(aim, to, correction_ms, false);
            if let Some(first) = fix.first_mut() {
                first.wait += millis(reaction);
            }
            steps.extend(fix);
        }
        if let Some(last) = steps.last_mut() {
            last.at = to;
        }
        steps
    }

    /// One minimum-jerk stroke from `from` to `to` lasting `total_ms`.
    fn stroke(&mut self, from: Point, to: Point, total_ms: f64, curved: bool) -> Vec<Step> {
        let length = distance(from, to);
        let direction = unit(from, to);
        let normal = normal(direction);

        let (c1, c2) = if curved && length > 1.0 {
            let side = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
            let bulge = side * length * self.rng.range(0.04, 0.18);
            let first = self.rng.range(0.6, 1.4);
            let second = self.rng.range(0.3, 1.0);
            (
                Point::new(
                    from.x + direction.x * length * 0.3 + normal.x * bulge * first,
                    from.y + direction.y * length * 0.3 + normal.y * bulge * first,
                ),
                Point::new(
                    from.x + direction.x * length * 0.7 + normal.x * bulge * second,
                    from.y + direction.y * length * 0.7 + normal.y * bulge * second,
                ),
            )
        } else {
            (lerp(from, to, 1.0 / 3.0), lerp(from, to, 2.0 / 3.0))
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let count = ((total_ms / SAMPLE_MS).ceil() as usize).max(2);
        let weights: Vec<f64> = (0..count).map(|_| self.rng.range(0.8, 1.2)).collect();
        let weight_sum: f64 = weights.iter().sum();

        let amplitude = TREMOR_PX * (length / 100.0).clamp(0.2, 1.0);
        let innovation = (1.0 - TREMOR_MEMORY * TREMOR_MEMORY).sqrt();
        let mut tremor = (0.0, 0.0);

        let mut steps = Vec::with_capacity(count);
        for (index, weight) in weights.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let tau = (index + 1) as f64 / count as f64;
            let along = cubic(from, c1, c2, to, min_jerk(tau));
            tremor.0 = TREMOR_MEMORY * tremor.0 + innovation * self.rng.gaussian(0.0, amplitude);
            tremor.1 = TREMOR_MEMORY * tremor.1 + innovation * self.rng.gaussian(0.0, amplitude);
            // The tremor fades to nothing on arrival, so the stroke ends on target.
            let fade = (PI * tau).sin().max(0.0);
            steps.push(Step {
                at: Point::new(along.x + tremor.0 * fade, along.y + tremor.1 * fade),
                wait: millis(total_ms * weight / weight_sum),
            });
        }
        if let Some(last) = steps.last_mut() {
            last.at = to;
        }
        steps
    }

    /// A point inside `rect` (in viewport coordinates) at which to click it.
    ///
    /// People do not hit the exact centre: this is normally distributed around
    /// it and kept inside the rectangle's edges.
    pub fn target_point(&mut self, rect: Rect) -> Point {
        let pad_x = (rect.width * 0.15).min(4.0);
        let pad_y = (rect.height * 0.15).min(4.0);
        let x = self
            .rng
            .gaussian(rect.x + rect.width / 2.0, rect.width / 6.0)
            .clamp(
                rect.x + pad_x,
                (rect.x + rect.width - pad_x).max(rect.x + pad_x),
            );
        let y = self
            .rng
            .gaussian(rect.y + rect.height / 2.0, rect.height / 6.0)
            .clamp(
                rect.y + pad_y,
                (rect.y + rect.height - pad_y).max(rect.y + pad_y),
            );
        Point::new(x, y)
    }

    /// Where a resting pointer might be on a viewport this big: somewhere in
    /// the middle, not in a corner.
    pub fn resting_point(&mut self, width: f64, height: f64) -> Point {
        Point::new(
            width * self.rng.range(0.3, 0.7),
            height * self.rng.range(0.3, 0.7),
        )
    }

    /// How long a button stays pressed during a click.
    pub fn click_dwell(&mut self) -> Duration {
        millis(self.rng.gaussian(75.0, 20.0).clamp(30.0, 160.0))
    }

    /// A reaction delay before starting to act, such as before typing.
    pub fn reaction(&mut self) -> Duration {
        millis(self.rng.gaussian(180.0, 50.0).clamp(80.0, 450.0))
    }

    /// The keystrokes, with their waits, that type `text`.
    ///
    /// Intervals are Gaussian around the typing speed, scaled by a rhythm that
    /// drifts slowly (people type in bursts), with extra pauses after spaces,
    /// punctuation and newlines and the odd hesitation. With a non-zero typo
    /// rate, some letters are typed wrong, noticed, deleted and retyped; the
    /// result of applying the plan is always exactly `text`.
    pub fn typing_plan(&mut self, text: &str) -> Vec<Keystroke> {
        let typing = self.behavior.typing;
        let floor = (typing.mean_ms * 0.25).max(25.0);
        let ceiling = typing.mean_ms * 6.0;
        let pause_scale = typing.mean_ms / 200.0;

        let mut plan = Vec::with_capacity(text.chars().count());
        let mut rhythm = 0.0_f64;
        let mut previous: Option<char> = None;

        for ch in text.chars() {
            rhythm = 0.9 * rhythm + self.rng.gaussian(0.0, 0.08);
            let interval = |rng: &mut Rng| {
                (rng.gaussian(typing.mean_ms, typing.sigma_ms) * rhythm.exp()).clamp(floor, ceiling)
            };

            let mut wait_ms = interval(&mut self.rng);
            if let Some(before) = previous {
                wait_ms += match before {
                    ' ' => self.rng.gaussian(90.0, 35.0),
                    ',' | ';' | ':' => self.rng.gaussian(160.0, 50.0),
                    '.' | '!' | '?' => self.rng.gaussian(320.0, 90.0),
                    '\n' => self.rng.gaussian(280.0, 80.0),
                    _ => 0.0,
                }
                .max(0.0)
                    * pause_scale;
            }
            if self.rng.chance(0.015) {
                wait_ms += self.rng.gaussian(450.0, 140.0).max(0.0);
            }

            if typing.typo_rate > 0.0
                && ch.is_ascii_alphabetic()
                && self.rng.chance(typing.typo_rate)
            {
                let wrong = adjacent_key(ch, &mut self.rng);
                plan.push(Keystroke {
                    action: KeyAction::Type(wrong),
                    wait: millis(wait_ms),
                });
                let noticed = self.rng.gaussian(260.0, 80.0).clamp(120.0, 600.0);
                plan.push(Keystroke {
                    action: KeyAction::Backspace,
                    wait: millis(noticed),
                });
                let retype = interval(&mut self.rng);
                plan.push(Keystroke {
                    action: KeyAction::Type(ch),
                    wait: millis(retype),
                });
            } else {
                plan.push(Keystroke {
                    action: KeyAction::Type(ch),
                    wait: millis(wait_ms),
                });
            }
            previous = Some(ch);
        }
        plan
    }

    /// The mouse-wheel notches that scroll `total_dy` pixels (negative is up),
    /// eased so the scroll starts and ends gently.
    pub fn scroll_plan(&mut self, total_dy: f64) -> Vec<WheelNotch> {
        let total = total_dy.abs();
        if total < 1.0 {
            return Vec::new();
        }
        let sign = total_dy.signum();
        let duration_ms = (250.0 + total * 0.9) / self.behavior.pointer.speed;

        let mut notches = Vec::new();
        let mut done = 0.0;
        let mut previous_tau = 0.0;
        while done < total {
            let step = self
                .rng
                .range(WHEEL_NOTCH_PX.0, WHEEL_NOTCH_PX.1)
                .min(total - done);
            done += step;
            let tau = invert_min_jerk(done / total);
            notches.push(WheelNotch {
                dy: sign * step,
                wait: millis((tau - previous_tau) * duration_ms),
            });
            previous_tau = tau;
        }
        notches
    }
}

/// Fitts's law movement time, in milliseconds.
fn movement_time_ms(distance: f64, target_width: f64, speed: f64) -> f64 {
    let difficulty = (distance / target_width.max(4.0) + 1.0).log2();
    ((FITTS_A_MS + FITTS_B_MS * difficulty) / speed).clamp(MIN_MOVE_MS, MAX_MOVE_MS)
}

/// The minimum-jerk position profile: 0 to 1 with zero velocity at both ends.
fn min_jerk(tau: f64) -> f64 {
    let t = tau.clamp(0.0, 1.0);
    t * t * t * (10.0 - 15.0 * t + 6.0 * t * t)
}

/// The `tau` for which [`min_jerk`] reaches `progress`, by bisection.
fn invert_min_jerk(progress: f64) -> f64 {
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        if min_jerk(middle) < progress {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

fn cubic(p0: Point, p1: Point, p2: Point, p3: Point, s: f64) -> Point {
    let m = 1.0 - s;
    let (a, b, c, d) = (m * m * m, 3.0 * m * m * s, 3.0 * m * s * s, s * s * s);
    Point::new(
        a * p0.x + b * p1.x + c * p2.x + d * p3.x,
        a * p0.y + b * p1.y + c * p2.y + d * p3.y,
    )
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

fn unit(from: Point, to: Point) -> Point {
    let length = distance(from, to);
    if length == 0.0 {
        Point::new(1.0, 0.0)
    } else {
        Point::new((to.x - from.x) / length, (to.y - from.y) / length)
    }
}

fn normal(direction: Point) -> Point {
    Point::new(-direction.y, direction.x)
}

fn millis(ms: f64) -> Duration {
    Duration::from_secs_f64(ms.max(0.0) / 1000.0)
}

/// A key next to `ch` on a QWERTY keyboard, in the same case.
fn adjacent_key(ch: char, rng: &mut Rng) -> char {
    const ROWS: [(&str, f64); 3] = [("qwertyuiop", 0.0), ("asdfghjkl", 0.25), ("zxcvbnm", 0.75)];
    let lower = ch.to_ascii_lowercase();
    let mut here = None;
    let mut keys = Vec::new();
    for (row, (letters, offset)) in ROWS.iter().enumerate() {
        for (column, key) in letters.chars().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let position = (column as f64 + offset, row as f64);
            if key == lower {
                here = Some(position);
            }
            keys.push((key, position));
        }
    }
    let Some(here) = here else { return ch };
    let neighbours: Vec<char> = keys
        .iter()
        .filter(|(key, (x, y))| *key != lower && (x - here.0).hypot(y - here.1) < 1.3)
        .map(|(key, _)| *key)
        .collect();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pick = (rng.next_f64() * neighbours.len() as f64) as usize;
    let chosen = neighbours
        .get(pick.min(neighbours.len().saturating_sub(1)))
        .copied()
        .unwrap_or(lower);
    if ch.is_ascii_uppercase() {
        chosen.to_ascii_uppercase()
    } else {
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(seed: u64) -> Humanizer {
        Humanizer::new(Behavior::builder().seed(seed).build().unwrap())
    }

    fn total(steps: &[Step]) -> Duration {
        steps.iter().map(|step| step.wait).sum()
    }

    // ------------------------------------------------------------------
    // Pointer
    // ------------------------------------------------------------------

    #[test]
    fn a_path_ends_exactly_on_its_target() {
        for seed in 0..50 {
            let to = Point::new(612.5, 333.25);
            let path = person(seed).mouse_path(Point::new(10.0, 10.0), to, 60.0);
            assert_eq!(path.last().unwrap().at, to, "seed {seed}");
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_path_and_another_seed_a_different_one() {
        let (from, to) = (Point::new(0.0, 0.0), Point::new(500.0, 200.0));
        let a = person(1).mouse_path(from, to, 50.0);
        let b = person(1).mouse_path(from, to, 50.0);
        let c = person(2).mouse_path(from, to, 50.0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn a_zero_length_move_is_a_single_immediate_step() {
        let here = Point::new(40.0, 40.0);
        let path = person(3).mouse_path(here, here, 50.0);
        assert_eq!(
            path,
            [Step {
                at: here,
                wait: Duration::ZERO
            }]
        );
    }

    #[test]
    fn movement_time_follows_fitts_law_farther_and_smaller_take_longer() {
        let near = movement_time_ms(100.0, 100.0, 1.0);
        let far = movement_time_ms(1000.0, 100.0, 1.0);
        let small = movement_time_ms(1000.0, 10.0, 1.0);
        assert!(near < far && far < small, "{near} < {far} < {small}");
        assert!(movement_time_ms(500.0, 50.0, 2.0) < movement_time_ms(500.0, 50.0, 1.0));
        assert_eq!(movement_time_ms(1.0e9, 1.0, 1.0), MAX_MOVE_MS);
    }

    #[test]
    fn a_move_takes_about_as_long_as_fitts_law_says() {
        let (from, to) = (Point::new(0.0, 0.0), Point::new(800.0, 0.0));
        let expected = movement_time_ms(800.0, 80.0, 1.0);
        for seed in 0..30 {
            let mut p = person(seed);
            let path = p.mouse_path(from, to, 80.0);
            let took = total(&path).as_secs_f64() * 1000.0;
            // Allow the random scatter and an overshoot correction.
            assert!(
                took > expected * 0.8 && took < expected * 1.15 + 450.0,
                "seed {seed}: {took} vs {expected}"
            );
        }
    }

    #[test]
    fn speed_is_bell_shaped_slow_at_the_ends_and_fast_in_the_middle() {
        let (from, to) = (Point::new(0.0, 0.0), Point::new(900.0, 0.0));
        let mut slow_ends = 0;
        for seed in 0..40 {
            let path = Humanizer::new(
                Behavior::builder()
                    .overshoot(0.0)
                    .seed(seed)
                    .build()
                    .unwrap(),
            )
            .mouse_path(from, to, 60.0);
            let speeds: Vec<f64> = path
                .windows(2)
                .map(|pair| distance(pair[0].at, pair[1].at) / pair[1].wait.as_secs_f64().max(1e-6))
                .collect();
            let n = speeds.len();
            let mean = |slice: &[f64]| slice.iter().sum::<f64>() / slice.len() as f64;
            let start = mean(&speeds[..n / 10]);
            let middle = mean(&speeds[n * 2 / 5..n * 3 / 5]);
            let end = mean(&speeds[n - n / 10..]);
            if middle > 3.0 * start && middle > 3.0 * end {
                slow_ends += 1;
            }
        }
        assert!(
            slow_ends >= 38,
            "only {slow_ends} of 40 paths had a bell-shaped speed"
        );
    }

    #[test]
    fn a_long_path_is_curved_not_a_straight_line() {
        let (from, to) = (Point::new(0.0, 0.0), Point::new(800.0, 0.0));
        let mut bowed = 0;
        for seed in 0..40 {
            let path = Humanizer::new(
                Behavior::builder()
                    .overshoot(0.0)
                    .seed(seed)
                    .build()
                    .unwrap(),
            )
            .mouse_path(from, to, 60.0);
            let deviation = path.iter().map(|step| step.at.y.abs()).fold(0.0, f64::max);
            if deviation > 15.0 {
                bowed += 1;
            }
        }
        assert!(
            bowed >= 36,
            "only {bowed} of 40 long paths bowed away from the straight line"
        );
    }

    #[test]
    fn the_pointer_settles_the_last_few_samples_stay_near_the_target() {
        for seed in 0..30 {
            let to = Point::new(700.0, 300.0);
            let path = Humanizer::new(
                Behavior::builder()
                    .overshoot(0.0)
                    .seed(seed)
                    .build()
                    .unwrap(),
            )
            .mouse_path(Point::new(0.0, 0.0), to, 60.0);
            for step in &path[path.len() - 4..] {
                assert!(distance(step.at, to) < 12.0, "seed {seed}: {:?}", step.at);
            }
        }
    }

    #[test]
    fn long_moves_sometimes_overshoot_and_short_ones_never_do() {
        let always = Behavior::builder().overshoot(1.0).build().unwrap();
        let (from, far) = (Point::new(0.0, 0.0), Point::new(900.0, 0.0));
        let overshot = (0..20).filter(|seed| {
            let behavior = Behavior {
                seed: Some(*seed),
                ..always
            };
            let path = Humanizer::new(behavior).mouse_path(from, far, 60.0);
            path.iter().any(|step| step.at.x > far.x + 2.0)
        });
        assert!(
            overshot.count() >= 18,
            "an always-overshooting person should pass the target"
        );

        let near = Point::new(150.0, 0.0);
        for seed in 0..20 {
            let behavior = Behavior {
                seed: Some(seed),
                ..always
            };
            let path = Humanizer::new(behavior).mouse_path(from, near, 60.0);
            assert!(
                path.iter().all(|step| step.at.x <= near.x + 4.0),
                "seed {seed}"
            );
        }
    }

    #[test]
    fn a_faster_pointer_arrives_sooner() {
        let (from, to) = (Point::new(0.0, 0.0), Point::new(600.0, 100.0));
        let slow = Humanizer::new(
            Behavior::builder()
                .pointer_speed(0.5)
                .overshoot(0.0)
                .seed(5)
                .build()
                .unwrap(),
        )
        .mouse_path(from, to, 60.0);
        let fast = Humanizer::new(
            Behavior::builder()
                .pointer_speed(2.0)
                .overshoot(0.0)
                .seed(5)
                .build()
                .unwrap(),
        )
        .mouse_path(from, to, 60.0);
        assert!(total(&fast) < total(&slow));
    }

    #[test]
    fn clicks_land_inside_the_target_but_not_always_at_its_centre() {
        let rect = Rect {
            x: 100.0,
            y: 200.0,
            width: 120.0,
            height: 40.0,
        };
        let mut p = person(11);
        let points: Vec<Point> = (0..200).map(|_| p.target_point(rect)).collect();
        assert!(points.iter().all(|pt| pt.x >= rect.x
            && pt.x <= rect.x + rect.width
            && pt.y >= rect.y
            && pt.y <= rect.y + rect.height));
        let centre = Point::new(160.0, 220.0);
        assert!(
            points.iter().filter(|pt| **pt == centre).count() < 3,
            "clicks must not all hit the centre"
        );
        let mean_x = points.iter().map(|pt| pt.x).sum::<f64>() / 200.0;
        assert!(
            (mean_x - 160.0).abs() < 8.0,
            "clicks cluster around the centre, mean {mean_x}"
        );
    }

    #[test]
    fn a_tiny_target_is_still_hit_inside_its_bounds() {
        let rect = Rect {
            x: 10.0,
            y: 10.0,
            width: 3.0,
            height: 2.0,
        };
        let mut p = person(2);
        for _ in 0..50 {
            let pt = p.target_point(rect);
            assert!(
                pt.x >= 10.0 && pt.x <= 13.0 && pt.y >= 10.0 && pt.y <= 12.0,
                "{pt:?}"
            );
        }
    }

    #[test]
    fn click_dwell_and_reaction_are_bounded() {
        let mut p = person(4);
        for _ in 0..200 {
            let dwell = p.click_dwell().as_millis();
            assert!((30..=160).contains(&dwell), "{dwell}");
            let reaction = p.reaction().as_millis();
            assert!((80..=450).contains(&reaction), "{reaction}");
        }
    }

    // ------------------------------------------------------------------
    // Typing
    // ------------------------------------------------------------------

    fn mean_and_deviation(values: &[f64]) -> (f64, f64) {
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        (mean, variance.sqrt())
    }

    fn intervals(plan: &[Keystroke], skip_boundaries: &str, text: &str) -> Vec<f64> {
        // Intervals of letters that follow a letter, so boundary pauses and
        // hesitations do not count.
        let chars: Vec<char> = text.chars().collect();
        plan.iter()
            .enumerate()
            .filter(|(i, _)| *i > 0 && !skip_boundaries.contains(chars[i - 1]))
            .map(|(_, k)| k.wait.as_secs_f64() * 1000.0)
            .collect()
    }

    #[test]
    fn the_typing_speed_sets_the_mean_interval() {
        let text = "the quick brown fox jumps over the lazy dog ".repeat(60);
        for wpm in [30.0, 60.0, 120.0] {
            let mut p =
                Humanizer::new(Behavior::builder().typing_wpm(wpm).seed(9).build().unwrap());
            let plan = p.typing_plan(&text);
            let (mean, _) = mean_and_deviation(&intervals(&plan, " ", &text));
            let expected = 12_000.0 / wpm;
            assert!(
                (mean - expected).abs() < expected * 0.15,
                "{wpm} wpm: mean {mean:.1} vs {expected:.1}"
            );
        }
    }

    #[test]
    fn intervals_vary_with_a_spread_close_to_the_requested_variability() {
        let text = "abcdefghij".repeat(300);
        let mut p = Humanizer::new(
            Behavior::builder()
                .typing_wpm(60.0)
                .typing_variability(0.35)
                .seed(1)
                .build()
                .unwrap(),
        );
        let plan = p.typing_plan(&text);
        let (mean, deviation) = mean_and_deviation(&intervals(&plan, "", &text));
        let ratio = deviation / mean;
        assert!((0.2..=0.55).contains(&ratio), "relative spread {ratio:.2}");
    }

    #[test]
    fn zero_variability_still_has_a_drifting_rhythm_but_stays_in_a_narrow_band() {
        let text = "abcdefghij".repeat(100);
        let mut p = Humanizer::new(
            Behavior::builder()
                .typing_variability(0.0)
                .seed(1)
                .build()
                .unwrap(),
        );
        let (mean, deviation) = mean_and_deviation(&intervals(&p.typing_plan(&text), "", &text));
        assert!(deviation / mean < 0.35, "{}", deviation / mean);
    }

    #[test]
    fn words_end_with_a_longer_pause_than_letters_inside_a_word() {
        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa ".repeat(40);
        let mut p = person(5);
        let plan = p.typing_plan(&text);
        let chars: Vec<char> = text.chars().collect();
        let mut after_space = Vec::new();
        let mut inside = Vec::new();
        for (i, key) in plan.iter().enumerate().skip(1) {
            let ms = key.wait.as_secs_f64() * 1000.0;
            if chars[i - 1] == ' ' {
                after_space.push(ms);
            } else if chars[i - 1] != ' ' && chars[i] != ' ' {
                inside.push(ms);
            }
        }
        let (space_mean, _) = mean_and_deviation(&after_space);
        let (inside_mean, _) = mean_and_deviation(&inside);
        assert!(
            space_mean > inside_mean * 1.2,
            "{space_mean:.0} vs {inside_mean:.0}"
        );
    }

    #[test]
    fn sentence_ends_pause_longer_than_commas() {
        let text = "yes, no. ".repeat(200);
        let mut p = person(6);
        let plan = p.typing_plan(&text);
        let chars: Vec<char> = text.chars().collect();
        let after = |ch: char| {
            let waits: Vec<f64> = plan
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(i, _)| chars[i - 1] == ch)
                .map(|(_, k)| k.wait.as_secs_f64() * 1000.0)
                .collect();
            mean_and_deviation(&waits).0
        };
        assert!(after('.') > after(','), "{} vs {}", after('.'), after(','));
    }

    #[test]
    fn without_typos_the_plan_types_exactly_the_text() {
        let text = "Hello, World! 123\nnext line";
        let plan = person(8).typing_plan(text);
        let typed: String = plan
            .iter()
            .filter_map(|k| match k.action {
                KeyAction::Type(c) => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(typed, text);
        assert!(plan.iter().all(|k| k.action != KeyAction::Backspace));
    }

    fn apply(plan: &[Keystroke]) -> String {
        let mut buffer = String::new();
        for key in plan {
            match key.action {
                KeyAction::Type(c) => buffer.push(c),
                KeyAction::Backspace => {
                    buffer.pop();
                }
            }
        }
        buffer
    }

    #[test]
    fn typos_are_corrected_so_the_result_is_always_the_original_text() {
        let text = "The Quick Brown Fox Jumps Over The Lazy Dog 0123 and so on";
        let behavior = Behavior::builder().typo_rate(0.2).build().unwrap();
        let mut typos = 0;
        for seed in 0..60 {
            let mut p = Humanizer::new(Behavior {
                seed: Some(seed),
                ..behavior
            });
            let plan = p.typing_plan(text);
            assert_eq!(apply(&plan), text, "seed {seed}");
            typos += plan
                .iter()
                .filter(|k| k.action == KeyAction::Backspace)
                .count();
        }
        assert!(
            typos > 100,
            "a 20% typo rate should produce typos, got {typos}"
        );
    }

    #[test]
    fn a_typo_is_followed_by_a_pause_then_a_backspace_then_the_right_key() {
        let behavior = Behavior::builder().typo_rate(0.2).seed(3).build().unwrap();
        let plan = Humanizer::new(behavior).typing_plan(&"abcdefghijklmnop".repeat(10));
        let at = plan
            .iter()
            .position(|k| k.action == KeyAction::Backspace)
            .expect("a typo");
        assert!(at >= 1 && matches!(plan[at - 1].action, KeyAction::Type(_)));
        assert!(
            plan[at].wait >= Duration::from_millis(120),
            "the typist takes a moment to notice"
        );
        assert!(matches!(plan[at + 1].action, KeyAction::Type(_)));
    }

    #[test]
    fn a_wrong_key_is_a_neighbour_of_the_right_one_and_keeps_its_case() {
        let mut rng = Rng::new(1);
        for _ in 0..100 {
            let wrong = adjacent_key('g', &mut rng);
            assert!(
                "tyfhvb".contains(wrong) || "rtyufhjvbn".contains(wrong),
                "{wrong}"
            );
            assert_ne!(wrong, 'g');
            let upper = adjacent_key('G', &mut rng);
            assert!(upper.is_ascii_uppercase());
        }
        assert_eq!(adjacent_key('7', &mut rng), '7', "only letters get typos");
    }

    #[test]
    fn typing_is_deterministic_for_a_seed() {
        let a = person(77).typing_plan("deterministic");
        let b = person(77).typing_plan("deterministic");
        assert_eq!(a, b);
    }

    #[test]
    fn empty_text_is_an_empty_plan() {
        assert!(person(1).typing_plan("").is_empty());
    }

    // ------------------------------------------------------------------
    // Scrolling
    // ------------------------------------------------------------------

    #[test]
    fn a_scroll_adds_up_to_the_requested_distance_in_wheel_sized_notches() {
        for total in [250.0, -640.0, 1000.0] {
            let notches = person(2).scroll_plan(total);
            let sum: f64 = notches.iter().map(|n| n.dy).sum();
            assert!((sum - total).abs() < 1e-6, "{sum} vs {total}");
            assert!(notches.iter().all(|n| n.dy.abs() <= 120.0 + 1e-9));
            assert!(notches.iter().all(|n| n.dy.signum() == total.signum()));
        }
        assert!(person(2).scroll_plan(0.0).is_empty());
    }

    #[test]
    fn scrolling_eases_the_notches_come_faster_in_the_middle() {
        let notches = person(3).scroll_plan(2000.0);
        let n = notches.len();
        let waits: Vec<f64> = notches.iter().map(|k| k.wait.as_secs_f64()).collect();
        let first = waits[..n / 5].iter().sum::<f64>() / (n / 5) as f64;
        let middle = waits[n * 2 / 5..n * 3 / 5].iter().sum::<f64>() / (n / 5) as f64;
        assert!(middle < first, "{middle} vs {first}");
    }

    // ------------------------------------------------------------------
    // Configuration
    // ------------------------------------------------------------------

    #[test]
    fn the_builder_rejects_values_outside_their_range() {
        for builder in [
            Behavior::builder().typing_wpm(1.0),
            Behavior::builder().typing_wpm(1000.0),
            Behavior::builder().typing_variability(-0.1),
            Behavior::builder().typo_rate(0.5),
            Behavior::builder().pointer_speed(0.0),
            Behavior::builder().pointer_speed(f64::NAN),
            Behavior::builder().overshoot(1.5),
        ] {
            assert!(
                matches!(builder.build(), Err(SeleniumBaseError::InvalidConfig(_))),
                "{builder:?}"
            );
        }
        assert!(Behavior::builder().build().is_ok());
    }

    #[test]
    fn a_wpm_becomes_the_mean_interval() {
        let behavior = Behavior::builder().typing_wpm(60.0).build().unwrap();
        assert!((behavior.mean_keystroke_ms() - 200.0).abs() < 1e-9);
    }

    #[test]
    fn a_fingerprint_config_maps_to_a_matching_behaviour() {
        let config = HumanizeConfig {
            enabled: true,
            min_keystroke_delay_ms: 40,
            max_keystroke_delay_ms: 180,
            mouse_steps: 24,
        };
        let behavior = Behavior::from_config(&config);
        assert!((behavior.mean_keystroke_ms() - 110.0).abs() < 1e-9);
        let swapped = HumanizeConfig {
            min_keystroke_delay_ms: 180,
            max_keystroke_delay_ms: 40,
            ..config
        };
        assert_eq!(
            Behavior::from_config(&swapped),
            behavior,
            "swapped bounds are normalised"
        );
    }

    #[test]
    fn an_unseeded_person_differs_from_run_to_run() {
        let a = Humanizer::new(Behavior::default()).typing_plan("entropy matters here");
        let b = Humanizer::new(Behavior::default()).typing_plan("entropy matters here");
        assert_ne!(a, b);
    }

    #[test]
    fn min_jerk_runs_from_zero_to_one_and_inverts() {
        assert_eq!(min_jerk(0.0), 0.0);
        assert!((min_jerk(1.0) - 1.0).abs() < 1e-12);
        assert!((min_jerk(0.5) - 0.5).abs() < 1e-12);
        for p in [0.1, 0.37, 0.8] {
            assert!((min_jerk(invert_min_jerk(p)) - p).abs() < 1e-9);
        }
    }
}
