//! The running processes, read with `sysinfo`, for the tests that check no
//! browser or driver is left behind. `sysinfo` lists processes on every system,
//! so these checks also run on Windows, which has no `pgrep`.
//!
//! Include it with `#[path = "support/processes.rs"] mod processes;`.

#![allow(dead_code)]

use std::collections::HashMap;
use std::ffi::OsString;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Every running process, with its command line and its parent.
fn snapshot() -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    system
}

/// A command line as `pgrep -f` matches it: the arguments joined by spaces.
/// A Windows program's `.exe` is left out, so `msedgedriver --port=` matches
/// `msedgedriver.exe --port=9515` too.
fn command_line(cmd: &[OsString]) -> String {
    let mut words: Vec<String> = cmd
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    if let Some(program) = words.first_mut() {
        if program.to_ascii_lowercase().ends_with(".exe") {
            program.truncate(program.len() - ".exe".len());
        }
    }
    words.join(" ")
}

/// The ids of the running processes whose command line holds `pattern`, like
/// `pgrep -f pattern`: this test process is left out.
pub fn matching(pattern: &str) -> Vec<u32> {
    let own = std::process::id();
    let mut pids: Vec<u32> = snapshot()
        .processes()
        .values()
        .filter(|process| process.pid().as_u32() != own)
        .filter(|process| command_line(process.cmd()).contains(pattern))
        .map(|process| process.pid().as_u32())
        .collect();
    pids.sort_unstable();
    pids
}

/// Every process below `pid`: its children, their children, and so on.
pub fn descendants(pid: u32) -> Vec<u32> {
    let system = snapshot();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for process in system.processes().values() {
        if let Some(parent) = process.parent() {
            children
                .entry(parent.as_u32())
                .or_default()
                .push(process.pid().as_u32());
        }
    }
    let mut found = Vec::new();
    let mut queue = vec![pid];
    while let Some(parent) = queue.pop() {
        for &child in children.get(&parent).into_iter().flatten() {
            // A process cannot be its own ancestor, but a reused id could make
            // the table look like a cycle; never visit one twice.
            if child != pid && !found.contains(&child) {
                found.push(child);
                queue.push(child);
            }
        }
    }
    found
}

/// Whether a process with the id `pid` is still in the process table, as
/// `kill -0 pid` tells.
pub fn alive(pid: u32) -> bool {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing(),
    );
    system.process(pid).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_windows_program_is_matched_without_its_exe() {
        let cmd = [
            OsString::from(r"C:\drivers\msedgedriver.EXE"),
            OsString::from("--port=9515"),
        ];
        assert_eq!(command_line(&cmd), r"C:\drivers\msedgedriver --port=9515");
        let unix = [
            OsString::from("/usr/bin/geckodriver"),
            OsString::from("--port"),
        ];
        assert_eq!(command_line(&unix), "/usr/bin/geckodriver --port");
    }

    #[test]
    fn this_process_is_alive_and_a_child_that_ended_is_not() {
        assert!(alive(std::process::id()));
        // The test binary lists its tests and exits, on every system.
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(!alive(pid), "{pid} ended and was waited for");
    }
}
