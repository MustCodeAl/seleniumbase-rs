//! Interactive TUI wizard (`sbrs-dev plugin new`) built on [`tui_kit`].
//!
//! Guides plugin authors through selecting template, backend, architectural
//! focus, integrations, headless default, dev environment, AI agent rules, IDE,
//! target execution environment, and destination folder before previewing and
//! writing the scaffolded plugin crate.

use std::path::PathBuf;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::cli::plugin_scaffold::{
    render_plugin_files, scaffold_plugin, PluginScaffoldError, PluginScaffoldOptions,
    ScaffoldResult,
};

use super::tui_kit::{run, Input, Machine};

const TEMPLATES: &[&str] = &["library", "cli"];
const BACKENDS: &[Option<&str>] = &[None, Some("cdp"), Some("webdriver")];
const FOCUSES: &[&str] = &["general", "cli", "scraping"];
const INTEGRATIONS: &[Option<&str>] = &[None, Some("mcp"), Some("ci"), Some("mcp,ci")];
const DEV_ENVS: &[&str] = &["cargo", "mise", "docker", "gitlab-ci", "github-actions"];
const AI_AGENTS: &[&str] = &[
    "none",
    "cursor",
    "claude-code",
    "gemini",
    "antigravity",
    "codex",
];
const IDES: &[&str] = &["cursor", "vscode", "zed", "clion", "neovim", "helix"];
const TARGET_ENVS: &[&str] = &["local", "docker", "cloud"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Name,
    Template,
    Backend,
    Focus,
    Integrations,
    Headless,
    DevEnv,
    AiAgent,
    Ide,
    TargetEnv,
    OutDir,
    Create,
}

const FIELDS: &[FieldKind] = &[
    FieldKind::Name,
    FieldKind::Template,
    FieldKind::Backend,
    FieldKind::Focus,
    FieldKind::Integrations,
    FieldKind::Headless,
    FieldKind::DevEnv,
    FieldKind::AiAgent,
    FieldKind::Ide,
    FieldKind::TargetEnv,
    FieldKind::OutDir,
    FieldKind::Create,
];

/// Messages accepted by [`PluginWizard`].
#[derive(Debug, Clone)]
pub(crate) enum WizardMsg {
    Key(Input),
    ScaffoldDone(Result<ScaffoldResult, String>),
}

impl From<Input> for WizardMsg {
    fn from(key: Input) -> Self {
        Self::Key(key)
    }
}

/// Side effects emitted by [`PluginWizard`].
#[derive(Debug, Clone)]
pub(crate) enum WizardEffect {
    Scaffold(PluginScaffoldOptions),
}

/// Interactive state machine for `sbrs-dev plugin new`.
#[derive(Debug)]
pub(crate) struct PluginWizard {
    opts: PluginScaffoldOptions,
    out_dir_text: String,
    cursor: usize,
    editing_text: bool,
    status: String,
    error: Option<String>,
    result: Option<ScaffoldResult>,
    cancelled: bool,
    size: (u16, u16),
}

impl PluginWizard {
    /// Creates a new wizard pre-seeded with CLI defaults/flags.
    #[must_use]
    pub(crate) fn new(opts: PluginScaffoldOptions) -> Self {
        let out_dir_text = opts
            .out_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let initial_cursor = if opts.name.trim().is_empty() {
            0
        } else {
            FIELDS.len() - 1
        };
        let editing_text = initial_cursor == 0;
        Self {
            opts,
            out_dir_text,
            cursor: initial_cursor,
            editing_text,
            status: String::from(
                "Arrows/j/k navigate · Space/Left/Right cycle · Enter edit/create · q quit",
            ),
            error: None,
            result: None,
            cancelled: false,
            size: (100, 30),
        }
    }

    fn current_field(&self) -> FieldKind {
        FIELDS[self.cursor]
    }

    fn cycle_field(&mut self, forward: bool) {
        let step = |idx: usize, len: usize| -> usize {
            if forward {
                (idx + 1) % len
            } else {
                (idx + len - 1) % len
            }
        };
        match self.current_field() {
            FieldKind::Template => {
                let idx = TEMPLATES
                    .iter()
                    .position(|&v| v == self.opts.template)
                    .unwrap_or(0);
                self.opts.template = TEMPLATES[step(idx, TEMPLATES.len())].to_owned();
            }
            FieldKind::Backend => {
                let cur = self.opts.backend.as_deref();
                let idx = BACKENDS.iter().position(|&v| v == cur).unwrap_or(0);
                self.opts.backend = BACKENDS[step(idx, BACKENDS.len())].map(str::to_owned);
            }
            FieldKind::Focus => {
                let idx = FOCUSES
                    .iter()
                    .position(|&v| v == self.opts.focus)
                    .unwrap_or(0);
                self.opts.focus = FOCUSES[step(idx, FOCUSES.len())].to_owned();
            }
            FieldKind::Integrations => {
                let cur = self.opts.integrations.as_deref();
                let idx = INTEGRATIONS.iter().position(|&v| v == cur).unwrap_or(0);
                self.opts.integrations =
                    INTEGRATIONS[step(idx, INTEGRATIONS.len())].map(str::to_owned);
            }
            FieldKind::Headless => self.opts.headless = !self.opts.headless,
            FieldKind::DevEnv => {
                let idx = DEV_ENVS
                    .iter()
                    .position(|&v| v == self.opts.dev_env)
                    .unwrap_or(0);
                self.opts.dev_env = DEV_ENVS[step(idx, DEV_ENVS.len())].to_owned();
            }
            FieldKind::AiAgent => {
                let idx = AI_AGENTS
                    .iter()
                    .position(|&v| v == self.opts.ai_agent)
                    .unwrap_or(0);
                self.opts.ai_agent = AI_AGENTS[step(idx, AI_AGENTS.len())].to_owned();
            }
            FieldKind::Ide => {
                let idx = IDES.iter().position(|&v| v == self.opts.ide).unwrap_or(0);
                self.opts.ide = IDES[step(idx, IDES.len())].to_owned();
            }
            FieldKind::TargetEnv => {
                let idx = TARGET_ENVS
                    .iter()
                    .position(|&v| v == self.opts.target_env)
                    .unwrap_or(0);
                self.opts.target_env = TARGET_ENVS[step(idx, TARGET_ENVS.len())].to_owned();
            }
            FieldKind::Name | FieldKind::OutDir | FieldKind::Create => {}
        }
    }

    fn effective_options(&self) -> PluginScaffoldOptions {
        let mut opts = self.opts.clone();
        opts.out_dir = if self.out_dir_text.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(self.out_dir_text.trim()))
        };
        opts
    }

    fn preview_paths(&self) -> Vec<String> {
        let mut opts = self.effective_options();
        if opts.name.trim().is_empty() {
            opts.name = String::from("my-plugin");
        }
        match render_plugin_files(&opts) {
            Ok((files, _)) => files
                .into_iter()
                .map(|(path, _)| path.display().to_string())
                .collect(),
            Err(err) => vec![format!("Preview error: {err}")],
        }
    }

    fn field_label_and_value(&self, field: FieldKind) -> (&'static str, String) {
        match field {
            FieldKind::Name => ("Plugin name", self.opts.name.clone()),
            FieldKind::Template => ("Template", self.opts.template.clone()),
            FieldKind::Backend => (
                "Backend",
                self.opts
                    .backend
                    .clone()
                    .unwrap_or_else(|| String::from("none")),
            ),
            FieldKind::Focus => ("Focus", self.opts.focus.clone()),
            FieldKind::Integrations => (
                "Integrations",
                self.opts
                    .integrations
                    .clone()
                    .unwrap_or_else(|| String::from("none")),
            ),
            FieldKind::Headless => (
                "Headless",
                if self.opts.headless { "yes" } else { "no" }.to_owned(),
            ),
            FieldKind::DevEnv => ("Dev environment", self.opts.dev_env.clone()),
            FieldKind::AiAgent => ("AI agent rules", self.opts.ai_agent.clone()),
            FieldKind::Ide => ("Primary IDE", self.opts.ide.clone()),
            FieldKind::TargetEnv => ("Target environment", self.opts.target_env.clone()),
            FieldKind::OutDir => (
                "Output directory",
                if self.out_dir_text.is_empty() {
                    String::from("(default: plugin name)")
                } else {
                    self.out_dir_text.clone()
                },
            ),
            FieldKind::Create => ("Action", String::from("[ Create Plugin Crate ]")),
        }
    }
}

impl Machine for PluginWizard {
    type Msg = WizardMsg;
    type Effect = WizardEffect;

    fn update(&mut self, msg: Self::Msg) -> Vec<Self::Effect> {
        match msg {
            WizardMsg::ScaffoldDone(Ok(res)) => {
                self.result = Some(res);
                self.error = None;
                Vec::new()
            }
            WizardMsg::ScaffoldDone(Err(err)) => {
                self.error = Some(err);
                Vec::new()
            }
            WizardMsg::Key(input) => {
                if self.editing_text {
                    match input {
                        Input::Esc | Input::Enter => {
                            self.editing_text = false;
                            if self.current_field() == FieldKind::Name && input == Input::Enter {
                                self.cursor = FIELDS.len() - 1;
                            }
                        }
                        Input::Tab => {
                            self.editing_text = false;
                            self.cursor = (self.cursor + 1) % FIELDS.len();
                        }
                        Input::Down => {
                            self.editing_text = false;
                            self.cursor = (self.cursor + 1) % FIELDS.len();
                        }
                        Input::Up => {
                            self.editing_text = false;
                            self.cursor = (self.cursor + FIELDS.len() - 1) % FIELDS.len();
                        }
                        Input::Backspace => match self.current_field() {
                            FieldKind::Name => {
                                self.opts.name.pop();
                            }
                            FieldKind::OutDir => {
                                self.out_dir_text.pop();
                            }
                            _ => {}
                        },
                        Input::Char(c) if !c.is_control() => match self.current_field() {
                            FieldKind::Name => self.opts.name.push(c),
                            FieldKind::OutDir => self.out_dir_text.push(c),
                            _ => {}
                        },
                        Input::Ctrl('c') => self.cancelled = true,
                        _ => {}
                    }
                    return Vec::new();
                }

                match input {
                    Input::Ctrl('c') | Input::Char('q') | Input::Esc => {
                        self.cancelled = true;
                        Vec::new()
                    }
                    Input::Down | Input::Char('j') | Input::Tab => {
                        self.cursor = (self.cursor + 1) % FIELDS.len();
                        Vec::new()
                    }
                    Input::Up | Input::Char('k') => {
                        self.cursor = (self.cursor + FIELDS.len() - 1) % FIELDS.len();
                        Vec::new()
                    }
                    Input::Right | Input::Char(' ') => {
                        match self.current_field() {
                            FieldKind::Name | FieldKind::OutDir => self.editing_text = true,
                            FieldKind::Create => {
                                return vec![WizardEffect::Scaffold(self.effective_options())];
                            }
                            _ => self.cycle_field(true),
                        }
                        Vec::new()
                    }
                    Input::Left => {
                        self.cycle_field(false);
                        Vec::new()
                    }
                    Input::Enter => match self.current_field() {
                        FieldKind::Name | FieldKind::OutDir => {
                            self.editing_text = true;
                            Vec::new()
                        }
                        FieldKind::Create => {
                            vec![WizardEffect::Scaffold(self.effective_options())]
                        }
                        _ => {
                            self.cycle_field(true);
                            Vec::new()
                        }
                    },
                    _ => Vec::new(),
                }
            }
        }
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.size = (width, height);
    }

    fn view(&self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                " sbrs-dev plugin new ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Interactive SeleniumBase Plugin Scaffolder"),
        ]))
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(header, layout[0]);

        let columns = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(layout[1]);

        render_fields(self, frame, columns[0]);
        render_preview(self, frame, columns[1]);

        let footer_line = if let Some(err) = &self.error {
            Line::styled(
                format!(" ERROR: {err}"),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else if self.editing_text {
            Line::styled(
                " Typing mode: Enter or Esc when done",
                Style::default().fg(Color::Yellow),
            )
        } else {
            Line::styled(
                format!(" {}", self.status),
                Style::default().fg(Color::DarkGray),
            )
        };
        let footer = Paragraph::new(footer_line).block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, layout[2]);
    }

    fn finished(&self) -> bool {
        self.cancelled || self.result.is_some()
    }
}

fn render_fields(wizard: &PluginWizard, frame: &mut Frame, area: Rect) {
    let items: Vec<ListItem> = FIELDS
        .iter()
        .enumerate()
        .map(|(idx, &field)| {
            let (label, val) = wizard.field_label_and_value(field);
            let selected = idx == wizard.cursor;
            let value_style = if field == FieldKind::Create {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if selected && wizard.editing_text {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let display_val = if field == FieldKind::Name && val.is_empty() {
                String::from("<enter plugin name>")
            } else {
                val
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{label:<18} "), Style::default().fg(Color::White)),
                Span::styled(display_val, value_style),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(wizard.cursor));

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Plugin Options ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_symbol("> ")
        .highlight_style(Style::default().bg(Color::DarkGray));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_preview(wizard: &PluginWizard, frame: &mut Frame, area: Rect) {
    let paths = wizard.preview_paths();
    let lines: Vec<Line> = paths
        .into_iter()
        .map(|p| Line::styled(format!("  {p}"), Style::default().fg(Color::Green)))
        .collect();
    let preview = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(" Generated Files Preview ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)),
    );
    frame.render_widget(preview, area);
}

/// Runs the interactive plugin scaffolding TUI and returns the generated crate
/// result if confirmed, or `None` when cancelled by the user.
///
/// # Errors
///
/// Returns [`PluginScaffoldError::Io`] if terminal setup or scaffolding fails.
pub fn run_plugin_scaffold_tui(
    initial: PluginScaffoldOptions,
) -> Result<Option<ScaffoldResult>, PluginScaffoldError> {
    let mut wizard = PluginWizard::new(initial);
    let io_res = run(&mut wizard, |effect| match effect {
        WizardEffect::Scaffold(opts) => {
            let outcome = scaffold_plugin(&opts).map_err(|e| e.to_string());
            Some(WizardMsg::ScaffoldDone(outcome))
        }
    });
    if let Err(err) = io_res {
        return Err(PluginScaffoldError::Io {
            path: PathBuf::from("<terminal>"),
            source: err,
        });
    }
    Ok(wizard.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_name_and_cycling_options_updates_state() {
        let mut wizard = PluginWizard::new(PluginScaffoldOptions::default());
        for ch in ['m', 'y', '-', 'p', 'l', 'u', 'g'] {
            wizard.update(WizardMsg::Key(Input::Char(ch)));
        }
        wizard.update(WizardMsg::Key(Input::Esc));
        assert_eq!(wizard.opts.name, "my-plug");

        // Move to Template and cycle to "cli"
        wizard.update(WizardMsg::Key(Input::Down));
        wizard.update(WizardMsg::Key(Input::Right));
        assert_eq!(wizard.opts.template, "cli");

        // Trigger scaffold effect from Create button
        wizard.cursor = FIELDS.len() - 1;
        let effects = wizard.update(WizardMsg::Key(Input::Enter));
        assert_eq!(effects.len(), 1);
        let WizardEffect::Scaffold(opts) = &effects[0];
        assert_eq!(opts.name, "my-plug");
        assert_eq!(opts.template, "cli");
    }

    #[test]
    fn quit_sets_finished() {
        let mut wizard = PluginWizard::new(PluginScaffoldOptions::new("demo"));
        wizard.update(WizardMsg::Key(Input::Char('q')));
        assert!(wizard.finished());
        assert!(wizard.result.is_none());
    }

    #[test]
    fn error_on_io_error_does_not_finish() {
        let mut wizard = PluginWizard::new(PluginScaffoldOptions::new("demo"));
        wizard.update(WizardMsg::ScaffoldDone(Err("bad dir".to_owned())));
        assert!(!wizard.finished());
        assert_eq!(wizard.error.as_deref(), Some("bad dir"));
    }

    #[test]
    fn render_runs_without_panicking_in_test_backend() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let wizard = PluginWizard::new(PluginScaffoldOptions::new("demo-plugin"));
        let backend = TestBackend::new(90, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| wizard.view(f)).unwrap();
    }
}
