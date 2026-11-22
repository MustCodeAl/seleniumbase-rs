//! Capability traits for the public `BaseCase` API.
//!
//! These traits describe cross-cutting concerns of the test framework (browser
//! control, element interaction, assertions, screenshots). They are implemented
//! by [`crate::BaseCase`] (WebDriver) and by [`crate::sb_cdp::Page`] (Pure
//! CDP), so a helper written once over the traits runs on either engine.
//!
//! None of them mentions an engine's own types. Finding an element and keeping
//! the handle is engine-specific; [`BaseCase::find_element`] returns an
//! [`Element`](crate::Element) for code that wants it, and a CDP `Page` offers
//! `locator(..)`.
//!
//! The methods are native `async fn`s, with no boxed future. A trait declares each
//! one as `fn name(..) -> impl Future<Output = ..> + Send`, so the future is `Send`
//! in code that is generic over the trait (it can go to `tokio::spawn`), and an
//! `impl` writes a plain `async fn`. The traits are not dyn compatible: take a
//! generic or an `impl Trait` parameter, not `&mut dyn BrowserApi`.
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

use crate::api::base_case::BaseCase;
use crate::sb_cdp::Page;
use crate::utils::selectors::Selector;

mod private {
    pub trait EngineCapability {}
    impl EngineCapability for crate::api::base_case::BaseCase {}
    impl EngineCapability for crate::sb_cdp::Page {}
}

/// Browser-level navigation and lifecycle operations.
///
/// This trait is sealed and cannot be implemented for types outside `seleniumbase-rs`.
pub trait BrowserApi: private::EngineCapability {
    /// Open `url` in the active browser window/tab.
    fn open(&mut self, url: &str) -> impl Future<Output = crate::Result<()>> + Send;

    /// Close the browser session.
    fn quit(&mut self) -> impl Future<Output = crate::Result<()>> + Send;

    /// Reload the current page.
    fn refresh(&self) -> impl Future<Output = crate::Result<()>> + Send;

    /// Navigate back in browser history.
    fn go_back(&self) -> impl Future<Output = crate::Result<()>> + Send;

    /// Navigate forward in browser history.
    fn go_forward(&self) -> impl Future<Output = crate::Result<()>> + Send;

    /// Return the current page title.
    fn title(&mut self) -> impl Future<Output = crate::Result<String>> + Send;

    /// Return the current page URL.
    fn url(&mut self) -> impl Future<Output = crate::Result<String>> + Send;
}

/// Element finding and interaction operations.
///
/// This trait is sealed and cannot be implemented for types outside `seleniumbase-rs`.
pub trait ElementApi: private::EngineCapability {
    /// Click the element matching `selector`.
    fn click(
        &mut self,
        selector: impl Into<Selector> + Send,
    ) -> impl Future<Output = crate::Result<()>> + Send;

    /// Double-click the element matching `selector`.
    fn double_click(
        &mut self,
        selector: impl Into<Selector> + Send,
    ) -> impl Future<Output = crate::Result<()>> + Send;

    /// Type `text` into the element matching `selector`.
    fn type_text(
        &mut self,
        selector: impl Into<Selector> + Send,
        text: &str,
    ) -> impl Future<Output = crate::Result<()>> + Send;

    /// Return the visible text of the element matching `selector`.
    fn text(
        &mut self,
        selector: impl Into<Selector> + Send,
    ) -> impl Future<Output = crate::Result<String>> + Send;

    /// Return the value of attribute `attr` on the element matching `selector`, if any.
    fn attribute(
        &mut self,
        selector: impl Into<Selector> + Send,
        attr: &str,
    ) -> impl Future<Output = crate::Result<Option<String>>> + Send;
}

/// Assertion helpers used in tests.
///
/// This trait is sealed and cannot be implemented for types outside `seleniumbase-rs`.
pub trait AssertionApi: private::EngineCapability {
    /// Assert that the page title equals `expected`.
    fn assert_title(&mut self, expected: &str) -> impl Future<Output = crate::Result<()>> + Send;

    /// Assert that the element matching `selector` is visible, waiting for it as
    /// Python's `assert_element` does.
    fn assert_element(
        &self,
        selector: impl Into<Selector> + Send,
    ) -> impl Future<Output = crate::Result<()>> + Send;

    /// Assert that `expected` text appears inside the element matching `selector`.
    /// The text comes first and the selector second, as in Python.
    fn assert_text(
        &mut self,
        expected: &str,
        selector: impl Into<Selector> + Send,
    ) -> impl Future<Output = crate::Result<()>> + Send;

    /// Assert that no JavaScript errors were logged on the current page.
    fn assert_no_js_errors(&self) -> impl Future<Output = crate::Result<()>> + Send;
}

/// Screenshot capture operations.
///
/// This trait is sealed and cannot be implemented for types outside `seleniumbase-rs`.
pub trait ScreenshotApi: private::EngineCapability {
    /// Save a screenshot to the logs directory with `filename`.
    fn save_screenshot(
        &self,
        filename: &str,
    ) -> impl Future<Output = crate::Result<PathBuf>> + Send;

    /// Return the current page screenshot as PNG bytes.
    fn screenshot_as_png(&self) -> impl Future<Output = crate::Result<Vec<u8>>> + Send;
}

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

    async fn title(&mut self) -> crate::Result<String> {
        BaseCase::title(self).await
    }

    async fn url(&mut self) -> crate::Result<String> {
        BaseCase::url(self).await
    }
}

impl ElementApi for BaseCase {
    async fn click(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<()> {
        BaseCase::click(self, selector).await
    }

    async fn double_click(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<()> {
        BaseCase::double_click(self, selector).await
    }

    async fn type_text(
        &mut self,
        selector: impl Into<Selector> + Send,
        text: &str,
    ) -> crate::Result<()> {
        BaseCase::type_text(self, selector, text).await
    }

    async fn text(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<String> {
        BaseCase::text(self, selector).await
    }

    async fn attribute(
        &mut self,
        selector: impl Into<Selector> + Send,
        attr: &str,
    ) -> crate::Result<Option<String>> {
        BaseCase::attribute(self, selector, attr).await
    }
}

impl AssertionApi for BaseCase {
    async fn assert_title(&mut self, expected: &str) -> crate::Result<()> {
        BaseCase::assert_title(self, expected).await
    }

    async fn assert_element(&self, selector: impl Into<Selector> + Send) -> crate::Result<()> {
        BaseCase::assert_element(self, selector).await
    }

    async fn assert_text(
        &mut self,
        expected: &str,
        selector: impl Into<Selector> + Send,
    ) -> crate::Result<()> {
        BaseCase::assert_text(self, expected, selector).await
    }

    async fn assert_no_js_errors(&self) -> crate::Result<()> {
        BaseCase::assert_no_js_errors(self).await
    }
}

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

    async fn title(&mut self) -> crate::Result<String> {
        Page::title(self).await
    }

    async fn url(&mut self) -> crate::Result<String> {
        Page::url(self).await
    }
}

impl ElementApi for Page {
    async fn click(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<()> {
        self.locator(selector).click().await
    }

    async fn double_click(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<()> {
        self.locator(selector).double_click().await
    }

    /// Replaces what is in the field, typing one key at a time.
    async fn type_text(
        &mut self,
        selector: impl Into<Selector> + Send,
        text: &str,
    ) -> crate::Result<()> {
        let field = self.locator(selector);
        field.clear().await?;
        field.type_text(text).await
    }

    async fn text(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<String> {
        self.locator(selector).text().await
    }

    async fn attribute(
        &mut self,
        selector: impl Into<Selector> + Send,
        attr: &str,
    ) -> crate::Result<Option<String>> {
        self.locator(selector).attribute(attr).await
    }
}

impl AssertionApi for Page {
    async fn assert_title(&mut self, expected: &str) -> crate::Result<()> {
        self.expect().to_have_title(expected).await
    }

    async fn assert_element(&self, selector: impl Into<Selector> + Send) -> crate::Result<()> {
        self.locator(selector).expect().to_be_visible().await
    }

    async fn assert_text(
        &mut self,
        expected: &str,
        selector: impl Into<Selector> + Send,
    ) -> crate::Result<()> {
        self.locator(selector)
            .expect()
            .to_contain_text(expected)
            .await
    }

    async fn assert_no_js_errors(&self) -> crate::Result<()> {
        Page::assert_no_js_errors(self).await
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// An engine that only writes down what it was asked to do.
    #[derive(Default)]
    struct Recorder {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl private::EngineCapability for Recorder {}

    impl Recorder {
        fn note(&self, call: String) {
            self.calls
                .lock()
                .expect("the recorder's lock is not poisoned")
                .push(call);
        }
    }

    impl BrowserApi for Recorder {
        async fn open(&mut self, url: &str) -> crate::Result<()> {
            self.note(format!("open {url}"));
            Ok(())
        }

        async fn quit(&mut self) -> crate::Result<()> {
            self.note("quit".to_owned());
            Ok(())
        }

        async fn refresh(&self) -> crate::Result<()> {
            self.note("refresh".to_owned());
            Ok(())
        }

        async fn go_back(&self) -> crate::Result<()> {
            self.note("go_back".to_owned());
            Ok(())
        }

        async fn go_forward(&self) -> crate::Result<()> {
            self.note("go_forward".to_owned());
            Ok(())
        }

        async fn title(&mut self) -> crate::Result<String> {
            Ok("Dashboard".to_owned())
        }

        async fn url(&mut self) -> crate::Result<String> {
            Ok("https://example.com/dashboard".to_owned())
        }
    }

    impl ElementApi for Recorder {
        async fn click(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<()> {
            self.note(format!("click {}", selector.into()));
            Ok(())
        }

        async fn double_click(
            &mut self,
            selector: impl Into<Selector> + Send,
        ) -> crate::Result<()> {
            self.note(format!("double_click {}", selector.into()));
            Ok(())
        }

        async fn type_text(
            &mut self,
            selector: impl Into<Selector> + Send,
            text: &str,
        ) -> crate::Result<()> {
            self.note(format!("type_text {} {text}", selector.into()));
            Ok(())
        }

        async fn text(&mut self, selector: impl Into<Selector> + Send) -> crate::Result<String> {
            Ok(selector.into().to_string())
        }

        async fn attribute(
            &mut self,
            _selector: impl Into<Selector> + Send,
            attr: &str,
        ) -> crate::Result<Option<String>> {
            Ok(Some(attr.to_owned()))
        }
    }

    impl AssertionApi for Recorder {
        async fn assert_title(&mut self, expected: &str) -> crate::Result<()> {
            self.note(format!("assert_title {expected}"));
            Ok(())
        }

        async fn assert_element(&self, selector: impl Into<Selector> + Send) -> crate::Result<()> {
            self.note(format!("assert_element {}", selector.into()));
            Ok(())
        }

        async fn assert_text(
            &mut self,
            expected: &str,
            selector: impl Into<Selector> + Send,
        ) -> crate::Result<()> {
            self.note(format!("assert_text {expected} {}", selector.into()));
            Ok(())
        }

        async fn assert_no_js_errors(&self) -> crate::Result<()> {
            self.note("assert_no_js_errors".to_owned());
            Ok(())
        }
    }

    impl ScreenshotApi for Recorder {
        async fn save_screenshot(&self, filename: &str) -> crate::Result<PathBuf> {
            self.note(format!("save_screenshot {filename}"));
            Ok(PathBuf::from(filename))
        }

        async fn screenshot_as_png(&self) -> crate::Result<Vec<u8>> {
            Ok(b"\x89PNG".to_vec())
        }
    }

    /// A helper written once over the traits.
    async fn sign_in<E>(sb: &mut E) -> crate::Result<()>
    where
        E: BrowserApi + ElementApi + AssertionApi,
    {
        sb.open("https://example.com/login").await?;
        sb.type_text("#user", "al").await?;
        sb.click("#submit").await?;
        sb.assert_title("Dashboard").await
    }

    /// Spawns the helper for any engine that can be sent to another thread.
    fn spawn_sign_in<E>(mut sb: E) -> tokio::task::JoinHandle<crate::Result<()>>
    where
        E: BrowserApi + ElementApi + AssertionApi + Send + 'static,
    {
        tokio::spawn(async move { sign_in(&mut sb).await })
    }

    /// The same for the traits whose methods take `&self`.
    fn spawn_checks<E>(sb: Arc<E>) -> tokio::task::JoinHandle<crate::Result<()>>
    where
        E: AssertionApi + ScreenshotApi + Send + Sync + 'static,
    {
        tokio::spawn(async move {
            sb.assert_element("#dashboard").await?;
            sb.assert_no_js_errors().await?;
            sb.save_screenshot("shot.png").await?;
            sb.screenshot_as_png().await.map(|_| ())
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_type_implements_the_traits_with_plain_async_fns() {
        let mut engine = Recorder::default();
        let calls = Arc::clone(&engine.calls);

        sign_in(&mut engine).await.unwrap();

        assert_eq!(
            *calls.lock().unwrap(),
            [
                "open https://example.com/login",
                "type_text #user al",
                "click #submit",
                "assert_title Dashboard",
            ]
        );
        assert_eq!(engine.title().await.unwrap(), "Dashboard");
        assert_eq!(engine.text("#heading").await.unwrap(), "#heading");
        assert_eq!(
            engine.attribute("a", "href").await.unwrap().as_deref(),
            Some("href")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_helper_over_the_traits_can_be_spawned() {
        let engine = Recorder::default();
        let calls = Arc::clone(&engine.calls);

        spawn_sign_in(engine)
            .await
            .expect("the task was not cancelled")
            .unwrap();

        assert_eq!(calls.lock().unwrap().len(), 4);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_read_only_methods_can_be_spawned_on_a_shared_engine() {
        let engine = Arc::new(Recorder::default());

        spawn_checks(Arc::clone(&engine))
            .await
            .expect("the task was not cancelled")
            .unwrap();

        assert_eq!(
            *engine.calls.lock().unwrap(),
            [
                "assert_element #dashboard",
                "assert_no_js_errors",
                "save_screenshot shot.png",
            ]
        );
    }
}
