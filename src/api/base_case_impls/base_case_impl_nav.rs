// Navigation helpers.

/// How long the AngularJS wait of a page-load hook lasts, in seconds (Python's
/// `MINI_TIMEOUT`).
const ANGULARJS_HOOK_WAIT_SECS: u64 = 2;


impl BaseCase {
    /// Opens the configured start page if one exists, otherwise `about:blank`.
    pub async fn goto_start_page(&mut self) -> Result<(), SeleniumBaseError> {
        self.open_start_page().await
    }

    /// Opens the configured start page if one exists, otherwise `about:blank`.
    pub async fn open_start_page(&mut self) -> Result<(), SeleniumBaseError> {
        let url = self.config.start_page.clone().unwrap_or_else(|| "about:blank".to_owned());
        self.open(&url).await
    }

    /// Waits for the page's AngularJS requests when the run settings ask for
    /// it (`--wfa`). An AngularJS request still in flight after two seconds
    /// does not fail the call: the page is used as it is, as in Python.
    async fn wait_for_angularjs_if_asked(&self) -> Result<(), SeleniumBaseError> {
        if !self.run_settings.wait_for_angularjs() {
            return Ok(());
        }
        match self.wait_for_angularjs(ANGULARJS_HOOK_WAIT_SECS).await {
            Err(SeleniumBaseError::WaitTimeout(_)) => Ok(()),
            other => other,
        }
    }

    /// The run settings' hooks for a page that has just loaded: the AngularJS
    /// wait (`--wfa`), the check of the browser log for JavaScript errors
    /// (`--check-js`) and the switch-off of the `beforeunload` dialog
    /// (`--disable-beforeunload`). The last two do nothing in a browser that
    /// is not Chromium-based, which has neither the log nor the dialog.
    /// Nothing runs when the run settings skip the JavaScript waits
    /// (`--skip-js-waits`).
    async fn run_page_load_hooks(&self) -> Result<(), SeleniumBaseError> {
        if self.run_settings.skip_js_waits() {
            return Ok(());
        }
        self.wait_for_angularjs_if_asked().await?;
        if self.run_settings.check_js() && self.is_chromium() {
            self.assert_no_js_errors().await?;
        }
        if self.run_settings.disable_beforeunload() && self.is_chromium() {
            self.disable_beforeunload().await?;
        }
        Ok(())
    }

}
