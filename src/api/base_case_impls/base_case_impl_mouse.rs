// Keyboard and mouse helpers.

use crate::api::misc_calls::{Arrow, ClickNth, HoverAndClick, HoverClick, PressArrow};

/// The pointer events a CAPTCHA press is made of, sent over the DevTools
/// Protocol, so this needs a Chromium browser.
impl crate::sb_cdp::PointerSink for BaseCase {
    async fn pointer_move(
        &self,
        at: crate::sb_cdp::Point,
        held: bool,
    ) -> Result<(), SeleniumBaseError> {
        self.dispatch_mouse("mouseMoved", at.x, at.y, u8::from(held))
            .await
    }

    async fn pointer_down(&self, at: crate::sb_cdp::Point) -> Result<(), SeleniumBaseError> {
        self.dispatch_mouse("mousePressed", at.x, at.y, 1).await
    }

    async fn pointer_up(&self, at: crate::sb_cdp::Point) -> Result<(), SeleniumBaseError> {
        self.dispatch_mouse("mouseReleased", at.x, at.y, 0).await
    }
}

/// What the CAPTCHA solver reads: the page through a script, the cookies of
/// the browser.
impl crate::sb_cdp::CaptchaSurface for BaseCase {
    async fn eval(&self, script: &str) -> Result<serde_json::Value, SeleniumBaseError> {
        self.execute_script(&format!("return {script};")).await
    }

    async fn cookie_names(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let cookies = self.get_cookies().await?;
        let list = cookies.get("cookies").unwrap_or(&cookies);
        Ok(list
            .as_array()
            .map(|cookies| {
                cookies
                    .iter()
                    .filter_map(|cookie| cookie.get("name").and_then(serde_json::Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }
}


impl BaseCase {
    /// Clicks the currently focused element.
    pub async fn click_active_element(&self) -> Result<(), SeleniumBaseError> {
        let before = self.demo_page_before().await;
        self.execute_script("document.activeElement.click();").await?;
        self.demo_pause_after(before).await;
        Ok(())
    }

    /// Clicks the `number`th visible element `selector` matches, counting from
    /// 1, as Python's `click_nth_visible_element(selector, number, by,
    /// timeout=None, scroll=True)` does. The keywords are
    /// [`ClickNth::timeout`] and [`ClickNth::scroll`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use seleniumbase_rs::api::base_case::BaseCase;
    /// # async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.click_nth_visible_element("[type=checkbox]", 5).await?; // the fifth checkbox
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `number` is 0, since the
    /// count starts at 1, and [`SeleniumBaseError::AssertionFailed`] if fewer
    /// than `number` visible elements match.
    pub fn click_nth_visible_element<'a>(
        &'a mut self,
        selector: &'a str,
        number: usize,
    ) -> ClickNth<'a> {
        ClickNth::new(self, selector, number)
    }

    /// Presses the Up arrow key, as Python's
    /// `press_up_arrow(selector="body", times=1)` does. The keywords are
    /// [`PressArrow::selector`] and [`PressArrow::times`].
    pub fn press_up_arrow(&self) -> PressArrow<'_> {
        PressArrow::new(self, Arrow::Up)
    }

    /// Presses the Down arrow key; see
    /// [`press_up_arrow`](Self::press_up_arrow).
    pub fn press_down_arrow(&self) -> PressArrow<'_> {
        PressArrow::new(self, Arrow::Down)
    }

    /// Presses the Left arrow key; see
    /// [`press_up_arrow`](Self::press_up_arrow).
    pub fn press_left_arrow(&self) -> PressArrow<'_> {
        PressArrow::new(self, Arrow::Left)
    }

    /// Presses the Right arrow key; see
    /// [`press_up_arrow`](Self::press_up_arrow).
    pub fn press_right_arrow(&self) -> PressArrow<'_> {
        PressArrow::new(self, Arrow::Right)
    }

    /// Alias for `hover`.
    pub async fn hover_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.hover(css).await
    }

    /// Alias for `hover`.
    pub async fn hover_on_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.hover(css).await
    }

    /// Alias for `hover`.
    pub async fn hover_over_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.hover(css).await
    }

    /// Hovers over `hover_selector`, then double-clicks `click_selector`, which
    /// usually appears on the hover. Python's `hover_and_double_click(
    /// hover_selector, click_selector, hover_by, click_by, timeout=None)`; the
    /// keyword is [`HoverAndClick::timeout`].
    pub fn hover_and_double_click<'a>(
        &'a mut self,
        hover_selector: &'a str,
        click_selector: &'a str,
    ) -> HoverAndClick<'a> {
        HoverAndClick::new(self, hover_selector, click_selector, HoverClick::Double)
    }

    /// Hovers over `hover_selector`, then JavaScript-clicks `click_selector`.
    /// Python's `hover_and_js_click(hover_selector, click_selector, hover_by,
    /// click_by, timeout=None)`; the keyword is [`HoverAndClick::timeout`].
    pub fn hover_and_js_click<'a>(
        &'a mut self,
        hover_selector: &'a str,
        click_selector: &'a str,
    ) -> HoverAndClick<'a> {
        HoverAndClick::new(self, hover_selector, click_selector, HoverClick::Js)
    }
    
    /// The work of [`click_nth_visible_element`](Self::click_nth_visible_element).
    pub(crate) async fn run_click_nth(
        &mut self,
        selector: &str,
        number: usize,
        timeout: Option<Duration>,
        scroll: bool,
    ) -> Result<(), SeleniumBaseError> {
        if number == 0 {
            return Err(SeleniumBaseError::InvalidConfig(
                "number counts from 1, as in Python (got 0)".to_owned(),
            ));
        }
        let seconds = self.wait_secs(timeout);
        self.await_ready_within(selector, Ready::Present, seconds)
            .await?;
        let visible = self.find_visible_elements(selector).await?;
        let Some(element) = visible.get(number - 1) else {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Not enough matching {{{selector}}} elements to click number {number}!"
            )));
        };
        if scroll {
            element.scroll_into_view().await?;
        }
        element.click().await?;
        Ok(())
    }
    
    /// The work of the four `press_*_arrow` calls.
    pub(crate) async fn run_press_arrow(
        &self,
        arrow: Arrow,
        selector: &str,
        times: u32,
    ) -> Result<(), SeleniumBaseError> {
        if times < 1 {
            return Ok(());
        }
        let seconds = self.wait_secs(None);
        self.await_ready_within(selector, Ready::Present, seconds)
            .await?;
        self.demo_highlight(selector).await?;
        let element = self.session.webdriver()?
            .find(Selector::auto(selector).to_by()?)
            .await?;
        if !self.run_settings.demo_mode() && !self.run_settings.slow_mode() {
            element.scroll_into_view().await?;
        }
        let key = match arrow {
            Arrow::Up => Key::Up,
            Arrow::Down => Key::Down,
            Arrow::Left => Key::Left,
            Arrow::Right => Key::Right,
        };
        for _ in 0..times {
            element.send_keys(key.value().to_string()).await?;
            tokio::time::sleep(Duration::from_millis(10)).await;
            if self.run_settings.slow_mode() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        Ok(())
    }
    
    /// The work of [`hover_and_double_click`](Self::hover_and_double_click) and
    /// [`hover_and_js_click`](Self::hover_and_js_click).
    pub(crate) async fn run_hover_and_click(
        &mut self,
        hover_selector: &str,
        click_selector: &str,
        kind: HoverClick,
        timeout: Option<Duration>,
    ) -> Result<(), SeleniumBaseError> {
        let seconds = self.wait_secs(timeout);
        self.await_ready_within(hover_selector, Ready::Visible, seconds)
            .await?;
        self.hover(hover_selector).await?;
        self.await_ready_within(click_selector, Ready::Visible, seconds)
            .await?;
        match kind {
            HoverClick::Double => self.double_click(click_selector).await,
            HoverClick::Js => self.js_click(click_selector).await,
        }
    }
}

impl BaseCase {
    /// Clicks the first `css` match inside the first `parent_css` match.
    ///
    /// When the parent is an `iframe` the click happens inside the frame and
    /// focus then returns to the top-level page. Corresponds to Python's
    /// `nested_click`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if either element is
    /// missing, or [`SeleniumBaseError::WebDriver`] if the click fails.
    pub async fn nested_click(
        &mut self,
        parent_css: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let parent = self.find_element(parent_css).await?;
        let tag = parent.tag_name().await?;
        if tag.eq_ignore_ascii_case("iframe") || tag.eq_ignore_ascii_case("frame") {
            self.switch_to_frame(parent_css).await?;
            let clicked = self.click(css).await;
            self.switch_to_default_content().await?;
            return clicked;
        }
        let by = Selector::auto(css).to_by()?;
        let child = parent
            .find(by)
            .await
            .map_err(|_| SeleniumBaseError::element_not_found(format!("{parent_css} {css}")))?;
        child.click().await?;
        Ok(())
    }

    /// Attempts the first supported CAPTCHA widget on the page.
    ///
    /// Checkbox widgets (Cloudflare Turnstile, reCAPTCHA, hCaptcha, Friendly
    /// Captcha) are clicked and the DataDome slider is dragged, using trusted
    /// mouse events sent over the DevTools Protocol, so this needs a Chromium
    /// browser. Returns the widget that was attempted, or `None` if the page
    /// shows no supported widget. The widget is found in shadow roots and
    /// readable frames, pressed along a human path once it stands still, and
    /// checked for a result (a token in its response field, the clearance
    /// cookie, a page with no widget); the steps are those of
    /// [`Page::solve_captcha`](crate::sb_cdp::Page::solve_captcha), which this
    /// shares. A widget that could show a result and did not is pressed again,
    /// up to three times. Whether the site then accepts the challenge
    /// is up to the site; check the page afterwards.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Captcha`] if the widget was pressed the
    /// allowed number of times and the page never showed it cleared, and an
    /// error if the page script or a DevTools command fails.
    pub async fn solve_captcha(
            &self,
        ) -> Result<Option<crate::sb_cdp::Captcha>, SeleniumBaseError> {
            Ok(self
                .solve_captcha_with(&crate::sb_cdp::SolveOptions::default())
                .await?
                .map(|solved| solved.captcha))
        }
    
        /// Like [`solve_captcha`](Self::solve_captcha), with the settings of
        /// `options`, and says what was done and what showed it worked.
        ///
        /// # Errors
        ///
        /// See [`solve_captcha`](Self::solve_captcha).
        pub async fn solve_captcha_with(
            &self,
            options: &crate::sb_cdp::SolveOptions,
        ) -> Result<Option<crate::sb_cdp::Solved>, SeleniumBaseError> {
            crate::sb_cdp::solve_captcha(self, options).await
        }

    /// Sends one raw mouse event; `buttons` is the DevTools button bitmask.
    async fn dispatch_mouse(
        &self,
        kind: &str,
        x: f64,
        y: f64,
        buttons: u8,
    ) -> Result<(), SeleniumBaseError> {
        self.execute_cdp_with_params(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": kind,
                "x": x,
                "y": y,
                "button": if buttons == 0 && kind != "mouseReleased" { "none" } else { "left" },
                "buttons": buttons,
                "clickCount": u8::from(kind != "mouseMoved"),
            }),
        )
        .await?;
        Ok(())
    }
}
