//! Slide presentations rendered with reveal.js.
//!
//! The decks are those of Python's `create_presentation`, `add_slide` and
//! `save_presentation`. A [`Presentation`] has a [`PresentationTheme`] (Python's
//! `theme`, default `serif`), a default [`Transition`] (default `none`) and a
//! list of [`Slide`]s. A slide is HTML (`content`) and, optionally, an `image`,
//! a block of `code`, an `iframe`, more HTML after those (`content2`), speaker
//! `notes` and a transition of its own. [`Presentation::save`] writes the deck
//! as one HTML file in the `saved_presentations` folder, `my_presentation.html`
//! by default.
//!
//! The HTML of a slide (`content`, `content2`, `notes`) is written to the page
//! as given, because Python takes HTML there too. The `code` is shown as text,
//! and the `image` and `iframe` addresses are escaped. Do not pass the text of
//! a page you do not control as slide content.
//!
//! The file is not offline-capable: it loads a pinned version of reveal.js and
//! its theme from the jsDelivr CDN and the code highlighter from cdnjs, so the
//! browser that opens it needs network access.
//!
//! [`BaseCase`](crate::api::base_case::BaseCase) keeps any number of named
//! presentations (`create_presentation`, `add_slide`, `save_presentation`,
//! `begin_presentation`); this module is the part that needs no browser.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::api::presentation::{Presentation, PresentationTheme, Slide, Transition};
//! use std::time::Duration;
//!
//! let mut deck = Presentation::new(PresentationTheme::Sky, Transition::Slide);
//! deck.add_slide(Slide::new("<h1>What's new</h1>").with_notes("Mention the new waits"));
//! deck.add_slide(Slide::new("<h2>Thanks</h2>").with_transition(Transition::Fade));
//!
//! // Notes shown, a new slide every five seconds.
//! let html = deck.to_html(true, Duration::from_secs(5), false);
//! assert!(html.contains("theme/sky.css"));
//! assert!(html.contains("showNotes: true"));
//! assert!(html.contains("autoSlide: 5000"));
//! assert_eq!(html.matches("<section").count(), 2);
//! ```

use crate::api::chart::{html_escape, CHART_JS_TAG};
use crate::artifacts::bare_file_name;
use crate::error::SeleniumBaseError;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

/// The folder, in the working directory, where presentations are saved:
/// Python's `Presentations.SAVED_FOLDER`.
pub const SAVED_FOLDER: &str = "saved_presentations";

/// The file a presentation is saved to when no name is given.
pub const DEFAULT_FILENAME: &str = "my_presentation.html";

/// The name of a presentation (or a chart) when no name is given.
pub const DEFAULT_NAME: &str = "default";

/// The reveal.js release the saved page loads. Pinned, so that a new release
/// cannot change how a saved deck looks.
const REVEAL_BASE_URL: &str = "https://cdn.jsdelivr.net/npm/reveal.js@4.6.1/dist";

/// The code highlighter that formats a slide's `code`.
const PRETTIFY_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/prettify/r298/run_prettify.min.js";

/// What the page shows between the charts of several slides, in place of the
/// second and later copies of the chart library.
const CHARTS_ADDED: &str = "<!--Charts Added-->";

/// The element that is visible while the last, empty slide of a shown
/// presentation is the current one. `begin_presentation` waits for it.
pub(crate) const END_MARKER_SELECTOR: &str = "section.present p.End_Presentation_Now";

/// The empty last slide of a presentation that is being shown.
const END_MARKER_SLIDE: &str =
    "\n<section data-transition=\"none\">\n<p class=\"End_Presentation_Now\"> </p>\n</section>\n";

/// The styles of the slide parts: code blocks, the image and the frame.
const STYLE: &str = "<style>\n\
pre{background-color:#fbe8ff;border-radius:8px;}\n\
div[flex_div]{height:68vh;margin:0;align-items:center;justify-content:center;}\n\
img[rounded]{border-radius:16px;max-width:64%;}\n\
</style>\n";

/// A reveal.js theme: Python's `theme`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PresentationTheme {
    /// `serif`, the default.
    #[default]
    Serif,
    /// `white`.
    White,
    /// `black`.
    Black,
    /// `beige`.
    Beige,
    /// `simple`.
    Simple,
    /// `sky`.
    Sky,
    /// `league`.
    League,
    /// `moon`.
    Moon,
    /// `night`.
    Night,
    /// `blood`.
    Blood,
    /// `solarized`.
    Solarized,
}

impl PresentationTheme {
    /// Every theme, in the order Python lists them.
    pub const ALL: [Self; 11] = [
        Self::Serif,
        Self::White,
        Self::Black,
        Self::Beige,
        Self::Simple,
        Self::Sky,
        Self::League,
        Self::Moon,
        Self::Night,
        Self::Blood,
        Self::Solarized,
    ];

    /// The theme's name, which is also the name of its reveal.js stylesheet.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Serif => "serif",
            Self::White => "white",
            Self::Black => "black",
            Self::Beige => "beige",
            Self::Simple => "simple",
            Self::Sky => "sky",
            Self::League => "league",
            Self::Moon => "moon",
            Self::Night => "night",
            Self::Blood => "blood",
            Self::Solarized => "solarized",
        }
    }
}

impl fmt::Display for PresentationTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PresentationTheme {
    type Err = SeleniumBaseError;

    /// Reads a theme name, ignoring case. An empty name and `default` are the
    /// default theme, as in Python.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Theme {NAME} not found! Valid themes: [...]`.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let lower = name.to_lowercase();
        if lower.is_empty() || lower == "default" {
            return Ok(Self::default());
        }
        Self::ALL
            .into_iter()
            .find(|theme| theme.as_str() == lower)
            .ok_or_else(|| {
                SeleniumBaseError::InvalidConfig(format!(
                    "Theme {{{name}}} not found! Valid themes: {}",
                    python_list(Self::ALL.iter().map(|theme| theme.as_str()))
                ))
            })
    }
}

/// How one slide gives way to the next: Python's `transition`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Transition {
    /// `none`, the default.
    #[default]
    None,
    /// `slide`.
    Slide,
    /// `fade`.
    Fade,
    /// `zoom`.
    Zoom,
    /// `convex`.
    Convex,
    /// `concave`.
    Concave,
}

impl Transition {
    /// Every transition, in the order Python lists them.
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::Slide,
        Self::Fade,
        Self::Zoom,
        Self::Convex,
        Self::Concave,
    ];

    /// The transition's name, as reveal.js knows it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Slide => "slide",
            Self::Fade => "fade",
            Self::Zoom => "zoom",
            Self::Convex => "convex",
            Self::Concave => "concave",
        }
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Transition {
    type Err = SeleniumBaseError;

    /// Reads a transition name, ignoring case. An empty name and `default` are
    /// `none`, as in Python.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Transition {NAME} not found! Valid transitions: [...]`.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let lower = name.to_lowercase();
        if lower.is_empty() || lower == "default" {
            return Ok(Self::default());
        }
        Self::ALL
            .into_iter()
            .find(|transition| transition.as_str() == lower)
            .ok_or_else(|| {
                SeleniumBaseError::InvalidConfig(format!(
                    "Transition {{{name}}} not found! Valid transitions: {}",
                    python_list(Self::ALL.iter().map(|transition| transition.as_str()))
                ))
            })
    }
}

/// `items` written as a Python list of strings, as Python's error texts show
/// the valid names.
fn python_list<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let quoted: Vec<String> = items.map(|item| format!("'{item}'")).collect();
    format!("[{}]", quoted.join(", "))
}

/// One slide: Python's `add_slide(content, image, code, iframe, content2,
/// notes, transition)`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Slide {
    /// The HTML of the slide, written to the page as given.
    pub content: String,
    /// The address of an image to show below the content.
    pub image: Option<String>,
    /// Code to show below the content, with syntax highlighting. It is shown
    /// as text.
    pub code: Option<String>,
    /// The address of a page to show in a frame below the content.
    pub iframe: Option<String>,
    /// HTML to show after the image, the code and the frame.
    pub content2: Option<String>,
    /// Speaker notes, as HTML. They are only seen when the presentation is
    /// saved or shown with notes.
    pub notes: Option<String>,
    /// The transition into this slide, instead of the presentation's.
    pub transition: Option<Transition>,
}

impl Slide {
    /// A slide with the HTML `content` and nothing else.
    #[must_use]
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_owned(),
            ..Self::default()
        }
    }

    /// Shows the image at `url` below the content.
    #[must_use]
    pub fn with_image(mut self, url: &str) -> Self {
        self.image = Some(url.to_owned());
        self
    }

    /// Shows `code` below the content, with syntax highlighting.
    #[must_use]
    pub fn with_code(mut self, code: &str) -> Self {
        self.code = Some(code.to_owned());
        self
    }

    /// Shows the page at `url` in a frame below the content.
    #[must_use]
    pub fn with_iframe(mut self, url: &str) -> Self {
        self.iframe = Some(url.to_owned());
        self
    }

    /// Shows the HTML `content2` after the image, the code and the frame.
    #[must_use]
    pub fn with_content2(mut self, content2: &str) -> Self {
        self.content2 = Some(content2.to_owned());
        self
    }

    /// Adds the speaker notes `notes` (HTML).
    #[must_use]
    pub fn with_notes(mut self, notes: &str) -> Self {
        self.notes = Some(notes.to_owned());
        self
    }

    /// Gives this slide a transition of its own.
    #[must_use]
    pub fn with_transition(mut self, transition: Transition) -> Self {
        self.transition = Some(transition);
        self
    }
}

/// `text`, unless it is missing or empty: Python tests these arguments for
/// truth, so an empty string is the same as none.
fn present(text: Option<&str>) -> Option<&str> {
    text.filter(|text| !text.is_empty())
}

/// A deck of slides with a theme and a default transition.
#[derive(Clone, Debug, Default)]
pub struct Presentation {
    theme: PresentationTheme,
    transition: Transition,
    slides: Vec<Slide>,
}

impl Presentation {
    /// An empty presentation with the given theme and default transition.
    #[must_use]
    pub fn new(theme: PresentationTheme, transition: Transition) -> Self {
        Self {
            theme,
            transition,
            slides: Vec::new(),
        }
    }

    /// The theme of the deck.
    #[must_use]
    pub fn theme(&self) -> PresentationTheme {
        self.theme
    }

    /// The transition a slide has unless it sets its own.
    #[must_use]
    pub fn transition(&self) -> Transition {
        self.transition
    }

    /// The slides, in order.
    #[must_use]
    pub fn slides(&self) -> &[Slide] {
        &self.slides
    }

    /// Adds `slide` at the end.
    pub fn add_slide(&mut self, slide: Slide) {
        self.slides.push(slide);
    }

    /// The `<section>` of one slide, in Python's layout: the content, then the
    /// image, the code, the frame, `content2` and the notes.
    fn section(&self, slide: &Slide) -> String {
        let transition = slide.transition.unwrap_or(self.transition);
        // Python starts a new line before HTML, so the markup reads well.
        let lead = |text: &str| if text.starts_with('<') { "\n" } else { "" };
        let mut html = format!(
            "\n<section data-transition=\"{transition}\">{}{}",
            lead(&slide.content),
            slide.content
        );
        if let Some(image) = present(slide.image.as_deref()) {
            let _ = write!(
                html,
                "\n<div flex_div><img rounded src=\"{}\" /></div>",
                html_escape(image)
            );
        }
        if let Some(code) = present(slide.code.as_deref()) {
            let _ = write!(
                html,
                "\n<div></div>\n<pre class=\"prettyprint\">\n{}</pre>",
                html_escape(code)
            );
        }
        if let Some(iframe) = present(slide.iframe.as_deref()) {
            let _ = write!(
                html,
                "\n<div></div>\n<iframe src=\"{}\" style=\"width:92%;height:550px;\" \
                 title=\"iframe content\"></iframe>",
                html_escape(iframe)
            );
        }
        if let Some(content2) = present(slide.content2.as_deref()) {
            html.push_str(lead(content2));
            html.push_str(content2);
        }
        let _ = write!(
            html,
            "\n<aside class=\"notes\">{}</aside>\n</section>\n",
            slide.notes.as_deref().unwrap_or_default()
        );
        html
    }

    /// The deck as one HTML page.
    ///
    /// With `show_notes` the speaker notes are shown next to the slides. A
    /// non-zero `interval` plays the deck by itself, a new slide every
    /// `interval`; zero turns that off. With `end_marker` an empty last slide
    /// is added, which `begin_presentation` waits for.
    ///
    /// A chart that was put on a slide brings its own copy of the chart
    /// library; the page keeps the first copy only.
    #[must_use]
    pub fn to_html(&self, show_notes: bool, interval: Duration, end_marker: bool) -> String {
        let mut html = String::from(
            "<html>\n<head>\n<meta charset=\"utf-8\">\n\
             <meta http-equiv=\"Content-Type\" content=\"text/html; charset=utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0, \
             maximum-scale=1.0, user-scalable=no, minimal-ui\">\n",
        );
        let _ = write!(
            html,
            "<link rel=\"stylesheet\" href=\"{REVEAL_BASE_URL}/reveal.css\">\n\
             <link rel=\"stylesheet\" href=\"{REVEAL_BASE_URL}/theme/{}.css\">\n",
            self.theme
        );
        html.push_str(STYLE);
        html.push_str(
            "</head>\n\n<body>\n<!-- Generated by seleniumbase-rs -->\n\
             <div class=\"reveal\">\n<div class=\"slides\">\n",
        );
        for slide in &self.slides {
            html.push_str(&self.section(slide));
        }
        if end_marker {
            html.push_str(END_MARKER_SLIDE);
        }
        let _ = write!(
            html,
            "\n</div>\n</div>\n\
             <script src=\"{REVEAL_BASE_URL}/reveal.js\"></script>\n\
             <script src=\"{PRETTIFY_URL}\"></script>\n\
             <script>Reveal.initialize(\
             {{showNotes: {show_notes}, slideNumber: true, progress: true, hash: false, \
             autoSlide: {},}});</script>\n</body>\n</html>\n",
            interval.as_millis()
        );
        keep_first_chart_libs(&mut html);
        html
    }

    /// Writes the deck to `filename` in the `saved_presentations` folder of the
    /// working directory, which is made if it is missing, and returns the path.
    /// See [`to_html`](Self::to_html) for `show_notes` and `interval`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `filename` does not end
    /// in `.html` (Python's text, `Presentation file must end in ".html"!`) or
    /// has a directory part, and an I/O error if the file cannot be written.
    pub fn save(
        &self,
        filename: &str,
        show_notes: bool,
        interval: Duration,
    ) -> Result<PathBuf, SeleniumBaseError> {
        self.save_in(Path::new(SAVED_FOLDER), filename, show_notes, interval)
    }

    /// [`save`](Self::save) into `folder` instead of `saved_presentations`.
    ///
    /// # Errors
    ///
    /// The same as [`save`](Self::save).
    pub fn save_in(
        &self,
        folder: &Path,
        filename: &str,
        show_notes: bool,
        interval: Duration,
    ) -> Result<PathBuf, SeleniumBaseError> {
        self.write_in(folder, filename, show_notes, interval, false)
    }

    /// Writes the page to `filename` in `folder`, with or without the end
    /// marker slide, and returns the path.
    pub(crate) fn write_in(
        &self,
        folder: &Path,
        filename: &str,
        show_notes: bool,
        interval: Duration,
        end_marker: bool,
    ) -> Result<PathBuf, SeleniumBaseError> {
        check_filename(filename)?;
        let path = folder.join(filename);
        fs::create_dir_all(folder).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to create the folder '{}' for the presentation: {error}",
                folder.display()
            ))
        })?;
        fs::write(&path, self.to_html(show_notes, interval, end_marker)).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to write presentation '{}': {error}",
                path.display()
            ))
        })?;
        Ok(path)
    }
}

/// Checks the name of a presentation file: it must end in `.html` and be one
/// plain file name, so it cannot leave the folder it is saved to.
pub(crate) fn check_filename(filename: &str) -> Result<(), SeleniumBaseError> {
    if !filename.ends_with(".html") {
        return Err(SeleniumBaseError::InvalidConfig(
            "Presentation file must end in \".html\"!".to_owned(),
        ));
    }
    bare_file_name(filename).map(|_| ())
}

/// The interval a presentation or a chart waits or plays at: `requested`, or,
/// if that is zero, the run's `--interval`, or zero (Python's
/// `if interval == 0 and self.interval`).
pub(crate) fn resolve_interval(requested: Duration, run_interval: Option<Duration>) -> Duration {
    if requested.is_zero() {
        run_interval.unwrap_or(Duration::ZERO)
    } else {
        requested
    }
}

/// Keeps the first copy of the chart library in `html` and turns the others
/// into a comment, so that charts on several slides load it once. The first
/// copy comes before every chart script that follows it.
fn keep_first_chart_libs(html: &mut String) {
    if let Some(first) = html.find(CHART_JS_TAG) {
        let split = first + CHART_JS_TAG.len();
        let rest = html[split..].replace(CHART_JS_TAG, CHARTS_ADDED);
        html.truncate(split);
        html.push_str(&rest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck() -> Presentation {
        Presentation::new(PresentationTheme::default(), Transition::default())
    }

    #[test]
    fn the_defaults_are_the_serif_theme_and_no_transition() {
        let mut deck = deck();
        deck.add_slide(Slide::new("<h1>Hi</h1>"));
        let html = deck.to_html(false, Duration::ZERO, false);
        assert!(
            html.contains("reveal.js@4.6.1/dist/theme/serif.css"),
            "{html}"
        );
        assert!(html.contains("reveal.js@4.6.1/dist/reveal.css"));
        assert!(html.contains("reveal.js@4.6.1/dist/reveal.js\""));
        assert!(html.contains("<section data-transition=\"none\">\n<h1>Hi</h1>"));
        assert!(html.contains("showNotes: false"));
        assert!(html.contains("autoSlide: 0,"));
        assert!(!html.contains("End_Presentation_Now"));
        assert!(!html.contains("reveal.js@4/"), "no floating major version");
    }

    #[test]
    fn every_theme_and_transition_has_python_s_name() {
        let names: Vec<&str> = PresentationTheme::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            names,
            [
                "serif",
                "white",
                "black",
                "beige",
                "simple",
                "sky",
                "league",
                "moon",
                "night",
                "blood",
                "solarized"
            ]
        );
        for theme in PresentationTheme::ALL {
            let html =
                Presentation::new(theme, Transition::Slide).to_html(false, Duration::ZERO, false);
            assert!(
                html.contains(&format!("/theme/{}.css", theme.as_str())),
                "{theme}"
            );
        }
        let names: Vec<&str> = Transition::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            names,
            ["none", "slide", "fade", "zoom", "convex", "concave"]
        );
        for transition in Transition::ALL {
            let mut deck = Presentation::new(PresentationTheme::Serif, transition);
            deck.add_slide(Slide::new("x"));
            let html = deck.to_html(false, Duration::ZERO, false);
            assert!(
                html.contains(&format!("data-transition=\"{transition}\"")),
                "{transition}"
            );
        }
    }

    #[test]
    fn a_slide_transition_overrides_the_presentation_s() {
        let mut deck = Presentation::new(PresentationTheme::Serif, Transition::Zoom);
        deck.add_slide(Slide::new("a"));
        deck.add_slide(Slide::new("b").with_transition(Transition::Fade));
        let html = deck.to_html(false, Duration::ZERO, false);
        assert_eq!(html.matches("data-transition=\"zoom\"").count(), 1);
        assert_eq!(html.matches("data-transition=\"fade\"").count(), 1);
    }

    #[test]
    fn the_parts_of_a_slide_come_in_python_s_order() {
        let mut deck = deck();
        deck.add_slide(
            Slide::new("<h2>Title</h2>")
                .with_image("https://example.test/a.png?x=1&y=2")
                .with_code("let a = 1 < 2;")
                .with_iframe("https://example.test/frame")
                .with_content2("<p>after</p>")
                .with_notes("say hello"),
        );
        let html = deck.to_html(true, Duration::ZERO, false);
        let positions: Vec<usize> = [
            "<h2>Title</h2>",
            "<div flex_div><img rounded src=\"https://example.test/a.png?x=1&amp;y=2\" /></div>",
            "<pre class=\"prettyprint\">\nlet a = 1 &lt; 2;</pre>",
            "<iframe src=\"https://example.test/frame\"",
            "\n<p>after</p>",
            "<aside class=\"notes\">say hello</aside>",
        ]
        .iter()
        .map(|part| {
            html.find(part)
                .unwrap_or_else(|| panic!("{part} missing in {html}"))
        })
        .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{positions:?}"
        );
        assert!(html.contains("showNotes: true"));
    }

    #[test]
    fn plain_text_content_stays_on_the_section_line_and_html_starts_a_new_one() {
        let mut deck = deck();
        deck.add_slide(Slide::new("plain text").with_content2("more"));
        let html = deck.to_html(false, Duration::ZERO, false);
        assert!(
            html.contains("<section data-transition=\"none\">plain text"),
            "{html}"
        );
        assert!(
            html.contains("plain textmore"),
            "no line before text: {html}"
        );
    }

    #[test]
    fn the_html_of_a_slide_is_not_escaped_but_a_frame_address_is() {
        let mut deck = deck();
        deck.add_slide(Slide::new("<b>bold</b>").with_iframe("x\" onload=\"alert(1)"));
        let html = deck.to_html(false, Duration::ZERO, false);
        assert!(html.contains("<b>bold</b>"));
        assert!(!html.contains("\" onload=\"alert(1)"), "{html}");
        assert!(html.contains("x&quot; onload=&quot;alert(1)"));
    }

    #[test]
    fn empty_optional_parts_are_left_out_like_python_does() {
        let mut deck = deck();
        deck.add_slide(Slide::new("x").with_image("").with_code("").with_iframe(""));
        let html = deck.to_html(false, Duration::ZERO, false);
        assert!(!html.contains("<img"));
        assert!(!html.contains("<pre class"));
        assert!(!html.contains("<iframe"));
        assert!(html.contains("<aside class=\"notes\"></aside>"));
    }

    #[test]
    fn the_interval_plays_the_deck_by_itself_in_milliseconds() {
        let html = deck().to_html(false, Duration::from_millis(2500), false);
        assert!(html.contains("autoSlide: 2500,"), "{html}");
    }

    #[test]
    fn the_end_marker_is_the_last_slide_and_only_when_asked() {
        let mut deck = deck();
        deck.add_slide(Slide::new("one"));
        let with = deck.to_html(false, Duration::ZERO, true);
        let last_slide = with.rfind("<section").unwrap();
        assert!(
            with[last_slide..].contains("<p class=\"End_Presentation_Now\"> </p>"),
            "{with}"
        );
        assert_eq!(with.matches("<section").count(), 2);
        let without = deck.to_html(false, Duration::ZERO, false);
        assert!(!without.contains("End_Presentation_Now"));
        assert!(END_MARKER_SELECTOR.ends_with("p.End_Presentation_Now"));
    }

    #[test]
    fn charts_on_several_slides_load_the_chart_library_once() {
        let mut deck = deck();
        for _ in 0..3 {
            deck.add_slide(Slide::new(&format!("{CHART_JS_TAG}\n<canvas></canvas>")));
        }
        let html = deck.to_html(false, Duration::ZERO, false);
        assert_eq!(html.matches(CHART_JS_TAG).count(), 1, "{html}");
        assert_eq!(html.matches(CHARTS_ADDED).count(), 2);
        // The one copy stays in the first slide, before the others' charts.
        assert!(html.find(CHART_JS_TAG).unwrap() < html.find("<canvas>").unwrap());
    }

    #[test]
    fn theme_and_transition_names_are_read_ignoring_case() {
        assert_eq!(
            "Sky".parse::<PresentationTheme>().unwrap(),
            PresentationTheme::Sky
        );
        assert_eq!(
            "default".parse::<PresentationTheme>().unwrap(),
            PresentationTheme::Serif
        );
        assert_eq!(
            "".parse::<PresentationTheme>().unwrap(),
            PresentationTheme::Serif
        );
        assert_eq!("CONVEX".parse::<Transition>().unwrap(), Transition::Convex);
        assert_eq!("default".parse::<Transition>().unwrap(), Transition::None);
    }

    #[test]
    fn an_unknown_theme_or_transition_gets_python_s_text() {
        let error = "neon".parse::<PresentationTheme>().unwrap_err().to_string();
        assert!(
            error.contains("Theme {neon} not found! Valid themes: ['serif', 'white', 'black'"),
            "{error}"
        );
        let error = "spin".parse::<Transition>().unwrap_err().to_string();
        assert!(
            error.contains("Transition {spin} not found! Valid transitions: ['none', 'slide'"),
            "{error}"
        );
    }

    #[test]
    fn a_deck_is_saved_under_the_name_given_in_the_folder_given() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("decks");
        let mut deck = deck();
        deck.add_slide(Slide::new("<h1>Saved</h1>"));

        let path = deck
            .save_in(&folder, DEFAULT_FILENAME, false, Duration::ZERO)
            .unwrap();
        assert_eq!(path, folder.join("my_presentation.html"));
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("<h1>Saved</h1>"));
    }

    #[test]
    fn a_file_name_must_end_in_html_and_stay_in_its_folder() {
        let dir = tempfile::tempdir().unwrap();
        let error = deck()
            .save_in(dir.path(), "deck.txt", false, Duration::ZERO)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Presentation file must end in \".html\"!"),
            "{error}"
        );
        for bad in ["../deck.html", "a/deck.html", "/tmp/deck.html"] {
            assert!(
                deck()
                    .save_in(dir.path(), bad, false, Duration::ZERO)
                    .is_err(),
                "{bad}"
            );
        }
        assert!(
            fs::read_dir(dir.path()).unwrap().next().is_none(),
            "nothing was written"
        );
    }

    #[test]
    fn saving_into_a_folder_that_cannot_exist_names_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        fs::write(&file, "x").unwrap();
        let error = deck()
            .save_in(&file.join("sub"), "d.html", false, Duration::ZERO)
            .unwrap_err()
            .to_string();
        assert!(error.contains("a-file"), "{error}");
    }

    #[test]
    fn the_end_marker_is_in_the_file_only_when_it_is_written_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = deck()
            .write_in(dir.path(), "d.html", false, Duration::ZERO, true)
            .unwrap();
        assert!(fs::read_to_string(path)
            .unwrap()
            .contains("End_Presentation_Now"));
    }

    #[test]
    fn the_interval_is_the_one_given_else_the_run_s_else_zero() {
        let two = Duration::from_secs(2);
        let nine = Duration::from_secs(9);
        assert_eq!(resolve_interval(two, Some(nine)), two);
        assert_eq!(resolve_interval(Duration::ZERO, Some(nine)), nine);
        assert_eq!(resolve_interval(Duration::ZERO, None), Duration::ZERO);
    }
}
