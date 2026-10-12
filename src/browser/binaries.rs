//! Finding the program of a browser that is not Google Chrome.
//!
//! Brave, Comet, Opera, Chrome for Testing and `chrome-headless-shell` are run
//! through a driver like Chrome is, but the driver has to be told where the
//! browser lives. [`locate`] looks in the places each is installed, then on the
//! `PATH`, and for the two Chrome builds in the `downloaded_drivers` folder
//! that `sbase get cft` and `sbase get chs` fill.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::browser::config::Browser;

/// The folder the driver and browser downloads go in, relative to the working
/// directory.
pub const DOWNLOADS_DIR: &str = "downloaded_drivers";

/// The folder a Chrome for Testing download is unpacked into.
pub const CHROME_FOR_TESTING_DIR: &str = "cft";

/// The folder a `chrome-headless-shell` download is unpacked into.
pub const CHROME_HEADLESS_SHELL_DIR: &str = "chs";

/// An operating system whose install locations are known.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    /// macOS.
    MacOs,
    /// Windows.
    Windows,
    /// Linux.
    Linux,
}

impl Platform {
    /// The system this program runs on, if it is one of the three.
    #[must_use]
    pub const fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else if cfg!(target_os = "windows") {
            Some(Self::Windows)
        } else if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else {
            None
        }
    }
}

/// Where `browser` is installed on `platform`, most likely place first.
///
/// `env` looks an environment variable up; Windows paths are built from
/// `LOCALAPPDATA`, `PROGRAMFILES` and `PROGRAMFILES(X86)`. The list is empty
/// for a browser that has no install location to find: Safari and Internet
/// Explorer are started by their driver, and Chrome for Testing and
/// `chrome-headless-shell` are downloaded (see [`downloaded_executable`]).
#[must_use]
pub fn install_locations(
    browser: Browser,
    platform: Platform,
    env: &dyn Fn(&str) -> Option<String>,
) -> Vec<PathBuf> {
    let windows = |relative: &str| -> Vec<PathBuf> {
        ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"]
            .iter()
            .filter_map(|variable| env(variable))
            .filter(|root| !root.is_empty())
            .map(|root| PathBuf::from(root).join(relative))
            .collect()
    };
    let paths = |list: &[&str]| list.iter().map(PathBuf::from).collect::<Vec<_>>();
    match (browser, platform) {
        (Browser::Brave, Platform::MacOs) => {
            paths(&["/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"])
        }
        (Browser::Brave, Platform::Windows) => {
            windows(r"BraveSoftware\Brave-Browser\Application\brave.exe")
        }
        (Browser::Brave, Platform::Linux) => paths(&[
            "/usr/bin/brave-browser",
            "/usr/bin/brave",
            "/opt/brave.com/brave/brave-browser",
            "/snap/bin/brave",
        ]),
        (Browser::Opera, Platform::MacOs) => {
            paths(&["/Applications/Opera.app/Contents/MacOS/Opera"])
        }
        (Browser::Opera, Platform::Windows) => windows(r"Programs\Opera\opera.exe")
            .into_iter()
            .chain(windows(r"Opera\opera.exe"))
            .collect(),
        (Browser::Opera, Platform::Linux) => paths(&[
            "/usr/bin/opera",
            "/snap/bin/opera",
            "/usr/lib/x86_64-linux-gnu/opera/opera",
        ]),
        (Browser::Comet, Platform::MacOs) => {
            paths(&["/Applications/Comet.app/Contents/MacOS/Comet"])
        }
        (Browser::Comet, Platform::Windows) => windows(r"Perplexity\Comet\Application\comet.exe"),
        (Browser::Comet, Platform::Linux) => paths(&["/usr/bin/comet"]),
        (Browser::Edge, Platform::MacOs) => {
            paths(&["/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"])
        }
        (Browser::Edge, Platform::Windows) => windows(r"Microsoft\Edge\Application\msedge.exe"),
        (Browser::Edge, Platform::Linux) => paths(&[
            "/usr/bin/microsoft-edge",
            "/usr/bin/microsoft-edge-stable",
            "/opt/microsoft/msedge/msedge",
        ]),
        _ => Vec::new(),
    }
}

/// The names `browser` goes by on the `PATH`.
#[must_use]
pub fn path_names(browser: Browser) -> &'static [&'static str] {
    match browser {
        Browser::Brave => &["brave-browser", "brave"],
        Browser::Opera => &["opera"],
        Browser::Comet => &["comet"],
        Browser::Edge => &["microsoft-edge", "microsoft-edge-stable", "msedge"],
        Browser::ChromeHeadlessShell => &["chrome-headless-shell"],
        _ => &[],
    }
}

/// The file names the executable of a downloaded Chrome build can have.
fn downloaded_names(browser: Browser) -> &'static [&'static str] {
    match browser {
        Browser::ChromeForTesting => &[
            "Google Chrome for Testing",
            "chrome",
            "chrome.exe",
            "Chromium",
        ],
        Browser::ChromeHeadlessShell => &["chrome-headless-shell", "chrome-headless-shell.exe"],
        _ => &[],
    }
}

/// Looks for the executable of a downloaded build in `root` and the folders
/// under it, at most six levels down.
///
/// `root` is the folder `sbase get cft` or `sbase get chs` unpacked a build
/// into; the build's own folder name (`chrome-mac-arm64`, ...) depends on the
/// platform, so every folder is searched. A directory that happens to carry
/// the executable's name (the `Google Chrome for Testing.app` bundle's folder
/// is not one, but `chrome` the resource folder can be) is not an executable.
#[must_use]
pub fn downloaded_executable(browser: Browser, root: &Path) -> Option<PathBuf> {
    let names = downloaded_names(browser);
    if names.is_empty() {
        return None;
    }
    find_file(root, names, 6)
}

fn find_file(dir: &Path, names: &[&str], depth: usize) -> Option<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(dir).ok()?.filter_map(Result::ok).collect();
    // A stable order, so the same folder always gives the same answer.
    entries.sort_by_key(fs::DirEntry::file_name);
    let mut folders = Vec::new();
    for entry in entries {
        let path = entry.path();
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        if is_file
            && path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|n| names.contains(&n))
        {
            return Some(path);
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            folders.push(path);
        }
    }
    if depth == 0 {
        return None;
    }
    folders
        .into_iter()
        .find_map(|folder| find_file(&folder, names, depth - 1))
}

/// Looks for a file called one of `names` in `dir` and the folders under it, at
/// most six levels down. Folders are never matched.
pub(crate) fn find_executable(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    find_file(dir, names, 6)
}

/// The program of `browser` on this machine, if it can be found.
///
/// The install locations come first, then the `PATH`; Chrome for Testing and
/// `chrome-headless-shell` are looked for in the downloads folder, under the
/// working directory. `None` for Safari and Internet Explorer, which need no
/// program path, and for a browser that is not installed.
#[must_use]
pub fn locate(browser: Browser) -> Option<PathBuf> {
    locate_with(
        browser,
        Platform::current(),
        &|name| std::env::var(name).ok(),
        Path::new(DOWNLOADS_DIR),
        &|path| path.is_file(),
        &|name| which::which(name).ok(),
    )
}

/// [`locate`] with the machine's facts passed in, so it can be tried against
/// any layout.
pub(crate) fn locate_with(
    browser: Browser,
    platform: Option<Platform>,
    env: &dyn Fn(&str) -> Option<String>,
    downloads: &Path,
    is_file: &dyn Fn(&Path) -> bool,
    on_path: &dyn Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let subfolder = match browser {
        Browser::ChromeForTesting => Some(CHROME_FOR_TESTING_DIR),
        Browser::ChromeHeadlessShell => Some(CHROME_HEADLESS_SHELL_DIR),
        _ => None,
    };
    if let Some(subfolder) = subfolder {
        return downloaded_executable(browser, &downloads.join(subfolder));
    }
    platform
        .into_iter()
        .flat_map(|platform| install_locations(browser, platform, env))
        .find(|path| is_file(path))
        .or_else(|| path_names(browser).iter().find_map(|name| on_path(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn windows_env(name: &str) -> Option<String> {
        match name {
            "LOCALAPPDATA" => Some(r"C:\Users\me\AppData\Local".to_owned()),
            "PROGRAMFILES" => Some(r"C:\Program Files".to_owned()),
            _ => None,
        }
    }

    #[test]
    fn the_install_locations_are_the_vendors_defaults() {
        let first = |browser, platform| {
            install_locations(browser, platform, &no_env)
                .first()
                .cloned()
        };
        assert_eq!(
            first(Browser::Brave, Platform::MacOs),
            Some("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser".into())
        );
        assert_eq!(
            first(Browser::Opera, Platform::MacOs),
            Some("/Applications/Opera.app/Contents/MacOS/Opera".into())
        );
        assert_eq!(
            first(Browser::Comet, Platform::MacOs),
            Some("/Applications/Comet.app/Contents/MacOS/Comet".into())
        );
        assert_eq!(
            first(Browser::Brave, Platform::Linux),
            Some("/usr/bin/brave-browser".into())
        );
        assert_eq!(
            first(Browser::Opera, Platform::Linux),
            Some("/usr/bin/opera".into())
        );
        // Edge is found so its version picks the msedgedriver to run.
        assert_eq!(
            first(Browser::Edge, Platform::MacOs),
            Some("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into())
        );
        assert_eq!(
            first(Browser::Edge, Platform::Linux),
            Some("/usr/bin/microsoft-edge".into())
        );
        assert_eq!(
            install_locations(Browser::Edge, Platform::Windows, &windows_env)[0],
            PathBuf::from(r"C:\Users\me\AppData\Local")
                .join(r"Microsoft\Edge\Application\msedge.exe")
        );
    }

    #[test]
    fn windows_locations_are_built_from_the_environment() {
        let brave = install_locations(Browser::Brave, Platform::Windows, &windows_env);
        assert_eq!(
            brave,
            [
                PathBuf::from(r"C:\Users\me\AppData\Local")
                    .join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
                PathBuf::from(r"C:\Program Files")
                    .join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
            ]
        );
        // With no variables set there is nowhere to look.
        assert!(install_locations(Browser::Opera, Platform::Windows, &no_env).is_empty());
    }

    #[test]
    fn browsers_with_nothing_to_find_have_no_locations() {
        for browser in [
            Browser::Chrome,
            Browser::Safari,
            Browser::InternetExplorer,
            Browser::ChromeForTesting,
            Browser::ChromeHeadlessShell,
        ] {
            for platform in [Platform::MacOs, Platform::Windows, Platform::Linux] {
                assert!(
                    install_locations(browser, platform, &windows_env).is_empty(),
                    "{browser:?} on {platform:?}"
                );
            }
        }
    }

    #[test]
    fn an_installed_browser_wins_over_the_path_and_the_path_is_the_fallback() {
        let downloads = Path::new("/no/downloads");
        let installed = |path: &Path| path == Path::new("/usr/bin/brave");
        let on_path =
            |name: &str| (name == "brave-browser").then(|| PathBuf::from("/opt/x/brave-browser"));
        // The second Linux candidate exists, so the PATH is not consulted.
        assert_eq!(
            locate_with(
                Browser::Brave,
                Some(Platform::Linux),
                &no_env,
                downloads,
                &installed,
                &on_path
            ),
            Some("/usr/bin/brave".into())
        );
        // Nothing is installed in a known place: the PATH answers.
        assert_eq!(
            locate_with(
                Browser::Brave,
                Some(Platform::Linux),
                &no_env,
                downloads,
                &|_| false,
                &on_path
            ),
            Some("/opt/x/brave-browser".into())
        );
        // An unknown system still has the PATH.
        assert_eq!(
            locate_with(
                Browser::Brave,
                None,
                &no_env,
                downloads,
                &|_| false,
                &on_path
            ),
            Some("/opt/x/brave-browser".into())
        );
        assert_eq!(
            locate_with(
                Browser::Opera,
                Some(Platform::Linux),
                &no_env,
                downloads,
                &|_| false,
                &on_path
            ),
            None
        );
    }

    #[test]
    fn a_downloaded_build_is_found_whatever_the_platform_folder_is_called() {
        let dir = tempfile::tempdir().unwrap();
        let cft = dir.path().join("cft");
        let exe = cft
            .join("chrome-mac-arm64")
            .join("Google Chrome for Testing.app/Contents/MacOS");
        fs::create_dir_all(&exe).unwrap();
        fs::write(exe.join("Google Chrome for Testing"), b"x").unwrap();
        fs::create_dir_all(
            cft.join("chrome-mac-arm64/Google Chrome for Testing.app/Contents/Frameworks"),
        )
        .unwrap();

        let chs = dir.path().join("chs/chrome-headless-shell-linux64");
        fs::create_dir_all(&chs).unwrap();
        fs::write(chs.join("chrome-headless-shell"), b"x").unwrap();

        let find = |browser| {
            locate_with(
                browser,
                Some(Platform::Linux),
                &no_env,
                dir.path(),
                &|_| false,
                &|_| None,
            )
        };
        assert_eq!(
            find(Browser::ChromeForTesting),
            Some(exe.join("Google Chrome for Testing"))
        );
        assert_eq!(
            find(Browser::ChromeHeadlessShell),
            Some(chs.join("chrome-headless-shell"))
        );
    }

    #[test]
    fn nothing_downloaded_finds_nothing_and_a_folder_is_not_a_program() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            downloaded_executable(Browser::ChromeForTesting, &dir.path().join("cft")),
            None
        );
        // A folder named like the program is not the program.
        fs::create_dir_all(dir.path().join("cft/chrome")).unwrap();
        assert_eq!(
            downloaded_executable(Browser::ChromeForTesting, &dir.path().join("cft")),
            None
        );
        // And other browsers are never looked for in a downloads folder.
        assert_eq!(downloaded_executable(Browser::Brave, dir.path()), None);
    }

    #[test]
    fn the_search_stops_at_a_bounded_depth() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a/b/c/d/e/f/g");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("chrome-headless-shell"), b"x").unwrap();
        assert_eq!(
            downloaded_executable(Browser::ChromeHeadlessShell, dir.path()),
            None
        );
        let shallow = dir.path().join("a/b/c");
        fs::write(shallow.join("chrome-headless-shell"), b"x").unwrap();
        assert_eq!(
            downloaded_executable(Browser::ChromeHeadlessShell, dir.path()),
            Some(shallow.join("chrome-headless-shell"))
        );
    }
}
