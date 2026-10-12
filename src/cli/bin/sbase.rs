#![allow(deprecated)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use seleniumbase_rs::api::recorder::{RecordedAction, RecorderSession};
use seleniumbase_rs::artifacts::{artifact_path, ensure_latest_logs_dir};
use seleniumbase_rs::cli::commands::{
    blank_file_input_page, download_file_name, format_deferred_summary, html_data_url,
    looks_like_pdf, parse_deferred_spec_json, parse_deferred_specs, DeferredResult, DeferredSpec,
};
use seleniumbase_rs::cli::scaffold::{Scaffold, ScaffoldError};
use seleniumbase_rs::cli::scripts::sb_mkdir::Suite;
use seleniumbase_rs::cli::scripts::sb_mkfile::TestFile;
use seleniumbase_rs::cli::scripts::*;
use seleniumbase_rs::common::encryption::{decrypt_with_passphrase, encrypt_with_passphrase};
use seleniumbase_rs::common::shutdown::{
    drain_cleanups, run_until_shutdown, timeout_from_env, Outcome, Shutdown,
};
// use seleniumbase_rs::dashboard::write_dashboard_html;
use seleniumbase_rs::api::scenario::{run_scenario, write_dashboard_html, Scenario};
use seleniumbase_rs::browser::driver_version::binary_major;
use seleniumbase_rs::config::proxy_list::ProxyList;
use seleniumbase_rs::config::settings::{Settings, SettingsOverrides};
use seleniumbase_rs::resources::assets;
use seleniumbase_rs::stealth::patcher::find_system_chrome;
use seleniumbase_rs::utilities::grid_server::{default_state_dir, GridRole};
use seleniumbase_rs::utilities::importer::{
    import_playwright, import_puppeteer, ScriptImportOptions, ScriptLanguage,
};
use seleniumbase_rs::utils::translate::Language;
use seleniumbase_rs::{
    import_python, init_tracing_from_runtime, BaseCase, Browser, ChromeBinaryPatcher,
    ChromedriverPatcher, DriverMode, EnginePatch, ImportOptions, ImportSeverity, PythonSource,
    RuntimeConfig,
};
use serde_json::{json, Value};
use thirtyfour::extensions::cdp::NetworkConditions;

#[cfg(feature = "turso")]
mod report;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("CARGO_PKG_NAME"), ")");

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BrowserArg {
    Chrome,
    Chromium,
    Edge,
    Firefox,
    /// Brave, run with chromedriver.
    Brave,
    /// Perplexity's Comet, run with chromedriver.
    Comet,
    /// Opera, run with operadriver.
    Opera,
    /// Safari, run with safaridriver (macOS).
    Safari,
    /// Internet Explorer, run with IEDriverServer (Windows).
    Ie,
    /// Chrome for Testing, downloaded when missing.
    Cft,
    /// chrome-headless-shell, downloaded when missing.
    Chs,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GridActionArg {
    /// Start it and wait until it is ready.
    Start,
    /// Stop it.
    Stop,
    /// Stop it if it is running, then start it.
    Restart,
    /// Say whether it is running and ready.
    Status,
}

impl From<GridActionArg> for sb_grid::GridAction {
    fn from(value: GridActionArg) -> Self {
        match value {
            GridActionArg::Start => Self::Start,
            GridActionArg::Stop => Self::Stop,
            GridActionArg::Restart => Self::Restart,
            GridActionArg::Status => Self::Status,
        }
    }
}

/// The options `grid-hub` and `grid-node` share.
#[derive(clap::Args, Debug)]
struct GridArgs {
    /// What to do with the Grid process.
    #[arg(value_enum)]
    action: GridActionArg,
    /// The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable).
    /// It is never downloaded.
    #[arg(long, value_name = "PATH")]
    jar: Option<PathBuf>,
    /// The Java executable (default: `java` on PATH).
    #[arg(long, value_name = "PATH")]
    java: Option<PathBuf>,
    /// The address to listen on.
    #[arg(long = "grid-host", value_name = "HOST")]
    host: Option<String>,
    /// The port to listen on (default: 4444 for a hub, 5555 for a node).
    #[arg(long = "grid-port", value_name = "PORT")]
    port: Option<u16>,
    /// For a node, the hub to register with: a host or a URL.
    #[arg(long, value_name = "HOST_OR_URL")]
    hub: Option<String>,
    /// Close sessions that have been idle this many seconds.
    #[arg(long, value_name = "SECONDS")]
    session_timeout: Option<u64>,
    /// How many seconds `start` waits for the Grid to become ready.
    #[arg(long, value_name = "SECONDS", default_value_t = 60)]
    wait: u64,
    /// Log at INFO instead of WARNING.
    #[arg(long = "grid-verbose")]
    verbose: bool,
    /// Start even if something already accepts connections on the port.
    #[arg(long)]
    skip_port_check: bool,
    /// More arguments for the Selenium Server, after `--`.
    #[arg(last = true)]
    extra_args: Vec<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ImportSourceArg {
    Auto,
    SeleniumBase,
    Selenium,
}

/// The languages `sbase import-playwright` reads.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum PlaywrightLanguageArg {
    /// JavaScript.
    Js,
    /// TypeScript.
    Ts,
    /// Python.
    Python,
}

impl From<PlaywrightLanguageArg> for ScriptLanguage {
    fn from(value: PlaywrightLanguageArg) -> Self {
        match value {
            PlaywrightLanguageArg::Js => ScriptLanguage::JavaScript,
            PlaywrightLanguageArg::Ts => ScriptLanguage::TypeScript,
            PlaywrightLanguageArg::Python => ScriptLanguage::Python,
        }
    }
}

/// The languages `sbase import-puppeteer` reads.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum PuppeteerLanguageArg {
    /// JavaScript.
    Js,
    /// TypeScript.
    Ts,
    /// A Chrome DevTools Recorder JSON export.
    Json,
}

impl From<PuppeteerLanguageArg> for ScriptLanguage {
    fn from(value: PuppeteerLanguageArg) -> Self {
        match value {
            PuppeteerLanguageArg::Js => ScriptLanguage::JavaScript,
            PuppeteerLanguageArg::Ts => ScriptLanguage::TypeScript,
            PuppeteerLanguageArg::Json => ScriptLanguage::RecorderJson,
        }
    }
}

impl From<BrowserArg> for Browser {
    fn from(value: BrowserArg) -> Self {
        match value {
            BrowserArg::Chrome => Browser::Chrome,
            BrowserArg::Chromium => Browser::Chromium,
            BrowserArg::Edge => Browser::Edge,
            BrowserArg::Firefox => Browser::Firefox,
            BrowserArg::Brave => Browser::Brave,
            BrowserArg::Comet => Browser::Comet,
            BrowserArg::Opera => Browser::Opera,
            BrowserArg::Safari => Browser::Safari,
            BrowserArg::Ie => Browser::InternetExplorer,
            BrowserArg::Cft => Browser::ChromeForTesting,
            BrowserArg::Chs => Browser::ChromeHeadlessShell,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "sbase", version = VERSION, about = "SeleniumBase Rust CLI", long_about = "A Rust port of the Python SeleniumBase testing framework. Provides browser automation, stealth/undetected modes, and a command-line helper.", before_long_help = logo_helper::LOGO)]
struct Cli {
    #[arg(
        long,
        value_name = "URL",
        help = "WebDriver server URL (default: SB_WEBDRIVER_URL, else http://localhost:4444)"
    )]
    webdriver: Option<String>,
    #[arg(
        long,
        value_enum,
        help = "Browser engine to launch (default: chrome, or the config file's)"
    )]
    browser: Option<BrowserArg>,
    #[arg(long, default_value_t = false, help = "Run the browser in headed mode")]
    headed: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Run the browser in headless mode"
    )]
    headless: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Enable Chrome DevTools Protocol mode"
    )]
    cdp: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Enable undetected-chromedriver style evasions"
    )]
    uc: bool,
    #[arg(long, help = "Override the default user agent string")]
    user_agent: Option<String>,
    #[arg(short = 'a', long, help = "Alias for --user-agent")]
    agent: Option<String>,
    #[arg(long, help = "Locale string for the browser context")]
    locale: Option<String>,
    #[arg(
        long,
        default_value_t = false,
        help = "Refuse the requests of ad and tracker hosts (Chromium WebDriver sessions)"
    )]
    ad_block: bool,
    #[arg(
        long,
        help = "Proxy (host:port or scheme://host:port); with --proxy-list, a name from the list"
    )]
    proxy: Option<String>,
    #[arg(
        long,
        value_name = "FILE",
        help = "File of proxies to choose from; without --proxy a random one is used"
    )]
    proxy_list: Option<String>,
    #[arg(long, help = "URL to a proxy auto-config (PAC) file")]
    proxy_pac_url: Option<String>,
    #[arg(long, help = "Path to a persistent browser user-data directory")]
    user_data_dir: Option<String>,
    #[arg(long, help = "Path to an unpacked extension directory")]
    extension_dir: Option<String>,
    #[arg(
        long = "reuse-session",
        visible_aliases = ["reuse_session", "rs"],
        default_value_t = false,
        help = "Share browsers among the tests of the run (SB_REUSE_SESSION)"
    )]
    reuse_session: bool,
    #[arg(
        long = "rcs",
        visible_aliases = ["reuse-class-session", "reuse_class_session"],
        default_value_t = false,
        help = "Share a browser among the tests of one Rust module (SB_REUSE_CLASS_SESSION)"
    )]
    reuse_class_session: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Use a mobile device emulation profile"
    )]
    mobile: bool,
    #[arg(
        long = "esc-end",
        visible_aliases = ["esc_end", "ee"],
        default_value_t = false,
        help = "End the current test when ESC is pressed in the browser"
    )]
    esc_end: bool,
    #[arg(
        long = "rec-sb-cdp",
        visible_alias = "rec_sb_cdp",
        default_value_t = false,
        help = "Write recordings as pure CDP code (sb_cdp Page and Locator calls)"
    )]
    rec_sb_cdp: bool,
    #[arg(short = 'n', long, help = "Number of parallel threads for test runs")]
    threads: Option<usize>,
    #[arg(short = 'c', long, help = "Path to a TOML/JSON settings file")]
    config: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print the sbase version, as `sbase --version` does.
    Version,
    /// Open a URL in the browser.
    Open {
        /// URL to open in the browser.
        url: String,
    },
    /// Run a smoke test against a URL.
    Smoke {
        #[arg(help = "URL to open or smoke-test")]
        url: String,
        #[arg(long, help = "Expected substring in the page title")]
        title_contains: Option<String>,
    },
    /// Execute a Chrome DevTools Protocol command.
    Cdp {
        #[arg(long, help = "CDP method name (e.g. Runtime.evaluate)")]
        cmd: String,
        #[arg(long, help = "JSON object of CDP parameters")]
        params: Option<String>,
    },
    /// Clear the browser cache via CDP.
    CacheClear,
    /// Throttle the browser connection to 3G speeds.
    Throttle3g,
    /// Capture a screenshot of the current page.
    Screenshot {
        #[arg(
            long,
            help = "Output image file path (default: a new file in latest_logs/)"
        )]
        path: Option<String>,
    },
    /// Save the current page source to a file.
    SaveSource {
        #[arg(
            long,
            help = "Output HTML file path (default: a new file in latest_logs/)"
        )]
        path: Option<String>,
    },
    /// Assert that an element exists.
    AssertElement {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Wait until an element contains the expected text.
    WaitForText {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Expected text substring to wait for")]
        text: String,
        #[arg(long, default_value_t = 10, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Hover over an element.
    Hover {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Hover over one element, then click another.
    HoverAndClick {
        #[arg(long, help = "CSS selector to hover over")]
        hover_css: String,
        #[arg(long, help = "CSS selector to click")]
        click_css: String,
    },
    /// Select an option from a dropdown.
    SelectOption {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Visible text of the option to select")]
        text: Option<String>,
        #[arg(long, help = "Option value attribute")]
        value: Option<String>,
    },
    /// Drag one element onto another.
    DragAndDrop {
        #[arg(long, help = "CSS selector of the element to drag")]
        source_css: String,
        #[arg(long, help = "CSS selector of the drop target")]
        target_css: String,
    },
    /// Click an element using CDP.
    CdpClickElement {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Type text using CDP input injection.
    CdpTypeText {
        #[arg(long, help = "Text to inject via CDP")]
        text: String,
    },

    /// Navigate back in browser history.
    GoBack,
    /// Navigate forward in browser history.
    GoForward,
    /// Reload the current page.
    Refresh,

    /// Print the visible text of an element.
    GetText {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Print an element attribute value.
    GetAttribute {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Attribute name to read or assert")]
        attribute: String,
    },
    /// Print an element property value.
    GetProperty {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Property name to read")]
        property: String,
    },

    /// Print the page title.
    GetTitle,
    /// Print the current URL.
    GetCurrentUrl,

    /// Delete all browser cookies.
    ClearCookies,

    /// Accept the active browser alert.
    AcceptAlert,
    /// Dismiss the active browser alert.
    DismissAlert,
    /// Print the text of the active alert.
    GetAlertText,
    /// Type text into the active alert prompt.
    TypeAlertText {
        #[arg(long, help = "Text to type into the alert prompt")]
        text: String,
    },
    /// Clear the page local storage.
    ClearLocalStorage,
    /// Print a local storage value by key.
    GetLocalStorageItem {
        #[arg(long, help = "Storage key")]
        key: String,
    },
    /// Set a local storage key/value pair.
    SetLocalStorageItem {
        #[arg(long, help = "Storage key")]
        key: String,
        #[arg(long, help = "Value to store in local storage")]
        value: String,
    },
    /// Remove a local storage entry by key.
    RemoveLocalStorageItem {
        #[arg(long, help = "Storage key")]
        key: String,
    },
    /// Switch focus to a window by handle.
    SwitchToWindow {
        #[arg(long, help = "Window handle to switch to")]
        handle: String,
    },
    /// Switch focus to an iframe.
    SwitchToFrame {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Switch focus back to the main document.
    SwitchToDefaultContent,

    /// Print all browser cookies as JSON.
    GetCookies,
    /// Export the recorded test actions.
    ExportRecording,
    /// Patch a chromedriver binary to reduce detection surface.
    PatchChromedriver {
        #[arg(long, help = "Path to the chromedriver binary")]
        path: String,
    },
    /// Patch a Chrome/Chromium binary for native-level spoofing.
    PatchChrome {
        #[arg(long, help = "Path to the Chrome/Chromium binary")]
        path: String,
        /// Directory where the patched copy is cached.
        #[arg(long, help = "Directory where patched binaries are cached")]
        cache_dir: Option<String>,
    },
    /// Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted.
    ///
    /// The passphrase comes from the SB_ENCRYPTION_KEY environment variable, never
    /// from an argument, so it stays out of shell history and process listings.
    /// Python's `sbase obfuscate` is the same command.
    #[command(visible_alias = "obfuscate")]
    Encrypt {
        /// The text to encrypt.
        text: Option<String>,
    },
    /// Decrypt a token made by `sbase encrypt`; read from stdin if TOKEN is omitted.
    /// Python's `sbase unobfuscate` is the same command.
    #[command(visible_alias = "unobfuscate")]
    Decrypt {
        /// The token to decrypt.
        token: Option<String>,
    },
    /// Show the test runs recorded in a results database.
    ///
    /// Lists the latest runs; `--run` shows one run's results and `--flaky`
    /// the tests that both pass and fail across recent runs. The database is
    /// the one a `ResultStore` writes: pass its path with `--db` or set
    /// SB_REPORT_DB (default: reports/results.db).
    #[cfg(feature = "turso")]
    Report {
        /// Path of the results database.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Show the results of this run.
        #[arg(long, conflicts_with = "flaky")]
        run: Option<seleniumbase_rs::storage::RunId>,
        /// With --run, show only the tests that failed.
        #[arg(long, requires = "run")]
        failed: bool,
        /// Show tests that both pass and fail across the latest runs.
        #[arg(long)]
        flaky: bool,
        /// How many runs to list, or to examine for flaky tests.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Run a diagnostic check on the environment and configuration.
    Doctor,
    /// Assert that an element contains the expected text.
    AssertTextVisible {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Expected visible text")]
        text: String,
    },
    /// Assert that an element does not contain the given text.
    AssertTextNotVisible {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Text that should not be visible")]
        text: String,
    },
    /// Assert an element attribute equals an expected value.
    AssertAttribute {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Attribute name to read or assert")]
        attribute: String,
        #[arg(long, help = "Expected attribute value")]
        value: String,
    },
    /// Assert the page title contains the expected text.
    AssertTitle {
        #[arg(long, help = "Expected page title substring")]
        text: String,
    },
    /// Wait until the document readyState is complete.
    WaitForReadyStateComplete,
    /// Print the browser window position.
    GetWindowPosition,
    /// Set the browser window position.
    SetWindowPosition {
        #[arg(long, help = "Window X coordinate in pixels")]
        x: u32,
        #[arg(long, help = "Window Y coordinate in pixels")]
        y: u32,
    },
    /// Close the current browser window.
    CloseWindow,
    /// Switch focus to the parent frame.
    SwitchToParentFrame,
    /// Print whether an element is visible.
    IsElementVisible {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Print whether the expected text is visible in an element.
    IsTextVisible {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Expected text to check")]
        text: String,
    },
    /// Wait until an element is no longer visible.
    WaitForElementNotVisible {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, default_value_t = 10, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Save browser cookies to a JSON file.
    SaveCookies {
        #[arg(long, help = "JSON file to save cookies to")]
        file: String,
    },
    /// Load browser cookies from a JSON file.
    LoadCookies {
        #[arg(long, help = "JSON file to load cookies from")]
        file: String,
    },
    /// Highlight an element, then click it.
    HighlightClick {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Print whether a checkbox is checked.
    IsChecked {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Check a checkbox only if it is unchecked.
    CheckIfUnchecked {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Uncheck a checkbox only if it is checked.
    UncheckIfChecked {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Open a new browser window.
    OpenNewWindow,
    /// Open a new browser tab.
    OpenNewTab,
    /// Switch focus to the newest browser window.
    SwitchToNewestWindow,
    /// Switch focus to the default window.
    SwitchToDefaultWindow,
    /// Print a CSS selector for the currently focused element.
    GetActiveElementCss,
    /// Wait until an element is present in the DOM.
    WaitForElementPresent {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, default_value_t = 10, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Append text to an input or textarea.
    AddText {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Text to append to the element")]
        text: String,
    },
    /// Send keystrokes to an element.
    SendKeys {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Keys to send to the element")]
        text: String,
    },
    /// Print the value of a form element.
    GetValue {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Click all visible elements matching a selector.
    ClickVisibleElements {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Wait for an alert, then accept it.
    WaitForAndAcceptAlert {
        #[arg(long, default_value_t = 10, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Wait for an alert, then dismiss it.
    WaitForAndDismissAlert {
        #[arg(long, default_value_t = 10, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Print whether a link with the given text is visible.
    IsLinkTextVisible {
        #[arg(long, help = "Exact link text to check")]
        text: String,
    },
    /// Print whether a link containing the given text is visible.
    IsPartialLinkTextVisible {
        #[arg(long, help = "Link text substring to check")]
        text: String,
    },
    /// Assert a link with the given text is visible.
    AssertLinkText {
        #[arg(long, help = "Exact link text to assert")]
        text: String,
    },
    /// Click a link containing the given text.
    ClickPartialLinkText {
        #[arg(long, help = "Link text substring to click")]
        text: String,
    },
    /// Type text with human-like timing and noise.
    HumanType {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Text to type with humanized timing")]
        text: String,
    },
    /// Click an element with human-like timing and noise.
    HumanClick {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Smoothly scroll an element into view.
    SmoothScrollTo {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Perform a UC-mode click on an element.
    UcClick {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
    },
    /// Perform UC-mode typing into an element.
    UcType {
        #[arg(long, help = "CSS selector for the target element")]
        css: String,
        #[arg(long, help = "Text to type in UC mode")]
        text: String,
    },
    /// Download a driver into ./downloaded_drivers/ (Python's `sbase get`).
    ///
    /// Without a version, chromedriver and edgedriver are the drivers for the
    /// installed Chrome and Edge, and geckodriver is the newest release.
    #[command(visible_alias = "get")]
    Install {
        /// The driver: chromedriver, geckodriver (or firefoxdriver), edgedriver
        /// (or msedgedriver).
        #[arg(default_value = "chromedriver")]
        driver: String,
        /// `latest`, a major version (chromedriver, edgedriver), a full build,
        /// or a geckodriver version such as 0.36.0.
        version: Option<String>,
    },
    /// Run the project's tests, or one example, through Cargo.
    ///
    /// The global options reach the tests as SB_* environment variables (for
    /// example `sbase --headless --browser firefox test` sets SB_HEADLESS and
    /// SB_BROWSER), so a test that loads `Settings` sees them, and `-n` sets
    /// how many tests run at once.
    Test {
        /// Run only the tests whose name contains this text.
        #[arg(conflicts_with = "example")]
        filter: Option<String>,
        /// Run one integration test file from tests/.
        #[arg(long, value_name = "NAME", conflicts_with = "example")]
        test: Option<String>,
        /// Run one example from examples/ instead of the tests.
        #[arg(long, value_name = "NAME")]
        example: Option<String>,
        /// Build with the release profile.
        #[arg(long)]
        release: bool,
        /// Cargo features to enable, separated by commas.
        #[arg(long, value_delimiter = ',')]
        features: Vec<String>,
        /// Arguments for the test harness, after `--` (for example --nocapture).
        #[arg(last = true)]
        harness_args: Vec<String>,
    },
    /// Start, stop, restart or check a Selenium Grid hub.
    ///
    /// Needs a Selenium Server jar you supply; nothing is downloaded. The
    /// process record and log are kept in a folder of your own (SB_GRID_DIR).
    GridHub(GridArgs),
    /// Start, stop, restart or check a Selenium Grid node.
    ///
    /// A node registers with the hub named by `--hub`, or with this machine's.
    GridNode(GridArgs),
    /// Write the actions of a JSON scenario in another language.
    ///
    /// Rewrites each step's `action` (`click`, `open`, ...) with the translation
    /// dictionary and changes nothing else in the file; the steps keep running,
    /// because a scenario can be read in any of the languages. Python
    /// SeleniumBase translates Python test files; a Rust test is compiled code,
    /// so this translates the portable JSON scenario instead.
    Translate {
        /// The scenario file.
        file: PathBuf,
        /// The language: a code (en, zh, nl, fr, it, ja, ko, pt, ru, es) or its
        /// English name.
        #[arg(value_parser = clap::value_parser!(Language))]
        language: Language,
        /// Print the translation and write nothing (the default).
        #[arg(short, long, conflicts_with_all = ["overwrite", "copy"])]
        print: bool,
        /// Replace the file with the translation.
        #[arg(short, long, conflicts_with = "copy")]
        overwrite: bool,
        /// Write the translation to a new file next to it, such as `login_es.json`.
        #[arg(short, long)]
        copy: bool,
    },
    /// Create a folder with a runnable browser-test suite.
    ///
    /// Names are relative to the current directory and use letters, digits, `_`,
    /// `-` and `.`, with `/` between folders. Nothing is overwritten unless
    /// `--force` is given.
    Mkdir {
        /// Folder to create, for example `tests/ui`.
        dir: String,
        /// Create only the scaffolding, with no example tests.
        #[arg(short, long)]
        basic: bool,
        /// Replace generated files that already exist.
        #[arg(long)]
        force: bool,
    },
    /// Create a browser-test file.
    ///
    /// The name follows the same rules as `mkdir`; `.rs` is added when missing.
    /// An existing file is never overwritten unless `--force` is given.
    #[command(visible_alias = "new")]
    Mkfile {
        /// File to create, for example `tests/login.rs`.
        file: String,
        /// Page the test opens (default: https://example.com).
        #[arg(long)]
        url: Option<String>,
        /// Leave the test body empty: it only opens the page.
        #[arg(short, long)]
        basic: bool,
        /// Replace the file if it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Launch the interactive commander (needs a build with the `tui` feature).
    Commander,
    /// Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature).
    ///
    /// Lists the tests under PATH and creates, edits and saves a case plan for
    /// each, `case_plans/<test>.md` next to its file, and writes a summary of the
    /// plans to `case_summary.md`. PATH is a folder of tests or one test file;
    /// by default the `tests/` and `examples/` folders of the current folder.
    ///
    /// The Python command opens a Tkinter window; this one runs in the terminal.
    #[command(visible_alias = "case-plans")]
    Caseplans {
        /// A folder of tests, or one test file.
        path: Option<String>,
        /// Write the starter checklist `CASE_PLANS.md` instead; this needs no
        /// `tui` feature.
        #[arg(long, conflicts_with = "path")]
        starter: bool,
    },
    /// Launch the Behave Commander, a terminal app (needs a build with the `tui` feature).
    ///
    /// Lists the `.feature` files under PATH with their scenarios, runs the ones
    /// you mark through the crate's behave runner and shows the results. PATH is
    /// a folder of feature files or one file; by default `features`.
    ///
    /// The runner has only the built-in steps: steps written in Rust are not
    /// loaded, and a scenario that uses one is reported as undefined. The Python
    /// command opens a Tkinter window; this one runs in the terminal.
    #[command(visible_alias = "gui-behave")]
    BehaveGui {
        /// A folder of feature files, or one `.feature` file.
        path: Option<String>,
        /// Prompt for a name, a title and scenarios and write a new feature
        /// file instead; this needs no `tui` feature.
        #[arg(long, conflicts_with = "path")]
        new: bool,
    },
    /// Print a file.
    Print {
        #[arg(long, help = "File path to print")]
        file: String,
    },
    /// Objectify a test recording.
    Objectify,
    /// Create an HTML presentation.
    Mkpres {
        #[arg(long, help = "Output HTML presentation path")]
        file: String,
    },
    /// Create an HTML chart.
    Mkchart {
        #[arg(long, help = "Output HTML chart path")]
        file: String,
    },
    /// Create a test recording scaffold.
    Mkrec {
        #[arg(long, help = "Output recording scaffold path")]
        file: String,
    },
    /// Run a scenario file and write an optional dashboard.
    RunScenario {
        #[arg(long, help = "Scenario file to run")]
        file: String,
        #[arg(long, help = "Optional path to write a scenario dashboard")]
        dashboard: Option<String>,
    },
    /// Generate a completion script for a supported shell.
    Completions {
        #[arg(value_enum, help = "Shell to generate a completion script for")]
        shell: Shell,
    },
    /// Convert common SeleniumBase or Selenium Python code into a Rust test.
    ImportPython {
        /// Python source file to convert.
        file: String,
        /// Write generated Rust to this path instead of stdout.
        #[arg(short, long, help = "Output file for the generated Rust test")]
        output: Option<String>,
        /// Override source API detection.
        #[arg(long, value_enum, default_value_t = ImportSourceArg::Auto, help = "Source API to assume when converting Python code")]
        source: ImportSourceArg,
        /// Generated Rust test function name.
        #[arg(long, help = "Name of the generated test function")]
        test_name: Option<String>,
    },
    /// Convert Playwright tests or a script (JavaScript, TypeScript or Python) into Rust tests for the pure CDP engine.
    #[command(visible_alias = "import-pw")]
    ImportPlaywright {
        /// Playwright source file to convert.
        file: String,
        #[arg(short, long, help = "Output file for the generated Rust tests")]
        output: Option<String>,
        #[arg(
            long,
            value_enum,
            help = "Source language (default: from the file extension, else from the content)"
        )]
        language: Option<PlaywrightLanguageArg>,
        #[arg(
            long,
            help = "Name of the test generated for a script without test titles (default: the file name)"
        )]
        test_name: Option<String>,
    },
    /// Convert Puppeteer scripts or tests (JavaScript, TypeScript) or a Chrome Recorder JSON export into Rust tests for the pure CDP engine.
    #[command(visible_alias = "import-pptr")]
    ImportPuppeteer {
        /// Puppeteer source file (or Recorder `.json` export) to convert.
        file: String,
        #[arg(short, long, help = "Output file for the generated Rust tests")]
        output: Option<String>,
        #[arg(
            long,
            value_enum,
            help = "Source language (default: from the file extension, else from the content)"
        )]
        language: Option<PuppeteerLanguageArg>,
        #[arg(
            long,
            help = "Name of the test generated for a script without test titles (default: the file name)"
        )]
        test_name: Option<String>,
    },

    /// Download a file from a URL (browser download with an HTTP fallback).
    DownloadFile {
        #[arg(help = "URL of the file to download")]
        url: String,
        #[arg(
            short,
            long,
            help = "Output file path (defaults to the URL file name in the current directory)"
        )]
        output: Option<String>,
        #[arg(
            long,
            default_value_t = 30,
            help = "Seconds to wait for the browser download before falling back to HTTP"
        )]
        timeout: u64,
    },
    /// Print a page to PDF and save it to disk.
    PrintPdf {
        #[arg(help = "URL to render as a PDF")]
        url: String,
        #[arg(short, long, default_value = "page.pdf", help = "Output PDF file path")]
        output: String,
    },
    /// Print the text extracted from a page or PDF URL.
    PdfText {
        #[arg(help = "URL of the page or PDF to extract text from")]
        url: String,
        #[arg(long, help = "Also keep the intermediate PDF at this path")]
        save_pdf: Option<String>,
    },
    /// Assert that a page or PDF URL contains the expected text.
    AssertPdfText {
        #[arg(help = "URL of the page or PDF to check")]
        url: String,
        #[arg(long, help = "Text that must appear in the PDF")]
        text: String,
        #[arg(long, help = "Also keep the intermediate PDF at this path")]
        save_pdf: Option<String>,
    },
    /// Upload a local file into a file input.
    ChooseFile {
        #[arg(help = "CSS selector of the <input type=file> element")]
        selector: String,
        #[arg(help = "Local file to upload")]
        file: String,
        #[arg(
            long,
            help = "Page to open first (defaults to a generated blank upload page)"
        )]
        url: Option<String>,
    },
    /// Run several assertions, then report every failure at once.
    Deferred {
        #[arg(long, help = "URL to open before running the assertions")]
        url: Option<String>,
        #[arg(
            long = "assert",
            value_name = "SPEC",
            help = "Assertion spec: element=<css>, text=<css>:<text>, title=<text>, or url=<text>"
        )]
        asserts: Vec<String>,
        #[arg(long, help = "JSON file containing an array of assertion specs")]
        spec: Option<String>,
    },
    /// Record browser interactions into a replayable action file.
    Record {
        #[arg(help = "URL to open and record")]
        url: String,
        #[arg(
            short,
            long,
            default_value = "actions.json",
            help = "Output action file (.json or .yaml)"
        )]
        output: String,
        #[arg(
            long,
            help = "Stop recording after this many seconds (default: run until Ctrl-C)"
        )]
        duration: Option<u64>,
    },
    /// Replay a recorded action file against a fresh browser session.
    Replay {
        #[arg(help = "JSON action file produced by `sbase record`")]
        file: String,
    },
}

/// The error of a token that would not open. The library's own error names the cipher and its
/// hint is about browsers; the usual cause is the passphrase, which the person gave.
fn wrong_passphrase(
    error: seleniumbase_rs::SeleniumBaseError,
) -> seleniumbase_rs::SeleniumBaseError {
    match error {
        seleniumbase_rs::SeleniumBaseError::Unsupported(_) => {
            seleniumbase_rs::SeleniumBaseError::invalid_config(
                "decryption failed: the passphrase in SB_ENCRYPTION_KEY is wrong, or the token \
                 was modified",
            )
        }
        other => other,
    }
}

/// Return the on-disk path a URL-ish argument points at, when it is local.
fn local_path_for(url: &str) -> Option<PathBuf> {
    let candidate = match url.strip_prefix("file://") {
        Some(rest) => PathBuf::from(rest),
        None if url.contains("://") => return None,
        None => PathBuf::from(url),
    };
    candidate.exists().then_some(candidate)
}

/// Turn a local path or URL into something a browser can navigate to.
fn navigable_url(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    match local_path_for(url) {
        Some(path) if !url.starts_with("file://") => {
            let absolute = std::fs::canonicalize(&path)?;
            Ok(format!("file://{}", absolute.display()))
        }
        _ => Ok(url.to_string()),
    }
}

/// Writes `bytes` to `path`, creating the folder it names first.
fn write_creating_parent(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, bytes)
}

impl Commands {
    /// Whether the command speaks the DevTools Protocol, so that it needs a
    /// session with a DevTools connection when no mode was asked for.
    fn needs_devtools(&self) -> bool {
        matches!(
            self,
            Self::Cdp { .. }
                | Self::CacheClear
                | Self::Throttle3g
                | Self::CdpClickElement { .. }
                | Self::CdpTypeText { .. }
                | Self::PrintPdf { .. }
                | Self::PdfText { .. }
                | Self::AssertPdfText { .. }
        )
    }
}

/// Download `url` to `dest` over plain HTTP. Returns the number of bytes written.
async fn http_download(url: &str, dest: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(dest, &bytes)?;
    Ok(bytes.len() as u64)
}

/// Resolve `url` to a PDF on disk, rendering the page when it is not already a PDF.
///
/// Returns the PDF path plus an optional scratch directory that must outlive it.
async fn resolve_pdf(
    sb: &mut BaseCase,
    url: &str,
    save_pdf: Option<&str>,
) -> Result<(PathBuf, Option<tempfile::TempDir>), Box<dyn std::error::Error>> {
    if let Some(path) = local_path_for(url) {
        if looks_like_pdf(url) {
            return Ok((path, None));
        }
    } else if looks_like_pdf(url) && !url.contains("://") {
        return Err(format!("PDF file not found: {url}").into());
    }

    let (holder, path) = match save_pdf {
        Some(target) => (None, PathBuf::from(target)),
        None => {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("page.pdf");
            (Some(dir), path)
        }
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let target = path.to_str().ok_or("PDF output path is not valid UTF-8")?;

    if looks_like_pdf(url) {
        http_download(url, &path).await?;
    } else {
        sb.open(&navigable_url(url)?).await?;
        sb.print_to_pdf(target).await?;
    }
    Ok((path, holder))
}

/// The passphrase for `encrypt` and `decrypt`.
///
/// It is read from the environment and not from an argument, so it does not
/// end up in shell history or a process listing.
fn encryption_passphrase() -> Result<String, Box<dyn std::error::Error>> {
    match std::env::var("SB_ENCRYPTION_KEY") {
        Ok(passphrase) if !passphrase.is_empty() => Ok(passphrase),
        _ => Err("set the SB_ENCRYPTION_KEY environment variable to the passphrase to use".into()),
    }
}

/// `value` if given, otherwise all of standard input without its trailing
/// newline, so `echo secret | sbase encrypt` works.
fn text_or_stdin(value: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(value) = value {
        return Ok(value);
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
    Ok(text.trim_end_matches(['\n', '\r']).to_owned())
}

/// Runs `grid-hub` or `grid-node` and prints what happened.
async fn run_grid(role: GridRole, args: &GridArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = sb_grid::GridCommand::new(role, args.action.into(), default_state_dir());
    command.jar = args.jar.clone();
    command.java = args.java.clone();
    command.host = args.host.clone();
    command.port = args.port;
    command.hub = args.hub.clone();
    command.session_timeout = args.session_timeout.map(Duration::from_secs);
    command.wait = Duration::from_secs(args.wait);
    command.verbose = args.verbose;
    command.skip_port_check = args.skip_port_check;
    command.extra_args = args.extra_args.clone();
    for line in sb_grid::run(&command).await? {
        println!("{line}");
    }
    Ok(())
}

/// Runs `translate` and prints what happened: the scenario for a print, the
/// file written otherwise, and a note on the actions it did not know.
fn run_translate(
    command: &sb_translate::TranslateCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = sb_translate::run(command)?;
    match &report.written {
        Some(path) => println!(
            "Wrote {} ({} steps in {})",
            path.display(),
            report.translated,
            command.language
        ),
        None => print!("{}", report.text),
    }
    if !report.unknown.is_empty() {
        eprintln!(
            "sbase: left {} unknown action(s) as they were: {}",
            report.unknown.len(),
            report.unknown.join(", ")
        );
    }
    Ok(())
}

/// What the commands that are terminal apps do in a build without the `tui`
/// feature: say how to get it, and leave the terminal alone.
#[cfg(not(feature = "tui"))]
fn no_terminal_ui() -> ! {
    eprintln!(
        "this sbase was built without the terminal UI; \
         reinstall it with `cargo install seleniumbase-rs --features tui`"
    );
    std::process::exit(2);
}

/// Where the file-generating commands create files: the current directory.
fn here(replace: bool) -> Scaffold {
    Scaffold::new(".").replacing(replace)
}

/// Reports what a file-generating command created, or why it could not: the
/// reason goes to standard error and the exit status is 1.
fn report_created(outcome: Result<Vec<PathBuf>, ScaffoldError>) {
    match outcome {
        Ok(paths) => {
            for path in paths {
                println!("Created {}", path.display());
            }
        }
        Err(error) => {
            eprintln!("sbase: {error}");
            std::process::exit(1);
        }
    }
}

/// `error` with what the command was doing in front of its message.
///
/// The result is an I/O error of the same kind, so `main` still gives it the
/// exit status of a failed local file operation.
fn context(doing: &str, error: std::io::Error) -> std::io::Error {
    std::io::Error::new(error.kind(), format!("failed to {doing}: {error}"))
}

async fn run_doctor() -> Result<(), Box<dyn std::error::Error>> {
    println!("seleniumbase-rs environment diagnostics");
    println!("========================================");

    let runtime = RuntimeConfig::from_env().unwrap_or_default();
    println!("SB_WEBDRIVER_URL: {}", runtime.webdriver_url);
    println!(
        "SB_CHROME_BIN: {}",
        runtime
            .chrome_bin
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(auto-detect)".to_owned())
    );
    println!(
        "SB_PATCH_CACHE_DIR: {}",
        runtime
            .patch_cache_dir
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(platform cache)".to_owned())
    );
    println!("SB_LOG_LEVEL: {}", runtime.log_level);
    println!("SB_LOG_FORMAT: {:?}", runtime.log_format);
    println!(
        "SB_SHUTDOWN_TIMEOUT_SECS: {}",
        runtime.shutdown_timeout.as_secs()
    );

    println!();
    println!("Browser binary");
    let chrome_bin = runtime.chrome_bin.clone().or_else(find_system_chrome);
    match &chrome_bin {
        Some(path) if path.exists() => {
            println!("  Found: {}", path.display());
        }
        Some(path) => {
            println!("  Configured path missing: {}", path.display());
            println!("  Hint: set SB_CHROME_BIN to a valid Chrome/Chromium executable");
        }
        None => {
            println!("  Not found");
            println!("  Hint: install Google Chrome/Chromium or set SB_CHROME_BIN");
        }
    }

    println!();
    println!("Patch cache");
    if let Some(path) = &chrome_bin {
        let patcher = ChromeBinaryPatcher::new(path).with_cache_dir(
            runtime.patch_cache_dir.clone().unwrap_or_else(|| {
                dirs::cache_dir()
                    .unwrap_or_default()
                    .join("sb-chrome-patches")
            }),
        );
        match patcher.patched_path() {
            Ok(p) => {
                if p.exists() {
                    println!("  Cached patched binary: {}", p.display());
                } else {
                    println!("  Cache path: {}", p.display());
                    println!(
                        "  Hint: run `sbase patch-chrome --path <chrome>` to populate the cache"
                    );
                }
            }
            Err(e) => {
                println!("  Unable to compute cache path: {e}");
                println!("  Hint: verify the Chrome path and cache directory are writable");
            }
        }
    } else {
        println!("  Skipped (no Chrome binary found)");
    }

    println!();
    println!("Chromedriver");
    match assets::lookup_chromedriver() {
        assets::DriverLookup::Found(path) => {
            println!("  Found: {}", path.display());
            let driver = binary_major(&path).await;
            let browser = match find_system_chrome() {
                Some(chrome) => binary_major(&chrome).await,
                None => None,
            };
            match (driver, browser) {
                (Some(driver), Some(browser)) if driver == browser => {
                    println!("  Version: {driver} (matches Chrome {browser})");
                }
                (Some(driver), Some(browser)) => {
                    println!("  Version: {driver}, but Chrome is {browser}");
                    println!(
                        "  Hint: a chromedriver only drives its own Chrome version; \
                         run `sbase install` to fetch the matching one"
                    );
                }
                (Some(driver), None) => println!("  Version: {driver}"),
                (None, _) => {}
            }
            match ChromedriverPatcher::new(&path).needs_patch() {
                Ok(true) => {
                    println!("  Status: contains automation markers");
                    println!(
                        "  Hint: run `sbase patch-chromedriver --path {}`",
                        path.display()
                    );
                }
                Ok(false) => {
                    println!("  Status: no obvious automation markers");
                }
                Err(e) => {
                    println!("  Status: unable to inspect binary ({e})");
                }
            }
        }
        assets::DriverLookup::MissingConfigured(path) => {
            println!(
                "  {} is set to {}, which is not a file",
                assets::CHROMEDRIVER_PATH_VAR,
                path.display()
            );
            println!("  Hint: fix or unset CHROMEDRIVER_PATH, or run `sbase install`");
        }
        assets::DriverLookup::NotFound => {
            println!("  Not found in {}/ or on PATH", assets::DRIVERS_DIR);
            println!("  Hint: run `sbase install` or set CHROMEDRIVER_PATH");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match execute().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // `error:`, its causes and a `hint:`, and an exit status that
            // says which kind of failure it was (see `exit_code_for`).
            rich_helper::print_error_report(error.as_ref());
            std::process::ExitCode::from(rich_helper::exit_code_for(error.as_ref()))
        }
    }
}

async fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = RuntimeConfig::from_env().unwrap_or_default();
    init_tracing_from_runtime(&runtime);

    let args = Cli::parse();
    let is_record = matches!(args.command, Commands::Record { .. });
    // `record` stops itself on SIGINT or SIGTERM so that it can save what it
    // captured. A signal cancels any other command, which closes its browser.
    //
    // `run` holds the locals of every command, so its future is large. It is
    // boxed so that it lives on the heap: kept on the main thread's stack, and
    // copied again by the signal race, it overflowed the 8 MiB stack in debug
    // builds as soon as a command compiled a few regular expressions.
    let work = Box::pin(run(args));
    let outcome = if is_record {
        Outcome::Completed(work.await)
    } else {
        run_until_shutdown(work).await?
    };
    // Browsers dropped on the way out are closed in the background; wait.
    drain_cleanups(timeout_from_env()).await;
    match outcome {
        Outcome::Completed(result) => result,
        Outcome::Interrupted(signal) => {
            eprintln!("{signal} received; stopped.");
            std::process::exit(signal.exit_code());
        }
    }
}

/// Which importer `import_script` runs.
#[derive(Clone, Copy, Debug)]
enum ScriptTool {
    Playwright,
    Puppeteer,
}

/// `sbase import-playwright` and `sbase import-puppeteer`: the Rust goes to
/// `output` or standard output, one diagnostic line per `TODO` to standard
/// error. As with `import-python`, a gap is a warning and the exit status is
/// 0; a statement the importer could not read at all is an error (status 1);
/// a file it cannot read fails with the I/O status, and a source it refuses
/// (too large, the wrong tool) with status 2.
fn import_script(
    tool: ScriptTool,
    file: &str,
    output: Option<&str>,
    language: Option<ScriptLanguage>,
    test_name: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(file).map_err(|error| {
        std::io::Error::new(error.kind(), format!("failed to read {file}: {error}"))
    })?;
    let path = Path::new(file);
    let default_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("imported_test");
    let options = ScriptImportOptions {
        language: language.or_else(|| ScriptLanguage::from_path(path)),
        test_name: test_name.unwrap_or(default_name).to_owned(),
    };
    let (result, name) = match tool {
        ScriptTool::Playwright => (import_playwright(&source, &options)?, "Playwright"),
        ScriptTool::Puppeteer => (import_puppeteer(&source, &options)?, "Puppeteer"),
    };
    if let Some(path) = output {
        std::fs::write(path, &result.rust)?;
    } else {
        print!("{}", result.rust);
    }
    for diagnostic in &result.diagnostics {
        eprintln!(
            "{:?} at line {}: {}",
            diagnostic.severity, diagnostic.line, diagnostic.message
        );
    }
    if result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ImportSeverity::Error)
    {
        return Err(format!("{name} import completed with errors; review generated TODOs.").into());
    }
    Ok(())
}

async fn run(args: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match &args.command {
        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_owned();
            let mut completion = Vec::new();
            generate(*shell, &mut command, name, &mut completion);
            std::io::stdout().write_all(&completion)?;
            return Ok(());
        }
        Commands::ImportPython {
            file,
            output,
            source,
            test_name,
        } => {
            let python = std::fs::read_to_string(file).map_err(|error| {
                std::io::Error::new(error.kind(), format!("failed to read {file}: {error}"))
            })?;
            let default_name = std::path::Path::new(file)
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("imported_python_test");
            let result = import_python(
                &python,
                &ImportOptions {
                    source: match source {
                        ImportSourceArg::Auto => PythonSource::Auto,
                        ImportSourceArg::SeleniumBase => PythonSource::SeleniumBase,
                        ImportSourceArg::Selenium => PythonSource::Selenium,
                    },
                    test_name: test_name.as_deref().unwrap_or(default_name).to_owned(),
                },
            );
            if let Some(path) = output {
                std::fs::write(path, &result.rust)?;
            } else {
                print!("{}", result.rust);
            }
            for diagnostic in &result.diagnostics {
                eprintln!(
                    "{:?} at line {}: {}",
                    diagnostic.severity, diagnostic.line, diagnostic.message
                );
            }
            if result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == ImportSeverity::Error)
            {
                return Err("Python import completed with errors; review generated TODOs.".into());
            }
            return Ok(());
        }
        Commands::ImportPlaywright {
            file,
            output,
            language,
            test_name,
        } => {
            return import_script(
                ScriptTool::Playwright,
                file,
                output.as_deref(),
                language.map(ScriptLanguage::from),
                test_name.as_deref(),
            );
        }
        Commands::ImportPuppeteer {
            file,
            output,
            language,
            test_name,
        } => {
            return import_script(
                ScriptTool::Puppeteer,
                file,
                output.as_deref(),
                language.map(ScriptLanguage::from),
                test_name.as_deref(),
            );
        }
        Commands::Mkdir { dir, basic, force } => {
            report_created(
                Suite::new(dir).and_then(|suite| suite.basic(*basic).create(&here(*force))),
            );
            return Ok(());
        }
        Commands::Mkfile {
            file,
            url,
            basic,
            force,
        } => {
            report_created(
                TestFile::new(file)
                    .and_then(|test| match url {
                        Some(url) => test.url(url),
                        None => Ok(test),
                    })
                    .and_then(|test| test.basic(*basic).create(&here(*force)))
                    .map(|path| vec![path]),
            );
            return Ok(());
        }
        Commands::Translate {
            file,
            language,
            overwrite,
            copy,
            ..
        } => {
            let action = if *overwrite {
                sb_translate::TranslateAction::Overwrite
            } else if *copy {
                sb_translate::TranslateAction::Copy
            } else {
                sb_translate::TranslateAction::Print
            };
            return run_translate(&sb_translate::TranslateCommand {
                file: file.clone(),
                language: *language,
                action,
            });
        }
        Commands::GridHub(grid) => return run_grid(GridRole::Hub, grid).await,
        Commands::GridNode(grid) => return run_grid(GridRole::Node, grid).await,
        _ => {}
    }

    if args.cdp && args.uc {
        return Err("Choose either --cdp or --uc, not both.".into());
    }

    // Start from the config file (the one `--config` names, else the global
    // one) with the `SB_*` variables on top, as `Settings::load` does for the
    // tests `sbase test` runs, then apply only what the command line asked for
    // explicitly. An option left alone keeps the environment's value or, failing
    // that, the file's, and one that was passed beats both.
    let mut settings = Settings::load(args.config.as_deref())?;
    let proxy = match args.proxy_list.as_deref() {
        Some(path) => {
            let list = ProxyList::from_file(path)?;
            match args.proxy.as_deref() {
                Some(name_or_proxy) => Some(list.resolve(name_or_proxy)?),
                None => {
                    let entry = list
                        .random()
                        .ok_or_else(|| format!("the proxy list {path} is empty"))?;
                    rich_helper::print_notice(&format!("Using proxy {entry}"));
                    Some(entry.spec().to_owned())
                }
            }
        }
        None => args.proxy,
    };
    let overrides = SettingsOverrides {
        browser: args
            .browser
            .and_then(|browser| browser.to_possible_value())
            .map(|value| value.get_name().to_owned()),
        headless: args
            .headless
            .then_some(true)
            .or_else(|| args.headed.then_some(false)),
        mode: args
            .cdp
            .then(|| "cdp".to_owned())
            .or_else(|| args.uc.then(|| "uc".to_owned())),
        user_agent: args.user_agent.or(args.agent),
        locale: args.locale,
        ad_block: args.ad_block.then_some(true),
        proxy,
        proxy_pac_url: args.proxy_pac_url,
        user_data_dir: args.user_data_dir,
        extension_dir: args.extension_dir,
        reuse_session: args.reuse_session.then_some(true),
        reuse_class_session: args.reuse_class_session.then_some(true),
        mobile: args.mobile.then_some(true),
        threads: args.threads,
        esc_end: args.esc_end.then_some(true),
        rec_sb_cdp: args.rec_sb_cdp.then_some(true),
    };
    overrides.apply(&mut settings);

    // The runtime variables the library reads for a `BrowserConfig`: the
    // WebDriver server (unless `--webdriver` names one) and the browser binary.
    let runtime = RuntimeConfig::from_env()?;
    let mut config = settings.to_browser_config();
    // A command that speaks the DevTools Protocol gets a session that has a
    // DevTools connection when no mode was asked for, on a Chromium browser.
    let default_mode = if args.command.needs_devtools() && config.browser.is_chromium_based() {
        DriverMode::Cdp
    } else {
        DriverMode::WebDriver
    };
    let mode = settings
        .mode
        .as_deref()
        .map(|m| match m.to_lowercase().as_str() {
            "uc" => DriverMode::Uc,
            "cdp" => DriverMode::Cdp,
            _ => DriverMode::WebDriver,
        })
        .unwrap_or(default_mode);
    config.webdriver_url = args.webdriver.unwrap_or(runtime.webdriver_url);
    config.browser_binary_path = runtime.chrome_bin;
    config.mode = mode;
    config.auto_start_driver = true;

    match args.command {
        Commands::Open { url } => {
            let mut sb = BaseCase::new(config).await?;
            sb.open(&url).await?;
            let title = sb.get_title().await?;
            println!("Title: {title}");
            sb.quit().await?;
        }
        Commands::Smoke {
            url,
            title_contains,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.open(&url).await?;
            if let Some(expected) = title_contains.as_deref() {
                sb.assert_title_contains(expected).await?;
                println!("Assertion passed: title contains '{expected}'");
            } else {
                println!("Loaded: {}", sb.get_title().await?);
            }
            sb.quit().await?;
        }
        Commands::Cdp { cmd, params } => {
            let mut sb = BaseCase::new(config).await?;
            let result = if let Some(raw_params) = params.as_deref() {
                let parsed: Value = serde_json::from_str(raw_params)?;
                sb.execute_cdp_with_params(&cmd, parsed).await?
            } else {
                sb.execute_cdp(&cmd).await?
            };
            println!("{result}");
            sb.quit().await?;
        }
        Commands::CacheClear => {
            let mut sb = BaseCase::new(config).await?;
            sb.clear_browser_cache().await?;
            println!("CDP cache clear command sent.");
            sb.quit().await?;
        }
        Commands::Throttle3g => {
            let mut sb = BaseCase::new(config).await?;
            let mut conditions = NetworkConditions::new();
            conditions.offline = false;
            conditions.latency = 200;
            conditions.download_throughput = 256 * 1024;
            conditions.upload_throughput = 64 * 1024;
            sb.set_network_conditions(&conditions).await?;
            let result = sb
                .execute_cdp_with_params("Network.setCacheDisabled", json!({"cacheDisabled": true}))
                .await?;
            println!("3G throttle enabled. {result}");
            sb.quit().await?;
        }
        Commands::Screenshot { path } => {
            let mut sb = BaseCase::new(config).await?;
            if let Some(target_path) = path.as_deref() {
                // The path the user named on the command line, not one in the logs folder.
                let png = sb.screenshot_as_png().await?;
                write_creating_parent(Path::new(target_path), &png)?;
                println!("Saved screenshot: {target_path}");
            } else {
                let out = sb.save_screenshot_to_logs().await?;
                println!("Saved screenshot: {}", out.display());
            }
            sb.quit().await?;
        }
        Commands::SaveSource { path } => {
            let mut sb = BaseCase::new(config).await?;
            if let Some(target_path) = path.as_deref() {
                let source = sb.get_page_source().await?;
                write_creating_parent(Path::new(target_path), source.as_bytes())?;
                println!("Saved page source: {target_path}");
            } else {
                let out = sb.save_page_source_to_logs().await?;
                println!("Saved page source: {}", out.display());
            }
            sb.quit().await?;
        }
        Commands::AssertElement { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_element(&css).await?;
            println!("Assertion passed: element exists for selector '{css}'");
            sb.quit().await?;
        }
        Commands::WaitForText { css, text, timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_text(&css, &text, timeout).await?;
            println!("Text found for selector '{css}': {text}");
            sb.quit().await?;
        }
        Commands::Hover { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.hover(&css).await?;
            println!("Hovered over element '{css}'");
            sb.quit().await?;
        }
        Commands::HoverAndClick {
            hover_css,
            click_css,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.hover_and_click(&hover_css, &click_css).await?;
            println!("Hovered '{hover_css}' and clicked '{click_css}'");
            sb.quit().await?;
        }
        Commands::SelectOption { css, text, value } => {
            let mut sb = BaseCase::new(config).await?;
            if let Some(t) = text {
                sb.select_option_by_text(&css, &t).await?;
                println!("Selected option by text '{t}' on '{css}'");
            } else if let Some(v) = value {
                sb.select_option_by_value(&css, &v).await?;
                println!("Selected option by value '{v}' on '{css}'");
            } else {
                println!("Must provide either --text or --value for SelectOption");
            }
            sb.quit().await?;
        }
        Commands::DragAndDrop {
            source_css,
            target_css,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.drag_and_drop(&source_css, &target_css).await?;
            println!("Dragged '{source_css}' and dropped on '{target_css}'");
            sb.quit().await?;
        }
        Commands::CdpClickElement { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.cdp_click_element(&css).await?;
            println!("CDP clicked element '{css}'");
            sb.quit().await?;
        }
        Commands::CdpTypeText { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.cdp_type_text(&text).await?;
            println!("CDP typed text '{text}'");
            sb.quit().await?;
        }

        Commands::GoBack => {
            let mut sb = BaseCase::new(config).await?;
            sb.go_back().await?;
            println!("Went back");
            sb.quit().await?;
        }
        Commands::GoForward => {
            let mut sb = BaseCase::new(config).await?;
            sb.go_forward().await?;
            println!("Went forward");
            sb.quit().await?;
        }
        Commands::Refresh => {
            let mut sb = BaseCase::new(config).await?;
            sb.refresh().await?;
            println!("Refreshed page");
            sb.quit().await?;
        }

        Commands::GetText { css } => {
            let mut sb = BaseCase::new(config).await?;
            let text = sb.get_text(&css).await?;
            println!("Text for '{}': {}", css, text);
            sb.quit().await?;
        }
        Commands::GetAttribute { css, attribute } => {
            let mut sb = BaseCase::new(config).await?;
            let val = sb.get_attribute(&css, &attribute).await?;
            if let Some(v) = val {
                println!("Attribute '{}' for '{}': {}", attribute, css, v);
            } else {
                println!("Attribute '{}' not found for '{}'", attribute, css);
            }
            sb.quit().await?;
        }
        Commands::GetProperty { css, property } => {
            let mut sb = BaseCase::new(config).await?;
            let val = sb.get_property(&css, &property).await?;
            if let Some(v) = val {
                println!("Property '{}' for '{}': {}", property, css, v);
            } else {
                println!("Property '{}' not found for '{}'", property, css);
            }
            sb.quit().await?;
        }

        Commands::GetTitle => {
            let mut sb = BaseCase::new(config).await?;
            let title = sb.get_title().await?;
            println!("Title: {}", title);
            sb.quit().await?;
        }
        Commands::GetCurrentUrl => {
            let mut sb = BaseCase::new(config).await?;
            let url = sb.get_current_url().await?;
            println!("Current URL: {}", url);
            sb.quit().await?;
        }

        Commands::ClearCookies => {
            let mut sb = BaseCase::new(config).await?;
            sb.clear_browser_cookies().await?;
            println!("Cookies cleared");
            sb.quit().await?;
        }

        Commands::AcceptAlert => {
            let mut sb = BaseCase::new(config).await?;
            sb.accept_alert().await?;
            println!("Accepted alert");
            sb.quit().await?;
        }
        Commands::DismissAlert => {
            let mut sb = BaseCase::new(config).await?;
            sb.dismiss_alert().await?;
            println!("Dismissed alert");
            sb.quit().await?;
        }
        Commands::GetAlertText => {
            let mut sb = BaseCase::new(config).await?;
            let text = sb.get_alert_text().await?;
            println!("Alert text: {}", text);
            sb.quit().await?;
        }
        Commands::TypeAlertText { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.type_alert_text(&text).await?;
            println!("Typed alert text: {}", text);
            sb.quit().await?;
        }
        Commands::ClearLocalStorage => {
            let mut sb = BaseCase::new(config).await?;
            sb.clear_local_storage().await?;
            println!("Cleared local storage");
            sb.quit().await?;
        }
        Commands::GetLocalStorageItem { key } => {
            let mut sb = BaseCase::new(config).await?;
            let val = sb.get_local_storage_item(&key).await?;
            println!("Local storage item '{}': {}", key, val);
            sb.quit().await?;
        }
        Commands::SetLocalStorageItem { key, value } => {
            let mut sb = BaseCase::new(config).await?;
            sb.set_local_storage_item(&key, &value).await?;
            println!("Set local storage item '{}' to '{}'", key, value);
            sb.quit().await?;
        }
        Commands::RemoveLocalStorageItem { key } => {
            let mut sb = BaseCase::new(config).await?;
            sb.remove_local_storage_item(&key).await?;
            println!("Removed local storage item '{}'", key);
            sb.quit().await?;
        }
        Commands::SwitchToWindow { handle } => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_window(&handle).await?;
            println!("Switched to window '{}'", handle);
            sb.quit().await?;
        }
        Commands::SwitchToFrame { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_frame(&css).await?;
            println!("Switched to frame '{}'", css);
            sb.quit().await?;
        }
        Commands::SwitchToDefaultContent => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_default_content().await?;
            println!("Switched to default content");
            sb.quit().await?;
        }

        Commands::GetCookies => {
            let mut sb = BaseCase::new(config).await?;
            let cookies = sb.get_cookies().await?;
            println!("Cookies: {:?}", cookies);
            sb.quit().await?;
        }
        Commands::ExportRecording => {
            let mut sb = BaseCase::new(config).await?;
            let (json_file, rust_file) = sb.save_recording_to_logs()?;
            println!("Saved recording json: {}", json_file.display());
            println!("Saved recording script: {}", rust_file.display());
            sb.quit().await?;
        }
        Commands::PatchChromedriver { path } => {
            seleniumbase_rs::stealth::patcher::patch_chromedriver(&path)?;
            println!("Successfully patched chromedriver binary at: {path}");
        }
        Commands::PatchChrome { path, cache_dir } => {
            let cache = cache_dir
                .as_deref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    dirs::cache_dir()
                        .unwrap_or_default()
                        .join("sb-chrome-patches")
                });
            let patcher = ChromeBinaryPatcher::new(path).with_cache_dir(cache);
            let patched = patcher.patch(EnginePatch::chrome_binary())?;
            println!("Patched Chrome binary available at: {}", patched.display());
        }
        Commands::Version => println!("sbase {VERSION}"),
        Commands::Encrypt { text } => {
            let passphrase = encryption_passphrase()?;
            let text = text_or_stdin(text)?;
            println!("{}", encrypt_with_passphrase(&text, &passphrase)?);
        }
        Commands::Decrypt { token } => {
            let passphrase = encryption_passphrase()?;
            let token = text_or_stdin(token)?;
            let plain = decrypt_with_passphrase(&token, &passphrase).map_err(wrong_passphrase)?;
            println!("{plain}");
        }
        #[cfg(feature = "turso")]
        Commands::Report {
            db,
            run,
            failed,
            flaky,
            limit,
            json,
        } => {
            report::run(db, run, failed, flaky, limit, json).await?;
        }
        Commands::Doctor => {
            run_doctor().await?;
        }
        Commands::AssertTextVisible { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_text_visible(&text, &css).await?;
            println!("Text '{}' is visible in '{}'", text, css);
            sb.quit().await?;
        }
        Commands::AssertTextNotVisible { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_text_not_visible(&text, &css).await?;
            println!("Text '{}' is not visible in '{}'", text, css);
            sb.quit().await?;
        }
        Commands::AssertAttribute {
            css,
            attribute,
            value,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_attribute(&css, &attribute, &value).await?;
            println!("Attribute '{}' of '{}' is '{}'", attribute, css, value);
            sb.quit().await?;
        }
        Commands::AssertTitle { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_title(&text).await?;
            println!("Title is '{}'", text);
            sb.quit().await?;
        }
        Commands::WaitForReadyStateComplete => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_ready_state_complete().await?;
            println!("Ready state complete");
            sb.quit().await?;
        }
        Commands::GetWindowPosition => {
            let mut sb = BaseCase::new(config).await?;
            let (x, y) = sb.get_window_position().await?;
            println!("Window position: x={}, y={}", x, y);
            sb.quit().await?;
        }
        Commands::SetWindowPosition { x, y } => {
            let mut sb = BaseCase::new(config).await?;
            sb.set_window_position(x, y).await?;
            println!("Set window position to x={}, y={}", x, y);
            sb.quit().await?;
        }
        Commands::CloseWindow => {
            let mut sb = BaseCase::new(config).await?;
            sb.close_window().await?;
            println!("Closed window");
            sb.quit().await?;
        }
        Commands::SwitchToParentFrame => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_parent_frame().await?;
            println!("Switched to parent frame");
            sb.quit().await?;
        }
        Commands::IsElementVisible { css } => {
            let mut sb = BaseCase::new(config).await?;
            let visible = sb.is_element_visible(&css).await?;
            println!("Element '{}' is visible: {}", css, visible);
            sb.quit().await?;
        }
        Commands::IsTextVisible { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            let visible = sb.is_text_visible(&text, &css).await?;
            println!("Text '{}' in '{}' is visible: {}", text, css, visible);
            sb.quit().await?;
        }
        Commands::WaitForElementNotVisible { css, timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_element_not_visible(&css, timeout).await?;
            println!("Element '{}' is not visible", css);
            sb.quit().await?;
        }
        Commands::SaveCookies { file } => {
            let sb = BaseCase::new(config).await?;
            sb.save_cookies(&file).await?;
            println!("Saved cookies to '{}'", file);
        }
        Commands::LoadCookies { file } => {
            let sb = BaseCase::new(config).await?;
            sb.load_cookies(&file).await?;
            println!("Loaded cookies from '{}'", file);
        }
        Commands::HighlightClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.highlight_click(&css).await?;
            println!("Highlighted and clicked '{}'", css);
        }
        Commands::IsChecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            let checked = sb.is_checked(&css).await?;
            println!("Element '{}' is checked: {}", css, checked);
        }
        Commands::CheckIfUnchecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.check_if_unchecked(&css).await?;
            println!("Checked '{}'", css);
        }
        Commands::UncheckIfChecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uncheck_if_checked(&css).await?;
            println!("Unchecked '{}'", css);
        }
        Commands::OpenNewWindow => {
            let mut sb = BaseCase::new(config).await?;
            sb.open_new_window().await?;
            println!("Opened new window");
        }
        Commands::OpenNewTab => {
            let mut sb = BaseCase::new(config).await?;
            sb.open_new_tab().await?;
            println!("Opened new tab");
        }
        Commands::SwitchToNewestWindow => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_newest_window().await?;
            println!("Switched to newest window");
        }
        Commands::SwitchToDefaultWindow => {
            let mut sb = BaseCase::new(config).await?;
            sb.switch_to_default_window().await?;
            println!("Switched to default window");
        }
        Commands::GetActiveElementCss => {
            let mut sb = BaseCase::new(config).await?;
            let css = sb.get_active_element_css().await?;
            println!("Active element CSS: {}", css);
            sb.quit().await?;
        }
        Commands::WaitForElementPresent { css, timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_element_present(&css, timeout).await?;
            println!("Element '{}' is present", css);
        }
        Commands::AddText { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.add_text(&css, &text).await?;
            println!("Added text to '{}'", css);
        }
        Commands::SendKeys { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.send_keys(&css, &text).await?;
            println!("Sent keys to '{}'", css);
        }
        Commands::GetValue { css } => {
            let mut sb = BaseCase::new(config).await?;
            let value = sb.get_value(&css).await?;
            println!("Value of '{}': {}", css, value);
        }
        Commands::ClickVisibleElements { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.click_visible_elements(&css).await?;
            println!("Clicked visible elements matching '{}'", css);
        }
        Commands::WaitForAndAcceptAlert { timeout } => {
            let sb = BaseCase::new(config).await?;
            sb.wait_for_and_accept_alert(timeout).await?;
            println!("Waited for and accepted alert");
        }
        Commands::WaitForAndDismissAlert { timeout } => {
            let sb = BaseCase::new(config).await?;
            sb.wait_for_and_dismiss_alert(timeout).await?;
            println!("Waited for and dismissed alert");
        }
        Commands::IsLinkTextVisible { text } => {
            let sb = BaseCase::new(config).await?;
            let visible = sb.is_link_text_visible(&text).await?;
            println!("Link text '{}' is visible: {}", text, visible);
        }
        Commands::IsPartialLinkTextVisible { text } => {
            let sb = BaseCase::new(config).await?;
            let visible = sb.is_partial_link_text_visible(&text).await?;
            println!("Partial link text '{}' is visible: {}", text, visible);
        }
        Commands::AssertLinkText { text } => {
            let sb = BaseCase::new(config).await?;
            sb.assert_link_text(&text).await?;
            println!("Link text '{}' asserted", text);
        }
        Commands::ClickPartialLinkText { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.click_partial_link_text(&text).await?;
            println!("Clicked partial link text '{}'", text);
        }
        Commands::HumanType { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.human_type(&css, &text).await?;
            println!("Human typed into '{}'", css);
        }
        Commands::HumanClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.human_click(&css).await?;
            println!("Human clicked '{}'", css);
        }
        Commands::SmoothScrollTo { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.smooth_scroll_to(&css).await?;
            println!("Smooth scrolled to '{}'", css);
        }
        Commands::UcClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uc_click(&css).await?;
            println!("UC clicked '{}'", css);
        }
        Commands::UcType { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uc_type(&css, &text).await?;
            println!("UC typed into '{}'", css);
        }
        Commands::Install { driver, version } => {
            let name: sb_install::DriverName = driver.parse()?;
            let path = sb_install::install_driver(name, version.as_deref()).await?;
            println!("{name} installed at {}", path.display());
        }
        Commands::Test {
            filter,
            test,
            example,
            release,
            features,
            harness_args,
        } => {
            let target = match (test, example) {
                (_, Some(name)) => run::TestTarget::Example(name),
                (Some(name), None) => run::TestTarget::Test(name),
                (None, None) => run::TestTarget::All,
            };
            let mut test_run = run::TestRun::new()
                .target(target)
                .release(release)
                .features(features)
                .harness_args(harness_args);
            if let Some(filter) = filter {
                test_run = test_run.filter(filter);
            }
            if let Some(threads) = settings.threads {
                test_run = test_run.threads(threads);
            }
            // Only what the command line asked for: the tests load the config
            // file and the rest of the environment themselves.
            for (name, value) in overrides.env() {
                test_run = test_run.env(name, value);
            }
            if config.webdriver_url != "http://localhost:4444" {
                test_run = test_run.env("SB_WEBDRIVER_URL", config.webdriver_url.clone());
            }
            let status = test_run.status()?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::Mkdir { .. }
        | Commands::Mkfile { .. }
        | Commands::Translate { .. }
        | Commands::GridHub(_)
        | Commands::GridNode(_) => {
            unreachable!("these commands return before browser configuration")
        }
        #[cfg(feature = "tui")]
        Commands::Commander => {
            if let Err(e) = sb_commander::run_commander() {
                eprintln!("Failed to run commander: {}", e);
                std::process::exit(1);
            }
        }
        #[cfg(not(feature = "tui"))]
        Commands::Commander => no_terminal_ui(),
        // The file-writing commands below hand their error to `main`, which
        // prints `error: ...` and exits with the status of its kind (6 for a
        // local file), instead of printing it here and exiting with 0.
        Commands::Caseplans { starter: true, .. } => {
            let path =
                sb_caseplans::run_caseplans().map_err(|e| context("generate case plans", e))?;
            println!("Generated {}", path.display());
        }
        #[cfg(feature = "tui")]
        Commands::Caseplans { path, .. } => {
            let target = PathBuf::from(path.as_deref().unwrap_or("."));
            if let Err(e) = caseplans_tui::run(&target) {
                eprintln!("Failed to run the case plans generator: {}", e);
                std::process::exit(1);
            }
        }
        #[cfg(not(feature = "tui"))]
        Commands::Caseplans { .. } => no_terminal_ui(),
        Commands::BehaveGui { new: true, .. } => {
            let path =
                sb_behave_gui::run_gui().map_err(|e| context("create the BDD feature file", e))?;
            println!("Created BDD feature file at {}", path.display());
        }
        #[cfg(feature = "tui")]
        Commands::BehaveGui { path, .. } => {
            let target = PathBuf::from(path.as_deref().unwrap_or(feature_files::DEFAULT_FOLDER));
            if let Err(e) = behave_gui_tui::run(&target) {
                eprintln!("Failed to run the behave commander: {}", e);
                std::process::exit(1);
            }
        }
        #[cfg(not(feature = "tui"))]
        Commands::BehaveGui { .. } => no_terminal_ui(),
        Commands::Print { file } => {
            let content =
                sb_print::print_file(&file).map_err(|e| context(&format!("read {file}"), e))?;
            println!("{}", content);
        }
        Commands::Objectify => {
            let path = sb_objectify::objectify_page()
                .map_err(|e| context("generate the page object", e))?;
            println!("Generated {}", path.display());
        }
        Commands::Mkpres { file } => {
            let path = sb_mkpres::make_presentation(&file)
                .map_err(|e| context("create the presentation", e))?;
            println!("Created presentation at {}", path.display());
        }
        Commands::Mkchart { file } => {
            let path = sb_mkchart::make_chart(&file).map_err(|e| context("create the chart", e))?;
            println!("Created chart at {}", path.display());
        }
        Commands::Mkrec { file } => {
            let path = sb_recorder::make_recorder_file(&file)
                .map_err(|e| context("create the recorder file", e))?;
            println!("Created recorder file at {}", path.display());
        }
        Commands::RunScenario { file, dashboard } => {
            let scenario_json = std::fs::read_to_string(&file)?;
            let scenario: Scenario = serde_json::from_str(&scenario_json)?;
            let mut sb = BaseCase::new(config).await?;
            let summary = run_scenario(&mut sb, &scenario).await?;

            if let Some(dashboard_path) = dashboard.as_deref() {
                write_dashboard_html(&summary, dashboard_path)?;
                println!("Dashboard written: {dashboard_path}");
            } else {
                let logs_dir = ensure_latest_logs_dir()?;
                let dashboard_path = artifact_path(&logs_dir, "dashboard", "html");
                write_dashboard_html(&summary, &dashboard_path)?;
                println!("Dashboard written: {}", dashboard_path.display());
            }

            println!(
                "Scenario '{}' steps: total={}, passed={}, failed={}",
                summary.scenario_name,
                summary.total_steps,
                summary.passed_steps,
                summary.failed_steps
            );
            if !summary.errors.is_empty() {
                println!("Errors:");
                for error in &summary.errors {
                    println!("- {error}");
                }
            }

            let (json_file, rust_file) = sb.save_recording_to_logs()?;
            println!("Recording json: {}", json_file.display());
            println!("Recording script: {}", rust_file.display());
            sb.quit().await?;
            if summary.failed_steps > 0 {
                return Err(format!(
                    "scenario '{}' failed: {} of {} steps failed",
                    summary.scenario_name, summary.failed_steps, summary.total_steps
                )
                .into());
            }
        }
        Commands::DownloadFile {
            url,
            output,
            timeout,
        } => {
            let dest = match output.as_deref() {
                Some(path) => PathBuf::from(path),
                None => std::env::current_dir()?.join(download_file_name(&url)),
            };
            let browser_name = dest
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("download")
                .to_string();

            let mut sb = BaseCase::new(config).await?;
            let downloaded = match sb.download_file(&url, &browser_name, timeout).await {
                Ok(path) => {
                    if path != dest {
                        if let Some(parent) = dest.parent() {
                            if !parent.as_os_str().is_empty() {
                                std::fs::create_dir_all(parent)?;
                            }
                        }
                        std::fs::copy(&path, &dest)?;
                    }
                    true
                }
                Err(err) => {
                    eprintln!("Browser download unavailable ({err}); falling back to HTTP.");
                    false
                }
            };
            sb.quit().await?;
            if !downloaded {
                http_download(&url, &dest).await?;
            }
            let size = std::fs::metadata(&dest).map(|meta| meta.len()).unwrap_or(0);
            println!("Saved {} ({size} bytes)", dest.display());
        }
        Commands::PrintPdf { url, output } => {
            let mut sb = BaseCase::new(config).await?;
            sb.open(&navigable_url(&url)?).await?;
            if let Some(parent) = Path::new(&output).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            sb.print_to_pdf(&output).await?;
            sb.quit().await?;
            let size = std::fs::metadata(&output)
                .map(|meta| meta.len())
                .unwrap_or(0);
            println!("Saved PDF: {output} ({size} bytes)");
        }
        Commands::PdfText { url, save_pdf } => {
            let mut sb = BaseCase::new(config).await?;
            let (path, _scratch) = resolve_pdf(&mut sb, &url, save_pdf.as_deref()).await?;
            let target = path.to_str().ok_or("PDF path is not valid UTF-8")?;
            let text = sb.get_pdf_text(target)?;
            sb.quit().await?;
            println!("{text}");
        }
        Commands::AssertPdfText {
            url,
            text,
            save_pdf,
        } => {
            let mut sb = BaseCase::new(config).await?;
            let (path, _scratch) = resolve_pdf(&mut sb, &url, save_pdf.as_deref()).await?;
            let target = path.to_str().ok_or("PDF path is not valid UTF-8")?;
            let outcome = sb.assert_pdf_text(target, &text);
            sb.quit().await?;
            outcome?;
            println!("Assertion passed: PDF contains '{text}'");
        }
        Commands::ChooseFile {
            selector,
            file,
            url,
        } => {
            let page = match url.as_deref() {
                Some(target) => navigable_url(target)?,
                None => html_data_url(&blank_file_input_page(&selector)),
            };
            let mut sb = BaseCase::new(config).await?;
            sb.open(&page).await?;
            let outcome = sb.choose_file(&selector, &file).await;
            let value = match outcome {
                Ok(()) => sb.get_value(&selector).await.unwrap_or_default(),
                Err(err) => {
                    sb.quit().await?;
                    return Err(err.into());
                }
            };
            sb.quit().await?;
            println!("Uploaded '{file}' into '{selector}'");
            if !value.is_empty() {
                println!("Input value: {value}");
            }
        }
        Commands::Deferred { url, asserts, spec } => {
            let mut specs = parse_deferred_specs(&asserts)?;
            if let Some(path) = spec.as_deref() {
                let raw = std::fs::read_to_string(path)?;
                specs.extend(parse_deferred_spec_json(&raw)?);
            }
            if specs.is_empty() {
                return Err("no assertions provided: pass --assert or --spec".into());
            }

            let mut sb = BaseCase::new(config).await?;
            if let Some(target) = url.as_deref() {
                sb.open(&navigable_url(target)?).await?;
            }
            let mut results = Vec::with_capacity(specs.len());
            for entry in &specs {
                let outcome = match entry {
                    DeferredSpec::Element(selector) => sb.assert_element(selector).await,
                    DeferredSpec::Text(selector, expected) => {
                        sb.assert_text(selector, expected).await
                    }
                    DeferredSpec::Title(expected) => sb.assert_title_contains(expected).await,
                    DeferredSpec::Url(expected) => sb.assert_url_contains(expected).await,
                };
                results.push(DeferredResult {
                    description: entry.describe(),
                    error: outcome.err().map(|err| err.to_string()),
                });
            }
            sb.quit().await?;

            print!("{}", format_deferred_summary(&results));
            if results.iter().any(|result| !result.passed()) {
                std::process::exit(1);
            }
        }
        Commands::Record {
            url,
            output,
            duration,
        } => {
            let mut shutdown = Shutdown::install()?;
            let mut sb = BaseCase::new(config).await?;
            let mut session = RecorderSession::new();
            session.goto(&mut sb, &navigable_url(&url)?).await?;
            session.install(&sb).await?;

            match duration {
                Some(secs) => println!("Recording for {secs}s (Ctrl-C to stop early)..."),
                None => println!("Recording... press Ctrl-C to stop."),
            }
            let deadline = duration.map(|secs| Instant::now() + Duration::from_secs(secs));
            loop {
                tokio::select! {
                    signal = shutdown.recv() => {
                        println!("\n{signal} received; stopping recorder.");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                }
                if sb.get_current_url().await.is_err() {
                    println!("Browser session ended; stopping recorder.");
                    break;
                }
                let captured = session.drain(&mut sb).await.unwrap_or(0);
                if captured > 0 {
                    println!("Captured {captured} action(s) ({} total)", session.len());
                }
                if deadline.is_some_and(|end| Instant::now() >= end) {
                    println!("Duration elapsed; stopping recorder.");
                    break;
                }
            }
            let _ = session.drain(&mut sb).await;
            let saved = session.save(Path::new(&output))?;
            println!(
                "Recorded {} action(s) -> {}",
                session.len(),
                saved.display()
            );
            let _ = sb.quit().await;
        }
        Commands::Replay { file } => {
            let actions: Vec<RecordedAction> = RecorderSession::load_json(Path::new(&file))?;
            if actions.is_empty() {
                return Err(format!("no actions found in '{file}'").into());
            }
            let mut sb = BaseCase::new(config).await?;
            let report = RecorderSession::replay(&mut sb, &actions).await?;
            sb.quit().await?;

            for step in &report.steps {
                match &step.error {
                    Some(err) => println!("FAIL  {} -> {err}", step.description),
                    None => println!("ok    {}", step.description),
                }
            }
            println!(
                "Replayed {} step(s): {} passed, {} failed, {} skipped",
                report.steps.len(),
                report.passed(),
                report.failed(),
                report.skipped
            );
            if !report.is_success() {
                std::process::exit(1);
            }
        }
        Commands::Completions { .. }
        | Commands::ImportPython { .. }
        | Commands::ImportPlaywright { .. }
        | Commands::ImportPuppeteer { .. } => {
            unreachable!("pure CLI commands return before browser configuration")
        }
    }

    Ok(())
}
