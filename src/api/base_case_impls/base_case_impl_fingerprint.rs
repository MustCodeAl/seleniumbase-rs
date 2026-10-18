// Capturing and comparing the fingerprint the browser shows a page. Included at the end of
// `base_case.rs`, like the files beside it; the pure CDP engine's twin is
// `Page::capture_fingerprint`.

impl BaseCase {
    /// Reads what the browser shows the page it has open, as a
    /// [`CapturedFingerprint`](crate::stealth::capture::CapturedFingerprint).
    ///
    /// Runs the capture script as one asynchronous script (WebDriver's *Execute
    /// Async Script*) and maps its readings to a
    /// [`Fingerprint`](crate::stealth::fingerprint::Fingerprint) with the same code the
    /// pure CDP engine uses ([`Page::capture_fingerprint`](crate::sb_cdp::Page::capture_fingerprint)),
    /// so a browser gives the same fingerprint through either. The script asks for no
    /// permission and changes nothing in the page.
    ///
    /// What the page shows depends on the page: an insecure (`http:`) page has no
    /// Client Hints, media devices, battery or Bluetooth, and the capture then leaves
    /// those settings `Natural` and says so under `unanswered`. Open an `https:` page, or
    /// `about:blank`, first for the whole fingerprint.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::WebDriver`] if the driver refuses the script,
    /// [`SeleniumBaseError::Stealth`] if the script failed or its answer is not a
    /// reading this build knows.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::{BaseCase, BrowserConfig};
    ///
    /// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let mut sb = BaseCase::new(BrowserConfig::default().with_headless(true)).await?;
    /// sb.open("https://example.com").await?;
    ///
    /// let captured = sb.capture_fingerprint().await?;
    /// captured.fingerprint.save("this-chrome.json")?;
    /// sb.quit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn capture_fingerprint(
        &self,
    ) -> Result<crate::stealth::capture::CapturedFingerprint, SeleniumBaseError> {
        let readings = self
            .session
            .execute_async_script(&crate::stealth::capture::capture_async_script())
            .await?;
        crate::stealth::capture::CapturedFingerprint::from_probe(
            &readings,
            crate::stealth::capture::timestamp(),
        )
    }

    /// Whether the browser presents `expected`: captures what the page shows and compares
    /// it with the fingerprint.
    ///
    /// See [`Fingerprint::compare_to`](crate::stealth::fingerprint::Fingerprint::compare_to)
    /// for what is compared and what is listed as unverifiable. A WebDriver session
    /// without `DriverMode::Uc` or `DriverMode::Cdp` applies only the launch arguments of a
    /// fingerprint, so the comparison of such a session lists what the page does not show.
    ///
    /// # Errors
    ///
    /// As [`capture_fingerprint`](Self::capture_fingerprint).
    pub async fn compare_fingerprint(
        &self,
        expected: &crate::stealth::fingerprint::Fingerprint,
    ) -> Result<crate::stealth::fingerprint::FingerprintComparison, SeleniumBaseError> {
        let captured = self.capture_fingerprint().await?;
        Ok(expected.compare_to(&captured.fingerprint))
    }
}

#[cfg(test)]
mod tests_fingerprint_capture {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use crate::stealth::fingerprint::{Fingerprint, OsType};
    use serde_json::json;

    const CHROME_MACOS: &str = include_str!("../../../tests/fixtures/fingerprint/probe_chrome_macos.json");

    /// A WebDriver whose asynchronous scripts answer `readings`.
    fn driver_that_reads(readings: &str) -> MockWebDriver {
        let readings: Value = serde_json::from_str(readings).expect("a fixture is JSON");
        MockWebDriver::start(json!({}), move |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/async") => Reply::Value(readings.clone()),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    #[tokio::test]
    async fn a_capture_runs_one_asynchronous_script_and_maps_its_readings() {
        let driver = driver_that_reads(CHROME_MACOS);
        let sb = driver.base_case().await;

        let captured = sb.capture_fingerprint().await.unwrap();

        let scripts = driver.requests("/execute/async");
        assert_eq!(scripts.len(), 1, "{scripts:?}");
        let script = scripts[0].body["script"].as_str().unwrap();
        assert!(script.starts_with("var sbrsDone = arguments[arguments.length - 1];"));
        assert!(script.contains("getHighEntropyValues"));
        assert_eq!(scripts[0].body["args"], json!([]), "the callback is WebDriver's own");
        let fp = &captured.fingerprint;
        assert_eq!(fp.platform().as_str(), Some("MacIntel"));
        assert_eq!(fp.os_type(), OsType::Macos);
        assert_eq!(fp.core_version(), Some(155));
        assert_eq!(captured.probe.url, "https://example.test/");
        assert_eq!(captured.probe.browser_version.as_deref(), Some("155.0.8059.40"));
    }

    #[tokio::test]
    async fn both_engines_map_the_same_readings_to_the_same_fingerprint() {
        // The pure CDP engine hands the same value to the same function.
        let driver = driver_that_reads(CHROME_MACOS);
        let sb = driver.base_case().await;
        let raw: Value = serde_json::from_str(CHROME_MACOS).unwrap();

        let through_webdriver = sb.capture_fingerprint().await.unwrap();
        let direct =
            crate::stealth::capture::CapturedFingerprint::from_probe(&raw, "any time").unwrap();

        assert_eq!(through_webdriver.fingerprint, direct.fingerprint);
        assert_eq!(through_webdriver.unmapped, direct.unmapped);
    }

    #[tokio::test]
    async fn a_failure_the_script_reports_is_a_stealth_error() {
        let driver = driver_that_reads(r#"{ "version": 1, "failure": "ReferenceError: boom" }"#);
        let sb = driver.base_case().await;

        let error = sb.capture_fingerprint().await.unwrap_err();

        assert!(matches!(error, SeleniumBaseError::Stealth { .. }), "{error:?}");
        assert!(error.to_string().contains("ReferenceError: boom"), "{error}");
    }

    #[tokio::test]
    async fn a_driver_that_refuses_the_script_is_a_webdriver_error() {
        let driver = MockWebDriver::start(json!({}), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/execute/async") => Reply::Error(500, "javascript error"),
                _ => Reply::Value(Value::Null),
            }
        });
        let sb = driver.base_case().await;

        let error = sb.capture_fingerprint().await.unwrap_err();

        assert!(matches!(error, SeleniumBaseError::WebDriver(_)), "{error:?}");
        assert_eq!(error.exit_code(), 4);
    }

    #[tokio::test]
    async fn comparing_with_the_browser_lists_what_differs() {
        let driver = driver_that_reads(CHROME_MACOS);
        let sb = driver.base_case().await;
        let claimed = Fingerprint::macos_desktop();

        let comparison = sb.compare_fingerprint(&claimed).await.unwrap();

        // The preset claims Los Angeles; the browser is in Berlin.
        let differing: Vec<_> = comparison.differences.iter().map(|d| d.setting).collect();
        assert!(differing.contains(&"timezone"), "{differing:?}");
        assert!(comparison.matched.contains(&"platform"));
        assert!(comparison.matched.contains(&"hardware_concurrency"));
    }
}
