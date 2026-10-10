//! Making a chromedriver start when the operating system refuses it.
//!
//! A driver can be refused for three reasons, and each has a fix here:
//!
//! * It is not executable: a copy that lost its mode, or an archive that did
//!   not keep it. The owner's execute bit is set, and group and other get it
//!   only where they can already read.
//! * On macOS it carries the `com.apple.quarantine` flag that Gatekeeper puts
//!   on downloaded files. The flag is removed.
//! * On macOS its code signature no longer matches its contents, which is what
//!   patching a signed binary does. Apple Silicon kills such a program as it
//!   starts, so it is signed again ad hoc (`codesign --force --sign -`).
//!
//! Only the file you name is changed.
//!
//! # Examples
//!
//! ```no_run
//! use std::path::Path;
//! use seleniumbase_rs::browser::driver_access::make_runnable;
//!
//! # fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let repairs = make_runnable(Path::new("downloaded_drivers/chromedriver"), false)?;
//! if repairs.any() {
//!     println!("fixed: {repairs:?}");
//! }
//! # Ok(())
//! # }
//! ```

use std::fs;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};

use crate::error::SeleniumBaseError;

/// The flag macOS puts on files that came from the internet.
#[cfg(target_os = "macos")]
const QUARANTINE: &str = "com.apple.quarantine";

/// What [`make_runnable`] had to change.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Repairs {
    /// The execute permission was missing and was added.
    pub made_executable: bool,
    /// The macOS quarantine flag was present and was removed.
    pub quarantine_removed: bool,
    /// The binary was signed again (macOS only, and only when asked).
    pub signed: bool,
}

impl Repairs {
    /// Whether anything was changed.
    #[must_use]
    pub fn any(self) -> bool {
        self.made_executable || self.quarantine_removed || self.signed
    }
}

/// Makes `path` something the system will start.
///
/// With `resign` set, on macOS the binary is signed again ad hoc; do that after
/// changing its bytes. Everywhere else `resign` has no effect.
///
/// # Errors
///
/// Returns an error if the file cannot be inspected or its permissions changed,
/// or if `codesign` is asked for and fails.
pub fn make_runnable(path: &Path, resign: bool) -> Result<Repairs, SeleniumBaseError> {
    let repairs = Repairs {
        made_executable: make_executable(path)?,
        quarantine_removed: remove_quarantine(path),
        signed: resign && ad_hoc_sign(path)?,
    };
    if repairs.any() {
        tracing::info!(path = %path.display(), ?repairs, "repaired a driver the system would not start");
    }
    Ok(repairs)
}

/// Whether the system ended the program itself (`SIGKILL`) rather than the
/// program exiting.
///
/// This is how macOS refuses a binary with a bad signature or the quarantine
/// flag: it is killed before it can print anything.
#[must_use]
pub fn killed_by_the_system(status: ExitStatus) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal() == Some(9)
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        false
    }
}

/// Starts `binary`, configured by `configure`, and if the system says
/// permission was denied, repairs the file once and tries again.
///
/// A program killed just after it starts cannot be seen from here; use
/// [`killed_by_the_system`] on its exit status and call [`make_runnable`] with
/// `resign` set.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::BrowserLaunch`] if the program cannot be
/// started even after the repair.
pub fn spawn_repairing(
    binary: &Path,
    configure: impl Fn(&mut Command),
) -> Result<Child, SeleniumBaseError> {
    let spawn = || {
        let mut command = Command::new(binary);
        configure(&mut command);
        command.spawn()
    };
    let launch_error = |error: std::io::Error| {
        SeleniumBaseError::browser_launch(
            binary.display().to_string(),
            format!("failed to spawn: {error}"),
        )
    };
    match spawn() {
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            make_runnable(binary, false)?;
            spawn().map_err(launch_error)
        }
        other => other.map_err(launch_error),
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<bool, SeleniumBaseError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    let mode = permissions.mode();
    // The owner always; group and other only where they may already read.
    let wanted = mode | 0o100 | ((mode & 0o044) >> 2);
    if wanted == mode {
        return Ok(false);
    }
    permissions.set_mode(wanted);
    fs::set_permissions(path, permissions)?;
    Ok(true)
}

#[cfg(not(unix))]
fn make_executable(path: &Path) -> Result<bool, SeleniumBaseError> {
    fs::metadata(path)?;
    Ok(false)
}

#[cfg(target_os = "macos")]
fn remove_quarantine(path: &Path) -> bool {
    let succeeded = |args: [&str; 2]| {
        Command::new("xattr")
            .args(args)
            .arg(path)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    succeeded(["-p", QUARANTINE]) && succeeded(["-d", QUARANTINE])
}

#[cfg(not(target_os = "macos"))]
fn remove_quarantine(_path: &Path) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn ad_hoc_sign(path: &Path) -> Result<bool, SeleniumBaseError> {
    let output = Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(path)
        .output()
        .map_err(|error| {
            SeleniumBaseError::browser_launch(
                path.display().to_string(),
                format!("could not run codesign: {error}"),
            )
        })?;
    if output.status.success() {
        Ok(true)
    } else {
        Err(SeleniumBaseError::browser_launch(
            path.display().to_string(),
            format!(
                "codesign could not sign the driver again: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ))
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(clippy::unnecessary_wraps)]
fn ad_hoc_sign(_path: &Path) -> Result<bool, SeleniumBaseError> {
    Ok(false)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, mode: u32) -> std::path::PathBuf {
        let path = dir.join("driver");
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_driver_without_the_execute_bit_is_made_executable_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = script(dir.path(), 0o644);

        let first = make_runnable(&path, false).unwrap();
        assert!(first.made_executable);
        assert_eq!(mode(&path), 0o755);

        let second = make_runnable(&path, false).unwrap();
        assert!(!second.made_executable, "nothing was left to fix");
    }

    #[test]
    fn group_and_other_get_execute_only_where_they_can_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = script(dir.path(), 0o640);
        make_runnable(&path, false).unwrap();
        assert_eq!(mode(&path), 0o750);
    }

    #[test]
    fn a_script_that_lost_its_execute_bit_is_repaired_and_started() {
        let dir = tempfile::tempdir().unwrap();
        let path = script(dir.path(), 0o644);
        let refused = Command::new(&path).spawn().unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::PermissionDenied);

        let mut child = spawn_repairing(&path, |_| {}).expect("repaired, then started");
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn a_driver_that_is_not_there_is_a_launch_error() {
        let error = spawn_repairing(Path::new("/no/such/driver"), |_| {}).unwrap_err();
        assert!(error.to_string().contains("/no/such/driver"), "{error}");
    }

    #[test]
    fn only_a_kill_by_the_system_counts_as_a_refusal() {
        let killed = Command::new("sh")
            .args(["-c", "kill -9 $$"])
            .status()
            .unwrap();
        assert!(killed_by_the_system(killed));
        let exited = Command::new("sh").args(["-c", "exit 3"]).status().unwrap();
        assert!(!killed_by_the_system(exited));
    }

    /// Changes a byte of a signed program the loader reads at start-up, so its
    /// signature no longer matches and macOS kills it.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_program_macos_kills_for_its_signature_runs_again_after_signing_it() {
        // A copy of this test program: a single-architecture Mach-O that the
        // linker signed. (System binaries are universal, so their first bytes
        // are a table of architectures, not a Mach-O header.)
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("driver-copy");
        fs::copy(std::env::current_exe().unwrap(), &path).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        // The `reserved` field of the Mach-O header, which nothing reads, but
        // which is covered by the signature of the first page.
        bytes[28] ^= 0xff;
        fs::write(&path, bytes).unwrap();

        let tampered = Command::new(&path).arg("--help").output().unwrap();
        if !killed_by_the_system(tampered.status) {
            eprintln!("this system did not kill the altered binary; nothing to repair");
            return;
        }

        let repairs = make_runnable(&path, true).unwrap();
        assert!(repairs.signed);
        let again = Command::new(&path).arg("--help").output().unwrap();
        assert!(
            again.status.success(),
            "still refused after signing: {:?}",
            again.status
        );
    }
}
