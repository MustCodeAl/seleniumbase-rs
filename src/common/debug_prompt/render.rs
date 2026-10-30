//! What a debug prompt shows besides the page: where in the test's source the
//! pause happened, why it was reached, and a source listing in the manner of
//! pdbp's sticky mode.
//!
//! ```text
//! > tests/login.rs(11)
//!    ...
//!    9             sb.type_text("#user", "demo").await?;
//!   10             sb.click("#go").await?;
//!   11  ->         sb.breakpoint("look").await?;
//!   12             Ok(())
//! ```
//!
//! The listing is drawn from the test's own file. The place comes from
//! [`SourceSpot::caller`] (`std::panic::Location`), so it needs no debug
//! information and works for any build profile. A returned error carries no
//! position, so a post-mortem lists the test body and marks the lines that
//! name the selector or the address the error is about.

use std::fmt::Write as _;
use std::panic::Location;
use std::path::{Path, PathBuf};

use super::PromptKind;
use crate::common::color::{palette, sanitize, Color, Style};
use crate::error::SeleniumBaseError;

/// Lines shown on each side of the line of a breakpoint.
const WINDOW: usize = 5;
/// The same for the `ll` command.
const LONG_WINDOW: usize = 30;
/// Lines of a test body shown at most.
const MAX_BODY_LINES: usize = 30;
/// The same for the `ll` command.
const MAX_LONG_BODY_LINES: usize = 80;
/// The biggest source file the listing reads.
const MAX_SOURCE_BYTES: u64 = 4 << 20;
/// The mark of a line that has been left out.
const DOTS: &str = "   ...";

/// A word of the code.
const KEYWORD: Style = Style::PLAIN.fg(Color::LightBlue);
/// A string or character literal.
const STRING: Style = Style::PLAIN.fg(Color::Green);
/// A comment.
const COMMENT: Style = Style::PLAIN.fg(Color::LightBlack);
/// A number.
const NUMBER: Style = Style::PLAIN.fg(Color::Magenta);
/// A macro name, such as `assert_eq!`.
const MACRO: Style = Style::PLAIN.fg(Color::Cyan);
/// The line numbers and the line of a header (pdbp's turquoise).
const LINE_NUMBER: Style = palette::PATH;
/// The file name in a header (pdbp's fuchsia).
const FILE_NAME: Style = Style::PLAIN.fg(Color::Magenta);
/// The line the pause is at (pdbp: white on blue).
const CURRENT_BAR: Style = Style::PLAIN.fg(Color::LightWhite).on(Color::Blue).bold();
/// The line the pause is at after a failure (pdbp: white on red).
const POST_MORTEM_BAR: Style = Style::PLAIN.fg(Color::LightWhite).on(Color::Red).bold();
/// A line that names what failed (pdbp: red on yellow).
const FAILING_BAR: Style = Style::PLAIN.fg(Color::Red).on(Color::LightYellow).bold();

/// A place in the source of a test: a file, a line and a column.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::common::debug_prompt::SourceSpot;
///
/// let here = SourceSpot::caller();
/// assert!(here.file().ends_with(".rs") || here.file().contains("doctest"));
/// assert!(here.line() > 0);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpot {
    file: String,
    line: u32,
    column: u32,
}

impl SourceSpot {
    /// A spot at `file`, `line` and `column`, counted from 1.
    #[must_use]
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }

    /// The place this function was called from.
    ///
    /// Like [`Location::caller`]: when a function marked `#[track_caller]`
    /// calls it, the place is where that function was called from. An `async
    /// fn` cannot be marked, so the calls that want their caller's place are
    /// plain functions that read it and return a future.
    #[must_use]
    #[track_caller]
    pub fn caller() -> Self {
        let location = Location::caller();
        Self::new(location.file(), location.line(), location.column())
    }

    /// The file, as the compiler was given it (relative to the workspace, or
    /// to the package).
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The line, counted from 1.
    #[must_use]
    pub const fn line(&self) -> u32 {
        self.line
    }

    /// The column, counted from 1.
    #[must_use]
    pub const fn column(&self) -> u32 {
        self.column
    }
}

/// What went wrong, as far as a prompt can tell from the error.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::common::debug_prompt::Failure;
/// use seleniumbase_rs::SeleniumBaseError;
///
/// let failure = Failure::from_error(&SeleniumBaseError::StaleElement {
///     selector: "#go".to_owned(),
/// });
/// assert_eq!(failure.label(), "stale element");
/// assert_eq!(failure.selector(), Some("#go"));
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Failure {
    label: String,
    message: String,
    selector: Option<String>,
    url: Option<String>,
}

impl Failure {
    /// The failure `error` describes: its kind, its message, and the selector
    /// or the address it is about, when it has one.
    #[must_use]
    pub fn from_error(error: &SeleniumBaseError) -> Self {
        let (selector, url) = match error {
            SeleniumBaseError::ElementNotFound { selector, .. }
            | SeleniumBaseError::ElementNotInteractable { selector, .. }
            | SeleniumBaseError::StaleElement { selector } => (Some(selector.clone()), None),
            SeleniumBaseError::InvalidSelector(selector) => (Some(selector.clone()), None),
            SeleniumBaseError::Navigation { url, .. }
            | SeleniumBaseError::Network { url, .. }
            | SeleniumBaseError::Download { url, .. } => (None, Some(url.clone())),
            _ => (None, None),
        };
        Self {
            label: error.category().replace('_', " "),
            message: error.to_string(),
            selector,
            url,
        }
    }

    /// The kind of the error in words, such as `element not found`.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The error as it is shown.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The selector the error is about.
    #[must_use]
    pub fn selector(&self) -> Option<&str> {
        self.selector.as_deref()
    }

    /// The address the error is about.
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// The texts that, found in a line of the test, suggest it is the line that failed.
    fn needles(&self) -> impl Iterator<Item = &str> {
        [self.selector(), self.url()]
            .into_iter()
            .flatten()
            .filter(|text| text.chars().count() > 1)
    }
}

/// Where a pause happened and why it was reached.
///
/// An empty scene (the default) gives the plain banner of a prompt that knows
/// nothing about the test's source.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::common::debug_prompt::{Scene, SourceSpot};
///
/// let scene = Scene::at(SourceSpot::new("tests/login.rs", 11, 9));
/// assert_eq!(scene.spot().map(SourceSpot::line), Some(11));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Scene {
    spot: Option<SourceSpot>,
    failure: Option<Failure>,
}

impl Scene {
    /// A scene at `spot`.
    #[must_use]
    pub fn at(spot: SourceSpot) -> Self {
        Self {
            spot: Some(spot),
            failure: None,
        }
    }

    /// This scene, reached because of `failure`.
    #[must_use]
    pub fn with_failure(mut self, failure: Failure) -> Self {
        self.failure = Some(failure);
        self
    }

    /// The place in the test's source, when known.
    #[must_use]
    pub fn spot(&self) -> Option<&SourceSpot> {
        self.spot.as_ref()
    }

    /// The failure that led here, when there was one.
    #[must_use]
    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }
}

/// The kind of a lexical token that gets a colour, or that the brace matcher needs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Keyword,
    Str,
    Comment,
    Number,
    Macro,
    Open,
    Close,
}

impl Kind {
    const fn style(self) -> Option<Style> {
        match self {
            Self::Keyword => Some(KEYWORD),
            Self::Str => Some(STRING),
            Self::Comment => Some(COMMENT),
            Self::Number => Some(NUMBER),
            Self::Macro => Some(MACRO),
            Self::Open | Self::Close => None,
        }
    }
}

/// A token of the source: its kind and its byte range in the whole text.
#[derive(Clone, Copy, Debug)]
struct Token {
    kind: Kind,
    start: usize,
    end: usize,
}

/// The words Rust reserves.
const KEYWORDS: [&str; 39] = [
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "yield",
];

/// Splits Rust source into the tokens worth colouring, and its braces.
///
/// A small scanner, not a parser: it knows comments, strings (raw ones too),
/// characters, numbers, keywords and macro names, which is what the listing
/// needs. Every token starts and ends at an ASCII byte or at the ends of the
/// text, so a range is always a valid slice.
fn lex(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let next = bytes.get(at + 1).copied();
        let (kind, end) = match byte {
            b'/' if next == Some(b'/') => (Some(Kind::Comment), line_end(bytes, at)),
            b'/' if next == Some(b'*') => (Some(Kind::Comment), block_comment_end(bytes, at + 2)),
            b'"' => (Some(Kind::Str), string_end(bytes, at + 1)),
            b'\'' => match char_literal_end(bytes, at) {
                Some(end) => (Some(Kind::Str), end),
                None => (None, at + 1),
            },
            b'{' => (Some(Kind::Open), at + 1),
            b'}' => (Some(Kind::Close), at + 1),
            b'0'..=b'9' => (Some(Kind::Number), number_end(bytes, at)),
            b'_' | b'a'..=b'z' | b'A'..=b'Z' => word(bytes, text, at),
            _ => (None, at + 1),
        };
        if let Some(kind) = kind {
            tokens.push(Token {
                kind,
                start: at,
                end,
            });
        }
        at = end;
    }
    tokens
}

/// The index of the line end after `from`, or the end of the text.
fn line_end(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(bytes.len(), |offset| from + offset)
}

/// The index after the `*/` that closes a block comment whose body starts at `from`.
fn block_comment_end(bytes: &[u8], from: usize) -> usize {
    bytes
        .get(from..)
        .and_then(|rest| rest.windows(2).position(|pair| pair == b"*/"))
        .map_or(bytes.len(), |offset| from + offset + 2)
}

/// The index after the quote that closes a string whose body starts at `from`.
fn string_end(bytes: &[u8], from: usize) -> usize {
    let mut at = from;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' => at += 2,
            b'"' => return at + 1,
            _ => at += 1,
        }
    }
    bytes.len()
}

/// The index after a character literal that starts at `at` (`'x'`, `'\n'`),
/// or `None` for a lifetime.
fn char_literal_end(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes.get(at + 1)? {
        b'\\' => {
            let close = bytes
                .get(at + 3..)?
                .iter()
                .take_while(|&&byte| byte != b'\n')
                .position(|&byte| byte == b'\'')?;
            Some(at + 3 + close + 1)
        }
        b'\'' => None,
        _ => (bytes.get(at + 2) == Some(&b'\'')).then_some(at + 3),
    }
}

/// The index after the number that starts at `at`.
fn number_end(bytes: &[u8], at: usize) -> usize {
    let mut end = at;
    while bytes
        .get(end)
        .is_some_and(|&byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        end += 1;
    }
    if bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
        end += 1;
        while bytes
            .get(end)
            .is_some_and(|&byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            end += 1;
        }
    }
    end
}

/// The token the word at `at` is, and where it ends: a raw string, a macro
/// name, a keyword, or nothing.
fn word(bytes: &[u8], text: &str, at: usize) -> (Option<Kind>, usize) {
    let mut end = at;
    while bytes
        .get(end)
        .is_some_and(|&byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        end += 1;
    }
    let name = text.get(at..end).unwrap_or("");
    if matches!(name, "r" | "br") {
        if let Some(raw_end) = raw_string_end(bytes, end) {
            return (Some(Kind::Str), raw_end);
        }
    }
    if bytes.get(end) == Some(&b'!') && matches!(bytes.get(end + 1), Some(b'(' | b'[' | b'{')) {
        return (Some(Kind::Macro), end + 1);
    }
    if KEYWORDS.contains(&name) {
        (Some(Kind::Keyword), end)
    } else {
        (None, end)
    }
}

/// The index after a raw string whose `#`s and opening quote start at `from`
/// (`r#"..."#`), or `None` when no raw string starts there.
fn raw_string_end(bytes: &[u8], from: usize) -> Option<usize> {
    let hashes = bytes
        .get(from..)?
        .iter()
        .take_while(|&&byte| byte == b'#')
        .count();
    let body = from + hashes;
    if bytes.get(body) != Some(&b'"') {
        return None;
    }
    let mut closing = vec![b'"'];
    closing.extend(std::iter::repeat_n(b'#', hashes));
    let rest = bytes.get(body + 1..)?;
    Some(
        rest.windows(closing.len())
            .position(|window| window == closing.as_slice())
            .map_or(bytes.len(), |offset| body + 1 + offset + closing.len()),
    )
}

/// One line of source as the listing shows it: no control characters, tabs as
/// four spaces, no trailing white space.
fn clean(line: &str) -> String {
    sanitize(line).replace('\t', "    ").trim_end().to_owned()
}

/// The text of a source file and where its tokens are.
#[derive(Clone, Debug)]
pub(super) struct Source {
    path: PathBuf,
    lines: Vec<String>,
    /// The byte offset of the start of each line in the lines joined by `\n`.
    starts: Vec<usize>,
    tokens: Vec<Token>,
}

impl Source {
    /// A source of `lines`, named `path`.
    pub(super) fn from_lines(path: impl Into<PathBuf>, lines: Vec<String>) -> Self {
        let lines: Vec<String> = lines.iter().map(|line| clean(line)).collect();
        let mut starts = Vec::with_capacity(lines.len());
        let mut offset = 0;
        for line in &lines {
            starts.push(offset);
            offset += line.len() + 1;
        }
        let tokens = lex(&lines.join("\n"));
        Self {
            path: path.into(),
            lines,
            starts,
            tokens,
        }
    }

    /// The file `file` names: as it is when absolute, else under the first of
    /// `roots` that has it.
    pub(super) fn find_in(file: &str, roots: &[PathBuf]) -> Option<Self> {
        let given = Path::new(file);
        if given.is_absolute() {
            return Self::load(given.to_path_buf());
        }
        roots.iter().find_map(|root| Self::load(root.join(given)))
    }

    /// The source file of `spot`.
    ///
    /// The compiler records the path relative to the workspace (or the
    /// package) and a test runs in its package's folder, so the file is looked
    /// for under the working directory and under every folder above the
    /// package.
    pub(super) fn read(spot: &SourceSpot) -> Option<Self> {
        let mut roots = Vec::new();
        if let Ok(directory) = std::env::current_dir() {
            roots.push(directory);
        }
        if let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR") {
            roots.extend(Path::new(&manifest).ancestors().map(Path::to_path_buf));
        }
        Self::find_in(spot.file(), &roots)
    }

    fn load(path: PathBuf) -> Option<Self> {
        let size = std::fs::metadata(&path).ok()?.len();
        if size > MAX_SOURCE_BYTES {
            return None;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        Some(Self::from_lines(
            path,
            text.lines().map(str::to_owned).collect(),
        ))
    }

    /// Where the source was read from.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Line `number` (from 1), cleaned for the screen.
    pub(super) fn line(&self, number: usize) -> Option<&str> {
        self.lines.get(number.checked_sub(1)?).map(String::as_str)
    }

    /// How many lines there are.
    pub(super) fn len(&self) -> usize {
        self.lines.len()
    }

    /// The line of the `}` that closes the first `{` found from line
    /// `start_line` on, skipping braces in strings, comments and characters.
    pub(super) fn body_end(&self, start_line: usize) -> Option<usize> {
        let from = *self.starts.get(start_line.checked_sub(1)?)?;
        let mut depth = 0_usize;
        for token in self.tokens.iter().filter(|token| token.start >= from) {
            match token.kind {
                Kind::Open => depth += 1,
                Kind::Close => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(self.starts.partition_point(|&start| start <= token.start));
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Line `number` with its tokens coloured when `color` is set.
    fn highlighted(&self, number: usize, color: bool) -> String {
        let Some(text) = self.line(number) else {
            return String::new();
        };
        if !color {
            return text.to_owned();
        }
        let start = number
            .checked_sub(1)
            .and_then(|index| self.starts.get(index))
            .copied()
            .unwrap_or(0);
        let end = start + text.len();
        let first = self.tokens.partition_point(|token| token.end <= start);
        let mut out = String::with_capacity(text.len() * 2);
        let mut at = start;
        for token in self.tokens.get(first..).unwrap_or_default() {
            if token.start >= end {
                break;
            }
            let Some(style) = token.kind.style() else {
                continue;
            };
            let from = token.start.max(at);
            let to = token.end.min(end);
            if from >= to {
                continue;
            }
            out.push_str(text.get(at - start..from - start).unwrap_or(""));
            out.push_str(&style.paint(text.get(from - start..to - start).unwrap_or(""), true));
            at = to;
        }
        out.push_str(text.get(at - start..).unwrap_or(""));
        out
    }
}

/// One line of Rust with its tokens coloured, or as it is when `color` is unset.
#[cfg(test)]
pub(super) fn highlight_rust(line: &str, color: bool) -> String {
    Source::from_lines("", vec![line.to_owned()]).highlighted(1, color)
}

/// What a marked line is marked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MarkKind {
    /// Where the pause is (`->`).
    Current,
    /// A line that names what failed (`>>`).
    Failing,
}

/// A marked line of a listing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Mark {
    line: usize,
    kind: MarkKind,
}

/// Which lines of a source to show, and which of them to mark.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct View {
    first: usize,
    last: usize,
    marks: Vec<Mark>,
    /// Lines before `first` are left out.
    cut_before: bool,
    /// Lines after `last` are left out.
    cut_after: bool,
}

/// Whether the pause is before, after or around the whole body of a test
/// (`--trace`, `--pdb`, `--final-debug`), as opposed to a call in it.
const fn body_kind(kind: PromptKind) -> bool {
    matches!(
        kind,
        PromptKind::Trace | PromptKind::PostMortem | PromptKind::FinalTrace
    )
}

/// The lines to show for a pause of `kind` at `spot_line`.
///
/// A breakpoint shows a window around its line. The other kinds point at the
/// `run` call that holds the test, so they show the body, from that line to
/// its matching brace, cut to a window when it is long. After a failure the
/// lines that mention the failing selector or address are marked; with none
/// to mark, the `run` line is.
pub(super) fn view_for(
    source: &Source,
    kind: PromptKind,
    spot_line: usize,
    failure: Option<&Failure>,
    long: bool,
) -> View {
    let total = source.len();
    let spot = spot_line.clamp(1, total.max(1));
    if !body_kind(kind) {
        let radius = if long { LONG_WINDOW } else { WINDOW };
        let first = spot.saturating_sub(radius).max(1);
        let last = (spot + radius).min(total);
        return View {
            first,
            last,
            marks: vec![Mark {
                line: spot,
                kind: MarkKind::Current,
            }],
            cut_before: first > 1,
            cut_after: last < total,
        };
    }
    let end = source
        .body_end(spot)
        .unwrap_or_else(|| spot + 2 * WINDOW)
        .min(total);
    let mut marks: Vec<Mark> = Vec::new();
    if kind == PromptKind::PostMortem {
        if let Some(failure) = failure {
            marks = (spot..=end)
                .filter(|&number| {
                    source
                        .line(number)
                        .is_some_and(|text| failure.needles().any(|needle| text.contains(needle)))
                })
                .map(|line| Mark {
                    line,
                    kind: MarkKind::Failing,
                })
                .collect();
        }
    }
    if marks.is_empty() {
        marks.push(Mark {
            line: spot,
            kind: MarkKind::Current,
        });
    }
    let most = if long {
        MAX_LONG_BODY_LINES
    } else {
        MAX_BODY_LINES
    };
    let (mut first, mut last) = (spot, end);
    if last - first + 1 > most {
        let focus = marks.first().map_or(spot, |mark| mark.line);
        first = focus.saturating_sub(most / 3).max(spot);
        last = first + most - 1;
        if last > end {
            last = end;
            first = (end + 1 - most).max(spot);
        }
    }
    View {
        first,
        last,
        marks,
        cut_before: first > spot,
        cut_after: last < end,
    }
}

/// The marker and the bar of a marked line.
const fn bar_for(mark: MarkKind, kind: PromptKind) -> (&'static str, Style) {
    match (mark, kind) {
        (MarkKind::Failing, _) => (">>", FAILING_BAR),
        (MarkKind::Current, PromptKind::PostMortem) => ("->", POST_MORTEM_BAR),
        (MarkKind::Current, _) => ("->", CURRENT_BAR),
    }
}

/// The numbered lines of `view`, one per line of text.
///
/// pdbp's layout: a right-aligned number, then a two-character marker column,
/// then the code. With `color` the numbers are cyan, the code is coloured by
/// token, and a marked line is one bar padded to the widest line shown.
pub(super) fn render_listing(
    source: &Source,
    view: &View,
    kind: PromptKind,
    color: bool,
) -> String {
    let width = (view.first..=view.last)
        .filter_map(|number| source.line(number))
        .map(|text| text.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    if view.cut_before {
        let _ = writeln!(out, "{}", palette::DIM.paint(DOTS, color));
    }
    for number in view.first..=view.last {
        let Some(text) = source.line(number) else {
            break;
        };
        let shown = LINE_NUMBER.paint(format!("{number:>4}"), color);
        match view.marks.iter().find(|mark| mark.line == number) {
            Some(mark) => {
                let (symbol, bar) = bar_for(mark.kind, kind);
                let body = format!("{symbol} {text}");
                if color {
                    let padded = format!("{body:<pad$}", pad = width + 3);
                    let _ = writeln!(out, "{shown}  {}", bar.paint(padded, true));
                } else {
                    let _ = writeln!(out, "{shown}  {body}");
                }
            }
            None if text.is_empty() => {
                let _ = writeln!(out, "{shown}");
            }
            None => {
                let _ = writeln!(out, "{shown}     {}", source.highlighted(number, color));
            }
        }
    }
    if view.cut_after {
        let _ = writeln!(out, "{}", palette::DIM.paint(DOTS, color));
    }
    out
}

/// `> file(line)` and a newline, with the file magenta and the line cyan as in
/// pdbp, and ` test body` before the newline when the pause is the `run` call
/// that holds the test (the kinds that are not a breakpoint).
fn spot_header(kind: PromptKind, file: &str, line: u32, color: bool) -> String {
    format!(
        "> {}({}){}\n",
        FILE_NAME.paint(sanitize(file), color),
        LINE_NUMBER.paint(line, color),
        if body_kind(kind) { " test body" } else { "" }
    )
}

/// The header alone, for the `w` command: the file where it was found when it
/// can be read, else as the compiler recorded it.
pub(super) fn where_header(kind: PromptKind, spot: &SourceSpot, color: bool) -> String {
    let file = Source::read(spot).map_or_else(
        || spot.file().to_owned(),
        |source| source.path().display().to_string(),
    );
    spot_header(kind, &file, spot.line(), color)
}

/// The header and the listing of a pause, ending with a blank line; empty
/// when the scene has no spot.
///
/// The header is `> file(line)`, with the file magenta and the line cyan as
/// in pdbp, and ` test body` after it when the spot is the `run` call that
/// holds the test. With `long` the window is wider (the `ll` command).
pub(super) fn code_block(kind: PromptKind, scene: &Scene, long: bool, color: bool) -> String {
    let Some(spot) = scene.spot() else {
        return String::new();
    };
    let source = Source::read(spot);
    let file = source.as_ref().map_or_else(
        || sanitize(spot.file()),
        |source| sanitize(&source.path().display().to_string()),
    );
    let mut out = spot_header(kind, &file, spot.line(), color);
    match source {
        Some(source) => {
            let line = usize::try_from(spot.line()).unwrap_or(1);
            let view = view_for(&source, kind, line, scene.failure(), long);
            out.push_str(&render_listing(&source, &view, kind, color));
        }
        None => {
            let _ = writeln!(
                out,
                "{}",
                palette::DIM.paint("  (the source file is not here)", color)
            );
        }
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SeleniumBaseError;

    /// A small test, the way a person writes one. Line numbers matter: the
    /// tests below point at them.
    const SAMPLE: [&str; 15] = [
        "use seleniumbase_rs::testing::Sb;",
        "",
        "#[rstest]",
        "#[tokio::test]",
        "async fn login(sb: Sb) -> Result<()> {",
        "    sb.run(async |sb| {",
        "        sb.open(\"https://example.com/login\").await?;",
        "        // fill the form",
        "        sb.type_text(\"#user\", \"demo\").await?;",
        "        sb.click(\"#go\").await?;",
        "        sb.breakpoint(\"look\").await?;",
        "        Ok(())",
        "    })",
        "    .await",
        "}",
    ];

    fn sample() -> Source {
        Source::from_lines(
            "tests/login.rs",
            SAMPLE.iter().map(|line| (*line).to_owned()).collect(),
        )
    }

    fn element_gone() -> Failure {
        Failure::from_error(&SeleniumBaseError::ElementNotFound {
            selector: "#go".to_owned(),
            strategy: "css".to_owned(),
        })
    }

    // ---- where the pause happened ----

    #[test]
    fn a_spot_is_the_line_of_the_call_that_asked_for_it() {
        let (spot, expected) = (SourceSpot::caller(), line!());

        assert_eq!(spot.line(), expected);
        assert!(spot.file().ends_with("render.rs"), "{}", spot.file());
        assert!(spot.column() > 0);
    }

    #[test]
    fn a_failure_names_the_selector_or_the_url_the_error_carries() {
        let gone = element_gone();
        assert_eq!(gone.label(), "element not found");
        assert_eq!(gone.selector(), Some("#go"));
        assert_eq!(gone.url(), None);
        assert_eq!(
            gone.message(),
            "element not found: #go (strategy: css)",
            "the message is the error as it is shown"
        );

        let navigation = Failure::from_error(&SeleniumBaseError::Navigation {
            url: "https://example.com/gone".to_owned(),
            reason: "net::ERR_NAME_NOT_RESOLVED".to_owned(),
        });
        assert_eq!(navigation.label(), "navigation");
        assert_eq!(navigation.selector(), None);
        assert_eq!(navigation.url(), Some("https://example.com/gone"));

        let other = Failure::from_error(&SeleniumBaseError::AssertionFailed("no".to_owned()));
        assert_eq!((other.selector(), other.url()), (None, None));
    }

    // ---- reading the source ----

    #[test]
    fn a_source_is_found_under_the_first_root_that_has_the_file() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(second.path().join("tests")).unwrap();
        std::fs::write(
            second.path().join("tests/login.rs"),
            "fn a() {}\n\tlet b;\n",
        )
        .unwrap();

        let roots = [first.path().to_path_buf(), second.path().to_path_buf()];
        let source = Source::find_in("tests/login.rs", &roots).expect("found in the second root");

        assert_eq!(source.line(1), Some("fn a() {}"));
        assert_eq!(source.line(2), Some("    let b;"), "a tab is four spaces");
        assert_eq!(source.line(3), None);
        assert_eq!(source.len(), 2);
        assert!(Source::find_in("tests/other.rs", &roots).is_none());
    }

    #[test]
    fn an_absolute_path_needs_no_root() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.rs");
        std::fs::write(&file, "fn x() {}\n").unwrap();

        let source = Source::find_in(file.to_str().unwrap(), &[]).expect("read as given");

        assert_eq!(source.line(1), Some("fn x() {}"));
    }

    // ---- brace matching ----

    #[test]
    fn the_body_of_a_run_ends_at_its_matching_brace() {
        // Line 6 opens the closure, line 13 closes it (`})`).
        assert_eq!(sample().body_end(6), Some(13));
    }

    #[test]
    fn braces_in_strings_comments_and_chars_do_not_count() {
        let source = Source::from_lines(
            "x.rs",
            [
                "sb.run(async |sb| {",
                "    let a = \"}\"; // }",
                "    /* } */ let b = '}';",
                "    let c = r#\"}\"#;",
                "    if true { }",
                "})",
            ]
            .iter()
            .map(|line| (*line).to_owned())
            .collect(),
        );

        assert_eq!(source.body_end(1), Some(6));
    }

    #[test]
    fn a_body_that_never_closes_has_no_end() {
        let source = Source::from_lines("x.rs", vec!["sb.run(async |sb| {".to_owned()]);

        assert_eq!(source.body_end(1), None);
        assert_eq!(sample().body_end(99), None);
    }

    // ---- which lines to show ----

    #[test]
    fn a_breakpoint_shows_a_window_around_its_line_and_marks_it() {
        let view = view_for(&sample(), PromptKind::Breakpoint, 11, None, false);

        assert_eq!((view.first, view.last), (6, 15));
        assert_eq!(
            view.marks,
            [Mark {
                line: 11,
                kind: MarkKind::Current
            }]
        );
        assert!(view.cut_before, "lines 1 to 5 are not shown");
        assert!(!view.cut_after, "the file ends at line 15");
    }

    #[test]
    fn a_window_stays_inside_the_file() {
        let view = view_for(&sample(), PromptKind::Breakpoint, 2, None, false);

        assert_eq!((view.first, view.last), (1, 7));
        assert!(!view.cut_before);
        assert!(view.cut_after);
    }

    #[test]
    fn the_long_list_is_wider() {
        let source = Source::from_lines(
            "long.rs",
            (1..=200).map(|n| format!("let _{n} = {n};")).collect(),
        );

        let short = view_for(&source, PromptKind::Breakpoint, 100, None, false);
        let long = view_for(&source, PromptKind::Breakpoint, 100, None, true);

        assert_eq!((short.first, short.last), (95, 105));
        assert_eq!((long.first, long.last), (70, 130));
    }

    #[test]
    fn a_trace_shows_the_body_from_the_run_line_to_its_end() {
        let view = view_for(&sample(), PromptKind::Trace, 6, None, false);

        assert_eq!((view.first, view.last), (6, 13));
        assert_eq!(
            view.marks,
            [Mark {
                line: 6,
                kind: MarkKind::Current
            }]
        );
        assert!(!view.cut_before && !view.cut_after);
    }

    #[test]
    fn a_post_mortem_marks_the_lines_that_name_the_failing_selector() {
        let gone = element_gone();

        let view = view_for(&sample(), PromptKind::PostMortem, 6, Some(&gone), false);

        assert_eq!(
            view.marks,
            [Mark {
                line: 10,
                kind: MarkKind::Failing
            }],
            "only line 10 mentions \"#go\""
        );
    }

    #[test]
    fn a_post_mortem_with_nothing_to_match_marks_the_run_line() {
        let other = Failure::from_error(&SeleniumBaseError::AssertionFailed("no".to_owned()));

        let view = view_for(&sample(), PromptKind::PostMortem, 6, Some(&other), false);

        assert_eq!(
            view.marks,
            [Mark {
                line: 6,
                kind: MarkKind::Current
            }]
        );
    }

    #[test]
    fn a_body_longer_than_the_limit_is_cut_around_the_failing_line() {
        let mut lines = vec!["sb.run(async |sb| {".to_owned()];
        lines.extend((2..=79).map(|n| format!("    sb.click(\"#b{n}\").await?;")));
        lines.push("    sb.click(\"#go\").await?;".to_owned());
        lines.extend((81..=99).map(|n| format!("    sb.click(\"#c{n}\").await?;")));
        lines.push("})".to_owned());
        let source = Source::from_lines("big.rs", lines);

        let view = view_for(
            &source,
            PromptKind::PostMortem,
            1,
            Some(&element_gone()),
            false,
        );

        assert_eq!(view.last - view.first + 1, 30, "{view:?}");
        assert!(view.first <= 80 && 80 <= view.last, "{view:?}");
        assert!(view.cut_before && view.cut_after);
    }

    // ---- the listing ----

    #[test]
    fn a_plain_listing_numbers_the_lines_and_points_at_the_current_one() {
        let source = sample();
        let view = view_for(&source, PromptKind::Breakpoint, 11, None, false);
        let view = View {
            first: 9,
            last: 12,
            cut_before: true,
            cut_after: true,
            ..view
        };

        let text = render_listing(&source, &view, PromptKind::Breakpoint, false);

        let expected = [
            "   ...",
            "   9             sb.type_text(\"#user\", \"demo\").await?;",
            "  10             sb.click(\"#go\").await?;",
            "  11  ->         sb.breakpoint(\"look\").await?;",
            "  12             Ok(())",
            "   ...",
            "",
        ]
        .join("\n");
        assert_eq!(text, expected);
    }

    #[test]
    fn the_failing_line_gets_its_own_marker() {
        let source = sample();
        let view = View {
            first: 9,
            last: 10,
            marks: vec![Mark {
                line: 10,
                kind: MarkKind::Failing,
            }],
            cut_before: false,
            cut_after: false,
        };

        let text = render_listing(&source, &view, PromptKind::PostMortem, false);

        assert_eq!(
            text,
            [
                "   9             sb.type_text(\"#user\", \"demo\").await?;",
                "  10  >>         sb.click(\"#go\").await?;",
                "",
            ]
            .join("\n")
        );
    }

    #[test]
    fn a_coloured_listing_paints_the_numbers_the_tokens_and_the_marked_line() {
        let source = Source::from_lines(
            "x.rs",
            vec!["let x = \"hi\"; // note".to_owned(), "go();".to_owned()],
        );
        let view = View {
            first: 1,
            last: 2,
            marks: vec![Mark {
                line: 2,
                kind: MarkKind::Current,
            }],
            cut_before: false,
            cut_after: false,
        };

        let text = render_listing(&source, &view, PromptKind::Breakpoint, true);

        let expected = [
            // Number in cyan, keyword, string and comment in their colours.
            "\x1b[36m   1\x1b[0m     \x1b[94mlet\x1b[0m x = \x1b[32m\"hi\"\x1b[0m; \x1b[90m// note\x1b[0m",
            // The marked line is one bar: white on blue, bold, padded to the widest line.
            "\x1b[36m   2\x1b[0m  \x1b[1;97;44m-> go();                \x1b[0m",
            "",
        ]
        .join("\n");
        assert_eq!(text, expected);
    }

    #[test]
    fn a_post_mortem_bar_is_red() {
        let source = Source::from_lines("x.rs", vec!["boom();".to_owned()]);
        let view = View {
            first: 1,
            last: 1,
            marks: vec![Mark {
                line: 1,
                kind: MarkKind::Current,
            }],
            cut_before: false,
            cut_after: false,
        };

        let text = render_listing(&source, &view, PromptKind::PostMortem, true);

        assert!(text.contains("\x1b[1;97;41m-> boom();\x1b[0m"), "{text:?}");
    }

    #[test]
    fn what_comes_from_the_file_cannot_move_the_cursor() {
        let source = Source::from_lines("x.rs", vec!["let s = \"\x1b[2J\"; \u{7}".to_owned()]);
        let view = View {
            first: 1,
            last: 1,
            marks: vec![],
            cut_before: false,
            cut_after: false,
        };

        let text = render_listing(&source, &view, PromptKind::Breakpoint, false);

        assert!(
            !text.contains('\x1b') && !text.contains('\u{7}'),
            "{text:?}"
        );
    }

    // ---- the colours of the code ----

    #[test]
    fn without_colour_the_line_is_as_written() {
        let line = "let x = \"hi\"; // note";

        assert_eq!(highlight_rust(line, false), line);
    }

    #[test]
    fn rust_tokens_get_their_colours() {
        assert_eq!(
            highlight_rust("let x = \"hi\"; // note", true),
            "\x1b[94mlet\x1b[0m x = \x1b[32m\"hi\"\x1b[0m; \x1b[90m// note\x1b[0m"
        );
        assert_eq!(
            highlight_rust("assert_eq!(1, 2);", true),
            "\x1b[36massert_eq!\x1b[0m(\x1b[35m1\x1b[0m, \x1b[35m2\x1b[0m);"
        );
        assert_eq!(
            highlight_rust("sb.click(\"#go\").await?;", true),
            "sb.click(\x1b[32m\"#go\"\x1b[0m).\x1b[94mawait\x1b[0m?;"
        );
    }

    #[test]
    fn a_word_that_only_starts_like_a_keyword_or_number_is_plain() {
        assert_eq!(
            highlight_rust("awaiting x1 letter", true),
            "awaiting x1 letter"
        );
        assert_eq!(
            highlight_rust("a != b", true),
            "a != b",
            "!= is not a macro"
        );
        assert_eq!(
            highlight_rust("fn f<'a>(x: &'a str)", true),
            "\x1b[94mfn\x1b[0m f<'a>(x: &'a str)"
        );
    }

    // ---- the block the banner holds ----

    fn on_disk(lines: &[&str]) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("login.rs");
        std::fs::write(&file, lines.join("\n")).unwrap();
        let path = file.to_str().unwrap().to_owned();
        (dir, path)
    }

    #[test]
    fn the_block_starts_with_the_file_and_line_then_lists_the_code() {
        let (_dir, path) = on_disk(&SAMPLE);
        let scene = Scene::at(SourceSpot::new(path.clone(), 11, 9));

        let text = code_block(PromptKind::Breakpoint, &scene, false, false);

        assert!(text.starts_with(&format!("> {path}(11)\n")), "{text}");
        assert!(
            text.contains("  11  ->         sb.breakpoint(\"look\").await?;\n"),
            "{text}"
        );
        assert!(
            text.ends_with("\n\n"),
            "a blank line closes the block: {text:?}"
        );
    }

    #[test]
    fn the_header_is_coloured_when_colour_is_on() {
        let (_dir, path) = on_disk(&SAMPLE);
        let scene = Scene::at(SourceSpot::new(path.clone(), 11, 9));

        let text = code_block(PromptKind::Breakpoint, &scene, false, true);

        assert!(
            text.starts_with(&format!("> \x1b[35m{path}\x1b[0m(\x1b[36m11\x1b[0m)\n")),
            "{text:?}"
        );
    }

    #[test]
    fn a_trace_names_the_body_in_the_header() {
        let (_dir, path) = on_disk(&SAMPLE);
        let scene = Scene::at(SourceSpot::new(path.clone(), 6, 5));

        let text = code_block(PromptKind::Trace, &scene, false, false);

        assert!(
            text.starts_with(&format!("> {path}(6) test body\n")),
            "{text}"
        );
    }

    #[test]
    fn a_missing_file_says_so_and_still_names_the_spot() {
        let scene = Scene::at(SourceSpot::new("nowhere/gone.rs", 3, 1));

        let text = code_block(PromptKind::Breakpoint, &scene, false, false);

        assert_eq!(
            text,
            "> nowhere/gone.rs(3)\n  (the source file is not here)\n\n"
        );
    }

    #[test]
    fn a_scene_without_a_spot_has_no_block() {
        assert_eq!(
            code_block(PromptKind::Breakpoint, &Scene::default(), false, true),
            ""
        );
    }
}
