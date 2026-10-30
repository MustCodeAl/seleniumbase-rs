//! Clicking a CAPTCHA with the operating system's pointer (feature `gui`).
//!
//! [`Page::solve_captcha`] clicks with DevTools input events, so the pointer
//! of the machine never moves. Some challenge frames refuse events that did
//! not come from the operating system. [`Page::gui_click_captcha`] is for
//! those: it finds the widget the same way, works out where its checkbox is on
//! the screen, and moves the real pointer there along a human path.
//!
//! The screen point is built from four measurements, none of which needs the
//! page's cooperation:
//!
//! 1. the widget's frame in viewport coordinates (the script of
//!    [`Captcha`]);
//! 2. the window's position and size, from `Browser.getWindowForTarget`;
//! 3. the viewport's size, from the page, which gives how much of the window
//!    is toolbar and frame and so where the page starts on the screen;
//! 4. where each vendor draws its checkbox inside its frame.
//!
//! The click goes to whatever window is on top at that point, and it is only
//! as exact as those measurements: a page zoom other than 100 percent, a
//! scaled display on Windows, or a docked browser panel shift it.
//! [`Page::gui_captcha_plan`] does steps 1 to 4 and nothing else, so the
//! arithmetic can be checked without touching the pointer.

use std::time::Duration;

use enigo::{Enigo, Mouse, Settings};
use serde::Deserialize;

use super::{Captcha, Page, Point, Rect, WindowBounds, WindowState};
use crate::api::gui::Gui;
use crate::error::SeleniumBaseError;
use crate::evasion::behavior::{Behavior, Humanizer};

/// How wide the target of the pointer is, in pixels: about a checkbox.
///
/// A smaller target takes the pointer longer to reach, as in Fitts's law.
const TARGET_WIDTH: f64 = 28.0;

/// How long to wait after the window was asked to come to the front.
const FOCUS_PAUSE: Duration = Duration::from_millis(120);

/// A script returning what [`ViewMetrics`] reads.
const VIEW_METRICS_SCRIPT: &str = "({ innerWidth: window.innerWidth, \
    innerHeight: window.innerHeight, \
    headless: /Headless/.test(navigator.userAgent) })";

/// What the page says about its own viewport.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewMetrics {
    inner_width: f64,
    inner_height: f64,
    /// A headless browser has no window on the screen to click.
    headless: bool,
}

/// Where the operating system's pointer goes to attempt a CAPTCHA widget.
///
/// The points are in screen pixels, with the origin at the top-left of the
/// main display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickPlan {
    /// The widget the plan is for.
    pub captcha: Captcha,
    /// Where to press: the checkbox, or the handle of a slider.
    pub press: (i32, i32),
    /// Where to let go, for a slider; `None` for a checkbox, which is clicked.
    pub release: Option<(i32, i32)>,
}

/// How the browser window sits on the screen, and how big its page is.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Geometry {
    /// The window as `Browser.getWindowForTarget` reports it, frame included.
    window: WindowBounds,
    inner_width: f64,
    inner_height: f64,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "window coordinates are far below 2^52, so they are exact as f64"
)]
fn exact(value: i64) -> f64 {
    value as f64
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the value is rounded and clamped into the range of i32 first"
)]
fn pixel(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn pixels(point: Point) -> (i32, i32) {
    (pixel(point.x), pixel(point.y))
}

impl Geometry {
    /// Where the top-left of the page is on the screen.
    ///
    /// What the window has beyond the page is the browser's own chrome. The
    /// side frames split the horizontal part evenly, and the bottom frame is
    /// taken to be as thick as a side frame; the rest of the vertical part is
    /// the toolbar above the page. A window with no frame (macOS) has only
    /// that toolbar, and a fullscreen or headless window has nothing.
    fn page_origin(&self) -> Point {
        let side = (exact(self.window.width) - self.inner_width).max(0.0) / 2.0;
        let toolbar = (exact(self.window.height) - self.inner_height - side).max(0.0);
        Point {
            x: exact(self.window.x) + side,
            y: exact(self.window.y) + toolbar,
        }
    }

    /// The screen position of a point given in viewport coordinates.
    fn screen_point(&self, at: Point) -> Point {
        let origin = self.page_origin();
        Point {
            x: origin.x + at.x,
            y: origin.y + at.y,
        }
    }
}

/// How far right of and below a widget's top-left corner its checkbox is
/// centred, in CSS pixels, or `None` for the slider, which has no checkbox.
///
/// The values are where each vendor draws the box: they are the offsets the
/// Python implementation uses. Windows draws the frames a little differently
/// for Turnstile and reCAPTCHA.
fn checkbox_offset(captcha: Captcha, windows: bool) -> Option<(f64, f64)> {
    match captcha {
        Captcha::Turnstile if windows => Some((27.0, 29.0)),
        Captcha::Turnstile => Some((25.0, 32.0)),
        Captcha::Recaptcha if windows => Some((29.0, 35.0)),
        Captcha::Recaptcha => Some((26.0, 35.0)),
        Captcha::Hcaptcha => Some((30.0, 36.0)),
        Captcha::FriendlyCaptcha => Some((27.0, 34.0)),
        Captcha::DataDome => None,
    }
}

/// Whether a reCAPTCHA frame at `area` is the invisible badge, which floats
/// in the bottom-right corner of the viewport and has no checkbox to click.
fn looks_like_invisible_recaptcha(area: Rect, inner_width: f64, inner_height: f64) -> bool {
    area.x > 1040.0
        && area.y > 640.0
        && (inner_width - area.x).abs() < 110.0
        && (inner_height - area.y).abs() < 110.0
}

/// Where the pointer goes for a `captcha` widget occupying `area` (in
/// viewport coordinates).
fn click_plan(captcha: Captcha, area: Rect, geometry: &Geometry, windows: bool) -> ClickPlan {
    let (press, release) = if let Some((dx, dy)) = checkbox_offset(captcha, windows) {
        // Never outside a widget that is smaller than the offset.
        let press = Point {
            x: area.x + dx.min(area.width / 2.0),
            y: area.y + dy.min(area.height / 2.0),
        };
        (press, None)
    } else {
        let slide = captcha.plan(area);
        (slide.press, slide.release)
    };
    ClickPlan {
        captcha,
        press: pixels(geometry.screen_point(press)),
        release: release.map(|point| pixels(geometry.screen_point(point))),
    }
}

/// Fails unless `point` is on a display of `size` pixels.
///
/// A click outside the screen would land on nothing or on another monitor, so
/// it is refused, as Python's `PyAutoGUI` refuses it.
fn ensure_on_screen(
    point: (i32, i32),
    size: (i32, i32),
    what: &str,
) -> Result<(), SeleniumBaseError> {
    let (x, y) = point;
    if x < 0 || y < 0 || x > size.0 || y > size.1 {
        return Err(SeleniumBaseError::gui(format!(
            "the {what} point ({x}, {y}) is outside the {} by {} screen; \
             move the browser window onto the main display",
            size.0, size.1
        )));
    }
    Ok(())
}

/// What `move_and_click` needs from the pointer of the machine.
///
/// A seam: the real implementation moves the user's pointer, so the logic
/// above it is tested against a recording fake.
trait Pointer {
    /// The size of the main display in pixels.
    fn screen_size(&self) -> Result<(i32, i32), SeleniumBaseError>;
    /// Where the pointer is now, if the system says.
    fn location(&self) -> Option<(i32, i32)>;
    fn move_to(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError>;
    fn press(&mut self) -> Result<(), SeleniumBaseError>;
    fn release(&mut self) -> Result<(), SeleniumBaseError>;
    fn drag(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), SeleniumBaseError>;
    /// Waits between two steps of a gesture.
    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// The real pointer: [`Gui`] moves and presses, and a second connection
/// reads where the pointer is, which [`Gui`] does not offer.
struct OsPointer {
    gui: Gui,
    reader: Option<Enigo>,
}

impl OsPointer {
    fn new() -> Result<Self, SeleniumBaseError> {
        Ok(Self {
            gui: Gui::new()?,
            reader: Enigo::new(&Settings::default()).ok(),
        })
    }
}

impl Pointer for OsPointer {
    fn screen_size(&self) -> Result<(i32, i32), SeleniumBaseError> {
        self.gui.screen_size()
    }

    fn location(&self) -> Option<(i32, i32)> {
        self.reader
            .as_ref()
            .and_then(|reader| Mouse::location(reader).ok())
    }

    fn move_to(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
        self.gui.move_mouse(x, y)
    }

    fn press(&mut self) -> Result<(), SeleniumBaseError> {
        self.gui.mouse_down()
    }

    fn release(&mut self) -> Result<(), SeleniumBaseError> {
        self.gui.mouse_up()
    }

    fn drag(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), SeleniumBaseError> {
        self.gui.drag(from, to)
    }
}

/// Moves the pointer to the plan's press point along a human path, then
/// clicks, or drags to the release point for a slider.
///
/// Nothing moves if a point of the plan is off the screen.
fn move_and_click(
    pointer: &mut impl Pointer,
    person: &mut Humanizer,
    plan: &ClickPlan,
) -> Result<(), SeleniumBaseError> {
    let size = pointer.screen_size()?;
    ensure_on_screen(plan.press, size, "press")?;
    if let Some(release) = plan.release {
        ensure_on_screen(release, size, "release")?;
    }

    let target = Point {
        x: f64::from(plan.press.0),
        y: f64::from(plan.press.1),
    };
    let start = pointer.location().map_or_else(
        || person.resting_point(f64::from(size.0), f64::from(size.1)),
        |(x, y)| Point {
            x: f64::from(x),
            y: f64::from(y),
        },
    );
    for step in person.mouse_path(start, target, TARGET_WIDTH) {
        pointer.pause(step.wait);
        let (x, y) = pixels(step.at);
        pointer.move_to(x.clamp(0, size.0), y.clamp(0, size.1))?;
    }
    // The pointer has to be exactly there, whatever the path ended on.
    pointer.move_to(plan.press.0, plan.press.1)?;
    pointer.pause(person.reaction());

    if let Some(release) = plan.release {
        pointer.drag(plan.press, release)
    } else {
        pointer.press()?;
        pointer.pause(person.click_dwell());
        pointer.release()
    }
}

impl Page {
    /// Where the operating system's pointer would go to attempt the first
    /// supported CAPTCHA widget on the page, without moving it.
    ///
    /// The widget is scrolled to the middle of the viewport first. Returns
    /// `None` if the page shows no supported widget, or only the invisible
    /// reCAPTCHA badge, which has nothing to click.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Gui`] if the browser is headless or its
    /// window is minimised, since there is no window on the screen to click,
    /// and [`SeleniumBaseError::CdpDriver`] if the page or the window cannot
    /// be measured.
    pub async fn gui_captcha_plan(&self) -> Result<Option<ClickPlan>, SeleniumBaseError> {
        let found = self.evaluate(Captcha::locate_script()).await?;
        let Some((captcha, area)) = Captcha::parse_located(&found) else {
            return Ok(None);
        };

        let view: ViewMetrics = self.evaluate_as(VIEW_METRICS_SCRIPT).await?;
        if view.headless {
            return Err(SeleniumBaseError::gui(
                "a headless browser has no window on the screen to click; \
                 launch it with a window, or use `solve_captcha`",
            ));
        }
        if captcha == Captcha::Recaptcha
            && looks_like_invisible_recaptcha(area, view.inner_width, view.inner_height)
        {
            return Ok(None);
        }

        let window = self.window().bounds().await?;
        if window.state == WindowState::Minimized {
            return Err(SeleniumBaseError::gui(
                "the browser window is minimised, so there is nothing on the screen to click",
            ));
        }
        let geometry = Geometry {
            window,
            inner_width: view.inner_width,
            inner_height: view.inner_height,
        };
        Ok(Some(click_plan(
            captcha,
            area,
            &geometry,
            cfg!(target_os = "windows"),
        )))
    }

    /// Attempts the first supported CAPTCHA widget with the operating
    /// system's pointer: the checkbox is clicked, and the `DataDome` slider is
    /// dragged.
    ///
    /// This is [`solve_captcha`](Self::solve_captcha) for challenge frames
    /// that refuse events sent over DevTools. It brings the window to the
    /// front, works out where the widget is on the screen (see
    /// [`gui_captcha_plan`](Self::gui_captcha_plan)), moves the real pointer
    /// there along a human path and presses. **It takes over the mouse of the
    /// machine for about a second**, and the click goes to whatever window is
    /// on top at that point. Whether the challenge is then accepted is up to
    /// the site; check the page afterwards.
    ///
    /// Returns the widget that was attempted, or `None` if the page shows no
    /// supported widget.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Gui`] if there is no desktop session, the
    /// system refuses the input, the browser is headless or minimised, or the
    /// checkbox is off the main display, and [`SeleniumBaseError::CdpDriver`]
    /// if the page cannot be measured.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::sb_cdp::Page;
    ///
    /// # async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// if let Some(widget) = page.gui_click_captcha().await? {
    ///     println!("clicked {widget}");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn gui_click_captcha(&self) -> Result<Option<Captcha>, SeleniumBaseError> {
        self.click_captcha_with(OsPointer::new).await
    }

    /// [`gui_click_captcha`](Self::gui_click_captcha) with the pointer
    /// supplied by `make_pointer`, which runs on a blocking thread because
    /// the pointer need not be `Send`.
    async fn click_captcha_with<P: Pointer>(
        &self,
        make_pointer: impl FnOnce() -> Result<P, SeleniumBaseError> + Send + 'static,
    ) -> Result<Option<Captcha>, SeleniumBaseError> {
        self.bring_to_front().await?;
        tokio::time::sleep(FOCUS_PAUSE).await;
        let Some(plan) = self.gui_captcha_plan().await? else {
            return Ok(None);
        };
        tokio::task::spawn_blocking(move || {
            let mut pointer = make_pointer()?;
            let mut person = Humanizer::new(Behavior::default());
            move_and_click(&mut pointer, &mut person, &plan)
        })
        .await
        .map_err(|error| {
            SeleniumBaseError::gui(format!("the pointer task did not finish: {error}"))
        })??;
        Ok(Some(plan.captcha))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::{json, Value};

    use super::*;
    use crate::sb_cdp::{Browser, MockCtrl};

    fn bounds(x: i64, y: i64, width: i64, height: i64) -> WindowBounds {
        WindowBounds {
            x,
            y,
            width,
            height,
            state: WindowState::Normal,
        }
    }

    fn geometry(window: WindowBounds, inner_width: f64, inner_height: f64) -> Geometry {
        Geometry {
            window,
            inner_width,
            inner_height,
        }
    }

    const AREA: Rect = Rect {
        x: 100.0,
        y: 200.0,
        width: 300.0,
        height: 65.0,
    };

    // ------------------------------------------------------------------
    // The arithmetic
    // ------------------------------------------------------------------

    #[test]
    fn a_framed_window_has_its_page_inside_the_side_frames_and_below_the_toolbar() {
        // 8px frames on both sides and at the bottom, a 92px toolbar.
        let window = geometry(bounds(100, 50, 1200, 900), 1184.0, 800.0);

        assert_eq!(window.page_origin(), Point { x: 108.0, y: 142.0 });
    }

    #[test]
    fn a_window_without_a_frame_has_only_a_toolbar() {
        // The macOS case: outer and inner width agree.
        let window = geometry(bounds(0, 25, 1440, 900), 1440.0, 813.0);

        assert_eq!(window.page_origin(), Point { x: 0.0, y: 112.0 });
    }

    #[test]
    fn a_fullscreen_window_starts_the_page_at_its_origin() {
        let window = geometry(bounds(0, 0, 1920, 1080), 1920.0, 1080.0);

        assert_eq!(window.page_origin(), Point { x: 0.0, y: 0.0 });
    }

    #[test]
    fn a_page_larger_than_its_window_never_moves_the_origin_back() {
        // A stale measurement: inner larger than outer.
        let window = geometry(bounds(10, 20, 800, 600), 900.0, 700.0);

        assert_eq!(window.page_origin(), Point { x: 10.0, y: 20.0 });
    }

    #[test]
    fn a_window_on_a_monitor_to_the_left_keeps_its_negative_origin() {
        let window = geometry(bounds(-1920, -30, 1920, 1080), 1920.0, 1000.0);

        let at = window.screen_point(Point { x: 10.0, y: 20.0 });

        assert_eq!(
            at,
            Point {
                x: -1910.0,
                y: 70.0
            }
        );
    }

    #[test]
    fn a_viewport_point_is_the_page_origin_plus_the_point() {
        let window = geometry(bounds(100, 50, 1200, 900), 1184.0, 800.0);

        let at = window.screen_point(Point { x: 125.0, y: 232.0 });

        assert_eq!(at, Point { x: 233.0, y: 374.0 });
    }

    #[test]
    fn every_vendor_has_its_own_checkbox_offset_and_the_slider_has_none() {
        let offsets: Vec<_> = Captcha::ALL
            .iter()
            .map(|kind| checkbox_offset(*kind, false))
            .collect();

        assert_eq!(
            offsets,
            [
                Some((25.0, 32.0)),
                Some((26.0, 35.0)),
                Some((30.0, 36.0)),
                Some((27.0, 34.0)),
                None,
            ]
        );
    }

    #[test]
    fn windows_draws_turnstile_and_recaptcha_a_little_differently() {
        assert_eq!(
            checkbox_offset(Captcha::Turnstile, true),
            Some((27.0, 29.0))
        );
        assert_eq!(
            checkbox_offset(Captcha::Recaptcha, true),
            Some((29.0, 35.0))
        );
        for kind in [Captcha::Hcaptcha, Captcha::FriendlyCaptcha] {
            assert_eq!(
                checkbox_offset(kind, true),
                checkbox_offset(kind, false),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_checkbox_plan_is_the_page_origin_plus_the_frame_plus_the_vendor_offset() {
        let window = geometry(bounds(100, 50, 1200, 900), 1184.0, 800.0);

        let plan = click_plan(Captcha::Turnstile, AREA, &window, false);

        // Origin (108, 142) + frame (100, 200) + offset (25, 32).
        assert_eq!(
            plan,
            ClickPlan {
                captcha: Captcha::Turnstile,
                press: (233, 374),
                release: None,
            }
        );
    }

    #[test]
    fn each_vendor_is_pressed_at_its_own_offset() {
        let window = geometry(bounds(0, 0, 1000, 800), 1000.0, 800.0);
        let area = Rect {
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 80.0,
        };

        let presses: Vec<_> = [
            Captcha::Turnstile,
            Captcha::Recaptcha,
            Captcha::Hcaptcha,
            Captcha::FriendlyCaptcha,
        ]
        .map(|kind| click_plan(kind, area, &window, false).press)
        .to_vec();

        assert_eq!(presses, [(35, 52), (36, 55), (40, 56), (37, 54)]);
    }

    #[test]
    fn a_widget_narrower_than_its_offset_is_pressed_inside_it() {
        let window = geometry(bounds(0, 0, 1000, 800), 1000.0, 800.0);
        let tiny = Rect {
            x: 50.0,
            y: 60.0,
            width: 20.0,
            height: 10.0,
        };

        let plan = click_plan(Captcha::Hcaptcha, tiny, &window, false);

        assert_eq!(plan.press, (60, 65), "no further than the widget's middle");
    }

    #[test]
    fn the_slider_is_pressed_at_its_handle_and_released_across_its_width() {
        let window = geometry(bounds(100, 50, 1200, 900), 1184.0, 800.0);

        let plan = click_plan(Captcha::DataDome, AREA, &window, false);

        // Origin (108, 142) + frame (100, 200) + handle at 30 and 270 across,
        // half way down the 65px.
        assert_eq!(plan.press, (238, 375));
        assert_eq!(plan.release, Some((478, 375)));
    }

    #[test]
    fn the_invisible_recaptcha_badge_is_told_apart_by_its_corner() {
        // The badge is 256 wide and sticks out of the right edge, so only its
        // left 70 pixels show; it sits 14 pixels above the bottom.
        let badge = Rect {
            x: 1210.0,
            y: 646.0,
            width: 256.0,
            height: 60.0,
        };
        assert!(looks_like_invisible_recaptcha(badge, 1280.0, 720.0));

        // The checkbox of a login form is nowhere near the corner.
        assert!(!looks_like_invisible_recaptcha(AREA, 1280.0, 720.0));
        // A big window with a widget in the middle of it.
        let middle = Rect {
            x: 1100.0,
            y: 700.0,
            ..badge
        };
        assert!(!looks_like_invisible_recaptcha(middle, 2400.0, 1400.0));
    }

    #[test]
    fn a_point_off_the_screen_is_refused_with_its_coordinates() {
        let screen = (1440, 900);

        assert!(ensure_on_screen((0, 0), screen, "press").is_ok());
        assert!(ensure_on_screen((1440, 900), screen, "press").is_ok());
        for point in [(-1, 10), (10, -1), (1441, 10), (10, 901)] {
            let error = ensure_on_screen(point, screen, "press")
                .unwrap_err()
                .to_string();
            assert!(
                error.contains(&format!("({}, {})", point.0, point.1)),
                "{error}"
            );
            assert!(error.contains("1440 by 900"), "{error}");
        }
    }

    #[test]
    fn pixels_round_to_the_nearest_whole_pixel_and_never_overflow() {
        assert_eq!(pixels(Point { x: 10.4, y: 10.6 }), (10, 11));
        assert_eq!(pixels(Point { x: -0.4, y: -1.5 }), (0, -2));
        assert_eq!(pixels(Point { x: 1e12, y: -1e12 }), (i32::MAX, i32::MIN));
    }

    // ------------------------------------------------------------------
    // The pointer, against a recording fake
    // ------------------------------------------------------------------

    #[derive(Debug, Clone, PartialEq)]
    enum Event {
        Move(i32, i32),
        Press,
        Release,
        Drag((i32, i32), (i32, i32)),
        Pause(Duration),
    }

    #[derive(Clone)]
    struct Fake {
        events: Arc<Mutex<Vec<Event>>>,
        screen: (i32, i32),
        at: Option<(i32, i32)>,
        /// Fails the move with this index.
        fail_move: Option<usize>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                events: Arc::default(),
                screen: (1440, 900),
                at: Some((20, 20)),
                fail_move: None,
            }
        }

        fn events(&self) -> Vec<Event> {
            self.events.lock().unwrap().clone()
        }

        fn moves(&self) -> Vec<(i32, i32)> {
            self.events()
                .into_iter()
                .filter_map(|event| match event {
                    Event::Move(x, y) => Some((x, y)),
                    _ => None,
                })
                .collect()
        }

        fn log(&self, event: Event) {
            self.events.lock().unwrap().push(event);
        }
    }

    impl Pointer for Fake {
        fn screen_size(&self) -> Result<(i32, i32), SeleniumBaseError> {
            Ok(self.screen)
        }

        fn location(&self) -> Option<(i32, i32)> {
            self.at
        }

        fn move_to(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
            if self.fail_move == Some(self.moves().len()) {
                return Err(SeleniumBaseError::gui("the pointer is gone"));
            }
            self.log(Event::Move(x, y));
            Ok(())
        }

        fn press(&mut self) -> Result<(), SeleniumBaseError> {
            self.log(Event::Press);
            Ok(())
        }

        fn release(&mut self) -> Result<(), SeleniumBaseError> {
            self.log(Event::Release);
            Ok(())
        }

        fn drag(&mut self, from: (i32, i32), to: (i32, i32)) -> Result<(), SeleniumBaseError> {
            self.log(Event::Drag(from, to));
            Ok(())
        }

        fn pause(&mut self, duration: Duration) {
            self.log(Event::Pause(duration));
        }
    }

    fn person() -> Humanizer {
        Humanizer::new(Behavior::builder().seed(7).build().unwrap())
    }

    fn checkbox_plan() -> ClickPlan {
        ClickPlan {
            captcha: Captcha::Turnstile,
            press: (700, 400),
            release: None,
        }
    }

    #[test]
    fn the_pointer_travels_to_the_checkbox_and_then_clicks_it() {
        let mut fake = Fake::new();

        move_and_click(&mut fake, &mut person(), &checkbox_plan()).unwrap();

        let events = fake.events();
        let moves = fake.moves();
        assert!(moves.len() > 5, "a path, not a jump: {} moves", moves.len());
        assert_eq!(moves.last(), Some(&(700, 400)), "ends on the checkbox");
        // The button goes down once, after the last move, and up after that.
        let last_move = events
            .iter()
            .rposition(|event| matches!(event, Event::Move(..)))
            .unwrap();
        let press = events.iter().position(|e| *e == Event::Press).unwrap();
        let release = events.iter().position(|e| *e == Event::Release).unwrap();
        assert!(last_move < press && press < release, "{events:?}");
        assert_eq!(events.iter().filter(|e| **e == Event::Press).count(), 1);
        assert!(
            events[press..release]
                .iter()
                .any(|event| matches!(event, Event::Pause(wait) if *wait > Duration::ZERO)),
            "the button is held for a moment: {events:?}"
        );
    }

    #[test]
    fn the_path_starts_where_the_pointer_is() {
        let mut fake = Fake::new();
        fake.at = Some((300, 500));

        move_and_click(&mut fake, &mut person(), &checkbox_plan()).unwrap();

        let first = fake.moves()[0];
        let distance = f64::from(first.0 - 300).hypot(f64::from(first.1 - 500));
        assert!(distance < 60.0, "starts near (300, 500), not at {first:?}");
    }

    #[test]
    fn an_unknown_pointer_position_still_gives_a_path() {
        let mut fake = Fake::new();
        fake.at = None;

        move_and_click(&mut fake, &mut person(), &checkbox_plan()).unwrap();

        assert_eq!(fake.moves().last(), Some(&(700, 400)));
    }

    #[test]
    fn no_step_leaves_the_screen_even_when_the_target_is_at_its_edge() {
        let mut fake = Fake::new();
        let plan = ClickPlan {
            press: (1439, 899),
            ..checkbox_plan()
        };
        fake.at = Some((0, 0));

        move_and_click(&mut fake, &mut person(), &plan).unwrap();

        for (x, y) in fake.moves() {
            assert!(
                (0..=1440).contains(&x) && (0..=900).contains(&y),
                "({x}, {y})"
            );
        }
    }

    #[test]
    fn a_slider_is_dragged_after_the_pointer_arrives_at_its_handle() {
        let mut fake = Fake::new();
        let plan = ClickPlan {
            captcha: Captcha::DataDome,
            press: (500, 400),
            release: Some((740, 400)),
        };

        move_and_click(&mut fake, &mut person(), &plan).unwrap();

        let events = fake.events();
        assert_eq!(fake.moves().last(), Some(&(500, 400)));
        assert_eq!(events.last(), Some(&Event::Drag((500, 400), (740, 400))));
        assert!(!events.contains(&Event::Press), "the drag presses itself");
    }

    #[test]
    fn a_plan_off_the_screen_moves_nothing() {
        for plan in [
            ClickPlan {
                press: (-5, 400),
                ..checkbox_plan()
            },
            ClickPlan {
                captcha: Captcha::DataDome,
                press: (100, 100),
                release: Some((5000, 100)),
            },
        ] {
            let mut fake = Fake::new();

            let error = move_and_click(&mut fake, &mut person(), &plan).unwrap_err();

            assert!(matches!(error, SeleniumBaseError::Gui(_)), "{error}");
            assert!(fake.events().is_empty(), "{:?}", fake.events());
        }
    }

    #[test]
    fn a_pointer_that_fails_mid_path_is_not_pressed() {
        let mut fake = Fake::new();
        fake.fail_move = Some(3);

        let error = move_and_click(&mut fake, &mut person(), &checkbox_plan()).unwrap_err();

        assert!(error.to_string().contains("the pointer is gone"), "{error}");
        assert!(!fake.events().contains(&Event::Press));
    }

    // ------------------------------------------------------------------
    // The DevTools side, against a mocked browser
    // ------------------------------------------------------------------

    /// What the page answers: the widget script gets `widget`, the viewport
    /// script gets `view`.
    fn script_page(mock: &MockCtrl, widget: &Value, view: &Value) {
        let (widget, view) = (widget.clone(), view.clone());
        mock.on("Runtime.evaluate", move |params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            let value = if expression.contains("querySelectorAll") {
                widget.clone()
            } else if expression.contains("navigator.userAgent") {
                view.clone()
            } else {
                Value::Null
            };
            Ok(json!({ "result": { "value": value } }))
        });
    }

    fn script_window(mock: &MockCtrl, state: &str) {
        let state = state.to_owned();
        mock.on("Browser.getWindowForTarget", move |_| {
            Ok(json!({
                "windowId": 4,
                "bounds": {
                    "left": 100, "top": 50, "width": 1200, "height": 900,
                    "windowState": state,
                },
            }))
        });
    }

    fn turnstile_widget() -> Value {
        // Kind 0 is Turnstile; the frame is AREA.
        json!({ "kind": 0, "x": 100.0, "y": 200.0, "width": 300.0, "height": 65.0 })
    }

    fn headed_view() -> Value {
        json!({ "innerWidth": 1184.0, "innerHeight": 800.0, "headless": false })
    }

    async fn page(mock_setup: impl FnOnce(&MockCtrl)) -> (Page, MockCtrl) {
        let (browser, mock) = Browser::new_mocked();
        mock_setup(&mock);
        let page = browser.default_page().await.unwrap();
        (page.with_timeout(Duration::from_millis(150)), mock)
    }

    #[tokio::test]
    async fn the_plan_for_a_widget_uses_its_frame_the_window_and_the_viewport() {
        let (page, mock) = page(|mock| {
            script_page(mock, &turnstile_widget(), &headed_view());
            script_window(mock, "normal");
        })
        .await;

        let plan = page.gui_captcha_plan().await.unwrap().unwrap();

        assert_eq!(
            plan,
            ClickPlan {
                captcha: Captcha::Turnstile,
                press: (233, 374),
                release: None,
            }
        );
        let asked = mock.calls_to("Browser.getWindowForTarget");
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].params["targetId"], "T1");
        assert!(
            mock.calls_to("Input.dispatchMouseEvent").is_empty(),
            "planning sends no input"
        );
    }

    #[tokio::test]
    async fn a_page_without_a_widget_has_no_plan_and_asks_nothing_more() {
        let (page, mock) = page(|mock| {
            script_page(mock, &Value::Null, &headed_view());
            script_window(mock, "normal");
        })
        .await;

        assert_eq!(page.gui_captcha_plan().await.unwrap(), None);
        assert!(mock.calls_to("Browser.getWindowForTarget").is_empty());
    }

    #[tokio::test]
    async fn a_headless_browser_is_refused_because_it_has_no_window_to_click() {
        let (page, _mock) = page(|mock| {
            script_page(
                mock,
                &turnstile_widget(),
                &json!({ "innerWidth": 1184.0, "innerHeight": 800.0, "headless": true }),
            );
            script_window(mock, "normal");
        })
        .await;

        let error = page.gui_captcha_plan().await.unwrap_err();

        assert!(matches!(error, SeleniumBaseError::Gui(_)), "{error}");
        assert!(error.to_string().contains("headless"), "{error}");
    }

    #[tokio::test]
    async fn a_minimised_window_is_refused() {
        let (page, _mock) = page(|mock| {
            script_page(mock, &turnstile_widget(), &headed_view());
            script_window(mock, "minimized");
        })
        .await;

        let error = page.gui_captcha_plan().await.unwrap_err();

        assert!(error.to_string().contains("minimised"), "{error}");
    }

    #[tokio::test]
    async fn the_invisible_recaptcha_badge_has_no_plan() {
        let badge = json!({ "kind": 1, "x": 1210.0, "y": 646.0, "width": 256.0, "height": 60.0 });
        let (page, _mock) = page(|mock| {
            script_page(
                mock,
                &badge,
                &json!({ "innerWidth": 1280.0, "innerHeight": 720.0, "headless": false }),
            );
            script_window(mock, "normal");
        })
        .await;

        assert_eq!(page.gui_captcha_plan().await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_browser_that_cannot_report_its_window_is_a_cdp_error() {
        let (page, _mock) = page(|mock| {
            script_page(mock, &turnstile_widget(), &headed_view());
            mock.fail("Browser.getWindowForTarget", "no such window");
        })
        .await;

        let error = page.gui_captcha_plan().await.unwrap_err();

        assert!(matches!(error, SeleniumBaseError::CdpDriver(_)), "{error}");
    }

    #[tokio::test]
    async fn the_click_brings_the_window_forward_then_plays_the_plan_on_the_pointer() {
        let (page, mock) = page(|mock| {
            script_page(mock, &turnstile_widget(), &headed_view());
            script_window(mock, "normal");
        })
        .await;
        let fake = Fake::new();
        let for_thread = fake.clone();

        let clicked = page
            .click_captcha_with(move || Ok(for_thread))
            .await
            .unwrap();

        assert_eq!(clicked, Some(Captcha::Turnstile));
        let order: Vec<_> = mock
            .calls()
            .into_iter()
            .map(|call| call.method)
            .filter(|method| {
                method == "Page.bringToFront" || method == "Browser.getWindowForTarget"
            })
            .collect();
        assert_eq!(order, ["Page.bringToFront", "Browser.getWindowForTarget"]);
        assert_eq!(fake.moves().last(), Some(&(233, 374)));
        assert!(fake.events().contains(&Event::Press));
        assert!(
            mock.calls_to("Input.dispatchMouseEvent").is_empty(),
            "the DevTools mouse is not used"
        );
    }

    #[tokio::test]
    async fn a_page_without_a_widget_leaves_the_pointer_alone() {
        let (page, _mock) = page(|mock| {
            script_page(mock, &Value::Null, &headed_view());
            script_window(mock, "normal");
        })
        .await;
        let fake = Fake::new();
        let for_thread = fake.clone();

        let clicked = page
            .click_captcha_with(move || Ok(for_thread))
            .await
            .unwrap();

        assert_eq!(clicked, None);
        assert!(fake.events().is_empty());
    }

    #[tokio::test]
    async fn no_desktop_session_is_reported_and_nothing_is_clicked() {
        let (page, _mock) = page(|mock| {
            script_page(mock, &turnstile_widget(), &headed_view());
            script_window(mock, "normal");
        })
        .await;

        let error = page
            .click_captcha_with(|| -> Result<Fake, _> { Err(SeleniumBaseError::gui("no display")) })
            .await
            .unwrap_err();

        assert!(matches!(error, SeleniumBaseError::Gui(_)), "{error}");
        assert!(error.to_string().contains("no display"), "{error}");
    }
}
