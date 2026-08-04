use std::time::Duration;
use thiserror::Error;
use tracing;

/// Shorthand result type used throughout the crate.
pub type Result<T> = std::result::Result<T, SeleniumBaseError>;

/// Errors that can occur when using SeleniumBase.
///
/// Existing tuple variants are preserved for backwards compatibility. New
/// structured variants carry additional context (selectors, URLs, binary
/// paths, durations, HTTP status codes) so errors are actionable for both
/// humans and retry logic.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SeleniumBaseError {
    // ------------------------------------------------------------------
    // Element interactions (structured)
    // ------------------------------------------------------------------
    /// The requested element could not be located.
    #[error("element not found: {selector} (strategy: {strategy})")]
    ElementNotFound { selector: String, strategy: String },

    /// The element was located but is not currently interactable.
    #[error("element not interactable: {selector} ({reason})")]
    ElementNotInteractable { selector: String, reason: String },

    /// A previously located element became stale before it could be used.
    #[error("stale element reference: {selector}")]
    StaleElement { selector: String },

    /// The selector string itself is malformed or unsupported.
    #[error("invalid selector: {0}")]
    InvalidSelector(String),

    // ------------------------------------------------------------------
    // Waits and timeouts (backwards-compatible tuple + structured helper)
    // ------------------------------------------------------------------
    /// A wait condition timed out.
    #[error("wait timeout: {0}")]
    WaitTimeout(String),

    // ------------------------------------------------------------------
    // Browser lifecycle (structured)
    // ------------------------------------------------------------------
    /// The browser process could not be started.
    #[error("failed to launch browser binary '{binary}': {reason}")]
    BrowserLaunch { binary: String, reason: String },

    /// The browser session disconnected unexpectedly.
    #[error("browser disconnected: {reason}")]
    BrowserDisconnected { reason: String },

    /// An operation was attempted before a session was started.
    #[error("no active browser session")]
    SessionNotStarted,

    /// Navigation to a URL failed.
    #[error("navigation to '{url}' failed: {reason}")]
    Navigation { url: String, reason: String },

    // ------------------------------------------------------------------
    // WebDriver / CDP / Playwright backends
    // ------------------------------------------------------------------
    /// The WebDriver backend returned an error.
    #[error("webdriver command failed: {0}")]
    WebDriver(#[from] thirtyfour::error::WebDriverError),

    /// The Chrome DevTools Protocol backend returned an error.
    #[error("CDP driver error: {0}")]
    CdpDriver(String),

    /// The Playwright backend returned an error.
    #[error("playwright error: {0}")]
    Playwright(String),

    // ------------------------------------------------------------------
    // Stealth and patching (structured)
    // ------------------------------------------------------------------
    /// A binary patch operation failed.
    #[error("binary patch failed for '{path}': {reason}")]
    Patcher { path: String, reason: String },

    /// A stealth / anti-detection operation failed.
    #[error("stealth error: {reason}")]
    Stealth { reason: String },

    // ------------------------------------------------------------------
    // Network and I/O
    // ------------------------------------------------------------------
    /// A network request failed.
    #[error("network request to '{url}' failed ({status}): {reason}")]
    Network {
        url: String,
        status: u16,
        reason: String,
    },

    /// A file download failed.
    #[error("download failed for '{url}': {reason}")]
    Download { url: String, reason: String },

    /// A generic I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // ------------------------------------------------------------------
    // Data parsing and serialization
    // ------------------------------------------------------------------
    /// A JSON serialization or deserialization error occurred.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Parsing an unstructured value failed.
    #[error("parse error: expected {expected}, got '{input}'")]
    Parse { input: String, expected: String },

    // ------------------------------------------------------------------
    // Configuration and assertions
    // ------------------------------------------------------------------
    /// The supplied configuration is invalid.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// An assertion failed.
    #[error("assertion failed: {0}")]
    AssertionFailed(String),

    /// The requested operation is not supported in the current configuration.
    #[error("unsupported operation: {0}")]
    Unsupported(String),

    // ------------------------------------------------------------------
    // Specialized subsystems
    // ------------------------------------------------------------------
    /// GUI input simulation failed.
    #[error("GUI input error: {0}")]
    Gui(String),

    /// Screenshot capture failed.
    #[error("screenshot error: {0}")]
    Screenshot(String),

    /// PDF generation or extraction failed.
    #[error("PDF error: {0}")]
    Pdf(String),

    /// Multi-factor authentication / TOTP error.
    #[error("authentication error: {0}")]
    Authentication(String),

    /// MCP server tool execution failed.
    #[error("MCP tool '{tool}' failed: {reason}")]
    Mcp { tool: String, reason: String },

    /// Python-to-Rust migration/import error.
    #[error("Python migration error: {0}")]
    PythonMigration(String),

    /// Test was explicitly skipped.
    #[error("test skipped: {0}")]
    Skipped(String),

    /// Browser test lifecycle helper encountered a failure.
    #[error("browser test lifecycle failed: {0}")]
    TestLifecycle(String),
}

impl SeleniumBaseError {
    // ------------------------------------------------------------------
    // Constructors
    // ------------------------------------------------------------------

    /// Construct a not-found error from a selector string.
    pub fn element_not_found(selector: impl Into<String>) -> Self {
        let selector = selector.into();
        Self::ElementNotFound {
            strategy: guess_strategy(&selector).to_owned(),
            selector,
        }
    }

    /// Construct a not-interactable error.
    pub fn element_not_interactable(
        selector: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::ElementNotInteractable {
            selector: selector.into(),
            reason: reason.into(),
        }
    }

    /// Construct a stale element error.
    pub fn stale_element(selector: impl Into<String>) -> Self {
        Self::StaleElement {
            selector: selector.into(),
        }
    }

    /// Construct a backwards-compatible wait-timeout error.
    pub fn wait_timeout(operation: impl Into<String>, duration: Option<Duration>) -> Self {
        let op = operation.into();
        match duration {
            Some(d) => Self::WaitTimeout(format!("{} (after {}ms)", op, d.as_millis())),
            None => Self::WaitTimeout(op),
        }
    }

    /// Construct a browser launch error.
    pub fn browser_launch(binary: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::BrowserLaunch {
            binary: binary.into(),
            reason: reason.into(),
        }
    }

    /// Construct a browser disconnected error.
    pub fn browser_disconnected(reason: impl Into<String>) -> Self {
        Self::BrowserDisconnected {
            reason: reason.into(),
        }
    }

    /// Construct a navigation error.
    pub fn navigation(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Navigation {
            url: url.into(),
            reason: reason.into(),
        }
    }

    /// Construct a patcher error for a binary path.
    pub fn patcher(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Patcher {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Construct a stealth operation error.
    pub fn stealth(reason: impl Into<String>) -> Self {
        Self::Stealth {
            reason: reason.into(),
        }
    }

    /// Construct a network error.
    pub fn network(url: impl Into<String>, status: u16, reason: impl Into<String>) -> Self {
        Self::Network {
            url: url.into(),
            status,
            reason: reason.into(),
        }
    }

    /// Construct a download error.
    pub fn download(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Download {
            url: url.into(),
            reason: reason.into(),
        }
    }

    /// Construct a parse error.
    pub fn parse(input: impl Into<String>, expected: impl Into<String>) -> Self {
        Self::Parse {
            input: input.into(),
            expected: expected.into(),
        }
    }

    /// Construct a screenshot error.
    pub fn screenshot(reason: impl Into<String>) -> Self {
        Self::Screenshot(reason.into())
    }

    /// Construct a PDF error.
    pub fn pdf(reason: impl Into<String>) -> Self {
        Self::Pdf(reason.into())
    }

    /// Construct an authentication error.
    pub fn authentication(reason: impl Into<String>) -> Self {
        Self::Authentication(reason.into())
    }

    /// Construct an MCP tool error.
    pub fn mcp(tool: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Mcp {
            tool: tool.into(),
            reason: reason.into(),
        }
    }

    /// Construct a Python migration error.
    pub fn python_migration(reason: impl Into<String>) -> Self {
        Self::PythonMigration(reason.into())
    }

    /// Construct a test-skipped error.
    pub fn skipped(reason: impl Into<String>) -> Self {
        Self::Skipped(reason.into())
    }

    /// Construct a test-lifecycle error.
    pub fn test_lifecycle(reason: impl Into<String>) -> Self {
        Self::TestLifecycle(reason.into())
    }

    /// Construct an invalid-config error.
    pub fn invalid_config(reason: impl Into<String>) -> Self {
        Self::InvalidConfig(reason.into())
    }

    /// Construct an assertion-failed error.
    pub fn assertion_failed(reason: impl Into<String>) -> Self {
        Self::AssertionFailed(reason.into())
    }

    /// Construct an unsupported-operation error.
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported(reason.into())
    }

    /// Construct a CDP driver error.
    pub fn cdp_driver(reason: impl Into<String>) -> Self {
        Self::CdpDriver(reason.into())
    }

    /// Construct a Playwright error.
    pub fn playwright(reason: impl Into<String>) -> Self {
        Self::Playwright(reason.into())
    }

    /// Construct a GUI error.
    pub fn gui(reason: impl Into<String>) -> Self {
        Self::Gui(reason.into())
    }

    // ------------------------------------------------------------------
    // Inspection helpers
    // ------------------------------------------------------------------

    /// Returns true if this error represents a recoverable transient condition.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::BrowserDisconnected { .. }
                | Self::Network { .. }
                | Self::Download { .. }
                | Self::WaitTimeout(_)
        )
    }

    /// Returns true if the test was explicitly skipped.
    pub fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped(_))
    }

    /// Returns true if the error is an element-related failure.
    pub fn is_element_error(&self) -> bool {
        matches!(
            self,
            Self::ElementNotFound { .. }
                | Self::ElementNotInteractable { .. }
                | Self::StaleElement { .. }
                | Self::InvalidSelector(_)
        )
    }

    /// Returns a short, stable category tag for metrics / dashboards.
    pub fn category(&self) -> &'static str {
        match self {
            Self::ElementNotFound { .. } => "element_not_found",
            Self::ElementNotInteractable { .. } => "element_not_interactable",
            Self::StaleElement { .. } => "stale_element",
            Self::InvalidSelector(_) => "invalid_selector",
            Self::WaitTimeout(_) => "wait_timeout",
            Self::BrowserLaunch { .. } => "browser_launch",
            Self::BrowserDisconnected { .. } => "browser_disconnected",
            Self::SessionNotStarted => "session_not_started",
            Self::Navigation { .. } => "navigation",
            Self::WebDriver(_) => "webdriver",
            Self::CdpDriver(_) => "cdp_driver",
            Self::Playwright(_) => "playwright",
            Self::Patcher { .. } => "patcher",
            Self::Stealth { .. } => "stealth",
            Self::Network { .. } => "network",
            Self::Download { .. } => "download",
            Self::Io(_) => "io",
            Self::Json(_) => "json",
            Self::Parse { .. } => "parse",
            Self::InvalidConfig(_) => "invalid_config",
            Self::AssertionFailed(_) => "assertion_failed",
            Self::Unsupported(_) => "unsupported",
            Self::Gui(_) => "gui",
            Self::Screenshot(_) => "screenshot",
            Self::Pdf(_) => "pdf",
            Self::Authentication(_) => "authentication",
            Self::Mcp { .. } => "mcp",
            Self::PythonMigration(_) => "python_migration",
            Self::Skipped(_) => "skipped",
            Self::TestLifecycle(_) => "test_lifecycle",
        }
    }

    /// Emit a structured `tracing::error!` event for this error, including
    /// category and hint when available.
    pub fn log(&self) {
        let hint = self.hint().unwrap_or_default();
        tracing::error!(
            error.category = self.category(),
            error.transient = self.is_transient(),
            error.hint = %hint,
            "{}",
            self
        );
    }

    /// Like `log`, but include an extra operation context string.
    pub fn log_in_context(&self, operation: impl AsRef<str>) {
        let op = operation.as_ref();
        let hint = self.hint().unwrap_or_default();
        tracing::error!(
            operation = op,
            error.category = self.category(),
            error.transient = self.is_transient(),
            error.hint = %hint,
            "{}: {}",
            op,
            self
        );
    }

    /// Returns a one-sentence remediation hint, if available.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::ElementNotFound { selector, strategy } => Some(format!(
                "Verify the selector is correct (detected strategy: {strategy}). Try waiting for the element or using a more stable locator. selector={selector}"
            )),
            Self::ElementNotInteractable { reason, .. } => Some(format!(
                "Scroll the element into view, dismiss overlays, or wait for animations to finish. {reason}"
            )),
            Self::StaleElement { .. } => Some(
                "The DOM changed between finding the element and using it. Re-query the element before interacting with it."
                    .to_owned(),
            ),
            Self::WaitTimeout { 0: op } => Some(format!(
                "Increase the timeout or verify the condition can be satisfied. operation={op}"
            )),
            Self::BrowserLaunch { binary, .. } => Some(format!(
                "Confirm '{binary}' exists in PATH and is executable, or use BrowserConfig to set an explicit binary path."
            )),
            Self::BrowserDisconnected { .. } => Some(
                "The browser process may have crashed or been closed. Restart the session."
                    .to_owned(),
            ),
            Self::SessionNotStarted => Some(
                "Call open() or start_session() before performing browser operations.".to_owned(),
            ),
            Self::Navigation { url, .. } => Some(format!(
                "Check that '{url}' is reachable and that DNS / TLS / proxy settings are correct."
            )),
            Self::Patcher { path, .. } => Some(format!(
                "Ensure '{path}' is a valid chromedriver binary and that you have write permissions."
            )),
            Self::Network { status, url, .. } if *status >= 500 => Some(format!(
                "Server error at {url}. Retry after a short delay or inspect the service status."
            )),
            Self::Network { url, .. } => Some(format!(
                "Inspect the request to {url}; verify headers, auth, and rate limits."
            )),
            Self::Download { url, .. } => Some(format!(
                "Verify the URL '{url}' returns a file and that the destination path is writable."
            )),
            Self::InvalidConfig(_) => Some(
                "Review the configuration values passed to the builder or CLI.".to_owned(),
            ),
            Self::Authentication(_) => Some(
                "Check credentials, TOTP seed, and clock skew for MFA.".to_owned(),
            ),
            Self::Mcp { tool, .. } => Some(format!(
                "Inspect the MCP tool '{tool}' arguments and server logs."
            )),
            _ => None,
        }
    }
}

fn guess_strategy(selector: &str) -> &str {
    if selector.starts_with("//") || selector.starts_with('/') || selector.starts_with("./") {
        "xpath"
    } else if selector.starts_with("link=") || selector.starts_with("link_text=") {
        "link text"
    } else if selector.starts_with("partial_link=") || selector.starts_with("partial_link_text=") {
        "partial link text"
    } else if selector.starts_with("name=") {
        "name"
    } else if selector.starts_with("id=") {
        "id"
    } else {
        "css selector"
    }
}

/// Extension trait for attaching context to `Result<T, E>` without losing the
/// original error source.
pub trait ResultExt<T, E>: Sized {
    /// Map an error into `SeleniumBaseError` with additional context.
    fn sb_context(self, context: impl Into<String>) -> Result<T>;
    /// Log any error with structured tracing fields and return the original
    /// error.
    fn log_err(self) -> Self;
}

impl<T, E: std::fmt::Display> ResultExt<T, E> for std::result::Result<T, E> {
    fn sb_context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|e| SeleniumBaseError::InvalidConfig(format!("{}: {}", context.into(), e)))
    }

    fn log_err(self) -> Self {
        if let Err(ref e) = self {
            tracing::error!("{e}");
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn element_not_found_carries_strategy() {
        let err = SeleniumBaseError::element_not_found("//div[@id='x']");
        assert!(matches!(
            err,
            SeleniumBaseError::ElementNotFound {
                selector,
                strategy,
            }
            if selector == "//div[@id='x']" && strategy == "xpath"
        ));
    }

    #[test]
    fn wait_timeout_keeps_compat_shape() {
        let err = SeleniumBaseError::wait_timeout("visible //h1", Some(Duration::from_secs(5)));
        assert!(matches!(err, SeleniumBaseError::WaitTimeout(_)));
        let msg = err.to_string();
        assert!(msg.contains("visible //h1"));
        assert!(msg.contains("5000"));
    }

    #[test]
    fn wait_timeout_without_duration() {
        let err = SeleniumBaseError::wait_timeout("ready", None);
        assert!(matches!(err, SeleniumBaseError::WaitTimeout(_)));
    }

    #[test]
    fn transient_detection() {
        assert!(SeleniumBaseError::browser_disconnected("reset").is_transient());
        assert!(!SeleniumBaseError::invalid_config("bad").is_transient());
    }

    #[test]
    fn skipped_detection() {
        assert!(SeleniumBaseError::skipped("mobile only").is_skipped());
        assert!(!SeleniumBaseError::assertion_failed("oops").is_skipped());
    }

    #[test]
    fn element_error_detection() {
        assert!(SeleniumBaseError::element_not_found("#x").is_element_error());
        assert!(!SeleniumBaseError::invalid_config("x".to_owned()).is_element_error());
    }

    #[test]
    fn category_is_stable() {
        assert_eq!(
            SeleniumBaseError::element_not_found("#x").category(),
            "element_not_found"
        );
    }

    #[test]
    fn hint_for_not_found() {
        let err = SeleniumBaseError::element_not_found("#missing");
        let hint = err.hint().unwrap();
        assert!(hint.contains("#missing"));
        assert!(hint.contains("css selector"));
    }

    #[test]
    fn display_is_actionable() {
        let err = SeleniumBaseError::patcher("chromedriver", "checksum mismatch");
        let msg = err.to_string();
        assert!(msg.contains("chromedriver"));
        assert!(msg.contains("checksum mismatch"));
    }

    #[test]
    fn result_ext_attaches_context() {
        let res: std::result::Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "oops"));
        let sb = res.sb_context("reading config");
        assert!(sb.is_err());
        let msg = sb.unwrap_err().to_string();
        assert!(msg.contains("reading config"));
        assert!(msg.contains("oops"));
    }
}
