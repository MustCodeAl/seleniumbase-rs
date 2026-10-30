//! The translator's state: what it has written, the names in scope and the
//! diagnostics. The translation itself is in `expr`, `stmt` and `cdp`.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::ast::Expr;
use crate::utilities::importer::lexer::Comment;
use crate::utilities::python_importer::{ImportDiagnostic, ImportSeverity};

/// What the translator knows about the Rust type of a value.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Ty {
    /// A borrowed string (`&str`, or a `&String` that derefs to one).
    Str,
    /// An owned `String`.
    String,
    /// A whole number.
    Int,
    /// A float.
    Float,
    /// A `bool`.
    Bool,
    /// A `Vec` of the element type (as it is when iterated by reference).
    List(Box<Ty>),
    /// A pure CDP `Locator`.
    Locator,
    /// The alert `switch_to_alert` returns.
    Alert,
    /// Nothing.
    Unit,
    /// Not known.
    Unknown,
}

impl Ty {
    /// A string, owned or borrowed.
    pub(super) fn is_text(&self) -> bool {
        matches!(self, Self::Str | Self::String)
    }

    /// A number.
    pub(super) fn is_number(&self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }
}

/// A translated expression.
#[derive(Clone, Debug)]
pub(super) struct Val {
    /// The Rust code.
    pub(super) code: String,
    /// Its type.
    pub(super) ty: Ty,
    /// It needs brackets when it is an operand.
    pub(super) compound: bool,
}

impl Val {
    /// A value that is one term.
    pub(super) fn new(code: impl Into<String>, ty: Ty) -> Self {
        Self {
            code: code.into(),
            ty,
            compound: false,
        }
    }

    /// A value written with an operator.
    pub(super) fn compound(code: impl Into<String>, ty: Ty) -> Self {
        Self {
            code: code.into(),
            ty,
            compound: true,
        }
    }

    /// The code, in brackets when it is compound.
    pub(super) fn operand(&self) -> String {
        if self.compound {
            format!("({})", self.code)
        } else {
            self.code.clone()
        }
    }
}

/// Why a statement or expression was not translated.
#[derive(Clone, Debug)]
pub(super) struct Skip {
    /// An error (the call cannot be written as it is) or a warning (the
    /// converter does not read it).
    pub(super) error: bool,
    /// What is wrong.
    pub(super) message: String,
}

impl Skip {
    /// The converter does not read this.
    pub(super) fn warn(message: impl Into<String>) -> Self {
        Self {
            error: false,
            message: message.into(),
        }
    }

    /// This cannot be written in Rust as it is.
    pub(super) fn error(message: impl Into<String>) -> Self {
        Self {
            error: true,
            message: message.into(),
        }
    }
}

/// A result that may be a [`Skip`].
pub(super) type TResult<T> = Result<T, Skip>;

/// A variable that was declared with `let`.
#[derive(Clone, Debug)]
pub(super) struct Var {
    /// Its Rust name.
    pub(super) rust: String,
    /// Its type.
    pub(super) ty: Ty,
    /// Where its `let` is in the output.
    pub(super) line_index: usize,
}

/// The state of one translation: one test.
pub(super) struct Tr<'a> {
    /// Where diagnostics go.
    pub(super) diags: &'a mut Vec<ImportDiagnostic>,
    /// The output: indentation level and text of each line.
    pub(super) out: Vec<(usize, String)>,
    /// The current indentation level.
    pub(super) depth: usize,
    /// The variables in scope, innermost last.
    pub(super) scopes: Vec<HashMap<String, Var>>,
    /// The test drives the pure CDP engine.
    pub(super) cdp: bool,
    /// The Python names that mean the browser (`self`, `sb`).
    pub(super) receivers: HashSet<String>,
    /// The names `sb_cdp` is imported under in `seleniumbase_rs::sb_cdp`.
    pub(super) imports: BTreeSet<&'static str>,
    /// The comments of the file.
    pub(super) comments: &'a [Comment],
    /// The lines that hold code.
    pub(super) code_lines: &'a HashSet<usize>,
    /// The comments up to this line are written or dropped.
    pub(super) comment_floor: usize,
    /// Comments are written (not in `setUp` and `tearDown`).
    pub(super) with_comments: bool,
    /// The Rust names taken by temporaries.
    pub(super) taken: HashSet<String>,
    /// The Python source, for messages.
    pub(super) source: &'a str,
    /// Warnings about the statement being translated (dropped arguments).
    pub(super) notes: Vec<String>,
    /// The Python names whose assignment was not converted, so a later use is
    /// a consequence and not a new error.
    pub(super) failed: HashSet<String>,
}

impl<'a> Tr<'a> {
    /// A translator with an empty scope.
    pub(super) fn new(
        diags: &'a mut Vec<ImportDiagnostic>,
        comments: &'a [Comment],
        code_lines: &'a HashSet<usize>,
        source: &'a str,
    ) -> Self {
        Self {
            diags,
            out: Vec::new(),
            depth: 0,
            scopes: vec![HashMap::new()],
            cdp: false,
            receivers: ["self", "sb"]
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            imports: BTreeSet::new(),
            comments,
            code_lines,
            comment_floor: 0,
            with_comments: true,
            taken: HashSet::new(),
            notes: Vec::new(),
            failed: HashSet::new(),

            source,
        }
    }

    /// Writes one line at the current indentation.
    pub(super) fn emit(&mut self, text: impl Into<String>) {
        self.out.push((self.depth, text.into()));
    }

    /// Records a diagnostic.
    pub(super) fn diag(&mut self, line: usize, error: bool, message: impl Into<String>) {
        self.diags.push(ImportDiagnostic {
            line,
            severity: if error {
                ImportSeverity::Error
            } else {
                ImportSeverity::Warning
            },
            message: message.into(),
        });
    }

    /// Writes a `TODO` comment for `skip` and records it.
    pub(super) fn todo(&mut self, line: usize, skip: &Skip, python: &str) {
        let message = one_line(&skip.message);
        self.diag(line, skip.error, message.clone());
        self.emit(format!("// TODO(source line {line}): {message}"));
        let python = one_line(python);
        if !python.is_empty() {
            self.emit(format!("//     {}", truncate(&python, 90)));
        }
    }

    /// The variable `name`, innermost scope first.
    pub(super) fn var(&self, name: &str) -> Option<&Var> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    /// Declares `name` in the innermost scope; the caller writes the `let`
    /// as the next line. Returns the Rust name.
    pub(super) fn declare(&mut self, name: &str, ty: Ty) -> String {
        let rust = rust_ident(name);
        let line_index = self.out.len();
        self.taken.insert(rust.clone());
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.to_owned(),
                Var {
                    rust: rust.clone(),
                    ty,
                    line_index,
                },
            );
        }
        rust
    }

    /// Makes the `let` of a variable `let mut`.
    pub(super) fn make_mutable(&mut self, line_index: usize) {
        if let Some((_, text)) = self.out.get_mut(line_index) {
            if let Some(rest) = text.strip_prefix("let ") {
                if !rest.starts_with("mut ") {
                    *text = format!("let mut {rest}");
                }
            }
        }
    }

    /// A fresh Rust name that starts like `hint`.
    pub(super) fn temp(&mut self, hint: &str) -> String {
        let base = rust_ident(hint);
        let mut name = base.clone();
        let mut count = 1;
        while self.taken.contains(&name) || self.var_named(&name) {
            count += 1;
            name = format!("{base}_{count}");
        }
        self.taken.insert(name.clone());
        name
    }

    fn var_named(&self, rust: &str) -> bool {
        self.scopes
            .iter()
            .any(|scope| scope.values().any(|var| var.rust == rust))
    }

    /// Writes the comments that come before `line` (own-line comments first,
    /// then the ones that trail code a statement before).
    pub(super) fn flush_comments(&mut self, before: usize) {
        if !self.with_comments {
            return;
        }
        let comments = self.comments;
        // The comments are in line order: skip to the first one past the floor.
        let first = comments.partition_point(|comment| comment.line <= self.comment_floor);
        for comment in &comments[first..] {
            if comment.line >= before {
                break;
            }
            let text = comment_text(&comment.text);
            self.emit(text);
        }
        self.comment_floor = self.comment_floor.max(before.saturating_sub(1));
    }

    /// Appends the comment that trails code on `line` to the last line written.
    pub(super) fn trailing_comment(&mut self, line: usize) {
        if !self.with_comments || !self.code_lines.contains(&line) || line <= self.comment_floor {
            return;
        }
        let comments = self.comments;
        let first = comments.partition_point(|comment| comment.line < line);
        let Some(comment) = comments.get(first).filter(|comment| comment.line == line) else {
            return;
        };
        if let Some((_, text)) = self.out.last_mut() {
            text.push(' ');
            text.push_str(&comment_text(&comment.text));
        }
        self.comment_floor = self.comment_floor.max(line);
    }
}

/// A Python comment as a Rust one.
fn comment_text(text: &str) -> String {
    if text.is_empty() {
        "//".to_owned()
    } else {
        format!("// {text}")
    }
}

/// `text` on one line, without runs of spaces.
pub(super) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` cut to `max` characters, with `...` when it was longer.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let head: String = text.chars().take(max).collect();
        format!("{head}...")
    }
}

/// A Rust string literal.
pub(super) fn quote(text: &str) -> String {
    format!("{text:?}")
}

/// A Rust identifier made from a Python name.
pub(super) fn rust_ident(name: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
        "gen", "sb", "page", "sb_cdp",
    ];
    let mut ident: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if ident.chars().any(|c| c.is_ascii_uppercase())
        && ident
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        ident = ident.to_ascii_lowercase();
    }
    if ident.is_empty() || ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    if KEYWORDS.contains(&ident.as_str()) {
        ident.push('_');
    }
    ident
}

/// Python source for a simple expression, for a comment.
pub(super) fn py_repr(expr: &Expr) -> String {
    use super::ast::{Arg, BinOp, UnOp};
    let join = |items: &[Expr]| items.iter().map(py_repr).collect::<Vec<_>>().join(", ");
    match expr {
        Expr::Name(name) => name.clone(),
        Expr::Str(text) => format!("{text:?}"),
        Expr::FStr(_) => "f\"...\"".to_owned(),
        Expr::Num { text, .. } => text.clone(),
        Expr::Bool(true) => "True".to_owned(),
        Expr::Bool(false) => "False".to_owned(),
        Expr::None => "None".to_owned(),
        Expr::List(items) => format!("[{}]", join(items)),
        Expr::Tuple(items) => format!("({})", join(items)),
        Expr::Dict(_) => "{...}".to_owned(),
        Expr::Attr(base, name) => format!("{}.{name}", py_repr(base)),
        Expr::Call(func, args) => {
            let args: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    Arg::Pos(value) => py_repr(value),
                    Arg::Kw(name, value) => format!("{name}={}", py_repr(value)),
                    Arg::Spread(text) => text.clone(),
                })
                .collect();
            format!("{}({})", py_repr(func), args.join(", "))
        }
        Expr::Index(base, index) => format!("{}[{}]", py_repr(base), py_repr(index)),
        Expr::Unary(UnOp::Not, operand) => format!("not {}", py_repr(operand)),
        Expr::Unary(UnOp::Neg, operand) => format!("-{}", py_repr(operand)),
        Expr::Unary(UnOp::Pos, operand) => format!("+{}", py_repr(operand)),
        Expr::Binary(left, op, right) => {
            let symbol = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::FloorDiv => "//",
                BinOp::Mod => "%",
                BinOp::And => "and",
                BinOp::Or => "or",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Lt => "<",
                BinOp::Le => "<=",
                BinOp::Gt => ">",
                BinOp::Ge => ">=",
                BinOp::In => "in",
                BinOp::NotIn => "not in",
                BinOp::Is => "is",
                BinOp::IsNot => "is not",
                BinOp::Other => "?",
            };
            format!("{} {symbol} {}", py_repr(left), py_repr(right))
        }
        Expr::IfExp(test, then, orelse) => format!(
            "{} if {} else {}",
            py_repr(then),
            py_repr(test),
            py_repr(orelse)
        ),
        Expr::Other(text) => text.clone(),
    }
}
