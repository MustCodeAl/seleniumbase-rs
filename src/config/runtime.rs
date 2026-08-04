//! Twelve-Factor runtime configuration.
//!
//! Runtime config is loaded from environment variables with an `SB_` prefix.
//! These values can change per deploy without code changes and are separate
//! from user-level `Settings` (which can also be loaded from files).

use crate::error::SeleniumBaseError;
use std::path::PathBuf;
use std::time::Duration;

/// Environment-derived runtime configuration.
///
/// All fields have sensible defaults so that local development works out of
/// the box while production deploys can override via the environment.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeConfig {
    /// WebDriver endpoint. Defaults to `http://localhost:4444`.
    pub webdriver_url: String,
    /// Explicit Chrome/Chromium binary path. Falls back to `find_system_chrome()`
    /// when unset.
    pub chrome_bin: Option<PathBuf>,
    /// Directory for cached patched binaries. Defaults to the platform cache
    /// directory (`dirs::cache_dir()`).
    pub patch_cache_dir: Option<PathBuf>,
    /// Tracing log level. Defaults to `info`.
    pub log_level: String,
    /// Tracing output format. `pretty` (default) or `json`.
    pub log_format: LogFormat,
    /// Graceful shutdown timeout. Defaults to 30 seconds.
    pub shutdown_timeout: Duration,
    /// Chromedriver listener port when auto-starting. `0` means ephemeral.
    pub chromedriver_port: u16,
    /// Default implicit wait timeout for element queries. Defaults to 30 s.
    pub implicit_wait: Duration,
}

/// Supported log output formats.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LogFormat {
    /// Human-readable colored output.
    #[default]
    Pretty,
    /// Structured JSON lines.
    Json,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            webdriver_url: "http://localhost:4444".to_owned(),
            chrome_bin: None,
            patch_cache_dir: None,
            log_level: "info".to_owned(),
            log_format: LogFormat::default(),
            shutdown_timeout: Duration::from_secs(30),
            chromedriver_port: 0,
            implicit_wait: Duration::from_secs(30),
        }
    }
}

impl RuntimeConfig {
    /// Load runtime configuration from environment variables.
    pub fn from_env() -> Result<Self, SeleniumBaseError> {
        let mut cfg = Self::default();

        if let Ok(v) = std::env::var("SB_WEBDRIVER_URL") {
            cfg.webdriver_url = v;
        }
        if let Ok(v) = std::env::var("SB_CHROME_BIN") {
            cfg.chrome_bin = Some(PathBuf::from(v));
        }
        if let Ok(v) = std::env::var("SB_PATCH_CACHE_DIR") {
            cfg.patch_cache_dir = Some(PathBuf::from(v));
        }
        if let Ok(v) = std::env::var("SB_LOG_LEVEL") {
            cfg.log_level = v;
        }
        if let Ok(v) = std::env::var("SB_LOG_FORMAT") {
            cfg.log_format = parse_log_format(&v)?;
        }
        if let Ok(v) = std::env::var("SB_SHUTDOWN_TIMEOUT_SECS") {
            cfg.shutdown_timeout = Duration::from_secs(parse_u64(&v, "SB_SHUTDOWN_TIMEOUT_SECS")?);
        }
        if let Ok(v) = std::env::var("SB_CHROMEDRIVER_PORT") {
            cfg.chromedriver_port = parse_u16(&v, "SB_CHROMEDRIVER_PORT")?;
        }
        if let Ok(v) = std::env::var("SB_IMPLICIT_WAIT_SECS") {
            cfg.implicit_wait = Duration::from_secs(parse_u64(&v, "SB_IMPLICIT_WAIT_SECS")?);
        }

        Ok(cfg)
    }
}

fn parse_log_format(value: &str) -> Result<LogFormat, SeleniumBaseError> {
    match value.to_lowercase().as_str() {
        "pretty" => Ok(LogFormat::Pretty),
        "json" => Ok(LogFormat::Json),
        _ => Err(SeleniumBaseError::InvalidConfig(format!(
            "SB_LOG_FORMAT must be 'pretty' or 'json', got: {value}"
        ))),
    }
}

fn parse_u64(value: &str, name: &str) -> Result<u64, SeleniumBaseError> {
    value
        .parse()
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("{name}: {e}")))
}

fn parse_u16(value: &str, name: &str) -> Result<u16, SeleniumBaseError> {
    value
        .parse()
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("{name}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        let cfg = RuntimeConfig::default();
        assert_eq!(cfg.webdriver_url, "http://localhost:4444");
        assert_eq!(cfg.log_level, "info");
        assert_eq!(cfg.log_format, LogFormat::Pretty);
        assert_eq!(cfg.shutdown_timeout, Duration::from_secs(30));
    }

    #[test]
    fn parses_json_format() {
        assert_eq!(parse_log_format("json").unwrap(), LogFormat::Json);
        assert_eq!(parse_log_format("JSON").unwrap(), LogFormat::Json);
        assert!(parse_log_format("xml").is_err());
    }
}
