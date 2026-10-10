//! Capability traits for the public `BaseCase` API.
//!
//! These traits describe cross-cutting concerns of the test framework (browser
//! control, element interaction, assertions, screenshots). They are implemented
//! by [`crate::BaseCase`] (WebDriver) and by [`crate::sb_cdp::Page`] (Pure
//! CDP), so a helper written once over the traits runs on either engine.
//!
//! None of them mentions an engine's own types. Finding an element and keeping
//! the handle is engine-specific; [`BaseCase::find_element`] returns a
//! `thirtyfour::WebElement` for code that wants it, and a CDP `Page` offers
//! `locator(..)`.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::{AssertionApi, BrowserApi, ElementApi};
//!
//! async fn sign_in<E>(sb: &mut E) -> Result<(), seleniumbase_rs::SeleniumBaseError>
//! where
//!     E: BrowserApi + ElementApi + AssertionApi,
//! {
//!     sb.open("https://example.com/login").await?;
//!     sb.type_text("#user", "alice").await?;
//!     sb.click("#submit").await?;
//!     sb.assert_element("#dashboard").await
//! }
//! ```

use std::path::PathBuf;

use async_trait::async_trait;

use crate::api::base_case::BaseCase;

/// Browser-level navigation and lifecycle operations.
#[async_trait]
pub trait BrowserApi {
    /// Open `url` in the active browser window/tab.
    async fn open(&mut self, url: &str) -> crate::Result<()>;

    /// Close the browser session.
    async fn quit(&mut self) -> crate::Result<()>;

    /// Reload the current page.
    async fn refresh(&self) -> crate::Result<()>;

    /// Navigate back in browser history.
    async fn go_back(&self) -> crate::Result<()>;

    /// Navigate forward in browser history.
    async fn go_forward(&self) -> crate::Result<()>;

    /// Return the current page title.
    async fn get_title(&mut self) -> crate::Result<String>;

    /// Return the current page URL.
    async fn get_url(&mut self) -> crate::Result<String>;
}

/// Element finding and interaction operations.
#[async_trait]
pub trait ElementApi {
    /// Click the element matching `css`.
    async fn click(&mut self, css: &str) -> crate::Result<()>;

    /// Double-click the element matching `css`.
    async fn double_click(&mut self, css: &str) -> crate::Result<()>;

    /// Type `text` into the element matching `css`.
    async fn type_text(&mut self, css: &str, text: &str) -> crate::Result<()>;

    /// Return the visible text of the element matching `css`.
    async fn get_text(&mut self, css: &str) -> crate::Result<String>;

    /// Return the value of attribute `attr` on the element matching `css`, if any.
    async fn get_attribute(&mut self, css: &str, attr: &str) -> crate::Result<Option<String>>;
}

/// Assertion helpers used in tests.
#[async_trait]
pub trait AssertionApi {
    /// Assert that the page title equals `expected`.
    async fn assert_title(&mut self, expected: &str) -> crate::Result<()>;

    /// Assert that the element matching `css` exists.
    async fn assert_element(&self, css: &str) -> crate::Result<()>;

    /// Assert that `expected` text appears inside the element matching `css`.
    async fn assert_text(&mut self, css: &str, expected: &str) -> crate::Result<()>;

    /// Assert that no JavaScript errors were logged on the current page.
    async fn assert_no_js_errors(&self) -> crate::Result<()>;
}

/// Screenshot capture operations.
#[async_trait]
pub trait ScreenshotApi {
    /// Save a screenshot to the logs directory with `filename`.
    async fn save_screenshot(&self, filename: &str) -> crate::Result<PathBuf>;

    /// Return the current page screenshot as PNG bytes.
    async fn screenshot_as_png(&self) -> crate::Result<Vec<u8>>;
}

#[async_trait]
impl BrowserApi for BaseCase {
    async fn open(&mut self, url: &str) -> crate::Result<()> {
        BaseCase::open(self, url).await
    }

    async fn quit(&mut self) -> crate::Result<()> {
        BaseCase::quit(self).await
    }

    async fn refresh(&self) -> crate::Result<()> {
        BaseCase::refresh(self).await
    }

    async fn go_back(&self) -> crate::Result<()> {
        BaseCase::go_back(self).await
    }

    async fn go_forward(&self) -> crate::Result<()> {
        BaseCase::go_forward(self).await
    }

    async fn get_title(&mut self) -> crate::Result<String> {
        BaseCase::get_title(self).await
    }

    async fn get_url(&mut self) -> crate::Result<String> {
        BaseCase::get_url(self).await
    }
}

#[async_trait]
impl ElementApi for BaseCase {
    async fn click(&mut self, css: &str) -> crate::Result<()> {
        BaseCase::click(self, css).await
    }

    async fn double_click(&mut self, css: &str) -> crate::Result<()> {
        BaseCase::double_click(self, css).await
    }

    async fn type_text(&mut self, css: &str, text: &str) -> crate::Result<()> {
        BaseCase::type_text(self, css, text).await
    }

    async fn get_text(&mut self, css: &str) -> crate::Result<String> {
        BaseCase::get_text(self, css).await
    }

    async fn get_attribute(&mut self, css: &str, attr: &str) -> crate::Result<Option<String>> {
        BaseCase::get_attribute(self, css, attr).await
    }
}

#[async_trait]
impl AssertionApi for BaseCase {
    async fn assert_title(&mut self, expected: &str) -> crate::Result<()> {
        BaseCase::assert_title(self, expected).await
    }

    async fn assert_element(&self, css: &str) -> crate::Result<()> {
        BaseCase::assert_element(self, css).await
    }

    async fn assert_text(&mut self, css: &str, expected: &str) -> crate::Result<()> {
        BaseCase::assert_text(self, css, expected).await
    }

    async fn assert_no_js_errors(&self) -> crate::Result<()> {
        BaseCase::assert_no_js_errors(self).await
    }
}

#[async_trait]
impl ScreenshotApi for BaseCase {
    async fn save_screenshot(&self, filename: &str) -> crate::Result<PathBuf> {
        BaseCase::save_screenshot(self, filename).await
    }

    async fn screenshot_as_png(&self) -> crate::Result<Vec<u8>> {
        BaseCase::screenshot_as_png(self).await
    }
}

// ----------------------------------------------------------------------
// Pure CDP
// ----------------------------------------------------------------------

use crate::sb_cdp::Page;

#[async_trait]
impl BrowserApi for Page {
    async fn open(&mut self, url: &str) -> crate::Result<()> {
        self.goto(url).await
    }

    /// Closes this tab. To close the whole browser, call
    /// [`Browser::close`](crate::sb_cdp::Browser::close).
    async fn quit(&mut self) -> crate::Result<()> {
        self.close().await
    }

    async fn refresh(&self) -> crate::Result<()> {
        self.reload().await
    }

    async fn go_back(&self) -> crate::Result<()> {
        self.back().await
    }

    async fn go_forward(&self) -> crate::Result<()> {
        self.forward().await
    }

    async fn get_title(&mut self) -> crate::Result<String> {
        self.title().await
    }

    async fn get_url(&mut self) -> crate::Result<String> {
        self.url().await
    }
}

#[async_trait]
impl ElementApi for Page {
    async fn click(&mut self, css: &str) -> crate::Result<()> {
        self.locator(css).click().await
    }

    async fn double_click(&mut self, css: &str) -> crate::Result<()> {
        self.locator(css).double_click().await
    }

    /// Replaces what is in the field, typing one key at a time.
    async fn type_text(&mut self, css: &str, text: &str) -> crate::Result<()> {
        let field = self.locator(css);
        field.clear().await?;
        field.type_text(text).await
    }

    async fn get_text(&mut self, css: &str) -> crate::Result<String> {
        self.locator(css).text().await
    }

    async fn get_attribute(&mut self, css: &str, attr: &str) -> crate::Result<Option<String>> {
        self.locator(css).attribute(attr).await
    }
}

#[async_trait]
impl AssertionApi for Page {
    async fn assert_title(&mut self, expected: &str) -> crate::Result<()> {
        self.expect().to_have_title(expected).await
    }

    async fn assert_element(&self, css: &str) -> crate::Result<()> {
        self.locator(css).expect().to_exist().await
    }

    async fn assert_text(&mut self, css: &str, expected: &str) -> crate::Result<()> {
        self.locator(css).expect().to_contain_text(expected).await
    }

    async fn assert_no_js_errors(&self) -> crate::Result<()> {
        Page::assert_no_js_errors(self).await
    }
}

#[async_trait]
impl ScreenshotApi for Page {
    /// Saves into the logs directory; `filename` must be a bare file name.
    async fn save_screenshot(&self, filename: &str) -> crate::Result<PathBuf> {
        let path = crate::artifacts::confined_path(
            &crate::artifacts::ensure_latest_logs_dir()?,
            filename,
        )?;
        tokio::fs::write(&path, self.screenshot().await?).await?;
        Ok(path)
    }

    async fn screenshot_as_png(&self) -> crate::Result<Vec<u8>> {
        self.screenshot().await
    }
}
