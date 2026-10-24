//! Website tours, as Python's `create_tour`, `add_tour_step`, `play_tour`,
//! `export_tour` and `set_introjs_colors` have them.
//!
//! A tour is a list of steps shown with one of five JavaScript libraries:
//! IntroJS (the default), Shepherd, Bootstrap Tour, DriverJS and Hopscotch.
//! The libraries come from the same CDNs and at the same versions as in
//! Python, because the scripts below call their old APIs. This module holds
//! what needs no browser: the steps, the themes, the scripts that show a tour
//! and export it, and the clock of an autoplaying tour. The calls themselves
//! are on [`BaseCase`](crate::BaseCase): `create_tour`, `add_tour_step`,
//! `play_tour` and `export_tour`.
//!
//! The text of a step is HTML, as in Python. It is placed in a JavaScript
//! string, so quotes and backslashes in it are escaped here; Python escapes
//! only the quotes that are not escaped already.

use std::str::FromStr;
use std::time::Duration;

use tokio::time::Instant;

use crate::error::SeleniumBaseError;
use crate::utils::selectors::{is_xpath_selector, xpath_to_css};

/// The name a tour has when none is given.
pub const DEFAULT_NAME: &str = "default";

/// The file `export_tour` writes when none is named.
pub const DEFAULT_FILENAME: &str = "my_tour.js";

/// The folder `export_tour` writes into, below the working directory.
pub const EXPORT_FOLDER: &str = "tours_exported";

/// The colour of the IntroJS buttons (Python's `TourColor.theme_color`).
pub const DEFAULT_THEME_COLOR: &str = "#367be5";

/// The colour of the IntroJS buttons under the mouse and of their border
/// (Python's `TourColor.hover_color`).
pub const DEFAULT_HOVER_COLOR: &str = "#245ac0";

/// The shortest time an autoplaying tour shows a step (Python: 0.5 s).
pub const MIN_INTERVAL: Duration = Duration::from_millis(500);

/// The shortest time an autoplaying Bootstrap tour shows a step (Python: 1 s).
pub const MIN_BOOTSTRAP_INTERVAL: Duration = Duration::from_secs(1);

/// How long the element of the first step is waited for (Python's
/// `SMALL_TIMEOUT`).
pub(crate) const FIRST_STEP_TIMEOUT_SECS: u64 = 7;

/// `name` when it is a name, else [`DEFAULT_NAME`]. Python treats an empty name
/// as no name.
#[must_use]
pub fn name_or_default(name: Option<&str>) -> &str {
    match name {
        Some(name) if !name.is_empty() => name,
        _ => DEFAULT_NAME,
    }
}

/// The error for a tour name that nothing was created under.
pub(crate) fn missing_tour(name: &str) -> SeleniumBaseError {
    SeleniumBaseError::InvalidConfig(format!("Tour {{{name}}} does not exist!"))
}

/// The CSS selector a tour step attaches to for `selector`, as Python's
/// `add_tour_step` reads it: `name=q` (or `&q`) is `[name="q"]`, an XPath
/// that has a CSS equivalent is converted, anything else is CSS already.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidSelector`] for an XPath that has no CSS
/// equivalent: the tour libraries take CSS only.
pub(crate) fn normalize_selector(selector: &str) -> Result<String, SeleniumBaseError> {
    if let Some(name) = selector
        .strip_prefix("name=")
        .or_else(|| selector.strip_prefix('&'))
    {
        let name = name.replace('\\', "\\\\").replace('"', "\\\"");
        return Ok(format!("[name=\"{name}\"]"));
    }
    if is_xpath_selector(selector) {
        return xpath_to_css(selector);
    }
    Ok(selector.to_owned())
}

/// The look of a Shepherd tour or of one of its steps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShepherdTheme {
    /// Python's `"light"`, also called `"arrows"`: the default.
    #[default]
    Light,
    /// Python's `"dark"`.
    Dark,
    /// Python's `"default"`, which is Shepherd's own plain theme. It is not
    /// what [`ShepherdTheme::default`] returns.
    Default,
    /// Python's `"square"`.
    Square,
    /// Python's `"square-dark"`.
    SquareDark,
}

impl ShepherdTheme {
    /// Python's second name for [`Light`](Self::Light).
    pub const ARROWS: Self = Self::Light;

    /// The CSS class Shepherd 1.8 gives a tour or a step of this theme.
    #[must_use]
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Light => "shepherd-theme-arrows",
            Self::Dark => "shepherd-theme-dark",
            Self::Default => "shepherd-theme-default",
            Self::Square => "shepherd-theme-square",
            Self::SquareDark => "shepherd-theme-square-dark",
        }
    }
}

impl FromStr for ShepherdTheme {
    type Err = SeleniumBaseError;

    /// Reads Python's name of a Shepherd theme (`light`, `arrows`, `dark`,
    /// `default`, `square`, `square-dark`), in any case.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_ascii_lowercase().as_str() {
            "light" | "arrows" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            "default" => Ok(Self::Default),
            "square" => Ok(Self::Square),
            "square-dark" => Ok(Self::SquareDark),
            _ => Err(SeleniumBaseError::InvalidConfig(format!(
                "{text:?} is not a Shepherd theme (expected light, arrows, dark, default, \
                 square or square-dark)"
            ))),
        }
    }
}

/// The library a tour is shown with, which is Python's `theme` of
/// `create_tour`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TourTheme {
    /// Bootstrap Tour.
    Bootstrap,
    /// DriverJS.
    DriverJs,
    /// Hopscotch.
    Hopscotch,
    /// IntroJS, which Python uses when no theme is given.
    #[default]
    IntroJs,
    /// Shepherd, with one of its looks.
    Shepherd(ShepherdTheme),
}

impl From<ShepherdTheme> for TourTheme {
    fn from(theme: ShepherdTheme) -> Self {
        Self::Shepherd(theme)
    }
}

impl FromStr for TourTheme {
    type Err = SeleniumBaseError;

    /// Reads the names Python's `create_tour(theme=...)` takes, in any case:
    /// `bootstrap`, `hopscotch`, `intro` or `introjs`, `driver` or `driverjs`,
    /// `shepherd`, and the Shepherd looks `light`, `arrows`, `dark`,
    /// `default`, `square` and `square-dark`. (Python falls back to IntroJS
    /// for any other text; this reports it.)
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_ascii_lowercase().as_str() {
            "bootstrap" => Ok(Self::Bootstrap),
            "hopscotch" => Ok(Self::Hopscotch),
            "intro" | "introjs" => Ok(Self::IntroJs),
            "driver" | "driverjs" => Ok(Self::DriverJs),
            "shepherd" => Ok(Self::Shepherd(ShepherdTheme::Light)),
            other => ShepherdTheme::from_str(other)
                .map(Self::Shepherd)
                .map_err(|_| {
                    SeleniumBaseError::InvalidConfig(format!(
                        "{text:?} is not a tour theme (expected bootstrap, driverjs, hopscotch, \
                     introjs, shepherd, or a Shepherd look: light, arrows, dark, default, \
                     square, square-dark)"
                    ))
                }),
        }
    }
}

/// Where a step sits next to its element (Python's `alignment`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TourAlignment {
    /// Above the element (Python's default, except for DriverJS and Hopscotch).
    Top,
    /// Below the element (the default of DriverJS and Hopscotch).
    Bottom,
    /// To the left of the element.
    Left,
    /// To the right of the element.
    Right,
}

impl TourAlignment {
    /// Python's name of the alignment.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

impl FromStr for TourAlignment {
    type Err = SeleniumBaseError;

    /// Reads `top`, `bottom`, `left` or `right`, in any case.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_ascii_lowercase().as_str() {
            "top" => Ok(Self::Top),
            "bottom" => Ok(Self::Bottom),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            _ => Err(SeleniumBaseError::InvalidConfig(format!(
                "{text:?} is not an alignment (expected top, bottom, left or right)"
            ))),
        }
    }
}

/// One step of a tour, with the arguments of Python's `add_tour_step`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TourStep {
    /// The text of the step, as HTML.
    pub message: String,
    /// The CSS selector of the element the step attaches to. `None` (and the
    /// empty text) is `html`, a step that points at nothing.
    pub selector: Option<String>,
    /// A heading above the message.
    pub title: Option<String>,
    /// The look of this step (Shepherd tours only).
    pub theme: Option<ShepherdTheme>,
    /// Where the step sits next to its element; `None` is the library's
    /// default.
    pub alignment: Option<TourAlignment>,
    /// How long the step is shown before the tour goes on by itself (Bootstrap
    /// tours only).
    pub duration: Option<Duration>,
}

impl TourStep {
    /// A step that shows `message` and points at nothing.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ..Self::default()
        }
    }

    /// The selector the step attaches to: `html` when it has none.
    #[must_use]
    pub fn selector_or_html(&self) -> &str {
        match self.selector.as_deref() {
            None | Some("") => "html",
            Some(selector) => selector,
        }
    }
}

/// The five libraries a tour is shown with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Library {
    Bootstrap,
    DriverJs,
    Hopscotch,
    IntroJs,
    Shepherd,
}

/// A script or style sheet a library needs in the page.
pub(crate) struct Resource {
    /// A script when true, else a style sheet.
    pub script: bool,
    pub url: &'static str,
    /// A JavaScript expression that is true once the script is in the page.
    pub loaded: Option<&'static str>,
}

const BOOTSTRAP_CSS: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/bootstrap-tour/0.12.0/css/bootstrap-tour-standalone.min.css";
const BOOTSTRAP_JS: &str = "https://seleniumbase.github.io/cdn/js/bootstraptour.min.js";
const DRIVERJS_CSS: &str = "https://cdn.jsdelivr.net/npm/driver.js@0.9.8/dist/driver.min.css";
const DRIVERJS_JS: &str = "https://cdn.jsdelivr.net/npm/driver.js@0.9.8/dist/driver.min.js";
const HOPSCOTCH_CSS: &str =
    "https://cdn.jsdelivr.net/npm/hopscotch@0.3.1/dist/css/hopscotch.min.css";
const HOPSCOTCH_JS: &str = "https://cdn.jsdelivr.net/npm/hopscotch@0.3.1/dist/js/hopscotch.min.js";
const INTROJS_CSS: &str = "https://cdn.jsdelivr.net/npm/intro.js@5.1.0/minified/introjs.min.css";
const INTROJS_JS: &str = "https://cdn.jsdelivr.net/npm/intro.js@5.1.0/intro.min.js";
const SHEPHERD_JS: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/shepherd/1.8.1/js/shepherd.min.js";
const SHEPHERD_CSS: [&str; 6] = [
    "https://cdnjs.cloudflare.com/ajax/libs/shepherd/1.8.1/css/shepherd-theme-arrows.css",
    "https://cdnjs.cloudflare.com/ajax/libs/shepherd/1.8.1/css/shepherd-theme-arrows-fix.css",
    "https://cdnjs.cloudflare.com/ajax/libs/shepherd/1.8.1/css/shepherd-theme-default.css",
    "https://cdnjs.cloudflare.com/ajax/libs/shepherd/1.8.1/css/shepherd-theme-dark.css",
    "https://cdnjs.cloudflare.com/ajax/libs/shepherd/1.8.1/css/shepherd-theme-square.css",
    "https://cdnjs.cloudflare.com/ajax/libs/shepherd/1.8.1/css/shepherd-theme-square-dark.css",
];
const TETHER_JS: &str = "https://cdn.jsdelivr.net/npm/tether@1.4.7/dist/js/tether.min.js";

const BOOTSTRAP_RESOURCES: &[Resource] = &[
    Resource {
        script: false,
        url: BOOTSTRAP_CSS,
        loaded: None,
    },
    Resource {
        script: true,
        url: BOOTSTRAP_JS,
        loaded: Some("typeof window.Tour !== 'undefined'"),
    },
];
const DRIVERJS_RESOURCES: &[Resource] = &[
    Resource {
        script: false,
        url: DRIVERJS_CSS,
        loaded: None,
    },
    Resource {
        script: true,
        url: DRIVERJS_JS,
        loaded: Some("typeof window.Driver !== 'undefined'"),
    },
];
const HOPSCOTCH_RESOURCES: &[Resource] = &[
    Resource {
        script: false,
        url: HOPSCOTCH_CSS,
        loaded: None,
    },
    Resource {
        script: true,
        url: HOPSCOTCH_JS,
        loaded: Some("typeof window.hopscotch !== 'undefined'"),
    },
];
const INTROJS_RESOURCES: &[Resource] = &[
    Resource {
        script: false,
        url: INTROJS_CSS,
        loaded: None,
    },
    Resource {
        script: true,
        url: INTROJS_JS,
        loaded: Some("typeof window.introJs !== 'undefined'"),
    },
];
const SHEPHERD_RESOURCES: &[Resource] = &[
    Resource {
        script: true,
        url: TETHER_JS,
        loaded: Some("typeof window.Tether !== 'undefined'"),
    },
    Resource {
        script: false,
        url: SHEPHERD_CSS[0],
        loaded: None,
    },
    Resource {
        script: false,
        url: SHEPHERD_CSS[1],
        loaded: None,
    },
    Resource {
        script: false,
        url: SHEPHERD_CSS[2],
        loaded: None,
    },
    Resource {
        script: false,
        url: SHEPHERD_CSS[3],
        loaded: None,
    },
    Resource {
        script: false,
        url: SHEPHERD_CSS[4],
        loaded: None,
    },
    Resource {
        script: false,
        url: SHEPHERD_CSS[5],
        loaded: None,
    },
    Resource {
        script: true,
        url: SHEPHERD_JS,
        loaded: Some("typeof window.Shepherd !== 'undefined'"),
    },
];

/// What each library's backdrop looks like (Python's `style_sheet`).
const BOOTSTRAP_STYLE: &str = "\
.tour-tour-element {
    pointer-events: none !important;
}
:not(.tour-tour-element) .orphan.tour-tour {
    box-shadow: 0 0 0 88422px rgba(0, 0, 0, 0.42);
    pointer-events: auto !important;
}
";
const DRIVERJS_STYLE: &str = "\
.driver-fix-stacking {
    pointer-events: none !important;
}
#driver-popover-item, .popover-class {
    pointer-events: auto !important;
}
button.driver-prev-btn.driver-disabled {
    visibility: hidden;
}
";
const HOPSCOTCH_STYLE: &str = "\
.hopscotch-bubble-container {
    font-size: 110%;
}
";
const SHEPHERD_STYLE: &str = "\
body.shepherd-active .shepherd-target.shepherd-enabled {
    box-shadow: 0 0 0 99999px rgba(0, 0, 0, 0.20);
    pointer-events: none !important;
    z-index: 9999;
}
body.shepherd-active .shepherd-orphan {
    box-shadow: 0 0 0 99999px rgba(0, 0, 0, 0.20);
    pointer-events: auto;
    z-index: 9999;
}
body.shepherd-active .shepherd-enabled.shepherd-element-attached-top {
    position: relative;
}
body.shepherd-active .shepherd-enabled.shepherd-element-attached-bottom {
    position: relative;
}
body.shepherd-active .shepherd-step {
    pointer-events: auto;
    z-index: 9999;
}
body.shepherd-active {
    pointer-events: none !important;
}
";

const SHEPHERD_HEADER: &str = "\
// Shepherd Tour
var tour = new Shepherd.Tour({
    defaults: {
        classes: @CLASSES@,
        scrollTo: true
    }
});
var allButtons = {
    skip: {
        text: \"Skip\",
        action: tour.cancel,
        classes: 'shepherd-button-secondary tour-button-left'
    },
    back: {
        text: \"Back\",
        action: tour.back,
        classes: 'shepherd-button-secondary'
    },
    next: {
        text: \"Next\",
        action: tour.next,
        classes: 'shepherd-button-primary tour-button-right'
    },
};
var firstStepButtons = [allButtons.skip, allButtons.next];
var midTourButtons = [allButtons.back, allButtons.next];
";
const BOOTSTRAP_HEADER: &str = "\
// Bootstrap Tour
var tour = new Tour({
    container: 'body',
    animation: true,
    keyboard: true,
    orphan: true,
    smartPlacement: true,
    autoscroll: true,
    backdrop: true,
    backdropContainer: 'body',
    backdropPadding: 3,
});
tour.addSteps([
";
const DRIVERJS_HEADER: &str = "\
// DriverJS Tour
var tour = new Driver({
    opacity: 0.24,  // Background opacity (0: no popover / overlay)
    padding: 6,    // Distance of element from around the edges
    allowClose: false, // Whether clicking on overlay should close
    overlayClickNext: false, // Move to next step on overlay click
    doneBtnText: 'Done', // Text that appears on the Done button
    closeBtnText: 'Close', // Text appearing on the Close button
    nextBtnText: 'Next', // Text that appears on the Next button
    prevBtnText: 'Previous', // Text appearing on Previous button
    showButtons: true, // This shows control buttons in the footer
    keyboardControl: true, // (escape to close, arrow keys to move)
    animate: true,   // Animate while changing highlighted element
});
tour.defineSteps([
";
const HOPSCOTCH_HEADER: &str = "\
// Hopscotch Tour
var tour = {
id: \"hopscotch_tour\",
steps: [
";
const INTROJS_HEADER: &str = "\
// IntroJS Tour
function startIntro(){
var intro = introJs();
intro.setOptions({
steps: [
";

const BOOTSTRAP_FOOTER_PLAY: &str = "\
]);
// Initialize the tour
tour.init();
// Start the tour
tour.start();
// Fix timing issue by restarting tour immediately
tour.restart();
// Save for later
$tour = tour;
";
const BOOTSTRAP_FOOTER_EXPORT: &str = "\
]);
// Initialize the tour
tour.init();
// Start the tour
tour.start();
$tour = tour;
$tour.restart();
";
const DRIVERJS_FOOTER: &str = "\
]
);
// Start the tour!
tour.start();
$tour = tour;
";
const HOPSCOTCH_FOOTER: &str = "\
]
};
// Start the tour!
hopscotch.startTour(tour);
$tour = hopscotch;
";
const INTROJS_FOOTER: &str = "\
]
});
intro.setOption(\"disableInteraction\", true);
intro.setOption(\"overlayOpacity\", .29);
intro.setOption(\"scrollToElement\", true);
intro.setOption(\"keyboardNavigation\", true);
intro.setOption(\"exitOnEsc\", true);
intro.setOption(\"hidePrev\", true);
intro.setOption(\"nextToDone\", true);
intro.setOption(\"exitOnOverlayClick\", false);
intro.setOption(\"showStepNumbers\", false);
intro.setOption(\"showProgress\", false);
intro.start();
$tour = intro;
};
// Start the tour
startIntro();
";
const SHEPHERD_FOOTER: &str = "\
// Start the tour
tour.start();
$tour = tour;
";

/// Page-side keys for a library, which are added once to a page.
const SHEPHERD_KEYS: &str = "\
if (!window.__sbTourKeysShepherd) {
    window.__sbTourKeysShepherd = true;
    document.body.addEventListener('keyup', function (event) {
        if (event.key === 'PageUp' || event.key === 'ArrowLeft') {
            Shepherd.activeTour.back(); }
        if (event.key === 'PageDown' || event.key === 'ArrowRight') {
            Shepherd.activeTour.next(); }
    });
}
";
const DRIVERJS_KEYS: &str = "\
if (!window.__sbTourKeysDriver) {
    window.__sbTourKeysDriver = true;
    document.body.addEventListener('keyup', function (event) {
        if (event.key === 'PageUp') { $tour.movePrevious(); }
        if (event.key === 'PageDown') { $tour.moveNext(); }
    });
}
";
const HOPSCOTCH_KEYS: &str = "\
if (!window.__sbTourKeysHopscotch) {
    window.__sbTourKeysHopscotch = true;
    document.body.addEventListener('keyup', function (event) {
        if (event.key === 'PageUp' || event.key === 'ArrowLeft') {
            $tour.prevStep(); }
        if (event.key === 'PageDown' || event.key === 'ArrowRight') {
            $tour.nextStep(); }
    });
}
";
const INTROJS_KEYS: &str = "\
if (!window.__sbTourKeysIntro) {
    window.__sbTourKeysIntro = true;
    document.body.addEventListener('keyup', function (event) {
        if (event.key === 'PageUp') { $tour.previousStep(); }
        if (event.key === 'PageDown') { $tour.nextStep(); }
    });
}
";

/// The three functions a script exported from a tour starts with.
const EXPORT_INJECTORS: &str = "\
function injectCSS(css_link) {var head = document.getElementsByTagName(\"head\")[0];\
var link = document.createElement(\"link\");link.rel = \"stylesheet\";\
link.type = \"text/css\";link.href = css_link;link.crossorigin = \"anonymous\";\
head.appendChild(link);};
function injectJS(js_link) {var head = document.getElementsByTagName(\"head\")[0];\
var script = document.createElement(\"script\");script.src = js_link;script.defer;\
script.type=\"text/javascript\";script.crossorigin = \"anonymous\";\
script.onload = function() { null };head.appendChild(script);};
function injectStyle(css) {var head = document.getElementsByTagName(\"head\")[0];\
var style = document.createElement(\"style\");style.type = \"text/css\";\
style.appendChild(document.createTextNode(css));head.appendChild(style);};
";

impl Library {
    /// The scripts and style sheets the library needs, in loading order.
    pub(crate) fn resources(self) -> &'static [Resource] {
        match self {
            Self::Bootstrap => BOOTSTRAP_RESOURCES,
            Self::DriverJs => DRIVERJS_RESOURCES,
            Self::Hopscotch => HOPSCOTCH_RESOURCES,
            Self::IntroJs => INTROJS_RESOURCES,
            Self::Shepherd => SHEPHERD_RESOURCES,
        }
    }

    /// Whether jQuery has to be in the page first.
    pub(crate) fn needs_jquery(self) -> bool {
        self == Self::Bootstrap
    }

    /// An element that shows once the tour has started, if there is a
    /// reliable one.
    pub(crate) fn ready_selector(self) -> Option<&'static str> {
        match self {
            Self::Shepherd => Some("a.tour-button-right"),
            Self::Bootstrap => Some(".tour-tour"),
            Self::Hopscotch => Some("button.hopscotch-next"),
            Self::DriverJs | Self::IntroJs => None,
        }
    }

    /// Whether the page's focus is taken off the element that had it before
    /// the tour started (Python does not for DriverJS, which focuses its Next
    /// button).
    pub(crate) fn blurs_focus(self) -> bool {
        self != Self::DriverJs
    }

    /// Whether the library plays on by itself after a time, so that the
    /// player has no timer of its own to keep (Bootstrap Tour: `duration`).
    pub(crate) fn times_its_own_steps(self) -> bool {
        self == Self::Bootstrap
    }

    /// The JavaScript global that tells the library has loaded.
    fn global(self) -> &'static str {
        match self {
            Self::Bootstrap => "Tour",
            Self::DriverJs => "Driver",
            Self::Hopscotch => "hopscotch",
            Self::IntroJs => "introJs",
            Self::Shepherd => "Shepherd",
        }
    }

    /// A script that tells whether the tour is still showing a step.
    ///
    /// It returns `{active, key, pending?, element?}`: `active` when a step is
    /// showing, `key` for what the step is (it changes with the step), and,
    /// for Shepherd, `pending` when the tour runs but its step is not open.
    /// It throws when there is no tour any more, which is how the end of a tour
    /// (and a closed window) shows.
    pub(crate) fn probe_script(self) -> &'static str {
        match self {
            Self::Shepherd => {
                "var s = Shepherd.activeTour.currentStep; var a = s.options.attachTo; \
                 var el = a ? String(a.element) : ''; var open = !!s.isOpen(); \
                 return {active: open, pending: !open, key: el + '|' + String(s.options.text), \
                 element: el};"
            }
            Self::Bootstrap => "return {active: $tour.ended() === false, key: ''};",
            Self::DriverJs => {
                "var on = !!$tour.isActivated; var b = document.querySelector('.driver-next-btn'); \
                 if (on && b) { b.focus(); } return {active: on, key: String($tour.currentStep)};"
            }
            Self::Hopscotch => {
                "return {active: !!$tour.isActive, key: String($tour.getCurrStepNum())};"
            }
            Self::IntroJs => {
                "var s = $tour._currentStep; \
                 return {active: s !== null && s !== undefined, key: String(s)};"
            }
        }
    }

    /// A script that goes on to the next step.
    pub(crate) fn advance_script(self) -> &'static str {
        match self {
            Self::Shepherd => "Shepherd.activeTour.next();",
            Self::Bootstrap => "$tour.next();",
            Self::DriverJs => "$tour.moveNext();",
            Self::Hopscotch => "$tour.nextStep();",
            Self::IntroJs => "try { $tour.nextStep(); } catch (e) { $tour.exit(); }",
        }
    }

    /// The arrow and page keys, which Python adds for the libraries that do
    /// not handle them (once to a page).
    fn keys_script(self) -> Option<&'static str> {
        match self {
            Self::Shepherd => Some(SHEPHERD_KEYS),
            Self::DriverJs => Some(DRIVERJS_KEYS),
            Self::Hopscotch => Some(HOPSCOTCH_KEYS),
            Self::IntroJs => Some(INTROJS_KEYS),
            Self::Bootstrap => None,
        }
    }

    /// Where a step sits when it has no alignment.
    fn default_alignment(self) -> TourAlignment {
        match self {
            Self::Hopscotch | Self::DriverJs => TourAlignment::Bottom,
            _ => TourAlignment::Top,
        }
    }
}

/// The colours of IntroJS buttons, as Python's `set_introjs_colors` sets them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroJsColors {
    theme_color: String,
    hover_color: String,
}

impl Default for IntroJsColors {
    fn default() -> Self {
        Self {
            theme_color: DEFAULT_THEME_COLOR.to_owned(),
            hover_color: DEFAULT_HOVER_COLOR.to_owned(),
        }
    }
}

/// Whether `text` is a hex colour: `#` and three or six hex digits.
fn is_hex_color(text: &str) -> bool {
    text.strip_prefix('#').is_some_and(|digits| {
        matches!(digits.len(), 3 | 6) && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

impl IntroJsColors {
    /// The colours for `theme_color` and `hover_color`; a colour that is
    /// `None` is the default.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Expecting a hex value color that starts with "#"!`, for a colour that
    /// is not `#` and three or six hex digits.
    pub fn new(
        theme_color: Option<&str>,
        hover_color: Option<&str>,
    ) -> Result<Self, SeleniumBaseError> {
        let pick = |color: Option<&str>, default: &str| match color {
            Some(color) if !color.is_empty() => {
                if is_hex_color(color) {
                    Ok(color.to_owned())
                } else {
                    Err(SeleniumBaseError::InvalidConfig(
                        "Expecting a hex value color that starts with \"#\"!".to_owned(),
                    ))
                }
            }
            _ => Ok(default.to_owned()),
        };
        Ok(Self {
            theme_color: pick(theme_color, DEFAULT_THEME_COLOR)?,
            hover_color: pick(hover_color, DEFAULT_HOVER_COLOR)?,
        })
    }

    /// The colour of the buttons.
    #[must_use]
    pub fn theme_color(&self) -> &str {
        &self.theme_color
    }

    /// The colour of the buttons under the mouse, and of their border.
    #[must_use]
    pub fn hover_color(&self) -> &str {
        &self.hover_color
    }

    /// The style sheet that gives IntroJS these colours.
    pub(crate) fn style(&self) -> String {
        let (theme, hover) = (&self.theme_color, &self.hover_color);
        format!(
            "\
.introjs-button.introjs-nextbutton,
.introjs-button.introjs-donebutton {{
    color: #fff !important;
    background-color: {theme} !important;
    border: 1px solid {hover} !important;
    text-shadow: none;
    box-shadow: none;
}}
.introjs-button.introjs-nextbutton:hover,
.introjs-button.introjs-donebutton:hover {{
    color: #fff !important;
    background-color: {hover} !important;
    border: 1px solid {hover} !important;
}}
.introjs-button {{
    box-sizing: content-box;
    text-decoration: none;
}}
.introjs-button.introjs-skipbutton {{
    color: {theme};
}}
.introjs-tooltip, .introjs-floating {{
    box-sizing: content-box;
    position: absolute;
}}
"
        )
    }
}

/// `text` as a JavaScript string in single quotes.
fn js_single(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('\'');
    for c in text.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '\'' => quoted.push_str("\\'"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\u{2028}' => quoted.push_str("\\u2028"),
            '\u{2029}' => quoted.push_str("\\u2029"),
            other => quoted.push(other),
        }
    }
    quoted.push('\'');
    quoted
}

/// `text` as a JavaScript string in double quotes (a JSON string).
fn js_double(text: &str) -> String {
    let mut quoted = serde_json::Value::String(text.to_owned()).to_string();
    // JSON leaves these two alone; older JavaScript engines do not accept
    // them inside a string.
    quoted = quoted
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    quoted
}

/// A tour: a library and the steps it shows.
#[derive(Clone, Debug, Default)]
pub struct Tour {
    theme: TourTheme,
    steps: Vec<TourStep>,
}

/// What it takes to start a tour and see it through.
pub(crate) struct TourPlan {
    pub library: Library,
    /// The script that starts the tour, with the keys that go with it.
    pub script: String,
    /// The style sheet of the library's backdrop.
    pub style: String,
    /// The selector of the first step, when the page has to have it.
    pub first_selector: Option<String>,
    /// The time between steps of a tour the player moves on, if it autoplays.
    pub autoplay: Option<Duration>,
}

impl Tour {
    /// A tour shown with `theme` and without steps.
    #[must_use]
    pub fn new(theme: impl Into<TourTheme>) -> Self {
        Self {
            theme: theme.into(),
            steps: Vec::new(),
        }
    }

    /// The theme the tour was made with.
    #[must_use]
    pub fn theme(&self) -> TourTheme {
        self.theme
    }

    /// The steps, in the order they are shown.
    #[must_use]
    pub fn steps(&self) -> &[TourStep] {
        &self.steps
    }

    /// Adds a step at the end.
    pub fn add_step(&mut self, step: TourStep) {
        self.steps.push(step);
    }

    pub(crate) fn library(&self) -> Library {
        match self.theme {
            TourTheme::Bootstrap => Library::Bootstrap,
            TourTheme::DriverJs => Library::DriverJs,
            TourTheme::Hopscotch => Library::Hopscotch,
            TourTheme::IntroJs => Library::IntroJs,
            TourTheme::Shepherd(_) => Library::Shepherd,
        }
    }

    /// The script that creates the tour and adds its steps, without starting
    /// it. `interval` is the time Bootstrap Tour shows a step that has no
    /// duration of its own.
    fn steps_script(&self, name: &str, interval: Option<Duration>) -> String {
        let library = self.library();
        let mut out = match (library, self.theme) {
            (Library::Shepherd, TourTheme::Shepherd(theme)) => {
                SHEPHERD_HEADER.replace("@CLASSES@", &js_single(theme.css_class()))
            }
            (Library::Bootstrap, _) => BOOTSTRAP_HEADER.to_owned(),
            (Library::DriverJs, _) => DRIVERJS_HEADER.to_owned(),
            (Library::Hopscotch, _) => HOPSCOTCH_HEADER.to_owned(),
            (_, _) => INTROJS_HEADER.to_owned(),
        };
        for (index, step) in self.steps.iter().enumerate() {
            let selector = step.selector_or_html();
            let alignment = step
                .alignment
                .unwrap_or_else(|| library.default_alignment())
                .as_str();
            let title = step.title.as_deref().unwrap_or_default();
            let message = step.message.as_str();
            let rendered = match library {
                Library::Shepherd => self.shepherd_step(name, index, step, selector, alignment),
                Library::Bootstrap => bootstrap_step(step, selector, alignment, interval),
                Library::DriverJs => driverjs_step(title, message, selector, alignment),
                Library::Hopscotch => hopscotch_step(title, message, selector, alignment),
                Library::IntroJs => introjs_step(title, message, selector, alignment),
            };
            out.push_str(&rendered);
        }
        out
    }

    fn shepherd_step(
        &self,
        name: &str,
        index: usize,
        step: &TourStep,
        selector: &str,
        alignment: &str,
    ) -> String {
        let base = match self.theme {
            TourTheme::Shepherd(theme) => theme,
            _ => ShepherdTheme::default(),
        };
        let mut classes = step.theme.unwrap_or(base).css_class().to_owned();
        if selector == "html" {
            classes.push_str(" shepherd-orphan");
        }
        let buttons = if index == 0 {
            "firstStepButtons"
        } else {
            "midTourButtons"
        };
        format!(
            "tour.addStep({name}, {{\n    title: {title},\n    classes: {classes},\n    \
             text: {text},\n    attachTo: {{element: {element}, on: '{alignment}'}},\n    \
             buttons: {buttons},\n    advanceOn: '.docs-link click'\n}});\n",
            name = js_single(name),
            title = js_single(step.title.as_deref().unwrap_or_default()),
            classes = js_single(&classes),
            text = js_single(&step.message),
            element = js_single(selector),
        )
    }

    /// Everything the player needs to start the tour called `name` and see it
    /// through. `interval` is the time between steps of an autoplaying tour.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a tour without steps.
    pub(crate) fn plan(
        &self,
        name: &str,
        interval: Option<Duration>,
        colors: &IntroJsColors,
    ) -> Result<TourPlan, SeleniumBaseError> {
        if self.steps.is_empty() {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "Tour {{{name}}} has no steps! Add one with add_tour_step first."
            )));
        }
        let library = self.library();
        let interval = interval.filter(|interval| !interval.is_zero());
        let (bootstrap_interval, autoplay) = if library.times_its_own_steps() {
            (interval.map(|i| i.max(MIN_BOOTSTRAP_INTERVAL)), None)
        } else {
            (None, interval.map(|i| i.max(MIN_INTERVAL)))
        };
        let mut script = self.steps_script(name, bootstrap_interval);
        script.push_str(footer(library, true));
        if let Some(keys) = library.keys_script() {
            script.push_str(keys);
        }
        let style = match library {
            Library::Bootstrap => BOOTSTRAP_STYLE.to_owned(),
            Library::DriverJs => DRIVERJS_STYLE.to_owned(),
            Library::Hopscotch => HOPSCOTCH_STYLE.to_owned(),
            Library::Shepherd => SHEPHERD_STYLE.to_owned(),
            Library::IntroJs => colors.style(),
        };
        let first_selector = self
            .steps
            .first()
            .map(TourStep::selector_or_html)
            .filter(|selector| *selector != "html")
            .map(ToOwned::to_owned);
        Ok(TourPlan {
            library,
            script,
            style,
            first_selector,
            autoplay,
        })
    }

    /// A JavaScript file that plays the tour in any page: it loads the page
    /// `url` if the browser is not there, loads the library, and shows the
    /// tour. Pasted into a browser's console, it plays the tour outside of a
    /// test.
    pub(crate) fn export_script(&self, name: &str, url: &str, colors: &IntroJsColors) -> String {
        let library = self.library();
        let url = js_double(url);
        let mut out = format!(
            "////////  Load Tour Start Page (if not there now)  ////////\n\n\
             if (window.location.href != {url}) {{\n    window.location.href={url};\n}}\n\n\
             ////////  Resources  ////////\n\n{EXPORT_INJECTORS}"
        );
        let style = |css: &str| format!("injectStyle({});\n", js_double(&css.replace('\n', "")));
        let css = |url: &str| format!("injectCSS({});\n", js_double(url));
        let js = |url: &str| format!("injectJS({});\n", js_double(url));
        match library {
            Library::Bootstrap => {
                out.push_str(&js(crate::sb_cdp::messenger::JQUERY_URL));
                out.push_str(
                    "\nfunction loadResources() { if ( typeof jQuery !== \"undefined\" ) {\n",
                );
                out.push_str(&css(BOOTSTRAP_CSS));
                out.push_str(&style(BOOTSTRAP_STYLE));
                out.push_str(&js(BOOTSTRAP_JS));
                out.push_str(
                    "} else { window.setTimeout(\"loadResources();\",100); } }\nloadResources()",
                );
            }
            Library::DriverJs => {
                out.push_str(&css(DRIVERJS_CSS));
                out.push_str(&style(DRIVERJS_STYLE));
                out.push_str(&js(DRIVERJS_JS));
            }
            Library::Hopscotch => {
                out.push_str(&css(HOPSCOTCH_CSS));
                out.push_str(&style(HOPSCOTCH_STYLE));
                out.push_str(&js(HOPSCOTCH_JS));
            }
            Library::IntroJs => {
                out.push_str(&css(INTROJS_CSS));
                out.push_str(&style(&colors.style()));
                out.push_str(&js(INTROJS_JS));
            }
            Library::Shepherd => {
                out.push_str(&js(TETHER_JS));
                out.push_str(
                    "\nfunction loadResources() { if ( typeof Tether !== \"undefined\" ) {\n",
                );
                for sheet in SHEPHERD_CSS {
                    out.push_str(&css(sheet));
                }
                out.push_str(&style(SHEPHERD_STYLE));
                out.push_str(&js(SHEPHERD_JS));
                out.push_str(
                    "} else { window.setTimeout(\"loadResources();\",100); } }\nloadResources()",
                );
            }
        }
        out.push_str("\n\n////////  Tour Code  ////////\n\n");
        out.push_str(&format!(
            "function loadTour() {{ if ( typeof {} !== \"undefined\" ) {{\n",
            library.global()
        ));
        out.push_str(&self.steps_script(name, None));
        out.push_str(footer(library, false));
        out.push_str("\n} else { window.setTimeout(\"loadTour();\",100); } }\nloadTour()\n");
        out
    }
}

/// What ends the script of a tour: the end of the list of steps and the start.
fn footer(library: Library, play: bool) -> &'static str {
    match library {
        Library::Shepherd => SHEPHERD_FOOTER,
        Library::Bootstrap if play => BOOTSTRAP_FOOTER_PLAY,
        Library::Bootstrap => BOOTSTRAP_FOOTER_EXPORT,
        Library::DriverJs => DRIVERJS_FOOTER,
        Library::Hopscotch => HOPSCOTCH_FOOTER,
        Library::IntroJs if play => INTROJS_FOOTER,
        // The exported script ends with the call of `startIntro`, inside the
        // function that waits for IntroJS.
        Library::IntroJs => INTROJS_FOOTER,
    }
}

/// `selector` as Bootstrap Tour is given it: jQuery's `:first` is added so that
/// only the first match is used.
fn first_match_only(selector: &str) -> String {
    let last = selector.rsplit(' ').next().unwrap_or(selector);
    if last.contains(":first") {
        selector.to_owned()
    } else {
        format!("{selector}:first")
    }
}

fn bootstrap_step(
    step: &TourStep,
    selector: &str,
    alignment: &str,
    interval: Option<Duration>,
) -> String {
    let element = if selector == "html" {
        String::new()
    } else {
        format!("    element: {},\n", js_single(&first_match_only(selector)))
    };
    let duration = step
        .duration
        .filter(|duration| !duration.is_zero())
        .or(interval)
        .map_or(0, |duration| duration.as_millis());
    let backdrop = if selector == "html" { "false" } else { "true" };
    format!(
        "{{\n{element}    title: {title},\n    content: {content},\n    orphan: true,\n    \
         autoscroll: true,\n    backdrop: {backdrop},\n    placement: '{alignment}',\n    \
         smartPlacement: true,\n    duration: {duration},\n    }},\n",
        title = js_single(step.title.as_deref().unwrap_or_default()),
        content = js_single(&step.message),
    )
}

fn driverjs_step(title: &str, message: &str, selector: &str, alignment: &str) -> String {
    let message = format!("<font size=\"3\" color=\"#33477B\"><b>{message}</b></font>");
    // Without a title the message becomes the title and the description is
    // empty.
    let (title, description) = if title.is_empty() {
        (message, String::new())
    } else {
        (title.to_owned(), message)
    };
    let (selector, animate, position) = if matches!(selector, "html" | "body") {
        ("body", "false", "mid-center")
    } else {
        (selector, "true", alignment)
    };
    format!(
        "{{\n    element: {selector},\n    animate: {animate},\n    popover: {{\n      \
         className: 'popover-class',\n      title: {title},\n      description: {description},\n      \
         position: '{position}',\n    }}\n    }},\n",
        selector = js_single(selector),
        title = js_single(&title),
        description = js_single(&description),
    )
}

fn hopscotch_step(title: &str, message: &str, selector: &str, alignment: &str) -> String {
    let (target, alignment, arrow_offset) = if selector == "html" {
        ("head", "bottom", "    arrowOffset: '200',\n")
    } else {
        (selector, alignment, "")
    };
    format!(
        "{{\n    target: {target},\n    title: {title},\n    content: {content},\n{arrow_offset}    \
         showPrevButton: 'true',\n    scrollDuration: '550',\n    placement: '{alignment}'}},\n",
        target = js_single(target),
        title = js_single(title),
        content = js_single(message),
    )
}

fn introjs_step(title: &str, message: &str, selector: &str, alignment: &str) -> String {
    let element = if selector == "html" {
        String::new()
    } else {
        format!("element: {},\n", js_single(selector))
    };
    let message = if title.is_empty() {
        message.to_owned()
    } else {
        format!("<center><b>{title}</b></center><hr>{message}")
    };
    let message = format!("<font size=\"3\" color=\"#33477B\">{message}</font>");
    format!(
        "{{{element}intro: {intro},\nposition: '{alignment}'}},\n",
        intro = js_single(&message),
    )
}

/// The clock of an autoplaying tour: it tells when the step on show has been
/// there for the interval and the tour should go on.
///
/// It reads the Tokio clock, so a test that pauses the clock decides how much
/// time has passed.
#[derive(Debug)]
pub(crate) struct Autoplay {
    interval: Duration,
    key: Option<String>,
    deadline: Instant,
}

impl Autoplay {
    /// A clock for steps that each last `interval`.
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            interval,
            key: None,
            deadline: Instant::now() + interval,
        }
    }

    /// Notes which step is on show and returns whether it is time to go on.
    /// A step seen for the first time starts a new interval.
    pub(crate) fn observe(&mut self, key: &str) -> bool {
        if self.key.as_deref() == Some(key) {
            return Instant::now() >= self.deadline;
        }
        self.restart(key);
        false
    }

    /// Starts a new interval for the step `key`.
    pub(crate) fn restart(&mut self, key: &str) {
        self.key = Some(key.to_owned());
        self.deadline = Instant::now() + self.interval;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour(theme: impl Into<TourTheme>) -> Tour {
        let mut tour = Tour::new(theme);
        tour.add_step(TourStep::new("Welcome"));
        tour.add_step(TourStep {
            message: "Click it".into(),
            selector: Some("#go".into()),
            ..TourStep::default()
        });
        tour
    }

    fn plan(tour: &Tour) -> TourPlan {
        tour.plan("default", None, &IntroJsColors::default())
            .expect("a plan")
    }

    #[test]
    fn python_theme_names_are_read_in_any_case() {
        let read = |text: &str| text.parse::<TourTheme>().unwrap();
        assert_eq!(read("Bootstrap"), TourTheme::Bootstrap);
        assert_eq!(read("hopscotch"), TourTheme::Hopscotch);
        assert_eq!(read("INTRO"), TourTheme::IntroJs);
        assert_eq!(read("introjs"), TourTheme::IntroJs);
        assert_eq!(read("driver"), TourTheme::DriverJs);
        assert_eq!(read("DriverJS"), TourTheme::DriverJs);
        assert_eq!(read("shepherd"), TourTheme::Shepherd(ShepherdTheme::Light));
        assert_eq!(read("arrows"), TourTheme::Shepherd(ShepherdTheme::Light));
        assert_eq!(read("dark"), TourTheme::Shepherd(ShepherdTheme::Dark));
        assert_eq!(read("default"), TourTheme::Shepherd(ShepherdTheme::Default));
        assert_eq!(read("square"), TourTheme::Shepherd(ShepherdTheme::Square));
        assert_eq!(
            read("square-dark"),
            TourTheme::Shepherd(ShepherdTheme::SquareDark)
        );
        assert!("neon".parse::<TourTheme>().is_err());
        assert!("middle".parse::<TourAlignment>().is_err());
        assert_eq!(
            "Left".parse::<TourAlignment>().unwrap(),
            TourAlignment::Left
        );
        assert_eq!(TourTheme::default(), TourTheme::IntroJs);
    }

    #[test]
    fn a_name_or_none_is_the_default_name() {
        assert_eq!(name_or_default(None), "default");
        assert_eq!(name_or_default(Some("")), "default");
        assert_eq!(name_or_default(Some("intro")), "intro");
    }

    #[test]
    fn selectors_are_made_css() {
        assert_eq!(normalize_selector("name=q").unwrap(), "[name=\"q\"]");
        assert_eq!(normalize_selector("&q").unwrap(), "[name=\"q\"]");
        assert_eq!(normalize_selector("//div[@id='x']").unwrap(), "div[id='x']");
        assert_eq!(normalize_selector("#go > a").unwrap(), "#go > a");
        assert!(normalize_selector("//div[text()='x']").is_err());
    }

    #[test]
    fn a_shepherd_tour_has_the_python_buttons_classes_and_start() {
        let mut tour = Tour::new(ShepherdTheme::Dark);
        tour.add_step(TourStep::new("It's <b>here</b>"));
        tour.add_step(TourStep {
            message: "Click".into(),
            selector: Some("#go".into()),
            title: Some("Go".into()),
            theme: Some(ShepherdTheme::Square),
            alignment: Some(TourAlignment::Right),
            ..TourStep::default()
        });
        let script = plan(&tour).script;
        assert!(script.contains("classes: 'shepherd-theme-dark',\n        scrollTo: true"));
        assert!(script.contains("var firstStepButtons = [allButtons.skip, allButtons.next];"));
        // The first step: no element, so the orphan class; the first buttons;
        // the message with its quote escaped.
        assert!(script.contains("tour.addStep('default', {\n    title: '',\n    classes: 'shepherd-theme-dark shepherd-orphan',\n    text: 'It\\'s <b>here</b>',\n    attachTo: {element: 'html', on: 'top'},\n    buttons: firstStepButtons,"));
        // The second: its own theme, title and alignment; the other buttons.
        assert!(script.contains("title: 'Go',\n    classes: 'shepherd-theme-square',\n    text: 'Click',\n    attachTo: {element: '#go', on: 'right'},\n    buttons: midTourButtons,"));
        assert!(script.ends_with(&format!("tour.start();\n$tour = tour;\n{SHEPHERD_KEYS}")));
    }

    #[test]
    fn a_bootstrap_tour_takes_its_durations_from_the_step_or_the_interval() {
        let mut tour = Tour::new(TourTheme::Bootstrap);
        tour.add_step(TourStep::new("One"));
        tour.add_step(TourStep {
            message: "Two".into(),
            selector: Some("#go p".into()),
            duration: Some(Duration::from_secs(3)),
            alignment: Some(TourAlignment::Left),
            ..TourStep::default()
        });
        let manual = plan(&tour).script;
        assert!(manual.starts_with("// Bootstrap Tour\nvar tour = new Tour({"));
        assert!(manual.contains("backdrop: false,"));
        assert!(manual.contains("element: '#go p:first',"));
        assert!(manual.contains("placement: 'left',"));
        assert!(manual.contains("duration: 0,"));
        assert!(manual.contains("duration: 3000,"));
        assert!(manual.ends_with("tour.restart();\n// Save for later\n$tour = tour;\n"));

        // An interval fills the steps without a duration, with 1 s as the
        // least; a bootstrap tour keeps its own time, so the player has none.
        let colors = IntroJsColors::default();
        let timed = tour
            .plan("default", Some(Duration::from_millis(200)), &colors)
            .unwrap();
        assert!(timed.script.contains("duration: 1000,"));
        assert!(timed.script.contains("duration: 3000,"));
        assert_eq!(timed.autoplay, None);
    }

    #[test]
    fn a_driverjs_step_without_a_title_shows_its_message_as_the_title() {
        let script = plan(&tour(TourTheme::DriverJs)).script;
        assert!(script.contains("// DriverJS Tour\nvar tour = new Driver({"));
        assert!(script.contains("tour.defineSteps([\n"));
        let message = "<font size=\"3\" color=\"#33477B\"><b>Welcome</b></font>";
        assert!(script.contains(&format!(
            "element: 'body',\n    animate: false,\n    popover: {{\n      className: 'popover-class',\n      title: '{message}',\n      description: '',\n      position: 'mid-center',"
        )));
        assert!(script.contains("element: '#go',\n    animate: true,"));
        assert!(script.contains("position: 'bottom',"));

        let mut titled = Tour::new(TourTheme::DriverJs);
        titled.add_step(TourStep {
            message: "Body".into(),
            selector: Some("#go".into()),
            title: Some("Head".into()),
            ..TourStep::default()
        });
        let script = plan(&titled).script;
        assert!(script.contains("title: 'Head',"));
        assert!(script
            .contains("description: '<font size=\"3\" color=\"#33477B\"><b>Body</b></font>',"));
    }

    #[test]
    fn hopscotch_points_a_step_without_an_element_at_the_head() {
        let script = plan(&tour(TourTheme::Hopscotch)).script;
        assert!(script.starts_with("// Hopscotch Tour\nvar tour = {\nid: \"hopscotch_tour\","));
        assert!(script.contains(
            "target: 'head',\n    title: '',\n    content: 'Welcome',\n    arrowOffset: '200',\n    showPrevButton: 'true',\n    scrollDuration: '550',\n    placement: 'bottom'},"
        ));
        assert!(script.contains("target: '#go',"));
        assert!(script.contains("$tour = hopscotch;"));
    }

    #[test]
    fn an_introjs_step_puts_its_title_above_the_message() {
        let mut tour = Tour::new(TourTheme::IntroJs);
        tour.add_step(TourStep {
            message: "Body".into(),
            selector: Some("#go".into()),
            title: Some("Head".into()),
            alignment: Some(TourAlignment::Bottom),
            ..TourStep::default()
        });
        tour.add_step(TourStep::new("Plain"));
        let script = plan(&tour).script;
        assert!(
            script.starts_with("// IntroJS Tour\nfunction startIntro(){\nvar intro = introJs();")
        );
        assert!(script.contains(
            "{element: '#go',\nintro: '<font size=\"3\" color=\"#33477B\"><center><b>Head</b></center><hr>Body</font>',\nposition: 'bottom'},"
        ));
        assert!(script.contains(
            "{intro: '<font size=\"3\" color=\"#33477B\">Plain</font>',\nposition: 'top'},"
        ));
        assert!(script.contains("intro.setOption(\"nextToDone\", true);"));
        assert!(script.contains("$tour = intro;\n};\n// Start the tour\nstartIntro();\n"));
    }

    #[test]
    fn the_first_step_is_waited_for_only_when_it_has_an_element() {
        assert_eq!(plan(&tour(TourTheme::IntroJs)).first_selector, None);
        let mut tour = Tour::new(TourTheme::IntroJs);
        tour.add_step(TourStep {
            message: "x".into(),
            selector: Some("#first".into()),
            ..TourStep::default()
        });
        assert_eq!(plan(&tour).first_selector.as_deref(), Some("#first"));
    }

    #[test]
    fn a_tour_without_steps_cannot_be_played() {
        let error = Tour::new(TourTheme::IntroJs)
            .plan("empty", None, &IntroJsColors::default())
            .err()
            .expect("an error");
        assert!(
            error.to_string().contains("Tour {empty} has no steps!"),
            "{error}"
        );
    }

    #[test]
    fn an_interval_is_at_least_half_a_second_and_zero_is_none() {
        let colors = IntroJsColors::default();
        let tour = tour(TourTheme::IntroJs);
        let at = |interval| tour.plan("default", interval, &colors).unwrap().autoplay;
        assert_eq!(at(None), None);
        assert_eq!(at(Some(Duration::ZERO)), None);
        assert_eq!(at(Some(Duration::from_millis(100))), Some(MIN_INTERVAL));
        assert_eq!(
            at(Some(Duration::from_secs(2))),
            Some(Duration::from_secs(2))
        );
    }

    #[test]
    fn every_library_has_a_probe_and_an_advance_script_and_its_resources() {
        for theme in [
            TourTheme::Bootstrap,
            TourTheme::DriverJs,
            TourTheme::Hopscotch,
            TourTheme::IntroJs,
            TourTheme::Shepherd(ShepherdTheme::Light),
        ] {
            let library = tour(theme).library();
            assert!(library.probe_script().contains("return"), "{theme:?}");
            assert!(!library.advance_script().is_empty(), "{theme:?}");
            let resources = library.resources();
            assert!(
                resources.iter().any(|resource| resource.script),
                "{theme:?}"
            );
            assert!(resources
                .iter()
                .all(|resource| resource.url.starts_with("https://")));
        }
    }

    #[test]
    fn introjs_colours_must_be_hex_and_none_resets_them() {
        let colors = IntroJsColors::new(Some("#f26721"), Some("#abc")).unwrap();
        assert_eq!(
            (colors.theme_color(), colors.hover_color()),
            ("#f26721", "#abc")
        );
        let style = colors.style();
        assert!(style.contains(
            "background-color: #f26721 !important;\n    border: 1px solid #abc !important;"
        ));
        assert!(style.contains(".introjs-button.introjs-skipbutton {\n    color: #f26721;"));
        assert_eq!(
            IntroJsColors::new(None, None).unwrap(),
            IntroJsColors::default()
        );
        for bad in ["red", "#12", "#12345", "#1234567", "f26721", "#ggg", "#12 "] {
            let error = IntroJsColors::new(Some(bad), None).unwrap_err();
            assert_eq!(
                error.to_string(),
                "invalid configuration: Expecting a hex value color that starts with \"#\"!",
                "{bad}"
            );
            assert!(IntroJsColors::new(None, Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_exported_tour_loads_its_page_its_library_and_plays() {
        let mut tour = Tour::new(ShepherdTheme::default());
        tour.add_step(TourStep::new("Hi"));
        let script = tour.export_script(
            "default",
            "https://a.test/x?y=\"1\"",
            &IntroJsColors::default(),
        );
        assert!(script.starts_with("////////  Load Tour Start Page (if not there now)  ////////"));
        assert!(script.contains("if (window.location.href != \"https://a.test/x?y=\\\"1\\\"\") {"));
        assert!(script.contains("function injectCSS(css_link)"));
        assert!(script.contains(
            "injectJS(\"https://cdn.jsdelivr.net/npm/tether@1.4.7/dist/js/tether.min.js\");"
        ));
        assert!(script.contains("injectJS(\"https://cdnjs.cloudflare.com/ajax/libs/shepherd/1.8.1/js/shepherd.min.js\");"));
        assert!(script.contains("function loadTour() { if ( typeof Shepherd !== \"undefined\" ) {"));
        assert!(script.contains("tour.addStep('default', {"));
        assert!(
            script.ends_with("} else { window.setTimeout(\"loadTour();\",100); } }\nloadTour()\n")
        );
        // It has no keys of its own and no waiting for the first element.
        assert!(!script.contains("__sbTourKeys"));

        let mut intro = Tour::new(TourTheme::IntroJs);
        intro.add_step(TourStep::new("Hi"));
        let colors = IntroJsColors::new(Some("#111111"), None).unwrap();
        let script = intro.export_script("default", "data:,", &colors);
        assert!(script
            .contains("injectJS(\"https://cdn.jsdelivr.net/npm/intro.js@5.1.0/intro.min.js\");"));
        assert!(script.contains("background-color: #111111 !important;"));
        assert!(script.contains("typeof introJs !== \"undefined\""));
    }

    #[tokio::test(start_paused = true)]
    async fn a_step_is_due_when_it_has_been_shown_for_the_interval() {
        let mut clock = Autoplay::new(Duration::from_millis(500));
        // The first sight of a step starts its interval.
        assert!(!clock.observe("one"));
        tokio::time::advance(Duration::from_millis(499)).await;
        assert!(!clock.observe("one"));
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(clock.observe("one"));
        // The tour goes on: the next step gets a whole interval.
        clock.restart("two");
        assert!(!clock.observe("two"));
        tokio::time::advance(Duration::from_millis(500)).await;
        assert!(clock.observe("two"));
    }

    #[tokio::test(start_paused = true)]
    async fn a_step_the_person_moved_to_gets_a_new_interval() {
        let mut clock = Autoplay::new(Duration::from_secs(2));
        assert!(!clock.observe("one"));
        tokio::time::advance(Duration::from_millis(1900)).await;
        // The person pressed Next before the interval was up.
        assert!(!clock.observe("two"));
        tokio::time::advance(Duration::from_millis(1900)).await;
        assert!(!clock.observe("two"));
        tokio::time::advance(Duration::from_millis(100)).await;
        assert!(clock.observe("two"));
    }
}
