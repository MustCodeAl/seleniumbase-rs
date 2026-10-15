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

    /// The embedded results or profile database failed.
    #[error("database error: {0}")]
    Database(String),

    /// A plugin (feature `plugins`, ADR 004) refused to go on, or one of its
    /// hooks failed, so the launch or the tab it was preparing was stopped.
    #[error("plugin error: {reason}")]
    Plugin {
        /// The plugin's name, or empty when the error does not name one.
        plugin: String,
        /// What went wrong, as the plugin set reported it.
        reason: String,
    },
}

#[cfg(feature = "plugins")]
impl From<seleniumbase_rs_plugin::PluginError> for SeleniumBaseError {
    fn from(error: seleniumbase_rs_plugin::PluginError) -> Self {
        use seleniumbase_rs_plugin::PluginError;
        let plugin = match &error {
            PluginError::DuplicateName(name) => name.clone(),
            PluginError::MissingDependency { plugin, .. }
            | PluginError::Requirement { plugin, .. }
            | PluginError::Hook { plugin, .. } => plugin.clone(),
            _ => String::new(),
        };
        Self::plugin(plugin, error.to_string())
    }
}

#[cfg(feature = "turso")]
impl From<turso::Error> for SeleniumBaseError {
    fn from(error: turso::Error) -> Self {
        Self::Database(error.to_string())
    }
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

    /// Construct a plugin error for the plugin named `plugin` (empty when
    /// unknown).
    pub fn plugin(plugin: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Plugin {
            plugin: plugin.into(),
            reason: reason.into(),
        }
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
            Self::Database(_) => "database",
            Self::Plugin { .. } => "plugin",
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

    /// The process exit status that `sbrs` uses for this error, by category.
    ///
    /// | Code | Meaning | Categories |
    /// |---|---|---|
    /// | 1 | a check or a test failed | `assertion_failed`, `test_lifecycle`, `skipped` |
    /// | 2 | the input was wrong | `invalid_config`, `invalid_selector`, `parse`, `json`, `unsupported`, `python_migration` |
    /// | 3 | an element could not be used | `element_not_found`, `element_not_interactable`, `stale_element`, `wait_timeout` |
    /// | 4 | the browser or its driver failed | `browser_launch`, `browser_disconnected`, `session_not_started`, `webdriver`, `cdp_driver`, `playwright`, `patcher`, `stealth`, `gui`, `screenshot`, `pdf`, `plugin` |
    /// | 5 | the network or a site failed | `navigation`, `network`, `download`, `authentication` |
    /// | 6 | a local file or database failed | `io`, `database` |
    /// | 7 | the MCP server failed | `mcp` |
    ///
    /// Code 2 is also what the argument parser uses for a mistyped option, so
    /// "fix what you typed" is one number either way.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::SeleniumBaseError;
    ///
    /// assert_eq!(SeleniumBaseError::element_not_found("#buy").exit_code(), 3);
    /// assert_eq!(SeleniumBaseError::invalid_config("bad").exit_code(), 2);
    /// ```
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::AssertionFailed(_) | Self::TestLifecycle(_) | Self::Skipped(_) => 1,
            Self::InvalidConfig(_)
            | Self::InvalidSelector(_)
            | Self::Parse { .. }
            | Self::Json(_)
            | Self::Unsupported(_)
            | Self::PythonMigration(_) => 2,
            Self::ElementNotFound { .. }
            | Self::ElementNotInteractable { .. }
            | Self::StaleElement { .. }
            | Self::WaitTimeout(_) => 3,
            Self::BrowserLaunch { .. }
            | Self::BrowserDisconnected { .. }
            | Self::SessionNotStarted
            | Self::WebDriver(_)
            | Self::CdpDriver(_)
            | Self::Playwright(_)
            | Self::Patcher { .. }
            | Self::Stealth { .. }
            | Self::Gui(_)
            | Self::Screenshot(_)
            | Self::Pdf(_)
            | Self::Plugin { .. } => 4,
            Self::Navigation { .. }
            | Self::Network { .. }
            | Self::Download { .. }
            | Self::Authentication(_) => 5,
            Self::Io(_) | Self::Database(_) => 6,
            Self::Mcp { .. } => 7,
        }
    }

    /// The likely cause of this error and how to fix it, in a sentence or two.
    ///
    /// Every error has one; the `Option` is kept so that existing callers that
    /// match on it keep compiling. Element errors name the selector and the
    /// strategy it was read as, and point out a likely typo in it, such as
    /// `lnk=Home` for `link=Home` or an XPath expression that does not start
    /// with `//`.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::SeleniumBaseError;
    ///
    /// let hint = SeleniumBaseError::element_not_found("lnk=Home").hint().unwrap();
    /// assert!(hint.contains("link="), "{hint}");
    /// ```
    pub fn hint(&self) -> Option<String> {
        Some(match self {
            Self::ElementNotFound { selector, strategy } => {
                let mut hint = format!(
                    "Nothing on the page matched the {strategy} `{selector}` in time. The page \
                     may still be loading, the element may be inside an iframe or a shadow root, \
                     or the selector may be wrong: wait for the element, check the selector in \
                     the browser's developer tools, and prefer a stable attribute such as an id \
                     or data-testid."
                );
                if let Some(advice) = selector_advice(selector) {
                    hint.push(' ');
                    hint.push_str(&advice);
                }
                hint
            }
            Self::ElementNotInteractable { selector, reason } => format!(
                "`{selector}` was found but could not be used ({reason}). It is usually hidden, \
                 disabled, covered by another element such as a banner or a dialog, or outside \
                 the window: wait for it to be visible and enabled, scroll it into view, or \
                 close what covers it."
            ),
            Self::StaleElement { selector } => format!(
                "The page changed between finding `{selector}` and using it, so the old \
                 reference points at nothing. Find the element again right before acting on it."
            ),
            Self::InvalidSelector(detail) => {
                let mut hint = "The selector is not valid syntax. Check its brackets and quotes, \
                                start XPath with // or (, and write CSS such as #id, .class or \
                                [name=\"q\"]."
                    .to_owned();
                if let Some(advice) = selector_advice(detail) {
                    hint.push(' ');
                    hint.push_str(&advice);
                }
                hint
            }
            Self::WaitTimeout(operation) => format!(
                "The condition was still not met when the time ran out ({operation}). The page \
                 may be slower than expected, or the condition may never hold: raise the \
                 timeout, or check the condition by hand on the page."
            ),
            Self::BrowserLaunch { binary, .. } => format!(
                "The browser at '{binary}' could not be started. It may be missing or not \
                 executable, or another browser may be using the same profile: run `sbrs \
                 doctor`, install one with `sbrs install`, or set an explicit binary path in \
                 BrowserConfig."
            ),
            Self::BrowserDisconnected { .. } => "The browser crashed, was closed, or stopped \
                 answering. Start a new session; if it keeps happening, run it headed to watch \
                 it, and check free memory."
                .to_owned(),
            Self::SessionNotStarted => "There is no browser to act on. Start one first \
                 (BaseCase::new, or start_browser in the MCP server), and do not use a session \
                 after quit()."
                .to_owned(),
            Self::Navigation { url, .. } => format!(
                "Check that '{url}' is spelled right and reachable from this machine, and that \
                 DNS, TLS certificates and any proxy settings are correct; open it in an \
                 ordinary browser to compare."
            ),
            Self::WebDriver(error) => webdriver_hint(&error.to_string()).to_owned(),
            Self::CdpDriver(message) => cdp_hint(message).to_owned(),
            Self::Playwright(_) => "The Playwright session failed. Check that its browser is \
                 installed and still running, and that the page was not closed, then retry."
                .to_owned(),
            Self::Patcher { path, .. } => format!(
                "Ensure '{path}' is a valid chromedriver binary and that you have write \
                 permissions."
            ),
            Self::Stealth { .. } => "A stealth setting could not be applied. Check that the \
                 fingerprint does not contradict itself (Fingerprint::validate, or \
                 validate_fingerprint in the MCP server) and that the browser is Chromium-based."
                .to_owned(),
            Self::Network { status, url, .. } => network_hint(*status, url),
            Self::Download { url, .. } => format!(
                "Verify the URL '{url}' returns a file and that the destination path is writable."
            ),
            Self::Io(error) => io_hint(error.kind()).to_owned(),
            Self::Json(error) => format!(
                "Text that should be JSON is not valid JSON, or has the wrong shape (line {}, \
                 column {}). Check the file, argument or response it came from.",
                error.line(),
                error.column()
            ),
            Self::Parse { .. } => "Check where the value came from (an argument, a \
                 configuration file or an environment variable) and give it in the expected form."
                .to_owned(),
            Self::InvalidConfig(_) => "Review the configuration values passed to the builder, \
                 the command line or the environment; `sbrs doctor` checks the browser setup."
                .to_owned(),
            Self::AssertionFailed(_) => "The page did not match what the test expected. Compare \
                 the expected value with what the page shows now (a screenshot or the page \
                 source helps); if the page is still changing, wait for it before asserting."
                .to_owned(),
            Self::Unsupported(_) => "This is not available with the current browser, engine or \
                 build features. DevTools features need a Chromium-based browser; other methods \
                 name the crate feature they need in their documentation."
                .to_owned(),
            Self::Gui(_) => "Desktop input could not be sent. The program may lack the \
                 accessibility (input) permission, or there may be no display: grant the \
                 permission, or run where a display is available."
                .to_owned(),
            Self::Screenshot(_) => "The browser could not capture the page. The window may be \
                 minimised or the tab closed: bring the window forward and retry, or capture \
                 one element instead of the whole page."
                .to_owned(),
            Self::Pdf(_) => "The PDF could not be made or read. Printing a page needs a \
                 Chromium-based browser; reading one needs a valid file, and its password if it \
                 is encrypted."
                .to_owned(),
            Self::Authentication(_) => {
                "Check credentials, TOTP seed, and clock skew for MFA.".to_owned()
            }
            Self::Mcp { tool, .. } => {
                format!("Inspect the MCP tool '{tool}' arguments and server logs.")
            }
            Self::PythonMigration(_) => "Part of the Python test has no Rust equivalent yet. \
                 Review the generated TODO comments and port those lines by hand; \
                 parity/api.toml lists what is supported."
                .to_owned(),
            Self::Skipped(_) => {
                "Nothing to fix: the test asked to be skipped. Remove the skip to run it."
                    .to_owned()
            }
            Self::TestLifecycle(_) => "The test, its browser start or its cleanup failed; read \
                 each message, starting with the test's own error, since a later failure is \
                 often a consequence of the first."
                .to_owned(),
            Self::Database(_) => "The results or profile database could not be read or \
                 written. Check its path (--db or SB_REPORT_DB), that no other program holds it \
                 open, and the free disk space."
                .to_owned(),
            Self::Plugin { plugin, .. } => plugin_hint(plugin),
        })
    }
}

/// The strategy a selector is looked up with, named for people.
///
/// The same rules as [`Selector::auto`](crate::utils::selectors::Selector::auto),
/// so the name in an error is the strategy that was really used.
fn guess_strategy(selector: &str) -> &'static str {
    use crate::utils::selectors::Selector;
    match Selector::auto(selector) {
        Selector::LinkText(_) => "link text",
        Selector::PartialLinkText(_) => "partial link text",
        Selector::XPath(_) => "xpath",
        Selector::Id(_) => "id",
        Selector::Css(_) => "css selector",
    }
}

/// The selector prefixes a person may mean, for spotting a mistyped one.
const KNOWN_PREFIXES: [&str; 9] = [
    "link=",
    "link_text=",
    "text=",
    "partial_link_text=",
    "partial_link=",
    "partial_text=",
    "p_link_text=",
    "p_link=",
    "p_text=",
];

/// A likely mistake in `selector`, said as advice, if one is cheap to spot.
///
/// Only looks at the text: a mistyped strategy prefix, a prefix from another
/// tool (`css=`, `xpath=`, `id=`, `name=`), XPath without its leading `//`, an
/// empty selector, or unbalanced brackets and quotes.
pub(crate) fn selector_advice(selector: &str) -> Option<String> {
    let trimmed = selector.trim();
    if trimmed.is_empty() {
        return Some("The selector is empty.".to_owned());
    }
    if let Some((prefix, rest)) = trimmed.split_once('=') {
        let is_word =
            !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_lowercase() || c == '_');
        let known = KNOWN_PREFIXES
            .iter()
            .any(|known| known.trim_end_matches('=') == prefix);
        if is_word && !known {
            match prefix {
                "css" => {
                    return Some(format!(
                        "Drop the `css=` prefix: a plain selector is read as CSS (`{rest}`)."
                    ))
                }
                "xpath" => {
                    return Some(format!(
                        "Drop the `xpath=` prefix: a selector starting with // is read as XPath \
                         (`{rest}`)."
                    ))
                }
                "id" => {
                    return Some(format!(
                        "`id=` is not a selector strategy here; write `#{rest}` instead."
                    ))
                }
                "name" => {
                    return Some(format!(
                        "`name=` is not a selector strategy here; write `[name=\"{rest}\"]` \
                         instead."
                    ))
                }
                _ => {}
            }
            let names = KNOWN_PREFIXES.map(|known| known.trim_end_matches('='));
            if let Some(closest) = closest_match(prefix, names) {
                return Some(format!("Did you mean `{closest}={rest}`?"));
            }
        }
    }
    use crate::utils::selectors::{is_xpath_selector, Selector};
    // Link text is plain text: brackets and quotes in it mean nothing.
    if !matches!(
        Selector::auto(trimmed),
        Selector::Css(_) | Selector::XPath(_)
    ) {
        return None;
    }
    let looks_like_xpath = trimmed.contains("[@") || trimmed.contains("text()");
    if looks_like_xpath && !is_xpath_selector(trimmed) {
        return Some(format!(
            "This looks like XPath, which must start with // to be read as XPath: `//{trimmed}`."
        ));
    }
    if !balanced(trimmed) {
        return Some("Its brackets, parentheses or quotes are not balanced.".to_owned());
    }
    None
}

/// Whether every bracket and parenthesis in `text` is closed, outside quotes,
/// and every quote is closed.
fn balanced(text: &str) -> bool {
    let mut open = Vec::new();
    let mut quote = None;
    for c in text.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[' | '(') => open.push(c),
            (None, ']') if open.pop() != Some('[') => return false,
            (None, ')') if open.pop() != Some('(') => return false,
            _ => {}
        }
    }
    open.is_empty() && quote.is_none()
}

/// The candidate closest to `input`, if one is close enough to be a typo.
///
/// Compares without regard to case, by edit distance, and accepts about one
/// edit per three characters (at least one), so `lnik` finds `link` but
/// `open` does not find `close`. Ties go to the earlier candidate.
pub(crate) fn closest_match<'a>(
    input: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let wanted = input.to_lowercase();
    let limit = (wanted.chars().count().div_ceil(3)).max(1);
    candidates
        .into_iter()
        .filter(|candidate| *candidate != input)
        .map(|candidate| (edit_distance(&wanted, &candidate.to_lowercase()), candidate))
        .filter(|(distance, _)| *distance <= limit)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate)
}

/// The Levenshtein distance between `a` and `b`, in characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = Vec::with_capacity(b.len() + 1);
        current.push(i + 1);
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous[j] + usize::from(ca != *cb);
            current.push(substitute.min(previous[j + 1] + 1).min(current[j] + 1));
        }
        previous = current;
    }
    previous[b.len()]
}

/// Advice for a [`SeleniumBaseError::Plugin`], naming the plugin when known.
fn plugin_hint(plugin: &str) -> String {
    let who = if plugin.is_empty() {
        "A plugin".to_owned()
    } else {
        format!("The plugin `{plugin}`")
    };
    format!(
        "{who} stopped the browser or the tab, and plugins fail closed: a browser whose \
         plugins did not all run is not handed out. A missing plugin needs the crate that \
         provides it added to the PluginSet; a requirement needs the browser started the way \
         the plugin asks (with a window, launched rather than connected, Chromium); a failed \
         hook is a question for the plugin's documentation or its author."
    )
}

/// Advice for a WebDriver failure, from the words its message uses.
fn webdriver_hint(message: &str) -> &'static str {
    let message = message.to_lowercase();
    if message.contains("no such element") {
        "The driver found no element for the selector. Wait for it to appear, check the \
         selector, or switch into the iframe that holds it."
    } else if message.contains("stale element") {
        "The page changed after the element was found. Find it again right before using it."
    } else if message.contains("click intercepted") || message.contains("not interactable") {
        "Another element covers the target, or it is hidden or disabled. Close the banner or \
         dialog in the way, scroll the target into view, or wait until it is enabled."
    } else if message.contains("session not created") || message.contains("only supports") {
        "The driver could not start the browser; a chromedriver whose version does not match \
         the browser is the usual cause. Run `sbrs doctor` to compare them and `sbrs install` \
         to fetch a matching driver."
    } else if message.contains("invalid session") || message.contains("disconnected") {
        "The browser behind this session has closed or crashed. Start a new session."
    } else if message.contains("timeout") || message.contains("timed out") {
        "The browser did not answer in time. The page may be busy or hung: wait for it to \
         load, or raise the timeout."
    } else {
        "The WebDriver server failed the command. Check that the browser is still open and \
         that the driver matches its version (`sbrs doctor`)."
    }
}

/// Advice for a DevTools Protocol failure, from the words its message uses.
fn cdp_hint(message: &str) -> &'static str {
    if message.contains("context was destroyed") || message.contains("Cannot find context") {
        "The page navigated away while the command ran. Wait for the new page to load, then \
         retry."
    } else if message.contains("Target closed")
        || message.contains("No target")
        || message.contains("Session closed")
        || message.contains("No session")
    {
        "The tab was closed. Switch to a tab that is still open, or open a new one."
    } else if ["SyntaxError", "ReferenceError", "TypeError", "RangeError"]
        .iter()
        .any(|kind| message.contains(kind))
    {
        "The script threw in the page. Run it in the browser's developer-tools console to see \
         the line that fails."
    } else {
        "The browser rejected a DevTools command. The tab may have closed or navigated \
         meanwhile: check that it is still open, and retry once the page has loaded."
    }
}

/// Advice for an HTTP failure, by status.
fn network_hint(status: u16, url: &str) -> String {
    match status {
        500.. => format!(
            "Server error at {url}. Retry after a short delay or inspect the service status."
        ),
        401 | 403 => format!(
            "{url} refused the request ({status}): it needs credentials or a permission the \
             request did not carry. Log in first, or check the headers, cookies and API key."
        ),
        404 | 410 => format!("{url} does not exist ({status}). Check the address for a typo."),
        429 => format!(
            "{url} is rate limiting the requests (429). Slow down, and retry after the delay the \
             server asks for."
        ),
        _ => format!("Inspect the request to {url}; verify headers, auth, and rate limits."),
    }
}

/// Advice for an operating-system I/O failure, by its kind.
fn io_hint(kind: std::io::ErrorKind) -> &'static str {
    use std::io::ErrorKind;
    match kind {
        ErrorKind::NotFound => {
            "A file or directory that was needed does not exist. Check the path, and that it is \
             relative to the directory the program runs in."
        }
        ErrorKind::PermissionDenied => {
            "The operating system refused access. Check the file's permissions and that no other \
             program has it locked."
        }
        ErrorKind::AlreadyExists => {
            "A file is already there. Choose another name, or remove the old file first."
        }
        _ => "A read or write failed. Check the path, the free disk space and the permissions.",
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

    /// One error of every kind, for checks that must hold for all of them.
    fn one_of_each() -> Vec<SeleniumBaseError> {
        vec![
            SeleniumBaseError::element_not_found("#x"),
            SeleniumBaseError::element_not_interactable("#x", "hidden"),
            SeleniumBaseError::stale_element("#x"),
            SeleniumBaseError::InvalidSelector("p[".to_owned()),
            SeleniumBaseError::wait_timeout("visible #x", None),
            SeleniumBaseError::browser_launch("chrome", "missing"),
            SeleniumBaseError::browser_disconnected("reset"),
            SeleniumBaseError::SessionNotStarted,
            SeleniumBaseError::navigation("https://a.test", "dns"),
            SeleniumBaseError::WebDriver(thirtyfour::error::WebDriverError::FatalError(
                "boom".to_owned(),
            )),
            SeleniumBaseError::cdp_driver("boom"),
            SeleniumBaseError::playwright("boom"),
            SeleniumBaseError::patcher("chromedriver", "read only"),
            SeleniumBaseError::stealth("bad"),
            SeleniumBaseError::network("https://a.test", 404, "gone"),
            SeleniumBaseError::download("https://a.test/f", "gone"),
            SeleniumBaseError::Io(std::io::Error::other("disk")),
            SeleniumBaseError::Json(serde_json::from_str::<u8>("x").unwrap_err()),
            SeleniumBaseError::parse("x", "a number"),
            SeleniumBaseError::invalid_config("bad"),
            SeleniumBaseError::assertion_failed("bad"),
            SeleniumBaseError::unsupported("bad"),
            SeleniumBaseError::gui("bad"),
            SeleniumBaseError::screenshot("bad"),
            SeleniumBaseError::pdf("bad"),
            SeleniumBaseError::authentication("bad"),
            SeleniumBaseError::mcp("tool", "bad"),
            SeleniumBaseError::python_migration("bad"),
            SeleniumBaseError::skipped("bad"),
            SeleniumBaseError::test_lifecycle("bad"),
            SeleniumBaseError::Database("bad".to_owned()),
            SeleniumBaseError::plugin("adblock", "bad"),
            SeleniumBaseError::plugin("", "bad"),
        ]
    }

    #[test]
    fn every_error_has_a_hint_that_says_more_than_its_message() {
        for error in one_of_each() {
            let hint = error.hint();
            let hint = hint.as_deref().unwrap_or_default();
            assert!(
                hint.len() > 40 && hint.ends_with(['.', '?']),
                "{}: {hint:?}",
                error.category()
            );
            assert_ne!(hint, error.to_string());
        }
    }

    #[test]
    fn exit_codes_follow_the_category() {
        let code = |error: SeleniumBaseError| error.exit_code();
        assert_eq!(code(SeleniumBaseError::assertion_failed("x")), 1);
        assert_eq!(code(SeleniumBaseError::invalid_config("x")), 2);
        assert_eq!(code(SeleniumBaseError::element_not_found("#x")), 3);
        assert_eq!(code(SeleniumBaseError::wait_timeout("x", None)), 3);
        assert_eq!(code(SeleniumBaseError::SessionNotStarted), 4);
        assert_eq!(code(SeleniumBaseError::navigation("u", "r")), 5);
        assert_eq!(code(SeleniumBaseError::Database("x".to_owned())), 6);
        assert_eq!(code(SeleniumBaseError::mcp("t", "r")), 7);
        for error in one_of_each() {
            assert!((1..=7).contains(&error.exit_code()), "{error}");
        }
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn a_plugin_error_keeps_the_plugins_name_and_its_message() {
        use seleniumbase_rs_plugin::PluginError;

        let error = SeleniumBaseError::from(PluginError::Hook {
            plugin: "adblock".to_owned(),
            hook: "on_page_created",
            message: "boom".to_owned(),
        });
        assert!(
            matches!(&error, SeleniumBaseError::Plugin { plugin, .. } if plugin == "adblock"),
            "{error:?}"
        );
        assert!(error.to_string().contains("on_page_created"), "{error}");
        assert!(error.hint().unwrap().contains("`adblock`"));
        assert_eq!(error.exit_code(), 4);

        let missing = SeleniumBaseError::from(PluginError::MissingDependency {
            plugin: "stealth".to_owned(),
            missing: "user-agent".to_owned(),
        });
        assert!(
            matches!(&missing, SeleniumBaseError::Plugin { plugin, .. } if plugin == "stealth")
        );
        assert!(missing.to_string().contains("`user-agent`"), "{missing}");

        let other = SeleniumBaseError::from(PluginError::other("no reason"));
        assert!(matches!(&other, SeleniumBaseError::Plugin { plugin, .. } if plugin.is_empty()));
        assert!(other.hint().unwrap().starts_with("A plugin"));
    }

    #[test]
    fn the_strategy_named_in_an_error_is_the_one_the_lookup_uses() {
        let strategy = |selector: &str| match SeleniumBaseError::element_not_found(selector) {
            SeleniumBaseError::ElementNotFound { strategy, .. } => strategy,
            other => panic!("{other}"),
        };
        assert_eq!(strategy("text=Home"), "link text");
        assert_eq!(strategy("p_link=Ho"), "partial link text");
        assert_eq!(strategy("parent::div"), "xpath");
        assert_eq!(strategy("(//a)[2]"), "xpath");
        assert_eq!(
            strategy("name=q"),
            "css selector",
            "name= is not a strategy, so it is looked up as CSS"
        );
    }

    #[test]
    fn a_likely_mistake_in_a_selector_is_pointed_out() {
        let advice = |selector: &str| selector_advice(selector).unwrap_or_default();
        assert!(
            advice("lnk=Home").contains("`link=Home`"),
            "{}",
            advice("lnk=Home")
        );
        assert!(advice("css=#a").contains("Drop the `css=` prefix"));
        assert!(advice("xpath=//a").contains("Drop the `xpath=` prefix"));
        assert!(advice("id=main").contains("`#main`"));
        assert!(advice("name=q").contains("[name=\"q\"]"));
        assert!(advice("div[@id='x']").contains("`//div[@id='x']`"));
        assert!(advice("#a[").contains("not balanced"));
        assert!(advice("  ").contains("empty"));
        for fine in [
            "#ok",
            "a[href='x']",
            "//a[@id='(']",
            "link=Foo (bar",
            "input[name=q]",
        ] {
            assert_eq!(selector_advice(fine), None, "{fine}");
        }
    }

    #[test]
    fn the_not_found_hint_carries_the_selector_advice() {
        let hint = SeleniumBaseError::element_not_found("lnk=Home")
            .hint()
            .unwrap();
        assert!(
            hint.contains("lnk=Home") && hint.contains("`link=Home`"),
            "{hint}"
        );
    }

    #[test]
    fn close_matches_allow_a_typo_or_two_but_not_a_different_word() {
        let tools = ["click_element", "type_text", "open_url", "close_browser"];
        assert_eq!(closest_match("clik_element", tools), Some("click_element"));
        assert_eq!(closest_match("Open_URL", tools), Some("open_url"));
        assert_eq!(closest_match("lnik", ["link", "name"]), Some("link"));
        assert_eq!(closest_match("open", ["close"]), None);
        assert_eq!(closest_match("make_coffee", tools), None);
        assert_eq!(closest_match("type_text", tools), None, "not a typo");
    }

    #[test]
    fn hints_follow_what_the_backend_said() {
        let hint = |error: SeleniumBaseError| error.hint().unwrap();
        assert!(hint(SeleniumBaseError::cdp_driver(
            "Execution context was destroyed"
        ))
        .contains("navigated away"));
        assert!(hint(SeleniumBaseError::cdp_driver(
            "ReferenceError: x is not defined"
        ))
        .contains("script threw"));
        assert!(
            hint(SeleniumBaseError::network("https://a.test", 403, "no")).contains("credentials")
        );
        assert!(
            hint(SeleniumBaseError::network("https://a.test", 503, "no")).contains("Server error")
        );
        assert!(hint(SeleniumBaseError::Io(std::io::Error::from(
            std::io::ErrorKind::NotFound
        )))
        .contains("does not exist"));
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
