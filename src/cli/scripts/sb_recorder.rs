//! `sbrs mkrec` (also `codegen` and `record`): record what you do in a
//! browser, then save it as a test file.
//!
//! This is Python's `sbase mkrec`. [`run_mkrec`] starts a browser in Recorder
//! Mode, waits until the person says they are done (the breakpoint prompt, or
//! SHIFT+ESC in the browser with [`MkrecOptions::esc_end`]), writes the
//! recording under `recordings/` and copies the code file to the name asked
//! for. Unlike Python it runs in this process: no test is compiled first.
//!
//! The pure parts, [`validate_file_name`] and [`MkrecOptions::check`], need no
//! browser, so the command line can refuse a bad name before one starts.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::api::recorder::SavedRecording;
use crate::common::shutdown::{Shutdown, Signal};
use crate::{BaseCase, BrowserConfig, SeleniumBaseError};

/// What the breakpoint prompt of `mkrec` tells the person to do.
pub const BREAKPOINT_MESSAGE: &str = "When done recording actions, type \"c\", and press [Enter].";

/// What `mkrec --ee` prints before it waits for SHIFT+ESC.
pub const SHIFT_ESC_MESSAGE: &str = ">>> Use [SHIFT + ESC] in the browser to end recording!";

/// How long `mkrec --ee` waits for SHIFT+ESC (Python waits 600 seconds).
pub const SHIFT_ESC_LIMIT: Duration = Duration::from_secs(600);

/// The name of the Gherkin feature and of the test class in a `mkrec` recording.
const FEATURE: &str = "RecorderTest";

/// The name of the scenario and of the test function in a `mkrec` recording.
const SCENARIO: &str = "test_recording";

/// What `sbrs mkrec FILE` was asked to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MkrecOptions {
    /// The test file to create (`FILE.rs`), relative to the current folder.
    /// A subfolder is allowed (`tests/login.rs`); the recording itself goes
    /// to `recordings/<stem>_rec.rs`.
    pub file: String,
    /// Replace `file` when it exists (`--overwrite`). It is replaced only
    /// after a recording was made, so a failed recording never deletes it.
    pub overwrite: bool,
    /// End the recording with SHIFT+ESC in the browser instead of the
    /// breakpoint prompt (`--ee`).
    pub esc_end: bool,
    /// Also write the Gherkin feature (`--behave`, `--rec-behave`).
    pub behave: bool,
    /// How long `esc_end` waits for SHIFT+ESC.
    pub limit: Duration,
}

impl MkrecOptions {
    /// The options for creating `file`, with nothing else asked for.
    ///
    /// ```
    /// use seleniumbase_rs::cli::scripts::sb_recorder::MkrecOptions;
    ///
    /// let options = MkrecOptions::new("tests/login.rs");
    /// assert_eq!(options.stem(), "login");
    /// assert!(!options.overwrite && !options.esc_end && !options.behave);
    /// ```
    #[must_use]
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            overwrite: false,
            esc_end: false,
            behave: false,
            limit: SHIFT_ESC_LIMIT,
        }
    }

    /// The first part of the recording's file names: `tests/login.rs` is
    /// recorded as `recordings/login_rec.rs`.
    #[must_use]
    pub fn stem(&self) -> String {
        Path::new(&self.file)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("recorded")
            .to_owned()
    }

    /// Checks the file name ([`validate_file_name`]) and that `file` does not
    /// exist yet, unless [`overwrite`](Self::overwrite) is on. Relative to the
    /// current folder.
    ///
    /// # Errors
    ///
    /// Returns the message Python prints after `ERROR: `.
    pub fn check(&self) -> Result<(), String> {
        self.check_in(Path::new("."))
    }

    /// [`check`](Self::check) for a file relative to `dir`.
    ///
    /// # Errors
    ///
    /// Returns the message Python prints after `ERROR: `.
    pub fn check_in(&self, dir: &Path) -> Result<(), String> {
        validate_file_name(&self.file)?;
        if !self.overwrite && dir.join(&self.file).exists() {
            return Err(format!(
                "File \"{}\" already exists in this directory!",
                self.file
            ));
        }
        Ok(())
    }
}

/// Python's checks of the file name `mkrec` is given, with `.rs` for `.py`
/// and a subfolder of the current folder allowed.
///
/// # Errors
///
/// Returns the message Python prints after `ERROR: `:
/// `File name must end with ".rs"!`, `Invalid file name!`,
/// `File name cannot start with "-"!` or
/// `File must be created in the current directory!` (an absolute path, or one
/// that goes up with `..`).
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_recorder::validate_file_name;
///
/// assert!(validate_file_name("new_test.rs").is_ok());
/// assert!(validate_file_name("tests/login.rs").is_ok());
/// assert_eq!(
///     validate_file_name("new_test.py").unwrap_err(),
///     "File name must end with \".rs\"!"
/// );
/// assert_eq!(
///     validate_file_name("../x.rs").unwrap_err(),
///     "File must be created in the current directory!"
/// );
/// ```
pub fn validate_file_name(file: &str) -> Result<(), String> {
    if !file.ends_with(".rs") {
        return Err("File name must end with \".rs\"!".to_owned());
    }
    if file.contains('*') || file.chars().count() < 4 {
        return Err("Invalid file name!".to_owned());
    }
    if file.starts_with('-') {
        return Err("File name cannot start with \"-\"!".to_owned());
    }
    let path = Path::new(file);
    let leaves_the_folder = path.is_absolute()
        || file.starts_with('/')
        || file.contains('\\')
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir));
    if leaves_the_folder {
        return Err("File must be created in the current directory!".to_owned());
    }
    Ok(())
}

/// What [`run_mkrec`] did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MkrecOutcome {
    /// The files the recording was written to under `recordings/`; `None`
    /// when nothing was recorded.
    pub saved: Option<SavedRecording>,
    /// The test file the recording was copied to; `None` when nothing was
    /// recorded.
    pub copied_to: Option<PathBuf>,
    /// The recording was ended by Ctrl-C (or SIGTERM) and not by the person
    /// saying they were done. It was saved all the same.
    pub interrupted: bool,
}

/// How the wait for the end of the recording ended.
enum Ended {
    /// The prompt or SHIFT+ESC ended it, or it failed.
    Finished(Result<(), SeleniumBaseError>),
    /// A signal ended it.
    Signal(Signal),
}

/// Waits until the person is done: the breakpoint prompt, or SHIFT+ESC.
async fn wait_for_the_end(
    sb: &mut BaseCase,
    options: &MkrecOptions,
    out: &mut dyn Write,
) -> Result<(), SeleniumBaseError> {
    if options.esc_end {
        writeln!(out, "\n\n{SHIFT_ESC_MESSAGE}")?;
        out.flush()?;
        sb.wait_for_recorder_end(options.limit).await?;
        return Ok(());
    }
    match sb.recorder_breakpoint(BREAKPOINT_MESSAGE).await {
        // `q` at the prompt: stop, and keep what was recorded.
        Ok(()) | Err(SeleniumBaseError::Skipped(_)) => Ok(()),
        Err(other) => Err(other),
    }
}

/// Records in a browser and writes the test: Python's `sbase mkrec FILE`.
///
/// `config` is the browser to start; Recorder Mode, printing and the
/// ESC-ends-the-test key are set here. A start page in the configuration
/// (`--url`) is opened before recording starts. In order:
///
/// 1. [`MkrecOptions::check`] and `* RECORDING initialized: FILE`.
/// 2. The browser starts in Recorder Mode and the recording is named after
///    the file.
/// 3. With [`MkrecOptions::esc_end`] it waits for SHIFT+ESC in the browser
///    (and prints how); otherwise the `[RECORDER BREAKPOINT]` prompt waits for
///    `c` and Enter. Ctrl-C ends the wait and saves what was recorded.
/// 4. The recording is written (`>>> RECORDING SAVED as: recordings/x_rec.rs`,
///    then the file with line numbers) and the browser quits.
/// 5. The code file is copied over `file` (`*** RECORDING COPIED to: FILE`),
///    and with [`MkrecOptions::behave`] the feature is shown
///    (`*** BEHAVE RECORDING at: recordings/features/x_rec.feature`).
///
/// An existing `file` is replaced only in step 5, so a recording that fails
/// never deletes it.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for a bad file name, a browser
/// that is not Chromium-based, and the driver's error when the browser cannot
/// be started or reached. `Ok` with no files in the outcome means nothing was
/// recorded.
pub async fn run_mkrec(
    config: BrowserConfig,
    options: &MkrecOptions,
    out: &mut dyn Write,
) -> Result<MkrecOutcome, SeleniumBaseError> {
    options.check().map_err(SeleniumBaseError::InvalidConfig)?;
    writeln!(out, "\n* RECORDING initialized: {}\n", options.file)?;
    out.flush()?;

    // The recording prints the code file, as `sbase print FILE -n` does for
    // Python, and a plain ESC must keep pausing the recorder, not end it.
    let mut config = config.with_recorder_mode(true).with_rec_print(true);
    config = config.with_esc_end(false);
    if options.behave {
        config = config.with_rec_behave(true);
    }
    let mut shutdown = Shutdown::install()?;
    let mut sb = BaseCase::new(config).await?;
    sb.set_recording_target(&options.stem(), FEATURE, SCENARIO);
    if let Err(error) = sb.activate_recorder().await {
        let _ = sb.quit().await;
        return Err(error);
    }

    let ended = tokio::select! {
        result = wait_for_the_end(&mut sb, options, out) => Ended::Finished(result),
        signal = shutdown.recv() => Ended::Signal(signal),
    };
    let interrupted = match ended {
        Ended::Finished(Ok(())) => false,
        Ended::Finished(Err(error)) => {
            let _ = sb.quit().await;
            return Err(error);
        }
        Ended::Signal(signal) => {
            writeln!(out, "\n{signal} received; saving the recording.")?;
            true
        }
    };

    let saved = sb.finish_recording().await;
    let quit = sb.quit().await;
    let saved = saved?;
    quit?;

    let Some(saved) = saved else {
        return Ok(MkrecOutcome {
            saved: None,
            copied_to: None,
            interrupted,
        });
    };
    let copied_to = copy_recording(&saved.code, &options.file)?;
    writeln!(out, "\n*** RECORDING COPIED to: {}\n", options.file)?;
    if let Some(feature) = &saved.feature {
        print_numbered(out, feature)?;
        writeln!(out, "\n*** BEHAVE RECORDING at: {}\n", feature.display())?;
    }
    out.flush()?;
    Ok(MkrecOutcome {
        saved: Some(saved),
        copied_to: Some(copied_to),
        interrupted,
    })
}

/// Copies the recording `code` over `file`, making the folders of `file`.
fn copy_recording(code: &Path, file: &str) -> Result<PathBuf, SeleniumBaseError> {
    let target = PathBuf::from(file);
    if let Some(parent) = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(code, &target)?;
    Ok(target)
}

/// The file with a line number before each line, as `sbase print FILE -n` shows it.
fn print_numbered(out: &mut dyn Write, path: &Path) -> Result<(), SeleniumBaseError> {
    let text = std::fs::read_to_string(path)?;
    writeln!(out)?;
    for (number, line) in text.lines().enumerate() {
        writeln!(out, "{:>4}  {line}", number + 1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_name_is_refused_with_the_message_python_prints() {
        for (name, message) in [
            ("new_test.py", "File name must end with \".rs\"!"),
            ("new_test", "File name must end with \".rs\"!"),
            ("*.rs", "Invalid file name!"),
            ("a*b.rs", "Invalid file name!"),
            (".rs", "Invalid file name!"),
            ("-x.rs", "File name cannot start with \"-\"!"),
            (
                "/tmp/x.rs",
                "File must be created in the current directory!",
            ),
            ("../x.rs", "File must be created in the current directory!"),
            (
                "a/../../x.rs",
                "File must be created in the current directory!",
            ),
            ("a\\b.rs", "File must be created in the current directory!"),
        ] {
            assert_eq!(validate_file_name(name).unwrap_err(), message, "{name}");
        }
    }

    #[test]
    fn a_file_in_the_current_folder_or_below_it_is_fine() {
        for name in ["a.rs", "new_test.rs", "tests/login.rs", "tests/a/b.rs"] {
            assert_eq!(validate_file_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn the_stem_is_the_file_name_without_folders_and_extension() {
        assert_eq!(MkrecOptions::new("tests/login.rs").stem(), "login");
        assert_eq!(MkrecOptions::new("new_test.rs").stem(), "new_test");
    }

    #[test]
    fn an_existing_file_is_refused_unless_it_may_be_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("taken.rs"), "keep me").unwrap();
        let mut options = MkrecOptions::new("taken.rs");

        assert_eq!(
            options.check_in(dir.path()).unwrap_err(),
            "File \"taken.rs\" already exists in this directory!"
        );
        options.overwrite = true;
        assert_eq!(options.check_in(dir.path()), Ok(()));
        assert_eq!(MkrecOptions::new("free.rs").check_in(dir.path()), Ok(()));
    }

    #[test]
    fn the_copy_makes_the_folders_and_replaces_an_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let code = dir.path().join("x_rec.rs");
        std::fs::write(&code, "new").unwrap();
        let file = dir.path().join("tests").join("x.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "old").unwrap();

        let copied = copy_recording(&code, file.to_str().unwrap()).unwrap();

        assert_eq!(copied, file);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new");
    }

    #[test]
    fn a_feature_is_shown_with_line_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let feature = dir.path().join("x_rec.feature");
        std::fs::write(
            &feature,
            "Feature: RecorderTest\n  Scenario: test_recording\n",
        )
        .unwrap();
        let mut out = Vec::new();

        print_numbered(&mut out, &feature).unwrap();

        let shown = String::from_utf8(out).unwrap();
        assert!(shown.contains("   1  Feature: RecorderTest\n"), "{shown}");
        assert!(
            shown.contains("   2    Scenario: test_recording\n"),
            "{shown}"
        );
    }
}
