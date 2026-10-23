//! Running `Xvfb`, the virtual X display a headed browser opens its window on
//! when the machine has no screen.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::browser::options::VirtualDisplay;
use crate::error::SeleniumBaseError;

/// Where X servers put their sockets.
const SOCKET_DIR: &str = "/tmp/.X11-unix";

/// The display numbers tried, lowest first. Real servers sit at 0 and a little
/// above; 99 is the number Xvfb tools conventionally start from.
const FIRST_DISPLAY: u32 = 99;
const LAST_DISPLAY: u32 = 199;

/// How long `Xvfb` may take to open its socket.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Whether a session with these settings should start a virtual display:
/// only on Linux, and only for a browser with a window.
///
/// This is Python SeleniumBase's rule for `--xvfb`: it needs Xvfb, which only
/// Linux has, and a headless browser draws nothing.
#[must_use]
pub fn wanted(display: Option<VirtualDisplay>, headless: bool, on_linux: bool) -> bool {
    display.is_some() && on_linux && !headless
}

/// The lowest display number in the range that `taken` does not report as used.
#[must_use]
pub fn first_free_display(taken: &dyn Fn(u32) -> bool) -> Option<u32> {
    (FIRST_DISPLAY..=LAST_DISPLAY).find(|&number| !taken(number))
}

/// A running `Xvfb`, stopped when this value is dropped.
#[derive(Debug)]
pub struct XvfbProcess {
    child: Child,
    display: String,
}

impl XvfbProcess {
    /// Starts `Xvfb` for `screen` on the first free display number and waits
    /// until it accepts connections.
    ///
    /// # Errors
    ///
    /// Returns an error if `Xvfb` is not installed, if every display number is
    /// taken, or if it exits or fails to open its socket within ten seconds.
    pub fn start(screen: VirtualDisplay) -> Result<Self, SeleniumBaseError> {
        let binary = which::which("Xvfb").map_err(|_| {
            SeleniumBaseError::browser_launch(
                "Xvfb",
                "Xvfb is not installed or not on the PATH; install it (on Debian or Ubuntu: \
                 apt install xvfb), or run headless",
            )
        })?;
        Self::start_with(&binary, Path::new(SOCKET_DIR), Path::new("/tmp"), screen)
    }

    /// [`start`](Self::start) with the program, the folder X servers keep their
    /// sockets in, and the folder of their lock files named.
    pub(crate) fn start_with(
        binary: &Path,
        socket_dir: &Path,
        lock_dir: &Path,
        screen: VirtualDisplay,
    ) -> Result<Self, SeleniumBaseError> {
        // Python raises a screen smaller than 1024x768 to that size.
        let requested = screen;
        let screen = screen.clamped();
        if screen != requested {
            tracing::info!(
                "the virtual display {requested} is smaller than {}; using {screen}",
                VirtualDisplay::MINIMUM
            );
        }

        let number = first_free_display(&|number| {
            socket_dir.join(format!("X{number}")).exists()
                || lock_dir.join(format!(".X{number}-lock")).exists()
        })
        .ok_or_else(|| {
            SeleniumBaseError::browser_launch(
                binary.display().to_string(),
                format!("every display from :{FIRST_DISPLAY} to :{LAST_DISPLAY} is in use"),
            )
        })?;
        let binary_text = binary.display().to_string();
        let mut child = Command::new(binary)
            .args(screen.xvfb_args(number))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                SeleniumBaseError::browser_launch(&binary_text, format!("failed to spawn: {error}"))
            })?;

        let socket: PathBuf = socket_dir.join(format!("X{number}"));
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            if socket.exists() {
                break;
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(SeleniumBaseError::browser_launch(
                    binary_text,
                    format!("Xvfb exited as it started ({status})"),
                ));
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SeleniumBaseError::browser_launch(
                    binary_text,
                    format!(
                        "Xvfb did not open {} within {STARTUP_TIMEOUT:?}",
                        socket.display()
                    ),
                ));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Ok(Self {
            child,
            display: format!(":{number}"),
        })
    }

    /// The display to give a browser, such as `:99` (the `DISPLAY` variable).
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Stops the server. It is also stopped when the value is dropped.
    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for XvfbProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_display_is_started_only_on_linux_for_a_browser_with_a_window() {
        let screen = Some(VirtualDisplay::default());
        assert!(wanted(screen, false, true));
        assert!(!wanted(screen, true, true), "headless draws nothing");
        assert!(!wanted(screen, false, false), "only Linux has Xvfb");
        assert!(!wanted(None, false, true), "not asked for");
    }

    #[test]
    fn the_first_number_not_in_use_is_chosen() {
        assert_eq!(first_free_display(&|_| false), Some(99));
        assert_eq!(first_free_display(&|number| number < 103), Some(103));
        assert_eq!(first_free_display(&|_| true), None);
    }

    /// A script standing in for Xvfb: it reads the display number from its first
    /// argument, creates that display's socket in `socket_dir`, and stays.
    #[cfg(unix)]
    fn fake_xvfb(dir: &Path, socket_dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("Xvfb");
        let script = format!(
            "#!/bin/sh\nn=\"${{1#:}}\"\n: > '{}'/X$n\nexec sleep 30\n",
            socket_dir.display()
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        // The first run of a freshly written script can be slow on macOS.
        let _ = Command::new(&path).arg(":0").output();
        path
    }

    #[cfg(unix)]
    #[test]
    fn xvfb_is_started_on_a_free_display_and_stopped_with_its_value() {
        let dir = tempfile::tempdir().unwrap();
        let sockets = dir.path().join("sockets");
        std::fs::create_dir_all(&sockets).unwrap();
        let xvfb = fake_xvfb(dir.path(), &sockets);
        // :99 and :100 are taken, by a socket and by a lock file.
        std::fs::write(sockets.join("X99"), b"").unwrap();
        std::fs::write(dir.path().join(".X100-lock"), b"").unwrap();

        let mut process =
            XvfbProcess::start_with(&xvfb, &sockets, dir.path(), VirtualDisplay::new(800, 600))
                .unwrap();
        assert_eq!(process.display(), ":101");
        assert!(sockets.join("X101").exists());
        assert!(process.child.try_wait().unwrap().is_none(), "it is running");

        process.stop();
        assert!(
            process.child.try_wait().unwrap().is_some(),
            "it was stopped"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_xvfb_that_exits_at_once_is_an_error_that_names_it() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Xvfb");
        std::fs::write(&path, "#!/bin/sh\nexit 3\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = Command::new(&path).output();

        let error =
            XvfbProcess::start_with(&path, dir.path(), dir.path(), VirtualDisplay::default())
                .unwrap_err();
        assert!(error.to_string().contains("exited"), "{error}");
    }

    #[test]
    fn a_missing_program_is_a_launch_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = XvfbProcess::start_with(
            Path::new("/no/such/Xvfb"),
            dir.path(),
            dir.path(),
            VirtualDisplay::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("/no/such/Xvfb"), "{error}");
    }
}
