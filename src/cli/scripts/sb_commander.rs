//! `sbrs commander`: a terminal menu that runs the project's tests and
//! examples with Cargo.

use std::{
    io::{self, stdout, Stdout},
    path::Path,
    process::{Command, Output},
    time::Duration,
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};

const POPUP_TAIL_LINES: usize = 20;
const TICK_RATE_MS: u64 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestKind {
    Test,
    Example,
}

#[derive(Debug, Clone)]
struct TestItem {
    name: String,
    kind: TestKind,
}

#[derive(Debug, Clone)]
struct Popup {
    title: String,
    content: String,
    success: bool,
}

struct App {
    items: Vec<TestItem>,
    filtered: Vec<usize>,
    selected: usize,
    filter: String,
    filter_mode: bool,
    popup: Option<Popup>,
}

impl App {
    fn new() -> io::Result<Self> {
        let mut items = Vec::new();
        discover_tests(Path::new("tests"), TestKind::Test, &mut items)?;
        discover_tests(Path::new("examples"), TestKind::Example, &mut items)?;
        let filtered: Vec<usize> = (0..items.len()).collect();
        Ok(Self {
            items,
            filtered,
            selected: 0,
            filter: String::new(),
            filter_mode: false,
            popup: None,
        })
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn apply_filter(&mut self) {
        let query = self.filter.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| query.is_empty() || item.name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    fn selected_item(&self) -> Option<&TestItem> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.items.get(i))
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len();
        let new = self.selected as isize + delta;
        self.selected = if new < 0 {
            len - 1
        } else if new >= len as isize {
            0
        } else {
            new as usize
        };
    }
}

/// Adds the `*.rs` files of `dir` to `out`, sorted by name: the order of
/// `read_dir` depends on the file system, and a menu that reshuffles itself on
/// every refresh is hard to use.
fn discover_tests(dir: &Path, kind: TestKind, out: &mut Vec<TestItem>) -> io::Result<()> {
    for path in super::discovery::rust_files(dir)? {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            out.push(TestItem {
                name: stem.to_owned(),
                kind,
            });
        }
    }
    Ok(())
}

/// Runs the item with Cargo and describes the outcome. Failing to start Cargo
/// at all (not installed, not on `PATH`) is an outcome too, shown in the popup
/// like a failed test, rather than an error that closes the menu.
fn run_test(item: &TestItem) -> Popup {
    run_test_with("cargo", item)
}

fn run_test_with(program: &str, item: &TestItem) -> Popup {
    let mut cmd = Command::new(program);
    match item.kind {
        TestKind::Test => cmd.args(["test", "--test", &item.name]),
        TestKind::Example => cmd.args(["run", "--example", &item.name]),
    };
    match cmd.output() {
        Ok(output) => build_popup(item, &output),
        Err(error) => Popup {
            title: format!("FAIL: {} (could not run `{program}`)", item.name),
            content: error.to_string(),
            success: false,
        },
    }
}

/// The popup for a finished run: the last [`POPUP_TAIL_LINES`] lines of the
/// output. Standard error comes first and standard output last, because Cargo
/// puts its build chatter on the first and the test results (the part worth
/// reading) on the second; the other way round, a few compiler warnings push
/// the failure out of the tail.
fn build_popup(item: &TestItem, output: &Output) -> Popup {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stderr}\n{stdout}").trim().to_owned();
    let content = if combined.is_empty() {
        "No output captured.".to_owned()
    } else {
        let lines: Vec<&str> = combined.lines().collect();
        lines
            .iter()
            .rev()
            .take(POPUP_TAIL_LINES)
            .copied()
            .rev()
            .collect::<Vec<_>>()
            .join("\n")
    };
    let success = output.status.success();
    let title = if success {
        format!("PASS: {}", item.name)
    } else {
        format!(
            "FAIL: {} (exit code: {:?})",
            item.name,
            output.status.code()
        )
    };
    Popup {
        title,
        content,
        success,
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

pub fn run_commander() -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let result = (|| -> io::Result<()> {
        let mut app = App::new()?;
        run_app(&mut terminal, &mut app)
    })();
    restore_terminal(&mut terminal)?;
    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<()> {
    let tick_rate = Duration::from_millis(TICK_RATE_MS);
    let mut last_tick = std::time::Instant::now();
    loop {
        terminal.draw(|f| draw(f, app))?;
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && handle_key(app, key)? {
                    return Ok(());
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
    }
}

/// Handles one key press; `true` means quit.
fn handle_key(app: &mut App, key: event::KeyEvent) -> io::Result<bool> {
    // In raw mode Ctrl-C is a key, not a signal. Without this the only way out
    // was `q`, and in the filter box Ctrl-C typed a `c`.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Ok(true);
    }
    if app.filter_mode {
        match key.code {
            KeyCode::Enter => app.filter_mode = false,
            KeyCode::Esc => {
                app.filter_mode = false;
                app.filter.clear();
                app.apply_filter();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.filter.push(c);
                app.apply_filter();
            }
            KeyCode::Backspace => {
                app.filter.pop();
                app.apply_filter();
            }
            _ => {}
        }
    } else if app.popup.is_some() {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('Q') => {
                app.popup = None;
            }
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
            KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
            KeyCode::Char('r') | KeyCode::Char('R') => {
                *app = App::new()?;
            }
            KeyCode::Char('/') | KeyCode::Char('f') => {
                app.filter_mode = true;
            }
            KeyCode::Enter => {
                if let Some(item) = app.selected_item() {
                    app.popup = Some(run_test(item));
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

fn draw(frame: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let header = Paragraph::new(Text::styled(
        "SeleniumBase Commander",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Test Runner"));
    frame.render_widget(header, chunks[0]);

    let filter_text = if app.filter_mode {
        format!("/{}", app.filter)
    } else if app.filter.is_empty() {
        String::new()
    } else {
        format!("filter: {}", app.filter)
    };
    let filter_para = Paragraph::new(filter_text).style(Style::default().fg(Color::Yellow));
    frame.render_widget(filter_para, chunks[2]);

    let hints = if app.filter_mode {
        "Enter=apply  Esc=clear  Backspace=delete"
    } else if app.popup.is_some() {
        "Esc/Enter/q=close popup"
    } else if app.is_empty() {
        "q=quit  r=refresh"
    } else {
        "q=quit  ↑↓/jk=navigate  Enter=run  r=refresh  /=filter"
    };
    let footer = Paragraph::new(hints).style(Style::default().fg(Color::Gray));
    frame.render_widget(footer, chunks[3]);

    let list_area = chunks[1];
    if app.is_empty() {
        let empty = Paragraph::new(
            "No tests found.\n\nCreate a tests/ directory with *.rs files, \
             or add examples/*.rs files.",
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Tests"));
        frame.render_widget(empty, list_area);
    } else if app.filtered.is_empty() {
        let no_match = Paragraph::new("No tests match the current filter.")
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Tests"));
        frame.render_widget(no_match, list_area);
    } else {
        let items: Vec<ListItem> = app
            .filtered
            .iter()
            .map(|&idx| {
                let item = &app.items[idx];
                let kind_label = match item.kind {
                    TestKind::Test => "[test]",
                    TestKind::Example => "[example]",
                };
                let content = format!(" {kind_label} {}", item.name);
                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Tests"))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );

        let mut state = ListState::default().with_selected(Some(app.selected));
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    if let Some(popup) = &app.popup {
        let area = centered_rect(80, 70, frame.area());
        let popup_block = Block::default()
            .title(popup.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if popup.success {
                Color::Green
            } else {
                Color::Red
            }));
        let popup_para = Paragraph::new(popup.content.as_str())
            .block(popup_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(Clear, area);
        frame.render_widget(popup_para, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn item(name: &str) -> TestItem {
        TestItem {
            name: name.to_owned(),
            kind: TestKind::Test,
        }
    }

    fn app(names: &[&str]) -> App {
        let items: Vec<TestItem> = names.iter().map(|name| item(name)).collect();
        App {
            filtered: (0..items.len()).collect(),
            items,
            selected: 0,
            filter: String::new(),
            filter_mode: false,
            popup: None,
        }
    }

    fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
        handle_key(app, KeyEvent::new(code, modifiers)).expect("handling a key cannot fail")
    }

    #[test]
    fn ctrl_c_quits_in_every_mode() {
        let ctrl_c = (KeyCode::Char('c'), KeyModifiers::CONTROL);

        let mut browsing = app(&["a"]);
        assert!(press(&mut browsing, ctrl_c.0, ctrl_c.1));

        let mut filtering = app(&["a"]);
        filtering.filter_mode = true;
        assert!(press(&mut filtering, ctrl_c.0, ctrl_c.1));
        assert!(filtering.filter.is_empty(), "no 'c' typed into the filter");

        let mut reading = app(&["a"]);
        reading.popup = Some(Popup {
            title: String::new(),
            content: String::new(),
            success: true,
        });
        assert!(press(&mut reading, ctrl_c.0, ctrl_c.1));
    }

    #[test]
    fn plain_keys_still_work_and_a_plain_c_is_just_text() {
        let mut filtering = app(&["calc", "login"]);
        filtering.filter_mode = true;

        assert!(!press(
            &mut filtering,
            KeyCode::Char('c'),
            KeyModifiers::NONE
        ));
        assert_eq!(filtering.filter, "c");
        assert_eq!(filtering.filtered, [0]);
        // Other Ctrl chords are not text either.
        press(&mut filtering, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(filtering.filter, "c");

        let mut browsing = app(&["a"]);
        assert!(press(&mut browsing, KeyCode::Char('q'), KeyModifiers::NONE));
    }

    #[test]
    fn the_selection_wraps_and_a_filter_keeps_it_in_range() {
        let mut menu = app(&["a", "b", "c"]);
        press(&mut menu, KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(menu.selected, 2, "up from the top wraps to the bottom");
        press(&mut menu, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(menu.selected, 0);

        menu.selected = 2;
        menu.filter = "a".into();
        menu.apply_filter();
        assert_eq!(menu.filtered, [0]);
        assert_eq!(
            menu.selected, 0,
            "the cursor cannot point past the filtered list"
        );
        menu.filter = "zzz".into();
        menu.apply_filter();
        assert!(menu.selected_item().is_none());
    }

    #[test]
    fn discovered_tests_are_sorted_and_only_rust_files() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["zeta.rs", "alpha.rs", "mid.rs", "notes.txt"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub.rs")).unwrap();
        let mut items = vec![item("earlier")];

        discover_tests(dir.path(), TestKind::Example, &mut items).unwrap();

        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["earlier", "alpha", "mid", "zeta"]);
        assert!(items[1..].iter().all(|i| i.kind == TestKind::Example));
    }

    #[cfg(unix)]
    fn finished(code: i32, stdout: &str, stderr: &str) -> Output {
        use std::os::unix::process::ExitStatusExt;
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_popup_ends_with_the_test_results_not_with_cargo_chatter() {
        // Thirty compiler warnings on stderr, the verdict on stdout.
        let warnings = (1..=30)
            .map(|n| format!("warning: unused variable `v{n}`"))
            .collect::<Vec<_>>()
            .join("\n");
        let verdict = "test login ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed";

        let popup = build_popup(&item("login"), &finished(101, verdict, &warnings));

        assert!(popup.content.ends_with("1 failed"), "{}", popup.content);
        assert!(
            popup.content.contains("test login ... FAILED"),
            "{}",
            popup.content
        );
        assert!(!popup.success);
        assert_eq!(popup.title, "FAIL: login (exit code: Some(101))");
        assert!(popup.content.lines().count() <= POPUP_TAIL_LINES);
    }

    #[cfg(unix)]
    #[test]
    fn a_passing_run_is_green_and_an_empty_one_says_so() {
        let popup = build_popup(&item("ok"), &finished(0, "test result: ok", ""));
        assert!(popup.success);
        assert_eq!(popup.title, "PASS: ok");

        let silent = build_popup(&item("quiet"), &finished(0, "", ""));
        assert_eq!(silent.content, "No output captured.");
    }

    #[test]
    fn a_missing_cargo_is_reported_in_the_popup_not_as_an_error() {
        let popup = run_test_with("sbase-no-such-program-7f3a", &item("login"));

        assert!(!popup.success);
        assert!(popup.title.contains("could not run"), "{}", popup.title);
        assert!(!popup.content.is_empty());
    }
}
