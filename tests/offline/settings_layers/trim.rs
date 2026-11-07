//! One rule reads a value in every source: it is trimmed, and a blank one is unset. The
//! variables of the process, the pairs of a command line and the flat keys of a settings file
//! all go through the readers of the option table, so none keeps the blanks around a value.

use std::path::Path;

use seleniumbase_rs::config::settings::Settings;

use super::layered;

/// Values with blanks around them, as a CI matrix or a shell script may export them.
const PADDED: [(&str, &str); 5] = [
    ("SB_HEADLESS", " true "),
    ("SB_USER_AGENT", "  agent-of-the-test\t"),
    ("SB_TIMEOUT", " 7 "),
    ("SB_BROWSER", "\nfirefox\n"),
    ("SB_WINDOW_SIZE", " 800,600 "),
];

#[test]
fn a_padded_value_is_trimmed_in_the_variables_the_pairs_and_the_file() {
    let check = Settings::check_with(None::<&Path>, |name| {
        PADDED
            .iter()
            .find(|(variable, _)| *variable == name)
            .map(|(_, value)| (*value).to_owned())
    });
    assert!(check.problems.is_empty(), "{:?}", check.problems);
    let by_variable = check.settings;
    assert!(by_variable.browser.headless);
    assert_eq!(
        by_variable.browser.user_agent.as_deref(),
        Some("agent-of-the-test")
    );
    assert_eq!(by_variable.browser.kind.to_string(), "firefox");

    let mut by_pair = Settings::default();
    by_pair
        .apply_options(
            &PADDED
                .iter()
                .map(|(name, value)| (*name, (*value).to_owned()))
                .collect::<Vec<_>>(),
        )
        .expect("padded values are valid");
    assert_eq!(by_pair, by_variable);

    let by_file = layered(
        "headless = \" true \"\nuser_agent = \"  agent-of-the-test\\t\"\ntimeout_seconds = 7\n\
         browser = \" firefox \"\nwindow_size = \" 800,600 \"\n",
        &[],
    );
    assert_eq!(by_file, by_variable);
}

#[test]
fn a_blank_value_is_unset_in_the_variables_the_pairs_and_the_file() {
    // It does not even replace what a lower source says.
    let blank = layered(
        "user_agent = \"agent-of-the-file\"\nproxy = \"proxy.test:8080\"\n",
        &[("SB_USER_AGENT", " \t "), ("SB_PROXY", "")],
    );
    assert_eq!(
        blank.browser.user_agent.as_deref(),
        Some("agent-of-the-file")
    );
    assert_eq!(
        blank.browser.proxy.server.as_deref(),
        Some("proxy.test:8080")
    );

    let mut settings = blank;
    settings
        .apply_options(&[
            ("SB_USER_AGENT", "   ".to_owned()),
            ("SB_PROXY", String::new()),
        ])
        .expect("blank values are unset");
    assert_eq!(
        settings.browser.user_agent.as_deref(),
        Some("agent-of-the-file")
    );

    // A blank value of the file is unset too, so the variable below it stays what it is.
    let blank = layered(
        "user_agent = \"   \"\nproxy = \"\"\n",
        &[("SB_USER_AGENT", "agent-of-the-variable")],
    );
    assert_eq!(
        blank.browser.user_agent.as_deref(),
        Some("agent-of-the-variable")
    );
    assert_eq!(blank.browser.proxy.server, None);
}
