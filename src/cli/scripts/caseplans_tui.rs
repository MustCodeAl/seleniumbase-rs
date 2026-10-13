//! `sbrs caseplans` as a terminal app (feature `tui`): the Python Case Plans
//! Generator without Tkinter.
//!
//! It lists the tests found under a folder, marks which already have a case
//! plan, creates boilerplate plans for the marked ones, opens a plan in a small
//! editor and saves it, and writes a summary of all plans to `case_summary.md`.
//! What a plan is and where it lives is in [`super::case_plans`]; this module is
//! the state machine ([`CasePlans`]) and its drawing.

use std::io;
use std::path::Path;

use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{
    Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;

use super::case_plans;
use super::tui_kit::{centered_size, clip, follow, Input, Machine, TextBuffer};

/// The screen size assumed until the terminal says otherwise.
const DEFAULT_SIZE: (u16, u16) = (80, 24);

/// Rows of the screen that are not the list: the header (3), the borders of
/// the list (2), and the filter, status and hint rows (3).
const LIST_CHROME: u16 = 8;

/// Rows of the screen that are not the editor's text: the header (3), the
/// borders (2), and the status and hint rows (2).
const EDITOR_CHROME: u16 = 7;

/// One test in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    id: String,
    /// Whether a case plan exists for it.
    plan: bool,
    marked: bool,
}

impl Entry {
    fn row(&self) -> String {
        format!(
            "{} {} {}",
            if self.marked { "[x]" } else { "[ ]" },
            if self.plan { "[plan]" } else { "[    ]" },
            self.id
        )
    }
}

/// How serious the message in the status row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Status {
    level: Level,
    text: String,
}

/// What to do once the plan being edited has been saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AfterSave {
    /// Keep editing.
    Nothing,
    /// Go back to the list.
    Close,
    /// Leave the app.
    Quit,
}

/// A plan open in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Editor {
    id: String,
    text: TextBuffer,
    top: usize,
    dirty: bool,
    /// Set while the app asks what to do with unsaved changes: `Close` or
    /// `Quit`, whichever the key that raised the question was for.
    asking: Option<AfterSave>,
    /// Set from the moment the save is asked for until the answer comes.
    saving: Option<AfterSave>,
}

/// What a key did to the editor that the app has to act on.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EditorOutcome {
    Stay,
    Save(Effect),
    Close,
    Quit,
}

impl Editor {
    /// The text as saved: LF line ends and a newline at the end.
    fn text_to_save(&self) -> String {
        let mut text = self.text.to_text();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text
    }

    fn save(&mut self, after: AfterSave) -> EditorOutcome {
        self.asking = None;
        self.saving = Some(after);
        EditorOutcome::Save(Effect::Save {
            id: self.id.clone(),
            text: self.text_to_save(),
        })
    }

    /// Leaves the editor, or asks first when there is something to lose.
    fn leave(&mut self, then: AfterSave) -> EditorOutcome {
        if self.dirty {
            self.asking = Some(then);
            EditorOutcome::Stay
        } else if then == AfterSave::Quit {
            EditorOutcome::Quit
        } else {
            EditorOutcome::Close
        }
    }

    /// Applies a key; `rows` is how many lines of text fit on the screen.
    fn key(&mut self, input: Input, rows: usize) -> EditorOutcome {
        if let Some(after) = self.asking {
            return match input {
                Input::Char('s' | 'S') => self.save(after),
                Input::Char('d' | 'D') if after == AfterSave::Quit => EditorOutcome::Quit,
                Input::Char('d' | 'D') => EditorOutcome::Close,
                Input::Esc => {
                    self.asking = None;
                    EditorOutcome::Stay
                }
                _ => EditorOutcome::Stay,
            };
        }
        let page = isize::try_from(rows.saturating_sub(1).max(1)).unwrap_or(1);
        match input {
            Input::Ctrl('c') => return self.leave(AfterSave::Quit),
            Input::Esc => return self.leave(AfterSave::Close),
            Input::Ctrl('s') => return self.save(AfterSave::Nothing),
            Input::Char(c) => self.dirty |= self.text.insert(c),
            Input::Enter => {
                self.text.newline();
                self.dirty = true;
            }
            Input::Backspace => self.dirty |= self.text.backspace(),
            Input::Delete => self.dirty |= self.text.delete(),
            Input::Left => self.text.left(),
            Input::Right => self.text.right(),
            Input::Up => self.text.vertical(-1),
            Input::Down => self.text.vertical(1),
            Input::PageUp => self.text.vertical(-page),
            Input::PageDown => self.text.vertical(page),
            Input::Home => self.text.home(),
            Input::End => self.text.end(),
            Input::Tab | Input::Ctrl(_) => {}
        }
        self.top = follow(self.top, self.text.row(), rows);
        EditorOutcome::Stay
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    List,
    Editor(Box<Editor>),
}

/// What happens to the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Msg {
    Key(Input),
    /// Boilerplate plans were written for these tests; `kept` others had one.
    Created {
        ids: Vec<String>,
        kept: usize,
    },
    /// The plan of a test was read.
    Opened {
        id: String,
        text: String,
    },
    /// The plan of a test was saved to `path`.
    Saved {
        id: String,
        path: String,
    },
    /// The summary was written to `path`.
    SummaryWritten {
        path: String,
        customized: usize,
        boilerplate: usize,
        unfinished: usize,
    },
    /// There is no plan to summarise.
    NoPlans,
    /// The tests were listed again.
    Scanned(Vec<(String, bool)>),
    /// Something failed; the text says what.
    Failed(String),
}

impl From<Input> for Msg {
    fn from(input: Input) -> Self {
        Self::Key(input)
    }
}

/// What the app asks the outside world to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Effect {
    /// Write boilerplate plans for these tests.
    Create(Vec<String>),
    /// Read the plan of this test.
    Open(String),
    /// Save `text` as the plan of this test.
    Save { id: String, text: String },
    /// Write `case_summary.md`.
    Summarize,
    /// List the tests again.
    Rescan,
}

/// The Case Plans Generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CasePlans {
    entries: Vec<Entry>,
    /// Where the tests were looked for, for the empty screen.
    looked_in: String,
    /// The row of the visible list the cursor is on.
    cursor: usize,
    top: usize,
    filter: String,
    filtering: bool,
    screen: Screen,
    status: Option<Status>,
    size: (u16, u16),
    done: bool,
}

impl CasePlans {
    /// An app listing `tests`, pairs of a test id and whether it has a plan.
    /// `looked_in` says where they were looked for.
    pub(crate) fn new(tests: Vec<(String, bool)>, looked_in: &str) -> Self {
        Self {
            entries: entries_of(tests, &[]),
            looked_in: looked_in.to_owned(),
            cursor: 0,
            top: 0,
            filter: String::new(),
            filtering: false,
            screen: Screen::List,
            status: None,
            size: DEFAULT_SIZE,
            done: false,
        }
    }

    /// The indexes of the entries the filter lets through.
    fn visible(&self) -> Vec<usize> {
        let query = self.filter.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| query.is_empty() || entry.id.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    fn list_rows(&self) -> usize {
        usize::from(self.size.1.saturating_sub(LIST_CHROME))
    }

    fn editor_rows(&self) -> usize {
        usize::from(self.size.1.saturating_sub(EDITOR_CHROME))
    }

    fn say(&mut self, level: Level, text: impl Into<String>) {
        self.status = Some(Status {
            level,
            text: text.into(),
        });
    }

    /// Keeps the cursor inside the visible list and the window on the cursor.
    fn settle_cursor(&mut self) {
        let count = self.visible().len();
        self.cursor = self.cursor.min(count.saturating_sub(1));
        self.top = follow(self.top, self.cursor, self.list_rows());
    }

    /// Puts the cursor on row `row` of the shown list, or on its last row.
    fn go_to(&mut self, row: usize) {
        let count = self.visible().len();
        if count == 0 {
            return;
        }
        self.cursor = row.min(count - 1);
        self.top = follow(self.top, self.cursor, self.list_rows());
    }

    /// Moves one row, going round at either end.
    fn step(&mut self, forward: bool) {
        let count = self.visible().len();
        if count == 0 {
            return;
        }
        self.cursor = if forward {
            (self.cursor + 1) % count
        } else {
            (self.cursor + count - 1) % count
        };
        self.top = follow(self.top, self.cursor, self.list_rows());
    }

    fn cursor_entry(&self) -> Option<&Entry> {
        self.visible()
            .get(self.cursor)
            .and_then(|&index| self.entries.get(index))
    }

    fn on_key(&mut self, input: Input) -> Vec<Effect> {
        if matches!(self.screen, Screen::Editor(_)) {
            return self.editor_key(input);
        }
        // In raw mode Ctrl-C is a key, not a signal; in the filter box it
        // would otherwise type a `c`.
        if input == Input::Ctrl('c') {
            self.done = true;
            return Vec::new();
        }
        if self.filtering {
            self.filter_key(input);
            return Vec::new();
        }
        self.list_key(input)
    }

    fn list_key(&mut self, input: Input) -> Vec<Effect> {
        let page = self.list_rows().saturating_sub(1).max(1);
        match input {
            Input::Char('q' | 'Q') => self.done = true,
            Input::Down | Input::Char('j') => self.step(true),
            Input::Up | Input::Char('k') => self.step(false),
            Input::PageDown => self.go_to(self.cursor.saturating_add(page)),
            Input::PageUp => self.go_to(self.cursor.saturating_sub(page)),
            Input::Home => self.go_to(0),
            Input::End => self.go_to(usize::MAX),
            Input::Char(' ') => {
                if let Some(&index) = self.visible().get(self.cursor) {
                    self.entries[index].marked ^= true;
                }
            }
            Input::Char('a') => {
                for index in self.visible() {
                    self.entries[index].marked = true;
                }
            }
            Input::Char('n') => {
                for entry in &mut self.entries {
                    entry.marked = false;
                }
            }
            Input::Char('c') => return self.create(),
            Input::Enter | Input::Char('e') => return self.open(),
            Input::Char('s') => return self.summarize(),
            Input::Char('/' | 'f') => self.filtering = true,
            Input::Char('r') => return vec![Effect::Rescan],
            _ => {}
        }
        Vec::new()
    }

    /// Asks for boilerplate plans for the marked tests, or the one under the
    /// cursor when none is marked, leaving out those that have a plan.
    fn create(&mut self) -> Vec<Effect> {
        let mut chosen: Vec<&Entry> = self.entries.iter().filter(|entry| entry.marked).collect();
        if chosen.is_empty() {
            chosen.extend(self.cursor_entry());
        }
        let nothing_listed = chosen.is_empty();
        let missing: Vec<String> = chosen
            .iter()
            .filter(|entry| !entry.plan)
            .map(|entry| entry.id.clone())
            .collect();
        if nothing_listed {
            return Vec::new();
        }
        if missing.is_empty() {
            self.say(
                Level::Warning,
                "No new Case Plans were generated: the selected tests already have Case Plans",
            );
            return Vec::new();
        }
        vec![Effect::Create(missing)]
    }

    /// Asks for the plan of the row under the cursor to be opened.
    fn open(&mut self) -> Vec<Effect> {
        match self.cursor_entry() {
            Some(entry) if entry.plan => vec![Effect::Open(entry.id.clone())],
            Some(entry) => {
                let text = format!("{} has no Case Plan yet: press c to create one", entry.id);
                self.say(Level::Warning, text);
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    fn summarize(&mut self) -> Vec<Effect> {
        if self.entries.iter().any(|entry| entry.plan) {
            vec![Effect::Summarize]
        } else {
            self.say(Level::Warning, NO_PLANS);
            Vec::new()
        }
    }

    fn filter_key(&mut self, input: Input) {
        match input {
            Input::Enter => self.filtering = false,
            Input::Esc => {
                self.filtering = false;
                self.filter.clear();
            }
            Input::Backspace => {
                self.filter.pop();
            }
            Input::Char(c) => self.filter.push(c),
            _ => {}
        }
        self.settle_cursor();
    }

    fn editor_key(&mut self, input: Input) -> Vec<Effect> {
        let rows = self.editor_rows();
        let Screen::Editor(editor) = &mut self.screen else {
            return Vec::new();
        };
        match editor.key(input, rows) {
            EditorOutcome::Stay => Vec::new(),
            EditorOutcome::Save(effect) => vec![effect],
            EditorOutcome::Close => {
                self.screen = Screen::List;
                Vec::new()
            }
            EditorOutcome::Quit => {
                self.done = true;
                Vec::new()
            }
        }
    }

    /// A plan was saved: it is no longer modified, and what was to follow the
    /// save (closing, quitting) happens now.
    fn saved(&mut self, id: &str, path: &str) {
        for entry in &mut self.entries {
            if entry.id == id {
                entry.plan = true;
            }
        }
        self.say(Level::Info, format!("Saved {path}"));
        let after = match &mut self.screen {
            Screen::Editor(editor) if editor.id == id => {
                editor.dirty = false;
                editor.saving.take()
            }
            _ => None,
        };
        match after {
            Some(AfterSave::Close) => self.screen = Screen::List,
            Some(AfterSave::Quit) => self.done = true,
            Some(AfterSave::Nothing) | None => {}
        }
    }
}

/// What the status row says when there is no plan to summarise.
const NO_PLANS: &str = "No existing Case Plans found! Create some boilerplates first!";

/// The entries for `tests`, with the marks `earlier` entries had.
fn entries_of(tests: Vec<(String, bool)>, earlier: &[Entry]) -> Vec<Entry> {
    tests
        .into_iter()
        .map(|(id, plan)| {
            let marked = earlier.iter().any(|entry| entry.marked && entry.id == id);
            Entry { id, plan, marked }
        })
        .collect()
}

impl Machine for CasePlans {
    type Msg = Msg;
    type Effect = Effect;

    fn update(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Key(input) => return self.on_key(input),
            Msg::Created { ids, kept } => {
                for entry in &mut self.entries {
                    if ids.contains(&entry.id) {
                        entry.plan = true;
                    }
                }
                let text = match ids.len() {
                    0 => format!("No Case Plans were generated: {kept} already existed"),
                    1 => "1 new boilerplate Case Plan was generated".to_owned(),
                    count => format!("{count} new boilerplate Case Plans were generated"),
                };
                self.say(Level::Info, text);
            }
            Msg::Opened { id, text } => {
                self.screen = Screen::Editor(Box::new(Editor {
                    id,
                    text: TextBuffer::from_text(&text),
                    top: 0,
                    dirty: false,
                    asking: None,
                    saving: None,
                }));
                self.status = None;
            }
            Msg::Saved { id, path } => self.saved(&id, &path),
            Msg::SummaryWritten {
                path,
                customized,
                boilerplate,
                unfinished,
            } => self.say(
                Level::Info,
                format!(
                    "Summary written to {path}: {customized} customized, \
                     {boilerplate} boilerplate, {unfinished} unfinished"
                ),
            ),
            Msg::NoPlans => self.say(Level::Warning, NO_PLANS),
            Msg::Scanned(tests) => {
                self.entries = entries_of(tests, &self.entries);
                self.settle_cursor();
                let count = self.entries.len();
                let noun = if count == 1 { "test" } else { "tests" };
                self.say(Level::Info, format!("Found {count} {noun}"));
            }
            Msg::Failed(text) => {
                if let Screen::Editor(editor) = &mut self.screen {
                    editor.saving = None;
                }
                self.say(Level::Error, text);
            }
        }
        Vec::new()
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        self.settle_cursor();
        let rows = self.editor_rows();
        if let Screen::Editor(editor) = &mut self.screen {
            editor.top = follow(editor.top, editor.text.row(), rows);
        }
    }

    fn view(&self, frame: &mut Frame) {
        match &self.screen {
            Screen::List => self.view_list(frame),
            Screen::Editor(editor) => self.view_editor(frame, editor),
        }
    }

    fn finished(&self) -> bool {
        self.done
    }
}

impl CasePlans {
    fn view_list(&self, frame: &mut Frame) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
        draw_header(frame, chunks[0]);

        let marked = self.entries.iter().filter(|entry| entry.marked).count();
        let visible = self.visible();
        let title = if self.filter.is_empty() {
            format!("Tests: {} found, {marked} marked", self.entries.len())
        } else {
            format!(
                "Tests: {} of {} shown, {marked} marked",
                visible.len(),
                self.entries.len()
            )
        };
        let block = Block::default().borders(Borders::ALL).title(title);
        if self.entries.is_empty() {
            let text = [
                "No tests found.",
                "",
                &format!("Looked in: {}", self.looked_in),
                "Test files are the *.rs files of tests/ and examples/,",
                "or of the folder given: sbrs caseplans <folder>",
            ]
            .join("\n");
            let empty = Paragraph::new(text)
                .alignment(Alignment::Center)
                .block(block);
            frame.render_widget(empty, chunks[1]);
        } else if visible.is_empty() {
            let none = Paragraph::new("No tests match the filter.")
                .alignment(Alignment::Center)
                .block(block);
            frame.render_widget(none, chunks[1]);
        } else {
            let items: Vec<ListItem> = visible
                .iter()
                .skip(self.top)
                .take(self.list_rows().max(1))
                .map(|&index| ListItem::new(self.entries[index].row()))
                .collect();
            let list = List::new(items)
                .block(block)
                .highlight_symbol("> ")
                .highlight_spacing(HighlightSpacing::Always)
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                );
            let mut state =
                ListState::default().with_selected(Some(self.cursor.saturating_sub(self.top)));
            frame.render_stateful_widget(list, chunks[1], &mut state);
        }

        let width = usize::from(chunks[2].width);
        let filter_text = if self.filtering {
            format!("/{}", self.filter)
        } else if self.filter.is_empty() {
            String::new()
        } else {
            format!("filter: {}", self.filter)
        };
        frame.render_widget(
            Paragraph::new(clip(&filter_text, width)).style(Style::default().fg(Color::Yellow)),
            chunks[2],
        );
        if self.filtering {
            let typed = u16::try_from(filter_text.chars().count()).unwrap_or(u16::MAX);
            let x = (chunks[2].x + typed).min(chunks[2].right().saturating_sub(1));
            frame.set_cursor_position(Position::new(x, chunks[2].y));
        }
        self.draw_status(frame, chunks[3]);
        let hints = if self.filtering {
            "Enter keep  Esc clear  Backspace delete"
        } else if self.entries.is_empty() {
            "q quit  r rescan"
        } else {
            "q quit  \u{2191}\u{2193} move  Space mark  c create  Enter open  s summary  / filter  r rescan"
        };
        draw_hints(frame, chunks[4], hints);
    }

    fn view_editor(&self, frame: &mut Frame, editor: &Editor) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
        draw_header(frame, chunks[0]);

        let title = if editor.dirty {
            format!("{} *", editor.id)
        } else {
            editor.id.clone()
        };
        let block = Block::default().borders(Borders::ALL).title(title);
        let inner = block.inner(chunks[1]);
        frame.render_widget(block, chunks[1]);
        let width = usize::from(inner.width);
        let rows = usize::from(inner.height);
        // A line longer than the screen scrolls sideways, keeping the cursor
        // at the right edge.
        let left = editor.text.col().saturating_sub(width.saturating_sub(1));
        let lines: Vec<Line> = editor
            .text
            .lines()
            .iter()
            .skip(editor.top)
            .take(rows)
            .map(|line| Line::raw(line.chars().skip(left).take(width).collect::<String>()))
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
        self.draw_status(frame, chunks[2]);

        if let Some(after) = editor.asking {
            draw_hints(frame, chunks[3], "s save  d discard  Esc keep editing");
            let verb = if after == AfterSave::Quit {
                "quit"
            } else {
                "close"
            };
            let area = centered_size(64, 5, frame.area());
            let text = format!(
                "Save the changes to {}?\n\ns save and {verb}   d discard   Esc keep editing",
                editor.id
            );
            let popup = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Unsaved changes")
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(Clear, area);
            frame.render_widget(popup, area);
        } else {
            draw_hints(
                frame,
                chunks[3],
                "Ctrl-S save  Esc back  Ctrl-C quit  \u{2191}\u{2193}\u{2190}\u{2192} Home End move",
            );
            let column = u16::try_from(editor.text.col() - left).unwrap_or(u16::MAX);
            let row =
                u16::try_from(editor.text.row().saturating_sub(editor.top)).unwrap_or(u16::MAX);
            frame.set_cursor_position(Position::new(
                (inner.x + column).min(inner.right().saturating_sub(1)),
                (inner.y + row).min(inner.bottom().saturating_sub(1)),
            ));
        }
    }

    /// The status message, in the colour of its level.
    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        if let Some(status) = &self.status {
            let color = match status.level {
                Level::Info => Color::Green,
                Level::Warning => Color::Yellow,
                Level::Error => Color::Red,
            };
            frame.render_widget(
                Paragraph::new(clip(&status.text, usize::from(area.width)))
                    .style(Style::default().fg(color)),
                area,
            );
        }
    }
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(Text::styled(
        "SeleniumBase Case Plans Generator",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Case Plans"));
    frame.render_widget(header, area);
}

fn draw_hints(frame: &mut Frame, area: Rect, hints: &str) {
    frame.render_widget(
        Paragraph::new(clip(hints, usize::from(area.width)))
            .style(Style::default().fg(Color::Gray)),
        area,
    );
}

/// Carries out an effect in `base`, the folder the test ids are relative to,
/// and says what came of it. `target` is what was asked to be listed.
pub(crate) fn perform(base: &Path, target: &Path, effect: Effect) -> Option<Msg> {
    Some(match effect {
        Effect::Create(ids) => {
            let missing: Vec<String> = ids
                .iter()
                .filter(|id| !case_plans::has_plan(base, id))
                .cloned()
                .collect();
            match case_plans::create_boilerplates(base, &ids) {
                Ok(made) => Msg::Created {
                    ids: missing,
                    kept: made.kept,
                },
                Err(error) => Msg::Failed(format!("cannot create the Case Plans: {error}")),
            }
        }
        Effect::Open(id) => match case_plans::read_plan(base, &id) {
            Ok(text) => Msg::Opened { id, text },
            Err(error) => Msg::Failed(format!("cannot open the Case Plan of {id}: {error}")),
        },
        Effect::Save { id, text } => match case_plans::write_plan(base, &id, &text) {
            Ok(path) => Msg::Saved {
                path: path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
                id,
            },
            Err(error) => Msg::Failed(format!("cannot save the Case Plan of {id}: {error}")),
        },
        Effect::Summarize => match scan(base, target).and_then(|tests| {
            let ids: Vec<String> = tests.into_iter().map(|(id, _)| id).collect();
            case_plans::write_summary(base, &ids)
        }) {
            Ok(Some((path, summary))) => Msg::SummaryWritten {
                path: path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
                customized: summary.customized,
                boilerplate: summary.boilerplate,
                unfinished: summary.unfinished,
            },
            Ok(None) => Msg::NoPlans,
            Err(error) => Msg::Failed(format!("cannot write the summary: {error}")),
        },
        Effect::Rescan => match scan(base, target) {
            Ok(tests) => Msg::Scanned(tests),
            Err(error) => Msg::Failed(format!("cannot list the tests: {error}")),
        },
    })
}

/// Runs the Case Plans Generator over the tests under `target` (a folder or a
/// file; `.` for the project in the current folder) until the user quits.
///
/// # Errors
///
/// Returns the error of listing the tests or of the terminal.
pub fn run(target: &Path) -> io::Result<()> {
    let base = std::env::current_dir()?;
    let tests = scan(&base, target)?;
    let mut app = CasePlans::new(tests, &target.display().to_string());
    super::tui_kit::run(&mut app, |effect| perform(&base, target, effect))
}

/// The tests under `target` with whether each has a plan.
fn scan(base: &Path, target: &Path) -> io::Result<Vec<(String, bool)>> {
    Ok(case_plans::test_ids(base, target)?
        .into_iter()
        .map(|id| {
            let plan = case_plans::has_plan(base, &id);
            (id, plan)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::scripts::tui_kit::{render, render_cursor};

    fn tests(list: &[(&str, bool)]) -> Vec<(String, bool)> {
        list.iter()
            .map(|(id, plan)| ((*id).to_owned(), *plan))
            .collect()
    }

    fn app(list: &[(&str, bool)]) -> CasePlans {
        CasePlans::new(tests(list), "tests")
    }

    fn sample() -> CasePlans {
        app(&[
            ("tests/about.rs::page", true),
            ("tests/login.rs::valid", false),
            ("tests/login.rs::invalid", false),
            ("examples/demo.rs", true),
        ])
    }

    fn press(app: &mut CasePlans, input: Input) -> Vec<Effect> {
        app.update(Msg::Key(input))
    }

    fn type_text(app: &mut CasePlans, text: &str) {
        for c in text.chars() {
            press(app, Input::Char(c));
        }
    }

    fn status(app: &CasePlans) -> (Level, &str) {
        let status = app.status.as_ref().expect("a status message");
        (status.level, status.text.as_str())
    }

    fn marked(app: &CasePlans) -> Vec<&str> {
        app.entries
            .iter()
            .filter(|e| e.marked)
            .map(|e| e.id.as_str())
            .collect()
    }

    fn open_editor(app: &mut CasePlans, id: &str, text: &str) {
        app.update(Msg::Opened {
            id: id.to_owned(),
            text: text.to_owned(),
        });
    }

    fn editor(app: &CasePlans) -> &Editor {
        match &app.screen {
            Screen::Editor(editor) => editor,
            Screen::List => panic!("the list is showing, not the editor"),
        }
    }

    // ------------------------------------------------------------------
    // The list
    // ------------------------------------------------------------------

    #[test]
    fn the_cursor_wraps_round_the_list_in_both_directions() {
        let mut app = sample();

        press(&mut app, Input::Up);
        assert_eq!(app.cursor, 3, "up from the top goes to the bottom");
        press(&mut app, Input::Down);
        assert_eq!(app.cursor, 0);
        press(&mut app, Input::Char('j'));
        press(&mut app, Input::Char('j'));
        press(&mut app, Input::Char('k'));
        assert_eq!(app.cursor, 1);
    }

    #[test]
    fn page_keys_jump_a_page_and_stop_at_the_ends() {
        let tests: Vec<(String, bool)> = (0..30).map(|n| (format!("t{n:02}"), false)).collect();
        let mut app = CasePlans::new(tests, "tests");
        app.resize(80, 14);

        press(&mut app, Input::PageDown);
        assert_eq!(app.cursor, 5, "a page is the six visible rows less one");
        for _ in 0..10 {
            press(&mut app, Input::PageDown);
        }
        assert_eq!(app.cursor, 29);
        press(&mut app, Input::Home);
        assert_eq!(app.cursor, 0);
        press(&mut app, Input::End);
        assert_eq!(app.cursor, 29);
        assert_eq!(app.top, 24, "the window shows the last six rows");
    }

    #[test]
    fn space_marks_and_unmarks_the_row_under_the_cursor() {
        let mut app = sample();

        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));
        assert_eq!(marked(&app), ["tests/login.rs::valid"]);
        press(&mut app, Input::Char(' '));
        assert!(marked(&app).is_empty());
    }

    #[test]
    fn a_marks_everything_shown_and_n_clears_the_marks() {
        let mut app = sample();

        press(&mut app, Input::Char('/'));
        type_text(&mut app, "login");
        press(&mut app, Input::Enter);
        press(&mut app, Input::Char('a'));
        assert_eq!(
            marked(&app),
            ["tests/login.rs::valid", "tests/login.rs::invalid"],
            "only what the filter shows"
        );

        press(&mut app, Input::Char('n'));
        assert!(marked(&app).is_empty());
    }

    #[test]
    fn the_filter_narrows_the_list_and_keeps_the_cursor_in_range() {
        let mut app = sample();
        app.cursor = 3;

        press(&mut app, Input::Char('/'));
        assert!(app.filtering);
        type_text(&mut app, "LOGIN");
        assert_eq!(app.visible(), [1, 2], "the match ignores case");
        assert_eq!(app.cursor, 1, "the cursor cannot point past the shown rows");

        press(&mut app, Input::Backspace);
        assert_eq!(app.filter, "LOGI");
        press(&mut app, Input::Enter);
        assert!(!app.filtering);
        assert_eq!(app.filter, "LOGI", "Enter keeps the filter");

        press(&mut app, Input::Char('/'));
        press(&mut app, Input::Esc);
        assert!(!app.filtering);
        assert!(app.filter.is_empty(), "Esc clears it");
        assert_eq!(app.visible().len(), 4);
    }

    #[test]
    fn keys_typed_in_the_filter_are_text_not_commands() {
        let mut app = sample();

        press(&mut app, Input::Char('/'));
        type_text(&mut app, "qcs ");

        assert_eq!(app.filter, "qcs ");
        assert!(!app.finished());
    }

    #[test]
    fn q_and_ctrl_c_quit_from_the_list_and_ctrl_c_quits_from_the_filter() {
        let mut app = sample();
        press(&mut app, Input::Char('q'));
        assert!(app.finished());

        let mut app = sample();
        press(&mut app, Input::Ctrl('c'));
        assert!(app.finished());

        let mut app = sample();
        press(&mut app, Input::Char('/'));
        press(&mut app, Input::Ctrl('c'));
        assert!(app.finished());
        assert!(app.filter.is_empty(), "no c typed into the filter");
    }

    // ------------------------------------------------------------------
    // Creating plans
    // ------------------------------------------------------------------

    #[test]
    fn c_creates_plans_for_the_marked_tests_that_have_none() {
        let mut app = sample();
        for _ in 0..4 {
            press(&mut app, Input::Char(' '));
            press(&mut app, Input::Down);
        }

        let effects = press(&mut app, Input::Char('c'));

        assert_eq!(
            effects,
            [Effect::Create(vec![
                "tests/login.rs::valid".to_owned(),
                "tests/login.rs::invalid".to_owned()
            ])],
            "the two that already have a plan are left out"
        );
    }

    #[test]
    fn with_nothing_marked_c_creates_the_plan_of_the_row_under_the_cursor() {
        let mut app = sample();
        press(&mut app, Input::Down);

        let effects = press(&mut app, Input::Char('c'));

        assert_eq!(
            effects,
            [Effect::Create(vec!["tests/login.rs::valid".to_owned()])]
        );
    }

    #[test]
    fn c_on_tests_that_all_have_plans_says_so_and_creates_nothing() {
        let mut app = sample();
        press(&mut app, Input::Char(' '));

        let effects = press(&mut app, Input::Char('c'));

        assert!(effects.is_empty());
        let (level, text) = status(&app);
        assert_eq!(level, Level::Warning);
        assert!(text.contains("already have Case Plans"), "{text}");
    }

    #[test]
    fn created_plans_show_as_having_one_and_are_counted() {
        let mut app = sample();

        app.update(Msg::Created {
            ids: vec!["tests/login.rs::valid".to_owned()],
            kept: 0,
        });

        assert!(app.entries[1].plan);
        assert!(!app.entries[2].plan);
        assert_eq!(
            status(&app),
            (Level::Info, "1 new boilerplate Case Plan was generated")
        );

        app.update(Msg::Created {
            ids: vec!["a".to_owned(), "b".to_owned()],
            kept: 1,
        });
        assert_eq!(
            status(&app),
            (Level::Info, "2 new boilerplate Case Plans were generated")
        );
    }

    // ------------------------------------------------------------------
    // Opening, editing and saving
    // ------------------------------------------------------------------

    #[test]
    fn enter_opens_the_plan_of_the_row_and_a_missing_plan_says_how_to_make_one() {
        let mut app = sample();
        assert_eq!(
            press(&mut app, Input::Enter),
            [Effect::Open("tests/about.rs::page".to_owned())]
        );

        press(&mut app, Input::Down);
        let effects = press(&mut app, Input::Enter);

        assert!(effects.is_empty());
        let (level, text) = status(&app);
        assert_eq!(level, Level::Warning);
        assert!(
            text.contains("no Case Plan yet") && text.contains("press c"),
            "{text}"
        );
    }

    #[test]
    fn an_opened_plan_is_shown_in_the_editor_with_the_cursor_at_its_start() {
        let mut app = sample();

        open_editor(
            &mut app,
            "tests/about.rs::page",
            "title\r\n---\r\n| a |\r\n",
        );

        let editor = editor(&app);
        assert_eq!(editor.id, "tests/about.rs::page");
        assert_eq!(
            editor.text.to_text(),
            "title\n---\n| a |\n",
            "line ends are LF"
        );
        assert_eq!((editor.text.row(), editor.text.col()), (0, 0));
        assert!(!editor.dirty);
    }

    #[test]
    fn typing_edits_the_text_and_marks_it_modified() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\ncd\n");

        press(&mut app, Input::Char('X'));
        press(&mut app, Input::Enter);
        press(&mut app, Input::Down);
        press(&mut app, Input::End);
        press(&mut app, Input::Char('!'));

        let editor = editor(&app);
        assert_eq!(editor.text.to_text(), "X\nab\ncd!\n");
        assert!(editor.dirty);
    }

    #[test]
    fn moving_in_the_editor_does_not_make_it_modified() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\ncd\n");

        for input in [
            Input::Down,
            Input::Right,
            Input::Home,
            Input::End,
            Input::Up,
        ] {
            press(&mut app, input);
        }

        assert!(!editor(&app).dirty);
    }

    #[test]
    fn ctrl_s_saves_the_text_with_a_closing_newline() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab");
        press(&mut app, Input::Char('X'));

        let effects = press(&mut app, Input::Ctrl('s'));

        assert_eq!(
            effects,
            [Effect::Save {
                id: "tests/about.rs::page".to_owned(),
                text: "Xab\n".to_owned()
            }]
        );
    }

    #[test]
    fn a_saved_plan_is_no_longer_modified_and_the_row_has_a_plan() {
        let mut app = sample();
        open_editor(&mut app, "tests/login.rs::valid", "ab\n");
        press(&mut app, Input::Char('X'));
        press(&mut app, Input::Ctrl('s'));

        app.update(Msg::Saved {
            id: "tests/login.rs::valid".to_owned(),
            path: "tests/case_plans/login.valid.md".to_owned(),
        });

        assert!(!editor(&app).dirty);
        assert!(app.entries[1].plan);
        assert_eq!(
            status(&app),
            (Level::Info, "Saved tests/case_plans/login.valid.md")
        );
    }

    #[test]
    fn a_failed_save_keeps_the_text_and_says_why() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\n");
        press(&mut app, Input::Char('X'));
        press(&mut app, Input::Ctrl('s'));

        app.update(Msg::Failed("permission denied".to_owned()));

        assert!(editor(&app).dirty);
        assert_eq!(status(&app), (Level::Error, "permission denied"));
    }

    #[test]
    fn esc_goes_back_to_the_list_when_nothing_changed() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\n");

        press(&mut app, Input::Esc);

        assert_eq!(app.screen, Screen::List);
        assert!(!app.finished());
    }

    #[test]
    fn esc_with_unsaved_changes_asks_and_s_saves_then_goes_back() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\n");
        press(&mut app, Input::Char('X'));

        let asked = press(&mut app, Input::Esc);
        assert!(asked.is_empty());
        assert_eq!(editor(&app).asking, Some(AfterSave::Close));

        let effects = press(&mut app, Input::Char('s'));
        assert_eq!(effects.len(), 1, "{effects:?}");
        assert!(matches!(effects[0], Effect::Save { .. }));
        assert!(
            matches!(app.screen, Screen::Editor(_)),
            "still open until saved"
        );

        app.update(Msg::Saved {
            id: "tests/about.rs::page".to_owned(),
            path: "p".to_owned(),
        });
        assert_eq!(app.screen, Screen::List);
    }

    #[test]
    fn d_discards_the_changes_and_esc_keeps_editing() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\n");
        press(&mut app, Input::Char('X'));
        press(&mut app, Input::Esc);
        press(&mut app, Input::Esc);
        assert_eq!(
            editor(&app).asking,
            None,
            "Esc on the question keeps editing"
        );
        assert_eq!(editor(&app).text.to_text(), "Xab\n");

        press(&mut app, Input::Esc);
        press(&mut app, Input::Char('d'));

        assert_eq!(app.screen, Screen::List);
        assert!(!app.finished());
    }

    #[test]
    fn ctrl_c_in_the_editor_quits_at_once_when_nothing_changed() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\n");

        press(&mut app, Input::Ctrl('c'));

        assert!(app.finished());
    }

    #[test]
    fn ctrl_c_with_unsaved_changes_asks_before_quitting() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\n");
        press(&mut app, Input::Char('X'));

        press(&mut app, Input::Ctrl('c'));
        assert!(!app.finished());
        assert_eq!(editor(&app).asking, Some(AfterSave::Quit));

        press(&mut app, Input::Char('d'));
        assert!(app.finished());
    }

    #[test]
    fn saving_before_quitting_quits_once_the_save_is_done() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\n");
        press(&mut app, Input::Char('X'));
        press(&mut app, Input::Ctrl('c'));

        let effects = press(&mut app, Input::Char('s'));
        assert_eq!(effects.len(), 1);
        assert!(!app.finished(), "not before the file is written");
        app.update(Msg::Saved {
            id: "tests/about.rs::page".to_owned(),
            path: "p".to_owned(),
        });

        assert!(app.finished());
    }

    #[test]
    fn a_save_that_fails_while_quitting_keeps_the_editor_open() {
        let mut app = sample();
        open_editor(&mut app, "tests/about.rs::page", "ab\n");
        press(&mut app, Input::Char('X'));
        press(&mut app, Input::Ctrl('c'));
        press(&mut app, Input::Char('s'));

        app.update(Msg::Failed("disk full".to_owned()));

        assert!(!app.finished());
        assert!(matches!(app.screen, Screen::Editor(_)));
        assert_eq!(editor(&app).saving, None);
    }

    // ------------------------------------------------------------------
    // The summary and rescanning
    // ------------------------------------------------------------------

    #[test]
    fn s_asks_for_the_summary_when_some_test_has_a_plan() {
        let mut app = sample();

        assert_eq!(press(&mut app, Input::Char('s')), [Effect::Summarize]);
    }

    #[test]
    fn s_with_no_plan_anywhere_warns_without_asking() {
        let mut app = app(&[("tests/a.rs::x", false)]);

        let effects = press(&mut app, Input::Char('s'));

        assert!(effects.is_empty());
        assert_eq!(
            status(&app),
            (
                Level::Warning,
                "No existing Case Plans found! Create some boilerplates first!"
            )
        );
    }

    #[test]
    fn the_written_summary_is_reported_with_its_counts() {
        let mut app = sample();

        app.update(Msg::SummaryWritten {
            path: "case_summary.md".to_owned(),
            customized: 2,
            boilerplate: 1,
            unfinished: 0,
        });

        assert_eq!(
            status(&app),
            (
                Level::Info,
                "Summary written to case_summary.md: 2 customized, 1 boilerplate, 0 unfinished"
            )
        );
    }

    #[test]
    fn r_lists_the_tests_again_and_keeps_the_marks() {
        let mut app = sample();
        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));
        assert_eq!(press(&mut app, Input::Char('r')), [Effect::Rescan]);

        app.update(Msg::Scanned(tests(&[
            ("tests/login.rs::valid", true),
            ("tests/new.rs::fresh", false),
        ])));

        assert_eq!(app.entries.len(), 2);
        assert_eq!(marked(&app), ["tests/login.rs::valid"]);
        assert!(app.entries[0].plan, "the plan that appeared is shown");
        assert_eq!(app.cursor, 1.min(app.entries.len() - 1));
    }

    #[test]
    fn a_failure_is_shown_as_an_error() {
        let mut app = sample();

        app.update(Msg::Failed("cannot read tests".to_owned()));

        assert_eq!(status(&app), (Level::Error, "cannot read tests"));
    }

    // ------------------------------------------------------------------
    // The effects, against a folder on disk
    // ------------------------------------------------------------------

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        std::fs::write(
            dir.path().join("tests/login.rs"),
            "#[test]\nfn valid() {}\n#[test]\nfn invalid() {}\n",
        )
        .unwrap();
        dir
    }

    fn performing(base: &Path, effect: Effect) -> Msg {
        perform(base, Path::new("."), effect).expect("every effect has a result")
    }

    #[test]
    fn the_app_lists_what_the_scan_finds_with_the_plans_that_exist() {
        let dir = project();
        case_plans::write_plan(dir.path(), "tests/login.rs::valid", "x\n").unwrap();

        let found = scan(dir.path(), Path::new(".")).unwrap();

        assert_eq!(
            found,
            tests(&[
                ("tests/login.rs::valid", true),
                ("tests/login.rs::invalid", false)
            ])
        );
    }

    #[test]
    fn creating_writes_the_plans_and_says_which_were_new() {
        let dir = project();
        case_plans::write_plan(dir.path(), "tests/login.rs::valid", "mine\n").unwrap();

        let msg = performing(
            dir.path(),
            Effect::Create(vec![
                "tests/login.rs::valid".to_owned(),
                "tests/login.rs::invalid".to_owned(),
            ]),
        );

        assert_eq!(
            msg,
            Msg::Created {
                ids: vec!["tests/login.rs::invalid".to_owned()],
                kept: 1
            }
        );
        assert_eq!(
            case_plans::read_plan(dir.path(), "tests/login.rs::valid").unwrap(),
            "mine\n"
        );
        assert!(case_plans::has_plan(dir.path(), "tests/login.rs::invalid"));
    }

    #[test]
    fn opening_reads_the_plan_and_a_plan_that_is_gone_is_a_failure() {
        let dir = project();
        case_plans::write_plan(dir.path(), "tests/login.rs::valid", "steps\n").unwrap();

        assert_eq!(
            performing(dir.path(), Effect::Open("tests/login.rs::valid".to_owned())),
            Msg::Opened {
                id: "tests/login.rs::valid".to_owned(),
                text: "steps\n".to_owned()
            }
        );
        let Msg::Failed(text) = performing(
            dir.path(),
            Effect::Open("tests/login.rs::invalid".to_owned()),
        ) else {
            panic!("a missing plan cannot be opened");
        };
        assert!(text.contains("tests/login.rs::invalid"), "{text}");
    }

    #[test]
    fn saving_writes_the_text_and_reports_the_path_below_the_project() {
        let dir = project();

        let msg = performing(
            dir.path(),
            Effect::Save {
                id: "tests/login.rs::valid".to_owned(),
                text: "new text\n".to_owned(),
            },
        );

        assert_eq!(
            msg,
            Msg::Saved {
                id: "tests/login.rs::valid".to_owned(),
                path: "tests/case_plans/login.valid.md".to_owned()
            }
        );
        assert_eq!(
            case_plans::read_plan(dir.path(), "tests/login.rs::valid").unwrap(),
            "new text\n"
        );
    }

    #[test]
    fn the_summary_is_written_for_the_listed_tests_with_a_plan() {
        let dir = project();
        assert_eq!(performing(dir.path(), Effect::Summarize), Msg::NoPlans);
        case_plans::create_boilerplates(dir.path(), &["tests/login.rs::valid".to_owned()]).unwrap();

        let msg = performing(dir.path(), Effect::Summarize);

        assert_eq!(
            msg,
            Msg::SummaryWritten {
                path: "case_summary.md".to_owned(),
                customized: 0,
                boilerplate: 1,
                unfinished: 0
            }
        );
        assert!(dir.path().join("case_summary.md").is_file());
    }

    #[test]
    fn a_rescan_lists_a_test_that_was_added_since() {
        let dir = project();
        std::fs::write(dir.path().join("tests/cart.rs"), "#[test]\nfn empty() {}\n").unwrap();

        let Msg::Scanned(found) = performing(dir.path(), Effect::Rescan) else {
            panic!("a rescan lists the tests");
        };

        assert_eq!(found[0].0, "tests/cart.rs::empty");
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn e_opens_like_enter() {
        let mut app = sample();

        assert_eq!(
            press(&mut app, Input::Char('e')),
            [Effect::Open("tests/about.rs::page".to_owned())]
        );
    }

    // ------------------------------------------------------------------
    // Drawing
    // ------------------------------------------------------------------

    const W: usize = 80;

    /// A bordered block `W` wide, with `title` in its top edge and `rows`
    /// inside.
    fn framed(title: &str, rows: &[&str]) -> Vec<String> {
        let mut block = vec![format!(
            "┌{title}{}┐",
            "─".repeat(W - 2 - title.chars().count())
        )];
        block.extend(rows.iter().map(|row| format!("│{row:<0$}│", W - 2)));
        block.push(format!("└{}┘", "─".repeat(W - 2)));
        block
    }

    /// `text` centered in the inside of a block `W` wide; when the space left
    /// over is odd, ratatui gives the extra column to the left.
    fn centered_in_frame(text: &str) -> String {
        format!(
            "{}{text}",
            " ".repeat((W - 2 - text.chars().count()).div_ceil(2))
        )
    }

    fn header() -> Vec<String> {
        framed(
            "Case Plans",
            &[&centered_in_frame("SeleniumBase Case Plans Generator")],
        )
    }

    const LIST_HINTS: &str =
        "q quit  ↑↓ move  Space mark  c create  Enter open  s summary  / filter  r rescan";

    #[test]
    fn the_list_screen_shows_marks_plans_and_the_cursor() {
        let mut app = sample();
        app.resize(80, 14);
        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));

        let mut expected = header();
        expected.extend(framed(
            "Tests: 4 found, 1 marked",
            &[
                "  [ ] [plan] tests/about.rs::page",
                "> [x] [    ] tests/login.rs::valid",
                "  [ ] [    ] tests/login.rs::invalid",
                "  [ ] [plan] examples/demo.rs",
                "",
                "",
            ],
        ));
        expected.extend([String::new(), String::new(), LIST_HINTS.to_owned()]);
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn a_long_list_shows_the_window_the_cursor_is_in() {
        let tests: Vec<(String, bool)> = (0..30).map(|n| (format!("t{n:02}"), false)).collect();
        let mut app = CasePlans::new(tests, "tests");
        app.resize(80, 14);
        press(&mut app, Input::End);

        let mut expected = header();
        expected.extend(framed(
            "Tests: 30 found, 0 marked",
            &[
                "  [ ] [    ] t24",
                "  [ ] [    ] t25",
                "  [ ] [    ] t26",
                "  [ ] [    ] t27",
                "  [ ] [    ] t28",
                "> [ ] [    ] t29",
            ],
        ));
        expected.extend([String::new(), String::new(), LIST_HINTS.to_owned()]);
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn a_filter_in_force_and_a_status_message_have_their_own_rows() {
        let mut app = sample();
        app.resize(80, 14);
        press(&mut app, Input::Char('/'));
        type_text(&mut app, "login");
        press(&mut app, Input::Enter);
        app.update(Msg::Created {
            ids: vec!["tests/login.rs::valid".to_owned()],
            kept: 0,
        });

        let mut expected = header();
        expected.extend(framed(
            "Tests: 2 of 4 shown, 0 marked",
            &[
                "> [ ] [plan] tests/login.rs::valid",
                "  [ ] [    ] tests/login.rs::invalid",
                "",
                "",
                "",
                "",
            ],
        ));
        expected.extend([
            "filter: login".to_owned(),
            "1 new boilerplate Case Plan was generated".to_owned(),
            LIST_HINTS.to_owned(),
        ]);
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn while_filtering_the_row_shows_what_is_typed_and_the_hints_change() {
        let mut app = sample();
        app.resize(80, 14);
        press(&mut app, Input::Char('/'));
        type_text(&mut app, "log");

        let mut expected = header();
        expected.extend(framed(
            "Tests: 2 of 4 shown, 0 marked",
            &[
                "> [ ] [    ] tests/login.rs::valid",
                "  [ ] [    ] tests/login.rs::invalid",
                "",
                "",
                "",
                "",
            ],
        ));
        expected.extend([
            "/log".to_owned(),
            String::new(),
            "Enter keep  Esc clear  Backspace delete".to_owned(),
        ]);
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn a_filter_that_matches_nothing_says_so() {
        let mut app = sample();
        app.resize(80, 14);
        press(&mut app, Input::Char('/'));
        type_text(&mut app, "zzz");

        let screen = render(&app, 80, 14);

        assert!(
            screen.contains(&centered_in_frame("No tests match the filter.")),
            "{screen}"
        );
    }

    #[test]
    fn with_no_tests_the_screen_says_where_it_looked_and_how_to_point_it_elsewhere() {
        let mut app = app(&[]);
        app.resize(80, 14);

        let mut expected = header();
        expected.extend(framed(
            "Tests: 0 found, 0 marked",
            &[
                &centered_in_frame("No tests found."),
                "",
                &centered_in_frame("Looked in: tests"),
                &centered_in_frame("Test files are the *.rs files of tests/ and examples/,"),
                &centered_in_frame("or of the folder given: sbrs caseplans <folder>"),
                "",
            ],
        ));
        expected.extend([String::new(), String::new(), "q quit  r rescan".to_owned()]);
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn an_error_and_a_warning_read_the_same_in_text_and_differ_in_colour_only() {
        let mut app = sample();
        app.resize(80, 14);
        app.update(Msg::Failed("cannot write".to_owned()));
        let error = render(&app, 80, 14);
        app.update(Msg::NoPlans);
        let warning = render(&app, 80, 14);

        assert_eq!(error.lines().nth(12), Some("cannot write"), "{error}");
        assert_eq!(
            warning.lines().nth(12),
            Some("No existing Case Plans found! Create some boilerplates first!"),
            "{warning}"
        );
    }

    #[test]
    fn the_editor_shows_the_plan_with_a_marker_when_modified() {
        let mut app = sample();
        app.resize(80, 14);
        open_editor(
            &mut app,
            "tests/about.rs::page",
            "``about.rs::page``\n---\n| a |\n",
        );
        press(&mut app, Input::Char('X'));

        let mut expected = header();
        expected.extend(framed(
            "tests/about.rs::page *",
            &["X``about.rs::page``", "---", "| a |", "", "", "", ""],
        ));
        expected.extend([
            String::new(),
            "Ctrl-S save  Esc back  Ctrl-C quit  ↑↓←→ Home End move".to_owned(),
        ]);
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn the_cursor_is_drawn_in_the_text_of_the_editor() {
        let mut app = sample();
        app.resize(80, 14);
        open_editor(&mut app, "tests/about.rs::page", "ab\ncd\n");

        // Inside the left border and below the header and the top border.
        assert_eq!(render_cursor(&app, 80, 14), Some((1, 4)));
        press(&mut app, Input::Down);
        press(&mut app, Input::Right);
        assert_eq!(render_cursor(&app, 80, 14), Some((2, 5)));
    }

    #[test]
    fn the_list_has_no_cursor_of_its_own_unless_filtering() {
        let mut app = sample();
        app.resize(80, 14);
        assert_eq!(render_cursor(&app, 80, 14), None);

        press(&mut app, Input::Char('/'));
        type_text(&mut app, "ab");
        // After the typed text, on the filter row below the list.
        assert_eq!(render_cursor(&app, 80, 14), Some((3, 11)));
    }

    #[test]
    fn a_tall_plan_scrolls_to_keep_the_cursor_in_view() {
        let mut app = sample();
        app.resize(80, 14);
        let text: String = (0..20).map(|n| format!("line {n:02}\n")).collect();
        open_editor(&mut app, "tests/about.rs::page", &text);

        for _ in 0..10 {
            press(&mut app, Input::Down);
        }

        let editor = editor(&app);
        assert_eq!(editor.text.row(), 10);
        assert_eq!(editor.top, 4, "seven rows are shown, ending at the cursor");
        let screen = render(&app, 80, 14);
        assert!(
            screen.contains("line 04") && screen.contains("line 10"),
            "{screen}"
        );
        assert!(
            !screen.contains("line 03") && !screen.contains("line 11"),
            "{screen}"
        );
    }

    #[test]
    fn the_unsaved_changes_question_is_drawn_over_the_editor() {
        let mut app = sample();
        app.resize(80, 14);
        open_editor(&mut app, "tests/about.rs::page", "ab\n");
        press(&mut app, Input::Char('X'));
        press(&mut app, Input::Esc);

        let screen = render(&app, 80, 14);

        for wanted in [
            "Unsaved changes",
            "s save and close",
            "d discard",
            "Esc keep editing",
        ] {
            assert!(screen.contains(wanted), "{wanted}: {screen}");
        }
    }
}
