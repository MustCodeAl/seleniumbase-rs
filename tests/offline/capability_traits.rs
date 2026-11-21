//! The capability traits are native `async fn` traits.
//!
//! A type implements them with a plain `async fn` and no attribute macro, and a
//! helper written over them can be spawned onto a multi-threaded runtime, because
//! the future of every method is `Send`. The engines (`BaseCase`, `sb_cdp::Page`)
//! are covered by their own tests; this file uses an engine of its own, so it
//! needs no browser and no feature.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use seleniumbase_rs::utils::selectors::SelectorBuf;
use seleniumbase_rs::{AssertionApi, BrowserApi, ElementApi, ScreenshotApi};

/// An engine that only writes down what it was asked to do.
#[derive(Default)]
struct Recorder {
    calls: Arc<Mutex<Vec<String>>>,
}

impl Recorder {
    fn note(&self, call: String) {
        self.calls
            .lock()
            .expect("the recorder's lock is not poisoned")
            .push(call);
    }
}

impl BrowserApi for Recorder {
    async fn open(&mut self, url: &str) -> seleniumbase_rs::Result<()> {
        self.note(format!("open {url}"));
        Ok(())
    }

    async fn quit(&mut self) -> seleniumbase_rs::Result<()> {
        self.note("quit".to_owned());
        Ok(())
    }

    async fn refresh(&self) -> seleniumbase_rs::Result<()> {
        self.note("refresh".to_owned());
        Ok(())
    }

    async fn go_back(&self) -> seleniumbase_rs::Result<()> {
        self.note("go_back".to_owned());
        Ok(())
    }

    async fn go_forward(&self) -> seleniumbase_rs::Result<()> {
        self.note("go_forward".to_owned());
        Ok(())
    }

    async fn title(&mut self) -> seleniumbase_rs::Result<String> {
        Ok("Dashboard".to_owned())
    }

    async fn url(&mut self) -> seleniumbase_rs::Result<String> {
        Ok("https://example.com/dashboard".to_owned())
    }
}

impl ElementApi for Recorder {
    async fn click(
        &mut self,
        selector: impl Into<SelectorBuf> + Send,
    ) -> seleniumbase_rs::Result<()> {
        self.note(format!("click {}", selector.into()));
        Ok(())
    }

    async fn double_click(
        &mut self,
        selector: impl Into<SelectorBuf> + Send,
    ) -> seleniumbase_rs::Result<()> {
        self.note(format!("double_click {}", selector.into()));
        Ok(())
    }

    async fn type_text(
        &mut self,
        selector: impl Into<SelectorBuf> + Send,
        text: &str,
    ) -> seleniumbase_rs::Result<()> {
        self.note(format!("type_text {} {text}", selector.into()));
        Ok(())
    }

    async fn text(
        &mut self,
        selector: impl Into<SelectorBuf> + Send,
    ) -> seleniumbase_rs::Result<String> {
        Ok(selector.into().to_string())
    }

    async fn attribute(
        &mut self,
        _selector: impl Into<SelectorBuf> + Send,
        attr: &str,
    ) -> seleniumbase_rs::Result<Option<String>> {
        Ok(Some(attr.to_owned()))
    }
}

impl AssertionApi for Recorder {
    async fn assert_title(&mut self, expected: &str) -> seleniumbase_rs::Result<()> {
        self.note(format!("assert_title {expected}"));
        Ok(())
    }

    async fn assert_element(
        &self,
        selector: impl Into<SelectorBuf> + Send,
    ) -> seleniumbase_rs::Result<()> {
        self.note(format!("assert_element {}", selector.into()));
        Ok(())
    }

    async fn assert_text(
        &mut self,
        expected: &str,
        selector: impl Into<SelectorBuf> + Send,
    ) -> seleniumbase_rs::Result<()> {
        self.note(format!("assert_text {expected} {}", selector.into()));
        Ok(())
    }

    async fn assert_no_js_errors(&self) -> seleniumbase_rs::Result<()> {
        self.note("assert_no_js_errors".to_owned());
        Ok(())
    }
}

impl ScreenshotApi for Recorder {
    async fn save_screenshot(&self, filename: &str) -> seleniumbase_rs::Result<PathBuf> {
        self.note(format!("save_screenshot {filename}"));
        Ok(PathBuf::from(filename))
    }

    async fn screenshot_as_png(&self) -> seleniumbase_rs::Result<Vec<u8>> {
        Ok(b"\x89PNG".to_vec())
    }
}

/// A helper written once over the traits.
async fn sign_in<E>(sb: &mut E) -> seleniumbase_rs::Result<()>
where
    E: BrowserApi + ElementApi + AssertionApi,
{
    sb.open("https://example.com/login").await?;
    sb.type_text("#user", "al").await?;
    sb.click("#submit").await?;
    sb.assert_title("Dashboard").await
}

/// Spawns the helper for any engine that can be sent to another thread. This
/// compiles only while the future of every trait method is `Send`: the engine
/// is a type parameter here, so the compiler can rely on nothing but the bound
/// the traits declare.
fn spawn_sign_in<E>(mut sb: E) -> tokio::task::JoinHandle<seleniumbase_rs::Result<()>>
where
    E: BrowserApi + ElementApi + AssertionApi + Send + 'static,
{
    tokio::spawn(async move { sign_in(&mut sb).await })
}

/// The same for the traits whose methods take `&self`.
fn spawn_checks<E>(sb: Arc<E>) -> tokio::task::JoinHandle<seleniumbase_rs::Result<()>>
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
