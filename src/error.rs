use thiserror::Error;

/// Errors that can occur when using SeleniumBase.
#[derive(Debug, Error)]
pub enum SeleniumBaseError {
    #[error("wait timeout: {0}")]
    WaitTimeout(String),
    #[error("webdriver command failed: {0}")]
    WebDriver(#[from] thirtyfour::error::WebDriverError),
    #[error("invalid selector: {0}")]
    InvalidSelector(String),
    #[error("assertion failed: {0}")]
    AssertionFailed(String),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("CDP driver error: {0}")]
    CdpDriver(String),
    #[error("GUI input error: {0}")]
    Gui(String),
    #[error("playwright error: {0}")]
    Playwright(String),
    #[error("test skipped: {0}")]
    Skipped(String),
}
