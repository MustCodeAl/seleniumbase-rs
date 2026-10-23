//! `Page::breakpoint`: Python's `breakpoint()` for a tab driven over CDP, and
//! the tab as the debug prompt sees it.
//!
//! The prompt itself is [`DebugPrompt`]; this module only connects it to a
//! [`Page`].

use std::io::Write;

use async_trait::async_trait;
use serde_json::Value;

use super::Page;
use crate::common::debug_prompt::{
    breakpoint_prompt, thread_test_id, DebugPrompt, LineInput, PromptKind, PromptOutcome,
    PromptPage,
};
use crate::error::SeleniumBaseError;

#[async_trait]
impl PromptPage for Page {
    async fn url(&self) -> Result<String, SeleniumBaseError> {
        Page::url(self).await
    }

    async fn title(&self) -> Result<String, SeleniumBaseError> {
        Page::title(self).await
    }

    async fn screenshot_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        Page::screenshot(self).await
    }

    async fn page_source(&self) -> Result<String, SeleniumBaseError> {
        Page::content(self).await
    }

    async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
        Page::evaluate(self, expression).await
    }
}

impl Page {
    /// Pauses the test with the browser open, as Python's `breakpoint()` does.
    ///
    /// A prompt is written to standard error and the test waits for a command:
    /// `c` goes on, `s` saves a screenshot and `src` the page source to
    /// `latest_logs/<test id>/`, `u` shows the address and title, `js <expr>`
    /// evaluates an expression, `q` stops the test and `h` lists these. Work in
    /// the browser meanwhile.
    ///
    /// The pause is skipped, with a warning in the log, when standard input
    /// is not a terminal (CI, a pipe, `cargo nextest`), and silently when the
    /// environment variable `SB_BREAKPOINTS` is `0`, so a breakpoint left in a
    /// test never hangs a run. In a terminal, run
    /// `cargo test -- --test-threads 1 --nocapture` so the answers reach this
    /// test.
    ///
    /// ```no_run
    /// use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
    ///
    /// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let browser = Browser::launch(LaunchOptions::default()).await?;
    /// let page = browser.default_page().await?;
    /// page.goto("https://example.com").await?;
    /// page.breakpoint("Paused at the home page.").await?;
    /// browser.close().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::TestLifecycle`] when the person quits the
    /// prompt (`q`), and [`SeleniumBaseError::Io`] when the terminal cannot be
    /// read or written.
    pub async fn breakpoint(&self, message: &str) -> Result<(), SeleniumBaseError> {
        let Some(mut prompt) = breakpoint_prompt() else {
            return Ok(());
        };
        self.pause_at(&mut prompt, PromptKind::Breakpoint, message)
            .await
    }

    /// Pauses at `prompt` as `kind` and turns a quit into an error.
    pub(crate) async fn pause_at<I, W>(
        &self,
        prompt: &mut DebugPrompt<I, W>,
        kind: PromptKind,
        message: &str,
    ) -> Result<(), SeleniumBaseError>
    where
        I: LineInput,
        W: Write + Send,
    {
        match prompt.pause(kind, &thread_test_id(), message, self).await? {
            PromptOutcome::Continue => Ok(()),
            PromptOutcome::Quit => Err(SeleniumBaseError::test_lifecycle(
                "stopped at the debug prompt",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use base64::prelude::*;
    use serde_json::json;

    use super::*;
    use crate::sb_cdp::Browser;

    const PNG: &[u8] = b"\x89PNG-from-the-mock";

    async fn mocked_page() -> (Browser, Page) {
        let (browser, mock) = Browser::new_mocked();
        mock.reply(
            "Runtime.evaluate",
            json!({ "result": { "value": "Example Domain" } }),
        );
        mock.reply(
            "Page.captureScreenshot",
            json!({ "data": BASE64_STANDARD.encode(PNG) }),
        );
        let page = browser.default_page().await.unwrap();
        (browser, page)
    }

    fn at_prompt(
        script: &'static str,
        logs: &std::path::Path,
    ) -> DebugPrompt<crate::common::debug_prompt::ReaderInput<&'static [u8]>, Vec<u8>> {
        DebugPrompt::from_reader(script.as_bytes(), Vec::new()).with_logs_dir(logs)
    }

    #[tokio::test]
    async fn the_prompt_reads_and_saves_what_the_tab_shows() {
        let (_browser, page) = mocked_page().await;
        let logs = tempfile::tempdir().unwrap();
        let mut prompt = at_prompt("u\ns\nsrc\njs document.title\nc\n", logs.path());

        page.pause_at(&mut prompt, PromptKind::Breakpoint, "look")
            .await
            .unwrap();

        let text = String::from_utf8_lossy(prompt.output()).into_owned();
        assert!(text.contains("[BREAKPOINT]: look"), "{text}");
        assert!(
            text.contains("page: Example Domain  \"Example Domain\""),
            "{text}"
        );
        assert!(
            text.contains("\"Example Domain\"\n"),
            "the js value is printed: {text}"
        );
        let folder = std::fs::read_dir(logs.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(folder.join("breakpoint_1.png")).unwrap(), PNG);
        assert_eq!(
            std::fs::read_to_string(folder.join("page_source_1.html")).unwrap(),
            "Example Domain"
        );
    }

    #[tokio::test]
    async fn quitting_the_prompt_is_a_test_failure() {
        let (_browser, page) = mocked_page().await;
        let logs = tempfile::tempdir().unwrap();
        let mut prompt = at_prompt("q\n", logs.path());

        let error = page
            .pause_at(&mut prompt, PromptKind::Breakpoint, "m")
            .await
            .unwrap_err();

        assert!(
            matches!(&error, SeleniumBaseError::TestLifecycle(text)
                if text == "stopped at the debug prompt"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn without_a_terminal_a_breakpoint_goes_on_at_once() {
        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() {
            return;
        }
        let (_browser, page) = mocked_page().await;

        page.breakpoint("nobody is here").await.unwrap();
    }
}
