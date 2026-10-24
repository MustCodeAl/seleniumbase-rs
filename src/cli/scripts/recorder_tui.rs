//! `sbrs recorder` as a terminal app (feature `tui`): Python's Recorder
//! Desktop App without Tkinter.
//!
//! A form asks for what `sbrs mkrec` needs (the file, the start page, the
//! browser, the output format) and has three actions:
//!
//! * **Record** hands the terminal to `sbrs mkrec ...` (a child process) and
//!   takes it back when the recording is over, then shows the files it wrote.
//!   The child needs the terminal: `mkrec` waits for `c` and Enter, or for
//!   SHIFT+ESC in the browser.
//! * **Run** runs the recorded test: `cargo test --test NAME` for a file in
//!   `tests/`, `cargo run --example NAME` for one in `examples/`.
//! * **Replay** runs `sbrs replay recordings/NAME_rec.json` in a fresh
//!   browser, with nothing to compile.
//!
//! The form ([`RecorderForm`]) checks the answers and builds the command lines;
//! the state machine ([`RecorderApp`]) handles keys; [`Env`] starts the
//! processes. Only the last one touches the terminal, so the rest is tested
//! with plain values.

use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::sb_recorder::{validate_file_name, MkrecOptions};
use super::tui_kit::{self, clip, Input, Machine, TextBuffer};
use crate::common::shutdown::Shutdown;
use crate::utils::urls::normalize_page_url;

/// The file the form starts with (Python's `new_recording.py`).
const DEFAULT_FILE: &str = "new_recording.rs";

/// What Python says when the URL is not one.
const INVALID_URL: &str = "Enter a valid URL! (Eg. seleniumbase.io)";

/// What Python says when MS Edge is asked for with UC Mode.
const EDGE_WITH_UC: &str = "MS Edge cannot be combined with UC Mode \
    because it uses msedgedriver, not chromedriver!";

/// The screen size assumed until the terminal says otherwise.
const DEFAULT_SIZE: (u16, u16) = (80, 24);

/// The rows of the form, one for each [`Field`].
const FORM_ROWS: u16 = 9;

/// Rows of the screen that are not the output: the header (3), the form with
/// its borders, the borders of the output (2) and the status and hint rows (2).
const OUTPUT_CHROME: u16 = 3 + FORM_ROWS + 2 + 2 + 2;

/// The width of the marker in front of a row of the form (`> `).
const MARK_WIDTH: usize = 2;

/// The width of the label of a row that has one.
const LABEL_WIDTH: usize = 16;

/// The most lines of output kept.
const OUTPUT_LIMIT: usize = 1000;

/// What the output shows before anything was done.
const HELP: [&str; 6] = [
    "Record gives this terminal to `sbrs mkrec`. Use the browser, then type c",
    "and press Enter here, or press SHIFT+ESC in the browser (--ee box ticked).",
    "The recording goes to recordings/NAME_rec.rs and .json and to the file.",
    "Run starts `cargo test --test NAME` for a file in tests/ (or an example).",
    "Replay runs recordings/NAME_rec.json in a fresh browser, no compiling.",
    "Ctrl-R records, Ctrl-T runs, Ctrl-P replays, from any box.",
];

/// Takes the item after (or before) `current` in `all`, going round.
fn cycle<T: Copy + PartialEq, const N: usize>(all: [T; N], current: T, forward: bool) -> T {
    let at = all.iter().position(|&item| item == current).unwrap_or(0);
    all[if forward {
        (at + 1) % N
    } else {
        (at + N - 1) % N
    }]
}

/// A text box that starts with `text` and has the cursor after it.
fn typed(text: &str) -> TextBuffer {
    let mut buffer = TextBuffer::from_text(text);
    buffer.end();
    buffer
}

/// The Chromium-based browsers `mkrec` can record in, in the order of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserChoice {
    Chrome,
    Chromium,
    Edge,
    Opera,
    Brave,
    Comet,
}

impl BrowserChoice {
    const ALL: [Self; 6] = [
        Self::Chrome,
        Self::Chromium,
        Self::Edge,
        Self::Opera,
        Self::Brave,
        Self::Comet,
    ];

    /// The browser a `--browser` word names, if the recorder can use it.
    fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|browser| browser.name().eq_ignore_ascii_case(name))
    }

    /// The word of `--browser` and `SB_BROWSER`.
    const fn name(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::Edge => "edge",
            Self::Opera => "opera",
            Self::Brave => "brave",
            Self::Comet => "comet",
        }
    }

    /// The name in the menu, as Python's app words it.
    const fn label(self) -> &'static str {
        match self {
            Self::Chrome => "Google Chrome",
            Self::Chromium => "Chromium Browser",
            Self::Edge => "MS Edge (No Stealth)",
            Self::Opera => "Opera Browser",
            Self::Brave => "Brave Browser",
            Self::Comet => "Comet Browser",
        }
    }
}

/// What the recording is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    /// A test for `cargo test` (the default).
    Test,
    /// A script that opens the browser itself (`--rec-sb-mgr`).
    Script,
    /// Pure CDP code (`--rec-sb-cdp`).
    Cdp,
    /// A test and a Gherkin feature (`--behave`).
    Gherkin,
}

impl OutputFormat {
    const ALL: [Self; 4] = [Self::Test, Self::Script, Self::Cdp, Self::Gherkin];

    const fn label(self) -> &'static str {
        match self {
            Self::Test => "Test (rstest / BaseCase)",
            Self::Script => "Script / SB() (--rec-sb-mgr)",
            Self::Cdp => "Pure CDP Mode / sb_cdp",
            Self::Gherkin => "Test + Behave Gherkin file",
        }
    }
}

/// What `sbrs recorder` was started with: the options of the command line
/// that the form shows. Python's app reads `--uc`, `--behave`, `--ee` and the
/// like from its command line in the same way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Start {
    /// The start page (`--url`).
    pub url: Option<String>,
    /// The browser (`--browser`), when it is one the recorder can use.
    pub browser: Option<String>,
    /// Stealth Mode (`--uc`, or `--cdp` as Python takes it).
    pub uc: bool,
    /// End the recording with SHIFT+ESC (`--ee`).
    pub esc_end: bool,
    /// Also write a Gherkin feature (`--rec-behave`).
    pub behave: bool,
    /// Write a script (`--rec-sb-mgr`).
    pub sb_mgr: bool,
    /// Write pure CDP code (`--rec-sb-cdp`).
    pub sb_cdp: bool,
}

impl Start {
    /// The form these options fill in. As in Python, a script wins over pure
    /// CDP code and that over the feature, and Edge has no Stealth Mode.
    pub(crate) fn form(&self) -> RecorderForm {
        let mut form = RecorderForm::default();
        if let Some(url) = &self.url {
            form.url = typed(url);
        }
        if let Some(browser) = self.browser.as_deref().and_then(BrowserChoice::from_name) {
            form.browser = browser;
        }
        form.uc = self.uc && form.browser != BrowserChoice::Edge;
        form.esc_end = self.esc_end;
        form.format = if self.sb_mgr {
            OutputFormat::Script
        } else if self.sb_cdp {
            OutputFormat::Cdp
        } else if self.behave {
            OutputFormat::Gherkin
        } else {
            OutputFormat::Test
        };
        form
    }
}

/// A program to start, with what it is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Job {
    program: Program,
    args: Vec<String>,
    env: Vec<(&'static str, &'static str)>,
}

/// The program of a [`Job`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Program {
    /// This very program, `sbrs`.
    ThisProgram,
    Cargo,
}

impl Job {
    fn sbrs(args: Vec<String>) -> Self {
        Self {
            program: Program::ThisProgram,
            args,
            env: Vec::new(),
        }
    }

    fn cargo(args: &[&str]) -> Self {
        Self {
            program: Program::Cargo,
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            env: Vec::new(),
        }
    }

    /// The command as a shell would take it, for showing and for copying.
    fn line(&self) -> String {
        let mut words: Vec<String> = self
            .env
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        words.push(
            match self.program {
                Program::ThisProgram => "sbrs",
                Program::Cargo => "cargo",
            }
            .to_owned(),
        );
        words.extend(self.args.iter().map(|arg| quote(arg)));
        words.join(" ")
    }
}

/// `word` as a shell takes it: as it is when it holds nothing a shell reads,
/// else in single quotes.
fn quote(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// The folders and the file of a relative path, `.` left out.
fn normal_parts(file: &str) -> Vec<&str> {
    Path::new(file)
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect()
}

/// The answers of the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecorderForm {
    file: TextBuffer,
    url: TextBuffer,
    overwrite: bool,
    browser: BrowserChoice,
    uc: bool,
    format: OutputFormat,
    esc_end: bool,
    demo: bool,
}

impl Default for RecorderForm {
    /// Python's start: `new_recording.py` that may be overwritten, Chrome, no
    /// Stealth Mode, a test.
    fn default() -> Self {
        Self {
            file: typed(DEFAULT_FILE),
            url: typed(""),
            overwrite: true,
            browser: BrowserChoice::Chrome,
            uc: false,
            format: OutputFormat::Test,
            esc_end: false,
            demo: false,
        }
    }
}

impl RecorderForm {
    /// The file name asked for, if `mkrec` takes it.
    fn checked_file(&self) -> Result<String, String> {
        let file = self.file.to_text().trim().to_owned();
        validate_file_name(&file).map_err(|message| format!("Invalid filename: {message}"))?;
        Ok(file)
    }

    /// The start page with `https://` where it was left out.
    fn checked_url(&self) -> Result<String, String> {
        normalize_page_url(&self.url.to_text()).map_err(|_| INVALID_URL.to_owned())
    }

    fn check_browser(&self) -> Result<(), String> {
        if self.browser == BrowserChoice::Edge && self.uc {
            return Err(EDGE_WITH_UC.to_owned());
        }
        Ok(())
    }

    /// What Record starts: `sbrs mkrec FILE --url=URL ...`, run from `dir`.
    ///
    /// # Errors
    ///
    /// Returns what to tell the person when an answer is wrong, or when the
    /// file exists and may not be overwritten.
    pub(crate) fn record(&self, dir: &Path) -> Result<Effect, String> {
        let file = self.checked_file()?;
        let url = self.checked_url()?;
        self.check_browser()?;
        if !self.overwrite && dir.join(&file).exists() {
            return Err(format!(
                "\"{file}\" already exists; tick \"Overwrite\" to replace it."
            ));
        }
        let mut args = vec![
            "mkrec".to_owned(),
            file.clone(),
            format!("--url={url}"),
            "--headed".to_owned(),
            format!("--browser={}", self.browser.name()),
        ];
        let flags = [
            (self.uc, "--uc"),
            (self.esc_end, "--ee"),
            (self.format == OutputFormat::Script, "--rec-sb-mgr"),
            (self.format == OutputFormat::Cdp, "--rec-sb-cdp"),
            (self.overwrite, "--overwrite"),
            (self.format == OutputFormat::Gherkin, "--behave"),
        ];
        args.extend(
            flags
                .into_iter()
                .filter(|(on, _)| *on)
                .map(|(_, flag)| flag.to_owned()),
        );
        Ok(Effect::Record {
            job: Job::sbrs(args),
            stem: MkrecOptions::new(file.clone()).stem(),
            behave: self.format == OutputFormat::Gherkin,
            file,
        })
    }

    /// What Run starts: the recorded test, by the name of its file in `dir`.
    ///
    /// # Errors
    ///
    /// Returns what to tell the person when the file is not there, or is not
    /// a test or an example that cargo can run by name.
    pub(crate) fn run(&self, dir: &Path) -> Result<Effect, String> {
        let file = self.checked_file()?;
        self.check_browser()?;
        if !dir.join(&file).is_file() {
            return Err(format!("\"{file}\" does not exist yet; record it first."));
        }
        let stem = MkrecOptions::new(file.clone()).stem();
        let mut job = match normal_parts(&file).as_slice() {
            ["tests", _] => Job::cargo(&["test", "--test", &stem]),
            ["examples", _] => Job::cargo(&["run", "--example", &stem]),
            _ => {
                return Err(format!(
                    "Run needs a file in tests/ or in examples/, not \"{file}\"; Replay works for any recording."
                ))
            }
        };
        job.env.push(("SB_BROWSER", self.browser.name()));
        if self.uc {
            job.env.push(("SB_MODE", "uc"));
        }
        if self.demo {
            job.env.push(("SB_DEMO", "true"));
        }
        Ok(Effect::Run { job })
    }

    /// What Replay starts: `sbrs replay recordings/NAME_rec.json`, with the
    /// recording named after the file, looked for in `dir`.
    ///
    /// # Errors
    ///
    /// Returns what to tell the person when there is no recording to replay.
    pub(crate) fn replay(&self, dir: &Path) -> Result<Effect, String> {
        let file = self.checked_file()?;
        self.check_browser()?;
        let json = format!("recordings/{}_rec.json", MkrecOptions::new(file).stem());
        if !dir.join(&json).is_file() {
            return Err(format!("\"{json}\" does not exist yet; record first."));
        }
        let mut args = vec![
            "replay".to_owned(),
            json,
            "--headed".to_owned(),
            format!("--browser={}", self.browser.name()),
        ];
        if self.uc {
            args.push("--uc".to_owned());
        }
        if self.demo {
            args.push("--demo".to_owned());
        }
        Ok(Effect::Replay {
            job: Job::sbrs(args),
        })
    }
}

/// The rows of the form, in the order Tab visits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    File,
    Url,
    Overwrite,
    Browser,
    Uc,
    Format,
    EscEnd,
    Demo,
    Buttons,
}

impl Field {
    const ALL: [Self; FORM_ROWS as usize] = [
        Self::File,
        Self::Url,
        Self::Overwrite,
        Self::Browser,
        Self::Uc,
        Self::Format,
        Self::EscEnd,
        Self::Demo,
        Self::Buttons,
    ];
}

/// What a button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Record,
    Run,
    Replay,
}

impl Action {
    const ALL: [Self; 3] = [Self::Record, Self::Run, Self::Replay];

    const fn label(self) -> &'static str {
        match self {
            Self::Record => "Record",
            Self::Run => "Run",
            Self::Replay => "Replay",
        }
    }

    const fn color(self) -> Color {
        match self {
            Self::Record => Color::Red,
            Self::Run => Color::Green,
            Self::Replay => Color::Cyan,
        }
    }
}

/// How serious the message in the status row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Info,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Status {
    level: Level,
    text: String,
}

/// What the output pane shows.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Output {
    title: String,
    lines: Vec<String>,
    /// How many lines the view is scrolled back from the end.
    back: usize,
}

impl Output {
    fn help() -> Self {
        Self {
            title: "Output".to_owned(),
            lines: HELP.iter().map(|line| (*line).to_owned()).collect(),
            back: 0,
        }
    }

    /// The `rows` lines to show: the last ones, or those `back` lines earlier.
    fn visible(&self, rows: usize) -> &[String] {
        let back = self.back.min(self.lines.len().saturating_sub(rows));
        let end = self.lines.len() - back;
        &self.lines[end.saturating_sub(rows)..end]
    }
}

/// What happens to the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Msg {
    Key(Input),
    /// An action is over.
    Finished(Finished),
}

impl From<Input> for Msg {
    fn from(input: Input) -> Self {
        Self::Key(input)
    }
}

/// How an action ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finished {
    action: Action,
    ok: bool,
    /// The line for the status row.
    summary: String,
    /// What the output pane shows.
    lines: Vec<String>,
}

/// What the app asks the outside world to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Effect {
    /// Record: hand the terminal to `mkrec`, then report the files `file`
    /// and `stem` stand for.
    Record {
        job: Job,
        file: String,
        stem: String,
        behave: bool,
    },
    /// Run the recorded test and capture what it prints.
    Run { job: Job },
    /// Replay the recording and capture what it prints.
    Replay { job: Job },
}

/// The Recorder app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecorderApp {
    form: RecorderForm,
    focus: Field,
    /// The button the Enter key presses when the buttons have the focus.
    button: Action,
    /// The folder the files are in: the current one.
    dir: PathBuf,
    output: Output,
    status: Option<Status>,
    size: (u16, u16),
    done: bool,
}

/// Edits a one-line text box.
fn edit(text: &mut TextBuffer, input: Input) {
    match input {
        Input::Char(c) => {
            text.insert(c);
        }
        Input::Backspace => {
            text.backspace();
        }
        Input::Delete => {
            text.delete();
        }
        Input::Left => text.left(),
        Input::Right => text.right(),
        Input::Home => text.home(),
        Input::End => text.end(),
        _ => {}
    }
}

impl RecorderApp {
    /// An app showing `form`, whose files are in `dir`.
    pub(crate) fn new(form: RecorderForm, dir: PathBuf) -> Self {
        Self {
            form,
            focus: Field::File,
            button: Action::Record,
            dir,
            output: Output::help(),
            status: None,
            size: DEFAULT_SIZE,
            done: false,
        }
    }

    fn output_rows(&self) -> usize {
        usize::from(self.size.1.saturating_sub(OUTPUT_CHROME))
    }

    fn say(&mut self, level: Level, text: impl Into<String>) {
        self.status = Some(Status {
            level,
            text: text.into(),
        });
    }

    /// Scrolls the output a page back (`older`) or forward.
    fn scroll(&mut self, older: bool) {
        let rows = self.output_rows();
        let page = rows.saturating_sub(1).max(1);
        let most = self.output.lines.len().saturating_sub(rows);
        self.output.back = if older {
            self.output.back.saturating_add(page).min(most)
        } else {
            self.output.back.saturating_sub(page)
        };
    }

    /// Starts `action`, or says what is wrong with the form.
    fn start(&mut self, action: Action) -> Vec<Effect> {
        let made = match action {
            Action::Record => self.form.record(&self.dir),
            Action::Run => self.form.run(&self.dir),
            Action::Replay => self.form.replay(&self.dir),
        };
        match made {
            Ok(effect) => {
                let (Effect::Record { job, .. } | Effect::Run { job } | Effect::Replay { job }) =
                    &effect;
                let verb = match action {
                    Action::Record => "Recording with",
                    Action::Run => "Running",
                    Action::Replay => "Replaying with",
                };
                let text = format!("{verb}: {}", job.line());
                self.say(Level::Info, text);
                vec![effect]
            }
            Err(message) => {
                self.say(Level::Error, message);
                Vec::new()
            }
        }
    }

    fn on_key(&mut self, input: Input) -> Vec<Effect> {
        match input {
            // In raw mode Ctrl-C is a key, not a signal.
            Input::Ctrl('c') | Input::Esc => {
                self.done = true;
                return Vec::new();
            }
            Input::Ctrl('r') => return self.start(Action::Record),
            Input::Ctrl('t') => return self.start(Action::Run),
            Input::Ctrl('p') => return self.start(Action::Replay),
            Input::Tab | Input::Down => {
                self.focus = cycle(Field::ALL, self.focus, true);
                return Vec::new();
            }
            Input::Up => {
                self.focus = cycle(Field::ALL, self.focus, false);
                return Vec::new();
            }
            Input::PageUp => {
                self.scroll(true);
                return Vec::new();
            }
            Input::PageDown => {
                self.scroll(false);
                return Vec::new();
            }
            _ => {}
        }
        match self.focus {
            Field::File => {
                if input == Input::Enter {
                    self.focus = Field::Url;
                } else {
                    edit(&mut self.form.file, input);
                }
            }
            // Return in the URL box starts the recording, as in Python's app.
            Field::Url => {
                if input == Input::Enter {
                    return self.start(Action::Record);
                }
                edit(&mut self.form.url, input);
            }
            Field::Overwrite => toggle(&mut self.form.overwrite, input),
            Field::Uc => toggle(&mut self.form.uc, input),
            Field::EscEnd => toggle(&mut self.form.esc_end, input),
            Field::Demo => toggle(&mut self.form.demo, input),
            Field::Browser => match input {
                Input::Char(' ') | Input::Right | Input::Enter => {
                    self.form.browser = cycle(BrowserChoice::ALL, self.form.browser, true);
                }
                Input::Left => {
                    self.form.browser = cycle(BrowserChoice::ALL, self.form.browser, false);
                }
                _ => {}
            },
            Field::Format => match input {
                Input::Char(' ') | Input::Right | Input::Enter => {
                    self.form.format = cycle(OutputFormat::ALL, self.form.format, true);
                }
                Input::Left => {
                    self.form.format = cycle(OutputFormat::ALL, self.form.format, false);
                }
                _ => {}
            },
            Field::Buttons => match input {
                Input::Right => self.button = cycle(Action::ALL, self.button, true),
                Input::Left => self.button = cycle(Action::ALL, self.button, false),
                Input::Enter | Input::Char(' ') => return self.start(self.button),
                _ => {}
            },
        }
        Vec::new()
    }
}

/// Space or Enter ticks or unticks a box.
fn toggle(value: &mut bool, input: Input) {
    if matches!(input, Input::Char(' ') | Input::Enter) {
        *value = !*value;
    }
}

impl Machine for RecorderApp {
    type Msg = Msg;
    type Effect = Effect;

    fn update(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Key(input) => return self.on_key(input),
            Msg::Finished(finished) => {
                let level = if finished.ok {
                    Level::Info
                } else {
                    Level::Error
                };
                self.output = Output {
                    title: format!("Output: {}", finished.action.label()),
                    lines: finished.lines,
                    back: 0,
                };
                self.say(level, finished.summary);
            }
        }
        Vec::new()
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        let most = self.output.lines.len().saturating_sub(self.output_rows());
        self.output.back = self.output.back.min(most);
    }

    fn view(&self, frame: &mut Frame) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(FORM_ROWS + 2),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
        draw_header(frame, chunks[0]);
        self.draw_form(frame, chunks[1]);
        self.draw_output(frame, chunks[2]);
        if let Some(status) = &self.status {
            let color = match status.level {
                Level::Info => Color::Green,
                Level::Error => Color::Red,
            };
            frame.render_widget(
                Paragraph::new(clip(&status.text, usize::from(chunks[3].width)))
                    .style(Style::default().fg(color)),
                chunks[3],
            );
        }
        let hints = match self.focus {
            Field::File | Field::Url => {
                "Esc quit  Tab next  Enter next/record  ^R record  ^T run  ^P replay  PgUp/PgDn"
            }
            Field::Buttons => {
                "Esc quit  Tab next  \u{2190}\u{2192} choose  Enter go  ^R record  ^T run  ^P replay"
            }
            _ => "Esc quit  Tab next  Space change  ^R record  ^T run  ^P replay  PgUp/PgDn output",
        };
        frame.render_widget(
            Paragraph::new(clip(hints, usize::from(chunks[4].width)))
                .style(Style::default().fg(Color::Gray)),
            chunks[4],
        );
    }

    fn finished(&self) -> bool {
        self.done
    }
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(Text::styled(
        "SeleniumBase Recorder",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Recorder"));
    frame.render_widget(header, area);
}

/// The part of `text` that shows in a box `width` characters wide with the
/// cursor at character `col`, and the cursor's place in that part.
fn window(text: &str, col: usize, width: usize) -> (String, usize) {
    let width = width.max(1);
    let start = (col + 1).saturating_sub(width);
    (text.chars().skip(start).take(width).collect(), col - start)
}

impl RecorderApp {
    fn draw_form(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Record a test");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let value_width = usize::from(inner.width).saturating_sub(MARK_WIDTH + LABEL_WIDTH);
        let mut lines = Vec::new();
        let mut cursor = None;
        for (row, field) in Field::ALL.into_iter().enumerate() {
            let focused = field == self.focus;
            let marker = if focused { "> " } else { "  " };
            let on = |value: bool| if value { "[x]" } else { "[ ]" };
            let line = match field {
                Field::File | Field::Url => {
                    let (label, text) = if field == Field::File {
                        ("File name:", &self.form.file)
                    } else {
                        ("Start URL:", &self.form.url)
                    };
                    let (shown, at) = window(&text.to_text(), text.col(), value_width);
                    if focused {
                        cursor = Some((row, at));
                    }
                    let mut spans = vec![Span::raw(format!("{marker}{label:<LABEL_WIDTH$}"))];
                    if shown.is_empty() && !focused {
                        spans.push(Span::styled(
                            "e.g. seleniumbase.io/demo_page",
                            Style::default().fg(Color::DarkGray),
                        ));
                    } else {
                        spans.push(Span::raw(shown));
                    }
                    Line::from(spans)
                }
                Field::Overwrite => Line::from(format!(
                    "{marker}{} Overwrite the file if it exists",
                    on(self.form.overwrite)
                )),
                Field::Browser => Line::from(format!(
                    "{marker}{:<LABEL_WIDTH$}< {} >",
                    "Browser:",
                    self.form.browser.label()
                )),
                Field::Uc => Line::from(format!(
                    "{marker}{} Stealth Mode / UC + CDP Mode",
                    on(self.form.uc)
                )),
                Field::Format => Line::from(format!(
                    "{marker}{:<LABEL_WIDTH$}< {} >",
                    "Output format:",
                    self.form.format.label()
                )),
                Field::EscEnd => Line::from(format!(
                    "{marker}{} End with SHIFT+ESC in the browser (--ee)",
                    on(self.form.esc_end)
                )),
                Field::Demo => Line::from(format!(
                    "{marker}{} Demo Mode for Run and Replay",
                    on(self.form.demo)
                )),
                Field::Buttons => {
                    let mut spans = vec![Span::raw(marker)];
                    for action in Action::ALL {
                        let style = if focused && action == self.button {
                            Style::default()
                                .bg(action.color())
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(action.color())
                        };
                        spans.push(Span::styled(format!("[ {} ]", action.label()), style));
                        spans.push(Span::raw("  "));
                    }
                    Line::from(spans)
                }
            };
            lines.push(if focused && field != Field::Buttons {
                line.style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                line
            });
        }
        frame.render_widget(Paragraph::new(lines), inner);
        if let Some((row, at)) = cursor {
            let x = inner.x + u16::try_from(MARK_WIDTH + LABEL_WIDTH + at).unwrap_or(u16::MAX);
            let y = inner.y + u16::try_from(row).unwrap_or(u16::MAX);
            frame.set_cursor_position(Position::new(
                x.min(inner.right().saturating_sub(1)),
                y.min(inner.bottom().saturating_sub(1)),
            ));
        }
    }

    fn draw_output(&self, frame: &mut Frame, area: Rect) {
        let mut title = self.output.title.clone();
        if self.output.back > 0 {
            title.push_str(" (scrolled back; PgDn goes forward)");
        }
        let block = Block::default().borders(Borders::ALL).title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let width = usize::from(inner.width);
        let lines: Vec<Line> = self
            .output
            .visible(usize::from(inner.height))
            .iter()
            .map(|line| Line::from(clip(line, width)))
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// `line` without the escape sequences of colours and cursor moves, and
/// without other control characters.
fn strip_ansi(line: &str) -> String {
    let mut plain = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.next() == Some('[') {
                for end in chars.by_ref() {
                    if ('@'..='~').contains(&end) {
                        break;
                    }
                }
            }
        } else if c == '\t' {
            plain.push_str("    ");
        } else if !c.is_control() {
            plain.push(c);
        }
    }
    plain
}

/// The lines of what a program printed, as text to show: of a line redrawn
/// with carriage returns only the last drawing is kept.
fn plain_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| strip_ansi(line.rsplit('\r').next().unwrap_or(line)))
        .collect()
}

/// The status line for a captured run: its result line, else how it ended.
fn summarize(action: Action, ok: bool, ended: &str, lines: &[String]) -> String {
    let marker = match action {
        Action::Run => "test result:",
        Action::Replay => "Replayed ",
        Action::Record => "",
    };
    let detail = lines
        .iter()
        .rev()
        .find(|line| !marker.is_empty() && line.trim_start().starts_with(marker))
        .map_or_else(|| ended.to_owned(), |line| line.trim().to_owned());
    format!(
        "{} {}: {detail}",
        action.label(),
        if ok { "passed" } else { "failed" }
    )
}

/// The files `mkrec` wrote for `file` (and the feature, when asked for),
/// found in `dir`, and the test itself with line numbers.
fn recording_report(dir: &Path, file: &str, stem: &str, behave: bool) -> Vec<String> {
    let mut candidates = vec![
        file.to_owned(),
        format!("recordings/{stem}_rec.rs"),
        format!("recordings/{stem}_rec.json"),
    ];
    if behave {
        candidates.push(format!("recordings/features/{stem}_rec.feature"));
    }
    let made: Vec<String> = candidates
        .into_iter()
        .filter(|path| dir.join(path).is_file())
        .collect();
    if made.is_empty() {
        return vec!["Nothing was recorded.".to_owned()];
    }
    let mut lines = vec!["Files:".to_owned()];
    lines.extend(made.iter().map(|path| format!("  {path}")));
    if let Ok(text) = std::fs::read_to_string(dir.join(file)) {
        lines.push(String::new());
        lines.push(format!("{file}:"));
        lines.extend(
            text.lines()
                .enumerate()
                .map(|(number, line)| format!("{:>4}  {line}", number + 1)),
        );
    }
    lines.truncate(OUTPUT_LIMIT);
    lines
}

/// The process `job` describes.
fn command(job: &Job) -> io::Result<Command> {
    let program = match job.program {
        Program::ThisProgram => std::env::current_exe()?,
        Program::Cargo => PathBuf::from("cargo"),
    };
    let mut command = Command::new(program);
    command.args(&job.args);
    for (name, value) in &job.env {
        command.env(name, value);
    }
    Ok(command)
}

/// What the effects of the app work on.
#[derive(Default)]
pub(crate) struct Env {
    /// Listens for Ctrl-C and SIGTERM once a recording was started. On the
    /// ordinary terminal Ctrl-C goes to this process as well as to `mkrec`,
    /// which saves the recording on it; this process must not die before
    /// `mkrec` has said so.
    signals: Option<Shutdown>,
}

impl Env {
    /// Carries out an effect and says how it ended.
    pub(crate) fn perform(&mut self, effect: Effect) -> Option<Msg> {
        Some(Msg::Finished(match effect {
            Effect::Record {
                job,
                file,
                stem,
                behave,
            } => {
                if self.signals.is_none() {
                    self.signals = Shutdown::install().ok();
                }
                record(&job, &file, &stem, behave)
            }
            Effect::Run { job } => capture(Action::Run, &job),
            Effect::Replay { job } => capture(Action::Replay, &job),
        }))
    }
}

/// Hands the terminal to `mkrec` and reports what it wrote.
fn record(job: &Job, file: &str, stem: &str, behave: bool) -> Finished {
    let ran = tui_kit::suspend(|| {
        println!("$ {}\n", job.line());
        let _ = io::stdout().flush();
        let status = command(job).and_then(|mut command| command.status());
        match &status {
            Ok(status) => println!("\n[sbrs recorder] mkrec ended: {status}"),
            Err(error) => println!("\n[sbrs recorder] mkrec could not start: {error}"),
        }
        println!("Press Enter to go back to the recorder.");
        let _ = io::stdout().flush();
        let _ = io::stdin().lock().read_line(&mut String::new());
        status
    });
    let finished = |ok: bool, summary: String, lines: Vec<String>| Finished {
        action: Action::Record,
        ok,
        summary,
        lines,
    };
    match ran.and_then(|status| status) {
        Ok(status) if status.success() => finished(
            true,
            format!("Recorded {file}"),
            recording_report(Path::new("."), file, stem, behave),
        ),
        Ok(status) => finished(
            false,
            format!("Record failed: mkrec ended with {status}"),
            vec![
                "Its messages were printed on the terminal before this screen came back."
                    .to_owned(),
                "To see them again, run:".to_owned(),
                format!("  {}", job.line()),
            ],
        ),
        Err(error) => finished(
            false,
            format!("Record failed: {error}"),
            vec![format!("  {}", job.line())],
        ),
    }
}

/// Runs `job` with its output captured, and reports what it printed.
fn capture(action: Action, job: &Job) -> Finished {
    let output = command(job).and_then(|mut command| {
        command
            .stdin(Stdio::null())
            .env("NO_COLOR", "1")
            .env("CARGO_TERM_COLOR", "never")
            .output()
    });
    match output {
        Err(error) => Finished {
            action,
            ok: false,
            summary: format!(
                "{} failed: cannot start {}: {error}",
                action.label(),
                job.line()
            ),
            lines: Vec::new(),
        },
        Ok(output) => {
            // The results of `cargo test` come last, on the standard output;
            // the messages of `sbrs` that explain a failure, on the error.
            let (first, second) = if action == Action::Run {
                (&output.stderr, &output.stdout)
            } else {
                (&output.stdout, &output.stderr)
            };
            let mut lines = plain_lines(first);
            lines.extend(plain_lines(second));
            let skip = lines.len().saturating_sub(OUTPUT_LIMIT);
            lines.drain(..skip);
            let ok = output.status.success();
            Finished {
                action,
                ok,
                summary: summarize(action, ok, &ended(&output.status), &lines),
                lines,
            }
        }
    }
}

fn ended(status: &ExitStatus) -> String {
    status.to_string()
}

/// Runs the Recorder app until the user quits.
///
/// # Errors
///
/// Returns the error of the terminal.
pub fn run(start: &Start) -> io::Result<()> {
    let mut app = RecorderApp::new(start.form(), PathBuf::from("."));
    let mut env = Env::default();
    tui_kit::run(&mut app, |effect| env.perform(effect))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::scripts::tui_kit::{render, render_cursor};

    fn app() -> RecorderApp {
        let mut app = RecorderApp::new(RecorderForm::default(), PathBuf::from("."));
        app.resize(80, 24);
        app
    }

    fn press(app: &mut RecorderApp, input: Input) -> Vec<Effect> {
        app.update(Msg::Key(input))
    }

    fn type_text(app: &mut RecorderApp, text: &str) {
        for c in text.chars() {
            press(app, Input::Char(c));
        }
    }

    /// The form with a URL, as the person would have typed it.
    fn form_with(url: &str) -> RecorderForm {
        RecorderForm {
            url: typed(url),
            ..RecorderForm::default()
        }
    }

    fn job(effect: Effect) -> Job {
        match effect {
            Effect::Record { job, .. } | Effect::Run { job } | Effect::Replay { job } => job,
        }
    }

    fn args(effect: Effect) -> Vec<String> {
        job(effect).args
    }

    fn status(app: &RecorderApp) -> (Level, &str) {
        let status = app.status.as_ref().expect("a status message");
        (status.level, status.text.as_str())
    }

    /// An empty project folder.
    fn project() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temp dir")
    }

    fn write(dir: &Path, path: &str) {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
        std::fs::write(path, "// test\n").expect("a file");
    }

    // ------------------------------------------------------------------
    // The command lines
    // ------------------------------------------------------------------

    #[test]
    fn the_form_starts_as_pythons_app_does() {
        let form = RecorderForm::default();

        assert_eq!(form.file.to_text(), "new_recording.rs");
        assert_eq!(form.url.to_text(), "");
        assert!(form.overwrite, "Python ticks Overwrite to begin with");
        assert_eq!(form.browser, BrowserChoice::Chrome);
        assert_eq!((form.uc, form.esc_end, form.demo), (false, false, false));
        assert_eq!(form.format, OutputFormat::Test);
    }

    #[test]
    fn record_is_sbrs_mkrec_with_the_file_the_page_and_the_browser() {
        let dir = project();

        let effect = form_with("seleniumbase.io/demo_page")
            .record(dir.path())
            .expect("a good form");

        assert_eq!(
            args(effect),
            [
                "mkrec",
                "new_recording.rs",
                "--url=https://seleniumbase.io/demo_page",
                "--headed",
                "--browser=chrome",
                "--overwrite",
            ]
        );
    }

    #[test]
    fn each_option_adds_its_own_flag() {
        let dir = project();
        let record = |change: &dyn Fn(&mut RecorderForm)| {
            let mut form = form_with("example.com");
            form.overwrite = false;
            change(&mut form);
            args(form.record(dir.path()).expect("a good form"))
        };
        let extra = |all: Vec<String>| all[5..].to_vec();

        assert!(extra(record(&|_| {})).is_empty(), "nothing else by default");
        assert_eq!(extra(record(&|f| f.overwrite = true)), ["--overwrite"]);
        assert_eq!(extra(record(&|f| f.uc = true)), ["--uc"]);
        assert_eq!(extra(record(&|f| f.esc_end = true)), ["--ee"]);
        assert_eq!(
            extra(record(&|f| f.format = OutputFormat::Script)),
            ["--rec-sb-mgr"]
        );
        assert_eq!(
            extra(record(&|f| f.format = OutputFormat::Cdp)),
            ["--rec-sb-cdp"]
        );
        assert_eq!(
            extra(record(&|f| f.format = OutputFormat::Gherkin)),
            ["--behave"]
        );
        assert_eq!(
            extra(record(&|f| {
                f.uc = true;
                f.esc_end = true;
                f.overwrite = true;
                f.format = OutputFormat::Gherkin;
            })),
            ["--uc", "--ee", "--overwrite", "--behave"]
        );
        for browser in BrowserChoice::ALL {
            let all = record(&|f| f.browser = browser);
            assert_eq!(all[4], format!("--browser={}", browser.name()));
        }
    }

    #[test]
    fn the_url_is_one_argument_and_shows_quoted_in_the_command_line() {
        let dir = project();
        let url = "https://example.com/a b?x=1&y='2'";

        let effect = form_with(url).record(dir.path()).expect("a good form");
        let job = job(effect);

        assert_eq!(job.args[2], format!("--url={url}"), "one word, as it is");
        assert!(
            job.line()
                .contains(r"'--url=https://example.com/a b?x=1&y='\''2'\'''"),
            "{}",
            job.line()
        );
        assert_eq!(quote("plain-word_1.rs"), "plain-word_1.rs");
        assert_eq!(quote(""), "''");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn a_page_that_is_not_one_is_refused_with_pythons_words() {
        let dir = project();

        for url in ["", "   ", "/tmp/page.html", "ftp://example.com"] {
            assert_eq!(
                form_with(url).record(dir.path()).unwrap_err(),
                INVALID_URL,
                "{url:?}"
            );
        }
        assert!(form_with("about:blank").record(dir.path()).is_ok());
    }

    #[test]
    fn a_file_name_mkrec_would_refuse_is_refused_first() {
        let dir = project();
        for (name, message) in [
            ("new_test.py", "File name must end with \".rs\"!"),
            ("../x.rs", "File must be created in the current directory!"),
            ("-x.rs", "File name cannot start with \"-\"!"),
        ] {
            let mut form = form_with("example.com");
            form.file = typed(name);

            assert_eq!(
                form.record(dir.path()).unwrap_err(),
                format!("Invalid filename: {message}"),
                "{name}"
            );
        }
        let mut form = form_with("example.com");
        form.file = typed("  tests/login.rs  ");
        assert_eq!(args(form.record(dir.path()).unwrap())[1], "tests/login.rs");
    }

    #[test]
    fn an_existing_file_needs_overwrite() {
        let dir = project();
        write(dir.path(), "new_recording.rs");
        let mut form = form_with("example.com");

        assert!(form.record(dir.path()).is_ok(), "overwrite is ticked");
        form.overwrite = false;
        let message = form.record(dir.path()).unwrap_err();
        assert!(message.contains("already exists"), "{message}");
        assert!(message.contains("Overwrite"), "{message}");
    }

    #[test]
    fn edge_has_no_stealth_mode() {
        let dir = project();
        let mut form = form_with("example.com");
        form.browser = BrowserChoice::Edge;
        form.uc = true;

        assert_eq!(form.record(dir.path()).unwrap_err(), EDGE_WITH_UC);
        form.uc = false;
        assert!(form.record(dir.path()).is_ok());

        let started = Start {
            browser: Some("edge".to_owned()),
            uc: true,
            ..Start::default()
        };
        assert!(!started.form().uc, "as in Python, Edge switches it off");
    }

    #[test]
    fn run_starts_the_test_cargo_knows_by_its_file() {
        let dir = project();
        write(dir.path(), "tests/login.rs");
        write(dir.path(), "examples/tour.rs");
        write(dir.path(), "tests/deep/more.rs");
        write(dir.path(), "new_recording.rs");
        let run = |file: &str, change: &dyn Fn(&mut RecorderForm)| {
            let mut form = RecorderForm::default();
            form.file = typed(file);
            change(&mut form);
            form.run(dir.path())
        };

        let test = job(run("tests/login.rs", &|_| {}).unwrap());
        assert_eq!(test.args, ["test", "--test", "login"]);
        assert_eq!(test.program, Program::Cargo);
        assert_eq!(test.env, [("SB_BROWSER", "chrome")]);
        assert_eq!(test.line(), "SB_BROWSER=chrome cargo test --test login");

        let example = job(run("examples/tour.rs", &|_| {}).unwrap());
        assert_eq!(example.args, ["run", "--example", "tour"]);

        let all = job(run("./tests/login.rs", &|f| {
            f.uc = true;
            f.demo = true;
            f.browser = BrowserChoice::Brave;
        })
        .unwrap());
        assert_eq!(
            all.env,
            [
                ("SB_BROWSER", "brave"),
                ("SB_MODE", "uc"),
                ("SB_DEMO", "true")
            ]
        );

        for file in ["tests/deep/more.rs", "new_recording.rs"] {
            let message = run(file, &|_| {}).unwrap_err();
            assert!(message.contains("tests/ or in examples/"), "{message}");
        }
        let message = run("tests/missing.rs", &|_| {}).unwrap_err();
        assert!(message.contains("does not exist yet"), "{message}");
    }

    #[test]
    fn replay_starts_the_recording_named_after_the_file() {
        let dir = project();
        let mut form = RecorderForm::default();
        form.file = typed("tests/login.rs");

        let message = form.replay(dir.path()).unwrap_err();
        assert!(message.contains("recordings/login_rec.json"), "{message}");
        assert!(message.contains("record first"), "{message}");

        write(dir.path(), "recordings/login_rec.json");
        let plain = form.replay(dir.path()).unwrap();
        assert_eq!(
            job(plain.clone()).line(),
            "sbrs replay recordings/login_rec.json --headed --browser=chrome"
        );
        assert_eq!(job(plain).program, Program::ThisProgram);

        form.uc = true;
        form.demo = true;
        assert_eq!(
            args(form.replay(dir.path()).unwrap()),
            [
                "replay",
                "recordings/login_rec.json",
                "--headed",
                "--browser=chrome",
                "--uc",
                "--demo"
            ]
        );
    }

    #[test]
    fn the_command_line_options_fill_in_the_form() {
        let form = Start {
            url: Some("seleniumbase.io".to_owned()),
            browser: Some("brave".to_owned()),
            uc: true,
            esc_end: true,
            behave: true,
            ..Start::default()
        }
        .form();

        assert_eq!(form.url.to_text(), "seleniumbase.io");
        assert_eq!(form.browser, BrowserChoice::Brave);
        assert!(form.uc && form.esc_end);
        assert_eq!(form.format, OutputFormat::Gherkin);
        assert_eq!(form.url.col(), 15, "the cursor is after the text");

        let python_order = |behave, sb_mgr, sb_cdp| {
            Start {
                behave,
                sb_mgr,
                sb_cdp,
                ..Start::default()
            }
            .form()
            .format
        };
        assert_eq!(python_order(true, false, false), OutputFormat::Gherkin);
        assert_eq!(python_order(true, true, true), OutputFormat::Script);
        assert_eq!(python_order(true, false, true), OutputFormat::Cdp);
        assert_eq!(python_order(false, false, false), OutputFormat::Test);
        assert_eq!(Start::default().form(), RecorderForm::default());
        let unknown = Start {
            browser: Some("firefox".to_owned()),
            ..Start::default()
        };
        assert_eq!(unknown.form().browser, BrowserChoice::Chrome);
    }

    // ------------------------------------------------------------------
    // The keys
    // ------------------------------------------------------------------

    #[test]
    fn typing_goes_into_the_box_with_the_focus() {
        let mut app = app();

        for _ in 0..3 {
            press(&mut app, Input::Backspace);
        }
        type_text(&mut app, "q.rs");
        assert_eq!(app.form.file.to_text(), "new_recordingq.rs");
    }

    #[test]
    fn tab_and_the_arrows_go_round_the_rows() {
        let mut app = app();
        assert_eq!(app.focus, Field::File);

        press(&mut app, Input::Tab);
        assert_eq!(app.focus, Field::Url);
        press(&mut app, Input::Down);
        assert_eq!(app.focus, Field::Overwrite);
        press(&mut app, Input::Up);
        press(&mut app, Input::Up);
        press(&mut app, Input::Up);
        assert_eq!(app.focus, Field::Buttons, "up from the first row wraps");
        press(&mut app, Input::Tab);
        assert_eq!(app.focus, Field::File, "and so does Tab from the last");
    }

    #[test]
    fn letters_are_text_in_a_box_so_q_does_not_quit() {
        let mut app = app();

        type_text(&mut app, "qQ");

        assert!(!app.finished());
        assert!(app.form.file.to_text().ends_with("qQ"));
    }

    #[test]
    fn escape_and_ctrl_c_quit_from_every_row() {
        for quit in [Input::Esc, Input::Ctrl('c')] {
            for field in Field::ALL {
                let mut app = app();
                app.focus = field;

                assert!(press(&mut app, quit).is_empty());

                assert!(app.finished(), "{quit:?} on {field:?}");
            }
        }
    }

    #[test]
    fn space_ticks_boxes_and_the_arrows_choose_from_a_menu() {
        let mut app = app();

        app.focus = Field::Overwrite;
        press(&mut app, Input::Char(' '));
        assert!(!app.form.overwrite);
        press(&mut app, Input::Enter);
        assert!(app.form.overwrite);

        app.focus = Field::Uc;
        press(&mut app, Input::Char(' '));
        app.focus = Field::EscEnd;
        press(&mut app, Input::Char(' '));
        app.focus = Field::Demo;
        press(&mut app, Input::Char(' '));
        assert!(app.form.uc && app.form.esc_end && app.form.demo);

        app.focus = Field::Browser;
        press(&mut app, Input::Right);
        assert_eq!(app.form.browser, BrowserChoice::Chromium);
        press(&mut app, Input::Left);
        press(&mut app, Input::Left);
        assert_eq!(
            app.form.browser,
            BrowserChoice::Comet,
            "the menu goes round"
        );
        press(&mut app, Input::Char(' '));
        assert_eq!(app.form.browser, BrowserChoice::Chrome);

        app.focus = Field::Format;
        press(&mut app, Input::Right);
        assert_eq!(app.form.format, OutputFormat::Script);
        press(&mut app, Input::Left);
        press(&mut app, Input::Left);
        assert_eq!(app.form.format, OutputFormat::Gherkin);
    }

    #[test]
    fn enter_in_the_file_box_moves_on_and_in_the_url_box_records() {
        let mut app = app();

        assert!(press(&mut app, Input::Enter).is_empty());
        assert_eq!(app.focus, Field::Url);

        let none = press(&mut app, Input::Enter);
        assert!(none.is_empty(), "no URL yet");
        assert_eq!(status(&app), (Level::Error, INVALID_URL));

        type_text(&mut app, "example.com");
        let effects = press(&mut app, Input::Enter);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::Record { .. }));
    }

    #[test]
    fn the_control_keys_start_the_actions_from_any_row() {
        let dir = project();
        write(dir.path(), "tests/login.rs");
        write(dir.path(), "recordings/login_rec.json");
        let mut form = form_with("example.com");
        form.file = typed("tests/login.rs");
        let mut app = RecorderApp::new(form, dir.path().to_path_buf());
        app.focus = Field::Demo;

        let record = press(&mut app, Input::Ctrl('r'));
        assert!(matches!(record[..], [Effect::Record { .. }]));
        assert_eq!(status(&app).0, Level::Info);
        assert!(status(&app)
            .1
            .starts_with("Recording with: sbrs mkrec tests/login.rs"));
        let run = press(&mut app, Input::Ctrl('t'));
        assert!(matches!(run[..], [Effect::Run { .. }]));
        assert_eq!(
            status(&app).1,
            "Running: SB_BROWSER=chrome cargo test --test login"
        );
        let replay = press(&mut app, Input::Ctrl('p'));
        assert!(matches!(replay[..], [Effect::Replay { .. }]));
        assert!(status(&app)
            .1
            .starts_with("Replaying with: sbrs replay recordings/login_rec.json"));
    }

    #[test]
    fn a_wrong_form_starts_nothing_and_says_why() {
        let mut app = app();

        let effects = press(&mut app, Input::Ctrl('r'));

        assert!(effects.is_empty());
        assert_eq!(status(&app), (Level::Error, INVALID_URL));
        assert!(press(&mut app, Input::Ctrl('t')).is_empty());
        assert!(
            status(&app).1.contains("does not exist yet"),
            "{:?}",
            status(&app)
        );
    }

    #[test]
    fn the_buttons_are_chosen_with_the_arrows_and_pressed_with_enter() {
        let dir = project();
        write(dir.path(), "tests/login.rs");
        let mut form = form_with("example.com");
        form.file = typed("tests/login.rs");
        let mut app = RecorderApp::new(form, dir.path().to_path_buf());
        app.focus = Field::Buttons;

        press(&mut app, Input::Right);
        let effects = press(&mut app, Input::Enter);
        assert!(
            matches!(effects[..], [Effect::Run { .. }]),
            "Run is next to Record"
        );
        press(&mut app, Input::Left);
        press(&mut app, Input::Left);
        let effects = press(&mut app, Input::Char(' '));
        assert!(matches!(effects[..], [Effect::Replay { .. }]) || effects.is_empty());
        assert_eq!(app.button, Action::Replay, "the choice goes round");
    }

    // ------------------------------------------------------------------
    // What the actions report
    // ------------------------------------------------------------------

    fn finished(action: Action, ok: bool, lines: &[&str]) -> Msg {
        Msg::Finished(Finished {
            action,
            ok,
            summary: format!("{} done", action.label()),
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        })
    }

    #[test]
    fn a_finished_action_replaces_the_output_and_sets_the_status() {
        let mut app = app();

        app.update(finished(Action::Run, false, &["one", "two"]));

        assert_eq!(app.output.title, "Output: Run");
        assert_eq!(app.output.lines, ["one", "two"]);
        assert_eq!(status(&app), (Level::Error, "Run done"));
        app.update(finished(Action::Replay, true, &[]));
        assert_eq!(status(&app), (Level::Info, "Replay done"));
    }

    #[test]
    fn the_output_shows_its_end_and_pages_back_and_forward() {
        let mut app = app();
        let lines: Vec<String> = (1..=20).map(|n| format!("line {n}")).collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        app.update(finished(Action::Run, true, &refs));
        let rows = app.output_rows();
        assert_eq!(rows, 6, "an 80x24 screen leaves six rows");

        assert_eq!(app.output.visible(rows).last().unwrap(), "line 20");
        assert_eq!(app.output.visible(rows).len(), 6);
        press(&mut app, Input::PageUp);
        assert_eq!(app.output.visible(rows).last().unwrap(), "line 15");
        press(&mut app, Input::PageUp);
        press(&mut app, Input::PageUp);
        press(&mut app, Input::PageUp);
        assert_eq!(
            app.output.visible(rows).first().unwrap(),
            "line 1",
            "stops at the start"
        );
        press(&mut app, Input::PageDown);
        assert_eq!(app.output.visible(rows).first().unwrap(), "line 6");
        for _ in 0..5 {
            press(&mut app, Input::PageDown);
        }
        assert_eq!(app.output.back, 0);
        assert_eq!(
            Output::help().visible(100).len(),
            HELP.len(),
            "short output is all shown"
        );
    }

    #[test]
    fn captured_output_is_plain_text() {
        assert_eq!(strip_ansi("\u{1b}[1;32mok\u{1b}[0m done\u{7}"), "ok done");
        assert_eq!(strip_ansi("a\tb"), "a    b");
        assert_eq!(
            plain_lines(b"one\r\ntwo \r50%\r100%\nlast"),
            ["one", "100%", "last"],
            "a redrawn line keeps its last drawing"
        );
        assert_eq!(plain_lines(b"caf\xc3\xa9 \xff"), ["caf\u{e9} \u{fffd}"]);
    }

    #[test]
    fn the_status_names_the_result_line_or_how_the_process_ended() {
        let lines: Vec<String> = ["running 1 test", "test result: ok. 1 passed; 0 failed", ""]
            .map(String::from)
            .to_vec();

        assert_eq!(
            summarize(Action::Run, true, "exit status: 0", &lines),
            "Run passed: test result: ok. 1 passed; 0 failed"
        );
        assert_eq!(
            summarize(
                Action::Run,
                false,
                "exit status: 101",
                &["error[E0432]".to_owned()]
            ),
            "Run failed: exit status: 101"
        );
        assert_eq!(
            summarize(
                Action::Replay,
                false,
                "exit status: 1",
                &[
                    "FAIL  click".to_owned(),
                    "Replayed 2 step(s): 1 passed, 1 failed, 0 skipped".to_owned()
                ]
            ),
            "Replay failed: Replayed 2 step(s): 1 passed, 1 failed, 0 skipped"
        );
    }

    #[test]
    fn the_report_lists_the_files_that_exist_and_shows_the_test() {
        let dir = project();
        assert_eq!(
            recording_report(dir.path(), "tests/login.rs", "login", false),
            ["Nothing was recorded."]
        );
        write(dir.path(), "tests/login.rs");
        write(dir.path(), "recordings/login_rec.rs");
        write(dir.path(), "recordings/login_rec.json");
        write(dir.path(), "recordings/features/login_rec.feature");

        let plain = recording_report(dir.path(), "tests/login.rs", "login", false);
        assert_eq!(
            plain,
            [
                "Files:",
                "  tests/login.rs",
                "  recordings/login_rec.rs",
                "  recordings/login_rec.json",
                "",
                "tests/login.rs:",
                "   1  // test",
            ]
        );
        let behave = recording_report(dir.path(), "tests/login.rs", "login", true);
        assert!(behave.contains(&"  recordings/features/login_rec.feature".to_owned()));
    }

    // ------------------------------------------------------------------
    // The screen
    // ------------------------------------------------------------------

    #[test]
    fn the_screen_shows_every_row_of_the_form_and_the_help() {
        let screen = render(&app(), 80, 24);

        for text in [
            "SeleniumBase Recorder",
            "> File name:      new_recording.rs",
            "  Start URL:      e.g. seleniumbase.io/demo_page",
            "[x] Overwrite the file if it exists",
            "Browser:        < Google Chrome >",
            "[ ] Stealth Mode / UC + CDP Mode",
            "Output format:  < Test (rstest / BaseCase) >",
            "[ ] End with SHIFT+ESC in the browser (--ee)",
            "[ ] Demo Mode for Run and Replay",
            "[ Record ]  [ Run ]  [ Replay ]",
            "Record gives this terminal to `sbrs mkrec`. Use the browser, then type c",
            "Esc quit  Tab next",
            "^P replay  PgUp/PgDn",
        ] {
            assert!(screen.contains(text), "{text}\n{screen}");
        }
        assert_eq!(screen.lines().count(), 24);
    }

    #[test]
    fn the_cursor_is_in_the_box_with_the_focus_and_hidden_elsewhere() {
        let mut app = app();

        // Inside the form's border (1), after the marker and the label.
        assert_eq!(
            render_cursor(&app, 80, 24),
            Some((u16::try_from(1 + 2 + 16 + 16).unwrap(), 4))
        );
        press(&mut app, Input::Tab);
        type_text(&mut app, "abc");
        assert_eq!(render_cursor(&app, 80, 24), Some((1 + 2 + 16 + 3, 5)));
        press(&mut app, Input::Tab);
        assert_eq!(render_cursor(&app, 80, 24), None);
    }

    #[test]
    fn a_long_text_scrolls_so_the_cursor_stays_in_the_box() {
        assert_eq!(window("abcdef", 6, 4), ("def".to_owned(), 3));
        assert_eq!(window("abcdef", 2, 4), ("abcd".to_owned(), 2));
        assert_eq!(window("", 0, 0), (String::new(), 0));
    }

    #[test]
    fn the_status_and_the_output_are_drawn() {
        let mut app = app();
        press(&mut app, Input::Ctrl('r'));
        app.update(finished(Action::Run, false, &["test x ... FAILED"]));

        let screen = render(&app, 80, 24);

        assert!(screen.contains("Run done"), "{screen}");
        assert!(screen.contains("Output: Run"), "{screen}");
        assert!(screen.contains("test x ... FAILED"), "{screen}");
        assert!(!screen.contains("Record gives this terminal"), "{screen}");
    }

    #[test]
    fn the_form_rows_match_the_height_the_layout_gives_them() {
        assert_eq!(Field::ALL.len(), usize::from(FORM_ROWS));
        // The help fits an 80 column screen, inside the border of the output.
        for line in HELP {
            assert!(line.chars().count() <= 78, "too long: {line}");
        }
        // So does the hint row, whichever row has the focus.
        for field in Field::ALL {
            let mut app = app();
            app.focus = field;
            let screen = render(&app, 80, 24);
            let hints = screen.lines().last().expect("a last row");
            assert!(
                hints.starts_with("Esc quit  Tab next"),
                "{field:?}: {hints}"
            );
            assert!(hints.contains("^R record"), "{field:?}: {hints}");
            assert!(hints.contains("^P replay"), "{field:?}: {hints}");
        }
        // A tiny screen must not panic.
        let _ = render(&app(), 20, 8);
        let _ = render(&app(), 1, 1);
    }
}
