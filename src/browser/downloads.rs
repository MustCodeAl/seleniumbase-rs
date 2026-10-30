//! Where a browser saves downloads: `./downloaded_files`, as in Python
//! SeleniumBase.
//!
//! SeleniumBase keeps what a test downloads in `downloaded_files/`, in the
//! folder the tests run from, and its `get_downloads_folder` and
//! `assert_downloaded_file` look there. A browser the library launches is told
//! to save there, whichever engine it runs on:
//!
//! * a WebDriver session asks for it in its capabilities: for Chrome and Edge
//!   the preferences `download.default_directory`,
//!   `download.prompt_for_download` and `download.directory_upgrade` (and
//!   `profile.default_content_setting_values.automatic_downloads`, so several
//!   downloads in a row are not asked about), for Firefox the
//!   `browser.download.*` ones. A preference set in the session's own
//!   capabilities wins over these;
//! * the pure CDP engine sends `Browser.setDownloadBehavior` when it launches,
//!   so [`Page::assert_downloaded_file`](crate::sb_cdp::Page::assert_downloaded_file)
//!   finds a file before any folder has been chosen. Choosing one with
//!   [`Browser::set_download_dir`](crate::sb_cdp::Browser::set_download_dir)
//!   or a `Page` download method still replaces it.
//!
//! A browser the library only attaches to keeps its own settings.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use thirtyfour::Capabilities;

use crate::browser::config::Browser;

/// The name of the folder downloads are saved in, relative to the working
/// directory.
pub const DOWNLOADS_DIR: &str = "downloaded_files";

/// The folder downloads are saved in: [`DOWNLOADS_DIR`] in the working
/// directory, as an absolute path.
///
/// It is worked out on each call, so it follows the working directory; when
/// that cannot be read the path is the relative [`DOWNLOADS_DIR`].
///
/// # Examples
///
/// ```
/// let folder = seleniumbase_rs::browser::downloads::downloads_dir();
/// assert!(folder.ends_with("downloaded_files"));
/// ```
#[must_use]
pub fn downloads_dir() -> PathBuf {
    std::env::current_dir().map_or_else(
        |_| PathBuf::from(DOWNLOADS_DIR),
        |cwd| cwd.join(DOWNLOADS_DIR),
    )
}

/// The Chrome and Edge preferences that save downloads in `dir` without
/// asking, by dotted name.
fn chromium_preferences(dir: &Path) -> Map<String, Value> {
    let mut preferences = Map::new();
    preferences.insert(
        "download.default_directory".into(),
        json!(dir.to_string_lossy()),
    );
    preferences.insert("download.prompt_for_download".into(), json!(false));
    preferences.insert("download.directory_upgrade".into(), json!(true));
    // 1 is Chrome's "allow" in a content-setting preference.
    preferences.insert(
        "profile.default_content_setting_values.automatic_downloads".into(),
        json!(1),
    );
    preferences
}

/// The Firefox preferences that save downloads in `dir` without asking.
fn firefox_preferences(dir: &Path) -> Map<String, Value> {
    let mut preferences = Map::new();
    // 2 is "use the folder in `browser.download.dir`".
    preferences.insert("browser.download.folderList".into(), json!(2));
    preferences.insert("browser.download.dir".into(), json!(dir.to_string_lossy()));
    preferences.insert("browser.download.useDownloadDir".into(), json!(true));
    preferences.insert(
        "browser.download.manager.showWhenStarting".into(),
        json!(false),
    );
    preferences
}

/// Adds to `options` (the `goog:chromeOptions`, `ms:edgeOptions` or
/// `moz:firefoxOptions` object) the `preferences` that its `prefs` does not
/// name yet.
pub(crate) fn add_missing_preferences(options: &mut Value, preferences: Map<String, Value>) {
    if !options.is_object() {
        *options = json!({});
    }
    let Some(prefs) = options
        .as_object_mut()
        .map(|object| object.entry("prefs").or_insert_with(|| json!({})))
        .and_then(Value::as_object_mut)
    else {
        // `prefs` holds something that is not a map: leave it as it is.
        return;
    };
    for (name, value) in preferences {
        prefs.entry(name).or_insert(value);
    }
}

/// Asks, in the capabilities `caps` of a WebDriver session for `browser`, for
/// downloads to be saved in `dir`. A preference that `caps` already holds
/// stays as it is; Safari and Internet Explorer have none to set.
pub(crate) fn ask_for_downloads_in(caps: &mut Capabilities, browser: Browser, dir: &Path) {
    let (key, preferences) = match browser {
        Browser::Safari | Browser::InternetExplorer | Browser::Remote => return,
        Browser::Firefox => ("moz:firefoxOptions", firefox_preferences(dir)),
        Browser::Edge => ("ms:edgeOptions", chromium_preferences(dir)),
        _ => ("goog:chromeOptions", chromium_preferences(dir)),
    };
    match caps.get_mut(key) {
        Some(options) => add_missing_preferences(options, preferences),
        None => {
            let mut options = json!({});
            add_missing_preferences(&mut options, preferences);
            // `Capabilities::set` only fails when serialising, which a JSON
            // value cannot.
            let _ = caps.set(key, &options);
        }
    }
}

/// Creates the downloads folder for a session of `browser` that was asked to
/// save there, so the browser finds it. A failure is logged and does not stop
/// the session: the browser then saves wherever it would by default.
pub(crate) fn create_folder_for(browser: Browser) {
    if matches!(
        browser,
        Browser::Safari | Browser::InternetExplorer | Browser::Remote
    ) {
        return;
    }
    let dir = downloads_dir();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        tracing::warn!(
            folder = %dir.display(),
            %error,
            "the downloads folder cannot be created; the browser saves downloads elsewhere"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        PathBuf::from("/work/downloaded_files")
    }

    fn caps_with(key: &str, options: &Value) -> Capabilities {
        let mut caps = Capabilities::new();
        caps.set(key, options).unwrap();
        caps
    }

    fn prefs(caps: &Capabilities, key: &str) -> Value {
        serde_json::to_value(caps).unwrap()[key]["prefs"].clone()
    }

    #[test]
    fn the_folder_is_downloaded_files_in_the_working_directory() {
        let folder = downloads_dir();
        assert!(folder.is_absolute(), "{folder:?}");
        assert!(folder.ends_with("downloaded_files"), "{folder:?}");
        assert_eq!(
            folder.parent(),
            Some(std::env::current_dir().unwrap().as_path())
        );
    }

    #[test]
    fn chrome_is_asked_to_save_there_without_prompting() {
        let mut caps = caps_with("goog:chromeOptions", &json!({ "args": ["--x"] }));
        ask_for_downloads_in(&mut caps, Browser::Chrome, &dir());
        let prefs = prefs(&caps, "goog:chromeOptions");
        assert_eq!(
            prefs["download.default_directory"],
            "/work/downloaded_files"
        );
        assert_eq!(prefs["download.prompt_for_download"], false);
        assert_eq!(prefs["download.directory_upgrade"], true);
        assert_eq!(
            prefs["profile.default_content_setting_values.automatic_downloads"],
            1
        );
        let kept = serde_json::to_value(&caps).unwrap();
        assert_eq!(kept["goog:chromeOptions"]["args"], json!(["--x"]));
    }

    #[test]
    fn a_preference_that_is_set_already_wins_and_the_others_are_kept() {
        let mut caps = caps_with(
            "goog:chromeOptions",
            &json!({ "prefs": {
                "download.default_directory": "/elsewhere",
                "profile.managed_default_content_settings.images": 2
            } }),
        );
        ask_for_downloads_in(&mut caps, Browser::Brave, &dir());
        let prefs = prefs(&caps, "goog:chromeOptions");
        assert_eq!(prefs["download.default_directory"], "/elsewhere");
        assert_eq!(prefs["profile.managed_default_content_settings.images"], 2);
        assert_eq!(prefs["download.prompt_for_download"], false);
    }

    #[test]
    fn edge_and_firefox_use_their_own_options_and_preferences() {
        let mut edge = Capabilities::new();
        ask_for_downloads_in(&mut edge, Browser::Edge, &dir());
        assert_eq!(
            prefs(&edge, "ms:edgeOptions")["download.default_directory"],
            "/work/downloaded_files"
        );

        let mut firefox = Capabilities::new();
        ask_for_downloads_in(&mut firefox, Browser::Firefox, &dir());
        let prefs = prefs(&firefox, "moz:firefoxOptions");
        assert_eq!(prefs["browser.download.folderList"], 2);
        assert_eq!(prefs["browser.download.dir"], "/work/downloaded_files");
        assert_eq!(prefs["browser.download.useDownloadDir"], true);
    }

    #[test]
    fn safari_and_internet_explorer_have_nothing_to_set() {
        for browser in [Browser::Safari, Browser::InternetExplorer] {
            let mut caps = Capabilities::new();
            ask_for_downloads_in(&mut caps, browser, &dir());
            assert_eq!(serde_json::to_value(&caps).unwrap(), json!({}));
        }
    }

    #[test]
    fn prefs_that_are_not_a_map_are_left_alone() {
        let mut caps = caps_with("goog:chromeOptions", &json!({ "prefs": "odd" }));
        ask_for_downloads_in(&mut caps, Browser::Chrome, &dir());
        assert_eq!(prefs(&caps, "goog:chromeOptions"), json!("odd"));
    }
}
