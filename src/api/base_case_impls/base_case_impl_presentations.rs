// Presentation helpers.
// Names the presentation modules here, so that the code below and the tests
// read like the chart ones. Nothing is imported twice: these names are only
// used by the presentation methods.
use crate::api::presentation::{self as deck, Presentation, PresentationTheme, Slide, Transition};

/// The error for a presentation name that nothing was created under.
fn missing_presentation(name: &str) -> SeleniumBaseError {
    SeleniumBaseError::InvalidConfig(format!("Presentation {{{name}}} does not exist!"))
}

/// The name, file, notes and interval of `save_presentation` and
/// `begin_presentation`: they take the same keyword arguments.
#[derive(Clone, Copy, Default)]
struct PresentationOptions<'a> {
    name: Option<&'a str>,
    filename: Option<&'a str>,
    show_notes: bool,
    interval: Duration,
}

impl BaseCase {
    /// Creates a presentation that slides can be added to, as Python's
    /// `create_presentation(name, theme, transition)`. It returns a call
    /// builder: awaiting it runs the call with the keyword arguments that were
    /// set.
    ///
    /// `.name(..)` names the presentation, so that several can exist at the
    /// same time (the default name is `default`), `.theme(..)` sets its
    /// [`PresentationTheme`] (default `serif`) and `.transition(..)` the
    /// [`Transition`] its slides share unless they set their own (default
    /// `none`). Creating a presentation under a name that is in use starts it
    /// again.
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::api::presentation::{PresentationTheme, Transition};
    ///
    /// # async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.create_presentation()
    ///     .theme(PresentationTheme::Sky)
    ///     .transition(Transition::Fade)
    ///     .await?;
    /// sb.add_slide("<h1>Hello</h1>").notes("Say hello").await?;
    /// sb.add_slide("<h2>Bye</h2>").image("https://example.test/a.png").await?;
    /// let path = sb.save_presentation().show_notes(true).await?;
    /// println!("saved to {}", path.display());
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_presentation(&mut self) -> CreatePresentation<'_> {
        CreatePresentation {
            sb: self,
            name: None,
            theme: PresentationTheme::default(),
            transition: Transition::default(),
        }
    }

    /// Adds a slide to a presentation, as Python's `add_slide(content, image,
    /// code, iframe, content2, notes, transition, name)`. `content` is HTML.
    ///
    /// The keyword arguments are call-builder methods, each named as in
    /// Python: `.image(url)`, `.code(text)`, `.iframe(url)`, `.content2(html)`,
    /// `.notes(html)`, `.transition(..)` and `.name(..)`, which names the
    /// presentation the slide goes to. As in Python, a presentation that does
    /// not exist yet is created with the default theme and transition.
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    pub fn add_slide<'a>(&'a mut self, content: &'a str) -> AddSlide<'a> {
        AddSlide {
            sb: self,
            slide: Slide::new(content),
            name: None,
        }
    }

    /// Saves a presentation as an HTML file, as Python's
    /// `save_presentation(name, filename, show_notes, interval)`, and returns
    /// the path of the file.
    ///
    /// The file goes to `saved_presentations/my_presentation.html` unless
    /// `.filename(..)` names another file, which must end in `.html`.
    /// `.name(..)` picks the presentation, `.show_notes(true)` shows the
    /// speaker notes and `.interval(..)` plays the slides by itself, a new one
    /// every `interval`. Without an interval, the run's `--interval`
    /// ([`RunSettings::interval`]) is used, if there is one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Presentation {NAME} does not exist!`, if there is no presentation of
    /// that name, `Presentation file must end in ".html"!` for another file
    /// name, and an error if the file cannot be written. The page loads
    /// reveal.js from a CDN, so it shows unstyled slides when the browser is
    /// offline.
    pub fn save_presentation(&self) -> SavePresentation<'_> {
        SavePresentation {
            sb: self,
            options: PresentationOptions::default(),
        }
    }

    /// Shows a presentation in the browser and waits until it ends, as
    /// Python's `begin_presentation(name, filename, show_notes, interval)`: it
    /// saves the presentation with an empty last slide, opens the file, and
    /// returns when that slide shows, when the page is no longer one of the
    /// saved presentations, or when the window closes.
    ///
    /// It takes the same keyword arguments as
    /// [`save_presentation`](Self::save_presentation). Where nobody watches
    /// (a headless browser or a virtual display) nothing is shown and the
    /// result is `None`; otherwise it is the path of the file.
    ///
    /// # Errors
    ///
    /// The same as [`save_presentation`](Self::save_presentation), and the
    /// error of opening the file.
    pub fn begin_presentation(&mut self) -> BeginPresentation<'_> {
        BeginPresentation {
            sb: self,
            options: PresentationOptions::default(),
        }
    }

    /// Writes the presentation `options` names into `folder` and returns the
    /// path. The checks run in Python's order: the presentation, then the file
    /// name.
    fn write_presentation(
        &self,
        options: &PresentationOptions<'_>,
        folder: &Path,
        end_marker: bool,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let name = chart_lib::name_or_default(options.name);
        let presentation = self
            .presentations
            .get(name)
            .ok_or_else(|| missing_presentation(name))?;
        let filename = options.filename.unwrap_or(deck::DEFAULT_FILENAME);
        deck::check_filename(filename)?;
        let interval = deck::resolve_interval(options.interval, self.run_settings.interval());
        presentation.write_in(folder, filename, options.show_notes, interval, end_marker)
    }

    /// Waits while the browser shows a page in `folder` (a folder of saved
    /// presentations or charts): until the window closes, the page leaves the
    /// folder, or `end_selector` is visible. Whatever fails ends the wait, as
    /// in Python, where a closed window is an exception that is swallowed.
    async fn wait_while_shown(&self, folder: &str, end_selector: Option<&str>) {
        let folder = format!("{folder}/");
        loop {
            let Ok(handles) = self.get_all_window_handles().await else {
                return;
            };
            if handles.is_empty() {
                return;
            }
            let Ok(url) = self.get_current_url().await else {
                return;
            };
            if !url.contains(&folder) {
                return;
            }
            if let Some(selector) = end_selector {
                if self.is_element_visible(selector).await.unwrap_or(false) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// The call builder of [`BaseCase::create_presentation`].
#[must_use = "nothing happens until the call is awaited"]
pub struct CreatePresentation<'a> {
    sb: &'a mut BaseCase,
    name: Option<&'a str>,
    theme: PresentationTheme,
    transition: Transition,
}

impl<'a> CreatePresentation<'a> {
    /// Names the presentation (Python's `name`; the default is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the theme (Python's `theme`; the default is `serif`).
    pub fn theme(mut self, theme: PresentationTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Sets the transition between slides (Python's `transition`; the default
    /// is `none`).
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transition = transition;
        self
    }
}

impl<'a> std::future::IntoFuture for CreatePresentation<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = chart_lib::name_or_default(self.name).to_owned();
            self.sb
                .presentations
                .insert(name, Presentation::new(self.theme, self.transition));
            Ok(())
        })
    }
}

/// The call builder of [`BaseCase::add_slide`].
#[must_use = "nothing happens until the call is awaited"]
pub struct AddSlide<'a> {
    sb: &'a mut BaseCase,
    slide: Slide,
    name: Option<&'a str>,
}

impl<'a> AddSlide<'a> {
    /// Shows the image at `url` below the content (Python's `image`).
    pub fn image(mut self, url: &str) -> Self {
        self.slide = self.slide.with_image(url);
        self
    }

    /// Shows `code` below the content, with syntax highlighting (Python's
    /// `code`). It is shown as text.
    pub fn code(mut self, code: &str) -> Self {
        self.slide = self.slide.with_code(code);
        self
    }

    /// Shows the page at `url` in a frame below the content (Python's
    /// `iframe`).
    pub fn iframe(mut self, url: &str) -> Self {
        self.slide = self.slide.with_iframe(url);
        self
    }

    /// Shows the HTML `content2` after the image, the code and the frame
    /// (Python's `content2`).
    pub fn content2(mut self, content2: &str) -> Self {
        self.slide = self.slide.with_content2(content2);
        self
    }

    /// Adds speaker notes, as HTML (Python's `notes`). They are only seen when
    /// the presentation is saved or shown with notes.
    pub fn notes(mut self, notes: &str) -> Self {
        self.slide = self.slide.with_notes(notes);
        self
    }

    /// Gives this slide a transition of its own (Python's `transition`).
    pub fn transition(mut self, transition: Transition) -> Self {
        self.slide = self.slide.with_transition(transition);
        self
    }

    /// Picks the presentation the slide goes to (Python's `name`; the default
    /// is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }
}

impl<'a> std::future::IntoFuture for AddSlide<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = chart_lib::name_or_default(self.name).to_owned();
            self.sb
                .presentations
                .entry(name)
                .or_insert_with(|| {
                    Presentation::new(PresentationTheme::default(), Transition::default())
                })
                .add_slide(self.slide);
            Ok(())
        })
    }
}

/// The call builder of [`BaseCase::save_presentation`].
#[must_use = "nothing happens until the call is awaited"]
pub struct SavePresentation<'a> {
    sb: &'a BaseCase,
    options: PresentationOptions<'a>,
}

impl<'a> SavePresentation<'a> {
    /// Picks the presentation (Python's `name`; the default is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.options.name = Some(name);
        self
    }

    /// Names the file, which must end in `.html` (Python's `filename`; the
    /// default is `my_presentation.html`).
    pub fn filename(mut self, filename: &'a str) -> Self {
        self.options.filename = Some(filename);
        self
    }

    /// Shows the speaker notes next to the slides (Python's `show_notes`).
    pub fn show_notes(mut self, show_notes: bool) -> Self {
        self.options.show_notes = show_notes;
        self
    }

    /// Plays the slides by themselves, a new one every `interval` (Python's
    /// `interval`; zero, the default, uses the run's `--interval`, if any).
    pub fn interval(mut self, interval: Duration) -> Self {
        self.options.interval = interval;
        self
    }
}

impl<'a> std::future::IntoFuture for SavePresentation<'a> {
    type Output = Result<PathBuf, SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, PathBuf>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let path = self.sb.write_presentation(
                &self.options,
                Path::new(deck::SAVED_FOLDER),
                false,
            )?;
            tracing::info!(path = %path.display(), "presentation saved");
            Ok(path)
        })
    }
}

/// The call builder of [`BaseCase::begin_presentation`].
#[must_use = "nothing happens until the call is awaited"]
pub struct BeginPresentation<'a> {
    sb: &'a mut BaseCase,
    options: PresentationOptions<'a>,
}

impl<'a> BeginPresentation<'a> {
    /// Picks the presentation (Python's `name`; the default is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.options.name = Some(name);
        self
    }

    /// Names the file, which must end in `.html` (Python's `filename`; the
    /// default is `my_presentation.html`).
    pub fn filename(mut self, filename: &'a str) -> Self {
        self.options.filename = Some(filename);
        self
    }

    /// Shows the speaker notes next to the slides (Python's `show_notes`).
    pub fn show_notes(mut self, show_notes: bool) -> Self {
        self.options.show_notes = show_notes;
        self
    }

    /// Plays the slides by themselves, a new one every `interval` (Python's
    /// `interval`; zero, the default, uses the run's `--interval`, if any).
    pub fn interval(mut self, interval: Duration) -> Self {
        self.options.interval = interval;
        self
    }
}

impl<'a> std::future::IntoFuture for BeginPresentation<'a> {
    type Output = Result<Option<PathBuf>, SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, Option<PathBuf>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            // A presentation is for a person to watch.
            if self.sb.nobody_watches() {
                return Ok(None);
            }
            let path = self.sb.write_presentation(
                &self.options,
                Path::new(deck::SAVED_FOLDER),
                true,
            )?;
            let file = path.to_str().ok_or_else(|| {
                SeleniumBaseError::InvalidConfig("the presentation path is not UTF-8".to_owned())
            })?;
            self.sb.open_html_file(file).await?;
            self.sb
                .wait_while_shown(deck::SAVED_FOLDER, Some(deck::END_MARKER_SELECTOR))
                .await;
            Ok(Some(path))
        })
    }
}

#[cfg(test)]
mod tests_presentations {
    use super::*;
    use std::future::IntoFuture;

    fn case() -> BaseCase {
        BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected())
    }

    fn assert_send<T: Send>(_: &T) {}

    #[tokio::test]
    async fn two_named_presentations_exist_side_by_side() {
        let mut sb = case();
        sb.create_presentation().await.unwrap();
        sb.create_presentation()
            .name("other")
            .theme(PresentationTheme::Sky)
            .transition(Transition::Zoom)
            .await
            .unwrap();
        sb.add_slide("<h1>one</h1>").await.unwrap();
        sb.add_slide("<h1>two</h1>").name("other").await.unwrap();
        sb.add_slide("<h1>three</h1>").name("other").await.unwrap();

        assert_eq!(sb.presentations["default"].slides().len(), 1);
        assert_eq!(sb.presentations["default"].theme(), PresentationTheme::Serif);
        assert_eq!(sb.presentations["other"].slides().len(), 2);
        assert_eq!(sb.presentations["other"].theme(), PresentationTheme::Sky);
        assert_eq!(sb.presentations["other"].transition(), Transition::Zoom);
    }

    #[tokio::test]
    async fn creating_a_presentation_again_starts_it_over() {
        let mut sb = case();
        sb.add_slide("old").await.unwrap();
        sb.create_presentation().await.unwrap();
        assert!(sb.presentations["default"].slides().is_empty());
    }

    #[tokio::test]
    async fn a_slide_for_a_presentation_that_does_not_exist_creates_it_as_python_does() {
        let mut sb = case();
        sb.add_slide("x").name("fresh").await.unwrap();
        assert_eq!(sb.presentations["fresh"].slides().len(), 1);
        assert_eq!(sb.presentations["fresh"].transition(), Transition::None);
    }

    #[tokio::test]
    async fn the_keyword_arguments_of_a_slide_reach_the_slide() {
        let mut sb = case();
        sb.add_slide("c")
            .image("i")
            .code("k")
            .iframe("f")
            .content2("c2")
            .notes("n")
            .transition(Transition::Fade)
            .await
            .unwrap();
        let slide = &sb.presentations["default"].slides()[0];
        assert_eq!(
            slide,
            &Slide::new("c")
                .with_image("i")
                .with_code("k")
                .with_iframe("f")
                .with_content2("c2")
                .with_notes("n")
                .with_transition(Transition::Fade)
        );
    }

    #[tokio::test]
    async fn saving_or_beginning_a_presentation_that_does_not_exist_says_so() {
        let mut sb = case();
        let error = sb.save_presentation().name("nope").await.unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::InvalidConfig(m) if m == "Presentation {nope} does not exist!"),
            "{error:?}"
        );
        // The default name is checked too.
        let error = sb.save_presentation().await.unwrap_err();
        assert!(error.to_string().contains("Presentation {default} does not exist!"));
        sb.config = sb.config.clone().with_headless(false);
        let error = sb.begin_presentation().name("nope").await.unwrap_err();
        assert!(error.to_string().contains("Presentation {nope} does not exist!"));
    }

    #[tokio::test]
    async fn a_presentation_file_must_end_in_html() {
        let mut sb = case();
        sb.add_slide("x").await.unwrap();
        let error = sb.save_presentation().filename("deck.md").await.unwrap_err();
        assert!(error.to_string().contains("Presentation file must end in \".html\"!"));
    }

    #[tokio::test]
    async fn begin_presentation_shows_nothing_where_nobody_watches() {
        // A disconnected session fails on any browser call, so Ok(None) also
        // proves that the browser was not touched, and nothing was written.
        for config in [
            BrowserConfig::default().with_headless(true),
            BrowserConfig::default().with_headless(false).with_xvfb(true),
        ] {
            let mut sb = BaseCase::with_session(config, BrowserSession::disconnected());
            sb.add_slide("x").await.unwrap();
            assert_eq!(sb.begin_presentation().await.unwrap(), None);
        }
    }

    #[test]
    fn a_presentation_is_written_with_the_run_s_interval_unless_one_is_given() {
        let folder = tempfile::tempdir().unwrap();
        let settings = RunSettings::default().with_interval(Duration::from_secs(4));
        let mut sb = BaseCase::with_session(
            BrowserConfig::default().with_run_settings(settings),
            BrowserSession::disconnected(),
        );
        sb.presentations.insert(
            "default".to_owned(),
            Presentation::new(PresentationTheme::Moon, Transition::Slide),
        );
        sb.presentations
            .get_mut("default")
            .unwrap()
            .add_slide(Slide::new("<h1>hi</h1>"));

        let mut options = PresentationOptions::default();
        let path = sb.write_presentation(&options, folder.path(), false).unwrap();
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(html.contains("autoSlide: 4000,"), "{html}");
        assert!(html.contains("theme/moon.css"));
        assert!(!html.contains("End_Presentation_Now"));

        options.interval = Duration::from_millis(1500);
        options.show_notes = true;
        options.filename = Some("other.html");
        let path = sb.write_presentation(&options, folder.path(), true).unwrap();
        assert_eq!(path, folder.path().join("other.html"));
        let html = std::fs::read_to_string(&path).unwrap();
        assert!(html.contains("autoSlide: 1500,") && html.contains("showNotes: true"));
        assert!(html.contains("End_Presentation_Now"));
        // The end marker is in the file only: the presentation is unchanged.
        assert_eq!(sb.presentations["default"].slides().len(), 1);
    }

    #[test]
    fn the_futures_of_the_presentation_builders_are_send() {
        let mut sb = case();
        assert_send(&sb.create_presentation().into_future());
        assert_send(&sb.add_slide("x").into_future());
        assert_send(&sb.save_presentation().into_future());
        assert_send(&sb.begin_presentation().into_future());
    }
}
