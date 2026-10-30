//! Passing CAPTCHA widgets that ask for a click or a drag.
//!
//! [`Page::solve_captcha`] finds a widget in the page, its open shadow roots and
//! the frames it may read, waits for it to stand still, presses it along a human
//! pointer path and reads the page for a result (see [`SolveOptions`] and
//! [`Evidence`]). The steps are in the `engine` module and run against any
//! `Surface`, which is how `BaseCase::solve_captcha` shares them; the scripts
//! the steps send to the page are in the `script` module.

use std::fmt;
use std::time::Duration;

use serde_json::Value;

use super::{Page, Point, Rect};
use crate::error::SeleniumBaseError;
use crate::evasion::behavior::Behavior;

mod engine;
mod script;

pub(crate) use engine::{solve, Surface};

/// How far into a checkbox widget its box sits, in CSS pixels.
///
/// Every supported widget draws its checkbox against the left edge.
const CHECKBOX_INSET: f64 = 28.0;

/// How far from each end of a slider widget the handle starts and the drag
/// stops, in CSS pixels.
const SLIDER_INSET: f64 = 30.0;

/// How long a widget must stand still before it is pressed.
const SETTLE: Duration = Duration::from_millis(240);

/// A CAPTCHA widget that [`Page::solve_captcha`] knows how to attempt.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::Captcha;
///
/// assert_eq!(Captcha::Turnstile.to_string(), "Cloudflare Turnstile");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Captcha {
    /// Cloudflare Turnstile.
    Turnstile,
    /// Google reCAPTCHA.
    Recaptcha,
    /// hCaptcha.
    Hcaptcha,
    /// Friendly Captcha.
    FriendlyCaptcha,
    /// The `DataDome` slider.
    DataDome,
}

impl Captcha {
    /// Every supported widget, in the order they are looked for.
    pub const ALL: [Self; 5] = [
        Self::Turnstile,
        Self::Recaptcha,
        Self::Hcaptcha,
        Self::FriendlyCaptcha,
        Self::DataDome,
    ];

    /// The position of the widget in [`ALL`](Self::ALL), which is how the page
    /// scripts name it.
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Turnstile => 0,
            Self::Recaptcha => 1,
            Self::Hcaptcha => 2,
            Self::FriendlyCaptcha => 3,
            Self::DataDome => 4,
        }
    }

    /// The selectors that find the widget, in tiers: the frames of the vendor
    /// first, then the containers the page draws around them. A tier is tried
    /// across the whole page (its shadow roots and readable frames) before the
    /// next one is.
    ///
    /// The frames are named by what the vendor's own script gives them (the
    /// address, an id, a title), so a page that serves the frame from another
    /// address is still found.
    fn tiers(self) -> &'static [&'static [&'static str]] {
        match self {
            Self::Turnstile => &[
                &[
                    r#"iframe[src*="challenges.cloudflare.com"]"#,
                    r#"iframe[id^="cf-chl-widget"]"#,
                    r#"iframe[title*="Cloudflare security challenge"]"#,
                ],
                &[
                    ".cf-turnstile",
                    ".cf-turnstile-wrapper",
                    "ngx-turnstile",
                    "#turnstile-widget",
                ],
            ],
            Self::Recaptcha => &[&[
                r#"iframe[src*="recaptcha/api2/anchor"]"#,
                r#"iframe[src*="recaptcha/enterprise/anchor"]"#,
            ]],
            Self::Hcaptcha => &[&[
                r#"iframe[src*="hcaptcha.com"][src*="checkbox"]"#,
                "iframe[data-hcaptcha-widget-id]",
            ]],
            Self::FriendlyCaptcha => &[
                &[
                    r#"iframe[src*="friendlycaptcha"]"#,
                    "iframe[data--frc-frame-id]",
                ],
                &[".frc-captcha"],
            ],
            Self::DataDome => &[&[r#"iframe[src*="captcha-delivery.com"]"#]],
        }
    }

    /// The names of the hidden fields in which the page receives the answer.
    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Turnstile => &["cf-turnstile-response"],
            Self::Recaptcha => &["g-recaptcha-response"],
            Self::Hcaptcha => &["h-captcha-response", "g-recaptcha-response"],
            Self::FriendlyCaptcha => &["frc-captcha-solution", "frc-captcha-response"],
            Self::DataDome => &[],
        }
    }

    /// Selectors of a success state the widget shows, for a widget whose
    /// inside the page may read (not a frame from another origin).
    fn success_selectors(self) -> &'static [&'static str] {
        match self {
            Self::Turnstile => &["#success-icon"],
            Self::Recaptcha => &[".recaptcha-checkbox-checked"],
            Self::Hcaptcha | Self::FriendlyCaptcha | Self::DataDome => &[],
        }
    }

    /// The cookie the vendor sets once the visitor is let through.
    pub(crate) fn clearance_cookie(self) -> Option<&'static str> {
        match self {
            Self::Turnstile => Some("cf_clearance"),
            Self::DataDome => Some("datadome"),
            Self::Recaptcha | Self::Hcaptcha | Self::FriendlyCaptcha => None,
        }
    }

    /// How to operate a widget occupying `area` (in viewport coordinates).
    pub(crate) fn plan(self, area: Rect) -> Plan {
        let inset = match self {
            Self::DataDome => SLIDER_INSET,
            _ => CHECKBOX_INSET,
        };
        let y = area.y + area.height / 2.0;
        let press = Point {
            x: area.x + inset.min(area.width / 2.0),
            y,
        };
        let release = (self == Self::DataDome).then(|| Point {
            x: area.x + area.width - SLIDER_INSET.min(area.width / 2.0),
            y,
        });
        Plan { press, release }
    }

    /// A script that scrolls the first visible widget to the middle of the
    /// viewport and returns `{ kind, x, y, width, height }` for it, or `null`.
    ///
    /// The rectangle is in viewport coordinates, which is what mouse events
    /// use. `kind` is an index into [`ALL`](Self::ALL). The widget is looked
    /// for in the page, its open shadow roots and the frames the page may read.
    #[cfg(any(feature = "gui", test))]
    pub(crate) fn locate_script() -> String {
        script::locate()
    }

    /// Reads the answer of [`locate_script`](Self::locate_script).
    pub(crate) fn parse_located(found: &Value) -> Option<(Self, Rect)> {
        let kind = Self::ALL.get(usize::try_from(found.get("kind")?.as_u64()?).ok()?)?;
        let number = |key: &str| found.get(key).and_then(Value::as_f64);
        Some((
            *kind,
            Rect {
                x: number("x")?,
                y: number("y")?,
                width: number("width")?,
                height: number("height")?,
            },
        ))
    }
}

/// Where to press a widget and, for a slider, where to let go.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Plan {
    pub(crate) press: Point,
    pub(crate) release: Option<Point>,
}

/// What showed that a CAPTCHA was cleared.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::Evidence;
///
/// assert_eq!(Evidence::Token.to_string(), "the response field holds a token");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Evidence {
    /// The widget's response field (such as `cf-turnstile-response`) now holds
    /// a token.
    Token,
    /// The expression given to [`SolveOptions::success_marker`] became true.
    Marker,
    /// The vendor's clearance cookie (`cf_clearance` for Cloudflare) arrived.
    Cookie,
    /// The page was replaced by one that shows no widget.
    Navigated,
    /// The widget shows a success state that the page may read.
    WidgetState,
    /// The widget went away and stayed away.
    WidgetGone,
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Token => "the response field holds a token",
            Self::Marker => "the success marker became true",
            Self::Cookie => "the clearance cookie arrived",
            Self::Navigated => "the page was replaced by one with no widget",
            Self::WidgetState => "the widget shows a success state",
            Self::WidgetGone => "the widget went away",
        })
    }
}

/// How [`Page::solve_captcha_with`] finds, presses and checks a widget.
///
/// Every setting has a default that suits a real page, so
/// `SolveOptions::default()` is what [`Page::solve_captcha`] uses.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use seleniumbase_rs::sb_cdp::SolveOptions;
///
/// let options = SolveOptions::new()
///     .locate_timeout(Duration::from_secs(10))
///     .attempts(2)
///     .success_marker("document.querySelector('#welcome')");
/// assert_eq!(options.attempts_allowed(), 2);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SolveOptions {
    locate_timeout: Duration,
    settle: Duration,
    verify_timeout: Duration,
    attempts: u32,
    backoff: Duration,
    marker: Option<String>,
    behavior: Behavior,
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            locate_timeout: Duration::from_secs(6),
            settle: SETTLE,
            verify_timeout: Duration::from_secs(5),
            attempts: 3,
            backoff: Duration::from_millis(600),
            marker: None,
            behavior: Behavior::default(),
        }
    }
}

impl SolveOptions {
    /// The default settings: wait up to 6 s for a widget that is on its way,
    /// require it to stand still for 240 ms, wait up to 5 s after each press
    /// for the page to show the result, and press up to 3 times.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How long to wait for a widget that is on its way: the page carries the
    /// signs of a challenge, or a widget element exists but has no size yet.
    /// A page with no such sign returns `None` at once, whatever this is. Use
    /// [`Duration::ZERO`] to never wait.
    #[must_use]
    pub fn locate_timeout(mut self, timeout: Duration) -> Self {
        self.locate_timeout = timeout;
        self
    }

    /// How long the widget must stand still (the same place and size, on
    /// consecutive readings) before it is pressed. An animated or scrolling
    /// page moves a widget that was measured too early.
    #[must_use]
    pub fn settle(mut self, still_for: Duration) -> Self {
        self.settle = still_for;
        self
    }

    /// How long to wait after a press for the page to show the result.
    #[must_use]
    pub fn verify_timeout(mut self, timeout: Duration) -> Self {
        self.verify_timeout = timeout;
        self
    }

    /// How many times to press the widget at most. A widget is pressed again
    /// only when the page can show whether it cleared (it has a response field,
    /// or a [success marker](Self::success_marker) was given) and did not.
    /// Zero counts as one.
    #[must_use]
    pub fn attempts(mut self, attempts: u32) -> Self {
        self.attempts = attempts;
        self
    }

    /// The wait before the second press. It doubles for each further press,
    /// up to 8 s, and is spread by 25 percent either way so that a second try
    /// is not on a clock.
    #[must_use]
    pub fn backoff(mut self, base: Duration) -> Self {
        self.backoff = base;
        self
    }

    /// A script expression that is true once the site shows its own page or
    /// the result of the widget, such as `document.querySelector('#welcome')`.
    /// It is checked after each press, and it makes a missing response field
    /// count as a failure to retry instead of a result that cannot be seen.
    #[must_use]
    pub fn success_marker(mut self, expression: impl Into<String>) -> Self {
        self.marker = Some(expression.into());
        self
    }

    /// The pace of the pointer: how fast it travels, whether it overshoots.
    /// Seed it to make the paths reproducible.
    #[must_use]
    pub fn behavior(mut self, behavior: Behavior) -> Self {
        self.behavior = behavior;
        self
    }

    /// How many presses are allowed at most; at least one.
    #[must_use]
    pub fn attempts_allowed(&self) -> u32 {
        self.attempts.max(1)
    }
}

/// What [`Page::solve_captcha_with`] did to a widget.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Solved {
    /// The widget that was pressed.
    pub captcha: Captcha,
    /// How many times it was pressed; zero when it was already clear.
    pub attempts: u32,
    /// What showed it cleared, or `None` when nothing the solver can read did:
    /// the widget was pressed once and the result cannot be seen from outside
    /// (a frame from another origin, with no response field on the page).
    pub evidence: Option<Evidence>,
    /// How long the call took, from the first reading to the verdict.
    pub elapsed: Duration,
}

/// A sentence about what was done and what showed it worked, such as
/// `cleared after 1 press: the response field holds a token`.
impl fmt::Display for Solved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let presses = if self.attempts == 1 {
            "1 press".to_owned()
        } else {
            format!("{} presses", self.attempts)
        };
        match self.evidence {
            Some(evidence) if self.attempts == 0 => write!(f, "already clear: {evidence}"),
            Some(evidence) => write!(f, "cleared after {presses}: {evidence}"),
            None => write!(
                f,
                "pressed ({presses}); nothing on the page shows whether it cleared"
            ),
        }
    }
}

impl fmt::Display for Captcha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Turnstile => "Cloudflare Turnstile",
            Self::Recaptcha => "reCAPTCHA",
            Self::Hcaptcha => "hCaptcha",
            Self::FriendlyCaptcha => "Friendly Captcha",
            Self::DataDome => "DataDome",
        })
    }
}

impl Page {
    /// Attempts the first supported CAPTCHA widget found on the page, and
    /// checks that it cleared.
    ///
    /// The widget is looked for in the page, in its open shadow roots and in
    /// the frames it may read, by the names the vendors give their frames and
    /// containers; a form checkbox that is not part of a challenge is never
    /// touched. A page that carries the signs of a challenge but has not drawn
    /// the widget yet is waited for. The widget is pressed once it stands
    /// still, with trusted pointer events that travel along a curved path from
    /// where the pointer was, rest on the box and hold the button for a moment;
    /// checkbox widgets are pressed at the left edge where they draw their box
    /// and the `DataDome` slider is dragged.
    ///
    /// The result is then looked for, because the widgets live in frames whose
    /// insides cannot be read: the response field of the widget holds a token,
    /// the clearance cookie arrived, or the page was replaced by one with no
    /// widget. If the page can show a result (it has a response field) and it
    /// does not within the time allowed, the widget is pressed again after a
    /// growing wait, up to three presses. See [`SolveOptions`] to change any of
    /// this, and [`solve_captcha_with`](Self::solve_captcha_with) for what was
    /// seen.
    ///
    /// Returns the widget that was attempted, or `None` if the page shows no
    /// supported widget.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Captcha`] if the widget was pressed the
    /// allowed number of times and the page never showed it cleared, and
    /// [`SeleniumBaseError::CdpDriver`] if the browser rejects the input.
    pub async fn solve_captcha(&self) -> Result<Option<Captcha>, SeleniumBaseError> {
        Ok(self
            .solve_captcha_with(&SolveOptions::default())
            .await?
            .map(|solved| solved.captcha))
    }

    /// Like [`solve_captcha`](Self::solve_captcha), with the settings of
    /// `options`, and says what was done and what showed it worked.
    ///
    /// # Errors
    ///
    /// See [`solve_captcha`](Self::solve_captcha).
    pub async fn solve_captcha_with(
        &self,
        options: &SolveOptions,
    ) -> Result<Option<Solved>, SeleniumBaseError> {
        solve(self, options).await
    }
}

impl Surface for Page {
    async fn eval(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.evaluate(script).await
    }

    async fn cookie_names(&self) -> Result<Vec<String>, SeleniumBaseError> {
        Ok(self
            .cookies()
            .all()
            .await?
            .into_iter()
            .map(|cookie| cookie.name)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const AREA: Rect = Rect {
        x: 100.0,
        y: 200.0,
        width: 300.0,
        height: 65.0,
    };

    #[test]
    fn checkboxes_are_pressed_near_the_left_edge_at_mid_height() {
        let plan = Captcha::Turnstile.plan(AREA);
        assert_eq!(plan.press, Point { x: 128.0, y: 232.5 });
        assert_eq!(plan.release, None);
    }

    #[test]
    fn a_narrow_widget_is_pressed_no_further_than_its_middle() {
        let narrow = Rect {
            width: 20.0,
            ..AREA
        };
        assert_eq!(Captcha::Recaptcha.plan(narrow).press.x, 110.0);
    }

    #[test]
    fn the_slider_is_dragged_across_its_width() {
        let plan = Captcha::DataDome.plan(AREA);
        let release = plan.release.expect("a slider is dragged");
        assert_eq!(plan.press.x, 130.0);
        assert_eq!(release.x, 370.0);
        assert_eq!(plan.press.y, release.y);
    }

    #[test]
    fn every_widget_has_a_distinct_name_and_its_own_frame_selectors() {
        let names: std::collections::HashSet<_> =
            Captcha::ALL.iter().map(ToString::to_string).collect();
        assert_eq!(names.len(), Captcha::ALL.len());

        let mut selectors = std::collections::HashSet::new();
        for kind in Captcha::ALL {
            assert!(!kind.tiers().is_empty(), "{kind} has no selector");
            for tier in kind.tiers() {
                assert!(!tier.is_empty(), "{kind} has an empty tier");
                for selector in *tier {
                    assert!(selectors.insert(*selector), "{selector} is used twice");
                }
            }
        }
    }

    #[test]
    fn a_widget_is_named_by_its_position_in_all() {
        for (position, kind) in Captcha::ALL.iter().enumerate() {
            assert_eq!(kind.index(), position, "{kind}");
        }
    }

    #[test]
    fn only_the_vendors_that_set_a_clearance_cookie_name_one() {
        assert_eq!(Captcha::Turnstile.clearance_cookie(), Some("cf_clearance"));
        assert_eq!(Captcha::DataDome.clearance_cookie(), Some("datadome"));
        assert_eq!(Captcha::Recaptcha.clearance_cookie(), None);
    }

    #[test]
    fn a_located_widget_is_read_back_with_its_kind_and_box() {
        let found = json!({ "kind": 2, "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 });
        let (kind, area) = Captcha::parse_located(&found).expect("a well-formed answer");
        assert_eq!(kind, Captcha::Hcaptcha);
        assert_eq!(
            area,
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0
            }
        );
    }

    #[test]
    fn no_widget_and_nonsense_answers_read_as_none() {
        assert!(Captcha::parse_located(&json!(null)).is_none());
        assert!(Captcha::parse_located(
            &json!({ "kind": 99, "x": 0, "y": 0, "width": 1, "height": 1 })
        )
        .is_none());
        assert!(Captcha::parse_located(&json!({ "kind": 0 })).is_none());
    }

    #[test]
    fn the_defaults_wait_a_little_and_press_at_most_three_times() {
        let options = SolveOptions::default();
        assert_eq!(options, SolveOptions::new());
        assert_eq!(options.attempts_allowed(), 3);
        assert!(options.locate_timeout > Duration::ZERO);
        assert!(
            options.backoff > Duration::ZERO,
            "a zero wait is a busy loop"
        );
        assert!(options.marker.is_none());
    }

    #[test]
    fn the_locate_script_asks_the_page_for_the_first_widget_and_scrolls_to_it() {
        let script = Captcha::locate_script();
        assert!(script.contains("return find(true, -1);"), "{script}");
    }

    #[test]

    fn the_settings_chain_and_zero_attempts_count_as_one() {
        let options = SolveOptions::new()
            .attempts(0)
            .locate_timeout(Duration::ZERO)
            .verify_timeout(Duration::from_secs(1))
            .settle(Duration::from_millis(10))
            .backoff(Duration::from_millis(5))
            .success_marker("window.done");
        assert_eq!(options.attempts_allowed(), 1);
        assert_eq!(options.locate_timeout, Duration::ZERO);
        assert_eq!(options.marker.as_deref(), Some("window.done"));
    }

    #[test]
    fn every_kind_of_evidence_has_its_own_words() {
        let all = [
            Evidence::Token,
            Evidence::Marker,
            Evidence::Cookie,
            Evidence::Navigated,
            Evidence::WidgetState,
            Evidence::WidgetGone,
        ];
        let words: std::collections::HashSet<_> = all.iter().map(ToString::to_string).collect();
        assert_eq!(words.len(), all.len());
    }
}
