//! The plugin surface of the real `sbrs` binary: `--plugin`, `--plugin-opt`, the
//! `[plugins]` table of the settings file, `SB_PLUGINS`, and `sbrs plugin
//! list|info`.
//!
//! Nothing here starts a browser. The WebDriver engine runs against a fake
//! WebDriver server on a loopback port (the tests read the capabilities it was
//! asked for and the DevTools commands sent through it), and the pure CDP
//! engine (`sbrs devtools`) against a script that records its arguments and
//! exits, so what the tests see is what `sbrs` asked a browser to do. A
//! plugin's effect on a real Chrome is tested in the library
//! (`tests/webdriver_plugins_chrome.rs`, `tests/sb_cdp_plugins.rs`).
//! Run with `cargo test -p seleniumbase-rs-cli`.

#![cfg(feature = "plugins-builtin")]

#[cfg(unix)]
#[path = "support/fake_chrome.rs"]
mod fake_chrome;
#[path = "support/fake_webdriver.rs"]
mod fake_webdriver;
#[path = "support/sbrs.rs"]
mod sbrs;

use std::path::Path;
use std::process::Output;

use fake_webdriver::FakeWebDriver;
use sbrs::{row, run, stderr, stdout, words};

/// A switch shield adds to every browser it launches.
const SHIELD_SWITCH: &str = "--disable-background-networking";

const OPEN: [&str; 2] = ["open", "https://example.test/"];

/// What `sbrs` did against a fake WebDriver server.
struct Run {
    driver: FakeWebDriver,
    output: Output,
}

impl Run {
    fn stderr(&self) -> String {
        stderr(&self.output)
    }

    /// The command-line arguments of the Chrome the session asked for.
    fn chrome_args(&self) -> Vec<String> {
        self.driver.chrome_args().expect("a chrome session")
    }

    fn has_switch(&self, switch: &str) -> bool {
        self.chrome_args().iter().any(|arg| arg == switch)
    }

    fn has_switch_starting(&self, start: &str) -> bool {
        self.chrome_args().iter().any(|arg| arg.starts_with(start))
    }

    /// The URL patterns of every `Network.setBlockedURLs` sent to the browser.
    fn blocked_urls(&self) -> Vec<String> {
        self.driver
            .requests()
            .iter()
            .filter(|request| request.path == "/goog/cdp/execute")
            .filter(|request| request.body["cmd"] == "Network.setBlockedURLs")
            .flat_map(|request| {
                request.body["params"]["urls"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|url| url.as_str().map(str::to_owned))
            .collect()
    }
}

/// Runs `sbrs <flags> --webdriver <fake> <command...>` in `dir` with `env` set.
fn against_a_fake_server_in(
    dir: &Path,
    env: &[(&str, &str)],
    flags: &[&str],
    command: &[&str],
) -> Run {
    let driver = FakeWebDriver::start();
    let mut args = vec!["--webdriver", driver.url()];
    args.extend(flags);
    args.extend(command);
    let output = run(dir, env, &args);
    Run { driver, output }
}

/// The same in a folder with a settings file that holds `settings`, when given.
fn against_a_fake_server(
    settings: Option<&str>,
    env: &[(&str, &str)],
    flags: &[&str],
    command: &[&str],
) -> Run {
    let dir = tempfile::tempdir().expect("temp dir");
    if let Some(text) = settings {
        std::fs::write(dir.path().join("sbase_config.toml"), text).expect("write the settings");
    }
    against_a_fake_server_in(dir.path(), env, flags, command)
}

fn in_a_new_folder(env: &[(&str, &str)], args: &[&str]) -> Output {
    let dir = tempfile::tempdir().expect("temp dir");
    run(dir.path(), env, args)
}

// ---------------------------------------------------------------------------
// sbrs plugin list / info
// ---------------------------------------------------------------------------

#[test]
fn plugin_list_shows_every_builtin_plugin_and_none_is_on_by_default() {
    let output = in_a_new_folder(&[], &["plugin", "list"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert_eq!(
        words(text.lines().next().expect("a header")),
        ["NAME", "CATEGORY", "ENABLED", "SUMMARY"]
    );
    for (name, category) in [
        ("shield", "privacy"),
        ("block-resources", "core"),
        ("crx", "extension"),
    ] {
        let columns = row(&text, name);
        assert_eq!(columns[1], category, "{name}");
        assert_eq!(columns[2], "no", "{name} is off");
        assert!(columns.len() > 3, "{name} has a summary: {columns:?}");
    }
    assert!(text.contains("--plugin NAME"), "{text}");
}

#[test]
fn plugin_list_marks_what_the_file_the_variable_and_the_flags_turn_on() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "[plugins]\nenable = [\"crx\"]\n",
    )
    .unwrap();
    let enabled = |output: &Output| -> Vec<String> {
        let text = stdout(output);
        ["shield", "block-resources", "crx"]
            .into_iter()
            .filter(|name| row(&text, name)[2] == "yes")
            .map(str::to_owned)
            .collect()
    };

    let file = run(dir.path(), &[], &["plugin", "list"]);
    assert!(file.status.success(), "{}", stderr(&file));
    assert_eq!(enabled(&file), ["crx"], "the file alone");

    let variable = run(dir.path(), &[("SB_PLUGINS", "shield")], &["plugin", "list"]);
    assert_eq!(
        enabled(&variable),
        ["shield"],
        "SB_PLUGINS replaces the names the file enables"
    );

    let flag = run(
        dir.path(),
        &[("SB_PLUGINS", "shield")],
        &["--plugin", "block-resources", "plugin", "list"],
    );
    assert!(flag.status.success(), "{}", stderr(&flag));
    assert_eq!(
        enabled(&flag),
        ["shield", "block-resources"],
        "--plugin adds to the variable"
    );

    let off = run(dir.path(), &[("SB_PLUGINS", "")], &["plugin", "list"]);
    assert!(
        enabled(&off).is_empty(),
        "an empty SB_PLUGINS turns all off"
    );
}

#[test]
fn plugin_list_does_not_read_the_settings_that_are_not_plugins() {
    // A bad value of another variable stops a browser command, not this.
    let output = in_a_new_folder(&[("SB_HEADLESS", "maybe")], &["plugin", "list"]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn plugin_info_shows_what_a_plugin_declares() {
    let output = in_a_new_folder(&[], &["plugin", "info", "shield"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.starts_with("shield "),
        "name and version first: {text}"
    );
    for expected in [
        "category      privacy",
        "enabled       no",
        "a Chromium-based browser",
        "launch.args",
        "enforced",
        "--plugin-opt 'shield.KEY=VALUE'",
        "[plugins.shield]",
    ] {
        assert!(text.contains(expected), "{expected:?} in:\n{text}");
    }
}

#[test]
fn plugin_info_shows_the_options_in_effect_and_whether_the_plugin_is_on() {
    let output = in_a_new_folder(
        &[],
        &[
            "--plugin",
            "shield",
            "--plugin-opt",
            r#"shield.level="strict""#,
            "plugin",
            "info",
            "shield",
        ],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("enabled       yes"), "{text}");
    assert!(text.contains(r#"level = "strict""#), "{text}");
}

#[test]
fn plugin_info_of_an_unknown_name_exits_2_and_lists_the_known_ones() {
    let output = in_a_new_folder(&[], &["plugin", "info", "sheild"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    let message = stderr(&output);
    assert!(message.contains("`sheild`"), "{message}");
    assert!(
        message.contains("known: shield, block-resources, crx"),
        "{message}"
    );
}

#[test]
fn plugin_needs_a_subcommand_and_info_needs_a_name() {
    for args in [
        &["plugin"][..],
        &["plugin", "info"],
        &["plugin", "nonsense"],
    ] {
        let output = in_a_new_folder(&[], args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn help_states_the_precedence_of_the_plugin_options() {
    let output = in_a_new_folder(&[], &["--help"]);

    assert!(output.status.success(), "{}", stderr(&output));
    // The help may wrap lines; compare the words.
    let help = words(&stdout(&output)).join(" ");
    assert!(help.contains("--plugin"), "{help}");
    assert!(help.contains("--plugin-opt"), "{help}");
    assert!(
        help.contains("the command line, then SB_PLUGINS"),
        "the precedence is in the help: {help}"
    );
    assert!(help.contains("then the settings file"), "{help}");
}

// ---------------------------------------------------------------------------
// The WebDriver engine: every command that starts a browser with BaseCase
// ---------------------------------------------------------------------------

#[test]
fn a_plugin_flag_runs_the_plugin_in_the_browser_the_command_starts() {
    let with = against_a_fake_server(None, &[], &["--plugin", "shield"], &OPEN);
    assert!(with.output.status.success(), "{}", with.stderr());
    assert!(
        stdout(&with.output).contains("Title: Fake page"),
        "the command still ran: {}",
        stdout(&with.output)
    );
    assert!(with.has_switch(SHIELD_SWITCH), "{:?}", with.chrome_args());

    let without = against_a_fake_server(None, &[], &[], &OPEN);
    assert!(without.output.status.success(), "{}", without.stderr());
    assert!(
        !without.has_switch(SHIELD_SWITCH),
        "no plugin, no shield switch: {:?}",
        without.chrome_args()
    );
}

#[test]
fn other_commands_that_start_a_browser_get_the_plugins_too() {
    // A page command acts on the start page, which may be named after the command too.
    for command in [
        &["get-title", "--url", "https://example.test/"][..],
        &["smoke", "https://example.test/", "--title-contains", "Fake"],
        &["--url", "https://example.test/", "get-current-url"],
    ] {
        let run = against_a_fake_server(None, &[], &["--plugin", "shield"], command);
        assert!(run.output.status.success(), "{command:?}: {}", run.stderr());
        assert!(run.has_switch(SHIELD_SWITCH), "{command:?}");
    }
}

#[test]
fn the_plugins_table_of_the_settings_file_turns_a_plugin_on() {
    let file = "[plugins]\nenable = [\"shield\"]\n";

    let run = against_a_fake_server(Some(file), &[], &[], &OPEN);

    assert!(run.output.status.success(), "{}", run.stderr());
    assert!(run.has_switch(SHIELD_SWITCH), "{:?}", run.chrome_args());
}

#[test]
fn a_config_flag_names_the_file_whose_plugins_table_counts() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("elsewhere.toml"),
        "[plugins]\nenable = [\"shield\"]\n",
    )
    .unwrap();

    let run = against_a_fake_server_in(dir.path(), &[], &["--config", "elsewhere.toml"], &OPEN);

    assert!(run.output.status.success(), "{}", run.stderr());
    assert!(run.has_switch(SHIELD_SWITCH));
}

#[test]
fn sb_plugins_turns_a_plugin_on_and_replaces_the_names_of_the_file() {
    let from_variable = against_a_fake_server(None, &[("SB_PLUGINS", "shield")], &[], &OPEN);
    assert!(
        from_variable.output.status.success(),
        "{}",
        from_variable.stderr()
    );
    assert!(from_variable.has_switch(SHIELD_SWITCH));

    let file = "[plugins]\nenable = [\"shield\"]\n";
    let off = against_a_fake_server(Some(file), &[("SB_PLUGINS", "")], &[], &OPEN);
    assert!(off.output.status.success(), "{}", off.stderr());
    assert!(
        !off.has_switch(SHIELD_SWITCH),
        "SB_PLUGINS= beats the file: {:?}",
        off.chrome_args()
    );

    // The variable names other plugins than the file: the file's are not run.
    let file = "[plugins]\nenable = [\"block-resources\"]\n";
    let swapped = against_a_fake_server(Some(file), &[("SB_PLUGINS", "shield")], &[], &OPEN);
    assert!(swapped.output.status.success(), "{}", swapped.stderr());
    assert!(swapped.has_switch(SHIELD_SWITCH));
    assert!(
        swapped.blocked_urls().is_empty(),
        "block-resources was replaced: {:?}",
        swapped.blocked_urls()
    );
}

#[test]
fn a_plugin_flag_adds_to_the_variable_and_the_file() {
    let file = "[plugins]\nenable = [\"block-resources\"]\n";

    let run = against_a_fake_server(
        Some(file),
        &[],
        &["--plugin", "shield", "--plugin", "block-resources"],
        &OPEN,
    );

    assert!(run.output.status.success(), "{}", run.stderr());
    assert!(run.has_switch(SHIELD_SWITCH), "shield was added");
    assert!(
        !run.blocked_urls().is_empty(),
        "block-resources, enabled by the file and named again, ran once"
    );

    let on_the_variable = against_a_fake_server(
        None,
        &[("SB_PLUGINS", "shield")],
        &["--plugin", "block-resources"],
        &OPEN,
    );
    assert!(
        on_the_variable.output.status.success(),
        "{}",
        on_the_variable.stderr()
    );
    assert!(on_the_variable.has_switch(SHIELD_SWITCH));
    assert!(!on_the_variable.blocked_urls().is_empty());
}

#[test]
fn a_plugin_option_flag_beats_the_file_key_by_key() {
    let file = "[plugins]\nenable = [\"shield\"]\n\n[plugins.shield]\nlevel = \"basic\"\n\
                extra_block_hosts = [\"tracker.example\"]\n";
    let strict_rule = "--host-resolver-rules=";

    let from_file = against_a_fake_server(Some(file), &[], &[], &OPEN);
    assert!(from_file.output.status.success(), "{}", from_file.stderr());
    assert!(
        !from_file.has_switch_starting(strict_rule),
        "the file asks for the basic level: {:?}",
        from_file.chrome_args()
    );

    let flagged = against_a_fake_server(
        Some(file),
        &[],
        &["--plugin-opt", r#"shield.level="strict""#],
        &OPEN,
    );
    assert!(flagged.output.status.success(), "{}", flagged.stderr());
    assert!(
        flagged.has_switch_starting(strict_rule),
        "the flag raised the level: {:?}",
        flagged.chrome_args()
    );
    assert!(
        flagged
            .blocked_urls()
            .iter()
            .any(|url| url.contains("tracker.example")),
        "the other option of the file stayed: {:?}",
        flagged.blocked_urls()
    );
}

#[test]
fn a_plugin_option_flag_works_for_a_plugin_the_variable_turned_on() {
    let run = against_a_fake_server(
        None,
        &[("SB_PLUGINS", "shield")],
        &["--plugin-opt", r#"shield.level="strict""#],
        &OPEN,
    );

    assert!(run.output.status.success(), "{}", run.stderr());
    assert!(run.has_switch_starting("--host-resolver-rules="));
}

/// A mistake is reported with status 2, before any browser is asked for.
fn assert_refused_before_a_browser(run: &Run, wanted: &[&str]) {
    assert_eq!(run.output.status.code(), Some(2), "{}", run.stderr());
    let message = run.stderr();
    for text in wanted {
        assert!(message.contains(text), "{text:?} in:\n{message}");
    }
    assert!(
        run.driver.requests().is_empty(),
        "no session was requested: {:?}",
        run.driver.requests()
    );
}

#[test]
fn an_unknown_plugin_is_refused_with_the_known_names_and_no_browser_starts() {
    let run = against_a_fake_server(None, &[], &["--plugin", "sheild"], &OPEN);
    assert_refused_before_a_browser(&run, &["`sheild`", "known: shield, block-resources, crx"]);

    let from_variable = against_a_fake_server(None, &[("SB_PLUGINS", "shield,sheild")], &[], &OPEN);
    assert_refused_before_a_browser(&from_variable, &["`sheild`"]);

    let file = "[plugins]\nenable = [\"nope\"]\n";
    let from_file = against_a_fake_server(Some(file), &[], &[], &OPEN);
    assert_refused_before_a_browser(&from_file, &["`nope`"]);
}

#[test]
fn an_option_of_a_plugin_nothing_turns_on_is_refused() {
    let run = against_a_fake_server(
        None,
        &[],
        &["--plugin-opt", r#"shield.level="strict""#],
        &OPEN,
    );

    assert_refused_before_a_browser(&run, &["`shield`", "--plugin shield", "SB_PLUGINS"]);
}

#[test]
fn an_option_the_plugin_does_not_know_is_refused() {
    let run = against_a_fake_server(
        None,
        &[],
        &[
            "--plugin",
            "shield",
            "--plugin-opt",
            r#"shield.lvel="strict""#,
        ],
        &OPEN,
    );

    assert_refused_before_a_browser(&run, &["shield", "lvel"]);
}

#[test]
fn a_value_without_toml_quotes_is_refused_with_the_quoted_spelling() {
    let run = against_a_fake_server(
        None,
        &[],
        &["--plugin", "shield", "--plugin-opt", "shield.level=strict"],
        &OPEN,
    );

    assert_refused_before_a_browser(
        &run,
        &[
            "not a TOML value",
            r#"--plugin-opt 'shield.level="strict"'"#,
        ],
    );
}

#[test]
fn malformed_plugin_flags_are_refused() {
    for flags in [
        &["--plugin", ""][..],
        &["--plugin", "shield,crx"],
        &["--plugin", "shield", "--plugin-opt", "shield.level"],
        &["--plugin", "shield", "--plugin-opt", "level=1"],
    ] {
        let run = against_a_fake_server(None, &[], flags, &OPEN);
        assert_refused_before_a_browser(&run, &["--plugin"]);
    }
}

#[test]
fn a_command_that_starts_no_browser_does_not_check_the_plugins() {
    let output = in_a_new_folder(&[], &["--plugin", "sheild", "version"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("sbrs "), "{}", stdout(&output));
}

#[test]
fn a_command_that_starts_no_browser_ignores_the_plugin_options_even_when_they_are_wrong() {
    for flags in [
        &["--plugin-opt", r#"shield.level="strict""#][..],
        &["--plugin-opt", "nonsense"],
        &["--plugin", "a b"],
        &["--plugin", ""],
        &["--plugin", "shield,crx"],
    ] {
        let mut args = flags.to_vec();
        args.push("version");
        let output = in_a_new_folder(&[], &args);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
        assert!(stdout(&output).starts_with("sbrs "), "{args:?}");
    }
}

#[test]
fn a_flag_that_turns_a_plugin_back_on_keeps_the_options_of_the_file() {
    let file = "[plugins]\nenable = [\"shield\"]\n\n[plugins.shield]\nlevel = \"strict\"\n";
    let variable = [("SB_PLUGINS", "block-resources")];

    // The variable does not name shield, so shield is off.
    let without = against_a_fake_server(Some(file), &variable, &[], &OPEN);
    assert!(without.output.status.success(), "{}", without.stderr());
    assert!(
        !without.has_switch(SHIELD_SWITCH),
        "{:?}",
        without.chrome_args()
    );

    // `--plugin shield` brings it back at the level the file chose.
    let back = against_a_fake_server(Some(file), &variable, &["--plugin", "shield"], &OPEN);
    assert!(back.output.status.success(), "{}", back.stderr());
    assert!(back.has_switch(SHIELD_SWITCH), "{:?}", back.chrome_args());
    assert!(
        back.has_switch_starting("--host-resolver-rules="),
        "the strict level of the file: {:?}",
        back.chrome_args()
    );
    assert!(
        !back.blocked_urls().is_empty(),
        "block-resources, named by the variable, ran too"
    );
}

#[test]
fn each_active_plugin_is_logged_with_the_place_that_turned_it_on() {
    let info = [("SB_LOG_LEVEL", "info")];
    let file = "[plugins]\nenable = [\"shield\"]\n";

    let from_file = against_a_fake_server(Some(file), &info, &[], &OPEN);
    assert!(from_file.output.status.success(), "{}", from_file.stderr());
    let log = from_file.stderr();
    for wanted in [
        "plugin enabled",
        "shield",
        "the settings file in the current folder",
    ] {
        assert!(log.contains(wanted), "{wanted:?} in:\n{log}");
    }

    let from_flag = against_a_fake_server(None, &info, &["--plugin", "block-resources"], &OPEN);
    let log = from_flag.stderr();
    assert!(log.contains("block-resources"), "{log}");
    assert!(log.contains("--plugin"), "{log}");

    let from_variable = against_a_fake_server(
        None,
        &[("SB_LOG_LEVEL", "info"), ("SB_PLUGINS", "shield")],
        &[],
        &OPEN,
    );
    assert!(
        from_variable.stderr().contains("SB_PLUGINS"),
        "{}",
        from_variable.stderr()
    );

    let none = against_a_fake_server(None, &info, &[], &OPEN);
    assert!(
        !none.stderr().contains("plugin enabled"),
        "no plugin, no line: {}",
        none.stderr()
    );
}

#[test]
fn the_plugin_commands_read_only_the_plugin_table_of_the_file() {
    // `headless` is not a boolean: a browser command stops on it, `plugin list` does not.
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "headless = \"maybe\"\n[plugins]\nenable = [\"shield\"]\n",
    )
    .unwrap();

    let output = run(dir.path(), &[], &["plugin", "list"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(row(&stdout(&output), "shield")[2], "yes");
}

// ---------------------------------------------------------------------------
// sbrs test passes the names on to the tests
// ---------------------------------------------------------------------------

/// A `cargo` that records the `SB_PLUGINS` it was started with.
#[cfg(unix)]
struct FakeCargo {
    dir: tempfile::TempDir,
}

#[cfg(unix)]
impl FakeCargo {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("cargo");
        std::fs::write(
            &script,
            "#!/bin/sh\nif [ -n \"${SB_PLUGINS+set}\" ]; then printf 'SB_PLUGINS=%s' \"$SB_PLUGINS\"; \
             else printf 'unset'; fi > \"$(dirname \"$0\")/seen.txt\"\n",
        )
        .expect("write the script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::process::Command::new(&script).status();
        let _ = std::fs::remove_file(dir.path().join("seen.txt"));
        Self { dir }
    }

    /// The PATH that finds this `cargo` first.
    fn path(&self) -> String {
        let old = std::env::var("PATH").unwrap_or_default();
        format!("{}:{old}", self.dir.path().display())
    }

    /// What the last `cargo` saw of `SB_PLUGINS`, or `None` if none ran.
    fn seen(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.path().join("seen.txt")).ok()
    }
}

#[cfg(unix)]
#[test]
fn test_passes_the_plugin_names_on_as_sb_plugins() {
    let cargo = FakeCargo::new();
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "[plugins]\nenable = [\"crx\"]\n",
    )
    .unwrap();
    let path = cargo.path();

    let output = run(
        dir.path(),
        &[("PATH", path.as_str())],
        &["--plugin", "shield", "test"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        cargo.seen().as_deref(),
        Some("SB_PLUGINS=crx,shield"),
        "the names the file enables and the flag adds"
    );

    // The variable replaces the file's names, and the flag adds to them.
    let output = run(
        dir.path(),
        &[("PATH", path.as_str()), ("SB_PLUGINS", "block-resources")],
        &["--plugin", "shield", "test"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        cargo.seen().as_deref(),
        Some("SB_PLUGINS=block-resources,shield")
    );
}

#[cfg(unix)]
#[test]
fn test_does_not_set_sb_plugins_when_no_plugin_flag_was_given() {
    let cargo = FakeCargo::new();
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "[plugins]\nenable = [\"crx\"]\n",
    )
    .unwrap();

    let path = cargo.path();
    let output = run(dir.path(), &[("PATH", path.as_str())], &["test"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        cargo.seen().as_deref(),
        Some("unset"),
        "the tests read the file themselves"
    );
}

#[cfg(unix)]
#[test]
fn test_refuses_plugin_options_it_cannot_pass_on() {
    let cargo = FakeCargo::new();
    let dir = tempfile::tempdir().expect("temp dir");
    let path = cargo.path();

    let output = run(
        dir.path(),
        &[("PATH", path.as_str())],
        &[
            "--plugin",
            "shield",
            "--plugin-opt",
            r#"shield.level="strict""#,
            "test",
        ],
    );

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("[plugins.NAME]"),
        "{}",
        stderr(&output)
    );
    assert_eq!(cargo.seen(), None, "the tests were not started");
}

// ---------------------------------------------------------------------------
// The pure CDP engine: sbrs devtools
// ---------------------------------------------------------------------------

/// The arguments of `sbrs <flags> devtools` against a fake browser.
#[cfg(unix)]
fn devtools(
    chrome: &fake_chrome::FakeChrome,
    settings: Option<&str>,
    env: &[(&str, &str)],
    flags: &[&str],
) -> Output {
    let dir = tempfile::tempdir().expect("temp dir");
    if let Some(text) = settings {
        std::fs::write(dir.path().join("sbase_config.toml"), text).unwrap();
    }
    let browser = chrome.path().to_string_lossy().into_owned();
    let mut args = flags.to_vec();
    args.extend([
        "devtools",
        "--browser-path",
        browser.as_str(),
        "--port",
        "0",
        "--headless",
    ]);
    run(dir.path(), env, &args)
}

#[cfg(unix)]
#[test]
fn devtools_gives_the_plugins_to_the_browser_it_launches() {
    let with = fake_chrome::FakeChrome::new();
    let output = devtools(&with, None, &[], &["--plugin", "shield"]);
    let args = with.args().unwrap_or_else(|| {
        panic!("the browser was started: {}", stderr(&output));
    });
    assert!(args.iter().any(|arg| arg == SHIELD_SWITCH), "{args:?}");

    let without = fake_chrome::FakeChrome::new();
    let output = devtools(&without, None, &[], &[]);
    let args = without.args().unwrap_or_else(|| {
        panic!("the browser was started: {}", stderr(&output));
    });
    assert!(!args.iter().any(|arg| arg == SHIELD_SWITCH), "{args:?}");
}

#[cfg(unix)]
#[test]
fn devtools_reads_the_plugins_table_and_sb_plugins_and_the_option_flags() {
    let file = "[plugins]\nenable = [\"shield\"]\n";

    let from_file = fake_chrome::FakeChrome::new();
    devtools(&from_file, Some(file), &[], &[]);
    assert!(from_file
        .args()
        .expect("started")
        .iter()
        .any(|arg| arg == SHIELD_SWITCH));

    let off = fake_chrome::FakeChrome::new();
    devtools(&off, Some(file), &[("SB_PLUGINS", "")], &[]);
    assert!(!off
        .args()
        .expect("started")
        .iter()
        .any(|arg| arg == SHIELD_SWITCH));

    let strict = fake_chrome::FakeChrome::new();
    devtools(
        &strict,
        None,
        &[("SB_PLUGINS", "shield")],
        &["--plugin-opt", r#"shield.level="strict""#],
    );
    assert!(strict
        .args()
        .expect("started")
        .iter()
        .any(|arg| arg.starts_with("--host-resolver-rules=")));
}

#[cfg(unix)]
#[test]
fn devtools_reads_only_the_plugin_table_of_a_file_that_is_wrong_for_a_browser() {
    // `headless` is not a boolean; `devtools` takes its headless flag from the command line.
    let file = "headless = \"maybe\"\n[plugins]\nenable = [\"shield\"]\n";
    let chrome = fake_chrome::FakeChrome::new();

    let output = devtools(&chrome, Some(file), &[], &[]);

    let args = chrome.args().unwrap_or_else(|| {
        panic!("the browser was started: {}", stderr(&output));
    });
    assert!(args.iter().any(|arg| arg == SHIELD_SWITCH), "{args:?}");
}

#[cfg(unix)]
#[test]
fn devtools_refuses_an_unknown_plugin_before_it_launches_a_browser() {
    let chrome = fake_chrome::FakeChrome::new();

    let output = devtools(&chrome, None, &[], &["--plugin", "sheild"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).contains("`sheild`"), "{}", stderr(&output));
    assert_eq!(chrome.args(), None, "no browser was started");
}

#[cfg(unix)]
#[test]
fn devtools_does_not_read_the_settings_that_are_not_plugins() {
    let chrome = fake_chrome::FakeChrome::new();

    // The bad variable would stop a BaseCase command; devtools never read it.
    devtools(
        &chrome,
        None,
        &[("SB_HEADLESS", "maybe")],
        &["--plugin", "shield"],
    );

    assert!(chrome.args().is_some(), "the browser was started");
}
