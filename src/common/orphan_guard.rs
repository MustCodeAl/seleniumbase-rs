//! Ends a driver or browser this crate started when the program that started
//! it ends, also when that program is killed and has no chance to clean up.
//!
//! On Unix a small `/bin/sh` watcher starts first, as the leader of a new
//! process group, and the driver or browser joins that group. The watcher reads
//! a pipe whose other end this program holds and never writes to. When this
//! program ends, by any means, the system closes that end: the watcher's `read`
//! returns, and it sends `SIGTERM` to its whole group (the driver, the browser
//! the driver started, and their helpers), then `SIGKILL` two seconds later.
//! The watcher ignores the first signal and leads the group until it has sent
//! both, so the group's number cannot pass to other processes in between.
//!
//! Dropping an [`OrphanGuard`] closes the pipe too and so ends the group: keep
//! the guard exactly as long as the processes it guards. Where `/bin/sh` cannot
//! be started, and on Windows, nothing is watched and processes start as before.
//!
//! Without this, a test run that is interrupted leaves its browsers running.
//! On macOS one of them can then hold the profile the person uses, and the
//! Chrome they open closes at once, handing over to the hidden one.

#[cfg(unix)]
use std::process::{Child, Stdio};

/// The watcher's script. `kill 0` signals the watcher's own process group.
#[cfg(unix)]
const WATCHER: &str = "trap '' TERM; read -r _; kill -s TERM 0; sleep 2; kill -s KILL 0";

/// Keeps the processes started in its group alive while it lives, and ends
/// them when it is dropped or when this program ends.
#[derive(Debug)]
pub(crate) struct OrphanGuard {
    /// The watcher, `None` when it could not be started.
    #[cfg(unix)]
    watcher: Option<Child>,
}

impl OrphanGuard {
    /// Starts a watcher for the processes enrolled with [`enroll`](Self::enroll).
    pub(crate) fn start() -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            let watcher = std::process::Command::new("/bin/sh")
                .args(["-c", WATCHER])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .inspect_err(|error| {
                    tracing::debug!(%error, "no watcher: a launched process may outlive this program");
                })
                .ok();
            Self { watcher }
        }
        #[cfg(not(unix))]
        Self {}
    }

    /// The process group the guarded processes join, while a watcher runs.
    #[cfg(unix)]
    fn group(&self) -> Option<i32> {
        self.watcher
            .as_ref()
            .and_then(|watcher| i32::try_from(watcher.id()).ok())
    }

    /// Puts the process `command` starts into the guarded group.
    pub(crate) fn enroll(&self, command: &mut std::process::Command) {
        #[cfg(unix)]
        if let Some(group) = self.group() {
            use std::os::unix::process::CommandExt as _;
            command.process_group(group);
        }
        #[cfg(not(unix))]
        let _ = command;
    }

    /// [`enroll`](Self::enroll) for a Tokio command.
    pub(crate) fn enroll_tokio(&self, command: &mut tokio::process::Command) {
        #[cfg(unix)]
        if let Some(group) = self.group() {
            command.process_group(group);
        }
        #[cfg(not(unix))]
        let _ = command;
    }
}

impl Drop for OrphanGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(mut watcher) = self.watcher.take() {
            // Closing the pipe tells the watcher to end the group.
            drop(watcher.stdin.take());
            // Collect its exit status once it is done, so it leaves no zombie.
            std::thread::spawn(move || {
                let _ = watcher.wait();
            });
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::*;

    /// Whether a process with this id still runs (a zombie counts as gone).
    fn running(pid: u32) -> bool {
        Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|out| {
                let stat = String::from_utf8_lossy(&out.stdout);
                !stat.trim().is_empty() && !stat.trim_start().starts_with('Z')
            })
    }

    fn gone_within(pid: u32, limit: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < limit {
            if !running(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn a_guarded_process_runs_until_the_guard_is_dropped() {
        let guard = OrphanGuard::start();
        let mut command = Command::new("sleep");
        command.arg("30").stdout(Stdio::null());
        guard.enroll(&mut command);
        let mut child = command.spawn().unwrap();
        let pid = child.id();

        std::thread::sleep(Duration::from_millis(300));
        assert!(running(pid), "the guarded process started and runs");

        drop(guard);
        let _ = child.wait();
        assert!(!running(pid), "dropping the guard ended the process");
    }

    #[test]
    fn a_process_that_ignores_the_polite_signal_is_killed() {
        let guard = OrphanGuard::start();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "trap '' TERM; sleep 30"]);
        guard.enroll(&mut command);
        let mut child = command.spawn().unwrap();
        let pid = child.id();
        std::thread::sleep(Duration::from_millis(300));

        drop(guard);
        let started = Instant::now();
        let _ = child.wait();
        assert!(
            started.elapsed() >= Duration::from_secs(1),
            "it was given time first"
        );
        assert!(gone_within(pid, Duration::from_secs(5)));
    }

    /// The case the guard exists for: the program that started the process is
    /// killed outright. A child copy of this test binary plays that program: it
    /// starts a guarded `sleep`, prints its id and is then killed with SIGKILL.
    #[test]
    fn a_killed_parent_takes_its_guarded_processes_with_it() {
        let exe = std::env::current_exe().unwrap();
        let mut parent = Command::new(exe)
            .args([
                "--exact",
                "common::orphan_guard::tests::helper_parent",
                "--ignored",
                "--nocapture",
            ])
            .env("SB_ORPHAN_GUARD_HELPER", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let mut out = parent.stdout.take().unwrap();
        let pid = read_pid(&mut out);
        assert!(running(pid), "the helper's guarded process runs");

        parent.kill().unwrap();
        let _ = parent.wait();
        assert!(
            gone_within(pid, Duration::from_secs(5)),
            "the guarded process outlived the killed program that started it"
        );
    }

    fn read_pid(out: &mut impl std::io::Read) -> u32 {
        let mut text = String::new();
        let mut byte = [0_u8; 1];
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if out.read(&mut byte).unwrap_or(0) == 0 {
                break;
            }
            if byte[0] == b'\n' {
                if let Some(pid) = text.trim().strip_prefix("guarded=") {
                    return pid.parse().unwrap();
                }
                text.clear();
            } else {
                text.push(char::from(byte[0]));
            }
        }
        panic!("the helper printed no process id: {text}");
    }

    /// Run only as the helper of the test above: starts a guarded `sleep`,
    /// prints its id and waits to be killed.
    #[test]
    #[ignore = "a helper process for a_killed_parent_takes_its_guarded_processes_with_it"]
    fn helper_parent() {
        if std::env::var_os("SB_ORPHAN_GUARD_HELPER").is_none() {
            return;
        }
        let guard = OrphanGuard::start();
        let mut command = Command::new("sleep");
        command.arg("60").stdout(Stdio::null());
        guard.enroll(&mut command);
        let child = command.spawn().unwrap();
        println!("guarded={}", child.id());
        std::thread::sleep(Duration::from_secs(60));
        drop(guard);
    }
}
