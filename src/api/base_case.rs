#![allow(deprecated)]

use base64::prelude::*;
use rand::RngExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info, instrument};

use crate::api::chart::{Chart, ChartSeries, ChartType};
use crate::api::deferred::DeferredAsserts;
use crate::api::html::BeautifulSoup;
use crate::api::master_qa::{MasterQA, MasterQaSession};
use crate::api::page_scripts::{self, TextMatch};
use crate::api::pdf;
use crate::api::presentation::Presentation;
use crate::api::recorder::{ActionRecorder, RecordedAction, RecorderSession};
use crate::api::run_settings::RunSettings;
use crate::api::tour::{Tour, TourTheme};
use crate::artifacts::{artifact_path, ensure_latest_logs_dir};
use crate::browser::config::BrowserConfig;
#[cfg(feature = "playwright")]
use crate::browser::playwright::PlaywrightSession;
use crate::browser::session::BrowserSession;
use crate::error::SeleniumBaseError;
use crate::js_code::quote;
use crate::utils::selectors::{css_to_xpath, Selector};
use serde_json::Value;
use std::collections::HashMap;
use thirtyfour::common::keys::Key;
#[allow(deprecated)]
use thirtyfour::extensions::cdp::NetworkConditions;
use thirtyfour::prelude::By;

/// How long [`BaseCase::highlight`] leaves its outline on an element.
const HIGHLIGHT_MILLIS: u32 = 900;

pub struct BaseCase {
    session: BrowserSession,
    config: BrowserConfig,
    recorder: Arc<Mutex<ActionRecorder>>,
    /// Index into the recorder marking where the active recording window began.
    recording_start: Option<usize>,
    tour: Option<Tour>,
    deferred: DeferredAsserts,
    presentation: Option<Presentation>,
    chart: Option<Chart>,
    qa_session: Option<MasterQaSession>,
    #[cfg(feature = "playwright")]
    playwright_session: Option<PlaywrightSession>,
    /// How long the whole test may run, from [`BaseCase::set_time_limit`].
    time_limit: Option<Duration>,
    /// When this test case was created, which is where the time limit counts from.
    started: std::time::Instant,
    /// Default implicit wait timeout used by [`wait_for_element_visible_default`].
    timeout_secs: u64,
    /// The default timeout that [`BaseCase::set_default_timeout`] replaced, so
    /// [`BaseCase::reset_default_timeout`] can put it back.
    default_timeout_before: Option<u64>,
    #[cfg(feature = "gui")]
    gui_held: Option<(i32, i32)>,
    /// Additional browser sessions created by [`BaseCase::get_new_driver`].
    extra_sessions: Vec<BrowserSession>,
    /// Run-wide demo, slow, fast and similar settings.
    run_settings: RunSettings,
    /// The pages [`BaseCase::set_content_to_frame`] replaced, oldest first, for
    /// [`BaseCase::set_content_to_default`] to bring back.
    swapped_pages: Vec<SwappedPage>,
}

impl BaseCase {
    /// Creates a new test case and connects to the browser described by `config`.
    ///
    /// If the configuration names a start page (`start_page`, Python's
    /// `--start-page`), the browser is taken there before this returns.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser cannot be started or the start page
    /// cannot be opened; the browser is closed again in the second case.
    #[instrument(skip(config))]
    pub async fn new(config: BrowserConfig) -> Result<Self, SeleniumBaseError> {
        info!(browser = ?config.browser, headless = config.headless, "creating BaseCase");
        let session = BrowserSession::connect(config.clone()).await?;
        let mut case = Self::with_session(config, session);
        if case.config.start_page.is_some() {
            if let Err(error) = case.open_start_page().await {
                // Do not leave a browser running that nobody holds.
                let _ = case.quit().await;
                return Err(error);
            }
        }
        Ok(case)
    }

    /// Creates a `BaseCase` from an existing browser session. Used internally for
    /// reconnecting and for test-only construction.
    #[instrument(skip(config, session))]
    pub fn with_session(config: BrowserConfig, session: BrowserSession) -> Self {
        Self {
            session,
            config,
            recorder: Arc::new(Mutex::new(ActionRecorder::default())),
            recording_start: None,
            tour: None,
            deferred: DeferredAsserts::default(),
            presentation: None,
            chart: None,
            qa_session: None,
            #[cfg(feature = "playwright")]
            playwright_session: None,
            time_limit: None,
            started: std::time::Instant::now(),
            timeout_secs: 10,
            default_timeout_before: None,
            #[cfg(feature = "gui")]
            gui_held: None,
            extra_sessions: Vec::new(),
            run_settings: RunSettings::default(),
            swapped_pages: Vec::new(),
        }
    }

    /// The run-wide settings in force.
    pub fn run_settings(&self) -> &RunSettings {
        &self.run_settings
    }

    /// Replaces the run-wide settings: demo and slow mode, fast mode, how long
    /// messages last, whether the JavaScript waits are skipped, whether `sleep`
    /// is recorded and whether `check_window` resets its baseline.
    pub fn set_run_settings(&mut self, settings: RunSettings) {
        self.run_settings = settings;
    }

    /// Ends the test if ESC was pressed (`--esc-end`), then, in demo mode,
    /// outlines the element `css` picks, if the page has it.
    async fn demo_highlight(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.skip_if_esc().await?;
        if self.run_settings.demo_mode() {
            // A selector the outline script cannot use must not fail the action.
            let _ = self
                .execute_script(&page_scripts::highlight_script(css, HIGHLIGHT_MILLIS))
                .await;
        }
        Ok(())
    }

    /// Pauses for the time demo or slow mode asks for after an action.
    async fn demo_pause(&self, tiny: bool) {
        let pause = self.run_settings.pause(tiny);
        if !pause.is_zero() {
            tokio::time::sleep(pause).await;
        }
    }

    /// Creates a `BaseCase` without a live browser session. Useful for unit tests
    /// of helper methods that do not interact with the browser.
    pub fn without_session(config: BrowserConfig) -> Self {
        Self::with_session(config, BrowserSession::disconnected())
    }

    /// Sets the default implicit wait timeout used by helper macros and returns
    /// the previous value.
    ///
    /// The timeout is capped by the time left of the optional test time limit
    /// ([`set_time_limit`](Self::set_time_limit)).
    pub async fn set_timeout(&mut self, secs: u64) -> Result<u64, SeleniumBaseError> {
        let old = self.timeout_secs;
        self.timeout_secs = secs;
        Ok(old)
    }

    /// Returns the current default implicit wait timeout.
    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    /// Activates the optional Playwright-compatible stealth browser mode.
    ///
    /// This creates a fresh Chromium session through
    /// [`rustwright`](https://github.com/Skyvern-AI/rustwright) native Rust CDP
    /// engine and stores it in the test case. When this mode is active you can
    /// use [`playwright_session`](BaseCase::playwright_session) or the helpers
    /// in [`crate::api::playwright`] to interact with pages.
    ///
    /// This method is only available when the `playwright` feature is enabled.
    #[cfg(feature = "playwright")]
    pub async fn activate_playwright_mode(&mut self) -> Result<(), SeleniumBaseError> {
        self.playwright_session = Some(PlaywrightSession::launch().await?);
        Ok(())
    }

    /// Returns a mutable reference to the active Playwright session, if any.
    #[cfg(feature = "playwright")]
    pub fn playwright_session(&mut self) -> Option<&mut PlaywrightSession> {
        self.playwright_session.as_mut()
    }

    /// Asserts that `text` is visible inside the element selected by `css`.
    pub async fn assert_text_visible(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let is_visible = self.is_text_visible(text, css).await?;
        if !is_visible {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Text '{}' was not visible in element '{}'",
                text, css
            )));
        }
        Ok(())
    }

    /// Asserts that `text` is not visible inside the element selected by `css`.
    pub async fn assert_text_not_visible(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let is_visible = self.is_text_visible(text, css).await?;
        if is_visible {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Text '{}' was unexpectedly visible in element '{}'",
                text, css
            )));
        }
        Ok(())
    }

    /// Asserts that the attribute named `attribute` on `css` equals `value`.
    pub async fn assert_attribute(
        &mut self,
        css: &str,
        attribute: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let actual_value = self.get_attribute(css, attribute).await?;
        if actual_value.as_deref() != Some(value) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected attribute '{}' of '{}' to be '{}', but got '{}'",
                attribute,
                css,
                value,
                actual_value.unwrap_or_default()
            )));
        }
        Ok(())
    }

    /// Asserts that the current page title equals `expected`.
    pub async fn assert_title(&mut self, expected: &str) -> Result<(), SeleniumBaseError> {
        let title = self.get_title().await?;
        if title != expected {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Expected title to be '{}', but got '{}'",
                expected, title
            )));
        }
        Ok(())
    }

    /// Waits until `document.readyState` is `complete`, for up to the default
    /// wait: the timeout [`set_timeout`](Self::set_timeout) and
    /// [`set_default_timeout`](Self::set_default_timeout) set (10 seconds
    /// until either is called), cut to the time the test has left if it has a
    /// [time limit](Self::set_time_limit). Returns at once if the run settings
    /// skip the JavaScript waits.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::TestLifecycle`] if the test is already
    /// past its time limit, and [`SeleniumBaseError::WaitTimeout`] if the page
    /// is still loading when the wait ends.
    pub async fn wait_for_ready_state_complete(&self) -> Result<(), SeleniumBaseError> {
        self.skip_if_esc().await?;
        if self.run_settings.skip_js_waits() {
            self.arm_esc_end().await;
            return Ok(());
        }
        self.check_time_limit()?;
        poll_until_true(
            self.default_wait_secs(),
            "Page readyState did not become 'complete'".to_owned(),
            || async {
                let state = self.execute_script("return document.readyState;").await?;
                Ok(state.as_str() == Some("complete"))
            },
        )
        .await?;
        self.arm_esc_end().await;
        Ok(())
    }

    /// Returns the current browser window position as `(x, y)`.
    pub async fn get_window_position(&self) -> Result<(i64, i64), SeleniumBaseError> {
        let rect = self.session.driver().get_window_rect().await?;
        Ok((rect.x, rect.y))
    }

    /// Moves the browser window to `(x, y)`.
    pub async fn set_window_position(&self, x: u32, y: u32) -> Result<(), SeleniumBaseError> {
        let rect = self.session.driver().get_window_rect().await?;
        self.session
            .driver()
            .set_window_rect(
                x.into(),
                y.into(),
                rect.width.try_into().unwrap_or(0),
                rect.height.try_into().unwrap_or(0),
            )
            .await?;
        Ok(())
    }

    /// Closes the current browser window or tab.
    pub async fn close_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.session.driver().close_window().await?;
        Ok(())
    }

    /// Switches the context to the parent frame.
    pub async fn switch_to_parent_frame(&mut self) -> Result<(), SeleniumBaseError> {
        self.session.driver().enter_parent_frame().await?;
        Ok(())
    }

    /// Returns `true` if `css` is visible on the page.
    pub async fn is_element_visible(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Returns `true` if `text` is visible inside the element selected by `css`.
    pub async fn is_text_visible(&self, text: &str, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => {
                if !elem.is_displayed().await.unwrap_or(false) {
                    return Ok(false);
                }
                let elem_text = elem.text().await.unwrap_or_default();
                Ok(elem_text.contains(text))
            }
            Err(_) => Ok(false),
        }
    }

    /// Waits up to `timeout` seconds for the element `css` to become invisible.
    pub async fn wait_for_element_not_visible(
        &mut self,
        css: &str,
        timeout: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.check_time_limit()?;
        self.record("wait_for_element_not_visible", Some(css), None);
        let by = Selector::auto(css).to_by()?;
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(self.effective_timeout(timeout));
        loop {
            if start.elapsed() > timeout_dur {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Element {} did not become invisible within {} seconds",
                    css, timeout
                )));
            }
            match self.session.driver().find(by.clone()).await {
                Ok(elem) => {
                    if !elem.is_displayed().await.unwrap_or(true) {
                        return Ok(());
                    }
                }
                Err(_) => return Ok(()), // Not present means not visible
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Saves all current cookies to `file_path` as JSON.
    pub async fn save_cookies(&self, file_path: &str) -> Result<(), SeleniumBaseError> {
        let cookies = self.session.driver().get_all_cookies().await?;
        let json = serde_json::to_string_pretty(&cookies).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("Failed to serialize cookies: {}", e))
        })?;
        std::fs::write(file_path, json).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("Failed to write cookies: {}", e))
        })?;
        Ok(())
    }

    /// Loads cookies from `file_path` (JSON) and adds them to the browser.
    pub async fn load_cookies(&self, file_path: &str) -> Result<(), SeleniumBaseError> {
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("Failed to read cookies: {}", e))
        })?;
        let cookies: Vec<thirtyfour::cookie::Cookie> =
            serde_json::from_str(&content).map_err(|e| {
                SeleniumBaseError::InvalidConfig(format!("Failed to deserialize cookies: {}", e))
            })?;
        for cookie in cookies {
            self.session.driver().add_cookie(cookie).await?;
        }
        Ok(())
    }

    /// Highlights the element `css`, then clicks it.
    pub async fn highlight_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.highlight(css).await?;
        self.click(css).await
    }

    /// Executes the `is_checked` action.
    pub async fn is_checked(&mut self, css: &str) -> Result<bool, SeleniumBaseError> {
        let is_checked = self.get_property(css, "checked").await?;
        Ok(is_checked.as_deref() == Some("true"))
    }

    /// Executes the `check_if_unchecked` action.
    pub async fn check_if_unchecked(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_checked(css).await? {
            self.click(css).await?;
        }
        Ok(())
    }

    /// Executes the `uncheck_if_checked` action.
    pub async fn uncheck_if_checked(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_checked(css).await? {
            self.click(css).await?;
        }
        Ok(())
    }

    /// Executes the `open_new_window` action.
    ///
    /// With browser plugins (feature `plugins`), the new window gets their
    /// packages and `on_page_created`, as the first window did.
    pub async fn open_new_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.execute_script("window.open('');").await?;
        self.switch_to_newest_window().await?;
        #[cfg(feature = "plugins")]
        self.session.apply_plugins_to_window().await?;
        Ok(())
    }

    /// Executes the `open_new_tab` action.
    pub async fn open_new_tab(&mut self) -> Result<(), SeleniumBaseError> {
        self.open_new_window().await // Essentially the same in modern browsers
    }

    /// Opens a new tab, switches to it, and navigates to `url`.
    ///
    /// This is the SeleniumBase `open_new_tab(url)` form. As with
    /// [`open`][Self::open], a URL without a scheme is opened over HTTPS.
    ///
    /// # Errors
    ///
    /// Returns an error if the tab cannot be opened, or if `url` is empty or
    /// cannot be made into a valid URL.
    pub async fn open_new_tab_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        let url = crate::utils::urls::normalize_page_url(url)?;
        self.record("open_new_tab", Some(&url), None);
        self.open_new_tab().await?;
        self.switch_to_newest_window().await?;
        self.session.goto(&url).await
    }

    /// Executes the `switch_to_default_window` action.
    pub async fn switch_to_default_window(&mut self) -> Result<(), SeleniumBaseError> {
        let handles = self.session.driver().windows().await?;
        if let Some(first) = handles.first() {
            self.session
                .driver()
                .switch_to_window(first.clone())
                .await?;
        }
        Ok(())
    }

    /// Executes the `switch_to_newest_window` action.
    pub async fn switch_to_newest_window(&mut self) -> Result<(), SeleniumBaseError> {
        let handles = self.session.driver().windows().await?;
        if let Some(last) = handles.last() {
            self.session.driver().switch_to_window(last.clone()).await?;
        }
        Ok(())
    }

    /// Gives the window the session is on the packages and `on_page_created`
    /// of its browser plugins (feature `plugins`,
    /// [`BrowserConfig::with_plugins`]).
    ///
    /// `BaseCase::new` does this for the first window, and
    /// [`open_new_window`](Self::open_new_window), `open_new_tab` and
    /// [`switch_to_new_window`](Self::switch_to_new_window) for the windows
    /// they open. Call it after switching to a window the page opened, such
    /// as a pop-up: switching alone runs no hook. It does nothing without
    /// plugins.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Plugin`] for the first package or hook that
    /// fails.
    #[cfg(feature = "plugins")]
    pub async fn apply_plugins_to_window(&self) -> Result<(), SeleniumBaseError> {
        self.session.apply_plugins_to_window().await
    }

    /// Executes the `get_active_element_css` action.
    pub async fn get_active_element_css(&self) -> Result<String, SeleniumBaseError> {
        let script = r#"
            let el = document.activeElement;
            if (!el) return '';
            let path = [];
            while (el.nodeType === Node.ELEMENT_NODE) {
                let selector = el.nodeName.toLowerCase();
                if (el.id) {
                    selector += '#' + el.id;
                    path.unshift(selector);
                    break;
                } else {
                    let sib = el, nth = 1;
                    while (sib = sib.previousElementSibling) {
                        if (sib.nodeName.toLowerCase() == selector)
                           nth++;
                    }
                    if (nth != 1) selector += ":nth-of-type("+nth+")";
                }
                path.unshift(selector);
                el = el.parentNode;
            }
            return path.join(' > ');
        "#;
        match self.execute_script(script).await? {
            Value::String(s) => Ok(s),
            _ => Ok(String::new()),
        }
    }

    /// Waits up to `timeout` seconds for `css` to appear in the DOM.
    ///
    /// The element is looked for at least once, so an element that is already
    /// there is found even with a `timeout` of zero.
    pub async fn wait_for_element_present(
        &mut self,
        css: &str,
        timeout: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(self.effective_timeout(timeout));
        loop {
            if self.session.driver().find(by.clone()).await.is_ok() {
                return Ok(());
            }
            if start.elapsed() > timeout_dur {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Element {} not present after {} seconds",
                    css, timeout
                )));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[instrument(skip(self))]
    /// Navigates to `url`.
    ///
    /// A URL without a scheme is opened over HTTPS, so `"seleniumbase.io"` and
    /// `"https://seleniumbase.io"` are equivalent.
    ///
    /// # Errors
    ///
    /// Returns an error if `url` is empty or cannot be made into a valid URL.
    pub async fn open(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.check_time_limit()?;
        self.skip_if_esc().await?;
        let url = crate::utils::urls::normalize_page_url(url)?;
        info!(%url, "opening url");
        self.record("open", Some(&url), None);
        self.session.goto(&url).await?;
        self.demo_pause(false).await;
        self.arm_esc_end().await;
        Ok(())
    }

    /// Executes the `refresh` action.
    pub async fn refresh(&self) -> Result<(), SeleniumBaseError> {
        self.session.refresh().await?;
        self.arm_esc_end().await;
        Ok(())
    }

    /// Navigates back in browser history.
    pub async fn go_back(&self) -> Result<(), SeleniumBaseError> {
        self.session.back().await?;
        self.arm_esc_end().await;
        Ok(())
    }

    /// Navigates forward in browser history.
    pub async fn go_forward(&self) -> Result<(), SeleniumBaseError> {
        self.session.forward().await?;
        self.arm_esc_end().await;
        Ok(())
    }

    /// Clicks the element selected by `css`.
    #[instrument(skip(self))]
    pub async fn click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        debug!(%css, "clicking element");
        self.skip_if_esc().await?;
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Clickable).await?;
        self.record("click", Some(css), None);
        self.demo_highlight(css).await?;
        self.session.click(by).await?;
        self.demo_pause(false).await;
        self.arm_esc_end().await;
        Ok(())
    }

    /// Executes the `type_text` action.
    #[instrument(skip(self, text))]
    pub async fn type_text(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        debug!(%css, text_len = text.len(), "typing text");
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Visible).await?;
        self.record("type_text", Some(css), Some(text));
        self.demo_highlight(css).await?;
        self.session.type_text(by, text).await?;
        self.demo_pause(true).await;
        Ok(())
    }

    /// Clears the value of the element selected by `css`.
    pub async fn clear(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Visible).await?;
        self.record("clear", Some(css), None);
        self.session.clear(by).await
    }

    /// Executes the `click_link_text` action.
    pub async fn click_link_text(&mut self, link_text: &str) -> Result<(), SeleniumBaseError> {
        self.skip_if_esc().await?;
        let by = thirtyfour::prelude::By::LinkText(link_text.to_owned());
        self.record("click_link_text", Some(link_text), None);
        self.session.click(by).await?;
        self.arm_esc_end().await;
        Ok(())
    }

    /// Submits the form containing the element selected by `css`.
    pub async fn submit(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Present).await?;
        self.record("submit", Some(css), None);
        self.session.submit(by).await
    }

    /// Returns the visible text of the element `css`.
    pub async fn get_text(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Present).await?;
        self.session.text(by).await
    }

    /// Hovers over the element selected by `css`.
    pub async fn hover(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Visible).await?;
        self.record("hover", Some(css), None);
        self.session.hover(by).await
    }

    /// Executes the `hover_and_click` action.
    pub async fn hover_and_click(
        &mut self,
        hover_css: &str,
        click_css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.hover(hover_css).await?;
        self.sleep(0.1).await;
        self.click(click_css).await
    }

    /// Selects the `<option>` with visible text `text`.
    pub async fn select_option_by_text(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Present).await?;
        self.record("select_option_by_text", Some(css), Some(text));
        self.session.select_option_by_text(by, text).await
    }

    /// Selects the `<option>` whose `value` matches.
    pub async fn select_option_by_value(
        &mut self,
        css: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Present).await?;
        self.record("select_option_by_value", Some(css), Some(value));
        self.session.select_option_by_value(by, value).await
    }

    /// Switches context into the frame selected by `css`.
    pub async fn switch_to_frame(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.record("switch_to_frame", Some(css), None);
        self.session.switch_to_frame(by).await
    }

    /// Switches context back to the top-level document.
    pub async fn switch_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        self.record("switch_to_default_content", None, None);
        self.session.switch_to_default_content().await
    }

    /// Drags the source element onto the target element.
    pub async fn drag_and_drop(
        &mut self,
        source_css: &str,
        target_css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let source_by = Selector::auto(source_css).to_by()?;
        let target_by = Selector::auto(target_css).to_by()?;
        self.await_ready(source_css, Ready::Visible).await?;
        self.await_ready(target_css, Ready::Visible).await?;
        self.record("drag_and_drop", Some(source_css), Some(target_css));
        self.session.drag_and_drop(source_by, target_by).await
    }

    /// Returns `true` if `css` exists in the DOM.
    pub async fn is_element_present(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.session.element_present(by).await
    }

    /// Executes the `assert_element` action.
    pub async fn assert_element(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_element_present(css).await? {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected element '{css}' to be present"
        )))
    }

    /// Executes the `assert_element_absent` action.
    pub async fn assert_element_absent(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_element_present(css).await? {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected element '{css}' to be absent"
        )))
    }

    /// Returns the current page title.
    pub async fn get_title(&mut self) -> Result<String, SeleniumBaseError> {
        self.session.current_title().await
    }

    /// Returns the current page URL.
    pub async fn get_current_url(&mut self) -> Result<String, SeleniumBaseError> {
        self.session.current_url().await
    }

    /// Executes the `assert_url_contains` action.
    pub async fn assert_url_contains(&mut self, expected: &str) -> Result<(), SeleniumBaseError> {
        let url = self.get_current_url().await?;
        if url.contains(expected) {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected URL to contain '{expected}', got '{url}'"
        )))
    }

    /// Executes the `get_page_source` action.
    pub async fn get_page_source(&self) -> Result<String, SeleniumBaseError> {
        self.session.page_source().await
    }

    /// Executes arbitrary JavaScript and returns the result.
    pub async fn execute_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.session.execute_script(script).await
    }

    /// Activates CDP domains on the current session.
    pub async fn activate_cdp_mode(&self) -> Result<(), SeleniumBaseError> {
        self.session.activate_cdp_mode().await
    }

    /// Reconnects the underlying WebDriver session (useful in UC mode for a clean session).
    pub async fn reconnect(&mut self) -> Result<(), SeleniumBaseError> {
        self.session.reconnect(&self.config).await
    }

    /// Executes a CDP command without parameters.
    pub async fn execute_cdp(&self, method: &str) -> Result<Value, SeleniumBaseError> {
        self.session.execute_cdp(method).await
    }

    /// Executes a CDP command with the given JSON parameters.
    pub async fn execute_cdp_with_params(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, SeleniumBaseError> {
        self.session.execute_cdp_with_params(method, params).await
    }

    /// Clears the browser cache via CDP.
    pub async fn clear_browser_cache(&self) -> Result<(), SeleniumBaseError> {
        self.session.clear_browser_cache().await
    }

    /// Clears all browser cookies via CDP.
    pub async fn clear_browser_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.session.clear_browser_cookies().await
    }

    /// Returns all cookies via CDP as JSON.
    pub async fn get_cookies(&self) -> Result<Value, SeleniumBaseError> {
        self.session.get_cookies().await
    }

    /// Dispatches a CDP mouse click at screen coordinates `(x, y)`.
    pub async fn cdp_mouse_click(&self, x: f64, y: f64) -> Result<(), SeleniumBaseError> {
        self.session.cdp_mouse_click(x, y).await
    }

    /// Inserts `text` via CDP without using element focus.
    pub async fn cdp_type_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.session.cdp_type_text(text).await
    }

    /// Finds `css` and dispatches a CDP mouse click at its center.
    pub async fn cdp_click_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("cdp_click_element", Some(css), None);
        let by = Selector::auto(css).to_by()?;
        let element = self.session.find(by).await?;
        let rect = element.rect().await?;
        let center_x = rect.x + (rect.width / 2.0);
        let center_y = rect.y + (rect.height / 2.0);
        self.session.cdp_mouse_click(center_x, center_y).await
    }

    /// Configures network throttling/latency via CDP.
    pub async fn set_network_conditions(
        &self,
        conditions: &NetworkConditions,
    ) -> Result<(), SeleniumBaseError> {
        self.session.set_network_conditions(conditions).await
    }

    /// Sets the browser timezone via CDP (e.g. "America/New_York").
    pub async fn set_timezone(&self, timezone_id: &str) -> Result<(), SeleniumBaseError> {
        self.session.set_timezone(timezone_id).await
    }

    /// Sets the browser geolocation via CDP.
    pub async fn set_geolocation(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> Result<(), SeleniumBaseError> {
        self.session
            .set_geolocation(latitude, longitude, accuracy)
            .await
    }

    /// Alias for `wait_for_element_present`.
    pub async fn wait_for_element(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.session.wait_for_element(by, timeout_secs).await?;
        Ok(())
    }

    /// Returns the value of `attribute` on the element `css`, if present.
    pub async fn get_attribute(
        &mut self,
        css: &str,
        attribute_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Present).await?;
        self.session.get_attribute(by, attribute_name).await
    }

    /// Executes the `get_property` action.
    pub async fn get_property(
        &mut self,
        css: &str,
        property_name: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.session.get_property(by, property_name).await
    }

    /// Waits up to `timeout` seconds for `css` to become visible.
    pub async fn wait_for_element_visible(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.skip_if_esc().await?;
        let by = Selector::auto(css).to_by()?;
        self.record("wait_for_element_visible", Some(css), None);
        self.session
            .wait_for_element_visible(by, timeout_secs)
            .await?;
        Ok(())
    }

    /// Waits up to the configured default timeout for `css` to become visible.
    ///
    /// Use [`set_timeout`](BaseCase::set_timeout) or the [`sb_with_timeout!`]
    /// macro to adjust the default.
    pub async fn wait_for_element_visible_default(
        &self,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_element_visible(css, self.default_wait_secs())
            .await
    }

    /// Waits up to `timeout` seconds for `css` to be removed from the DOM.
    pub async fn wait_for_element_absent(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.record("wait_for_element_absent", Some(css), None);
        self.session.wait_for_element_absent(by, timeout_secs).await
    }

    /// Executes the `wait_for_text` action.
    pub async fn wait_for_text(
        &self,
        css: &str,
        expected_substring: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.session
            .wait_for_text(by, expected_substring, timeout_secs)
            .await
    }

    /// Executes the `assert_title_contains` action.
    pub async fn assert_title_contains(&mut self, expected: &str) -> Result<(), SeleniumBaseError> {
        let title = self.get_title().await?;
        if title.contains(expected) {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected title to contain '{expected}', got '{title}'"
        )))
    }

    /// Asserts that the element selected by `css` contains `expected`.
    ///
    /// The text comes first and the selector second, as in Python's
    /// `assert_text(text, selector)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] when the text is not
    /// there. If the two arguments look swapped (`expected` selects an element
    /// whose text contains `css`), the message says so.
    pub async fn assert_text(
        &mut self,
        expected: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("assert_text", Some(css), Some(expected));
        let by = Selector::auto(css).to_by()?;
        match self.session.text(by).await {
            Ok(text) if text.contains(expected) => Ok(()),
            Ok(text) => Err(self
                .text_assertion_failed(
                    format!("expected text '{expected}' in element '{css}', got '{text}'"),
                    expected,
                    css,
                    false,
                )
                .await),
            Err(error) => match self.swapped_text_hint(expected, css, false).await {
                Some(hint) => Err(SeleniumBaseError::AssertionFailed(hint)),
                None => Err(error),
            },
        }
    }

    /// Asserts that the text of the element selected by `css` is exactly
    /// `expected`.
    ///
    /// The text comes first and the selector second, as in Python's
    /// `assert_exact_text(text, selector)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] when the text differs;
    /// the message says so if the two arguments look swapped.
    pub async fn assert_exact_text(
        &mut self,
        expected: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        match self.get_text(css).await {
            Ok(text) if text == expected => Ok(()),
            Ok(text) => Err(self
                .text_assertion_failed(
                    format!("expected exact text '{expected}' in element '{css}', got '{text}'"),
                    expected,
                    css,
                    true,
                )
                .await),
            Err(error) => match self.swapped_text_hint(expected, css, true).await {
                Some(hint) => Err(SeleniumBaseError::AssertionFailed(hint)),
                None => Err(error),
            },
        }
    }

    /// A failed text assertion, with the hint about swapped arguments added
    /// when it applies.
    async fn text_assertion_failed(
        &self,
        message: String,
        expected: &str,
        css: &str,
        exact: bool,
    ) -> SeleniumBaseError {
        match self.swapped_text_hint(expected, css, exact).await {
            Some(hint) => SeleniumBaseError::AssertionFailed(format!("{message}; {hint}")),
            None => SeleniumBaseError::AssertionFailed(message),
        }
    }

    /// Says that a text assertion was called with its arguments the wrong way
    /// round, if it was: `expected` selects an element whose text is `css`.
    /// Looked at only after an assertion failed, so a passing one costs
    /// nothing.
    async fn swapped_text_hint(&self, expected: &str, css: &str, exact: bool) -> Option<String> {
        let by = Selector::auto(expected).to_by().ok()?;
        let shown = self
            .session
            .driver()
            .find(by)
            .await
            .ok()?
            .text()
            .await
            .ok()?;
        let fits = if exact {
            shown.trim() == css.trim()
        } else {
            shown.contains(css)
        };
        fits.then(|| {
            format!(
                "the arguments look swapped: the order is (text, selector), as in Python, \
                 and '{expected}' selects an element whose text is '{css}'"
            )
        })
    }

    /// Scrolls the element `css` into view and outlines it in magenta for
    /// about a second, then puts back the outline it had. Does nothing if the
    /// page has no such element. Corresponds to Python's `highlight`, which
    /// animates the element and restores its style too; this is one outline
    /// that is restored by a timer in the page, so the call does not block.
    pub async fn highlight(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.skip_if_esc().await?;
        self.execute_script(&page_scripts::highlight_script(css, HIGHLIGHT_MILLIS))
            .await?;
        Ok(())
    }

    /// Displays a transient message overlay on the page for `duration_secs`.
    pub async fn post_message_for(
        &self,
        message: &str,
        duration_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        let escaped = quote(message);
        let script = format!(
            "(() => {{
                const id='sb-rs-msg';
                let box=document.getElementById(id);
                if(!box) {{
                  box=document.createElement('div');
                  box.id=id;
                  box.style.position='fixed';
                  box.style.top='16px';
                  box.style.right='16px';
                  box.style.zIndex='2147483647';
                  box.style.background='rgba(20,20,20,0.92)';
                  box.style.color='#fff';
                  box.style.padding='10px 14px';
                  box.style.borderRadius='8px';
                  box.style.fontFamily='Arial,sans-serif';
                  box.style.fontSize='14px';
                  document.body.appendChild(box);
                }}
                box.textContent={msg};
                setTimeout(() => {{ if (box && box.parentNode) box.parentNode.removeChild(box); }}, {ms});
            }})();",
            msg = escaped,
            ms = duration_secs.saturating_mul(1000),
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Captures a screenshot and writes it to `path`.
    pub async fn save_screenshot_to_path<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), SeleniumBaseError> {
        self.session.screenshot(path.as_ref()).await
    }

    /// Returns the current screenshot as PNG bytes.
    pub async fn screenshot_as_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        self.session.screenshot_as_png().await
    }

    /// Saves the current page source to `path`.
    async fn save_page_source_to_path<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), SeleniumBaseError> {
        let html = self.get_page_source().await?;
        std::fs::write(path.as_ref(), html).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to write page source '{}': {e}",
                path.as_ref().display()
            ))
        })?;
        Ok(())
    }

    /// Executes the `save_screenshot_to_logs` action.
    pub async fn save_screenshot_to_logs(&self) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = artifact_path(&dir, "screenshot", "png");
        self.save_screenshot_to_path(&path).await?;
        Ok(path)
    }

    /// Executes the `save_page_source_to_logs` action.
    pub async fn save_page_source_to_logs(&self) -> Result<PathBuf, SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let path = artifact_path(&dir, "page_source", "html");
        self.save_page_source_to_path(&path).await?;
        Ok(path)
    }

    /// Executes the `recorded_actions` action.
    pub fn recorded_actions(&self) -> Result<Vec<RecordedAction>, SeleniumBaseError> {
        let recorder = self
            .recorder
            .lock()
            .map_err(|_| SeleniumBaseError::Unsupported("recorder mutex poisoned".to_owned()))?;
        Ok(recorder.actions.clone())
    }

    /// Starts a new recording window.
    ///
    /// Actions performed after this call are captured by
    /// [`stop_recording`](Self::stop_recording). Calling it again resets the
    /// window to the current point in the action log.
    ///
    /// ```no_run
    /// # use seleniumbase_rs::{BaseCase, BrowserConfig};
    /// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    /// sb.start_recording();
    /// sb.open("https://seleniumbase.io/demo_page").await?;
    /// let actions = sb.stop_recording()?;
    /// assert_eq!(actions.len(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn start_recording(&mut self) {
        let start = self
            .recorder
            .lock()
            .map(|recorder| recorder.actions.len())
            .unwrap_or(0);
        self.recording_start = Some(start);
    }

    /// Returns `true` when [`start_recording`](Self::start_recording) is active.
    pub fn is_recording(&self) -> bool {
        self.recording_start.is_some()
    }

    /// Returns the actions captured since [`start_recording`](Self::start_recording).
    ///
    /// When recording was never started, every recorded action is returned.
    pub fn stop_recording(&self) -> Result<Vec<RecordedAction>, SeleniumBaseError> {
        let actions = self.recorded_actions()?;
        let start = self.recording_start.unwrap_or(0).min(actions.len());
        Ok(actions[start..].to_vec())
    }

    /// Returns the current recording window as a [`RecorderSession`], ready to
    /// be serialized to JSON/YAML or replayed.
    pub fn recorder_session(&self) -> Result<RecorderSession, SeleniumBaseError> {
        Ok(RecorderSession::from_actions(self.stop_recording()?))
    }

    /// Replays a list of recorded actions against this test case.
    pub async fn replay_actions(
        &mut self,
        actions: &[RecordedAction],
    ) -> Result<crate::api::recorder::ReplayReport, SeleniumBaseError> {
        RecorderSession::replay(self, actions).await
    }

    /// Executes the `export_recording_as_rust` action: the recording as a
    /// standalone Rust program that calls `BaseCase`. When
    /// [`rec_sb_cdp`](Self::rec_sb_cdp) is on (`--rec-sb-cdp`) the program
    /// calls the pure CDP engine instead, as
    /// [`export_recording_as_cdp_rust`](Self::export_recording_as_cdp_rust)
    /// writes it.
    pub fn export_recording_as_rust(&self) -> Result<String, SeleniumBaseError> {
        if self.rec_sb_cdp() {
            return self.export_recording_as_cdp_rust();
        }
        let recorder = self
            .recorder
            .lock()
            .map_err(|_| SeleniumBaseError::Unsupported("recorder mutex poisoned".to_owned()))?;
        Ok(recorder.to_rust_script())
    }

    /// Executes the `save_recording_to_logs` action.
    pub fn save_recording_to_logs(&self) -> Result<(PathBuf, PathBuf), SeleniumBaseError> {
        let dir = ensure_latest_logs_dir()?;
        let json_path = artifact_path(&dir, "recording", "json");
        let rust_path = artifact_path(&dir, "recording", "rs");

        let actions = self.recorded_actions()?;
        let json_data = serde_json::to_string_pretty(&actions).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to serialize recording json: {e}"))
        })?;
        std::fs::write(&json_path, json_data).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to write recording json: {e}"))
        })?;

        let rust_script = self.export_recording_as_rust()?;
        std::fs::write(&rust_path, rust_script).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to write recording rust file: {e}"))
        })?;

        Ok((json_path, rust_path))
    }

    /// Pauses execution for `seconds`.
    ///
    /// With [`RunSettings::with_record_sleep`] on, the pause goes into the
    /// recording as a `sleep` action.
    pub async fn sleep(&self, seconds: f64) {
        if self.run_settings.record_sleep() {
            self.record("sleep", None, Some(&seconds.to_string()));
        }
        let millis = if seconds <= 0.0 {
            0_u64
        } else {
            (seconds * 1000.0) as u64
        };
        tokio::time::sleep(Duration::from_millis(millis)).await;
    }

    /// Executes the `execute_async_script` action.
    pub async fn execute_async_script(&self, script: &str) -> Result<Value, SeleniumBaseError> {
        self.session.execute_async_script(script).await
    }

    /// Maximizes the browser window.
    pub async fn maximize_window(&self) -> Result<(), SeleniumBaseError> {
        self.record("maximize_window", None, None);
        self.session.maximize_window().await
    }

    /// Resizes the browser window to `width` x `height`.
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<(), SeleniumBaseError> {
        self.record(
            "set_window_size",
            Some(&format!("{},{}", width, height)),
            None,
        );
        self.session.set_window_size(width, height).await
    }

    /// Executes the `get_window_size` action.
    pub async fn get_window_size(&self) -> Result<(u32, u32), SeleniumBaseError> {
        self.session.get_window_size().await
    }

    /// Executes the `switch_to_window` action.
    pub async fn switch_to_window(&self, handle: &str) -> Result<(), SeleniumBaseError> {
        self.record("switch_to_window", Some(handle), None);
        self.session.switch_to_window(handle).await
    }

    /// Double-clicks the element selected by `css`.
    pub async fn double_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Clickable).await?;
        self.record("double_click", Some(css), None);
        self.session.double_click(by).await
    }

    /// Right-clicks the element selected by `css`.
    pub async fn context_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.await_ready(css, Ready::Clickable).await?;
        self.record("context_click", Some(css), None);
        self.session.context_click(by).await
    }

    /// Executes the `is_enabled` action.
    pub async fn is_enabled(&mut self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.session.is_enabled(by).await
    }

    /// Executes the `is_selected` action.
    pub async fn is_selected(&mut self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.session.is_selected(by).await
    }

    /// Executes the `is_displayed` action.
    pub async fn is_displayed(&mut self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.session.is_displayed(by).await
    }

    /// Executes the `accept_alert` action.
    pub async fn accept_alert(&self) -> Result<(), SeleniumBaseError> {
        self.record("accept_alert", None, None);
        self.session.switch_to_alert_accept().await
    }

    /// Executes the `dismiss_alert` action.
    pub async fn dismiss_alert(&self) -> Result<(), SeleniumBaseError> {
        self.record("dismiss_alert", None, None);
        self.session.switch_to_alert_dismiss().await
    }

    /// Executes the `get_alert_text` action.
    pub async fn get_alert_text(&self) -> Result<String, SeleniumBaseError> {
        self.session.get_alert_text().await
    }

    /// Executes the `type_alert_text` action.
    pub async fn type_alert_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("type_alert_text", Some(text), None);
        self.session.type_alert_text(text).await
    }

    /// Executes the `set_local_storage_item` action.
    pub async fn set_local_storage_item(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "window.localStorage.setItem({}, {});",
            quote(key),
            quote(value)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `get_local_storage_item` action.
    pub async fn get_local_storage_item(&self, key: &str) -> Result<Value, SeleniumBaseError> {
        let script = format!("return window.localStorage.getItem({});", quote(key));
        self.execute_script(&script).await
    }

    /// Executes the `remove_local_storage_item` action.
    pub async fn remove_local_storage_item(&self, key: &str) -> Result<(), SeleniumBaseError> {
        let script = format!("window.localStorage.removeItem({});", quote(key));
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `clear_local_storage` action.
    pub async fn clear_local_storage(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("window.localStorage.clear();").await?;
        Ok(())
    }

    /// Executes the `scroll_to_bottom` action.
    pub async fn scroll_to_bottom(&self) -> Result<(), SeleniumBaseError> {
        self.record("scroll_to_bottom", None, None);
        self.execute_script("window.scrollTo(0, document.body.scrollHeight);")
            .await?;
        Ok(())
    }

    /// Executes the `scroll_to_top` action.
    pub async fn scroll_to_top(&self) -> Result<(), SeleniumBaseError> {
        self.record("scroll_to_top", None, None);
        self.execute_script("window.scrollTo(0, 0);").await?;
        Ok(())
    }

    /// Executes the `scroll_to` action.
    pub async fn scroll_to(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("scroll_to", Some(css), None);
        let script = format!("document.querySelector({}).scrollIntoView();", quote(css));
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `assert_element_visible` action.
    pub async fn assert_element_visible(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_displayed(css).await? {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected element '{}' to be visible",
            css
        )))
    }

    /// Executes the `assert_element_not_visible` action.
    pub async fn assert_element_not_visible(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let is_visible = self.is_displayed(css).await.unwrap_or_default();
        if !is_visible {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected element '{}' to not be visible",
            css
        )))
    }

    /// Executes the `click_if_visible` action.
    pub async fn click_if_visible(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_displayed(css).await.unwrap_or(false) {
            self.click(css).await?;
        }
        Ok(())
    }

    /// Waits up to `timeout` seconds for `css` to become clickable.
    pub async fn wait_for_element_clickable(
        &mut self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.record("wait_for_element_clickable", Some(css), None);
        let by = Selector::auto(css).to_by()?;
        self.session
            .wait_for_element_clickable(by, timeout_secs)
            .await?;
        Ok(())
    }

    /// Returns the shadow root of the element `css`.
    pub async fn get_shadow_root(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.record("get_shadow_root", Some(css), None);
        let by = Selector::auto(css).to_by()?;
        let element = self.session.wait_for_element(by, 10).await?;
        element
            .get_shadow_root()
            .await
            .map_err(SeleniumBaseError::WebDriver)
    }

    /// Executes the `add_cookie` action.
    pub async fn add_cookie(&self, name: &str, value: &str) -> Result<(), SeleniumBaseError> {
        self.record("add_cookie", Some(name), Some(value));
        self.session.add_cookie(name, value).await
    }

    /// Executes the `get_cookie` action.
    pub async fn get_cookie(
        &self,
        name: &str,
    ) -> Result<thirtyfour::cookie::Cookie, SeleniumBaseError> {
        self.session.get_cookie(name).await
    }

    /// Executes the `delete_cookie` action.
    pub async fn delete_cookie(&self, name: &str) -> Result<(), SeleniumBaseError> {
        self.record("delete_cookie", Some(name), None);
        self.session.delete_cookie(name).await
    }

    /// Executes the `js_click` action.
    pub async fn js_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("js_click", Some(css), None);
        let script = format!("document.querySelector({}).click();", quote(css));
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `js_type` action.
    pub async fn js_type(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("js_type", Some(css), Some(text));
        let script = format!(
            "document.querySelector({}).value = {};",
            quote(css),
            quote(text)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `set_attribute` action.
    pub async fn set_attribute(
        &mut self,
        css: &str,
        attribute: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record(
            "set_attribute",
            Some(css),
            Some(&format!("{}={}", attribute, value)),
        );
        let script = format!(
            "document.querySelector({}).setAttribute({}, {});",
            quote(css),
            quote(attribute),
            quote(value)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `remove_attribute` action.
    pub async fn remove_attribute(
        &mut self,
        css: &str,
        attribute: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("remove_attribute", Some(css), Some(attribute));
        let script = format!(
            "document.querySelector({}).removeAttribute({});",
            quote(css),
            quote(attribute)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Executes the `choose_file` action.
    pub async fn choose_file(
        &mut self,
        css: &str,
        file_path: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.record("choose_file", Some(css), Some(file_path));
        let by = Selector::auto_by(css)?;
        let element = self.session.wait_for_element(by, 10).await?;

        let path = std::path::Path::new(file_path);
        let abs_path = std::fs::canonicalize(path)
            .map_err(|e| SeleniumBaseError::AssertionFailed(format!("File not found: {}", e)))?;

        // For choose_file, we use send_keys with the absolute file path on an <input type="file">
        element
            .send_keys(abs_path.to_string_lossy().as_ref())
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// Executes the `delete_all_cookies` action.
    pub async fn delete_all_cookies(&mut self) -> Result<(), SeleniumBaseError> {
        self.record("delete_all_cookies", None, None);
        self.session.delete_all_cookies().await
    }

    /// Executes the `switch_to_new_window` action.
    pub async fn switch_to_new_window(&mut self) -> Result<(), SeleniumBaseError> {
        self.record("switch_to_new_window", None, None);
        self.session.switch_to_new_window().await
    }

    /// Returns the first element matching `css`.
    pub async fn find_element(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.record("find_element", Some(css), None);
        let by = Selector::auto_by(css)?;
        self.session.wait_for_element(by, 10).await
    }

    /// Returns all elements matching `css`.
    pub async fn find_elements(
        &mut self,
        css: &str,
    ) -> Result<Vec<thirtyfour::WebElement>, SeleniumBaseError> {
        self.record("find_elements", Some(css), None);
        let by = Selector::auto_by(css)?;
        self.session.find_elements(by).await
    }

    /// Executes the `slow_click` action.
    pub async fn slow_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("slow_click", Some(css), None);
        let by = Selector::auto_by(css)?;
        let element = self.session.wait_for_element_clickable(by, 10).await?;
        self.hover(css).await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        element
            .click()
            .await
            .map_err(SeleniumBaseError::WebDriver)?;
        Ok(())
    }

    /// Closes the browser and ends the session.
    ///
    /// With [`BrowserConfig::reuse_session`] on, the browser is given back,
    /// open, for the next test instead (see [`crate::browser::reuse`]).
    #[instrument(skip(self))]
    pub async fn quit(&mut self) -> Result<(), SeleniumBaseError> {
        info!("quitting BaseCase");
        self.session.quit().await
    }

    /// The WebDriver session id of the active browser (Python's
    /// `self.driver.session_id`), or `None` without a session. Tests that
    /// reuse a browser see the same id.
    #[must_use]
    pub fn session_id(&self) -> Option<String> {
        self.session.session_id()
    }

    fn record(&self, name: &str, target: Option<&str>, value: Option<&str>) {
        if let Ok(mut recorder) = self.recorder.lock() {
            recorder.record(name, target, value);
        }
    }

    /// Appends `text` to the element selected by `css`.
    pub async fn add_text(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.record("add_text", Some(css), Some(text));
        let elem = self.session.driver().find(by).await?;
        elem.send_keys(text).await?;
        Ok(())
    }

    /// Executes the `send_keys` action.
    pub async fn send_keys(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.await_ready(css, Ready::Visible).await?;
        self.add_text(css, text).await
    }

    /// Executes the `get_value` action.
    pub async fn get_value(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        // The live `value` property reflects typed input; the HTML `value`
        // attribute only reflects the initial markup.
        if let Some(value) = self.get_property(css, "value").await? {
            return Ok(value);
        }
        Ok(self.get_attribute(css, "value").await?.unwrap_or_default())
    }

    /// Executes the `click_visible_elements` action.
    pub async fn click_visible_elements(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        let elems = self.session.driver().find_all(by).await?;
        for elem in elems {
            if elem.is_displayed().await.unwrap_or(false) {
                elem.click().await?;
            }
        }
        Ok(())
    }

    /// Executes the `wait_for_and_accept_alert` action.
    pub async fn wait_for_and_accept_alert(&self, timeout: u64) -> Result<(), SeleniumBaseError> {
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(timeout);
        loop {
            if start.elapsed() > timeout_dur {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Alert not present after {} seconds",
                    timeout
                )));
            }
            if self.session.driver().get_alert_text().await.is_ok() {
                self.session.driver().accept_alert().await?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Executes the `wait_for_and_dismiss_alert` action.
    pub async fn wait_for_and_dismiss_alert(&self, timeout: u64) -> Result<(), SeleniumBaseError> {
        let start = std::time::Instant::now();
        let timeout_dur = Duration::from_secs(timeout);
        loop {
            if start.elapsed() > timeout_dur {
                return Err(SeleniumBaseError::WaitTimeout(format!(
                    "Alert not present after {} seconds",
                    timeout
                )));
            }
            if self.session.driver().get_alert_text().await.is_ok() {
                self.session.driver().dismiss_alert().await?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Executes the `is_link_text_visible` action.
    pub async fn is_link_text_visible(&self, link_text: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::LinkText(link_text).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Executes the `is_partial_link_text_visible` action.
    pub async fn is_partial_link_text_visible(
        &self,
        partial_link_text: &str,
    ) -> Result<bool, SeleniumBaseError> {
        let by = Selector::PartialLinkText(partial_link_text).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Executes the `assert_link_text` action.
    pub async fn assert_link_text(&self, link_text: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_link_text_visible(link_text).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Link text '{}' is not visible",
                link_text
            )));
        }
        Ok(())
    }

    /// Executes the `click_partial_link_text` action.
    pub async fn click_partial_link_text(
        &mut self,
        partial_link_text: &str,
    ) -> Result<(), SeleniumBaseError> {
        let by = Selector::PartialLinkText(partial_link_text).to_by()?;
        self.record("click_partial_link_text", Some(partial_link_text), None);
        self.session.driver().find(by).await?.click().await?;
        Ok(())
    }

    /// Types `text` character by character at a human rhythm.
    ///
    /// Intervals are Gaussian with pauses at word and sentence boundaries (see
    /// [`stealth::behavior`](crate::stealth::behavior)). The pace comes from the
    /// fingerprint's [`HumanizeConfig`](crate::stealth::HumanizeConfig) when the
    /// configuration has one, and is a typical typist's otherwise. For pointer
    /// paths as well as typing, use [`sb_cdp::Page::human`](crate::sb_cdp::Page::human).
    pub async fn human_type(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        use crate::stealth::behavior::{Behavior, Humanizer, KeyAction};

        let by = Selector::auto(css).to_by()?;
        self.record("human_type", Some(css), Some(text));
        let elem = self.session.driver().find(by).await?;
        elem.click().await?; // Focus the field

        let behavior = self
            .config
            .fingerprint
            .as_ref()
            .map_or_else(Behavior::default, |fp| Behavior::from_config(&fp.humanize));
        for key in Humanizer::new(behavior).typing_plan(text) {
            tokio::time::sleep(key.wait).await;
            // Typos are off here, so only characters are planned.
            if let KeyAction::Type(c) = key.action {
                elem.send_keys(&c.to_string()).await?;
            }
        }
        Ok(())
    }

    /// Executes the `human_click` action, adding a random pre-click delay.
    pub async fn human_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.record("human_click", Some(css), None);
        let elem = self.session.driver().find(by).await?;

        let mut rng = rand::rng();
        let pre_delay = rng.random_range(100..=300);
        tokio::time::sleep(Duration::from_millis(pre_delay)).await;

        elem.click().await?;
        Ok(())
    }

    /// Executes the `smooth_scroll_to` action.
    pub async fn smooth_scroll_to(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).scrollIntoView({{behavior: 'smooth', block: 'center'}});",
            quote(css)
        );
        self.execute_script(&script).await?;
        tokio::time::sleep(Duration::from_millis(800)).await;
        Ok(())
    }

    /// Executes the `uc_click` action (alias for human_click, often used in UC mode stealth).
    pub async fn uc_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.human_click(css).await
    }

    /// Executes the `uc_type` action (alias for human_type, often used in UC mode stealth).
    pub async fn uc_type(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.human_type(css, text).await
    }

    /// Opens `url` in UC mode after reconnecting the session.
    ///
    /// Reconnecting can help evade detection schemes that fingerprint the
    /// WebDriver session state across navigations.
    pub async fn uc_open_with_reconnect(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.reconnect().await?;
        self.open(url).await?;
        Ok(())
    }

    /// Opens `url` in UC mode after a brief disconnect/reconnect cycle.
    ///
    /// This is a best-effort anti-detection helper: it quits the current
    /// WebDriver session, creates a fresh one with the same config, and then
    /// navigates to `url`.
    pub async fn uc_open_with_disconnect(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.quit().await?;
        let session = BrowserSession::connect(self.config.clone()).await?;
        self.session = session;
        self.open(url).await?;
        Ok(())
    }

    // --- JS Code Execution Helpers ---

    pub async fn execute_active_css_js(&mut self) -> Result<(), SeleniumBaseError> {
        let js = crate::stealth::js::active_css_js::get_active_css_js();
        self.execute_script(js).await?;
        Ok(())
    }

    pub async fn activate_recorder(&mut self) -> Result<(), SeleniumBaseError> {
        let js = crate::stealth::js::recorder_js::get_recorder_js();
        self.execute_script(js).await?;
        Ok(())
    }

    // --- Tour Helpers ---

    pub async fn create_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.tour = Some(Tour::new(name));
        Ok(())
    }

    pub async fn add_tour_step(
        &mut self,
        message: &str,
        target: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        match self.tour.as_mut() {
            Some(tour) => {
                tour.add_step(message, target);
                Ok(())
            }
            None => Err(SeleniumBaseError::InvalidConfig(
                "No tour created. Call create_tour first.".to_owned(),
            )),
        }
    }

    pub async fn play_tour(&mut self) -> Result<(), SeleniumBaseError> {
        let tour = self
            .tour
            .take()
            .ok_or_else(|| SeleniumBaseError::InvalidConfig("No tour created.".to_owned()))?;
        let result = tour.play(self).await;
        self.tour = Some(tour);
        result
    }

    pub async fn export_tour(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        match self.tour.as_ref() {
            Some(tour) => tour.export_html(filename),
            None => Err(SeleniumBaseError::InvalidConfig(
                "No tour created.".to_owned(),
            )),
        }
    }

    // --- Visual Testing ---

    /// Compares the structure of the page with the baseline saved under
    /// `name`, or saves it as the baseline when there is none (or when the run
    /// resets baselines).
    ///
    /// Corresponds to Python's `check_window`. The baseline is the folder
    /// `visual_baseline/<test>/<name>` in the working directory, where the
    /// test is the name of the running test (the last part of the thread's
    /// name under `cargo test`, the function), as in Python: two tests of the
    /// same name share their baselines, so give each a `name` of its own. It
    /// holds `page_url.txt`, `baseline.png` and
    /// `tags_level_1.txt` to `tags_level_3.txt`, one entry for each tag below
    /// `<body>`: the tag names (level 1), with the sorted names of its
    /// attributes (level 2), with the attribute values (level 3).
    ///
    /// The levels work as in Python. A `level` of 0 is a dry run that compares
    /// everything and logs a difference without failing. Level 1 compares the
    /// tags, level 2 the tags and attribute names and level 3 the tags and
    /// attribute values. At every level a page on another domain than the
    /// baseline's is a difference too. The screenshot is saved for a person to
    /// look at and is not compared, so a repainted pixel is no difference. The
    /// `baseline`, `check_domain` and `full_diff` arguments are not ported;
    /// [`RunSettings::with_visual_baseline_reset`] sets the baseline again.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if the page differs from
    /// the baseline at a `level` above 0, and
    /// [`SeleniumBaseError::InvalidConfig`] if `level` is not 0, 1, 2 or 3,
    /// `name` is not a plain name or the baseline cannot be read or written.
    pub async fn check_window(&mut self, name: &str, level: f64) -> Result<(), SeleniumBaseError> {
        use crate::api::run_settings::{
            apply_check_level, baseline_dir, check_level, check_page_structure,
        };
        let checked = check_level(level)?;
        let dir = baseline_dir(name)?;
        // Python waits for the page to settle before it reads it.
        let _ = self.wait_for_ready_state_complete().await;
        let url = self.get_current_url().await?;
        let html = self.get_page_source().await?;
        let png = self.screenshot_as_png().await?;
        let outcome = check_page_structure(
            &dir,
            &url,
            &html,
            &png,
            self.run_settings.reset_visual_baseline(),
            checked,
        );
        apply_check_level(outcome, level)
    }

    /// Compares a screenshot of the page with the baseline image saved under
    /// `name` (feature `visual`), or saves it as the baseline when there is
    /// none (or when the run resets baselines).
    ///
    /// Unlike [`check_window`](Self::check_window), which compares the HTML
    /// structure, this compares pixels, by structural similarity with the
    /// default threshold; see [`crate::visual`]. The baseline is
    /// `visual_baseline/<test>/<name>.png`, and a mismatch leaves
    /// `<name>.actual.png` and `<name>.diff.png` beside it. `<test>` names the
    /// running test by its whole path, from the thread `cargo test` runs it
    /// on: the test binary (the crate or the file in `tests/`), the module
    /// path and the function, joined with `.`, such as
    /// `cart.checkout.renders`, so tests of the same name in other modules or
    /// files keep separate baselines. (`check_window` names its folder after
    /// the function alone, as Python does.)
    /// [`RunSettings::with_visual_baseline_reset`] saves new baselines instead
    /// of comparing. For another folder or threshold, use a
    /// [`VisualBaseline`](crate::visual::VisualBaseline) with
    /// [`check_page`](crate::visual::VisualBaseline::check_page).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::AssertionFailed`] when the page looks different
    /// from the baseline or the screenshot has another size,
    /// [`SeleniumBaseError::InvalidConfig`] when `name` is not 1 to 80 ASCII
    /// letters, digits, `-` or `_`, and the screenshot's or the file system's
    /// error otherwise.
    #[cfg(feature = "visual")]
    pub async fn assert_visual_match(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        use crate::api::run_settings::visual_match_folder;
        use crate::visual::VisualBaseline;

        let baselines = VisualBaseline::new(visual_match_folder())
            .update_baselines(self.run_settings.reset_visual_baseline());
        // As check_window does, let the page settle before it is captured.
        let _ = self.wait_for_ready_state_complete().await;
        baselines.check_page(name, &*self).await.map(|_| ())
    }

    // --- MasterQA Integration ---

    pub async fn verify(&mut self, statement: &str) -> Result<(), SeleniumBaseError> {
        if !MasterQA::verify(statement) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Manual QA verification failed for: {statement}"
            )));
        }
        Ok(())
    }

    // --- Deferred Asserts ---

    /// Checks now that `css` is visible, and records a failure, with the
    /// page's URL, for [`process_deferred_asserts`](Self::process_deferred_asserts)
    /// to report. Waits a couple of seconds for the element, as Python does.
    /// Corresponds to Python's `deferred_assert_element`.
    pub async fn deferred_assert_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let mut deferred = std::mem::take(&mut self.deferred);
        deferred.check_element(self, css).await;
        self.deferred = deferred;
        Ok(())
    }

    /// Checks now that `css` contains `text`, and records a failure, with the
    /// page's URL, for [`process_deferred_asserts`](Self::process_deferred_asserts)
    /// to report. Corresponds to Python's `deferred_assert_text`.
    pub async fn deferred_assert_text(
        &mut self,
        text: &str,
        css: &str,
    ) -> Result<(), SeleniumBaseError> {
        let mut deferred = std::mem::take(&mut self.deferred);
        deferred.check_text(self, text, css).await;
        self.deferred = deferred;
        Ok(())
    }

    pub async fn process_deferred_asserts(&mut self) -> Result<(), SeleniumBaseError> {
        let mut deferred = std::mem::take(&mut self.deferred);
        let result = deferred.process(self).await;
        self.deferred = deferred;
        result
    }

    // --- Dashboards and Charts ---

    pub async fn create_presentation(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.presentation = Some(Presentation::new(title));
        Ok(())
    }

    pub async fn add_presentation_slide(&mut self, content: &str) -> Result<(), SeleniumBaseError> {
        match self.presentation.as_mut() {
            Some(presentation) => {
                presentation.add_slide(content);
                Ok(())
            }
            None => Err(SeleniumBaseError::InvalidConfig(
                "No presentation created. Call create_presentation first.".to_owned(),
            )),
        }
    }

    pub async fn save_presentation(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        match self.presentation.as_ref() {
            Some(presentation) => presentation.save(filename),
            None => Err(SeleniumBaseError::InvalidConfig(
                "No presentation created.".to_owned(),
            )),
        }
    }

    pub async fn create_pie_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Pie));
        Ok(())
    }

    pub async fn create_bar_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Bar));
        Ok(())
    }

    pub async fn create_line_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Line));
        Ok(())
    }

    pub async fn create_area_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Area));
        Ok(())
    }

    pub async fn create_column_chart(&mut self, title: &str) -> Result<(), SeleniumBaseError> {
        self.chart = Some(Chart::new(title, ChartType::Column));
        Ok(())
    }

    pub async fn add_data_point(
        &mut self,
        label: &str,
        value: i32,
    ) -> Result<(), SeleniumBaseError> {
        match self.chart.as_mut() {
            Some(chart) => {
                chart.add_data_point(label, value);
                Ok(())
            }
            None => Err(SeleniumBaseError::InvalidConfig(
                "No chart created. Call create_pie_chart first.".to_owned(),
            )),
        }
    }

    pub async fn save_pie_chart(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        self.save_chart(filename).await
    }

    pub async fn save_chart(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        match self.chart.as_ref() {
            Some(chart) => chart.save(filename),
            None => Err(SeleniumBaseError::InvalidConfig(
                "No chart created.".to_owned(),
            )),
        }
    }

    // --- Additional Python parity methods ---

    /// Alias for `type_text` that clears the field first.
    pub async fn update_text(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.record("update_text", Some(css), Some(text));
        let by = Selector::auto(css).to_by()?;
        self.demo_highlight(css).await?;
        self.session.type_text(by, text).await?;
        self.demo_pause(true).await;
        Ok(())
    }

    /// Focus the element matching `css`.
    pub async fn focus(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!("document.querySelector({}).focus();", quote(css));
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Minimize the browser window.
    pub async fn minimize_window(&self) -> Result<(), SeleniumBaseError> {
        self.session.driver().minimize_window().await?;
        Ok(())
    }

    /// Set the window rectangle directly.
    pub async fn set_window_rect(
        &self,
        x: i64,
        y: i64,
        width: u32,
        height: u32,
    ) -> Result<(), SeleniumBaseError> {
        self.session
            .driver()
            .set_window_rect(x, y, width, height)
            .await?;
        Ok(())
    }

    /// Reset window size to the default 1280x720.
    pub async fn reset_window_size(&self) -> Result<(), SeleniumBaseError> {
        self.set_window_size(1280, 720).await
    }

    /// Scroll vertically by `delta_y` pixels.
    pub async fn scroll_by_y(&self, delta_y: i64) -> Result<(), SeleniumBaseError> {
        self.execute_script(&format!("window.scrollBy(0, {delta_y});"))
            .await?;
        Ok(())
    }

    /// Scroll up by 400 pixels.
    pub async fn scroll_up(&self) -> Result<(), SeleniumBaseError> {
        self.scroll_by_y(-400).await
    }

    /// Scroll down by 400 pixels.
    pub async fn scroll_down(&self) -> Result<(), SeleniumBaseError> {
        self.scroll_by_y(400).await
    }

    /// Click the element matching the given XPath.
    pub async fn click_xpath(&mut self, xpath: &str) -> Result<(), SeleniumBaseError> {
        self.record("click_xpath", Some(xpath), None);
        let by = By::XPath(xpath.to_owned());
        self.session.click(by).await
    }

    /// JavaScript-click the element only if it is present in the DOM.
    pub async fn js_click_if_present(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        if self.is_element_present(css).await? {
            self.js_click(css).await?;
        }
        Ok(())
    }

    /// JavaScript-click all elements matching `css`.
    pub async fn js_click_all(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("js_click_all", Some(css), None);
        let script = format!(
            "document.querySelectorAll({}).forEach(e => e.click());",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Click the element using jQuery if available, otherwise fall back to JS click.
    pub async fn jquery_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.record("jquery_click", Some(css), None);
        let literal = quote(css);
        let script = format!(
            "(function(){{
                if (window.jQuery && jQuery({literal}).length) {{ jQuery({literal})[0].click(); }}
                else {{ document.querySelector({literal}).click(); }}
            }})();"
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Hide the element matching `css` by setting `display:none`.
    pub async fn hide_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).style.display='none';",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Show the element matching `css` by setting `display:block`.
    pub async fn show_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelector({}).style.display='block';",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Remove the element matching `css` from the DOM.
    pub async fn remove_element(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var e=document.querySelector({}); if(e) e.parentNode.removeChild(e);",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Block common ad elements on the current page.
    pub async fn block_ads(&mut self) -> Result<(), SeleniumBaseError> {
        let selectors = [
            "[id*='google_ads']",
            "[id*='ad-']",
            "[class*='ad-']",
            "[class*='ads ']",
            "iframe[src*='ads']",
            "iframe[src*='doubleclick']",
        ];
        let joined = selectors.join(",");
        let script = format!(
            "document.querySelectorAll({}).forEach(e => e.remove());",
            quote(&joined)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Assert that the current URL exactly equals `expected`.
    pub async fn assert_url(&mut self, expected: &str) -> Result<(), SeleniumBaseError> {
        let url = self.get_current_url().await?;
        if url == expected {
            return Ok(());
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected URL '{expected}', got '{url}'"
        )))
    }

    /// Return the unique absolute links on the current page.
    pub async fn get_unique_links(&self) -> Result<Vec<String>, SeleniumBaseError> {
        let script = r#"
            return Array.from(document.querySelectorAll('a[href]'))
                .map(a => a.href)
                .filter((v, i, a) => a.indexOf(v) === i);
        "#;
        match self.execute_script(script).await? {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    /// Fail if any link on the page returns HTTP 404.
    pub async fn assert_no_404_errors(&mut self) -> Result<(), SeleniumBaseError> {
        let links = self.get_unique_links().await?;
        crate::install_tls_provider();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| SeleniumBaseError::Unsupported(e.to_string()))?;
        let mut broken = Vec::new();
        for link in links {
            if link.starts_with("http://") || link.starts_with("https://") {
                match client.head(&link).send().await {
                    Ok(resp) if resp.status().as_u16() == 404 => broken.push(link),
                    _ => {}
                }
            }
        }
        if broken.is_empty() {
            Ok(())
        } else {
            Err(SeleniumBaseError::AssertionFailed(format!(
                "found {} broken link(s) with 404: {:?}",
                broken.len(),
                broken
            )))
        }
    }

    /// Alias for `assert_no_404_errors`.
    pub async fn assert_no_broken_links(&mut self) -> Result<(), SeleniumBaseError> {
        self.assert_no_404_errors().await
    }

    /// Return all visible text options for a `<select>` element.
    pub async fn get_select_options(
        &mut self,
        css: &str,
    ) -> Result<Vec<String>, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        let element = self.session.wait_for_element(by, 10).await?;
        let select = thirtyfour::components::SelectElement::new(&element).await?;
        let mut options = Vec::new();
        for opt in select.options().await? {
            options.push(opt.text().await.unwrap_or_default());
        }
        Ok(options)
    }

    /// Set the `value` property of the element.
    pub async fn set_value(&mut self, css: &str, value: &str) -> Result<(), SeleniumBaseError> {
        self.await_ready(css, Ready::Present).await?;
        let script = format!(
            "document.querySelector({}).value = {};",
            quote(css),
            quote(value)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Sets the text of the element `css` with a script: the `value` of an
    /// `<input>` or `<textarea>` (firing `input` and `change`, as a user's
    /// typing would), the `textContent` of any other element. Waits for the
    /// element to exist first. Corresponds to Python's `set_text`; unlike
    /// Python it presses no Enter for a trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not
    /// appear.
    pub async fn set_text(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.await_ready(css, Ready::Present).await?;
        let found = self
            .execute_script(&page_scripts::set_text_script(css, text))
            .await?;
        if found.as_bool() != Some(true) {
            return Err(SeleniumBaseError::element_not_found(css));
        }
        Ok(())
    }

    /// Get the `src` URL of an image element.
    pub async fn get_image_url(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        self.get_attribute(css, "src").await?.ok_or_else(|| {
            SeleniumBaseError::AssertionFailed(format!("element '{css}' has no src attribute"))
        })
    }

    /// Extract the domain (scheme, host and a port that is not the scheme's
    /// default) from the current URL: `http://localhost:8080` for
    /// `http://localhost:8080/a/b?c=d`.
    pub async fn get_domain_url(&mut self) -> Result<String, SeleniumBaseError> {
        let url = self.get_current_url().await?;
        reqwest::Url::parse(&url)
            .map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))
            .map(|u| {
                let host = u.host_str().unwrap_or("");
                match u.port() {
                    Some(port) => format!("{}://{host}:{port}", u.scheme()),
                    None => format!("{}://{host}", u.scheme()),
                }
            })
    }

    /// Convert a simple XPath expression to an approximate CSS selector.
    pub fn convert_xpath_to_css(&self, xpath: &str) -> Result<String, SeleniumBaseError> {
        crate::utils::selectors::xpath_to_css(xpath)
    }

    /// Fails the test once it has run for more than `seconds`.
    ///
    /// The clock started when this test case was created. The limit is checked
    /// whenever the test opens a page, waits for the page to be ready or waits
    /// for an element, and the next of those fails with a
    /// [`SeleniumBaseError::TestLifecycle`] error saying that the limit was
    /// exceeded. Waits are also cut short so that none outlasts the limit by
    /// more than a second. `0` removes the limit, as in Python.
    ///
    /// A test that never does one of those things is not interrupted; wrap
    /// the test in `tokio::time::timeout` for a hard stop.
    ///
    /// # Errors
    ///
    /// Never fails; the `Result` keeps the signature the method always had.
    pub fn set_time_limit(&mut self, seconds: u64) -> Result<(), SeleniumBaseError> {
        self.time_limit = (seconds > 0).then(|| Duration::from_secs(seconds));
        Ok(())
    }

    /// Fails with a time-limit error if the test has run past its limit.
    fn check_time_limit(&self) -> Result<(), SeleniumBaseError> {
        match self.time_limit {
            Some(limit) if self.started.elapsed() > limit => {
                let plural = if limit.as_secs() == 1 { "" } else { "s" };
                Err(SeleniumBaseError::test_lifecycle(format!(
                    "This test has exceeded the time limit of {} second{plural}!",
                    limit.as_secs()
                )))
            }
            _ => Ok(()),
        }
    }

    /// Return whether a JavaScript alert is currently present.
    pub async fn is_alert_present(&self) -> Result<bool, SeleniumBaseError> {
        Ok(self.session.driver().get_alert_text().await.is_ok())
    }

    /// Get the browser user agent string.
    pub async fn get_user_agent(&self) -> Result<String, SeleniumBaseError> {
        match self.execute_script("return navigator.userAgent;").await? {
            Value::String(ua) => Ok(ua),
            _ => Ok(String::new()),
        }
    }

    /// Get the browser locale code.
    pub async fn get_locale_code(&self) -> Result<String, SeleniumBaseError> {
        match self
            .execute_script("return navigator.language || 'en-US';")
            .await?
        {
            Value::String(l) => Ok(l),
            _ => Ok("en-US".to_owned()),
        }
    }

    /// Return whether the element is clickable (visible and enabled).
    pub async fn is_element_clickable(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)
                && elem.is_enabled().await.unwrap_or(false)),
            Err(_) => Ok(false),
        }
    }

    /// Return whether the element is enabled.
    pub async fn is_element_enabled(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        self.session.is_enabled(by).await
    }

    /// `requested` seconds, cut to the time the test has left if it has a time
    /// limit (rounded up to a whole second).
    fn effective_timeout(&self, requested: u64) -> u64 {
        match self.time_limit {
            Some(limit) => {
                let left = limit.saturating_sub(self.started.elapsed());
                let seconds = left.as_secs() + u64::from(left.subsec_nanos() > 0);
                requested.min(seconds)
            }
            None => requested,
        }
    }

    /// How long a wait that has no time of its own lasts, in seconds: the
    /// default [`set_timeout`](Self::set_timeout) and
    /// [`set_default_timeout`](Self::set_default_timeout) set (10 until either
    /// is called), cut to the time the test has left if it has a
    /// [time limit](Self::set_time_limit).
    fn default_wait_secs(&self) -> u64 {
        self.effective_timeout(self.timeout_secs)
    }

    /// Return whether `text` exactly matches the visible text of the element.
    pub async fn is_exact_text_visible(
        &self,
        text: &str,
        css: &str,
    ) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => Ok(elem.is_displayed().await.unwrap_or(false)
                && elem.text().await.unwrap_or_default() == text),
            Err(_) => Ok(false),
        }
    }

    /// Return whether the element has any non-empty visible text.
    pub async fn is_non_empty_text_visible(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        match self.session.driver().find(by).await {
            Ok(elem) => {
                let text = elem.text().await.unwrap_or_default();
                Ok(elem.is_displayed().await.unwrap_or(false) && !text.trim().is_empty())
            }
            Err(_) => Ok(false),
        }
    }

    /// Alias for `assert_element_absent`.
    pub async fn assert_element_not_present(&self, css: &str) -> Result<(), SeleniumBaseError> {
        self.assert_element_absent(css).await
    }

    /// Assert that all provided CSS selectors are present.
    pub async fn assert_elements_present(
        &self,
        selectors: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for css in selectors {
            self.assert_element(css).await?;
        }
        Ok(())
    }

    /// Wait until any of the provided selectors is visible, then return it.
    pub async fn wait_for_any_of_elements_visible(
        &self,
        selectors: &[&str],
        timeout_secs: u64,
    ) -> Result<String, SeleniumBaseError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            for css in selectors {
                if self.is_element_visible(css).await.unwrap_or(false) {
                    return Ok((*css).to_owned());
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(SeleniumBaseError::WaitTimeout(
                    "none of the selectors became visible".to_owned(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }

    /// Assert that at least one of the selectors is visible.
    pub async fn assert_any_of_elements_visible(
        &self,
        selectors: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for css in selectors {
            if self.is_element_visible(css).await.unwrap_or(false) {
                return Ok(());
            }
        }
        Err(SeleniumBaseError::AssertionFailed(
            "none of the selectors is visible".to_owned(),
        ))
    }

    /// Return the inner text of the element.
    pub async fn get_text_content(&mut self, css: &str) -> Result<String, SeleniumBaseError> {
        let by = Selector::auto(css).to_by()?;
        let elem = self.session.driver().find(by).await?;
        Ok(elem.text().await.unwrap_or_default())
    }

    /// Types `text` into the element `css`, one key at a time, so the page
    /// sees every key press. A trailing newline presses Enter instead of
    /// being typed. Corresponds to Python's `press_keys(selector, text)`.
    ///
    /// To press one named key such as `Escape` on whatever has focus, use
    /// [`press_key`](Self::press_key).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not
    /// appear.
    pub async fn press_keys(&self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.await_ready(css, Ready::Present).await?;
        let by = Selector::auto(css).to_by()?;
        let element = self.session.driver().find(by).await?;
        self.demo_highlight(css).await?;
        let (body, enter) = split_trailing_enter(text);
        for key in body.chars() {
            element.send_keys(key.to_string()).await?;
        }
        if enter {
            element.send_keys(Key::Enter.value().to_string()).await?;
        }
        self.demo_pause(true).await;
        Ok(())
    }

    /// Presses one named key on the element that has focus: `enter`,
    /// `return`, `tab`, `escape` (or `esc`), `space`, `up`, `down`, `left` or
    /// `right`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidSelector`] for any other name.
    pub async fn press_key(&self, key_name: &str) -> Result<(), SeleniumBaseError> {
        let key = match key_name.to_ascii_lowercase().as_str() {
            "enter" => Key::Enter,
            "return" => Key::Return,
            "tab" => Key::Tab,
            "escape" => Key::Escape,
            "esc" => Key::Escape,
            "space" => Key::Space,
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            _ => {
                return Err(SeleniumBaseError::InvalidSelector(format!(
                    "unknown key '{key_name}'"
                )))
            }
        };
        self.session
            .driver()
            .action_chain()
            .send_keys(key.value().to_string())
            .perform()
            .await?;
        Ok(())
    }

    // --- Session storage helpers ---

    pub async fn set_session_storage_item(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "window.sessionStorage.setItem({}, {});",
            quote(key),
            quote(value)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    pub async fn get_session_storage_item(&self, key: &str) -> Result<Value, SeleniumBaseError> {
        let script = format!("return window.sessionStorage.getItem({});", quote(key));
        self.execute_script(&script).await
    }

    pub async fn remove_session_storage_item(&self, key: &str) -> Result<(), SeleniumBaseError> {
        let script = format!("window.sessionStorage.removeItem({});", quote(key));
        self.execute_script(&script).await?;
        Ok(())
    }

    pub async fn clear_session_storage(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("window.sessionStorage.clear();")
            .await?;
        Ok(())
    }

    pub async fn get_session_storage_keys(&self) -> Result<Vec<String>, SeleniumBaseError> {
        match self
            .execute_script("return Object.keys(window.sessionStorage);")
            .await?
        {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    pub async fn get_session_storage_items(
        &self,
    ) -> Result<HashMap<String, String>, SeleniumBaseError> {
        let script = r#"
            const items = {};
            for (let i = 0; i < window.sessionStorage.length; i++) {
                const k = window.sessionStorage.key(i);
                items[k] = window.sessionStorage.getItem(k);
            }
            return items;
        "#;
        match self.execute_script(script).await? {
            Value::Object(map) => Ok(map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
                .collect()),
            _ => Ok(HashMap::new()),
        }
    }

    pub async fn get_local_storage_keys(&self) -> Result<Vec<String>, SeleniumBaseError> {
        match self
            .execute_script("return Object.keys(window.localStorage);")
            .await?
        {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    pub async fn get_local_storage_items(
        &self,
    ) -> Result<HashMap<String, String>, SeleniumBaseError> {
        let script = r#"
            const items = {};
            for (let i = 0; i < window.localStorage.length; i++) {
                const k = window.localStorage.key(i);
                items[k] = window.localStorage.getItem(k);
            }
            return items;
        "#;
        match self.execute_script(script).await? {
            Value::Object(map) => Ok(map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
                .collect()),
            _ => Ok(HashMap::new()),
        }
    }

    // --- Navigation aliases ---

    pub async fn get(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn goto(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn go_to(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn open_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn visit(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn visit_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await
    }

    pub async fn reload(&mut self) -> Result<(), SeleniumBaseError> {
        self.refresh().await
    }

    pub async fn reload_page(&mut self) -> Result<(), SeleniumBaseError> {
        self.refresh().await
    }

    /// Open `url` only if the current URL is not already `url`.
    pub async fn open_if_not_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        if self.get_current_url().await? != url {
            self.open(url).await?;
        }
        Ok(())
    }

    pub async fn goto_if_not_url(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open_if_not_url(url).await
    }

    // --- Input aliases ---

    pub async fn input(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.type_text(css, text).await
    }

    pub async fn fill(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.type_text(css, text).await
    }

    pub async fn write(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.type_text(css, text).await
    }

    pub async fn select(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.select_option_by_text(css, text).await
    }

    pub async fn right_click(&mut self, css: &str) -> Result<(), SeleniumBaseError> {
        self.context_click(css).await
    }

    /// Alias to find an element that can be used for further chaining.
    pub async fn get_element(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.find_element(css).await
    }

    /// Alias for `find_element`.
    pub async fn locator(
        &mut self,
        css: &str,
    ) -> Result<thirtyfour::WebElement, SeleniumBaseError> {
        self.find_element(css).await
    }

    /// Alias for `wait_for_element_visible`.
    pub async fn wait_for_selector(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_element_visible(css, timeout_secs).await
    }

    /// Alias for `wait_for_element` (present).
    pub async fn wait_for_query_selector(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_element(css, timeout_secs).await
    }
}

// Additional BaseCase implementations split out to keep the file manageable.
include!("base_case_impls/base_case_impl_pdf_html.rs");
include!("base_case_impls/base_case_impl_extra.rs");
include!("base_case_impls/base_case_impl_cdp_page.rs");
include!("base_case_impls/base_case_impl_dialog_inspector.rs");
include!("base_case_impls/base_case_impl_shadow.rs");
#[cfg(feature = "gui")]
include!("base_case_impls/base_case_impl_gui.rs");
include!("base_case_impls/base_case_impl_masterqa.rs");
include!("base_case_impls/base_case_impl_common.rs");
include!("base_case_impls/base_case_impl_links.rs");
include!("base_case_impls/base_case_impl_downloads.rs");
include!("base_case_impls/base_case_impl_storage.rs");
include!("base_case_impls/base_case_impl_dom.rs");
include!("base_case_impls/base_case_impl_window.rs");
include!("base_case_impls/base_case_impl_drivers.rs");
include!("base_case_impls/base_case_impl_mouse.rs");
include!("base_case_impls/base_case_impl_alerts.rs");
include!("base_case_impls/base_case_impl_browser.rs");
include!("base_case_impls/base_case_impl_nav.rs");
include!("base_case_impls/base_case_impl_media.rs");
include!("base_case_impls/base_case_impl_tours.rs");
include!("base_case_impls/base_case_impl_charts.rs");
include!("base_case_impls/base_case_impl_presentations.rs");
include!("base_case_impls/base_case_impl_jslibs.rs");
include!("base_case_impls/base_case_impl_misc.rs");
include!("base_case_impls/base_case_impl_addon.rs");
include!("base_case_impls/base_case_impl_parity.rs");
include!("base_case_impls/base_case_impl_esc.rs");
include!("base_case_impls/base_case_impl_rec_cdp.rs");
#[cfg(test)]
include!("base_case_impls/base_case_impl_driver_tests.rs");
#[cfg(test)]
include!("base_case_impls/base_case_impl_text_tests.rs");
#[cfg(test)]
include!("base_case_impls/base_case_impl_traffic_tests.rs");
#[cfg(test)]
include!("base_case_impls/base_case_impl_frame_tests.rs");
