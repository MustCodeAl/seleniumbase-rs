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
    /// Tracing filter from `SB_LOG_LEVEL`, such as `debug` or
    /// `seleniumbase_rs=debug,info`. `None` (a blank variable counts as unset)
    /// leaves it to `RUST_LOG`, then to `info`; see
    /// [`init_tracing_from_runtime`](crate::init_tracing_from_runtime).
    pub log_level: Option<String>,
    /// Tracing output format. `pretty` (default) or `json`.
    pub log_format: LogFormat,
    /// Graceful shutdown timeout. Defaults to 30 seconds.
    pub shutdown_timeout: Duration,
    /// Chromedriver listener port when auto-starting. `0` means ephemeral.
    pub chromedriver_port: u16,
    /// Default implicit wait timeout for element queries. Defaults to 30 s.
    pub implicit_wait: Duration,
    /// Client-side timeout for each HTTP request sent to a Remote WebDriver
    /// server such as a Selenium Grid.
    ///
    /// `None` (the default) leaves the WebDriver client's own timeout in
    /// place. Set it higher than the page-load timeout and higher than the
    /// time a new-session request may wait in the grid's queue; its purpose is
    /// to stop a command blocking forever when a grid node dies mid-session.
    pub remote_webdriver_timeout: Option<Duration>,
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
            log_level: None,
            log_format: LogFormat::default(),
            shutdown_timeout: Duration::from_secs(30),
            chromedriver_port: 0,
            implicit_wait: Duration::from_secs(30),
            remote_webdriver_timeout: None,
        }
    }
}

impl RuntimeConfig {
    /// Load runtime configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for the first variable
    /// that does not parse. A program that would rather keep the good
    /// variables and report the bad ones uses
    /// [`from_env_lenient`](Self::from_env_lenient).
    pub fn from_env() -> Result<Self, SeleniumBaseError> {
        Self::from_vars(env_var)
    }

    /// [`from_env`](Self::from_env) reading the variables from `var`, which
    /// looks one up by name, instead of the process environment.
    ///
    /// # Errors
    ///
    /// As [`from_env`](Self::from_env).
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self, SeleniumBaseError> {
        let (config, problems) = Self::from_vars_lenient(var);
        problems.into_iter().next().map_or(Ok(config), Err)
    }

    /// Load runtime configuration from the environment, keeping every variable
    /// that parses. A variable that does not keeps its default and comes back
    /// in the list, so one typo does not throw away the rest (a bad
    /// `SB_SHUTDOWN_TIMEOUT_SECS` must not drop `SB_LOG_LEVEL=debug`).
    #[must_use]
    pub fn from_env_lenient() -> (Self, Vec<SeleniumBaseError>) {
        Self::from_vars_lenient(env_var)
    }

    /// [`from_env_lenient`](Self::from_env_lenient) reading the variables from
    /// `var`.
    #[must_use]
    pub fn from_vars_lenient(
        var: impl Fn(&str) -> Option<String>,
    ) -> (Self, Vec<SeleniumBaseError>) {
        let mut cfg = Self::default();
        let mut problems = Vec::new();
        let mut keep = |parsed: Result<(), SeleniumBaseError>| {
            if let Err(problem) = parsed {
                problems.push(problem);
            }
        };

        if let Some(v) = var("SB_WEBDRIVER_URL") {
            cfg.webdriver_url = v;
        }
        if let Some(v) = var("SB_CHROME_BIN") {
            cfg.chrome_bin = Some(PathBuf::from(v));
        }
        if let Some(v) = var("SB_PATCH_CACHE_DIR") {
            cfg.patch_cache_dir = Some(PathBuf::from(v));
        }
        if let Some(v) = var("SB_LOG_LEVEL") {
            cfg.log_level = Some(v).filter(|level| !level.trim().is_empty());
        }
        if let Some(v) = var("SB_LOG_FORMAT") {
            keep(parse_log_format(&v).map(|format| cfg.log_format = format));
        }
        if let Some(v) = var("SB_SHUTDOWN_TIMEOUT_SECS") {
            keep(
                parse_u64(&v, "SB_SHUTDOWN_TIMEOUT_SECS")
                    .map(|secs| cfg.shutdown_timeout = Duration::from_secs(secs)),
            );
        }
        if let Some(v) = var("SB_CHROMEDRIVER_PORT") {
            keep(parse_u16(&v, "SB_CHROMEDRIVER_PORT").map(|port| cfg.chromedriver_port = port));
        }
        if let Some(v) = var("SB_IMPLICIT_WAIT_SECS") {
            keep(
                parse_u64(&v, "SB_IMPLICIT_WAIT_SECS")
                    .map(|secs| cfg.implicit_wait = Duration::from_secs(secs)),
            );
        }
        if let Some(v) = var("SB_REMOTE_WEBDRIVER_TIMEOUT") {
            keep(
                parse_u64(&v, "SB_REMOTE_WEBDRIVER_TIMEOUT")
                    .map(|secs| cfg.remote_webdriver_timeout = Some(Duration::from_secs(secs))),
            );
        }

        (cfg, problems)
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
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
        assert_eq!(cfg.log_level, None);
        assert_eq!(cfg.log_format, LogFormat::Pretty);
        assert_eq!(cfg.shutdown_timeout, Duration::from_secs(30));
    }

    fn vars(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn one_bad_variable_does_not_drop_the_good_ones() {
        let pairs = &[
            ("SB_SHUTDOWN_TIMEOUT_SECS", "soon"),
            ("SB_LOG_LEVEL", "debug"),
            ("SB_WEBDRIVER_URL", "http://grid:4444"),
        ];
        let (cfg, problems) = RuntimeConfig::from_vars_lenient(vars(pairs));
        assert_eq!(cfg.log_level.as_deref(), Some("debug"));
        assert_eq!(cfg.webdriver_url, "http://grid:4444");
        assert_eq!(cfg.shutdown_timeout, Duration::from_secs(30));
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].to_string().contains("SB_SHUTDOWN_TIMEOUT_SECS"));
        // The strict form still refuses the same input.
        assert!(RuntimeConfig::from_vars(vars(pairs)).is_err());
    }

    #[test]
    fn a_blank_log_level_is_unset() {
        for text in ["", "   "] {
            let text: &'static str = text;
            let (cfg, problems) = RuntimeConfig::from_vars_lenient(move |name| {
                (name == "SB_LOG_LEVEL").then(|| text.to_owned())
            });
            assert_eq!(cfg.log_level, None, "{text:?}");
            assert!(problems.is_empty());
        }
    }

    #[test]
    fn parses_json_format() {
        assert_eq!(parse_log_format("json").unwrap(), LogFormat::Json);
        assert_eq!(parse_log_format("JSON").unwrap(), LogFormat::Json);
        assert!(parse_log_format("xml").is_err());
    }
}
