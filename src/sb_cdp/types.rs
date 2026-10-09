//! Plain data and small enums shared across the Pure CDP API.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A rectangle in CSS pixels.
///
/// For elements, `x` and `y` are measured from the top-left of the document,
/// not the viewport, so they stay stable as the page scrolls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Distance from the left edge.
    pub x: f64,
    /// Distance from the top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// A point in viewport coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

impl Point {
    /// A point at `(x, y)`.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A snapshot of a DOM element at the moment it was queried.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ElementInfo {
    /// Lower-case tag name, such as `div`.
    pub tag: String,
    /// Rendered text (`innerText`).
    pub text: String,
    /// The element's outer HTML.
    pub html: String,
    /// Every attribute, by name.
    pub attributes: BTreeMap<String, String>,
    /// Position and size in the document.
    pub rect: Rect,
    /// Whether the element is rendered and takes up space.
    pub visible: bool,
}

/// An open browser tab.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageInfo {
    /// The DevTools target id, which identifies the tab.
    pub id: String,
    /// The tab's current URL.
    pub url: String,
    /// The tab's title.
    pub title: String,
}

/// A condition an element can be waited on to reach.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::State;
///
/// // `Hidden` is satisfied by an element that is missing altogether;
/// // `Absent` additionally requires that it is gone from the DOM.
/// assert_ne!(State::Hidden, State::Absent);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum State {
    /// At least one match exists in the DOM, visible or not.
    Present,
    /// At least one match exists and is rendered with a non-zero size.
    Visible,
    /// No match is visible; matches that do exist may be hidden.
    Hidden,
    /// No match exists in the DOM.
    Absent,
}

impl State {
    /// A phrase for error messages, such as "to be visible".
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Present => "to be present",
            Self::Visible => "to be visible",
            Self::Hidden => "to be hidden",
            Self::Absent => "to be absent",
        }
    }
}

/// How to choose an option in a `<select>` dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectBy<'a> {
    /// The option whose visible text matches.
    Text(&'a str),
    /// The option whose `value` attribute matches.
    Value(&'a str),
    /// The option at this zero-based position.
    Index(usize),
}

/// Where to scroll the page.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Scroll {
    /// To the top of the page.
    Top,
    /// To the bottom of the page.
    Bottom,
    /// To an absolute vertical offset in pixels.
    To(f64),
    /// By a number of pixels; negative scrolls up.
    By(f64),
    /// Down by a percentage of the viewport height.
    PageDown(f64),
    /// Up by a percentage of the viewport height.
    PageUp(f64),
}

/// A browser permission that can be granted without a prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Permission {
    /// Location.
    Geolocation,
    /// Desktop notifications.
    Notifications,
    /// Microphone.
    Microphone,
    /// Camera.
    Camera,
    /// Reading from and writing to the clipboard.
    Clipboard,
    /// MIDI devices.
    Midi,
    /// Motion and orientation sensors.
    Sensors,
}

impl Permission {
    /// Every permission this crate knows how to grant.
    pub const ALL: [Self; 7] = [
        Self::Geolocation,
        Self::Notifications,
        Self::Microphone,
        Self::Camera,
        Self::Clipboard,
        Self::Midi,
        Self::Sensors,
    ];

    /// The name the DevTools Protocol uses for this permission.
    pub(crate) fn protocol_name(self) -> &'static str {
        match self {
            Self::Geolocation => "geolocation",
            Self::Notifications => "notifications",
            Self::Microphone => "audioCapture",
            Self::Camera => "videoCapture",
            Self::Clipboard => "clipboardReadWrite",
            Self::Midi => "midi",
            Self::Sensors => "sensors",
        }
    }
}
