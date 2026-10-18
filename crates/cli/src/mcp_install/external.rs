//! Running the clients' own commands (`claude mcp ...`, `codex mcp ...`), and showing them the
//! way a user would type them.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A command to run, or to show.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cmd {
    /// The program: the path found on `PATH`, or its bare name when it was not found.
    pub program: PathBuf,
    /// Its arguments, passed as they are (no shell is involved).
    pub args: Vec<String>,
    /// The folder to run in; the current folder when `None`.
    pub cwd: Option<PathBuf>,
}

impl Cmd {
    /// The command as a user would type it, with the program called by its name.
    pub fn display(&self) -> String {
        let name = self
            .program
            .file_stem()
            .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
        let mut line = name;
        for arg in &self.args {
            line.push(' ');
            line.push_str(&quote(arg));
        }
        line
    }

    /// Runs the command and waits. Its output is captured, not shown.
    ///
    /// # Errors
    ///
    /// A message when the program cannot be started or exits with a failure status; it holds
    /// what the program wrote to standard error.
    pub fn run(&self) -> Result<(), String> {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        let output = command
            .output()
            .map_err(|e| format!("cannot run `{}`: {e}", self.display()))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = [stderr.trim(), stdout.trim()]
            .into_iter()
            .find(|text| !text.is_empty())
            .unwrap_or("it printed nothing");
        Err(format!(
            "`{}` exited with {}: {detail}",
            self.display(),
            output.status
        ))
    }
}

/// `arg` quoted for the user's shell when it needs it.
pub fn quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c));
    if plain {
        arg.to_owned()
    } else if cfg!(windows) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

/// Whether the `codex` at `program` has `codex mcp add` and `codex mcp remove`, judged by its
/// own `codex mcp --help`. An older Codex without them is edited through `config.toml`.
pub fn codex_has_mcp_commands(program: &Path) -> bool {
    let Ok(output) = Command::new(program)
        .args(["mcp", "--help"])
        .stdin(Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let help = String::from_utf8_lossy(&output.stdout);
    let lists = |command: &str| {
        help.lines()
            .any(|line| line.split_whitespace().next() == Some(command))
    };
    lists("add") && lists("remove")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn quote_wraps_what_a_shell_would_split() {
        assert_eq!(quote("plain-word_1.2/x"), "plain-word_1.2/x");
        assert_eq!(quote(""), "''");
        assert_eq!(quote("two words"), "'two words'");
        assert_eq!(quote(r#"{"a":"b"}"#), r#"'{"a":"b"}'"#);
        assert_eq!(quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn display_names_the_program_and_quotes_the_arguments() {
        let cmd = Cmd {
            program: PathBuf::from("/opt/bin/claude"),
            args: vec![
                "mcp".into(),
                "remove".into(),
                "--scope".into(),
                "user".into(),
            ],
            cwd: None,
        };
        assert_eq!(cmd.display(), "claude mcp remove --scope user");
    }
}
