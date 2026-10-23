//! The interactive pause of the debug modes: Python's `breakpoint()` in a test,
//! `--trace`, `--pdb` and `--final-debug`, with the browser left open.
//!
//! A [`DebugPrompt`] writes a banner, reads commands and acts on the open page
//! through a [`PromptPage`] until the person types `c` (continue) or `q`
//! (quit). It reads from any [`LineInput`] and writes to any [`Write`], so the
//! tests drive it with a script; [`DebugPrompt::from_terminal`] is the one that
//! talks to a person, and is `None` when nobody can answer (CI, a pipe).
//!
//! ```text
//! [BREAKPOINT]: Paused at login page setup.
//!   test:  login.valid_login   page: https://example.com/login  "Login"
//! Perform your manual actions in the browser now.
//! Commands: c continue | s screenshot | src page source | u url | js <expr> | q quit | h help
//! Type 'c' and press [ENTER] to continue execution:
//! ```
//!
//! The text goes to the raw standard error handle, not through `eprintln!`, so
//! the output capture of `cargo test` does not hide it. Several tests running
//! at once take turns: one prompt at a time. Pass `--test-threads 1` (and
//! `--nocapture` for a person typing at the terminal) so the answers reach the
//! test that is waiting; `cargo nextest` gives tests no standard input at all.
//!
//! The page is read and driven through [`PromptPage`], implemented for
//! [`BaseCase`](crate::BaseCase) and [`sb_cdp::Page`](crate::sb_cdp::Page).
//! `BaseCase::breakpoint` and `Page::breakpoint` are the calls a test makes.
//!
//! # Examples
//!
//! A scripted prompt, as the tests use it:
//!
//! ```
//! use async_trait::async_trait;
//! use seleniumbase_rs::common::debug_prompt::{
//!     DebugPrompt, PromptKind, PromptOutcome, PromptPage,
//! };
//! use seleniumbase_rs::SeleniumBaseError;
//!
//! struct Fixed;
//!
//! #[async_trait]
//! impl PromptPage for Fixed {
//!     async fn url(&self) -> Result<String, SeleniumBaseError> {
//!         Ok("https://example.com/".to_owned())
//!     }
//!     async fn title(&self) -> Result<String, SeleniumBaseError> {
//!         Ok("Example".to_owned())
//!     }
//!     async fn screenshot_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
//!         Ok(Vec::new())
//!     }
//!     async fn page_source(&self) -> Result<String, SeleniumBaseError> {
//!         Ok("<html></html>".to_owned())
//!     }
//!     async fn evaluate(&self, _expression: &str) -> Result<serde_json::Value, SeleniumBaseError> {
//!         Ok(serde_json::json!(2))
//!     }
//! }
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), SeleniumBaseError> {
//! let mut prompt = DebugPrompt::from_reader("u\nc\n".as_bytes(), Vec::new());
//! let outcome = prompt
//!     .pause(PromptKind::Breakpoint, "demo.test", "look around", &Fixed)
//!     .await?;
//! assert_eq!(outcome, PromptOutcome::Continue);
//! assert!(String::from_utf8_lossy(prompt.output()).contains("[BREAKPOINT]: look around"));
//! # Ok(())
//! # }
//! ```

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::artifacts::LATEST_LOGS_DIR;
use crate::error::SeleniumBaseError;

/// The environment variable that turns `breakpoint()` off: `0`, `false`, `no`
/// or `off`, as Python's `PYTHONBREAKPOINT=0` does.
pub const BREAKPOINTS_VARIABLE: &str = "SB_BREAKPOINTS";

/// The most characters of a test id used as a folder name; a longer id keeps
/// its end, the most specific part.
const MAX_FOLDER_LEN: usize = 120;

/// How many numbered files one test may save before the prompt gives up.
const MAX_NUMBERED_FILES: u32 = 10_000;

/// The prompt that asks for the next command.
const COMMAND_PROMPT: &str = "> ";

/// The command summary every banner ends with.
const COMMANDS: &str =
    "Commands: c continue | s screenshot | src page source | u url | js <expr> | q quit | h help";

/// One prompt at a time, whichever runtime or test asks.
static TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Why the test is paused; it names the banner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptKind {
    /// A `breakpoint()` call in the test (`[BREAKPOINT]`).
    Breakpoint,
    /// A breakpoint where a person records actions by hand
    /// (`[RECORDER BREAKPOINT]`). Continuing saves a screenshot.
    RecorderBreakpoint,
    /// `--trace`: before the test body (`[TRACE]`).
    Trace,
    /// `--pdb`: after a failed test, the browser still open (`[POST MORTEM]`).
    PostMortem,
    /// `--final-debug`: after every test, before the browser closes
    /// (`[FINAL TRACE]`).
    FinalTrace,
}

impl PromptKind {
    /// The words in the brackets of the banner, such as `POST MORTEM`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Breakpoint => "BREAKPOINT",
            Self::RecorderBreakpoint => "RECORDER BREAKPOINT",
            Self::Trace => "TRACE",
            Self::PostMortem => "POST MORTEM",
            Self::FinalTrace => "FINAL TRACE",
        }
    }

    /// Whether the banner tells the person to work in the browser now.
    const fn invites_manual_actions(self) -> bool {
        matches!(self, Self::Breakpoint | Self::RecorderBreakpoint)
    }
}

/// How a pause ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptOutcome {
    /// Go on with the test: `c`, `continue`, or the end of the input.
    Continue,
    /// Stop the test: `q` or `quit`.
    Quit,
}

/// What the prompt can do with the open page.
///
/// Implemented for [`BaseCase`](crate::BaseCase) and
/// [`sb_cdp::Page`](crate::sb_cdp::Page). A command that fails is reported at
/// the prompt and the pause goes on.
#[async_trait]
pub trait PromptPage: Sync {
    /// The page's current address.
    ///
    /// # Errors
    ///
    /// Returns the browser's error when the page cannot be asked.
    async fn url(&self) -> Result<String, SeleniumBaseError>;

    /// The page's title.
    ///
    /// # Errors
    ///
    /// Returns the browser's error when the page cannot be asked.
    async fn title(&self) -> Result<String, SeleniumBaseError>;

    /// The viewport as PNG bytes.
    ///
    /// # Errors
    ///
    /// Returns the browser's error when it cannot capture the screen.
    async fn screenshot_png(&self) -> Result<Vec<u8>, SeleniumBaseError>;

    /// The page's current HTML.
    ///
    /// # Errors
    ///
    /// Returns the browser's error when the page cannot be asked.
    async fn page_source(&self) -> Result<String, SeleniumBaseError>;

    /// Evaluates one JavaScript expression and returns its JSON value.
    ///
    /// # Errors
    ///
    /// Returns the browser's error when the script throws.
    async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError>;
}

/// Where a prompt reads the person's answers from, one line at a time.
#[async_trait]
pub trait LineInput: Send {
    /// The next line without its line end, or `None` at the end of the input.
    ///
    /// # Errors
    ///
    /// Returns the error of the underlying reader.
    async fn read_line(&mut self) -> io::Result<Option<String>>;
}

/// The standard input of the process, read on a thread of its own so a pause
/// never holds up the runtime's workers.
///
/// A line that is being waited for cannot be taken back: if the test is
/// stopped while the prompt waits, the thread ends only when a line (or the
/// end of the input) arrives.
#[derive(Debug)]
pub struct TerminalInput(());

#[async_trait]
impl LineInput for TerminalInput {
    async fn read_line(&mut self) -> io::Result<Option<String>> {
        read_stdin_line().await
    }
}

/// Lines from any reader, for scripts, pipes and tests. Reading is done on the
/// calling task, so use [`TerminalInput`] for a terminal.
#[derive(Debug)]
pub struct ReaderInput<R>(R);

#[async_trait]
impl<R: BufRead + Send> LineInput for ReaderInput<R> {
    async fn read_line(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        let read = self.0.read_line(&mut line)?;
        Ok((read > 0).then(|| trim_line_end(&line).to_owned()))
    }
}

/// `line` without its `\n` or `\r\n`.
fn trim_line_end(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

/// Reads one line of the process's standard input without blocking a runtime
/// worker. `None` at the end of the input (a closed pipe, Ctrl-D, nextest's
/// null input).
///
/// Crate-private so the other prompts that ask a person a question (the
/// recorder's breakpoint, `MasterQA`) read the same way.
pub(crate) async fn read_stdin_line() -> io::Result<Option<String>> {
    tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        let read = io::stdin().lock().read_line(&mut line)?;
        Ok((read > 0).then(|| trim_line_end(&line).to_owned()))
    })
    .await
    .map_err(io::Error::other)?
}

/// An interactive pause. See the [module documentation](self).
#[derive(Debug)]
pub struct DebugPrompt<I, W> {
    input: I,
    output: W,
    logs_dir: PathBuf,
}

impl DebugPrompt<TerminalInput, io::Stderr> {
    /// A prompt that talks to the person at the terminal, or `None` when
    /// standard input is not a terminal (CI, a pipe, nextest): the caller logs
    /// a warning and goes on, so a test never hangs.
    #[must_use]
    pub fn from_terminal() -> Option<Self> {
        io::stdin()
            .is_terminal()
            .then(|| Self::new(TerminalInput(()), io::stderr()))
    }
}

impl<R: BufRead + Send, W: Write + Send> DebugPrompt<ReaderInput<R>, W> {
    /// A prompt that reads its commands from `input` and writes to `output`.
    pub fn from_reader(input: R, output: W) -> Self {
        Self::new(ReaderInput(input), output)
    }
}

impl<I: LineInput, W: Write + Send> DebugPrompt<I, W> {
    /// A prompt that reads its commands from `input` and writes to `output`.
    ///
    /// Files the commands save go to `latest_logs/<test id>/`; see
    /// [`with_logs_dir`](Self::with_logs_dir).
    pub fn new(input: I, output: W) -> Self {
        Self {
            input,
            output,
            logs_dir: PathBuf::from(LATEST_LOGS_DIR),
        }
    }

    /// Saves the files of `s` and `src` under `dir` instead of `latest_logs`.
    #[must_use]
    pub fn with_logs_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.logs_dir = dir.into();
        self
    }

    /// What the prompt has written so far.
    pub fn output(&self) -> &W {
        &self.output
    }

    /// Pauses until the person says `c` or `q`.
    ///
    /// Writes the banner for `kind`, naming `test_id`, `message` and the page,
    /// then reads commands: `c` or `continue`, `s` (saves
    /// `breakpoint_<n>.png`), `src` (saves `page_source_<n>.html`), `u`, `js
    /// <expr>`, `q` or `quit`, `h`. Both files go to `<logs>/<test id>/`. The end
    /// of the input counts as `c`, so a closed input never hangs the test.
    /// Continuing from a [`PromptKind::RecorderBreakpoint`] also saves a
    /// screenshot.
    ///
    /// A command that fails (the browser is gone, the script throws) is
    /// reported and the pause goes on.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] when the input cannot be read or the
    /// output cannot be written.
    pub async fn pause(
        &mut self,
        kind: PromptKind,
        test_id: &str,
        message: &str,
        page: &dyn PromptPage,
    ) -> Result<PromptOutcome, SeleniumBaseError> {
        let _turn = TURN.lock().await;
        let folder = self.logs_dir.join(folder_name(test_id));
        let banner = banner(kind, test_id, message, &page_line(page).await);
        self.say(&banner)?;
        let outcome = loop {
            let Some(line) = self.input.read_line().await? else {
                self.say("\ninput closed: continuing\n")?;
                break PromptOutcome::Continue;
            };
            match parse(&line) {
                Command::Continue => break PromptOutcome::Continue,
                Command::Quit => break PromptOutcome::Quit,
                Command::Screenshot => {
                    let report = save_screenshot(page, &folder).await;
                    self.say(&format!("{report}\n"))?;
                }
                Command::Source => {
                    let report = save_source(page, &folder).await;
                    self.say(&format!("{report}\n"))?;
                }
                Command::Url => {
                    let line = page_line(page).await;
                    self.say(&format!("{line}\n"))?;
                }
                Command::Js(expression) => {
                    let report = evaluate(page, expression).await;
                    self.say(&format!("{report}\n"))?;
                }
                Command::JsUsage => self.say("usage: js <expression>\n")?,
                Command::Help => self.say(&help())?,
                Command::Empty => {}
                Command::Unknown(word) => {
                    self.say(&format!("unknown command '{word}'; h shows the commands\n"))?;
                }
            }
            self.say(COMMAND_PROMPT)?;
        };
        if kind == PromptKind::RecorderBreakpoint && outcome == PromptOutcome::Continue {
            let report = save_screenshot(page, &folder).await;
            self.say(&format!("{report}\n"))?;
        }
        Ok(outcome)
    }

    /// Writes `text` in one piece, so a prompt of another process does not
    /// interleave with it, and flushes.
    fn say(&mut self, text: &str) -> io::Result<()> {
        self.output.write_all(text.as_bytes())?;
        self.output.flush()
    }
}

/// What a line typed at the prompt asks for.
#[derive(Debug, Eq, PartialEq)]
enum Command<'a> {
    Continue,
    Quit,
    Screenshot,
    Source,
    Url,
    Js(&'a str),
    JsUsage,
    Help,
    Empty,
    Unknown(&'a str),
}

/// The command in `line`.
fn parse(line: &str) -> Command<'_> {
    let line = line.trim();
    let (word, rest) = line
        .split_once(char::is_whitespace)
        .map_or((line, ""), |(word, rest)| (word, rest.trim()));
    match word.to_ascii_lowercase().as_str() {
        "" => Command::Empty,
        "c" | "continue" => Command::Continue,
        "q" | "quit" => Command::Quit,
        "s" | "screenshot" => Command::Screenshot,
        "src" | "source" => Command::Source,
        "u" | "url" => Command::Url,
        "js" if rest.is_empty() => Command::JsUsage,
        "js" => Command::Js(rest),
        "h" | "help" | "?" => Command::Help,
        _ => Command::Unknown(word),
    }
}

/// The text a pause starts with, ending at the prompt for the first command.
fn banner(kind: PromptKind, test_id: &str, message: &str, page: &str) -> String {
    let mut text = format!("\n[{}]", kind.label());
    if !message.is_empty() {
        text.push_str(": ");
        text.push_str(message);
    }
    text.push_str(&format!("\n  test:  {test_id}   {page}\n"));
    if kind.invites_manual_actions() {
        text.push_str("Perform your manual actions in the browser now.\n");
    }
    text.push_str(COMMANDS);
    text.push_str("\nType 'c' and press [ENTER] to continue execution: ");
    text
}

/// The answer to `h`.
fn help() -> String {
    format!(
        "{COMMANDS}\n  c, continue   go on with the test\n  s, screenshot save the viewport as breakpoint_<n>.png\n  src, source   save the page's HTML as page_source_<n>.html\n  u, url        show the page's address and title\n  js <expr>     evaluate a JavaScript expression and show its value\n  q, quit       stop the test\n{COMMAND_PROMPT}"
    )
}

/// `page: <url>  "<title>"`, or why the page cannot be asked.
async fn page_line(page: &dyn PromptPage) -> String {
    match (page.url().await, page.title().await) {
        (Ok(url), Ok(title)) => format!("page: {url}  \"{title}\""),
        (Ok(url), Err(_)) => format!("page: {url}"),
        (Err(error), _) => format!("page: (unavailable: {error})"),
    }
}

/// Saves the viewport as `breakpoint_<n>.png` in `folder`; what to tell the
/// person.
async fn save_screenshot(page: &dyn PromptPage, folder: &Path) -> String {
    match page.screenshot_png().await {
        Ok(bytes) => match save_numbered(folder, "breakpoint", "png", &bytes).await {
            Ok(path) => format!("saved {}", path.display()),
            Err(error) => format!("could not save the screenshot: {error}"),
        },
        Err(error) => format!("could not take a screenshot: {error}"),
    }
}

/// Saves the page's HTML as `page_source_<n>.html` in `folder`; what to tell
/// the person.
async fn save_source(page: &dyn PromptPage, folder: &Path) -> String {
    match page.page_source().await {
        Ok(html) => match save_numbered(folder, "page_source", "html", html.as_bytes()).await {
            Ok(path) => format!("saved {}", path.display()),
            Err(error) => format!("could not save the page source: {error}"),
        },
        Err(error) => format!("could not read the page source: {error}"),
    }
}

/// The value of `expression` as JSON, or the error; what to tell the person.
async fn evaluate(page: &dyn PromptPage, expression: &str) -> String {
    // A trailing `;` is fine at a prompt, but not inside `return (..)`.
    let expression = expression.trim_end_matches(|c: char| c == ';' || c.is_whitespace());
    match page.evaluate(expression).await {
        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
        Err(error) => format!("error: {error}"),
    }
}

/// Writes `bytes` to the first of `<stem>_1.<extension>`, `<stem>_2...` in `dir`
/// that does not exist, creating `dir`. An existing file is never replaced.
async fn save_numbered(
    dir: &Path,
    stem: &str,
    extension: &str,
    bytes: &[u8],
) -> io::Result<PathBuf> {
    tokio::fs::create_dir_all(dir).await?;
    for number in 1..=MAX_NUMBERED_FILES {
        let path = dir.join(format!("{stem}_{number}.{extension}"));
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(mut file) => {
                file.write_all(bytes).await?;
                file.flush().await?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(format!(
        "{MAX_NUMBERED_FILES} files named like {stem}_<n>.{extension} in {}",
        dir.display()
    )))
}

/// A folder name for `test_id`: letters, digits, `.`, `-` and `_` only, so it
/// cannot name a place outside `latest_logs`; `test` when nothing is left.
pub(crate) fn folder_name(test_id: &str) -> String {
    let cleaned: String = test_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let start = cleaned.len().saturating_sub(MAX_FOLDER_LEN);
    // Only ASCII is left, so any byte index is a character boundary.
    let name = cleaned[start..].trim_start_matches('.');
    if name.is_empty() {
        "test".to_owned()
    } else {
        name.to_owned()
    }
}

/// Whether `breakpoint()` should pause, given the value of
/// [`BREAKPOINTS_VARIABLE`]: it should unless that is `0`, `false`, `no` or
/// `off`.
fn enabled_by(value: Option<&str>) -> bool {
    !value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}

/// Whether `SB_BREAKPOINTS` leaves `breakpoint()` on.
pub(crate) fn breakpoints_enabled() -> bool {
    enabled_by(std::env::var(BREAKPOINTS_VARIABLE).ok().as_deref())
}

/// The prompt for a `breakpoint()` call, or `None` (after saying why in the
/// log) when it should not wait: the variable turns it off, or nobody is at
/// the terminal.
pub(crate) fn breakpoint_prompt() -> Option<DebugPrompt<TerminalInput, io::Stderr>> {
    if !breakpoints_enabled() {
        tracing::debug!("breakpoint skipped: {BREAKPOINTS_VARIABLE} turns breakpoints off");
        return None;
    }
    let prompt = DebugPrompt::from_terminal();
    if prompt.is_none() {
        tracing::warn!(
            "breakpoint skipped: standard input is not a terminal (run with --test-threads 1 and --nocapture in a terminal to pause)"
        );
    }
    prompt
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::Mutex;

    use super::*;

    /// A page whose answers are fixed and whose calls are recorded.
    struct FakePage {
        url: Result<String, String>,
        screenshot: Result<Vec<u8>, String>,
        calls: Mutex<Vec<String>>,
    }

    impl FakePage {
        fn new() -> Self {
            Self {
                url: Ok("https://example.com/login".to_owned()),
                screenshot: Ok(b"\x89PNG-bytes".to_vec()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn note(&self, call: &str) {
            self.calls.lock().unwrap().push(call.to_owned());
        }
    }

    fn browser_error(text: &str) -> SeleniumBaseError {
        SeleniumBaseError::test_lifecycle(text)
    }

    #[async_trait]
    impl PromptPage for FakePage {
        async fn url(&self) -> Result<String, SeleniumBaseError> {
            self.url.clone().map_err(|text| browser_error(&text))
        }

        async fn title(&self) -> Result<String, SeleniumBaseError> {
            Ok("Login".to_owned())
        }

        async fn screenshot_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
            self.note("screenshot");
            self.screenshot.clone().map_err(|text| browser_error(&text))
        }

        async fn page_source(&self) -> Result<String, SeleniumBaseError> {
            self.note("source");
            Ok("<html><body>hi</body></html>".to_owned())
        }

        async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError> {
            self.note(&format!("evaluate {expression}"));
            match expression {
                "1 + 1" => Ok(serde_json::json!(2)),
                "document.title" => Ok(serde_json::json!("Login")),
                other => Err(browser_error(&format!("ReferenceError: {other}"))),
            }
        }
    }

    /// A prompt that reads `script` and keeps its files in `logs`.
    fn prompt(script: &str, logs: &Path) -> DebugPrompt<ReaderInput<Cursor<Vec<u8>>>, Vec<u8>> {
        DebugPrompt::from_reader(Cursor::new(script.as_bytes().to_vec()), Vec::new())
            .with_logs_dir(logs)
    }

    fn written(prompt: &DebugPrompt<ReaderInput<Cursor<Vec<u8>>>, Vec<u8>>) -> String {
        String::from_utf8_lossy(prompt.output()).into_owned()
    }

    async fn run(
        script: &str,
        kind: PromptKind,
        page: &FakePage,
        logs: &Path,
    ) -> (PromptOutcome, String) {
        let mut prompt = prompt(script, logs);
        let outcome = prompt
            .pause(
                kind,
                "login.valid_login",
                "Paused at login page setup.",
                page,
            )
            .await
            .unwrap();
        (outcome, written(&prompt))
    }

    #[tokio::test]
    async fn the_banner_names_the_kind_the_test_and_the_page() {
        let logs = tempfile::tempdir().unwrap();

        let (outcome, text) =
            run("c\n", PromptKind::Breakpoint, &FakePage::new(), logs.path()).await;

        assert_eq!(outcome, PromptOutcome::Continue);
        assert!(
            text.contains("[BREAKPOINT]: Paused at login page setup."),
            "{text}"
        );
        assert!(
            text.contains("test:  login.valid_login   page: https://example.com/login  \"Login\""),
            "{text}"
        );
        assert!(
            text.contains("Perform your manual actions in the browser now."),
            "{text}"
        );
        assert!(text.contains(COMMANDS), "{text}");
        assert!(
            text.ends_with("Type 'c' and press [ENTER] to continue execution: "),
            "{text}"
        );
    }

    #[tokio::test]
    async fn every_kind_has_its_own_label_and_only_breakpoints_invite_manual_actions() {
        let logs = tempfile::tempdir().unwrap();
        let cases = [
            (PromptKind::Breakpoint, "[BREAKPOINT]", true),
            (
                PromptKind::RecorderBreakpoint,
                "[RECORDER BREAKPOINT]",
                true,
            ),
            (PromptKind::Trace, "[TRACE]", false),
            (PromptKind::PostMortem, "[POST MORTEM]", false),
            (PromptKind::FinalTrace, "[FINAL TRACE]", false),
        ];

        for (kind, label, invites) in cases {
            let page = FakePage::new();
            let (_, text) = run("c\n", kind, &page, logs.path()).await;

            assert!(text.contains(label), "{kind:?}: {text}");
            assert_eq!(
                text.contains("Perform your manual actions"),
                invites,
                "{kind:?}: {text}"
            );
        }
    }

    #[tokio::test]
    async fn a_banner_without_a_message_has_no_colon() {
        let logs = tempfile::tempdir().unwrap();
        let mut prompt = prompt("c\n", logs.path());

        prompt
            .pause(PromptKind::Trace, "t", "", &FakePage::new())
            .await
            .unwrap();

        let text = written(&prompt);
        assert!(text.starts_with("\n[TRACE]\n"), "{text}");
    }

    #[tokio::test]
    async fn c_and_continue_in_any_case_continue() {
        let logs = tempfile::tempdir().unwrap();

        for script in ["c\n", "continue\n", "C\n", "  Continue  \r\n"] {
            let (outcome, _) = run(
                script,
                PromptKind::Breakpoint,
                &FakePage::new(),
                logs.path(),
            )
            .await;

            assert_eq!(outcome, PromptOutcome::Continue, "{script:?}");
        }
    }

    #[tokio::test]
    async fn q_and_quit_stop_the_test() {
        let logs = tempfile::tempdir().unwrap();

        for script in ["q\n", "quit\n", "u\nq\nc\n"] {
            let (outcome, _) = run(
                script,
                PromptKind::Breakpoint,
                &FakePage::new(),
                logs.path(),
            )
            .await;

            assert_eq!(outcome, PromptOutcome::Quit, "{script:?}");
        }
    }

    #[tokio::test]
    async fn the_end_of_the_input_continues_instead_of_hanging() {
        let logs = tempfile::tempdir().unwrap();

        let (outcome, text) = run("", PromptKind::PostMortem, &FakePage::new(), logs.path()).await;

        assert_eq!(outcome, PromptOutcome::Continue);
        assert!(text.contains("input closed: continuing"), "{text}");
    }

    #[tokio::test]
    async fn s_saves_numbered_screenshots_in_the_test_folder() {
        let logs = tempfile::tempdir().unwrap();
        let page = FakePage::new();

        let (outcome, text) = run("s\ns\nc\n", PromptKind::Breakpoint, &page, logs.path()).await;

        assert_eq!(outcome, PromptOutcome::Continue);
        let folder = logs.path().join("login.valid_login");
        assert_eq!(
            std::fs::read(folder.join("breakpoint_1.png")).unwrap(),
            b"\x89PNG-bytes"
        );
        assert!(folder.join("breakpoint_2.png").exists());
        assert!(
            text.contains("breakpoint_1.png"),
            "the path is printed: {text}"
        );
        assert!(text.contains("breakpoint_2.png"), "{text}");
    }

    #[tokio::test]
    async fn src_saves_the_page_source() {
        let logs = tempfile::tempdir().unwrap();

        let (_, text) = run(
            "src\nc\n",
            PromptKind::Breakpoint,
            &FakePage::new(),
            logs.path(),
        )
        .await;

        let file = logs
            .path()
            .join("login.valid_login")
            .join("page_source_1.html");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "<html><body>hi</body></html>"
        );
        assert!(text.contains("page_source_1.html"), "{text}");
    }

    #[tokio::test]
    async fn a_saved_file_is_never_replaced() {
        let logs = tempfile::tempdir().unwrap();
        let folder = logs.path().join("login.valid_login");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("breakpoint_1.png"), b"earlier").unwrap();

        run(
            "s\nc\n",
            PromptKind::Breakpoint,
            &FakePage::new(),
            logs.path(),
        )
        .await;

        assert_eq!(
            std::fs::read(folder.join("breakpoint_1.png")).unwrap(),
            b"earlier"
        );
        assert!(folder.join("breakpoint_2.png").exists());
    }

    #[tokio::test]
    async fn u_shows_the_address_and_the_title() {
        let logs = tempfile::tempdir().unwrap();

        let (_, text) = run(
            "u\nc\n",
            PromptKind::Breakpoint,
            &FakePage::new(),
            logs.path(),
        )
        .await;

        // Once in the banner, once for the command.
        assert_eq!(
            text.matches("page: https://example.com/login  \"Login\"")
                .count(),
            2,
            "{text}"
        );
    }

    #[tokio::test]
    async fn js_prints_the_value_as_json_and_survives_an_error() {
        let logs = tempfile::tempdir().unwrap();
        let page = FakePage::new();

        let (outcome, text) = run(
            "js 1 + 1;\njs document.title\njs nope\nc\n",
            PromptKind::Breakpoint,
            &page,
            logs.path(),
        )
        .await;

        assert_eq!(
            outcome,
            PromptOutcome::Continue,
            "an error does not end the pause"
        );
        assert_eq!(
            page.calls(),
            ["evaluate 1 + 1", "evaluate document.title", "evaluate nope"]
        );
        assert!(text.contains("execution: 2\n"), "{text}");
        assert!(text.contains("> \"Login\"\n"), "{text}");
        assert!(
            text.contains("error: ") && text.contains("ReferenceError: nope"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn js_without_an_expression_says_how_to_use_it() {
        let logs = tempfile::tempdir().unwrap();
        let page = FakePage::new();

        let (_, text) = run("js\nc\n", PromptKind::Breakpoint, &page, logs.path()).await;

        assert!(text.contains("usage: js <expression>"), "{text}");
        assert!(page.calls().is_empty());
    }

    #[tokio::test]
    async fn h_lists_the_commands_and_an_unknown_one_is_refused() {
        let logs = tempfile::tempdir().unwrap();

        let (outcome, text) = run(
            "h\nfrobnicate\n\nc\n",
            PromptKind::Breakpoint,
            &FakePage::new(),
            logs.path(),
        )
        .await;

        assert_eq!(outcome, PromptOutcome::Continue);
        assert!(
            text.contains("save the viewport as breakpoint_<n>.png"),
            "{text}"
        );
        assert!(text.contains("unknown command 'frobnicate'"), "{text}");
    }

    #[tokio::test]
    async fn a_page_that_cannot_be_asked_does_not_stop_the_pause() {
        let logs = tempfile::tempdir().unwrap();
        let mut page = FakePage::new();
        page.url = Err("no such window".to_owned());
        page.screenshot = Err("no such window".to_owned());

        let (outcome, text) = run("s\nc\n", PromptKind::Breakpoint, &page, logs.path()).await;

        assert_eq!(outcome, PromptOutcome::Continue);
        assert!(text.contains("page: (unavailable: "), "{text}");
        assert!(text.contains("could not take a screenshot: "), "{text}");
        assert!(
            !logs.path().join("login.valid_login").exists(),
            "nothing was written"
        );
    }

    #[tokio::test]
    async fn continuing_from_a_recorder_breakpoint_saves_a_screenshot() {
        let logs = tempfile::tempdir().unwrap();
        let page = FakePage::new();

        run("c\n", PromptKind::RecorderBreakpoint, &page, logs.path()).await;

        assert_eq!(page.calls(), ["screenshot"]);
        assert!(logs
            .path()
            .join("login.valid_login")
            .join("breakpoint_1.png")
            .exists());
    }

    #[tokio::test]
    async fn quitting_or_another_kind_saves_nothing() {
        let logs = tempfile::tempdir().unwrap();
        let page = FakePage::new();

        run("q\n", PromptKind::RecorderBreakpoint, &page, logs.path()).await;
        run("c\n", PromptKind::Breakpoint, &page, logs.path()).await;

        assert!(page.calls().is_empty(), "{:?}", page.calls());
    }

    #[tokio::test]
    async fn a_test_id_cannot_name_a_folder_outside_the_logs() {
        let logs = tempfile::tempdir().unwrap();
        let page = FakePage::new();
        let mut prompt = prompt("s\nc\n", logs.path());

        prompt
            .pause(PromptKind::Breakpoint, "../../etc/passwd", "m", &page)
            .await
            .unwrap();

        let entries: Vec<_> = std::fs::read_dir(logs.path()).unwrap().collect();
        assert_eq!(entries.len(), 1, "one folder inside the logs");
        let name = entries[0].as_ref().unwrap().file_name();
        assert_eq!(name.to_str().unwrap(), "_.._etc_passwd");
    }

    #[test]
    fn folder_names_keep_dots_but_not_a_leading_one() {
        assert_eq!(folder_name("login.valid_login"), "login.valid_login");
        assert_eq!(folder_name("a::b c"), "a__b_c");
        assert_eq!(folder_name(".."), "test");
        assert_eq!(folder_name(""), "test");
        assert_eq!(folder_name(&"x".repeat(500)).len(), MAX_FOLDER_LEN);
    }

    #[test]
    fn breakpoints_are_on_unless_the_variable_says_off() {
        assert!(enabled_by(None));
        assert!(enabled_by(Some("1")));
        assert!(enabled_by(Some("")));
        for off in ["0", "false", "FALSE", "no", "off", " 0 "] {
            assert!(!enabled_by(Some(off)), "{off:?}");
        }
    }

    #[test]
    fn a_pipe_is_not_a_terminal() {
        // Under `cargo test` and nextest standard input is not a person.
        if !io::stdin().is_terminal() {
            assert!(DebugPrompt::from_terminal().is_none());
            assert!(breakpoint_prompt().is_none());
        }
    }

    #[test]
    fn a_line_loses_only_its_line_end() {
        assert_eq!(trim_line_end("c\n"), "c");
        assert_eq!(trim_line_end("c\r\n"), "c");
        assert_eq!(trim_line_end("  c "), "  c ");
    }

    #[test]
    fn commands_are_recognised() {
        assert_eq!(parse("c"), Command::Continue);
        assert_eq!(parse("QUIT"), Command::Quit);
        assert_eq!(parse("src"), Command::Source);
        assert_eq!(parse("source"), Command::Source);
        assert_eq!(parse("s"), Command::Screenshot);
        assert_eq!(parse("u"), Command::Url);
        assert_eq!(parse("js  1 + 1 "), Command::Js("1 + 1"));
        assert_eq!(parse("js"), Command::JsUsage);
        assert_eq!(parse("?"), Command::Help);
        assert_eq!(parse("   "), Command::Empty);
        assert_eq!(parse("go now"), Command::Unknown("go"));
    }

    /// Two prompts started together both finish, one turn after the other:
    /// the turn is released when a pause ends, so neither waits for ever.
    #[tokio::test]
    async fn prompts_take_turns() {
        let logs = tempfile::tempdir().unwrap();
        let first_page = FakePage::new();
        let second_page = FakePage::new();
        let mut first = prompt("c\n", logs.path());
        let mut second = prompt("c\n", logs.path());

        let (a, b) = tokio::join!(
            first.pause(PromptKind::Breakpoint, "one", "first", &first_page),
            second.pause(PromptKind::Breakpoint, "two", "second", &second_page),
        );

        assert_eq!(a.unwrap(), PromptOutcome::Continue);
        assert_eq!(b.unwrap(), PromptOutcome::Continue);
    }
}
