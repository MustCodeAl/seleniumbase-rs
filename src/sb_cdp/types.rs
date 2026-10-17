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

/// The display a page is shown on (`window.screen`), in CSS pixels.
///
/// This describes the monitor, not the browser window: for a window, see
/// [`Window`](super::Window). For an element's rectangle in screen
/// coordinates, see [`Locator::screen_rect`](super::Locator::screen_rect).
///
/// Python's `sb.cdp.get_screen_rect()`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ScreenRect {
    /// The width of the display.
    pub width: f64,
    /// The height of the display.
    pub height: f64,
    /// The width left over by taskbars and docks.
    pub avail_width: f64,
    /// The height left over by taskbars and docks.
    pub avail_height: f64,
    /// The left edge of the usable area.
    pub avail_left: f64,
    /// The top edge of the usable area.
    pub avail_top: f64,
    /// Bits per pixel of colour.
    pub color_depth: u32,
    /// Bits per pixel of the display.
    pub pixel_depth: u32,
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
    /// Background sync.
    BackgroundSync,
    /// Background fetch.
    BackgroundFetch,
    /// Writing sanitised content to the clipboard.
    ClipboardSanitizedWrite,
    /// Capturing a screen, window or tab.
    DisplayCapture,
    /// Storage that the browser does not clear under pressure.
    DurableStorage,
    /// Detecting when the user is idle.
    IdleDetection,
    /// Listing the fonts installed on the machine.
    LocalFonts,
    /// MIDI system-exclusive messages.
    MidiSysex,
    /// Near-field communication.
    Nfc,
    /// Payment handlers.
    PaymentHandler,
    /// Periodic background sync.
    PeriodicBackgroundSync,
    /// Storage access for embedded content.
    StorageAccess,
    /// Storage access requested by a top-level site for its embedded content.
    TopLevelStorageAccess,
    /// Keeping the screen awake.
    WakeLockScreen,
    /// Keeping the system awake.
    WakeLockSystem,
    /// Placing windows on any screen.
    WindowManagement,
}

impl Permission {
    /// Every permission Python's `grant_all_permissions` grants.
    pub const ALL: [Self; 23] = [
        Self::Geolocation,
        Self::Notifications,
        Self::Microphone,
        Self::Camera,
        Self::Clipboard,
        Self::Midi,
        Self::Sensors,
        Self::BackgroundSync,
        Self::BackgroundFetch,
        Self::ClipboardSanitizedWrite,
        Self::DisplayCapture,
        Self::DurableStorage,
        Self::IdleDetection,
        Self::LocalFonts,
        Self::MidiSysex,
        Self::Nfc,
        Self::PaymentHandler,
        Self::PeriodicBackgroundSync,
        Self::StorageAccess,
        Self::TopLevelStorageAccess,
        Self::WakeLockScreen,
        Self::WakeLockSystem,
        Self::WindowManagement,
    ];

    /// The name the DevTools Protocol uses for this permission, such as
    /// `audioCapture` for [`Microphone`](Self::Microphone).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Geolocation => "geolocation",
            Self::Notifications => "notifications",
            Self::Microphone => "audioCapture",
            Self::Camera => "videoCapture",
            Self::Clipboard => "clipboardReadWrite",
            Self::Midi => "midi",
            Self::Sensors => "sensors",
            Self::BackgroundSync => "backgroundSync",
            Self::BackgroundFetch => "backgroundFetch",
            Self::ClipboardSanitizedWrite => "clipboardSanitizedWrite",
            Self::DisplayCapture => "displayCapture",
            Self::DurableStorage => "durableStorage",
            Self::IdleDetection => "idleDetection",
            Self::LocalFonts => "localFonts",
            Self::MidiSysex => "midiSysex",
            Self::Nfc => "nfc",
            Self::PaymentHandler => "paymentHandler",
            Self::PeriodicBackgroundSync => "periodicBackgroundSync",
            Self::StorageAccess => "storageAccess",
            Self::TopLevelStorageAccess => "topLevelStorageAccess",
            Self::WakeLockScreen => "wakeLockScreen",
            Self::WakeLockSystem => "wakeLockSystem",
            Self::WindowManagement => "windowManagement",
        }
    }
}
