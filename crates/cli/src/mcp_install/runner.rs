//! Running programs for the commands that look at the machine or start something beyond editing a
//! config file: `brew list sbrs`, `git ls-remote`, and the upgrade command `upgrade --run` starts.
//!
//! Everything goes through the [`Runner`] an [`Env`](super::env::Env) holds, so a test hands the
//! code a runner that answers from a script and records what it was asked to run. No test starts a
//! real program through it.

use std::fmt;
use std::process::{Command, Stdio};

use super::external::Cmd;

/// Runs programs on behalf of `upgrade`.
pub trait Runner: fmt::Debug + Send + Sync {
    /// Runs `cmd` with this program's terminal, so the person sees what it prints, and waits.
    ///
    /// # Errors
    ///
    /// A message when the program cannot be started or exits with a failure status.
    fn run(&self, cmd: &Cmd) -> Result<(), String>;

    /// What `cmd` writes to standard output when it exits with status 0. Nothing is shown and
    /// nothing is read from the terminal. `None` when the program cannot be started or fails.
    fn output(&self, cmd: &Cmd) -> Option<String>;
}

/// The runner of the real program: starts the processes.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, cmd: &Cmd) -> Result<(), String> {
        let mut command = Command::new(&cmd.program);
        command.args(&cmd.args);
        if let Some(cwd) = &cmd.cwd {
            command.current_dir(cwd);
        }
        let status = command
            .status()
            .map_err(|e| format!("cannot run `{}`: {e}", cmd.display()))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("`{}` exited with {status}", cmd.display()))
        }
    }

    fn output(&self, cmd: &Cmd) -> Option<String> {
        let mut command = Command::new(&cmd.program);
        command
            .args(&cmd.args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            // `git` asks for a password on the terminal when a repository wants one; a lookup
            // must fail instead of waiting for it.
            .env("GIT_TERMINAL_PROMPT", "0");
        if let Some(cwd) = &cmd.cwd {
            command.current_dir(cwd);
        }
        let output = command.output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// The runner of an environment built for a test: it starts nothing and finds nothing.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub struct NoRunner;

#[cfg(test)]
impl Runner for NoRunner {
    fn run(&self, cmd: &Cmd) -> Result<(), String> {
        Err(format!("`{}` was not run: this is a test", cmd.display()))
    }

    fn output(&self, _cmd: &Cmd) -> Option<String> {
        None
    }
}
