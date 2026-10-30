//! Finds the selectors in the source of a test: the string literals that are the selector
//! argument of a call.

use std::ops::Range;

use super::lexer::{Kind, Lexed};
use super::{is_navigation_method, selector_positions};

/// A string literal that is a selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Found {
    /// Where the literal starts in the source.
    pub(super) start: usize,
    /// Where the literal ends.
    pub(super) end: usize,
    /// The literal as written, quotes and `r#` included.
    pub(super) literal: String,
    /// The URL that the last `open` of the same function loaded, if there was one.
    pub(super) url: Option<String>,
    /// How many inline `mod` blocks hold the call.
    pub(super) mod_depth: usize,
}

/// The text between the quotes of a string literal, plain or raw.
pub(super) fn literal_value(literal: &str) -> &str {
    match (literal.find('"'), literal.rfind('"')) {
        (Some(first), Some(last)) if last > first => &literal[first + 1..last],
        _ => "",
    }
}

/// The selectors of `source`, in the order they are written.
pub(super) fn scan(source: &str, lexed: &Lexed) -> Vec<Found> {
    let mut found = Vec::new();
    let mut url: Option<String> = None;
    let mut blocks: Vec<bool> = Vec::new();
    let mut mod_depth = 0_usize;
    for (index, token) in lexed.tokens.iter().enumerate() {
        match token.kind {
            Kind::Punct(b'{') => {
                let is_mod = index >= 2
                    && lexed.is_ident(source, index - 2, "mod")
                    && lexed.tokens[index - 1].kind == Kind::Ident;
                blocks.push(is_mod);
                if is_mod {
                    mod_depth += 1;
                }
            }
            Kind::Punct(b'}') => {
                if blocks.pop() == Some(true) {
                    mod_depth = mod_depth.saturating_sub(1);
                }
            }
            Kind::Ident => {
                let name = lexed.text(source, index);
                if name == "fn" {
                    url = None;
                    continue;
                }
                let is_call =
                    index > 0 && lexed.is_punct(index - 1, b'.') && lexed.is_punct(index + 1, b'(');
                let close = lexed.partner.get(index + 1).copied().flatten();
                let (true, Some(close)) = (is_call, close) else {
                    continue;
                };
                let arguments = arguments(lexed, index + 2, close);
                if is_navigation_method(name) {
                    url = literal_argument(source, lexed, &arguments, 0)
                        .map(|(_, _, text)| literal_value(text).to_owned());
                } else {
                    for &position in selector_positions(name) {
                        if let Some((start, end, text)) =
                            literal_argument(source, lexed, &arguments, position)
                        {
                            found.push(Found {
                                start,
                                end,
                                literal: text.to_owned(),
                                url: url.clone(),
                                mod_depth,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// The token ranges of the arguments between the parenthesis that opens before `from` and the
/// one at `close`.
fn arguments(lexed: &Lexed, from: usize, close: usize) -> Vec<Range<usize>> {
    let mut arguments = Vec::new();
    let mut begin = from;
    let mut i = from;
    while i < close {
        match lexed.tokens[i].kind {
            Kind::Punct(b'(' | b'[' | b'{') => {
                if let Some(Some(end)) = lexed.partner.get(i) {
                    i = *end;
                }
            }
            Kind::Punct(b',') => {
                if begin < i {
                    arguments.push(begin..i);
                }
                begin = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if begin < close {
        arguments.push(begin..close);
    }
    arguments
}

/// The string literal that is, on its own, the argument at `position`: where it starts and ends
/// and its text. A literal that spans lines is not one.
fn literal_argument<'a>(
    source: &'a str,
    lexed: &Lexed,
    arguments: &[Range<usize>],
    position: usize,
) -> Option<(usize, usize, &'a str)> {
    let range = arguments.get(position)?;
    if range.len() != 1 {
        return None;
    }
    let token = lexed.tokens.get(range.start)?;
    if token.kind != Kind::Str {
        return None;
    }
    let text = source.get(token.start..token.end)?;
    if text.contains(['\n', '\r']) {
        return None;
    }
    Some((token.start, token.end, text))
}
