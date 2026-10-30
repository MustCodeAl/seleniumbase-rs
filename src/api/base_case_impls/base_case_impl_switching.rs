// What the calls of `crate::api::switching` do: wait for a frame, a window or an
// alert, and switch to it.
// This file is included at the end of `base_case.rs` so it shares the same scope.

/// The CSS that selects the `<iframe>` or `<frame>` whose attribute `attribute`
/// (`id` or `name`) is `value`.
fn frame_with_attribute(attribute: &str, value: &str) -> String {
    let quoted = format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""));
    format!("iframe[{attribute}={quoted}], frame[{attribute}={quoted}]")
}

impl BaseCase {
    /// One try at entering `frame`: `true` if the driver is now in it.
    ///
    /// A frame given as text is looked for the way Python's
    /// `driver.switch_to.frame(text)` does, by `id`, then by `name`, and then
    /// as a selector; a selector needs a frame that is displayed unless
    /// `invisible`. Failing to find or to enter a frame is not an error: the
    /// frame may not be there yet.
    async fn try_enter_frame(
        &self,
        frame: &FrameRef,
        invisible: bool,
    ) -> Result<bool, SeleniumBaseError> {
        let driver = self.session.webdriver()?;
        let locator = match frame {
            FrameRef::Index(index) => return Ok(driver.enter_frame(*index).await.is_ok()),
            FrameRef::Locator(locator) => locator,
        };
        for attribute in ["id", "name"] {
            let lookup = By::Css(frame_with_attribute(attribute, locator));
            if let Ok(found) = driver.find(lookup).await {
                if found.enter_frame().await.is_ok() {
                    return Ok(true);
                }
            }
        }
        let Ok(by) = Selector::auto(locator).to_by() else {
            return Ok(false);
        };
        let Ok(found) = driver.find(by).await else {
            return Ok(false);
        };
        if !invisible && !found.is_displayed().await.unwrap_or(false) {
            return Ok(false);
        }
        Ok(found.enter_frame().await.is_ok())
    }

    /// The Python-shaped `switch_to_frame`: waits for `frame`, then enters it.
    pub(crate) async fn run_switch_to_frame(
        &mut self,
        frame: &FrameRef,
        timeout: Option<Duration>,
        invisible: bool,
    ) -> Result<(), SeleniumBaseError> {
        self.check_time_limit()?;
        let presence = if invisible { "present" } else { "visible" };
        let failure = format!("Frame {{{frame}}} was not {presence}");
        let this = &*self;
        poll_until_true(this.wait_secs(timeout), failure, || {
            this.try_enter_frame(frame, invisible)
        })
        .await?;
        // A position cannot be replayed from a text, so only a text is recorded.
        if let FrameRef::Locator(locator) = frame {
            self.record("switch_to_frame", Some(locator), None);
        }
        Ok(())
    }

    /// The Python-shaped `with frame_switch(frame):`: enters `frame`, runs
    /// `steps`, and leaves the frame again whether or not `steps` succeeded.
    pub(crate) async fn run_frame_switch<T>(
        &mut self,
        frame: &FrameRef,
        timeout: Option<Duration>,
        steps: impl AsyncFnOnce(&mut Self) -> Result<T, SeleniumBaseError>,
    ) -> Result<T, SeleniumBaseError> {
        self.run_switch_to_frame(frame, timeout, false).await?;
        let outcome = steps(self).await;
        if matches!(frame, FrameRef::Locator(_)) {
            self.record("switch_to_parent_frame", None, None);
        }
        let left = self.switch_to_parent_frame().await;
        // The steps' own error comes first: leaving a frame after a failed step
        // may fail for the same reason.
        let value = outcome?;
        left?;
        Ok(value)
    }

    /// The Python-shaped `switch_to_window`: waits for `window`, then switches
    /// to it.
    pub(crate) async fn run_switch_to_window(
        &self,
        window: &WindowRef,
        timeout: Option<Duration>,
    ) -> Result<(), SeleniumBaseError> {
        self.check_time_limit()?;
        let handle = poll_until_found(
            self.wait_secs(timeout),
            format!("Window {{{window}}} was not present"),
            || async move {
                let handles = self.get_all_window_handles().await?;
                Ok(match window.pick(&handles) {
                    Some(handle) => Polled::Found(handle),
                    None => Polled::Pending {
                        seen: Some(format!("{} windows open", handles.len())),
                    },
                })
            },
        )
        .await?;
        self.record("switch_to_window", Some(&handle), None);
        self.session.switch_to_window(&handle).await
    }

    /// The Python-shaped `switch_to_alert`: waits for an alert and returns a
    /// handle on it.
    pub(crate) async fn run_switch_to_alert(
        &self,
        timeout: Option<Duration>,
    ) -> Result<Alert, SeleniumBaseError> {
        self.check_time_limit()?;
        poll_until_true(
            self.wait_secs(timeout),
            "Alert was not present".to_owned(),
            || self.is_alert_present(),
        )
        .await?;
        Ok(Alert::new(
            self.session.webdriver()?.clone(),
            Arc::clone(&self.recorder),
        ))
    }
}
