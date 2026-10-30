// Python's highlight family, `flash`, and the work Demo Mode does around an
// action: the outline before it, the green flash and Messenger message after an
// assertion that passed. The call builders and the page scripts are in
// `crate::api::demo`; this file holds what needs the case's private state.

use crate::api::demo::{
    self, Flash, Highlight, HighlightClick, HighlightElements, HighlightIfVisible, HighlightType,
};
use crate::api::run_settings::DEFAULT_HIGHLIGHT_LOOPS;

/// How much longer than the message lasts the demo-mode message waits (Python:
/// `msg_dur + 0.07`).
const DEMO_MESSAGE_SETTLE: Duration = Duration::from_millis(70);

impl BaseCase {
    /// Scrolls to the element `selector` picks, slowly, and outlines it with a
    /// box shadow that cycles through colours, then puts its style back.
    /// Corresponds to Python's `highlight(selector, loops=None, scroll=True,
    /// timeout=None)`: the keywords are the builder's
    /// [`loops`](Highlight::loops), [`scroll`](Highlight::scroll) and
    /// [`timeout`](Highlight::timeout).
    ///
    /// The call waits until the animation has ended: four loops of about 0.2 s,
    /// or `--highlights` (which wins over `loops`), halved where nobody watches.
    ///
    /// # Errors
    ///
    /// Returns the wait error if the element is not visible within the timeout,
    /// and [`SeleniumBaseError::Skipped`] if ESC was pressed (`--esc-end`).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use seleniumbase_rs::api::base_case::BaseCase;
    /// # async fn demo(sb: &BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.highlight("h1").await?;
    /// sb.highlight("h1").loops(2).scroll(false).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn highlight<'a>(&'a self, selector: &'a str) -> Highlight<'a> {
        Highlight::new(self, selector)
    }

    /// Highlights the element `selector` picks, then clicks it. Corresponds to
    /// Python's `highlight_click(selector, loops=3, scroll=True, timeout=None)`.
    /// In demo mode the click does the highlighting, so the call only waits for
    /// the element and clicks.
    pub fn highlight_click<'a>(&'a mut self, selector: &'a str) -> HighlightClick<'a> {
        HighlightClick::new(self, selector)
    }

    /// Highlights the field `selector` picks, then replaces its text with
    /// `text`. Corresponds to Python's `highlight_type(selector, text, loops=3,
    /// scroll=True, timeout=None)`, which is `highlight_update_text`.
    pub fn highlight_type<'a>(
        &'a mut self,
        selector: &'a str,
        text: &'a str,
    ) -> HighlightType<'a> {
        HighlightType::new(self, selector, text)
    }

    /// Highlights the field `selector` picks, then replaces its text with
    /// `text`. Corresponds to Python's `highlight_update_text(selector, text,
    /// loops=3, scroll=True, timeout=None)`.
    pub fn highlight_update_text<'a>(
        &'a mut self,
        selector: &'a str,
        text: &'a str,
    ) -> HighlightType<'a> {
        HighlightType::new(self, selector, text)
    }

    /// Highlights the element `selector` picks if it is visible, and does
    /// nothing, with no error, if it is not. Corresponds to Python's
    /// `highlight_if_visible(selector, loops=4, scroll=True)`.
    pub fn highlight_if_visible<'a>(&'a self, selector: &'a str) -> HighlightIfVisible<'a> {
        HighlightIfVisible::new(self, selector)
    }

    /// Highlights every visible element `selector` matches, one after the
    /// other. Corresponds to Python's `highlight_elements(selector, loops=None,
    /// scroll=True, limit=0)`; [`limit`](HighlightElements::limit) 0 is all.
    pub fn highlight_elements<'a>(&'a self, selector: &'a str) -> HighlightElements<'a> {
        HighlightElements::new(self, selector)
    }

    /// Paints a quickly-vanishing dot over the element `selector` picks.
    /// Corresponds to Python's `flash(selector, duration=1, color="44CC88",
    /// pause=0)`: the keywords are [`duration`](Flash::duration),
    /// [`color`](Flash::color) and [`pause`](Flash::pause).
    pub fn flash<'a>(&'a self, selector: &'a str) -> Flash<'a> {
        Flash::new(self, selector)
    }

    /// Scrolls to the element `selector` picks, flashes it green, shows `message`
    /// as a Messenger success message (HTML, as in Python) and waits until it
    /// has gone. Corresponds to Python's `post_message_and_highlight(message,
    /// selector)`.
    ///
    /// # Errors
    ///
    /// Returns the wait error if the element is not visible within 7 s.
    pub async fn post_message_and_highlight(
        &self,
        message: &str,
        selector: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.show_assert_success(message, Some(selector)).await
    }

    /// Turns demo mode on: from now on every action highlights its element,
    /// pauses afterwards, and the assertions announce themselves. Corresponds to
    /// Python's `activate_demo_mode()`, and is `RunSettings::with_demo_mode(true)`
    /// for a case that is running.
    ///
    /// # Errors
    ///
    /// This does not fail; it returns a `Result` as Python's method may.
    pub async fn activate_demo_mode(&mut self) -> Result<(), SeleniumBaseError> {
        self.run_settings = self.run_settings.clone().with_demo_mode(true);
        Ok(())
    }

    /// Turns demo mode off. Corresponds to Python's `deactivate_demo_mode()`.
    ///
    /// # Errors
    ///
    /// This does not fail; it returns a `Result` as Python's method may.
    pub async fn deactivate_demo_mode(&mut self) -> Result<(), SeleniumBaseError> {
        self.run_settings = self.run_settings.clone().with_demo_mode(false);
        Ok(())
    }

    /// How many loops a highlight plays: see [`demo::resolve_loops`].
    pub(crate) fn highlight_loops(&self, requested: Option<u32>, method_default: u32) -> u32 {
        demo::resolve_loops(
            requested,
            method_default,
            self.run_settings.highlights(),
            self.nobody_watches(),
        )
    }

    /// The visible element `css` picks (`html` is `body`), waiting up to
    /// `timeout`.
    async fn visible_element(
        &self,
        css: &str,
        timeout: Duration,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        let by = Selector::auto(demo::highlight_target(css)).to_by()?;
        self.session
            .wait_for_element_visible(by, demo::whole_seconds_up(timeout))
            .await
    }

    /// Runs `script` with `element` as `arguments[0]`. A script that returns a
    /// promise is waited for.
    async fn run_on_element(
        &self,
        script: &str,
        element: &thirtyfour::WebElement,
    ) -> Result<Value, SeleniumBaseError> {
        let returned = self.session.webdriver()?
            .execute(script, vec![element.to_json()?])
            .await?;
        Ok(returned.json().clone())
    }

    /// Plays the highlight animation on `element` and waits the 65 ms Python
    /// waits after it.
    async fn animate_highlight(
        &self,
        element: &thirtyfour::WebElement,
        loops: u32,
        scroll: bool,
    ) -> Result<(), SeleniumBaseError> {
        let script = demo::highlight_animation_script(loops, demo::STEP_MS, scroll);
        self.run_on_element(&script, element).await?;
        tokio::time::sleep(demo::SETTLE).await;
        Ok(())
    }

    /// The work of [`highlight`](Self::highlight).
    pub(crate) async fn run_highlight(
        &self,
        css: &str,
        loops: Option<u32>,
        scroll: bool,
        timeout: Duration,
    ) -> Result<(), SeleniumBaseError> {
        self.skip_if_esc().await?;
        let element = self.visible_element(css, timeout).await?;
        let loops = self.highlight_loops(loops, DEFAULT_HIGHLIGHT_LOOPS);
        self.animate_highlight(&element, loops, scroll).await
    }

    /// The work of [`highlight_click`](Self::highlight_click).
    pub(crate) async fn run_highlight_click(
        &mut self,
        css: &str,
        loops: Option<u32>,
        scroll: bool,
        timeout: Duration,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.visible_element(css, timeout).await?;
        if !self.run_settings.demo_mode() {
            let loops = self.highlight_loops(loops, demo::CLICK_LOOPS);
            self.animate_highlight(&element, loops, scroll).await?;
        }
        self.click(css).await
    }

    /// The work of [`highlight_type`](Self::highlight_type).
    pub(crate) async fn run_highlight_type(
        &mut self,
        css: &str,
        text: &str,
        loops: Option<u32>,
        scroll: bool,
        timeout: Duration,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.visible_element(css, timeout).await?;
        if !self.run_settings.demo_mode() {
            let loops = self.highlight_loops(loops, demo::CLICK_LOOPS);
            self.animate_highlight(&element, loops, scroll).await?;
        }
        self.update_text(css, text).await
    }

    /// The work of [`highlight_if_visible`](Self::highlight_if_visible).
    pub(crate) async fn run_highlight_if_visible(
        &self,
        css: &str,
        loops: Option<u32>,
        scroll: bool,
    ) -> Result<(), SeleniumBaseError> {
        if !self.is_element_visible(css).await.unwrap_or(false) {
            return Ok(());
        }
        // A page that changes under the highlight is not a failure; only ESC
        // ends the test.
        match self
            .run_highlight(css, loops, scroll, demo::DEFAULT_TIMEOUT)
            .await
        {
            Err(skipped @ SeleniumBaseError::Skipped(_)) => Err(skipped),
            _ => Ok(()),
        }
    }

    /// The work of [`highlight_elements`](Self::highlight_elements).
    pub(crate) async fn run_highlight_elements(
        &self,
        css: &str,
        loops: Option<u32>,
        scroll: bool,
        limit: usize,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(demo::highlight_target(css)).to_by()?;
        let elements = self.session.find_all(by).await?;
        let loops = self.highlight_loops(loops, DEFAULT_HIGHLIGHT_LOOPS);
        let mut count = 0_usize;
        for element in elements {
            let shown = element.is_displayed().await.unwrap_or(false);
            if shown && self.animate_highlight(&element, loops, scroll).await.is_ok() {
                count += 1;
            }
            if limit > 0 && count >= limit {
                break;
            }
        }
        Ok(())
    }

    /// The work of [`flash`](Self::flash).
    pub(crate) async fn run_flash(
        &self,
        css: &str,
        duration: Duration,
        color: &str,
        pause: Duration,
    ) -> Result<(), SeleniumBaseError> {
        if !demo::is_hex_colour(color) {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "the flash colour {color:?} is not hex digits such as \"44CC88\""
            )));
        }
        let element = self.visible_element(css, demo::DEFAULT_TIMEOUT).await?;
        let id = format!("flash-{:016x}", rand::rng().random::<u64>());
        // A page that cannot show the dot is not a failure, as in Python.
        let _ = self
            .run_on_element(&demo::flash_script(color, duration, &id), &element)
            .await;
        if !pause.is_zero() {
            tokio::time::sleep(pause).await;
        }
        Ok(())
    }

    /// What demo mode does to the element of an action before the action: the
    /// highlight animation, with its slow scroll. A selector the page cannot
    /// show is the action's business, not this one's.
    pub(crate) async fn demo_outline(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let element = self.visible_element(css, demo::DEMO_OUTLINE_TIMEOUT).await?;
        let loops = self.highlight_loops(None, DEFAULT_HIGHLIGHT_LOOPS);
        self.animate_highlight(&element, loops, true).await
    }

    /// What slow mode does to the element of an action: wait 0.08 s, scroll
    /// to it slowly, wait 0.12 s.
    pub(crate) async fn slow_mode_scroll(&self, css: &str) -> Result<(), SeleniumBaseError> {
        tokio::time::sleep(demo::SLOW_BEFORE).await;
        let element = self.visible_element(css, demo::DEMO_OUTLINE_TIMEOUT).await?;
        self.run_on_element(&demo::slow_scroll_script(), &element)
            .await?;
        tokio::time::sleep(demo::SLOW_AFTER).await;
        Ok(())
    }

    /// In demo mode, announces an assertion that passed: flashes the element
    /// `css` picks green (none for a message only) and shows `markup` as a
    /// Messenger success message, as Python's `__highlight_with_assert_success`
    /// does. Does nothing outside demo mode, and never fails the assertion.
    pub(crate) async fn demo_assert_success(&self, markup: &str, css: Option<&str>) {
        if self.run_settings.demo_mode() {
            let _ = self.show_assert_success(markup, css).await;
        }
    }

    /// The effect of a passed assertion: scroll, five-colour flash, the
    /// message (and its wait), then the element's shadow put back.
    async fn show_assert_success(
        &self,
        markup: &str,
        css: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        let Some(css) = css else {
            self.post_demo_success_message(markup).await;
            return Ok(());
        };
        let element = self.visible_element(css, demo::DEFAULT_TIMEOUT).await?;
        let script = demo::success_flash_script(demo::STEP_MS, true);
        self.run_on_element(&script, &element).await?;
        self.post_demo_success_message(markup).await;
        let restored = self
            .run_on_element(demo::RESTORE_SHADOW_SCRIPT, &element)
            .await;
        tokio::time::sleep(demo::SETTLE).await;
        restored.map(drop)
    }

    /// Shows `markup` as a success message in the future theme at the bottom
    /// right (the top right in a mobile emulator) and waits until it has gone,
    /// as Python's `post_messenger_success_message` does. True when it was shown.
    async fn post_demo_success_message(&self, markup: &str) -> bool {
        let location = if self.config.mobile {
            MessengerLocation::TopRight
        } else {
            MessengerLocation::BottomRight
        };
        let settings = MessengerSettings {
            location,
            ..MessengerSettings::default()
        };
        if self.set_messenger_theme(settings).await.is_err() {
            return false;
        }
        let shown = self.message_time(None);
        let options = MessageOptions {
            duration: shown,
            pause: false,
            style: MessageStyle::Success,
        };
        let script = format!("return {};", post_markup_script(markup, &options));
        if !self.messenger_posted(&script).await {
            return false;
        }
        tokio::time::sleep(shown + DEMO_MESSAGE_SETTLE).await;
        true
    }
}
