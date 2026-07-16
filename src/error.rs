use thiserror::Error;

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
}
