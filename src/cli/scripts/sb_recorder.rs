//! `sbrs mkrec` and the two interactive recorders.
//!
//! [`make_recorder_file`] writes a starter test. [`start_recorder`] and
//! [`record_live_session`] read commands from the terminal; their loops are
//! [`run_recorder`] and [`run_live_session`], which take any reader and writer
//! so they can run without one.

use crate::api::recorder::ActionRecorder;
use crate::cli::commands::write_new_file;
use crate::BaseCase;
use crate::BrowserConfig;
use crate::SeleniumBaseError;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// The test `sbrs mkrec` writes. The file is kept apart so a test can compile
/// it against the real `BaseCase` API.
const STARTER_TEMPLATE: &str = include_str!("templates/starter_recording.tpl");

/// Writes a starter recording template to `file`.
///
/// Missing folders are created. A file that already exists is left alone and
/// reported as an [`io::ErrorKind::AlreadyExists`] error.
pub fn make_recorder_file(file: &str) -> std::io::Result<PathBuf> {
    let path = PathBuf::from(file);
    write_new_file(&path, STARTER_TEMPLATE)?;
    Ok(path)
}

/// Runs a simple stdin-based recorder without a live browser.
pub fn start_recorder() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(e) = run_recorder(stdin.lock(), stdout.lock()) {
        eprintln!("Recorder stopped: {e}");
    }
}

/// Records actions typed as `<action> <selector> [value]` until `quit`, `save
/// <file>` or the end of `input`.
///
/// `<action>` is the name of a `BaseCase` method that the script generator
/// knows (`click`, `type_text`, `assert_text`, `go_back`, ...); `type` is
/// accepted for `type_text`. A line that would add nothing to the generated
/// script (an unknown action, a missing argument) is reported and not
/// recorded, instead of being acknowledged and then missing from the file.
/// When `save` cannot write the file the recording is kept, so the command can
/// be tried again with another name.
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_recorder::run_recorder;
///
/// let dir = tempfile::tempdir().unwrap();
/// let file = dir.path().join("login.rs");
/// let script = format!("click #login\ntype #user admin\nsave {}\n", file.display());
/// let mut transcript = Vec::new();
/// run_recorder(script.as_bytes(), &mut transcript).unwrap();
///
/// let saved = std::fs::read_to_string(&file).unwrap();
/// assert!(saved.contains("sb.click(\"#login\")"));
/// assert!(saved.contains("sb.type_text(\"#user\", \"admin\")"));
/// ```
pub fn run_recorder(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    writeln!(output, "Starting SeleniumBase action recorder...")?;
    writeln!(
        output,
        "Type actions as: <action> <selector> [value], for example: click #login"
    )?;
    writeln!(
        output,
        "Type 'save <filename>' to export, or 'quit' to exit."
    )?;

    let mut recorder = ActionRecorder::default();
    let mut line = String::new();
    loop {
        write!(output, "> ")?;
        output.flush()?;
        line.clear();
        // Zero bytes is the end of the input (Ctrl-D, or a pipe that ran dry).
        if input.read_line(&mut line)? == 0 {
            writeln!(output)?;
            if !recorder.actions.is_empty() {
                writeln!(
                    output,
                    "Input ended before 'save': the recording was not saved."
                )?;
            }
            break;
        }
        let command = line.trim();
        if command.is_empty() {
            continue;
        }
        if command.eq_ignore_ascii_case("quit") {
            break;
        }
        if let Some(path) = command.strip_prefix("save ") {
            let path = path.trim();
            match write_new_file(Path::new(path), &recorder.to_rust_script()) {
                Ok(()) => {
                    writeln!(output, "Saved recorded test to {path}")?;
                    break;
                }
                Err(e) => writeln!(output, "Failed to save: {e}")?,
            }
            continue;
        }
        match record_action(&mut recorder, command) {
            Ok(message) | Err(message) => writeln!(output, "{message}")?,
        }
    }
    Ok(())
}

/// Records `command` (`<action> [selector] [value]`) if the script generator can
/// turn it into a call, and says what happened either way.
fn record_action(recorder: &mut ActionRecorder, command: &str) -> Result<String, String> {
    let mut parts = command.splitn(3, ' ');
    let word = parts.next().unwrap_or_default();
    let action = if word == "type" { "type_text" } else { word };
    let target = parts.next().filter(|target| !target.is_empty());
    let value = parts.next();

    recorder
        .try_record(action, target, value)
        .map_err(|reason| {
            format!("Not recorded: '{command}': {reason} (usage: <action> <selector> [value])")
        })?;
    Ok(match target {
        Some(target) => format!("Recorded: {word} {target}"),
        None => format!("Recorded: {word}"),
    })
}

/// Launches a live `BaseCase` session on `url` and records actions from an
/// interactive prompt.
///
/// Supported commands:
/// - `open <url>`
/// - `click <css>`
/// - `type <css> <text>`
/// - `assert_text <css> <text>`
/// - `save <file>`
/// - `quit`
pub async fn record_live_session(url: &str) -> Result<(), SeleniumBaseError> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open(url).await?;

    println!("Live recording session started at {}", url);
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_live_session(&mut sb, stdin.lock(), stdout.lock()).await?;

    sb.quit().await?;
    println!("Live recording session ended.");
    Ok(())
}

/// The command loop of [`record_live_session`]: runs each line of `input` on
/// `sb` until `quit` or the end of `input`.
///
/// A command that fails is reported to `output` and the session goes on. An
/// `Err` is returned only when `input` or `output` itself fails.
pub async fn run_live_session(
    sb: &mut BaseCase,
    mut input: impl BufRead,
    mut output: impl Write,
) -> Result<(), SeleniumBaseError> {
    writeln!(
        output,
        "Commands: open <url>, click <css>, type <css> <text>, assert_text <css> <text>, save <file>, quit"
    )?;
    let mut line = String::new();
    loop {
        write!(output, "> ")?;
        output.flush()?;
        line.clear();
        // Zero bytes is the end of the input; without this check a closed
        // stdin would repeat the prompt forever.
        if input.read_line(&mut line)? == 0 {
            writeln!(output)?;
            break;
        }
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("quit") {
            break;
        }
        if trimmed.is_empty() {
            continue;
        }

        if let Err(e) = handle_live_command(sb, trimmed, &mut output).await {
            writeln!(output, "Error: {e}")?;
        }
    }
    Ok(())
}

async fn handle_live_command(
    sb: &mut BaseCase,
    input: &str,
    output: &mut impl Write,
) -> Result<(), SeleniumBaseError> {
    if let Some(rest) = input.strip_prefix("open ") {
        let url = rest.trim();
        sb.open(url).await?;
        writeln!(output, "Opened {url}")?;
        return Ok(());
    }
    if let Some(rest) = input.strip_prefix("click ") {
        let css = rest.trim();
        sb.click(css).await?;
        writeln!(output, "Clicked {css}")?;
        return Ok(());
    }
    if let Some(rest) = input.strip_prefix("type ") {
        match rest.split_once(' ') {
            Some((css, text)) => {
                sb.type_text(css, text).await?;
                writeln!(output, "Typed into {css}")?;
            }
            None => writeln!(output, "Usage: type <css> <text>")?,
        }
        return Ok(());
    }
    if let Some(rest) = input.strip_prefix("assert_text ") {
        match rest.split_once(' ') {
            Some((css, text)) => {
                sb.assert_text(css, text).await?;
                writeln!(output, "Asserted text in {css}")?;
            }
            None => writeln!(output, "Usage: assert_text <css> <text>")?,
        }
        return Ok(());
    }
    if let Some(rest) = input.strip_prefix("save ") {
        let file = rest.trim();
        let script = sb.export_recording_as_rust()?;
        write_new_file(Path::new(file), &script)?;
        writeln!(output, "Saved recording to {file}")?;
        return Ok(());
    }

    writeln!(output, "Unknown command: {input}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the stdin recorder on `commands` and returns what it printed.
    fn transcript(commands: &str) -> String {
        let mut out = Vec::new();
        run_recorder(commands.as_bytes(), &mut out).expect("the recorder runs");
        String::from_utf8(out).expect("utf-8")
    }

    #[test]
    fn the_recorder_stops_when_its_input_ends() {
        // Before the fix this never returned: read_line gives Ok(0) forever.
        let out = transcript("click #a\n");
        assert!(out.contains("Recorded: click #a"), "{out}");
        assert!(out.contains("was not saved"), "{out}");
    }

    #[test]
    fn a_typed_action_ends_up_in_the_saved_script() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("t.rs");

        let out = transcript(&format!(
            "type #user admin\nclick #go\ngo_back\nsave {}\n",
            file.display()
        ));

        assert!(out.contains("Saved recorded test"), "{out}");
        let saved = std::fs::read_to_string(&file).unwrap();
        assert!(
            saved.contains("sb.type_text(\"#user\", \"admin\").await?;"),
            "{saved}"
        );
        assert!(saved.contains("sb.click(\"#go\")"), "{saved}");
        assert!(
            saved.contains("sb.go_back()"),
            "an action without a target: {saved}"
        );
    }

    #[test]
    fn a_line_that_adds_nothing_to_the_script_is_not_called_recorded() {
        let out = transcript("fly #a\nclick\nquit\n");

        assert_eq!(out.matches("Not recorded").count(), 2, "{out}");
        assert!(!out.contains("Recorded:"), "{out}");
    }

    #[test]
    fn a_mistyped_action_gets_a_suggestion_and_the_usage() {
        let out = transcript("clik #a\nquit\n");

        assert!(out.contains("did you mean \"click\""), "{out}");
        assert!(out.contains("usage: <action> <selector>"), "{out}");
        assert!(!out.contains("Recorded:"), "{out}");
    }

    #[test]
    fn a_failed_save_keeps_the_recording_for_another_try() {
        let dir = tempfile::tempdir().unwrap();
        let taken = dir.path().join("taken.rs");
        std::fs::write(&taken, "keep me").unwrap();
        let free = dir.path().join("free.rs");

        let out = transcript(&format!(
            "click #a\nsave {}\nsave {}\n",
            taken.display(),
            free.display()
        ));

        assert!(
            out.contains("Failed to save") && out.contains("already exists"),
            "{out}"
        );
        assert_eq!(std::fs::read_to_string(&taken).unwrap(), "keep me");
        assert!(std::fs::read_to_string(&free)
            .unwrap()
            .contains("sb.click(\"#a\")"));
    }

    #[tokio::test]
    async fn the_live_session_stops_at_the_end_of_its_input() {
        // Only commands that need no browser: a `BaseCase` without a session
        // would panic on a real one.
        let mut sb = BaseCase::without_session(BrowserConfig::default());
        let mut out = Vec::new();

        // Before the fix a closed stdin made this loop repeat the prompt for ever.
        run_live_session(&mut sb, "bogus\ntype #a\n".as_bytes(), &mut out)
            .await
            .expect("only an I/O failure ends the session with an error");

        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("Unknown command: bogus"), "{out}");
        assert!(out.contains("Usage: type <css> <text>"), "{out}");
    }

    #[tokio::test]
    async fn nothing_runs_after_quit() {
        let mut sb = BaseCase::without_session(BrowserConfig::default());
        let mut out = Vec::new();

        run_live_session(&mut sb, "quit\nbogus\n".as_bytes(), &mut out)
            .await
            .unwrap();

        assert!(!String::from_utf8(out).unwrap().contains("Unknown command"));
    }
}
