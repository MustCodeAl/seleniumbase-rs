//! `sbrs convert`: turn an exported Selenium WebDriver test into a Rust test.
//!
//! Python's `sbase convert FILE.py` converts a Selenium IDE (or Katalon Recorder)
//! export, a WebDriver `unittest` file, into a SeleniumBase one named `FILE_SB.py`
//! and keeps the original. This is the same command for the Rust port: it runs the
//! Selenium reading of [`import_python`] and writes `FILE_SB.rs` next to the input.
//! The input is never touched, and an output that exists is not replaced unless the
//! caller says so. `sbrs import-python` does the same conversion for any Python
//! source and writes where it is told to.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::cli::scaffold::rust_fn_name;
use crate::error::SeleniumBaseError;
use crate::utilities::python_importer::{
    import_python, ImportDiagnostic, ImportOptions, PythonSource,
};

/// What [`convert`] did.
#[derive(Clone, Debug)]
pub struct ConvertReport {
    /// The Rust file that was written.
    pub output: PathBuf,
    /// What the importer could not convert, one entry per `TODO` it left in the file.
    pub diagnostics: Vec<ImportDiagnostic>,
    /// Whether the importer reported no error: warnings leave a `TODO`, an error means a
    /// statement it could not read at all.
    pub complete: bool,
}

/// The file [`convert`] writes for `input`: its name with `_SB` added to the stem and
/// the extension `rs`, in the same folder (`login.py` becomes `login_SB.rs`).
#[must_use]
pub fn output_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map_or_else(|| "test".into(), |stem| stem.to_string_lossy());
    input.with_file_name(format!("{stem}_SB.rs"))
}

/// Converts the Selenium WebDriver test in `input` and writes [`output_path`].
///
/// The generated test is named after the input file. With `force` an output that
/// exists is replaced; without it, it is left as it was.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when `input` does not end in `.py` or the
/// output exists and `force` is not set; [`SeleniumBaseError::Io`] when the input
/// cannot be read or the output cannot be written. Nothing is written on an error.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_convert::{convert, output_path};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let dir = tempfile::tempdir()?;
/// let input = dir.path().join("login.py");
/// std::fs::write(&input, "driver.get(\"https://example.test\")\n")?;
///
/// let report = convert(&input, false)?;
///
/// assert_eq!(report.output, output_path(&input));
/// assert!(std::fs::read_to_string(&report.output)?.contains("example.test"));
/// assert!(input.is_file());
/// # Ok(())
/// # }
/// ```
pub fn convert(input: &Path, force: bool) -> Result<ConvertReport, SeleniumBaseError> {
    if input.extension().is_none_or(|extension| extension != "py") {
        return Err(SeleniumBaseError::invalid_config(format!(
            "{} is not a Python file: the name must end in .py",
            input.display()
        )));
    }
    let output = output_path(input);
    if !force && output.exists() {
        return Err(SeleniumBaseError::invalid_config(format!(
            "{} already exists; remove it or pass --force",
            output.display()
        )));
    }
    let python = fs::read_to_string(input).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("failed to read {}: {error}", input.display()),
        )
    })?;
    let stem = input
        .file_stem()
        .map_or_else(|| "test".into(), |stem| stem.to_string_lossy());
    let result = import_python(
        &python,
        &ImportOptions {
            source: PythonSource::Selenium,
            test_name: rust_fn_name(&stem),
        },
    );
    write_output(&output, &result.rust, force).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!("failed to write {}: {error}", output.display()),
        )
    })?;
    Ok(ConvertReport {
        output,
        complete: result.is_complete(),
        diagnostics: result.diagnostics,
    })
}

/// Writes `text` to `path`; without `force` the file must not exist (checked again as the
/// file is created, so a file made since the first look is not replaced either).
fn write_output(path: &Path, text: &str, force: bool) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    options.open(path)?.write_all(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORT: &str = r#"
from selenium.webdriver.common.by import By

driver.get("https://example.test/login")
username = driver.find_element(By.ID, "username")
username.send_keys("alice")
driver.find_element(By.CSS_SELECTOR, "button").click()
driver.quit()
"#;

    fn export_in(dir: &Path, name: &str) -> PathBuf {
        let input = dir.join(name);
        fs::write(&input, EXPORT).unwrap();
        input
    }

    #[test]
    fn the_output_is_named_with_sb_and_rs_next_to_the_input() {
        assert_eq!(
            output_path(Path::new("tests/login.py")),
            Path::new("tests/login_SB.rs")
        );
        assert_eq!(output_path(Path::new("a.b.py")), Path::new("a.b_SB.rs"));
        assert_eq!(output_path(Path::new("x")), Path::new("x_SB.rs"));
    }

    #[test]
    fn converting_writes_a_rust_test_and_keeps_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let input = export_in(dir.path(), "login.py");

        let report = convert(&input, false).unwrap();

        assert_eq!(report.output, dir.path().join("login_SB.rs"));
        assert!(report.complete, "{:?}", report.diagnostics);
        let rust = fs::read_to_string(&report.output).unwrap();
        assert!(rust.contains("https://example.test/login"), "{rust}");
        assert!(rust.contains("sb.click(\"button\")"), "{rust}");
        assert!(rust.contains("fn login"), "{rust}");
        assert_eq!(fs::read_to_string(&input).unwrap(), EXPORT);
    }

    #[test]
    fn an_output_that_exists_is_not_replaced_without_force() {
        let dir = tempfile::tempdir().unwrap();
        let input = export_in(dir.path(), "login.py");
        let output = dir.path().join("login_SB.rs");
        fs::write(&output, "mine").unwrap();

        let error = convert(&input, false).unwrap_err().to_string();

        assert!(error.contains("login_SB.rs") && error.contains("--force"), "{error}");
        assert_eq!(fs::read_to_string(&output).unwrap(), "mine");

        let report = convert(&input, true).unwrap();
        assert_ne!(fs::read_to_string(&report.output).unwrap(), "mine");
    }

    #[test]
    fn a_file_that_is_not_python_or_is_missing_is_refused_and_named() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("notes.txt");
        fs::write(&text, EXPORT).unwrap();
        let error = convert(&text, false).unwrap_err().to_string();
        assert!(error.contains("notes.txt") && error.contains(".py"), "{error}");

        let missing = dir.path().join("none.py");
        let error = convert(&missing, false).unwrap_err().to_string();
        assert!(error.contains("none.py"), "{error}");
        assert!(!dir.path().join("none_SB.rs").exists());
    }

    #[test]
    fn what_cannot_be_converted_stays_a_todo_and_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("odd.py");
        fs::write(
            &input,
            "from selenium import webdriver\ndriver.get(\"https://a.test\")\ndriver.frobnicate(1)\n",
        )
        .unwrap();

        let report = convert(&input, false).unwrap();

        assert!(!report.diagnostics.is_empty());
        let rust = fs::read_to_string(&report.output).unwrap();
        assert!(rust.contains("TODO"), "{rust}");
    }
}
