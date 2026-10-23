//! `sbrs test -k` and `-m`: pytest's name and marker expressions, translated
//! for the test runner that will run them.
//!
//! pytest selects tests with `-k "login and not slow"` (a word is a substring
//! of the test's name) and `-m smoke` (a marker). A Rust test has no markers
//! and libtest has no expression language, so the two options are mapped as
//! follows:
//!
//! * A **marker** is a module-path segment: `-m smoke` selects the tests in a
//!   `mod smoke { .. }`, whatever the module's depth.
//! * **libtest** (`cargo test`) can only run the tests whose name contains
//!   *one of* some words (positional filters are ORed) and leave out the tests
//!   that contain a word (`--skip`). An expression of the shape
//!   `(a or b) and not c and not d` is that, so it becomes the filters `a b`
//!   and the skips `c d`. Anything else (`a and b`, `not (a and b)`) is
//!   refused with the advice to use the nextest runner.
//! * **nextest** (`cargo nextest`) has an expression language of its own, the
//!   filterset, so `and`, `or`, `not` and parentheses carry over: a word `w`
//!   becomes `test(~w)`.
//!
//! pytest matches `-k` words without regard to case; both runners here are
//! case-sensitive.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scripts::run::Runner;
//! use seleniumbase_rs::cli::scripts::run_filter::{translate, NameFilter};
//!
//! let filter = translate(Some("login or signup"), None, Runner::Cargo)?;
//! assert_eq!(
//!     filter,
//!     NameFilter::Libtest {
//!         filters: vec!["login".into(), "signup".into()],
//!         skips: vec![],
//!     }
//! );
//!
//! let filter = translate(Some("a and b"), None, Runner::Nextest)?;
//! assert_eq!(
//!     filter,
//!     NameFilter::Nextest { filterset: "test(~a) and test(~b)".into() }
//! );
//! assert!(translate(Some("a and b"), None, Runner::Cargo).is_err());
//! # Ok::<(), seleniumbase_rs::cli::scripts::run_filter::FilterError>(())
//! ```

use std::fmt;

use super::run::Runner;

/// What a `-k` or `-m` expression turns into for one runner.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum NameFilter {
    /// For `cargo test`: positional filters (a test runs when its name
    /// contains one of them) and `--skip` names.
    Libtest {
        /// The names to run; empty means every test.
        filters: Vec<String>,
        /// The names to leave out.
        skips: Vec<String>,
    },
    /// For `cargo nextest`: a filterset expression (`-E`).
    Nextest {
        /// The filterset; empty means every test.
        filterset: String,
    },
    /// No `-k` and no `-m` was given.
    #[default]
    All,
}

impl NameFilter {
    /// Whether the filter selects every test.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Libtest { filters, skips } => filters.is_empty() && skips.is_empty(),
            Self::Nextest { filterset } => filterset.is_empty(),
            Self::All => true,
        }
    }
}

/// Why an expression could not be turned into a filter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilterError {
    /// The expression is not valid: an unbalanced parenthesis, a missing
    /// name, a stray operator.
    Syntax {
        /// The option, `-k` or `-m`.
        option: &'static str,
        /// What is wrong.
        message: String,
    },
    /// The expression needs more than libtest can do.
    NeedsNextest {
        /// The option and its expression as the user wrote them.
        written: String,
    },
    /// A word has a character that nextest's filterset cannot carry.
    BadWord {
        /// The option, `-k` or `-m`.
        option: &'static str,
        /// The word.
        word: String,
    },
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { option, message } => write!(f, "invalid {option} expression: {message}"),
            Self::NeedsNextest { written } => write!(
                f,
                "{written} needs the nextest runner (sbrs test --runner nextest): \
                 libtest can only OR names and skip names"
            ),
            Self::BadWord { option, word } => write!(
                f,
                "{option}: `{word}` has a character a test name filter cannot carry"
            ),
        }
    }
}

impl std::error::Error for FilterError {}

/// A parsed `-k` or `-m` expression.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Expr {
    Word(String),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

/// How a word becomes a test selector.
#[derive(Clone, Copy)]
enum Words {
    /// `-k`: the test's name contains the word.
    Name,
    /// `-m`: the word is a segment of the test's module path.
    Marker,
}

impl Words {
    fn option(self) -> &'static str {
        match self {
            Self::Name => "-k",
            Self::Marker => "-m",
        }
    }

    /// The libtest filter or skip for `word`.
    fn libtest(self, word: &str) -> String {
        match self {
            Self::Name => word.to_owned(),
            Self::Marker => format!("{word}::"),
        }
    }

    /// The filterset predicate for `word`.
    fn predicate(self, word: &str) -> String {
        match self {
            Self::Name => format!("test(~{word})"),
            Self::Marker => {
                let escaped: String = word
                    .chars()
                    .flat_map(|c| {
                        let plain = c.is_alphanumeric() || c == '_';
                        let escape = (!plain).then_some('\\');
                        escape.into_iter().chain(std::iter::once(c))
                    })
                    .collect();
                format!("test(/(^|::){escaped}(::|$)/)")
            }
        }
    }
}

/// The characters a word may not have: parentheses and spaces end a word, and
/// these are special in a filterset.
fn carriable(word: &str) -> bool {
    !word.chars().any(|c| matches!(c, '\\' | ',' | '"' | '\''))
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        match c {
            '(' | ')' => {
                if !word.is_empty() {
                    tokens.push(std::mem::take(&mut word));
                }
                tokens.push(c.to_string());
            }
            c if c.is_whitespace() => {
                if !word.is_empty() {
                    tokens.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

struct Parser {
    tokens: Vec<String>,
    at: usize,
    option: &'static str,
}

impl Parser {
    fn syntax(&self, message: impl Into<String>) -> FilterError {
        FilterError::Syntax {
            option: self.option,
            message: message.into(),
        }
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.at).map(String::as_str)
    }

    fn expr(&mut self) -> Result<Expr, FilterError> {
        let mut items = vec![self.and()?];
        while self.peek() == Some("or") {
            self.at += 1;
            items.push(self.and()?);
        }
        Ok(if items.len() == 1 {
            items.remove(0)
        } else {
            Expr::Or(items)
        })
    }

    fn and(&mut self) -> Result<Expr, FilterError> {
        let mut items = vec![self.not()?];
        while self.peek() == Some("and") {
            self.at += 1;
            items.push(self.not()?);
        }
        Ok(if items.len() == 1 {
            items.remove(0)
        } else {
            Expr::And(items)
        })
    }

    fn not(&mut self) -> Result<Expr, FilterError> {
        match self.peek() {
            Some("not") => {
                self.at += 1;
                Ok(Expr::Not(Box::new(self.not()?)))
            }
            Some("(") => {
                self.at += 1;
                let inner = self.expr()?;
                if self.peek() == Some(")") {
                    self.at += 1;
                    Ok(inner)
                } else {
                    Err(self.syntax("missing a closing parenthesis"))
                }
            }
            Some(")") => Err(self.syntax("unexpected closing parenthesis")),
            Some(word @ ("and" | "or")) => {
                Err(self.syntax(format!("expected a name before `{word}`")))
            }
            Some(word) => {
                let word = word.to_owned();
                self.at += 1;
                Ok(Expr::Word(word))
            }
            None => Err(self.syntax("expected a name at the end")),
        }
    }
}

fn parse(text: &str, option: &'static str) -> Result<Expr, FilterError> {
    let mut parser = Parser {
        tokens: tokenize(text),
        at: 0,
        option,
    };
    if parser.tokens.is_empty() {
        return Err(parser.syntax("the expression is empty"));
    }
    let expr = parser.expr()?;
    match parser.peek() {
        None => Ok(expr),
        Some(")") => Err(parser.syntax("unexpected closing parenthesis")),
        Some(word) => Err(parser.syntax(format!("unexpected `{word}`"))),
    }
}

/// The words of an `or` of words (nested ors included), or `None` when the
/// expression is anything else.
fn word_alternatives(expr: &Expr) -> Option<Vec<&str>> {
    match expr {
        Expr::Word(word) => Some(vec![word]),
        Expr::Or(items) => {
            let mut words = Vec::new();
            for item in items {
                words.extend(word_alternatives(item)?);
            }
            Some(words)
        }
        Expr::Not(_) | Expr::And(_) => None,
    }
}

fn conjuncts<'e>(expr: &'e Expr, out: &mut Vec<&'e Expr>) {
    match expr {
        Expr::And(items) => items.iter().for_each(|item| conjuncts(item, out)),
        other => out.push(other),
    }
}

/// What libtest can run for one expression: the alternatives that must match
/// (at most one group) and the names to skip. `None` when it is more than that.
type LibtestParts = (Vec<Vec<String>>, Vec<String>);

fn libtest_parts(expr: &Expr, words: Words) -> Option<LibtestParts> {
    let mut parts = Vec::new();
    conjuncts(expr, &mut parts);
    let (mut groups, mut skips) = (Vec::new(), Vec::new());
    for part in parts {
        match part {
            Expr::Not(inner) => skips.extend(
                word_alternatives(inner)?
                    .into_iter()
                    .map(|word| words.libtest(word)),
            ),
            other => groups.push(
                word_alternatives(other)?
                    .into_iter()
                    .map(|word| words.libtest(word))
                    .collect(),
            ),
        }
    }
    Some((groups, skips))
}

fn filterset(expr: &Expr, words: Words, in_and: bool, in_not: bool) -> String {
    match expr {
        Expr::Word(word) => words.predicate(word),
        Expr::Not(inner) => format!("not {}", filterset(inner, words, false, true)),
        Expr::And(items) => {
            let text = items
                .iter()
                .map(|item| filterset(item, words, true, false))
                .collect::<Vec<_>>()
                .join(" and ");
            if in_not {
                format!("({text})")
            } else {
                text
            }
        }
        Expr::Or(items) => {
            let text = items
                .iter()
                .map(|item| filterset(item, words, false, false))
                .collect::<Vec<_>>()
                .join(" or ");
            if in_and || in_not {
                format!("({text})")
            } else {
                text
            }
        }
    }
}

fn check_words(expr: &Expr, option: &'static str) -> Result<(), FilterError> {
    match expr {
        Expr::Word(word) if !carriable(word) => Err(FilterError::BadWord {
            option,
            word: word.clone(),
        }),
        Expr::Word(_) => Ok(()),
        Expr::Not(inner) => check_words(inner, option),
        Expr::And(items) | Expr::Or(items) => {
            items.iter().try_for_each(|item| check_words(item, option))
        }
    }
}

/// Turns the `-k` and `-m` expressions into what `runner` takes.
///
/// # Errors
///
/// [`FilterError::Syntax`] for an expression that does not parse,
/// [`FilterError::NeedsNextest`] for one that libtest cannot express (`and` of
/// two names, `-k` and `-m` that both name tests to run, a negated `and`), and
/// [`FilterError::BadWord`] for a word with a quote, a comma or a backslash.
pub fn translate(
    k: Option<&str>,
    m: Option<&str>,
    runner: Runner,
) -> Result<NameFilter, FilterError> {
    let mut exprs = Vec::new();
    for (text, words) in [(k, Words::Name), (m, Words::Marker)] {
        if let Some(text) = text {
            let expr = parse(text, words.option())?;
            check_words(&expr, words.option())?;
            exprs.push((words, text, expr));
        }
    }
    if exprs.is_empty() {
        return Ok(NameFilter::All);
    }
    match runner {
        Runner::Nextest => {
            let single = exprs.len() == 1;
            let parts: Vec<String> = exprs
                .iter()
                .map(|(words, _, expr)| {
                    let text = filterset(expr, *words, false, false);
                    if single || !matches!(expr, Expr::Or(_)) {
                        text
                    } else {
                        format!("({text})")
                    }
                })
                .collect();
            Ok(NameFilter::Nextest {
                filterset: parts.join(" and "),
            })
        }
        Runner::Cargo => {
            let written = exprs
                .iter()
                .map(|(words, text, _)| format!("{} \"{text}\"", words.option()))
                .collect::<Vec<_>>()
                .join(" with ");
            let refused = || FilterError::NeedsNextest {
                written: written.clone(),
            };
            let mut filters: Vec<String> = Vec::new();
            let mut skips = Vec::new();
            let mut groups = 0;
            for (words, _, expr) in &exprs {
                let (found, skipped) = libtest_parts(expr, *words).ok_or_else(refused)?;
                for group in found {
                    groups += 1;
                    filters = group;
                }
                skips.extend(skipped);
            }
            if groups > 1 {
                return Err(refused());
            }
            Ok(NameFilter::Libtest { filters, skips })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn libtest(k: &str) -> (Vec<String>, Vec<String>) {
        match translate(Some(k), None, Runner::Cargo).unwrap_or_else(|e| panic!("{k}: {e}")) {
            NameFilter::Libtest { filters, skips } => (filters, skips),
            other => panic!("{k}: {other:?}"),
        }
    }

    fn nextest(k: Option<&str>, m: Option<&str>) -> String {
        match translate(k, m, Runner::Nextest).expect("translates") {
            NameFilter::Nextest { filterset } => filterset,
            other => panic!("{other:?}"),
        }
    }

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn a_word_or_an_or_of_words_are_positional_filters() {
        assert_eq!(libtest("login"), (strings(&["login"]), vec![]));
        assert_eq!(
            libtest("login or signup"),
            (strings(&["login", "signup"]), vec![])
        );
        assert_eq!(
            libtest("(a or b) or c"),
            (strings(&["a", "b", "c"]), vec![])
        );
    }

    #[test]
    fn a_not_is_a_skip() {
        assert_eq!(libtest("not admin"), (vec![], strings(&["admin"])));
        assert_eq!(libtest("not a and not b"), (vec![], strings(&["a", "b"])));
        assert_eq!(libtest("not (a or b)"), (vec![], strings(&["a", "b"])));
    }

    #[test]
    fn filters_and_skips_combine() {
        assert_eq!(
            libtest("(a or b) and not c and not d"),
            (strings(&["a", "b"]), strings(&["c", "d"]))
        );
        assert_eq!(
            libtest("login and not slow"),
            (strings(&["login"]), strings(&["slow"]))
        );
    }

    #[test]
    fn what_libtest_cannot_do_names_the_nextest_runner() {
        for expr in [
            "a and b",
            "not (a and b)",
            "(a or b) and (c or d)",
            "not not a",
        ] {
            let error = translate(Some(expr), None, Runner::Cargo).expect_err(expr);
            assert_eq!(
                error.to_string(),
                format!(
                    "-k \"{expr}\" needs the nextest runner (sbrs test --runner nextest): \
                     libtest can only OR names and skip names"
                )
            );
        }
    }

    #[test]
    fn nextest_keeps_the_expression() {
        assert_eq!(nextest(Some("a and b"), None), "test(~a) and test(~b)");
        assert_eq!(nextest(Some("a or b"), None), "test(~a) or test(~b)");
        assert_eq!(nextest(Some("not a"), None), "not test(~a)");
        assert_eq!(
            nextest(Some("(a or b) and not c"), None),
            "(test(~a) or test(~b)) and not test(~c)"
        );
        assert_eq!(
            nextest(Some("not (a and b)"), None),
            "not (test(~a) and test(~b))"
        );
        assert_eq!(
            nextest(Some("not (a or b)"), None),
            "not (test(~a) or test(~b))"
        );
    }

    #[test]
    fn a_marker_is_a_module_path_segment() {
        let NameFilter::Libtest { filters, skips } =
            translate(None, Some("smoke or slow"), Runner::Cargo).unwrap()
        else {
            panic!("not libtest")
        };
        assert_eq!(filters, strings(&["smoke::", "slow::"]));
        assert!(skips.is_empty());
        let NameFilter::Libtest { filters, skips } =
            translate(None, Some("not slow"), Runner::Cargo).unwrap()
        else {
            panic!("not libtest")
        };
        assert!(filters.is_empty());
        assert_eq!(skips, strings(&["slow::"]));
        assert_eq!(nextest(None, Some("smoke")), "test(/(^|::)smoke(::|$)/)");
        assert_eq!(nextest(None, Some("a.b")), "test(/(^|::)a\\.b(::|$)/)");
    }

    #[test]
    fn k_and_m_together_are_anded() {
        assert_eq!(
            nextest(Some("login or signup"), Some("smoke")),
            "(test(~login) or test(~signup)) and test(/(^|::)smoke(::|$)/)"
        );
        // One positive group and skips from both stay libtest.
        let NameFilter::Libtest { filters, skips } =
            translate(Some("login"), Some("not slow"), Runner::Cargo).unwrap()
        else {
            panic!("not libtest")
        };
        assert_eq!(filters, strings(&["login"]));
        assert_eq!(skips, strings(&["slow::"]));
        // Two positive groups cannot be ANDed by libtest.
        let error = translate(Some("login"), Some("smoke"), Runner::Cargo).unwrap_err();
        assert_eq!(
            error,
            FilterError::NeedsNextest {
                written: "-k \"login\" with -m \"smoke\"".into()
            }
        );
    }

    #[test]
    fn no_option_selects_everything() {
        assert_eq!(translate(None, None, Runner::Cargo), Ok(NameFilter::All));
        assert!(NameFilter::All.is_empty());
        assert!(!NameFilter::Nextest {
            filterset: "x".into()
        }
        .is_empty());
    }

    #[test]
    fn a_broken_expression_is_a_syntax_error() {
        for (expr, message) in [
            ("", "the expression is empty"),
            ("   ", "the expression is empty"),
            ("a and", "expected a name at the end"),
            ("and a", "expected a name before `and`"),
            ("(a", "missing a closing parenthesis"),
            ("a)", "unexpected closing parenthesis"),
            ("a b", "unexpected `b`"),
            ("a or or b", "expected a name before `or`"),
            ("not", "expected a name at the end"),
        ] {
            for runner in [Runner::Cargo, Runner::Nextest] {
                assert_eq!(
                    translate(Some(expr), None, runner),
                    Err(FilterError::Syntax {
                        option: "-k",
                        message: message.into()
                    }),
                    "{expr:?}"
                );
            }
        }
        assert_eq!(
            translate(None, Some("a and"), Runner::Cargo)
                .unwrap_err()
                .to_string(),
            "invalid -m expression: expected a name at the end"
        );
    }

    #[test]
    fn a_word_with_a_quote_comma_or_backslash_is_refused() {
        for word in ["a,b", "a\\b", "a\"b", "a'b"] {
            assert_eq!(
                translate(Some(word), None, Runner::Nextest),
                Err(FilterError::BadWord {
                    option: "-k",
                    word: word.into()
                })
            );
        }
    }

    #[test]
    fn rust_paths_are_words() {
        assert_eq!(
            libtest("login::valid_password"),
            (strings(&["login::valid_password"]), vec![])
        );
        assert_eq!(nextest(Some("a::b[1]"), None), "test(~a::b[1])");
    }
}
