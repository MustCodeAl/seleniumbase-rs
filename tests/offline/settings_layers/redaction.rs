//! A value the settings file gives that is refused is not repeated in the message, as it may be
//! a secret; only the value is hidden, not the letters of the message that spell it.

use super::refused;

/// The text of the error of the settings file `key = "<secret>"`.
fn refused_secret(key: &str, secret: &str) -> String {
    refused("sbase_config.toml", &format!("{key} = \"{secret}\"\n"))
}

#[test]
fn a_hidden_value_is_the_quoted_one_and_never_a_letter_of_the_message() {
    // A secret of one letter is a letter of the words of the message: the message for it must
    // be the message for a long secret, with the secret hidden in the same places.
    let keys = [
        "headless",
        "remote_webdriver_timeout_seconds",
        "page_load_strategy",
        "window_size",
        "protocol",
        "device_metrics",
        "mode",
        "browser",
        "port",
        "xvfb_metrics",
        "window_position",
    ];
    for key in keys {
        let long = refused_secret(key, "zq9");
        assert!(!long.contains("zq9"), "{key}: {long}");
        for letter in ["a", "e", "o", "s"] {
            let short = refused_secret(key, letter);
            assert_eq!(short, long, "{key} = {letter:?}");
        }
    }
}

#[test]
fn a_one_letter_secret_leaves_the_words_of_the_message_alone() {
    let error = refused_secret("headless", "a");
    assert!(error.ends_with("cannot parse bool: …"), "{error}");
    assert!(error.contains("failed to parse TOML settings"), "{error}");

    // The value of a table is read by serde, which quotes it with double quotes.
    let error = refused("sbase_config.toml", "[run]\ndemo_mode = \"a\"\n");
    assert!(error.contains("expected a boolean"), "{error}");
    assert!(error.contains("string \"…\""), "{error}");
}
