//! Pins of the settings before they are regrouped (`docs/design/settings_v2.md`, step 1).
//!
//! Every `SB_*` variable of the option table, every settings-file key and alias, the order
//! of the sources, and the texts of the errors are checked here by the path the row of the
//! table names (`TestOption::field`), read back from `serde_json::to_value(&settings)`, so
//! the tests keep their meaning when the fields move into groups. The findings of the note
//! that are behaviour (1, 2, 4, 5, 7, 8, 9, 10, 11, 13) are tests of what the code does today;
//! the step that fixes one flips its test.

use std::path::{Path, PathBuf};
use std::time::Duration;

use seleniumbase_rs::browser::config::BrowserConfig;
use seleniumbase_rs::browser::options::{grid_url, GridProtocol, WindowSize};
use seleniumbase_rs::config::browser_options::settings_key;
use seleniumbase_rs::config::env_vars::variables;
use seleniumbase_rs::config::settings::{Settings, SettingsOverrides};
use seleniumbase_rs::config::test_options::{OptionKind, OptionSpec, TEST_OPTIONS};
use seleniumbase_rs::config::RuntimeConfig;
use seleniumbase_rs::core::test_run::ReportSettings;
use seleniumbase_rs::sb_cdp::LaunchOptions;
use serde_json::{json, Value};

/// The value at the dotted `path` of the serialized settings. `TestSettings` flattens its
/// `TestData`, so `test.data.env` is `test.env` in the JSON.
fn json_at<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let path = path.replacen("test.data.", "test.", 1);
    let mut at = value;
    for part in path.split('.') {
        at = at.get(part)?;
    }
    Some(at)
}

/// The first non-null value of an object key, anywhere in `value`.
fn find_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map
            .get(key)
            .filter(|found| !found.is_null())
            .or_else(|| map.values().find_map(|inner| find_key(inner, key))),
        Value::Array(items) => items.iter().find_map(|inner| find_key(inner, key)),
        _ => None,
    }
}

fn to_json(settings: &Settings) -> Value {
    serde_json::to_value(settings).expect("settings serialize")
}

/// A valid value of a row: the text of the variable, the value in a settings file, and the
/// settings that must be given with it.
struct Sample {
    text: String,
    file: Value,
    companions: Vec<(&'static str, String, Value)>,
}

fn sample(row: &OptionSpec, dir: &Path) -> Sample {
    let text = |text: &str| Sample {
        text: text.to_owned(),
        file: json!(text),
        companions: Vec::new(),
    };
    let number = |n: u32| Sample {
        text: n.to_string(),
        file: json!(n),
        companions: Vec::new(),
    };
    match row.env {
        "SB_BROWSER" => return text("firefox"),
        "SB_MODE" => return text("cdp"),
        "SB_LOCALE" | "SB_LOCALE_CODE" => return text("fr"),
        "SB_PROTOCOL" => return text("https"),
        "SB_CAP_FILE" => return text(&dir.join("caps.json").display().to_string()),
        "SB_CAP_STRING" => return text(r#"{"a":1}"#),
        "SB_EXTENSION_ZIP" => return text(&dir.join("ext.zip").display().to_string()),
        "SB_PROXY_BYPASS_LIST" => return text("a.test,b.test"),
        "SB_FIREFOX_ARG" => return text("-headless"),
        "SB_FIREFOX_PREF" => return text("a.b:1"),
        "SB_DRIVER_VERSION" => return text("latest"),
        "SB_CHROMIUM_ARG" => return text("--mute-audio"),
        "SB_DISABLE_FEATURES" => return text("Foo,Bar"),
        "SB_ENV" => return text("qa"),
        "SB_TEST_RUNNER" => return text("nextest"),
        "SB_VARIABLES" => {
            return Sample {
                text: r#"{"k":1}"#.to_owned(),
                file: json!({"k": 1}),
                companions: Vec::new(),
            }
        }
        // The size of the virtual display is used when the display is on, so it is given with
        // `SB_XVFB`, as the window sides are given in pairs.
        "SB_XVFB_METRICS" => {
            return Sample {
                companions: vec![("SB_XVFB", "true".to_owned(), json!(true))],
                ..text("800,600")
            }
        }

        "SB_WINDOW_WIDTH" => {
            return Sample {
                companions: vec![("SB_WINDOW_HEIGHT", "600".to_owned(), json!(600))],
                ..number(800)
            }
        }
        "SB_WINDOW_HEIGHT" => {
            return Sample {
                companions: vec![("SB_WINDOW_WIDTH", "800".to_owned(), json!(800))],
                ..number(600)
            }
        }
        _ => {}
    }
    match row.kind {
        OptionKind::Switch => Sample {
            text: "true".to_owned(),
            file: json!(true),
            companions: Vec::new(),
        },
        OptionKind::Value(hint) | OptionKind::Env(hint) | OptionKind::Custom(hint) => match hint {
            "Text" => text("sample-text"),
            "Url" => text("http://example.test/"),
            "Path" => text("/tmp/sbrs-pin"),
            "Seconds" | "Count" | "Pixels" => number(7),
            "Port" => number(4444),
            "Float" => number(2),
            "Size" => text("800,600"),
            "Position" => text("10,20"),
            "Metrics" => text("360,640,2"),
            "Strategy" => text("eager"),
            other => panic!("no sample for the hint {other} of {}", row.env),
        },
        other => panic!("{} is not a row that reads a variable: {other:?}", row.env),
    }
}

/// The files the samples refer to.
fn fixture_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(dir.path().join("caps.json"), r#"{"a": 1}"#).expect("write caps");
    std::fs::write(dir.path().join("ext.zip"), b"PK").expect("write zip");
    dir
}

/// A settings file with the text `content`.
fn file_with(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write the settings file");
    path
}

fn variable_rows() -> impl Iterator<Item = &'static OptionSpec> {
    TEST_OPTIONS
        .iter()
        .filter(|row| row.kind.reads_env() && !row.env.is_empty())
}

/// Where a row's key sits in a settings file: the table (none for a top-level key) and the key.
fn location(row: &OptionSpec) -> (Option<&'static str>, &'static str) {
    if let Some(key) = row.flat_key() {
        return (None, key);
    }
    match row.field.split_once('.') {
        None => (None, row.field),
        Some(("test", rest)) => (Some("test"), rest.strip_prefix("data.").unwrap_or(rest)),
        Some((table, rest)) => (Some(table), rest),
    }
}

/// `entries` as the text of a settings file, TOML or JSON.
fn render(toml: bool, table: Option<&str>, entries: &[(String, Value)]) -> String {
    let map: serde_json::Map<String, Value> = entries.iter().cloned().collect();
    let root = match table {
        Some(table) => json!({ table: map }),
        None => Value::Object(map),
    };
    if toml {
        toml::to_string(&root).expect("a TOML file")
    } else {
        serde_json::to_string(&root).expect("a JSON file")
    }
}

#[test]
fn every_variable_of_the_table_changes_the_field_its_row_names() {
    let dir = fixture_dir();
    let defaults = to_json(&Settings::default());
    let mut checked = 0;
    for row in variable_rows() {
        let sample = sample(row, dir.path());
        let mut pairs = vec![(row.env, sample.text.clone())];
        pairs.extend(
            sample
                .companions
                .iter()
                .map(|(name, text, _)| (*name, text.clone())),
        );
        let mut settings = Settings::default();
        settings
            .apply_options(&pairs)
            .unwrap_or_else(|error| panic!("{} = {:?}: {error}", row.env, sample.text));
        if row.env == "SB_SETTINGS_FILE" {
            assert_eq!(
                settings.settings_file.as_deref(),
                Some(Path::new(&sample.text))
            );
            continue;
        }
        let after = to_json(&settings);
        assert_ne!(
            json_at(&after, row.field),
            json_at(&defaults, row.field),
            "{} did not change {}",
            row.env,
            row.field
        );
        checked += 1;
    }
    assert!(checked >= 110, "only {checked} variables were checked");
}

#[test]
fn a_blank_variable_is_unset_and_the_last_pair_of_a_variable_wins() {
    let mut settings = Settings::default();
    settings
        .apply_options(&[
            ("SB_HEADLESS", "  ".to_owned()),
            ("SB_USER_AGENT", String::new()),
        ])
        .expect("blank values are unset");
    assert_eq!(to_json(&settings), to_json(&Settings::default()));

    let mut settings = Settings::default();
    settings
        .apply_options(&[
            ("SB_BROWSER", "firefox".to_owned()),
            ("SB_BROWSER", "edge".to_owned()),
        ])
        .expect("both are valid");
    assert_eq!(settings.browser.kind.to_string(), "edge");
}

#[test]
fn every_key_and_alias_of_a_row_reads_the_same_setting_in_toml_and_json() {
    let dir = fixture_dir();
    let defaults = to_json(&Settings::default());
    let mut files = 0;
    for row in TEST_OPTIONS.iter().filter(|row| {
        row.has_field()
            && matches!(
                row.kind,
                OptionKind::Switch | OptionKind::Value(_) | OptionKind::Env(_) | OptionKind::Custom(_)
            )
            // The file of `fingerprint.file` must exist, `settings_file` names the file.
            && !matches!(row.field, "settings_file" | "fingerprint.file")
    }) {
        let sample = sample(row, dir.path());
        let (table, key) = location(row);
        let mut spellings = vec![key.to_owned()];
        if table.is_none() {
            spellings.push(row.long.to_owned());
            spellings.extend(row.aliases.iter().map(|alias| (*alias).to_owned()));
            // The names of Python's `SB(...)` are not keys of a file (`a_key_may_use_...`).
            spellings.push(row.long.to_ascii_uppercase());
        }
        for spelling in spellings {
            if table.is_none() {
                assert_eq!(settings_key(&spelling), Some(key), "{spelling}");
            }
            let mut entries = vec![(spelling.clone(), sample.file.clone())];
            // The companions are written under the key of their own row.
            for (variable, _, value) in &sample.companions {
                let companion = TEST_OPTIONS
                    .iter()
                    .find(|other| other.env == *variable)
                    .expect("a companion row");
                entries.push((location(companion).1.to_owned(), value.clone()));
            }
            for (toml, name) in [(true, "settings.toml"), (false, "settings.json")] {
                let content = render(toml, table, &entries);
                let path = file_with(dir.path(), name, &content);
                let settings = Settings::from_file(&path)
                    .unwrap_or_else(|error| panic!("{spelling} in {content}: {error}"));
                assert_ne!(
                    json_at(&to_json(&settings), row.field),
                    json_at(&defaults, row.field),
                    "{spelling} did not change {} in {content}",
                    row.field
                );
                files += 1;
            }
        }
    }
    assert!(files >= 400, "only {files} files were read");
}

#[test]
fn a_key_may_use_dashes_underscores_and_any_case() {
    assert_eq!(settings_key("Dark-Mode"), Some("dark_mode"));
    assert_eq!(settings_key("block-images"), Some("block_images"));
    assert_eq!(settings_key("timeout"), Some("timeout_seconds"));
    assert_eq!(settings_key("blok_images"), None);
    // The names Python's `SB(...)` knows an option by are not keys of the file; only the
    // flags of a row are.
    assert_eq!(settings_key("ad_block_on"), None);
    assert_eq!(settings_key("is_mobile"), None);
}

/// A file layered under variables and pairs: the file, then the variables, then the pairs.
fn layered(row: &OptionSpec, file: &Value, variable: &str, pair: &str) -> (String, String, String) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let (table, key) = location(row);
    let path = file_with(
        dir.path(),
        "sbase_config.toml",
        &render(true, table, &[(key.to_owned(), file.clone())]),
    );
    let read = |settings: &Settings| {
        let at = to_json(settings);
        let value = json_at(&at, row.field).expect("the field exists").clone();
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_f64().map(|number| number.to_string()))
            .expect("a text or a number")
    };

    let from_file = Settings::check_with(Some(&path), |_| None);
    assert!(from_file.problems.is_empty(), "{:?}", from_file.problems);
    let file_value = read(&from_file.settings);

    let with_variable = Settings::check_with(Some(&path), |name| {
        (name == row.env).then(|| variable.to_owned())
    });
    assert!(with_variable.problems.is_empty());
    let variable_value = read(&with_variable.settings);

    let mut settings = with_variable.settings;
    SettingsOverrides {
        options: vec![(row.env, pair.to_owned())],
    }
    .try_apply(&mut settings)
    .expect("the pair is valid");
    (file_value, variable_value, read(&settings))
}

#[test]
fn the_file_gives_way_to_the_variable_and_the_variable_to_the_pair() {
    let row = |env: &str| {
        TEST_OPTIONS
            .iter()
            .find(|row| row.env == env && row.kind.reads_env())
            .expect("a row")
    };
    // One setting of each group: the browser, the session, the run, the test, the report.
    let texts = [
        (
            "SB_USER_AGENT",
            "agent-file",
            "agent-variable",
            "agent-pair",
        ),
        ("SB_MODE", "cdp", "uc", "webdriver"),
        (
            "SB_ACCOUNT",
            "account-file",
            "account-variable",
            "account-pair",
        ),
        (
            "SB_DASH_TITLE",
            "title-file",
            "title-variable",
            "title-pair",
        ),
    ];
    for (env, file, variable, pair) in texts {
        let seen = layered(row(env), &json!(file), variable, pair);
        assert_eq!(
            seen,
            (file.to_owned(), variable.to_owned(), pair.to_owned()),
            "{env}"
        );
    }
    let seen = layered(row("SB_DEMO_SLEEP"), &json!(1), "2", "3");
    assert_eq!(seen, ("1".to_owned(), "2".to_owned(), "3".to_owned()));
}

#[test]
fn sb_plugins_replaces_the_names_of_the_file_and_keeps_the_options_of_the_rest() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(
        dir.path(),
        "sbase_config.toml",
        "[plugins]\nenable = [\"a\", \"b\"]\n[plugins.a]\nlevel = \"x\"\n[plugins.b]\nlevel = \"strict\"\n",
    );
    let check = |value: Option<&str>| {
        Settings::check_with(Some(&path), |name| {
            (name == "SB_PLUGINS").then(|| value.unwrap_or_default().to_owned())
        })
        .settings
        .plugins
    };
    let plugins = check(Some("b"));
    assert_eq!(plugins.enabled(), ["b"]);
    assert_eq!(
        plugins.options_of("b").expect("b keeps its options")["level"],
        "strict"
    );
    assert!(plugins.options_of("a").is_none());
    // An empty value is an answer: no plugin at all.
    assert!(check(Some("")).enabled().is_empty());
}

#[test]
fn a_refused_variable_names_its_variable_and_its_flag() {
    let error = Settings::default()
        .apply_options(&[("SB_HEADLESS", "maybe".to_owned())])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("SB_HEADLESS (--headless): cannot parse bool: maybe"),
        "{error}"
    );
    let error = Settings::default()
        .apply_options(&[("SB_TIMEOUT", "soon".to_owned())])
        .unwrap_err()
        .to_string();
    assert!(error.contains("SB_TIMEOUT: "), "{error}");
}

#[test]
fn a_refused_file_value_a_typo_and_a_repeated_key_have_their_texts() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let toml = |content: &str| file_with(dir.path(), "sbase_config.toml", content);

    let error = Settings::from_file(toml("headless = \"maybe\"\n"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("failed to parse TOML settings"), "{error}");
    let json = file_with(dir.path(), "settings.json", r#"{"headless": "maybe"}"#);
    let error = Settings::from_file(json).unwrap_err().to_string();
    assert!(error.contains("failed to parse settings"), "{error}");

    let error = Settings::from_file(toml("blok_images = true\n"))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "unknown setting \"blok_images\" in sbase_config.toml; `sbrs options` lists the browser options"
        ),
        "{error}"
    );

    let error = Settings::from_file(toml("dark = true\ndark_mode = true\n"))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "\"dark\" and \"dark_mode\" are the same setting (dark_mode) in the settings file"
        ),
        "{error}"
    );
}

#[test]
fn check_lists_every_problem_in_the_order_of_the_table() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(dir.path(), "sbase_config.toml", "");
    let check = Settings::check_with(Some(&path), |name| match name {
        "SB_THREADS" => Some("lots".to_owned()),
        "SB_HEADLESS" => Some("maybe".to_owned()),
        "SB_WINDOW_WIDTH" => Some("wide".to_owned()),
        _ => None,
    });
    let sources: Vec<&str> = check
        .problems
        .iter()
        .map(|problem| problem.source.as_str())
        .collect();
    assert_eq!(sources, ["SB_HEADLESS", "SB_WINDOW_WIDTH", "SB_THREADS"]);
    let texts: Vec<String> = check.problems.iter().map(|p| p.error.to_string()).collect();
    assert!(
        texts[0].contains("SB_HEADLESS (--headless): cannot parse bool: maybe"),
        "{texts:?}"
    );
    assert!(texts[1].contains("SB_WINDOW_WIDTH: "), "{texts:?}");
    // `SB_THREADS` has no flag of its own on the root, so it is named alone.
    assert!(texts[2].contains("SB_THREADS: "), "{texts:?}");
}

// ---------------------------------------------------------------------------------------
// The findings of docs/design/settings_v2.md
// ---------------------------------------------------------------------------------------

/// Every variable the table reads, as of the first step of `docs/design/settings_v2.md`.
/// A variable dropped or renamed by the regroup fails here; one added is added here.
const VARIABLES: [&str; 114] = [
    "SB_ACCOUNT",
    "SB_AD_BLOCK",
    "SB_ALLURE_DIR",
    "SB_ARCHIVE_DOWNLOADS",
    "SB_ARCHIVE_LOGS",
    "SB_BLOCK_IMAGES",
    "SB_BROWSER",
    "SB_CAP_FILE",
    "SB_CAP_STRING",
    "SB_CHECK_JS",
    "SB_CHROMIUM_ARG",
    "SB_CHROME_BIN",
    "SB_CRUMBS",
    "SB_DARK_MODE",
    "SB_DASHBOARD",
    "SB_DASH_TITLE",
    "SB_DATA",
    "SB_DATABASE_ENV",
    "SB_DEMO",
    "SB_DEMO_SLEEP",
    "SB_DEVICE_METRICS",
    "SB_DEVTOOLS",
    "SB_DISABLE_BEFOREUNLOAD",
    "SB_DISABLE_COOKIES",
    "SB_DISABLE_CSP",
    "SB_DISABLE_FEATURES",
    "SB_DISABLE_GPU",
    "SB_DISABLE_JS",
    "SB_DISABLE_WS",
    "SB_DO_NOT_TRACK",
    "SB_DRIVER_VERSION",
    "SB_ENABLE_3D_APIS",
    "SB_ENABLE_SYNC",
    "SB_ENV",
    "SB_ESC_END",
    "SB_EXTENSION_DIR",
    "SB_EXTENSION_ZIP",
    "SB_EXTERNAL_PDF",
    "SB_FINAL_DEBUG",
    "SB_FINGERPRINT",
    "SB_FIREFOX_ARG",
    "SB_FIREFOX_PREF",
    "SB_GUEST_MODE",
    "SB_HEADLESS",
    "SB_HEADLESS1",
    "SB_HIGHLIGHTS",
    "SB_HOST_RESOLVER_RULES",
    "SB_HTML",
    "SB_INCOGNITO",
    "SB_INTERVAL",
    "SB_JUNIT_XML",
    "SB_LIST_FAIL_PAGE",
    "SB_LOCALE",
    "SB_LOCALE_CODE",
    "SB_LOG_CDP_EVENTS",
    "SB_MAXIMIZE",
    "SB_MESSAGE_DURATION",
    "SB_MOBILE",
    "SB_MODE",
    "SB_NO_SANDBOX",
    "SB_NO_SCREENSHOT",
    "SB_PAGE_LOAD_STRATEGY",
    "SB_PDB",
    "SB_PORT",
    "SB_PROTOCOL",
    "SB_PROXY",
    "SB_PROXY_BYPASS_LIST",
    "SB_PROXY_DRIVER",
    "SB_PROXY_PAC_URL",
    "SB_RECORDER",
    "SB_REC_BEHAVE",
    "SB_REC_PRINT",
    "SB_REC_SB_CDP",
    "SB_REC_SB_MGR",
    "SB_REC_SLEEP",
    "SB_REMOTE_DEBUG",
    "SB_REMOTE_WEBDRIVER_TIMEOUT",
    "SB_RERUNS",
    "SB_RERUNS_DELAY",
    "SB_REUSE_CLASS_SESSION",
    "SB_REUSE_SESSION",
    "SB_SAVE_SCREENSHOT",
    "SB_SERVER",
    "SB_SETTINGS_FILE",
    "SB_SKIP_JS_WAITS",
    "SB_SLOW",
    "SB_START_PAGE",
    "SB_SWIFTSHADER",
    "SB_TEST_RUNNER",
    "SB_THREADS",
    "SB_TIMEOUT",
    "SB_TIMEOUT_MULTIPLIER",
    "SB_TIME_LIMIT",
    "SB_TRACE",
    "SB_UC_CDP_EVENTS",
    "SB_USER_AGENT",
    "SB_USER_DATA_DIR",
    "SB_USE_AUTO_EXT",
    "SB_VAR1",
    "SB_VAR2",
    "SB_VAR3",
    "SB_VARIABLES",
    "SB_VERIFY_DELAY",
    "SB_VISUAL_BASELINE",
    "SB_WAIT_FOR_ANGULARJS",
    "SB_WEBDRIVER_URL",
    "SB_WINDOW_HEIGHT",
    "SB_WINDOW_POSITION",
    "SB_WINDOW_SIZE",
    "SB_WINDOW_WIDTH",
    "SB_WITH_DB_REPORTING",
    "SB_WITH_S3_LOGGING",
    "SB_XVFB",
    "SB_XVFB_METRICS",
];

#[test]
fn the_table_reads_exactly_the_variables_of_the_first_step() {
    let read: std::collections::BTreeSet<&str> = variable_rows().map(|row| row.env).collect();
    let pinned: std::collections::BTreeSet<&str> = VARIABLES.into_iter().collect();
    assert_eq!(pinned.len(), VARIABLES.len(), "a variable is listed twice");
    let unread: Vec<_> = pinned.difference(&read).collect();
    let unpinned: Vec<_> = read.difference(&pinned).collect();
    assert!(
        unread.is_empty() && unpinned.is_empty(),
        "no longer read: {unread:?}; read and not pinned: {unpinned:?}"
    );
}

/// The run group is read flat (`demo = true`, `timeout_seconds = 12`) or in its table, and the
/// `[test]` key it held (`rec_sleep`) still works.
#[test]
fn the_run_group_is_read_flat_or_in_its_table() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let read =
        |name: &str, content: &str| Settings::from_file(file_with(dir.path(), name, content));
    let flat = read(
        "flat.toml",
        "demo = true\ntimeout_seconds = 12\nvisual_baseline = true\ndemo_sleep = 0.5\n",
    )
    .expect("flat keys");
    let table = read(
        "table.toml",
        "[run]\ndemo_mode = true\ndefault_timeout = 12\nreset_visual_baseline = true\ndemo_sleep = 0.5\n",
    )
    .expect("a [run] table");
    assert_eq!(flat.run, table.run);
    assert_eq!(flat.run.default_timeout(), Some(Duration::from_secs(12)));
    let json = read(
        "flat.json",
        r#"{"demo": true, "timeout": 12, "visual-baseline": true, "demo_sleep": 0.5}"#,
    )
    .expect("JSON");
    assert_eq!(json.run, flat.run);
    let legacy = read("legacy.toml", "[test]\nrec_sleep = true\n").expect("the old [test] key");
    assert_eq!(
        legacy.run,
        Settings::from_file(file_with(
            dir.path(),
            "new.toml",
            "[run]\nrecord_sleep = true\n"
        ))
        .expect("the new key")
        .run
    );

    let twice = read("twice.toml", "demo = true\n[run]\ndemo_mode = true\n")
        .unwrap_err()
        .to_string();
    assert!(
        twice.contains("\"demo\" and \"[run] demo_mode\" are the same setting (run.demo_mode)"),
        "{twice}"
    );
    let unknown = read("unknown.toml", "[run]\nnosuch = 1\n")
        .unwrap_err()
        .to_string();
    assert!(
        unknown.contains("unknown setting \"nosuch\" in the [run] table"),
        "{unknown}"
    );
    // A negative number of seconds is refused where it is read.
    assert!(read("negative.toml", "demo_sleep = -1\n").is_err());
}

/// Finding 1: a `[report]` table was refused by the loader (the field said it is that table).
#[test]
fn finding_1_a_report_table_is_read_by_the_loader() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let toml = file_with(
        dir.path(),
        "sbase_config.toml",
        "[report]\nhtml = \"r.html\"\nlist_fail_pages = true\ndb = \"r.db\"\n[report.databases]\nqa = \"qa.db\"\n",
    );
    let settings = Settings::from_file(toml).expect("a [report] table is a table of the file");
    assert_eq!(settings.report.html.as_deref(), Some(Path::new("r.html")));
    // The old spelling of the key is still read.
    assert!(settings.report.list_fail_page);
    assert_eq!(settings.report.db.as_deref(), Some(Path::new("r.db")));
    assert_eq!(settings.report.databases["qa"], Path::new("qa.db"));
    let json = file_with(
        dir.path(),
        "settings.json",
        r#"{"report": {"dashboard": true}}"#,
    );
    assert!(
        Settings::from_file(json)
            .expect("JSON too")
            .report
            .dashboard
    );
}

/// `[test]` still reads the keys it held before the report moved to `[report]`; they land in
/// `report`, and one setting written in both tables is refused.
#[test]
fn the_test_table_still_reads_the_report_keys_it_held() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let toml = |name: &str, content: &str| file_with(dir.path(), name, content);
    let old = Settings::from_file(toml(
        "old.toml",
        "[test]\nreruns = 2\nhtml = \"t.html\"\ndashboard = true\nlist_fail_page = true\n\
         [test.databases]\nqa = \"qa.db\"\n",
    ))
    .expect("the old [test] table");
    assert_eq!(old.test.reruns, 2);
    assert_eq!(old.report.html.as_deref(), Some(Path::new("t.html")));
    assert!(old.report.dashboard && old.report.list_fail_page);
    assert_eq!(old.report.databases["qa"], Path::new("qa.db"));

    let twice = Settings::from_file(toml(
        "twice.toml",
        "[test]\nhtml = \"a.html\"\n[report]\nhtml = \"b.html\"\n",
    ))
    .unwrap_err()
    .to_string();
    assert!(
        twice.contains(
            "\"[test] html\" and \"[report] html\" are the same setting (report.html) in the settings file"
        ),
        "{twice}"
    );

    // A report key is no key of the other tables.
    let misplaced = Settings::from_file(toml("misplaced.toml", "[report]\npdb = true\n"))
        .unwrap_err()
        .to_string();
    assert!(
        misplaced.contains("unknown setting \"pdb\" in the [report] table"),
        "{misplaced}"
    );
}

/// Finding 2: a key of `[test]` that is no setting was dropped without a word.
#[test]
fn finding_2_a_typo_in_the_test_table_is_refused() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(dir.path(), "sbase_config.toml", "[test]\nrerun = 2\n");
    let error = Settings::from_file(path).unwrap_err().to_string();
    assert!(
        error.contains("unknown setting \"rerun\" in the [test] table of sbase_config.toml"),
        "{error}"
    );
    // The aliases of a row are accepted there as at the top level.
    let path = file_with(
        dir.path(),
        "alias.toml",
        "[test]\nftrace = true\nreruns = 1\n",
    );
    let settings = Settings::from_file(path).expect("an alias of the row");
    assert!(settings.test.final_debug);
}

/// Finding 3: the report options were held by `TestSettings` and `ReportSettings`, and
/// `SB_HTML` was read again by `from_settings_with`.
#[test]
fn finding_3_the_report_options_are_declared_and_read_once() {
    let mut settings = Settings::default();
    settings.report.list_fail_page = true;
    settings.report.html = Some(PathBuf::from("from-report.html"));
    settings
        .report
        .databases
        .insert("qa".to_owned(), PathBuf::from("qa.db"));
    settings.report.database_env = Some("qa".to_owned());
    let none = |_: &str| None;
    let report = ReportSettings::from_settings_with(&settings, none);
    assert!(report.list_fail_page);
    assert_eq!(report.html.as_deref(), Some(Path::new("from-report.html")));
    assert_eq!(
        report.db.as_deref(),
        Some(Path::new("qa.db")),
        "the database of the environment"
    );
    // The variables of the report are read by `Settings::load`; only the results database is
    // chosen here, and the variable wins over the file's.
    let report = ReportSettings::from_settings_with(&settings, |name| {
        ["SB_HTML", "SB_REPORT_DB"]
            .contains(&name)
            .then(|| format!("{name}.file"))
    });
    assert_eq!(report.html.as_deref(), Some(Path::new("from-report.html")));
    assert_eq!(report.db.as_deref(), Some(Path::new("SB_REPORT_DB.file")));
    // A run saved by an earlier version wrote `list_fail_pages`.
    let saved: ReportSettings = serde_json::from_str(r#"{"list_fail_pages": true}"#).expect("json");
    assert!(saved.list_fail_page);
}

/// Finding 4: `to_launch_options` goes through the browser configuration, so it passes what
/// `Layer::from_config` passes: the phone, the PAC file, the bypass list, the preferences.
#[test]
fn finding_4_to_launch_options_passes_what_the_browser_config_passes() {
    let mut settings = Settings::default();
    settings
        .apply_options(&[
            ("SB_MOBILE", "true".to_owned()),
            ("SB_DEVICE_METRICS", "412,732,3".to_owned()),
            (
                "SB_PROXY_PAC_URL",
                "http://pac.example/proxy.pac".to_owned(),
            ),
            (
                "SB_PROXY_BYPASS_LIST",
                "*.example.test;localhost".to_owned(),
            ),
            ("SB_BLOCK_IMAGES", "true".to_owned()),
        ])
        .expect("valid values");
    let direct = settings.to_launch_options().expect("launch options");
    let through_the_config = LaunchOptions::builder()
        .browser_config(&settings.to_browser_config())
        .build()
        .expect("launch options");
    // The phone: a mobile user agent, and the screen that was asked for.
    assert!(direct.user_agent().is_some());
    assert_eq!(direct.user_agent(), through_the_config.user_agent());
    let profile = std::path::Path::new("/tmp/profile");
    let direct_args = direct.browser_args(profile);
    assert_eq!(direct_args, through_the_config.browser_args(profile));
    // The proxy's PAC file and bypass list reach the command line.
    assert!(
        direct_args
            .iter()
            .any(|arg| arg.starts_with("--proxy-pac-url=")),
        "{direct_args:?}"
    );
    assert!(
        direct_args
            .iter()
            .any(|arg| arg.starts_with("--proxy-bypass-list=")),
        "{direct_args:?}"
    );
    // The profile preference of `block_images` is checked by a unit test in `config::launch_map`,
    // where the options keep it.
}

/// Finding 5: `headless` has three defaults, one for each entry point.
#[test]
fn finding_5_headless_has_one_default_for_each_entry_point() {
    assert!(!Settings::default().browser.headless);
    assert!(BrowserConfig::default().browser.headless);
    let options = LaunchOptions::builder().build().expect("launch options");
    assert_eq!(options.is_headless(), cfg!(target_os = "linux"));
}

/// Finding 7: `RuntimeConfig` read `SB_CHROME_BIN` without the blank rule of the table.
#[test]
fn finding_7_the_browser_config_reads_a_blank_binary_as_unset() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(dir.path(), "sbase_config.toml", "");
    let blank = |name: &str| (name == "SB_CHROME_BIN").then(String::new);
    let config = BrowserConfig::from_vars(blank).expect("valid");
    assert_eq!(config.browser.executable_path, None);
    assert_eq!(
        Settings::check_with(Some(&path), blank)
            .settings
            .browser
            .executable_path,
        None
    );
    // The same for the other two variables both read.
    let blank = |name: &str| {
        ["SB_WEBDRIVER_URL", "SB_REMOTE_WEBDRIVER_TIMEOUT"]
            .contains(&name)
            .then(|| " ".to_owned())
    };
    let config = BrowserConfig::from_vars(blank).expect("valid");
    assert_eq!(config.session.webdriver_url(), "http://localhost:4444");
    assert_eq!(config.session.remote_webdriver_timeout_seconds, None);
    // A value is read as `Settings` reads it, and `RuntimeConfig` no longer reads these three.
    let given = |name: &str| match name {
        "SB_CHROME_BIN" => Some("/opt/chrome".to_owned()),
        "SB_WEBDRIVER_URL" => Some("http://grid:4444".to_owned()),
        "SB_REMOTE_WEBDRIVER_TIMEOUT" => Some("30".to_owned()),
        _ => None,
    };
    let config = BrowserConfig::from_vars(given).expect("valid");
    let settings = Settings::check_with(Some(&path), given).settings;
    assert_eq!(
        config.browser.executable_path,
        settings.browser.executable_path
    );
    assert_eq!(
        config.session.webdriver_url(),
        settings.session.webdriver_url()
    );
    assert_eq!(
        config.session.remote_webdriver_timeout_seconds,
        settings.session.remote_webdriver_timeout_seconds
    );
    assert_eq!(config.session.remote_webdriver_timeout_seconds, Some(30));
    assert!(RuntimeConfig::from_vars(given).is_ok());
    // A timeout that is no number is refused naming its variable.
    let wrong = |name: &str| (name == "SB_REMOTE_WEBDRIVER_TIMEOUT").then(|| "soon".to_owned());
    let error = BrowserConfig::from_vars(wrong).expect_err("not a number");
    assert!(
        error.to_string().contains("SB_REMOTE_WEBDRIVER_TIMEOUT"),
        "{error}"
    );
}

/// Finding 8: `BrowserConfig::threads` was written from `Settings::threads` and read by nothing.
#[test]
fn finding_8_the_browser_config_carries_no_threads() {
    // `Settings::threads` is read by `sbrs test`; the `BrowserConfig` of a session had a copy
    // that nothing read.
    let mut settings = Settings::default();
    settings.test.threads = Some(3);
    let config = serde_json::to_value(settings.to_browser_config()).expect("json");
    assert!(find_key(&config, "threads").is_none(), "{config}");
    assert_eq!(settings.test.threads, Some(3));
}

/// Finding 9: `Settings` and `BrowserConfig::with_grid` built a Grid address with two functions.
#[test]
fn finding_9_one_function_builds_a_grid_address() {
    let address = |port: u16, protocol: &str| {
        let mut settings = Settings::default();
        settings
            .apply_options(&[
                ("SB_SERVER", "grid.example.test".to_owned()),
                ("SB_PORT", port.to_string()),
                ("SB_PROTOCOL", protocol.to_owned()),
            ])
            .expect("a Grid address");
        settings.to_browser_config().session.webdriver_url()
    };
    for (port, protocol, kind) in [
        (4445, "https", GridProtocol::Https),
        (80, "http", GridProtocol::Http),
        (443, "https", GridProtocol::Https),
    ] {
        assert_eq!(
            address(port, protocol),
            grid_url(kind, "grid.example.test", port).expect("a Grid address")
        );
    }
    // The protocol's own port is left out of the address.
    assert_eq!(address(80, "http"), "http://grid.example.test/wd/hub");
}

/// Settings whose window is `width` by `height` pixels.
fn settings_with_window(width: u32, height: u32) -> Settings {
    let mut settings = Settings::default();
    settings.browser.window_size = Some(WindowSize::new(width, height));
    settings
}

/// Finding 10: the higher source decides between the spellings of one setting, so `SB_WINDOW_WIDTH`
/// and `_HEIGHT` beat a `window_size` of the file; within one source the size wins.
#[test]
fn finding_10_the_higher_source_decides_between_the_spellings_of_the_window_size() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(
        dir.path(),
        "sbase_config.toml",
        "window_size = \"800,600\"\n",
    );
    let check = Settings::check_with(Some(&path), |name| match name {
        "SB_WINDOW_WIDTH" => Some("1024".to_owned()),
        "SB_WINDOW_HEIGHT" => Some("768".to_owned()),
        _ => None,
    });
    assert!(check.problems.is_empty(), "{:?}", check.problems);
    let size = |settings: &Settings| {
        find_key(
            &serde_json::to_value(settings.to_browser_config()).expect("json"),
            "window_size",
        )
        .cloned()
    };
    let file_size = size(&settings_with_window(800, 600));
    let variable_size = size(&settings_with_window(1024, 768));
    assert_ne!(file_size, variable_size);
    assert_eq!(size(&check.settings), variable_size);
}

/// Finding 10, the other spellings: within one source the size and the locale code win; across
/// sources the higher one decides.
#[test]
fn finding_10_the_size_and_the_locale_code_win_within_a_source_only() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(
        dir.path(),
        "sbase_config.toml",
        "window_width = 800\nwindow_height = 600\nlocale_code = \"ar_eg\"\n",
    );
    let window = |settings: &Settings| {
        let config = settings.to_browser_config();
        config
            .browser
            .window_size
            .map(|size| (size.width, size.height))
    };
    let lang = |settings: &Settings| settings.to_browser_config().browser.lang;
    // The variables beat the file, whichever spelling each uses.
    let check = Settings::check_with(Some(&path), |name| match name {
        "SB_WINDOW_SIZE" => Some("1024,768".to_owned()),
        "SB_LOCALE" => Some("de".to_owned()),
        _ => None,
    });
    assert!(check.problems.is_empty(), "{:?}", check.problems);
    assert_eq!(window(&check.settings), Some((1024, 768)));
    assert_eq!(lang(&check.settings).as_deref(), Some("de"));
    // Within the variables, the size and the code win over the other spelling.
    let check = Settings::check_with(Some(&path), |name| match name {
        "SB_WINDOW_SIZE" => Some("1024,768".to_owned()),
        "SB_WINDOW_WIDTH" => Some("1".to_owned()),
        "SB_WINDOW_HEIGHT" => Some("2".to_owned()),
        "SB_LOCALE" => Some("de".to_owned()),
        "SB_LOCALE_CODE" => Some("fr_FR".to_owned()),
        _ => None,
    });
    assert!(check.problems.is_empty(), "{:?}", check.problems);
    assert_eq!(window(&check.settings), Some((1024, 768)));
    assert_eq!(lang(&check.settings).as_deref(), Some("fr-FR"));
    // A file alone is read as before.
    let check = Settings::check_with(Some(&path), |_| None);
    assert_eq!(window(&check.settings), Some((800, 600)));
    assert_eq!(lang(&check.settings).as_deref(), Some("ar-EG"));
}

/// Finding 11: `settings_file` was accepted as a key of the file and did nothing.
#[test]
fn finding_11_a_settings_file_cannot_name_the_settings_file() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    for key in ["settings_file", "settings"] {
        let path = file_with(
            dir.path(),
            "sbase_config.toml",
            &format!("{key} = \"other.toml\"\n"),
        );
        let error = Settings::from_file(path).unwrap_err().to_string();
        assert!(
            error.contains(&format!("\"{key}\" names the settings file")),
            "{error}"
        );
    }
}

/// Finding 13: seven variables the code reads were missing from the listing (`sbrs options --env`).
#[test]
fn finding_13_the_seven_variables_are_in_the_listing() {
    let listed = variables();
    for name in [
        "SB_REC_KEEP_SECRETS",
        "SB_REC_FILE",
        "SB_S3_BUCKET",
        "SB_S3_REGION",
        "SB_S3_PREFIX",
        "SB_OCR_MODELS",
        "SB_GRID_DIR",
    ] {
        assert!(
            listed.iter().any(|variable| variable.name == name),
            "{name} is not listed"
        );
    }
}
