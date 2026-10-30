//! Switching into frames and windows, and handling alerts, as Python's
//! `switch_to_frame`, `frame_switch`, `switch_to_window` and `switch_to_alert`
//! do.
//!
//! Each call takes Python's required arguments positionally and its keyword
//! arguments as builder methods, and `.await` runs it (see [`crate::api::call`]):
//!
//! ```no_run
//! use std::time::Duration;
//! use seleniumbase_rs::BaseCase;
//!
//! # async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! sb.switch_to_frame("iframeResult").await?;           // the id or the name of a frame
//! sb.switch_to_frame("iframe[title*=\"Pay\"]").await?;  // or a selector (CSS or XPath)
//! sb.switch_to_frame(0).await?;                         // or its position, from 0
//! sb.switch_to_parent_frame().await?;
//!
//! // Python's `with self.frame_switch(frame):`. The frame is left when the
//! // closure ends, also when it fails.
//! sb.frame_switch("#payment")
//!     .timeout(Duration::from_secs(5))
//!     .run(async |sb| {
//!         sb.click("button#pay").await?;
//!         sb.assert_text("Paid", "h1").await
//!     })
//!     .await?;
//!
//! sb.switch_to_window(0).await?;                        // the first tab; -1 is the newest
//! let alert = sb.switch_to_alert().await?;              // waits for the alert
//! let text = alert.text().await?;
//! alert.accept().await?;
//! # let _ = text;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::future::IntoFuture;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use thirtyfour::WebDriver;

use crate::api::base_case::BaseCase;
use crate::api::call::CallFuture;
use crate::api::recorder::ActionRecorder;
use crate::error::SeleniumBaseError;

/// The frame Python's `switch_to_frame(frame)` is given: a selector, the `id`
/// or the `name` of the frame, or its position.
///
/// A `&str` or a `String` becomes [`FrameRef::Locator`], an integer
/// [`FrameRef::Index`], so a call is written as in Python:
/// `sb.switch_to_frame("iframeResult")`, `sb.switch_to_frame(0)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameRef {
    /// A selector (CSS, XPath, `link=` and so on), or the `id` or the `name` of
    /// the frame. The `id` and the `name` are tried first, as Python does, then
    /// the selector.
    Locator(String),
    /// The position of the frame among the frames of the current document,
    /// counting from 0.
    Index(u16),
}

impl From<&str> for FrameRef {
    fn from(locator: &str) -> Self {
        Self::Locator(locator.to_owned())
    }
}

impl From<&String> for FrameRef {
    fn from(locator: &String) -> Self {
        Self::Locator(locator.clone())
    }
}

impl From<String> for FrameRef {
    fn from(locator: String) -> Self {
        Self::Locator(locator)
    }
}

impl From<u16> for FrameRef {
    fn from(index: u16) -> Self {
        Self::Index(index)
    }
}

impl fmt::Display for FrameRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locator(locator) => f.write_str(locator),
            Self::Index(index) => write!(f, "{index}"),
        }
    }
}

/// The window Python's `switch_to_window(window)` is given: a window handle,
/// or the position of the window among the open ones.
///
/// A `&str` or a `String` becomes [`WindowRef::Handle`], an integer
/// [`WindowRef::Index`]: `sb.switch_to_window(0)` is the first tab and
/// `sb.switch_to_window(-1)` the newest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowRef {
    /// A window handle, as [`BaseCase::get_current_window_handle`] and
    /// [`BaseCase::get_all_window_handles`] give them.
    Handle(String),
    /// The position of the window in [`BaseCase::get_all_window_handles`],
    /// counting from 0. A negative number counts from the end, as a Python
    /// list index does: `-1` is the newest window, `-2` the one before.
    Index(i32),
}

impl WindowRef {
    /// The handle of the window this names among `handles`, if it is open.
    pub(crate) fn pick(&self, handles: &[String]) -> Option<String> {
        match self {
            Self::Handle(handle) => handles.contains(handle).then(|| handle.clone()),
            Self::Index(index) => {
                let position = if *index >= 0 {
                    usize::try_from(*index).ok()?
                } else {
                    handles
                        .len()
                        .checked_sub(usize::try_from(index.unsigned_abs()).ok()?)?
                };
                handles.get(position).cloned()
            }
        }
    }
}

impl From<&str> for WindowRef {
    fn from(handle: &str) -> Self {
        Self::Handle(handle.to_owned())
    }
}

impl From<&String> for WindowRef {
    fn from(handle: &String) -> Self {
        Self::Handle(handle.clone())
    }
}

impl From<String> for WindowRef {
    fn from(handle: String) -> Self {
        Self::Handle(handle)
    }
}

impl From<i32> for WindowRef {
    fn from(index: i32) -> Self {
        Self::Index(index)
    }
}

impl From<usize> for WindowRef {
    fn from(index: usize) -> Self {
        Self::Index(i32::try_from(index).unwrap_or(i32::MAX))
    }
}

impl fmt::Display for WindowRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handle(handle) => f.write_str(handle),
            Self::Index(index) => write!(f, "{index}"),
        }
    }
}

/// Python's `switch_to_frame(frame, timeout=None, invisible=False)`: wait for
/// a frame and switch into it.
///
/// It comes from [`BaseCase::switch_to_frame`]. The keywords are
/// [`timeout`](Self::timeout) and [`invisible`](Self::invisible).
#[must_use = "nothing happens until the call is awaited"]
pub struct SwitchToFrame<'a> {
    sb: &'a mut BaseCase,
    frame: FrameRef,
    timeout: Option<Duration>,
    invisible: bool,
}

impl<'a> SwitchToFrame<'a> {
    pub(crate) fn new(sb: &'a mut BaseCase, frame: FrameRef) -> Self {
        Self {
            sb,
            frame,
            timeout: None,
            invisible: false,
        }
    }

    /// How long to wait for the frame. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Whether a frame that is not displayed may be entered when it is
    /// selected by a selector. Python's `invisible`, `false` by default. A frame
    /// given by its `id`, its `name` or its position is entered whether or not it
    /// is displayed.
    pub fn invisible(mut self, invisible: bool) -> Self {
        self.invisible = invisible;
        self
    }
}

impl<'a> IntoFuture for SwitchToFrame<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                frame,
                timeout,
                invisible,
            } = self;
            sb.run_switch_to_frame(&frame, timeout, invisible).await
        })
    }
}

/// Python's `with self.frame_switch(frame, timeout=None):`: switch into a
/// frame, run a block of steps in it, and leave the frame again.
///
/// It comes from [`BaseCase::frame_switch`]. The one keyword is
/// [`timeout`](Self::timeout). Nothing happens until [`run`](Self::run) is
/// awaited.
#[must_use = "nothing happens until `run` is called with the steps to do in the frame"]
pub struct FrameSwitch<'a> {
    sb: &'a mut BaseCase,
    frame: FrameRef,
    timeout: Option<Duration>,
}

impl<'a> FrameSwitch<'a> {
    pub(crate) fn new(sb: &'a mut BaseCase, frame: FrameRef) -> Self {
        Self {
            sb,
            frame,
            timeout: None,
        }
    }

    /// How long to wait for the frame. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Switches into the frame, runs `steps` with the case, and goes back to
    /// the parent frame, whether `steps` succeeded or not. Returns what `steps`
    /// returned.
    ///
    /// Blocks nest: each leaves only the frame it entered.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the frame does not appear
    /// in time; `steps` is not run then. Otherwise returns the error of
    /// `steps`, or, when `steps` succeeded, the error of leaving the frame.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let heading = sb
    ///     .frame_switch("iframe#content")
    ///     .run(async |sb| sb.get_text("h1").await)
    ///     .await?;
    /// // Back in the page that holds the frame.
    /// sb.assert_text("Welcome", "#top").await?;
    /// # let _ = heading;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run<T, F>(self, steps: F) -> Result<T, SeleniumBaseError>
    where
        F: AsyncFnOnce(&mut BaseCase) -> Result<T, SeleniumBaseError>,
    {
        let Self { sb, frame, timeout } = self;
        sb.run_frame_switch(&frame, timeout, steps).await
    }
}

/// Python's `switch_to_window(window, timeout=None)`: wait for a window and
/// switch to it.
///
/// It comes from [`BaseCase::switch_to_window`]. The one keyword is
/// [`timeout`](Self::timeout).
#[must_use = "nothing happens until the call is awaited"]
pub struct SwitchToWindow<'a> {
    sb: &'a BaseCase,
    window: WindowRef,
    timeout: Option<Duration>,
}

impl<'a> SwitchToWindow<'a> {
    pub(crate) fn new(sb: &'a BaseCase, window: WindowRef) -> Self {
        Self {
            sb,
            window,
            timeout: None,
        }
    }

    /// How long to wait for the window. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl<'a> IntoFuture for SwitchToWindow<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                window,
                timeout,
            } = self;
            sb.run_switch_to_window(&window, timeout).await
        })
    }
}

/// Python's `switch_to_alert(timeout=None)`: wait for an alert, confirm box or
/// prompt, and get a handle on it.
///
/// It comes from [`BaseCase::switch_to_alert`]. The one keyword is
/// [`timeout`](Self::timeout). Awaiting it gives the [`Alert`].
#[must_use = "nothing happens until the call is awaited"]
pub struct SwitchToAlert<'a> {
    sb: &'a BaseCase,
    timeout: Option<Duration>,
}

impl<'a> SwitchToAlert<'a> {
    pub(crate) fn new(sb: &'a BaseCase) -> Self {
        Self { sb, timeout: None }
    }

    /// How long to wait for the alert. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl<'a> IntoFuture for SwitchToAlert<'a> {
    type Output = Result<Alert, SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, Alert>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self { sb, timeout } = self;
            sb.run_switch_to_alert(timeout).await
        })
    }
}

/// An alert, confirm box or prompt that is open in the browser, as Python's
/// `switch_to_alert()` returns it.
///
/// It is a handle on the browser's current prompt and does not borrow the
/// case, so the case can go on while the alert is held. Once the alert is
/// accepted or dismissed, the handle's calls fail: get a new one for the next
/// alert.
#[derive(Clone)]
pub struct Alert {
    driver: WebDriver,
    recorder: Arc<Mutex<ActionRecorder>>,
}

impl fmt::Debug for Alert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Alert").finish_non_exhaustive()
    }
}

impl Alert {
    pub(crate) fn new(driver: WebDriver, recorder: Arc<Mutex<ActionRecorder>>) -> Self {
        Self { driver, recorder }
    }

    /// Records the action for a recording of the case, as the case's own
    /// `accept_alert` and `dismiss_alert` do.
    fn record(&self, name: &str, value: Option<&str>) {
        if let Ok(mut recorder) = self.recorder.lock() {
            recorder.record(name, value, None);
        }
    }

    /// The text of the alert. Python's `alert.text`.
    ///
    /// # Errors
    ///
    /// Returns the driver's error if no alert is open.
    pub async fn text(&self) -> Result<String, SeleniumBaseError> {
        self.driver
            .get_alert_text()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Accepts the alert (clicks OK). Python's `alert.accept()`.
    ///
    /// # Errors
    ///
    /// Returns the driver's error if no alert is open.
    pub async fn accept(&self) -> Result<(), SeleniumBaseError> {
        self.record("accept_alert", None);
        self.driver
            .accept_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Dismisses the alert (clicks Cancel). Python's `alert.dismiss()`.
    ///
    /// # Errors
    ///
    /// Returns the driver's error if no alert is open.
    pub async fn dismiss(&self) -> Result<(), SeleniumBaseError> {
        self.record("dismiss_alert", None);
        self.driver
            .dismiss_alert()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Types `text` into the prompt. Python's `alert.send_keys(text)`.
    ///
    /// # Errors
    ///
    /// Returns the driver's error if no prompt is open.
    pub async fn send_keys(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("type_alert_text", Some(text));
        self.driver
            .send_alert_text(text)
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameRef, WindowRef};

    fn handles() -> Vec<String> {
        ["w1", "w2", "w3"].map(str::to_owned).to_vec()
    }

    #[test]
    fn a_window_index_counts_from_the_start_or_from_the_end() {
        let handles = handles();
        let pick = |window: WindowRef| window.pick(&handles);
        assert_eq!(pick(0.into()).as_deref(), Some("w1"));
        assert_eq!(pick(2.into()).as_deref(), Some("w3"));
        assert_eq!(pick(3.into()), None);
        assert_eq!(pick((-1).into()).as_deref(), Some("w3"));
        assert_eq!(pick((-3).into()).as_deref(), Some("w1"));
        assert_eq!(pick((-4).into()), None);
        assert_eq!(pick(i32::MIN.into()), None);
    }

    #[test]
    fn a_window_handle_is_only_found_while_it_is_open() {
        let handles = handles();
        assert_eq!(WindowRef::from("w2").pick(&handles).as_deref(), Some("w2"));
        assert_eq!(WindowRef::from("w9").pick(&handles), None);
        assert_eq!(WindowRef::from("w2").pick(&[]), None);
    }

    #[test]
    fn a_frame_or_window_is_named_as_it_is_written() {
        // The calls are written as in Python: a string, or a number.
        assert_eq!(
            FrameRef::from("iframeResult"),
            FrameRef::Locator("iframeResult".into())
        );
        assert_eq!(
            FrameRef::from(String::from("#a")),
            FrameRef::Locator("#a".into())
        );
        assert_eq!(FrameRef::from(1), FrameRef::Index(1));
        assert_eq!(WindowRef::from("abc"), WindowRef::Handle("abc".into()));
        assert_eq!(WindowRef::from(-1), WindowRef::Index(-1));
        let position: usize = 2;
        assert_eq!(WindowRef::from(position), WindowRef::Index(2));
        assert_eq!(FrameRef::Index(3).to_string(), "3");
        assert_eq!(WindowRef::Index(-1).to_string(), "-1");
    }
}
