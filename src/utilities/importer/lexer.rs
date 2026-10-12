//! The tokenizer the JavaScript, TypeScript and Python front ends share.
//!
//! It reads the subset of each language that browser scripts are written in:
//! identifiers, every string form (JavaScript single, double and template
//! quotes; Python single, double and triple quotes with `r`, `b`, `u` and `f`
//! prefixes), numbers, regular-expression literals, punctuation and comments.
//! String literals are decoded here, once, so the rest of the importer only
//! ever sees their values. A template literal with `${}` or an f-string with
//! `{}` placeholders is kept as opaque text: it is never decoded or evaluated.
//!
//! The tokenizer is a loop over the input with no recursion, so no input can
//! exhaust the stack here; depth is bounded by the parser.

use std::fmt;

/// Which language's rules to read with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Dialect {
    /// JavaScript and TypeScript.
    Js,
    /// Python.
    Python,
}

/// What a token is.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
    /// A name or keyword.
    Ident(String),
    /// A string literal, decoded.
    Str(String),
    /// A template literal with `${}` substitutions or an f-string with
    /// placeholders: its source text, never decoded.
    Interpolated(String),
    /// A number literal.
    Num(f64),
    /// A JavaScript regular-expression literal.
    Regex { pattern: String, flags: String },
    /// Punctuation or an operator.
    Punct(&'static str),
}

/// A token and where it is.
#[derive(Clone, Debug)]
pub(super) struct Token {
    pub(super) tok: Tok,
    /// 1-based line of the first character.
    pub(super) line: usize,
    /// 0-based column of the first character, with tabs to the next multiple
    /// of 8 (Python's rule), for indentation.
    pub(super) col: usize,
    /// Byte offset of the first character.
    pub(super) start: usize,
    /// Byte offset after the last character.
    pub(super) end: usize,
    /// A line break comes between this token and the one before it.
    pub(super) nl_before: bool,
    /// Python only: this token starts a logical line (it follows a line break
    /// outside brackets that was not escaped with a backslash).
    pub(super) line_start: bool,
}

/// A comment and where it starts.
#[derive(Clone, Debug)]
pub(super) struct Comment {
    /// 1-based line.
    pub(super) line: usize,
    /// Byte offset of the comment marker.
    pub(super) start: usize,
    /// The text without the markers.
    pub(super) text: String,
}

/// The tokens and comments of a source file.
#[derive(Debug, Default)]
pub(super) struct Lexed {
    pub(super) tokens: Vec<Token>,
    pub(super) comments: Vec<Comment>,
}

/// Why the source could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LexError {
    /// 1-based line where the problem starts.
    pub(super) line: usize,
    /// What is wrong.
    pub(super) message: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// JavaScript punctuators, longest first so the first match is the longest.
const JS_PUNCT: &[&str] = &[
    ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=",
    "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
    "**", "<<", ">>", "{", "}", "(", ")", "[", "]", ";", ",", "<", ">", "+", "-", "*", "/", "%",
    "&", "|", "^", "!", "~", "?", ":", "=", ".", "@", "#",
];

/// Python operators and delimiters, longest first.
const PY_PUNCT: &[&str] = &[
    "**=", "//=", ">>=", "<<=", "...", "->", ":=", "==", "!=", "<=", ">=", "**", "//", "<<", ">>",
    "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "@=", "{", "}", "(", ")", "[", "]", ";", ",",
    "<", ">", "+", "-", "*", "/", "%", "&", "|", "^", "~", ":", "=", ".", "@",
];

/// JavaScript keywords after which a `/` starts a regular expression.
const REGEX_KEYWORDS: &[&str] = &[
    "return",
    "typeof",
    "instanceof",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "throw",
    "case",
    "do",
    "else",
    "await",
    "yield",
];

/// Reads `source` into tokens and comments.
pub(super) fn tokenize(source: &str, dialect: Dialect) -> Result<Lexed, LexError> {
    Lexer::new(source, dialect).run()
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    line: usize,
    /// Byte offset where the current line starts.
    line_offset: usize,
    dialect: Dialect,
    out: Lexed,
    nl_pending: bool,
    /// Python: a logical line ends at the next line break.
    py_line_start_pending: bool,
    /// Python: open brackets, inside which line breaks do not end a line.
    depth: usize,
    /// JavaScript: whether a `/` here would start a regular expression.
    regex_allowed: bool,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, dialect: Dialect) -> Self {
        Self {
            src,
            pos: 0,
            line: 1,
            line_offset: 0,
            dialect,
            out: Lexed::default(),
            nl_pending: true,
            py_line_start_pending: true,
            depth: 0,
            regex_allowed: true,
        }
    }

    fn error(&self, line: usize, message: impl Into<String>) -> LexError {
        LexError {
            line,
            message: message.into(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    /// Consumes one character, keeping the line count.
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.line_offset = self.pos;
        }
        Some(c)
    }

    /// The column of byte offset `at` on the current line.
    fn column(&self, at: usize) -> usize {
        let mut col = 0;
        for c in self.src[self.line_offset..at].chars() {
            col = if c == '\t' {
                (col / 8 + 1) * 8
            } else {
                col + 1
            };
        }
        col
    }

    fn run(mut self) -> Result<Lexed, LexError> {
        // A byte-order mark and, in JavaScript, a `#!` line are not code.
        if self.rest().starts_with('\u{feff}') {
            self.pos += '\u{feff}'.len_utf8();
            self.line_offset = self.pos;
        }
        if self.dialect == Dialect::Js && self.rest().starts_with("#!") {
            while let Some(c) = self.peek() {
                if c == '\n' {
                    break;
                }
                self.bump();
            }
        }
        while let Some(c) = self.peek() {
            match c {
                '\n' => {
                    self.bump();
                    self.newline();
                }
                c if c.is_whitespace() => {
                    self.bump();
                }
                '\\' if self.dialect == Dialect::Python
                    && matches!(self.peek_at(1), Some('\n' | '\r')) =>
                {
                    // An explicit line join.
                    self.bump();
                    if self.peek() == Some('\r') {
                        self.bump();
                    }
                    if self.peek() == Some('\n') {
                        self.bump();
                    }
                    self.nl_pending = true;
                }
                '#' if self.dialect == Dialect::Python => self.line_comment(1),
                '/' if self.dialect == Dialect::Js && self.peek_at(1) == Some('/') => {
                    self.line_comment(2);
                }
                '/' if self.dialect == Dialect::Js && self.peek_at(1) == Some('*') => {
                    self.block_comment()?;
                }
                _ => self.token()?,
            }
        }
        Ok(self.out)
    }

    fn newline(&mut self) {
        self.nl_pending = true;
        if self.dialect == Dialect::Python && self.depth == 0 {
            self.py_line_start_pending = true;
        }
    }

    fn line_comment(&mut self, marker: usize) {
        let start = self.pos;
        let line = self.line;
        for _ in 0..marker {
            self.bump();
        }
        let text_start = self.pos;
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.bump();
        }
        self.out.comments.push(Comment {
            line,
            start,
            text: self.src[text_start..self.pos].trim().to_owned(),
        });
    }

    fn block_comment(&mut self) -> Result<(), LexError> {
        let start = self.pos;
        let line = self.line;
        self.bump();
        self.bump();
        let text_start = self.pos;
        loop {
            if self.rest().starts_with("*/") {
                let text = self.src[text_start..self.pos].to_owned();
                self.bump();
                self.bump();
                self.out.comments.push(Comment {
                    line,
                    start,
                    text: text
                        .lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .filter(|l| !l.is_empty())
                        .collect::<Vec<_>>()
                        .join(" "),
                });
                return Ok(());
            }
            match self.bump() {
                Some('\n') => self.nl_pending = true,
                Some(_) => {}
                None => return Err(self.error(line, "a /* comment is never closed")),
            }
        }
    }

    fn push(&mut self, tok: Tok, start: usize, line: usize, col: usize) {
        self.regex_allowed = match &tok {
            Tok::Ident(name) => REGEX_KEYWORDS.contains(&name.as_str()),
            Tok::Str(_) | Tok::Interpolated(_) | Tok::Num(_) | Tok::Regex { .. } => false,
            Tok::Punct(p) => !matches!(*p, ")" | "]" | "}"),
        };
        if let Tok::Punct(p) = &tok {
            match *p {
                "(" | "[" | "{" => self.depth += 1,
                ")" | "]" | "}" => self.depth = self.depth.saturating_sub(1),
                _ => {}
            }
        }
        let line_start = self.dialect == Dialect::Python && self.py_line_start_pending;
        self.out.tokens.push(Token {
            tok,
            line,
            col,
            start,
            end: self.pos,
            nl_before: self.nl_pending,
            line_start,
        });
        self.nl_pending = false;
        self.py_line_start_pending = false;
    }

    fn token(&mut self) -> Result<(), LexError> {
        let start = self.pos;
        let line = self.line;
        let col = self.column(start);
        let c = self.peek().unwrap_or_default();

        if self.dialect == Dialect::Python {
            if let Some((prefix_len, quote)) = self.python_string_start() {
                let tok = self.python_string(prefix_len, quote)?;
                self.push(tok, start, line, col);
                return Ok(());
            }
        }
        if is_ident_start(c, self.dialect) {
            while let Some(c) = self.peek() {
                if is_ident_continue(c, self.dialect) {
                    self.bump();
                } else {
                    break;
                }
            }
            let name = self.src[start..self.pos].to_owned();
            self.push(Tok::Ident(name), start, line, col);
            return Ok(());
        }
        if c.is_ascii_digit() || (c == '.' && self.peek_at(1).is_some_and(|d| d.is_ascii_digit())) {
            let value = self.number(line)?;
            self.push(Tok::Num(value), start, line, col);
            return Ok(());
        }
        match c {
            '\'' | '"' if self.dialect == Dialect::Js => {
                let value = self.js_string(c)?;
                self.push(Tok::Str(value), start, line, col);
                return Ok(());
            }
            '`' if self.dialect == Dialect::Js => {
                let tok = self.template()?;
                self.push(tok, start, line, col);
                return Ok(());
            }
            '/' if self.dialect == Dialect::Js && self.regex_allowed => {
                if let Some(tok) = self.regex() {
                    self.push(tok, start, line, col);
                    return Ok(());
                }
            }
            _ => {}
        }
        let table = match self.dialect {
            Dialect::Js => JS_PUNCT,
            Dialect::Python => PY_PUNCT,
        };
        if let Some(punct) = table.iter().find(|p| self.rest().starts_with(**p)) {
            self.pos += punct.len();
            self.push(Tok::Punct(punct), start, line, col);
            return Ok(());
        }
        Err(self.error(line, format!("unexpected character {c:?}")))
    }

    fn number(&mut self, line: usize) -> Result<f64, LexError> {
        let start = self.pos;
        let radix = match (self.peek(), self.peek_at(1)) {
            (Some('0'), Some('x' | 'X')) => 16,
            (Some('0'), Some('o' | 'O')) => 8,
            (Some('0'), Some('b' | 'B')) => 2,
            _ => 10,
        };
        if radix != 10 {
            self.bump();
            self.bump();
            let digits_start = self.pos;
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() || c == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
            let digits: String = self.src[digits_start..self.pos]
                .chars()
                .filter(|c| *c != '_')
                .collect();
            if self.peek() == Some('n') {
                self.bump();
            }
            return u64::from_str_radix(&digits, radix)
                .map(|n| n as f64)
                .map_err(|_| self.error(line, "a number literal is malformed"));
        }
        let mut seen_dot = false;
        let mut seen_exp = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '_' {
                self.bump();
            } else if c == '.' && !seen_dot && !seen_exp {
                // `1..toString()` and `x.5` are not ours to read; one dot only.
                seen_dot = true;
                self.bump();
            } else if matches!(c, 'e' | 'E')
                && !seen_exp
                && self
                    .peek_at(1)
                    .is_some_and(|d| d.is_ascii_digit() || d == '+' || d == '-')
            {
                seen_exp = true;
                self.bump();
                if matches!(self.peek(), Some('+' | '-')) {
                    self.bump();
                }
            } else {
                break;
            }
        }
        let text: String = self.src[start..self.pos]
            .chars()
            .filter(|c| *c != '_')
            .collect();
        // A BigInt suffix, or a Python imaginary one, is read past.
        if matches!(self.peek(), Some('n' | 'j' | 'J')) {
            self.bump();
        }
        text.parse::<f64>()
            .map_err(|_| self.error(line, "a number literal is malformed"))
    }

    fn js_string(&mut self, quote: char) -> Result<String, LexError> {
        let line = self.line;
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                None | Some('\n') => {
                    return Err(self.error(line, "a string literal is never closed"));
                }
                Some(c) if c == quote => return Ok(out),
                Some('\\') => self.js_escape(&mut out, line)?,
                Some(c) => out.push(c),
            }
        }
    }

    /// Decodes one JavaScript escape (the backslash is already read).
    fn js_escape(&mut self, out: &mut String, line: usize) -> Result<(), LexError> {
        let Some(c) = self.bump() else {
            return Err(self.error(line, "a string literal is never closed"));
        };
        match c {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            '0' if !self.peek().is_some_and(|d| d.is_ascii_digit()) => out.push('\0'),
            'x' => {
                let code = self.hex_digits(2);
                out.push(code.and_then(char::from_u32).unwrap_or('\u{fffd}'));
            }
            'u' => {
                let code = if self.peek() == Some('{') {
                    self.bump();
                    let digits_start = self.pos;
                    while self.peek().is_some_and(|d| d.is_ascii_hexdigit()) {
                        self.bump();
                    }
                    let digits = &self.src[digits_start..self.pos];
                    let code =
                        if self.peek() == Some('}') && !digits.is_empty() && digits.len() <= 8 {
                            u32::from_str_radix(digits, 16).ok()
                        } else {
                            None
                        };
                    if self.peek() == Some('}') {
                        self.bump();
                    }
                    code
                } else {
                    self.hex_digits(4)
                };
                match code {
                    // A surrogate pair written as two escapes is one character.
                    Some(high @ 0xD800..=0xDBFF)
                        if self.rest().starts_with("\\u")
                            && self.rest().len() >= 6
                            && self.rest()[2..6].chars().all(|d| d.is_ascii_hexdigit()) =>
                    {
                        let low = u32::from_str_radix(&self.rest()[2..6], 16).unwrap_or(0);
                        if (0xDC00..=0xDFFF).contains(&low) {
                            for _ in 0..6 {
                                self.bump();
                            }
                            let combined = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                            out.push(char::from_u32(combined).unwrap_or('\u{fffd}'));
                        } else {
                            out.push('\u{fffd}');
                        }
                    }
                    Some(code) => out.push(char::from_u32(code).unwrap_or('\u{fffd}')),
                    None => out.push('\u{fffd}'),
                }
            }
            '\r' => {
                if self.peek() == Some('\n') {
                    self.bump();
                }
            }
            '\n' | '\u{2028}' | '\u{2029}' => {}
            other => out.push(other),
        }
        Ok(())
    }

    /// Reads exactly `count` hex digits, or none if they are not there.
    fn hex_digits(&mut self, count: usize) -> Option<u32> {
        let digits: String = self.rest().chars().take(count).collect();
        if digits.chars().count() == count && digits.chars().all(|d| d.is_ascii_hexdigit()) {
            for _ in 0..count {
                self.bump();
            }
            u32::from_str_radix(&digits, 16).ok()
        } else {
            None
        }
    }

    fn template(&mut self) -> Result<Tok, LexError> {
        let line = self.line;
        let start = self.pos;
        self.bump();
        let mut out = String::new();
        let mut interpolated = false;
        loop {
            match self.bump() {
                None => return Err(self.error(line, "a template literal is never closed")),
                Some('`') => break,
                Some('\\') => self.js_escape(&mut out, line)?,
                Some('$') if self.peek() == Some('{') => {
                    interpolated = true;
                    self.bump();
                    self.skip_substitution(line)?;
                }
                Some(c) => out.push(c),
            }
        }
        Ok(if interpolated {
            Tok::Interpolated(self.src[start..self.pos].to_owned())
        } else {
            Tok::Str(out)
        })
    }

    /// Skips a `${ ... }` substitution, strings inside it included.
    fn skip_substitution(&mut self, line: usize) -> Result<(), LexError> {
        let mut depth = 1_usize;
        while depth > 0 {
            match self.bump() {
                None => return Err(self.error(line, "a template literal is never closed")),
                Some('{') => depth += 1,
                Some('}') => depth -= 1,
                Some(quote @ ('\'' | '"' | '`')) => loop {
                    match self.bump() {
                        None => {
                            return Err(self.error(line, "a template literal is never closed"));
                        }
                        Some('\\') => {
                            self.bump();
                        }
                        Some(c) if c == quote => break,
                        Some(_) => {}
                    }
                },
                Some(_) => {}
            }
        }
        Ok(())
    }

    /// A regular-expression literal, or `None` (with nothing consumed) when
    /// the `/` cannot start one on this line.
    fn regex(&mut self) -> Option<Tok> {
        let rest = self.rest();
        let mut chars = rest.char_indices().skip(1);
        let mut in_class = false;
        let mut end = None;
        while let Some((at, c)) = chars.next() {
            match c {
                '\n' | '\r' => return None,
                '\\' => {
                    if matches!(chars.next(), None | Some((_, '\n'))) {
                        return None;
                    }
                }
                '[' => in_class = true,
                ']' => in_class = false,
                '/' if !in_class => {
                    end = Some(at);
                    break;
                }
                _ => {}
            }
        }
        let end = end?;
        if end == 1 {
            // `//` is a comment, handled before this.
            return None;
        }
        let pattern = rest[1..end].to_owned();
        let flags: String = rest[end + 1..]
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect();
        let consumed = end + 1 + flags.len();
        for _ in rest[..consumed].chars() {
            self.bump();
        }
        Some(Tok::Regex { pattern, flags })
    }

    /// The prefix length and quote of a Python string starting here, if one does.
    fn python_string_start(&self) -> Option<(usize, char)> {
        let rest = self.rest();
        let prefix_len = rest
            .chars()
            .take_while(|c| matches!(c.to_ascii_lowercase(), 'r' | 'b' | 'u' | 'f'))
            .take(2)
            .count();
        let quote = rest[prefix_len..].chars().next()?;
        if !matches!(quote, '\'' | '"') {
            return None;
        }
        let prefix = rest[..prefix_len].to_ascii_lowercase();
        let valid = matches!(
            prefix.as_str(),
            "" | "r" | "b" | "u" | "f" | "rb" | "br" | "fr" | "rf"
        );
        valid.then_some((prefix_len, quote))
    }

    fn python_string(&mut self, prefix_len: usize, quote: char) -> Result<Tok, LexError> {
        let line = self.line;
        let start = self.pos;
        let prefix = self.rest()[..prefix_len].to_ascii_lowercase();
        let raw = prefix.contains('r');
        let formatted = prefix.contains('f');
        for _ in 0..prefix_len {
            self.bump();
        }
        let triple: String = std::iter::repeat_n(quote, 3).collect();
        let is_triple = self.rest().starts_with(&triple);
        for _ in 0..(if is_triple { 3 } else { 1 }) {
            self.bump();
        }
        let mut out = String::new();
        let mut placeholder = false;
        loop {
            if is_triple && self.rest().starts_with(&triple) {
                for _ in 0..3 {
                    self.bump();
                }
                break;
            }
            let Some(c) = self.bump() else {
                return Err(self.error(line, "a string literal is never closed"));
            };
            match c {
                c if c == quote && !is_triple => break,
                '\n' if !is_triple => {
                    return Err(self.error(line, "a string literal is never closed"));
                }
                '\\' if raw => {
                    // A raw string keeps the backslash; it only stops the
                    // next quote from closing the string.
                    out.push('\\');
                    if let Some(next) = self.bump() {
                        out.push(next);
                    }
                }
                '\\' => self.python_escape(&mut out, line)?,
                '{' if formatted => {
                    if self.peek() == Some('{') {
                        self.bump();
                        out.push('{');
                    } else {
                        placeholder = true;
                        out.push('{');
                    }
                }
                '}' if formatted && self.peek() == Some('}') => {
                    self.bump();
                    out.push('}');
                }
                c => out.push(c),
            }
        }
        Ok(if placeholder {
            Tok::Interpolated(self.src[start..self.pos].to_owned())
        } else {
            Tok::Str(out)
        })
    }

    /// Decodes one Python escape (the backslash is already read).
    fn python_escape(&mut self, out: &mut String, line: usize) -> Result<(), LexError> {
        let Some(c) = self.bump() else {
            return Err(self.error(line, "a string literal is never closed"));
        };
        match c {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'a' => out.push('\u{7}'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            '\\' | '\'' | '"' => out.push(c),
            '\n' => {}
            '\r' => {
                if self.peek() == Some('\n') {
                    self.bump();
                }
            }
            'x' => {
                let code = self.hex_digits(2);
                out.push(code.and_then(char::from_u32).unwrap_or('\u{fffd}'));
            }
            'u' => {
                let code = self.hex_digits(4);
                out.push(code.and_then(char::from_u32).unwrap_or('\u{fffd}'));
            }
            'U' => {
                let code = self.hex_digits(8);
                out.push(code.and_then(char::from_u32).unwrap_or('\u{fffd}'));
            }
            '0'..='7' => {
                let mut code = c.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    match self.peek().and_then(|d| d.to_digit(8)) {
                        Some(d) => {
                            self.bump();
                            code = code * 8 + d;
                        }
                        None => break,
                    }
                }
                out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
            }
            other => {
                // Python keeps an unknown escape as written.
                out.push('\\');
                out.push(other);
            }
        }
        Ok(())
    }
}

fn is_ident_start(c: char, dialect: Dialect) -> bool {
    c == '_' || (c == '$' && dialect == Dialect::Js) || c.is_alphabetic()
}

fn is_ident_continue(c: char, dialect: Dialect) -> bool {
    is_ident_start(c, dialect) || c.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(source: &str, dialect: Dialect) -> Vec<Tok> {
        tokenize(source, dialect)
            .unwrap_or_else(|e| panic!("{e}"))
            .tokens
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }

    #[test]
    fn javascript_strings_are_decoded() {
        assert_eq!(
            toks(
                r#"'a\'b' "c\"d\n" `e${"}"}f` `g\u{41}\x42\u0043`"#,
                Dialect::Js
            ),
            vec![
                Tok::Str("a'b".into()),
                Tok::Str("c\"d\n".into()),
                Tok::Interpolated("`e${\"}\"}f`".into()),
                Tok::Str("gABC".into()),
            ]
        );
    }

    #[test]
    fn a_surrogate_pair_escape_is_one_character() {
        assert_eq!(
            toks(r#""\uD83D\uDE00 \uD83D""#, Dialect::Js),
            vec![Tok::Str("\u{1F600} \u{fffd}".into())]
        );
    }

    #[test]
    fn python_string_prefixes_and_triple_quotes() {
        assert_eq!(
            toks(
                "r'a\\b' b\"c\" f'{{x}}' f'{y}' '''multi\nline''' \"\\x41\\101\"",
                Dialect::Python
            ),
            vec![
                Tok::Str("a\\b".into()),
                Tok::Str("c".into()),
                Tok::Str("{x}".into()),
                Tok::Interpolated("f'{y}'".into()),
                Tok::Str("multi\nline".into()),
                Tok::Str("AA".into()),
            ]
        );
    }

    #[test]
    fn a_slash_is_a_regex_only_where_a_value_can_start() {
        assert_eq!(
            toks("a / b; x = /a\\/[/]b/gi;", Dialect::Js),
            vec![
                Tok::Ident("a".into()),
                Tok::Punct("/"),
                Tok::Ident("b".into()),
                Tok::Punct(";"),
                Tok::Ident("x".into()),
                Tok::Punct("="),
                Tok::Regex {
                    pattern: "a\\/[/]b".into(),
                    flags: "gi".into()
                },
                Tok::Punct(";"),
            ]
        );
    }

    #[test]
    fn comments_are_kept_apart_from_tokens() {
        let lexed = tokenize("a // one\n/* two\n * three */ b # c", Dialect::Js);
        // `#` is a punctuator in JavaScript.
        let lexed = lexed.unwrap();
        let texts: Vec<_> = lexed.comments.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["one", "two three"]);
        assert_eq!(lexed.comments[1].line, 2);
        let python = tokenize("a # it's\nb", Dialect::Python).unwrap();
        assert_eq!(python.comments[0].text, "it's");
        assert_eq!(python.tokens.len(), 2);
    }

    #[test]
    fn python_logical_lines_ignore_breaks_inside_brackets_and_after_a_backslash() {
        let lexed = tokenize("f(a,\n  b)\nx = 1 + \\\n  2\ny", Dialect::Python).unwrap();
        let starts: Vec<_> = lexed
            .tokens
            .iter()
            .filter(|t| t.line_start)
            .map(|t| t.tok.clone())
            .collect();
        assert_eq!(
            starts,
            vec![
                Tok::Ident("f".into()),
                Tok::Ident("x".into()),
                Tok::Ident("y".into())
            ]
        );
    }

    #[test]
    fn numbers_in_every_form() {
        assert_eq!(
            toks("1 2.5 .5 1e3 0x1F 1_000 10n", Dialect::Js),
            [1.0, 2.5, 0.5, 1000.0, 31.0, 1000.0, 10.0]
                .into_iter()
                .map(Tok::Num)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unclosed_literals_name_their_line() {
        assert_eq!(tokenize("a\n'open", Dialect::Js).unwrap_err().line, 2);
        assert!(tokenize("/* never", Dialect::Js).is_err());
        assert!(tokenize("'''never", Dialect::Python).is_err());
        assert!(tokenize("`never", Dialect::Js).is_err());
    }

    #[test]
    fn a_nul_and_other_control_characters_stay_inside_strings() {
        assert_eq!(
            toks("'a\\0b\u{0}c'", Dialect::Js),
            vec![Tok::Str("a\0b\0c".into())]
        );
    }

    #[test]
    fn columns_count_tabs_to_the_next_multiple_of_eight() {
        let lexed = tokenize("if x:\n\ty\n        z", Dialect::Python).unwrap();
        let cols: Vec<_> = lexed.tokens.iter().map(|t| t.col).collect();
        assert_eq!(cols, [0, 3, 4, 8, 8]);
    }
}
