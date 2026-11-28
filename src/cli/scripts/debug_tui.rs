//! Colorful Ratatui breakpoint debugger for interactive pauses (`sbrs record` / `--pdb`).
//!
//! Renders the breakpoint header, live page URL/title, source listing around the
//! paused line, command output log, and an interactive command bar inside the
//! alternate screen using [`tui_kit`].

use std::io;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::common::debug_prompt::PromptOutcome;

use super::tui_kit::{Input, Machine};

/// Messages handled by [`DebugTui`].
#[derive(Debug, Clone)]
pub(crate) enum DebugMsg {
    Key(Input),
    Output(String),
    PageSummary { url: String, title: String },
}

impl From<Input> for DebugMsg {
    fn from(key: Input) -> Self {
        Self::Key(key)
    }
}

/// Side effects requested by [`DebugTui`] for the live browser page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DebugEffect {
    RunCommand(String),
    RefreshPage,
}

/// Full-screen colorful debugger state machine.
#[derive(Debug)]
pub(crate) struct DebugTui {
    title: String,
    test_id: String,
    message: String,
    source_lines: Vec<(usize, String, bool)>,
    url: String,
    page_title: String,
    input: String,
    log: Vec<String>,
    outcome: Option<PromptOutcome>,
}

impl DebugTui {
    /// Creates a debugger TUI pre-populated with context lines and header text.
    #[must_use]
    pub(crate) fn new(
        title: impl Into<String>,
        test_id: impl Into<String>,
        message: impl Into<String>,
        source_lines: Vec<(usize, String, bool)>,
        url: impl Into<String>,
        page_title: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            test_id: test_id.into(),
            message: message.into(),
            source_lines,
            url: url.into(),
            page_title: page_title.into(),
            input: String::new(),
            log: vec![String::from(
                "Commands: c continue | s screenshot | src source | u url | js <expr> | find <css> | q quit",
            )],
            outcome: None,
        }
    }

    /// Outcome once the user leaves the debugger.
    #[must_use]
    pub(crate) fn outcome(&self) -> PromptOutcome {
        self.outcome.unwrap_or(PromptOutcome::Continue)
    }

    fn push_log(&mut self, text: impl Into<String>) {
        for line in text.into().lines() {
            self.log.push(line.to_owned());
        }
        if self.log.len() > 200 {
            let drain = self.log.len() - 200;
            self.log.drain(0..drain);
        }
    }
}

impl Machine for DebugTui {
    type Msg = DebugMsg;
    type Effect = DebugEffect;

    fn update(&mut self, msg: Self::Msg) -> Vec<Self::Effect> {
        match msg {
            DebugMsg::Output(text) => {
                self.push_log(text);
                Vec::new()
            }
            DebugMsg::PageSummary { url, title } => {
                self.url = url;
                self.page_title = title;
                Vec::new()
            }
            DebugMsg::Key(input) => match input {
                Input::Ctrl('c') | Input::Esc => {
                    self.outcome = Some(PromptOutcome::Quit);
                    Vec::new()
                }
                Input::Backspace => {
                    self.input.pop();
                    Vec::new()
                }
                Input::Enter => {
                    let cmd = self.input.trim().to_owned();
                    self.input.clear();
                    match cmd.as_str() {
                        "" => Vec::new(),
                        "c" | "continue" => {
                            self.outcome = Some(PromptOutcome::Continue);
                            Vec::new()
                        }
                        "q" | "quit" => {
                            self.outcome = Some(PromptOutcome::Quit);
                            Vec::new()
                        }
                        "r" | "retry" => {
                            self.outcome = Some(PromptOutcome::Retry);
                            Vec::new()
                        }
                        "u" | "url" => {
                            self.push_log(format!("> {cmd}"));
                            vec![DebugEffect::RefreshPage, DebugEffect::RunCommand(cmd)]
                        }
                        _ => {
                            self.push_log(format!("> {cmd}"));
                            vec![DebugEffect::RunCommand(cmd)]
                        }
                    }
                }
                Input::Char(c) if !c.is_control() => {
                    self.input.push(c);
                    Vec::new()
                }
                _ => Vec::new(),
            },
        }
    }

    fn resize(&mut self, _width: u16, _height: u16) {}

    fn view(&self, frame: &mut Frame) {
        let areas = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(7),
            Constraint::Min(7),
            Constraint::Length(3),
        ])
        .split(frame.area());

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    format!(" {} ", self.title),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(&self.test_id, Style::default().fg(Color::Yellow)),
                Span::raw(" - "),
                Span::styled(&self.message, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled(
                    " URL: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&self.url, Style::default().fg(Color::Cyan)),
                Span::raw("  |  "),
                Span::styled(&self.page_title, Style::default().fg(Color::Gray)),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        frame.render_widget(header, areas[0]);

        let code_lines: Vec<Line> = if self.source_lines.is_empty() {
            vec![Line::styled(
                "  (no source location attached)",
                Style::default().fg(Color::DarkGray),
            )]
        } else {
            self.source_lines
                .iter()
                .map(|(num, text, active)| {
                    if *active {
                        Line::from(vec![
                            Span::styled(
                                format!("{num:>4} -> "),
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                text.as_str(),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ])
                    } else {
                        Line::from(vec![
                            Span::styled(
                                format!("{num:>4}    "),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(text.as_str(), Style::default().fg(Color::Gray)),
                        ])
                    }
                })
                .collect()
        };
        let code = Paragraph::new(code_lines).block(
            Block::default()
                .title(" Source ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
        frame.render_widget(code, areas[1]);

        let log_lines: Vec<Line> = self
            .log
            .iter()
            .rev()
            .take(areas[2].height.saturating_sub(2) as usize)
            .rev()
            .map(|l| {
                let style = if l.starts_with("> ") {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Line::styled(l.as_str(), style)
            })
            .collect();
        let output = Paragraph::new(log_lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(" Console Output ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        );
        frame.render_widget(output, areas[2]);

        let prompt = Paragraph::new(Line::from(vec![
            Span::styled(
                "(sbrs-debug) ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&self.input),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        );
        frame.render_widget(prompt, areas[3]);
    }

    fn finished(&self) -> bool {
        self.outcome.is_some()
    }
}

/// Runs [`DebugTui`] in the terminal with `perform` executing page commands cleanly.
///
/// # Errors
///
/// Returns `io::Error` if terminal initialization fails.
pub(crate) fn run_debug_tui(
    machine: &mut DebugTui,
    perform: impl FnMut(DebugEffect) -> Option<DebugMsg>,
) -> io::Result<PromptOutcome> {
    super::tui_kit::run(machine, perform)?;
    Ok(machine.outcome())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continue_and_quit_commands_resolve_outcome() {
        let mut tui = DebugTui::new(
            "BREAKPOINT",
            "test.rs",
            "Paused",
            vec![(10, String::from("sb.click(\"#ok\").await?;"), true)],
            "https://example.test",
            "Example",
        );
        tui.update(DebugMsg::Key(Input::Char('c')));
        tui.update(DebugMsg::Key(Input::Enter));
        assert!(tui.finished());
        assert_eq!(tui.outcome(), PromptOutcome::Continue);
    }

    #[test]
    fn render_draws_cleanly() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let tui = DebugTui::new(
            "RECORDER BREAKPOINT",
            "login.rs",
            "Inspect page",
            vec![(12, String::from("sb.open(url).await?;"), true)],
            "https://example.test",
            "Example",
        );
        let backend = TestBackend::new(90, 26);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| tui.view(f)).unwrap();
    }
}
