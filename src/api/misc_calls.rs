//! Call builders for helpers whose Python signatures take keyword arguments:
//! the hover-and-click pair, `click_nth_visible_element`, the arrow keys,
//! `scroll_up` and `scroll_down`, the offset clicks and drags,
//! `wait_for_attribute`, `open_new_tab`, `add_cookie`, `download_file` and
//! `save_file_as`.
//!
//! The required arguments stay positional, in Python's order. Each keyword
//! argument is a builder method with Python's name, and `.await` runs the call
//! (see [`crate::api::call`]):
//!
//! ```no_run
//! use std::time::Duration;
//! use seleniumbase_rs::api::base_case::BaseCase;
//! use seleniumbase_rs::browser::config::BrowserConfig;
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let mut sb = BaseCase::new(BrowserConfig::default()).await?;
//! sb.open("https://example.com").await?;
//! sb.click_nth_visible_element("a", 2).await?;                  // the second link: 1-based
//! sb.press_down_arrow().times(3).await?;                        // 3 times, on `body`
//! sb.scroll_down().amount(50.0).await?;                         // half a window
//! sb.click_with_offset("h1", 10, 5).center(true).await?;        // from the element's centre
//! sb.wait_for_attribute("#app", "data-ready").value("yes").timeout(Duration::from_secs(5)).await?;
//! sb.open_new_tab().switch_to(false).await?;                    // stay on this tab
//! let saved = sb.download_file("https://example.com/a.csv").destination_folder("out").await?;
//! # let _ = saved;
//! # Ok(())
//! # }
//! ```

use std::future::IntoFuture;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::api::base_case::{BaseCase, CookieExpiry};
use crate::api::call::CallFuture;
use crate::error::SeleniumBaseError;

impl From<bool> for CookieExpiry {
    /// Python's `expiry=False` is [`CookieExpiry::Session`] and `expiry=True` is
    /// [`CookieExpiry::Day`].
    fn from(expiry: bool) -> Self {
        if expiry {
            Self::Day
        } else {
            Self::Session
        }
    }
}

/// Which click [`HoverAndClick`] makes after the hover.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HoverClick {
    /// `hover_and_double_click`.
    /// A double click.
    Double,
    /// `hover_and_js_click`.
    /// A click made by a script.
    Js,
}

/// The arrow key [`PressArrow`] presses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arrow {
    /// The Up arrow.
    Up,
    /// The Down arrow.
    Down,
    /// The Left arrow.
    Left,
    /// The Right arrow.
    Right,
}

/// How far a scroll of `percent` of a window `window_height` pixels high moves
/// the page: negative going up, as Python's `scroll_up` and `scroll_down` work
/// it out.
pub(crate) fn scroll_distance(window_height: f64, percent: f64, up: bool) -> f64 {
    let pixels = window_height * percent / 100.0;
    if up {
        -pixels
    } else {
        pixels
    }
}

/// The keyword arguments of `click_with_offset` and `double_click_with_offset`
/// that are not the selector and the offset.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OffsetOptions {
    /// Draw a dot at the click on a canvas; `None` is "on in demo mode".
    pub(crate) mark: Option<bool>,
    /// How long to wait for the element; `None` is the default timeout.
    pub(crate) timeout: Option<Duration>,
    /// Count the offset from the centre of the element; `None` is `false`.
    pub(crate) center: Option<bool>,
    /// Scroll to the element first.
    pub(crate) scroll: bool,
}

/// Python's `hover_and_double_click(hover_selector, click_selector, timeout=None)`
/// and `hover_and_js_click(...)`: hover over the first element, then
/// double-click (or JavaScript-click) the second, which usually appears on the
/// hover.
///
/// It comes from [`BaseCase::hover_and_double_click`] and
/// [`BaseCase::hover_and_js_click`]. The one keyword is
/// [`timeout`](Self::timeout), how long to wait for each element.
#[must_use = "nothing happens until the call is awaited"]
pub struct HoverAndClick<'a> {
    sb: &'a mut BaseCase,
    hover_selector: &'a str,
    click_selector: &'a str,
    kind: HoverClick,
    timeout: Option<Duration>,
}

impl<'a> HoverAndClick<'a> {
    pub(crate) fn new(
        sb: &'a mut BaseCase,
        hover_selector: &'a str,
        click_selector: &'a str,
        kind: HoverClick,
    ) -> Self {
        Self {
            sb,
            hover_selector,
            click_selector,
            kind,
            timeout: None,
        }
    }

    /// How long to wait for each element. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl<'a> IntoFuture for HoverAndClick<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                hover_selector,
                click_selector,
                kind,
                timeout,
            } = self;
            sb.run_hover_and_click(hover_selector, click_selector, kind, timeout)
                .await
        })
    }
}

/// Python's `click_nth_visible_element(selector, number, by, timeout=None,
/// scroll=True)`: click the `number`th visible match, counting from 1.
///
/// It comes from [`BaseCase::click_nth_visible_element`]. The keywords are
/// [`timeout`](Self::timeout) and [`scroll`](Self::scroll) (default on). A
/// `number` of 0 is an [`SeleniumBaseError::InvalidConfig`] error: the count
/// starts at 1, so a call written for the old 0-based count fails instead of
/// clicking the wrong element.
#[must_use = "nothing happens until the call is awaited"]
pub struct ClickNth<'a> {
    sb: &'a mut BaseCase,
    selector: &'a str,
    number: usize,
    timeout: Option<Duration>,
    scroll: bool,
}

impl<'a> ClickNth<'a> {
    pub(crate) fn new(sb: &'a mut BaseCase, selector: &'a str, number: usize) -> Self {
        Self {
            sb,
            selector,
            number,
            timeout: None,
            scroll: true,
        }
    }

    /// How long to wait for the elements. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Whether to scroll to the element first. Python's `scroll`.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }
}

impl<'a> IntoFuture for ClickNth<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                selector,
                number,
                timeout,
                scroll,
            } = self;
            sb.run_click_nth(selector, number, timeout, scroll).await
        })
    }
}

/// Python's `press_up_arrow(selector="body", times=1)` and its three
/// siblings: send an arrow key to an element, one or more times in a row.
///
/// It comes from [`BaseCase::press_up_arrow`] (and `down`, `left`, `right`).
/// The keywords are [`selector`](Self::selector) and [`times`](Self::times);
/// 0 times does nothing.
#[must_use = "nothing happens until the call is awaited"]
pub struct PressArrow<'a> {
    sb: &'a BaseCase,
    arrow: Arrow,
    selector: &'a str,
    times: u32,
}

impl<'a> PressArrow<'a> {
    pub(crate) fn new(sb: &'a BaseCase, arrow: Arrow) -> Self {
        Self {
            sb,
            arrow,
            selector: "body",
            times: 1,
        }
    }

    /// The element that gets the key (default `body`). Python's `selector`.
    pub fn selector(mut self, selector: &'a str) -> Self {
        self.selector = selector;
        self
    }

    /// How many times in a row to press the key (default 1). Python's `times`.
    pub fn times(mut self, times: u32) -> Self {
        self.times = times;
        self
    }
}

impl<'a> IntoFuture for PressArrow<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.sb
                .run_press_arrow(self.arrow, self.selector, self.times)
                .await
        })
    }
}

/// Python's `scroll_down(amount=25)` and `scroll_up(amount=25)`: scroll by a
/// percentage of the window's height.
///
/// It comes from [`BaseCase::scroll_down`] and [`BaseCase::scroll_up`]. The
/// one keyword is [`amount`](Self::amount), a percentage (default 25).
#[must_use = "nothing happens until the call is awaited"]
pub struct Scroll<'a> {
    sb: &'a BaseCase,
    up: bool,
    amount: f64,
}

impl<'a> Scroll<'a> {
    pub(crate) fn new(sb: &'a BaseCase, up: bool) -> Self {
        Self {
            sb,
            up,
            amount: 25.0,
        }
    }

    /// How far to scroll, as a percentage of the window's height (default 25).
    /// Python's `amount`.
    pub fn amount(mut self, amount: impl Into<f64>) -> Self {
        self.amount = amount.into();
        self
    }
}

impl<'a> IntoFuture for Scroll<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.sb.run_scroll(self.up, self.amount).await })
    }
}

/// Python's `drag_and_drop_with_offset(selector, x, y, by, timeout=None)`: drag
/// an element by `x` pixels right and `y` pixels down.
///
/// It comes from [`BaseCase::drag_and_drop_with_offset`]. The one keyword is
/// [`timeout`](Self::timeout).
#[must_use = "nothing happens until the call is awaited"]
pub struct DragByOffset<'a> {
    sb: &'a mut BaseCase,
    selector: &'a str,
    x: i64,
    y: i64,
    timeout: Option<Duration>,
}

impl<'a> DragByOffset<'a> {
    pub(crate) fn new(sb: &'a mut BaseCase, selector: &'a str, x: i64, y: i64) -> Self {
        Self {
            sb,
            selector,
            x,
            y,
            timeout: None,
        }
    }

    /// How long to wait for the element to be visible. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl<'a> IntoFuture for DragByOffset<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                selector,
                x,
                y,
                timeout,
            } = self;
            sb.run_drag_by_offset(selector, x, y, timeout).await
        })
    }
}

/// Python's `wait_for_attribute(selector, attribute, value=None, by,
/// timeout=None)` and `wait_for_attribute_not_present(...)`: wait until an
/// element has (or no longer has) an attribute, and optionally a given value
/// of it.
///
/// It comes from [`BaseCase::wait_for_attribute`] and
/// [`BaseCase::wait_for_attribute_not_present`]. The keywords are
/// [`value`](Self::value) (without one, the attribute only has to exist) and
/// [`timeout`](Self::timeout) (default: the case's timeout).
#[must_use = "nothing happens until the call is awaited"]
pub struct WaitForAttribute<'a> {
    sb: &'a mut BaseCase,
    selector: &'a str,
    attribute: &'a str,
    value: Option<&'a str>,
    timeout: Option<Duration>,
    absent: bool,
}

impl<'a> WaitForAttribute<'a> {
    pub(crate) fn new(
        sb: &'a mut BaseCase,
        selector: &'a str,
        attribute: &'a str,
        absent: bool,
    ) -> Self {
        Self {
            sb,
            selector,
            attribute,
            value: None,
            timeout: None,
            absent,
        }
    }

    /// The value the attribute must have. Python's `value`.
    pub fn value(mut self, value: &'a str) -> Self {
        self.value = Some(value);
        self
    }

    /// How long to wait. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl<'a> IntoFuture for WaitForAttribute<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                selector,
                attribute,
                value,
                timeout,
                absent,
            } = self;
            sb.run_wait_for_attribute(selector, attribute, value, timeout, absent)
                .await
        })
    }
}

/// Python's `click_with_offset(selector, x, y, by, mark=None, timeout=None,
/// center=None, scroll=True)` and `double_click_with_offset(...)`: click an
/// element at an offset in pixels. `(0, 0)` is the top-left corner of the
/// element, or its centre with [`center`](Self::center).
///
/// It comes from [`BaseCase::click_with_offset`] and
/// [`BaseCase::double_click_with_offset`]. The keywords are
/// [`mark`](Self::mark) (draw a dot at the click on a canvas; on by default in
/// demo mode), [`timeout`](Self::timeout), [`center`](Self::center) and
/// [`scroll`](Self::scroll) (default on).
#[must_use = "nothing happens until the call is awaited"]
pub struct ClickWithOffset<'a> {
    sb: &'a mut BaseCase,
    selector: &'a str,
    x: f64,
    y: f64,
    double: bool,
    options: OffsetOptions,
}

impl<'a> ClickWithOffset<'a> {
    pub(crate) fn new(
        sb: &'a mut BaseCase,
        selector: &'a str,
        (x, y): (f64, f64),
        double: bool,
    ) -> Self {
        Self {
            sb,
            selector,
            x,
            y,
            double,
            options: OffsetOptions {
                mark: None,
                timeout: None,
                center: None,
                scroll: true,
            },
        }
    }

    /// Whether to draw a dot at the click, on a `<canvas>` element. Python's
    /// `mark`.
    pub fn mark(mut self, mark: bool) -> Self {
        self.options.mark = Some(mark);
        self
    }

    /// How long to wait for the element to be visible. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = Some(timeout);
        self
    }

    /// Whether the offset counts from the centre of the element instead of its
    /// top-left corner. Python's `center`.
    pub fn center(mut self, center: bool) -> Self {
        self.options.center = Some(center);
        self
    }

    /// Whether to scroll to the element first. Python's `scroll`.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.options.scroll = scroll;
        self
    }
}

impl<'a> IntoFuture for ClickWithOffset<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                selector,
                x,
                y,
                double,
                options,
            } = self;
            sb.run_click_with_offset(selector, (x, y), double, options)
                .await
        })
    }
}

/// Python's `open_new_tab(switch_to=True)` and `open_new_window(switch_to=True)`
/// (the same call): open a new browser tab, and switch to it unless told not
/// to.
///
/// It comes from [`BaseCase::open_new_tab`] and [`BaseCase::open_new_window`].
/// The one keyword is [`switch_to`](Self::switch_to).
#[must_use = "nothing happens until the call is awaited"]
pub struct OpenNewTab<'a> {
    sb: &'a mut BaseCase,
    switch_to: bool,
}

impl<'a> OpenNewTab<'a> {
    pub(crate) fn new(sb: &'a mut BaseCase) -> Self {
        Self {
            sb,
            switch_to: true,
        }
    }

    /// Whether to switch to the new tab (default) or stay on the current one.
    /// Python's `switch_to`.
    pub fn switch_to(mut self, switch_to: bool) -> Self {
        self.switch_to = switch_to;
        self
    }
}

impl<'a> IntoFuture for OpenNewTab<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.sb.run_open_new_window(self.switch_to).await })
    }
}

/// Python's `add_cookie(cookie_dict, expiry=False)`: add a cookie, given as the
/// W3C cookie object (`name`, `value`, and optionally `path`, `domain`,
/// `secure`, `httpOnly`, `expiry` and `sameSite`) that
/// [`BaseCase::get_cookies`] returns.
///
/// It comes from [`BaseCase::add_cookie`]. The one keyword is
/// [`expiry`](Self::expiry): by default (Python's `False`) the cookie's own
/// `expiry` is dropped, so it lasts for the browser session. A `domain` that
/// is not part of the page's origin becomes the origin's host, as in Python.
#[must_use = "nothing happens until the call is awaited"]
pub struct AddCookie<'a> {
    sb: &'a BaseCase,
    cookie: &'a Value,
    expiry: CookieExpiry,
}

impl<'a> AddCookie<'a> {
    pub(crate) fn new(sb: &'a BaseCase, cookie: &'a Value) -> Self {
        Self {
            sb,
            cookie,
            expiry: CookieExpiry::Session,
        }
    }

    /// What happens to the cookie's expiry: a `bool` as in Python (`true` is a
    /// day from now) or any [`CookieExpiry`]. Python's `expiry`.
    pub fn expiry(mut self, expiry: impl Into<CookieExpiry>) -> Self {
        self.expiry = expiry.into();
        self
    }
}

impl<'a> IntoFuture for AddCookie<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.sb.run_add_cookie(self.cookie, self.expiry).await })
    }
}

/// Python's `download_file(file_url, destination_folder=None)`: fetch a URL
/// with the browser's user agent and save the body, under the last part of the
/// URL's path, in a folder (default: the downloads folder). It resolves to
/// the path of the saved file.
///
/// It comes from [`BaseCase::download_file`]. The one keyword is
/// [`destination_folder`](Self::destination_folder). The call fails if the
/// server does not answer with a success status.
#[must_use = "nothing happens until the call is awaited"]
pub struct DownloadFile<'a> {
    sb: &'a BaseCase,
    file_url: &'a str,
    destination_folder: Option<PathBuf>,
}

impl<'a> DownloadFile<'a> {
    pub(crate) fn new(sb: &'a BaseCase, file_url: &'a str) -> Self {
        Self {
            sb,
            file_url,
            destination_folder: None,
        }
    }

    /// The folder to save the file in; it is created if it does not exist.
    /// Python's `destination_folder`.
    pub fn destination_folder(mut self, folder: impl AsRef<Path>) -> Self {
        self.destination_folder = Some(folder.as_ref().to_path_buf());
        self
    }
}

impl<'a> IntoFuture for DownloadFile<'a> {
    type Output = Result<PathBuf, SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, PathBuf>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.sb
                .run_download(self.file_url, None, self.destination_folder.as_deref())
                .await
        })
    }
}

/// Python's `save_file_as(file_url, new_file_name, destination_folder=None)`:
/// like [`DownloadFile`], but the file is saved under `new_file_name`, which
/// must be a plain file name.
///
/// It comes from [`BaseCase::save_file_as`]. The one keyword is
/// [`destination_folder`](Self::destination_folder).
#[must_use = "nothing happens until the call is awaited"]
pub struct SaveFileAs<'a> {
    sb: &'a BaseCase,
    file_url: &'a str,
    new_file_name: &'a str,
    destination_folder: Option<PathBuf>,
}

impl<'a> SaveFileAs<'a> {
    pub(crate) fn new(sb: &'a BaseCase, file_url: &'a str, new_file_name: &'a str) -> Self {
        Self {
            sb,
            file_url,
            new_file_name,
            destination_folder: None,
        }
    }

    /// The folder to save the file in; it is created if it does not exist.
    /// Python's `destination_folder`.
    pub fn destination_folder(mut self, folder: impl AsRef<Path>) -> Self {
        self.destination_folder = Some(folder.as_ref().to_path_buf());
        self
    }
}

impl<'a> IntoFuture for SaveFileAs<'a> {
    type Output = Result<PathBuf, SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, PathBuf>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.sb
                .run_download(
                    self.file_url,
                    Some(self.new_file_name),
                    self.destination_folder.as_deref(),
                )
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scroll_is_a_percentage_of_the_window_height() {
        assert_eq!(scroll_distance(800.0, 25.0, false), 200.0);
        assert_eq!(scroll_distance(800.0, 25.0, true), -200.0);
        assert_eq!(scroll_distance(1000.0, 100.0, false), 1000.0);
        assert_eq!(scroll_distance(800.0, 0.0, true), 0.0);
    }

    #[test]
    fn python_s_expiry_flag_is_a_cookie_expiry() {
        assert_eq!(CookieExpiry::from(false), CookieExpiry::Session);
        assert_eq!(CookieExpiry::from(true), CookieExpiry::Day);
    }
}
