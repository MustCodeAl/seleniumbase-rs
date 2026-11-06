use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::api::run_settings::RunSettings;
use crate::browser::config::BrowserConfig;
use crate::browser::options::{BrowserSettings, LaunchSwitch, SessionSettings};
use crate::config::env_vars::{EnvVars, Read};
use crate::config::layers::{self, Layer, Refused};
use crate::config::plugins::{PluginSelection, PLUGINS_ENV};
use crate::config::settings_file::SettingsFile;

use crate::config::test_settings::{TestData, TestSettings};
use crate::core::test_run::ReportSettings;

use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::Fingerprint;

/// The variable that names the fingerprint file to apply to every browser.
pub const FINGERPRINT_ENV: &str = "SB_FINGERPRINT";

/// The variable that names the page every browser opens first (Python's `--start-page`;
/// `--start-page`, `--url` and `--start_page` on `sbrs`).
pub const START_PAGE_ENV: &str = "SB_START_PAGE";

/// The variable that names the settings file to read instead of the global file of the
/// current folder (Python's `--settings-file`). A relative path is relative to the folder
/// the program runs in; [`Settings::load`] reads it.
pub const SETTINGS_FILE_ENV: &str = "SB_SETTINGS_FILE";

/// The `[fingerprint]` table of the settings file: which fingerprint file every browser
/// of the run presents (`SB_FINGERPRINT`, and `--fingerprint` on `sbrs`).
///
/// The file is the v2 `Fingerprint` JSON that `Fingerprint::save` and
/// `sbrs fingerprint capture` write. A relative path is relative to the folder the
/// program runs in. The command line wins over `SB_FINGERPRINT`, which wins over this
/// table.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::Settings;
///
/// let settings: Settings = toml::from_str(
///     r#"
///     headless = true
///
///     [fingerprint]
///     file = "profiles/real-chrome.json"
///     "#,
/// )?;
///
/// assert_eq!(
///     settings.fingerprint.file.as_deref(),
///     Some(std::path::Path::new("profiles/real-chrome.json"))
/// );
/// # Ok::<(), toml::de::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FingerprintSettings {
    /// The fingerprint file to apply. `None` applies none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
}

impl FingerprintSettings {
    /// Whether no fingerprint file is named.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.file.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
/// Every setting of a run, in groups: what the browser starts, what a WebDriver session reads,
/// how the run behaves, the test data and debug modes, the reports, the fingerprint and the
/// plugins. `Settings` is what a settings file, the `SB_*` variables and the flags of `sbrs`
/// write; [`to_browser_config`](Self::to_browser_config) and
/// [`to_launch_options`](Self::to_launch_options) start a browser from it.
///
/// [`browser`](Self::browser), [`session`](Self::session) and [`run`](Self::run) are the very
/// types a [`BrowserConfig`] holds, so a setting has one name in both. The
/// option table ([`TEST_OPTIONS`](crate::config::test_options::TEST_OPTIONS)) names the path of
/// each setting (`browser.proxy.server`, `session.grid.port`), its variable and its flag.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::Settings;
///
/// let mut settings = Settings::default();
/// settings.browser.headless = true;
/// settings.browser.proxy.server = Some("proxy.example.com:8080".to_owned());
/// settings.session.grid.port = Some(4445);
/// let config = settings.to_browser_config();
/// assert!(config.browser.headless);
/// ```
pub struct Settings {
    /// What both engines start: the browser, its window, proxy, profile and switches
    /// (`SB_BROWSER`, `SB_HEADLESS`, `SB_PROXY`, `SB_DARK_MODE`, ...). The file writes each
    /// setting as a flat key (`dark_mode = true`).
    pub browser: BrowserSettings,
    /// What only a WebDriver session reads: the driver mode, the server, the Grid, the reuse of
    /// browsers and the capabilities (`SB_MODE`, `SB_WEBDRIVER_URL`, `SB_SERVER`,
    /// `SB_REUSE_SESSION`, ...).
    pub session: SessionSettings,
    /// How a whole test run behaves: demo and slow mode, the default wait, the pauses, the
    /// highlights. It is the [`RunSettings`] of a [`BaseCase`](crate::BaseCase) made from
    /// [`to_browser_config`](Self::to_browser_config); the file may write each setting flat
    /// (`demo = true`) or in the table (`[run] demo_mode = true`).
    pub run: RunSettings,
    /// The `[test]` table: test data, debug modes, reruns and the number of threads.
    pub test: TestSettings,
    /// The `[report]` table, the one table of the reports, the evidence kept and the results
    /// database (`SB_HTML`, `SB_JUNIT_XML`, `SB_ALLURE_DIR`, `SB_DASHBOARD`, ...). The `[test]`
    /// table of a settings file still reads the report keys it held. See
    /// [`ReportSettings::from_settings`] for the results database.
    pub report: ReportSettings,
    /// The fingerprint file every browser presents (the `[fingerprint]` table, and
    /// `SB_FINGERPRINT`). Turn it into the fingerprint with
    /// [`load_fingerprint`](Self::load_fingerprint); [`to_browser_config`](Self::to_browser_config)
    /// does it for a `BaseCase`.
    #[serde(default, skip_serializing_if = "FingerprintSettings::is_empty")]
    pub fingerprint: FingerprintSettings,
    /// The browser plugins to turn on by name (the `[plugins]` table, and
    /// `SB_PLUGINS`, which replaces `enable`). A build without the feature
    /// `plugins` refuses a non-empty selection when it loads the settings. Turn
    /// it into plugins with `BrowserConfig::with_plugin_selection` or
    /// `LaunchOptionsBuilder::plugin_selection` (feature `plugins`).
    #[serde(default, skip_serializing_if = "PluginSelection::is_empty")]
    pub plugins: PluginSelection,
    /// The settings file the run reads (`SB_SETTINGS_FILE`, Python's `--settings-file`).
    /// It names the file, so it is never read from one.
    #[serde(skip)]
    pub settings_file: Option<PathBuf>,
}

/// The settings a command line asked for explicitly, and nothing else.
///
/// Each option is a pair of an `SB_*` variable and the text of its value, and there is one only
/// for an option the user passed. That
/// distinction matters: an option left at its default must not hide the value
/// from a config file or an `SB_*` variable, but an option that was passed
/// beats both: it is the highest layer of the settings. The same overrides can be applied to a
/// [`Settings`] directly
/// ([`try_apply`](Self::try_apply)) or handed to a child process, such as a test run,
/// as environment variables ([`env`](Self::env)).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::settings::{Settings, SettingsOverrides};
///
/// let overrides = SettingsOverrides {
///     options: vec![("SB_HEADLESS", "true".to_owned()),
///         ("SB_PROXY", "proxy.example.com:8080".to_owned())],
///
/// };
///
/// let mut settings = Settings::default();
/// overrides.try_apply(&mut settings)?;
/// assert!(settings.browser.headless);
/// assert_eq!(settings.browser.proxy.server.as_deref(), Some("proxy.example.com:8080"));
///
/// let env = overrides.env();
/// assert!(env.contains(&("SB_HEADLESS", "true".to_owned())));
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
///
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsOverrides {
    /// The options, each as its `SB_*` variable and the text of its value (`--dark` is
    /// `("SB_DARK_MODE", "true")`, `--headed` is `("SB_HEADLESS", "false")`). They are read as the
    /// environment is read, so a blank value is no option and a value that does not parse is
    /// refused naming its variable and its flag; for one variable given twice, the last pair
    /// wins.
    pub options: Vec<(&'static str, String)>,
}

impl SettingsOverrides {
    /// Lays the options over `settings`, as the `SB_*` variables are laid over them (see
    /// [`Settings::apply_options`]), then checks the result with [`Settings::validate`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a value that does not parse (naming
    /// its variable) and for a result that is not valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::browser::options::LaunchSwitch;
    ///
    /// use seleniumbase_rs::config::settings::{Settings, SettingsOverrides};
    ///
    /// let overrides = SettingsOverrides {
    ///     options: vec![("SB_DARK_MODE", "true".to_owned())],
    /// };
    /// let mut settings = Settings::default();
    /// overrides.try_apply(&mut settings)?;
    /// assert!(settings.browser.has(LaunchSwitch::DarkMode));
    ///
    /// let wrong = SettingsOverrides {
    ///     options: vec![("SB_DEVICE_METRICS", "1,2".to_owned())],
    /// };
    /// assert!(wrong.try_apply(&mut Settings::default()).is_err());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn try_apply(&self, settings: &mut Settings) -> Result<(), SeleniumBaseError> {
        settings.apply_options(&self.options)
    }

    /// The `SB_*` environment variables that carry these options to a child
    /// process, one for each option.
    ///
    /// A process that loads its settings with [`Settings::load`] sees them as
    /// the highest-priority overrides it can receive from outside. A proxy
    /// address may contain a password, so hand the result only to processes
    /// you trust with it.
    #[must_use]
    pub fn env(&self) -> Vec<(&'static str, String)> {
        self.options.clone()
    }
}

/// The part of a settings file that [`Settings::load_plugin_table`] reads. Every other key
/// is left alone, so a value that is wrong for a browser does not stop a program that
/// wants the plugins only.
#[derive(Deserialize)]
struct PluginsOnly {
    #[serde(default)]
    plugins: PluginSelection,
}

/// The part of a settings file that [`Settings::resolve_fingerprint_file`] reads.
#[derive(Deserialize)]
struct FingerprintOnly {
    #[serde(default)]
    fingerprint: FingerprintSettings,
}

/// The `[fingerprint]` table of the settings file `path` (TOML for a `.toml` name, JSON
/// otherwise), without checking the rest of the file.
fn parse_fingerprint_table(path: &Path) -> Result<FingerprintSettings, SeleniumBaseError> {
    let content = fs::read_to_string(path)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("failed to read settings: {e}")))?;
    let file: FingerprintOnly = if path.extension().is_some_and(|ext| ext == "toml") {
        toml::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse TOML settings: {e}"))
        })?
    } else {
        serde_json::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse settings: {e}"))
        })?
    };
    Ok(file.fingerprint)
}

/// The `[plugins]` table of the settings file `path` (TOML for a `.toml` name, JSON
/// otherwise), without checking the rest of the file.
fn parse_plugins_table(path: &Path) -> Result<PluginSelection, SeleniumBaseError> {
    let content = fs::read_to_string(path)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("failed to read settings: {e}")))?;
    let file: PluginsOnly = if path.extension().is_some_and(|ext| ext == "toml") {
        toml::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse TOML settings: {e}"))
        })?
    } else {
        serde_json::from_str(&content).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to parse settings: {e}"))
        })?
    };
    Ok(file.plugins)
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let settings = Self::parse_file(path)?;
        settings.plugins.ensure_supported()?;
        settings.check_fingerprint()?;
        Ok(settings)
    }

    /// Reads a settings file without checking that this build can run the
    /// plugins it names, so that [`load`](Self::load) can let `SB_PLUGINS`
    /// replace them first and check the result.
    fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        let settings = Self::read_file(path)?;
        settings.validate()?;
        Ok(settings)
    }

    /// Reads and deserializes a settings file, without validating its values.
    fn read_file<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        SettingsFile::read(path.as_ref())?.settings()
    }

    /// Checks what the values do not say by themselves: the window size.
    ///
    /// A browser, a mode and the rest are typed fields, refused where they are read.
    /// [`from_file`](Self::from_file), [`load`](Self::load) and
    /// [`from_env`](Self::from_env) call it, so a window with one side is an error there.
    /// Call it
    /// yourself on a `Settings` built by hand, before
    /// [`to_browser_config`](Self::to_browser_config).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a window width without a height
    /// (or the other way round)
    /// or one of zero pixels.
    pub fn validate(&self) -> Result<(), SeleniumBaseError> {
        // A lone side of the window is a size with a side of zero (`SB_WINDOW_WIDTH` read
        // before `SB_WINDOW_HEIGHT` may still come from a higher source).
        if let Some(size) = self.browser.window_size
            && (size.width == 0 || size.height == 0)
        {
            let side = |pixels: u32| (pixels > 0).then_some(pixels);
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "window_width and window_height must both be set, and above zero: ({:?}, {:?})",
                side(size.width),
                side(size.height)
            )));
        }
        Ok(())
    }

    /// The fingerprint the `[fingerprint]` table or `SB_FINGERPRINT` names, read from its
    /// file; `None` when the settings name none.
    ///
    /// # Errors
    ///
    /// As [`Fingerprint::load`]: the file cannot be read, is too large, or is not a
    /// fingerprint.
    pub fn load_fingerprint(&self) -> Result<Option<Fingerprint>, SeleniumBaseError> {
        self.fingerprint
            .file
            .as_deref()
            .map(Fingerprint::load)
            .transpose()
    }

    /// Checks that the fingerprint file the settings name, if any, loads.
    ///
    /// [`from_file`](Self::from_file), [`load`](Self::load) and
    /// [`from_env`](Self::from_env) call it, so a wrong path is an error where the
    /// settings are read and not a browser without the fingerprint. Call it yourself on
    /// a `Settings` built by hand, before [`to_browser_config`](Self::to_browser_config).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] naming the file and the reason.
    pub fn check_fingerprint(&self) -> Result<(), SeleniumBaseError> {
        self.load_fingerprint().map(drop).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!("the fingerprint of the settings: {error}"))
        })
    }

    /// The fingerprint file a command presents, without reading the rest of the settings:
    /// `flag` (the command line), else `SB_FINGERPRINT`, else the `[fingerprint]` table of
    /// the settings file `config` (the global file when `None`, as [`load`](Self::load)
    /// finds it).
    ///
    /// A command that starts a browser of its own (`sbrs devtools`) uses this, so that a
    /// value of the file that is wrong for a browser (`headless = "maybe"`) is not its
    /// problem. The file named is not read.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] if the settings file cannot be read or its
    /// `[fingerprint]` table is not valid.
    pub fn resolve_fingerprint_file<P: AsRef<Path>>(
        config: Option<P>,
        flag: Option<&Path>,
    ) -> Result<Option<PathBuf>, SeleniumBaseError> {
        Self::fingerprint_file_with(config, flag, |name| std::env::var(name).ok())
    }

    /// [`resolve_fingerprint_file`](Self::resolve_fingerprint_file) with the variable
    /// taken from `var`.
    fn fingerprint_file_with<P: AsRef<Path>>(
        config: Option<P>,
        flag: Option<&Path>,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<PathBuf>, SeleniumBaseError> {
        if let Some(flag) = flag {
            return Ok(Some(flag.to_owned()));
        }
        if let Some(file) = EnvVars::new("environment variable", &var).text(FINGERPRINT_ENV) {
            return Ok(Some(PathBuf::from(file)));
        }
        let table = match config {
            Some(path) => Some(path.as_ref().to_owned()),
            None => Self::global_file(),
        };
        match table {
            Some(path) => Ok(parse_fingerprint_table(&path)?.file),
            None => Ok(None),
        }
    }

    pub fn from_env() -> Result<Self, SeleniumBaseError> {
        Self::apply_overrides(Self::default(), |name| std::env::var(name).ok())
    }

    /// Loads the global config file if one exists.
    ///
    /// Searches, in order: `sbase_config.toml`, `.sbase_config.toml`, `.sbase_config`.
    pub fn load_global() -> Result<Self, SeleniumBaseError> {
        let settings = Self::parse_global()?;
        settings.plugins.ensure_supported()?;
        settings.check_fingerprint()?;
        Ok(settings)
    }

    /// [`load_global`](Self::load_global) without the plugin support check.
    fn parse_global() -> Result<Self, SeleniumBaseError> {
        match Self::global_file() {
            Some(candidate) => Self::parse_file(candidate),
            None => Ok(Self::default()),
        }
    }

    /// The global settings file of the current folder, if there is one.
    fn global_file() -> Option<PathBuf> {
        Self::global_file_with(&|name| std::env::var(name).ok())
    }

    /// The settings file to read: the one `SB_SETTINGS_FILE` names (when `var` reports it
    /// and it is not blank), else the global file of the current folder if there is one.
    pub(super) fn global_file_with(var: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
        if let Some(path) = EnvVars::new("environment variable", var).text(SETTINGS_FILE_ENV) {
            return Some(PathBuf::from(path));
        }
        ["sbase_config.toml", ".sbase_config.toml", ".sbase_config"]
            .into_iter()
            .find(|candidate| Path::new(candidate).is_file())
            .map(PathBuf::from)
    }

    /// Loads the global config file if one exists ([`load_global`](Self::load_global)),
    /// then applies the `SB_*` environment variables on top.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the file cannot be read
    /// or parsed, or a variable or the result is not valid.
    pub fn load() -> Result<Self, SeleniumBaseError> {
        Self::load_with(None::<&Path>, |name| std::env::var(name).ok())
    }

    /// Loads the settings file `path` (TOML for a `.toml` name, JSON
    /// otherwise), then applies the `SB_*` environment variables on top.
    /// [`from_file`](Self::from_file) alone does not read the environment.
    ///
    /// # Errors
    ///
    /// As [`load`](Self::load).
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self, SeleniumBaseError> {
        Self::load_with(Some(path), |name| std::env::var(name).ok())
    }

    /// Loads only the plugin selection: the `[plugins]` table of the settings
    /// file `path` (the global file when `None`, as [`load`](Self::load)
    /// finds it) with `SB_PLUGINS` on top, which is [`load`](Self::load)'s
    /// `plugins` without the rest of the settings.
    ///
    /// A command that starts a browser without needing the other settings
    /// (`sbrs devtools`, `sbrs plugin list`, `sbrs-mcp`) uses this, so that a
    /// value of the file that is wrong for a browser (`headless = "maybe"`) or
    /// an invalid `SB_HEADLESS` is not its problem: only the `[plugins]` table
    /// is read from the file. Whether this build can run the plugins is not
    /// checked here, because the command line adds to the selection first:
    /// call [`PluginSelection::ensure_supported`] on the result, or build it.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] if the file cannot be read or is
    /// not valid TOML (JSON for a name that does not end in `.toml`), or its
    /// `[plugins]` table is not a plugin selection.
    pub fn load_plugin_selection<P: AsRef<Path>>(
        path: Option<P>,
    ) -> Result<PluginSelection, SeleniumBaseError> {
        Self::plugin_selection_with(path, |name| std::env::var(name).ok())
    }

    /// [`load_plugin_selection`](Self::load_plugin_selection) with the
    /// variable taken from `var`.
    fn plugin_selection_with<P: AsRef<Path>>(
        path: Option<P>,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<PluginSelection, SeleniumBaseError> {
        let mut selection = Self::load_plugin_table(path)?;
        // As in `apply_overrides`: the variable names the plugins, and an
        // empty one is an answer, not an unset variable.
        if let Some(list) = var(PLUGINS_ENV) {
            selection.set_enabled_from_list(&list);
        }
        Ok(selection)
    }

    /// The `[plugins]` table of the settings file `path` (the global file
    /// when `None`, as [`load`](Self::load) finds it) and nothing else of it:
    /// no other value of the file is checked, and `SB_PLUGINS` is not read.
    /// A folder without a settings file selects no plugin.
    ///
    /// A program that applies `SB_PLUGINS` and command-line flags itself uses
    /// this, and so can tell which plugins the file turned on.
    ///
    /// # Errors
    ///
    /// As [`load_plugin_selection`](Self::load_plugin_selection).
    pub fn load_plugin_table<P: AsRef<Path>>(
        path: Option<P>,
    ) -> Result<PluginSelection, SeleniumBaseError> {
        match path {
            Some(path) => parse_plugins_table(path.as_ref()),
            None => match Self::global_file() {
                Some(candidate) => parse_plugins_table(&candidate),
                None => Ok(PluginSelection::new()),
            },
        }
    }

    /// [`load`](Self::load) and [`load_from`](Self::load_from) with the
    /// overrides taken from `var`.
    fn load_with<P: AsRef<Path>>(
        path: Option<P>,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, SeleniumBaseError> {
        // The plugin support check waits for the variables, which may replace the file's
        // plugins (`SB_PLUGINS=` turns them off) before it runs.
        let file = match path {
            Some(path) => Some(path.as_ref().to_path_buf()),
            None => Self::global_file_with(&var),
        };
        let file = file.map(|file| SettingsFile::read(&file)).transpose()?;
        // What the file refuses is met with what the variables refuse, in the order of the
        // option table, whichever source gave each, as `check` lists them. The file alone is
        // not checked for a window with one side: a variable may give the other side.
        let mut refused = Vec::new();
        let below = match file {
            Some(mut file) => {
                refused.extend(file.check());
                file.into_layer()
            }
            None => Layer::defaults(),
        };
        let (settings, from_variables) = Self::over(
            &below,
            None,
            EnvVars::new("environment variable", var).read(),
        )?;
        refused.extend(from_variables);
        Self::finish(settings, refused)
    }

    /// Reads the settings as [`load_from`](Self::load_from) does (the file `path`, or the
    /// global file when `None`, then the `SB_*` variables), and reports everything it
    /// would refuse, not only the first thing: the file that cannot be read, each variable
    /// that does not parse, then the browser, the mode and the window size, and the plugins.
    ///
    /// A part that cannot be read is left at its default, so the rest is still checked and
    /// the returned settings are the ones a program would run with if it ignored the
    /// problems. Nothing is started.
    pub fn check<P: AsRef<Path>>(path: Option<P>) -> SettingsCheck {
        Self::check_with(path, |name| std::env::var(name).ok())
    }

    /// [`check`](Self::check) with the variables taken from `var`.
    pub fn check_with<P: AsRef<Path>>(
        path: Option<P>,
        var: impl Fn(&str) -> Option<String>,
    ) -> SettingsCheck {
        let file = match &path {
            Some(path) => Some(path.as_ref().to_path_buf()),
            None => Self::global_file_with(&var),
        };
        // Each source is read on its own, and what a source refuses is left out of it, so that
        // the rest of it is still read and checked.
        let mut refused: Vec<Refused> = Vec::new();
        let below = match &file {
            Some(file) => match SettingsFile::read(file) {
                Ok(mut read) => {
                    refused.extend(read.check());
                    read.into_layer()
                }
                // A file that cannot be read at all is left out whole.
                Err(error) => {
                    refused.push(Refused {
                        row: 0,
                        problem: SettingsProblem {
                            source: file.display().to_string(),
                            error,
                        },
                    });
                    Layer::defaults()
                }
            },
            None => Layer::defaults(),
        };
        let env = EnvVars::new("environment variable", var).read();
        let (settings, from_variables) = Self::over(&below, None, env).unwrap_or_else(|error| {
            let problem = SettingsProblem {
                source: "settings".to_owned(),
                error,
            };
            (
                Self::default(),
                vec![Refused {
                    row: usize::MAX,
                    problem,
                }],
            )
        });
        refused.extend(from_variables);
        // The problems of the file and of the variables run in the order of the option table,
        // whichever source gave each.
        refused.sort_by_key(|refused| refused.row);
        let mut problems: Vec<SettingsProblem> =
            refused.into_iter().map(|refused| refused.problem).collect();
        if let Err(error) = settings.validate() {
            problems.push(SettingsProblem {
                source: "validation".to_owned(),
                error,
            });
        }
        if let Err(error) = settings.plugins.ensure_supported() {
            problems.push(SettingsProblem {
                source: "plugins".to_owned(),
                error,
            });
        }
        if let Err(error) = settings.check_fingerprint() {
            problems.push(SettingsProblem {
                source: "fingerprint".to_owned(),
                error,
            });
        }
        SettingsCheck {
            file,
            settings,
            problems,
        }
    }

    /// Converts these settings into a `BrowserConfig` used by `BaseCase`.
    ///
    /// Settings that came from a file or the environment are checked already.
    /// For one built by hand, call [`validate`](Self::validate) first: a
    /// window with one side only is dropped here.
    pub fn to_browser_config(&self) -> BrowserConfig {
        self.apply_process_wide();
        let mut config = BrowserConfig {
            browser: self.browser.clone(),
            session: self.session.clone(),
            ..BrowserConfig::default()
        };

        // A multiplier below the minimum is raised to it, as `with_timeout_multiplier` does.
        let mut run = self.run.clone();
        if let Some(multiplier) = run.timeout_multiplier {
            run = run.with_timeout_multiplier(multiplier);
        }
        config.run = run;

        // What the settings mean beyond their values: `--headless1` is headless, a size with a
        // side of zero (a lone `SB_WINDOW_WIDTH`) is no size, and sharing a browser per module
        // is sharing browsers.
        if config.browser.has(LaunchSwitch::Headless1) {
            config.browser.headless = true;
        }
        if config
            .browser
            .window_size
            .is_some_and(|size| size.width == 0 || size.height == 0)
        {
            config.browser.window_size = None;
        }
        // A phone screen is a phone: `BrowserConfig::with_device_metrics` turns mobile
        // emulation on too.
        if config.browser.device_metrics.is_some() {
            config.browser.mobile = true;
        }

        config.session.reuse = self.session.reuse || self.session.reuse_class.is_some();

        // A file can hold any string for `env`; the variable is checked as it is read.
        config.test_data = self.test.data.clone();
        if let Some(env) = &self.test.data.env {
            match TestData::parse_env(env) {
                Ok(env) => config.test_data.env = Some(env),
                Err(error) => {
                    // A `Settings` that was read is checked already; this is one made by hand.
                    tracing::warn!(%error, "the test data of the settings is ignored");
                    config.test_data = TestData::default();
                }
            }
        }
        config.fingerprint = self.load_fingerprint().unwrap_or_else(|error| {
            // A `Settings` that was read is checked already; this is one made by hand.
            tracing::warn!(%error, "the fingerprint file of the settings is ignored");
            None
        });
        config
    }

    /// The settings of the layer `below` with the variables `env` laid over them, and the
    /// variables that are refused (named by their variable and flag): a refused variable is left
    /// out and the rest is read. `file` is the settings file the settings already name.
    fn over(
        below: &Layer,
        file: Option<PathBuf>,
        env: Read,
    ) -> Result<(Self, Vec<Refused>), SeleniumBaseError> {
        let Read {
            layer,
            refused,
            plugins_named,
            settings_file,
        } = env;

        let mut settings = layers::extract(&[below, &layer])
            .map_err(|error| SeleniumBaseError::InvalidConfig(layers::describe(&error)))?;
        if plugins_named {
            // The variable names the plugins, so the options of the others go with them.
            settings.plugins.prune_unused();
        }
        settings.settings_file = settings_file.or(file);
        Ok((settings, refused))
    }

    /// `settings` if no value was refused, else the error of the first one in the order of the
    /// option table, whichever source gave it (the order `check` lists them in); then the checks
    /// of the whole: the window size, the plugins this build can run, and the fingerprint file.
    fn finish(settings: Self, mut refused: Vec<Refused>) -> Result<Self, SeleniumBaseError> {
        // The first in the order of the option table, whichever source gave it.
        refused.sort_by_key(|refused| refused.row);
        if let Some(refused) = refused.into_iter().next() {
            return Err(refused.problem.error);
        }
        settings.validate()?;
        // Plugins that cannot run are an error here, not a silent omission.
        settings.plugins.ensure_supported()?;
        // A fingerprint file that cannot be read is too, not a browser without it.
        settings.check_fingerprint()?;
        Ok(settings)
    }

    /// Applies the `SB_*` overrides that `var` reports on top of `settings`.
    ///
    /// `var` looks a variable up by name, so the overrides can come from the
    /// process environment or from any other source. A blank value (empty or
    /// whitespace only, as a CI matrix exports for an unset entry) counts as
    /// unset, as `SB_LOG_LEVEL` does in `RuntimeConfig`; `SB_PLUGINS` is the
    /// exception, where an empty value is an answer: no plugin at all.
    fn apply_overrides(
        settings: Self,
        var: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, SeleniumBaseError> {
        let (settings, problems) = Self::apply_overrides_lenient(settings, var);
        Self::finish(
            settings,
            problems
                .into_iter()
                .map(|problem| Refused { row: 0, problem })
                .collect(),
        )
    }

    /// Writes options given as `SB_*` variables and the text of their values, the way
    /// [`load`](Self::load) writes the environment: the pairs of the same variable are read
    /// last one first, a blank value counts as unset, and a pair whose variable no option of
    /// the table reads (the shorthands) is ignored. `SB_PLUGINS` is read too, and names the
    /// plugins as the variable does: its list replaces the selection and drops the options of
    /// the plugins it leaves out, and an empty value turns them all off. The result is checked with
    /// [`validate`](Self::validate).
    ///
    /// This is how the flags of `sbrs` that have no field of their own in
    /// [`SettingsOverrides`] reach the settings.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a value that does not parse (naming
    /// its variable) and for a result that is not valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::browser::options::LaunchSwitch;
    /// use seleniumbase_rs::Settings;
    ///
    ///
    /// let mut settings = Settings::default();
    /// settings.apply_options(&[
    ///     ("SB_DEVICE_METRICS", "360,640,2".to_owned()),
    ///     ("SB_BLOCK_IMAGES", "true".to_owned()),
    /// ])?;
    /// assert!(settings.browser.has(LaunchSwitch::BlockImages));
    /// assert!(settings.browser.device_metrics.is_some());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn apply_options(&mut self, options: &[(&str, String)]) -> Result<(), SeleniumBaseError> {
        let (laid, problems) = Self::lay(std::mem::take(self), EnvVars::pairs(options).read());
        *self = laid;
        if let Some(problem) = problems.into_iter().next() {
            return Err(problem.error);
        }
        self.validate()
    }

    /// [`apply_overrides`](Self::apply_overrides) up to the checks: a variable that does
    /// not parse is left out and comes back in the list, in the order the variables are
    /// read, instead of ending the function. Nothing is validated.
    fn apply_overrides_lenient(
        settings: Self,
        var: impl Fn(&str) -> Option<String>,
    ) -> (Self, Vec<SettingsProblem>) {
        let env = EnvVars::new("environment variable", var).read();
        Self::lay(settings, env)
    }

    /// `settings` with the variables `env` laid over them, as [`over`](Self::over) does; the
    /// variables that are refused come back as problems, in the order of the option table.
    fn lay(settings: Self, env: Read) -> (Self, Vec<SettingsProblem>) {
        let laid = Layer::of_settings(&settings)
            .and_then(|below| Self::over(&below, settings.settings_file.clone(), env));
        match laid {
            Ok((laid, refused)) => (
                laid,
                refused.into_iter().map(|refused| refused.problem).collect(),
            ),
            Err(error) => (
                settings,
                vec![SettingsProblem {
                    source: "settings".to_owned(),
                    error,
                }],
            ),
        }
    }
}

/// What [`Settings::check`] found.
#[derive(Debug)]
pub struct SettingsCheck {
    /// The settings file that was read: the one named, or the one found in the current
    /// folder. `None` when there is none.
    pub file: Option<PathBuf>,
    /// The settings: the file's, with the variables on top. A part that could not be read
    /// is at its default.
    pub settings: Settings,
    /// Everything [`Settings::load_from`] would refuse, in the order of the option table
    /// whichever source gave each (the first is the error `load_from` returns); empty
    /// when it would succeed.
    pub problems: Vec<SettingsProblem>,
}

/// Something wrong with the settings, and where it came from.
#[derive(Debug)]
pub struct SettingsProblem {
    /// The settings file, an `SB_*` variable, or `validation` or `plugins` for a check of the
    /// result.
    pub source: String,
    /// What is wrong.
    pub error: SeleniumBaseError,
}

/// The error of `problem`, saying which variable (and which flag) it came from when its text
/// does not: `SB_HEADLESS (--headless): cannot parse bool: maybe`.
pub(crate) fn named(problem: SettingsProblem) -> SeleniumBaseError {
    let SettingsProblem { source, error } = problem;
    let SeleniumBaseError::InvalidConfig(text) = &error else {
        return error;
    };
    if !source.starts_with("SB_") || text.contains(&source) {
        return error;
    }
    let flag = crate::config::test_options::TEST_OPTIONS
        .iter()
        .find(|row| row.env == source && row.kind.is_flag() && row.kind.reads_env())
        .map_or_else(String::new, |row| format!(" (--{})", row.long));
    SeleniumBaseError::InvalidConfig(format!("{source}{flag}: {text}"))
}

pub(crate) fn parse_bool(value: &str) -> Result<bool, SeleniumBaseError> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(SeleniumBaseError::InvalidConfig(format!(
            "cannot parse bool: {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::config::{Browser, DriverMode};
    use crate::browser::options::WindowSize;
    use crate::browser::reuse::ReuseClass;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn a_refused_variable_is_named_with_its_flag() {
        let refused = |name: &'static str, value: &str| {
            Settings::default()
                .apply_options(&[(name, value.to_owned())])
                .unwrap_err()
                .to_string()
        };

        let error = refused("SB_HEADLESS", "maybe");
        assert!(
            error.contains("SB_HEADLESS (--headless): cannot parse bool: maybe"),
            "{error}"
        );
        // A variable with no flag of its own is named alone.
        let error = refused("SB_TIMEOUT", "soon");
        assert!(error.contains("SB_TIMEOUT: "), "{error}");
        // A name or a mode is checked after the settings are read, and says where it was set.
        let error = refused("SB_BROWSER", "chrom");
        assert!(error.contains("SB_BROWSER"), "{error}");
        let error = refused("SB_MODE", "stealth");
        assert!(error.contains("SB_MODE"), "{error}");
    }

    #[test]
    fn default_settings_are_sensible() {
        let s = Settings::default();
        assert_eq!(s.browser.kind, Browser::Chrome);
        assert!(!s.browser.headless);
        assert_eq!(s.run, RunSettings::default());
        assert!(s.browser.switches.is_empty());
        assert_eq!(s.browser.window_size, None);
        assert!(s.browser.proxy.server.is_none());
    }

    /// The settings a TOML settings file with the text `text` gives.
    fn settings_of(text: &str) -> Settings {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(text.as_bytes()).unwrap();
        Settings::from_file(file.path()).unwrap()
    }

    fn with_vars(
        pairs: &'static [(&'static str, &'static str)],
    ) -> Result<Settings, SeleniumBaseError> {
        Settings::apply_overrides(Settings::default(), |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn the_webdriver_url_and_the_browser_binary_reach_the_browser_config() {
        let defaults = Settings::default().to_browser_config();
        assert_eq!(defaults.session.webdriver_url(), "http://localhost:4444");
        assert_eq!(defaults.browser.executable_path, None);

        let config = with_vars(&[
            ("SB_WEBDRIVER_URL", "http://grid.test:4444"),
            ("SB_CHROME_BIN", "/opt/chrome/chrome"),
        ])
        .unwrap()
        .to_browser_config();
        assert_eq!(config.session.webdriver_url(), "http://grid.test:4444");
        assert_eq!(
            config.browser.executable_path,
            Some(PathBuf::from("/opt/chrome/chrome"))
        );
    }

    #[test]
    fn the_window_size_reaches_the_launch_settings_when_both_sides_are_set() {
        assert_eq!(
            Settings::default().to_browser_config().browser.window_size,
            None
        );
        let config = with_vars(&[("SB_WINDOW_WIDTH", "1280"), ("SB_WINDOW_HEIGHT", "720")])
            .unwrap()
            .to_browser_config();
        assert_eq!(config.browser.window_size, Some(WindowSize::new(1280, 720)));
    }

    #[test]
    fn a_lone_or_empty_window_dimension_is_an_error() {
        assert!(with_vars(&[("SB_WINDOW_WIDTH", "1280")]).is_err());
        assert!(with_vars(&[("SB_WINDOW_WIDTH", "0"), ("SB_WINDOW_HEIGHT", "720")]).is_err());
    }

    #[test]
    fn the_timeout_reaches_the_run_settings() {
        assert_eq!(
            Settings::default()
                .to_browser_config()
                .run_settings()
                .default_timeout(),
            None
        );
        let config = with_vars(&[("SB_TIMEOUT", "5")])
            .unwrap()
            .to_browser_config();
        assert_eq!(
            config.run_settings().default_timeout(),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn a_blank_variable_counts_as_unset() {
        let settings = with_vars(&[
            ("SB_MODE", ""),
            ("SB_BROWSER", "  "),
            ("SB_CHROME_BIN", ""),
            ("SB_WEBDRIVER_URL", ""),
            ("SB_TIMEOUT", ""),
        ])
        .unwrap();
        assert_eq!(settings.session.mode, None);
        assert_eq!(settings.browser.kind, Browser::Chrome);
        assert_eq!(settings.browser.executable_path, None);
        assert_eq!(settings.session.webdriver_url, None);
        assert_eq!(settings.run.default_timeout, None);
    }

    #[test]
    fn an_unknown_browser_or_mode_is_an_error_not_chrome() {
        let browser = with_vars(&[("SB_BROWSER", "chrom")]).unwrap_err();
        assert!(browser.to_string().contains("chrom"), "{browser}");
        let mode = with_vars(&[("SB_MODE", "stealth")]).unwrap_err();
        assert!(mode.to_string().contains("stealth"), "{mode}");
        assert!(with_vars(&[("SB_BROWSER", "Firefox"), ("SB_MODE", "UC")]).is_ok());

        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"browser = \"chrom\"\n").unwrap();
        assert!(Settings::from_file(file.path()).is_err());
    }

    #[test]
    fn remote_webdriver_timeout_defaults_to_unset() {
        // SeleniumBase's REMOTE_WEBDRIVER_TIMEOUT defaults to None, leaving the
        // client's own timeout in place.
        assert!(Settings::default()
            .session
            .remote_webdriver_timeout_seconds
            .is_none());
        assert!(Settings::default()
            .to_browser_config()
            .session
            .remote_webdriver_timeout_seconds
            .is_none());
    }

    #[test]
    fn remote_webdriver_timeout_reaches_the_browser_config() {
        let mut settings = Settings::default();
        settings.session.remote_webdriver_timeout_seconds = Some(420);
        assert_eq!(
            settings
                .to_browser_config()
                .session
                .remote_webdriver_timeout_seconds,
            Some(420)
        );
    }

    #[test]
    fn remote_webdriver_timeout_round_trips_through_json() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(br#"{"remote_webdriver_timeout_seconds":600}"#)
            .unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.session.remote_webdriver_timeout_seconds, Some(600));
    }

    #[test]
    fn settings_from_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let json = r#"{"browser":"firefox","headless":true,"timeout_seconds":60,"proxy":"http://proxy:8080","window_width":1280,"window_height":720,"user_data_dir":null}"#;
        tmp.write_all(json.as_bytes()).unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.browser.kind, Browser::Firefox);
        assert!(s.browser.headless);
        assert_eq!(s.run.default_timeout, Some(Duration::from_secs(60)));
        assert!(!s.test.pdb);
        assert_eq!(
            s.browser.proxy.server,
            Some("http://proxy:8080".to_string())
        );
        assert_eq!(s.browser.window_size, Some(WindowSize::new(1280, 720)));
        assert_eq!(s.browser.user_data_dir, None);
    }

    #[test]
    fn settings_from_toml() {
        let mut tmp = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        let toml = r#"
    browser = "edge"
    headless = true
    proxy = "http://proxy:8080"
    mode = "uc"
    mobile = true
    threads = 4
    "#;
        tmp.write_all(toml.as_bytes()).unwrap();
        let s = Settings::from_file(tmp.path()).unwrap();
        assert_eq!(s.browser.kind, Browser::Edge);
        assert!(s.browser.headless);
        assert_eq!(
            s.browser.proxy.server,
            Some("http://proxy:8080".to_string())
        );
        assert_eq!(s.session.mode, Some(DriverMode::Uc));
        assert!(s.browser.mobile);
        assert_eq!(s.test.threads, Some(4));
    }

    #[test]
    fn to_browser_config_maps_fields() {
        let mut s = Settings::default();
        s.browser.kind = Browser::Firefox;
        s.browser.headless = true;
        s.session.mode = Some(DriverMode::Cdp);
        s.browser.mobile = true;
        s.test.threads = Some(2);
        s.browser.proxy.pac_url = Some("http://proxy/proxy.pac".to_string());
        let config = s.to_browser_config();
        assert!(matches!(config.browser.kind, Browser::Firefox));
        assert!(config.browser.headless);
        assert!(matches!(config.session.mode, Some(DriverMode::Cdp)));
        assert!(config.browser.mobile);

        assert_eq!(
            config.browser.proxy.pac_url,
            Some("http://proxy/proxy.pac".to_string())
        );
    }

    /// A folder holding a fingerprint file, and the path of the file.
    fn fingerprint_file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fp.json");
        Fingerprint::windows_desktop().save(&path).unwrap();
        (dir, path)
    }

    fn every_option(fingerprint: &Path) -> SettingsOverrides {
        let pair = |variable: &'static str, value: &str| (variable, value.to_owned());
        SettingsOverrides {
            options: vec![
                pair(FINGERPRINT_ENV, &fingerprint.display().to_string()),
                pair("SB_BROWSER", "firefox"),
                pair("SB_HEADLESS", "true"),
                pair("SB_MODE", "uc"),
                pair("SB_USER_AGENT", "TestAgent/1.0"),
                pair("SB_LOCALE", "fr-FR"),
                pair("SB_AD_BLOCK", "true"),
                pair("SB_PROXY", "proxy.test:8080"),
                pair("SB_PROXY_PAC_URL", "http://proxy.test/proxy.pac"),
                pair("SB_USER_DATA_DIR", "/tmp/profile"),
                pair("SB_EXTENSION_DIR", "/tmp/ext"),
                pair("SB_REUSE_SESSION", "true"),
                pair("SB_REUSE_CLASS_SESSION", "true"),
                pair("SB_MOBILE", "true"),
                pair("SB_THREADS", "4"),
                pair("SB_ESC_END", "true"),
                pair("SB_REC_SB_CDP", "true"),
                pair(START_PAGE_ENV, "https://example.test/"),
            ],
        }
    }

    #[test]
    fn reuse_follows_its_variables_and_reaches_the_browser_config() {
        let off = Settings::default().to_browser_config();
        assert!(!off.session.reuse);
        assert_eq!(off.session.reuse_class, None);

        let run = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_REUSE_SESSION").then(|| "true".to_owned())
        })
        .unwrap()
        .to_browser_config();
        assert!(run.session.reuse);
        assert_eq!(run.session.reuse_class, None, "--rs shares across the run");

        let module = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_REUSE_CLASS_SESSION").then(|| "1".to_owned())
        })
        .unwrap()
        .to_browser_config();
        assert!(module.session.reuse, "--rcs implies --rs, as in Python");
        assert_eq!(module.session.reuse_class, Some(ReuseClass::TestModule));

        let from_file = settings_of("reuse_class_session = true");
        assert_eq!(from_file.session.reuse_class, Some(ReuseClass::TestModule));
        let only = SettingsOverrides {
            options: vec![("SB_REUSE_CLASS_SESSION", "true".to_owned())],
        };
        assert_eq!(only.env(), [("SB_REUSE_CLASS_SESSION", "true".to_owned())]);
    }

    #[test]
    fn overrides_that_are_set_replace_the_settings() {
        let (_dir, file) = fingerprint_file();
        let mut settings = Settings::default();
        every_option(&file).try_apply(&mut settings).unwrap();

        assert_eq!(settings.fingerprint.file.as_deref(), Some(file.as_path()));
        let (browser, session) = (&settings.browser, &settings.session);
        assert_eq!(browser.kind, Browser::Firefox);
        assert!(browser.headless && browser.ad_block && session.reuse && browser.mobile);
        assert_eq!(session.mode, Some(DriverMode::Uc));
        assert_eq!(browser.user_agent.as_deref(), Some("TestAgent/1.0"));
        assert_eq!(browser.lang.as_deref(), Some("fr-FR"));
        assert_eq!(browser.proxy.server.as_deref(), Some("proxy.test:8080"));
        assert_eq!(
            browser.proxy.pac_url.as_deref(),
            Some("http://proxy.test/proxy.pac")
        );
        assert_eq!(
            browser.user_data_dir.as_deref(),
            Some(Path::new("/tmp/profile"))
        );
        assert_eq!(browser.extensions, [PathBuf::from("/tmp/ext")]);
        assert_eq!(settings.test.threads, Some(4));
        assert!(browser.has(LaunchSwitch::EscEnd) && browser.has(LaunchSwitch::RecSbCdp));
        assert_eq!(browser.url.as_deref(), Some("https://example.test/"));
        assert_eq!(session.reuse_class, Some(ReuseClass::TestModule));
    }

    #[test]
    fn the_plugins_table_of_a_toml_file_reaches_the_settings() {
        let settings: Settings = toml::from_str(
            r#"
            [browser]

            headless = true

            [plugins]
            enable = ["shield", "crx"]

            [plugins.shield]
            level = "strict"
            "#,
        )
        .unwrap();

        assert!(settings.browser.headless);
        assert_eq!(settings.plugins.enabled(), ["shield", "crx"]);
        assert_eq!(
            settings.plugins.options_of("shield").unwrap()["level"],
            "strict"
        );
    }

    #[test]
    fn settings_without_plugins_write_no_plugins_key() {
        let text = toml::to_string(&Settings::default()).unwrap();
        assert!(!text.contains("plugins"), "{text}");
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert!(!json.contains("plugins"), "{json}");

        let with: Settings = toml::from_str("[plugins]\nenable = [\"shield\"]").unwrap();
        let round_trip: Settings = toml::from_str(&toml::to_string(&with).unwrap()).unwrap();
        assert_eq!(round_trip, with);

        // The options table comes after `enable` and the other keys on the way out.
        let with: Settings = toml::from_str(
            "headless = true\n[plugins]\nenable = [\"shield\"]\n[plugins.shield]\nlevel = \"strict\"\n",
        )
        .unwrap();
        let round_trip: Settings = toml::from_str(&toml::to_string(&with).unwrap()).unwrap();
        assert_eq!(round_trip, with);
        let json_trip: Settings =
            serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(json_trip, with);
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn sb_plugins_replaces_the_names_and_keeps_the_options_of_the_file() {
        let from_file: Settings =
            toml::from_str("[plugins]\nenable = [\"crx\"]\n[plugins.shield]\nlevel = \"strict\"\n")
                .unwrap();

        let settings = Settings::apply_overrides(from_file, |name| {
            (name == "SB_PLUGINS").then(|| "shield, block-resources".to_owned())
        })
        .unwrap();

        assert_eq!(settings.plugins.enabled(), ["shield", "block-resources"]);
        assert!(settings.plugins.options_of("shield").is_some());
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn an_sb_plugins_list_drops_the_options_of_the_names_it_no_longer_has() {
        let from_file: Settings = toml::from_str(
            "[plugins]\nenable = [\"shield\"]\n[plugins.shield]\nlevel = \"strict\"\n",
        )
        .unwrap();

        let off = Settings::apply_overrides(from_file.clone(), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
        assert!(off.plugins.is_empty(), "{:?}", off.plugins);

        let other = Settings::apply_overrides(from_file, |name| {
            (name == "SB_PLUGINS").then(|| "crx".to_owned())
        })
        .unwrap();
        assert_eq!(other.plugins.enabled(), ["crx"]);
        assert!(other.plugins.options_of("shield").is_none());
    }

    #[cfg(feature = "plugins")]
    #[test]
    fn an_empty_sb_plugins_turns_the_plugins_of_the_file_off() {
        let from_file: Settings = toml::from_str("[plugins]\nenable = [\"shield\"]\n").unwrap();

        let settings = Settings::apply_overrides(from_file.clone(), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
        assert!(settings.plugins.enabled().is_empty());

        let untouched = Settings::apply_overrides(from_file, |_| None).unwrap();
        assert_eq!(untouched.plugins.enabled(), ["shield"]);
    }

    #[test]
    fn the_plugin_table_is_read_whatever_the_rest_of_the_file_holds() {
        // Not a valid `Settings` file: `headless` is not a boolean, `browser` is no browser.
        let body = "headless = \"maybe\"\nbrowser = \"nope\"\n[plugins]\nenable = [\"shield\"]\n\
                    [plugins.shield]\nlevel = \"strict\"\n";
        let mut toml_file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        toml_file.write_all(body.as_bytes()).unwrap();
        assert!(
            Settings::from_file(toml_file.path()).is_err(),
            "the file as a whole is wrong"
        );

        let table = Settings::load_plugin_table(Some(toml_file.path())).unwrap();
        assert_eq!(table.enabled(), ["shield"]);
        assert_eq!(table.options_of("shield").unwrap()["level"], "strict");
        let selection = Settings::plugin_selection_with(Some(toml_file.path()), |_| None).unwrap();
        assert_eq!(selection, table);

        let mut json_file = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        json_file
            .write_all(br#"{"headless":"maybe","plugins":{"enable":["crx"]}}"#)
            .unwrap();
        let table = Settings::load_plugin_table(Some(json_file.path())).unwrap();
        assert_eq!(table.enabled(), ["crx"]);

        // A file that is not TOML at all still cannot be read.
        let mut broken = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        broken.write_all(b"[plugins\n").unwrap();
        let error = Settings::load_plugin_table(Some(broken.path())).unwrap_err();
        assert!(error.to_string().contains("failed to parse"), "{error}");
    }

    #[test]
    fn a_settings_file_without_a_plugins_table_selects_nothing() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"headless = true\n").unwrap();
        assert!(Settings::load_plugin_table(Some(file.path()))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_plugin_selection_alone_is_the_file_with_sb_plugins_on_top() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(
            b"headless = true\n[plugins]\nenable = [\"shield\", \"crx\"]\n\
              [plugins.shield]\nlevel = \"strict\"\n",
        )
        .unwrap();

        let from_file = Settings::plugin_selection_with(Some(file.path()), |_| None).unwrap();
        assert_eq!(from_file.enabled(), ["shield", "crx"]);
        assert_eq!(from_file.options_of("shield").unwrap()["level"], "strict");

        // The variable replaces the names and drops the options of the others;
        // another variable with a bad value is not read at all.
        let replaced = Settings::plugin_selection_with(Some(file.path()), |name| match name {
            "SB_PLUGINS" => Some("crx".to_owned()),
            "SB_HEADLESS" => Some("maybe".to_owned()),
            _ => None,
        })
        .unwrap();
        assert_eq!(replaced.enabled(), ["crx"]);
        assert!(replaced.options_of("shield").is_none());

        // An empty variable is an answer: no plugin at all.
        let off = Settings::plugin_selection_with(Some(file.path()), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
        assert!(off.is_empty(), "{off:?}");

        // A file that is not there is an error, as it is for `load_from`.
        let error =
            Settings::plugin_selection_with(Some("no-such-settings.toml"), |_| None).unwrap_err();
        assert!(error.to_string().contains("failed to read"), "{error}");
    }

    #[cfg(not(feature = "plugins"))]
    #[test]
    fn without_plugin_support_an_empty_sb_plugins_still_overrides_the_file() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"[plugins]\nenable = [\"shield\"]\n[plugins.shield]\nlevel = \"strict\"\n")
            .unwrap();

        let settings = Settings::load_with(Some(file.path()), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
        assert!(settings.plugins.is_empty());

        // Without the override the file's selection stops the load.
        let error = Settings::load_with(Some(file.path()), |_| None).unwrap_err();
        assert!(
            error.to_string().contains("--features plugins-builtin"),
            "{error}"
        );
    }

    #[cfg(not(feature = "plugins"))]
    #[test]
    fn without_plugin_support_a_selection_stops_the_load() {
        let hint = "--features plugins-builtin";
        let error = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_PLUGINS").then(|| "shield".to_owned())
        })
        .unwrap_err();
        assert!(error.to_string().contains(hint), "{error}");

        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"[plugins]\nenable = [\"shield\"]\n")
            .unwrap();
        let error = Settings::from_file(file.path()).unwrap_err();
        assert!(error.to_string().contains(hint), "{error}");

        // Asking for nothing is fine.
        Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_PLUGINS").then(String::new)
        })
        .unwrap();
    }

    fn variables(
        pairs: &'static [(&'static str, &'static str)],
    ) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn check_reports_every_bad_variable_and_keeps_the_good_ones() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"locale = \"de-DE\"\n").unwrap();

        let check = Settings::check_with(
            Some(file.path()),
            variables(&[
                ("SB_HEADLESS", "maybe"),
                ("SB_THREADS", "many"),
                ("SB_WINDOW_WIDTH", "wide"),
                ("SB_BROWSER", "chrom"),
                ("SB_MOBILE", "yes"),
            ]),
        );

        // A name that is wrong is refused when it is read, so the browser is a problem of
        // its variable, in the order of the table.
        let sources: Vec<&str> = check.problems.iter().map(|p| p.source.as_str()).collect();
        assert_eq!(
            sources,
            ["SB_BROWSER", "SB_HEADLESS", "SB_WINDOW_WIDTH", "SB_THREADS"]
        );
        assert!(check.problems[0].error.to_string().contains("chrom"));
        assert!(check.problems[1].error.to_string().contains("maybe"));
        assert!(check.problems[3]
            .error
            .to_string()
            .starts_with("invalid configuration: SB_THREADS:"));
        // The good variables and the file are kept.
        assert!(check.settings.browser.mobile);
        assert_eq!(check.settings.browser.lang.as_deref(), Some("de-DE"));
        assert_eq!(check.file.as_deref(), Some(file.path()));
    }

    #[test]
    fn check_finds_nothing_in_good_settings_and_load_agrees() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"browser = \"edge\"\nthreads = 2\n")
            .unwrap();
        let pairs: &'static [(&str, &str)] = &[("SB_HEADLESS", "true"), ("SB_TIMEOUT", "7")];

        let check = Settings::check_with(Some(file.path()), variables(pairs));
        let loaded = Settings::load_with(Some(file.path()), variables(pairs)).unwrap();

        assert!(check.problems.is_empty(), "{:?}", check.problems);
        assert_eq!(check.settings, loaded);
    }

    #[test]
    fn check_names_a_file_that_cannot_be_read_and_still_checks_the_variables() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"headless = \"maybe\"\n").unwrap();

        let broken = Settings::check_with(Some(file.path()), variables(&[("SB_THREADS", "x")]));
        let missing = Settings::check_with(Some("no-such-settings.toml"), variables(&[]));

        let sources: Vec<&str> = broken.problems.iter().map(|p| p.source.as_str()).collect();
        assert_eq!(
            sources,
            [file.path().display().to_string().as_str(), "SB_THREADS"]
        );
        assert!(broken.problems[0]
            .error
            .to_string()
            .contains("failed to parse TOML settings"));
        assert_eq!(missing.problems.len(), 1);
        assert!(missing.problems[0]
            .error
            .to_string()
            .contains("failed to read"));
    }

    #[test]
    fn the_first_problem_check_reports_is_the_error_load_returns() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"browser = \"firefox\"\n").unwrap();
        for pairs in [
            &[("SB_HEADLESS", "maybe"), ("SB_THREADS", "x")][..],
            &[("SB_BROWSER", "chrom")][..],
            &[("SB_WINDOW_WIDTH", "800")][..],
        ] {
            let pairs: &'static [(&str, &str)] = Box::leak(pairs.to_vec().into_boxed_slice());
            let check = Settings::check_with(Some(file.path()), variables(pairs));
            let error = Settings::load_with(Some(file.path()), variables(pairs)).unwrap_err();
            assert_eq!(
                check.problems[0].error.to_string(),
                error.to_string(),
                "{pairs:?}"
            );
        }
    }

    /// The settings of a TOML settings file with the text `file` and the variables `vars` over
    /// it, read by `load` and by `check`, which must agree.
    fn layered(file: &str, vars: &'static [(&'static str, &'static str)]) -> Settings {
        let mut tmp = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        tmp.write_all(file.as_bytes()).unwrap();
        let check = Settings::check_with(Some(tmp.path()), variables(vars));
        assert!(check.problems.is_empty(), "{:?}", check.problems);
        let loaded = Settings::load_with(Some(tmp.path()), variables(vars)).unwrap();
        assert_eq!(check.settings, loaded);
        loaded
    }

    #[test]
    fn the_display_and_its_size_come_from_whichever_source_gives_each() {
        use crate::browser::options::VirtualDisplay;
        let big = Some(VirtualDisplay::new(1920, 1080));
        let display = |file: &str, vars| layered(file, vars).browser.virtual_display;
        // The switch in the file and the size in a variable, and the other way round.
        assert_eq!(
            display("xvfb = true", &[("SB_XVFB_METRICS", "1920,1080")]),
            big
        );
        assert_eq!(
            display("xvfb_metrics = \"1920,1080\"", &[("SB_XVFB", "true")]),
            big
        );
        // A size with no switch opens no display.
        assert_eq!(display("xvfb_metrics = \"1920,1080\"", &[]), None);
        assert_eq!(display("", &[("SB_XVFB_METRICS", "1920,1080")]), None);
        // A higher source that repeats the switch leaves the size of a lower one, and one that
        // turns the switch off closes the display.
        let both = "xvfb = true\nxvfb_metrics = \"1920,1080\"\n";
        assert_eq!(display(both, &[("SB_XVFB", "true")]), big);
        assert_eq!(display(both, &[("SB_XVFB", "false")]), None);
        // The size of the higher source wins.
        assert_eq!(
            display(both, &[("SB_XVFB_METRICS", "800,600")]),
            Some(VirtualDisplay::new(800, 600))
        );
    }

    #[test]
    fn the_event_capture_is_worked_out_from_both_variables_of_all_the_sources() {
        use crate::browser::options::CdpEventCapture::{Always, Off, UndetectedOnly};
        let capture = |file: &str, vars| layered(file, vars).session.cdp_events;
        // `SB_UC_CDP_EVENTS` never lowers the capture of every session, whichever source says so.
        assert_eq!(
            capture("log_cdp_events = true", &[("SB_UC_CDP_EVENTS", "false")]),
            Always
        );
        // Turning the capture of every session off leaves the undetected-only one alone.
        assert_eq!(
            capture("uc_cdp_events = true", &[("SB_LOG_CDP_EVENTS", "false")]),
            UndetectedOnly
        );
        assert_eq!(
            capture("log_cdp_events = true", &[("SB_LOG_CDP_EVENTS", "false")]),
            Off
        );
        assert_eq!(
            capture("uc_cdp_events = true", &[("SB_LOG_CDP_EVENTS", "true")]),
            Always
        );
        assert_eq!(
            capture(
                "log_cdp_events = true\nuc_cdp_events = true\n",
                &[("SB_LOG_CDP_EVENTS", "false")]
            ),
            UndetectedOnly
        );
    }

    #[test]
    fn the_chromium_features_follow_the_variable_that_names_them() {
        let chromium = |file: &str, vars| layered(file, vars).browser.chromium;
        // A list of switches replaces the lower one's, and the features named in it with it.
        let over = chromium(
            "chromium_arg = \"--a,disable-features=A\"",
            &[("SB_CHROMIUM_ARG", "--y")],
        );
        assert_eq!(over.args, ["--y"]);
        assert!(over.disable_features.is_empty(), "{over:?}");
        // The features of `SB_DISABLE_FEATURES` are not the switches' own.
        let kept = chromium("disable_features = \"X\"", &[("SB_CHROMIUM_ARG", "--y")]);
        assert_eq!(kept.args, ["--y"]);
        assert_eq!(kept.disable_features, ["X"]);
        // The two variables name features together; one variable replaces its own kind.
        let mut both = chromium(
            "chromium_arg = \"disable-features=A\"",
            &[("SB_DISABLE_FEATURES", "B")],
        )
        .disable_features;
        both.sort();
        assert_eq!(both, ["A", "B"]);
        let replaced = chromium("disable_features = \"X\"", &[("SB_DISABLE_FEATURES", "B")]);
        assert_eq!(replaced.disable_features, ["B"]);
    }

    #[test]
    fn a_pair_works_out_the_display_and_the_events_over_the_settings_it_is_laid_on() {
        use crate::browser::options::{CdpEventCapture, VirtualDisplay};
        let pair = |variable: &'static str, value: &str| (variable, value.to_owned());
        let mut settings = Settings::default();
        settings.browser.virtual_display = Some(VirtualDisplay::default());
        settings
            .apply_options(&[pair("SB_XVFB_METRICS", "1920,1080")])
            .unwrap();
        let big = Some(VirtualDisplay::new(1920, 1080));
        assert_eq!(settings.browser.virtual_display, big);
        // The switch again does not forget the size the settings already hold.
        settings.apply_options(&[pair("SB_XVFB", "true")]).unwrap();
        assert_eq!(settings.browser.virtual_display, big);
        settings.session.cdp_events = CdpEventCapture::Always;
        settings
            .apply_options(&[pair("SB_UC_CDP_EVENTS", "false")])
            .unwrap();
        assert_eq!(settings.session.cdp_events, CdpEventCapture::Always);
    }

    #[test]
    fn a_pair_for_sb_plugins_names_the_plugins_as_the_variable_does() {
        let mut settings = Settings::default();
        settings.plugins.set_enabled_from_list("first,second");
        settings
            .apply_options(&[("SB_PLUGINS", "shield".to_owned())])
            .unwrap();
        assert_eq!(settings.plugins.enabled(), ["shield"]);
        // An empty value is an answer: no plugin at all.
        settings
            .apply_options(&[("SB_PLUGINS", String::new())])
            .unwrap();
        assert!(settings.plugins.enabled().is_empty());
    }

    #[test]
    fn load_meets_the_problems_of_the_file_and_of_the_variables_in_the_order_check_lists_them() {
        // The refused value of the file is at a later row than the refused variable.
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"window_width = \"wide\"\n").unwrap();
        let pairs: &'static [(&str, &str)] = &[("SB_HEADLESS", "maybe")];
        let check = Settings::check_with(Some(file.path()), variables(pairs));
        let error = Settings::load_with(Some(file.path()), variables(pairs)).unwrap_err();
        assert_eq!(check.problems.len(), 2, "{:?}", check.problems);
        assert_eq!(check.problems[0].source, "SB_HEADLESS");
        assert_eq!(check.problems[0].error.to_string(), error.to_string());
    }

    #[test]
    fn a_lone_window_side_of_the_file_is_completed_by_a_variable() {
        let size = layered("window_width = 800\n", &[("SB_WINDOW_HEIGHT", "600")])
            .browser
            .window_size;
        assert_eq!(size, Some(WindowSize::new(800, 600)));
        // With nothing to complete it, it is refused by both.
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"window_width = 800\n").unwrap();
        let check = Settings::check_with(Some(file.path()), variables(&[]));
        let error = Settings::load_with(Some(file.path()), variables(&[])).unwrap_err();
        assert_eq!(check.problems[0].error.to_string(), error.to_string());
    }

    #[test]
    fn laying_no_option_over_loaded_settings_changes_nothing() {
        let mut settings = settings_of(
            r#"
            browser = "firefox"
            headless = true
            proxy = "proxy.test:8080"
            proxy_bypass_list = "a.test,b.test"
            window_size = "800,600"
            window_position = "10,20"
            device_metrics = "360,640,2"
            locale = "fr-FR"
            user_agent = "agent"
            extension_dir = "/a,/b"
            chromium_arg = "--mute-audio,disable-features=Foo"
            disable_features = "Bar"
            host_resolver_rules = "MAP a.test 127.0.0.1"
            xvfb = true
            xvfb_metrics = "1920,1080"
            remote_debug = true
            dark_mode = true
            block_images = true
            mode = "cdp"
            server = "hub.test"
            port = 4445
            protocol = "https"
            page_load_strategy = "eager"
            cap_string = '{"acceptInsecureCerts": true}'
            firefox_arg = "-private"
            log_cdp_events = true
            reuse_class_session = true
            crumbs = true
            threads = 3
            timeout_seconds = 12
            demo = true

            [report]
            dashboard = true
            dash_title = "Title"
            "#,
        );
        settings.plugins.set_enabled_from_list("shield");
        settings.test.data.account = Some("account".to_owned());
        let before = settings.clone();
        settings.apply_options(&[]).unwrap();
        assert_eq!(settings, before);
    }

    #[test]
    fn a_refused_value_of_a_table_is_not_repeated_in_the_error() {
        // The value may be a secret: the error names the file and the key and leaves it out,
        // as it does for a flat key.
        for (file, key) in [
            ("[run]\ndemo_mode = \"hunter2\"\n", "demo_mode"),
            ("[test]\nthreads = \"hunter2\"\n", "threads"),
            ("[report]\ndashboard = \"hunter2\"\n", "dashboard"),
        ] {
            let mut tmp = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
            tmp.write_all(file.as_bytes()).unwrap();
            let error = Settings::from_file(tmp.path()).unwrap_err().to_string();
            assert!(error.contains(key), "{error}");
            assert!(!error.contains("hunter2"), "{error}");
            let check = Settings::check_with(Some(tmp.path()), variables(&[]));
            assert_eq!(check.problems.len(), 1, "{:?}", check.problems);
            let error = check.problems[0].error.to_string();
            assert!(error.contains(key), "{error}");
            assert!(!error.contains("hunter2"), "{error}");
        }
    }

    #[test]

    fn a_driver_mode_that_was_asked_for_is_told_from_none_even_when_it_is_webdriver() {
        // `sbrs` gives a command its own mode only when none was asked for, so asking for
        // WebDriver must not read as asking for nothing.
        assert_eq!(Settings::default().session.mode, None);
        assert_eq!(with_vars(&[("SB_MODE", "  ")]).unwrap().session.mode, None);
        assert_eq!(
            with_vars(&[("SB_MODE", "webdriver")]).unwrap().session.mode,
            Some(DriverMode::WebDriver)
        );
        assert_eq!(
            settings_of("mode = \"webdriver\"").session.mode,
            Some(DriverMode::WebDriver)
        );
        assert_eq!(settings_of("headless = true").session.mode, None);
        let mut settings = Settings::default();
        settings
            .apply_options(&[("SB_MODE", "webdriver".to_owned())])
            .unwrap();
        assert_eq!(settings.session.mode, Some(DriverMode::WebDriver));
        assert_eq!(settings.session.driver_mode(), DriverMode::WebDriver);
        assert_eq!(
            Settings::default().session.driver_mode(),
            DriverMode::WebDriver
        );
    }

    #[test]
    fn rec_sb_cdp_is_off_by_default_and_follows_its_variable_and_key() {
        assert!(!Settings::default().browser.has(LaunchSwitch::RecSbCdp));
        assert!(!Settings::default().to_browser_config().rec_sb_cdp());
        let settings = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_REC_SB_CDP").then(|| "yes".to_owned())
        })
        .unwrap();
        assert!(settings.browser.has(LaunchSwitch::RecSbCdp));
        assert!(settings.to_browser_config().rec_sb_cdp());
        assert!(!settings.to_browser_config().esc_end());
        let from_file = settings_of("rec_sb_cdp = true");
        assert!(from_file.browser.has(LaunchSwitch::RecSbCdp));
        let only = SettingsOverrides {
            options: vec![("SB_REC_SB_CDP", "true".to_owned())],
        };
        assert_eq!(only.env(), [("SB_REC_SB_CDP", "true".to_owned())]);
    }

    #[test]
    fn esc_end_is_off_by_default_and_reaches_the_browser_config() {
        assert!(!Settings::default().browser.has(LaunchSwitch::EscEnd));
        assert!(!Settings::default().to_browser_config().esc_end());
        let mut settings = Settings::default();
        settings.browser.set(LaunchSwitch::EscEnd, true);
        assert!(settings.to_browser_config().esc_end());
    }

    #[test]
    fn the_esc_end_variable_and_the_config_file_key_turn_it_on() {
        for (value, expected) in [("true", true), ("1", true), ("off", false)] {
            let settings = Settings::apply_overrides(Settings::default(), |name| {
                (name == "SB_ESC_END").then(|| value.to_owned())
            })
            .unwrap();
            assert_eq!(
                settings.browser.has(LaunchSwitch::EscEnd),
                expected,
                "{value}"
            );
        }
        assert!(Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_ESC_END").then(|| "maybe".to_owned())
        })
        .is_err());

        assert!(settings_of("esc_end = true")
            .browser
            .has(LaunchSwitch::EscEnd));
        let old: Settings =
            serde_json::from_value(serde_json::json!({"browser": {"kind": Browser::Edge}}))
                .unwrap();
        assert!(
            !old.browser.has(LaunchSwitch::EscEnd),
            "a file without the key leaves it off"
        );
    }

    #[test]
    fn the_esc_end_override_is_exported_as_its_variable() {
        let only = SettingsOverrides {
            options: vec![("SB_ESC_END", "true".to_owned())],
        };
        assert_eq!(only.env(), [("SB_ESC_END", "true".to_owned())]);
    }

    #[test]
    fn overrides_that_are_not_set_leave_the_settings_alone() {
        let mut before = Settings::default();
        before.browser.kind = Browser::Edge;
        before.browser.headless = true;
        before.browser.proxy.server = Some("from-config:1".to_owned());
        before.test.threads = Some(2);
        let mut after = before.clone();

        SettingsOverrides::default().try_apply(&mut after).unwrap();

        assert_eq!(after, before);
    }

    #[test]
    fn an_explicit_false_beats_a_true_in_the_settings() {
        let mut settings = Settings::default();
        settings.browser.headless = true;
        SettingsOverrides {
            options: vec![("SB_HEADLESS", "false".to_owned())],
        }
        .try_apply(&mut settings)
        .unwrap();
        assert!(!settings.browser.headless);
    }

    #[test]
    fn the_environment_form_means_the_same_as_applying_directly() {
        let (_dir, file) = fingerprint_file();
        let overrides = every_option(&file);
        let vars: std::collections::HashMap<_, _> = overrides.env().into_iter().collect();
        assert_eq!(vars.len(), 18, "one variable per option");

        let via_env =
            Settings::apply_overrides(Settings::default(), |name| vars.get(name).cloned()).unwrap();
        let mut direct = Settings::default();
        overrides.try_apply(&mut direct).unwrap();

        assert_eq!(via_env, direct);
    }

    #[test]
    fn options_that_are_not_set_produce_no_variable() {
        assert!(SettingsOverrides::default().env().is_empty());
        let only_proxy = SettingsOverrides {
            options: vec![("SB_PROXY", "p:1".to_owned())],
        };
        assert_eq!(only_proxy.env(), [("SB_PROXY", "p:1".to_owned())]);
    }

    #[test]
    fn an_override_lookup_that_finds_nothing_changes_nothing() {
        let mut original = Settings::default();
        original.browser.kind = Browser::Edge;
        original.browser.mobile = true;
        let same = Settings::apply_overrides(original.clone(), |_| None).unwrap();
        assert_eq!(same, original);
    }

    #[test]
    fn a_bad_override_value_is_an_error() {
        let result = Settings::apply_overrides(Settings::default(), |name| {
            (name == "SB_THREADS").then(|| "many".to_owned())
        });
        assert!(result.is_err());
    }

    #[test]
    fn the_settings_file_names_its_fingerprint_in_a_table_and_a_default_writes_none() {
        let from_toml: Settings =
            toml::from_str("headless = true\n[fingerprint]\nfile = \"profiles/real.json\"\n")
                .unwrap();
        let from_json: Settings =
            serde_json::from_str(r#"{"fingerprint": {"file": "profiles/real.json"}}"#).unwrap();

        assert_eq!(
            from_toml.fingerprint.file.as_deref(),
            Some(Path::new("profiles/real.json"))
        );
        assert_eq!(from_json.fingerprint, from_toml.fingerprint);
        let round_trip: Settings = toml::from_str(&toml::to_string(&from_toml).unwrap()).unwrap();
        assert_eq!(round_trip, from_toml);
        assert!(!toml::to_string(&Settings::default())
            .unwrap()
            .contains("fingerprint"));
        assert!(!serde_json::to_string(&Settings::default())
            .unwrap()
            .contains("fingerprint"));
        assert!(Settings::default().fingerprint.is_empty());
    }

    #[test]
    fn a_misspelt_key_of_the_fingerprint_table_is_an_error() {
        let result = toml::from_str::<Settings>("[fingerprint]\nfle = \"x.json\"\n");

        assert!(result.is_err());
    }

    #[test]
    fn the_fingerprint_variable_beats_the_table_of_the_file() {
        let (dir, from_variable) = fingerprint_file();
        let from_table = dir.path().join("table.json");
        Fingerprint::macos_desktop().save(&from_table).unwrap();
        let file: Settings = toml::from_str(&format!(
            "[fingerprint]\nfile = '{}'\n",
            from_table.display()
        ))
        .unwrap();
        let variable = from_variable.display().to_string();

        let untouched = Settings::apply_overrides(file.clone(), |_| None).unwrap();
        let replaced = Settings::apply_overrides(file, |name| {
            (name == FINGERPRINT_ENV).then(|| variable.clone())
        })
        .unwrap();

        assert_eq!(
            untouched.fingerprint.file.as_deref(),
            Some(from_table.as_path())
        );
        assert_eq!(
            replaced.fingerprint.file.as_deref(),
            Some(from_variable.as_path())
        );
    }

    #[test]
    fn the_command_line_beats_the_variable_and_the_table() {
        let (dir, from_flag) = fingerprint_file();
        let from_variable = dir.path().join("variable.json");
        Fingerprint::linux_desktop().save(&from_variable).unwrap();
        let variable = from_variable.display().to_string();
        let mut settings = Settings::apply_overrides(Settings::default(), |name| {
            (name == FINGERPRINT_ENV).then(|| variable.clone())
        })
        .unwrap();
        assert_eq!(
            settings.fingerprint.file.as_deref(),
            Some(from_variable.as_path())
        );

        SettingsOverrides {
            options: vec![(FINGERPRINT_ENV, from_flag.display().to_string())],
        }
        .try_apply(&mut settings)
        .unwrap();

        assert_eq!(
            settings.fingerprint.file.as_deref(),
            Some(from_flag.as_path())
        );
    }

    #[test]
    fn the_fingerprint_reaches_the_browser_config() {
        let (_dir, file) = fingerprint_file();
        let settings = Settings {
            fingerprint: FingerprintSettings { file: Some(file) },
            ..Settings::default()
        };

        let config = settings.to_browser_config();

        assert_eq!(config.fingerprint, Some(Fingerprint::windows_desktop()));
        assert_eq!(settings.load_fingerprint().unwrap(), config.fingerprint);
        assert!(Settings::default()
            .to_browser_config()
            .fingerprint
            .is_none());
        assert!(Settings::default().load_fingerprint().unwrap().is_none());
    }

    #[test]
    fn a_fingerprint_file_that_cannot_be_used_stops_the_settings_from_loading() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.json").display().to_string();
        let garbage = dir.path().join("garbage.json");
        fs::write(&garbage, "{ \"screen_width\": 1 }").unwrap();
        let garbage = garbage.display().to_string();

        let not_there = Settings::apply_overrides(Settings::default(), |name| {
            (name == FINGERPRINT_ENV).then(|| missing.clone())
        })
        .unwrap_err();
        let not_a_fingerprint = Settings::apply_overrides(Settings::default(), |name| {
            (name == FINGERPRINT_ENV).then(|| garbage.clone())
        })
        .unwrap_err();

        assert!(
            matches!(not_there, SeleniumBaseError::InvalidConfig(_)),
            "{not_there:?}"
        );
        assert!(
            not_there.to_string().contains("missing.json"),
            "{not_there}"
        );
        assert!(
            not_a_fingerprint.to_string().contains("screen_width"),
            "{not_a_fingerprint}"
        );
        assert_eq!(not_there.exit_code(), 2);
    }

    #[test]
    fn a_settings_file_with_a_wrong_fingerprint_path_does_not_load() {
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(b"[fingerprint]\nfile = \"/no/such/fingerprint.json\"\n")
            .unwrap();

        let error = Settings::from_file(file.path()).unwrap_err();

        assert!(error.to_string().contains("fingerprint.json"), "{error}");
    }

    #[test]
    fn a_blank_fingerprint_variable_counts_as_unset() {
        let settings = with_vars(&[("SB_FINGERPRINT", "   ")]).unwrap();

        assert!(settings.fingerprint.is_empty());
    }

    #[test]
    fn a_settings_made_by_hand_with_a_missing_file_gets_no_fingerprint_and_says_why_when_asked() {
        let settings = Settings {
            fingerprint: FingerprintSettings {
                file: Some(PathBuf::from("/no/such/fingerprint.json")),
            },
            ..Settings::default()
        };

        assert!(settings.to_browser_config().fingerprint.is_none());
        let error = settings.check_fingerprint().unwrap_err();
        assert!(error.to_string().contains("fingerprint.json"), "{error}");
    }

    #[test]
    fn the_override_is_exported_as_its_variable() {
        let only = SettingsOverrides {
            options: vec![(FINGERPRINT_ENV, "profiles/real.json".to_owned())],
        };

        assert_eq!(
            only.env(),
            [("SB_FINGERPRINT", "profiles/real.json".to_owned())]
        );
    }

    #[test]
    fn the_fingerprint_file_a_command_presents_comes_from_the_flag_then_the_variable_then_the_table(
    ) {
        // Not a valid `Settings` file: `headless` is not a boolean. Only the table is read.
        let body = "headless = \"maybe\"\n[fingerprint]\nfile = \"from-table.json\"\n";
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(body.as_bytes()).unwrap();
        let variable =
            |name: &str| (name == FINGERPRINT_ENV).then(|| "from-variable.json".to_owned());
        let flag = Path::new("from-flag.json");

        let resolved = |flag, var: &dyn Fn(&str) -> Option<String>| {
            Settings::fingerprint_file_with(Some(file.path()), flag, var).unwrap()
        };

        assert_eq!(resolved(Some(flag), &variable).as_deref(), Some(flag));
        assert_eq!(
            resolved(None, &variable).as_deref(),
            Some(Path::new("from-variable.json"))
        );
        assert_eq!(
            resolved(None, &|_| None).as_deref(),
            Some(Path::new("from-table.json"))
        );
        assert_eq!(
            resolved(None, &|_| Some("  ".to_owned())).as_deref(),
            Some(Path::new("from-table.json"))
        );
        // Nothing is named anywhere.
        let mut empty = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        empty.write_all(b"headless = true\n").unwrap();
        assert!(
            Settings::fingerprint_file_with(Some(empty.path()), None, |_| None)
                .unwrap()
                .is_none()
        );
        // A settings file that is not there is an error, as it is for the plugins.
        let error = Settings::fingerprint_file_with(Some("no-such-settings.toml"), None, |_| None)
            .unwrap_err();
        assert!(error.to_string().contains("failed to read"), "{error}");
    }

    #[test]
    fn parse_bool_values() {
        assert!(parse_bool("true").unwrap());
        assert!(parse_bool("YES").unwrap());
        assert!(parse_bool("1").unwrap());
        assert!(!parse_bool("false").unwrap());
        assert!(!parse_bool("OFF").unwrap());
        assert!(parse_bool("not-a-bool").is_err());
    }

    #[test]
    fn the_start_page_comes_from_the_variable_the_file_or_the_command_line() {
        // Nothing names a page: a browser opens none.
        assert_eq!(Settings::default().browser.url, None);
        assert_eq!(Settings::default().to_browser_config().browser.url, None);

        // The variable, a blank one counting as unset.
        let from_variable = with_vars(&[("SB_START_PAGE", "https://start.test/")]).unwrap();
        assert_eq!(
            from_variable.browser.url.as_deref(),
            Some("https://start.test/")
        );
        assert_eq!(
            from_variable.to_browser_config().browser.url.as_deref(),
            Some("https://start.test/")
        );
        assert_eq!(
            with_vars(&[("SB_START_PAGE", "  ")]).unwrap().browser.url,
            None
        );

        // The `start_page` key of the settings file, and the variable over it.
        let from_file = settings_of("start_page = \"https://file.test/\"");
        assert_eq!(from_file.browser.url.as_deref(), Some("https://file.test/"));
        let both = Settings::apply_overrides(from_file, |name| {
            (name == START_PAGE_ENV).then(|| "https://start.test/".to_owned())
        })
        .unwrap();
        assert_eq!(both.browser.url.as_deref(), Some("https://start.test/"));

        // The command line over both, and on to the tests as the variable.
        let mut settings = both;
        let flag = SettingsOverrides {
            options: vec![(START_PAGE_ENV, "https://flag.test/".to_owned())],
        };
        flag.try_apply(&mut settings).unwrap();
        assert_eq!(settings.browser.url.as_deref(), Some("https://flag.test/"));
        assert_eq!(
            flag.env(),
            [(START_PAGE_ENV, "https://flag.test/".to_owned())]
        );
        // A settings file that does not name one writes none.
        assert!(
            serde_json::to_value(Settings::default()).unwrap()["browser"]
                .get("url")
                .is_none()
        );
    }
}
