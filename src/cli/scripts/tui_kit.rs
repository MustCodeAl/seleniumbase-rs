//! What the terminal apps (`sbase caseplans`, `sbase behave-gui`) share: the
//! key presses they understand, the message pump that drives a state machine,
//! and the terminal itself.
//!
//! An app is a [`Machine`]: a message goes in, the state changes, and a list of
//! **effects** (read this file, run these scenarios) comes out. The machine
//! never touches the file system or the terminal, so what it does is tested
//! with plain values; [`run`] is the thin shell that reads keys, draws the
//! state and performs the effects.

use std::collections::VecDeque;
use std::io::{self, stdout, Stdout};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};

/// A key press, reduced to what the apps react to.
///
/// In raw mode Ctrl-C is a key, not a signal, so it is an [`Input`] like the
/// others: every app treats it as quit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Input {
    /// A character, typed with or without Shift.
    Char(char),
    /// A character with Ctrl held, in lower case.
    Ctrl(char),
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Enter,
    Esc,
    Backspace,
    Delete,
    Tab,
}

impl Input {
    /// The input for a key event, or `None` for a key the apps ignore. Only
    /// presses count: a terminal that reports releases and repeats would
    /// otherwise type every character twice.
    pub(crate) fn from_key(key: KeyEvent) -> Option<Self> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        Some(match key.code {
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Self::Ctrl(c.to_ascii_lowercase())
            }
            KeyCode::Char(c) => Self::Char(c),
            KeyCode::Up => Self::Up,
            KeyCode::Down => Self::Down,
            KeyCode::Left => Self::Left,
            KeyCode::Right => Self::Right,
            KeyCode::Home => Self::Home,
            KeyCode::End => Self::End,
            KeyCode::PageUp => Self::PageUp,
            KeyCode::PageDown => Self::PageDown,
            KeyCode::Enter => Self::Enter,
            KeyCode::Esc => Self::Esc,
            KeyCode::Backspace => Self::Backspace,
            KeyCode::Delete => Self::Delete,
            KeyCode::Tab => Self::Tab,
            _ => return None,
        })
    }
}

/// A terminal app as a state machine.
pub(crate) trait Machine {
    /// What can happen to the app: a key, or the result of an effect.
    type Msg: From<Input>;
    /// What the app asks the outside world to do.
    type Effect;

    /// Applies `msg` and returns the effects it asks for.
    fn update(&mut self, msg: Self::Msg) -> Vec<Self::Effect>;
    /// Tells the app how big its screen is, at the start and when it changes.
    fn resize(&mut self, width: u16, height: u16);
    /// Draws the state.
    fn view(&self, frame: &mut Frame);
    /// Whether the app has been told to quit.
    fn finished(&self) -> bool;
}

/// Applies `msg`, performs the effects it asks for, and feeds the results back
/// in, until nothing is left to do or the app is finished.
///
/// `before_effects` is called with the state after each message that asks for
/// effects, before they run: that is when to redraw, so that a long effect
/// (running scenarios) shows what it is doing instead of a stale screen.
pub(crate) fn settle<M: Machine>(
    machine: &mut M,
    msg: M::Msg,
    perform: &mut impl FnMut(M::Effect) -> Option<M::Msg>,
    mut before_effects: impl FnMut(&M),
) {
    let mut queue = VecDeque::from([msg]);
    while let Some(msg) = queue.pop_front() {
        let effects = machine.update(msg);
        if machine.finished() {
            return;
        }
        if effects.is_empty() {
            continue;
        }
        before_effects(machine);
        for effect in effects {
            if let Some(next) = perform(effect) {
                queue.push_back(next);
            }
        }
    }
}

/// The terminal in the alternate screen and raw mode. Dropping it puts the
/// terminal back, also when the app fails or panics.
struct Screen {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Screen {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        if let Err(error) = execute!(out, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        match Terminal::new(CrosstermBackend::new(out)) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = disable_raw_mode();
                let _ = execute!(stdout(), LeaveAlternateScreen);
                Err(error)
            }
        }
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        // Nothing can be done about a terminal that will not be restored.
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

/// Runs `machine` full screen until it is finished, performing its effects
/// with `perform`.
///
/// # Errors
///
/// Returns the error of setting up or drawing on the terminal, or of reading a
/// key.
pub(crate) fn run<M: Machine>(
    machine: &mut M,
    mut perform: impl FnMut(M::Effect) -> Option<M::Msg>,
) -> io::Result<()> {
    let mut screen = Screen::enter()?;
    let size = screen.terminal.size()?;
    machine.resize(size.width, size.height);
    loop {
        screen.terminal.draw(|frame| machine.view(frame))?;
        if machine.finished() {
            return Ok(());
        }
        match event::read()? {
            Event::Key(key) => {
                let Some(input) = Input::from_key(key) else {
                    continue;
                };
                let mut drawn = Ok(());
                settle(machine, input.into(), &mut perform, |state| {
                    if drawn.is_ok() {
                        drawn = screen.terminal.draw(|frame| state.view(frame)).map(|_| ());
                    }
                });
                drawn?;
            }
            Event::Resize(width, height) => machine.resize(width, height),
            _ => {}
        }
    }
}

/// The first row to show, in a window of `rows` rows over a list, so that the
/// row `cursor` is visible. The window stays where it is while the cursor
/// moves inside it.
pub(crate) fn follow(top: usize, cursor: usize, rows: usize) -> usize {
    if rows == 0 {
        0
    } else if cursor < top {
        cursor
    } else if cursor >= top + rows {
        cursor + 1 - rows
    } else {
        top
    }
}

/// Text being edited: lines and a cursor.
///
/// The cursor is a line and a column counted in characters, never in the
/// middle of one. Text round-trips exactly: `from_text(t).to_text() == t` for
/// any `t` with LF line ends, so a file that ends in a newline keeps its last
/// empty line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextBuffer {
    lines: Vec<String>,
    row: usize,
    col: usize,
}

/// The byte index of character `col` of `line`, or its end.
fn byte_at(line: &str, col: usize) -> usize {
    line.char_indices()
        .nth(col)
        .map_or(line.len(), |(at, _)| at)
}

fn width(line: &str) -> usize {
    line.chars().count()
}

impl TextBuffer {
    /// The buffer for `text`, with the cursor at its start. A CRLF line end
    /// becomes LF.
    pub(crate) fn from_text(text: &str) -> Self {
        Self {
            lines: text
                .replace("\r\n", "\n")
                .split('\n')
                .map(str::to_owned)
                .collect(),
            row: 0,
            col: 0,
        }
    }

    /// The text, with LF line ends.
    pub(crate) fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    pub(crate) fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The line the cursor is on, counted from 0.
    pub(crate) fn row(&self) -> usize {
        self.row
    }

    /// The column the cursor is on, in characters, counted from 0.
    pub(crate) fn col(&self) -> usize {
        self.col
    }

    /// Types a character at the cursor. A control character is not text and
    /// is ignored; it returns whether the text changed.
    pub(crate) fn insert(&mut self, c: char) -> bool {
        if c.is_control() {
            return false;
        }
        let line = &mut self.lines[self.row];
        line.insert(byte_at(line, self.col), c);
        self.col += 1;
        true
    }

    /// Breaks the line at the cursor.
    pub(crate) fn newline(&mut self) {
        let line = &mut self.lines[self.row];
        let rest = line.split_off(byte_at(line, self.col));
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
    }

    /// Deletes the character before the cursor, or joins the line to the one
    /// above at its start. Returns whether the text changed.
    pub(crate) fn backspace(&mut self) -> bool {
        if self.col > 0 {
            let line = &mut self.lines[self.row];
            line.remove(byte_at(line, self.col - 1));
            self.col -= 1;
            true
        } else if self.row > 0 {
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = width(&self.lines[self.row]);
            self.lines[self.row].push_str(&line);
            true
        } else {
            false
        }
    }

    /// Deletes the character at the cursor, or joins the next line at the end
    /// of a line. Returns whether the text changed.
    pub(crate) fn delete(&mut self) -> bool {
        if self.col < width(&self.lines[self.row]) {
            let line = &mut self.lines[self.row];
            line.remove(byte_at(line, self.col));
            true
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
            true
        } else {
            false
        }
    }

    pub(crate) fn left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = width(&self.lines[self.row]);
        }
    }

    pub(crate) fn right(&mut self) {
        if self.col < width(&self.lines[self.row]) {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    /// Moves `delta` lines down (up if negative), stopping at the first and
    /// last line. The column stays, or moves to the end of a shorter line.
    pub(crate) fn vertical(&mut self, delta: isize) {
        self.row = self
            .row
            .saturating_add_signed(delta)
            .min(self.lines.len() - 1);
        self.col = self.col.min(width(&self.lines[self.row]));
    }

    pub(crate) fn home(&mut self) {
        self.col = 0;
    }

    pub(crate) fn end(&mut self) {
        self.col = width(&self.lines[self.row]);
    }
}

/// A rectangle of `width` by `height` characters in the middle of `area`, or
/// as much of that as `area` has room for.
pub(crate) fn centered_size(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// The rows of `text` that fit in `rows` rows of `width` columns, cut at the
/// width: a plain stand-in for wrapping that keeps the layout predictable.
pub(crate) fn clip(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

/// Draws `machine` on a screen of `width` by `height` characters and returns
/// the text on it: one line per row, without the spaces that end a row.
#[cfg(test)]
pub(crate) fn render<M: Machine>(machine: &M, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("a test terminal can be made");
    terminal
        .draw(|frame| machine.view(frame))
        .expect("drawing on a test terminal cannot fail");
    snapshot(terminal.backend().buffer())
}

/// Draws `machine` on a screen of `width` by `height` characters and returns
/// where the cursor is, or `None` if the app left it hidden.
#[cfg(test)]
pub(crate) fn render_cursor<M: Machine>(
    machine: &M,
    width: u16,
    height: u16,
) -> Option<(u16, u16)> {
    let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("a test terminal can be made");
    terminal
        .draw(|frame| machine.view(frame))
        .expect("drawing on a test terminal cannot fail");
    let backend = terminal.backend();
    backend.cursor_visible().then(|| {
        let at = backend.cursor_position();
        (at.x, at.y)
    })
}

/// The text of a buffer, one line per row, without the spaces that end a row.
#[cfg(test)]
pub(crate) fn snapshot(buffer: &ratatui::buffer::Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> Option<Input> {
        Input::from_key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn keys_become_inputs_and_ctrl_chords_are_told_apart_from_text() {
        assert_eq!(
            press(KeyCode::Char('a'), KeyModifiers::NONE),
            Some(Input::Char('a'))
        );
        assert_eq!(
            press(KeyCode::Char('A'), KeyModifiers::SHIFT),
            Some(Input::Char('A'))
        );
        assert_eq!(
            press(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(Input::Ctrl('c'))
        );
        assert_eq!(
            press(KeyCode::Char('S'), KeyModifiers::CONTROL),
            Some(Input::Ctrl('s'))
        );
        assert_eq!(press(KeyCode::Down, KeyModifiers::NONE), Some(Input::Down));
        assert_eq!(press(KeyCode::Esc, KeyModifiers::NONE), Some(Input::Esc));
        assert_eq!(press(KeyCode::F(5), KeyModifiers::NONE), None);
    }

    #[test]
    fn only_presses_count_not_releases() {
        let mut release = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;

        assert_eq!(Input::from_key(release), None);
    }

    #[test]
    fn the_window_follows_the_cursor_and_stays_put_while_it_is_inside() {
        // Ten rows visible.
        assert_eq!(follow(0, 3, 10), 0);
        assert_eq!(follow(0, 9, 10), 0);
        assert_eq!(follow(0, 10, 10), 1, "one past the bottom scrolls one row");
        assert_eq!(follow(5, 14, 10), 5);
        assert_eq!(follow(5, 4, 10), 4, "above the top scrolls up to it");
        assert_eq!(follow(5, 40, 10), 31);
        assert_eq!(follow(7, 7, 0), 0, "no room, no scroll");
    }

    fn buffer(text: &str) -> TextBuffer {
        TextBuffer::from_text(text)
    }

    #[test]
    fn text_goes_in_and_out_unchanged_apart_from_crlf() {
        for text in ["", "a", "a\n", "a\n\nb\n", "h\u{e9}llo\nw\u{f6}rld"] {
            assert_eq!(buffer(text).to_text(), text, "{text:?}");
        }
        assert_eq!(buffer("a\r\nb\r\n").to_text(), "a\nb\n");
        assert_eq!(buffer("").lines(), [""], "an empty text has one empty line");
    }

    #[test]
    fn typing_inserts_at_the_cursor_and_moves_it() {
        let mut text = buffer("ac");
        text.right();

        assert!(text.insert('b'));

        assert_eq!(text.to_text(), "abc");
        assert_eq!((text.row(), text.col()), (0, 2));
    }

    #[test]
    fn a_control_character_is_not_text() {
        let mut text = buffer("ab");

        assert!(!text.insert('\u{7}'));
        assert!(!text.insert('\t'));

        assert_eq!(text.to_text(), "ab");
        assert_eq!(text.col(), 0);
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        let mut text = buffer("\u{e9}\u{e9}");
        text.right();

        text.insert('x');
        assert_eq!(text.to_text(), "\u{e9}x\u{e9}");
        assert_eq!(text.col(), 2);

        assert!(text.backspace());
        assert!(text.backspace());
        assert_eq!(text.to_text(), "\u{e9}");
    }

    #[test]
    fn enter_splits_the_line_at_the_cursor() {
        let mut text = buffer("abcd");
        text.right();
        text.right();

        text.newline();

        assert_eq!(text.lines(), ["ab", "cd"]);
        assert_eq!((text.row(), text.col()), (1, 0));
    }

    #[test]
    fn backspace_at_the_start_of_a_line_joins_it_to_the_one_above() {
        let mut text = buffer("ab\ncd");
        text.vertical(1);

        assert!(text.backspace());

        assert_eq!(text.lines(), ["abcd"]);
        assert_eq!(
            (text.row(), text.col()),
            (0, 2),
            "the cursor is at the join"
        );
    }

    #[test]
    fn backspace_at_the_very_start_does_nothing() {
        let mut text = buffer("ab");

        assert!(!text.backspace());

        assert_eq!(text.to_text(), "ab");
    }

    #[test]
    fn delete_removes_the_character_at_the_cursor_or_joins_the_next_line() {
        let mut text = buffer("ab\ncd");

        assert!(text.delete());
        assert_eq!(text.lines(), ["b", "cd"]);
        text.end();
        assert!(text.delete());
        assert_eq!(text.lines(), ["bcd"]);
        text.end();
        assert!(!text.delete(), "nothing after the last character");
    }

    #[test]
    fn the_arrows_cross_line_ends_and_stop_at_the_text_ends() {
        let mut text = buffer("ab\nc");

        text.left();
        assert_eq!((text.row(), text.col()), (0, 0));
        text.right();
        text.right();
        text.right();
        assert_eq!(
            (text.row(), text.col()),
            (1, 0),
            "right at a line end wraps"
        );
        text.left();
        assert_eq!(
            (text.row(), text.col()),
            (0, 2),
            "left at a line start wraps"
        );
        text.vertical(5);
        text.end();
        text.right();
        assert_eq!((text.row(), text.col()), (1, 1));
    }

    #[test]
    fn moving_down_to_a_shorter_line_puts_the_cursor_at_its_end() {
        let mut text = buffer("abcdef\nxy\nlonger line");
        text.end();

        text.vertical(1);
        assert_eq!(text.col(), 2);
        text.vertical(1);
        assert_eq!(text.col(), 2, "the column is not remembered");
        text.vertical(-9);
        assert_eq!(text.row(), 0);
        text.vertical(9);
        assert_eq!(text.row(), 2);
    }

    #[test]
    fn a_centered_rectangle_is_in_the_middle_and_never_bigger_than_the_area() {
        let area = Rect::new(10, 5, 100, 40);

        assert_eq!(centered_size(50, 20, area), Rect::new(35, 15, 50, 20));
        assert_eq!(centered_size(500, 90, area), area);
    }

    #[test]
    fn clipping_cuts_at_the_width_in_characters() {
        assert_eq!(clip("abcdef", 3), "abc");
        assert_eq!(clip("ab", 3), "ab");
        assert_eq!(clip("h\u{e9}llo", 2), "h\u{e9}");
    }

    /// A counter that asks for a doubling and quits at 8.
    #[derive(Default)]
    struct Doubler {
        value: u32,
        done: bool,
    }

    enum Msg {
        Key(Input),
        Doubled(u32),
    }

    impl From<Input> for Msg {
        fn from(input: Input) -> Self {
            Self::Key(input)
        }
    }

    impl Machine for Doubler {
        type Msg = Msg;
        type Effect = u32;

        fn update(&mut self, msg: Msg) -> Vec<u32> {
            match msg {
                Msg::Key(Input::Char('1')) => {
                    self.value = 1;
                    vec![self.value]
                }
                Msg::Key(_) => Vec::new(),
                Msg::Doubled(value) => {
                    self.value = value;
                    self.done = value >= 8;
                    vec![value]
                }
            }
        }

        fn resize(&mut self, _: u16, _: u16) {}

        fn view(&self, _: &mut Frame) {}

        fn finished(&self) -> bool {
            self.done
        }
    }

    #[test]
    fn the_results_of_effects_are_fed_back_until_the_app_is_done() {
        let mut machine = Doubler::default();
        let mut performed = Vec::new();
        let mut redraws = 0;

        settle(
            &mut machine,
            Msg::Key(Input::Char('1')),
            &mut |effect| {
                performed.push(effect);
                Some(Msg::Doubled(effect * 2))
            },
            |_| redraws += 1,
        );

        // 1 is performed first; its result 2 is fed back, then 4, then 8, which
        // finishes the app before its effect runs.
        assert_eq!(performed, [1, 2, 4]);
        assert_eq!(machine.value, 8);
        assert!(machine.finished());
        assert_eq!(
            redraws, 3,
            "the screen is redrawn before each batch of effects"
        );
    }

    #[test]
    fn a_message_with_no_effects_changes_state_and_nothing_else() {
        let mut machine = Doubler::default();
        let mut performed = 0;

        settle(
            &mut machine,
            Msg::Key(Input::Char('x')),
            &mut |_| {
                performed += 1;
                None
            },
            |_| panic!("nothing to redraw for"),
        );

        assert_eq!(performed, 0);
        assert_eq!(machine.value, 0);
    }

    #[test]
    fn an_effect_with_no_result_ends_the_chain() {
        let mut machine = Doubler::default();

        settle(
            &mut machine,
            Msg::Key(Input::Char('1')),
            &mut |_| None,
            |_| {},
        );

        assert_eq!(machine.value, 1);
        assert!(!machine.finished());
    }

    #[test]
    fn a_snapshot_is_the_text_of_the_buffer_row_by_row() {
        let mut buffer = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 6, 3));
        buffer.set_string(0, 0, "ab", ratatui::style::Style::default());
        buffer.set_string(2, 2, "cd", ratatui::style::Style::default());

        assert_eq!(snapshot(&buffer), "ab\n\n  cd");
    }
}
