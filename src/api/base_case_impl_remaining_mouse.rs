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
