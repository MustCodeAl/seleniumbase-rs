// Keyboard and mouse helpers.

impl BaseCase {
    /// Clicks the currently focused element.
    pub async fn click_active_element(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("document.activeElement.click();").await?;
        Ok(())
    }

    /// Clicks the nth visible element matching `css` (0-based).
    pub async fn click_nth_visible_element(
        &mut self,
        css: &str,
        n: usize,
    ) -> Result<(), SeleniumBaseError> {
        let visible = self.find_visible_elements(css).await?;
        let el = visible.get(n).ok_or_else(|| {
            SeleniumBaseError::AssertionFailed(format!(
                "Only {} visible elements matching '{}'",
                visible.len(),
                css
            ))
        })?;
        el.click().await.map_err(SeleniumBaseError::WebDriver)
    }

    /// Presses the Up arrow key on the active element.
    pub async fn press_up_arrow(&self) -> Result<(), SeleniumBaseError> {
        self.press_keys("up").await
    }

    /// Presses the Down arrow key on the active element.
    pub async fn press_down_arrow(&self) -> Result<(), SeleniumBaseError> {
        self.press_keys("down").await
    }

    /// Presses the Left arrow key on the active element.
    pub async fn press_left_arrow(&self) -> Result<(), SeleniumBaseError> {
        self.press_keys("left").await
    }

    /// Presses the Right arrow key on the active element.
    pub async fn press_right_arrow(&self) -> Result<(), SeleniumBaseError> {
        self.press_keys("right").await
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

    /// Hovers over `css`, then double-clicks it.
    pub async fn hover_and_double_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.hover(css).await?;
        self.double_click(css).await
    }

    /// Hovers over `css`, then JavaScript-clicks it.
    pub async fn hover_and_js_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.hover(css).await?;
        self.js_click(css).await
    }

    /// Highlights all elements matching `css`.
    pub async fn highlight_elements(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelectorAll('{}').forEach(el => {{ el.style.outline = '3px solid red'; el.style.background = 'yellow'; }});",
            js_escape(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Highlights `css` only if it is visible.
    pub async fn highlight_if_visible(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_element_visible(css).await.unwrap_or(false) {
            self.highlight(css).await?;
        }
        Ok(())
    }

    /// Highlights `css`, then types `text` into it.
    pub async fn highlight_type(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.highlight(css).await?;
        self.type_text(css, text).await
    }

    /// Highlights `css`, then updates its text content.
    pub async fn highlight_update_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.highlight(css).await?;
        self.set_text(css, text).await
    }

    /// Flashes a highlight on `css` `times` times.
    pub async fn flash(&self, css: &str, times: usize) -> Result<(), SeleniumBaseError> {
        let script = format!(
            r#"
            (async function() {{
                var el = document.querySelector('{}');
                if (!el) return;
                for (var i = 0; i < {}; i++) {{
                    el.style.outline = '4px solid red';
                    await new Promise(r => setTimeout(r, 200));
                    el.style.outline = '';
                    await new Promise(r => setTimeout(r, 200));
                }}
            }})();
            "#,
            js_escape(css),
            times
        );
        self.execute_script(&script).await?;
        Ok(())
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
    /// shows no supported widget. Whether the site then accepts the challenge
    /// is up to the site; check the page afterwards.
    ///
    /// # Errors
    ///
    /// Returns an error if the page script or a DevTools command fails.
    pub async fn solve_captcha(
        &self,
    ) -> Result<Option<crate::sb_cdp::Captcha>, SeleniumBaseError> {
        use crate::sb_cdp::Captcha;

        let found = self
            .execute_script(&format!("return {};", Captcha::locate_script()))
            .await?;
        let Some((kind, area)) = Captcha::parse_located(&found) else {
            return Ok(None);
        };

        let plan = kind.plan(area);
        self.dispatch_mouse("mouseMoved", plan.press.x, plan.press.y, 0).await?;
        self.sleep(0.15).await;
        match plan.release {
            None => self.cdp_mouse_click(plan.press.x, plan.press.y).await?,
            Some(release) => {
                self.dispatch_mouse("mousePressed", plan.press.x, plan.press.y, 1).await?;
                // Move in steps: a slider ignores a jump from end to end.
                const STEPS: u32 = 12;
                for step in 1..=STEPS {
                    let along = f64::from(step) / f64::from(STEPS);
                    let x = plan.press.x + (release.x - plan.press.x) * along;
                    let y = plan.press.y + (release.y - plan.press.y) * along;
                    self.dispatch_mouse("mouseMoved", x, y, 1).await?;
                    self.sleep(0.02).await;
                }
                self.dispatch_mouse("mouseReleased", release.x, release.y, 0).await?;
            }
        }
        Ok(Some(kind))
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
