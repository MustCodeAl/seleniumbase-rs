//! The highlight family and Demo Mode: what `--demo` and `--slow` show on the
//! page, and the calls that draw attention to an element on purpose.
//!
//! Python's `highlight(selector, loops=None, scroll=True, timeout=None)` and its
//! relatives take keyword arguments, so here they return call builders: the
//! selector (and the text) stay positional, each keyword is a builder method with
//! Python's name, and `.await` runs the call (see [`crate::api::call`]):
//!
//! ```no_run
//! use std::time::Duration;
//! use seleniumbase_rs::api::base_case::BaseCase;
//! use seleniumbase_rs::browser::config::BrowserConfig;
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let mut sb = BaseCase::new(BrowserConfig::default()).await?;
//! sb.open("https://example.com").await?;
//! sb.highlight("h1").await?;                                    // 4 loops, scrolls first
//! sb.highlight("h1").loops(2).scroll(false).await?;             // Python: loops=2, scroll=False
//! sb.highlight_click("a").timeout(Duration::from_secs(3)).await?;
//! sb.highlight_type("#q", "rust").loops(1).await?;
//! sb.highlight_if_visible(".banner").await?;                    // never an error
//! sb.highlight_elements("li").limit(3).await?;
//! sb.flash("h1").duration(Duration::from_millis(1500)).color("FF8800").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Demo Mode
//!
//! With [`RunSettings::with_demo_mode`](crate::api::run_settings::RunSettings::with_demo_mode)
//! (`--demo`), or after [`BaseCase::activate_demo_mode`](crate::api::base_case::BaseCase::activate_demo_mode),
//! every action first scrolls its element to the middle of the page slowly and
//! plays the highlight animation on it (four loops of about 0.2 s), then acts, then
//! pauses for `--demo-sleep` (0.5 s by default; a third of that after an action
//! that did not change the page's URL). The assertions that Python's demo mode
//! announces (`assert_element`, `assert_text`, `assert_title`, ...) flash the
//! element green and show a Messenger success message such as
//! `ASSERT TEXT: {Welcome} in CSS SELECTOR: h1` (see [`assertion messages`](self#assertion-messages)).
//! Slow mode (`--slow`) only scrolls and pauses.
//!
//! The animation is a box shadow that cycles through six colours, 33 ms apart,
//! and is put back as it was found. `--highlights=N` overrides the number of loops
//! even when a call names its own, and where nobody watches (headless, or on a
//! virtual display) the loops are halved, rounded up.
//!
//! # Assertion messages
//!
//! The text of a demo-mode assertion message is HTML like Python's, but the
//! selector, text, attribute and value parts are escaped. The words are English
//! only.

use std::future::IntoFuture;
use std::time::Duration;

use crate::api::base_case::BaseCase;
use crate::api::call::CallFuture;
use crate::api::run_settings::DEFAULT_HIGHLIGHT_LOOPS;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::messenger::html_escape;
use crate::utils::selectors::Selector;

/// How far apart the steps of the highlight animation are, in milliseconds. Six
/// steps make a loop of about 0.2 s, as in Python.
pub(crate) const STEP_MS: u32 = 33;

/// How long a highlight waits after it ends (Python: 0.065 s).
pub(crate) const SETTLE: Duration = Duration::from_millis(65);

/// How long the highlight calls wait for the element to be visible unless the
/// call says otherwise (Python's `settings.SMALL_TIMEOUT`, 7 s).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(7);

/// How long the demo outline waits for an element that the action has already
/// waited for.
pub(crate) const DEMO_OUTLINE_TIMEOUT: Duration = Duration::from_secs(1);

/// What slow mode waits before and after its scroll (Python: 0.08 s and 0.12 s).
pub(crate) const SLOW_BEFORE: Duration = Duration::from_millis(80);
pub(crate) const SLOW_AFTER: Duration = Duration::from_millis(120);

/// The colour of the dot `flash` paints unless the call names one.
pub const DEFAULT_FLASH_COLOR: &str = "44CC88";

/// How many loops `highlight_click`, `highlight_type` and `highlight_update_text`
/// play unless the call or `--highlights` says otherwise.
pub(crate) const CLICK_LOOPS: u32 = 3;

/// The box shadow of every step of the highlight animation, after the first.
const HIGHLIGHT_COLOURS: [&str; 6] = [
    "rgba(255, 0, 0, 1)",
    "rgba(128, 0, 128, 1)",
    "rgba(0, 0, 255, 1)",
    "rgba(0, 255, 0, 1)",
    "rgba(128, 128, 0, 1)",
    "rgba(128, 0, 128, 1)",
];

/// The box shadows of the effect an assertion that passed has in demo mode.
const SUCCESS_COLOURS: [&str; 5] = [
    "rgba(128, 128, 128, 0.5)",
    "rgba(205, 30, 0, 1)",
    "rgba(128, 0, 128, 1)",
    "rgba(50, 50, 128, 1)",
    "rgba(50, 205, 50, 1)",
];

/// The start of every box shadow the animations set.
const SHADOW_PREFIX: &str = "0px 0px 6px 6px ";

/// Puts back the box shadow that `success_flash_script` saved on the element.
pub(crate) const RESTORE_SHADOW_SCRIPT: &str = "var el = arguments[0]; \
     if (!el) { return false; } \
     el.style.boxShadow = el.__sbShadow || ''; \
     delete el.__sbShadow; \
     return true;";

/// The piece of a script that scrolls `el` to the middle of the page, slowly, and
/// waits (for at most a second) until the page has stopped moving.
const SLOW_SCROLL: &str = "try { \
         el.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'smooth' }); \
         var last = null; var still = 0; \
         for (var t = 0; t < 40 && still < 3; t++) { \
             await sleep(25); \
             var now = window.scrollX + ',' + window.scrollY; \
             if (now === last) { still++; } else { still = 0; last = now; } \
         } \
     } catch (e) { try { el.scrollIntoView(); } catch (e2) {} }";

fn colour_list(colours: &[&str]) -> String {
    colours
        .iter()
        .map(|colour| format!("'{colour}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The script that scrolls the element it is given (`arguments[0]`) into view
/// slowly. It resolves to `true`.
pub(crate) fn slow_scroll_script() -> String {
    format!(
        "var el = arguments[0]; \
         if (!el) {{ return false; }} \
         return (async function (el) {{ \
             var sleep = function (ms) {{ return new Promise(function (done) {{ setTimeout(done, ms); }}); }}; \
             {SLOW_SCROLL} \
             return true; \
         }})(el);"
    )
}

/// The script of one highlight: scroll to the element it is given
/// (`arguments[0]`) when `scroll` is set, play `loops` loops of the six-colour box
/// shadow `step_ms` apart, and put the element's own box shadow back. It
/// resolves to `true` when the animation has ended (a WebDriver "execute script"
/// waits for the promise).
pub(crate) fn highlight_animation_script(loops: u32, step_ms: u32, scroll: bool) -> String {
    let scroll = if scroll { SLOW_SCROLL } else { "" };
    let colours = colour_list(&HIGHLIGHT_COLOURS);
    format!(
        "var el = arguments[0]; \
         if (!el) {{ return false; }} \
         return (async function (el) {{ \
             var sleep = function (ms) {{ return new Promise(function (done) {{ setTimeout(done, ms); }}); }}; \
             {scroll} \
             var original = el.style.boxShadow; \
             var colours = [{colours}]; \
             try {{ \
                 el.style.boxShadow = '{SHADOW_PREFIX}rgba(128, 128, 128, 0.5)'; \
                 for (var n = 0; n < {loops}; n++) {{ \
                     for (var i = 0; i < colours.length; i++) {{ \
                         el.style.boxShadow = '{SHADOW_PREFIX}' + colours[i]; \
                         await sleep({step_ms}); \
                     }} \
                 }} \
             }} finally {{ el.style.boxShadow = original; }} \
             return true; \
         }})(el);"
    )
}

/// The script of the effect a passed assertion has in demo mode: scroll to the
/// element (`arguments[0]`), keep its box shadow, and cycle the five colours of
/// Python's `highlight_with_js_2`. [`RESTORE_SHADOW_SCRIPT`] puts the shadow back
/// after the message has been shown.
pub(crate) fn success_flash_script(step_ms: u32, scroll: bool) -> String {
    let scroll = if scroll { SLOW_SCROLL } else { "" };
    let colours = colour_list(&SUCCESS_COLOURS);
    format!(
        "var el = arguments[0]; \
         if (!el) {{ return false; }} \
         return (async function (el) {{ \
             var sleep = function (ms) {{ return new Promise(function (done) {{ setTimeout(done, ms); }}); }}; \
             {scroll} \
             el.__sbShadow = el.style.boxShadow; \
             var colours = [{colours}]; \
             for (var i = 0; i < colours.length; i++) {{ \
                 el.style.boxShadow = '{SHADOW_PREFIX}' + colours[i]; \
                 await sleep({step_ms}); \
             }} \
             return true; \
         }})(el);"
    )
}

/// The script of `flash`: paint a dot of `#color` at the centre of the element
/// (`arguments[0]`) that fades out over `duration`, and take it away with a page
/// timer. `id` names the dot; `color` must be a hex colour
/// ([`is_hex_colour`]).
pub(crate) fn flash_script(color: &str, duration: Duration, id: &str) -> String {
    let seconds = duration.as_secs_f64();
    let millis = duration.as_millis();
    let rule = "@keyframes show-pointer-ani { \
         0% { opacity: 1; transform: scale(2, 2); } \
         25% { transform: scale(5, 5); } \
         50% { transform: scale(3, 3); } \
         75% { transform: scale(2, 2); } \
         100% { transform: scale(1, 1); opacity: 0; } }";
    format!(
        "var el = arguments[0]; \
         if (!el) {{ return false; }} \
         try {{ \
             if (el.scrollIntoViewIfNeeded) {{ el.scrollIntoViewIfNeeded(true); }} \
             else {{ el.scrollIntoView({{ block: 'center' }}); }} \
         }} catch (e) {{}} \
         var rect = el.getBoundingClientRect(); \
         var left = rect.left + window.pageXOffset + rect.width / 2 - 4; \
         var top = rect.top + window.pageYOffset + rect.height / 2 - 4; \
         var added = false; \
         var sheets = Array.prototype.slice.call(document.styleSheets); \
         for (var s = 0; s < sheets.length && !added; s++) {{ \
             try {{ sheets[s].insertRule('{rule}', sheets[s].cssRules.length); added = true; }} catch (e) {{}} \
         }} \
         if (!added) {{ \
             var style = document.createElement('style'); \
             style.textContent = '{rule}'; \
             document.head.appendChild(style); \
         }} \
         var dot = document.createElement('div'); \
         dot.id = '{id}'; \
         dot.style.cssText = 'position:absolute;z-index:99999999;padding:0;margin:0;' \
             + 'left:' + left.toFixed(1) + 'px;top:' + top.toFixed(1) + 'px;opacity:0.7;' \
             + 'width:8px;height:8px;border-radius:50%;background:#{color};' \
             + 'animation:show-pointer-ani {seconds:.2}s ease 1;'; \
         document.body.insertAdjacentElement('afterBegin', dot); \
         setTimeout(function () {{ \
             var gone = document.getElementById('{id}'); \
             if (gone) {{ gone.remove(); }} \
         }}, {millis}); \
         return true;"
    )
}

/// Whether `color` is something `flash` can paint with: three to eight hex
/// digits, with no `#` (Python: `color="44CC88"`).
pub(crate) fn is_hex_colour(color: &str) -> bool {
    (3..=8).contains(&color.len()) && color.chars().all(|c| c.is_ascii_hexdigit())
}

/// The selector a highlight looks for: `html` is `body`, as in Python.
pub(crate) fn highlight_target(css: &str) -> &str {
    if css == "html" {
        "body"
    } else {
        css
    }
}

/// How many loops a highlight plays. `requested` is the call's `loops` (a zero
/// is "not given", as in Python's `if not loops`), `method_default` the call's own
/// default, `setting` the run's `--highlights`, which wins over both. Where nobody
/// watches the count is halved, rounded up.
pub(crate) fn resolve_loops(
    requested: Option<u32>,
    method_default: u32,
    setting: Option<u32>,
    nobody_watches: bool,
) -> u32 {
    let mut loops = match requested.unwrap_or(method_default) {
        0 => DEFAULT_HIGHLIGHT_LOOPS,
        loops => loops,
    };
    if let Some(forced) = setting {
        loops = forced;
    }
    if nobody_watches {
        loops = loops.div_ceil(2);
    }
    loops
}

/// A wait in whole seconds, rounded up, for the waits that count seconds.
pub(crate) fn whole_seconds_up(wait: Duration) -> u64 {
    wait.as_secs() + u64::from(wait.subsec_nanos() > 0)
}

// ---------------------------------------------------------------------------
// The messages of passed assertions in demo mode (Python: `messenger_post`).
// ---------------------------------------------------------------------------

/// How Python names the way `css` finds its element in the messages.
pub(crate) fn strategy_word(css: &str) -> &'static str {
    match Selector::auto(css) {
        Selector::XPath(_) => "XPATH",
        Selector::LinkText(_) => "LINK TEXT",
        Selector::PartialLinkText(_) => "PARTIAL LINK TEXT",
        Selector::Css(_) | Selector::Id(_) => "CSS SELECTOR",
    }
}

/// `<b>ASSERT CSS SELECTOR</b>: h1`
pub(crate) fn element_message(css: &str) -> String {
    format!("<b>ASSERT {}</b>: {}", strategy_word(css), html_escape(css))
}

/// `<b>ASSERT TEXT</b>: {Welcome} in CSS SELECTOR: h1` (or `ASSERT EXACT TEXT`).
pub(crate) fn text_message(css: &str, text: &str, exact: bool) -> String {
    let label = if exact {
        "ASSERT EXACT TEXT"
    } else {
        "ASSERT TEXT"
    };
    format!(
        "<b>{label}</b>: {{{}}} in {}: {}",
        html_escape(text),
        strategy_word(css),
        html_escape(css)
    )
}

/// `<b>ASSERT NON-EMPTY TEXT</b> in CSS SELECTOR: h1`
pub(crate) fn non_empty_text_message(css: &str) -> String {
    format!(
        "<b>ASSERT NON-EMPTY TEXT</b> in {}: {}",
        strategy_word(css),
        html_escape(css)
    )
}

/// `<b>ASSERT ATTRIBUTE</b>: [href="/x"] in CSS SELECTOR: a`; without a value
/// `[href]`.
pub(crate) fn attribute_message(css: &str, attribute: &str, value: Option<&str>) -> String {
    let part = match value {
        Some(value) if !value.is_empty() => {
            format!("[{}=\"{}\"]", html_escape(attribute), html_escape(value))
        }
        _ => format!("[{}]", html_escape(attribute)),
    };
    format!(
        "<b>ASSERT ATTRIBUTE</b>: {part} in {}: {}",
        strategy_word(css),
        html_escape(css)
    )
}

/// `<b>ASSERT TITLE</b>: {Example}`: the label in bold, the text in braces.
pub(crate) fn braced_message(label: &str, text: &str) -> String {
    format!("<b>{label}</b>: {{{}}}", html_escape(text))
}

/// `<b>ASSERT NO 404 ERRORS</b>`: the label alone.
pub(crate) fn label_message(label: &str) -> String {
    format!("<b>{label}</b>")
}

/// `<b>ASSERT DOWNLOADED FILE</b>: [report.csv]`: the label and the name in
/// brackets.
pub(crate) fn bracketed_message(label: &str, text: &str) -> String {
    format!("<b>{label}</b>: [{}]", html_escape(text))
}

// ---------------------------------------------------------------------------
// The call builders.
// ---------------------------------------------------------------------------

/// Python's `highlight(selector, loops=None, scroll=True, timeout=None)`: wait
/// for the element to be visible, scroll to it slowly, outline it and put it
/// back.
///
/// It comes from [`BaseCase::highlight`]; the keywords are [`loops`](Self::loops)
/// (default 4, or `--highlights`), [`scroll`](Self::scroll) (default on) and
/// [`timeout`](Self::timeout) (default [`DEFAULT_TIMEOUT`]). Each loop lasts
/// about 0.2 s. An element that does not become visible within the timeout is the
/// usual wait error.
#[must_use = "nothing happens until the call is awaited"]
pub struct Highlight<'a> {
    pub(crate) sb: &'a BaseCase,
    pub(crate) selector: &'a str,
    pub(crate) loops: Option<u32>,
    pub(crate) scroll: bool,
    pub(crate) timeout: Duration,
}

impl<'a> Highlight<'a> {
    pub(crate) fn new(sb: &'a BaseCase, selector: &'a str) -> Self {
        Self {
            sb,
            selector,
            loops: None,
            scroll: true,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// How many times the animation repeats. Python's `loops`.
    pub fn loops(mut self, loops: u32) -> Self {
        self.loops = Some(loops);
        self
    }

    /// Whether to scroll to the element first. Python's `scroll`.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }

    /// How long to wait for the element to be visible. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl<'a> IntoFuture for Highlight<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.sb
                .run_highlight(self.selector, self.loops, self.scroll, self.timeout)
                .await
        })
    }
}

/// Python's `highlight_click(selector, loops=3, scroll=True, timeout=None)`:
/// wait for the element, highlight it (not in demo mode, where the click does
/// that), and click it. From [`BaseCase::highlight_click`]; the keywords are those
/// of [`Highlight`].
#[must_use = "nothing happens until the call is awaited"]
pub struct HighlightClick<'a> {
    pub(crate) sb: &'a mut BaseCase,
    pub(crate) selector: &'a str,
    pub(crate) loops: Option<u32>,
    pub(crate) scroll: bool,
    pub(crate) timeout: Duration,
}

impl<'a> HighlightClick<'a> {
    pub(crate) fn new(sb: &'a mut BaseCase, selector: &'a str) -> Self {
        Self {
            sb,
            selector,
            loops: None,
            scroll: true,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// How many times the animation repeats (default 3). Python's `loops`.
    pub fn loops(mut self, loops: u32) -> Self {
        self.loops = Some(loops);
        self
    }

    /// Whether to scroll to the element first. Python's `scroll`.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }

    /// How long to wait for the element to be visible. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl<'a> IntoFuture for HighlightClick<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                selector,
                loops,
                scroll,
                timeout,
            } = self;
            sb.run_highlight_click(selector, loops, scroll, timeout)
                .await
        })
    }
}

/// Python's `highlight_type(selector, text, loops=3, scroll=True,
/// timeout=None)` and `highlight_update_text` (the same call): wait for the
/// field, highlight it (not in demo mode), and replace its text with `text`.
/// From [`BaseCase::highlight_type`] and [`BaseCase::highlight_update_text`].
#[must_use = "nothing happens until the call is awaited"]
pub struct HighlightType<'a> {
    pub(crate) sb: &'a mut BaseCase,
    pub(crate) selector: &'a str,
    pub(crate) text: &'a str,
    pub(crate) loops: Option<u32>,
    pub(crate) scroll: bool,
    pub(crate) timeout: Duration,
}

impl<'a> HighlightType<'a> {
    pub(crate) fn new(sb: &'a mut BaseCase, selector: &'a str, text: &'a str) -> Self {
        Self {
            sb,
            selector,
            text,
            loops: None,
            scroll: true,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// How many times the animation repeats (default 3). Python's `loops`.
    pub fn loops(mut self, loops: u32) -> Self {
        self.loops = Some(loops);
        self
    }

    /// Whether to scroll to the field first. Python's `scroll`.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }

    /// How long to wait for the field to be visible. Python's `timeout`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl<'a> IntoFuture for HighlightType<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                sb,
                selector,
                text,
                loops,
                scroll,
                timeout,
            } = self;
            sb.run_highlight_type(selector, text, loops, scroll, timeout)
                .await
        })
    }
}

/// Python's `highlight_if_visible(selector, loops=4, scroll=True)`: highlight
/// the element when it is visible, and do nothing, with no error, when it is
/// not. From [`BaseCase::highlight_if_visible`].
#[must_use = "nothing happens until the call is awaited"]
pub struct HighlightIfVisible<'a> {
    pub(crate) sb: &'a BaseCase,
    pub(crate) selector: &'a str,
    pub(crate) loops: Option<u32>,
    pub(crate) scroll: bool,
}

impl<'a> HighlightIfVisible<'a> {
    pub(crate) fn new(sb: &'a BaseCase, selector: &'a str) -> Self {
        Self {
            sb,
            selector,
            loops: None,
            scroll: true,
        }
    }

    /// How many times the animation repeats. Python's `loops`.
    pub fn loops(mut self, loops: u32) -> Self {
        self.loops = Some(loops);
        self
    }

    /// Whether to scroll to the element first. Python's `scroll`.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }
}

impl<'a> IntoFuture for HighlightIfVisible<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.sb
                .run_highlight_if_visible(self.selector, self.loops, self.scroll)
                .await
        })
    }
}

/// Python's `highlight_elements(selector, loops=None, scroll=True, limit=0)`:
/// highlight every visible element the selector matches, one after the other,
/// up to `limit` of them (0 is all). From [`BaseCase::highlight_elements`].
#[must_use = "nothing happens until the call is awaited"]
pub struct HighlightElements<'a> {
    pub(crate) sb: &'a BaseCase,
    pub(crate) selector: &'a str,
    pub(crate) loops: Option<u32>,
    pub(crate) scroll: bool,
    pub(crate) limit: usize,
}

impl<'a> HighlightElements<'a> {
    pub(crate) fn new(sb: &'a BaseCase, selector: &'a str) -> Self {
        Self {
            sb,
            selector,
            loops: None,
            scroll: true,
            limit: 0,
        }
    }

    /// How many times the animation repeats. Python's `loops`.
    pub fn loops(mut self, loops: u32) -> Self {
        self.loops = Some(loops);
        self
    }

    /// Whether to scroll to each element first. Python's `scroll`.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }

    /// The most elements to highlight; 0 means all of them. Python's `limit`.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

impl<'a> IntoFuture for HighlightElements<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.sb
                .run_highlight_elements(self.selector, self.loops, self.scroll, self.limit)
                .await
        })
    }
}

/// Python's `flash(selector, duration=1, color="44CC88", pause=0)`: paint a
/// quickly-vanishing dot over the element. From [`BaseCase::flash`].
///
/// The dot is 8 px wide, grows and fades over [`duration`](Self::duration) (1 s),
/// has the [`color`](Self::color) `44CC88` (a hex colour without `#`), and the
/// call returns after [`pause`](Self::pause) (none, so the next action starts
/// during the flash). A colour that is not hex is an
/// [`InvalidConfig`](SeleniumBaseError::InvalidConfig) error.
#[must_use = "nothing happens until the call is awaited"]
pub struct Flash<'a> {
    pub(crate) sb: &'a BaseCase,
    pub(crate) selector: &'a str,
    pub(crate) duration: Duration,
    pub(crate) color: &'a str,
    pub(crate) pause: Duration,
}

impl<'a> Flash<'a> {
    pub(crate) fn new(sb: &'a BaseCase, selector: &'a str) -> Self {
        Self {
            sb,
            selector,
            duration: Duration::from_secs(1),
            color: DEFAULT_FLASH_COLOR,
            pause: Duration::ZERO,
        }
    }

    /// How long the dot lasts. Python's `duration`.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// The colour of the dot, as hex digits without `#`. Python's `color`.
    pub fn color(mut self, color: &'a str) -> Self {
        self.color = color;
        self
    }

    /// How long the call waits after it has painted the dot. Python's `pause`.
    pub fn pause(mut self, pause: Duration) -> Self {
        self.pause = pause;
        self
    }
}

impl<'a> IntoFuture for Flash<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.sb
                .run_flash(self.selector, self.duration, self.color, self.pause)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::config::BrowserConfig;

    fn assert_send<T: Send>(_: &T) {}

    fn case() -> BaseCase {
        BaseCase::without_session(BrowserConfig::default())
    }

    fn position(script: &str, needle: &str) -> usize {
        script
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} is not in {script}"))
    }

    #[test]
    fn the_animation_cycles_the_six_colours_in_pythons_order() {
        let script = highlight_animation_script(2, STEP_MS, true);
        let mut last = 0;
        for colour in [
            "rgba(255, 0, 0, 1)",
            "rgba(128, 0, 128, 1)",
            "rgba(0, 0, 255, 1)",
            "rgba(0, 255, 0, 1)",
            "rgba(128, 128, 0, 1)",
        ] {
            let at = position(&script, colour);
            assert!(at > last, "{colour} is out of order in {script}");
            last = at;
        }
        assert!(script.contains("rgba(128, 128, 128, 0.5)"), "{script}");
        assert!(script.contains("n < 2;"), "two loops: {script}");
        assert!(script.contains("await sleep(33)"), "{script}");
        assert!(script.contains("0px 0px 6px 6px"), "{script}");
    }

    #[test]
    fn the_animation_puts_the_boxshadow_back_even_when_it_fails() {
        let script = highlight_animation_script(1, STEP_MS, false);
        assert!(
            script.contains("var original = el.style.boxShadow;"),
            "{script}"
        );
        assert!(
            position(&script, "finally") < position(&script, "el.style.boxShadow = original"),
            "{script}"
        );
    }

    #[test]
    fn the_scroll_is_part_of_the_script_only_when_asked_for() {
        assert!(highlight_animation_script(1, STEP_MS, true).contains("behavior: 'smooth'"));
        assert!(!highlight_animation_script(1, STEP_MS, false).contains("scrollIntoView"));
        assert!(success_flash_script(STEP_MS, true).contains("behavior: 'smooth'"));
        assert!(!success_flash_script(STEP_MS, false).contains("scrollIntoView"));
    }

    #[test]
    fn the_assertion_effect_has_pythons_five_colours_and_keeps_the_shadow() {
        let script = success_flash_script(STEP_MS, false);
        let mut last = 0;
        for colour in SUCCESS_COLOURS {
            let at = position(&script, colour);
            assert!(at >= last, "{colour} is out of order in {script}");
            last = at;
        }
        assert!(
            script.contains("el.__sbShadow = el.style.boxShadow;"),
            "{script}"
        );
        assert!(RESTORE_SHADOW_SCRIPT.contains("el.style.boxShadow = el.__sbShadow"));
    }

    #[test]
    fn a_flash_paints_a_dot_of_the_colour_and_takes_it_away() {
        let script = flash_script("FF8800", Duration::from_millis(1500), "flash-abc");
        assert!(script.contains("background:#FF8800;"), "{script}");
        assert!(script.contains("show-pointer-ani 1.50s ease 1"), "{script}");
        assert!(
            script.contains("width:8px;height:8px;border-radius:50%"),
            "{script}"
        );
        assert!(script.contains("dot.id = 'flash-abc';"), "{script}");
        assert!(script.contains("}, 1500);"), "{script}");
        assert!(script.contains("@keyframes show-pointer-ani"), "{script}");
    }

    #[test]
    fn only_hex_digits_make_a_flash_colour() {
        for good in ["44CC88", "fff", "FF8800AA"] {
            assert!(is_hex_colour(good), "{good}");
        }
        for bad in ["", "#44CC88", "red", "44CC88; x", "44CC88'", "1234567890"] {
            assert!(!is_hex_colour(bad), "{bad}");
        }
    }

    #[test]
    fn the_loops_follow_pythons_rules() {
        // The argument, else the method's default; zero is "not given" and
        // means Python's HIGHLIGHTS (4).
        assert_eq!(resolve_loops(None, 4, None, false), 4);
        assert_eq!(resolve_loops(None, 3, None, false), 3);
        assert_eq!(resolve_loops(Some(2), 3, None, false), 2);
        assert_eq!(resolve_loops(Some(0), 3, None, false), 4);
        // --highlights wins even over an argument.
        assert_eq!(resolve_loops(Some(2), 3, Some(7), false), 7);
        // Nobody watching: halved, rounded up.
        assert_eq!(resolve_loops(None, 4, None, true), 2);
        assert_eq!(resolve_loops(None, 3, None, true), 2);
        assert_eq!(resolve_loops(Some(1), 3, None, true), 1);
        assert_eq!(resolve_loops(Some(2), 3, Some(5), true), 3);
    }

    #[test]
    fn html_is_body_and_a_wait_is_rounded_up_to_whole_seconds() {
        assert_eq!(highlight_target("html"), "body");
        assert_eq!(highlight_target("html > body"), "html > body");
        assert_eq!(whole_seconds_up(Duration::from_millis(1)), 1);
        assert_eq!(whole_seconds_up(Duration::from_secs(7)), 7);
        assert_eq!(whole_seconds_up(Duration::from_millis(2500)), 3);
        assert_eq!(whole_seconds_up(Duration::ZERO), 0);
    }

    #[test]
    fn the_assertion_messages_are_pythons_with_the_parts_escaped() {
        assert_eq!(element_message("#m"), "<b>ASSERT CSS SELECTOR</b>: #m");
        assert_eq!(
            element_message("//div[@id='a']"),
            "<b>ASSERT XPATH</b>: //div[@id=&#x27;a&#x27;]".replace("&#x27;", &html_escape("'"))
        );
        assert_eq!(
            text_message("#m", "Hi", false),
            "<b>ASSERT TEXT</b>: {Hi} in CSS SELECTOR: #m"
        );
        assert_eq!(
            text_message("#m", "Hi <b>", true),
            "<b>ASSERT EXACT TEXT</b>: {Hi &lt;b&gt;} in CSS SELECTOR: #m"
        );
        assert_eq!(
            non_empty_text_message("p"),
            "<b>ASSERT NON-EMPTY TEXT</b> in CSS SELECTOR: p"
        );
        assert_eq!(
            attribute_message("a", "href", None),
            "<b>ASSERT ATTRIBUTE</b>: [href] in CSS SELECTOR: a"
        );
        assert_eq!(
            attribute_message("a", "href", Some("/x")),
            format!(
                "<b>ASSERT ATTRIBUTE</b>: [href=\"{}\"] in CSS SELECTOR: a",
                html_escape("/x")
            )
        );
        assert_eq!(
            braced_message("ASSERT TITLE CONTAINS", "Exa"),
            "<b>ASSERT TITLE CONTAINS</b>: {Exa}"
        );
        assert_eq!(
            label_message("ASSERT NO 404 ERRORS"),
            "<b>ASSERT NO 404 ERRORS</b>"
        );
        assert_eq!(
            bracketed_message("ASSERT DOWNLOADED FILE", "a.csv"),
            "<b>ASSERT DOWNLOADED FILE</b>: [a.csv]"
        );
    }

    #[test]
    fn the_strategy_word_names_how_the_selector_finds_its_element() {
        assert_eq!(strategy_word("#a"), "CSS SELECTOR");
        assert_eq!(strategy_word("//a"), "XPATH");
        assert_eq!(strategy_word("link=Home"), "LINK TEXT");
        assert_eq!(strategy_word("partial_link=Ho"), "PARTIAL LINK TEXT");
    }

    #[test]
    fn the_builders_start_with_pythons_defaults() {
        let mut sb = case();
        let highlight = sb.highlight("#a");
        assert_eq!(
            (highlight.loops, highlight.scroll, highlight.timeout),
            (None, true, DEFAULT_TIMEOUT)
        );
        let custom = sb
            .highlight("#a")
            .loops(2)
            .scroll(false)
            .timeout(Duration::from_secs(1));
        assert_eq!(
            (custom.loops, custom.scroll, custom.timeout),
            (Some(2), false, Duration::from_secs(1))
        );
        let elements = sb.highlight_elements("li").limit(3);
        assert_eq!((elements.limit, elements.scroll), (3, true));
        assert_eq!(sb.highlight_elements("li").limit, 0);
        let flash = sb.flash("#a");
        assert_eq!(
            (flash.duration, flash.color, flash.pause),
            (Duration::from_secs(1), "44CC88", Duration::ZERO)
        );
        let flash = sb
            .flash("#a")
            .duration(Duration::from_secs(2))
            .color("FFF")
            .pause(Duration::from_millis(5));
        assert_eq!(
            (flash.duration, flash.color, flash.pause),
            (Duration::from_secs(2), "FFF", Duration::from_millis(5))
        );
        let click = sb.highlight_click("#a").loops(1);
        assert_eq!(click.loops, Some(1));
        let typed = sb.highlight_type("#a", "x").scroll(false);
        assert!(!typed.scroll);
    }

    #[test]
    fn the_futures_of_the_builders_are_send() {
        let mut sb = case();
        assert_send(&sb.highlight("#a").into_future());
        assert_send(&sb.highlight_if_visible("#a").into_future());
        assert_send(&sb.highlight_elements("#a").into_future());
        assert_send(&sb.flash("#a").into_future());
        assert_send(&sb.highlight_click("#a").into_future());
        assert_send(&sb.highlight_type("#a", "x").into_future());
        assert_send(&sb.highlight_update_text("#a", "x").into_future());
    }
}
