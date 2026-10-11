// Native GUI automation helpers for `BaseCase`.
// Included at the end of `base_case.rs` so the code lives in the same module
// and can access private fields and helpers.

use crate::api::gui::Gui;

/// A function-body script that returns the position of the centre of the
/// element matching `css` as screen coordinates `{x, y}`, which is where the
/// native pointer has to go to click it.
///
/// The browser reports an element's position relative to its page, so the
/// script adds what lies between the page and the screen: the window's own
/// position, its toolbar (the part of the outer height that is not page), its
/// side borders, and the offset of every same-origin frame the element sits
/// in. It scrolls the element into view first, since a pointer cannot click
/// what is off screen.
fn element_center_script(css: &str) -> String {
    format!(
        "var selector = {}; \
         var el = document.querySelector(selector); \
         if (!el) {{ throw new Error('Element not found: ' + selector); }} \
         el.scrollIntoView({{block: 'center', inline: 'center'}}); \
         var r = el.getBoundingClientRect(); \
         var x = r.left + r.width / 2; \
         var y = r.top + r.height / 2; \
         var win = window; \
         while (win.parent !== win) {{ \
             var frame = null; \
             try {{ frame = win.frameElement; }} catch (e) {{}} \
             if (!frame) {{ break; }} \
             var f = frame.getBoundingClientRect(); \
             x += f.left + frame.clientLeft; \
             y += f.top + frame.clientTop; \
             win = win.parent; \
         }} \
         var border = Math.max(0, (win.outerWidth - win.innerWidth) / 2); \
         var toolbar = Math.max(0, win.outerHeight - win.innerHeight - border); \
         return {{x: Math.round(win.screenX + border + x), y: Math.round(win.screenY + toolbar + y)}};",
        js_string_literal(css)
    )
}

/// The `(x, y)` in the `{x, y}` object `element_center_script` returned.
fn screen_point(value: &Value) -> Result<(i32, i32), SeleniumBaseError> {
    let coordinate = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_f64)
            .map(|number| number.round() as i32)
            .ok_or_else(|| {
                SeleniumBaseError::AssertionFailed(format!(
                    "expected a number for '{name}' in the element position, got {value}"
                ))
            })
    };
    Ok((coordinate("x")?, coordinate("y")?))
}

impl BaseCase {
    /// Query the screen coordinates of the center of the element matching
    /// `css`. Returns `(x, y)` in absolute screen pixels.
    ///
    /// Takes the window's toolbar and borders into account, and the frames the
    /// element is in when they are of the same origin. Assumes the browser's
    /// pixels are the screen's, which holds when the display is not scaled.
    async fn element_center_screen_coords(&self, css: &str) -> Result<(i32, i32), SeleniumBaseError> {
        let value = self
            .session
            .execute_script(&element_center_script(css))
            .await?;
        screen_point(&value)
    }

    /// Move the mouse to absolute screen coordinates `(x, y)` and click.
    pub fn gui_click_x_y(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
        let mut gui = Gui::new()?;
        gui.move_mouse(x, y)?;
        gui.click()?;
        Ok(())
    }

    /// Move the mouse to the center of the element matching `css` and click.
    pub async fn gui_click_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let (x, y) = self.element_center_screen_coords(css).await?;
        let mut gui = Gui::new()?;
        gui.move_mouse(x, y)?;
        gui.click()?;
        Ok(())
    }

    /// Move the mouse to the center of the element matching `css` plus an
    /// offset and click.
    pub async fn gui_click_with_offset(
        &mut self,
        css: &str,
        x: i32,
        y: i32,
    ) -> Result<(), SeleniumBaseError> {
        let (cx, cy) = self.element_center_screen_coords(css).await?;
        let mut gui = Gui::new()?;
        gui.move_mouse(cx + x, cy + y)?;
        gui.click()?;
        Ok(())
    }

    /// Press a single key by name. See [`Gui::key_from_str`] for valid names.
    pub fn gui_press_key(&self, key: &str) -> Result<(), SeleniumBaseError> {
        let mut gui = Gui::new()?;
        gui.press_key(key)?;
        Ok(())
    }

    /// Press a chord of keys by name. Modifier keys are held while normal keys
    /// are clicked.
    pub fn gui_press_keys(&self, keys: &[&str]) -> Result<(), SeleniumBaseError> {
        let mut gui = Gui::new()?;
        gui.press_keys(keys)?;
        Ok(())
    }

    /// Type the given text using native input.
    pub fn gui_write(&self, text: &str) -> Result<(), SeleniumBaseError> {
        let mut gui = Gui::new()?;
        gui.write(text)?;
        Ok(())
    }

    /// Drag the mouse from one screen point to another.
    pub fn gui_drag_and_drop_points(
        &mut self,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
    ) -> Result<(), SeleniumBaseError> {
        let mut gui = Gui::new()?;
        gui.drag((from_x, from_y), (to_x, to_y))?;
        Ok(())
    }

    /// Drag the center of `source_css` to the center of `target_css`.
    pub async fn gui_drag_and_drop(
        &mut self,
        source_css: &str,
        target_css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let from = self.element_center_screen_coords(source_css).await?;
        let to = self.element_center_screen_coords(target_css).await?;
        let mut gui = Gui::new()?;
        gui.drag(from, to)?;
        Ok(())
    }

    /// Press the left mouse button at `(x, y)` without releasing it.
    /// The held coordinates are tracked so `gui_release` can resume the
    /// position before releasing.
    pub fn gui_click_and_hold(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
        let mut gui = Gui::new()?;
        gui.move_mouse(x, y)?;
        gui.mouse_down()?;
        self.gui_held = Some((x, y));
        Ok(())
    }

    /// Release the left mouse button, moving back to the last held coordinates
    /// if a prior `gui_click_and_hold` call was made.
    pub fn gui_release(&mut self) -> Result<(), SeleniumBaseError> {
        let mut gui = Gui::new()?;
        if let Some((x, y)) = self.gui_held {
            gui.move_mouse(x, y)?;
        }
        gui.mouse_up()?;
        self.gui_held = None;
        Ok(())
    }

    /// Move the mouse to the center of the element matching `css`.
    pub async fn gui_move_to_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let (x, y) = self.element_center_screen_coords(css).await?;
        let mut gui = Gui::new()?;
        gui.move_mouse(x, y)?;
        Ok(())
    }

    /// Move the mouse to absolute screen coordinates `(x, y)`.
    pub fn gui_hover_x_y(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
        let mut gui = Gui::new()?;
        gui.move_mouse(x, y)?;
        Ok(())
    }

    /// Move the mouse to the center of the element matching `css`.
    pub async fn gui_hover_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.gui_move_to_element(css).await
    }

    /// Move the mouse to the center of the element matching `css` and click it.
    pub async fn gui_hover_and_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let (x, y) = self.element_center_screen_coords(css).await?;
        let mut gui = Gui::new()?;
        gui.move_mouse(x, y)?;
        gui.click()?;
        Ok(())
    }

    // --- UC-mode GUI bypass aliases ---
    // These are the same native-input helpers as `gui_*`, but exposed under the
    // `uc_*` naming convention used by Python SeleniumBase for anti-detection
    // workflows that bypass the browser input path.

    /// UC alias for [`gui_click_x_y`].
    pub fn uc_gui_click_x_y(&mut self, x: i32, y: i32) -> Result<(), SeleniumBaseError> {
        self.gui_click_x_y(x, y)
    }

    /// UC alias for [`gui_write`].
    pub fn uc_gui_write(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.gui_write(text)
    }

    /// UC alias for [`gui_press_key`].
    pub fn uc_gui_press_key(&self, key: &str) -> Result<(), SeleniumBaseError> {
        self.gui_press_key(key)
    }

    /// Best-effort CAPTCHA handler: waits for the element matching `css` to be
    /// visible, then clicks it using native mouse input.
    ///
    /// The wait matters because a challenge widget loads after the page does.
    /// This is a pragmatic fallback for simple checkbox-style challenges: it
    /// clicks where a person would and nothing more. It does not solve
    /// audio/image CAPTCHAs, and it does not check whether the click passed
    /// the challenge.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not
    /// become visible, and [`SeleniumBaseError::Gui`] if the native click
    /// fails.
    pub async fn uc_gui_handle_captcha(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.wait_for_element_visible(css, self.timeout_secs).await?;
        self.gui_click_element(css).await
    }
}

#[cfg(test)]
mod tests_gui {
    use super::chrome_support::open_html;
    use super::*;
    use serde_json::json;

    #[test]
    fn the_selector_is_data_in_the_script_not_code() {
        let script = element_center_script("a[title='x\"y'] \\ \n");
        assert!(
            script.starts_with(r#"var selector = "a[title='x\"y'] \\ \n";"#),
            "{script}"
        );
        assert!(script.contains("throw new Error('Element not found: ' + selector)"));
    }

    #[test]
    fn a_screen_point_comes_from_the_x_and_y_of_the_script_result() {
        assert_eq!(screen_point(&json!({"x": 132, "y": 217})).unwrap(), (132, 217));
        assert_eq!(screen_point(&json!({"x": 131.6, "y": -4.2})).unwrap(), (132, -4));
    }

    #[test]
    fn a_result_without_a_position_is_an_error_naming_what_is_missing() {
        for value in [json!(null), json!({"x": 1}), json!({"x": "1", "y": 2}), json!([1, 2])] {
            let error = screen_point(&value).unwrap_err().to_string();
            assert!(error.contains("expected a number"), "{value}: {error}");
        }
    }

    /// Runs the position script in a page that pretends to be a headed window
    /// with an 85px toolbar and 8px side borders, and returns the result.
    async fn position_in_a_headed_window(
        page: &crate::sb_cdp::Page,
        css: &str,
        context: &str,
    ) -> serde_json::Value {
        page.evaluate(
            "Object.defineProperty(window, 'outerHeight', {configurable: true, value: window.innerHeight + 85 + 8}); \
             Object.defineProperty(window, 'outerWidth', {configurable: true, value: window.innerWidth + 16}); 1",
        )
        .await
        .unwrap();
        page.evaluate(format!(
            "{context}.eval({})",
            js_string_literal(&format!("(function () {{ {} }})()", element_center_script(css)))
        ))
        .await
        .unwrap()
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn the_pointer_target_includes_the_window_toolbar_and_borders() {
        let (browser, page) = open_html(
            "<!doctype html><body style='margin:0'>\
             <button id=b style='position:absolute;left:100px;top:100px;width:20px;height:20px'>x</button>",
        )
        .await;
        let (screen_x, screen_y): (i64, i64) =
            (page.evaluate_as("window.screenX").await.unwrap(), page.evaluate_as("window.screenY").await.unwrap());

        let point = position_in_a_headed_window(&page, "#b", "window").await;
        // Centre of the button is (110, 110) in the page. Side border 8px; the
        // toolbar is the 93px of outer height minus the 8px bottom border.
        assert_eq!(point["x"], screen_x + 8 + 110, "{point}");
        assert_eq!(point["y"], screen_y + 85 + 110, "{point}");
        browser.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn the_pointer_target_includes_the_frame_the_element_is_in() {
        let (browser, page) = open_html(
            "<!doctype html><body style='margin:0'>\
             <iframe id=f style='position:absolute;left:200px;top:50px;width:300px;height:200px;border:0' \
              srcdoc=\"<body style='margin:0'><button id='b' style='position:absolute;left:10px;top:20px;width:20px;height:20px'>x</button>\"></iframe>",
        )
        .await;
        page.wait_for_load().await.unwrap();
        page.wait_for_function("document.getElementById('f').contentDocument.getElementById('b')").await.unwrap();
        let (screen_x, screen_y): (i64, i64) =
            (page.evaluate_as("window.screenX").await.unwrap(), page.evaluate_as("window.screenY").await.unwrap());

        let point = position_in_a_headed_window(&page, "#b", "document.getElementById('f').contentWindow").await;
        // Button centre (20, 30) inside the frame, which sits at (200, 50).
        assert_eq!(point["x"], screen_x + 8 + 200 + 20, "{point}");
        assert_eq!(point["y"], screen_y + 85 + 50 + 30, "{point}");
        browser.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn a_missing_element_is_an_error_not_a_position() {
        let (browser, page) = open_html("<!doctype html><p>nothing</p>").await;
        let script = format!("(function () {{ {} }})()", element_center_script("#nope"));
        let error = page.evaluate(script).await.unwrap_err().to_string();
        assert!(error.contains("Element not found: #nope"), "{error}");
        browser.close().await.unwrap();
    }
}
