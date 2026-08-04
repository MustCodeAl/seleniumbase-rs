use std::{
    io::{self, stdout, Stdout},
    path::Path,
    process::{Command, Output},
    time::Duration,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
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

fn discover_tests(dir: &Path, kind: TestKind, out: &mut Vec<TestItem>) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                out.push(TestItem {
                    name: stem.to_owned(),
                    kind,
                });
            }
        }
    }
    Ok(())
}

fn run_test(item: &TestItem) -> io::Result<Popup> {
    let mut cmd = match item.kind {
        TestKind::Test => {
            let mut c = Command::new("cargo");
            c.args(["test", "--test", &item.name]);
            c
        }
        TestKind::Example => {
            let mut c = Command::new("cargo");
            c.args(["run", "--example", &item.name]);
            c
        }
    };
    let output = cmd.output()?;
    let popup = build_popup(item, &output);
    Ok(popup)
}

fn build_popup(item: &TestItem, output: &Output) -> Popup {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}").trim().to_owned();
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

fn handle_key(app: &mut App, key: event::KeyEvent) -> io::Result<bool> {
    if app.filter_mode {
        match key.code {
            KeyCode::Enter => app.filter_mode = false,
            KeyCode::Esc => {
                app.filter_mode = false;
                app.filter.clear();
                app.apply_filter();
            }
            KeyCode::Char(c) => {
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
                    app.popup = Some(run_test(item)?);
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
