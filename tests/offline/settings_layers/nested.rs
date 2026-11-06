//! A settings file accepts the nested form beside the flat keys (`docs/design/settings_v2.md`,
//! open question 2): the tables `[browser]` and `[session]`, with the tables and maps inside
//! them (`browser.proxy`, `browser.chromium`, `browser.switches`, `session.grid`). The nested
//! form is what `Settings` serializes to, so a file written from a serialized `Settings`
//! loads back equal.

use seleniumbase_rs::browser::options::LaunchSwitch;
use seleniumbase_rs::config::settings::Settings;

use super::{file_with, layered, refused};

/// Settings with a value in each group and in the tables and maps of the browser and the
/// session, built from the variables so that every kind of setting is there.
fn populated() -> Settings {
    let mut settings = Settings::default();
    let options: Vec<(&str, String)> = [
        ("SB_BROWSER", "firefox"),
        ("SB_HEADLESS", "true"),
        ("SB_USER_AGENT", "agent-of-the-test"),
        ("SB_LOCALE", "fr"),
        ("SB_START_PAGE", "http://start.test/"),
        ("SB_MOBILE", "true"),
        ("SB_DEVICE_METRICS", "360,640,2"),
        ("SB_WINDOW_SIZE", "800,600"),
        ("SB_WINDOW_POSITION", "10,20"),
        ("SB_PROXY", "proxy.test:8080"),
        ("SB_PROXY_PAC_URL", "http://pac.test/proxy.pac"),
        ("SB_PROXY_BYPASS_LIST", "a.test,b.test"),
        ("SB_USER_DATA_DIR", "/tmp/profile"),
        ("SB_EXTENSION_DIR", "/a,/b"),
        ("SB_AD_BLOCK", "true"),
        ("SB_DARK_MODE", "true"),
        ("SB_INCOGNITO", "true"),
        ("SB_CHROMIUM_ARG", "--mute-audio"),
        ("SB_DISABLE_FEATURES", "Foo,Bar"),
        ("SB_HOST_RESOLVER_RULES", "MAP a.test b.test"),
        ("SB_XVFB", "true"),
        ("SB_XVFB_METRICS", "1024,768"),
        ("SB_REMOTE_DEBUG", "true"),
        ("SB_MODE", "cdp"),
        ("SB_WEBDRIVER_URL", "http://driver.test:9515"),
        ("SB_SERVER", "hub.test"),
        ("SB_PORT", "4445"),
        ("SB_PROTOCOL", "https"),
        ("SB_REMOTE_WEBDRIVER_TIMEOUT", "99"),
        ("SB_PAGE_LOAD_STRATEGY", "eager"),
        ("SB_CAP_STRING", r#"{"a":1}"#),
        ("SB_FIREFOX_ARG", "-headless"),
        ("SB_FIREFOX_PREF", "a.b:1"),
        ("SB_LOG_CDP_EVENTS", "true"),
        ("SB_REUSE_SESSION", "true"),
        ("SB_REUSE_CLASS_SESSION", "true"),
        ("SB_CRUMBS", "true"),
        ("SB_TIMEOUT", "12"),
        ("SB_TIMEOUT_MULTIPLIER", "2"),
        ("SB_DEMO", "true"),
        ("SB_DEMO_SLEEP", "1.5"),
        ("SB_HIGHLIGHTS", "3"),
        ("SB_RERUNS", "2"),
        ("SB_TIME_LIMIT", "30"),
        ("SB_THREADS", "4"),
        ("SB_ENV", "qa"),
        ("SB_ACCOUNT", "account-of-the-test"),
        ("SB_VARIABLES", r#"{"k":1}"#),
        ("SB_PDB", "true"),
        ("SB_HTML", "report.html"),
        ("SB_DASH_TITLE", "A_title"),
        ("SB_DASHBOARD", "true"),
    ]
    .into_iter()
    .map(|(name, value)| (name, value.to_owned()))
    .collect();
    settings
        .apply_options(&options)
        .expect("every option is valid");
    settings
}

#[test]
fn a_file_written_from_serialized_settings_loads_back_equal_in_toml_and_json() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    for (what, settings) in [
        ("the defaults", Settings::default()),
        ("a populated one", populated()),
    ] {
        let toml_text = toml::to_string(&settings).expect("the settings are TOML");
        let json_text = serde_json::to_string_pretty(&settings).expect("the settings are JSON");
        for (name, text) in [("written.toml", toml_text), ("written.json", json_text)] {
            let path = file_with(dir.path(), name, &text);
            let loaded = Settings::from_file(&path)
                .unwrap_or_else(|error| panic!("{what} in {name}: {error}\n{text}"));
            assert_eq!(loaded, settings, "{what} in {name}:\n{text}");
        }
    }
}

#[test]
fn the_nested_tables_sit_beside_the_flat_keys_of_other_settings() {
    let settings = layered(
        "dark_mode = true\nheadless = true\n\
         [browser.proxy]\nserver = \"proxy.test:8080\"\n\
         [browser.chromium]\nargs = [\"--mute-audio\"]\n\
         [session.grid]\nport = 4445\n",
        &[],
    );
    assert!(settings.browser.headless);
    assert!(settings.browser.has(LaunchSwitch::DarkMode));
    assert_eq!(
        settings.browser.proxy.server.as_deref(),
        Some("proxy.test:8080")
    );
    assert_eq!(settings.browser.chromium.args, ["--mute-audio"]);
    assert_eq!(settings.session.grid.port, Some(4445));

    // The side of a window that a flat key gives and the one that a table gives are one size.
    let settings = layered(
        "window_width = 800\n[browser.window_size]\nheight = 600\n",
        &[],
    );
    let size = settings.browser.window_size.expect("a window size");
    assert_eq!((size.width, size.height), (800, 600));

    // A variable is laid over a table as over a flat key.
    let settings = layered(
        "[browser]\nuser_agent = \"agent-of-the-file\"\n[browser.proxy]\nserver = \"file.test:1\"\n",
        &[("SB_PROXY", "variable.test:2")],
    );
    assert_eq!(
        settings.browser.user_agent.as_deref(),
        Some("agent-of-the-file")
    );
    assert_eq!(
        settings.browser.proxy.server.as_deref(),
        Some("variable.test:2")
    );

    // The browser kind is a flat key `browser` when it is a text, and a table when it is one.
    let settings = layered("browser = \"firefox\"\n[session]\nreuse = true\n", &[]);
    assert_eq!(settings.browser.kind.to_string(), "firefox");
    assert!(settings.session.reuse);
}

#[test]
fn a_setting_written_flat_and_nested_is_refused_naming_both_spellings() {
    // (the file, and the two spellings the message names)
    let cases: [(&str, &str, &str); 6] = [
        (
            "dark_mode = true\n[browser.switches]\ndark-mode = true\n",
            "dark_mode",
            "[browser.switches] dark-mode",
        ),
        (
            "proxy = \"a.test:1\"\n[browser.proxy]\nserver = \"b.test:2\"\n",
            "proxy",
            "[browser.proxy] server",
        ),
        (
            "window_size = \"800,600\"\n[browser.window_size]\nwidth = 800\n",
            "window_size",
            "[browser.window_size] width",
        ),
        (
            "server = \"hub.test\"\n[session.grid]\nserver = \"hub.test\"\n",
            "server",
            "[session.grid] server",
        ),
        (
            "chromium_arg = \"--a\"\n[browser.chromium]\nargs = [\"--a\"]\n",
            "chromium_arg",
            "[browser.chromium] args",
        ),
        (
            "mode = \"cdp\"\n[session]\nmode = \"uc\"\n",
            "mode",
            "[session] mode",
        ),
    ];
    for (content, flat, nested) in cases {
        let error = refused("sbase_config.toml", content);
        assert!(error.contains("are the same setting"), "{content}: {error}");
        assert!(error.contains(&format!("\"{flat}\"")), "{content}: {error}");
        assert!(
            error.contains(&format!("\"{nested}\"")),
            "{content}: {error}"
        );
    }
    // The same in JSON, where the browser kind is a flat key `browser` and a table of that
    // name (spelled as the file spells it) is the nested form.
    let error = refused(
        "settings.json",
        r#"{"browser": "firefox", "Browser": {"kind": "chrome"}}"#,
    );
    assert!(error.contains("are the same setting"), "{error}");
    assert!(error.contains("\"browser\""), "{error}");
    assert!(error.contains("\"[Browser] kind\""), "{error}");
}

#[test]
fn a_misspelled_key_of_a_nested_table_is_refused() {
    let error = refused("sbase_config.toml", "[browser]\nhedless = true\n");
    assert!(error.contains("sbase_config.toml"), "{error}");
    assert!(error.contains("hedless"), "{error}");
    let error = refused("sbase_config.toml", "[browser.proxy]\nsrever = \"a:1\"\n");
    assert!(error.contains("srever"), "{error}");
    let error = refused("sbase_config.toml", "[session.grid]\nprot = 1\n");
    assert!(error.contains("prot"), "{error}");
}

#[test]
fn check_lists_every_problem_of_the_nested_tables_sorted_by_row() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    // A key that is no field, two texts for a switch and a window side that is no number, all in
    // the nested form, and a variable: `check` finds each one, and not only the first.
    let path = file_with(
        dir.path(),
        "sbase_config.toml",
        "[browser]\nmobile = \"sometimes\"\nhedless = true\nheadless = \"maybe\"\n\
         [browser.window_size]\nwidth = \"wide\"\n",
    );
    let check = Settings::check_with(Some(&path), |name| {
        (name == "SB_THREADS").then(|| "lots".to_owned())
    });
    // The rows run `headless`, `window_width`, `mobile`, `threads`; a key that no row holds
    // comes last.
    let order: Vec<&str> = check
        .problems
        .iter()
        .map(|problem| {
            let text = problem.error.to_string();
            [
                "browser.headless:",
                "browser.window_size.width:",
                "browser.mobile:",
                "SB_THREADS",
                "browser.hedless:",
            ]
            .into_iter()
            .find(|key| text.contains(key))
            .unwrap_or("?")
        })
        .collect();
    assert_eq!(
        order,
        [
            "browser.headless:",
            "browser.window_size.width:",
            "browser.mobile:",
            "SB_THREADS",
            "browser.hedless:",
        ],
        "{:?}",
        check.problems
    );
}

#[test]
fn a_refused_nested_value_names_the_file_and_the_path_and_not_the_value() {
    let error = refused("sbase_config.toml", "[browser]\nheadless = \"hunter2\"\n");
    assert!(error.contains("sbase_config.toml"), "{error}");
    assert!(error.contains("headless"), "{error}");
    assert!(!error.contains("hunter2"), "{error}");
}
