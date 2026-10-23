//! `sbrs install` (also `sbrs get`): fetch a driver.
//!
//! Python SeleniumBase's `sbrs get` and `sbrs install` take a driver name and
//! a version; so does this one, for chromedriver, geckodriver and msedgedriver.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::browser::binaries::{downloaded_executable, locate, CHROMIUM_DIR, DOWNLOADS_DIR};
use crate::browser::chrome_for_testing::{
    install_chrome_for_testing, install_chrome_headless_shell, install_chromedriver,
};
use crate::browser::chromium_snapshots::{
    check_revision, install_chromium_with_driver, installed_revision,
};
use crate::browser::config::Browser;
use crate::browser::downloader::download_chrome_driver_for;
use crate::browser::driver_access::make_runnable;
use crate::browser::driver_install::{
    downloaded_driver, install_geckodriver, install_msedgedriver, GECKODRIVER_NAMES,
};
use crate::browser::driver_version::binary_major;
use crate::browser::options::{DriverVersion, ReleaseChannel};
use crate::error::SeleniumBaseError;
use crate::stealth::patcher::{find_system_chrome, patch_chromedriver};

/// A driver `sbrs install` can fetch.
///
/// The names Python SeleniumBase takes are read by [`FromStr`]: `chromedriver`,
/// `geckodriver` (also `firefoxdriver`), and `edgedriver` (also
/// `msedgedriver`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriverName {
    /// chromedriver, for Chrome.
    Chromedriver,
    /// geckodriver, for Firefox.
    Geckodriver,
    /// msedgedriver, for Microsoft Edge.
    Msedgedriver,
    /// chromedriver patched for the UC mode (`uc_driver`), kept in
    /// `downloaded_drivers/uc_driver-<version>/`.
    UcDriver,
    /// Chrome for Testing (`cft`), the browser.
    Cft,
    /// `chrome-headless-shell` (`chs`), the headless-only browser.
    Chs,
    /// A Chromium snapshot build (`chromium`), the browser.
    Chromium,
}

impl fmt::Display for DriverName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Chromedriver => "chromedriver",
            Self::Geckodriver => "geckodriver",
            Self::Msedgedriver => "msedgedriver",
            Self::UcDriver => "uc_driver",
            Self::Cft => "cft",
            Self::Chs => "chs",
            Self::Chromium => "chromium",
        })
    }
}

impl FromStr for DriverName {
    type Err = SeleniumBaseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "chromedriver" => Ok(Self::Chromedriver),
            "geckodriver" | "firefoxdriver" => Ok(Self::Geckodriver),
            "edgedriver" | "msedgedriver" => Ok(Self::Msedgedriver),
            "uc_driver" | "uc-driver" | "ucdriver" => Ok(Self::UcDriver),
            "cft" => Ok(Self::Cft),
            "chs" => Ok(Self::Chs),
            "chromium" => Ok(Self::Chromium),

            _ => Err(SeleniumBaseError::parse(
                text,
                "a driver: chromedriver, uc_driver, geckodriver (firefoxdriver), edgedriver \
                 (msedgedriver), cft, chs or chromium",
            )),
        }
    }
}

/// The major version of the browser at `program`, if it can be read.
async fn major_of(program: Option<PathBuf>) -> Option<u32> {
    match program {
        Some(program) => binary_major(&program).await,
        None => None,
    }
}

/// Downloads driver `name` into `./downloaded_drivers/` and returns the path of
/// the executable.
///
/// `version` is what Python's `sbrs get NAME VERSION` takes. Without one,
/// chromedriver and msedgedriver are the drivers for the installed Chrome and
/// Edge (the latest stable ones when the browser is not found), and geckodriver
/// is the newest release. With one: `latest`, a major version (`155`) or a full
/// build for chromedriver and msedgedriver, and `latest` or a version such as
/// `0.36.0` for geckodriver. Needs network access unless that driver is in
/// `downloaded_drivers/` already.
///
/// # Errors
///
/// Fails on a version that names no driver, an unsupported platform, or a
/// listing or download that fails.
pub async fn install_driver(
    name: DriverName,
    version: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    let mut request = InstallRequest::new(name);
    request.version = version.map(str::to_owned);
    Ok(install(&request).await?.path)
}

/// The environment variable that names the folder `--path` copies a driver to.
pub const PATH_DIR_VARIABLE: &str = "SB_DRIVER_PATH_DIR";

/// The folder `--path` copies a driver to when [`PATH_DIR_VARIABLE`] is not set.
const UNIX_PATH_DIR: &str = "/usr/local/bin";

/// What `sbrs install` is asked to fetch.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_install::{DriverName, InstallRequest};
///
/// let request = InstallRequest::new(DriverName::Chromium).with_revision(1_725_796);
/// assert_eq!(request.revision, Some(1_725_796));
/// assert!(!request.copy_to_path);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallRequest {
    /// Which program to fetch.
    pub name: DriverName,
    /// The version: a word (`stable`, `beta`, `dev`, `canary`, `latest`, `previous`, `mlatest`,
    /// `browser`, `keep`), a major version or a full build; geckodriver also reads `0.36.0`.
    /// `None` means the build for the installed browser.
    pub version: Option<String>,
    /// The snapshot revision of a Chromium (`--revision`); `chromium` only.
    pub revision: Option<u64>,
    /// Copy the driver into the folder `--path` names (see [`path_target_dir`]); drivers only.
    pub copy_to_path: bool,
}

impl InstallRequest {
    /// A request for `name`, for the installed browser.
    #[must_use]
    pub const fn new(name: DriverName) -> Self {
        Self {
            name,
            version: None,
            revision: None,
            copy_to_path: false,
        }
    }

    /// Asks for `version`.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Asks for a Chromium snapshot `revision`.
    #[must_use]
    pub const fn with_revision(mut self, revision: u64) -> Self {
        self.revision = Some(revision);
        self
    }

    /// Checks that the request asks for what its program can do; [`install`] does this first.
    ///
    /// # Errors
    ///
    /// Fails on a `revision` for anything but `chromium`, on a revision no snapshot can have,
    /// and on `copy_to_path` for a browser.
    pub fn validate(&self) -> Result<(), SeleniumBaseError> {
        check_request(self)
    }

    /// Asks for the driver to be copied into the `--path` folder.
    #[must_use]
    pub const fn with_copy_to_path(mut self, copy: bool) -> Self {
        self.copy_to_path = copy;
        self
    }
}

/// What an install left on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Installed {
    /// The program: the driver, or the browser's executable.
    pub path: PathBuf,
    /// Its version, when its folder or the download names one (`141.0.7390.76`); a Chromium
    /// snapshot is `r1725796`.
    pub version: Option<String>,
    /// The chromedriver that came with a Chromium snapshot, of the same revision.
    pub driver: Option<PathBuf>,
    /// Where `--path` copied the driver.
    pub copied_to: Option<PathBuf>,
}

impl DriverName {
    /// Whether this is a browser (`cft`, `chs`, `chromium`), not a driver.
    #[must_use]
    pub const fn is_browser(self) -> bool {
        matches!(self, Self::Cft | Self::Chs | Self::Chromium)
    }
}

/// Downloads what `request` asks for into `./downloaded_drivers/`, and copies the driver where
/// `--path` says when it is asked to.
///
/// One parser reads the version for `sbrs install` and for `--driver-version`
/// ([`DriverVersion`]); on top of it, `mlatest` here needs an installed browser to take the
/// milestone of, and says so when there is none. Needs network access unless the build is
/// already in `downloaded_drivers/`.
///
/// # Errors
///
/// Fails on a request that contradicts itself (a `--revision` for a driver, a `--path` for a
/// browser), a version that names no build, an unsupported platform, a download that fails, or
/// a `--path` folder that cannot be written (a symbolic link or a file that is not a regular
/// one is never replaced).
pub async fn install(request: &InstallRequest) -> Result<Installed, SeleniumBaseError> {
    check_request(request)?;
    // The folder is settled before anything is downloaded, so a wrong setting costs no download.
    let target = if request.copy_to_path {
        Some(path_target_dir(
            &|name| std::env::var(name).ok(),
            cfg!(windows),
        )?)
    } else {
        None
    };
    let mut installed = fetch(request).await?;
    if let Some(dir) = target {
        installed.copied_to = Some(copy_to_path_dir(&installed.path, &dir)?);
    }
    Ok(installed)
}

/// Refuses a request that asks for something its program cannot do.
fn check_request(request: &InstallRequest) -> Result<(), SeleniumBaseError> {
    if let Some(revision) = request.revision {
        if request.name != DriverName::Chromium {
            return Err(SeleniumBaseError::invalid_config(
                "--revision applies to chromium only",
            ));
        }
        check_revision(revision)?;
    }
    if request.copy_to_path && request.name.is_browser() {
        return Err(SeleniumBaseError::invalid_config(format!(
            "--path copies drivers, and {} is a browser",
            request.name
        )));
    }
    Ok(())
}

/// Refuses `mlatest` when no browser was found to take the milestone of.
fn needs_browser(text: Option<&str>, browser_major: Option<u32>) -> Result<(), SeleniumBaseError> {
    let mlatest = text.is_some_and(|text| text.trim().eq_ignore_ascii_case("mlatest"));
    if mlatest && browser_major.is_none() {
        return Err(SeleniumBaseError::invalid_config(
            "mlatest is the newest build of the installed browser's version, and no browser was \
             found; name a version, or use stable",
        ));
    }
    Ok(())
}

/// The version a program's folder is named after: `chromedriver-141.0.7390.76/chromedriver`
/// gives `141.0.7390.76`.
fn version_of(path: &Path, stem: &str) -> Option<String> {
    let prefix = format!("{stem}-");
    path.ancestors()
        .filter_map(|ancestor| ancestor.file_name()?.to_str())
        .find_map(|name| name.strip_prefix(&prefix))
        .map(str::to_owned)
}

/// What a chromedriver `text` asks for, downloaded: the driver for the installed Chrome when no
/// version is given.
async fn chromedriver(text: Option<&str>) -> Result<PathBuf, SeleniumBaseError> {
    let Some(text) = text else {
        return install_drivers().await;
    };
    let version: DriverVersion = text.parse()?;
    let chrome = major_of(find_system_chrome()).await;
    needs_browser(Some(text), chrome)?;
    install_chromedriver(&version, chrome, None).await
}

/// geckodriver: the newest release for `latest` and the words that mean it, the one already
/// downloaded for `keep`, else the version named (`0.36.0`).
async fn geckodriver(root: &Path, text: Option<&str>) -> Result<PathBuf, SeleniumBaseError> {
    let wanted = text.map(|text| text.trim().to_ascii_lowercase());
    match wanted.as_deref() {
        None | Some("" | "latest" | "stable" | "browser" | "mlatest") => {
            install_geckodriver(None, None).await
        }
        Some("keep") => match downloaded_driver(root, "geckodriver", &GECKODRIVER_NAMES) {
            Some(found) => Ok(found),
            None => install_geckodriver(None, None).await,
        },
        Some(_) => install_geckodriver(text, None).await,
    }
}

/// What a browser download takes from the version: `browser` and `keep` follow the installed
/// Chrome's major version, or the stable release when there is none.
fn browser_build(version: DriverVersion, chrome_major: Option<u32>) -> DriverVersion {
    match version {
        DriverVersion::MatchBrowser | DriverVersion::Keep => chrome_major.map_or(
            DriverVersion::Channel(ReleaseChannel::Stable),
            DriverVersion::Milestone,
        ),
        other => other,
    }
}

/// Whether a Chromium `text` is `keep`; a snapshot has no version, only a revision.
fn chromium_keeps(text: Option<&str>) -> Result<bool, SeleniumBaseError> {
    match text.map(|text| text.trim().to_ascii_lowercase()).as_deref() {
        None | Some("" | "latest" | "stable") => Ok(false),
        Some("keep") => Ok(true),
        Some(_) => Err(SeleniumBaseError::invalid_config(format!(
            "chromium has revisions, not versions: give --revision N, or latest or keep (not {})",
            text.unwrap_or_default().trim()
        ))),
    }
}

/// Downloads what `request` names, without the checks of [`install`] and without `--path`.
async fn fetch(request: &InstallRequest) -> Result<Installed, SeleniumBaseError> {
    let text = request.version.as_deref();
    let root = Path::new(DOWNLOADS_DIR);
    let plain = |path: PathBuf, stem: &str| Installed {
        version: version_of(&path, stem),
        path,
        driver: None,
        copied_to: None,
    };
    match request.name {
        DriverName::Chromedriver => Ok(plain(chromedriver(text).await?, "chromedriver")),
        DriverName::UcDriver => {
            let chromedriver = chromedriver(text).await?;
            let version = version_of(&chromedriver, "chromedriver");
            let path = make_uc_driver(root, &chromedriver, version.as_deref(), &|path| {
                patch_chromedriver(path)?;
                // The patch changed the bytes, so macOS needs the signature made again.
                make_runnable(path, true).map(|_| ())
            })?;
            Ok(Installed {
                path,
                version,
                driver: None,
                copied_to: None,
            })
        }
        DriverName::Geckodriver => Ok(plain(geckodriver(root, text).await?, "geckodriver")),
        DriverName::Msedgedriver => {
            let version: DriverVersion = match text {
                None => DriverVersion::MatchBrowser,
                Some(text) => text.parse()?,
            };
            let edge = major_of(locate(Browser::Edge)).await;
            needs_browser(text, edge)?;
            let path = install_msedgedriver(&version, edge, None).await?;
            Ok(plain(path, "msedgedriver"))
        }
        DriverName::Cft | DriverName::Chs => {
            let version: DriverVersion = match text {
                None => DriverVersion::MatchBrowser,
                Some(text) => text.parse()?,
            };
            let browser = if request.name == DriverName::Cft {
                Browser::ChromeForTesting
            } else {
                Browser::ChromeHeadlessShell
            };
            if version == DriverVersion::Keep {
                if let Some(path) = locate(browser) {
                    return Ok(plain(path, "cft"));
                }
            }
            let chrome = major_of(find_system_chrome()).await;
            needs_browser(text, chrome)?;
            let version = browser_build(version, chrome);
            let path = if browser == Browser::ChromeForTesting {
                install_chrome_for_testing(&version, None).await?
            } else {
                install_chrome_headless_shell(&version, None).await?
            };
            Ok(plain(path, "cft"))
        }
        DriverName::Chromium => {
            let keep = chromium_keeps(text)?;
            if keep && request.revision.is_none() {
                if let Some(path) =
                    downloaded_executable(Browser::Chromium, &root.join(CHROMIUM_DIR))
                {
                    return Ok(Installed {
                        path,
                        version: installed_revision(root).map(|revision| format!("r{revision}")),
                        driver: None,
                        copied_to: None,
                    });
                }
            }
            let snapshot = install_chromium_with_driver(request.revision, None).await?;
            Ok(Installed {
                path: snapshot.browser,
                version: Some(format!("r{}", snapshot.revision)),
                driver: snapshot.driver,
                copied_to: None,
            })
        }
    }
}

/// Where `--path` copies a driver: the folder named by [`PATH_DIR_VARIABLE`], else
/// `/usr/local/bin`. `env` looks a variable up; `windows` says whether the system is Windows,
/// where there is no such default.
///
/// # Errors
///
/// Returns an error on Windows when the variable is not set.
pub fn path_target_dir(
    env: &dyn Fn(&str) -> Option<String>,
    windows: bool,
) -> Result<PathBuf, SeleniumBaseError> {
    match env(PATH_DIR_VARIABLE).filter(|dir| !dir.trim().is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir)),
        None if windows => Err(SeleniumBaseError::invalid_config(format!(
            "--path needs {PATH_DIR_VARIABLE} on Windows"
        ))),
        None => Ok(PathBuf::from(UNIX_PATH_DIR)),
    }
}

/// Copies `executable` into `dir` under its own name, and returns the copy's path.
///
/// Nothing is run with more privilege than the caller has: a folder that cannot be written
/// gives an error that says so, and `sudo` is never started. A symbolic link, a folder or any
/// other file that is not a regular one at the target is refused, never written through. The
/// copy is made beside the target and moved onto it, so a copy that fails leaves the old file.
///
/// # Errors
///
/// Returns an error when `dir` is not a folder, the target is not a regular file, or the copy
/// fails.
pub fn copy_to_path_dir(executable: &Path, dir: &Path) -> Result<PathBuf, SeleniumBaseError> {
    let name = executable.file_name().ok_or_else(|| {
        SeleniumBaseError::invalid_config(format!("{} names no file", executable.display()))
    })?;
    if !dir.is_dir() {
        return Err(SeleniumBaseError::invalid_config(format!(
            "{} is not a folder; create it, or set {PATH_DIR_VARIABLE} to a folder on your PATH",
            dir.display()
        )));
    }
    let target = dir.join(name);
    match fs::symlink_metadata(&target) {
        Ok(found) if found.file_type().is_symlink() => {
            return Err(SeleniumBaseError::invalid_config(format!(
                "{} is a symbolic link; it is not replaced",
                target.display()
            )))
        }
        Ok(found) if !found.is_file() => {
            return Err(SeleniumBaseError::invalid_config(format!(
                "{} is not a regular file; it is not replaced",
                target.display()
            )))
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(not_written(dir, error)),
    }
    let mut temporary = name.to_os_string();
    temporary.push(".sbrs-copy");
    let temporary = dir.join(temporary);
    // A leftover of an earlier copy that was cut short.
    let _ = fs::remove_file(&temporary);
    copy_then_move(executable, &temporary, &target).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        not_written(dir, error)
    })?;
    Ok(target)
}

/// Writes `from` to the new file `temporary` and moves it onto `target`.
fn copy_then_move(from: &Path, temporary: &Path, target: &Path) -> io::Result<()> {
    let mut input = fs::File::open(from)?;
    // `create_new` never opens an existing file, nor a link that is already there.
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    io::copy(&mut input, &mut output)?;
    output.set_permissions(input.metadata()?.permissions())?;
    drop(output);
    make_runnable(temporary, false).map_err(io::Error::other)?;
    fs::rename(temporary, target)
}

/// The error for a folder that could not be written.
fn not_written(dir: &Path, error: io::Error) -> SeleniumBaseError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        SeleniumBaseError::invalid_config(format!(
            "cannot write into {}: {error}; run with sudo, or set {PATH_DIR_VARIABLE} to a folder \
             on your PATH",
            dir.display()
        ))
    } else {
        error.into()
    }
}

/// Makes `uc_driver-<version>/uc_driver`: a copy of `chromedriver`, patched by `patch`. One
/// that is already there is kept.
pub(crate) fn make_uc_driver(
    root: &Path,
    chromedriver: &Path,
    version: Option<&str>,
    patch: &dyn Fn(&Path) -> Result<(), SeleniumBaseError>,
) -> Result<PathBuf, SeleniumBaseError> {
    let name = if cfg!(windows) {
        "uc_driver.exe"
    } else {
        "uc_driver"
    };
    let folder = root.join(format!("uc_driver-{}", version.unwrap_or("local")));
    let target = folder.join(name);
    if target.is_file() {
        return Ok(target);
    }
    fs::create_dir_all(&folder)?;
    let partial = folder.join(format!("{name}.partial"));
    patched_copy(chromedriver, &partial, &target, patch).map_err(|error| {
        let _ = fs::remove_file(&partial);
        error
    })?;
    Ok(target)
}

/// Copies `from` to `partial`, patches it, and moves it onto `target`.
fn patched_copy(
    from: &Path,
    partial: &Path,
    target: &Path,
    patch: &dyn Fn(&Path) -> Result<(), SeleniumBaseError>,
) -> Result<(), SeleniumBaseError> {
    fs::copy(from, partial)?;
    patch(partial)?;

    fs::rename(partial, target)?;
    Ok(())
}

/// Downloads the chromedriver that matches the installed Chrome into
/// `./downloaded_drivers/` and returns the path of the executable.
///
/// A chromedriver only drives the Chrome of its own major version, so the driver
/// is the newest build of the installed Chrome's milestone, taken from Chrome for
/// Testing. When Chrome is not found, or Chrome for Testing lists no driver for
/// its version, the latest Stable driver is fetched instead. It only fetches
/// `chromedriver`; the CDP engine needs no driver at all. Needs network access.
///
/// # Errors
///
/// Fails if the platform is unsupported, the network request fails or the
/// archive cannot be unpacked.
///
/// ```no_run
/// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let driver = seleniumbase_rs::cli::scripts::sb_install::install_drivers().await?;
/// println!("chromedriver is at {}", driver.display());
/// # Ok(())
/// # }
/// ```
pub async fn install_drivers() -> Result<PathBuf, SeleniumBaseError> {
    let browser = match find_system_chrome() {
        Some(chrome) => binary_major(&chrome).await,
        None => None,
    };
    match download_chrome_driver_for(browser).await {
        Err(error) if browser.is_some() => {
            tracing::warn!(%error, "no chromedriver for the installed Chrome; using the stable one");
            download_chrome_driver_for(None).await
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const EVERY_NAME: [DriverName; 7] = [
        DriverName::Chromedriver,
        DriverName::UcDriver,
        DriverName::Geckodriver,
        DriverName::Msedgedriver,
        DriverName::Cft,
        DriverName::Chs,
        DriverName::Chromium,
    ];

    #[test]
    fn the_programs_python_adds_are_read_and_read_back() {
        for (text, name) in [
            ("uc_driver", DriverName::UcDriver),
            ("uc-driver", DriverName::UcDriver),
            (" UCDRIVER ", DriverName::UcDriver),
            ("cft", DriverName::Cft),
            ("chs", DriverName::Chs),
            ("Chromium", DriverName::Chromium),
        ] {
            assert_eq!(text.parse::<DriverName>().unwrap(), name, "{text}");
        }
        for name in EVERY_NAME {
            assert_eq!(name.to_string().parse::<DriverName>().unwrap(), name);
        }
        let browsers: Vec<_> = EVERY_NAME.into_iter().filter(|n| n.is_browser()).collect();
        assert_eq!(
            browsers,
            [DriverName::Cft, DriverName::Chs, DriverName::Chromium]
        );
        let error = "operadriver".parse::<DriverName>().unwrap_err().to_string();
        for word in ["uc_driver", "cft", "chs", "chromium"] {
            assert!(error.contains(word), "{word} missing from {error}");
        }
    }

    #[test]
    fn a_request_asks_only_for_what_its_program_can_do() {
        assert!(InstallRequest::new(DriverName::Chromium)
            .with_revision(1_725_796)
            .validate()
            .is_ok());
        for name in EVERY_NAME
            .into_iter()
            .filter(|n| *n != DriverName::Chromium)
        {
            let error = InstallRequest::new(name).with_revision(1).validate();
            let error = error.unwrap_err().to_string();
            assert!(
                error.contains("--revision applies to chromium only"),
                "{name}: {error}"
            );
        }
        assert!(InstallRequest::new(DriverName::Chromium)
            .with_revision(0)
            .validate()
            .is_err());
        for name in EVERY_NAME {
            let result = InstallRequest::new(name).with_copy_to_path(true).validate();
            assert_eq!(result.is_err(), name.is_browser(), "{name}");
        }
    }

    #[tokio::test]
    async fn a_contradiction_is_refused_before_anything_is_downloaded() {
        let request = InstallRequest::new(DriverName::Cft).with_copy_to_path(true);
        let error = install(&request).await.unwrap_err().to_string();
        assert!(error.contains("--path copies drivers"), "{error}");
        let request = InstallRequest::new(DriverName::Chromedriver).with_revision(5);
        assert!(install(&request).await.is_err());
    }

    #[test]
    fn mlatest_needs_an_installed_browser_and_nothing_else_does() {
        for text in ["mlatest", " MLatest "] {
            let error = needs_browser(Some(text), None).unwrap_err().to_string();
            assert!(error.contains("no browser was found"), "{error}");
            assert!(needs_browser(Some(text), Some(150)).is_ok());
        }
        for text in [
            None,
            Some("browser"),
            Some("stable"),
            Some("keep"),
            Some("previous"),
        ] {
            assert!(needs_browser(text, None).is_ok(), "{text:?}");
        }
    }

    #[test]
    fn every_version_word_is_read_by_the_one_parser() {
        for word in [
            "stable",
            "beta",
            "dev",
            "canary",
            "latest",
            "previous",
            "latest-1",
            "mlatest",
            "browser",
            "keep",
            "150",
            "150.0.7500.0",
        ] {
            assert!(word.parse::<DriverVersion>().is_ok(), "{word}");
        }
    }

    #[test]
    fn a_browser_download_follows_the_installed_chrome_or_the_stable_release() {
        let stable = DriverVersion::Channel(ReleaseChannel::Stable);
        assert_eq!(
            browser_build(DriverVersion::MatchBrowser, Some(150)),
            DriverVersion::Milestone(150)
        );
        assert_eq!(browser_build(DriverVersion::Keep, None), stable);
        assert_eq!(browser_build(DriverVersion::MatchBrowser, None), stable);
        for kept in [
            DriverVersion::Previous,
            DriverVersion::Milestone(7),
            DriverVersion::Channel(ReleaseChannel::Beta),
        ] {
            assert_eq!(browser_build(kept.clone(), Some(150)), kept);
        }
    }

    #[test]
    fn chromium_has_revisions_so_only_latest_and_keep_are_versions() {
        for (text, keeps) in [
            (None, false),
            (Some("latest"), false),
            (Some("stable"), false),
            (Some(" KEEP "), true),
        ] {
            assert_eq!(chromium_keeps(text).unwrap(), keeps, "{text:?}");
        }
        for text in ["150", "beta", "mlatest"] {
            let error = chromium_keeps(Some(text)).unwrap_err().to_string();
            assert!(error.contains("--revision"), "{error}");
        }
    }

    #[test]
    fn a_programs_version_is_the_one_its_folder_is_named_after() {
        let version = |path: &str, stem: &str| version_of(Path::new(path), stem);
        assert_eq!(
            version(
                "downloaded_drivers/chromedriver-141.0.7390.76/chromedriver",
                "chromedriver"
            )
            .as_deref(),
            Some("141.0.7390.76")
        );
        assert_eq!(
            version("d/geckodriver-v0.37.1/geckodriver", "geckodriver").as_deref(),
            Some("v0.37.1")
        );
        assert_eq!(
            version(
                "d/chromedriver-1.2.3.4/chromedriver_linux64/chromedriver",
                "chromedriver"
            )
            .as_deref(),
            Some("1.2.3.4")
        );
        assert_eq!(version("d/chromedriver", "chromedriver"), None);
        assert_eq!(version("d/cft/chrome", "cft"), None);
    }

    #[tokio::test]
    async fn keep_uses_the_newest_geckodriver_already_downloaded() {
        let root = tempfile::tempdir().unwrap();
        for tag in ["v0.36.0", "v0.37.1"] {
            let folder = root.path().join(format!("geckodriver-{tag}"));
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("geckodriver"), b"x").unwrap();
        }
        let found = geckodriver(root.path(), Some(" KEEP ")).await.unwrap();
        assert_eq!(
            found,
            root.path().join("geckodriver-v0.37.1").join("geckodriver")
        );
    }

    #[test]
    fn the_path_folder_comes_from_the_variable_else_usr_local_bin_and_windows_needs_it() {
        let set = |name: &str| (name == PATH_DIR_VARIABLE).then(|| "/opt/drivers".to_owned());
        let blank = |name: &str| (name == PATH_DIR_VARIABLE).then(|| "  ".to_owned());
        let unset = |_: &str| None::<String>;
        assert_eq!(
            path_target_dir(&set, false).unwrap(),
            PathBuf::from("/opt/drivers")
        );
        assert_eq!(
            path_target_dir(&set, true).unwrap(),
            PathBuf::from("/opt/drivers")
        );
        assert_eq!(
            path_target_dir(&unset, false).unwrap(),
            PathBuf::from("/usr/local/bin")
        );
        assert_eq!(
            path_target_dir(&blank, false).unwrap(),
            PathBuf::from("/usr/local/bin")
        );
        for env in [&unset as &dyn Fn(&str) -> Option<String>, &blank] {
            let error = path_target_dir(env, true).unwrap_err().to_string();
            assert!(
                error.contains("--path needs SB_DRIVER_PATH_DIR on Windows"),
                "{error}"
            );
        }
    }

    #[test]
    fn a_driver_is_copied_whole_and_an_older_copy_is_replaced() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let driver = source.path().join("chromedriver");
        fs::write(&driver, b"first").unwrap();

        let copied = copy_to_path_dir(&driver, dest.path()).unwrap();
        assert_eq!(copied, dest.path().join("chromedriver"));
        assert_eq!(fs::read(&copied).unwrap(), b"first");

        fs::write(&driver, b"second").unwrap();
        copy_to_path_dir(&driver, dest.path()).unwrap();
        assert_eq!(fs::read(&copied).unwrap(), b"second");
        // No leftover of the copying remains beside it.
        let names: Vec<_> = fs::read_dir(dest.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["chromedriver"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&copied).unwrap().permissions().mode();
            assert_ne!(mode & 0o111, 0, "the copy can run: {mode:o}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_at_the_target_is_never_written_through() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let driver = source.path().join("chromedriver");
        fs::write(&driver, b"new").unwrap();
        let precious = elsewhere.path().join("precious");
        fs::write(&precious, b"keep me").unwrap();
        let link = dest.path().join("chromedriver");

        std::os::unix::fs::symlink(&precious, &link).unwrap();
        let error = copy_to_path_dir(&driver, dest.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("symbolic link"), "{error}");
        assert_eq!(fs::read(&precious).unwrap(), b"keep me");
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());

        // A link that points nowhere is refused as well, not replaced.
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(elsewhere.path().join("gone"), &link).unwrap();
        assert!(copy_to_path_dir(&driver, dest.path()).is_err());
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(!elsewhere.path().join("gone").exists());
    }

    #[test]
    fn a_folder_that_is_missing_or_a_target_that_is_a_folder_is_refused() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let driver = source.path().join("chromedriver");
        fs::write(&driver, b"x").unwrap();

        let error = copy_to_path_dir(&driver, &dest.path().join("nope")).unwrap_err();
        assert!(error.to_string().contains("is not a folder"), "{error}");
        let error = copy_to_path_dir(&driver, &driver).unwrap_err();
        assert!(error.to_string().contains("is not a folder"), "{error}");

        fs::create_dir(dest.path().join("chromedriver")).unwrap();
        let error = copy_to_path_dir(&driver, dest.path()).unwrap_err();
        assert!(error.to_string().contains("not a regular file"), "{error}");
        assert!(dest.path().join("chromedriver").is_dir());
    }

    fn uc_name() -> &'static str {
        if cfg!(windows) {
            "uc_driver.exe"
        } else {
            "uc_driver"
        }
    }

    #[test]
    fn the_uc_driver_is_a_patched_copy_made_once() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("chromedriver-150.0.1.2");
        fs::create_dir_all(&source).unwrap();
        let source = source.join("chromedriver");
        fs::write(&source, b"driver cdc_").unwrap();
        let calls = AtomicUsize::new(0);
        let patch = |path: &Path| -> Result<(), SeleniumBaseError> {
            calls.fetch_add(1, Ordering::SeqCst);
            let mut bytes = fs::read(path)?;
            bytes.extend_from_slice(b"+patched");
            fs::write(path, bytes)?;
            Ok(())
        };

        let made = make_uc_driver(root.path(), &source, Some("150.0.1.2"), &patch).unwrap();
        assert_eq!(
            made,
            root.path().join("uc_driver-150.0.1.2").join(uc_name())
        );
        assert_eq!(fs::read(&made).unwrap(), b"driver cdc_+patched");
        assert_eq!(
            fs::read(&source).unwrap(),
            b"driver cdc_",
            "the original is untouched"
        );

        let again = make_uc_driver(root.path(), &source, Some("150.0.1.2"), &patch).unwrap();
        assert_eq!(again, made);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a driver that is there is kept"
        );
        let names: Vec<_> = fs::read_dir(made.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [uc_name()]);

        let local = make_uc_driver(root.path(), &source, None, &patch).unwrap();
        assert_eq!(local, root.path().join("uc_driver-local").join(uc_name()));
    }

    #[test]
    fn a_patch_that_fails_leaves_no_driver_behind() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("chromedriver");
        fs::write(&source, b"not a driver").unwrap();
        let patch = |_: &Path| -> Result<(), SeleniumBaseError> {
            Err(SeleniumBaseError::invalid_config("no marker to patch"))
        };
        let error = make_uc_driver(root.path(), &source, Some("1.2.3.4"), &patch).unwrap_err();
        assert!(error.to_string().contains("no marker to patch"), "{error}");
        let folder = root.path().join("uc_driver-1.2.3.4");
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 0);
    }

    #[test]
    fn python_driver_names_are_read() {
        for (text, name) in [
            ("chromedriver", DriverName::Chromedriver),
            ("geckodriver", DriverName::Geckodriver),
            ("FirefoxDriver", DriverName::Geckodriver),
            ("edgedriver", DriverName::Msedgedriver),
            (" msedgedriver ", DriverName::Msedgedriver),
        ] {
            assert_eq!(text.parse::<DriverName>().unwrap(), name, "{text}");
        }
        let error = "operadriver".parse::<DriverName>().unwrap_err();
        let text = error.to_string();
        assert!(
            text.contains("geckodriver") && text.contains("edgedriver"),
            "{text}"
        );
        assert_eq!(DriverName::Msedgedriver.to_string(), "msedgedriver");
    }

    #[tokio::test]
    async fn a_version_that_names_no_driver_is_refused_before_any_download() {
        for (name, version) in [
            (DriverName::Geckodriver, "v1"),
            (DriverName::Msedgedriver, "v155"),
            (DriverName::Chromedriver, "1.2.3"),
        ] {
            let error = install_driver(name, Some(version)).await.unwrap_err();
            assert!(error.to_string().contains(version), "{name}: {error}");
        }
    }
}
