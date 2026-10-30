//! Keeps Chrome's password prompts from taking the keyboard from the page.
//!
//! Logging in with a password that is on Chrome's list of leaked ones (the
//! demo site saucedemo.com's `secret_sauce` is) makes the browser show a "data
//! breach" prompt, and its password manager may offer to save the login. Both
//! are part of the browser's own interface, not of the page. While one is up,
//! Chrome hands the keys that `Input.dispatchKeyEvent` delivers to the prompt,
//! and answers the command at once as if all was well: the page never sees
//! them, `type_text` returns `Ok`, and the field stays empty or keeps only the
//! first characters. Waiting does not help, because the prompt stays.
//!
//! Python SeleniumBase starts Chrome with these preferences off, in both its
//! WebDriver and its CDP mode (`browser_launcher.py` and
//! `cdp_driver/config.py`). A browser this crate starts gets the same:
//!
//! * a WebDriver session asks for them in its capabilities
//!   ([`ask_for_quiet_passwords_in`]); a preference the capabilities already
//!   name wins;
//! * the pure CDP engine writes them into the profile of a browser it starts
//!   with a throwaway profile (`crate::sb_cdp::LaunchOptions`); a profile the
//!   caller names with `user_data_dir` is left as it is, as Python leaves it.
//!
//! A browser the library only attaches to keeps its own settings.

use serde_json::{json, Map, Value};
use thirtyfour::Capabilities;

use crate::browser::config::Browser;
use crate::browser::downloads::add_missing_preferences;

/// The Chrome and Edge preferences that turn the password prompts off, by
/// dotted name: the leaked-password check, the password manager, and the
/// offer to save a login. `profile.password_manager_enabled` is the older name
/// of the manager's switch, which current Chrome no longer registers; Python
/// sets it too, so older browsers are covered.
pub(crate) fn preferences() -> Map<String, Value> {
    let mut preferences = Map::new();
    preferences.insert(
        "profile.password_manager_leak_detection".into(),
        json!(false),
    );
    preferences.insert("profile.password_manager_enabled".into(), json!(false));
    preferences.insert("credentials_enable_service".into(), json!(false));
    preferences
}

/// Asks, in the capabilities `caps` of a WebDriver session for `browser`, for
/// the password prompts to stay off. A preference that `caps` already holds
/// stays as it is; Firefox, Safari and Internet Explorer have no such prompts
/// to turn off, and a remote browser is left to its server.
pub(crate) fn ask_for_quiet_passwords_in(caps: &mut Capabilities, browser: Browser) {
    let key = match browser {
        Browser::Firefox | Browser::Safari | Browser::InternetExplorer | Browser::Remote => return,
        Browser::Edge => "ms:edgeOptions",
        _ => "goog:chromeOptions",
    };
    match caps.get_mut(key) {
        Some(options) => add_missing_preferences(options, preferences()),
        None => {
            let mut options = json!({});
            add_missing_preferences(&mut options, preferences());
            // `Capabilities::set` only fails when serialising, which a JSON
            // value cannot.
            let _ = caps.set(key, &options);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps_with(key: &str, options: &Value) -> Capabilities {
        let mut caps = Capabilities::new();
        caps.set(key, options).unwrap();
        caps
    }

    fn prefs(caps: &Capabilities, key: &str) -> Value {
        serde_json::to_value(caps).unwrap()[key]["prefs"].clone()
    }

    #[test]
    fn the_leak_check_the_password_manager_and_the_save_offer_are_off() {
        let preferences = preferences();
        assert_eq!(
            preferences["profile.password_manager_leak_detection"],
            false
        );
        assert_eq!(preferences["profile.password_manager_enabled"], false);
        assert_eq!(preferences["credentials_enable_service"], false);
        assert_eq!(preferences.len(), 3, "{preferences:?}");
    }

    #[test]
    fn chrome_is_asked_to_keep_the_prompts_off_and_keeps_its_other_options() {
        let mut caps = caps_with("goog:chromeOptions", &json!({ "args": ["--x"] }));
        ask_for_quiet_passwords_in(&mut caps, Browser::Chrome);
        let prefs = prefs(&caps, "goog:chromeOptions");
        assert_eq!(prefs["profile.password_manager_leak_detection"], false);
        assert_eq!(prefs["profile.password_manager_enabled"], false);
        assert_eq!(prefs["credentials_enable_service"], false);
        let kept = serde_json::to_value(&caps).unwrap();
        assert_eq!(kept["goog:chromeOptions"]["args"], json!(["--x"]));
    }

    #[test]
    fn a_preference_that_is_set_already_wins_and_the_others_are_kept() {
        let mut caps = caps_with(
            "goog:chromeOptions",
            &json!({ "prefs": {
                "profile.password_manager_leak_detection": true,
                "profile.managed_default_content_settings.images": 2
            } }),
        );
        ask_for_quiet_passwords_in(&mut caps, Browser::Brave);
        let prefs = prefs(&caps, "goog:chromeOptions");
        assert_eq!(prefs["profile.password_manager_leak_detection"], true);
        assert_eq!(prefs["profile.managed_default_content_settings.images"], 2);
        assert_eq!(prefs["profile.password_manager_enabled"], false);
    }

    #[test]
    fn edge_uses_its_own_options() {
        let mut caps = Capabilities::new();
        ask_for_quiet_passwords_in(&mut caps, Browser::Edge);
        assert_eq!(
            prefs(&caps, "ms:edgeOptions")["profile.password_manager_leak_detection"],
            false
        );
    }

    #[test]
    fn browsers_without_these_prompts_have_nothing_to_set() {
        for browser in [
            Browser::Firefox,
            Browser::Safari,
            Browser::InternetExplorer,
            Browser::Remote,
        ] {
            let mut caps = Capabilities::new();
            ask_for_quiet_passwords_in(&mut caps, browser);
            assert_eq!(
                serde_json::to_value(&caps).unwrap(),
                json!({}),
                "{browser:?}"
            );
        }
    }

    #[test]
    fn prefs_that_are_not_a_map_are_left_alone() {
        let mut caps = caps_with("goog:chromeOptions", &json!({ "prefs": "odd" }));
        ask_for_quiet_passwords_in(&mut caps, Browser::Chrome);
        assert_eq!(prefs(&caps, "goog:chromeOptions"), json!("odd"));
    }

    #[test]
    fn the_downloads_and_the_prompts_preferences_sit_side_by_side() {
        let mut caps = Capabilities::new();
        crate::browser::downloads::ask_for_downloads_in(
            &mut caps,
            Browser::Chrome,
            std::path::Path::new("/work/downloaded_files"),
        );
        ask_for_quiet_passwords_in(&mut caps, Browser::Chrome);
        let prefs = prefs(&caps, "goog:chromeOptions");
        assert_eq!(prefs["download.prompt_for_download"], false);
        assert_eq!(prefs["profile.password_manager_leak_detection"], false);
    }
}
