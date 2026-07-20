//! Capability traits for the public `BaseCase` API.
//!
//! These traits describe cross-cutting concerns of the test framework (browser
//! control, element interaction, assertions, screenshots). They are implemented
//! by [`crate::BaseCase`] so that callers can depend on capabilities rather
//! than the concrete type, and so that future alternative test runners can
//! expose the same interface.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::api::base_case::BaseCase;

/// Browser-level navigation and lifecycle operations.
#[async_trait]
pub trait BrowserApi {
    /// Open `url` in the active browser window/tab.
    async fn open(&mut self, url: &str) -> crate::Result<()>;

    /// Close the browser session.
    async fn quit(self) -> crate::Result<()>;

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
    /// Find an element using a CSS selector.
    async fn find_element(&mut self, css: &str) -> crate::Result<thirtyfour::WebElement>;

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

    async fn quit(self) -> crate::Result<()> {
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
    async fn find_element(&mut self, css: &str) -> crate::Result<thirtyfour::WebElement> {
        BaseCase::find_element(self, css).await
    }

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
