//! The prompts of `sbrs encrypt` and `sbrs decrypt` on a terminal, as Python's `sbase
//! obfuscate` and `sbase unobfuscate` have them.
//!
//! With no argument and standard input a terminal, `encrypt` asks for the text without echo,
//! asks again to verify it, prints the token and how to decrypt it in a test, and asks for the
//! next text; `decrypt` asks for a token on the screen and prints the text. Both go on until
//! Ctrl-C or the end of the input (Ctrl-D), which print `Exiting...` and end with status 0.
//!
//! The passphrase comes from `SB_ENCRYPTION_KEY`. When it is not set, it is asked for once,
//! without echo; it is never printed and never written anywhere. The loops take their input
//! and output as parameters, so the tests run them without a terminal.

use std::error::Error;
use std::io::{self, BufRead, IsTerminal, Write};
use std::time::Duration;

use seleniumbase_rs::common::encryption::{decrypt_with_passphrase, encrypt_with_passphrase};
use seleniumbase_rs::common::shutdown::Shutdown;

/// What the loops end with: errors that can cross to the blocking thread the terminal loop
/// runs on.
type Failure = Box<dyn Error + Send + Sync>;

/// Where the prompts read from.
pub(crate) trait Input {
    /// A line read without echo, or `None` at the end of the input.
    fn hidden(&mut self) -> io::Result<Option<String>>;
    /// A line read with echo, or `None` at the end of the input.
    fn visible(&mut self) -> io::Result<Option<String>>;
}

/// The terminal: hidden lines come from `rpassword`, visible ones from standard input.
struct Terminal;

impl Input for Terminal {
    fn hidden(&mut self) -> io::Result<Option<String>> {
        match rpassword::read_password() {
            Ok(line) => Ok(Some(line)),
            // Ctrl-D ends the input; Ctrl-C reaches the read as an interruption.
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn visible(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line)? == 0 {
            return Ok(None);
        }
        Ok(Some(line.trim_end_matches(['\r', '\n']).to_owned()))
    }
}

/// Whether `encrypt` and `decrypt` with no argument should prompt: standard input is a
/// terminal. Anything else keeps the old behaviour of reading standard input once.
pub(crate) fn prompts() -> bool {
    io::stdin().is_terminal()
}

/// Which of the two commands to prompt for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    Encrypt,
    Decrypt,
}

/// Runs the prompts on the terminal until the end of the input, or until Ctrl-C or SIGTERM,
/// which print `Exiting...` and end the process with status 0.
pub(crate) async fn interactive(kind: Kind) -> Result<(), Box<dyn Error>> {
    let mut shutdown = Shutdown::install()?;
    // A line is read on a thread of its own, so that a signal can end the process while it
    // waits. (The handles are not locked: the signal path prints too.)
    let mut session = tokio::task::spawn_blocking(move || {
        let key = std::env::var("SB_ENCRYPTION_KEY")
            .ok()
            .filter(|key| !key.is_empty());
        let (mut out, mut err) = (io::stdout(), io::stderr());
        match kind {
            Kind::Encrypt => encrypt_loop(&mut Terminal, &mut out, &mut err, key),
            Kind::Decrypt => decrypt_loop(&mut Terminal, &mut out, &mut err, key),
        }
    });
    tokio::select! {
        done = &mut session => done
            .map_err(|error| format!("the prompt stopped: {error}"))?
            .map_err(|error| -> Box<dyn Error> { error })?,
        _ = shutdown.recv() => {
            // A read without echo is cut short by the signal and says goodbye itself.
            if let Ok(done) = tokio::time::timeout(Duration::from_millis(300), &mut session).await {
                done.map_err(|error| format!("the prompt stopped: {error}"))?
                    .map_err(|error| -> Box<dyn Error> { error })?;
                return Ok(());
            }
            println!("\nExiting...\n");
            // The read that was waiting turned the echo off and cannot put it back now.
            restore_echo();
            std::process::exit(0);
        }
    }
    Ok(())
}

/// Turns the terminal's echo back on after a read without echo was cut short.
fn restore_echo() {
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("stty").arg("echo").status();
    }
}

/// The passphrase, asked for without echo and, to catch a typo that would make every token
/// unreadable, a second time when `verify` is set. `None` at the end of the input.
fn ask_passphrase(
    input: &mut impl Input,
    err: &mut impl Write,
    verify: bool,
) -> Result<Option<String>, Failure> {
    loop {
        writeln!(
            err,
            "\nEnter the passphrase (not shown; or set SB_ENCRYPTION_KEY): (CTRL+C to exit)"
        )?;
        let Some(first) = input.hidden()? else {
            return Ok(None);
        };
        if first.is_empty() {
            writeln!(
                err,
                "*** ERROR: The passphrase cannot be empty! .. Please try again!"
            )?;
            continue;
        }
        if !verify {
            return Ok(Some(first));
        }
        writeln!(err, "Verify passphrase:")?;
        let Some(second) = input.hidden()? else {
            return Ok(None);
        };
        if first == second {
            return Ok(Some(first));
        }
        writeln!(
            err,
            "*** ERROR: Passphrases don't match! .. Please try again!"
        )?;
    }
}

fn exiting(out: &mut impl Write) -> Result<(), Failure> {
    writeln!(out, "\nExiting...\n")?;
    Ok(())
}

/// The `encrypt` prompts: text in, token out, until the input ends.
///
/// `passphrase` is the one from the environment; without it the loop asks once.
///
/// # Errors
///
/// Fails when the terminal cannot be read or written, or when a text cannot be encrypted.
pub(crate) fn encrypt_loop(
    input: &mut impl Input,
    out: &mut impl Write,
    err: &mut impl Write,
    passphrase: Option<String>,
) -> Result<(), Failure> {
    let key = match passphrase {
        Some(key) => key,
        None => match ask_passphrase(input, err, true)? {
            Some(key) => key,
            None => return exiting(out),
        },
    };
    loop {
        writeln!(err, "\nEnter password to obfuscate: (CTRL+C to exit)")?;
        let Some(text) = input.hidden()? else {
            return exiting(out);
        };
        writeln!(err, "Verify password:")?;
        let Some(again) = input.hidden()? else {
            return exiting(out);
        };
        if text != again {
            writeln!(
                err,
                "*** ERROR: Passwords don't match! .. Please try again!"
            )?;
            continue;
        }
        let token = encrypt_with_passphrase(&text, &key)?;
        writeln!(out, "\nHere is the obfuscated password:\n{token}")?;
        writeln!(out, "\nInside a test, use the following to decrypt it:\n")?;
        writeln!(
            out,
            "    seleniumbase_rs::common::encryption::decrypt_with_passphrase(\"{token}\", &key)"
        )?;
    }
}

/// The `decrypt` prompts: token in, text out, until the input ends. A token that does not
/// decrypt is reported and the loop asks for the next one.
///
/// `passphrase` is the one from the environment; without it the loop asks once.
///
/// # Errors
///
/// Fails when the terminal cannot be read or written.
pub(crate) fn decrypt_loop(
    input: &mut impl Input,
    out: &mut impl Write,
    err: &mut impl Write,
    passphrase: Option<String>,
) -> Result<(), Failure> {
    let key = match passphrase {
        Some(key) => key,
        None => match ask_passphrase(input, err, false)? {
            Some(key) => key,
            None => return exiting(out),
        },
    };
    loop {
        writeln!(
            err,
            "\nEnter obfuscated/encrypted string: (CTRL+C to exit):"
        )?;
        let Some(token) = input.visible()? else {
            return exiting(out);
        };
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match decrypt_with_passphrase(token, &key) {
            Ok(text) => writeln!(out, "\nHere is the unobfuscated string/password:\n{text}")?,
            Err(error) => writeln!(err, "error: {}", crate::wrong_passphrase(error))?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Lines to hand out, a `None` being the end of the input.
    #[derive(Default)]
    struct Script {
        hidden: VecDeque<Option<String>>,
        visible: VecDeque<Option<String>>,
    }

    impl Script {
        fn hidden(lines: &[&str]) -> Self {
            let mut script = Self::default();
            script
                .hidden
                .extend(lines.iter().map(|line| Some((*line).to_owned())));
            script
        }

        fn visible(lines: &[&str]) -> Self {
            let mut script = Self::default();
            script
                .visible
                .extend(lines.iter().map(|line| Some((*line).to_owned())));
            script
        }
    }

    impl Input for Script {
        fn hidden(&mut self) -> io::Result<Option<String>> {
            Ok(self.hidden.pop_front().flatten())
        }

        fn visible(&mut self) -> io::Result<Option<String>> {
            Ok(self.visible.pop_front().flatten())
        }
    }

    /// What a loop printed on standard output and on standard error.
    struct Printed {
        out: String,
        err: String,
    }

    fn run(script: &mut Script, passphrase: Option<&str>, decrypt: bool) -> Printed {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let key = passphrase.map(str::to_owned);
        let result = if decrypt {
            decrypt_loop(script, &mut out, &mut err, key)
        } else {
            encrypt_loop(script, &mut out, &mut err, key)
        };
        result.expect("the loop ends at the end of the input");
        Printed {
            out: String::from_utf8(out).expect("text"),
            err: String::from_utf8(err).expect("text"),
        }
    }

    /// The tokens a run printed.
    fn tokens(printed: &Printed) -> Vec<&str> {
        printed
            .out
            .lines()
            .filter(|line| line.starts_with("sbenc1:"))
            .collect()
    }

    #[test]
    fn a_text_is_asked_twice_and_comes_back_as_a_token_with_the_way_to_decrypt_it() {
        let printed = run(
            &mut Script::hidden(&["plain-secret", "plain-secret"]),
            Some("key-1"),
            false,
        );
        assert!(
            printed
                .err
                .contains("Enter password to obfuscate: (CTRL+C to exit)"),
            "{}",
            printed.err
        );
        assert!(printed.err.contains("Verify password:"), "{}", printed.err);
        assert!(
            printed.out.contains("Here is the obfuscated password:"),
            "{}",
            printed.out
        );
        assert!(printed
            .out
            .contains("Inside a test, use the following to decrypt it:"));
        let tokens = tokens(&printed);
        assert_eq!(tokens.len(), 1, "{}", printed.out);
        assert_eq!(
            decrypt_with_passphrase(tokens[0], "key-1").unwrap(),
            "plain-secret"
        );
        assert!(
            printed.out.contains(&format!(
                "seleniumbase_rs::common::encryption::decrypt_with_passphrase(\"{}\", &key)",
                tokens[0]
            )),
            "{}",
            printed.out
        );
        assert!(
            printed.out.trim_end().ends_with("Exiting..."),
            "{}",
            printed.out
        );
        assert!(!printed.out.contains("plain-secret") && !printed.err.contains("plain-secret"));
    }

    #[test]
    fn texts_that_do_not_match_are_asked_for_again() {
        let printed = run(
            &mut Script::hidden(&["a", "b", "c", "c"]),
            Some("key-1"),
            false,
        );
        assert_eq!(
            printed
                .err
                .matches("*** ERROR: Passwords don't match! .. Please try again!")
                .count(),
            1,
            "{}",
            printed.err
        );
        let tokens = tokens(&printed);
        assert_eq!(tokens.len(), 1);
        assert_eq!(decrypt_with_passphrase(tokens[0], "key-1").unwrap(), "c");
    }

    #[test]
    fn the_loop_goes_on_with_the_next_text() {
        let printed = run(
            &mut Script::hidden(&["one", "one", "two", "two"]),
            Some("key-1"),
            false,
        );
        let tokens = tokens(&printed);
        assert_eq!(tokens.len(), 2, "{}", printed.out);
        assert_eq!(decrypt_with_passphrase(tokens[0], "key-1").unwrap(), "one");
        assert_eq!(decrypt_with_passphrase(tokens[1], "key-1").unwrap(), "two");
    }

    #[test]
    fn a_missing_passphrase_is_asked_for_once_and_never_shown() {
        let mut script = Script::hidden(&[
            "hunter2-passphrase",
            "hunter2-passphrase",
            "plain-secret",
            "plain-secret",
            "other-secret",
            "other-secret",
        ]);
        let printed = run(&mut script, None, false);
        assert_eq!(
            printed.err.matches("Enter the passphrase").count(),
            1,
            "{}",
            printed.err
        );
        let tokens = tokens(&printed);
        assert_eq!(tokens.len(), 2);
        assert_eq!(
            decrypt_with_passphrase(tokens[1], "hunter2-passphrase").unwrap(),
            "other-secret"
        );
        for text in [&printed.out, &printed.err] {
            assert!(
                !text.contains("hunter2-passphrase") && !text.contains("plain-secret"),
                "{text}"
            );
        }
    }

    #[test]
    fn a_passphrase_that_does_not_verify_or_is_empty_is_asked_for_again() {
        let mut script = Script::hidden(&["", "k1", "k2", "k", "k", "text", "text"]);
        let printed = run(&mut script, None, false);
        assert!(
            printed.err.contains("The passphrase cannot be empty"),
            "{}",
            printed.err
        );
        assert!(
            printed.err.contains("Passphrases don't match"),
            "{}",
            printed.err
        );
        assert_eq!(
            decrypt_with_passphrase(tokens(&printed)[0], "k").unwrap(),
            "text"
        );
    }

    #[test]
    fn the_end_of_the_input_says_goodbye_wherever_it_comes() {
        for lines in [&[][..], &["key"][..], &["key", "key"][..]] {
            let printed = run(&mut Script::hidden(lines), None, false);
            assert!(
                printed.out.trim_end().ends_with("Exiting..."),
                "{lines:?}: {}",
                printed.out
            );
            assert!(tokens(&printed).is_empty());
        }
        let printed = run(&mut Script::hidden(&["only-one"]), Some("key"), false);
        assert!(
            printed.out.trim_end().ends_with("Exiting..."),
            "{}",
            printed.out
        );
    }

    #[test]
    fn a_token_is_asked_for_on_the_screen_and_the_text_comes_back() {
        let token = encrypt_with_passphrase("plain-secret", "key-1").unwrap();
        let printed = run(
            &mut Script::visible(&[&token, "  ", "not a token", &token]),
            Some("key-1"),
            true,
        );
        assert!(printed
            .err
            .contains("Enter obfuscated/encrypted string: (CTRL+C to exit):"));
        assert_eq!(
            printed
                .out
                .matches("Here is the unobfuscated string/password:\nplain-secret")
                .count(),
            2,
            "{}",
            printed.out
        );
        assert!(
            printed.err.contains("error: "),
            "a bad token is reported: {}",
            printed.err
        );
        assert!(printed.out.trim_end().ends_with("Exiting..."));
    }

    #[test]
    fn the_wrong_passphrase_prints_an_error_and_no_text() {
        let token = encrypt_with_passphrase("plain-secret", "right").unwrap();
        let printed = run(&mut Script::visible(&[&token]), Some("wrong"), true);
        assert!(printed.err.contains("passphrase"), "{}", printed.err);
        assert!(!printed.out.contains("plain-secret"), "{}", printed.out);
    }

    #[test]
    fn decrypt_asks_for_a_missing_passphrase_once_without_verifying() {
        let token = encrypt_with_passphrase("plain-secret", "key-1").unwrap();
        let mut script = Script::visible(&[&token]);
        script.hidden.push_back(Some("key-1".to_owned()));
        let printed = run(&mut script, None, true);
        assert_eq!(
            printed.err.matches("Enter the passphrase").count(),
            1,
            "{}",
            printed.err
        );
        assert!(
            !printed.err.contains("Verify passphrase"),
            "{}",
            printed.err
        );
        assert!(printed.out.contains("plain-secret"), "{}", printed.out);
    }
}
