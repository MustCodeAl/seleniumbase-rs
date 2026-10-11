//! Native dialogs for scripts that need a person to answer.
//!
//! Messages, yes/no questions and file choosers use the platform's own dialogs
//! through `rfd`. `rfd` has no text-input dialog, so a text prompt asks through
//! whatever the platform provides:
//!
//! * macOS: `osascript` (`display dialog`), always installed.
//! * Linux: `zenity`, then `kdialog`, when a graphical session is running.
//! * Anywhere else, or when none of those can be used: the terminal, if
//!   standard input is one.
//!
//! The calls block until the person answers.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::api::dialog::{show_confirm, show_prompt};
//!
//! if show_confirm("Continue?", "Do you want to continue?") {
//!     let name = show_prompt("Your name", "What is your name?", Some("guest"))
//!         .unwrap_or_else(|| "nobody".to_owned());
//!     println!("Hello, {name}!");
//! }
//! ```

use std::io::{BufRead, IsTerminal, Write};
use std::process::{Command, Stdio};

use crate::error::SeleniumBaseError;

/// Result of a user-facing dialog.
#[derive(Clone, Debug, Default)]
pub struct DialogResult {
    /// Whether the person confirmed the dialog rather than cancelling it.
    pub confirmed: bool,
    /// What the person typed, when the dialog asked for text and was confirmed.
    pub text: Option<String>,
}

/// Displays a simple message dialog.
pub fn show_message(title: &str, message: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .show();
}

/// Displays a yes/no confirmation dialog.
///
/// Returns `true` only if the person chose "Yes".
pub fn show_confirm(title: &str, message: &str) -> bool {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}

/// Asks for a line of text.
///
/// `default` is what the field starts with. Returns what the person typed, or
/// `None` if they cancelled **or** nothing could ask them (a reason is logged
/// in that case). Use [`try_prompt`] to tell the two apart.
pub fn show_prompt(title: &str, message: &str, default: Option<&str>) -> Option<String> {
    match try_prompt(title, message, default) {
        Ok(answer) => answer,
        Err(error) => {
            tracing::warn!(%error, "the text prompt could not be shown");
            None
        }
    }
}

/// Asks for a line of text, and says so when it cannot ask.
///
/// Returns `Ok(Some(text))` when the person confirmed, `Ok(None)` when they
/// cancelled. The text is what they typed, including an empty answer.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Gui`] when there is no way to ask: no dialog
/// program is available and standard input is not a terminal.
pub fn try_prompt(
    title: &str,
    message: &str,
    default: Option<&str>,
) -> Result<Option<String>, SeleniumBaseError> {
    ask(
        &prompt_commands(title, message, default.unwrap_or_default()),
        run_helper,
        |problems| {
            let stdin = std::io::stdin();
            if !stdin.is_terminal() {
                return Err(SeleniumBaseError::Gui(format!(
                    "cannot ask for text: no dialog program could be used ({}) and standard input is not a terminal",
                    if problems.is_empty() {
                        "none is known for this platform".to_owned()
                    } else {
                        problems.join("; ")
                    }
                )));
            }
            ask_terminal(
                &mut stdin.lock(),
                &mut std::io::stderr(),
                title,
                message,
                default,
            )
            .map_err(SeleniumBaseError::from)
        },
    )
}

/// Blocking prompt for simple synchronous scripts.
///
/// Like [`try_prompt`], with the answer as a [`DialogResult`]. If nothing can
/// ask, the result is "not confirmed" and the reason is logged.
pub fn prompt(title: &str, message: &str, default: Option<&str>) -> DialogResult {
    let text = show_prompt(title, message, default);
    DialogResult {
        confirmed: text.is_some(),
        text,
    }
}

/// Opens a native file chooser dialog and returns the selected path.
pub fn choose_file(title: &str) -> Option<String> {
    rfd::FileDialog::new()
        .set_title(title)
        .pick_file()
        .map(|p: std::path::PathBuf| p.to_string_lossy().to_string())
}

/// Saves to a path chosen by the user.
pub fn save_file(title: &str, default_name: &str) -> Option<String> {
    rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(default_name)
        .save_file()
        .map(|p: std::path::PathBuf| p.to_string_lossy().to_string())
}

/// A program that shows a text prompt, and the arguments to give it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PromptCommand {
    program: &'static str,
    args: Vec<String>,
}

/// What asking one program came to.
#[derive(Debug, Eq, PartialEq)]
enum Reply {
    /// The person confirmed with this text.
    Text(String),
    /// The person cancelled.
    Cancelled,
    /// The program could not be used; the reason is for the error message.
    Unavailable(String),
}

/// The programs to try, best first, on this platform.
///
/// Title, message and default go in as separate arguments and are never
/// spliced into a script, so no quote or newline in them can change what the
/// program does.
fn prompt_commands(title: &str, message: &str, default: &str) -> Vec<PromptCommand> {
    if cfg!(target_os = "macos") {
        vec![osascript_command(title, message, default)]
    } else if cfg!(target_os = "linux") {
        vec![
            zenity_command(title, message, default),
            kdialog_command(title, message, default),
        ]
    } else {
        Vec::new()
    }
}

/// `osascript` running a fixed script that reads title, message and default
/// from its arguments (`argv`).
fn osascript_command(title: &str, message: &str, default: &str) -> PromptCommand {
    let mut args: Vec<String> = [
        "on run argv",
        "set reply to display dialog (item 2 of argv) with title (item 1 of argv) default answer (item 3 of argv)",
        "return text returned of reply",
        "end run",
    ]
    .into_iter()
    .flat_map(|line| ["-e".to_owned(), line.to_owned()])
    .collect();
    args.extend([
        "--".to_owned(),
        title.to_owned(),
        message.to_owned(),
        default.to_owned(),
    ]);
    PromptCommand {
        program: "osascript",
        args,
    }
}

fn zenity_command(title: &str, message: &str, default: &str) -> PromptCommand {
    PromptCommand {
        program: "zenity",
        args: vec![
            "--entry".to_owned(),
            "--title".to_owned(),
            title.to_owned(),
            "--text".to_owned(),
            message.to_owned(),
            "--entry-text".to_owned(),
            default.to_owned(),
        ],
    }
}

fn kdialog_command(title: &str, message: &str, default: &str) -> PromptCommand {
    PromptCommand {
        program: "kdialog",
        args: vec![
            "--title".to_owned(),
            title.to_owned(),
            "--inputbox".to_owned(),
            message.to_owned(),
            default.to_owned(),
        ],
    }
}

/// Runs one prompt program and reads its answer.
fn run_helper(command: &PromptCommand) -> Reply {
    if cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
    {
        return Reply::Unavailable(format!(
            "{} needs a graphical session (neither DISPLAY nor WAYLAND_DISPLAY is set)",
            command.program
        ));
    }
    match Command::new(command.program)
        .args(&command.args)
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => interpret(
            command.program,
            output.status.code(),
            &output.stdout,
            &output.stderr,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Reply::Unavailable(format!("{} is not installed", command.program))
        }
        Err(error) => Reply::Unavailable(format!("cannot run {}: {error}", command.program)),
    }
}

/// Reads what a finished prompt program says.
///
/// `osascript` reports a cancelled dialog as error -128 on standard error;
/// `zenity` and `kdialog` exit with status 1. The answer is standard output
/// minus the single newline the program adds, so spaces the person typed
/// survive.
fn interpret(program: &str, exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Reply {
    let stderr = String::from_utf8_lossy(stderr);
    match exit_code {
        Some(0) => {
            let text = String::from_utf8_lossy(stdout);
            let text = text.strip_suffix('\n').unwrap_or(&text);
            Reply::Text(text.strip_suffix('\r').unwrap_or(text).to_owned())
        }
        Some(code) if program == "osascript" => {
            if stderr.contains("(-128)") {
                Reply::Cancelled
            } else {
                Reply::Unavailable(format!(
                    "osascript failed with status {code}: {}",
                    stderr.trim()
                ))
            }
        }
        Some(1) => Reply::Cancelled,
        Some(code) => Reply::Unavailable(format!(
            "{program} failed with status {code}: {}",
            stderr.trim()
        )),
        None => Reply::Unavailable(format!("{program} was ended by a signal")),
    }
}

/// Tries each program in turn, then `terminal`.
///
/// A program that answers, or is cancelled, ends the search: a person who
/// pressed Cancel is not asked again somewhere else.
fn ask(
    candidates: &[PromptCommand],
    mut run: impl FnMut(&PromptCommand) -> Reply,
    terminal: impl FnOnce(&[String]) -> Result<Option<String>, SeleniumBaseError>,
) -> Result<Option<String>, SeleniumBaseError> {
    let mut problems = Vec::new();
    for candidate in candidates {
        match run(candidate) {
            Reply::Text(text) => return Ok(Some(text)),
            Reply::Cancelled => return Ok(None),
            Reply::Unavailable(reason) => problems.push(reason),
        }
    }
    terminal(&problems)
}

/// Asks on a terminal: shows the question and reads one line.
///
/// An empty line gives `default` (or an empty answer if there is none); the end
/// of input counts as cancelling.
fn ask_terminal(
    input: &mut impl BufRead,
    output: &mut impl Write,
    title: &str,
    message: &str,
    default: Option<&str>,
) -> std::io::Result<Option<String>> {
    match default {
        Some(default) => write!(output, "{title}\n{message} [{default}]: ")?,
        None => write!(output, "{title}\n{message}: ")?,
    }
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let line = line.trim_end_matches(['\n', '\r']);
    Ok(Some(if line.is_empty() {
        default.unwrap_or_default().to_owned()
    } else {
        line.to_owned()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_result_defaults() {
        let result = DialogResult::default();
        assert!(!result.confirmed);
        assert!(result.text.is_none());
    }

    #[test]
    fn a_hostile_title_stays_one_argument_and_never_reaches_the_script() {
        let hostile = "\"; do shell script \"touch /tmp/pwned\"; \"\n'";
        let command = osascript_command(hostile, "m", "d");
        assert_eq!(command.program, "osascript");

        let separator = command
            .args
            .iter()
            .position(|arg| arg == "--")
            .expect("script lines end at --");
        // Everything before `--` is the fixed script; the text is after it.
        assert!(command.args[..separator]
            .iter()
            .all(|arg| !arg.contains("pwned") && !arg.contains("touch")));
        assert_eq!(command.args[separator + 1..], [hostile, "m", "d"]);
    }

    #[test]
    fn the_prompt_arguments_are_title_then_message_then_default() {
        let zenity = zenity_command("T", "M", "D");
        assert_eq!(
            zenity.args,
            [
                "--entry",
                "--title",
                "T",
                "--text",
                "M",
                "--entry-text",
                "D"
            ]
        );
        let kdialog = kdialog_command("T", "M", "D");
        assert_eq!(kdialog.args, ["--title", "T", "--inputbox", "M", "D"]);
    }

    #[test]
    fn each_platform_has_the_programs_it_ships_with() {
        let programs: Vec<&str> = prompt_commands("t", "m", "")
            .iter()
            .map(|command| command.program)
            .collect();
        if cfg!(target_os = "macos") {
            assert_eq!(programs, ["osascript"]);
        } else if cfg!(target_os = "linux") {
            assert_eq!(programs, ["zenity", "kdialog"]);
        } else {
            assert!(programs.is_empty());
        }
    }

    #[test]
    fn a_confirmed_answer_keeps_its_spaces_and_loses_only_the_added_newline() {
        let reply = interpret("zenity", Some(0), b"  Ada Lovelace \n", b"");
        assert_eq!(reply, Reply::Text("  Ada Lovelace ".to_owned()));
        assert_eq!(
            interpret("kdialog", Some(0), b"two\nlines\r\n", b""),
            Reply::Text("two\nlines".to_owned())
        );
        // Confirming an empty field is an answer, not a cancel.
        assert_eq!(
            interpret("osascript", Some(0), b"\n", b""),
            Reply::Text(String::new())
        );
    }

    #[test]
    fn cancelling_is_told_apart_from_a_program_that_failed() {
        let cancelled = b"execution error: User canceled. (-128)\n";
        assert_eq!(
            interpret("osascript", Some(1), b"", cancelled),
            Reply::Cancelled
        );
        assert_eq!(interpret("zenity", Some(1), b"", b""), Reply::Cancelled);
        assert_eq!(interpret("kdialog", Some(1), b"", b""), Reply::Cancelled);

        assert!(matches!(
            interpret("osascript", Some(1), b"", b"execution error: no window server (-1713)\n"),
            Reply::Unavailable(reason) if reason.contains("-1713")
        ));
        assert!(matches!(
            interpret("zenity", Some(5), b"", b"boom"),
            Reply::Unavailable(reason) if reason.contains("boom")
        ));
        assert!(matches!(
            interpret("zenity", None, b"", b""),
            Reply::Unavailable(reason) if reason.contains("signal")
        ));
    }

    fn command(program: &'static str) -> PromptCommand {
        PromptCommand {
            program,
            args: Vec::new(),
        }
    }

    #[test]
    fn the_search_moves_on_from_a_program_that_is_missing_but_not_from_a_cancel() {
        let candidates = [command("first"), command("second")];

        // The first is not installed; the second answers.
        let mut asked = Vec::new();
        let answer = ask(
            &candidates,
            |c| {
                asked.push(c.program);
                if c.program == "first" {
                    Reply::Unavailable("not installed".to_owned())
                } else {
                    Reply::Text("typed".to_owned())
                }
            },
            |_| panic!("the terminal is not needed"),
        )
        .unwrap();
        assert_eq!(answer.as_deref(), Some("typed"));
        assert_eq!(asked, ["first", "second"]);

        // The first is cancelled: nobody else is asked.
        let mut asked = Vec::new();
        let answer = ask(
            &candidates,
            |c| {
                asked.push(c.program);
                Reply::Cancelled
            },
            |_| panic!("a cancel must not fall back to the terminal"),
        )
        .unwrap();
        assert_eq!(answer, None);
        assert_eq!(asked, ["first"]);
    }

    #[test]
    fn when_no_program_can_be_used_the_terminal_is_told_why() {
        let candidates = [command("first"), command("second")];
        let answer = ask(
            &candidates,
            |c| Reply::Unavailable(format!("{} is not installed", c.program)),
            |problems| {
                assert_eq!(
                    problems,
                    ["first is not installed", "second is not installed"]
                );
                Ok(Some("from the terminal".to_owned()))
            },
        )
        .unwrap();
        assert_eq!(answer.as_deref(), Some("from the terminal"));

        let error = ask(
            &[],
            |_| unreachable!(),
            |_| Err(SeleniumBaseError::Gui("nothing can ask".to_owned())),
        )
        .unwrap_err();
        assert!(matches!(error, SeleniumBaseError::Gui(_)));
    }

    fn terminal(input: &str, default: Option<&str>) -> (Option<String>, String) {
        let mut output = Vec::new();
        let answer = ask_terminal(
            &mut input.as_bytes(),
            &mut output,
            "Title",
            "Question",
            default,
        )
        .unwrap();
        (answer, String::from_utf8(output).unwrap())
    }

    #[test]
    fn the_terminal_returns_the_typed_line_or_the_default_for_an_empty_one() {
        let (answer, shown) = terminal("Ada\n", Some("guest"));
        assert_eq!(answer.as_deref(), Some("Ada"));
        assert_eq!(shown, "Title\nQuestion [guest]: ");

        assert_eq!(terminal("\n", Some("guest")).0.as_deref(), Some("guest"));
        assert_eq!(terminal("\r\n", Some("guest")).0.as_deref(), Some("guest"));
        assert_eq!(terminal("\n", None).0.as_deref(), Some(""));
        assert_eq!(terminal("Bob", None).0.as_deref(), Some("Bob"));
        assert_eq!(
            terminal("  spaced  \n", None).0.as_deref(),
            Some("  spaced  ")
        );
        assert_eq!(terminal("x\n", None).1, "Title\nQuestion: ");
    }

    #[test]
    fn the_end_of_input_on_a_terminal_counts_as_cancelling() {
        assert_eq!(terminal("", Some("guest")).0, None);
    }

    /// Whatever the person types has to arrive intact. `osascript` takes the
    /// title, message and default as arguments; this runs the same argument
    /// mechanism with a script that returns an argument instead of opening a
    /// window.
    #[cfg(target_os = "macos")]
    #[test]
    fn osascript_arguments_round_trip_quotes_newlines_and_unicode() {
        let awkward = "say \"hi\" \\ 'there'\nsecond line, ünïcödé ✓ $(echo no)";
        let output = Command::new("osascript")
            .args([
                "-e",
                "on run argv",
                "-e",
                "return item 2 of argv",
                "-e",
                "end run",
                "--",
                "first",
                awkward,
                "third",
            ])
            .stdin(Stdio::null())
            .output()
            .expect("osascript runs");
        assert!(output.status.success(), "{output:?}");
        // The script's real arguments are passed the same way, and read the same way.
        assert_eq!(
            interpret(
                "osascript",
                output.status.code(),
                &output.stdout,
                &output.stderr
            ),
            Reply::Text(awkward.to_owned())
        );
    }

    // The native dialogs themselves cannot be shown headlessly; the examples
    // `dialog` and `dialogs` exercise them by hand.
}
