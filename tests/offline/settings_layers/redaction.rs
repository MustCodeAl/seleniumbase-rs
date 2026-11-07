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
fn a_secret_with_a_quote_or_a_backslash_is_hidden_as_serde_writes_it() {
    // serde quotes a text with its escapes (`zq9\\zq9` for `zq9\zq9`), so the secret as it is
    // written in the file is not the text in the message; neither of the two may be there.
    for secret in [r"zq9\zq9", "zq9\"zq9"] {
        let in_toml = secret.replace('\\', "\\\\").replace('"', "\\\"");
        let flat = refused_secret("headless", &in_toml);
        assert!(!flat.contains("zq9"), "{flat}");
        let table = refused(
            "sbase_config.toml",
            &format!("[run]\ndemo_mode = \"{in_toml}\"\n"),
        );
        assert!(!table.contains("zq9"), "{table}");
        assert!(table.contains("expected a boolean"), "{table}");
    }
}

#[test]
fn an_item_of_a_list_that_a_reader_refuses_is_hidden_with_the_value() {
    // The reader of `firefox_pref` names the entry it refuses, which is part of the value.
    let error = refused_secret("firefox_pref", "good:1,zq9entry");
    assert!(!error.contains("zq9entry"), "{error}");
    assert!(error.contains("firefox_pref"), "{error}");
}

#[test]
fn a_long_secret_that_spells_a_word_of_the_message_hides_that_word_too() {
    // A value of three characters or more is hidden wherever it stands alone, and the word of
    // the message that it spells is one of those places.
    let error = refused("sbase_config.toml", "[run]\ndemo_mode = \"boolean\"\n");
    assert!(error.contains("string \"…\""), "{error}");
    assert!(error.contains("expected a …"), "{error}");
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
