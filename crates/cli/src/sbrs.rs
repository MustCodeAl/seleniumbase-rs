#![recursion_limit = "1024"]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use seleniumbase_rs::api::recorder::{RecordedAction, RecorderSession};
use seleniumbase_rs::api::scenario::{run_scenario, write_dashboard, Scenario};
use seleniumbase_rs::artifacts::{artifact_path, ensure_latest_logs_dir};
use seleniumbase_rs::browser::driver_version::binary_major;
#[cfg(feature = "pdf")]
use seleniumbase_rs::cli::commands::looks_like_pdf;
use seleniumbase_rs::cli::commands::{
    blank_file_input_page, download_file_name, format_deferred_summary, html_data_url,
    parse_deferred_spec_json, parse_deferred_specs, DeferredResult, DeferredSpec,
};
use seleniumbase_rs::cli::devtools::{
    ensure_port_is_free, ready_message, DevtoolsRequest, DEFAULT_DEVTOOLS_PORT,
};
use seleniumbase_rs::cli::plugin_cmd::{self, PluginChoice};
use seleniumbase_rs::cli::prompt;
use seleniumbase_rs::cli::scaffold::{Scaffold, ScaffoldError};
use seleniumbase_rs::cli::scripts::*;
use seleniumbase_rs::common::encryption::{decrypt_with_passphrase, encrypt_with_passphrase};
use seleniumbase_rs::common::shutdown::{
    cleanup_within, drain_cleanups, run_until_shutdown, timeout_from_env, Outcome, Shutdown,
};
use seleniumbase_rs::config::plugins::PluginSelection;
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

mod args;
#[path = "args/install.rs"]
mod args_install;
mod completion;
mod crypto;

mod failure;

mod fig;
mod fingerprint;
mod info;

mod proxy;
mod recorder_flags;

mod browser_flags;
#[cfg(feature = "turso")]
mod history;
#[cfg(test)]
mod reference;
mod report;
mod strict;

mod test_args;
mod test_session;

use failure::ArgumentError;
use fingerprint::{FingerprintArgs, FingerprintCommand};
use recorder_flags::RecorderFlags;
use test_args::TestArgs;

/// What `sbrs --version` prints after the program name. It names the library
/// package, as it did when the binary was part of it, not this crate
/// (`seleniumbase-rs-cli`), so the line is the same as before the split.
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (seleniumbase-rs)");

#[derive(Clone, Copy, Debug, Eq, PartialEq, usage::ValueEnum)]
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

#[derive(Clone, Copy, Debug, usage::ValueEnum)]
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

/// The drivers `sbrs install` (and `sbrs get`) fetches.
#[derive(Clone, Copy, Debug, Eq, PartialEq, usage::ValueEnum)]
enum DriverArg {
    /// chromedriver, for Chrome.
    Chromedriver,
    /// geckodriver, for Firefox.
    #[usage(alias = "firefoxdriver")]
    Geckodriver,
    /// msedgedriver, for Microsoft Edge.
    #[usage(alias = "msedgedriver")]
    Edgedriver,
    /// chromedriver patched for the UC mode, kept as downloaded_drivers/uc_driver-VERSION/uc_driver.
    #[usage(name = "uc_driver", aliases("uc-driver", "ucdriver"))]
    UcDriver,
    /// Chrome for Testing, the browser, in downloaded_drivers/cft/.
    Cft,
    /// chrome-headless-shell, the headless-only browser, in downloaded_drivers/chs/.
    Chs,
    /// A Chromium snapshot build (see --revision), in downloaded_drivers/chromium/.
    Chromium,
}

impl From<DriverArg> for sb_install::DriverName {
    fn from(value: DriverArg) -> Self {
        match value {
            DriverArg::Chromedriver => Self::Chromedriver,
            DriverArg::Geckodriver => Self::Geckodriver,
            DriverArg::Edgedriver => Self::Msedgedriver,
            DriverArg::UcDriver => Self::UcDriver,
            DriverArg::Cft => Self::Cft,
            DriverArg::Chs => Self::Chs,
            DriverArg::Chromium => Self::Chromium,
        }
    }
}

/// The words `sbrs install DRIVER <Tab>` offers for the version, each one a spelling
/// `DriverVersion` reads (a test keeps it so). A major version such as `126` or a full build
/// is accepted as well, but there are too many to list.
const INSTALL_VERSIONS: [(&str, &str); 9] = [
    ("browser", "the driver for the installed browser"),
    ("stable", "the newest stable release"),
    ("latest", "the newest stable release"),
    ("beta", "the newest beta release"),
    ("dev", "the newest dev release"),
    ("canary", "the newest canary release"),
    (
        "previous",
        "the newest release of the major version before the stable one",
    ),
    (
        "mlatest",
        "the newest build of the installed browser's major version",
    ),
    (
        "keep",
        "the driver already downloaded, else the one for the installed browser",
    ),
];

/// The completer of `sbrs install DRIVER VERSION`: the words of [`INSTALL_VERSIONS`].
fn install_version_candidates<P>(
    _partial: &P,
    _ctx: &usage::complete::CompleteCtx<'_>,
) -> Vec<usage::complete::Candidate<'static>> {
    INSTALL_VERSIONS
        .iter()
        .map(|&(word, about)| usage::complete::Candidate::described(word, about))
        .collect()
}

// The options of `sbrs prompt`.
//
// A struct of their own, not fields of the `Prompt` variant: the parser builds each
// struct's arguments in a function of its own, which keeps the one that builds
// every subcommand small enough for a test thread's stack in a debug build. (Plain
// comments, not docs: a doc comment on this struct would be help text.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
struct PromptArgs {
    #[usage(
        long,
        value_name = "TEMPLATE",
        default = "{browser} {major} {driver}",
        value_hint = usage::ValueHint::Other, help = "The line to print; placeholders {browser} {major} {driver} {mark} {driver_major}, {{ and }} for braces"
    )]
    format: prompt::PromptFormat,
    #[usage(long, bool_value, help = "Mark the driver with + - ? instead of ✓ ✗ ?")]
    ascii: bool,
    #[usage(
        long,
        value_name = "PATH",
        value_hint = usage::ValueHint::AnyPath,
        help = "Browser to report on (default: SB_CHROME_BIN, else the installed Chrome or Chromium)"
    )]
    browser_path: Option<PathBuf>,
    #[usage(
        long,
        bool_value,
        help = "Run --version again instead of using the versions cached in the user cache folder"
    )]
    no_cache: bool,
    #[usage(
        long,
        bool_value,
        help = "Explain on standard error what was found and how"
    )]
    verbose: bool,
}

// The options of `sbrs doctor`. (A struct of its own for the reason `PromptArgs` is one; plain
// comments, not docs: a doc comment on this struct would be help text.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
struct DoctorArgs {
    #[usage(
        long,
        bool_value,
        help = "Print the problems found but exit with 0 (the default exits with 1 when the browser is missing or the chromedriver does not match it)"
    )]
    warn_only: bool,
}

// The `plugin` command: it holds only its own subcommands. (Plain comments, not docs: a
// doc comment on this struct would be help text.)
#[derive(Debug, usage::Args)]
struct PluginArgs {
    #[usage(subcommand)]
    command: PluginCommand,
}

#[derive(Debug, usage::Subcommands)]
enum PluginCommand {
    /// List the built-in plugins of this build, and whether they are on.
    ///
    /// Prints each plugin's name, category and summary, and `yes` under ENABLED when the
    /// settings file, SB_PLUGINS or the global --plugin option turns it on. A build without
    /// built-in plugins says so; rebuild it with --features plugins-builtin.
    #[usage(example = "sbrs plugin list")]
    List,
    /// Show what one plugin declares and what it may do.
    ///
    /// Prints the plugin's category, phase, capabilities (with their risk, and whether the
    /// host enforces them), what it needs, its place among the other plugins and the options
    /// it was given. Exits with status 2 and lists the known names when there is no such
    /// plugin.
    #[usage(example = "sbrs plugin info shield")]
    Info(PluginInfoArgs),
    /// Run the task of a plugin on a browser it starts, and write its results as JSON lines.
    ///
    /// Only plugins that offer a task can be run, and the named plugin is turned on for this
    /// run without --plugin. The task reads its input from --input (a JSON file, or `-` for
    /// standard input; `{}` when there is none) and writes one JSON value per line to --out
    /// (default: standard output). Its summary goes to standard error. The global options
    /// --plugin, --plugin-opt, --headless, --proxy and the like shape the browser and the other
    /// plugins. Ctrl-C stops the task, closes the browser and exits with status 130. Exits with
    /// status 2 and lists the plugins that have a task when the name is not one of them.
    #[usage(example = "sbrs plugin run crawler --input crawl.json --out found.jsonl")]
    Run(PluginRunArgs),
}

// The argument of `plugin info`. (Plain comments, not docs: a doc comment on this struct
// would be help text.)
#[derive(Debug, usage::Args)]
struct PluginInfoArgs {
    /// The plugin's name, as `sbrs plugin list` shows it.
    #[usage(value_hint = usage::ValueHint::Other)]
    name: String,
}

// The arguments of `plugin run`. (Plain comments, not docs: a doc comment on this struct would
// be help text.)
#[derive(Debug, usage::Args)]
struct PluginRunArgs {
    /// The plugin whose task to run, as `sbrs plugin list` shows it.
    #[usage(value_hint = usage::ValueHint::Other)]
    name: String,
    /// A JSON file with the task's input, or `-` to read it from standard input.
    #[usage(long, value_name = "FILE", value_hint = usage::ValueHint::AnyPath)]
    input: Option<String>,
    /// Write the results to this file, one JSON value per line (default: standard output).
    #[usage(long, value_name = "FILE", value_hint = usage::ValueHint::AnyPath)]
    out: Option<PathBuf>,
}

// The options `grid-hub` and `grid-node` share. (Plain comments, not docs: a doc
// comment on this struct would be help text.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
struct GridArgs {
    /// What to do with the Grid process.
    #[usage(value_enum)]
    action: GridActionArg,
    /// The Selenium Server jar (default: SB_SELENIUM_SERVER_JAR, else the newest in
    /// downloaded_drivers/).
    /// This command never downloads it.
    #[usage(long, value_name = "PATH", value_hint = usage::ValueHint::AnyPath)]
    jar: Option<PathBuf>,
    /// The Java executable (default: `java` on PATH).
    #[usage(long, value_name = "PATH", value_hint = usage::ValueHint::AnyPath)]
    java: Option<PathBuf>,
    /// The address to listen on.
    #[usage(long = "grid-host", value_name = "HOST", value_hint = usage::ValueHint::Hostname)]
    host: Option<String>,
    /// The port to listen on (default: 4444 for a hub, 5555 for a node).
    #[usage(long = "grid-port", value_name = "PORT", value_hint = usage::ValueHint::Other)]
    port: Option<u16>,
    /// For a node, the hub to register with: a host or a URL.
    #[usage(long, value_name = "HOST_OR_URL", value_hint = usage::ValueHint::Other)]
    hub: Option<String>,
    /// Close sessions that have been idle this many seconds.
    #[usage(long = "timeout", value_name = "SECONDS", value_hint = usage::ValueHint::Other)]
    session_timeout: Option<u64>,
    /// How many seconds `start` waits for the Grid to become ready.
    #[usage(long, value_name = "SECONDS", default = "60", value_hint = usage::ValueHint::Other)]
    wait: u64,
    /// Log at INFO instead of WARNING.
    #[usage(short = 'v', long = "verbose", bool_value)]
    verbose: bool,
    /// Start even if something already accepts connections on the port.
    #[usage(long, bool_value)]
    skip_port_check: bool,
    /// More arguments for the Selenium Server, after `--`.
    #[usage(double_dash = "required", value_hint = usage::ValueHint::Other)]
    extra_args: Vec<String>,
}

#[derive(Clone, Copy, Debug, usage::ValueEnum)]
enum ImportSourceArg {
    Auto,
    SeleniumBase,
    Selenium,
}

/// The languages `sbrs import-playwright` reads.
#[derive(Clone, Copy, Debug, usage::ValueEnum)]
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

/// The languages `sbrs import-puppeteer` reads.
#[derive(Clone, Copy, Debug, usage::ValueEnum)]
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

impl BrowserArg {
    /// The word that names the browser on the command line and in `SB_BROWSER`.
    const fn name(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::Edge => "edge",
            Self::Firefox => "firefox",
            Self::Brave => "brave",
            Self::Comet => "comet",
            Self::Opera => "opera",
            Self::Safari => "safari",
            Self::Ie => "ie",
            Self::Cft => "cft",
            Self::Chs => "chs",
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

#[derive(Debug, usage::Cli)]
#[usage(
    bin = "sbrs",
    name = "sbrs",
    version = VERSION,
    about = "SeleniumBaseRust CLI",
    long_about = "A Rust port of the Python SeleniumBase testing framework. Provides browser automation, stealth/undetected modes, and a command-line helper.",
    logo = logo_helper::LOGO,
    completion,
    unknown_flags = "error",
    args_override_self = false
)]
struct Cli {
    #[usage(
        long,
        global,
        value_name = "URL",
        value_hint = usage::ValueHint::Url,
        help = "WebDriver server URL (default: SB_WEBDRIVER_URL, else http://localhost:4444)"
    )]
    webdriver: Option<String>,
    #[usage(
        long,
        global,
        value_enum,
        help = "Browser engine to launch (SB_BROWSER; default: chrome, or the config file's)"
    )]
    browser: Option<BrowserArg>,
    #[usage(long, global, aliases = ["gui"], bool_value, help = "Run the browser in headed mode (SB_HEADLESS=false)")]
    headed: bool,
    #[usage(long, global, aliases = ["headless2"], bool_value, help = "Run the browser in headless mode (SB_HEADLESS)")]
    headless: bool,
    #[usage(
        long,
        global,
        bool_value,
        help = "Enable Chrome DevTools Protocol mode (SB_MODE=cdp)"
    )]
    cdp: bool,
    #[usage(
        long,
        global,
        bool_value,
        aliases = ["undetected", "undetectable"],

        help = "Enable undetected-chromedriver style evasions (SB_MODE=uc)"
    )]
    uc: bool,
    #[usage(long, global, value_hint = usage::ValueHint::Other, help = "Override the default user agent string (SB_USER_AGENT)")]
    user_agent: Option<String>,
    #[usage(short = 'a', long, global, value_hint = usage::ValueHint::Other, help = "Alias for --user-agent (SB_USER_AGENT)")]
    agent: Option<String>,
    #[usage(long, global, value_hint = usage::ValueHint::Other, help = "Locale string for the browser context (SB_LOCALE)")]
    locale: Option<String>,
    /// Page the browser opens first, Python's --start-page (SB_START_PAGE; aliases --url and --start_page)
    ///
    /// Every command starts its own browser and closes it at the end, so a command that
    /// acts on a page (get-text, click, screenshot, assert-element, ...) acts on this one:
    /// `sbrs --url https://example.com get-text --css p`. A bare host gets `https://` and
    /// a local file becomes a `file://` URL. The command line wins over SB_START_PAGE, which
    /// wins over the `start_page` key of the settings file. Commands that open a URL of
    /// their own (open, smoke, download-file, print-pdf, pdf-text, assert-pdf-text, record)
    /// refuse it.
    #[usage(
        long = "start-page",
        global,
        visible_aliases = ["url", "start_page"],
        value_name = "URL",
        value_hint = usage::ValueHint::Url
    )]
    start_page: Option<String>,
    /// Save a screenshot to latest_logs/ just before the browser closes, and print its path (SB_SAVE_SCREENSHOT)
    ///
    /// Python's `--save-screenshot`. A command that acts on the page (click, type, ...) also
    /// prints where it ended up as `Now at: <url> (<title>)`.
    #[usage(
        long = "save-screenshot",
        global,
        visible_alias = "save_screenshot",
        bool_value
    )]
    save_screenshot: bool,
    #[usage(
        long,
        global,
        bool_value,
        aliases = ["adblock", "block-ads"],

        help = "Refuse the requests of ad and tracker hosts (SB_AD_BLOCK; Chromium WebDriver sessions)"
    )]
    ad_block: bool,
    #[usage(
        long,
        global,
        aliases = ["proxy-server", "proxy-string"],

        value_hint = usage::ValueHint::Other, help = "Proxy (SB_PROXY; host:port or scheme://host:port); with --proxy-list, a name from the list"
    )]
    proxy: Option<String>,
    #[usage(
        long,
        global,
        value_name = "FILE",
        value_hint = usage::ValueHint::FilePath, help = "File of proxies to choose from; without --proxy a random one is used (SB_PROXY_LIST)"
    )]
    proxy_list: Option<String>,
    #[usage(long, global, aliases = ["pac-url"], value_hint = usage::ValueHint::Url, help = "URL to a proxy auto-config (PAC) file (SB_PROXY_PAC_URL)")]
    proxy_pac_url: Option<String>,
    #[usage(
        long,
        global,
        value_hint = usage::ValueHint::DirPath, help = "Path to a persistent browser user-data directory (SB_USER_DATA_DIR)"
    )]
    user_data_dir: Option<String>,
    #[usage(long, global, value_hint = usage::ValueHint::DirPath, help = "Path to an unpacked extension directory (SB_EXTENSION_DIR)")]
    extension_dir: Option<String>,
    #[usage(
        long = "reuse-session",
        global,
        visible_aliases = ["reuse_session", "rs"],
        bool_value,
        help = "Share browsers among the tests of the run (SB_REUSE_SESSION)"
    )]
    reuse_session: bool,
    #[usage(
        long = "rcs",
        global,
        visible_aliases = ["reuse-class-session", "reuse_class_session"],
        bool_value,
        help = "Share a browser among the tests of one Rust module (SB_REUSE_CLASS_SESSION)"
    )]
    reuse_class_session: bool,
    #[usage(
        long,
        global,
        bool_value,
        aliases = ["mobile-emulator"],

        help = "Use a mobile device emulation profile (SB_MOBILE)"
    )]
    mobile: bool,
    #[usage(
        long = "esc-end",
        global,
        visible_aliases = ["esc_end", "ee"],
        bool_value,
        help = "End the current test when ESC is pressed in the browser (SB_ESC_END)"
    )]
    esc_end: bool,
    #[usage(
        long = "rec-sb-cdp",
        global,
        visible_alias = "rec_sb_cdp",
        bool_value,
        help = "Write recordings as pure CDP code (SB_REC_SB_CDP; sb_cdp Page and Locator calls)"
    )]
    rec_sb_cdp: bool,
    #[usage(short = 'c', long, global, aliases = ["settings-file", "settings"], value_hint = usage::ValueHint::FilePath, extensions("toml", "json"), help = "Path to a TOML/JSON settings file (SB_SETTINGS_FILE)")]
    config: Option<String>,
    /// Make every browser the command starts present the fingerprint in FILE (SB_FINGERPRINT)
    ///
    /// The file is the fingerprint JSON that `sbrs fingerprint capture` and `export` write.
    /// A Chromium browser then runs in the CDP mode unless --uc or another mode is given.
    /// The command line wins over SB_FINGERPRINT, which wins over the [fingerprint] table of
    /// the settings file. The user agent, language and proxy named by their own options win
    /// over the fingerprint's.
    #[usage(long, global, value_name = "FILE", value_hint = usage::ValueHint::AnyPath)]
    fingerprint: Option<PathBuf>,
    /// Turn on a built-in plugin in the browsers the command starts (SB_PLUGINS; repeatable)
    ///
    /// Adds to the plugins the settings file (the [plugins] table) and SB_PLUGINS turn on.
    /// Precedence, highest first: the command line, then SB_PLUGINS (which replaces the names
    /// the file enables), then the settings file. `sbrs plugin list` shows the plugins this
    /// build has. `sbrs test` passes the names on to the tests as SB_PLUGINS.
    #[usage(long, global, value_name = "NAME", value_hint = usage::ValueHint::Other)]
    plugin: Vec<String>,
    /// Set one option of a plugin, NAME.KEY=VALUE with a TOML VALUE (repeatable)
    ///
    /// Quote the whole option so the shell keeps the quotes of a string:
    /// --plugin-opt 'shield.level="strict"'. An option given here replaces the same option
    /// from SB_PLUGINS or the settings file and keeps the plugin's other options, so the
    /// precedence is the command line, then SB_PLUGINS, then the file. The plugin must be
    /// turned on by --plugin, SB_PLUGINS or the file. Not available for `sbrs test`: put
    /// the options in the [plugins.NAME] table of the settings file.
    #[usage(long, global, value_name = "NAME.KEY=VALUE", value_hint = usage::ValueHint::Other)]
    plugin_opt: Vec<String>,
    /// The options of the browser and the page, one flag for each row of the option table
    /// (`sbrs options` lists them): `--dark`, `--metrics 360,640,2`, `--block-images`, ...
    #[usage(flatten, next_help_heading = "Browser options")]
    browser_flags: browser_flags::BrowserFlags,
    /// The options of Recorder Mode, Python's `--rec` and its companions.
    #[usage(flatten, next_help_heading = "Recorder options")]
    recorder: RecorderFlags,

    #[usage(subcommand)]
    command: Commands,
}

#[derive(Debug, usage::Subcommands)]
#[expect(
    clippy::large_enum_variant,
    reason = "one command is parsed per process, and `test` has a flag for every row of the option table"
)]
enum Commands {
    /// Print the sbrs version, as `sbrs --version` does.
    #[usage(help_heading = "Info and setup", example = "sbrs version")]
    Version,
    /// Open a URL in the browser.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs open https://example.com"
    )]
    Open {
        /// URL to open in the browser.
        #[usage(value_hint = usage::ValueHint::Url)]
        url: String,
    },
    /// Run a smoke test against a URL.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs smoke https://example.com --title-contains Example"
    )]
    Smoke {
        #[usage(value_hint = usage::ValueHint::Url, help = "URL to open or smoke-test")]
        url: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Expected substring in the page title")]
        title_contains: Option<String>,
    },
    /// Execute a Chrome DevTools Protocol command.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs cdp --cmd Browser.getVersion"
    )]
    Cdp {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CDP method name (e.g. Runtime.evaluate)")]
        cmd: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "JSON object of CDP parameters")]
        params: Option<String>,
    },
    /// Clear the browser cache via CDP.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com --user-data-dir ./profile cache-clear"
    )]
    CacheClear,
    /// Capture a screenshot of the current page.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com screenshot --path page.png"
    )]
    Screenshot {
        #[usage(
            long,
            value_hint = usage::ValueHint::FilePath, extensions("png"), help = "Output image file path (default: a new file in latest_logs/)"
        )]
        path: Option<String>,
    },
    /// Save the current page source to a file.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com save-source --path page.html"
    )]
    SaveSource {
        #[usage(
            long,
            value_hint = usage::ValueHint::FilePath, extensions("html", "htm"), help = "Output HTML file path (default: a new file in latest_logs/)"
        )]
        path: Option<String>,
    },
    /// Assert that an element exists.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com assert-element --css p"
    )]
    AssertElement {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Wait until an element contains the expected text.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com wait-for-text --css p --text 'This domain is for use in' --timeout 5"
    )]
    WaitForText {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Expected text substring to wait for")]
        text: String,
        #[usage(long, default = "10", value_hint = usage::ValueHint::Other, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Hover over an element.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com hover --css p"
    )]
    Hover {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Hover over one element, then click another.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com hover-and-click --hover-css nav --click-css 'nav a'"
    )]
    HoverAndClick {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector to hover over")]
        hover_css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector to click")]
        click_css: String,
    },
    /// Select an option from a dropdown.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com select-option --css select --text Two"
    )]
    SelectOption {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Visible text of the option to select")]
        text: Option<String>,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Option value attribute")]
        value: Option<String>,
    },
    /// Drag one element onto another.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com drag-and-drop --source-css '#a' --target-css '#b'"
    )]
    DragAndDrop {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector of the element to drag")]
        source_css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector of the drop target")]
        target_css: String,
    },
    /// Click an element using CDP.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com cdp-click-element --css a"
    )]
    CdpClickElement {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Type text using CDP input injection.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com cdp-type-text --text hello"
    )]
    CdpTypeText {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Text to inject via CDP")]
        text: String,
    },
    /// Print the visible text of an element.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-text --css p"
    )]
    GetText {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Print an element attribute value.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-attribute --css a --attribute href"
    )]
    GetAttribute {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Attribute name to read or assert")]
        attribute: String,
    },
    /// Print an element property value.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-property --css input --property value"
    )]
    GetProperty {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Property name to read")]
        property: String,
    },

    /// Print the page title.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-title"
    )]
    GetTitle,
    /// Print the current URL.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-current-url"
    )]
    GetCurrentUrl,

    /// Delete all browser cookies.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com --user-data-dir ./profile clear-cookies"
    )]
    ClearCookies,

    /// Accept the alert that the page opens when it loads.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com accept-alert"
    )]
    AcceptAlert,
    /// Dismiss the alert that the page opens when it loads.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com dismiss-alert"
    )]
    DismissAlert,
    /// Print the text of the alert that the page opens when it loads.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-alert-text"
    )]
    GetAlertText,
    /// Type text into the prompt that the page opens when it loads.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com type-alert-text --text hello"
    )]
    TypeAlertText {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Text to type into the alert prompt")]
        text: String,
    },
    /// Clear the page local storage.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com --user-data-dir ./profile clear-local-storage"
    )]
    ClearLocalStorage,
    /// Print a local storage value by key.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-local-storage-item --key theme"
    )]
    GetLocalStorageItem {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Storage key")]
        key: String,
    },
    /// Set a local storage key/value pair.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com --user-data-dir ./profile set-local-storage-item --key theme --value dark"
    )]
    SetLocalStorageItem {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Storage key")]
        key: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Value to store in local storage")]
        value: String,
    },
    /// Remove a local storage entry by key.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com --user-data-dir ./profile remove-local-storage-item --key theme"
    )]
    RemoveLocalStorageItem {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Storage key")]
        key: String,
    },
    /// Print all browser cookies as JSON.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-cookies"
    )]
    GetCookies,
    /// Patch a chromedriver binary to reduce detection surface.
    #[usage(
        help_heading = "Drivers, grid and servers",
        example = "sbrs patch-chromedriver --path ./chromedriver"
    )]
    PatchChromedriver {
        #[usage(long, value_hint = usage::ValueHint::ExecutablePath, help = "Path to the chromedriver binary")]
        path: String,
    },
    /// Patch a Chrome/Chromium binary for native-level spoofing.
    #[usage(
        help_heading = "Drivers, grid and servers",
        example = "sbrs patch-chrome --path ./chrome --cache-dir ./patched"
    )]
    PatchChrome {
        #[usage(long, value_hint = usage::ValueHint::ExecutablePath, help = "Path to the Chrome/Chromium binary")]
        path: String,
        /// Directory where the patched copy is cached.
        #[usage(long, value_hint = usage::ValueHint::DirPath, help = "Directory where patched binaries are cached (SB_PATCH_CACHE_DIR)")]
        cache_dir: Option<String>,
    },
    /// Run a small forward proxy on this machine (HTTP and CONNECT, no authentication).
    ///
    /// Point a browser at it with the global --proxy option. `CONNECT host:port` becomes a
    /// tunnel, so HTTPS passes through untouched; an absolute `http://` request is sent to
    /// its host once and the connection closed. It listens on 127.0.0.1:8899 unless told
    /// otherwise, and a request head over 64 KiB, more than 256 connections at once and 30
    /// seconds without a byte are refused or cut. Each request is logged at INFO
    /// (SB_LOG_LEVEL=info). Ctrl-C or SIGTERM stops it with status 0.
    #[usage(
        help_heading = "Drivers, grid and servers",
        example = "sbrs proxy --port 8899"
    )]
    Proxy(args::tools::ProxyArgs),
    /// Download Selenium Server 4, the jar that `grid-hub` and `grid-node` run.
    ///
    /// `sbrs download server` saves selenium-server-VERSION.jar in ./downloaded_drivers/ and
    /// prints its path and SHA-256. The release this build pins is checked against its
    /// checksum; another one is not.
    #[usage(
        help_heading = "Drivers, grid and servers",
        example = "sbrs download server"
    )]
    Download(args::tools::DownloadArgs),

    /// Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted.
    ///
    /// The passphrase comes from the SB_ENCRYPTION_KEY environment variable, never
    /// from an argument, so it stays out of shell history and process listings.
    /// Python's `sbrs obfuscate` is the same command.
    ///
    /// With no TEXT and a terminal on standard input it prompts as Python's `obfuscate`
    /// does: the text and its verification are read without echo, the token is printed
    /// with the line that decrypts it in a test, and it asks again until Ctrl-C. When
    /// SB_ENCRYPTION_KEY is not set it asks once for the passphrase (twice, to catch a
    /// typo), without echo; the passphrase is never printed or saved.

    #[usage(alias = "obfuscate")]
    #[usage(help_heading = "Security", example = "sbrs encrypt 'my secret'")]
    Encrypt {
        /// The text to encrypt.
        #[usage(value_hint = usage::ValueHint::Other)]
        text: Option<String>,
    },
    /// Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted.
    /// Python's `sbrs unobfuscate` is the same command.
    ///
    /// With no TOKEN and a terminal on standard input it asks for a token on the screen,
    /// prints the text and asks again until Ctrl-C, as Python's `unobfuscate` does. When
    /// SB_ENCRYPTION_KEY is not set it asks once for the passphrase, without echo.

    #[usage(alias = "unobfuscate")]
    #[usage(help_heading = "Security", example = "sbrs decrypt TOKEN")]
    Decrypt {
        /// The token to decrypt.
        #[usage(value_hint = usage::ValueHint::Other)]
        token: Option<String>,
    },
    /// Finish a test run and write its reports, or show the runs of a results database.
    ///
    /// `sbrs report finish` ends the run of the current folder: the dashboard, the HTML report
    /// and the JUnit XML are written a last time. Use it after a plain `cargo test`, which
    /// leaves the live dashboard going; `sbrs test` finishes its run by itself. `sbrs report
    /// history` lists the runs recorded in a results database (a build with the `turso`
    /// feature).
    #[usage(
        help_heading = "Info and setup",
        example = "sbrs report finish --html report.html"
    )]
    Report(args::report::ReportArgs),
    /// Run a diagnostic check on the environment and configuration.
    ///
    /// Prints the settings in effect, the browser found, the patch cache and the chromedriver
    /// found, with a hint for anything that needs a fix. Exits with status 1 when no browser
    /// is found (or the one SB_CHROME_BIN names does not exist) or when the chromedriver does
    /// not match the installed Chrome, and with 0 otherwise. A chromedriver that is not found
    /// is only a hint, since the pure CDP engine needs none. --warn-only prints the same and
    /// always exits with 0.
    #[usage(help_heading = "Info and setup", example = "sbrs doctor")]
    Doctor(DoctorArgs),
    /// Print one short line for a shell prompt: the installed Chrome's major
    /// version and whether a matching chromedriver is at hand.
    ///
    /// Prints `chrome 155 ✓driver`, with `✗driver` when the chromedriver is
    /// missing or of another version and `?driver` when its version cannot be
    /// read. It starts no browser and uses no network: versions come from
    /// files (a macOS app's Info.plist, the version folder beside chrome.exe),
    /// and one that only `--version` tells is cached for ten minutes in the
    /// user cache folder. Prints nothing and exits with 0 when no browser is
    /// found; --verbose says why, on standard error. The chromedriver is looked
    /// for as `sbrs doctor` does: CHROMEDRIVER_PATH, downloaded_drivers/, PATH.
    #[usage(help_heading = "Info and setup", example = "sbrs prompt")]
    Prompt(PromptArgs),
    /// Assert that an element contains the expected text.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com assert-text-visible --css p --text 'This domain is for use in'"
    )]
    AssertTextVisible {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Expected visible text")]
        text: String,
    },
    /// Assert that an element does not contain the given text.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com assert-text-not-visible --css p --text Error"
    )]
    AssertTextNotVisible {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Text that should not be visible")]
        text: String,
    },
    /// Assert an element attribute equals an expected value.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com assert-attribute --css a --attribute href --value https://iana.org/domains/example"
    )]
    AssertAttribute {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Attribute name to read or assert")]
        attribute: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Expected attribute value")]
        value: String,
    },
    /// Assert the page title contains the expected text.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com assert-title --text Example"
    )]
    AssertTitle {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Expected page title substring")]
        text: String,
    },
    /// Wait until the document readyState is complete.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com wait-for-ready-state-complete"
    )]
    WaitForReadyStateComplete,
    /// Print the browser window position.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-window-position"
    )]
    GetWindowPosition,
    /// Print whether an element is visible.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com is-element-visible --css p"
    )]
    IsElementVisible {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Print whether the expected text is visible in an element.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com is-text-visible --css p --text domain"
    )]
    IsTextVisible {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Expected text to check")]
        text: String,
    },
    /// Wait until an element is no longer visible.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com wait-for-element-not-visible --css '#spinner' --timeout 5"
    )]
    WaitForElementNotVisible {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, default = "10", value_hint = usage::ValueHint::Other, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Save browser cookies to a JSON file.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com save-cookies --file cookies.json"
    )]
    SaveCookies {
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("json"), help = "JSON file to save cookies to")]
        file: String,
    },
    /// Load browser cookies from a JSON file.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com --user-data-dir ./profile load-cookies --file cookies.json"
    )]
    LoadCookies {
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("json"), help = "JSON file to load cookies from")]
        file: String,
    },
    /// Highlight an element, then click it.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com highlight-click --css a"
    )]
    HighlightClick {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Print whether a checkbox is checked.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com is-checked --css '#agree'"
    )]
    IsChecked {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Check a checkbox only if it is unchecked.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com check-if-unchecked --css '#agree'"
    )]
    CheckIfUnchecked {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Uncheck a checkbox only if it is checked.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com uncheck-if-checked --css '#agree'"
    )]
    UncheckIfChecked {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Wait until an element is present in the DOM.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com wait-for-element-present --css p --timeout 5"
    )]
    WaitForElementPresent {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, default = "10", value_hint = usage::ValueHint::Other, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Append text to an input or textarea.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com add-text --css '#name' --text hello"
    )]
    AddText {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Text to append to the element")]
        text: String,
    },
    /// Send keystrokes to an element.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com send-keys --css '#name' --text hello"
    )]
    SendKeys {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Keys to send to the element")]
        text: String,
    },
    /// Print the value of a form element.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com get-value --css '#name'"
    )]
    GetValue {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Click all visible elements matching a selector.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com click-visible-elements --css button"
    )]
    ClickVisibleElements {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Wait for an alert that the page opens, then accept it.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com wait-for-and-accept-alert --timeout 5"
    )]
    WaitForAndAcceptAlert {
        #[usage(long, default = "10", value_hint = usage::ValueHint::Other, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Wait for an alert that the page opens, then dismiss it.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com wait-for-and-dismiss-alert --timeout 5"
    )]
    WaitForAndDismissAlert {
        #[usage(long, default = "10", value_hint = usage::ValueHint::Other, help = "Maximum wait time in seconds")]
        timeout: u64,
    },
    /// Print whether a link with the given text is visible.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com is-link-text-visible --text 'More information...'"
    )]
    IsLinkTextVisible {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Exact link text to check")]
        text: String,
    },
    /// Print whether a link containing the given text is visible.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com is-partial-link-text-visible --text 'More info'"
    )]
    IsPartialLinkTextVisible {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Link text substring to check")]
        text: String,
    },
    /// Assert a link with the given text is visible.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com assert-link-text --text 'More information...'"
    )]
    AssertLinkText {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Exact link text to assert")]
        text: String,
    },
    /// Click a link containing the given text.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com click-partial-link-text --text 'More info'"
    )]
    ClickPartialLinkText {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Link text substring to click")]
        text: String,
    },
    /// Type text with human-like timing and noise.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com human-type --css '#q' --text hello"
    )]
    HumanType {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Text to type with humanized timing")]
        text: String,
    },
    /// Click an element with human-like timing and noise.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com human-click --css a"
    )]
    HumanClick {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Smoothly scroll an element into view.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com smooth-scroll-to --css footer"
    )]
    SmoothScrollTo {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Perform a UC-mode click on an element.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --uc --url https://example.com uc-click --css a"
    )]
    UcClick {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
    },
    /// Perform UC-mode typing into an element.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --uc --url https://example.com uc-type --css '#q' --text hello"
    )]
    UcType {
        #[usage(long, value_hint = usage::ValueHint::Other, help = "CSS selector for the target element")]
        css: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Text to type in UC mode")]
        text: String,
    },
    /// Download a driver or a browser into ./downloaded_drivers/ (Python's `sbrs get`).
    ///
    /// Without a version, chromedriver and edgedriver are the drivers for the
    /// installed Chrome and Edge, and geckodriver is the newest release. uc_driver is the
    /// chromedriver patched for the UC mode; cft, chs and chromium are browsers, and chromium
    /// takes a snapshot --revision instead of a version.
    #[usage(alias = "get")]
    #[usage(
        help_heading = "Drivers, grid and servers",
        example = "sbrs install chromedriver latest"
    )]
    Install {
        /// The program: chromedriver, uc_driver, geckodriver (or firefoxdriver), edgedriver
        /// (or msedgedriver), or the browsers cft, chs and chromium.
        #[usage(value_enum)]
        driver: DriverArg,
        /// Also copy the driver into the folder named by SB_DRIVER_PATH_DIR (else /usr/local/bin).
        /// Never uses sudo and never replaces a symbolic link. Drivers only.
        #[usage(short = 'p', long, bool_value)]
        path: bool,
        /// The Chromium snapshot to take (chromium only); the newest when left out.
        #[usage(long, value_name = "REVISION", value_hint = usage::ValueHint::Other)]
        revision: Option<u64>,
        /// A version word (stable, beta, dev, canary, latest, previous, mlatest, browser, keep),
        /// a major version, a full build,
        /// or for geckodriver a version such as 0.36.0.
        #[usage(complete = install_version_candidates)]
        version: Option<String>,
    },
    /// Run the project's tests, or one example, through Cargo or cargo-nextest.
    ///
    /// Every option of Python SeleniumBase's pytest plugin is a flag of `test`, in each of
    /// its spellings (`--no-sandbox`, `--no_sandbox`, `--dcsp`): the flags reach the tests
    /// as SB_* environment variables, which a test that loads `Settings` reads. The options
    /// before the command (`sbrs --headless --browser firefox test`) reach them too, and a
    /// flag after it wins. `--plugin` reaches them as SB_PLUGINS, with the plugins the
    /// settings file and the variable already turn on; `--plugin-opt` cannot be passed
    /// on, so `sbrs test` refuses it.
    ///
    /// `-k` and `-m` select tests as pytest does (a marker is a module name), `-n` sets
    /// how many run at once, `--co` lists them, `--pdb`, `--trace` and `--final-debug`
    /// stop in a prompt (one test at a time, output shown), `--reruns N` runs a failing
    /// test again, and `--archive-logs` keeps the old latest_logs folder, which a run
    /// otherwise empties. `-c FILE` (or `--settings-file`) names the settings file the
    /// tests read.
    ///
    /// `sbrs test` starts the run of the tests before Cargo and ends it after: `--dashboard`
    /// writes dashboard.html (live while the tests run), `--html PATH` and `--junit-xml PATH`
    /// write a report, `--alluredir DIR` Allure results (the tested crate needs the library's
    /// `allure` feature), `--show-report` opens the report in a browser window and waits until
    /// it is closed, and `--with-db-reporting` records the run in the results database. The
    /// exit status is Cargo's, or 1 when a report could not be written. After a plain `cargo
    /// test` use `sbrs report finish`.

    #[usage(
        help_heading = "Tests",
        example = "sbrs test login",
        example = "sbrs test -k 'login and not slow' --runner nextest --headless --browser firefox"
    )]
    Test(TestArgs),
    /// Print how Python SeleniumBase's behave options map to `sbrs test`.
    #[usage(help_heading = "Tests", example = "sbrs behave-options")]
    BehaveOptions,
    /// List the browser options: each flag, its `SB_*` variable and its key in the settings file.
    ///
    /// The same options can be set in the settings file, in the environment and with these
    /// flags; the command line wins over the environment, which wins over the file.
    /// `--env` lists the environment variables instead, those of `sbrs-mcp` too. `--markdown`
    /// prints the table for the documentation, and `--all` adds the options of the test run
    /// (`sbrs test`).
    #[usage(
        help_heading = "Info and setup",
        example = "sbrs options",
        example = "sbrs options --markdown"
    )]
    Options(browser_flags::OptionsArgs),
    /// List the common methods of the test API, as they are called on `sb`.
    ///
    /// Python SeleniumBase's `sbase methods` in Rust spelling: the methods most tests use
    /// (`sb.open(url)`, `sb.click(selector)`, `sb.assert_text(text, selector)`, ...), each
    /// with what it does. The whole API is in `cargo doc --open`, and docs/parity.md maps
    /// Python's names to it.
    #[usage(help_heading = "Info and setup", example = "sbrs methods")]
    Methods,

    /// Start, stop, restart or check a Selenium Grid hub.
    ///
    /// Needs a Selenium Server jar: --jar, SB_SELENIUM_SERVER_JAR, or the newest one in
    /// downloaded_drivers/ (`sbrs download server` saves one there). This command downloads
    /// nothing. The
    /// process record and log are kept in a folder of your own (SB_GRID_DIR).
    #[usage(alias = "grid_hub")]
    #[usage(
        help_heading = "Drivers, grid and servers",
        example = "sbrs grid-hub start",
        example = "sbrs grid_hub start -v --timeout 120"
    )]
    GridHub(GridArgs),
    /// Start, stop, restart or check a Selenium Grid node.
    ///
    /// A node registers with the hub named by `--hub`, or with this machine's.
    #[usage(alias = "grid_node")]
    #[usage(
        help_heading = "Drivers, grid and servers",
        example = "sbrs grid-node start",
        example = "sbrs grid-node start --hub 127.0.0.1"
    )]
    GridNode(GridArgs),
    /// Write the actions of a JSON scenario in another language.
    ///
    /// Rewrites each step's `action` (`click`, `open`, ...) with the translation
    /// dictionary and changes nothing else in the file; the steps keep running,
    /// because a scenario can be read in any of the languages. Python
    /// SeleniumBase translates Python test files; a Rust test is compiled code,
    /// so this translates the portable JSON scenario instead.
    #[usage(
        help_heading = "Files and generators",
        example = "sbrs translate scenario.json zh --print"
    )]
    Translate {
        /// The scenario file.
        #[usage(value_hint = usage::ValueHint::FilePath, extensions("json"))]
        file: PathBuf,
        /// The language: a code (en, zh, nl, fr, it, ja, ko, pt, ru, es) or its
        /// English name.
        #[usage(
            choices("en", "zh", "nl", "fr", "it", "ja", "ko", "pt", "ru", "es"),
            choices_strict = false
        )]
        language: Language,
        /// Print the translation and write nothing (the default).
        #[usage(short, long, bool_value, conflicts("--overwrite", "--copy"))]
        #[expect(
            dead_code,
            reason = "printing is the default; the flag only conflicts with the others"
        )]
        print: bool,
        /// Replace the file with the translation.
        #[usage(short, long, bool_value, conflicts("--copy"))]
        overwrite: bool,
        /// Write the translation to a new file next to it, such as `login_es.json`.
        #[usage(short, long, bool_value)]
        copy: bool,
    },
    /// Create a folder with a runnable browser-test suite.
    ///
    /// Names are relative to the current directory and use letters, digits, `_`,
    /// `-` and `.`, with `/` between folders. Nothing is overwritten unless
    /// `--force` is given.
    ///
    /// The folder is one Cargo test target (`tests/<folder>/main.rs`) with example tests
    /// that use the rstest `sb` fixture, a parameterized test and a pure CDP test.
    /// `--bdd` adds a Gherkin scenario that runs on the built-in steps, and `--gha` a
    /// GitHub Actions workflow in `.github/workflows/` of the current folder.

    #[usage(help_heading = "Files and generators", example = "sbrs mkdir ui_tests")]
    Mkdir(args::generators::MkdirArgs),
    /// Create a browser-test file.
    ///
    /// The name follows the same rules as `mkdir`; `.rs` is added when missing.
    /// An existing file is never overwritten unless `--force` is given.
    ///
    /// The test uses the rstest `sb` fixture unless a syntax flag says otherwise: `--pf`
    /// (the default), `--bc` (a BaseCase made and quit by hand), `--cm` (the
    /// run_browser_test closure), `--dc` (the pure CDP `sb_cdp` fixture) or `--dm` (a pure
    /// CDP browser launched by hand). A language flag (`--en`, `--zh`, `--nl`, `--fr`,
    /// `--it`, `--ja`, `--ko`, `--pt`, `--ru`, `--es`) localises the strings it types and
    /// expects. `--rec` adds a recorder breakpoint for Recorder Mode, and `--uc` and
    /// `--cdp` write a basic test for UC Mode and for pure CDP.

    #[usage(alias = "new")]
    #[usage(
        help_heading = "Files and generators",
        example = "sbrs mkfile tests/login.rs"
    )]
    Mkfile(args::generators::MkfileArgs),
    /// Launch the Commander, a terminal app (needs a build with the `tui` feature).
    ///
    /// Lists the tests and examples under PATH and runs the one you pick with `cargo test
    /// --test NAME` or `cargo run --example NAME`, then shows the end of the output. PATH is a
    /// project folder, a folder of test files or one test file; by default the `tests/` and
    /// `examples/` folders of the current folder. `-k FILTER` starts with the list narrowed to
    /// the names that contain FILTER, as `/` does in the app.
    ///
    /// Python's `gui` is the same command, and its window is a terminal app here.

    #[usage(alias = "gui")]
    #[usage(
        help_heading = "Tests",
        example = "sbrs commander",
        example = "sbrs gui tests -k login"
    )]
    Commander {
        /// A project folder, a folder of test files, or one test file.
        #[usage(value_hint = usage::ValueHint::AnyPath)]
        #[cfg_attr(
            not(feature = "tui"),
            expect(dead_code, reason = "only the terminal app reads it")
        )]
        path: Option<String>,
        /// Start with the list narrowed to the names that contain FILTER.
        #[usage(short = 'k', value_name = "FILTER", value_hint = usage::ValueHint::Other)]
        #[cfg_attr(
            not(feature = "tui"),
            expect(dead_code, reason = "only the terminal app reads it")
        )]
        filter: Option<String>,
    },
    /// Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature).
    ///
    /// Lists the tests under PATH and creates, edits and saves a case plan for
    /// each, `case_plans/<test>.md` next to its file, and writes a summary of the
    /// plans to `case_summary.md`. PATH is a folder of tests or one test file;
    /// by default the `tests/` and `examples/` folders of the current folder. `-k FILTER`
    /// starts with the list narrowed to the tests whose id contains FILTER, as `/` does in
    /// the app.
    ///
    /// The Python command opens a Tkinter window; this one runs in the terminal.
    #[usage(alias = "case-plans")]
    #[usage(alias = "case_plans")]
    #[usage(
        help_heading = "Tests",
        example = "sbrs caseplans --starter",
        example = "sbrs case_plans tests -k login"
    )]
    Caseplans {
        /// A folder of tests, or one test file.
        #[usage(value_hint = usage::ValueHint::AnyPath)]
        #[cfg_attr(
            not(feature = "tui"),
            expect(dead_code, reason = "only the terminal app reads it")
        )]
        path: Option<String>,
        /// Start with the list narrowed to the tests whose id contains FILTER.
        #[usage(short = 'k', value_name = "FILTER", value_hint = usage::ValueHint::Other, conflicts("--starter"))]
        #[cfg_attr(
            not(feature = "tui"),
            expect(dead_code, reason = "only the terminal app reads it")
        )]
        filter: Option<String>,

        /// Write the starter checklist `CASE_PLANS.md` instead; this needs no
        /// `tui` feature.
        #[usage(long, bool_value, conflicts("path"))]
        starter: bool,
    },
    /// Launch the Behave Commander, a terminal app (needs a build with the `tui` feature).
    ///
    /// Lists the `.feature` files under PATH with their scenarios, runs the ones
    /// you mark with `cargo test` and shows the results. PATH is
    /// a folder of feature files or one file; by default `features`.
    ///
    /// A scenario is matched to the test your project binds to it with rstest-bdd:
    /// one with no test named after it is reported as undefined. The Python
    /// command opens a Tkinter window; this one runs in the terminal.
    #[usage(alias = "gui-behave")]
    #[usage(help_heading = "Tests", example = "sbrs behave-gui features")]
    BehaveGui {
        /// A folder of feature files, or one `.feature` file.
        #[usage(value_hint = usage::ValueHint::AnyPath, extensions("feature"))]
        #[cfg_attr(
            not(feature = "tui"),
            expect(dead_code, reason = "only the terminal app reads it")
        )]
        path: Option<String>,
        /// Prompt for a name, a title and scenarios and write a new feature
        /// file instead; this needs no `tui` feature.
        #[usage(long, bool_value, conflicts("path"))]
        new: bool,
    },
    /// Print a file.
    #[usage(
        help_heading = "Files and generators",
        example = "sbrs print --file tests/login.rs"
    )]
    Print {
        #[usage(long, value_hint = usage::ValueHint::FilePath, help = "File path to print")]
        file: String,
    },
    /// Objectify a test recording.
    #[usage(help_heading = "Files and generators", example = "sbrs objectify")]
    Objectify,
    /// Create an HTML presentation.
    #[usage(
        help_heading = "Files and generators",
        example = "sbrs mkpres --file deck.html"
    )]
    Mkpres {
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("html", "htm"), help = "Output HTML presentation path")]
        file: String,
    },
    /// Create an HTML chart.
    #[usage(
        help_heading = "Files and generators",
        example = "sbrs mkchart --file chart.html"
    )]
    Mkchart {
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("html", "htm"), help = "Output HTML chart path")]
        file: String,
    },
    /// Create a new test file with the Recorder: record your actions in a browser, then save them as a test.
    ///
    /// Python's `sbase mkrec` (also `codegen` and `record`). Starts a browser in Recorder
    /// Mode, opening --url first when it is given, and waits: type `c` and press Enter at the
    /// prompt, or press SHIFT+ESC in the browser with --ee. The recording is written to
    /// `recordings/NAME_rec.rs` and `recordings/NAME_rec.json`, shown, and copied to FILE.
    #[usage(
        visible_aliases = ["codegen", "record"],
        help_heading = "Record and replay",
        example = "sbrs mkrec new_test.rs --url seleniumbase.io/demo_page"
    )]
    Mkrec {
        #[usage(value_hint = usage::ValueHint::FilePath, extensions("rs"), help = "The test file to create (FILE.rs)")]
        file: String,
        #[usage(long, bool_value, help = "Overwrite the file when it exists")]
        overwrite: bool,
        #[usage(
            long,
            visible_alias = "gherkin",
            bool_value,
            help = "Also write the recording as a Gherkin feature (same as --rec-behave)"
        )]
        behave: bool,
    },
    /// Launch the Recorder, a terminal app (needs a build with the `tui` feature).
    ///
    /// A form for the file name, the start page, the browser and the output format, and three
    /// actions. Record hands the terminal to `sbrs mkrec` and takes it back when the recording is
    /// over; Run starts the recorded test (`cargo test --test NAME`); Replay runs
    /// `recordings/NAME_rec.json` in a fresh browser (`sbrs replay`). `--url`, `--browser`,
    /// `--uc`, `--cdp`, `--ee`, `--rec-behave`, `--rec-sb-mgr` and `--rec-sb-cdp` fill the form in
    /// when the app starts.
    ///
    /// Python's `sbase recorder` opens a Tkinter window; this one runs in the terminal.
    #[usage(
        help_heading = "Record and replay",
        example = "sbrs recorder",
        example = "sbrs recorder --uc --url seleniumbase.io"
    )]
    Recorder,
    /// Run a scenario file, the way to do several steps in one browser, and write an optional dashboard.
    #[usage(
        help_heading = "Tests",
        example = "sbrs run-scenario --file scenario.json"
    )]
    RunScenario {
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("json"), help = "Scenario file to run")]
        file: String,
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("html", "htm"), help = "Optional path to write a scenario dashboard")]
        dashboard: Option<String>,
    },
    /// Generate a completion script for a supported shell, or a Fig spec.
    #[usage(help_heading = "Info and setup", example = "sbrs completions zsh")]
    Completions {
        #[usage(
            value_enum,
            help = "Shell (or `fig` for a Fig spec) to generate completions for"
        )]
        shell: CompletionFormat,
    },
    /// Convert common SeleniumBase or Selenium Python code into a Rust test.
    #[usage(
        help_heading = "Files and generators",
        example = "sbrs import-python login.py --output tests/login.rs"
    )]
    ImportPython {
        /// Python source file to convert.
        #[usage(value_hint = usage::ValueHint::FilePath, extensions("py"))]
        file: String,
        /// Write generated Rust to this path instead of stdout.
        #[usage(short, long, value_hint = usage::ValueHint::FilePath, extensions("rs"), help = "Output file for the generated Rust test")]
        output: Option<String>,
        /// Override source API detection.
        #[usage(
            long,
            value_enum,
            default = "auto",
            help = "Source API to assume when converting Python code"
        )]
        source: ImportSourceArg,
        /// Generated Rust test function name.
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Name of the generated test function")]
        test_name: Option<String>,
    },
    /// Convert Playwright tests or a script (JavaScript, TypeScript or Python) into Rust tests for the pure CDP engine.
    #[usage(alias = "import-pw")]
    #[usage(
        help_heading = "Files and generators",
        example = "sbrs import-playwright login.spec.ts --output tests/login.rs"
    )]
    ImportPlaywright {
        /// Playwright source file to convert.
        #[usage(value_hint = usage::ValueHint::FilePath, extensions("js", "ts", "py"))]
        file: String,
        #[usage(short, long, value_hint = usage::ValueHint::FilePath, extensions("rs"), help = "Output file for the generated Rust tests")]
        output: Option<String>,
        #[usage(
            long,
            value_enum,
            help = "Source language (default: from the file extension, else from the content)"
        )]
        language: Option<PlaywrightLanguageArg>,
        #[usage(
            long,
            value_hint = usage::ValueHint::Other, help = "Name of the test generated for a script without test titles (default: the file name)"
        )]
        test_name: Option<String>,
    },
    /// Convert Puppeteer scripts or tests (JavaScript, TypeScript) or a Chrome Recorder JSON export into Rust tests for the pure CDP engine.
    #[usage(alias = "import-pptr")]
    #[usage(
        help_heading = "Files and generators",
        example = "sbrs import-puppeteer login.js --output tests/login.rs"
    )]
    ImportPuppeteer {
        /// Puppeteer source file (or Recorder `.json` export) to convert.
        #[usage(value_hint = usage::ValueHint::FilePath, extensions("js", "ts", "json"))]
        file: String,
        #[usage(short, long, value_hint = usage::ValueHint::FilePath, extensions("rs"), help = "Output file for the generated Rust tests")]
        output: Option<String>,
        #[usage(
            long,
            value_enum,
            help = "Source language (default: from the file extension, else from the content)"
        )]
        language: Option<PuppeteerLanguageArg>,
        #[usage(
            long,
            value_hint = usage::ValueHint::Other, help = "Name of the test generated for a script without test titles (default: the file name)"
        )]
        test_name: Option<String>,
    },

    /// Download a file from a URL (browser download with an HTTP fallback).
    #[usage(
        help_heading = "Page commands",
        example = "sbrs download-file https://example.com/report.pdf --output report.pdf"
    )]
    DownloadFile {
        #[usage(value_hint = usage::ValueHint::Url, help = "URL of the file to download")]
        url: String,
        #[usage(
            short,
            long,
            value_hint = usage::ValueHint::FilePath, help = "Output file path (defaults to the URL file name in the current directory)"
        )]
        output: Option<String>,
        #[usage(
            long,
            default = "30",
            value_hint = usage::ValueHint::Other, help = "Seconds to wait for the browser download before falling back to HTTP"
        )]
        timeout: u64,
    },
    /// Print a page to PDF and save it to disk.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs print-pdf https://example.com --output page.pdf"
    )]
    PrintPdf {
        #[usage(value_hint = usage::ValueHint::Url, help = "URL to render as a PDF")]
        url: String,
        #[usage(short, long, default = "page.pdf", value_hint = usage::ValueHint::FilePath, extensions("pdf"), help = "Output PDF file path")]
        output: String,
    },
    /// Print the text extracted from a page or PDF URL.
    #[cfg_attr(
        not(feature = "pdf"),
        expect(dead_code, reason = "a build without `pdf` only says how to get it")
    )]
    #[usage(
        help_heading = "Page commands",
        example = "sbrs pdf-text https://example.com/report.pdf"
    )]
    PdfText {
        #[usage(value_hint = usage::ValueHint::Url, help = "URL of the page or PDF to extract text from")]
        url: String,
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("pdf"), help = "Also keep the intermediate PDF at this path")]
        save_pdf: Option<String>,
    },
    /// Assert that a page or PDF URL contains the expected text.
    #[cfg_attr(
        not(feature = "pdf"),
        expect(dead_code, reason = "a build without `pdf` only says how to get it")
    )]
    #[usage(
        help_heading = "Page commands",
        example = "sbrs assert-pdf-text https://example.com/report.pdf --text Total"
    )]
    AssertPdfText {
        #[usage(value_hint = usage::ValueHint::Url, help = "URL of the page or PDF to check")]
        url: String,
        #[usage(long, value_hint = usage::ValueHint::Other, help = "Text that must appear in the PDF")]
        text: String,
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("pdf"), help = "Also keep the intermediate PDF at this path")]
        save_pdf: Option<String>,
    },
    /// Upload a local file into a file input.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com/upload choose-file 'input[type=file]' ./photo.png"
    )]
    ChooseFile {
        #[usage(value_hint = usage::ValueHint::Other, help = "CSS selector of the <input type=file> element")]
        selector: String,
        #[usage(value_hint = usage::ValueHint::FilePath, help = "Local file to upload")]
        file: String,
    },
    /// Run several assertions, then report every failure at once.
    ///
    /// The page they run on is the global --url, if there is one.
    #[usage(
        help_heading = "Page commands",
        example = "sbrs --url https://example.com deferred --assert element=p --assert 'text=p:This domain is for use in'"
    )]
    Deferred {
        #[usage(
            long = "assert",
            value_name = "SPEC",
            value_hint = usage::ValueHint::Other, help = "Assertion spec: element=<css>, text=<css>:<text>, title=<text>, or url=<text>"
        )]
        asserts: Vec<String>,
        #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("json"), help = "JSON file containing an array of assertion specs")]
        spec: Option<String>,
    },
    /// Replay a recorded action file (recordings/NAME_rec.json) against a fresh browser.
    #[usage(
        help_heading = "Record and replay",
        example = "sbrs replay recordings/new_test_rec.json"
    )]
    Replay {
        #[usage(value_hint = usage::ValueHint::FilePath, extensions("json", "yaml"), help = "JSON or YAML action file (recordings/NAME_rec.json)")]
        file: String,
    },
    /// Launch Chrome with an open DevTools endpoint and print MCP client configs.
    ///
    /// Starts Chrome with the Pure CDP engine (no chromedriver), prints the
    /// DevTools HTTP endpoint, the browser WebSocket URL and configs for
    /// Google's chrome-devtools-mcp and Microsoft's Playwright MCP, then keeps
    /// the browser running until Ctrl+C, which closes it and exits with 0.
    /// The global --browser (chrome, chromium, edge or another Chromium
    /// browser), --headless, --headed, --user-data-dir, --proxy, --user-agent, --locale, --ad-block, --extension-dir
    /// --proxy-list and --url (the page to open once the browser runs) apply as well.
    #[usage(
        help_heading = "Drivers, grid and servers",
        example = "sbrs devtools --port 9222"
    )]
    Devtools {
        #[usage(
            long,
            default_value_t = DEFAULT_DEVTOOLS_PORT,
            default = "9222",
            value_hint = usage::ValueHint::Other, help = "DevTools port on 127.0.0.1 (0 lets Chrome pick a free one)"
        )]
        port: u16,
        #[usage(
            long,
            value_name = "PATH",
            value_hint = usage::ValueHint::ExecutablePath, help = "Browser binary to launch, such as Microsoft Edge or another Chromium browser"
        )]
        browser_path: Option<String>,
        #[usage(
            long,
            bool_value,
            help = "Print the endpoint and configs as one line of JSON instead of text"
        )]
        json: bool,
    },
    /// List the built-in plugins, or show what one declares.
    ///
    /// Plugins run inside the browsers a command starts. Turn them on by name with the
    /// global options --plugin and --plugin-opt, the SB_PLUGINS variable, or the [plugins]
    /// table of the settings file; the command line wins over SB_PLUGINS, which wins over
    /// the file. `sbrs plugin list` shows the plugins this build has.
    #[usage(help_heading = "Info and setup", example = "sbrs plugin list")]
    Plugin(PluginArgs),
    /// Capture, write, check and compare browser fingerprints.
    ///
    /// A fingerprint file is what a browser claims to be: the user agent, the screen, the
    /// language, the graphics card and the rest, as one JSON file. `capture` reads it from
    /// a real browser; `export` writes a preset or a random identity; `check` and `diff`
    /// read files. The global --fingerprint FILE (or SB_FINGERPRINT, or the [fingerprint]
    /// table of the settings file) makes every browser a command starts present one.
    #[usage(help_heading = "Security", example = "sbrs fingerprint presets")]
    Fingerprint(FingerprintArgs),
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
    /// Whether the command starts a browser through `BaseCase`, so that the plugins the
    /// command line names must exist and are given to that browser.
    ///
    /// `devtools` is not here: it starts a browser of its own, in the Pure CDP engine, and
    /// checks and gives it the plugins itself. `test` is not here either: the tests it runs
    /// are built by the project, with the plugins the project has, so it only passes the
    /// names on.
    fn launches_browser(&self) -> bool {
        match self {
            Self::Version
            | Self::PatchChromedriver { .. }
            | Self::PatchChrome { .. }
            | Self::Encrypt { .. }
            | Self::Decrypt { .. }
            | Self::Doctor(_)
            | Self::Methods
            | Self::Prompt(_)
            | Self::Install { .. }
            | Self::Test(_)
            | Self::GridHub(_)
            | Self::GridNode(_)
            | Self::Proxy(_)
            | Self::Download(_)
            | Self::Translate { .. }
            | Self::Mkdir(_)
            | Self::Mkfile(_)
            | Self::Commander { .. }
            | Self::Recorder
            | Self::Caseplans { .. }
            | Self::BehaveGui { .. }
            | Self::Print { .. }
            | Self::Objectify
            | Self::Mkpres { .. }
            | Self::Mkchart { .. }
            | Self::Completions { .. }
            | Self::ImportPython { .. }
            | Self::ImportPlaywright { .. }
            | Self::ImportPuppeteer { .. }
            | Self::Devtools { .. }
            | Self::Plugin(_) => false,
            #[cfg(feature = "turso")]
            Self::Report(_) => false,
            _ => true,
        }
    }

    /// Whether the command speaks the DevTools Protocol, so that it needs a
    /// session with a DevTools connection when no mode was asked for.
    fn needs_devtools(&self) -> bool {
        matches!(
            self,
            Self::Cdp { .. }
                | Self::CacheClear
                | Self::CdpClickElement { .. }
                | Self::CdpTypeText { .. }
                | Self::PrintPdf { .. }
                | Self::PdfText { .. }
                | Self::AssertPdfText { .. }
                | Self::Fingerprint(_)
        )
    }

    /// Whether the command acts on, or reads, the page the browser is on. Every `sbrs`
    /// command starts a fresh browser, so such a command needs a start page (`--url`,
    /// `SB_START_PAGE` or the settings file) and is refused without one, before any browser
    /// starts. `cdp` is exempt: it speaks to the browser, not to a page.
    fn needs_page(&self) -> bool {
        self.acts_on_page()
            || self.changes_profile()
            || matches!(
                self,
                // Read what the page holds.
                Self::GetText { .. }
                        | Self::GetAttribute { .. }
                        | Self::GetProperty { .. }
                        | Self::GetValue { .. }
                        | Self::GetTitle
                        | Self::GetCurrentUrl
                        | Self::GetCookies
                        | Self::GetLocalStorageItem { .. }
                        | Self::GetWindowPosition
                        | Self::IsElementVisible { .. }
                        | Self::IsTextVisible { .. }
                        | Self::IsChecked { .. }
                        | Self::IsLinkTextVisible { .. }
                        | Self::IsPartialLinkTextVisible { .. }
                        | Self::AssertElement { .. }
                        | Self::AssertTextVisible { .. }
                        | Self::AssertTextNotVisible { .. }
                        | Self::AssertAttribute { .. }
                        | Self::AssertTitle { .. }
                        | Self::AssertLinkText { .. }
                        | Self::WaitForText { .. }
                        | Self::WaitForElementPresent { .. }
                        | Self::WaitForElementNotVisible { .. }
                        | Self::WaitForReadyStateComplete
                        | Self::Screenshot { .. }
                        | Self::SaveSource { .. }
                        | Self::SaveCookies { .. }
                        // Answer an alert that the page opens when it loads.
                        | Self::AcceptAlert
                        | Self::DismissAlert
                        | Self::GetAlertText
                        | Self::TypeAlertText { .. }
                        | Self::WaitForAndAcceptAlert { .. }
                        | Self::WaitForAndDismissAlert { .. }
            )
    }

    /// Whether the command changes the page, so that where the browser ended up is worth
    /// saying: these print `Now at: <url> (<title>)` after they act.
    fn acts_on_page(&self) -> bool {
        matches!(
            self,
            Self::Hover { .. }
                | Self::HoverAndClick { .. }
                | Self::SelectOption { .. }
                | Self::DragAndDrop { .. }
                | Self::CdpClickElement { .. }
                | Self::CdpTypeText { .. }
                | Self::AddText { .. }
                | Self::SendKeys { .. }
                | Self::ClickVisibleElements { .. }
                | Self::HighlightClick { .. }
                | Self::CheckIfUnchecked { .. }
                | Self::UncheckIfChecked { .. }
                | Self::HumanType { .. }
                | Self::HumanClick { .. }
                | Self::SmoothScrollTo { .. }
                | Self::UcClick { .. }
                | Self::UcType { .. }
                | Self::ClickPartialLinkText { .. }
        )
    }

    /// Whether the command's only lasting effect is on the browser profile (cookies, local
    /// storage, the cache). A fresh browser throws its profile away, so these need
    /// `--user-data-dir`.
    fn changes_profile(&self) -> bool {
        matches!(
            self,
            Self::ClearCookies
                | Self::LoadCookies { .. }
                | Self::SetLocalStorageItem { .. }
                | Self::RemoveLocalStorageItem { .. }
                | Self::ClearLocalStorage
                | Self::CacheClear
        )
    }

    /// Whether the command names the page it opens itself, so a start page is a second one.
    fn opens_own_url(&self) -> bool {
        matches!(
            self,
            Self::Open { .. }
                | Self::Smoke { .. }
                | Self::DownloadFile { .. }
                | Self::PrintPdf { .. }
                | Self::PdfText { .. }
                | Self::AssertPdfText { .. }
        )
    }
}

/// The name a command is typed as (`get-text`), for what a refusal says about it.
fn command_name(command: &Commands) -> String {
    let debug = format!("{command:?}");
    let variant: String = debug
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect();
    let mut name = String::with_capacity(variant.len() + 4);
    for (at, letter) in variant.chars().enumerate() {
        if letter.is_ascii_uppercase() {
            if at > 0 {
                name.push('-');
            }
            name.push(letter.to_ascii_lowercase());
        } else {
            name.push(letter);
        }
    }
    name
}

/// Refuses what only the command and its options together show to be a mistake.
///
/// `start_page_given` is whether the command line itself named a start page (`start_page` is
/// the page after the command line, `SB_START_PAGE` and the settings file have been
/// merged), and `user_data_dir` the profile folder the same sources name.
///
/// # Errors
///
/// An [`ArgumentError`] for a page command with no page to act on, a command that changes
/// the profile with no profile to keep, and a command that opens a URL of its own given a
/// start page on the command line.
fn check_page_arguments(
    command: &Commands,
    start_page_given: bool,
    start_page: Option<&str>,
    user_data_dir: Option<&str>,
) -> Result<(), ArgumentError> {
    let name = command_name(command);
    if start_page_given && command.opens_own_url() {
        return Err(ArgumentError::new(
            &name,
            format!("{name} opens its own URL; leave out --start-page"),
        ));
    }
    if command.needs_page() && start_page.is_none() {
        return Err(ArgumentError::new(
            &name,
            format!(
                "{name} acts on a page, and every sbrs command starts a fresh browser; give the \
             page: sbrs --url https://example.com {name} …"
            ),
        ));
    }
    if command.changes_profile() && user_data_dir.is_none() {
        return Err(ArgumentError::new(&name, format!("{name} changes the profile, which a fresh browser throws away; add --user-data-dir DIR")));
    }
    Ok(())
}

/// What a page command does after it has acted and before its browser quits.
#[derive(Clone, Copy, Debug)]
struct PageEnd {
    /// Print `Now at: <url> (<title>)`, so a click that navigates shows where it went.
    now_at: bool,
    /// Save a screenshot to `latest_logs/` and print its path (the global `--save-screenshot`).
    screenshot: bool,
}

impl PageEnd {
    /// Says where the browser is and saves the screenshot, as asked, and quits the browser.
    async fn finish(self, sb: &mut BaseCase) -> Result<(), Box<dyn std::error::Error>> {
        if self.now_at {
            let url = sb.get_current_url().await?;
            let title = sb.get_title().await?;
            println!("Now at: {url} ({title})");
        }
        if self.screenshot {
            let path = sb.save_screenshot_to_logs().await?;
            println!("Saved screenshot: {}", path.display());
        }
        sb.quit().await?;
        Ok(())
    }
}

/// Turns a start page into a URL a browser can open: a local path becomes a `file://` URL, a
/// bare host gets `https://`, and anything with a scheme (`about:blank`, `data:`) stays as it is.
fn start_page_url(page: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = navigable_url(page)?;
    let has_scheme = url.contains("://")
        || ["about:", "data:", "blob:", "javascript:"]
            .iter()
            .any(|scheme| url.starts_with(scheme));
    Ok(if has_scheme {
        url
    } else {
        format!("https://{url}")
    })
}

impl Commands {
    /// Whether the command opens no page of its own and takes none: the commands that read
    /// files, write files or print. A marker for the tests.
    #[cfg(test)]
    fn opens_no_page(&self) -> bool {
        !self.launches_browser()
    }
}

/// Download `url` to `dest` over plain HTTP. Returns the number of bytes written.
async fn http_download(url: &str, dest: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    seleniumbase_rs::install_tls_provider();
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
#[cfg(feature = "pdf")]
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
/// newline, so `echo secret | sbrs encrypt` works.
fn text_or_stdin(value: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(value) = value {
        return Ok(value);
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
    Ok(text.trim_end_matches(['\n', '\r']).to_owned())
}

/// Runs `download server`: saves the Selenium Server jar, then prints where it is and its
/// SHA-256, saying whether that was checked against the checksum pinned in this build.
async fn run_download(
    command: &args::tools::DownloadCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let args::tools::DownloadCommand::Server(server) = command;
    let folder = Path::new(seleniumbase_rs::browser::binaries::DOWNLOADS_DIR);
    let jar = sb_grid::download_server(server.version.as_deref(), folder).await?;
    println!("{}", jar.path.display());
    if jar.reused {
        eprintln!("It was downloaded already.");
    }
    if jar.verified {
        println!(
            "SHA-256 {}: verified against the checksum pinned in this build",
            jar.sha256
        );
    } else {
        println!(
            "SHA-256 {}: not verified (only Selenium Server {} has a checksum pinned in this build)",
            jar.sha256,
            sb_grid::SERVER_VERSION
        );
    }
    Ok(())
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
            "sbrs: left {} unknown action(s) as they were: {}",
            report.unknown.len(),
            report.unknown.join(", ")
        );
    }
    Ok(())
}

/// `sbrs recorder`: the Recorder terminal app. The options of the command line that name what
/// its form asks for (the page, the browser, Stealth Mode, how the recording ends and what it is
/// written as) are the form's first answers, as Python's app reads them from its command line.
#[cfg(feature = "tui")]
fn run_recorder_app(args: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let start = recorder_tui::Start {
        url: args.start_page.clone(),
        browser: args.browser.map(|browser| browser.name().to_owned()),
        uc: args.uc || args.cdp,
        esc_end: args.esc_end,
        behave: args.recorder.rec_behave,
        sb_mgr: args.recorder.rec_sb_mgr,
        sb_cdp: args.rec_sb_cdp,
    };
    if let Err(e) = recorder_tui::run(&start) {
        eprintln!("Failed to run the recorder: {e}");
        std::process::exit(1);
    }
    Ok(())
}

/// What the commands that are terminal apps do in a build without the `tui`
/// feature: say how to get it, and leave the terminal alone.
#[cfg(not(feature = "tui"))]
fn no_terminal_ui() -> ! {
    eprintln!(
        "this sbrs was built without the terminal UI; reinstall it with \
         `cargo install --git https://github.com/MustCodeAl/seleniumbase-rs \
         seleniumbase-rs-cli --features tui`"
    );
    std::process::exit(2);
}

/// What `pdf-text` and `assert-pdf-text` do in a build without the `pdf`
/// feature: say how to get it, before a browser is started.
#[cfg(not(feature = "pdf"))]
fn no_pdf_support() -> ! {
    eprintln!(
        "this sbrs was built without PDF text extraction; reinstall it with \
         `cargo install --git https://github.com/MustCodeAl/seleniumbase-rs \
         seleniumbase-rs-cli --features pdf`"
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
            eprintln!("sbrs: {error}");
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

/// What `sbrs doctor` found wrong, each problem with how to fix it: the error that gives the
/// command a failing exit status (1, "a check failed") unless `--warn-only` is given.
#[derive(Debug)]
struct DoctorFailed {
    problems: Vec<String>,
}

impl std::fmt::Display for DoctorFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.problems.len();
        let noun = if count == 1 { "problem" } else { "problems" };
        write!(f, "sbrs doctor found {count} {noun}")?;
        for problem in &self.problems {
            write!(f, "\n  - {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DoctorFailed {}

async fn run_doctor(options: &DoctorArgs) -> Result<(), Box<dyn std::error::Error>> {
    println!("seleniumbase-rs environment diagnostics");
    println!("========================================");
    // What is wrong, one sentence each with its fix: they decide the exit status.
    let mut failures: Vec<String> = Vec::new();

    let (runtime, problems) = RuntimeConfig::from_env_lenient();
    for problem in &problems {
        println!("Invalid variable, default used: {problem}");
    }
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
    println!(
        "SB_LOG_LEVEL: {}",
        runtime
            .log_level
            .as_deref()
            .unwrap_or("(unset: RUST_LOG, then info)")
    );
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
            failures.push(format!(
                "the browser at {} does not exist: set SB_CHROME_BIN to a valid Chrome or \
                 Chromium executable",
                path.display()
            ));
        }
        None => {
            println!("  Not found");
            println!("  Hint: install Google Chrome/Chromium or set SB_CHROME_BIN");
            failures.push(
                "no Chrome or Chromium was found: install Google Chrome or Chromium, or set \
                 SB_CHROME_BIN to its executable"
                    .to_owned(),
            );
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
                        "  Hint: run `sbrs patch-chrome --path <chrome>` to populate the cache"
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
            let browser = match &chrome_bin {
                Some(chrome) => binary_major(chrome).await,
                None => None,
            };
            match (driver, browser) {
                (Some(driver), Some(browser)) if driver == browser => {
                    println!("  Version: {driver} (matches Chrome {browser})");
                }
                (Some(driver), Some(browser)) => {
                    println!("  Version: {driver}, but Chrome is {browser}");
                    failures.push(format!(
                        "chromedriver {driver} cannot drive Chrome {browser}: a chromedriver only \
                         drives its own Chrome version, so run `sbrs install` to fetch the \
                         matching one"
                    ));
                    println!(
                        "  Hint: a chromedriver only drives its own Chrome version; \
                         run `sbrs install` to fetch the matching one"
                    );
                }
                (Some(driver), None) => println!("  Version: {driver}"),
                (None, _) => {}
            }
            match ChromedriverPatcher::new(&path).needs_patch() {
                Ok(true) => {
                    println!("  Status: contains automation markers");
                    println!(
                        "  Hint: run `sbrs patch-chromedriver --path {}`",
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
            println!("  Hint: fix or unset CHROMEDRIVER_PATH, or run `sbrs install`");
        }
        assets::DriverLookup::NotFound => {
            println!("  Not found in {}/ or on PATH", assets::DRIVERS_DIR);
            println!("  Hint: run `sbrs install` or set CHROMEDRIVER_PATH");
        }
    }

    if !failures.is_empty() {
        if !options.warn_only {
            return Err(Box::new(DoctorFailed { problems: failures }));
        }
        println!();
        println!("Problems (--warn-only: the exit status stays 0)");
        for failure in &failures {
            println!("  - {failure}");
        }
    }

    Ok(())
}

/// `sbrs prompt`: the line for a shell prompt, or nothing when no browser is
/// found. Nothing it finds is an error; `--verbose` notes go to standard error
/// so that `$(sbrs prompt --verbose)` still holds only the line.
async fn run_prompt(options: &PromptArgs) -> Result<(), Box<dyn std::error::Error>> {
    let browser = prompt::locate_browser(
        options.browser_path.as_deref(),
        std::env::var_os("SB_CHROME_BIN").as_deref(),
        find_system_chrome,
    );
    let driver = assets::lookup_chromedriver();
    let cache_path = prompt::VersionCache::default_path().filter(|_| !options.no_cache);
    let mut cache = cache_path
        .clone()
        .map_or_else(prompt::VersionCache::disabled, prompt::VersionCache::open);
    let now = std::time::SystemTime::now();
    let found = prompt::probe(&browser, &driver, &mut cache, now).await;
    // A cache that cannot be written only makes the next prompt slower.
    let saved = cache.save(now);
    if options.verbose {
        for note in &found.notes {
            eprintln!("sbrs prompt: {note}");
        }
        match (&cache_path, saved) {
            (None, _) => eprintln!("sbrs prompt: cache: off"),
            (Some(path), Ok(())) => eprintln!("sbrs prompt: cache: {}", path.display()),
            (Some(path), Err(error)) => {
                eprintln!(
                    "sbrs prompt: cache: could not write {}: {error}",
                    path.display()
                );
            }
        }
    }
    if let Some(segment) = &found.segment {
        let marks = if options.ascii {
            prompt::Marks::Ascii
        } else {
            prompt::Marks::Unicode
        };
        let mut out = std::io::stdout().lock();
        writeln!(out, "{}", options.format.render(segment, marks))?;
        out.flush()?;
    }
    Ok(())
}

/// The stack `sbrs` runs on. Building the parser for every command and
/// matching them needs more than the 1 MB a main thread gets on Windows (a
/// debug build overflowed it before printing anything), so `sbrs` runs on a
/// thread of its own with a stack of a known size on every system.
const STACK_BYTES: usize = 16 * 1024 * 1024;

fn main() -> std::process::ExitCode {
    let worker = std::thread::Builder::new()
        .name("sbrs".to_owned())
        .stack_size(STACK_BYTES)
        .spawn(run_command);
    match worker.map(std::thread::JoinHandle::join) {
        Ok(Ok(code)) => code,
        // The thread panicked; the panic message has been printed already.
        Ok(Err(_)) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: could not start sbrs: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Runs the command on a Tokio runtime and turns its result into an exit status.
/// Whether `error`, or any error it was caused by, is a broken pipe.
fn is_broken_pipe(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::BrokenPipe)
        {
            return true;
        }
        current = error.source();
    }
    false
}

fn run_command() -> std::process::ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: could not start the async runtime: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    match runtime.block_on(execute()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        // The reader of standard output went away (`sbrs ... | head`): the
        // normal end of a pipeline, not a failure.
        Err(error) if is_broken_pipe(error.as_ref()) => std::process::ExitCode::SUCCESS,
        // An option the command refuses, said as a parse failure is, with no browser started.
        Err(error) if error.is::<ArgumentError>() => {
            eprint!("{}", failure::render_argument_error(error.as_ref()));
            std::process::ExitCode::from(2)
        }
        Err(error) => {
            // `error:`, its causes and a `hint:`, and an exit status that
            // says which kind of failure it was (see `exit_code_for`).
            rich_helper::print_error_report(error.as_ref());
            std::process::ExitCode::from(rich_helper::exit_code_for(error.as_ref()))
        }
    }
}

/// `sbrs test --settings-file FILE` is `sbrs -c FILE test`: the one setting names the file the
/// command and the tests it starts read.
fn with_settings_file(mut cli: Cli) -> Cli {
    if cli.config.is_none() {
        if let Commands::Test(test) = &cli.command {
            cli.config = test.settings_file();
        }
    }
    cli
}

/// `Cli::parse`, after the checks of [`strict`] that usage 6 does not make itself. A command line that
/// does not parse prints the page of the command its words reached (see [`failure`]) and then the
/// error, on standard error, and exits with status 2. Help, version and completion requests are
/// answered by `Cli::parse` as before.
fn parse_command_line() -> Cli {
    let argv = test_args::python_spellings(std::env::args_os().skip(1).collect());
    let words: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsString::as_os_str).collect();
    if words
        .first()
        .is_some_and(|word| *word == failure::COMPLETE_WORD)
    {
        return Cli::parse();
    }
    if let Err(error) = strict::check(Cli::spec(), Cli::command(), &words) {
        eprint!("{}", failure::render(&words, &error));
        std::process::exit(2);
    }
    match Cli::parse_from(&words) {
        Ok(cli) => with_settings_file(cli),
        Err(error) if failure::is_request(&error) => Cli::parse(),
        Err(error) => {
            eprint!("{}", failure::render(&words, &error));
            std::process::exit(2);
        }
    }
}

async fn execute() -> Result<(), Box<dyn std::error::Error>> {
    // One bad SB_* variable must not drop the good ones, SB_LOG_LEVEL among
    // them; the bad ones are reported once logging works.
    let (runtime, problems) = RuntimeConfig::from_env_lenient();
    init_tracing_from_runtime(&runtime);
    for problem in &problems {
        tracing::warn!(%problem, "ignored an invalid SB_* variable, its default is used");
    }

    let args = parse_command_line();
    let stops_itself = matches!(
        args.command,
        Commands::Mkrec { .. }
            | Commands::Recorder
            | Commands::Devtools { .. }
            | Commands::Proxy(_)
    );
    // `encrypt` and `decrypt` with no argument on a terminal prompt in a loop and handle
    // Ctrl-C themselves (`Exiting...`, status 0).
    let stops_itself = stops_itself
        || (matches!(
            args.command,
            Commands::Encrypt { text: None } | Commands::Decrypt { token: None }
        ) && crypto::prompts());

    // `proxy` stops itself on SIGINT or SIGTERM and ends with 0. `record` stops itself on
    // SIGINT or SIGTERM so that it can save what it
    // captured, and `devtools` so that it can close its browser and exit
    // with 0. A signal cancels any other command, which closes its browser.
    //
    // `run` holds the locals of every command, so its future is large. It is
    // boxed so that it lives on the heap: kept on the main thread's stack, and
    // copied again by the signal race, it overflowed the 8 MiB stack in debug
    // builds as soon as a command compiled a few regular expressions.
    let work = Box::pin(run(args));
    let outcome = if stops_itself {
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

/// The proxy a command uses: `proxy` as given, or, with a proxy list, the
/// entry `proxy` names in it (a random entry when `proxy` is unset).
fn resolve_proxy(
    proxy: Option<&str>,
    proxy_list: Option<&str>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // The flag, else its variable.
    let proxy_list = proxy_list
        .map(str::to_owned)
        .or_else(|| seleniumbase_rs::config::env_vars::text("SB_PROXY_LIST"));
    let Some(path) = proxy_list.as_deref() else {
        return Ok(proxy.map(str::to_owned));
    };
    // With a list, the proxy may be the name that SB_PROXY gives.
    let proxy = proxy
        .map(str::to_owned)
        .or_else(|| seleniumbase_rs::config::env_vars::text("SB_PROXY"));
    let proxy = proxy.as_deref();
    let list = ProxyList::from_file(path)?;
    Ok(Some(match proxy {
        Some(name_or_proxy) => list.resolve(name_or_proxy)?,
        None => {
            let entry = list
                .random()
                .ok_or_else(|| format!("the proxy list {path} is empty"))?;
            rich_helper::print_notice(&format!("Using proxy {entry}"));
            entry.spec().to_owned()
        }
    }))
}

/// The folder `sbrs patch-chrome` keeps the patched copy in: the flag, else the variable
/// (`SB_PATCH_CACHE_DIR`), else a folder of the user's cache.
fn patch_cache_dir(flag: Option<String>, variable: Option<String>) -> PathBuf {
    flag.or(variable).map(PathBuf::from).unwrap_or_else(|| {
        dirs::cache_dir()
            .unwrap_or_default()
            .join("sb-chrome-patches")
    })
}

/// The browser `sbrs devtools` launches: its binary (`None` to search for
/// Chrome), whether to search for Chromium instead, and its name.
fn devtools_browser(
    browser: Option<BrowserArg>,
    browser_path: Option<&str>,
) -> Result<(Option<PathBuf>, bool, String), Box<dyn std::error::Error>> {
    if let Some(path) = browser_path {
        return Ok((
            Some(PathBuf::from(path)),
            false,
            format!("The browser at {path}"),
        ));
    }
    match browser {
        None | Some(BrowserArg::Chrome) => Ok((None, false, "Google Chrome".to_owned())),
        Some(BrowserArg::Chromium) => Ok((None, true, "Chromium".to_owned())),
        Some(BrowserArg::Firefox | BrowserArg::Safari | BrowserArg::Ie) => Err(
            "sbrs devtools launches Chromium browsers (Chrome, Chromium, Edge, ...), which \
             speak the DevTools Protocol; for Firefox and Safari see \
             docs/tutorials/devtools_mcp.md"
                .into(),
        ),
        Some(other) => {
            let kind = Browser::from(other);
            let name = match other {
                BrowserArg::Edge => "Microsoft Edge".to_owned(),
                _ => format!("{kind:?}"),
            };
            let path = seleniumbase_rs::browser::binaries::locate(kind).ok_or_else(|| {
                format!("{name} was not found on this machine; install it or pass --browser-path")
            })?;
            Ok((Some(path), false, name))
        }
    }
}

/// `sbrs devtools`: launches a browser with an open DevTools endpoint, prints
/// how other tools attach to it, and keeps it running until Ctrl-C (or
/// SIGTERM), which closes it and exits with 0, or until its window is closed.
async fn run_devtools(args: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let Commands::Devtools {
        port,
        browser_path,
        json,
    } = &args.command
    else {
        return Err("run_devtools runs only for sbrs devtools".into());
    };
    // Listen before the browser starts, so a Ctrl-C during the start is heard.
    let mut shutdown = Shutdown::install()?;
    let (executable, use_chromium, name) = devtools_browser(args.browser, browser_path.as_deref())?;
    let choice = plugin_choice(args)?;
    choice.check()?;
    choice.log();
    // The page to open once the browser runs: the global --url, else SB_START_PAGE.
    let page = args.start_page.clone().or_else(|| {
        std::env::var(seleniumbase_rs::config::settings::START_PAGE_ENV)
            .ok()
            .filter(|page| !page.trim().is_empty())
    });
    let request = DevtoolsRequest {
        headless: if args.headless {
            Some(true)
        } else if args.headed {
            Some(false)
        } else {
            None
        },
        port: *port,
        user_data_dir: args.user_data_dir.as_deref().map(PathBuf::from),
        proxy: resolve_proxy(args.proxy.as_deref(), args.proxy_list.as_deref())?,
        executable,
        use_chromium,
        user_agent: args.user_agent.clone().or_else(|| args.agent.clone()),
        locale: args.locale.clone(),
        ad_block: args.ad_block,
        extension_dir: args.extension_dir.as_deref().map(PathBuf::from),
        url: page.as_deref().map(start_page_url).transpose()?,
        plugins: choice.into_selection(),
        // The flag, else SB_FINGERPRINT, else the `[fingerprint]` table of the settings file.
        fingerprint: Settings::resolve_fingerprint_file(
            args.config.as_deref(),
            args.fingerprint.as_deref(),
        )?
        .map(seleniumbase_rs::Fingerprint::load)
        .transpose()?,
    };
    if args.uc || args.cdp {
        rich_helper::print_notice(
            "sbrs devtools always uses the Pure CDP engine, with no chromedriver; --uc and --cdp change nothing",
        );
    }
    ensure_port_is_free(request.port)?;
    let options = request.launch_options()?;
    let browser = seleniumbase_rs::sb_cdp::Browser::launch(options).await?;
    let endpoint = browser.devtools_endpoint();
    let mut out = std::io::stdout();
    if *json {
        let mut answer = endpoint.to_json();
        answer["browser"] = json!(name);
        writeln!(out, "{}", serde_json::to_string(&answer)?)?;
    } else {
        write!(out, "{}", ready_message(&endpoint, &name))?;
    }
    out.flush()?;

    loop {
        tokio::select! {
            signal = shutdown.recv() => {
                eprintln!("{signal} received; closing the browser.");
                break;
            }
            () = tokio::time::sleep(Duration::from_millis(500)) => {
                if !browser.is_connected() {
                    eprintln!("The browser was closed; exiting.");
                    return Ok(());
                }
            }
        }
    }
    let closed = cleanup_within(timeout_from_env(), async {
        if let Err(error) = browser.close().await {
            eprintln!("warning: closing the browser failed: {error}");
        }
    })
    .await;
    if !closed {
        eprintln!("warning: the browser did not close in time; it is being ended.");
    }
    Ok(())
}

/// Which importer `import_script` runs.
#[derive(Clone, Copy, Debug)]
enum ScriptTool {
    Playwright,
    Puppeteer,
}

/// `sbrs import-playwright` and `sbrs import-puppeteer`: the Rust goes to
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

/// What `sbrs completions` writes: a shell's completion script, or a Fig spec.
#[derive(Clone, Copy, Debug, usage::ValueEnum)]
enum CompletionFormat {
    Bash,
    Zsh,
    Fish,
    Elvish,
    Powershell,
    /// A Fig completion spec (TypeScript), the format of the withfig/autocomplete
    /// repository that Warp's built-in completions, inshellisense and Amazon Q use.
    Fig,
    /// A usage spec (KDL) for jdx/usage and mise, which turn it into completions,
    /// Markdown and man pages.
    Usage,
}

/// The usage spec, generated from the declarations above by a test in this file.
const USAGE_SPEC: &str = include_str!("../../../completions/sbrs.usage.kdl");

impl CompletionFormat {
    /// The shell this format is for, or `None` for the Fig and usage specs.
    fn shell(self) -> Option<usage::complete::Shell> {
        use usage::complete::Shell;
        match self {
            Self::Bash => Some(Shell::Bash),
            Self::Zsh => Some(Shell::Zsh),
            Self::Fish => Some(Shell::Fish),
            Self::Elvish => Some(Shell::Elvish),
            Self::Powershell => Some(Shell::PowerShell),
            Self::Fig | Self::Usage => None,
        }
    }
}

/// The plugins a command line asks for: the `[plugins]` table of the settings file and
/// `SB_PLUGINS` (nothing else of the settings is read), with `--plugin` and `--plugin-opt`
/// on top, and where each name came from.
fn plugin_choice(args: &Cli) -> Result<PluginChoice, seleniumbase_rs::SeleniumBaseError> {
    PluginChoice::load(args.config.as_deref(), &args.plugin, &args.plugin_opt)
}

/// `sbrs plugin list` and `sbrs plugin info NAME`. They start no browser and read only the
/// plugin part of the settings, so a variable that is wrong for a browser does not stop them.
async fn run_plugin(command: &PluginCommand, args: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let choice = plugin_choice(args)?;
    let text = match command {
        PluginCommand::List => plugin_cmd::list(choice.selection()),
        PluginCommand::Info(info) => plugin_cmd::info(&info.name, choice.selection())?,
        PluginCommand::Run(run) => return run_plugin_task(run, args).await,
    };
    std::io::stdout().lock().write_all(text.as_bytes())?;
    Ok(())
}

/// `sbrs plugin run NAME`: launches a browser with the plugins the command line names and the
/// plugin `NAME`, runs the task of `NAME` on it, and closes it. The results go to `--out` (or
/// standard output) as JSON lines; the summary of the task goes to standard error.
#[cfg(feature = "plugins")]
async fn run_plugin_task(
    run: &PluginRunArgs,
    args: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    use seleniumbase_rs::common::shutdown::Outcome;
    use seleniumbase_rs::plugin_host;

    // The task's plugin is named by the command line, whatever else turned it on.
    let mut names = args.plugin.clone();
    if !names.contains(&run.name) {
        names.push(run.name.clone());
    }
    let choice = PluginChoice::load(args.config.as_deref(), &names, &args.plugin_opt)?;

    let mut settings = match args.config.as_deref() {
        Some(path) => Settings::load_from(path)?,
        None => Settings::load()?,
    };
    SettingsOverrides {
        browser: args.browser.map(|browser| browser.name().to_owned()),
        headless: args
            .headless
            .then_some(true)
            .or_else(|| args.headed.then_some(false)),
        user_agent: args.user_agent.clone().or_else(|| args.agent.clone()),
        locale: args.locale.clone(),
        ad_block: args.ad_block.then_some(true),
        proxy: resolve_proxy(args.proxy.as_deref(), args.proxy_list.as_deref())?,
        user_data_dir: args.user_data_dir.clone(),
        extension_dir: args.extension_dir.clone(),
        fingerprint: args
            .fingerprint
            .as_ref()
            .map(|file| file.display().to_string()),
        start_page: args.start_page.as_deref().map(start_page_url).transpose()?,
        ..SettingsOverrides::default()
    }
    .try_apply(&mut settings)?;
    settings.check_fingerprint()?;
    settings.plugins = choice.selection().clone();

    // Every mistake in the name, the options and the input is reported before a browser starts.
    let options = plugin_cmd::task_launch_options(settings, &run.name)?;
    // After the options, so that a name with no task is told with the names that have one.
    choice.check()?;
    choice.log();

    let input = plugin_cmd::read_task_input(run.input.as_deref())?;
    let sink = plugin_cmd::JsonlSink::open(run.out.as_deref())?;

    match plugin_host::run_task(
        options,
        &run.name,
        &input,
        &sink,
        &plugin_cmd::TerminalPrompt,
    )
    .await?
    {
        Outcome::Completed(summary) => {
            eprintln!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
        Outcome::Interrupted(signal) => {
            eprintln!("{signal}: the task `{}` was stopped", run.name);
            std::process::exit(signal.exit_code());
        }
    }
}

/// `sbrs plugin run NAME` in a build without plugins: there is none to run.
#[cfg(not(feature = "plugins"))]
async fn run_plugin_task(
    run: &PluginRunArgs,
    _args: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    plugin_cmd::info(&run.name, &PluginSelection::new())?;
    Ok(())
}

async fn run(args: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match &args.command {
        Commands::Completions { shell } => {
            let completion = match (shell, shell.shell()) {
                (_, Some(usage::complete::Shell::Zsh)) => {
                    completion::zsh(&Cli::completion_script(usage::complete::Shell::Zsh))
                }
                (_, Some(usage::complete::Shell::PowerShell)) => completion::powershell(
                    &Cli::completion_script(usage::complete::Shell::PowerShell),
                ),
                (_, Some(shell)) => Cli::completion_script(shell),
                (CompletionFormat::Usage, None) => USAGE_SPEC.to_owned(),
                (_, None) => fig::spec(Cli::spec()),
            };
            std::io::stdout().write_all(completion.as_bytes())?;
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
        Commands::Mkdir(mkdir) => {
            report_created(
                mkdir
                    .suite()
                    .and_then(|suite| suite.create(&here(mkdir.force))),
            );
            return Ok(());
        }
        Commands::Mkfile(mkfile) => {
            mkfile.check()?;
            report_created(
                mkfile
                    .test_file()
                    .and_then(|test| test.create(&here(mkfile.force)))
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
        Commands::Proxy(proxy) => return proxy::run(proxy).await,
        Commands::Download(download) => return run_download(&download.command).await,
        Commands::Report(report) => return report::run(report).await,

        Commands::Encrypt { text: None } if crypto::prompts() => {
            return crypto::interactive(crypto::Kind::Encrypt).await;
        }
        Commands::Decrypt { token: None } if crypto::prompts() => {
            return crypto::interactive(crypto::Kind::Decrypt).await;
        }

        Commands::Devtools { .. } => return run_devtools(&args).await,
        #[cfg(feature = "tui")]
        Commands::Recorder => return run_recorder_app(&args),
        #[cfg(not(feature = "tui"))]
        Commands::Recorder => no_terminal_ui(),
        Commands::Options(options) => {
            print!("{}", browser_flags::options_listing(options));
            return Ok(());
        }

        Commands::BehaveOptions => {
            print!("{}", behave_options::render());
            return Ok(());
        }
        Commands::Methods => {
            print!("{}", info::methods_listing());
            return Ok(());
        }

        Commands::Prompt(options) => return run_prompt(options).await,
        Commands::Plugin(plugin) => return run_plugin(&plugin.command, &args).await,
        // The subcommands that start no browser end here; `capture` goes on to start one.
        Commands::Fingerprint(fingerprint) => match fingerprint::run_offline(&fingerprint.command)?
        {
            Some(0) => return Ok(()),
            Some(status) => std::process::exit(status),
            None => {}
        },
        _ => {}
    }

    if args.cdp && args.uc {
        return Err("Choose either --cdp or --uc, not both.".into());
    }

    // Start from the config file (the one `--config` names, else the global
    // one) with the `SB_*` variables on top, as `Settings::load` does for the
    // tests `sbrs test` runs, then apply only what the command line asked for
    // explicitly. An option left alone keeps the environment's value or, failing
    // that, the file's, and one that was passed beats both.
    let mut settings = match args.config.as_deref() {
        Some(path) => Settings::load_from(path)?,
        None => Settings::load()?,
    };
    let proxy = resolve_proxy(args.proxy.as_deref(), args.proxy_list.as_deref())?;
    // The page the command line names, with `https://` or `file://` where it was left out.
    let start_page_flag = args.start_page.as_deref().map(start_page_url).transpose()?;
    // The flags of the option table that the root does not declare by hand, as the variables
    // of their rows. They are read as the environment is, after the typed flags.
    let mut browser_options = args
        .browser_flags
        .options()
        .map_err(|message| ArgumentError::new(&command_name(&args.command), message))?;
    // `--rec` and the other Recorder Mode flags are variables of their rows too.
    browser_options.extend(args.recorder.options());
    let gives = |variable: &str| browser_options.iter().any(|(name, _)| *name == variable);
    if args.browser.is_some() && gives("SB_BROWSER") {
        return Err(ArgumentError::new(
            &command_name(&args.command),
            "choose --browser or a browser flag such as --firefox, not both",
        )
        .into());
    }
    if args.webdriver.is_some() && gives("SB_SERVER") {
        return Err(ArgumentError::new(
            &command_name(&args.command),
            "choose --webdriver or --server, not both",
        )
        .into());
    }

    let overrides = SettingsOverrides {
        browser: args.browser.map(|browser| browser.name().to_owned()),
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
        threads: None,
        esc_end: args.esc_end.then_some(true),
        rec_sb_cdp: args.rec_sb_cdp.then_some(true),
        fingerprint: args.fingerprint.map(|file| file.display().to_string()),
        start_page: start_page_flag.clone(),
        options: browser_options,
    };
    overrides.try_apply(&mut settings)?;
    // A fingerprint file named on the command line is checked here, like the variable's and
    // the file's: a wrong path is an error, not a browser without the fingerprint.
    settings.check_fingerprint()?;
    // What only the command and the options together show to be a mistake is refused here,
    // before any browser starts.
    check_page_arguments(
        &args.command,
        start_page_flag.is_some(),
        settings.start_page.as_deref(),
        settings.user_data_dir.as_deref(),
    )?;

    // `SB_WEBDRIVER_URL` and `SB_CHROME_BIN` are settings too, so
    // `to_browser_config` has them; `--webdriver` goes on top.
    let mut config = settings.to_browser_config();
    // A command that speaks the DevTools Protocol gets a session that has a
    // DevTools connection when no mode was asked for, on a Chromium browser.
    let default_mode = if (args.command.needs_devtools() || settings.fingerprint.file.is_some())
        && config.browser.is_chromium_based()
    {
        DriverMode::Cdp
    } else {
        DriverMode::WebDriver
    };
    let mode = settings
        .mode
        .as_deref()
        .and_then(|mode| mode.parse::<DriverMode>().ok())
        .unwrap_or(default_mode);
    if let Some(url) = args.webdriver {
        config.webdriver_url = url;
    }
    config.mode = mode;
    config.auto_start_driver = true;
    // `BaseCase::new` opens the start page before the command acts. A command with a URL of
    // its own opens only that one.
    config.start_page = if args.command.opens_own_url() {
        None
    } else {
        settings
            .start_page
            .as_deref()
            .map(start_page_url)
            .transpose()?
    };
    let start_page_set = config.start_page.is_some();
    // What a page command does just before its browser quits.
    let after = PageEnd {
        now_at: args.command.acts_on_page(),
        screenshot: args.save_screenshot || settings.test.save_screenshot,
    };

    // The plugins: what the settings file and `SB_PLUGINS` turn on, with `--plugin` and
    // `--plugin-opt` on top. Only a command that starts a browser, and `test`, which passes
    // the names on, looks at them; the others ignore the options. A browser is not started
    // with a mistake in them.
    let launches = args.command.launches_browser();
    let is_test = matches!(args.command, Commands::Test(_));
    let choice = if launches || is_test {
        Some(PluginChoice::load(
            args.config.as_deref(),
            &args.plugin,
            &args.plugin_opt,
        )?)
    } else {
        None
    };
    if is_test && !args.plugin_opt.is_empty() {
        return Err(seleniumbase_rs::SeleniumBaseError::invalid_config(
            "`sbrs test` cannot pass --plugin-opt to the tests: SB_PLUGINS carries plugin names \
             only. Put the options in the [plugins.NAME] table of the settings file",
        )
        .into());
    }
    let forward_plugins = !args.plugin.is_empty();
    if let (true, Some(choice)) = (launches, &choice) {
        choice.check()?;
        choice.log();
        config = plugin_cmd::apply_to_config(config, choice.selection())?;
    }
    let selection = choice.map_or_else(PluginSelection::new, PluginChoice::into_selection);

    match args.command {
        Commands::Fingerprint(FingerprintArgs {
            command: FingerprintCommand::Capture(capture),
        }) => fingerprint::capture(capture, config).await?,
        Commands::Fingerprint(_) => {
            unreachable!("the other fingerprint subcommands return before a browser starts")
        }
        Commands::Open { url } => {
            let mut sb = BaseCase::new(config).await?;
            sb.open(&url).await?;
            let title = sb.get_title().await?;
            println!("Title: {title}");
            after.finish(&mut sb).await?;
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
            after.finish(&mut sb).await?;
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
            after.finish(&mut sb).await?;
        }
        Commands::CacheClear => {
            let mut sb = BaseCase::new(config).await?;
            sb.clear_browser_cache().await?;
            println!("CDP cache clear command sent.");
            after.finish(&mut sb).await?;
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
            after.finish(&mut sb).await?;
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
            after.finish(&mut sb).await?;
        }
        Commands::AssertElement { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_element(&css).await?;
            println!("Assertion passed: element exists for selector '{css}'");
            after.finish(&mut sb).await?;
        }
        Commands::WaitForText { css, text, timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_text(&text, &css, timeout).await?;
            println!("Text found for selector '{css}': {text}");
            after.finish(&mut sb).await?;
        }
        Commands::Hover { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.hover(&css).await?;
            println!("Hovered over element '{css}'");
            after.finish(&mut sb).await?;
        }
        Commands::HoverAndClick {
            hover_css,
            click_css,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.hover_and_click(&hover_css, &click_css).await?;
            println!("Hovered '{hover_css}' and clicked '{click_css}'");
            after.finish(&mut sb).await?;
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
            after.finish(&mut sb).await?;
        }
        Commands::DragAndDrop {
            source_css,
            target_css,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.drag_and_drop(&source_css, &target_css).await?;
            println!("Dragged '{source_css}' and dropped on '{target_css}'");
            after.finish(&mut sb).await?;
        }
        Commands::CdpClickElement { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.cdp_click(&css).await?;
            println!("CDP clicked element '{css}'");
            after.finish(&mut sb).await?;
        }
        Commands::CdpTypeText { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.cdp_insert_text(&text).await?;
            println!("CDP typed text '{text}'");
            after.finish(&mut sb).await?;
        }

        Commands::GetText { css } => {
            let mut sb = BaseCase::new(config).await?;
            let text = sb.get_text(&css).await?;
            println!("Text for '{}': {}", css, text);
            after.finish(&mut sb).await?;
        }
        Commands::GetAttribute { css, attribute } => {
            let mut sb = BaseCase::new(config).await?;
            let val = sb.get_attribute(&css, &attribute).await?;
            if let Some(v) = val {
                println!("Attribute '{}' for '{}': {}", attribute, css, v);
            } else {
                println!("Attribute '{}' not found for '{}'", attribute, css);
            }
            after.finish(&mut sb).await?;
        }
        Commands::GetProperty { css, property } => {
            let mut sb = BaseCase::new(config).await?;
            let val = sb.get_property(&css, &property).await?;
            if let Some(v) = val {
                println!("Property '{}' for '{}': {}", property, css, v);
            } else {
                println!("Property '{}' not found for '{}'", property, css);
            }
            after.finish(&mut sb).await?;
        }

        Commands::GetTitle => {
            let mut sb = BaseCase::new(config).await?;
            let title = sb.get_title().await?;
            println!("Title: {}", title);
            after.finish(&mut sb).await?;
        }
        Commands::GetCurrentUrl => {
            let mut sb = BaseCase::new(config).await?;
            let url = sb.get_current_url().await?;
            println!("Current URL: {}", url);
            after.finish(&mut sb).await?;
        }

        Commands::ClearCookies => {
            let mut sb = BaseCase::new(config).await?;
            sb.clear_browser_cookies().await?;
            println!("Cookies cleared");
            after.finish(&mut sb).await?;
        }

        Commands::AcceptAlert => {
            let mut sb = BaseCase::new(config).await?;
            sb.accept_alert().await?;
            println!("Accepted alert");
            after.finish(&mut sb).await?;
        }
        Commands::DismissAlert => {
            let mut sb = BaseCase::new(config).await?;
            sb.dismiss_alert().await?;
            println!("Dismissed alert");
            after.finish(&mut sb).await?;
        }
        Commands::GetAlertText => {
            let mut sb = BaseCase::new(config).await?;
            let text = sb.get_alert_text().await?;
            println!("Alert text: {}", text);
            after.finish(&mut sb).await?;
        }
        Commands::TypeAlertText { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.type_alert_text(&text).await?;
            println!("Typed alert text: {}", text);
            after.finish(&mut sb).await?;
        }
        Commands::ClearLocalStorage => {
            let mut sb = BaseCase::new(config).await?;
            sb.clear_local_storage().await?;
            println!("Cleared local storage");
            after.finish(&mut sb).await?;
        }
        Commands::GetLocalStorageItem { key } => {
            let mut sb = BaseCase::new(config).await?;
            let val = sb.get_local_storage_item(&key).await?;
            println!("Local storage item '{}': {}", key, val);
            after.finish(&mut sb).await?;
        }
        Commands::SetLocalStorageItem { key, value } => {
            let mut sb = BaseCase::new(config).await?;
            sb.set_local_storage_item(&key, &value).await?;
            println!("Set local storage item '{}' to '{}'", key, value);
            after.finish(&mut sb).await?;
        }
        Commands::RemoveLocalStorageItem { key } => {
            let mut sb = BaseCase::new(config).await?;
            sb.remove_local_storage_item(&key).await?;
            println!("Removed local storage item '{}'", key);
            after.finish(&mut sb).await?;
        }

        Commands::GetCookies => {
            let mut sb = BaseCase::new(config).await?;
            let cookies = sb.get_cookies().await?;
            println!("Cookies: {:?}", cookies);
            after.finish(&mut sb).await?;
        }
        Commands::PatchChromedriver { path } => {
            seleniumbase_rs::stealth::patcher::patch_chromedriver(&path)?;
            println!("Successfully patched chromedriver binary at: {path}");
        }
        Commands::PatchChrome { path, cache_dir } => {
            let cache = patch_cache_dir(
                cache_dir,
                seleniumbase_rs::config::env_vars::text("SB_PATCH_CACHE_DIR"),
            );
            let patcher = ChromeBinaryPatcher::new(path).with_cache_dir(cache);
            let patched = patcher.patch(EnginePatch::chrome_binary())?;
            println!("Patched Chrome binary available at: {}", patched.display());
        }
        Commands::Version => println!("sbrs {VERSION}"),
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
        Commands::Doctor(options) => {
            run_doctor(&options).await?;
        }
        Commands::AssertTextVisible { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_text_visible(&text, &css).await?;
            println!("Text '{}' is visible in '{}'", text, css);
            after.finish(&mut sb).await?;
        }
        Commands::AssertTextNotVisible { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_text_not_visible(&text, &css).await?;
            println!("Text '{}' is not visible in '{}'", text, css);
            after.finish(&mut sb).await?;
        }
        Commands::AssertAttribute {
            css,
            attribute,
            value,
        } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_attribute(&css, &attribute, &value).await?;
            println!("Attribute '{}' of '{}' is '{}'", attribute, css, value);
            after.finish(&mut sb).await?;
        }
        Commands::AssertTitle { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_title(&text).await?;
            println!("Title is '{}'", text);
            after.finish(&mut sb).await?;
        }
        Commands::WaitForReadyStateComplete => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_ready_state_complete().await?;
            println!("Ready state complete");
            after.finish(&mut sb).await?;
        }
        Commands::GetWindowPosition => {
            let mut sb = BaseCase::new(config).await?;
            let (x, y) = sb.get_window_position().await?;
            println!("Window position: x={}, y={}", x, y);
            after.finish(&mut sb).await?;
        }
        Commands::IsElementVisible { css } => {
            let mut sb = BaseCase::new(config).await?;
            let visible = sb.is_element_visible(&css).await?;
            println!("Element '{}' is visible: {}", css, visible);
            after.finish(&mut sb).await?;
        }
        Commands::IsTextVisible { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            let visible = sb.is_text_visible(&text, &css).await?;
            println!("Text '{}' in '{}' is visible: {}", text, css, visible);
            after.finish(&mut sb).await?;
        }
        Commands::WaitForElementNotVisible { css, timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_element_not_visible(&css, timeout).await?;
            println!("Element '{}' is not visible", css);
            after.finish(&mut sb).await?;
        }
        Commands::SaveCookies { file } => {
            let mut sb = BaseCase::new(config).await?;
            sb.save_cookies_to_path(std::path::Path::new(&file)).await?;
            println!("Saved cookies to '{}'", file);
            after.finish(&mut sb).await?;
        }
        Commands::LoadCookies { file } => {
            let mut sb = BaseCase::new(config).await?;
            sb.load_cookies_from_path(
                std::path::Path::new(&file),
                seleniumbase_rs::CookieExpiry::Saved,
            )
            .await?;
            println!("Loaded cookies from '{}'", file);
            after.finish(&mut sb).await?;
        }
        Commands::HighlightClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.highlight_click(&css).await?;
            println!("Highlighted and clicked '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::IsChecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            let checked = sb.is_checked(&css).await?;
            println!("Element '{}' is checked: {}", css, checked);
            after.finish(&mut sb).await?;
        }
        Commands::CheckIfUnchecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.check_if_unchecked(&css).await?;
            println!("Checked '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::UncheckIfChecked { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uncheck_if_checked(&css).await?;
            println!("Unchecked '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::WaitForElementPresent { css, timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_element_present(&css, timeout).await?;
            println!("Element '{}' is present", css);
            after.finish(&mut sb).await?;
        }
        Commands::AddText { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.add_text(&css, &text).await?;
            println!("Added text to '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::SendKeys { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.send_keys(&css, &text).await?;
            println!("Sent keys to '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::GetValue { css } => {
            let mut sb = BaseCase::new(config).await?;
            let value = sb.get_value(&css).await?;
            println!("Value of '{}': {}", css, value);
            after.finish(&mut sb).await?;
        }
        Commands::ClickVisibleElements { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.click_visible_elements(&css).await?;
            println!("Clicked visible elements matching '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::WaitForAndAcceptAlert { timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_and_accept_alert(timeout).await?;
            println!("Waited for and accepted alert");
            after.finish(&mut sb).await?;
        }
        Commands::WaitForAndDismissAlert { timeout } => {
            let mut sb = BaseCase::new(config).await?;
            sb.wait_for_and_dismiss_alert(timeout).await?;
            println!("Waited for and dismissed alert");
            after.finish(&mut sb).await?;
        }
        Commands::IsLinkTextVisible { text } => {
            let mut sb = BaseCase::new(config).await?;
            let visible = sb.is_link_text_visible(&text).await?;
            println!("Link text '{}' is visible: {}", text, visible);
            after.finish(&mut sb).await?;
        }
        Commands::IsPartialLinkTextVisible { text } => {
            let mut sb = BaseCase::new(config).await?;
            let visible = sb.is_partial_link_text_visible(&text).await?;
            println!("Partial link text '{}' is visible: {}", text, visible);
            after.finish(&mut sb).await?;
        }
        Commands::AssertLinkText { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.assert_link_text(&text).await?;
            println!("Link text '{}' asserted", text);
            after.finish(&mut sb).await?;
        }
        Commands::ClickPartialLinkText { text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.click_partial_link_text(&text).await?;
            println!("Clicked partial link text '{}'", text);
            after.finish(&mut sb).await?;
        }
        Commands::HumanType { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.human_type(&css, &text).await?;
            println!("Human typed into '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::HumanClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.human_click(&css).await?;
            println!("Human clicked '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::SmoothScrollTo { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.smooth_scroll_to(&css).await?;
            println!("Smooth scrolled to '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::UcClick { css } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uc_click(&css).await?;
            println!("UC clicked '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::UcType { css, text } => {
            let mut sb = BaseCase::new(config).await?;
            sb.uc_type(&css, &text).await?;
            println!("UC typed into '{}'", css);
            after.finish(&mut sb).await?;
        }
        Commands::Install {
            driver,
            version,
            path,
            revision,
        } => {
            let request = args_install::request(driver, version, path, revision)?;
            let installed = sb_install::install(&request).await?;
            println!("{}", args_install::report(request.name, &installed));
        }
        Commands::Test(test) => {
            // What the options before the command asked for, then the flags after it, which win.
            let mut forwarded: Vec<(String, String)> = overrides
                .env()
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect();
            if config.webdriver_url != "http://localhost:4444" {
                forwarded.push(("SB_WEBDRIVER_URL".to_owned(), config.webdriver_url.clone()));
            }
            // The variable replaces the names the file enables, so it carries all of them.
            if forward_plugins {
                forwarded.push(("SB_PLUGINS".to_owned(), selection.to_list()));
            }
            let plan = test.plan(&settings, forwarded, args.config.as_deref())?;
            let code = plan.execute().await?;
            if code != 0 {
                std::process::exit(code);
            }
        }
        Commands::Mkdir(_)
        | Commands::Mkfile(_)
        | Commands::Translate { .. }
        | Commands::GridHub(_)
        | Commands::GridNode(_)
        | Commands::Proxy(_)
        | Commands::Download(_)
        | Commands::Report(_)
        | Commands::BehaveOptions
        | Commands::Methods
        | Commands::Options(_)
        | Commands::Devtools { .. }
        | Commands::Recorder => {
            unreachable!("these commands return before browser configuration")
        }
        #[cfg(feature = "tui")]
        Commands::Commander { path, filter } => {
            let target = path.as_deref().map(Path::new);
            if let Err(e) = sb_commander::run_commander(target, filter.as_deref()) {
                eprintln!("Failed to run commander: {}", e);
                std::process::exit(1);
            }
        }
        #[cfg(not(feature = "tui"))]
        Commands::Commander { .. } => no_terminal_ui(),
        // The file-writing commands below hand their error to `main`, which
        // prints `error: ...` and exits with the status of its kind (6 for a
        // local file), instead of printing it here and exiting with 0.
        Commands::Caseplans { starter: true, .. } => {
            let path =
                sb_caseplans::run_caseplans().map_err(|e| context("generate case plans", e))?;
            println!("Generated {}", path.display());
        }
        #[cfg(feature = "tui")]
        Commands::Caseplans { path, filter, .. } => {
            let target = PathBuf::from(path.as_deref().unwrap_or("."));
            if let Err(e) = caseplans_tui::run(&target, filter.as_deref()) {
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
        Commands::Mkrec {
            file,
            overwrite,
            behave,
        } => {
            let mut options = sb_recorder::MkrecOptions::new(file);
            options.overwrite = overwrite;
            options.esc_end = args.esc_end;
            options.behave = behave;
            // Refuse a bad name before a browser starts.
            if let Err(message) = options.check() {
                eprintln!("ERROR: {message}");
                std::process::exit(1);
            }
            let outcome = sb_recorder::run_mkrec(config, &options, &mut std::io::stdout()).await?;
            if outcome.copied_to.is_none() {
                eprintln!(
                    "ERROR: nothing was recorded, so {} was not written",
                    options.file
                );
                std::process::exit(1);
            }
            // A Ctrl-C leaves the prompt's read of standard input waiting; the recording is
            // saved and the browser is closed, so do not wait for a line.
            if outcome.interrupted {
                std::process::exit(0);
            }
        }
        Commands::RunScenario { file, dashboard } => {
            let scenario_json = std::fs::read_to_string(&file)?;
            let scenario: Scenario = serde_json::from_str(&scenario_json)?;
            let mut sb = BaseCase::new(config).await?;
            let summary = run_scenario(&mut sb, &scenario).await?;

            if let Some(dashboard_path) = dashboard.as_deref() {
                write_dashboard(&summary, dashboard_path)?;
                println!("Dashboard written: {dashboard_path}");
            } else {
                let logs_dir = ensure_latest_logs_dir()?;
                let dashboard_path = artifact_path(&logs_dir, "dashboard", "html");
                write_dashboard(&summary, &dashboard_path)?;
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
            after.finish(&mut sb).await?;
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
            after.finish(&mut sb).await?;
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
            after.finish(&mut sb).await?;
            let size = std::fs::metadata(&output)
                .map(|meta| meta.len())
                .unwrap_or(0);
            println!("Saved PDF: {output} ({size} bytes)");
        }
        #[cfg(not(feature = "pdf"))]
        Commands::PdfText { .. } | Commands::AssertPdfText { .. } => no_pdf_support(),
        #[cfg(feature = "pdf")]
        Commands::PdfText { url, save_pdf } => {
            let mut sb = BaseCase::new(config).await?;
            let (path, _scratch) = resolve_pdf(&mut sb, &url, save_pdf.as_deref()).await?;
            let target = path.to_str().ok_or("PDF path is not valid UTF-8")?;
            let text = sb.get_pdf_text(target)?;
            after.finish(&mut sb).await?;
            println!("{text}");
        }
        #[cfg(feature = "pdf")]
        Commands::AssertPdfText {
            url,
            text,
            save_pdf,
        } => {
            let mut sb = BaseCase::new(config).await?;
            let (path, _scratch) = resolve_pdf(&mut sb, &url, save_pdf.as_deref()).await?;
            let target = path.to_str().ok_or("PDF path is not valid UTF-8")?;
            let outcome = sb.assert_pdf_text(target, &text);
            after.finish(&mut sb).await?;
            outcome?;
            println!("Assertion passed: PDF contains '{text}'");
        }
        Commands::ChooseFile { selector, file } => {
            let mut sb = BaseCase::new(config).await?;
            // The start page (the global --url) is the page that has the file input;
            // without one, a blank page that has it.
            if !start_page_set {
                sb.open(&html_data_url(&blank_file_input_page(&selector)))
                    .await?;
            }
            let outcome = sb.choose_file(&selector, &file).await;
            let value = match outcome {
                Ok(()) => sb.get_value(&selector).await.unwrap_or_default(),
                Err(err) => {
                    let _ = sb.quit().await;
                    return Err(err.into());
                }
            };
            after.finish(&mut sb).await?;
            println!("Uploaded '{file}' into '{selector}'");
            if !value.is_empty() {
                println!("Input value: {value}");
            }
        }
        Commands::Deferred { asserts, spec } => {
            let mut specs = parse_deferred_specs(&asserts)?;
            if let Some(path) = spec.as_deref() {
                let raw = std::fs::read_to_string(path)?;
                specs.extend(parse_deferred_spec_json(&raw)?);
            }
            if specs.is_empty() {
                return Err("no assertions provided: pass --assert or --spec".into());
            }

            // The start page, if there is one, is open already.
            let mut sb = BaseCase::new(config).await?;
            let mut results = Vec::with_capacity(specs.len());
            for entry in &specs {
                let outcome = match entry {
                    DeferredSpec::Element(selector) => sb.assert_element(selector).await,
                    DeferredSpec::Text(selector, expected) => {
                        sb.assert_text(expected, selector).await
                    }
                    DeferredSpec::Title(expected) => sb.assert_title_contains(expected).await,
                    DeferredSpec::Url(expected) => sb.assert_url_contains(expected).await,
                };
                results.push(DeferredResult {
                    description: entry.describe(),
                    error: outcome.err().map(|err| err.to_string()),
                });
            }
            after.finish(&mut sb).await?;

            print!("{}", format_deferred_summary(&results));
            if results.iter().any(|result| !result.passed()) {
                std::process::exit(1);
            }
        }
        Commands::Replay { file } => {
            let actions: Vec<RecordedAction> = RecorderSession::load_json(Path::new(&file))?;
            if actions.is_empty() {
                return Err(format!("no actions found in '{file}'").into());
            }
            let mut sb = BaseCase::new(config).await?;
            let report = RecorderSession::replay(&mut sb, &actions).await?;
            after.finish(&mut sb).await?;

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
            if !report.skipped_names.is_empty() {
                println!("Skipped: {}", report.skipped_names.join(", "));
            }
            if !report.is_success() {
                std::process::exit(1);
            }
        }
        Commands::Completions { .. }
        | Commands::ImportPython { .. }
        | Commands::ImportPlaywright { .. }
        | Commands::ImportPuppeteer { .. }
        | Commands::Plugin(_)
        | Commands::Prompt(_) => {
            unreachable!("pure CLI commands return before browser configuration")
        }
    }

    Ok(())
}

/// Parses `argv`, whose first word is the program name, as `parse_command_line` does: the
/// checks of [`strict`] first, then the parser. A request for help is an error here, with
/// nothing to say.
#[cfg(test)]
fn try_parse_checked(argv: &[&str]) -> Result<Cli, String> {
    let argv: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsStr::new).collect();
    let words = argv.get(1..).unwrap_or_default();
    strict::check(Cli::spec(), Cli::command(), words)
        .and_then(|()| Cli::parse_from_argv(&argv))
        .map_err(|error| Cli::render_failure(words, &error))
}

#[cfg(test)]
mod devtools_cli_tests {
    use super::*;

    fn try_parse(argv: &[&str]) -> Result<Cli, String> {
        try_parse_checked(argv)
    }

    fn parse(argv: &[&str]) -> Cli {
        try_parse(argv).expect("the arguments parse")
    }

    #[test]
    fn devtools_takes_its_own_options_with_port_9222_by_default() {
        let cli = parse(&["sbrs", "devtools"]);
        assert!(!cli.headless && cli.user_data_dir.is_none() && cli.proxy.is_none());
        assert!(cli.start_page.is_none());
        let Commands::Devtools {
            port,
            browser_path,
            json,
        } = cli.command
        else {
            panic!("not the devtools command");
        };
        assert_eq!(port, 9222);
        assert!(!json);
        assert!(browser_path.is_none());

        // The browser options and the page are the global ones, wherever they are written.
        let cli = parse(&[
            "sbrs",
            "devtools",
            "--headless",
            "--port",
            "0",
            "--user-data-dir",
            "/tmp/devtools",
            "--proxy",
            "host:8080",
            "--browser-path",
            "/opt/edge",
            "--url",
            "example.com",
            "--json",
        ]);
        assert!(cli.headless);
        assert_eq!(cli.user_data_dir.as_deref(), Some("/tmp/devtools"));
        assert_eq!(cli.proxy.as_deref(), Some("host:8080"));
        assert_eq!(cli.start_page.as_deref(), Some("example.com"));
        let Commands::Devtools {
            port,
            browser_path,
            json,
        } = cli.command
        else {
            panic!("not the devtools command");
        };
        assert!(json);
        assert_eq!(port, 0);
        assert_eq!(browser_path.as_deref(), Some("/opt/edge"));
    }

    #[test]
    fn the_global_switches_before_the_command_still_apply() {
        let cli = parse(&[
            "sbrs",
            "--headless",
            "--browser",
            "chromium",
            "--proxy",
            "global:1",
            "devtools",
        ]);
        assert!(cli.headless);
        assert_eq!(cli.browser, Some(BrowserArg::Chromium));
        assert_eq!(cli.proxy.as_deref(), Some("global:1"));
        assert!(matches!(cli.command, Commands::Devtools { .. }));
    }

    #[test]
    fn a_port_out_of_range_is_refused() {
        assert!(try_parse(&["sbrs", "devtools", "--port", "70000"]).is_err());
    }

    #[test]
    fn devtools_launches_chromium_browsers_only() {
        let (path, use_chromium, name) = devtools_browser(None, None).expect("Chrome");
        assert!(path.is_none() && !use_chromium);
        assert_eq!(name, "Google Chrome");
        let (_, use_chromium, _) =
            devtools_browser(Some(BrowserArg::Chromium), None).expect("Chromium");
        assert!(use_chromium);
        let (path, _, _) = devtools_browser(Some(BrowserArg::Firefox), Some("/opt/edge"))
            .expect("an explicit binary wins");
        assert_eq!(path, Some(PathBuf::from("/opt/edge")));
        for other in [BrowserArg::Firefox, BrowserArg::Safari, BrowserArg::Ie] {
            let error = devtools_browser(Some(other), None).expect_err("not Chromium");
            assert!(error.to_string().contains("devtools_mcp.md"), "{error}");
        }
    }

    #[test]
    fn a_proxy_without_a_list_is_used_as_given() {
        assert_eq!(
            resolve_proxy(Some("host:1"), None).expect("no list"),
            Some("host:1".to_owned())
        );
        assert_eq!(resolve_proxy(None, None).expect("no proxy"), None);
    }
}

#[cfg(test)]
mod recorder_cli_tests {
    use super::*;

    fn parse(argv: &[&str]) -> Cli {
        try_parse_checked(argv).expect("the arguments parse")
    }

    #[test]
    fn mkrec_takes_a_file_and_starts_a_browser() {
        let cli = parse(&["sbrs", "mkrec", "new_test.rs"]);
        assert!(cli.command.launches_browser());
        // The page is the global --url, so mkrec does not refuse it.
        assert!(!cli.command.opens_own_url());
        match cli.command {
            Commands::Mkrec {
                file,
                overwrite,
                behave,
            } => {
                assert_eq!(file, "new_test.rs");
                assert!(!overwrite && !behave);
            }
            other => panic!("not mkrec: {other:?}"),
        }
        assert!(try_parse_checked(&["sbrs", "mkrec"]).is_err());
    }

    #[test]
    fn codegen_and_record_are_mkrec() {
        for name in ["mkrec", "codegen", "record"] {
            let cli = parse(&["sbrs", name, "t.rs", "--overwrite", "--gherkin"]);
            assert!(
                matches!(
                    cli.command,
                    Commands::Mkrec {
                        overwrite: true,
                        behave: true,
                        ..
                    }
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn mkrec_uses_the_global_page_and_recorder_flags() {
        let cli = parse(&[
            "sbrs",
            "mkrec",
            "t.rs",
            "--url",
            "example.test",
            "--ee",
            "--rec-sb-mgr",
            "--rec-behave",
            "--edge",
            "--uc",
        ]);
        assert_eq!(cli.start_page.as_deref(), Some("example.test"));
        assert!(cli.esc_end && cli.recorder.rec_sb_mgr && cli.recorder.rec_behave && cli.uc);
        assert!(!cli.recorder.rec && !cli.recorder.rec_print && !cli.recorder.rec_sleep);
    }

    #[test]
    fn the_recorder_flags_go_before_any_command() {
        let cli = parse(&["sbrs", "--rec", "--rec-print", "--rec-sleep", "test"]);
        assert!(cli.recorder.rec && cli.recorder.rec_print && cli.recorder.rec_sleep);
        for spelling in ["--recorder", "--record", "--codegen"] {
            assert!(
                parse(&["sbrs", spelling, "test"]).recorder.rec,
                "{spelling}"
            );
        }
        assert!(
            parse(&["sbrs", "--rec-gherkin", "test"])
                .recorder
                .rec_behave
        );
        assert!(
            parse(&["sbrs", "--record-sleep", "test"])
                .recorder
                .rec_sleep
        );
    }

    #[test]
    fn export_recording_and_the_live_record_form_are_gone() {
        assert!(try_parse_checked(&["sbrs", "export-recording", "a.json"]).is_err());
        // `record` is the mkrec alias, so its argument is a file name now.
        assert!(matches!(
            parse(&["sbrs", "record", "https://x.test/"]).command,
            Commands::Mkrec { .. }
        ));
        assert!(try_parse_checked(&["sbrs", "mkrec", "t.rs", "--duration", "5"]).is_err());
    }

    #[test]
    fn the_recorder_is_a_terminal_app_that_starts_no_browser() {
        let cli = parse(&["sbrs", "recorder"]);

        assert!(matches!(cli.command, Commands::Recorder));
        assert!(!cli.command.launches_browser());
        assert!(!cli.command.opens_own_url() && !cli.command.needs_page());
        assert!(try_parse_checked(&["sbrs", "recorder", "extra"]).is_err());
    }

    #[test]
    fn the_recorder_takes_the_options_that_fill_in_its_form() {
        let after = parse(&[
            "sbrs",
            "recorder",
            "--url",
            "example.test",
            "--browser",
            "brave",
            "--uc",
            "--ee",
            "--rec-behave",
        ]);
        assert_eq!(after.start_page.as_deref(), Some("example.test"));
        assert!(matches!(after.browser, Some(BrowserArg::Brave)));
        assert!(after.uc && after.esc_end && after.recorder.rec_behave);

        let before = parse(&["sbrs", "--cdp", "--rec-sb-cdp", "recorder"]);
        assert!(before.cdp && before.rec_sb_cdp);
    }

    #[test]
    fn the_command_lines_the_recorder_app_starts_are_accepted() {
        // The list of every flag the app can add to `mkrec` is pinned by the
        // tests of `recorder_tui`; here the parser takes it.
        let record = parse(&[
            "sbrs",
            "mkrec",
            "new_recording.rs",
            "--url=https://seleniumbase.io/demo_page",
            "--headed",
            "--browser=brave",
            "--uc",
            "--ee",
            "--rec-sb-mgr",
            "--overwrite",
            "--behave",
        ]);
        assert!(matches!(
            record.command,
            Commands::Mkrec {
                overwrite: true,
                behave: true,
                ..
            }
        ));
        assert_eq!(
            record.start_page.as_deref(),
            Some("https://seleniumbase.io/demo_page")
        );
        assert!(record.headed && record.uc && record.esc_end && record.recorder.rec_sb_mgr);
        assert!(matches!(record.browser, Some(BrowserArg::Brave)));

        let cdp = parse(&[
            "sbrs",
            "mkrec",
            "x.rs",
            "--url=https://example.test",
            "--headed",
            "--browser=chrome",
            "--rec-sb-cdp",
        ]);
        assert!(cdp.rec_sb_cdp && !cdp.uc);

        let replay = parse(&[
            "sbrs",
            "replay",
            "recordings/x_rec.json",
            "--headed",
            "--browser=chrome",
            "--uc",
            "--demo",
        ]);
        assert!(matches!(replay.command, Commands::Replay { .. }));
        assert!(replay.headed && replay.uc);
    }
}

#[cfg(test)]
mod plugin_cli_tests {
    use super::*;

    fn try_parse(argv: &[&str]) -> Result<Cli, String> {
        try_parse_checked(argv)
    }

    fn parse(argv: &[&str]) -> Cli {
        try_parse(argv).expect("the arguments parse")
    }

    #[test]
    fn the_plugin_options_repeat_and_come_before_the_command() {
        let cli = parse(&[
            "sbrs",
            "--plugin",
            "shield",
            "--plugin-opt",
            r#"shield.level="strict""#,
            "--plugin",
            "block-resources",
            "--plugin-opt",
            r#"block-resources.preset="media""#,
            "open",
            "https://example.test/",
        ]);

        assert_eq!(cli.plugin, ["shield", "block-resources"]);
        assert_eq!(
            cli.plugin_opt,
            [
                r#"shield.level="strict""#,
                r#"block-resources.preset="media""#
            ]
        );
        assert!(matches!(cli.command, Commands::Open { .. }));
        assert!(parse(&["sbrs", "open", "x"]).plugin.is_empty());
        assert!(parse(&["sbrs", "open", "x"]).start_page.is_none());
        assert!(parse(&["sbrs", "open", "x"]).plugin_opt.is_empty());
    }

    /// Python's `--start-page`, under each of its names, is the page a command acts on;
    /// like the other global options it goes before the command, or after it.
    #[test]
    fn the_start_page_goes_before_the_command_under_any_of_its_names() {
        for flag in ["--start-page", "--url", "--start_page"] {
            let cli = parse(&["sbrs", flag, "https://example.test/", "get-title"]);
            assert_eq!(
                cli.start_page.as_deref(),
                Some("https://example.test/"),
                "{flag}"
            );
        }
        // The option is global, so after the command is just as good.
        let cli = parse(&["sbrs", "get-title", "--url", "https://example.test/"]);
        assert_eq!(cli.start_page.as_deref(), Some("https://example.test/"));
    }

    #[test]
    fn a_plugin_option_after_the_command_is_global_like_the_other_options() {
        let cli = try_parse(&["sbrs", "open", "x", "--plugin", "shield"]).expect("accepted");
        assert_eq!(cli.plugin, ["shield"]);
        let cli = try_parse(&["sbrs", "open", "x", "--plugin-opt", "a.b=1"]).expect("accepted");
        assert_eq!(cli.plugin_opt, ["a.b=1"]);
        assert!(
            try_parse(&["sbrs", "--plugin"]).is_err(),
            "a name is required"
        );
    }

    #[test]
    fn the_plugin_commands_are_list_and_info_with_a_name() {
        let list = parse(&["sbrs", "plugin", "list"]);
        assert!(matches!(
            list.command,
            Commands::Plugin(PluginArgs {
                command: PluginCommand::List
            })
        ));

        let info = parse(&["sbrs", "--plugin", "shield", "plugin", "info", "shield"]);
        assert_eq!(info.plugin, ["shield"]);
        let Commands::Plugin(PluginArgs {
            command: PluginCommand::Info(info),
        }) = info.command
        else {
            panic!("not plugin info");
        };
        assert_eq!(info.name, "shield");
    }

    #[test]
    fn the_plugin_commands_need_their_subcommand_and_their_name() {
        for argv in [
            &["sbrs", "plugin"][..],
            &["sbrs", "plugin", "info"],
            &["sbrs", "plugin", "remove", "shield"],
            &["sbrs", "plugin", "list", "extra"],
        ] {
            assert!(try_parse(argv).is_err(), "{argv:?} was accepted");
        }
    }

    #[test]
    fn plugin_run_takes_a_name_an_input_and_an_output() {
        let cli = parse(&[
            "sbrs",
            "plugin",
            "run",
            "crawler",
            "--input",
            "-",
            "--out",
            "found.jsonl",
            "--plugin",
            "shield",
        ]);
        // The plugin options are global, so they stay valid after the subcommand.
        assert_eq!(cli.plugin, ["shield"]);
        let Commands::Plugin(PluginArgs {
            command: PluginCommand::Run(run),
        }) = cli.command
        else {
            panic!("not plugin run");
        };
        assert_eq!(run.name, "crawler");
        assert_eq!(run.input.as_deref(), Some("-"));
        assert_eq!(run.out, Some(PathBuf::from("found.jsonl")));

        let bare = parse(&["sbrs", "plugin", "run", "batcher"]);
        let Commands::Plugin(PluginArgs {
            command: PluginCommand::Run(run),
        }) = bare.command
        else {
            panic!("not plugin run");
        };
        assert_eq!((run.input, run.out), (None, None));

        for argv in [
            &["sbrs", "plugin", "run"][..],
            &["sbrs", "plugin", "run", "a", "b"],
        ] {
            assert!(try_parse(argv).is_err(), "{argv:?} was accepted");
        }
    }

    #[test]
    fn only_the_commands_that_start_a_browser_check_and_get_the_plugins() {
        let launches = |argv: &[&str]| parse(argv).command.launches_browser();

        assert!(launches(&["sbrs", "open", "x"]));
        assert!(launches(&["sbrs", "get-title"]));
        assert!(launches(&["sbrs", "mkrec", "x.rs"]));
        assert!(launches(&["sbrs", "run-scenario", "--file", "s.json"]));
        for argv in [
            &["sbrs", "doctor"][..],
            &["sbrs", "version"],
            &["sbrs", "devtools"],
            &["sbrs", "test"],
            &["sbrs", "plugin", "list"],
            &["sbrs", "completions", "bash"],
            &["sbrs", "install", "chromedriver"],
            &["sbrs", "mkfile", "tests/a.rs"],
        ] {
            assert!(!launches(argv), "{argv:?} starts no BaseCase browser");
        }
    }
}

#[cfg(test)]
mod strict_cli_tests {
    use super::*;

    fn refused(argv: &[&str]) -> String {
        try_parse_checked(argv).expect_err(&format!("{argv:?} was accepted"))
    }

    /// The result of [`strict::check`] alone, for the command line without the program name.
    fn checked(words: &[&str]) -> Result<(), String> {
        let words: Vec<&std::ffi::OsStr> = words.iter().map(std::ffi::OsStr::new).collect();
        strict::check(Cli::spec(), Cli::command(), &words)
            .map_err(|error| Cli::render_failure(&words, &error))
    }

    #[test]
    fn a_value_written_after_a_switch_is_refused_whatever_it_is() {
        for argv in [
            &["sbrs", "--headless=false", "open", "x"][..],
            &["sbrs", "--headless=true", "open", "x"],
            &["sbrs", "--headed=true", "open", "x"],
            &["sbrs", "--headless=", "open", "x"],
            &["sbrs", "--headless=0", "open", "x"],
            &["sbrs", "--cdp=yes", "open", "x"],
            &["sbrs", "--uc=1", "version"],
            &["sbrs", "devtools", "--json=true"],
            &["sbrs", "mkdir", "d", "--force=false"],
        ] {
            let error = refused(argv);
            assert!(error.contains("takes no value"), "{argv:?}: {error}");
        }
        // A global switch after the command is no flag of the command: refused either way.
        refused(&["sbrs", "open", "x", "--headless=true"]);
    }

    #[test]
    fn a_switch_without_a_value_is_on() {
        let cli = try_parse_checked(&["sbrs", "--headless", "--uc", "open", "x"]).expect("parses");
        assert!(cli.headless && cli.uc);
    }

    #[test]
    fn a_scalar_flag_given_twice_to_a_subcommand_is_refused() {
        for argv in [
            &["sbrs", "get-text", "--css", "a", "--css", "b"][..],
            &[
                "sbrs",
                "wait-for-text",
                "--css",
                "a",
                "--text",
                "t",
                "--text",
                "u",
            ],
            &["sbrs", "mkfile", "f.rs", "--url", "a", "--url", "b"],
            &["sbrs", "mkdir", "d", "--force", "--force"],
            &["sbrs", "mkdir", "d", "-b", "--basic"],
            &["sbrs", "import-python", "f.py", "-o", "a", "--output", "b"],
            &["sbrs", "devtools", "--port", "1", "--port", "2"],
            &[
                "sbrs",
                "download-file",
                "u",
                "--timeout",
                "1",
                "--timeout",
                "2",
            ],
        ] {
            let error = refused(argv);
            assert!(error.contains("multiple times"), "{argv:?}: {error}");
        }
    }

    /// The check is for the commands whose own declaration lets the last value win, which
    /// is every inline variant of `Commands`. Whatever flags they have now or get later,
    /// each one that is not a collection or a counter is refused when it is given twice.
    #[test]
    fn every_scalar_flag_of_every_command_is_refused_when_given_twice() {
        use usage::spec::CommandMeta;

        fn walk(
            meta: &CommandMeta<'_>,
            path: &mut Vec<String>,
            refused: &mut usize,
            failures: &mut Vec<String>,
        ) {
            // The root and the `Args` structs refuse a repeat themselves.
            if !path.is_empty() && meta.args_override_self {
                for flag_meta in meta.flags {
                    let flag = flag_meta.flag;
                    if flag_meta.builtin
                        || flag.action != usage::ArgAction::Set
                        || flag_meta.repeatable
                        || flag_meta.count
                    {
                        continue;
                    }
                    let spelling = match (flag.longs.first(), flag.shorts.first()) {
                        (Some(long), _) => format!("--{long}"),
                        (None, Some(short)) => format!("-{}", char::from(*short)),
                        (None, None) => continue,
                    };
                    let mut words: Vec<&str> = path.iter().map(String::as_str).collect();
                    for _ in 0..2 {
                        words.push(&spelling);
                        if flag.takes_value {
                            words.push("1");
                        }
                    }
                    match checked(&words) {
                        Err(error) if error.contains("multiple times") => *refused += 1,
                        other => failures.push(format!("{words:?}: {other:?}")),
                    }
                }
            }
            for sub in meta.subcommands {
                path.push(sub.cmd.name.to_owned());
                walk(sub, path, refused, failures);
                path.pop();
            }
        }

        let (mut refused, mut failures) = (0, Vec::new());
        walk(
            Cli::spec().root,
            &mut Vec::new(),
            &mut refused,
            &mut failures,
        );
        assert!(refused > 40, "the walk found only {refused} flags");
        assert!(failures.is_empty(), "accepted twice: {failures:#?}");
    }

    #[test]
    fn a_flag_that_may_be_repeated_may_be_given_twice() {
        for argv in [
            &["sbrs", "deferred", "--assert", "a", "--assert", "b"][..],
            &["sbrs", "test", "--features", "a", "--features", "b"],
            &["sbrs", "--plugin", "a", "--plugin", "b", "open", "x"],
        ] {
            if let Err(error) = try_parse_checked(argv) {
                panic!("{argv:?}: {error}");
            }
        }
    }

    #[test]
    fn a_global_flag_given_before_and_after_the_command_is_a_repeat() {
        for argv in [
            &["sbrs", "--headless", "get-title", "--headless"][..],
            &["sbrs", "--proxy", "a:1", "get-title", "--proxy", "b:2"],
            &["sbrs", "--proxy", "a:1", "devtools", "--proxy", "b:2"],
            &["sbrs", "--url", "a", "get-title", "--start-page", "b"],
            &["sbrs", "--mobile", "get-title", "--mobile"],
            &[
                "sbrs",
                "--save-screenshot",
                "get-title",
                "--save-screenshot",
            ],
            &["sbrs", "get-title", "--headless", "--headless"],
        ] {
            let error = refused(argv);
            assert!(error.contains("multiple times"), "{argv:?}: {error}");
        }
    }

    #[test]
    fn a_global_flag_may_come_before_or_after_the_command() {
        let before = try_parse_checked(&["sbrs", "--proxy", "a:1", "devtools"]).expect("parses");
        assert_eq!(before.proxy.as_deref(), Some("a:1"));
        let after = try_parse_checked(&["sbrs", "devtools", "--proxy", "b:2"]).expect("parses");
        assert_eq!(after.proxy.as_deref(), Some("b:2"));
        // A flag that may be repeated is no repeat when it is written on both sides.
        for argv in [
            &["sbrs", "--plugin", "a", "open", "x", "--plugin", "b"][..],
            &["sbrs", "--headless", "open", "x", "--uc", "--plugin", "a"],
        ] {
            if let Err(error) = try_parse_checked(argv) {
                panic!("{argv:?}: {error}");
            }
        }
    }

    #[test]
    fn help_with_a_path_that_names_no_command_is_refused() {
        for words in [
            &["help", "nosuch"][..],
            &["help", "plugin", "nosuch"],
            &["help", "nosuch", "plugin"],
            &["help", "plugin", "info", "extra"],
            &["help", "nosuch", "--help"],
            // `mkdir` has no subcommands, so no `help` of its own.
            &["help", "mkdir", "help"],
            &["--headless", "help", "nosuch"],
        ] {
            let error = checked(words).expect_err(&format!("{words:?} was accepted"));
            assert!(
                error.contains("unrecognized subcommand"),
                "{words:?}: {error}"
            );
        }
    }

    #[test]
    fn help_with_a_path_of_commands_is_left_to_the_parser() {
        for words in [
            &["help"][..],
            &["help", "mkdir"],
            &["help", "new"],
            &["help", "plugin"],
            &["help", "plugin", "info"],
            &["--headless", "help", "plugin"],
            // The page of a command, asked for as the page of the page.
            &["help", "--help"],
            &["help", "help"],
            &["help", "plugin", "help"],
            &["help", "mkdir", "--help"],
            &["help", "plugin", "info", "-h"],
        ] {
            assert_eq!(checked(words), Ok(()), "{words:?}");
        }
    }

    #[test]
    fn a_value_written_after_help_or_the_version_is_refused() {
        for words in [
            &["--help=false"][..],
            &["--help="],
            &["mkdir", "--help=1"],
            &["--version=1"],
            &["--headless", "--version=true"],
        ] {
            let error = checked(words).expect_err(&format!("{words:?} was accepted"));
            assert!(error.contains("takes no value"), "{words:?}: {error}");
        }
        // The first request answers: nothing after it is read.
        for words in [&["-h", "--help=1"][..], &["--help", "--version=1"]] {
            assert_eq!(checked(words), Ok(()), "{words:?}");
        }
    }

    #[test]
    fn a_request_for_help_or_the_version_ends_the_check() {
        for words in [
            &["mkdir", "--help", "--force", "--force"][..],
            &["mkdir", "--help", "--force=2"],
            &["--version", "--headless=true"],
            &["mkdir", "-h", "--force=1"],
        ] {
            assert_eq!(checked(words), Ok(()), "{words:?}");
        }
    }

    #[test]
    fn what_the_check_does_not_cover_is_left_to_the_parser() {
        for words in [
            &[][..],
            &["nosuch"],
            &["--nosuch"],
            &["open"],
            &["__complete_word__", "--shell", "bash"],
        ] {
            assert_eq!(checked(words), Ok(()), "{words:?}");
        }
    }
}

#[cfg(test)]
mod usage_spec_tests {
    use super::*;

    /// The check in `strict` holds every command to the rule of the root.
    #[test]
    fn the_root_refuses_a_flag_given_twice() {
        assert!(!Cli::spec().root.args_override_self);
    }

    /// The check in `strict` can see a value written after a switch only when the switch is
    /// declared `bool_value`: without it the parser drops the value before anything can look
    /// at it.
    #[test]
    fn every_switch_is_declared_bool_value() {
        /// A member of an `ArgGroup` (the language and syntax flags of `mkfile`) cannot be
        /// declared `bool_value` in usage 6.12, so it is not held to the rule.
        fn group_member(flag: &usage::Flag<'_>) -> bool {
            flag.longs.iter().any(|long| {
                args::language::LONGS.contains(long)
                    || args::generators::SYNTAX_LONGS.contains(long)
            })
        }

        fn walk(command: &usage::Command<'_>, path: &str, undeclared: &mut Vec<String>) -> usize {
            let mut switches = 0;
            for flag in command.flags {
                if flag.takes_value || flag.action != usage::ArgAction::Set || group_member(flag) {
                    continue;
                }
                switches += 1;
                if !flag.bool_value {
                    undeclared.push(format!(
                        "{path} --{}",
                        flag.longs.first().unwrap_or(&flag.name)
                    ));
                }
            }
            for sub in command.subcommands {
                switches += walk(sub, &format!("{path} {}", sub.name), undeclared);
            }
            switches
        }
        let mut undeclared = Vec::new();
        let switches = walk(Cli::command(), "sbrs", &mut undeclared);
        assert!(switches > 10, "the walk found only {switches} switches");
        assert!(
            undeclared.is_empty(),
            "declare as `bool_value`: {undeclared:?}"
        );
    }

    /// The spec states some defaults as literals, because a constant cannot be written
    /// into it; they have to be the constants the code uses.
    #[test]
    fn the_defaults_written_as_literals_are_the_constants_the_code_uses() {
        assert_eq!(DEFAULT_DEVTOOLS_PORT, 9222);
        assert_eq!(prompt::DEFAULT_FORMAT, "{browser} {major} {driver}");
    }

    /// The Fig spec is written from the same tables as the parser: the commands, their
    /// aliases, the flags with their short and long spellings, and the words a flag takes.
    #[test]
    fn the_fig_spec_lists_the_commands_flags_and_aliases_of_the_command_line() {
        let fig = fig::spec(Cli::spec());
        assert!(
            fig.starts_with("const completionSpec: Fig.Spec = {\n"),
            "{fig}"
        );
        assert!(fig.ends_with("export default completionSpec;\n"), "{fig}");
        for expected in [
            r#"name: ["install", "get"],"#,
            r#"name: ["mkfile", "new"],"#,
            r#"name: ["-a", "--agent"],"#,
            r#"name: ["--reuse-session", "--reuse_session", "--rs"],"#,
            r#"name: "--webdriver","#,
            r#"suggestions: ["chrome", "chromium", "edge","#,
            r#"isRequired: true,"#,
            r#"template: "filepaths","#,
            r#"name: ["-h", "--help"],"#,
            r#"name: ["-V", "--version"],"#,
        ] {
            assert!(fig.contains(expected), "{expected} in:\n{fig}");
        }
    }

    /// Every browser word the command line accepts reaches `SB_BROWSER` as it is spelled.
    #[test]
    fn a_browser_is_named_by_the_word_that_chooses_it() {
        use usage::spec::ValueEnum as _;
        for word in BrowserArg::CHOICES {
            let browser = BrowserArg::from_choice(word).expect("a listed word names a browser");
            assert_eq!(browser.name(), *word);
        }
        assert_eq!(BrowserArg::CHOICES.len(), 11);
    }

    /// The repository file `relative` against what the declarations generate. With
    /// `SB_UPDATE_COMPLETIONS` set the file is written instead of compared.
    #[cfg(not(feature = "turso"))]
    fn assert_generated(relative: &str, generated: &str) {
        // The files live at the repository root, two folders above this crate.
        let path = format!("{}/../../{relative}", env!("CARGO_MANIFEST_DIR"));
        if std::env::var_os("SB_UPDATE_COMPLETIONS").is_some() {
            std::fs::write(&path, generated).expect("write the generated file");
        } else {
            let committed = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                committed == generated,
                "{relative} is stale: run SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec"
            );
        }
    }

    // The generated files describe the default build: `sbrs report` exists only with the
    // `turso` feature, so the three tests below skip a build that has it.

    /// `completions/sbrs.usage.kdl` is generated from the declarations. After
    /// changing the CLI, write it again (and the two files below) with
    /// `SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`.
    #[cfg(not(feature = "turso"))]
    #[test]
    fn the_committed_usage_spec_matches_the_cli() {
        // `sbrs completions usage` prints this file, which the binary embeds.
        assert_generated("completions/sbrs.usage.kdl", &Cli::to_kdl());
    }

    /// `docs/completions/fig-sbrs.ts` is `sbrs completions fig`, a Fig spec prepared for
    /// the withfig/autocomplete repository (its README explains how to submit it).
    #[cfg(not(feature = "turso"))]
    #[test]
    fn the_committed_fig_spec_matches_the_cli() {
        assert_generated("docs/completions/fig-sbrs.ts", &fig::spec(Cli::spec()));
    }

    /// `docs/help/cli_reference.md` is the usage spec rendered as a page of the Docs7
    /// site (`reference.rs`), so the reference lists what the parser accepts.
    #[cfg(not(feature = "turso"))]
    #[test]
    fn the_committed_cli_reference_matches_the_cli() {
        let spec: usage_lib::Spec = Cli::to_kdl().parse().expect("the usage spec parses");
        assert_generated("docs/help/cli_reference.md", &reference::render(&spec));
    }
}

#[cfg(test)]
mod page_cli_tests {
    use super::*;

    fn parse(argv: &[&str]) -> Cli {
        try_parse_checked(argv).expect("the arguments parse")
    }

    fn refused(argv: &[&str]) -> String {
        try_parse_checked(argv).expect_err(&format!("{argv:?} was accepted"))
    }

    fn command(argv: &[&str]) -> Commands {
        parse(argv).command
    }

    /// `sbrs patch-chrome` keeps the copy in the folder of `--cache-dir`, else of
    /// `SB_PATCH_CACHE_DIR`, else of the user's cache. (The command itself needs a real
    /// Chrome binary, which a test cannot give it.)
    #[test]
    fn patch_chrome_takes_its_folder_from_the_flag_then_the_variable() {
        let flag = Some("from-the-flag".to_owned());
        let variable = Some("from-the-variable".to_owned());

        assert_eq!(
            patch_cache_dir(flag.clone(), variable.clone()),
            PathBuf::from("from-the-flag")
        );
        assert_eq!(
            patch_cache_dir(None, variable),
            PathBuf::from("from-the-variable")
        );
        assert!(patch_cache_dir(None, None).ends_with("sb-chrome-patches"));
    }

    /// Python's `--start-page`, under each of its names, is the page a command acts on; it
    /// goes before the command or after it, like the other global options.
    #[test]
    fn the_start_page_goes_before_or_after_the_command_under_any_of_its_names() {
        for flag in ["--start-page", "--url", "--start_page"] {
            let page = "https://example.test/";
            let before = parse(&["sbrs", flag, page, "get-title"]);
            assert_eq!(before.start_page.as_deref(), Some(page), "{flag} before");
            let after = parse(&["sbrs", "get-title", flag, page]);
            assert_eq!(after.start_page.as_deref(), Some(page), "{flag} after");
            let joined = format!("{flag}={page}");
            let equals = parse(&["sbrs", "get-text", "--css", "h1", &joined]);
            assert_eq!(equals.start_page.as_deref(), Some(page), "{joined}");
        }
        assert!(parse(&["sbrs", "get-title"]).start_page.is_none());
    }

    #[test]
    fn the_save_screenshot_switch_has_both_spellings_and_goes_anywhere() {
        for words in [
            &["sbrs", "--save-screenshot", "get-title"][..],
            &["sbrs", "--save_screenshot", "get-title"],
            &["sbrs", "get-title", "--save-screenshot"],
        ] {
            assert!(parse(words).save_screenshot, "{words:?}");
        }
        assert!(!parse(&["sbrs", "get-title"]).save_screenshot);
    }

    #[test]
    fn a_global_option_may_follow_the_command() {
        let cli = parse(&[
            "sbrs",
            "get-title",
            "--headless",
            "--browser",
            "chromium",
            "--proxy",
            "p:1",
        ]);
        assert!(cli.headless);
        assert_eq!(cli.browser, Some(BrowserArg::Chromium));
        assert_eq!(cli.proxy.as_deref(), Some("p:1"));
        let cli = parse(&["sbrs", "open", "x", "--plugin", "shield", "--plugin", "b"]);
        assert_eq!(cli.plugin, ["shield", "b"]);
    }

    /// Every option of the root is a browser or session option for the command that
    /// follows it, so every one is accepted after the command too.
    #[test]
    fn every_root_option_is_global() {
        let spec: usage_lib::Spec = Cli::to_kdl().parse().expect("the usage spec parses");
        let mut checked = 0;
        for flag in &spec.cmd.flags {
            if flag
                .long
                .iter()
                .any(|long| long == "help" || long == "version")
            {
                continue;
            }
            assert!(flag.global, "--{} is not global", flag.long.join("/"));
            checked += 1;
        }
        assert!(checked > 20, "only {checked} root options found");
    }

    #[test]
    fn the_page_commands_that_a_fresh_browser_makes_useless_are_gone() {
        for name in [
            "go-back",
            "go-forward",
            "refresh",
            "switch-to-window",
            "switch-to-frame",
            "switch-to-default-content",
            "switch-to-parent-frame",
            "switch-to-newest-window",
            "switch-to-default-window",
            "open-new-window",
            "open-new-tab",
            "close-window",
            "set-window-position",
            "get-active-element-css",
            "throttle3g",
        ] {
            let error = refused(&["sbrs", name]);
            assert!(error.contains("unrecognized subcommand"), "{name}: {error}");
        }
        // `replay` runs a recording in a browser, so it takes a page of its own to act on.
        assert!(!parse(&["sbrs", "replay", "a.json"]).command.opens_no_page());
    }

    #[test]
    fn the_commands_that_act_on_a_page_need_one() {
        for argv in [
            &["sbrs", "get-text", "--css", "h1"][..],
            &["sbrs", "get-title"],
            &["sbrs", "screenshot"],
            &["sbrs", "save-source"],
            &["sbrs", "hover", "--css", "a"],
            &["sbrs", "accept-alert"],
            &["sbrs", "clear-cookies"],
            &["sbrs", "assert-title", "--text", "x"],
            &["sbrs", "uc-click", "--css", "a"],
        ] {
            assert!(command(argv).needs_page(), "{argv:?}");
        }
        for argv in [
            &["sbrs", "cdp", "--cmd", "Browser.getVersion"][..],
            &["sbrs", "open", "x"],
            &["sbrs", "smoke", "x"],
            &["sbrs", "version"],
            &["sbrs", "mkdir", "d"],
            &["sbrs", "run-scenario", "--file", "s.json"],
            &["sbrs", "test"],
        ] {
            assert!(!command(argv).needs_page(), "{argv:?}");
        }
    }

    #[test]
    fn only_the_commands_that_change_the_page_say_where_they_ended() {
        for argv in [
            &["sbrs", "click-partial-link-text", "--text", "a"][..],
            &["sbrs", "hover", "--css", "a"],
            &["sbrs", "human-click", "--css", "a"],
            &["sbrs", "add-text", "--css", "a", "--text", "b"],
        ] {
            assert!(command(argv).acts_on_page(), "{argv:?}");
        }
        for argv in [
            &["sbrs", "get-text", "--css", "h1"][..],
            &["sbrs", "screenshot"],
            &["sbrs", "assert-title", "--text", "x"],
            &["sbrs", "open", "x"],
        ] {
            assert!(!command(argv).acts_on_page(), "{argv:?}");
        }
    }

    #[test]
    fn a_command_is_named_as_it_is_typed() {
        for (argv, name) in [
            (&["sbrs", "get-text", "--css", "h1"][..], "get-text"),
            (&["sbrs", "uc-click", "--css", "a"], "uc-click"),
            (
                &["sbrs", "cdp-click-element", "--css", "a"],
                "cdp-click-element",
            ),
            (
                &["sbrs", "wait-for-ready-state-complete"],
                "wait-for-ready-state-complete",
            ),
            (&["sbrs", "cache-clear"], "cache-clear"),
        ] {
            assert_eq!(command_name(&command(argv)), name);
        }
    }

    #[test]
    fn a_page_command_without_a_page_is_refused_before_a_browser_starts() {
        let get_text = command(&["sbrs", "get-text", "--css", "h1"]);
        let error = check_page_arguments(&get_text, false, None, None).expect_err("no page");
        let message = error.to_string();
        assert!(message.contains("get-text acts on a page"), "{message}");
        assert!(
            message.contains("every sbrs command starts a fresh browser"),
            "{message}"
        );
        assert!(
            message.contains("sbrs --url https://example.com get-text"),
            "{message}"
        );

        // A start page from the command line, the environment or the settings file is a page.
        check_page_arguments(&get_text, true, Some("https://example.test/"), None)
            .expect("the flag");
        check_page_arguments(&get_text, false, Some("https://example.test/"), None)
            .expect("the environment or the file");
        // `cdp` needs none, and neither does a command that has a URL of its own.
        let cdp = command(&["sbrs", "cdp", "--cmd", "Browser.getVersion"]);
        check_page_arguments(&cdp, false, None, None).expect("cdp");
        let open = command(&["sbrs", "open", "https://example.test/"]);
        check_page_arguments(&open, false, None, None).expect("open");
    }

    #[test]
    fn a_command_that_changes_the_profile_needs_a_profile_to_keep() {
        for argv in [
            &["sbrs", "clear-cookies"][..],
            &["sbrs", "cache-clear"],
            &["sbrs", "clear-local-storage"],
            &["sbrs", "load-cookies", "--file", "c.json"],
            &[
                "sbrs",
                "set-local-storage-item",
                "--key",
                "k",
                "--value",
                "v",
            ],
            &["sbrs", "remove-local-storage-item", "--key", "k"],
        ] {
            let profile = command(argv);
            let page = Some("https://example.test/");
            let error = check_page_arguments(&profile, true, page, None).expect_err("no profile");
            let message = error.to_string();
            assert!(
                message.contains("changes the profile"),
                "{argv:?}: {message}"
            );
            assert!(
                message.contains("add --user-data-dir DIR"),
                "{argv:?}: {message}"
            );
            check_page_arguments(&profile, true, page, Some("/tmp/profile"))
                .unwrap_or_else(|error| panic!("{argv:?}: {error}"));
        }
    }

    #[test]
    fn a_command_with_a_url_of_its_own_refuses_a_start_page_from_the_command_line() {
        for argv in [
            &["sbrs", "open", "https://example.test/"][..],
            &["sbrs", "smoke", "https://example.test/"],
            &["sbrs", "download-file", "https://example.test/f"],
        ] {
            let own = command(argv);
            let error = check_page_arguments(&own, true, Some("https://other.test/"), None)
                .expect_err("two pages");
            let message = error.to_string();
            assert!(message.contains("opens its own URL"), "{argv:?}: {message}");
            assert!(
                message.contains("leave out --start-page"),
                "{argv:?}: {message}"
            );
            // A page from the environment or the settings file is only a default.
            check_page_arguments(&own, false, Some("https://other.test/"), None)
                .unwrap_or_else(|error| panic!("{argv:?}: {error}"));
            assert!(own.opens_own_url(), "{argv:?}");
        }
    }

    #[test]
    fn a_start_page_gets_a_scheme() {
        assert_eq!(
            start_page_url("https://example.test/a").unwrap(),
            "https://example.test/a"
        );
        assert_eq!(
            start_page_url("example.test").unwrap(),
            "https://example.test"
        );
        assert_eq!(start_page_url("about:blank").unwrap(), "about:blank");
        assert_eq!(
            start_page_url("file:///tmp/a.html").unwrap(),
            "file:///tmp/a.html"
        );
        let here = std::env::current_dir().unwrap();
        let local = start_page_url(&here.display().to_string()).unwrap();
        assert!(local.starts_with("file://"), "{local}");
    }
}
