//! Capturing the fingerprint a tab shows a page.
//!
//! [`Page::capture_fingerprint`] runs the capture script in the tab and maps
//! what it reads to a [`Fingerprint`](crate::Fingerprint); the same script and
//! the same mapping serve the WebDriver engine
//! ([`BaseCase::capture_fingerprint`](crate::BaseCase::capture_fingerprint)).
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let browser = Browser::launch(LaunchOptions::default()).await?;
//! let page = browser.default_page().await?;
//! page.goto("https://example.com").await?;
//!
//! let captured = page.capture_fingerprint().await?;
//! captured.fingerprint.save("real-chrome.json")?;
//! # Ok(())
//! # }
//! ```

use super::Page;
use crate::error::SeleniumBaseError;
use crate::stealth::capture::{capture_expression, timestamp, CapturedFingerprint};
use crate::stealth::fingerprint::{Fingerprint, FingerprintComparison};

impl Page {
    /// Reads what this tab shows a page, as a [`CapturedFingerprint`].
    ///
    /// Runs the capture script in the page the tab has open now: the navigator
    /// and the Client Hints, the screen, the time zone, the WebGL strings, the
    /// media devices, the battery, the connection, the speech voices and
    /// Bluetooth, plus the readings that no setting holds (see
    /// [`CapturedFingerprint::unmapped`]). It sends one `Runtime.evaluate` and
    /// nothing else, asks for no permission and changes nothing in the page.
    ///
    /// What the page shows depends on the page: an insecure (`http:`) page has no
    /// Client Hints, media devices, battery or Bluetooth, and the capture then
    /// leaves those settings `Natural` and says so under `unanswered`. Capture
    /// from an `https:` page, or `about:blank`, for the whole fingerprint.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] if the script throws or the browser
    /// refuses it, [`SeleniumBaseError::Stealth`] if its answer is not a reading
    /// this build knows.
    pub async fn capture_fingerprint(&self) -> Result<CapturedFingerprint, SeleniumBaseError> {
        let readings = self.evaluate(capture_expression()).await?;
        CapturedFingerprint::from_probe(&readings, timestamp())
    }

    /// Whether this tab presents `expected`: captures what the page shows and
    /// compares it with the fingerprint.
    ///
    /// See [`Fingerprint::compare_to`] for what is compared and what is listed as
    /// unverifiable.
    ///
    /// # Errors
    ///
    /// As [`capture_fingerprint`](Self::capture_fingerprint).
    pub async fn compare_fingerprint(
        &self,
        expected: &Fingerprint,
    ) -> Result<FingerprintComparison, SeleniumBaseError> {
        let captured = self.capture_fingerprint().await?;
        Ok(expected.compare_to(&captured.fingerprint))
    }
}
