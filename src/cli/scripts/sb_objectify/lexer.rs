//! A small tokenizer for Rust source.
//!
//! It knows just enough to tell code from comments, strings and characters: identifiers,
//! punctuation, string literals (plain and raw) and, as one kind, everything else. Every token
//! keeps its byte range, so a caller can edit the source text around it.

/// What a token is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// An identifier or a keyword.
    Ident,
    /// A string literal, plain or raw.
    Str,
    /// One punctuation byte.
    Punct(u8),
    /// A number, a character, a byte or C string, a lifetime.
    Other,
}

/// One token and where it sits in the source.
#[derive(Clone, Copy, Debug)]
pub(super) struct Token {
    pub(super) kind: Kind,
    pub(super) start: usize,
    pub(super) end: usize,
}

/// The tokens of a source text, without its whitespace and comments.
#[derive(Debug, Default)]
pub(super) struct Lexed {
    pub(super) tokens: Vec<Token>,
    /// For a bracket, the index of the token that closes or opens it.
    pub(super) partner: Vec<Option<usize>>,
    /// The byte ranges of the strings and block comments that contain a line break.
    multiline: Vec<(usize, usize)>,
}

impl Lexed {
    /// The text of the token at `index`; empty when there is none.
    pub(super) fn text<'a>(&self, source: &'a str, index: usize) -> &'a str {
        self.tokens
            .get(index)
            .and_then(|token| source.get(token.start..token.end))
            .unwrap_or("")
    }

    /// Whether the token at `index` is the punctuation `byte`.
    pub(super) fn is_punct(&self, index: usize, byte: u8) -> bool {
        self.tokens
            .get(index)
            .is_some_and(|token| token.kind == Kind::Punct(byte))
    }

    /// Whether the token at `index` is the identifier `name`.
    pub(super) fn is_ident(&self, source: &str, index: usize, name: &str) -> bool {
        self.tokens
            .get(index)
            .is_some_and(|token| token.kind == Kind::Ident && self.text(source, index) == name)
    }

    /// Whether the line break at byte `position` lies inside a string or a block comment.
    pub(super) fn spans_line_break(&self, position: usize) -> bool {
        self.multiline
            .iter()
            .any(|&(start, end)| start <= position && position < end)
    }

    fn push(&mut self, kind: Kind, start: usize, end: usize) {
        self.tokens.push(Token { kind, start, end });
    }

    fn note_lines(&mut self, bytes: &[u8], start: usize, end: usize) {
        if bytes
            .get(start..end)
            .is_some_and(|text| text.contains(&b'\n'))
        {
            self.multiline.push((start, end));
        }
    }
}

/// Splits `source` into tokens.
pub(super) fn lex(source: &str) -> Lexed {
    let bytes = source.as_bytes();
    let mut lexed = Lexed::default();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let byte = bytes[i];
        if byte.is_ascii_whitespace() {
            i += 1;
        } else if byte == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if byte == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i = block_comment_end(bytes, i);
            lexed.note_lines(bytes, start, i);
        } else if byte == b'"' {
            i = string_end(bytes, i);
            lexed.push(Kind::Str, start, i);
            lexed.note_lines(bytes, start, i);
        } else if byte == b'\'' {
            i = quote_end(source, i);
            lexed.push(Kind::Other, start, i);
        } else if byte.is_ascii_digit() {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            lexed.push(Kind::Other, start, i);
        } else if is_word_byte(byte) {
            let (kind, end) = word(bytes, i);
            i = end;
            lexed.push(kind, start, i);
            lexed.note_lines(bytes, start, i);
        } else {
            i += 1;
            lexed.push(Kind::Punct(byte), start, i);
        }
    }
    lexed.partner = pair_brackets(&lexed.tokens);
    lexed
}

/// Whether `byte` can be part of an identifier. Bytes of non-ASCII characters count, so that a
/// character is never split.
fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

/// The end of the identifier that starts at `start`.
fn word_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && is_word_byte(bytes[i]) {
        i += 1;
    }
    i
}

/// The token that starts with the letter at `start`: an identifier, or a string with a prefix.
fn word(bytes: &[u8], start: usize) -> (Kind, usize) {
    let next = bytes.get(start + 1).copied();
    match bytes[start] {
        b'r' => {
            if let Some(end) = raw_string_end(bytes, start) {
                return (Kind::Str, end);
            }
            if next == Some(b'#') && bytes.get(start + 2).is_some_and(|b| is_word_byte(*b)) {
                return (Kind::Ident, word_end(bytes, start + 2));
            }
        }
        b'b' | b'c' => match next {
            Some(b'"') => return (Kind::Other, string_end(bytes, start + 1)),
            Some(b'\'') if bytes[start] == b'b' => {
                return (Kind::Other, char_end(bytes, start + 1));
            }
            Some(b'r') => {
                if let Some(end) = raw_string_end(bytes, start + 1) {
                    return (Kind::Other, end);
                }
            }
            _ => {}
        },
        _ => {}
    }
    (Kind::Ident, word_end(bytes, start))
}

/// The end of the block comment that starts at `start`; block comments nest.
fn block_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 0_usize;
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
            depth = depth.saturating_sub(1);
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    bytes.len()
}

/// The end of the string whose opening quote is at `quote`.
fn string_end(bytes: &[u8], quote: usize) -> usize {
    let mut i = quote + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The end of the raw string whose `r` is at `r`, or `None` when no raw string starts there.
fn raw_string_end(bytes: &[u8], r: usize) -> Option<usize> {
    let mut i = r + 1;
    let mut hashes = 0;
    while bytes.get(i) == Some(&b'#') {
        hashes += 1;
        i += 1;
    }
    if bytes.get(i) != Some(&b'"') {
        return None;
    }
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'"'
            && bytes
                .get(i + 1..i + 1 + hashes)
                .is_some_and(|tail| tail.iter().all(|b| *b == b'#'))
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    Some(bytes.len())
}

/// The end of the character literal whose opening quote is at `quote`.
fn char_end(bytes: &[u8], quote: usize) -> usize {
    let mut i = quote + 1;
    if bytes.get(i) == Some(&b'\\') {
        i += 2;
    }
    while i < bytes.len() && bytes[i] != b'\'' {
        i += 1;
    }
    (i + 1).min(bytes.len())
}

/// The end of what starts with the single quote at `quote`: a character or a lifetime.
fn quote_end(source: &str, quote: usize) -> usize {
    let bytes = source.as_bytes();
    match bytes.get(quote + 1) {
        None => quote + 1,
        Some(b'\\') => char_end(bytes, quote),
        Some(&first) => {
            let width = source
                .get(quote + 1..)
                .and_then(|rest| rest.chars().next())
                .map_or(1, char::len_utf8);
            if bytes.get(quote + 1 + width) == Some(&b'\'') {
                quote + 2 + width
            } else if is_word_byte(first) {
                word_end(bytes, quote + 1)
            } else {
                quote + 1
            }
        }
    }
}

/// For every bracket, the index of the one that closes or opens it.
fn pair_brackets(tokens: &[Token]) -> Vec<Option<usize>> {
    let mut partner = vec![None; tokens.len()];
    let mut open: Vec<usize> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Punct(b'(' | b'[' | b'{') => open.push(index),
            Kind::Punct(close @ (b')' | b']' | b'}')) => {
                let opening = match close {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                if let Some(&opener) = open.last() {
                    if tokens[opener].kind == Kind::Punct(opening) {
                        open.pop();
                        partner[opener] = Some(index);
                        partner[index] = Some(opener);
                    }
                }
            }
            _ => {}
        }
    }
    partner
}
