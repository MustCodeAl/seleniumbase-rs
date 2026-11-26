// Behavioral human-like interactions for `BaseCase`.
// Included in `base_case.rs`.

use crate::sb_cdp::human::humanize::{bezier_mouse_path, keystroke_delays, Point, Rng};

impl BaseCase {
    /// Clicks the element selected by `selector` using randomized curved
    /// Bézier pointer movement and human-like click timing.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if no element matches `selector`,
    /// or a CDP protocol error if the interaction commands fail.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::BaseCase;
    ///
    /// # async fn demo(mut sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.human_click("#submit-btn").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, selector))]
    pub async fn human_click(
        &mut self,
        selector: impl Into<Selector>,
    ) -> Result<(), SeleniumBaseError> {
        self.human_click_inner(&selector.into()).await
    }

    /// Inner implementation of `human_click`.
    async fn human_click_inner(&mut self, css: &Selector) -> Result<(), SeleniumBaseError> {
        let selector = shadow_find::checked(css)?;
        self.record_selector("human_click", css, None);
        self.await_ready(css, Ready::Clickable).await?;
        self.demo_highlight(css).await?;

        // Query element position on page for realistic pointer targeting
        let script = format!(
            r#"(function() {{
                var el = document.querySelector({0});
                if (!el) return null;
                if (el.scrollIntoViewIfNeeded) el.scrollIntoViewIfNeeded(true);
                else el.scrollIntoView({{ block: 'center', inline: 'center' }});
                var r = el.getBoundingClientRect();
                return [r.left, r.top, r.width, r.height];
            }})()"#,
            quote(css.as_str())
        );

        let rect = self.execute_script(&script).await.ok().and_then(|val| {
            let arr = val.as_array()?;
            if arr.len() == 4 {
                Some((arr[0].as_f64()?, arr[1].as_f64()?, arr[2].as_f64()?, arr[3].as_f64()?))
            } else {
                None
            }
        });

        let mut rng = Rng::from_entropy();

        if let Some((x, y, w, h)) = rect {
            // Target point near center with subtle human jitter within element bounds
            let pad_x = (w * 0.15).min(5.0);
            let pad_y = (h * 0.15).min(5.0);
            let target_x = x + w / 2.0 + rng.range(-pad_x, pad_x);
            let target_y = y + h / 2.0 + rng.range(-pad_y, pad_y);
            let target = Point::new(target_x, target_y);

            // Starting point from natural offset or resting place
            let start = Point::new(
                (target.x + rng.range(-150.0, 150.0)).max(0.0),
                (target.y + rng.range(-150.0, 150.0)).max(0.0),
            );

            let dist = ((target.x - start.x).powi(2) + (target.y - start.y).powi(2)).sqrt();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let steps = ((dist / 12.0).round() as u32).clamp(12, 30);
            let seed = rng.next_u64();
            let path = bezier_mouse_path(start, target, steps, seed);

            for pt in path {
                let _ = self.dispatch_mouse("mouseMoved", pt.x, pt.y, 0).await;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let step_ms = rng.range(8.0, 16.0).round() as u64;
                tokio::time::sleep(Duration::from_millis(step_ms)).await;
            }

            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let rest_ms = rng.range(40.0, 90.0).round() as u64;
            tokio::time::sleep(Duration::from_millis(rest_ms)).await;

            let _ = self.dispatch_mouse("mousePressed", target.x, target.y, 1).await;

            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let dwell_ms = rng.range(50.0, 110.0).round() as u64;
            tokio::time::sleep(Duration::from_millis(dwell_ms)).await;

            let _ = self.dispatch_mouse("mouseReleased", target.x, target.y, 0).await;
        }

        // Complete click via session element to ensure click listeners fire on all engines
        self.session.click(selector).await?;
        Ok(())
    }

    /// Types `text` character by character with human-like inter-keystroke intervals
    /// (e.g. 50-120ms with natural jitter).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::ElementNotFound`] if no element matches `selector`,
    /// or a WebDriver / CDP error if typing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::BaseCase;
    ///
    /// # async fn demo(mut sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.human_type("#username", "admin").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, selector, text))]
    pub async fn human_type(
        &mut self,
        selector: impl Into<Selector>,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.human_type_inner(&selector.into(), text).await
    }

    /// Inner implementation of `human_type`.
    async fn human_type_inner(
        &mut self,
        css: &Selector,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let selector = shadow_find::checked(css)?;
        self.record_selector("human_type", css, Some(text));
        let elem = self.session.find(selector).await?;
        elem.click().await?; // Focus the field

        let mut rng = Rng::from_entropy();
        let seed = rng.next_u64();
        let delays = keystroke_delays(text, 50, 120, seed);

        for (ch, delay_ms) in text.chars().zip(delays) {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            elem.send_keys(&ch.to_string()).await?;
        }

        Ok(())
    }
}
