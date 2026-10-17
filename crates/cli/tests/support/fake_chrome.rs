//! A stand-in for a Chromium executable, for tests that run a real binary and
//! need to see which arguments it started the browser with.
//!
//! The script writes its arguments, one per line, to a file and exits with a
//! failure, so the program that launched it reports that the browser did not
//! start. No browser is involved: what the test learns is what the program
//! asked a browser to do, which is where a plugin's launch switches show.

// A test that includes this file uses only some of what it offers.
#![allow(dead_code)]
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The script and the file it records its arguments in. Both go when this
/// value is dropped.
pub struct FakeChrome {
    dir: tempfile::TempDir,
    script: PathBuf,
    record: PathBuf,
}

impl FakeChrome {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("chrome");
        let record = dir.path().join("arguments.txt");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nfor argument in \"$@\"; do printf '%s\\n' \"$argument\"; done > '{}'\nexit 1\n",
                record.display()
            ),
        )
        .expect("write the script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
        // macOS delays the first run of a script that was just written; run
        // it once here so the launch under test does not wait for that.
        let _ = Command::new(&script).status();
        let _ = std::fs::remove_file(&record);
        Self {
            dir,
            script,
            record,
        }
    }

    /// The executable to give a program as its browser.
    pub fn path(&self) -> &Path {
        &self.script
    }

    /// The folder that holds the script.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// The arguments of the last start, or `None` if the browser was never
    /// started.
    pub fn args(&self) -> Option<Vec<String>> {
        let text = std::fs::read_to_string(&self.record).ok()?;
        Some(text.lines().map(str::to_owned).collect())
    }
}
