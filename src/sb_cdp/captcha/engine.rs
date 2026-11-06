//! The solve loop that [`Page::solve_captcha_with`](crate::sb_cdp::Page::solve_captcha_with)
//! and `BaseCase::solve_captcha_with` share.
//!
//! The loop is written against [`Surface`], the little the solver needs from a
//! browser tab, so the Pure CDP engine and the WebDriver one run the same
//! steps and a test can run them against a scripted tab:
//!
//! 1. **Locate.** One script finds the widget in the page, its open shadow
//!    roots and the frames it may read. A page with no widget and no sign of
//!    a challenge answers `None` at once; a page that carries the signs of a
//!    challenge is waited for.
//! 2. **Settle.** The widget is read again and again until it has stood still
//!    for [`SolveOptions::settle`], scrolled into view. An entrance animation
//!    or a late layout moves a widget that was read once.
//! 3. **Press.** The pointer travels from where it was along a curved path,
//!    rests on the box, and the button is held for a moment.
//! 4. **Verify.** The page is read until it shows a result: a token in the
//!    response field, the clearance cookie, a page with no widget, a success
//!    marker. The widget is not pressed again while it is verifying.
//! 5. **Retry.** Only when the page can show a result and did not: after a
//!    growing, spread-out wait, up to the attempts allowed, then a typed error.
//! 6. **Rounds.** Steps 1 to 5 are one round. When [`SolveOptions::rounds`] allows
//!    more, a round that spent its presses without a result is followed by the
//!    cooldown and a new round, which finds the widget again; the typed error
//!    comes after the last round. The rotations of the options are not acted on
//!    here: a [`Surface`] has no proxy and no fingerprint to change.

use std::time::Duration;

use serde_json::Value;
use tokio::time::{sleep, Instant};

use super::{script, Captcha, Evidence, SolveOptions, Solved};
use crate::error::SeleniumBaseError;
use crate::evasion::behavior::Humanizer;
use crate::evasion::humanize::Rng;
use crate::sb_cdp::human::{plan_click, plan_drag, play_click, play_drag, PointerSink};
use crate::sb_cdp::{Point, Rect};

/// The longest gap between two readings of the page while verifying.
const POLL: Duration = Duration::from_millis(150);

/// The longest gap between two readings of a widget that is settling.
const SAMPLE: Duration = Duration::from_millis(80);

/// How long a widget is given to stand still before it is pressed anyway.
const SETTLE_LIMIT: Duration = Duration::from_secs(4);

/// How long a page that cannot show a result is watched after a press.
const UNVERIFIED_WAIT: Duration = Duration::from_millis(1500);

/// How big the box inside a widget is, in pixels: about a checkbox.
const PRESS_TOLERANCE: f64 = 12.0;

/// The longest wait before a repeated press.
const BACKOFF_CAP: Duration = Duration::from_secs(8);

/// How many readings in a row must find the widget gone before it counts.
/// A widget that failed is drawn again, and that may leave a moment with none.
const GONE_READINGS: u32 = 2;

/// What the solver needs from a browser tab.
///
/// A seam for the Pure CDP [`Page`](crate::sb_cdp::Page) and `BaseCase`: both
/// read the page with a script, move a trusted pointer, and list the cookies.
pub(crate) trait Surface: PointerSink {
    /// Evaluates a script expression in the page and returns its value.
    fn eval(&self, script: &str) -> impl Future<Output = Result<Value, SeleniumBaseError>> + Send;

    /// The names of the browser's cookies.
    fn cookie_names(&self) -> impl Future<Output = Result<Vec<String>, SeleniumBaseError>> + Send;
}

/// A widget as the page script saw it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seen {
    captcha: Captcha,
    area: Rect,
    /// Whether the whole widget is inside the viewport.
    in_view: bool,
}

/// The answer of the locate script.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Probe {
    widget: Option<Seen>,
    /// A challenge is on its way: the page is marked, or a widget element
    /// has no size yet.
    pending: bool,
}

fn parse_probe(value: &Value) -> Probe {
    let found = value.get("widget").filter(|widget| !widget.is_null());
    Probe {
        widget: found.and_then(|widget| {
            let (captcha, area) = Captcha::parse_located(widget)?;
            Some(Seen {
                captcha,
                area,
                in_view: widget
                    .get("inView")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            })
        }),
        pending: value
            .get("pending")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// The signs of a result that the check script read.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "an image of the answer of the check script, which is a set of independent yes/no readings"
)]
struct Check {
    /// How many response fields of the widget the page has.
    fields: u32,
    /// Whether one of them holds a token.
    filled: bool,
    /// Whether the widget shows a success state the page can read.
    success: bool,
    /// Whether the widget is still shown.
    widget: bool,
    /// Whether a challenge is still on its way.
    pending: bool,
    /// The mark this document carries, if it is the one that was marked.
    epoch: Option<String>,
}

impl Check {
    /// `None` when the answer is not an object, so a garbled answer is not
    /// taken for "the widget is gone".
    fn parse(value: &Value) -> Option<Self> {
        value.as_object()?;
        let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);
        Some(Self {
            fields: value
                .get("fields")
                .and_then(Value::as_u64)
                .and_then(|count| u32::try_from(count).ok())
                .unwrap_or(0),
            filled: flag("filled"),
            success: flag("success"),
            widget: flag("widget"),
            pending: flag("pending"),
            epoch: value
                .get("epoch")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

/// What the page looked like when the widget was pressed.
#[derive(Debug, Clone, Default)]
struct Baseline {
    epoch: Option<String>,
    cookies: Vec<String>,
}

/// How a press ended.
#[derive(Debug, Clone, PartialEq)]
enum Verdict {
    Cleared(Evidence),
    /// The page cannot show a result, so none was seen.
    Unverified,
    /// The page could show a result and did not.
    Failed(String),
}

async fn probe<S: Surface>(surface: &S, scroll: bool) -> Result<Probe, SeleniumBaseError> {
    Ok(parse_probe(&surface.eval(&script::probe(scroll)).await?))
}

async fn check<S: Surface>(surface: &S, kind: Captcha) -> Result<Option<Check>, SeleniumBaseError> {
    let value = surface.eval(&script::check(kind.index())).await?;
    Ok(Check::parse(&value))
}

/// The wait between two readings of the page while verifying.
fn poll_interval(options: &SolveOptions) -> Duration {
    (options.verify_timeout / 20).clamp(Duration::from_millis(5), POLL)
}

/// The wait between two readings of a widget that is settling.
fn sample_interval(options: &SolveOptions) -> Duration {
    (options.settle / 3).clamp(Duration::from_millis(5), SAMPLE)
}

/// Whether two readings of a widget are the same place and size, to within
/// half a pixel.
fn same_place(a: Rect, b: Rect) -> bool {
    const EPSILON: f64 = 0.5;
    (a.x - b.x).abs() < EPSILON
        && (a.y - b.y).abs() < EPSILON
        && (a.width - b.width).abs() < EPSILON
        && (a.height - b.height).abs() < EPSILON
}

/// The wait before press number `attempt + 1`: `base` doubled for each press
/// so far, capped, and spread by a quarter either way. A base below a second
/// stays below a second; nothing rounds it to zero.
fn backoff(base: Duration, attempt: u32, rng: &mut Rng) -> Duration {
    let doublings = attempt.saturating_sub(1).min(8);
    let grown = base.saturating_mul(1_u32 << doublings).min(BACKOFF_CAP);
    grown.mul_f64(rng.range(0.75, 1.25))
}

/// The wait before press number `attempt + 1`: the options' retry policy when
/// they have one, otherwise [`backoff`].
fn wait_before(options: &SolveOptions, attempt: u32, rng: &mut Rng) -> Duration {
    match &options.retry_policy {
        Some(policy) => policy.delay_for_attempt(attempt as usize + 1),
        None => backoff(options.backoff, attempt, rng),
    }
}

/// Finds the widget, waiting for one that is on its way.
async fn locate<S: Surface>(
    surface: &S,
    options: &SolveOptions,
) -> Result<Option<Captcha>, SeleniumBaseError> {
    let started = Instant::now();
    loop {
        let found = probe(surface, false).await?;
        if let Some(seen) = found.widget {
            return Ok(Some(seen.captcha));
        }
        if !found.pending || started.elapsed() >= options.locate_timeout {
            return Ok(None);
        }
        sleep(POLL).await;
    }
}

/// Reads the widget until it has stood still, in view, for the settle time.
///
/// Returns the last reading if it never does within the limit, and `None` if
/// the widget is not there to be read.
async fn settle<S: Surface>(
    surface: &S,
    kind: Captcha,
    options: &SolveOptions,
) -> Result<Option<Rect>, SeleniumBaseError> {
    let sample = sample_interval(options);
    let started = Instant::now();
    let mut scroll = true;
    let mut still: Option<(Rect, Instant)> = None;

    loop {
        let seen = probe(surface, scroll)
            .await?
            .widget
            .filter(|seen| seen.captcha == kind);
        let reading = seen.map(|seen| seen.area);
        match seen {
            Some(seen) if seen.in_view => {
                scroll = false;

                match still {
                    Some((area, since)) if same_place(area, seen.area) => {
                        if since.elapsed() >= options.settle {
                            return Ok(Some(seen.area));
                        }
                    }
                    _ => still = Some((seen.area, Instant::now())),
                }
            }
            // Not wholly in the viewport, or not there: scroll to it again.
            _ => {
                scroll = true;
                still = None;
            }
        }
        if started.elapsed() >= SETTLE_LIMIT {
            return Ok(reading);
        }
        sleep(sample).await;
    }
}

/// Moves the pointer to the widget and presses it, or drags the slider.
async fn press<S: Surface>(
    surface: &S,
    person: &mut Humanizer,
    pointer: &mut Option<Point>,
    kind: Captcha,
    area: Rect,
) -> Result<(), SeleniumBaseError> {
    let plan = kind.plan(area);
    let from = if let Some(at) = *pointer {
        at
    } else {
        // The first move of a tab: the pointer is somewhere in the middle of
        // the viewport, not in a corner and not on the widget.
        let size = surface.eval(script::VIEWPORT).await?;
        let axis = |index: usize, fallback: f64| {
            size.get(index).and_then(Value::as_f64).unwrap_or(fallback)
        };
        let start = person.resting_point(axis(0, 1024.0), axis(1, 768.0));
        surface.pointer_move(start, false).await?;
        start
    };
    match plan.release {
        None => {
            let click = plan_click(person, from, plan.press, PRESS_TOLERANCE);
            play_click(surface, &click).await?;
            *pointer = Some(click.landing);
        }
        Some(release) => {
            let drag = plan_drag(person, from, plan.press, release, PRESS_TOLERANCE);
            play_drag(surface, &drag).await?;
            *pointer = Some(drag.release);
        }
    }
    Ok(())
}

/// What a reading shows of a result, if anything.
fn evidence_of(check: &Check, epoch: Option<&str>, gone_readings: u32) -> Option<Evidence> {
    if check.filled {
        return Some(Evidence::Token);
    }
    if check.success {
        return Some(Evidence::WidgetState);
    }
    if !check.widget && !check.pending {
        // The document is a different one from the one that was marked.
        let replaced = epoch.is_some() && check.epoch.as_deref() != epoch;
        if replaced {
            return Some(Evidence::Navigated);
        }
        if gone_readings >= GONE_READINGS {
            return Some(Evidence::WidgetGone);
        }
    }
    None
}

/// Reads the page after a press until it shows a result or the time is up.
async fn verify<S: Surface>(
    surface: &S,
    kind: Captcha,
    options: &SolveOptions,
    baseline: &Baseline,
) -> Result<Verdict, SeleniumBaseError> {
    let started = Instant::now();
    let poll = poll_interval(options);
    let clearance = clearance_cookies(kind, options);
    let mut fields = 0_u32;
    let mut gone = 0_u32;
    let mut last: Option<Check> = None;
    loop {
        // A page that is navigating answers with an error: read it again.
        if let Ok(Some(now)) = check(surface, kind).await {
            fields = fields.max(now.fields);
            gone = if now.widget || now.pending {
                0
            } else {
                gone + 1
            };
            if let Some(evidence) = evidence_of(&now, baseline.epoch.as_deref(), gone) {
                return Ok(Verdict::Cleared(evidence));
            }
            last = Some(now);
        }
        if let Some(expression) = &options.marker {
            let marker = surface.eval(&format!("!!({expression})")).await;
            if matches!(marker, Ok(Value::Bool(true))) {
                return Ok(Verdict::Cleared(Evidence::Marker));
            }
        }
        if !clearance.is_empty()
            && let Ok(cookies) = surface.cookie_names().await
        {
            let arrived = clearance.iter().any(|name| {
                cookies.iter().any(|cookie| cookie == name)
                    && !baseline.cookies.iter().any(|cookie| cookie == name)
            });
            if arrived {
                return Ok(Verdict::Cleared(Evidence::Cookie));
            }
        }

        let verifiable =
            fields > 0 || options.marker.is_some() || options.clearance_cookie.is_some();
        let wait = if verifiable {
            options.verify_timeout
        } else {
            options.verify_timeout.min(UNVERIFIED_WAIT)
        };
        if started.elapsed() >= wait {
            if !verifiable {
                return Ok(Verdict::Unverified);
            }
            let mut reason = if fields > 0 {
                format!("the response field stayed empty for {wait:.1?} after the press")
            } else if options.marker.is_some() {
                format!("the success marker stayed false for {wait:.1?} after the press")
            } else {
                let name = options.clearance_cookie.as_deref().unwrap_or_default();
                format!(
                    "the clearance cookie {name} did not arrive within {wait:.1?} after the press"
                )
            };
            if last.as_ref().is_some_and(|check| check.widget) {
                reason.push_str("; the widget is still shown");
            }
            return Ok(Verdict::Failed(reason));
        }
        sleep(poll).await;
    }
}

/// The cookies whose arrival shows that the visitor was let through: the one the vendor sets
/// and the one the options name.
fn clearance_cookies<'a>(kind: Captcha, options: &'a SolveOptions) -> Vec<&'a str> {
    kind.clearance_cookie()
        .into_iter()
        .chain(options.clearance_cookie.as_deref())
        .collect()
}

/// What a round came to.
enum Round {
    /// The call is over, with this answer: `None` for a page that shows no widget.
    Over(Option<Solved>),
    /// Every press of the round was spent and the page, which could show a result, never did.
    Spent { captcha: Captcha, reason: String },
}

/// What a call keeps from one round to the next.
struct Hand {
    person: Humanizer,
    spread: Rng,
    pointer: Option<Point>,
    /// The presses of all the rounds so far.
    presses: u32,
    /// The widget the last round found.
    widget: Option<Captcha>,
}

/// The answer of a call that cleared (or pressed, or lost) the widget of kind `captcha`.
fn report(
    captcha: Captcha,
    presses: u32,
    round: u32,
    evidence: Option<Evidence>,
    started: Instant,
) -> Solved {
    Solved {
        captcha,
        attempts: presses,
        rounds: round,
        evidence,
        elapsed: started.elapsed(),
    }
}

/// The answer when the widget cannot be read any more: `None` when it was never pressed (the
/// page shows no widget), otherwise the widget went away after a press, which is a result.
fn vanished(hand: &Hand, round: u32, started: Instant) -> Option<Solved> {
    let captcha = hand.widget?;
    (hand.presses > 0).then(|| {
        report(
            captcha,
            hand.presses,
            round,
            Some(Evidence::WidgetGone),
            started,
        )
    })
}

/// Finds a widget, presses it until the page shows it cleared, and says how.
///
/// See the [module documentation](self) for the steps. `None` means the page
/// shows no widget.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Captcha`] when the widget was pressed the
/// allowed number of times and the page, which could show a result, never
/// did. Errors of the browser (a script that fails to run, input it refuses)
/// are returned as they are.
pub(crate) async fn solve<S: Surface>(
    surface: &S,
    options: &SolveOptions,
) -> Result<Option<Solved>, SeleniumBaseError> {
    let started = Instant::now();
    let rounds = options.rounds_allowed();
    let mut hand = Hand {
        person: Humanizer::new(options.behavior),
        spread: Rng::from_entropy(),
        pointer: None,
        presses: 0,
        widget: None,
    };
    let mut round = 1_u32;
    loop {
        match run_round(surface, options, &mut hand, round, started).await? {
            Round::Over(answer) => return Ok(answer),
            Round::Spent { captcha, reason } if round >= rounds => {
                return Err(SeleniumBaseError::captcha(
                    captcha.to_string(),
                    hand.presses,
                    reason,
                ));
            }
            Round::Spent { captcha, reason } => {
                tracing::debug!(
                    widget = %captcha,
                    round,
                    %reason,
                    "the round ended without a result; cooling down before the next one"
                );
                sleep(options.cooldown).await;
                round += 1;
            }
        }
    }
}

/// One pass of the solver: finds the widget, then settles, presses and verifies up to the
/// attempts allowed. Ends the call with a verdict, or reports that the presses were spent.
async fn run_round<S: Surface>(
    surface: &S,
    options: &SolveOptions,
    hand: &mut Hand,
    round: u32,
    started: Instant,
) -> Result<Round, SeleniumBaseError> {
    let Some(kind) = locate(surface, options).await? else {
        // No widget at all, or, in a later round, one that went away.
        return Ok(Round::Over(vanished(hand, round, started)));
    };
    hand.widget = Some(kind);
    let allowed = options.attempts_allowed();
    let mut pressed = 0_u32;
    let mut reason = String::new();

    while pressed < allowed {
        // The mark of this document and the cookies before the press, to tell
        // afterwards what the press changed.
        let epoch = surface.eval(&script::epoch()).await?;
        let baseline = Baseline {
            epoch: epoch.as_str().map(str::to_owned),
            cookies: surface.cookie_names().await.unwrap_or_default(),
        };

        // The widget may already be clear: it solved itself, or an earlier
        // press landed late. Pressing a cleared widget can reset it.
        if let Ok(Some(now)) = check(surface, kind).await
            && now.filled
        {
            let cleared = report(kind, hand.presses, round, Some(Evidence::Token), started);
            return Ok(Round::Over(Some(cleared)));
        }

        let Some(area) = settle(surface, kind, options).await? else {
            // It went away while it was being read.
            return Ok(Round::Over(vanished(hand, round, started)));
        };

        pressed += 1;
        hand.presses += 1;
        tracing::debug!(widget = %kind, attempt = hand.presses, round, "pressing the CAPTCHA widget");
        press(surface, &mut hand.person, &mut hand.pointer, kind, area).await?;
        match verify(surface, kind, options, &baseline).await? {
            Verdict::Cleared(evidence) => {
                let cleared = report(kind, hand.presses, round, Some(evidence), started);
                return Ok(Round::Over(Some(cleared)));
            }
            Verdict::Unverified => {
                let pressed_once = report(kind, hand.presses, round, None, started);
                return Ok(Round::Over(Some(pressed_once)));
            }
            Verdict::Failed(why) => reason = why,
        }
        if pressed < allowed {
            sleep(wait_before(options, pressed, &mut hand.spread)).await;
        }
    }
    Ok(Round::Spent {
        captcha: kind,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::evasion::behavior::Behavior;

    /// One thing the solver did to the scripted tab.
    #[derive(Debug, Clone, PartialEq)]
    enum Event {
        Move { at: Point, held: bool },
        Down(Point),
        Up(Point),
    }

    /// What the solver asked the page.
    #[derive(Debug)]
    enum Ask {
        Probe,
        Check,
        Epoch,
        Viewport,
        Marker,
    }

    type Answer = Box<dyn FnMut(&Ask, &[Event]) -> Value + Send>;

    /// A tab that answers from a closure and records the pointer.
    struct Scripted {
        answer: Mutex<Answer>,
        events: Mutex<Vec<Event>>,
        cookies: Mutex<Vec<String>>,
    }

    impl Scripted {
        fn new(answer: impl FnMut(&Ask, &[Event]) -> Value + Send + 'static) -> Self {
            Self {
                answer: Mutex::new(Box::new(answer)),
                events: Mutex::default(),
                cookies: Mutex::default(),
            }
        }

        fn events(&self) -> Vec<Event> {
            self.events.lock().unwrap().clone()
        }

        fn presses(&self) -> usize {
            self.events()
                .iter()
                .filter(|event| matches!(event, Event::Down(_)))
                .count()
        }

        fn push(&self, event: Event) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "the scripted tab answers at once; the trait is what the solver awaits"
    )]
    impl PointerSink for Scripted {
        async fn pointer_move(&self, at: Point, held: bool) -> Result<(), SeleniumBaseError> {
            self.push(Event::Move { at, held });
            Ok(())
        }

        async fn pointer_down(&self, at: Point) -> Result<(), SeleniumBaseError> {
            self.push(Event::Down(at));
            Ok(())
        }

        async fn pointer_up(&self, at: Point) -> Result<(), SeleniumBaseError> {
            self.push(Event::Up(at));
            Ok(())
        }
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "the scripted tab answers at once; the trait is what the solver awaits"
    )]
    impl Surface for Scripted {
        async fn eval(&self, script: &str) -> Result<Value, SeleniumBaseError> {
            let ask = if script.contains("pending: pending() }") {
                Ask::Probe
            } else if script.contains("const k = DATA.kinds") {
                Ask::Check
            } else if script.contains("__sbCaptcha = Math.random") {
                Ask::Epoch
            } else if script.contains("window.innerWidth") {
                Ask::Viewport
            } else {
                Ask::Marker
            };
            let events = self.events();
            Ok((self.answer.lock().unwrap())(&ask, &events))
        }

        async fn cookie_names(&self) -> Result<Vec<String>, SeleniumBaseError> {
            Ok(self.cookies.lock().unwrap().clone())
        }
    }

    /// How many times the widget has been pressed according to `events`.
    fn presses_in(events: &[Event]) -> usize {
        events
            .iter()
            .filter(|event| matches!(event, Event::Down(_)))
            .count()
    }

    /// Short waits and a fast, seeded pointer, so a scenario runs in a blink
    /// of paused time.
    fn quick() -> SolveOptions {
        SolveOptions::new()
            .settle(Duration::from_millis(30))
            .verify_timeout(Duration::from_millis(400))
            .backoff(Duration::from_millis(20))
            .locate_timeout(Duration::from_millis(500))
            .behavior(
                Behavior::builder()
                    .pointer_speed(5.0)
                    .overshoot(0.0)
                    .seed(11)
                    .build()
                    .unwrap(),
            )
    }

    /// A Turnstile box of 300x65 at (100, 200), in view.
    fn widget() -> Value {
        json!({ "kind": 0, "x": 100.0, "y": 200.0, "width": 300.0, "height": 65.0,
                "inView": true })
    }

    fn probe_of(widget: impl Into<Value>, pending: bool) -> Value {
        json!({ "widget": widget.into(), "pending": pending })
    }

    fn reading(filled: bool, widget: bool, fields: u32) -> Value {
        json!({ "fields": fields, "filled": filled, "success": false, "widget": widget,
                "pending": false, "epoch": "doc-1" })
    }

    /// The answer for everything but the probe and the check.
    fn plain(ask: &Ask) -> Option<Value> {
        match ask {
            Ask::Epoch => Some(json!("doc-1")),
            Ask::Viewport => Some(json!([1000.0, 800.0])),
            Ask::Marker => Some(json!(false)),
            Ask::Probe | Ask::Check => None,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_page_with_no_widget_and_no_challenge_is_left_alone() {
        let tab =
            Scripted::new(|ask, _| plain(ask).unwrap_or_else(|| probe_of(Value::Null, false)));
        let result = solve(&tab, &quick()).await.unwrap();
        assert_eq!(result, None);
        assert!(tab.events().is_empty(), "nothing is pressed or moved");
    }

    #[tokio::test(start_paused = true)]
    async fn a_challenge_that_is_on_its_way_is_waited_for() {
        let mut probes = 0;
        let tab = Scripted::new(move |ask, events| match ask {
            Ask::Probe => {
                probes += 1;
                // Pending for the first few readings, then the widget is drawn.
                if probes < 4 {
                    probe_of(Value::Null, true)
                } else {
                    probe_of(widget(), true)
                }
            }
            Ask::Check => reading(!events.is_empty() && events.len() > 3, true, 1),
            other => plain(other).unwrap(),
        });
        let solved = solve(&tab, &quick()).await.unwrap().expect("a widget");
        assert_eq!(solved.captcha, Captcha::Turnstile);
        assert_eq!(solved.evidence, Some(Evidence::Token));
        assert_eq!(solved.attempts, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_challenge_that_never_draws_is_given_up_after_the_locate_timeout() {
        let tab = Scripted::new(|ask, _| plain(ask).unwrap_or_else(|| probe_of(Value::Null, true)));
        let result = solve(&tab, &quick()).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test(start_paused = true)]
    async fn a_widget_that_is_already_clear_is_not_pressed() {
        let tab = Scripted::new(|ask, _| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(true, true, 1),
            other => plain(other).unwrap(),
        });
        let solved = solve(&tab, &quick()).await.unwrap().expect("a widget");
        assert_eq!(solved.attempts, 0);
        assert_eq!(solved.evidence, Some(Evidence::Token));
        assert!(
            tab.events().is_empty(),
            "a cleared widget can be reset by a press"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_widget_is_pressed_on_its_checkbox_after_a_curved_path() {
        let tab = Scripted::new(|ask, events| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(events.iter().any(|e| matches!(e, Event::Up(_))), true, 1),
            other => plain(other).unwrap(),
        });
        solve(&tab, &quick()).await.unwrap().expect("a widget");

        let events = tab.events();
        let down = events
            .iter()
            .find_map(|event| match event {
                Event::Down(at) => Some(*at),
                _ => None,
            })
            .expect("a press");
        // The checkbox is at the left edge of the box: 100 + 28, mid-height.
        assert!((down.x - 128.0).abs() <= 6.0, "pressed at x = {}", down.x);
        assert!((down.y - 232.5).abs() <= 6.0, "pressed at y = {}", down.y);

        let moves = events
            .iter()
            .filter(|event| matches!(event, Event::Move { .. }))
            .count();
        assert!(moves > 8, "the pointer travelled, in {moves} moves");
        assert!(
            events
                .windows(2)
                .all(|pair| !matches!(pair, [Event::Up(_), Event::Down(_)])),
            "one press"
        );
        // Down comes after the moves and up after down.
        let order: Vec<_> = events
            .iter()
            .map(|event| matches!(event, Event::Move { .. }))
            .collect();
        assert!(order[..order.len() - 2].iter().all(|is_move| *is_move));
    }

    #[tokio::test(start_paused = true)]
    async fn a_widget_that_is_still_moving_is_read_again_until_it_stands_still() {
        let mut probes = 0;
        let tab = Scripted::new(move |ask, events| match ask {
            Ask::Probe => {
                probes += 1;
                // It slides in from the left over the first readings.
                let x = if probes < 6 {
                    10.0 * f64::from(probes)
                } else {
                    300.0
                };
                probe_of(
                    json!({ "kind": 0, "x": x, "y": 200.0, "width": 300.0, "height": 65.0,
                            "inView": true }),
                    false,
                )
            }
            Ask::Check => reading(events.iter().any(|e| matches!(e, Event::Up(_))), true, 1),
            other => plain(other).unwrap(),
        });
        solve(&tab, &quick()).await.unwrap().expect("a widget");
        let down = tab
            .events()
            .iter()
            .find_map(|event| match event {
                Event::Down(at) => Some(*at),
                _ => None,
            })
            .expect("a press");
        assert!(
            (down.x - 328.0).abs() <= 6.0,
            "pressed at x = {}, where the widget was before it settled",
            down.x
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_widget_that_needs_a_second_check_is_pressed_again_after_a_wait() {
        let tab = Scripted::new(|ask, events| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => {
                let presses = events
                    .iter()
                    .filter(|e| matches!(e, Event::Down(_)))
                    .count();
                reading(
                    presses >= 2 && events.iter().any(|e| matches!(e, Event::Up(_))),
                    true,
                    1,
                )
            }
            other => plain(other).unwrap(),
        });
        let solved = solve(&tab, &quick()).await.unwrap().expect("a widget");
        assert_eq!(solved.attempts, 2);
        assert_eq!(solved.evidence, Some(Evidence::Token));
        assert_eq!(tab.presses(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_widget_that_never_clears_ends_in_a_typed_error_after_the_attempts() {
        let tab = Scripted::new(|ask, _| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(false, true, 1),
            other => plain(other).unwrap(),
        });
        let error = solve(&tab, &quick()).await.unwrap_err();
        match &error {
            SeleniumBaseError::Captcha {
                captcha,
                attempts,
                reason,
            } => {
                assert_eq!(captcha, "Cloudflare Turnstile");
                assert_eq!(*attempts, 3);
                assert!(reason.contains("response field stayed empty"), "{reason}");
                assert!(reason.contains("still shown"), "{reason}");
            }
            other => panic!("expected a CAPTCHA error, got {other:?}"),
        }
        assert_eq!(tab.presses(), 3);
        assert_eq!(error.category(), "captcha");
    }

    #[tokio::test(start_paused = true)]
    async fn the_attempts_allowed_bound_the_presses() {
        let tab = Scripted::new(|ask, _| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(false, true, 1),
            other => plain(other).unwrap(),
        });
        let error = solve(&tab, &quick().attempts(1)).await.unwrap_err();
        assert!(matches!(
            error,
            SeleniumBaseError::Captcha { attempts: 1, .. }
        ));
        assert_eq!(tab.presses(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_page_that_cannot_show_a_result_is_pressed_once_and_reported_unverified() {
        let tab = Scripted::new(|ask, _| match ask {
            Ask::Probe => probe_of(widget(), false),
            // No response field on the page, and the widget stays: nothing to read.
            Ask::Check => reading(false, true, 0),
            other => plain(other).unwrap(),
        });
        let solved = solve(&tab, &quick()).await.unwrap().expect("a widget");
        assert_eq!(solved.attempts, 1);
        assert_eq!(solved.evidence, None);
        assert_eq!(
            tab.presses(),
            1,
            "a re-press can reset a widget that worked"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_success_marker_makes_a_page_with_no_field_verifiable() {
        let tab = Scripted::new(|ask, events| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(false, true, 0),
            Ask::Marker => json!(events.iter().any(|e| matches!(e, Event::Up(_)))),
            other => plain(other).unwrap(),
        });
        let solved = solve(&tab, &quick().success_marker("window.done"))
            .await
            .unwrap()
            .expect("a widget");
        assert_eq!(solved.evidence, Some(Evidence::Marker));
    }

    #[tokio::test(start_paused = true)]
    async fn a_clearance_cookie_that_arrives_is_a_result() {
        let tab = std::sync::Arc::new(Scripted::new(|ask, _| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(false, true, 0),
            other => plain(other).unwrap(),
        }));
        let setter = std::sync::Arc::clone(&tab);
        let watcher = tokio::spawn(async move {
            // The cookie arrives while the solver is verifying the press.
            loop {
                if setter.presses() > 0 {
                    setter
                        .cookies
                        .lock()
                        .unwrap()
                        .push("cf_clearance".to_owned());
                    return;
                }
                sleep(Duration::from_millis(5)).await;
            }
        });
        let solved = solve(&*tab, &quick()).await.unwrap().expect("a widget");
        watcher.await.unwrap();
        assert_eq!(solved.evidence, Some(Evidence::Cookie));
    }

    #[tokio::test(start_paused = true)]
    async fn a_round_that_ends_without_a_result_is_followed_by_another_after_the_cooldown() {
        // Two presses to a round; the third press, in the second round, clears it.
        let tab = Scripted::new(|ask, events| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(presses_in(events) >= 3, true, 1),
            other => plain(other).unwrap(),
        });
        let options = quick()
            .attempts(2)
            .rounds(2)
            .cooldown(Duration::from_secs(30));
        let started = Instant::now();
        let solved = solve(&tab, &options).await.unwrap().expect("a widget");
        assert_eq!(solved.rounds, 2);
        assert_eq!(solved.attempts, 3, "the presses of all rounds are counted");
        assert_eq!(solved.evidence, Some(Evidence::Token));
        assert_eq!(tab.presses(), 3);
        assert!(
            started.elapsed() >= Duration::from_secs(30),
            "the cooldown passed between the rounds: {:?}",
            started.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn one_round_is_the_default_and_a_cooldown_is_not_waited_for_after_the_last() {
        let tab = Scripted::new(|ask, _| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(false, true, 1),
            other => plain(other).unwrap(),
        });
        let started = Instant::now();
        let error = solve(
            &tab,
            &quick().attempts(1).cooldown(Duration::from_secs(600)),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            SeleniumBaseError::Captcha { attempts: 1, .. }
        ));
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "no round is to come, so nothing cools down: {:?}",
            started.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_rounds_allowed_bound_the_presses_and_the_error_counts_them_all() {
        let tab = Scripted::new(|ask, _| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(false, true, 1),
            other => plain(other).unwrap(),
        });
        let options = quick()
            .attempts(2)
            .rounds(3)
            .cooldown(Duration::from_secs(10));
        let started = Instant::now();
        let error = solve(&tab, &options).await.unwrap_err();
        match &error {
            SeleniumBaseError::Captcha { attempts, .. } => assert_eq!(*attempts, 6),
            other => panic!("expected a CAPTCHA error, got {other:?}"),
        }
        assert_eq!(tab.presses(), 6);
        assert!(
            started.elapsed() >= Duration::from_secs(20),
            "two cooldowns between three rounds: {:?}",
            started.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_widget_gone_when_the_next_round_begins_is_a_result() {
        // The widget is there until it is pressed, then it is gone for good.
        let tab = Scripted::new(|ask, events| match ask {
            Ask::Probe if events.is_empty() => probe_of(widget(), false),
            Ask::Probe => probe_of(Value::Null, false),
            Ask::Check => reading(false, true, 1),
            other => plain(other).unwrap(),
        });
        let options = quick().attempts(1).rounds(2);
        let solved = solve(&tab, &options).await.unwrap().expect("a widget");
        assert_eq!(solved.evidence, Some(Evidence::WidgetGone));
        assert_eq!(solved.attempts, 1);
        assert_eq!(solved.rounds, 2);
        assert_eq!(solved.captcha, Captcha::Turnstile);
    }

    /// Sets the cookie `name` in `tab` once the widget has been pressed.
    fn set_cookie_after_the_press(
        tab: &std::sync::Arc<Scripted>,
        name: &'static str,
    ) -> tokio::task::JoinHandle<()> {
        let setter = std::sync::Arc::clone(tab);
        tokio::spawn(async move {
            loop {
                if setter.presses() > 0 {
                    setter.cookies.lock().unwrap().push(name.to_owned());
                    return;
                }
                sleep(Duration::from_millis(5)).await;
            }
        })
    }

    /// A tab whose page has no response field, so only a cookie can show a result.
    fn tab_with_no_field() -> Scripted {
        Scripted::new(|ask, _| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check => reading(false, true, 0),
            other => plain(other).unwrap(),
        })
    }

    #[tokio::test(start_paused = true)]
    async fn a_named_clearance_cookie_that_arrives_is_a_result() {
        let tab = std::sync::Arc::new(tab_with_no_field());
        let watcher = set_cookie_after_the_press(&tab, "site_ok");
        let options = quick().clearance_cookie("site_ok");
        let solved = solve(&*tab, &options).await.unwrap().expect("a widget");
        watcher.await.unwrap();
        assert_eq!(solved.evidence, Some(Evidence::Cookie));
        assert_eq!(solved.attempts, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn the_vendors_clearance_cookie_still_counts_beside_a_named_one() {
        let tab = std::sync::Arc::new(tab_with_no_field());
        let watcher = set_cookie_after_the_press(&tab, "cf_clearance");
        let options = quick().clearance_cookie("site_ok");
        let solved = solve(&*tab, &options).await.unwrap().expect("a widget");
        watcher.await.unwrap();
        assert_eq!(solved.evidence, Some(Evidence::Cookie));
    }

    #[tokio::test(start_paused = true)]
    async fn a_named_clearance_cookie_lets_a_press_without_it_be_pressed_again_and_then_fail() {
        // Without the name this page cannot show a result: one press, "unverified".
        let tab = tab_with_no_field();
        let solved = solve(&tab, &quick()).await.unwrap().expect("a widget");
        assert_eq!(solved.evidence, None);
        assert_eq!(tab.presses(), 1);

        let tab = tab_with_no_field();
        let options = quick().attempts(2).clearance_cookie("site_ok");
        let error = solve(&tab, &options).await.unwrap_err();
        match &error {
            SeleniumBaseError::Captcha {
                attempts, reason, ..
            } => {
                assert_eq!(*attempts, 2);
                assert!(reason.contains("site_ok"), "{reason}");
            }
            other => panic!("expected a CAPTCHA error, got {other:?}"),
        }
        assert_eq!(tab.presses(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_named_clearance_cookie_that_was_there_before_the_press_is_not_a_result() {
        let tab = tab_with_no_field();
        tab.cookies.lock().unwrap().push("site_ok".to_owned());
        let options = quick().attempts(1).clearance_cookie("site_ok");
        let error = solve(&tab, &options).await.unwrap_err();
        assert!(matches!(error, SeleniumBaseError::Captcha { .. }));
    }

    #[tokio::test(start_paused = true)]

    async fn a_page_replaced_by_one_with_no_widget_is_a_result() {
        let tab = Scripted::new(|ask, events| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check if events.iter().any(|e| matches!(e, Event::Up(_))) => json!({
                "fields": 0, "filled": false, "success": false, "widget": false,
                "pending": false, "epoch": null,
            }),
            Ask::Check => reading(false, true, 1),
            other => plain(other).unwrap(),
        });
        let solved = solve(&tab, &quick()).await.unwrap().expect("a widget");
        assert_eq!(solved.evidence, Some(Evidence::Navigated));
    }

    #[tokio::test(start_paused = true)]
    async fn a_widget_gone_for_one_reading_only_is_not_yet_a_result() {
        let mut readings_after_press = 0;
        let tab = Scripted::new(move |ask, events| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check if events.iter().any(|e| matches!(e, Event::Up(_))) => {
                readings_after_press += 1;
                // Gone once (it is being drawn again), then back for good.
                if readings_after_press == 1 {
                    reading(false, false, 1)
                } else {
                    reading(false, true, 1)
                }
            }
            Ask::Check => reading(false, true, 1),
            other => plain(other).unwrap(),
        });
        let error = solve(&tab, &quick().attempts(1)).await.unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::Captcha { .. }),
            "{error:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_widget_gone_for_two_readings_is_a_result() {
        let tab = Scripted::new(|ask, events| match ask {
            Ask::Probe => probe_of(widget(), false),
            Ask::Check if events.iter().any(|e| matches!(e, Event::Up(_))) => {
                reading(false, false, 1)
            }
            Ask::Check => reading(false, true, 1),
            other => plain(other).unwrap(),
        });
        let solved = solve(&tab, &quick()).await.unwrap().expect("a widget");
        assert_eq!(solved.evidence, Some(Evidence::WidgetGone));
    }

    #[tokio::test(start_paused = true)]
    async fn the_slider_is_dragged_with_the_button_held() {
        let tab = Scripted::new(|ask, events| match ask {
            Ask::Probe => probe_of(
                json!({ "kind": 4, "x": 100.0, "y": 200.0, "width": 300.0, "height": 65.0,
                        "inView": true }),
                false,
            ),
            Ask::Check => reading(false, true, 0),
            Ask::Marker => json!(events.iter().any(|e| matches!(e, Event::Up(_)))),
            other => plain(other).unwrap(),
        });
        solve(&tab, &quick().success_marker("window.done"))
            .await
            .unwrap()
            .expect("a widget");
        let events = tab.events();
        let held = events
            .iter()
            .filter(|event| matches!(event, Event::Move { held: true, .. }))
            .count();
        assert!(held > 5, "the handle was dragged in {held} moves");
        let Some(Event::Up(release)) = events.last() else {
            panic!("the drag ends with a release: {events:?}");
        };
        assert!(
            (release.x - 370.0).abs() < 1.0,
            "released at x = {}",
            release.x
        );
    }

    #[test]
    fn the_backoff_grows_is_capped_and_never_rounds_to_nothing() {
        let mut rng = Rng::new(5);
        let base = Duration::from_millis(100);
        let first = backoff(base, 1, &mut rng);
        assert!(first >= Duration::from_millis(75) && first <= Duration::from_millis(125));
        let third = backoff(base, 3, &mut rng);
        assert!(third >= Duration::from_millis(300) && third <= Duration::from_millis(500));
        let huge = backoff(base, 30, &mut rng);
        assert!(huge <= BACKOFF_CAP.mul_f64(1.25));
        assert!(backoff(Duration::from_millis(1), 1, &mut rng) > Duration::ZERO);
    }

    #[test]
    fn a_retry_policy_sets_the_waits_with_no_spread() {
        let policy = crate::utilities::retry::RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(100),
            backoff_factor: 3.0,
            max_delay: Duration::from_millis(500),
        };
        let options = SolveOptions::new().retry_policy(policy);
        let mut rng = Rng::new(3);
        let waits: Vec<Duration> = (1..=4)
            .map(|attempt| wait_before(&options, attempt, &mut rng))
            .collect();
        assert_eq!(
            waits,
            [100, 300, 500, 500].map(Duration::from_millis),
            "the policy's own schedule, capped, the same on every run"
        );
        let without = SolveOptions::new().backoff(Duration::from_millis(100));
        let doubled = wait_before(&without, 3, &mut rng);
        assert!(doubled >= Duration::from_millis(300) && doubled <= Duration::from_millis(500));
    }

    #[test]

    fn the_backoff_is_spread_so_it_is_not_on_a_clock() {
        let mut rng = Rng::new(9);
        let waits: std::collections::HashSet<_> = (0..20)
            .map(|_| backoff(Duration::from_secs(1), 2, &mut rng))
            .collect();
        assert!(waits.len() > 10, "{waits:?}");
    }

    #[test]
    fn readings_are_the_same_place_to_half_a_pixel() {
        let a = Rect {
            x: 10.0,
            y: 10.0,
            width: 300.0,
            height: 65.0,
        };
        assert!(same_place(a, Rect { x: 10.3, ..a }));
        assert!(!same_place(a, Rect { x: 11.0, ..a }));
        assert!(!same_place(a, Rect { height: 66.0, ..a }));
    }

    #[test]
    fn a_garbled_check_is_not_taken_for_a_missing_widget() {
        assert_eq!(Check::parse(&json!(null)), None);
        assert_eq!(Check::parse(&json!("no")), None);
        let check = Check::parse(&json!({})).expect("an object");
        assert!(!check.widget && !check.filled);
    }

    #[test]
    fn a_probe_reads_the_widget_whether_or_not_it_is_in_view() {
        let probe = parse_probe(&json!({ "widget": widget(), "pending": true }));
        assert!(probe.pending);
        assert_eq!(
            probe.widget.map(|seen| seen.captcha),
            Some(Captcha::Turnstile)
        );
        let out = parse_probe(&json!({
            "widget": { "kind": 1, "x": 0, "y": 0, "width": 1, "height": 1, "inView": false },
            "pending": false,
        }));
        assert_eq!(out.widget.map(|seen| seen.in_view), Some(false));
        assert_eq!(parse_probe(&json!({ "widget": null })).widget, None);
    }

    #[test]
    fn evidence_is_read_from_a_check_in_order_of_strength() {
        let mut check = Check::parse(&reading(true, true, 1)).unwrap();
        assert_eq!(evidence_of(&check, Some("doc-1"), 0), Some(Evidence::Token));
        check.filled = false;
        check.success = true;
        assert_eq!(
            evidence_of(&check, Some("doc-1"), 0),
            Some(Evidence::WidgetState)
        );
        check.success = false;
        assert_eq!(evidence_of(&check, Some("doc-1"), 0), None);
        check.widget = false;
        assert_eq!(evidence_of(&check, Some("doc-1"), 1), None);
        assert_eq!(
            evidence_of(&check, Some("doc-1"), 2),
            Some(Evidence::WidgetGone)
        );
        check.epoch = None;
        assert_eq!(
            evidence_of(&check, Some("doc-1"), 0),
            Some(Evidence::Navigated)
        );
        // Without a mark to compare, a replaced document cannot be told.
        assert_eq!(evidence_of(&check, None, 0), None);
    }
}
