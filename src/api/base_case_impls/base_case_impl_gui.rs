// Native GUI automation helpers for `BaseCase`.
// Included at the end of `base_case.rs` so the code lives in the same module
// and can access private fields and helpers.

use crate::api::gui::Gui;

/// Escape a string for safe use inside a single-quoted JavaScript literal.
fn js_escape_single(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

impl BaseCase {
    /// Query the screen coordinates of the center of the element matching
    /// `css`. Returns `(x, y)` in absolute screen pixels.
    async fn element_center_screen_coords(&self, css: &str) -> Result<(i32, i32), SeleniumBaseError> {
        let escaped = js_escape_single(css);
        let script = format!(
            "var el = document.querySelector('{}'); \
             if (!el) throw new Error('Element not found: {}'); \
             var r = el.getBoundingClientRect(); \
             return {{x: Math.round(r.left + r.width/2 + window.screenX), y: Math.round(r.top + r.height/2 + window.screenY)}};",
            escaped, escaped
        );
        let value = self.session.execute_script(&script).await?;
        let obj = value.as_object().ok_or_else(|| {
            SeleniumBaseError::AssertionFailed("expected object from element rect script".to_owned())
        })?;
        let x = obj
            .get("x")
            .and_then(Value::as_i64)
            .ok_or_else(|| SeleniumBaseError::AssertionFailed("missing x coordinate".to_owned()))?;
        let y = obj
            .get("y")
            .and_then(Value::as_i64)
            .ok_or_else(|| SeleniumBaseError::AssertionFailed("missing y coordinate".to_owned()))?;
        Ok((x as i32, y as i32))
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
}
