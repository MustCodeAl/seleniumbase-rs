//! `sbrs behave-gui` as a terminal app (feature `tui`): the Python Behave
//! Commander without Tkinter.
//!
//! It lists the feature files under a folder with their scenarios, lets the
//! user mark some, runs them with `cargo test` and shows what
//! came of each scenario. What a run is, and what the runner can run, is in
//! [`super::feature_files`]; this module is the state machine ([`BehaveGui`])
//! and its drawing.

use std::io;
use std::path::{Path, PathBuf};

use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::feature_files::{self, FeatureFile, ScenarioRun, Verdict};
use super::tui_kit::{clip, follow, Input, Machine};

/// The screen size assumed until the terminal says otherwise.
const DEFAULT_SIZE: (u16, u16) = (80, 24);

/// Rows of the screen that are not the list: the header (3), the borders of
/// the list (2), and the input, status and hint rows (3).
const LIST_CHROME: u16 = 8;

/// Rows of the results screen that are not the list: the header (3), the
/// borders of the list (2), the details block (4) and the hint row (1).
const RESULTS_CHROME: u16 = 10;

/// A scenario in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ScenarioRow {
    line: usize,
    name: String,
    marked: bool,
}

/// A feature file in the list, with its scenarios.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FeatureRow {
    path: String,
    name: String,
    scenarios: Vec<ScenarioRow>,
}

/// What the app knows of a feature file: enough to list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeatureData {
    pub(crate) path: String,
    pub(crate) name: String,
    /// The line and name of each scenario.
    pub(crate) scenarios: Vec<(usize, String)>,
}

impl From<&FeatureFile> for FeatureData {
    fn from(feature: &FeatureFile) -> Self {
        Self {
            path: feature.path.clone(),
            name: feature.name.clone(),
            scenarios: feature
                .scenarios
                .iter()
                .map(|scenario| (scenario.line, scenario.name.clone()))
                .collect(),
        }
    }
}

/// A row of the list as shown: a feature, or one of its scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Feature(usize),
    Scenario(usize, usize),
}

/// What the keys are doing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Browse,
    Filter,
    /// Editing the tags; `before` is what to go back to on Esc.
    Tags {
        before: String,
    },
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

/// The results of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Results {
    runs: Vec<ScenarioRun>,
    cursor: usize,
    top: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    List,
    Results(Results),
}

/// What happens to the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Msg {
    Key(Input),
    /// A run is over.
    Finished(Vec<ScenarioRun>),
    /// The feature files were read again.
    Reloaded(Vec<FeatureData>),
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
    /// Run these scenarios: for each feature file (by index), the indexes of
    /// its scenarios, with the tags to run under.
    Run {
        chosen: Vec<(usize, Vec<usize>)>,
        tags: String,
    },
    /// Read the feature files again.
    Reload,
}

/// The Behave Commander.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BehaveGui {
    features: Vec<FeatureRow>,
    looked_in: String,
    cursor: usize,
    top: usize,
    filter: String,
    tags: String,
    mode: Mode,
    screen: Screen,
    status: Option<Status>,
    size: (u16, u16),
    done: bool,
}

impl BehaveGui {
    /// An app listing `features`; `looked_in` says where they were looked for.
    pub(crate) fn new(features: Vec<FeatureData>, looked_in: &str) -> Self {
        Self {
            features: rows_of(features, &[]),
            looked_in: looked_in.to_owned(),
            cursor: 0,
            top: 0,
            filter: String::new(),
            tags: String::new(),
            mode: Mode::Browse,
            screen: Screen::List,
            status: None,
            size: DEFAULT_SIZE,
            done: false,
        }
    }

    /// The rows the filter lets through: a feature that matches or has a
    /// scenario that matches, and the scenarios that match or belong to a
    /// feature that does.
    fn rows(&self) -> Vec<Row> {
        let query = self.filter.to_lowercase();
        let hit = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
        let mut rows = Vec::new();
        for (f, feature) in self.features.iter().enumerate() {
            let whole = hit(&feature.path) || hit(&feature.name);
            let scenarios: Vec<usize> = feature
                .scenarios
                .iter()
                .enumerate()
                .filter(|(_, scenario)| whole || hit(&scenario.name))
                .map(|(s, _)| s)
                .collect();
            if whole || !scenarios.is_empty() {
                rows.push(Row::Feature(f));
                rows.extend(scenarios.into_iter().map(|s| Row::Scenario(f, s)));
            }
        }
        rows
    }

    fn list_rows(&self) -> usize {
        usize::from(self.size.1.saturating_sub(LIST_CHROME))
    }

    fn results_rows(&self) -> usize {
        usize::from(self.size.1.saturating_sub(RESULTS_CHROME))
    }

    fn say(&mut self, level: Level, text: impl Into<String>) {
        self.status = Some(Status {
            level,
            text: text.into(),
        });
    }

    fn settle_cursor(&mut self) {
        let count = self.rows().len();
        self.cursor = self.cursor.min(count.saturating_sub(1));
        self.top = follow(self.top, self.cursor, self.list_rows());
    }

    /// Puts the cursor on row `row` of the shown list, or on its last row.
    fn go_to(&mut self, row: usize) {
        let count = self.rows().len();
        if count == 0 {
            return;
        }
        self.cursor = row.min(count - 1);
        self.top = follow(self.top, self.cursor, self.list_rows());
    }

    /// Moves one row, going round at either end.
    fn step(&mut self, forward: bool) {
        let count = self.rows().len();
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

    fn on_key(&mut self, input: Input) -> Vec<Effect> {
        // In raw mode Ctrl-C is a key, not a signal; in a text box it would
        // otherwise type a `c`.
        if input == Input::Ctrl('c') {
            self.done = true;
            return Vec::new();
        }
        if matches!(self.screen, Screen::Results(_)) {
            self.results_key(input);
            return Vec::new();
        }
        match self.mode {
            Mode::Filter => self.filter_key(input),
            Mode::Tags { .. } => self.tags_key(input),
            Mode::Browse => return self.list_key(input),
        }
        Vec::new()
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
            Input::Char(' ') => self.toggle(),
            Input::Char('a') => {
                for row in self.rows() {
                    if let Row::Scenario(f, s) = row {
                        self.features[f].scenarios[s].marked = true;
                    }
                }
            }
            Input::Char('n') => {
                for scenario in self.features.iter_mut().flat_map(|f| &mut f.scenarios) {
                    scenario.marked = false;
                }
            }
            Input::Enter => return self.run_selected(),
            Input::Char('t') => {
                self.mode = Mode::Tags {
                    before: self.tags.clone(),
                };
            }
            Input::Char('/' | 'f') => self.mode = Mode::Filter,
            Input::Char('l') => return vec![Effect::Reload],
            _ => {}
        }
        Vec::new()
    }

    /// Marks or unmarks the row under the cursor: a scenario by itself, a
    /// feature as all of its scenarios.
    fn toggle(&mut self) {
        match self.rows().get(self.cursor).copied() {
            Some(Row::Scenario(f, s)) => self.features[f].scenarios[s].marked ^= true,
            Some(Row::Feature(f)) => {
                let scenarios = &mut self.features[f].scenarios;
                let all = scenarios.iter().all(|scenario| scenario.marked);
                for scenario in scenarios {
                    scenario.marked = !all;
                }
            }
            None => {}
        }
    }

    /// Asks for the marked scenarios to be run, or all of them when none is
    /// marked, as the Python app does.
    fn run_selected(&mut self) -> Vec<Effect> {
        let total: usize = self.features.iter().map(|f| f.scenarios.len()).sum();
        if total == 0 {
            self.say(Level::Warning, "There are no scenarios to run.");
            return Vec::new();
        }
        let any_marked = self
            .features
            .iter()
            .any(|f| f.scenarios.iter().any(|s| s.marked));
        let chosen: Vec<(usize, Vec<usize>)> = self
            .features
            .iter()
            .enumerate()
            .filter_map(|(f, feature)| {
                let picked: Vec<usize> = feature
                    .scenarios
                    .iter()
                    .enumerate()
                    .filter(|(_, scenario)| !any_marked || scenario.marked)
                    .map(|(s, _)| s)
                    .collect();
                (!picked.is_empty()).then_some((f, picked))
            })
            .collect();
        let count: usize = chosen.iter().map(|(_, picked)| picked.len()).sum();
        let noun = if count == 1 { "scenario" } else { "scenarios" };
        self.say(Level::Info, format!("Running {count} {noun}..."));
        vec![Effect::Run {
            chosen,
            tags: self.tags.clone(),
        }]
    }

    fn filter_key(&mut self, input: Input) {
        match input {
            Input::Enter => self.mode = Mode::Browse,
            Input::Esc => {
                self.mode = Mode::Browse;
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

    fn tags_key(&mut self, input: Input) {
        match input {
            Input::Enter => self.mode = Mode::Browse,
            Input::Esc => {
                if let Mode::Tags { before } = std::mem::replace(&mut self.mode, Mode::Browse) {
                    self.tags = before;
                }
            }
            Input::Backspace => {
                self.tags.pop();
            }
            Input::Char(c) => self.tags.push(c),
            _ => {}
        }
    }

    fn results_key(&mut self, input: Input) {
        if matches!(input, Input::Esc | Input::Enter | Input::Char('q' | 'Q')) {
            self.screen = Screen::List;
            return;
        }
        let rows = self.results_rows();
        let page = rows.saturating_sub(1).max(1);
        let Screen::Results(results) = &mut self.screen else {
            return;
        };
        let count = results.runs.len();
        if count == 0 {
            return;
        }
        results.cursor = match input {
            Input::Down | Input::Char('j') => (results.cursor + 1) % count,
            Input::Up | Input::Char('k') => (results.cursor + count - 1) % count,
            Input::PageDown => results.cursor.saturating_add(page).min(count - 1),
            Input::PageUp => results.cursor.saturating_sub(page),
            Input::Home => 0,
            Input::End => count - 1,
            _ => results.cursor,
        };
        results.top = follow(results.top, results.cursor, rows);
    }
}

/// The rows for `features`, with the marks `earlier` rows had.
fn rows_of(features: Vec<FeatureData>, earlier: &[FeatureRow]) -> Vec<FeatureRow> {
    features
        .into_iter()
        .map(|feature| {
            let before = earlier.iter().find(|row| row.path == feature.path);
            let scenarios = feature
                .scenarios
                .into_iter()
                .map(|(line, name)| {
                    let marked = before.is_some_and(|row| {
                        row.scenarios
                            .iter()
                            .any(|s| s.marked && s.line == line && s.name == name)
                    });
                    ScenarioRow { line, name, marked }
                })
                .collect();
            FeatureRow {
                path: feature.path,
                name: feature.name,
                scenarios,
            }
        })
        .collect()
}

impl Machine for BehaveGui {
    type Msg = Msg;
    type Effect = Effect;

    fn update(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Key(input) => return self.on_key(input),
            Msg::Finished(runs) => {
                let totals = feature_files::totals(&runs);
                let level = if totals.failed > 0 {
                    Level::Error
                } else if totals.undefined > 0 {
                    Level::Warning
                } else {
                    Level::Info
                };
                let noun = if runs.len() == 1 {
                    "scenario"
                } else {
                    "scenarios"
                };
                self.say(
                    level,
                    format!(
                        "{} {noun}: {} passed, {} failed, {} undefined, {} skipped",
                        runs.len(),
                        totals.passed,
                        totals.failed,
                        totals.undefined,
                        totals.skipped
                    ),
                );
                self.screen = Screen::Results(Results {
                    runs,
                    cursor: 0,
                    top: 0,
                });
            }
            Msg::Reloaded(features) => {
                self.features = rows_of(features, &self.features);
                self.settle_cursor();
                let count = self.features.len();
                let noun = if count == 1 { "file" } else { "files" };
                self.say(Level::Info, format!("Read {count} feature {noun}"));
            }
            Msg::Failed(text) => self.say(Level::Error, text),
        }
        Vec::new()
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        self.settle_cursor();
        let rows = self.results_rows();
        if let Screen::Results(results) = &mut self.screen {
            results.top = follow(results.top, results.cursor, rows);
        }
    }

    fn view(&self, frame: &mut Frame) {
        match &self.screen {
            Screen::List => self.view_list(frame),
            Screen::Results(results) => self.view_results(frame, results),
        }
    }

    fn finished(&self) -> bool {
        self.done
    }
}

impl BehaveGui {
    /// The text of a row of the list, without its gutter.
    fn row_text(&self, row: Row) -> String {
        match row {
            Row::Feature(f) => {
                let feature = &self.features[f];
                let marked = feature.scenarios.iter().filter(|s| s.marked).count();
                let count = feature.scenarios.len();
                let mark = if marked == 0 {
                    "[ ]"
                } else if marked == count {
                    "[x]"
                } else {
                    "[-]"
                };
                let noun = if count == 1 { "scenario" } else { "scenarios" };
                format!(
                    "{mark} {} => {} ({count} {noun})",
                    feature.path, feature.name
                )
            }
            Row::Scenario(f, s) => {
                let scenario = &self.features[f].scenarios[s];
                format!(
                    "    {} line {}: {}",
                    if scenario.marked { "[x]" } else { "[ ]" },
                    scenario.line,
                    scenario.name
                )
            }
        }
    }

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

        let files = self.features.len();
        let scenarios: usize = self.features.iter().map(|f| f.scenarios.len()).sum();
        let marked: usize = self
            .features
            .iter()
            .map(|f| f.scenarios.iter().filter(|s| s.marked).count())
            .sum();
        let title = format!(
            "Features: {files} {}, {scenarios} {}, {marked} marked",
            if files == 1 { "file" } else { "files" },
            if scenarios == 1 {
                "scenario"
            } else {
                "scenarios"
            },
        );
        let block = Block::default().borders(Borders::ALL).title(title);
        let rows = self.rows();
        if self.features.is_empty() {
            let text = [
                "No feature files found.",
                "",
                &format!("Looked in: {}", self.looked_in),
                "Feature files are the *.feature files of a folder (default: features/),",
                "or pass a folder or a file: sbrs behave-gui <path>",
            ]
            .join("\n");
            frame.render_widget(
                Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .block(block),
                chunks[1],
            );
        } else if rows.is_empty() {
            frame.render_widget(
                Paragraph::new("Nothing matches the filter.")
                    .alignment(Alignment::Center)
                    .block(block),
                chunks[1],
            );
        } else {
            let items: Vec<ListItem> = rows
                .iter()
                .skip(self.top)
                .take(self.list_rows().max(1))
                .map(|&row| ListItem::new(self.row_text(row)))
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

        let input = match &self.mode {
            Mode::Filter => format!("/{}", self.filter),
            Mode::Tags { .. } => format!("Tags: {}", self.tags),
            Mode::Browse => {
                let mut parts = Vec::new();
                if !self.tags.is_empty() {
                    parts.push(format!("Tags: {}", self.tags));
                }
                if !self.filter.is_empty() {
                    parts.push(format!("Filter: {}", self.filter));
                }
                parts.join("  ")
            }
        };
        frame.render_widget(
            Paragraph::new(clip(&input, usize::from(chunks[2].width)))
                .style(Style::default().fg(Color::Yellow)),
            chunks[2],
        );
        if self.mode != Mode::Browse {
            let typed = u16::try_from(input.chars().count()).unwrap_or(u16::MAX);
            let x = (chunks[2].x + typed).min(chunks[2].right().saturating_sub(1));
            frame.set_cursor_position(Position::new(x, chunks[2].y));
        }
        if let Some(status) = &self.status {
            draw_status(frame, chunks[3], status);
        }
        let hints = match self.mode {
            Mode::Filter => "Enter keep  Esc clear  Backspace delete",
            Mode::Tags { .. } => "Enter accept  Esc cancel  e.g. @smoke,@web -@slow",
            Mode::Browse if self.features.is_empty() => "q quit  l reload",
            Mode::Browse => {
                "q quit  \u{2191}\u{2193} move  Space mark  a/n all/none  Enter run  t tags  / filter  l reload"
            }
        };
        draw_hints(frame, chunks[4], hints);
    }

    fn view_results(&self, frame: &mut Frame, results: &Results) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(frame.area());
        draw_header(frame, chunks[0]);

        let totals = feature_files::totals(&results.runs);
        let title = format!(
            "Results: {} passed, {} failed, {} undefined, {} skipped",
            totals.passed, totals.failed, totals.undefined, totals.skipped
        );
        let block = Block::default().borders(Borders::ALL).title(title);
        if results.runs.is_empty() {
            frame.render_widget(
                Paragraph::new("Nothing ran.")
                    .alignment(Alignment::Center)
                    .block(block),
                chunks[1],
            );
        } else {
            let items: Vec<ListItem> = results
                .runs
                .iter()
                .skip(results.top)
                .take(self.results_rows().max(1))
                .map(|run| {
                    let (label, color) = match run.verdict {
                        Verdict::Passed => ("PASS", Color::Green),
                        Verdict::Failed => ("FAIL", Color::Red),
                        Verdict::Undefined => ("UNDEF", Color::Yellow),
                        Verdict::Skipped => ("SKIP", Color::Gray),
                    };
                    ListItem::new(format!("{label:<5} {}:{} {}", run.file, run.line, run.name))
                        .style(Style::default().fg(color))
                })
                .collect();
            let list = List::new(items)
                .block(block)
                .highlight_symbol("> ")
                .highlight_spacing(HighlightSpacing::Always)
                .highlight_style(
                    Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
                );
            let mut state = ListState::default()
                .with_selected(Some(results.cursor.saturating_sub(results.top)));
            frame.render_stateful_widget(list, chunks[1], &mut state);
        }

        let details = Block::default().borders(Borders::ALL).title("Details");
        let inner = details.inner(chunks[2]);
        frame.render_widget(details, chunks[2]);
        if let Some(run) = results.runs.get(results.cursor) {
            let width = usize::from(inner.width);
            let text = format!(
                "{}\n{}",
                clip(&format!("{}:{} {}", run.file, run.line, run.name), width),
                clip(&run.messages.join(" | "), width)
            );
            frame.render_widget(Paragraph::new(text), inner);
        }
        draw_hints(
            frame,
            chunks[3],
            "Esc back  \u{2191}\u{2193} move  PgUp PgDn  Home End",
        );
    }
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(Text::styled(
        "SeleniumBase Behave Commander",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Behave"));
    frame.render_widget(header, area);
}

/// The status message, in the colour of its level.
fn draw_status(frame: &mut Frame, area: Rect, status: &Status) {
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

fn draw_hints(frame: &mut Frame, area: Rect, hints: &str) {
    frame.render_widget(
        Paragraph::new(clip(hints, usize::from(area.width)))
            .style(Style::default().fg(Color::Gray)),
        area,
    );
}

/// The shown form of a path: with `/` between folders, and without a leading
/// `./`.
fn shown(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").map_or(text.clone(), str::to_owned)
}

/// Reads the feature files under `target` into `loaded` and returns what the
/// app lists of them.
fn load(target: &Path, loaded: &mut Vec<FeatureFile>) -> io::Result<Vec<FeatureData>> {
    let mut features = Vec::new();
    for path in feature_files::find_feature_files(target)? {
        features.push(feature_files::load_feature(&path, &shown(&path))?);
    }
    *loaded = features;
    Ok(loaded.iter().map(FeatureData::from).collect())
}

/// What the effects of the app work on: the folder it looks in and the files
/// it has read.
pub(crate) struct Env {
    target: PathBuf,
    loaded: Vec<FeatureFile>,
}

impl Env {
    /// Carries out an effect and says what came of it.
    pub(crate) fn perform(&mut self, effect: Effect) -> Option<Msg> {
        Some(match effect {
            Effect::Run { chosen, tags } => {
                let filter = feature_files::parse_tags(&tags);
                let mut runs = Vec::new();
                for (file, scenarios) in chosen {
                    if let Some(feature) = self.loaded.get(file) {
                        runs.extend(feature_files::run_scenarios(feature, &scenarios, &filter));
                    }
                }
                Msg::Finished(runs)
            }
            Effect::Reload => match load(&self.target, &mut self.loaded) {
                Ok(features) => Msg::Reloaded(features),
                Err(error) => Msg::Failed(format!("cannot read the feature files: {error}")),
            },
        })
    }
}

/// Runs the Behave Commander over the feature files under `target` (a folder
/// or one `.feature` file) until the user quits.
///
/// # Errors
///
/// Returns the error of reading the files or of the terminal.
pub fn run(target: &Path) -> io::Result<()> {
    let mut loaded = Vec::new();
    let listed = load(target, &mut loaded)?;
    let mut app = BehaveGui::new(listed, &target.display().to_string());
    let mut env = Env {
        target: target.to_path_buf(),
        loaded,
    };
    super::tui_kit::run(&mut app, |effect| env.perform(effect))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::scripts::tui_kit::render;

    fn data(path: &str, name: &str, scenarios: &[(usize, &str)]) -> FeatureData {
        FeatureData {
            path: path.to_owned(),
            name: name.to_owned(),
            scenarios: scenarios
                .iter()
                .map(|(l, n)| (*l, (*n).to_owned()))
                .collect(),
        }
    }

    fn sample_data() -> Vec<FeatureData> {
        vec![
            data(
                "features/login.feature",
                "Log in",
                &[
                    (10, "valid login"),
                    (14, "wrong value"),
                    (26, "undefined step"),
                ],
            ),
            data("features/cart.feature", "Cart", &[(3, "add item")]),
        ]
    }

    fn sample() -> BehaveGui {
        let mut app = BehaveGui::new(sample_data(), "features");
        app.resize(80, 14);
        app
    }

    fn press(app: &mut BehaveGui, input: Input) -> Vec<Effect> {
        app.update(Msg::Key(input))
    }

    fn type_text(app: &mut BehaveGui, text: &str) {
        for c in text.chars() {
            press(app, Input::Char(c));
        }
    }

    fn status(app: &BehaveGui) -> (Level, &str) {
        let status = app.status.as_ref().expect("a status message");
        (status.level, status.text.as_str())
    }

    /// The marked scenarios as `(feature index, scenario index)`.
    fn marked(app: &BehaveGui) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        for (f, feature) in app.features.iter().enumerate() {
            for (s, scenario) in feature.scenarios.iter().enumerate() {
                if scenario.marked {
                    found.push((f, s));
                }
            }
        }
        found
    }

    fn run_of(name: &str, verdict: Verdict, messages: &[&str]) -> ScenarioRun {
        ScenarioRun {
            file: "features/login.feature".to_owned(),
            line: 10,
            name: name.to_owned(),
            verdict,
            messages: messages.iter().map(|m| (*m).to_owned()).collect(),
        }
    }

    // ------------------------------------------------------------------
    // The list
    // ------------------------------------------------------------------

    #[test]
    fn features_are_listed_with_their_scenarios_below_them() {
        let app = sample();

        assert_eq!(
            app.rows(),
            [
                Row::Feature(0),
                Row::Scenario(0, 0),
                Row::Scenario(0, 1),
                Row::Scenario(0, 2),
                Row::Feature(1),
                Row::Scenario(1, 0),
            ]
        );
    }

    #[test]
    fn the_cursor_wraps_round_the_rows() {
        let mut app = sample();

        press(&mut app, Input::Up);
        assert_eq!(app.cursor, 5);
        press(&mut app, Input::Down);
        assert_eq!(app.cursor, 0);
        press(&mut app, Input::Char('j'));
        press(&mut app, Input::Char('k'));
        press(&mut app, Input::Char('j'));
        assert_eq!(app.cursor, 1);
    }

    #[test]
    fn space_marks_a_scenario_and_on_a_feature_marks_all_of_its_scenarios() {
        let mut app = sample();

        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));
        assert_eq!(marked(&app), [(0, 0)]);

        // On the feature row: not all marked yet, so mark them all.
        press(&mut app, Input::Home);
        press(&mut app, Input::Char(' '));
        assert_eq!(marked(&app), [(0, 0), (0, 1), (0, 2)]);
        // All marked, so the next press clears them.
        press(&mut app, Input::Char(' '));
        assert!(marked(&app).is_empty());
    }

    #[test]
    fn a_marks_every_scenario_shown_and_n_clears_the_marks() {
        let mut app = sample();

        press(&mut app, Input::Char('/'));
        type_text(&mut app, "cart");
        press(&mut app, Input::Enter);
        press(&mut app, Input::Char('a'));
        assert_eq!(marked(&app), [(1, 0)], "only what the filter shows");

        press(&mut app, Input::Char('n'));
        assert!(marked(&app).is_empty());
    }

    #[test]
    fn the_filter_matches_scenario_names_feature_names_and_paths_ignoring_case() {
        let mut app = sample();

        press(&mut app, Input::Char('/'));
        type_text(&mut app, "WRONG");
        assert_eq!(
            app.rows(),
            [Row::Feature(0), Row::Scenario(0, 1)],
            "a matching scenario brings its feature row"
        );

        for _ in 0..5 {
            press(&mut app, Input::Backspace);
        }
        type_text(&mut app, "cart");
        assert_eq!(
            app.rows(),
            [Row::Feature(1), Row::Scenario(1, 0)],
            "a matching feature shows all of its scenarios"
        );

        for _ in 0..4 {
            press(&mut app, Input::Backspace);
        }
        type_text(&mut app, "login.feature");
        assert_eq!(app.rows().len(), 4);
    }

    #[test]
    fn esc_clears_the_filter_and_enter_keeps_it() {
        let mut app = sample();
        press(&mut app, Input::Char('/'));
        type_text(&mut app, "cart");
        press(&mut app, Input::Enter);
        assert_eq!(app.filter, "cart");
        assert_eq!(app.mode, Mode::Browse);

        press(&mut app, Input::Char('/'));
        press(&mut app, Input::Esc);

        assert!(app.filter.is_empty());
        assert_eq!(app.rows().len(), 6);
    }

    #[test]
    fn keys_typed_in_the_filter_and_the_tags_are_text_not_commands() {
        let mut app = sample();
        press(&mut app, Input::Char('/'));
        type_text(&mut app, "qant ");
        assert_eq!(app.filter, "qant ");
        assert!(!app.finished());

        press(&mut app, Input::Esc);
        press(&mut app, Input::Char('t'));
        type_text(&mut app, "qj ");
        assert_eq!(app.tags, "qj ");
        assert!(!app.finished());
    }

    #[test]
    fn ctrl_c_quits_from_every_mode_and_q_quits_from_the_list() {
        let keys: [&[Input]; 3] = [&[], &[Input::Char('/')], &[Input::Char('t')]];
        for setup in keys {
            let mut app = sample();
            for key in setup {
                press(&mut app, *key);
            }
            press(&mut app, Input::Ctrl('c'));
            assert!(app.finished(), "{setup:?}");
        }
        let mut app = sample();
        app.update(Msg::Finished(vec![run_of("a", Verdict::Passed, &[])]));
        press(&mut app, Input::Ctrl('c'));
        assert!(app.finished(), "from the results");

        let mut app = sample();
        press(&mut app, Input::Char('q'));
        assert!(app.finished());
    }

    // ------------------------------------------------------------------
    // Running
    // ------------------------------------------------------------------

    #[test]
    fn with_nothing_marked_enter_runs_every_scenario() {
        let mut app = sample();

        let effects = press(&mut app, Input::Enter);

        assert_eq!(
            effects,
            [Effect::Run {
                chosen: vec![(0, vec![0, 1, 2]), (1, vec![0])],
                tags: String::new(),
            }]
        );
        assert_eq!(status(&app), (Level::Info, "Running 4 scenarios..."));
    }

    #[test]
    fn with_some_marked_enter_runs_only_those() {
        let mut app = sample();
        press(&mut app, Input::Down);
        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));
        for _ in 0..3 {
            press(&mut app, Input::Down);
        }
        press(&mut app, Input::Char(' '));

        let effects = press(&mut app, Input::Enter);

        assert_eq!(
            effects,
            [Effect::Run {
                chosen: vec![(0, vec![1]), (1, vec![0])],
                tags: String::new(),
            }]
        );
        assert_eq!(status(&app), (Level::Info, "Running 2 scenarios..."));
    }

    #[test]
    fn one_scenario_is_not_pluralised() {
        let mut app = sample();
        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));

        press(&mut app, Input::Enter);

        assert_eq!(status(&app), (Level::Info, "Running 1 scenario..."));
    }

    #[test]
    fn no_scenarios_means_nothing_to_run() {
        let mut app = BehaveGui::new(vec![data("f.feature", "Empty", &[])], "features");

        let effects = press(&mut app, Input::Enter);

        assert!(effects.is_empty());
        assert_eq!(
            status(&app),
            (Level::Warning, "There are no scenarios to run.")
        );
    }

    #[test]
    fn the_tags_go_with_the_run_and_esc_puts_back_what_they_were() {
        let mut app = sample();
        press(&mut app, Input::Char('t'));
        assert!(matches!(app.mode, Mode::Tags { .. }));
        type_text(&mut app, "@smoke -@slow");
        press(&mut app, Input::Enter);
        assert_eq!(app.tags, "@smoke -@slow");

        let effects = press(&mut app, Input::Enter);
        assert!(
            matches!(&effects[..], [Effect::Run { tags, .. }] if tags == "@smoke -@slow"),
            "{effects:?}"
        );

        press(&mut app, Input::Esc); // back from the results
        press(&mut app, Input::Char('t'));
        type_text(&mut app, "more");
        press(&mut app, Input::Esc);
        assert_eq!(app.tags, "@smoke -@slow", "Esc reverts the edit");
    }

    #[test]
    fn a_finished_run_shows_its_results_and_totals() {
        let mut app = sample();
        press(&mut app, Input::Enter);

        app.update(Msg::Finished(vec![
            run_of("valid login", Verdict::Passed, &[]),
            run_of("wrong value", Verdict::Failed, &["the value is 5"]),
            run_of("undefined step", Verdict::Undefined, &["no handler"]),
            run_of("skipped one", Verdict::Skipped, &["tags"]),
        ]));

        assert!(matches!(app.screen, Screen::Results(_)));
        assert_eq!(
            status(&app),
            (
                Level::Error,
                "4 scenarios: 1 passed, 1 failed, 1 undefined, 1 skipped"
            )
        );
    }

    #[test]
    fn a_run_that_all_passed_is_info_and_one_with_only_undefined_steps_is_a_warning() {
        let mut app = sample();
        app.update(Msg::Finished(vec![run_of("a", Verdict::Passed, &[])]));
        assert_eq!(
            status(&app),
            (
                Level::Info,
                "1 scenario: 1 passed, 0 failed, 0 undefined, 0 skipped"
            )
        );

        app.update(Msg::Finished(vec![run_of("a", Verdict::Undefined, &["x"])]));
        assert_eq!(status(&app).0, Level::Warning);
    }

    #[test]
    fn the_results_are_browsed_and_esc_goes_back_to_the_list_with_its_marks() {
        let mut app = sample();
        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));
        app.update(Msg::Finished(vec![
            run_of("a", Verdict::Passed, &[]),
            run_of("b", Verdict::Failed, &["boom"]),
        ]));

        press(&mut app, Input::Down);
        press(&mut app, Input::Down);
        let Screen::Results(results) = &app.screen else {
            panic!("on the results");
        };
        assert_eq!(results.cursor, 0, "wrapped round two rows");

        press(&mut app, Input::Up);
        press(&mut app, Input::Esc);

        assert_eq!(app.screen, Screen::List);
        assert_eq!(marked(&app), [(0, 0)]);
        assert!(!app.finished());
    }

    #[test]
    fn q_on_the_results_goes_back_rather_than_quitting() {
        let mut app = sample();
        app.update(Msg::Finished(vec![run_of("a", Verdict::Passed, &[])]));

        press(&mut app, Input::Char('q'));

        assert_eq!(app.screen, Screen::List);
        assert!(!app.finished());
    }

    // ------------------------------------------------------------------
    // Reloading and failures
    // ------------------------------------------------------------------

    #[test]
    fn l_reads_the_files_again_and_keeps_the_marks_of_scenarios_still_there() {
        let mut app = sample();
        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));
        assert_eq!(press(&mut app, Input::Char('l')), [Effect::Reload]);

        app.update(Msg::Reloaded(vec![
            data(
                "features/login.feature",
                "Log in",
                &[(10, "valid login"), (30, "brand new")],
            ),
            data("features/other.feature", "Other", &[(1, "x")]),
        ]));

        assert_eq!(app.features.len(), 2);
        assert_eq!(marked(&app), [(0, 0)]);
        assert_eq!(app.features[0].scenarios[1].name, "brand new");
        assert!(app.cursor < app.rows().len());
    }

    #[test]
    fn a_failure_is_shown_as_an_error() {
        let mut app = sample();

        app.update(Msg::Failed("cannot read features".to_owned()));

        assert_eq!(status(&app), (Level::Error, "cannot read features"));
    }

    // ------------------------------------------------------------------
    // The effects, against a folder on disk
    // ------------------------------------------------------------------

    const CART: &str = "@shop\nFeature: Cart\n\
                        @smoke\nScenario: add item\nGiven I do nothing\nThen the value should be \"\"\n\
                        Scenario: wrong value\nThen the value should be \"x\"\n";

    /// A folder with `features/cart.feature`, and the environment reading it.
    fn on_disk() -> (tempfile::TempDir, Env, Vec<FeatureData>) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("features")).unwrap();
        std::fs::write(dir.path().join("features/cart.feature"), CART).unwrap();
        let target = dir.path().join("features");
        let mut loaded = Vec::new();
        let listed = load(&target, &mut loaded).unwrap();
        (dir, Env { target, loaded }, listed)
    }

    #[test]
    fn loading_lists_the_files_with_their_scenarios_and_lines() {
        let (dir, _env, listed) = on_disk();

        assert_eq!(listed.len(), 1);
        assert!(
            listed[0].path.ends_with("features/cart.feature"),
            "{}",
            listed[0].path
        );
        assert!(
            listed[0]
                .path
                .starts_with(&dir.path().to_string_lossy().replace('\\', "/")),
            "a path outside the current folder is shown whole"
        );
        assert_eq!(listed[0].name, "Cart");
        assert_eq!(
            listed[0].scenarios,
            [(4, "add item".to_owned()), (7, "wrong value".to_owned())]
        );
    }

    #[test]
    fn a_run_effect_runs_the_chosen_scenarios_through_the_runner() {
        let (_dir, mut env, _) = on_disk();

        let msg = env.perform(Effect::Run {
            chosen: vec![(0, vec![0, 1])],
            tags: String::new(),
        });

        let Some(Msg::Finished(runs)) = msg else {
            panic!("a run finishes with its results");
        };
        let verdicts: Vec<_> = runs.iter().map(|r| (r.name.as_str(), r.verdict)).collect();
        assert_eq!(
            verdicts,
            [
                ("add item", Verdict::Passed),
                ("wrong value", Verdict::Failed)
            ]
        );
        assert_eq!(runs[1].line, 7);
    }

    #[test]
    fn the_tags_of_a_run_leave_scenarios_out() {
        let (_dir, mut env, _) = on_disk();

        let msg = env.perform(Effect::Run {
            chosen: vec![(0, vec![0, 1])],
            tags: "@smoke".to_owned(),
        });

        let Some(Msg::Finished(runs)) = msg else {
            panic!("a run finishes with its results");
        };
        let verdicts: Vec<_> = runs.iter().map(|r| r.verdict).collect();
        assert_eq!(verdicts, [Verdict::Passed, Verdict::Skipped]);
    }

    #[test]
    fn a_run_effect_for_a_file_that_is_gone_from_the_list_runs_nothing() {
        let (_dir, mut env, _) = on_disk();

        let msg = env.perform(Effect::Run {
            chosen: vec![(5, vec![0])],
            tags: String::new(),
        });

        assert_eq!(msg, Some(Msg::Finished(Vec::new())));
    }

    #[test]
    fn a_reload_reads_a_file_added_since_and_a_missing_folder_lists_nothing() {
        let (dir, mut env, _) = on_disk();
        std::fs::write(
            dir.path().join("features/new.feature"),
            "Feature: New\nScenario: x\nGiven I do nothing\n",
        )
        .unwrap();

        let Some(Msg::Reloaded(listed)) = env.perform(Effect::Reload) else {
            panic!("a reload lists the files");
        };

        assert_eq!(listed.len(), 2);
        assert_eq!(env.loaded.len(), 2, "the run reads what the list shows");
        env.target = dir.path().join("nowhere");
        assert_eq!(env.perform(Effect::Reload), Some(Msg::Reloaded(Vec::new())));
    }

    #[test]
    fn a_shown_path_has_forward_slashes_and_no_leading_dot() {
        assert_eq!(
            shown(Path::new("./features/a.feature")),
            "features/a.feature"
        );
        assert_eq!(
            shown(Path::new("features\\a.feature")),
            "features/a.feature"
        );
        assert_eq!(shown(Path::new("features/a.feature")), "features/a.feature");
    }

    // ------------------------------------------------------------------
    // Drawing
    // ------------------------------------------------------------------

    const W: usize = 80;

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
            "Behave",
            &[&centered_in_frame("SeleniumBase Behave Commander")],
        )
    }

    const LIST_HINTS: &str =
        "q quit  ↑↓ move  Space mark  a/n all/none  Enter run  t tags  / filter  l reload";

    #[test]
    fn the_list_screen_shows_features_scenarios_marks_and_the_cursor() {
        let mut app = sample();
        press(&mut app, Input::Down);
        press(&mut app, Input::Char(' '));
        press(&mut app, Input::Down);

        let mut expected = header();
        expected.extend(framed(
            "Features: 2 files, 4 scenarios, 1 marked",
            &[
                "  [-] features/login.feature => Log in (3 scenarios)",
                "      [x] line 10: valid login",
                ">     [ ] line 14: wrong value",
                "      [ ] line 26: undefined step",
                "  [ ] features/cart.feature => Cart (1 scenario)",
                "      [ ] line 3: add item",
            ],
        ));
        expected.extend([String::new(), String::new(), LIST_HINTS.to_owned()]);
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn the_input_row_shows_the_tags_and_the_filter_in_force() {
        let mut app = sample();
        press(&mut app, Input::Char('t'));
        type_text(&mut app, "@smoke");
        press(&mut app, Input::Enter);
        press(&mut app, Input::Char('/'));
        type_text(&mut app, "cart");
        press(&mut app, Input::Enter);

        let screen = render(&app, 80, 14);

        assert_eq!(
            screen.lines().nth(11),
            Some("Tags: @smoke  Filter: cart"),
            "{screen}"
        );
    }

    #[test]
    fn while_editing_the_input_row_shows_what_is_typed_and_the_hints_change() {
        let mut app = sample();
        press(&mut app, Input::Char('/'));
        type_text(&mut app, "ca");
        let filtering = render(&app, 80, 14);
        press(&mut app, Input::Esc);
        press(&mut app, Input::Char('t'));
        type_text(&mut app, "@a");
        let tagging = render(&app, 80, 14);

        let rows: Vec<_> = filtering.lines().collect();
        assert_eq!(rows[11], "/ca");
        assert_eq!(rows[13], "Enter keep  Esc clear  Backspace delete");
        let rows: Vec<_> = tagging.lines().collect();
        assert_eq!(rows[11], "Tags: @a");
        assert_eq!(
            rows[13],
            "Enter accept  Esc cancel  e.g. @smoke,@web -@slow"
        );
    }

    #[test]
    fn with_no_feature_files_the_screen_says_where_it_looked() {
        let mut app = BehaveGui::new(Vec::new(), "features");
        app.resize(80, 14);

        let mut expected = header();
        expected.extend(framed(
            "Features: 0 files, 0 scenarios, 0 marked",
            &[
                &centered_in_frame("No feature files found."),
                "",
                &centered_in_frame("Looked in: features"),
                &centered_in_frame(
                    "Feature files are the *.feature files of a folder (default: features/),",
                ),
                &centered_in_frame("or pass a folder or a file: sbrs behave-gui <path>"),
                "",
            ],
        ));
        expected.extend([String::new(), String::new(), "q quit  l reload".to_owned()]);
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn the_status_row_shows_the_message() {
        let mut app = sample();
        press(&mut app, Input::Enter);

        let screen = render(&app, 80, 14);

        assert_eq!(
            screen.lines().nth(12),
            Some("Running 4 scenarios..."),
            "{screen}"
        );
    }

    #[test]
    fn the_results_screen_lists_each_scenario_with_its_verdict_and_the_details_of_one() {
        let mut app = sample();
        app.update(Msg::Finished(vec![
            run_of("valid login", Verdict::Passed, &[]),
            run_of(
                "wrong value",
                Verdict::Failed,
                &["step 'Then x' failed: the value is 5"],
            ),
            run_of(
                "undefined step",
                Verdict::Undefined,
                &["no handler registered for step: Given y"],
            ),
            run_of("skipped one", Verdict::Skipped, &["tags"]),
        ]));
        press(&mut app, Input::Down);

        let mut expected = header();
        expected.extend(framed(
            "Results: 1 passed, 1 failed, 1 undefined, 1 skipped",
            &[
                "  PASS  features/login.feature:10 valid login",
                "> FAIL  features/login.feature:10 wrong value",
                "  UNDEF features/login.feature:10 undefined step",
                "  SKIP  features/login.feature:10 skipped one",
            ],
        ));
        expected.extend(framed(
            "Details",
            &[
                "features/login.feature:10 wrong value",
                "step 'Then x' failed: the value is 5",
            ],
        ));
        expected.push("Esc back  ↑↓ move  PgUp PgDn  Home End".to_owned());
        assert_eq!(render(&app, 80, 14), expected.join("\n"));
    }

    #[test]
    fn a_passing_scenario_has_no_messages_to_show() {
        let mut app = sample();
        app.update(Msg::Finished(vec![run_of(
            "valid login",
            Verdict::Passed,
            &[],
        )]));

        let screen = render(&app, 80, 14);

        // The details block is rows 9 to 12: its two inner rows are the
        // scenario and, for a pass, nothing.
        let rows: Vec<&str> = screen.lines().collect();
        assert!(
            rows[10].starts_with("│features/login.feature:10 valid login"),
            "{screen}"
        );
        assert_eq!(rows[11], format!("│{}│", " ".repeat(W - 2)), "{screen}");
    }
}
