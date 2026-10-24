// Website tours: Python's `create_tour`, `add_tour_step`, `play_tour`,
// `export_tour` and `set_introjs_colors`. The tours themselves, and the
// scripts that show and export them, are in `crate::api::tour`.
//
// Names the tour module here, so that the code below and its tests read like
// the presentation ones. Nothing is imported twice: these names are only used
// by the tour methods.
use crate::api::tour::{
    self as tours, Autoplay, IntroJsColors, Library, ShepherdTheme, TourAlignment, TourPlan,
    TourStep, TourTheme,
};

/// How often the player asks the page whether the tour goes on (Python: 10 ms).
const TOUR_POLL: Duration = Duration::from_millis(10);

/// How long the player waits for a step that was not open before it checks
/// again (Python: 0.1 s).
const TOUR_RECHECK: Duration = Duration::from_millis(100);

/// How long the first buttons of a tour are waited for once it has started
/// (Python: 1.2 s).
const TOUR_READY_WAIT: Duration = Duration::from_millis(1200);

/// What a tour shows on the page at one moment, as the library's probe script
/// reports it.
struct TourState {
    /// A step is on show.
    active: bool,
    /// Shepherd only: the tour runs but its step is not open.
    pending: bool,
    /// Tells the step on show from the others.
    key: String,
    /// Shepherd only: the element of the step.
    element: String,
}

impl BaseCase {
    /// Creates a tour for the page, as Python's `create_tour(name, theme)`. It
    /// returns a call builder: awaiting it runs the call with the keyword
    /// arguments that were set.
    ///
    /// `.name(..)` names the tour, so that several can exist at the same time
    /// (the default name is `default`) and `.theme(..)` picks the library that
    /// shows it: a [`TourTheme`], or a
    /// [`ShepherdTheme`](crate::api::tour::ShepherdTheme) for one of
    /// Shepherd's looks. The default theme is IntroJS, as in Python. Creating a
    /// tour under a name that is in use starts it again.
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use seleniumbase_rs::api::tour::{ShepherdTheme, TourAlignment};
    ///
    /// # async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.open("https://seleniumbase.io/demo_page").await?;
    /// sb.create_shepherd_tour().theme(ShepherdTheme::Dark).await?;
    /// sb.add_tour_step("Welcome to the demo page.").await?;
    /// sb.add_tour_step("Type something here.")
    ///     .selector("#myTextInput")
    ///     .title("Input")
    ///     .alignment(TourAlignment::Bottom)
    ///     .await?;
    /// sb.play_tour().interval(Duration::from_secs(3)).await?;
    /// let path = sb.export_tour().filename("demo_tour.js").await?;
    /// println!("exported to {}", path.display());
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_tour(&mut self) -> CreateTour<'_> {
        CreateTour {
            sb: self,
            name: None,
            theme: TourTheme::default(),
        }
    }

    /// Creates a Shepherd tour, as Python's `create_shepherd_tour(name,
    /// theme)`: `.name(..)` names it and `.theme(..)` sets its look, a
    /// [`ShepherdTheme`] (default `light`, also called `arrows`).
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    pub fn create_shepherd_tour(&mut self) -> CreateShepherdTour<'_> {
        CreateShepherdTour {
            sb: self,
            name: None,
            theme: ShepherdTheme::default(),
        }
    }

    /// Creates a Bootstrap Tour, as Python's `create_bootstrap_tour(name)`;
    /// `.name(..)` names it.
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    pub fn create_bootstrap_tour(&mut self) -> CreateLibraryTour<'_> {
        CreateLibraryTour {
            sb: self,
            name: None,
            theme: TourTheme::Bootstrap,
        }
    }

    /// Creates a DriverJS tour, as Python's `create_driverjs_tour(name)`;
    /// `.name(..)` names it.
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    pub fn create_driverjs_tour(&mut self) -> CreateLibraryTour<'_> {
        CreateLibraryTour {
            sb: self,
            name: None,
            theme: TourTheme::DriverJs,
        }
    }

    /// Creates a Hopscotch tour, as Python's `create_hopscotch_tour(name)`;
    /// `.name(..)` names it.
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    pub fn create_hopscotch_tour(&mut self) -> CreateLibraryTour<'_> {
        CreateLibraryTour {
            sb: self,
            name: None,
            theme: TourTheme::Hopscotch,
        }
    }

    /// Creates an IntroJS tour, as Python's `create_introjs_tour(name)`;
    /// `.name(..)` names it.
    ///
    /// # Errors
    ///
    /// The call does not fail; the result is there so that every call of this
    /// kind reads the same.
    pub fn create_introjs_tour(&mut self) -> CreateLibraryTour<'_> {
        CreateLibraryTour {
            sb: self,
            name: None,
            theme: TourTheme::IntroJs,
        }
    }

    /// Adds a step to a tour, as Python's `add_tour_step(message, selector,
    /// name, title, theme, alignment, duration)`. `message` is HTML.
    ///
    /// The keyword arguments are call-builder methods, each named as in
    /// Python: `.selector(css)` is the element the step points at (a step
    /// without one points at nothing; `name=q` and an XPath that has a CSS
    /// equivalent are turned into CSS), `.name(..)` picks the tour,
    /// `.title(..)` puts a heading above the message, `.theme(..)` gives a
    /// Shepherd step a look of its own, `.alignment(..)` sets where the step
    /// sits next to the element (the default is [`TourAlignment::Top`],
    /// [`TourAlignment::Bottom`] for DriverJS and Hopscotch) and
    /// `.duration(..)` sets how long a Bootstrap step is shown before the tour
    /// goes on by itself.
    ///
    /// As in Python, a tour that does not exist yet is created, as an IntroJS
    /// tour.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] for an XPath selector
    /// that has no CSS equivalent.
    pub fn add_tour_step<'a>(&'a mut self, message: &'a str) -> AddTourStep<'a> {
        AddTourStep {
            sb: self,
            message,
            selector: None,
            name: None,
            title: None,
            theme: None,
            alignment: None,
            duration: None,
        }
    }

    /// Plays a tour on the current page and waits until it ends, as Python's
    /// `play_tour(name, interval)`.
    ///
    /// `.name(..)` picks the tour and `.interval(..)` makes it play by itself,
    /// a new step every `interval` (at least half a second, a second for
    /// Bootstrap). Without an interval, the run's `--interval`
    /// ([`RunSettings::interval`]) is used, if there is one, and otherwise
    /// the person moves through the tour with its buttons or the arrow keys.
    ///
    /// The call returns when the tour ends, when the window closes or the page
    /// is left, and at once where nobody watches (a headless browser or a
    /// virtual display). There is no other time limit, except the one of the
    /// test ([`set_time_limit`](Self::set_time_limit)). The libraries are
    /// loaded from CDNs, so the page needs the network.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Tour {NAME} does not exist!`, if there is no such tour, and another for
    /// a tour without steps. If the element of the first step is not in the
    /// page after 7 seconds, an error message is shown and the error is
    /// `Tour Error: {'SELECTOR'} was not found! Exiting due to failure on
    /// first tour step!`. A library that cannot be loaded is a
    /// [`SeleniumBaseError::Download`].
    pub fn play_tour(&self) -> PlayTour<'_> {
        PlayTour {
            sb: self,
            name: None,
            interval: Duration::ZERO,
        }
    }

    /// The same as [`play_tour`](Self::play_tour), as in Python.
    pub fn start_tour(&self) -> PlayTour<'_> {
        self.play_tour()
    }

    /// Saves a tour as a JavaScript file, as Python's `export_tour(name,
    /// filename, url)`, and returns the path of the file.
    ///
    /// The file goes to `tours_exported/my_tour.js` unless `.filename(..)`
    /// names another file, which must end in `.js`. `.name(..)` picks the
    /// tour. `.url(..)` is the page the tour starts on, by default the page the
    /// browser is on. Pasted into the console of a browser, the file takes the
    /// browser to that page, loads the tour library and plays the tour, with no
    /// test around it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Tour {NAME} does not exist!`, if there is no such tour,
    /// `Tour file must end in ".js"!` for another file name, and an error if
    /// the file cannot be written.
    pub fn export_tour(&self) -> ExportTour<'_> {
        ExportTour {
            sb: self,
            name: None,
            filename: None,
            url: None,
        }
    }

    /// Sets the colours of the buttons of IntroJS tours, as Python's
    /// `set_introjs_colors(theme_color, hover_color)`: the colour of the
    /// buttons, and the colour of a button under the mouse, which is also the
    /// colour of its border. A colour that is `None` goes back to its default
    /// (`#367be5` and `#245ac0`). The colours are used by the tours played or
    /// exported afterwards, and belong to this case.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] with Python's text,
    /// `Expecting a hex value color that starts with "#"!`, for a colour that
    /// is not `#` and three or six hex digits. Neither colour is changed then.
    pub fn set_introjs_colors(
        &mut self,
        theme_color: Option<&str>,
        hover_color: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        self.introjs_colors = IntroJsColors::new(theme_color, hover_color)?;
        Ok(())
    }

    /// Plays the tour `name` (see [`play_tour`](Self::play_tour)).
    async fn run_tour(
        &self,
        name: Option<&str>,
        interval: Duration,
    ) -> Result<(), SeleniumBaseError> {
        if self.nobody_watches() {
            return Ok(());
        }
        self.wait_for_ready_state_complete().await?;
        let interval = if interval.is_zero() {
            self.run_settings.interval()
        } else {
            Some(interval)
        };
        let name = tours::name_or_default(name);
        let tour = self
            .tours
            .get(name)
            .ok_or_else(|| tours::missing_tour(name))?;
        let plan = tour.plan(name, interval, &self.introjs_colors)?;
        self.load_tour_library(&plan).await?;
        self.check_first_tour_step(&plan).await?;
        self.execute_script(&plan.script).await?;
        if let Some(selector) = plan.library.ready_selector() {
            self.wait_visible_for(selector, TOUR_READY_WAIT).await;
        }
        if plan.library.blurs_focus() {
            // Python ignores a page that does not let go of the focus.
            let _ = self.execute_script("document.activeElement.blur();").await;
        }
        self.drive_tour(&plan).await
    }

    /// Puts the library of the tour, and its backdrop style, in the page, unless
    /// the page has them already.
    async fn load_tour_library(&self, plan: &TourPlan) -> Result<(), SeleniumBaseError> {
        self.add_css_style(&plan.style).await?;
        if plan.library.needs_jquery() {
            self.activate_jquery().await?;
        }
        for resource in plan.library.resources() {
            if resource.script {
                self.load_page_resource(PageResource::Script, resource.url, resource.loaded)
                    .await?;
            } else {
                let loaded = format!("!!document.querySelector('link[href=\"{}\"]')", resource.url);
                self.load_page_resource(PageResource::Stylesheet, resource.url, Some(&loaded))
                    .await?;
            }
        }
        Ok(())
    }

    /// Checks that the page has the element of the first step, as Python does.
    async fn check_first_tour_step(&self, plan: &TourPlan) -> Result<(), SeleniumBaseError> {
        let Some(selector) = plan.first_selector.as_deref() else {
            return Ok(());
        };
        if self
            .wait_for_element_present(selector, tours::FIRST_STEP_TIMEOUT_SECS)
            .await
            .is_ok()
        {
            return Ok(());
        }
        let message = format!("Tour Error: {{'{selector}'}} was not found!");
        // The message is for a person; the error below is for the test.
        let _ = self.post_error_message(&message).await;
        Err(SeleniumBaseError::WaitTimeout(format!(
            "{message} Exiting due to failure on first tour step!"
        )))
    }

    /// Waits up to `limit` for `css` to be visible, and goes on without it, as
    /// Python does for the buttons of a tour.
    async fn wait_visible_for(&self, css: &str, limit: Duration) {
        let end = tokio::time::Instant::now() + limit;
        while tokio::time::Instant::now() < end {
            if self.is_element_visible(css).await.unwrap_or(false) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Asks the page what the tour shows. `None` when there is no tour any more:
    /// it ended, or the window or the page is gone, which the libraries report
    /// as a script that throws.
    async fn tour_state(&self, library: Library) -> Option<TourState> {
        let value = self.execute_script(library.probe_script()).await.ok()?;
        let flag = |field: &str| value[field].as_bool().unwrap_or(false);
        let text = |field: &str| value[field].as_str().unwrap_or_default().to_owned();
        Some(TourState {
            active: flag("active"),
            pending: flag("pending"),
            key: text("key"),
            element: text("element"),
        })
    }

    /// Waits while the tour runs, and moves it on when it autoplays.
    async fn drive_tour(&self, plan: &TourPlan) -> Result<(), SeleniumBaseError> {
        let library = plan.library;
        let mut clock = plan.autoplay.map(Autoplay::new);
        loop {
            self.check_time_limit()?;
            tokio::time::sleep(TOUR_POLL).await;
            let Some(state) = self.tour_state(library).await else {
                return Ok(());
            };
            if state.active {
                if let Some(clock) = clock.as_mut() {
                    if clock.observe(&state.key) {
                        if self.execute_script(library.advance_script()).await.is_err() {
                            return Ok(());
                        }
                        match self.tour_state(library).await {
                            Some(next) => clock.restart(&next.key),
                            None => return Ok(()),
                        }
                    }
                }
                continue;
            }
            // A tour that looks over may be between two steps: ask again.
            tokio::time::sleep(TOUR_RECHECK).await;
            match self.tour_state(library).await {
                None => return Ok(()),
                Some(again) if again.active => {}
                Some(again) if again.pending && library == Library::Shepherd => {
                    if !self.skip_missing_shepherd_step(&again.element).await {
                        return Ok(());
                    }
                    if let Some(clock) = clock.as_mut() {
                        clock.restart("");
                    }
                }
                Some(_) => return Ok(()),
            }
        }
    }

    /// Moves a Shepherd tour past a step that did not open. If the element of
    /// the step is not in the page, the step is taken off the page and an
    /// error message says so, as in Python. Returns false when the tour is over.
    async fn skip_missing_shepherd_step(&self, element: &str) -> bool {
        if !element.is_empty()
            && self
                .wait_for_element_present(element, tours::FIRST_STEP_TIMEOUT_SECS)
                .await
                .is_err()
        {
            let _ = self
                .execute_script(
                    "document.querySelectorAll('div.shepherd-content').forEach(\
                     function (node) { node.remove(); });",
                )
                .await;
            let message = format!("Tour Error: {{'{element}'}} was not found!");
            let _ = self.post_error_message(&message).await;
        }
        self.execute_script(Library::Shepherd.advance_script())
            .await
            .is_ok()
    }

    /// Writes the script of the tour `name` into `folder` as `filename`, and
    /// returns the path.
    fn write_tour_export(
        &self,
        folder: &Path,
        name: &str,
        filename: &str,
        url: &str,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let tour = self
            .tours
            .get(name)
            .ok_or_else(|| tours::missing_tour(name))?;
        let script = tour.export_script(name, url, &self.introjs_colors);
        std::fs::create_dir_all(folder)?;
        let path = folder.join(filename);
        std::fs::write(&path, script)?;
        println!("\n>>> [{}] was saved!\n", path.display());
        Ok(path)
    }
}

/// The call builder of [`BaseCase::create_tour`].
#[must_use = "nothing happens until the call is awaited"]
pub struct CreateTour<'a> {
    sb: &'a mut BaseCase,
    name: Option<&'a str>,
    theme: TourTheme,
}

impl<'a> CreateTour<'a> {
    /// Names the tour (Python's `name`; the default is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the library that shows the tour (Python's `theme`; the default is
    /// IntroJS). A [`ShepherdTheme`] is a Shepherd tour with that look.
    pub fn theme(mut self, theme: impl Into<TourTheme>) -> Self {
        self.theme = theme.into();
        self
    }
}

impl<'a> std::future::IntoFuture for CreateTour<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = tours::name_or_default(self.name).to_owned();
            self.sb.tours.insert(name, Tour::new(self.theme));
            Ok(())
        })
    }
}

/// The call builder of [`BaseCase::create_shepherd_tour`].
#[must_use = "nothing happens until the call is awaited"]
pub struct CreateShepherdTour<'a> {
    sb: &'a mut BaseCase,
    name: Option<&'a str>,
    theme: ShepherdTheme,
}

impl<'a> CreateShepherdTour<'a> {
    /// Names the tour (Python's `name`; the default is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the look of the tour (Python's `theme`; the default is `light`).
    pub fn theme(mut self, theme: ShepherdTheme) -> Self {
        self.theme = theme;
        self
    }
}

impl<'a> std::future::IntoFuture for CreateShepherdTour<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = tours::name_or_default(self.name).to_owned();
            self.sb.tours.insert(name, Tour::new(self.theme));
            Ok(())
        })
    }
}

/// The call builder of the `create_*_tour` calls of the libraries that have no
/// looks: [`BaseCase::create_bootstrap_tour`], [`BaseCase::create_driverjs_tour`],
/// [`BaseCase::create_hopscotch_tour`] and [`BaseCase::create_introjs_tour`].
#[must_use = "nothing happens until the call is awaited"]
pub struct CreateLibraryTour<'a> {
    sb: &'a mut BaseCase,
    name: Option<&'a str>,
    theme: TourTheme,
}

impl<'a> CreateLibraryTour<'a> {
    /// Names the tour (Python's `name`; the default is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }
}

impl<'a> std::future::IntoFuture for CreateLibraryTour<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = tours::name_or_default(self.name).to_owned();
            self.sb.tours.insert(name, Tour::new(self.theme));
            Ok(())
        })
    }
}

/// The call builder of [`BaseCase::add_tour_step`].
#[must_use = "nothing happens until the call is awaited"]
pub struct AddTourStep<'a> {
    sb: &'a mut BaseCase,
    message: &'a str,
    selector: Option<&'a str>,
    name: Option<&'a str>,
    title: Option<&'a str>,
    theme: Option<ShepherdTheme>,
    alignment: Option<TourAlignment>,
    duration: Option<Duration>,
}

impl<'a> AddTourStep<'a> {
    /// Points the step at the element of this CSS selector (Python's
    /// `selector`; without one the step points at nothing).
    pub fn selector(mut self, selector: &'a str) -> Self {
        self.selector = Some(selector);
        self
    }

    /// Picks the tour the step goes to (Python's `name`; the default is
    /// `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// Puts a heading above the message (Python's `title`).
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Gives the step a look of its own (Python's `theme`; Shepherd tours
    /// only).
    pub fn theme(mut self, theme: ShepherdTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Sets where the step sits next to its element (Python's `alignment`).
    pub fn alignment(mut self, alignment: TourAlignment) -> Self {
        self.alignment = Some(alignment);
        self
    }

    /// Sets how long the step is shown before the tour goes on by itself
    /// (Python's `duration`; Bootstrap tours only).
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

impl<'a> std::future::IntoFuture for AddTourStep<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let selector = self
                .selector
                .filter(|selector| !selector.is_empty())
                .map(tours::normalize_selector)
                .transpose()?;
            let step = TourStep {
                message: self.message.to_owned(),
                selector,
                title: self.title.map(ToOwned::to_owned),
                theme: self.theme,
                alignment: self.alignment,
                duration: self.duration,
            };
            let name = tours::name_or_default(self.name).to_owned();
            self.sb
                .tours
                .entry(name)
                .or_insert_with(|| Tour::new(TourTheme::IntroJs))
                .add_step(step);
            Ok(())
        })
    }
}

/// The call builder of [`BaseCase::play_tour`] and [`BaseCase::start_tour`].
#[must_use = "nothing happens until the call is awaited"]
pub struct PlayTour<'a> {
    sb: &'a BaseCase,
    name: Option<&'a str>,
    interval: Duration,
}

impl<'a> PlayTour<'a> {
    /// Picks the tour (Python's `name`; the default is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// Plays the tour by itself, a new step every `interval` (Python's
    /// `interval`; zero, the default, uses the run's `--interval`, if any, and
    /// otherwise leaves the tour to the person).
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }
}

impl<'a> std::future::IntoFuture for PlayTour<'a> {
    type Output = Result<(), SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, ()>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.sb.run_tour(self.name, self.interval).await })
    }
}

/// The call builder of [`BaseCase::export_tour`].
#[must_use = "nothing happens until the call is awaited"]
pub struct ExportTour<'a> {
    sb: &'a BaseCase,
    name: Option<&'a str>,
    filename: Option<&'a str>,
    url: Option<&'a str>,
}

impl<'a> ExportTour<'a> {
    /// Picks the tour (Python's `name`; the default is `default`).
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// Names the file, which must end in `.js` (Python's `filename`; the
    /// default is `my_tour.js`).
    pub fn filename(mut self, filename: &'a str) -> Self {
        self.filename = Some(filename);
        self
    }

    /// Sets the page the tour starts on (Python's `url`; the default is the
    /// page the browser is on).
    pub fn url(mut self, url: &'a str) -> Self {
        self.url = Some(url);
        self
    }
}

impl<'a> std::future::IntoFuture for ExportTour<'a> {
    type Output = Result<PathBuf, SeleniumBaseError>;
    type IntoFuture = crate::api::call::CallFuture<'a, PathBuf>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let name = tours::name_or_default(self.name);
            if !self.sb.tours.contains_key(name) {
                return Err(tours::missing_tour(name));
            }
            let filename = self.filename.unwrap_or(tours::DEFAULT_FILENAME);
            if !filename.ends_with(".js") {
                return Err(SeleniumBaseError::InvalidConfig(
                    "Tour file must end in \".js\"!".to_owned(),
                ));
            }
            let url = match self.url {
                Some(url) if !url.is_empty() => url.to_owned(),
                _ => self.sb.get_current_url().await?,
            };
            self.sb
                .write_tour_export(Path::new(tours::EXPORT_FOLDER), name, filename, &url)
        })
    }
}

#[cfg(test)]
mod tests_tours {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;

    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    fn offline() -> BaseCase {
        BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected())
    }

    fn theme_of(sb: &BaseCase, name: &str) -> TourTheme {
        sb.tours[name].theme()
    }

    #[tokio::test]
    async fn each_constructor_makes_a_tour_with_its_own_library() {
        let mut sb = offline();
        sb.create_tour().await.unwrap();
        sb.create_tour().name("b").theme(TourTheme::Bootstrap).await.unwrap();
        sb.create_tour().name("light").theme(ShepherdTheme::Square).await.unwrap();
        sb.create_shepherd_tour().name("s").await.unwrap();
        sb.create_shepherd_tour().name("dark").theme(ShepherdTheme::Dark).await.unwrap();
        sb.create_bootstrap_tour().name("bt").await.unwrap();
        sb.create_driverjs_tour().name("d").await.unwrap();
        sb.create_hopscotch_tour().name("h").await.unwrap();
        sb.create_introjs_tour().name("i").await.unwrap();

        // The default library is IntroJS, as in Python, under the name
        // `default`.
        assert_eq!(theme_of(&sb, "default"), TourTheme::IntroJs);
        assert_eq!(theme_of(&sb, "b"), TourTheme::Bootstrap);
        assert_eq!(theme_of(&sb, "light"), TourTheme::Shepherd(ShepherdTheme::Square));
        assert_eq!(theme_of(&sb, "s"), TourTheme::Shepherd(ShepherdTheme::Light));
        assert_eq!(theme_of(&sb, "dark"), TourTheme::Shepherd(ShepherdTheme::Dark));
        assert_eq!(theme_of(&sb, "bt"), TourTheme::Bootstrap);
        assert_eq!(theme_of(&sb, "d"), TourTheme::DriverJs);
        assert_eq!(theme_of(&sb, "h"), TourTheme::Hopscotch);
        assert_eq!(theme_of(&sb, "i"), TourTheme::IntroJs);
        assert_eq!(sb.tours.len(), 9);
    }

    #[tokio::test]
    async fn a_tour_created_under_a_name_in_use_starts_again() {
        let mut sb = offline();
        sb.create_shepherd_tour().name("t").await.unwrap();
        sb.add_tour_step("one").name("t").await.unwrap();
        sb.create_introjs_tour().name("t").await.unwrap();
        assert_eq!(theme_of(&sb, "t"), TourTheme::IntroJs);
        assert!(sb.tours["t"].steps().is_empty());
    }

    #[tokio::test]
    async fn steps_go_to_the_named_tour_and_a_missing_tour_is_made_with_introjs() {
        let mut sb = offline();
        sb.create_bootstrap_tour().name("boot").await.unwrap();
        sb.add_tour_step("first").name("boot").await.unwrap();
        sb.add_tour_step("second")
            .name("boot")
            .selector("#go")
            .title("Go")
            .alignment(TourAlignment::Left)
            .duration(Duration::from_secs(2))
            .await
            .unwrap();
        // No tour of this name: Python makes an IntroJS one.
        sb.add_tour_step("lonely").name("new").await.unwrap();
        // No name at all: the tour called `default`.
        sb.add_tour_step("plain").await.unwrap();

        let steps = sb.tours["boot"].steps();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].message, "first");
        assert_eq!(steps[0].selector, None);
        assert_eq!(steps[1].selector.as_deref(), Some("#go"));
        assert_eq!(steps[1].title.as_deref(), Some("Go"));
        assert_eq!(steps[1].alignment, Some(TourAlignment::Left));
        assert_eq!(steps[1].duration, Some(Duration::from_secs(2)));
        assert_eq!(theme_of(&sb, "new"), TourTheme::IntroJs);
        assert_eq!(theme_of(&sb, "default"), TourTheme::IntroJs);
        assert_eq!(sb.tours["default"].steps()[0].message, "plain");
    }

    #[tokio::test]
    async fn a_step_selector_is_made_css_and_a_bad_xpath_is_refused() {
        let mut sb = offline();
        sb.add_tour_step("a").selector("name=q").await.unwrap();
        sb.add_tour_step("b").selector("//div[@id='x']").await.unwrap();
        sb.add_tour_step("c").selector("").await.unwrap();
        let selectors: Vec<_> = sb.tours["default"]
            .steps()
            .iter()
            .map(|step| step.selector.clone())
            .collect();
        assert_eq!(
            selectors,
            [Some("[name=\"q\"]".to_owned()), Some("div[id='x']".to_owned()), None]
        );
        let error = sb
            .add_tour_step("d")
            .selector("//div[text()='x']")
            .await
            .unwrap_err();
        assert!(matches!(error, SeleniumBaseError::InvalidSelector(_)), "{error:?}");
        assert_eq!(sb.tours["default"].steps().len(), 3, "the bad step was not added");
    }

    #[tokio::test]
    async fn the_introjs_colours_belong_to_the_case_and_are_checked() {
        let mut sb = offline();
        assert_eq!(sb.introjs_colors, IntroJsColors::default());
        sb.set_introjs_colors(Some("#f26721"), Some("#db5409")).unwrap();
        assert_eq!(sb.introjs_colors.theme_color(), "#f26721");
        assert_eq!(sb.introjs_colors.hover_color(), "#db5409");

        let error = sb.set_introjs_colors(Some("#123456"), Some("blue")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid configuration: Expecting a hex value color that starts with \"#\"!"
        );
        assert_eq!(sb.introjs_colors.theme_color(), "#f26721", "nothing changed");

        // None puts a colour back; nothing at all resets both.
        sb.set_introjs_colors(None, Some("#000")).unwrap();
        assert_eq!(sb.introjs_colors.theme_color(), tours::DEFAULT_THEME_COLOR);
        assert_eq!(sb.introjs_colors.hover_color(), "#000");
        sb.set_introjs_colors(None, None).unwrap();
        assert_eq!(sb.introjs_colors, IntroJsColors::default());
        assert_eq!(offline().introjs_colors, IntroJsColors::default());
    }

    #[tokio::test]
    async fn nothing_is_played_where_nobody_watches() {
        // A headless case with no browser behind it: any call to the driver
        // would panic, so this shows that none is made.
        let config = BrowserConfig {
            headless: true,
            ..BrowserConfig::default()
        };
        let mut sb = BaseCase::with_session(config, BrowserSession::disconnected());
        sb.add_tour_step("hello").await.unwrap();
        sb.play_tour().await.unwrap();
        sb.start_tour().interval(Duration::from_secs(1)).await.unwrap();
        // Python returns before it looks for the tour, too.
        sb.play_tour().name("missing").await.unwrap();
    }

    /// A page that shows the tours a test starts: it has every element, loads
    /// every library, and plays a Shepherd tour of `steps` steps by itself.
    /// `taken` counts the steps taken.
    fn tour_page(taken: Arc<AtomicUsize>, steps: usize, has_elements: bool) -> MockWebDriver {
        MockWebDriver::start(json!({}), move |request| {
            let script = request.body["script"].as_str().unwrap_or_default();
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/async") => Reply::Value(json!(true)),
                ("POST", "/execute/sync") if script.contains("readyState") => {
                    Reply::Value(json!("complete"))
                }
                ("POST", "/execute/sync") if script.starts_with("var s = Shepherd.activeTour") => {
                    let done = taken.load(Ordering::SeqCst);
                    if done >= steps {
                        // The tour is over: its object is gone.
                        Reply::Error(500, "javascript error")
                    } else {
                        Reply::Value(json!({
                            "active": true,
                            "pending": false,
                            "key": format!("step {done}"),
                            "element": "#go",
                        }))
                    }
                }
                ("POST", "/execute/sync") if script == "Shepherd.activeTour.next();" => {
                    taken.fetch_add(1, Ordering::SeqCst);
                    Reply::Value(Value::Null)
                }
                ("POST", "/element") | ("POST", "/elements") if !has_elements => {
                    Reply::Error(404, "no such element")
                }
                ("POST", "/element") => Reply::Value(element("e")),
                ("POST", "/elements") => Reply::Value(json!([element("e")])),
                ("GET", "/element/e/displayed" | "/element/e/enabled") => {
                    Reply::Value(json!(true))
                }
                ("GET", "/url") => Reply::Value(json!("https://example.test/start?x=1")),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    async fn two_step_shepherd_tour(driver: &MockWebDriver) -> BaseCase {
        let mut sb = driver.base_case().await.with_somebody_watching();
        sb.create_shepherd_tour().theme(ShepherdTheme::Dark).await.unwrap();
        sb.add_tour_step("Look here").selector("#go").await.unwrap();
        sb.add_tour_step("And then here").await.unwrap();
        sb
    }

    fn executed(driver: &MockWebDriver) -> Vec<String> {
        driver
            .all_requests()
            .iter()
            .filter_map(|request| request.body["script"].as_str().map(ToOwned::to_owned))
            .collect()
    }

    #[tokio::test]
    async fn playing_a_tour_that_does_not_exist_names_it() {
        let driver = tour_page(Arc::default(), 0, true);
        let mut sb = driver.base_case().await.with_somebody_watching();
        sb.add_tour_step("only").name("other").await.unwrap();
        let error = sb.play_tour().name("mine").await.unwrap_err();
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(_)), "{error:?}");
        assert!(error.to_string().contains("Tour {mine} does not exist!"), "{error}");
        let error = sb.play_tour().await.unwrap_err();
        assert!(error.to_string().contains("Tour {default} does not exist!"), "{error}");
    }

    #[tokio::test]
    async fn playing_a_tour_without_steps_is_refused() {
        let driver = tour_page(Arc::default(), 0, true);
        let mut sb = driver.base_case().await.with_somebody_watching();
        sb.create_tour().await.unwrap();
        let error = sb.play_tour().await.unwrap_err();
        assert!(error.to_string().contains("Tour {default} has no steps!"), "{error}");
    }

    #[tokio::test]
    async fn a_first_step_that_is_not_on_the_page_fails_with_pythons_text() {
        let driver = tour_page(Arc::default(), 0, false);
        let mut sb = two_step_shepherd_tour(&driver).await;
        // The element is waited for 7 seconds, scaled down for the test, and
        // the error message is shown for a moment only.
        sb.set_run_settings(
            RunSettings::default()
                .with_timeout_multiplier(0.1)
                .with_message_duration(Duration::from_millis(30)),
        );
        let error = sb.play_tour().await.unwrap_err();
        assert!(
            error.to_string().contains(
                "Tour Error: {'#go'} was not found! Exiting due to failure on first tour step!"
            ),
            "{error}"
        );
        // The tour was not started, and an error message was posted.
        let scripts = executed(&driver);
        assert!(!scripts.iter().any(|script| script.contains("tour.start()")));
        assert!(scripts.iter().any(|script| script.contains("Tour Error: {'#go'} was not found!")));
    }

    #[tokio::test]
    async fn an_autoplaying_tour_moves_on_by_itself_and_play_tour_returns_when_it_ends() {
        let taken = Arc::new(AtomicUsize::new(0));
        let driver = tour_page(Arc::clone(&taken), 2, true);
        let sb = two_step_shepherd_tour(&driver).await;

        let started = Instant::now();
        sb.play_tour()
            .interval(Duration::from_millis(100))
            .await
            .expect("the tour plays to its end");
        let played = started.elapsed();

        // An interval of 100 ms is raised to the least, half a second, for
        // each of the two steps.
        assert_eq!(taken.load(Ordering::SeqCst), 2, "each step was left once");
        assert!(played >= Duration::from_millis(1000), "played for {played:?}");
        assert!(played < Duration::from_secs(10), "played for {played:?}");

        let scripts = executed(&driver);
        let start = scripts
            .iter()
            .find(|script| script.contains("// Shepherd Tour"))
            .expect("the tour was started");
        assert!(start.contains("classes: 'shepherd-theme-dark',\n        scrollTo: true"));
        assert!(start.contains("attachTo: {element: '#go', on: 'top'}"));
        assert!(start.contains("buttons: midTourButtons"));
        assert!(start.contains("tour.start();\n$tour = tour;"));
        // The backdrop style went in before the tour.
        let style = scripts
            .iter()
            .position(|script| script.contains("shepherd-active"))
            .expect("the backdrop style");
        let started_at = scripts
            .iter()
            .position(|script| script.contains("// Shepherd Tour"))
            .unwrap();
        assert!(style < started_at);
        // Every library file was loaded, from the CDNs of Python.
        let loaded: Vec<String> = driver
            .requests("/execute/async")
            .iter()
            .filter_map(|request| request.body["script"].as_str().map(ToOwned::to_owned))
            .collect();
        assert!(loaded.iter().any(|s| s.contains("shepherd/1.8.1/js/shepherd.min.js")));
        assert!(loaded.iter().any(|s| s.contains("tether@1.4.7/dist/js/tether.min.js")));
        assert!(loaded.iter().any(|s| s.contains("shepherd-theme-square-dark.css")));
    }

    #[tokio::test]
    async fn the_interval_of_the_run_plays_a_tour_that_has_none_of_its_own() {
        let taken = Arc::new(AtomicUsize::new(0));
        let driver = tour_page(Arc::clone(&taken), 1, true);
        let mut sb = driver.base_case().await.with_somebody_watching();
        sb.set_run_settings(RunSettings::default().with_interval(Duration::from_millis(500)));
        sb.create_shepherd_tour().await.unwrap();
        sb.add_tour_step("only step").await.unwrap();
        sb.play_tour().await.expect("the tour plays to its end");
        assert_eq!(taken.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_introjs_colours_are_applied_when_the_tour_is_played() {
        let driver = tour_page(Arc::default(), 0, true);
        let mut sb = driver.base_case().await.with_somebody_watching();
        sb.set_introjs_colors(Some("#aa3300"), Some("#882200")).unwrap();
        sb.create_introjs_tour().await.unwrap();
        sb.add_tour_step("A step").await.unwrap();
        // The page has no IntroJS tour to ask about, so the player sees that
        // the tour is over; the style went in before that.
        sb.play_tour().await.unwrap();
        let scripts = executed(&driver);
        assert!(
            scripts.iter().any(|script| script.contains("background-color: #aa3300 !important;")),
            "{scripts:?}"
        );
    }

    #[tokio::test]
    async fn an_exported_tour_is_a_js_file_in_the_folder_and_checks_its_arguments() {
        let driver = tour_page(Arc::default(), 0, true);
        let mut sb = driver.base_case().await.with_somebody_watching();
        sb.create_hopscotch_tour().name("hops").await.unwrap();
        sb.add_tour_step("Hello <b>there</b>").name("hops").selector("#go").await.unwrap();

        // The checks come first, in the order of Python's.
        let error = sb.export_tour().name("nope").await.unwrap_err();
        assert!(error.to_string().contains("Tour {nope} does not exist!"), "{error}");
        let error = sb.export_tour().name("hops").filename("tour.html").await.unwrap_err();
        assert!(error.to_string().contains("Tour file must end in \".js\"!"), "{error}");

        let folder = tempfile::tempdir().unwrap();
        let path = sb
            .write_tour_export(folder.path(), "hops", "my_tour.js", "https://example.test/a")
            .unwrap();
        assert_eq!(path, folder.path().join("my_tour.js"));
        let script = std::fs::read_to_string(&path).unwrap();
        assert!(script.contains("if (window.location.href != \"https://example.test/a\") {"));
        assert!(script.contains("hopscotch@0.3.1/dist/js/hopscotch.min.js"));
        assert!(script.contains("function loadTour() { if ( typeof hopscotch !== \"undefined\" ) {"));
        assert!(script.contains("content: 'Hello <b>there</b>',"));
        assert!(script.contains("hopscotch.startTour(tour);"));
    }

    #[tokio::test]
    async fn the_default_export_goes_below_the_working_directory_and_starts_on_the_current_page() {
        let driver = tour_page(Arc::default(), 0, true);
        let mut sb = driver.base_case().await.with_somebody_watching();
        sb.add_tour_step("Hello").await.unwrap();
        let path = tempfile::tempdir().unwrap();
        // The folder is a relative one; build the file in a temporary folder
        // through the same code, and ask for the url the way the call does.
        let url = sb.get_current_url().await.unwrap();
        assert_eq!(url, "https://example.test/start?x=1");
        let written = sb
            .write_tour_export(&path.path().join(tours::EXPORT_FOLDER), "default", tours::DEFAULT_FILENAME, &url)
            .unwrap();
        assert!(written.ends_with("tours_exported/my_tour.js"), "{}", written.display());
        assert!(std::fs::read_to_string(written).unwrap().contains("example.test/start?x=1"));
    }
}
