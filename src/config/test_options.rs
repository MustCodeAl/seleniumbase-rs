//! The option table: every Python SeleniumBase option (`pytest` options and
//! `SB(...)` keywords) on one line, with the `sbrs test` flags that spell it,
//! the `SB_*` variable that carries it and the [`Settings`] field it lands in.
//!
//! The table is the single source of truth for four consumers:
//!
//! * [`TEST_OPTIONS`], the list `sbrs test` builds its flags from;
//! * the environment reader that [`Settings::load`] runs, which replaces a
//!   hand-written `if let Some(v) = var("SB_..")` chain;
//! * [`Settings::to_browser_config`] and [`Settings::to_launch_options`],
//!   which turn the fields into a browser start-up;
//! * `tests/test_options_table.rs`, which fails when `parity/api.toml` has an
//!   option the table does not spell.
//!
//! A flag written with `_` is the same flag as the one written with `-`
//! (`--no_sandbox` is `--no-sandbox`), and the table lists the `-` form once.
//! The names of `SB(...)` keywords that differ from the flag are the `sb_names`
//! of a row (`is_mobile` for `--mobile`).
//!
//! # Precedence
//!
//! An `sbrs test` flag wins over an `SB_*` variable already in the
//! environment, which wins over the settings file, which wins over the
//! default. A variable that is empty or blank counts as unset.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::config::test_options::{find_option, OptionKind};
//!
//! let dark = find_option("--dark").ok_or("no such option")?;
//! assert_eq!(dark.env, "SB_DARK_MODE");
//! assert_eq!(dark.kind, OptionKind::Switch);
//! assert_eq!(find_option("--dark_mode").map(|row| row.long), Some("dark-mode"));
//! # Ok::<(), &str>(())
//! ```

use std::path::PathBuf;

use crate::config::settings::{Settings, SettingsProblem};
use crate::config::test_settings::TestData;
use crate::error::SeleniumBaseError;

/// What an option is and how it is read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionKind {
    /// A flag that takes no value: `true`/`false` in the variable. Every
    /// switch of `sbrs` accepts `--flag=false` (`bool_value`).
    Switch,
    /// A flag that takes a value of the named kind (`Text`, `Path`, `Seconds`,
    /// ...); the name is the value's placeholder in the help.
    Value(&'static str),
    /// A setting with a variable but no flag of its own (`SB_TIMEOUT`).
    Env(&'static str),
    /// A switch that is shorthand for a fixed value of the row's variable:
    /// `--firefox` is `SB_BROWSER=firefox`. The environment reader skips it.
    Sets(&'static str),
    /// A value with its own check (`--env`, `--variables`).
    Custom(&'static str),
    /// Accepted for the sake of the spelling and ignored, for the reason given.
    NoOp(&'static str),
    /// An action of `sbrs test` itself, with no variable (`--all-scripts`).
    Cli(&'static str),
    /// An option of Python's `SB(...)` that has no flag, for the reason given.
    NotAFlag(&'static str),
}

impl OptionKind {
    /// Whether `sbrs test` has a flag for this kind of option.
    #[must_use]
    pub const fn is_flag(self) -> bool {
        !matches!(self, Self::Env(_) | Self::NotAFlag(_))
    }

    /// Whether the environment reader reads the row's variable.
    #[must_use]
    pub const fn reads_env(self) -> bool {
        matches!(
            self,
            Self::Switch | Self::Value(_) | Self::Env(_) | Self::Custom(_)
        )
    }
}

/// One row of the table. See the [module documentation](self).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptionSpec {
    /// The path of the field in [`Settings`] (`dark_mode`, `test.pdb`,
    /// `test.data.env`), or `none` when the row has no field.
    pub field: &'static str,
    /// The canonical flag, without the dashes: `dark-mode`.
    pub long: &'static str,
    /// The other flags that mean the same, without the dashes.
    pub aliases: &'static [&'static str],
    /// The names Python's `SB(...)` knows the option by, when they differ from
    /// the flags.
    pub sb_names: &'static [&'static str],
    /// The environment variable, or an empty string for none.
    pub env: &'static str,
    /// What the option is.
    pub kind: OptionKind,
    /// A one-line help.
    pub help: &'static str,
}

fn normalise(spelling: &str) -> String {
    spelling
        .trim_start_matches('-')
        .replace('_', "-")
        .to_ascii_lowercase()
}

impl OptionSpec {
    /// The flags of the row, the canonical one first, without the dashes.
    pub fn flags(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(self.long).chain(self.aliases.iter().copied())
    }

    /// Whether the row has a field in [`Settings`].
    #[must_use]
    pub fn has_field(&self) -> bool {
        self.field != "none"
    }

    /// Whether `spelling` is a flag of the row (`--dark`, `dark_mode`, ...).
    #[must_use]
    pub fn matches_flag(&self, spelling: &str) -> bool {
        let wanted = normalise(spelling);
        self.flags().any(|flag| normalise(flag) == wanted)
    }

    /// Whether `name` is a keyword of Python's `SB(...)` for this option: the
    /// `sb_names`, or any of the flags.
    #[must_use]
    pub fn matches_sb_name(&self, name: &str) -> bool {
        let wanted = normalise(name);
        self.matches_flag(name) || self.sb_names.iter().any(|sb| normalise(sb) == wanted)
    }
}

/// The row that spells `flag` (`--dark`, `dark_mode`, `-dark`), if any.
#[must_use]
pub fn find_option(flag: &str) -> Option<&'static OptionSpec> {
    TEST_OPTIONS.iter().find(|row| row.matches_flag(flag))
}

/// How an option's variable text is written into a [`Settings`] field.
pub(crate) trait OptionValue {
    /// Reads `text`, the value of the variable `name`.
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError>;
}

impl OptionValue for bool {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = crate::config::settings::parse_bool(text)?;
        Ok(())
    }
}

impl OptionValue for String {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        text.clone_into(self);
        Ok(())
    }
}

impl OptionValue for Option<String> {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = Some(text.to_owned());
        Ok(())
    }
}

impl OptionValue for Option<PathBuf> {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = Some(PathBuf::from(text));
        Ok(())
    }
}

fn parse_number<T>(name: &str, text: &str) -> Result<T, SeleniumBaseError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    text.parse()
        .map_err(|error| SeleniumBaseError::InvalidConfig(format!("{name}: {error}")))
}

fn parse_seconds(name: &str, text: &str) -> Result<f64, SeleniumBaseError> {
    match text.trim().parse::<f64>() {
        Ok(seconds) if seconds.is_finite() && seconds >= 0.0 => Ok(seconds),
        _ => Err(SeleniumBaseError::InvalidConfig(format!(
            "{name}: expected a number of seconds, 0 or more, not '{text}'"
        ))),
    }
}

macro_rules! number_values {
    ($($t:ty),+ $(,)?) => {$(
        impl OptionValue for Option<$t> {
            fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
                *self = Some(parse_number(name, text.trim())?);
                Ok(())
            }
        }
    )+};
}

number_values!(u16, u32, u64, usize);

impl OptionValue for u32 {
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = parse_number(name, text.trim())?;
        Ok(())
    }
}

impl OptionValue for f64 {
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = parse_seconds(name, text)?;
        Ok(())
    }
}

impl OptionValue for Option<f64> {
    fn assign(&mut self, name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = Some(parse_seconds(name, text)?);
        Ok(())
    }
}

/// `--env`: one of the 24 environments, in lower case.
fn set_env(settings: &mut Settings, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
    settings.test.data.env = Some(TestData::parse_env(text)?);
    Ok(())
}

/// `--variables`: a JSON object or a Python dictionary literal.
fn set_variables(
    settings: &mut Settings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    settings.test.data.variables = TestData::parse_variables(text)?;
    Ok(())
}

/// Reads the variable `env` and, when it is set, gives its text to `assign`.
fn read(
    var: &dyn Fn(&str) -> Option<String>,
    problems: &mut Vec<SettingsProblem>,
    env: &str,
    assign: impl FnOnce(&str, &str) -> Result<(), SeleniumBaseError>,
) {
    if let Some(text) = var(env) {
        if let Err(error) = assign(env, &text) {
            problems.push(SettingsProblem {
                source: env.to_owned(),
                error,
            });
        }
    }
}

macro_rules! kind_of {
    (switch) => {
        OptionKind::Switch
    };
    (value($t:ident)) => {
        OptionKind::Value(stringify!($t))
    };
    (env($t:ident)) => {
        OptionKind::Env(stringify!($t))
    };
    (sets($v:literal)) => {
        OptionKind::Sets($v)
    };
    (custom($t:ident, $f:path)) => {
        OptionKind::Custom(stringify!($t))
    };
    (noop($why:literal)) => {
        OptionKind::NoOp($why)
    };
    (cli($why:literal)) => {
        OptionKind::Cli($why)
    };
    (na($why:literal)) => {
        OptionKind::NotAFlag($why)
    };
}

macro_rules! apply_row {
    ($s:ident, $var:ident, $problems:ident, $env:literal, [switch], $($p:ident).+) => {
        read($var, $problems, $env, |name, text| {
            OptionValue::assign(&mut $s.$($p).+, name, text)
        })
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [value($t:ident)], $($p:ident).+) => {
        read($var, $problems, $env, |name, text| {
            OptionValue::assign(&mut $s.$($p).+, name, text)
        })
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [env($t:ident)], $($p:ident).+) => {
        read($var, $problems, $env, |name, text| {
            OptionValue::assign(&mut $s.$($p).+, name, text)
        })
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [custom($t:ident, $f:path)], $($p:ident).+) => {
        read($var, $problems, $env, |name, text| $f(&mut *$s, name, text))
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [sets($v:literal)], $($p:ident).+) => {};
    ($s:ident, $var:ident, $problems:ident, $env:literal, [noop($why:literal)], $($p:ident).+) => {};
    ($s:ident, $var:ident, $problems:ident, $env:literal, [cli($why:literal)], $($p:ident).+) => {};
    ($s:ident, $var:ident, $problems:ident, $env:literal, [na($why:literal)], $($p:ident).+) => {};
}

macro_rules! option_table {
    ($(
        (
            $($path:ident).+,
            $long:literal,
            [$($alias:literal),* $(,)?],
            [$($sb:literal),* $(,)?],
            $env:literal,
            [$($kind:tt)+],
            $help:literal
        )
    ),+ $(,)?) => {
        /// Every option of the table; see the [module documentation](self).
        pub static TEST_OPTIONS: &[OptionSpec] = &[$(
            OptionSpec {
                field: stringify!($($path).+),
                long: $long,
                aliases: &[$($alias),*],
                sb_names: &[$($sb),*],
                env: $env,
                kind: kind_of!($($kind)+),
                help: $help,
            }
        ),+];

        /// Writes every option whose variable `var` reports into `settings`;
        /// a value that does not parse goes to `problems`, in table order.
        #[allow(clippy::too_many_lines)]
        pub(crate) fn apply_env(
            settings: &mut Settings,
            var: &dyn Fn(&str) -> Option<String>,
            problems: &mut Vec<SettingsProblem>,
        ) {
            $(
                apply_row!(settings, var, problems, $env, [$($kind)+], $($path).+);
            )+
        }
    };
}

/// Hands every row of the table to the macro `$callback`, so that another
/// crate can build its own items (the `sbrs test` flags) from the same rows.
///
/// A row is `(field, "long", ["alias"..], ["sb name"..], "SB_VAR", [kind],
/// "help")`.
#[macro_export]
#[doc(hidden)]
macro_rules! sb_test_options {
    ($callback:ident) => {
        $callback! {
            // Settings that exist since the first release, in the order their
            // variables are read.
            (browser, "browser", [], [], "SB_BROWSER", [value(Browser)], "The browser to run: chrome, edge, firefox, brave, ..."),
            (headless, "headless", ["headless2"], ["headless", "headless2"], "SB_HEADLESS", [switch], "Run without a visible window (Chrome's new headless mode)"),
            (timeout_seconds, "timeout", [], [], "SB_TIMEOUT", [env(Seconds)], "The default wait, in whole seconds"),
            (webdriver_url, "webdriver-url", [], [], "SB_WEBDRIVER_URL", [env(Url)], "The WebDriver server to use"),
            (chrome_bin, "binary-location", ["bl"], ["binary_location"], "SB_CHROME_BIN", [value(Path)], "The browser binary to run"),
            (remote_webdriver_timeout_seconds, "remote-webdriver-timeout", [], [], "SB_REMOTE_WEBDRIVER_TIMEOUT", [env(Seconds)], "Client timeout for each request to a Remote WebDriver server"),
            (proxy, "proxy", ["proxy-server", "proxy-string"], [], "SB_PROXY", [value(Text)], "The proxy server: HOST:PORT or USER:PASS@HOST:PORT"),
            (window_width, "window-width", [], [], "SB_WINDOW_WIDTH", [env(Pixels)], "The browser window's width in pixels"),
            (window_height, "window-height", [], [], "SB_WINDOW_HEIGHT", [env(Pixels)], "The browser window's height in pixels"),
            (user_data_dir, "user-data-dir", [], [], "SB_USER_DATA_DIR", [value(Path)], "The browser profile folder"),
            (extension_dir, "extension-dir", [], [], "SB_EXTENSION_DIR", [value(Path)], "Unpacked extension folders to load, separated by commas"),
            (locale, "locale", [], [], "SB_LOCALE", [value(Text)], "The browser locale"),
            (user_agent, "user-agent", ["agent"], [], "SB_USER_AGENT", [value(Text)], "The user agent string"),
            (mode, "mode", [], [], "SB_MODE", [value(Mode)], "The driver mode: webdriver, cdp or uc"),
            (ad_block, "ad-block", ["adblock", "block-ads"], ["ad_block_on"], "SB_AD_BLOCK", [switch], "Block the requests of ad and tracker hosts"),
            (reuse_session, "reuse-session", ["rs"], [], "SB_REUSE_SESSION", [switch], "Share browsers among the tests of the run"),
            (reuse_class_session, "reuse-class-session", ["rcs"], [], "SB_REUSE_CLASS_SESSION", [switch], "Share a browser among the tests of one module"),
            (mobile, "mobile", ["mobile-emulator"], ["is_mobile"], "SB_MOBILE", [switch], "Emulate a mobile phone"),
            (threads, "threads", [], [], "SB_THREADS", [env(Count)], "The number of parallel test threads"),
            (proxy_pac_url, "proxy-pac-url", ["pac-url"], [], "SB_PROXY_PAC_URL", [value(Url)], "A proxy auto-config URL"),
            (esc_end, "esc-end", ["ee"], [], "SB_ESC_END", [switch], "End the current test when ESC is pressed in the browser"),
            (rec_sb_cdp, "rec-sb-cdp", [], [], "SB_REC_SB_CDP", [switch], "Write recordings as pure CDP code"),
            (fingerprint.file, "fingerprint", [], [], "SB_FINGERPRINT", [value(Path)], "A fingerprint file every browser presents"),
            (start_page, "start-page", ["url"], [], "SB_START_PAGE", [value(Url)], "The page every browser opens first"),
            (settings_file, "settings-file", ["settings"], [], "SB_SETTINGS_FILE", [value(Path)], "The settings file to read instead of sbase_config.toml"),
            // Which browser: shorthands for a value of SB_BROWSER, SB_HEADLESS and SB_MODE.
            (browser, "chrome", [], [], "SB_BROWSER", [sets("chrome")], "Run Chrome"),
            (browser, "edge", [], [], "SB_BROWSER", [sets("edge")], "Run Edge"),
            (browser, "firefox", [], [], "SB_BROWSER", [sets("firefox")], "Run Firefox"),
            (browser, "brave", [], [], "SB_BROWSER", [sets("brave")], "Run Brave"),
            (browser, "comet", [], [], "SB_BROWSER", [sets("comet")], "Run Comet"),
            (browser, "opera", [], [], "SB_BROWSER", [sets("opera")], "Run Opera"),
            (browser, "safari", [], [], "SB_BROWSER", [sets("safari")], "Run Safari"),
            (browser, "ie", [], [], "SB_BROWSER", [sets("ie")], "Run Internet Explorer"),
            (browser, "cft", [], [], "SB_BROWSER", [sets("cft")], "Run Chrome for Testing"),
            (browser, "chs", [], [], "SB_BROWSER", [sets("chs")], "Run chrome-headless-shell"),
            (browser, "chromium", ["use-chromium"], [], "SB_BROWSER", [sets("chromium")], "Run Chromium"),
            (headless, "headed", ["gui"], ["headed"], "SB_HEADLESS", [sets("false")], "Run with a visible window"),
            (mode, "uc", ["undetected", "undetectable"], ["uc", "undetected", "undetectable"], "SB_MODE", [sets("uc")], "Run in undetected-chromedriver mode"),
            (headless1, "headless1", [], [], "SB_HEADLESS1", [switch], "Run headless with chrome-headless-shell (Chrome's old headless mode)"),
            (locale_code, "locale-code", [], [], "SB_LOCALE_CODE", [value(Text)], "The browser language code, such as fr or ar_eg"),
            (server, "server", [], ["servername"], "SB_SERVER", [value(Text)], "The Selenium Grid server to connect to"),
            (port, "port", [], [], "SB_PORT", [value(Port)], "The port of the Selenium Grid server"),
            (protocol, "protocol", [], [], "SB_PROTOCOL", [value(Text)], "The protocol of the Selenium Grid server: http or https"),
            // Chromium switches.
            (chromium_arg, "chromium-arg", [], [], "SB_CHROMIUM_ARG", [value(List)], "Extra Chromium switches, separated by commas"),
            (dark_mode, "dark-mode", ["dark"], [], "SB_DARK_MODE", [switch], "Force Chrome's dark theme on web pages"),
            (devtools, "devtools", ["open-devtools"], [], "SB_DEVTOOLS", [switch], "Open the developer tools with each tab (headed only)"),
            (disable_features, "disable-features", [], [], "SB_DISABLE_FEATURES", [value(List)], "Chromium features to switch off, separated by commas"),
            (disable_gpu, "disable-gpu", [], [], "SB_DISABLE_GPU", [switch], "Switch the GPU off"),
            (disable_ws, "disable-ws", ["dws", "disable-web-security"], [], "SB_DISABLE_WS", [switch], "Switch web security off"),
            (guest_mode, "guest-mode", ["guest"], [], "SB_GUEST_MODE", [switch], "Open the browser as a guest"),
            (incognito, "incognito", ["incognito-mode"], [], "SB_INCOGNITO", [switch], "Open the browser in an incognito window"),
            (host_resolver_rules, "host-resolver-rules", [], [], "SB_HOST_RESOLVER_RULES", [value(Text)], "Chromium rules that map host names to addresses"),
            (no_sandbox, "no-sandbox", [], [], "SB_NO_SANDBOX", [switch], "Start Chromium without its sandbox"),
            (swiftshader, "swiftshader", [], [], "SB_SWIFTSHADER", [switch], "Render with SwiftShader, a software WebGL renderer"),
            (enable_3d_apis, "enable-3d-apis", [], [], "SB_ENABLE_3D_APIS", [switch], "Keep the GPU on in a headless session, for WebGL"),
            (maximize, "maximize", ["maximize-window", "fullscreen"], [], "SB_MAXIMIZE", [switch], "Maximize the browser window"),
            // Preferences and launch settings that were implemented before the table.
            (block_images, "block-images", [], [], "SB_BLOCK_IMAGES", [switch], "Do not load images"),
            (disable_js, "disable-js", [], [], "SB_DISABLE_JS", [switch], "Turn JavaScript off"),
            (disable_cookies, "disable-cookies", [], [], "SB_DISABLE_COOKIES", [switch], "Refuse every cookie"),
            (do_not_track, "do-not-track", [], [], "SB_DO_NOT_TRACK", [switch], "Send the DNT: 1 header"),
            (external_pdf, "external-pdf", [], [], "SB_EXTERNAL_PDF", [switch], "Download PDF files instead of showing them"),
            (enable_sync, "enable-sync", [], [], "SB_ENABLE_SYNC", [switch], "Leave Chrome Sync on"),
            (disable_csp, "disable-csp", ["dcsp", "no-csp"], [], "SB_DISABLE_CSP", [switch], "Strip the Content Security Policy from every response"),
            (use_auto_ext, "use-auto-ext", ["auto-ext"], [], "SB_USE_AUTO_EXT", [switch], "Keep Chrome's automation extension"),
            (proxy_driver, "proxy-driver", [], [], "SB_PROXY_DRIVER", [switch], "Download the driver through the proxy"),
            (page_load_strategy, "page-load-strategy", ["pls"], [], "SB_PAGE_LOAD_STRATEGY", [value(Strategy)], "How long a navigation waits: normal, eager or none"),
            (window_size, "window-size", [], ["size"], "SB_WINDOW_SIZE", [value(Size)], "The window size: WIDTH,HEIGHT"),
            (window_position, "window-position", [], ["position"], "SB_WINDOW_POSITION", [value(Position)], "The window position: X,Y"),
            (device_metrics, "device-metrics", ["metrics"], [], "SB_DEVICE_METRICS", [value(Metrics)], "Emulate a phone screen: WIDTH,HEIGHT,PIXEL_RATIO"),
            (proxy_bypass_list, "proxy-bypass-list", [], [], "SB_PROXY_BYPASS_LIST", [value(List)], "Hosts that skip the proxy"),
            (driver_version, "driver-version", [], [], "SB_DRIVER_VERSION", [value(Version)], "The driver version: a number, latest, stable, beta, ..."),
            (cap_file, "cap-file", [], [], "SB_CAP_FILE", [value(Path)], "A JSON file of capabilities to add to the session's"),
            (cap_string, "cap-string", [], [], "SB_CAP_STRING", [value(Json)], "A JSON object of capabilities to add to the session's"),
            (extension_zip, "extension-zip", ["crx"], [], "SB_EXTENSION_ZIP", [value(List)], "Extension .zip or .crx files to load, separated by commas"),
            (firefox_arg, "firefox-arg", [], [], "SB_FIREFOX_ARG", [value(List)], "Firefox arguments, separated by commas"),
            (firefox_pref, "firefox-pref", [], [], "SB_FIREFOX_PREF", [value(List)], "Firefox preferences, NAME:VALUE separated by commas"),
            (xvfb, "xvfb", [], [], "SB_XVFB", [switch], "Open a virtual display for a headed browser on Linux"),
            (xvfb_metrics, "xvfb-metrics", [], [], "SB_XVFB_METRICS", [value(Size)], "The virtual display size: WIDTH,HEIGHT"),
            (remote_debug, "remote-debug", ["remote-debugger"], [], "SB_REMOTE_DEBUG", [switch], "Open Chrome's remote debugger on port 9222"),
            (log_cdp_events, "log-cdp-events", ["log-cdp"], [], "SB_LOG_CDP_EVENTS", [switch], "Log the DevTools Protocol events"),
            (uc_cdp_events, "uc-cdp-events", ["uc-cdp"], [], "SB_UC_CDP_EVENTS", [switch], "Log the DevTools Protocol events in undetected mode"),
            // Run settings and page behaviour.
            (demo, "demo", ["demo-mode"], [], "SB_DEMO", [switch], "Highlight each element and pause after an action"),
            (slow, "slow", ["slow-mode", "slowmo"], [], "SB_SLOW", [switch], "Pause briefly after each action"),
            (demo_sleep, "demo-sleep", [], [], "SB_DEMO_SLEEP", [value(Seconds)], "Seconds to pause after each action in demo mode"),
            (message_duration, "message-duration", [], [], "SB_MESSAGE_DURATION", [value(Seconds)], "Seconds a message stays on the page"),
            (skip_js_waits, "skip-js-waits", ["sjw"], [], "SB_SKIP_JS_WAITS", [switch], "Do not wait for the page's JavaScript to settle"),
            (highlights, "highlights", [], [], "SB_HIGHLIGHTS", [value(Count)], "How many times to flash a highlighted element"),
            (interval, "interval", [], [], "SB_INTERVAL", [value(Seconds)], "Seconds between the steps of a tour or presentation"),
            (verify_delay, "verify-delay", [], [], "SB_VERIFY_DELAY", [value(Seconds)], "Seconds to wait before each MasterQA verification"),
            (check_js, "check-js", [], ["js_checking_on"], "SB_CHECK_JS", [switch], "Check each page for JavaScript errors after it opens"),
            (wait_for_angularjs, "wait-for-angularjs", ["wfa"], [], "SB_WAIT_FOR_ANGULARJS", [switch], "Wait for AngularJS after opening a page and clicking"),
            (disable_beforeunload, "disable-beforeunload", [], [], "SB_DISABLE_BEFOREUNLOAD", [switch], "Remove the beforeunload prompt after each page opens"),
            (visual_baseline, "visual-baseline", [], [], "SB_VISUAL_BASELINE", [switch], "Set the visual baselines again instead of comparing"),
            (crumbs, "crumbs", [], [], "SB_CRUMBS", [switch], "Delete the cookies when a reused browser is handed to the next test"),
            (timeout_multiplier, "timeout-multiplier", [], [], "SB_TIMEOUT_MULTIPLIER", [value(Float)], "Multiply the default waits by this number"),
            (test.rec_sleep, "rec-sleep", ["record-sleep"], [], "SB_REC_SLEEP", [switch], "Keep the sleeps of a recording"),
            // Test data.
            (test.data.env, "env", [], ["environment"], "SB_ENV", [custom(Env, set_env)], "The environment the tests run against: qa, staging, production, ..."),
            (test.data.account, "account", [], [], "SB_ACCOUNT", [value(Text)], "A free account name for the tests"),
            (test.data.data, "data", [], [], "SB_DATA", [value(Text)], "Free test data"),
            (test.data.var1, "var1", [], [], "SB_VAR1", [value(Text)], "A free variable for the tests"),
            (test.data.var2, "var2", [], [], "SB_VAR2", [value(Text)], "A free variable for the tests"),
            (test.data.var3, "var3", [], [], "SB_VAR3", [value(Text)], "A free variable for the tests"),
            (test.data.variables, "variables", [], [], "SB_VARIABLES", [custom(Dict, set_variables)], "A dictionary of values: JSON or a Python dictionary literal"),
            // Debug modes, run control and evidence.
            (test.pdb, "pdb", [], [], "SB_PDB", [switch], "Stop in a post-mortem prompt when a test fails"),
            (test.trace, "trace", [], [], "SB_TRACE", [switch], "Stop in a prompt before each test's body"),
            (test.final_debug, "final-debug", ["fdebug", "final-trace", "ftrace"], [], "SB_FINAL_DEBUG", [switch], "Stop in a prompt after each test, before the browser quits"),
            (test.reruns, "reruns", [], [], "SB_RERUNS", [value(Count)], "Run a failing test again this many times"),
            (test.reruns_delay, "reruns-delay", [], [], "SB_RERUNS_DELAY", [value(Seconds)], "Seconds to wait between reruns"),
            (test.time_limit, "time-limit", ["timelimit"], [], "SB_TIME_LIMIT", [value(Seconds)], "Fail a test that runs longer than this many seconds"),
            (test.runner, "runner", [], [], "SB_TEST_RUNNER", [value(Runner)], "The test runner: cargo or nextest"),
            (test.save_screenshot, "screenshot", ["save-screenshot", "ss"], [], "SB_SAVE_SCREENSHOT", [switch], "Save a screenshot of passing tests too"),
            (test.no_screenshot, "no-screenshot", ["ns"], [], "SB_NO_SCREENSHOT", [switch], "Save no screenshot of failing tests"),
            (test.list_fail_page, "list-fail-page", ["list-fail-pages", "lfp"], [], "SB_LIST_FAIL_PAGE", [switch], "Print the URL of the page a failing test ended on"),
            (test.archive_logs, "archive-logs", [], [], "SB_ARCHIVE_LOGS", [switch], "Keep the old latest_logs folder in archived_logs"),
            (test.archive_downloads, "archive-downloads", [], [], "SB_ARCHIVE_DOWNLOADS", [switch], "Keep the old downloaded_files folder in archived_files"),
            (test.dashboard, "dashboard", [], [], "SB_DASHBOARD", [switch], "Write the results dashboard"),
            (test.dash_title, "dash-title", [], [], "SB_DASH_TITLE", [value(Text)], "The dashboard's title"),
            (test.with_db_reporting, "with-db-reporting", [], [], "SB_WITH_DB_REPORTING", [switch], "Record each test's outcome in the results database"),
            (test.database_env, "database-env", [], [], "SB_DATABASE_ENV", [value(Text)], "Which results database to use"),
            (test.with_s3_logging, "with-s3-logging", [], [], "SB_WITH_S3_LOGGING", [switch], "Upload each test's logs to S3"),
            // The recorder.
            (test.recorder, "rec", ["record", "recorder", "codegen"], ["recorder_ext"], "SB_RECORDER", [switch], "Record the browser actions as a test"),
            (test.rec_behave, "rec-behave", ["rec-gherkin"], [], "SB_REC_BEHAVE", [switch], "Also write the recording as a Gherkin feature"),
            (test.rec_print, "rec-print", [], [], "SB_REC_PRINT", [switch], "Print the recording when the test ends"),
            (test.rec_sb_mgr, "rec-sb-mgr", [], [], "SB_REC_SB_MGR", [switch], "Write the recording as a script"),
            // Accepted and ignored, or not a flag.
            (none, "all-scripts", [], [], "", [cli("sbrs test runs every example after the tests")], "Run every example after the tests"),
            (none, "with-testing-base", [], [], "", [noop("always on in the sb fixture, as upstream")], "Accepted and ignored"),
            (none, "with-basic-test-info", [], [], "", [noop("always on in the sb fixture, as upstream")], "Accepted and ignored"),
            (none, "with-page-source", [], [], "", [noop("always on in the sb fixture, as upstream")], "Accepted and ignored"),
            (none, "with-screen-shots", [], [], "", [noop("always on in the sb fixture, as upstream")], "Accepted and ignored"),
            (none, "log-path", [], [], "", [noop("deprecated upstream; logs always go to latest_logs/")], "Accepted and ignored"),
            (none, "with-selenium", [], [], "", [noop("deprecated upstream; always on")], "Accepted and ignored"),
            (none, "is-pytest", [], [], "", [noop("an internal pytest flag")], "Accepted and ignored"),
            (none, "uc-sub", ["uc-subprocess"], [], "", [noop("deprecated upstream; undetected mode always uses a subprocess")], "Accepted and ignored"),
            (none, "enable-ws", ["enable-web-security"], [], "", [noop("web security is never switched off unless asked")], "Accepted and ignored"),
            (none, "fast", ["fast-mode"], [], "", [noop("the CDP engine makes no wait to skip")], "Accepted and ignored"),
            (none, "multi-proxy", [], [], "", [noop("each CDP browser answers its own proxy login; WebDriver cannot log in to a proxy")], "Accepted and ignored"),
            (none, "raise-test-failure", [], ["rtf"], "", [na("Sb::run always returns the error; the caller decides")], "Not a flag"),
            (none, "test-mode", [], ["test"], "", [na("Sb::run is SB(test=True); run_browser_test is the other form")], "Not a flag"),
        }
    };
}

sb_test_options!(option_table);

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn read_all(pairs: &[(&str, &str)]) -> (Settings, Vec<SettingsProblem>) {
        let mut settings = Settings::default();
        let mut problems = Vec::new();
        let var = |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        };
        apply_env(&mut settings, &var, &mut problems);
        (settings, problems)
    }

    #[test]
    fn every_spelling_belongs_to_one_row() {
        let mut seen = BTreeSet::new();
        for row in TEST_OPTIONS {
            for flag in row.flags() {
                assert!(
                    seen.insert(normalise(flag)),
                    "{flag} is spelled by two rows of the table"
                );
            }
        }
    }

    #[test]
    fn a_variable_belongs_to_one_row_unless_the_rows_are_shorthands() {
        let mut seen = BTreeSet::new();
        for row in TEST_OPTIONS.iter().filter(|row| row.kind.reads_env()) {
            assert!(
                row.env.starts_with("SB_"),
                "{}: {} is not an SB_ variable",
                row.long,
                row.env
            );
            assert!(seen.insert(row.env), "{} is read by two rows", row.env);
        }
        for row in TEST_OPTIONS
            .iter()
            .filter(|row| matches!(row.kind, OptionKind::Sets(_)))
        {
            assert!(
                TEST_OPTIONS
                    .iter()
                    .any(|other| other.env == row.env && other.kind.reads_env()),
                "{} sets {}, which no row reads",
                row.long,
                row.env
            );
        }
    }

    #[test]
    fn rows_without_a_variable_are_the_ones_that_do_nothing() {
        for row in TEST_OPTIONS {
            let nothing = matches!(
                row.kind,
                OptionKind::NoOp(_) | OptionKind::Cli(_) | OptionKind::NotAFlag(_)
            );
            assert_eq!(row.env.is_empty(), nothing, "{}", row.long);
            assert_eq!(
                !row.has_field(),
                nothing && row.field == "none",
                "{}",
                row.long
            );
            assert!(!row.help.is_empty(), "{}", row.long);
        }
    }

    #[test]
    fn a_flag_is_found_in_any_spelling() {
        for spelling in ["--dark", "dark", "--dark_mode", "-dark-mode", "--DARK"] {
            assert_eq!(find_option(spelling).map(|row| row.long), Some("dark-mode"));
        }
        assert!(find_option("--nosuch").is_none());
        let metrics = find_option("--metrics").unwrap();
        assert!(metrics.matches_sb_name("device_metrics"));
        let mobile = find_option("--mobile-emulator").unwrap();
        assert!(mobile.matches_sb_name("is_mobile"));
        assert!(!mobile.matches_sb_name("is_desktop"));
    }

    #[test]
    fn a_switch_reads_true_and_false_and_refuses_the_rest() {
        let (settings, problems) = read_all(&[("SB_DARK_MODE", "true"), ("SB_PDB", "1")]);
        assert!(problems.is_empty());
        assert!(settings.dark_mode && settings.test.pdb);

        let (settings, problems) = read_all(&[("SB_DARK_MODE", "off")]);
        assert!(problems.is_empty() && !settings.dark_mode);

        let (_, problems) = read_all(&[("SB_DEVTOOLS", "maybe")]);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].source, "SB_DEVTOOLS");
        assert_eq!(
            problems[0].error.to_string(),
            "invalid configuration: cannot parse bool: maybe"
        );
    }

    #[test]
    fn a_value_is_stored_as_text_a_path_a_number_or_seconds() {
        let (settings, problems) = read_all(&[
            ("SB_WINDOW_SIZE", "1280,720"),
            ("SB_CAP_FILE", "caps.json"),
            ("SB_PORT", "4445"),
            ("SB_HIGHLIGHTS", "3"),
            ("SB_RERUNS", "2"),
            ("SB_RERUNS_DELAY", "0.25"),
            ("SB_TIME_LIMIT", "30"),
            ("SB_TIMEOUT", "7"),
            ("SB_FINGERPRINT", "profile.json"),
            ("SB_DASH_TITLE", "My Run"),
        ]);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(settings.window_size.as_deref(), Some("1280,720"));
        assert_eq!(settings.cap_file, Some(PathBuf::from("caps.json")));
        assert_eq!(settings.port, Some(4445));
        assert_eq!(settings.highlights, Some(3));
        assert_eq!(settings.test.reruns, 2);
        assert!((settings.test.reruns_delay - 0.25).abs() < f64::EPSILON);
        assert_eq!(settings.test.time_limit, Some(30.0));
        assert_eq!(settings.timeout_seconds, Some(7));
        assert_eq!(
            settings.fingerprint.file,
            Some(PathBuf::from("profile.json"))
        );
        assert_eq!(settings.test.dash_title.as_deref(), Some("My Run"));
    }

    #[test]
    fn a_number_that_does_not_parse_names_its_variable() {
        let (_, problems) = read_all(&[
            ("SB_PORT", "http"),
            ("SB_TIME_LIMIT", "-1"),
            ("SB_INTERVAL", "soon"),
        ]);
        let sources: Vec<_> = problems.iter().map(|p| p.source.as_str()).collect();
        assert_eq!(sources, ["SB_PORT", "SB_INTERVAL", "SB_TIME_LIMIT"]);
        assert!(problems[0]
            .error
            .to_string()
            .starts_with("invalid configuration: SB_PORT: "));
        assert!(problems[2]
            .error
            .to_string()
            .contains("expected a number of seconds"));
    }

    #[test]
    fn shorthands_are_not_read_from_the_environment() {
        // SB_BROWSER=firefox is the `browser` row's; the `--chrome` and
        // `--firefox` rows only tell the command line what to write.
        let (settings, problems) = read_all(&[("SB_BROWSER", "firefox"), ("SB_MODE", "uc")]);
        assert!(problems.is_empty());
        assert_eq!(settings.browser, "firefox");
        assert_eq!(settings.mode.as_deref(), Some("uc"));
        let chrome = find_option("--chrome").unwrap();
        assert_eq!(chrome.kind, OptionKind::Sets("chrome"));
        assert_eq!(chrome.env, "SB_BROWSER");
        let headed = find_option("--gui").unwrap();
        assert_eq!(
            (headed.env, headed.kind),
            ("SB_HEADLESS", OptionKind::Sets("false"))
        );
    }

    #[test]
    fn test_data_is_checked_when_it_is_read() {
        let (settings, problems) = read_all(&[
            ("SB_ENV", "QA"),
            ("SB_ACCOUNT", "alice"),
            ("SB_DATA", "abc"),
            ("SB_VAR1", "x"),
            ("SB_VARIABLES", "{'k': 1}"),
        ]);
        assert!(problems.is_empty(), "{problems:?}");
        let data = &settings.test.data;
        assert_eq!(data.env(), "qa");
        assert_eq!(data.account.as_deref(), Some("alice"));
        assert_eq!(data.data.as_deref(), Some("abc"));
        assert_eq!(data.var1.as_deref(), Some("x"));
        assert_eq!(data.variables["k"], 1);

        let (settings, problems) = read_all(&[("SB_ENV", "nosuch"), ("SB_VARIABLES", "[1]")]);
        assert_eq!(settings.test.data.env(), "test");
        assert_eq!(problems.len(), 2);
        assert!(problems[0]
            .error
            .to_string()
            .contains("choose from qa, rc, staging"));
        assert!(problems[1]
            .error
            .to_string()
            .contains("Expecting a dictionary"));
    }

    #[test]
    fn rows_with_no_variable_read_nothing() {
        // `--all-scripts` has no variable, and `none` is not a field.
        let (settings, problems) = read_all(&[("", "true"), ("none", "true")]);
        assert!(problems.is_empty());
        assert_eq!(settings, Settings::default());
    }
}
