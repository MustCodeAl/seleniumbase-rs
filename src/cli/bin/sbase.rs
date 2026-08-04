#![allow(deprecated)]

use std::io::Write;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use seleniumbase_rs::artifacts::{artifact_path, ensure_latest_logs_dir};
use seleniumbase_rs::cli::scripts::*;
// use seleniumbase_rs::dashboard::write_dashboard_html;
use seleniumbase_rs::api::scenario::{run_scenario, write_dashboard_html, Scenario};
use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::stealth::patcher::find_system_chrome;
use seleniumbase_rs::{
    import_python, init_tracing_from_runtime, BaseCase, Browser, ChromeBinaryPatcher,
    ChromedriverPatcher, DriverMode, EnginePatch, ImportOptions, ImportSeverity, PythonSource,
    RuntimeConfig,
};
use serde_json::{json, Value};
use thirtyfour::extensions::cdp::NetworkConditions;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BrowserArg {
    Chrome,
    Chromium,
    Edge,
    Firefox,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ImportSourceArg {
    Auto,
    SeleniumBase,
    Selenium,
}

impl From<BrowserArg> for Browser {
    fn from(value: BrowserArg) -> Self {
        match value {
            BrowserArg::Chrome => Browser::Chrome,
            BrowserArg::Chromium => Browser::Chromium,
            BrowserArg::Edge => Browser::Edge,
            BrowserArg::Firefox => Browser::Firefox,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "sbase", version, about = "SeleniumBase Rust CLI")]
struct Cli {
    #[arg(
        long,
        default_value = "http://localhost:4444",
        help = "WebDriver server URL (e.g. http://localhost:4444)"
    )]
    webdriver: String,
    #[arg(long, value_enum, default_value_t = BrowserArg::Chrome, help = "Browser engine to launch")]
    browser: BrowserArg,
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
        help = "Enable the built-in ad-block extension"
    )]
    ad_block: bool,
    #[arg(long, help = "Proxy URL (scheme://host:port)")]
    proxy: Option<String>,
    #[arg(long, help = "URL to a proxy auto-config (PAC) file")]
    proxy_pac_url: Option<String>,
    #[arg(long, help = "Path to a persistent browser user-data directory")]
    user_data_dir: Option<String>,
    #[arg(long, help = "Path to an unpacked extension directory")]
    extension_dir: Option<String>,
    #[arg(long, help = "Reuse an existing browser session when possible")]
    reuse_session: bool,
    #[arg(long, help = "Record the session actions")]
    rs: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Use a mobile device emulation profile"
    )]
    mobile: bool,
    #[arg(short = 'n', long, help = "Number of parallel threads for test runs")]
    threads: Option<usize>,
    #[arg(short = 'c', long, help = "Path to a TOML/JSON settings file")]
    config: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
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
        #[arg(long, help = "Output image file path")]
        path: Option<String>,
    },
    /// Save the current page source to a file.
    SaveSource {
        #[arg(long, help = "Output HTML file path")]
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
    /// Install required dependencies and artifacts.
    Install,
    /// Create a directory.
    Mkdir {
        #[arg(long, help = "Directory to create")]
        dir: String,
    },
    /// Create a file.
    Mkfile {
        #[arg(long, help = "File path to create")]
        file: String,
    },
    /// Launch the interactive commander GUI.
    Commander,
    /// Generate case plans.
    Caseplans,
    /// Launch the behave/Gherkin GUI.
    BehaveGui,
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
}

async fn run_doctor() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::PathBuf;

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
    let chromedriver: Option<PathBuf> = which::which("chromedriver")
        .ok()
        .or_else(|| std::env::var("CHROMEDRIVER_PATH").ok().map(PathBuf::from));
    match chromedriver {
        Some(path) if path.exists() => {
            println!("  Found: {}", path.display());
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
        Some(path) => {
            println!("  Configured path missing: {}", path.display());
            println!("  Hint: install chromedriver or set CHROMEDRIVER_PATH");
        }
        None => {
            println!("  Not found in PATH");
            println!("  Hint: install chromedriver or set CHROMEDRIVER_PATH");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = RuntimeConfig::from_env().unwrap_or_default();
    init_tracing_from_runtime(&runtime);

    let args = Cli::parse();

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
            let python = std::fs::read_to_string(file)?;
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
        _ => {}
    }

    if args.cdp && args.uc {
        return Err("Choose either --cdp or --uc, not both.".into());
    }

    // Start from global config file (if any) and apply CLI overrides.
    let mut settings = match args.config.as_deref() {
        Some(path) => Settings::load(Some(path))?,
        None => Settings::load_global()?,
    };
    settings.browser = match args.browser {
        BrowserArg::Chrome => "chrome".to_owned(),
        BrowserArg::Chromium => "chromium".to_owned(),
        BrowserArg::Edge => "edge".to_owned(),
        BrowserArg::Firefox => "firefox".to_owned(),
    };
    if args.headless {
        settings.headless = true;
    } else if args.headed {
        settings.headless = false;
    }
    if args.cdp {
        settings.mode = Some("cdp".to_owned());
    } else if args.uc {
        settings.mode = Some("uc".to_owned());
    }
    if let Some(v) = args.user_agent.as_ref().or(args.agent.as_ref()) {
        settings.user_agent = Some(v.clone());
    }
    if let Some(v) = args.locale {
        settings.locale = Some(v);
    }
    if args.ad_block {
        settings.ad_block = true;
    }
    if let Some(v) = args.proxy {
        settings.proxy = Some(v);
    }
    if let Some(v) = args.proxy_pac_url {
        settings.proxy_pac_url = Some(v);
    }
    if let Some(v) = args.user_data_dir {
        settings.user_data_dir = Some(v);
    }
    if let Some(v) = args.extension_dir {
        settings.extension_dir = Some(v);
    }
    if args.reuse_session || args.rs {
        settings.reuse_session = true;
    }
    if args.mobile {
        settings.mobile = true;
    }
    if let Some(v) = args.threads {
        settings.threads = Some(v);
    }

    let mode = settings
        .mode
        .as_deref()
        .map(|m| match m.to_lowercase().as_str() {
            "uc" => DriverMode::Uc,
            "cdp" => DriverMode::Cdp,
            _ => DriverMode::WebDriver,
        })
        .unwrap_or(DriverMode::WebDriver);
    let mut config = settings.to_browser_config();
    config.webdriver_url = args.webdriver;
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
                sb.save_screenshot(target_path).await?;
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
                sb.save_page_source(target_path).await?;
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
        Commands::Install => match sb_install::install_drivers().await {
            Ok(path) => println!("Drivers installed successfully at {}", path.display()),
            Err(e) => eprintln!("Failed to install driver: {}", e),
        },
        Commands::Mkdir { dir } => {
            sb_mkdir::create_test_dir(&dir);
        }
        Commands::Mkfile { file } => {
            sb_mkfile::create_test_file(&file);
        }
        Commands::Commander => {
            if let Err(e) = sb_commander::run_commander() {
                eprintln!("Failed to run commander: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Caseplans => match sb_caseplans::run_caseplans() {
            Ok(path) => println!("Generated {}", path.display()),
            Err(e) => eprintln!("Failed to generate case plans: {}", e),
        },
        Commands::BehaveGui => match sb_behave_gui::run_gui() {
            Ok(path) => println!("Created BDD feature file at {}", path.display()),
            Err(e) => eprintln!("Failed to create BDD feature file: {}", e),
        },
        Commands::Print { file } => match sb_print::print_file(&file) {
            Ok(content) => println!("{}", content),
            Err(e) => eprintln!("Failed to read file {}: {}", file, e),
        },
        Commands::Objectify => match sb_objectify::objectify_page() {
            Ok(path) => println!("Generated {}", path.display()),
            Err(e) => eprintln!("Failed to generate page object: {}", e),
        },
        Commands::Mkpres { file } => match sb_mkpres::make_presentation(&file) {
            Ok(path) => println!("Created presentation at {}", path.display()),
            Err(e) => eprintln!("Failed to create presentation: {}", e),
        },
        Commands::Mkchart { file } => match sb_mkchart::make_chart(&file) {
            Ok(path) => println!("Created chart at {}", path.display()),
            Err(e) => eprintln!("Failed to create chart: {}", e),
        },
        Commands::Mkrec { file } => match sb_recorder::make_recorder_file(&file) {
            Ok(path) => println!("Created recorder file at {}", path.display()),
            Err(e) => eprintln!("Failed to create recorder file: {}", e),
        },
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
        }
        Commands::Completions { .. } | Commands::ImportPython { .. } => {
            unreachable!("pure CLI commands return before browser configuration")
        }
    }

    Ok(())
}
