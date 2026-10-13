//! `sbrs translate`: write a JSON scenario's actions in another language.
//!
//! Python SeleniumBase translates the method calls of a Python test file between
//! languages. A Rust test is compiled code with English method names, so here
//! the file that can be translated is the portable one, a JSON
//! [`Scenario`](crate::api::scenario::Scenario): each step's `action` is
//! rewritten with the dictionary of [`crate::utils::translate`], and nothing else
//! in the file changes.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::SeleniumBaseError;
use crate::utils::translate::scenario::translate_scenario;
use crate::utils::translate::Language;

/// What `translate` does with the translated scenario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslateAction {
    /// Return it, and write nothing.
    Print,
    /// Replace the file with it.
    Overwrite,
    /// Write it to a new file next to the original, named with the language
    /// code (`login.json` becomes `login_es.json`).
    Copy,
}

/// A `translate` command line.
#[derive(Clone, Debug)]
pub struct TranslateCommand {
    /// The scenario file.
    pub file: PathBuf,
    /// The language to write its actions in.
    pub language: Language,
    /// What to do with the result.
    pub action: TranslateAction,
}

/// What `translate` did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslateReport {
    /// The translated scenario.
    pub text: String,
    /// The file it was written to, for [`Overwrite`](TranslateAction::Overwrite)
    /// and [`Copy`](TranslateAction::Copy).
    pub written: Option<PathBuf>,
    /// How many steps had an action the dictionary knows.
    pub translated: usize,
    /// The actions it does not know, which were left as they were.
    pub unknown: Vec<String>,
}

/// The file [`TranslateAction::Copy`] writes: the name of `file` with `_` and the
/// language code before its extension.
#[must_use]
pub fn copy_path(file: &Path, language: Language) -> PathBuf {
    let stem = file
        .file_stem()
        .map_or_else(|| "scenario".into(), |stem| stem.to_string_lossy());
    let name = match file.extension() {
        Some(extension) => format!("{stem}_{}.{}", language.code(), extension.to_string_lossy()),
        None => format!("{stem}_{}", language.code()),
    };
    file.with_file_name(name)
}

/// Translates the scenario in `command.file`.
///
/// # Errors
///
/// Returns an error if the file cannot be read, is not a scenario, or the result
/// cannot be written; a [`Copy`](TranslateAction::Copy) never replaces a file
/// that exists, and an [`Overwrite`](TranslateAction::Overwrite) leaves the
/// original whole if it fails.
pub fn run(command: &TranslateCommand) -> Result<TranslateReport, SeleniumBaseError> {
    let source = fs::read_to_string(&command.file).map_err(|error| {
        SeleniumBaseError::invalid_config(format!(
            "cannot read {}: {error}",
            command.file.display()
        ))
    })?;
    let translation = translate_scenario(&source, command.language).map_err(|error| {
        SeleniumBaseError::invalid_config(format!("{}: {error}", command.file.display()))
    })?;
    let written = match command.action {
        TranslateAction::Print => None,
        TranslateAction::Overwrite => {
            replace_file(&command.file, &translation.text)?;
            Some(command.file.clone())
        }
        TranslateAction::Copy => {
            let target = copy_path(&command.file, command.language);
            create_new_file(&target, &translation.text)?;
            Some(target)
        }
    };
    Ok(TranslateReport {
        text: translation.text,
        written,
        translated: translation.translated,
        unknown: translation.unknown,
    })
}

/// Writes `text` to a new file, which must not exist.
fn create_new_file(path: &Path, text: &str) -> Result<(), SeleniumBaseError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                SeleniumBaseError::invalid_config(format!(
                    "{} already exists; remove it or choose --overwrite",
                    path.display()
                ))
            } else {
                error.into()
            }
        })?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

/// Replaces `path` with `text` by writing a file beside it and renaming that
/// over it, so a failure leaves the original as it was. The file keeps its
/// permissions, and a symbolic link stays a link: the file it points to is the
/// one replaced.
fn replace_file(path: &Path, text: &str) -> Result<(), SeleniumBaseError> {
    let target = fs::canonicalize(path)?;
    let dir = target
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(dir)?;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    // A temporary file is private to its owner (mode 0600); the file it replaces may be
    // shared with a group, or read by a CI job that runs as someone else.
    fs::set_permissions(temporary.path(), fs::metadata(&target)?.permissions())?;
    temporary
        .persist(&target)
        .map_err(|error| SeleniumBaseError::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENARIO: &str = r#"{"name": "login", "steps": [
  {"action": "open", "url": "https://example.com"},
  {"action": "click", "css": "button"},
  {"action": "levitate"}
]}
"#;

    fn command(dir: &Path, action: TranslateAction) -> TranslateCommand {
        let file = dir.join("login.json");
        fs::write(&file, SCENARIO).unwrap();
        TranslateCommand {
            file,
            language: Language::Spanish,
            action,
        }
    }

    #[test]
    fn print_translates_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let command = command(dir.path(), TranslateAction::Print);
        let report = run(&command).unwrap();
        assert!(report.text.contains("\"abrir\"") && report.text.contains("\"clic\""));
        assert_eq!(report.translated, 2);
        assert_eq!(report.unknown, ["levitate"]);
        assert_eq!(report.written, None);
        assert_eq!(fs::read_to_string(&command.file).unwrap(), SCENARIO);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn overwrite_replaces_the_file_and_leaves_no_stray_file() {
        let dir = tempfile::tempdir().unwrap();
        let command = command(dir.path(), TranslateAction::Overwrite);
        let report = run(&command).unwrap();
        assert_eq!(report.written.as_deref(), Some(command.file.as_path()));
        assert_eq!(fs::read_to_string(&command.file).unwrap(), report.text);
        assert!(report.text.contains("\"abrir\""));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn overwrite_keeps_the_mode_of_the_file() {
        use std::os::unix::fs::PermissionsExt;
        // The temporary file is created with mode 0600; the original's mode
        // (shared with a group, or with CI running as another user) must win.
        for mode in [0o644, 0o664, 0o755] {
            let dir = tempfile::tempdir().unwrap();
            let command = command(dir.path(), TranslateAction::Overwrite);
            fs::set_permissions(&command.file, fs::Permissions::from_mode(mode)).unwrap();
            run(&command).unwrap();
            let kept = fs::metadata(&command.file).unwrap().permissions().mode() & 0o7777;
            assert_eq!(kept, mode, "{kept:o} instead of {mode:o}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn overwrite_through_a_symlink_translates_what_it_points_to() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("shared");
        fs::create_dir(&shared).unwrap();
        let real = shared.join("login.json");
        fs::write(&real, SCENARIO).unwrap();
        let link = dir.path().join("login.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let report = run(&TranslateCommand {
            file: link.clone(),
            language: Language::Spanish,
            action: TranslateAction::Overwrite,
        })
        .unwrap();

        assert_eq!(report.written.as_deref(), Some(link.as_path()));
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the link was replaced by a regular file"
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), report.text);
        assert!(report.text.contains("\"abrir\""));
        // No stray temporary file beside the target, or beside the link.
        assert_eq!(fs::read_dir(&shared).unwrap().count(), 1);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn copy_writes_a_new_file_named_with_the_language_and_keeps_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let command = command(dir.path(), TranslateAction::Copy);
        let report = run(&command).unwrap();
        let copy = dir.path().join("login_es.json");
        assert_eq!(report.written.as_deref(), Some(copy.as_path()));
        assert_eq!(fs::read_to_string(&copy).unwrap(), report.text);
        assert_eq!(fs::read_to_string(&command.file).unwrap(), SCENARIO);
    }

    #[test]
    fn copy_never_replaces_a_file_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        let command = command(dir.path(), TranslateAction::Copy);
        fs::write(dir.path().join("login_es.json"), "mine").unwrap();
        let error = run(&command).unwrap_err().to_string();
        assert!(
            error.contains("login_es.json") && error.contains("already exists"),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("login_es.json")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn the_copy_name_keeps_the_extension_and_copes_without_one() {
        assert_eq!(
            copy_path(Path::new("tests/a.b.json"), Language::Chinese),
            Path::new("tests/a.b_zh.json")
        );
        assert_eq!(
            copy_path(Path::new("plan"), Language::Dutch),
            Path::new("plan_nl")
        );
    }

    #[test]
    fn a_file_that_is_missing_or_not_a_scenario_is_an_error_that_names_it() {
        let dir = tempfile::tempdir().unwrap();
        let missing = TranslateCommand {
            file: dir.path().join("none.json"),
            language: Language::French,
            action: TranslateAction::Print,
        };
        assert!(run(&missing).unwrap_err().to_string().contains("none.json"));

        let other = dir.path().join("other.json");
        fs::write(&other, r#"{"name": "x"}"#).unwrap();
        let not_scenario = TranslateCommand {
            file: other,
            ..missing
        };
        let error = run(&not_scenario).unwrap_err().to_string();
        assert!(
            error.contains("other.json") && error.contains("steps"),
            "{error}"
        );
    }
}
