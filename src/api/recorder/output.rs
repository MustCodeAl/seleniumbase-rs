//! The files a Recorder Mode recording is written to (Python's
//! `recordings/` folder), and the messages that say so.
//!
//! [`write_recording`] takes the processed actions
//! ([`ActionRecorder::from_raw_actions`](super::ActionRecorder::from_raw_actions))
//! and writes, under [`RecordingTarget::folder`]:
//!
//! | File | Content |
//! |---|---|
//! | `<stem>_rec.rs` | [`RecordingFormat::Test`]: an rstest test that uses the `sb` fixture. A later test of the same `stem` in the same process is added to the same file (`RECORDING ADDED to`). [`RecordingFormat::Script`]: a `#[tokio::main]` program (`--rec-sb-mgr`). [`RecordingFormat::Cdp`]: a pure CDP program (`--rec-sb-cdp`). |
//! | `<stem>_rec.json` | The processed actions, which `sbrs replay` and [`RecorderSession::load`](super::RecorderSession::load) read. Another test of the same `stem` writes `<stem>_rec_<scenario>.json`. |
//! | `features/<stem>_rec.feature` | With [`RecordingOptions::behave`]: a Gherkin feature, `Given Open "url"` then `And ...`. |
//!
//! The Gherkin feature has no glue file yet: binding it to the built-in step
//! library of the rstest-bdd area is a follow-up (see
//! [`SavedRecording::glue`]).

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::{check_file_name, rust_identifier, ActionRecorder, RecordedAction, RecorderSession};
use crate::common::color::{banner, palette};

use crate::SeleniumBaseError;

/// The folder the recordings go to, as in Python.
pub const RECORDINGS_FOLDER: &str = "recordings";

/// What the code file of a recording holds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RecordingFormat {
    /// An rstest test that asks for the `sb` fixture (the default).
    #[default]
    Test,
    /// A standalone `#[tokio::main]` program that calls `BaseCase`
    /// (`--rec-sb-mgr`, Python's `with SB() as sb:`).
    Script,
    /// A standalone program for the pure CDP engine (`--rec-sb-cdp`).
    Cdp,
}

/// Where a recording goes and what it is called.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordingTarget {
    /// The folder the files are written to; `recordings` by default.
    pub folder: PathBuf,
    /// The first part of the file names: `x` is written as `x_rec.rs`.
    pub file_stem: String,
    /// The name of the Gherkin feature.
    pub feature: String,
    /// The name of the test function and of the Gherkin scenario.
    pub scenario: String,
}

impl RecordingTarget {
    /// A target in the `recordings` folder.
    #[must_use]
    pub fn new(file_stem: &str, feature: &str, scenario: &str) -> Self {
        Self {
            folder: PathBuf::from(RECORDINGS_FOLDER),
            file_stem: file_stem.to_owned(),
            feature: feature.to_owned(),
            scenario: scenario.to_owned(),
        }
    }

    /// The same target in another folder.
    #[must_use]
    pub fn in_folder(mut self, folder: impl Into<PathBuf>) -> Self {
        self.folder = folder.into();
        self
    }

    /// The names a test gets when nothing named it: the file stem from
    /// `SB_REC_FILE`, else the first segment of the thread name (libtest names
    /// a test thread after the test, `module::test_fn`), else `recorded`; the
    /// scenario from the last segment, else `test_recording`; the feature is
    /// the stem in UpperCamelCase.
    #[must_use]
    pub fn from_environment() -> Self {
        let thread = std::thread::current();
        let name = thread
            .name()
            .filter(|name| !name.is_empty() && *name != "main");
        let stem = std::env::var("SB_REC_FILE")
            .ok()
            .filter(|stem| !stem.trim().is_empty())
            .or_else(|| name.and_then(|name| name.split("::").next().map(str::to_owned)))
            .unwrap_or_else(|| "recorded".to_owned());
        let scenario = name
            .and_then(|name| name.rsplit("::").next().map(str::to_owned))
            .unwrap_or_else(|| "test_recording".to_owned());
        let feature = upper_camel_case(&stem);
        Self::new(&stem, &feature, &scenario)
    }
}

impl RecordingTarget {
    /// The names a test gets from its id (`binary.module.function`, as the
    /// `sb` fixture knows it): the test binary is the file stem, so
    /// `tests/login.rs` is recorded in `recordings/login_rec.rs`, and the
    /// last part is the scenario and the name of the test function.
    #[must_use]
    pub fn from_test_id(test_id: &str) -> Self {
        let stem = test_id
            .split('.')
            .next()
            .filter(|stem| !stem.is_empty())
            .unwrap_or("recorded");
        let scenario = test_id
            .rsplit('.')
            .next()
            .filter(|scenario| !scenario.is_empty())
            .unwrap_or("test_recording");
        Self::new(stem, &upper_camel_case(stem), scenario)
    }
}

/// `my_test-file` as `MyTestFile`.
fn upper_camel_case(name: &str) -> String {
    let camel: String = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect();
    if camel.is_empty() {
        "Recording".to_owned()
    } else {
        camel
    }
}

/// How a recording is written.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RecordingOptions {
    /// What the code file holds.
    pub format: RecordingFormat,
    /// Also write a Gherkin feature (`--rec-behave`).
    pub behave: bool,
    /// Print the code file with line numbers (`--rec-print`).
    pub print: bool,
    /// Python's `--uc` output: the test starts in CDP mode.
    pub uc: bool,
    /// Colour the `RECORDING SAVED as:` banner the way Python does. Off by
    /// default: the caller that owns the stream decides, usually with
    /// [`use_color`](crate::common::color::use_color).
    pub color: bool,
}

/// The files [`write_recording`] wrote.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedRecording {
    /// The code file, `<stem>_rec.rs`.
    pub code: PathBuf,
    /// The processed actions as JSON.
    pub json: PathBuf,
    /// The Gherkin feature, with [`RecordingOptions::behave`].
    pub feature: Option<PathBuf>,
    /// The rstest-bdd glue that binds the feature to a test. Always `None`
    /// for now.
    ///
    /// FOLLOW-UP (after the built-in Gherkin step library lands): write
    /// `<stem>_rec_bdd.rs` with `#[scenario(path = ..., name = ...)]`.
    pub glue: Option<PathBuf>,
    /// The test was added to a code file that an earlier recording of this
    /// process had started.
    pub added: bool,
}

/// The code files this process has written, so a second test of the same
/// file is added to it and not written over it.
fn written() -> &'static Mutex<HashSet<PathBuf>> {
    static WRITTEN: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    WRITTEN.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Whether `path` was written by this process before; remembers it.
fn seen_before(path: &Path) -> bool {
    let key = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut written = written()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    !written.insert(key)
}

/// Writes a recording under `target` and says so on `out`.
///
/// `out` receives Python's messages: `Created recordings/` for a new folder,
/// `>>> RECORDING SAVED as: recordings/x_rec.rs` (or `ADDED to:`) under a line
/// of `*`, and with [`RecordingOptions::print`] the code file with line
/// numbers.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] when `target.file_stem` is
/// empty or has a path separator, and [`SeleniumBaseError::Io`] when a file
/// cannot be written.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::api::recorder::{
///     write_recording, ActionRecorder, RecordingOptions, RecordingTarget,
/// };
///
/// let mut recorder = ActionRecorder::default();
/// recorder.record("goto", Some("https://example.com"), None);
/// let folder = std::env::temp_dir().join("sbrs-doc-write-recording");
/// let target = RecordingTarget::new("login", "Login", "test_login").in_folder(&folder);
/// let mut out = Vec::new();
/// let saved = write_recording(&recorder.actions, &target, &RecordingOptions::default(), &mut out)?;
/// assert!(saved.code.ends_with("login_rec.rs"));
/// assert!(String::from_utf8_lossy(&out).contains("RECORDING SAVED as:"));
/// # let _ = std::fs::remove_dir_all(&folder);
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
pub fn write_recording(
    actions: &[RecordedAction],
    target: &RecordingTarget,
    options: &RecordingOptions,
    out: &mut dyn Write,
) -> Result<SavedRecording, SeleniumBaseError> {
    check_file_name(&target.file_stem)?;
    let recorder = ActionRecorder {
        actions: actions.to_vec(),
    };
    let folder = &target.folder;
    if !folder.exists() {
        std::fs::create_dir_all(folder)?;
        writeln!(out, "Created {}/", folder.display())?;
    }
    let stem = &target.file_stem;
    let code = folder.join(format!("{stem}_rec.rs"));
    let added = match options.format {
        RecordingFormat::Test => write_test(&code, &recorder, &target.scenario, options.uc)?,
        RecordingFormat::Script => {
            std::fs::write(&code, recorder.to_rust_script())?;
            false
        }
        RecordingFormat::Cdp => {
            std::fs::write(&code, recorder.to_cdp_script())?;
            false
        }
    };

    let json = if added {
        let scenario = rust_identifier(&target.scenario);
        folder.join(format!("{stem}_rec_{scenario}.json"))
    } else {
        folder.join(format!("{stem}_rec.json"))
    };
    RecorderSession::from_actions(actions.to_vec()).save(&json)?;

    let verb = if added { "ADDED to" } else { "SAVED as" };
    announce(out, verb, &code, options.color)?;

    let feature = if options.behave {
        let path = folder.join("features").join(format!("{stem}_rec.feature"));
        let appended = write_feature(&path, &recorder, target, options.uc, added)?;
        let verb = if appended { "ADDED to" } else { "SAVED as" };
        announce(out, verb, &path, options.color)?;
        Some(path)
    } else {
        None
    };
    if options.print {
        print_numbered(out, &code)?;
    }
    Ok(SavedRecording {
        code,
        json,
        feature,
        glue: None,
        added,
    })
}

/// The message, then a line of `*` as long as it (at most the terminal's width).
fn announce(out: &mut dyn Write, verb: &str, path: &Path, color: bool) -> std::io::Result<()> {
    let shown = path.display().to_string();
    let width = std::env::var("COLUMNS")
        .ok()
        .and_then(|columns| columns.trim().parse::<usize>().ok())
        .filter(|columns| *columns > 0)
        .unwrap_or(80);
    // As long as the message without its colours, so a colour never changes the length.
    let plain_length = ">>> RECORDING : ".len() + verb.chars().count() + shown.chars().count();
    let stars = "*".repeat(plain_length.min(width));
    let message = banner(
        ">>>",
        palette::BANNER_MARK,
        &format!(" RECORDING {verb}:"),
        &shown,
        color,
    );
    writeln!(out, "\n{message}\n{stars}")
}

/// The file with a line number before each line, as `sbrs print FILE` shows it.
fn print_numbered(out: &mut dyn Write, path: &Path) -> Result<(), SeleniumBaseError> {
    let text = std::fs::read_to_string(path)?;
    writeln!(out)?;
    for (number, line) in text.lines().enumerate() {
        writeln!(out, "{:>4}  {line}", number + 1)?;
    }
    Ok(())
}

/// The marker between the prelude of a generated test file and its tests.
const TEST_START: &str = "\n#[rstest]\n";
/// The end of every generated test function.
const TEST_END: &str = "\n    .await\n}\n";
/// The import a test needs when it types a password.
const SECRET_USE: &str = "use seleniumbase_rs::api::recorder::recorded_secret;\n";

/// Writes the test, or adds it to the file this process started. Returns
/// `true` when it was added as a new function.
fn write_test(
    path: &Path,
    recorder: &ActionRecorder,
    scenario: &str,
    uc: bool,
) -> Result<bool, SeleniumBaseError> {
    let full = recorder.to_rstest_test(scenario, uc);
    if !seen_before(path) || !path.exists() {
        std::fs::write(path, &full)?;
        return Ok(false);
    }
    let Some(split) = full.find(TEST_START) else {
        std::fs::write(path, &full)?;
        return Ok(false);
    };
    let (prelude, test) = full.split_at(split);
    let existing = std::fs::read_to_string(path)?;
    let (updated, added) = add_test(&existing, prelude, test, &rust_identifier(scenario));
    std::fs::write(path, updated)?;
    Ok(added)
}

/// `existing` with `test` (a generated function, from its newline before
/// `#[rstest]`) replacing the function of the same name, or added after the
/// others. Also the import that `prelude` has and `existing` lacks. The flag
/// is whether the test was added and not replaced.
fn add_test(existing: &str, prelude: &str, test: &str, name: &str) -> (String, bool) {
    let mut text = existing.to_owned();
    if prelude.contains(SECRET_USE) && !text.contains(SECRET_USE) {
        match text.find(TEST_START) {
            Some(at) => text.insert_str(at + 1, SECRET_USE),
            None => text.push_str(SECRET_USE),
        }
    }
    let header = format!("{TEST_START}#[tokio::test]\nasync fn {name}(");
    if let Some(start) = text.find(&header) {
        if let Some(length) = text[start..].find(TEST_END) {
            let end = start + length + TEST_END.len();
            text.replace_range(start..end, test);
            return (text, false);
        }
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(test);
    (text, true)
}

/// Writes the feature, or adds the scenario to the feature of this process.
/// Returns `true` when the scenario was added to an existing feature.
fn write_feature(
    path: &Path,
    recorder: &ActionRecorder,
    target: &RecordingTarget,
    uc: bool,
    add: bool,
) -> Result<bool, SeleniumBaseError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let feature = recorder.to_gherkin_scenario(&target.feature, &target.scenario, uc);
    if add && path.exists() {
        if let Some(at) = feature.find("  Scenario:") {
            let mut existing = std::fs::read_to_string(path)?;
            if !existing.ends_with('\n') {
                existing.push('\n');
            }
            existing.push('\n');
            existing.push_str(&feature[at..]);
            std::fs::write(path, existing)?;
            return Ok(true);
        }
    }
    std::fs::write(path, feature)?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("sbrs-rec-output-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    fn actions(url: &str) -> Vec<RecordedAction> {
        vec![
            RecordedAction::new("goto", Some(url), None),
            RecordedAction::new("click", Some("a"), None),
        ]
    }

    fn text(out: &[u8]) -> String {
        String::from_utf8_lossy(out).into_owned()
    }

    #[test]
    fn a_recording_writes_the_test_the_json_and_says_so() {
        let folder = temp("first");
        let target = RecordingTarget::new("login", "Login", "test_login").in_folder(&folder);
        let mut out = Vec::new();
        let saved = write_recording(
            &actions("https://a.test"),
            &target,
            &RecordingOptions::default(),
            &mut out,
        )
        .unwrap();

        assert_eq!(saved.code, folder.join("login_rec.rs"));
        assert_eq!(saved.json, folder.join("login_rec.json"));
        assert!(!saved.added && saved.feature.is_none() && saved.glue.is_none());
        let code = std::fs::read_to_string(&saved.code).unwrap();
        assert!(
            code.contains("async fn test_login(sb: Sb) -> Result<()> {"),
            "{code}"
        );
        assert!(code.contains("sb.goto(\"https://a.test\")"), "{code}");
        let back = RecorderSession::load(&saved.json).unwrap();
        assert_eq!(back.len(), 2);
        let said = text(&out);
        assert!(
            said.contains(&format!("Created {}/", folder.display())),
            "{said}"
        );
        assert!(said.contains("RECORDING SAVED as:"), "{said}");
        assert!(said.contains("login_rec.rs\n****"), "{said}");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_second_test_of_the_same_file_is_added_under_one_prelude() {
        let folder = temp("second");
        let mut out = Vec::new();
        let options = RecordingOptions::default();
        let one = RecordingTarget::new("shop", "Shop", "test_one").in_folder(&folder);
        let two = RecordingTarget::new("shop", "Shop", "test_two").in_folder(&folder);
        write_recording(&actions("https://a.test"), &one, &options, &mut out).unwrap();
        let saved = write_recording(&actions("https://b.test"), &two, &options, &mut out).unwrap();

        assert!(saved.added);
        assert_eq!(saved.json, folder.join("shop_rec_test_two.json"));
        let code = std::fs::read_to_string(&saved.code).unwrap();
        let prelude = "use seleniumbase_rs::testing::{rstest, sb, Sb};";
        assert_eq!(code.matches(prelude).count(), 1, "{code}");
        assert!(!code.contains("use rstest::"), "{code}");
        assert!(code.contains("async fn test_one("), "{code}");
        assert!(code.contains("async fn test_two("), "{code}");
        assert!(text(&out).contains("RECORDING ADDED to:"));
        // The same test again (a rerun) replaces its function.
        let again = write_recording(&actions("https://c.test"), &two, &options, &mut out).unwrap();
        assert!(!again.added);
        let code = std::fs::read_to_string(&again.code).unwrap();
        assert_eq!(code.matches("async fn test_two(").count(), 1, "{code}");
        assert!(
            code.contains("https://c.test") && !code.contains("https://b.test"),
            "{code}"
        );
        assert!(code.contains("https://a.test"), "{code}");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn behave_writes_a_feature_and_adds_scenarios_to_it() {
        let folder = temp("behave");
        let mut out = Vec::new();
        let options = RecordingOptions {
            behave: true,
            ..RecordingOptions::default()
        };
        let one = RecordingTarget::new("cart", "Cart", "test_a").in_folder(&folder);
        let two = RecordingTarget::new("cart", "Cart", "test_b").in_folder(&folder);
        let first = write_recording(&actions("https://a.test"), &one, &options, &mut out).unwrap();
        let second = write_recording(&actions("https://b.test"), &two, &options, &mut out).unwrap();

        let path = folder.join("features").join("cart_rec.feature");
        assert_eq!(first.feature.as_deref(), Some(path.as_path()));
        let feature = std::fs::read_to_string(&path).unwrap();
        assert_eq!(feature.matches("Feature: Cart").count(), 1, "{feature}");
        assert!(
            feature.contains(
                "  Scenario: test_a\n    Given Open \"https://a.test\"\n    And Click \"a\"\n"
            ),
            "{feature}"
        );
        assert!(
            feature.contains("  Scenario: test_b\n    Given Open \"https://b.test\"\n"),
            "{feature}"
        );
        assert!(second.glue.is_none(), "the glue waits for the step library");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_script_and_a_cdp_program_replace_the_file_and_print_adds_line_numbers() {
        let folder = temp("script");
        let mut out = Vec::new();
        let target = RecordingTarget::new("tool", "Tool", "test_tool").in_folder(&folder);
        for format in [RecordingFormat::Script, RecordingFormat::Cdp] {
            let options = RecordingOptions {
                format,
                print: true,
                ..RecordingOptions::default()
            };
            let saved =
                write_recording(&actions("https://a.test"), &target, &options, &mut out).unwrap();
            assert!(!saved.added);
            let code = std::fs::read_to_string(&saved.code).unwrap();
            assert!(code.contains("#[tokio::main]"), "{code}");
        }
        let said = text(&out);
        assert!(said.contains("\n   1  "), "numbered lines: {said}");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn the_saved_banner_is_coloured_like_pythons_when_asked() {
        let folder = temp("color");
        let target = RecordingTarget::new("tint", "Tint", "test_tint").in_folder(&folder);
        let mut out = Vec::new();
        let options = RecordingOptions {
            color: true,
            ..RecordingOptions::default()
        };
        let saved =
            write_recording(&actions("https://a.test"), &target, &options, &mut out).unwrap();

        let said = text(&out);
        let path = saved.code.display();
        // `>>>` light red on light yellow, the path red on light yellow: the
        // colorama pairs of `sbase mkrec`. The words between are plain.
        let line =
            format!("\x1b[91;103m>>>\x1b[0m RECORDING SAVED as: \x1b[31;103m{path}\x1b[0m\n");
        assert!(said.contains(&line), "{said:?}");
        // The line of stars under it is plain.
        let stars = said.split(&line).nth(1).unwrap().lines().next().unwrap();
        assert!(
            stars.starts_with("****") && stars.chars().all(|c| c == '*'),
            "{stars:?}"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn without_colour_the_banner_and_the_numbered_code_are_plain() {
        let folder = temp("nocolor");
        let target = RecordingTarget::new("plain", "Plain", "test_plain").in_folder(&folder);
        let mut out = Vec::new();
        let options = RecordingOptions {
            print: true,
            ..RecordingOptions::default()
        };
        write_recording(&actions("https://a.test"), &target, &options, &mut out).unwrap();

        let said = text(&out);
        assert!(!said.contains('\x1b'), "{said:?}");
        assert!(said.contains(">>> RECORDING SAVED as: "), "{said}");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]

    fn a_stem_that_leaves_the_folder_is_refused() {
        let target = RecordingTarget::new("../x", "X", "t").in_folder(temp("bad"));
        let error = write_recording(&[], &target, &RecordingOptions::default(), &mut Vec::new())
            .unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
    }

    #[test]
    fn names_default_from_the_thread_and_the_feature_is_camel_case() {
        let target = std::thread::Builder::new()
            .name("login::flows::valid_login".to_owned())
            .spawn(RecordingTarget::from_environment)
            .unwrap()
            .join()
            .unwrap();
        if std::env::var_os("SB_REC_FILE").is_none() {
            assert_eq!(target.file_stem, "login");
            assert_eq!(target.feature, "Login");
        }
        assert_eq!(target.scenario, "valid_login");
        assert_eq!(upper_camel_case("my_test-file"), "MyTestFile");
        assert_eq!(upper_camel_case("--"), "Recording");
    }
}
