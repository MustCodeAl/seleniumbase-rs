// Recordings as pure CDP code (Python's `--rec-sb-cdp`, `rec_sb_cdp`).
// Included in `base_case.rs` like the files beside it.

impl BaseCase {
    /// Whether recordings are written as pure CDP code
    /// ([`BrowserConfig::with_rec_sb_cdp`]).
    #[must_use]
    pub fn rec_sb_cdp(&self) -> bool {
        self.config.rec_sb_cdp()
    }

    /// The recording as a standalone `#[tokio::main]` program for the pure CDP
    /// engine ([`ActionRecorder::to_cdp_script`]), whatever the
    /// [`rec_sb_cdp`](Self::rec_sb_cdp) setting is.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] if the recorder's lock is
    /// poisoned.
    pub fn export_recording_as_cdp_rust(&self) -> Result<String, SeleniumBaseError> {
        let recorder = self
            .recorder
            .lock()
            .map_err(|_| SeleniumBaseError::Unsupported("recorder mutex poisoned".to_owned()))?;
        Ok(recorder.to_cdp_script())
    }
}

#[cfg(test)]
mod tests_rec_sb_cdp {
    use super::webdriver_support::{element, MockWebDriver, Reply};
    use super::*;
    use serde_json::json;

    /// A case on a mock page that has recorded an `open` and a `click`.
    async fn recorded(rec_sb_cdp: bool) -> BaseCase {
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/element") => Reply::Value(element("e1")),
                // `rec_sb_cdp` turns Recorder Mode on, which reads the page's URL first.
                ("GET", "/url") => Reply::Value(json!("about:blank")),
                ("GET", "/window") => Reply::Value(json!("H1")),
                ("GET", "/window/handles") => Reply::Value(json!(["H1"])),
                ("GET", "/element/e1/displayed" | "/element/e1/enabled") => {
                    Reply::Value(json!(true))
                }
                _ => Reply::Value(Value::Null),
            }
        });
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url().to_owned())
            .with_rec_sb_cdp(rec_sb_cdp);
        config.auto_start_driver = false;
        let mut sb = BaseCase::new(config)
            .await
            .expect("the mock WebDriver accepts a session");
        sb.open("https://example.test/").await.unwrap();
        sb.click("#go").await.unwrap();
        sb
    }

    #[tokio::test]
    async fn a_recording_is_written_for_basecase_unless_pure_cdp_is_asked_for() {
        let sb = recorded(false).await;
        assert!(!sb.rec_sb_cdp());

        let script = sb.export_recording_as_rust().unwrap();

        assert!(script.contains("sb.open(\"https://example.test/\").await?;"), "{script}");
        assert!(script.contains("sb.click(\"#go\").await?;"), "{script}");
        assert!(!script.contains("sb_cdp"), "{script}");
    }

    #[tokio::test]
    async fn with_rec_sb_cdp_the_recording_is_written_as_pure_cdp_code() {
        let sb = recorded(true).await;
        assert!(sb.rec_sb_cdp());

        let script = sb.export_recording_as_rust().unwrap();

        assert!(script.starts_with("use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};\n"), "{script}");
        assert!(
            script.contains("    page.goto(\"https://example.test/\").await?;\n    page.locator(\"#go\").click().await?;\n"),
            "{script}"
        );
        assert!(!script.contains("sb.open"), "{script}");
        assert_eq!(script, sb.export_recording_as_cdp_rust().unwrap());
    }

    #[tokio::test]
    async fn the_pure_cdp_export_can_be_asked_for_by_name_without_the_setting() {
        let sb = recorded(false).await;

        let script = sb.export_recording_as_cdp_rust().unwrap();

        assert!(script.contains("page.locator(\"#go\").click().await?;"), "{script}");
        assert!(
            sb.export_recording_as_rust().unwrap().contains("sb.click"),
            "the setting is still off"
        );
    }
}
