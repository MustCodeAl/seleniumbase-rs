//! A Python parser for the statements and expressions of a test file.
//!
//! It reads the tokens of the importers' shared lexer (`Dialect::Python`),
//! follows the indentation of the logical lines and builds the small tree of
//! [`super::ast`]. A statement it cannot read becomes
//! [`StmtKind::Unsupported`] with its source, so one odd line never stops the
//! file. The input is untrusted: nesting is bounded.

use std::collections::HashSet;

use super::ast::{
    Arg, BinOp, ClassDef, Expr, FPart, FuncDef, Handler, Param, Stmt, StmtKind, UnOp,
};
use crate::utilities::importer::lexer::{tokenize, Comment, Dialect, LexError, Tok, Token};

/// The deepest nesting of brackets and blocks the parser follows.
const MAX_DEPTH: usize = 60;

/// Words that cannot be a name.
const RESERVED: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda",
    "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield",
];

/// Assignment operators that update a target.
const AUG_ASSIGN: &[&str] = &[
    "+=", "-=", "*=", "/=", "//=", "%=", "**=", "&=", "|=", "^=", ">>=", "<<=", "@=",
];

type PResult<T> = Result<T, String>;

/// A parsed file.
pub(super) struct Module {
    /// Its statements.
    pub(super) stmts: Vec<Stmt>,
    /// Its comments.
    pub(super) comments: Vec<Comment>,
    /// The lines that hold a token (a comment on one of them is a trailing
    /// comment).
    pub(super) code_lines: HashSet<usize>,
}

/// Reads `source` as a Python module.
///
/// # Errors
///
/// The lexer's error when a string or bracket is never closed.
pub(super) fn parse_module(source: &str) -> Result<Module, LexError> {
    let lexed = tokenize(source, Dialect::Python)?;
    let code_lines = lexed.tokens.iter().map(|token| token.line).collect();
    let mut parser = Parser {
        src: source,
        toks: lexed.tokens,
        pos: 0,
        depth: 0,
    };
    let mut stmts = Vec::new();
    while let Some(first) = parser.peek() {
        let col = first.col;
        let before = parser.pos;
        stmts.extend(parser.block(col));
        if parser.pos == before {
            parser.pos += 1;
        }
    }
    Ok(Module {
        stmts,
        comments: lexed.comments,
        code_lines,
    })
}

/// Reads `text` as one expression (the inside of an f-string brace).
fn parse_expression(text: &str, depth: usize) -> PResult<Expr> {
    let lexed = tokenize(text, Dialect::Python).map_err(|error| error.message)?;
    let mut parser = Parser {
        src: text,
        toks: lexed.tokens,
        pos: 0,
        depth: depth + 1,
    };
    if parser.depth > MAX_DEPTH {
        return Err("nested too deeply".to_owned());
    }
    let value = parser.testlist()?;
    if parser.peek().is_some() {
        return Err(format!("unexpected {}", parser.describe()));
    }
    Ok(value)
}

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Token>,
    pos: usize,
    depth: usize,
}

impl Parser<'_> {
    // ------------------------------------------------------------------
    // Tokens
    // ------------------------------------------------------------------

    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }

    fn peek_at(&self, n: usize) -> Option<&Token> {
        self.toks.get(self.pos + n)
    }

    fn tok(&self) -> Option<&Tok> {
        self.peek().map(|token| &token.tok)
    }

    /// The token at the cursor, unless it starts a new logical line: an
    /// operator there begins the next statement and cannot continue the
    /// expression before it.
    fn op_tok(&self) -> Option<&Tok> {
        if self.at_line_end() {
            None
        } else {
            self.tok()
        }
    }

    fn is_punct(&self, punct: &str) -> bool {
        matches!(self.tok(), Some(Tok::Punct(p)) if *p == punct)
    }

    fn is_punct_at(&self, n: usize, punct: &str) -> bool {
        matches!(self.peek_at(n).map(|t| &t.tok), Some(Tok::Punct(p)) if *p == punct)
    }

    fn is_kw(&self, word: &str) -> bool {
        matches!(self.tok(), Some(Tok::Ident(w)) if w == word)
    }

    fn is_kw_at(&self, n: usize, word: &str) -> bool {
        matches!(self.peek_at(n).map(|t| &t.tok), Some(Tok::Ident(w)) if w == word)
    }

    /// The next token starts a new logical line (or there is none).
    fn at_line_end(&self) -> bool {
        self.peek().is_none_or(|token| token.line_start)
    }

    fn describe(&self) -> String {
        match self.tok() {
            None => "the end of the file".to_owned(),
            Some(Tok::Ident(word)) => format!("`{word}`"),
            Some(Tok::Punct(punct)) => format!("`{punct}`"),
            Some(Tok::Str(_) | Tok::Interpolated(_)) => "a string".to_owned(),
            Some(Tok::Num(_)) => "a number".to_owned(),
            Some(Tok::Regex { .. }) => "a regular expression".to_owned(),
        }
    }

    fn expect_punct(&mut self, punct: &str) -> PResult<()> {
        if self.is_punct(punct) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected `{punct}` but found {}", self.describe()))
        }
    }

    fn expect_kw(&mut self, word: &str) -> PResult<()> {
        if self.is_kw(word) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected `{word}` but found {}", self.describe()))
        }
    }

    fn expect_name(&mut self) -> PResult<String> {
        match self.tok() {
            Some(Tok::Ident(word)) if !RESERVED.contains(&word.as_str()) => {
                let word = word.clone();
                self.pos += 1;
                Ok(word)
            }
            _ => Err(format!("expected a name but found {}", self.describe())),
        }
    }

    /// The end offset of the token before the cursor.
    fn prev_end(&self) -> usize {
        self.pos
            .checked_sub(1)
            .and_then(|index| self.toks.get(index))
            .map_or(0, |token| token.end)
    }

    /// The line of the token before the cursor.
    fn prev_line(&self) -> usize {
        self.pos
            .checked_sub(1)
            .and_then(|index| self.toks.get(index))
            .map_or(1, |token| token.line)
    }

    /// The source from token `from` to the token before the cursor.
    fn source_since(&self, from: usize) -> String {
        let start = self.toks.get(from).map_or(0, |token| token.start);
        self.src
            .get(start..self.prev_end().max(start))
            .unwrap_or_default()
            .to_owned()
    }

    fn guarded<T>(&mut self, read: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        if self.depth >= MAX_DEPTH {
            return Err("nested too deeply".to_owned());
        }
        self.depth += 1;
        let result = read(self);
        self.depth -= 1;
        result
    }

    /// The next token is `word` at the start of a line indented `col`.
    fn at_clause(&self, col: usize, word: &str) -> bool {
        self.peek().is_some_and(|token| {
            token.line_start && token.col == col && matches!(&token.tok, Tok::Ident(w) if w == word)
        })
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    /// The statements of a block whose lines are indented `col`.
    fn block(&mut self, col: usize) -> Vec<Stmt> {
        let mut out = Vec::new();
        while let Some(token) = self.peek() {
            if token.line_start && token.col < col {
                break;
            }
            let before = self.pos;
            self.statement(col, &mut out);
            if self.pos == before {
                self.pos += 1;
            }
        }
        out
    }

    /// One statement (or several joined by `;`), or an `Unsupported` stand-in.
    fn statement(&mut self, col: usize, out: &mut Vec<Stmt>) {
        let kept = out.len();
        let first = self.pos;
        let Some(token) = self.peek() else {
            return;
        };
        let (line, start) = (token.line, token.start);
        if let Err(message) = self.guarded(|p| p.statement_inner(col, out)) {
            out.truncate(kept);
            self.pos = first;
            let end = self.skip_statement(col);
            let source = self.src.get(start..end).unwrap_or_default().to_owned();
            out.push(Stmt {
                line,
                end_line: self.prev_line(),
                span: (start, end),
                kind: StmtKind::Unsupported(format!("{message}: {}", first_line(&source))),
            });
        }
    }

    /// Skips the statement at the cursor with its block and its `elif`,
    /// `else`, `except` and `finally` parts; returns where it ends.
    fn skip_statement(&mut self, col: usize) -> usize {
        self.pos += 1;
        loop {
            while self
                .peek()
                .is_some_and(|token| !token.line_start || token.col > col)
            {
                self.pos += 1;
            }
            let continues = ["elif", "else", "except", "finally"]
                .iter()
                .any(|word| self.at_clause(col, word));
            if !continues {
                break;
            }
            self.pos += 1;
        }
        self.prev_end()
    }

    fn statement_inner(&mut self, col: usize, out: &mut Vec<Stmt>) -> PResult<()> {
        let Some(token) = self.peek() else {
            return Ok(());
        };
        let (line, start) = (token.line, token.start);
        let word = match &token.tok {
            Tok::Ident(word) => word.clone(),
            Tok::Punct("@") => "@".to_owned(),
            _ => String::new(),
        };
        let compound = match word.as_str() {
            "if" => Some(self.if_stmt(col)?),
            "while" => Some(self.while_stmt(col)?),
            "for" => Some(self.for_stmt(col)?),
            "try" => Some(self.try_stmt(col)?),
            "with" => Some(self.with_stmt(col)?),
            "def" => Some(StmtKind::FuncDef(self.func_def(col, Vec::new())?)),
            "class" => Some(self.class_def(col)?),
            "@" => Some(self.decorated(col)?),
            "async" if ["def", "with", "for"].iter().any(|w| self.is_kw_at(1, w)) => {
                self.pos += 1;
                return self.statement_inner(col, out);
            }
            _ => None,
        };
        if let Some(kind) = compound {
            out.push(Stmt {
                line,
                end_line: self.prev_line(),
                span: (start, self.prev_end()),
                kind,
            });
            return Ok(());
        }
        self.simple_line(out)
    }

    /// `a; b; c` on one logical line.
    fn simple_line(&mut self, out: &mut Vec<Stmt>) -> PResult<()> {
        loop {
            let Some(token) = self.peek() else {
                return Ok(());
            };
            let (line, start) = (token.line, token.start);
            let kind = self.simple_statement()?;
            out.push(Stmt {
                line,
                end_line: self.prev_line(),
                span: (start, self.prev_end()),
                kind,
            });
            if self.is_punct(";") {
                self.pos += 1;
                if self.at_line_end() {
                    return Ok(());
                }
                continue;
            }
            break;
        }
        if self.at_line_end() {
            Ok(())
        } else {
            Err(format!(
                "unexpected {} at the end of a statement",
                self.describe()
            ))
        }
    }

    fn simple_statement(&mut self) -> PResult<StmtKind> {
        let word = match self.tok() {
            Some(Tok::Ident(word)) => word.clone(),
            _ => String::new(),
        };
        match word.as_str() {
            "pass" => {
                self.pos += 1;
                Ok(StmtKind::Pass)
            }
            "break" => {
                self.pos += 1;
                Ok(StmtKind::Break)
            }
            "continue" => {
                self.pos += 1;
                Ok(StmtKind::Continue)
            }
            "return" => {
                self.pos += 1;
                if self.at_line_end() || self.is_punct(";") {
                    Ok(StmtKind::Return(None))
                } else {
                    Ok(StmtKind::Return(Some(self.testlist()?)))
                }
            }
            "assert" => {
                self.pos += 1;
                let test = self.expr()?;
                let message = if self.is_punct(",") {
                    self.pos += 1;
                    Some(self.expr()?)
                } else {
                    None
                };
                Ok(StmtKind::Assert { test, message })
            }
            "raise" => {
                self.pos += 1;
                if self.at_line_end() || self.is_punct(";") {
                    return Ok(StmtKind::Raise(None));
                }
                let value = self.expr()?;
                if self.is_kw("from") {
                    self.pos += 1;
                    self.expr()?;
                }
                Ok(StmtKind::Raise(Some(value)))
            }
            "import" | "from" => {
                let from = self.pos;
                self.skip_line();
                Ok(StmtKind::Import(self.source_since(from)))
            }
            "global" | "nonlocal" | "del" => {
                let from = self.pos;
                self.skip_line();
                Ok(StmtKind::Other(self.source_since(from)))
            }
            _ => self.expression_statement(),
        }
    }

    /// Skips the rest of the logical line.
    fn skip_line(&mut self) {
        self.pos += 1;
        while !self.at_line_end() && !self.is_punct(";") {
            self.pos += 1;
        }
    }

    fn expression_statement(&mut self) -> PResult<StmtKind> {
        let first = self.testlist()?;
        if self.is_punct("=") {
            let mut targets = vec![first];
            let mut value;
            loop {
                self.pos += 1;
                value = self.testlist()?;
                if self.is_punct("=") {
                    targets.push(value);
                } else {
                    break;
                }
            }
            return Ok(StmtKind::Assign { targets, value });
        }
        if let Some(Tok::Punct(op)) = self.tok() {
            if AUG_ASSIGN.contains(op) {
                let op = (*op).to_owned();
                self.pos += 1;
                let value = self.testlist()?;
                return Ok(StmtKind::AugAssign {
                    target: first,
                    op,
                    value,
                });
            }
        }
        if self.is_punct(":") {
            self.pos += 1;
            self.expr()?;
            let value = if self.is_punct("=") {
                self.pos += 1;
                Some(self.testlist()?)
            } else {
                None
            };
            return Ok(StmtKind::AnnAssign {
                target: first,
                value,
            });
        }
        Ok(StmtKind::Expr(first))
    }

    /// The indented block (or the rest of the line) after a `:`.
    fn suite(&mut self, header_col: usize) -> PResult<Vec<Stmt>> {
        self.expect_punct(":")?;
        if !self.at_line_end() {
            let mut body = Vec::new();
            self.simple_line(&mut body)?;
            return Ok(body);
        }
        let Some(token) = self.peek() else {
            return Ok(Vec::new());
        };
        if token.col <= header_col {
            return Err("expected an indented block".to_owned());
        }
        let col = token.col;
        Ok(self.block(col))
    }

    fn if_stmt(&mut self, col: usize) -> PResult<StmtKind> {
        self.pos += 1;
        let test = self.expr()?;
        let body = self.suite(col)?;
        let orelse = if self.at_clause(col, "elif") {
            let token = self.peek().map(|t| (t.line, t.start));
            let (line, start) = token.unwrap_or((1, 0));
            let nested = self.if_stmt(col)?;
            vec![Stmt {
                line,
                end_line: self.prev_line(),
                span: (start, self.prev_end()),
                kind: nested,
            }]
        } else {
            self.else_clause(col)?
        };
        Ok(StmtKind::If { test, body, orelse })
    }

    /// An `else:` part, if there is one.
    fn else_clause(&mut self, col: usize) -> PResult<Vec<Stmt>> {
        if self.at_clause(col, "else") {
            self.pos += 1;
            self.suite(col)
        } else {
            Ok(Vec::new())
        }
    }

    fn while_stmt(&mut self, col: usize) -> PResult<StmtKind> {
        self.pos += 1;
        let test = self.expr()?;
        let body = self.suite(col)?;
        let orelse = self.else_clause(col)?;
        Ok(StmtKind::While { test, body, orelse })
    }

    fn for_stmt(&mut self, col: usize) -> PResult<StmtKind> {
        self.pos += 1;
        let target = self.target_list()?;
        self.expect_kw("in")?;
        let iter = self.testlist()?;
        let body = self.suite(col)?;
        let orelse = self.else_clause(col)?;
        Ok(StmtKind::For {
            target,
            iter,
            body,
            orelse,
        })
    }

    /// The names a `for` or `with` binds: postfix expressions joined by commas.
    fn target_list(&mut self) -> PResult<Expr> {
        let first = self.postfix()?;
        if !self.is_punct(",") {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.is_punct(",") {
            self.pos += 1;
            if self.is_kw("in") || self.is_punct(":") {
                break;
            }
            items.push(self.postfix()?);
        }
        Ok(Expr::Tuple(items))
    }

    fn try_stmt(&mut self, col: usize) -> PResult<StmtKind> {
        self.pos += 1;
        let body = self.suite(col)?;
        let mut handlers = Vec::new();
        while self.at_clause(col, "except") {
            self.pos += 1;
            if self.is_punct("*") {
                self.pos += 1;
            }
            let ty = if self.is_punct(":") {
                None
            } else {
                Some(self.expr()?)
            };
            let name = if self.is_kw("as") {
                self.pos += 1;
                Some(self.expect_name()?)
            } else {
                None
            };
            let handler_body = self.suite(col)?;
            handlers.push(Handler {
                ty,
                name,
                body: handler_body,
            });
        }
        let orelse = self.else_clause(col)?;
        let finalbody = if self.at_clause(col, "finally") {
            self.pos += 1;
            self.suite(col)?
        } else {
            Vec::new()
        };
        Ok(StmtKind::Try {
            body,
            handlers,
            orelse,
            finalbody,
        })
    }

    fn with_stmt(&mut self, col: usize) -> PResult<StmtKind> {
        self.pos += 1;
        let mut items = Vec::new();
        loop {
            let context = self.expr()?;
            let target = if self.is_kw("as") {
                self.pos += 1;
                Some(self.postfix()?)
            } else {
                None
            };
            items.push((context, target));
            if self.is_punct(",") {
                self.pos += 1;
            } else {
                break;
            }
        }
        let body = self.suite(col)?;
        Ok(StmtKind::With { items, body })
    }

    fn decorated(&mut self, col: usize) -> PResult<StmtKind> {
        let mut decorators = Vec::new();
        while self.is_punct("@") {
            self.pos += 1;
            decorators.push(self.expr()?);
            if !self.at_line_end() {
                return Err("expected the end of a decorator line".to_owned());
            }
        }
        if self.is_kw("async") && self.is_kw_at(1, "def") {
            self.pos += 1;
        }
        if self.is_kw("def") {
            Ok(StmtKind::FuncDef(self.func_def(col, decorators)?))
        } else if self.is_kw("class") {
            self.class_def(col)
        } else {
            Err("a decorator must be followed by a def or a class".to_owned())
        }
    }

    fn func_def(&mut self, col: usize, decorators: Vec<Expr>) -> PResult<FuncDef> {
        self.pos += 1;
        let name = self.expect_name()?;
        self.expect_punct("(")?;
        let mut params = Vec::new();
        loop {
            if self.is_punct(")") {
                self.pos += 1;
                break;
            }
            let starred = self.is_punct("*") || self.is_punct("**");
            if starred || self.is_punct("/") {
                self.pos += 1;
                if self.is_punct(",") || self.is_punct(")") {
                    if self.is_punct(",") {
                        self.pos += 1;
                    }
                    continue;
                }
            }
            let param = self.expect_name()?;
            if self.is_punct(":") {
                self.pos += 1;
                self.expr()?;
            }
            let default = if self.is_punct("=") {
                self.pos += 1;
                Some(self.expr()?)
            } else {
                None
            };
            params.push(Param {
                name: param,
                default,
                starred,
            });
            if self.is_punct(",") {
                self.pos += 1;
            } else if !self.is_punct(")") {
                return Err(format!("unexpected {} in parameters", self.describe()));
            }
        }
        if self.is_punct("->") {
            self.pos += 1;
            self.expr()?;
        }
        let body = self.suite(col)?;
        Ok(FuncDef {
            name,
            params,
            decorators,
            body,
        })
    }

    fn class_def(&mut self, col: usize) -> PResult<StmtKind> {
        self.pos += 1;
        let name = self.expect_name()?;
        let mut bases = Vec::new();
        if self.is_punct("(") {
            self.pos += 1;
            for arg in self.arguments()? {
                if let Arg::Pos(base) = arg {
                    bases.push(base);
                }
            }
        }
        let body = self.suite(col)?;
        Ok(StmtKind::ClassDef(ClassDef { name, bases, body }))
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    /// `a, b` without brackets (a tuple) or a single expression.
    fn testlist(&mut self) -> PResult<Expr> {
        let first = self.expr()?;
        if !self.is_punct(",") {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.is_punct(",") {
            self.pos += 1;
            if self.at_line_end() || self.is_punct("=") || self.is_punct(")") {
                break;
            }
            items.push(self.expr()?);
        }
        Ok(Expr::Tuple(items))
    }

    fn expr(&mut self) -> PResult<Expr> {
        self.guarded(|p| {
            if p.is_kw("lambda") {
                return p.lambda();
            }
            let body = p.or_test()?;
            if p.is_kw("if") && !p.at_line_end() {
                p.pos += 1;
                let test = p.or_test()?;
                p.expect_kw("else")?;
                let orelse = p.expr()?;
                return Ok(Expr::IfExp(
                    Box::new(test),
                    Box::new(body),
                    Box::new(orelse),
                ));
            }
            Ok(body)
        })
    }

    fn lambda(&mut self) -> PResult<Expr> {
        let from = self.pos;
        self.pos += 1;
        let mut depth = 0_usize;
        while let Some(tok) = self.tok() {
            match tok {
                Tok::Punct("(" | "[" | "{") => depth += 1,
                Tok::Punct(")" | "]" | "}") => depth = depth.saturating_sub(1),
                Tok::Punct(":") if depth == 0 => break,
                _ => {}
            }
            if self.at_line_end() && depth == 0 {
                break;
            }
            self.pos += 1;
        }
        self.expect_punct(":")?;
        self.expr()?;
        Ok(Expr::Other(self.source_since(from)))
    }

    fn or_test(&mut self) -> PResult<Expr> {
        let mut left = self.and_test()?;
        while self.is_kw("or") && !self.at_line_end() {
            self.pos += 1;
            let right = self.and_test()?;
            left = Expr::Binary(Box::new(left), BinOp::Or, Box::new(right));
        }
        Ok(left)
    }

    fn and_test(&mut self) -> PResult<Expr> {
        let mut left = self.not_test()?;
        while self.is_kw("and") && !self.at_line_end() {
            self.pos += 1;
            let right = self.not_test()?;
            left = Expr::Binary(Box::new(left), BinOp::And, Box::new(right));
        }
        Ok(left)
    }

    fn not_test(&mut self) -> PResult<Expr> {
        if self.is_kw("not") {
            self.pos += 1;
            let operand = self.guarded(Self::not_test)?;
            return Ok(Expr::Unary(UnOp::Not, Box::new(operand)));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> PResult<Expr> {
        let mut left = self.bitwise()?;
        loop {
            let (op, width) = match self.op_tok() {
                Some(Tok::Punct("==")) => (BinOp::Eq, 1),
                Some(Tok::Punct("!=")) => (BinOp::Ne, 1),
                Some(Tok::Punct("<")) => (BinOp::Lt, 1),
                Some(Tok::Punct("<=")) => (BinOp::Le, 1),
                Some(Tok::Punct(">")) => (BinOp::Gt, 1),
                Some(Tok::Punct(">=")) => (BinOp::Ge, 1),
                Some(Tok::Ident(w)) if w == "in" => (BinOp::In, 1),
                Some(Tok::Ident(w)) if w == "not" && self.is_kw_at(1, "in") => (BinOp::NotIn, 2),
                Some(Tok::Ident(w)) if w == "is" && self.is_kw_at(1, "not") => (BinOp::IsNot, 2),
                Some(Tok::Ident(w)) if w == "is" => (BinOp::Is, 1),
                _ => break,
            };
            self.pos += width;
            let right = self.bitwise()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    /// `| ^ & << >>`: read, but kept as source.
    fn bitwise(&mut self) -> PResult<Expr> {
        let from = self.pos;
        let mut left = self.arith()?;
        while matches!(
            self.op_tok(),
            Some(Tok::Punct("|" | "^" | "&" | "<<" | ">>"))
        ) {
            self.pos += 1;
            self.arith()?;
            left = Expr::Other(self.source_since(from));
        }
        Ok(left)
    }

    fn arith(&mut self) -> PResult<Expr> {
        let mut left = self.term()?;
        loop {
            let op = match self.op_tok() {
                Some(Tok::Punct("+")) => BinOp::Add,
                Some(Tok::Punct("-")) => BinOp::Sub,
                _ => break,
            };
            self.pos += 1;
            let right = self.term()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn term(&mut self) -> PResult<Expr> {
        let from = self.pos;
        let mut left = self.factor()?;
        loop {
            let op = match self.op_tok() {
                Some(Tok::Punct("*")) => BinOp::Mul,
                Some(Tok::Punct("/")) => BinOp::Div,
                Some(Tok::Punct("//")) => BinOp::FloorDiv,
                Some(Tok::Punct("%")) => BinOp::Mod,
                Some(Tok::Punct("@")) => BinOp::Other,
                _ => break,
            };
            self.pos += 1;
            let right = self.factor()?;
            left = if op == BinOp::Other {
                Expr::Other(self.source_since(from))
            } else {
                Expr::Binary(Box::new(left), op, Box::new(right))
            };
        }
        Ok(left)
    }

    fn factor(&mut self) -> PResult<Expr> {
        let op = match self.tok() {
            Some(Tok::Punct("-")) => Some(UnOp::Neg),
            Some(Tok::Punct("+")) => Some(UnOp::Pos),
            Some(Tok::Punct("~")) => None,
            _ => return self.power(),
        };
        let from = self.pos;
        self.pos += 1;
        let operand = self.guarded(Self::factor)?;
        Ok(match op {
            Some(op) => Expr::Unary(op, Box::new(operand)),
            None => Expr::Other(self.source_since(from)),
        })
    }

    fn power(&mut self) -> PResult<Expr> {
        let from = self.pos;
        if self.is_kw("await") {
            self.pos += 1;
        }
        let base = self.postfix()?;
        if matches!(self.op_tok(), Some(Tok::Punct("**"))) {
            self.pos += 1;
            self.guarded(Self::factor)?;
            return Ok(Expr::Other(self.source_since(from)));
        }
        Ok(base)
    }

    /// An atom with its attribute, call and subscript trailers.
    fn postfix(&mut self) -> PResult<Expr> {
        let from = self.pos;
        let mut value = self.atom()?;
        loop {
            if self.is_punct(".") {
                self.pos += 1;
                let name = match self.tok() {
                    Some(Tok::Ident(word)) => word.clone(),
                    _ => return Err(format!("expected a name but found {}", self.describe())),
                };
                self.pos += 1;
                value = Expr::Attr(Box::new(value), name);
            } else if self.is_punct("(") && !self.at_line_end() {
                self.pos += 1;
                let args = self.guarded(Self::arguments)?;
                value = Expr::Call(Box::new(value), args);
            } else if self.is_punct("[") && !self.at_line_end() {
                self.pos += 1;
                value = self.guarded(|p| p.subscript(value, from))?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    /// The inside of `value[...]`; the `[` is read.
    fn subscript(&mut self, value: Expr, from: usize) -> PResult<Expr> {
        let index = self.slice_or_expr()?;
        if self.is_punct(":") {
            self.skip_to_close();
            self.expect_punct("]")?;
            return Ok(Expr::Other(self.source_since(from)));
        }
        let index = if self.is_punct(",") {
            let mut items = vec![index];
            while self.is_punct(",") {
                self.pos += 1;
                if self.is_punct("]") {
                    break;
                }
                items.push(self.slice_or_expr()?);
            }
            Expr::Tuple(items)
        } else {
            index
        };
        if self.is_punct(":") {
            self.skip_to_close();
        }
        self.expect_punct("]")?;
        Ok(Expr::Index(Box::new(value), Box::new(index)))
    }

    /// An index, or nothing before a `:` of a slice.
    fn slice_or_expr(&mut self) -> PResult<Expr> {
        if self.is_punct(":") {
            return Ok(Expr::Other(String::new()));
        }
        self.expr()
    }

    /// Skips to the bracket that closes the one the cursor is inside.
    fn skip_to_close(&mut self) {
        let mut depth = 0_usize;
        while let Some(tok) = self.tok() {
            match tok {
                Tok::Punct("(" | "[" | "{") => depth += 1,
                Tok::Punct(")" | "]" | "}") => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            self.pos += 1;
        }
    }

    /// The arguments of a call; the `(` is read, the `)` is read here.
    fn arguments(&mut self) -> PResult<Vec<Arg>> {
        let mut args = Vec::new();
        loop {
            if self.is_punct(")") {
                self.pos += 1;
                return Ok(args);
            }
            let from = self.pos;
            if self.is_punct("*") || self.is_punct("**") {
                self.pos += 1;
                self.expr()?;
                args.push(Arg::Spread(self.source_since(from)));
            } else if matches!(self.tok(), Some(Tok::Ident(_))) && self.is_punct_at(1, "=") {
                let name = self.expect_name()?;
                self.pos += 1;
                let value = self.expr()?;
                args.push(Arg::Kw(name, value));
            } else {
                let value = self.expr()?;
                if self.is_kw("for") || self.is_kw("async") {
                    self.skip_to_close();
                    args.push(Arg::Pos(Expr::Other(self.source_since(from))));
                } else {
                    args.push(Arg::Pos(value));
                }
            }
            if self.is_punct(",") {
                self.pos += 1;
            } else if !self.is_punct(")") {
                return Err(format!(
                    "expected `,` or `)` in a call but found {}",
                    self.describe()
                ));
            }
        }
    }

    fn atom(&mut self) -> PResult<Expr> {
        let Some(token) = self.peek() else {
            return Err("expected an expression but found the end of the file".to_owned());
        };
        let (tok, start, end) = (token.tok.clone(), token.start, token.end);
        let from = self.pos;
        match tok {
            Tok::Ident(word) => match word.as_str() {
                "True" => {
                    self.pos += 1;
                    Ok(Expr::Bool(true))
                }
                "False" => {
                    self.pos += 1;
                    Ok(Expr::Bool(false))
                }
                "None" => {
                    self.pos += 1;
                    Ok(Expr::None)
                }
                _ if RESERVED.contains(&word.as_str()) => {
                    Err(format!("expected an expression but found `{word}`"))
                }
                _ => {
                    self.pos += 1;
                    Ok(Expr::Name(word))
                }
            },
            Tok::Num(_) => {
                let text = self
                    .src
                    .get(start..end)
                    .unwrap_or_default()
                    .replace('_', "");
                self.pos += 1;
                let lower = text.to_ascii_lowercase();
                let float =
                    !lower.starts_with("0x") && (lower.contains('.') || lower.contains('e'));
                Ok(Expr::Num { text, float })
            }
            Tok::Str(_) | Tok::Interpolated(_) => self.string(),
            Tok::Punct("(") => {
                self.pos += 1;
                self.guarded(|p| p.parenthesized(from))
            }
            Tok::Punct("[") => {
                self.pos += 1;
                self.guarded(|p| p.list(from))
            }
            Tok::Punct("{") => {
                self.pos += 1;
                self.guarded(|p| p.dict(from))
            }
            Tok::Punct("...") => {
                self.pos += 1;
                Ok(Expr::Other("...".to_owned()))
            }
            _ => Err(format!("unexpected {}", self.describe())),
        }
    }

    fn parenthesized(&mut self, from: usize) -> PResult<Expr> {
        if self.is_punct(")") {
            self.pos += 1;
            return Ok(Expr::Tuple(Vec::new()));
        }
        let first = self.expr()?;
        if self.is_kw("for") || self.is_kw("async") {
            self.skip_to_close();
            self.expect_punct(")")?;
            return Ok(Expr::Other(self.source_since(from)));
        }
        if self.is_punct(",") {
            let mut items = vec![first];
            while self.is_punct(",") {
                self.pos += 1;
                if self.is_punct(")") {
                    break;
                }
                items.push(self.expr()?);
            }
            self.expect_punct(")")?;
            return Ok(Expr::Tuple(items));
        }
        self.expect_punct(")")?;
        Ok(first)
    }

    fn list(&mut self, from: usize) -> PResult<Expr> {
        let mut items = Vec::new();
        loop {
            if self.is_punct("]") {
                self.pos += 1;
                return Ok(Expr::List(items));
            }
            items.push(self.expr()?);
            if self.is_kw("for") || self.is_kw("async") {
                self.skip_to_close();
                self.expect_punct("]")?;
                return Ok(Expr::Other(self.source_since(from)));
            }
            if self.is_punct(",") {
                self.pos += 1;
            } else if !self.is_punct("]") {
                return Err(format!("unexpected {} in a list", self.describe()));
            }
        }
    }

    fn dict(&mut self, from: usize) -> PResult<Expr> {
        let mut pairs = Vec::new();
        loop {
            if self.is_punct("}") {
                self.pos += 1;
                return Ok(Expr::Dict(pairs));
            }
            if self.is_punct("**") {
                self.skip_to_close();
                self.expect_punct("}")?;
                return Ok(Expr::Other(self.source_since(from)));
            }
            let key = self.expr()?;
            if !self.is_punct(":") {
                // A set display: kept as source.
                self.skip_to_close();
                self.expect_punct("}")?;
                return Ok(Expr::Other(self.source_since(from)));
            }
            self.pos += 1;
            let value = self.expr()?;
            if self.is_kw("for") || self.is_kw("async") {
                self.skip_to_close();
                self.expect_punct("}")?;
                return Ok(Expr::Other(self.source_since(from)));
            }
            pairs.push((key, value));
            if self.is_punct(",") {
                self.pos += 1;
            } else if !self.is_punct("}") {
                return Err(format!("unexpected {} in a dict", self.describe()));
            }
        }
    }

    /// One or more adjacent string literals, plain or f-strings.
    fn string(&mut self) -> PResult<Expr> {
        let mut parts: Vec<FPart> = Vec::new();
        let mut formatted = false;
        let mut first = true;
        while let Some(token) = self.peek() {
            if !first && token.line_start {
                break;
            }
            match &token.tok {
                Tok::Str(text) => push_lit(&mut parts, text),
                Tok::Interpolated(raw) => {
                    formatted = true;
                    for part in parse_fstring(raw, self.depth)? {
                        match part {
                            FPart::Lit(text) => push_lit(&mut parts, &text),
                            expr @ FPart::Expr(_) => parts.push(expr),
                        }
                    }
                }
                _ => break,
            }
            first = false;
            self.pos += 1;
        }
        if formatted {
            return Ok(Expr::FStr(parts));
        }
        let mut text = String::new();
        for part in parts {
            if let FPart::Lit(piece) = part {
                text.push_str(&piece);
            }
        }
        Ok(Expr::Str(text))
    }
}

/// Appends literal text, joining it to a literal before it.
fn push_lit(parts: &mut Vec<FPart>, text: &str) {
    if let Some(FPart::Lit(last)) = parts.last_mut() {
        last.push_str(text);
    } else {
        parts.push(FPart::Lit(text.to_owned()));
    }
}

/// The first line of `source`, for a message.
fn first_line(source: &str) -> String {
    source.lines().next().unwrap_or_default().trim().to_owned()
}

/// Splits the source text of an f-string (prefix and quotes included) into
/// literal text and expressions.
fn parse_fstring(raw: &str, depth: usize) -> PResult<Vec<FPart>> {
    let quote_at = raw
        .find(['\'', '"'])
        .ok_or_else(|| "an f-string without a quote".to_owned())?;
    let is_raw = raw[..quote_at].to_ascii_lowercase().contains('r');
    let rest = &raw[quote_at..];
    let quote = rest.chars().next().unwrap_or('"');
    let triple: String = std::iter::repeat_n(quote, 3).collect();
    let width = if rest.starts_with(&triple) && rest.len() >= 6 {
        3
    } else {
        1
    };
    let body = rest
        .get(width..rest.len().saturating_sub(width))
        .ok_or_else(|| "an f-string that is not closed".to_owned())?;
    let chars: Vec<char> = body.chars().collect();
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        match c {
            '{' if chars.get(index + 1) == Some(&'{') => {
                literal.push('{');
                index += 2;
            }
            '}' if chars.get(index + 1) == Some(&'}') => {
                literal.push('}');
                index += 2;
            }
            '{' => {
                if !literal.is_empty() {
                    parts.push(FPart::Lit(decode_literal(&literal, is_raw)));
                    literal.clear();
                }
                let (end, text) = brace_expression(&chars, index + 1)?;
                parts.push(FPart::Expr(parse_expression(text.trim(), depth)?));
                index = end;
            }
            '\\' if !is_raw => {
                literal.push(c);
                if let Some(next) = chars.get(index + 1) {
                    literal.push(*next);
                }
                index += 2;
            }
            _ => {
                literal.push(c);
                index += 1;
            }
        }
    }
    if !literal.is_empty() {
        parts.push(FPart::Lit(decode_literal(&literal, is_raw)));
    }
    Ok(parts)
}

/// The expression text of a brace that starts at `start` (after the `{`) and
/// the index after the closing `}`. A `!conversion` or `:spec` is skipped.
fn brace_expression(chars: &[char], start: usize) -> PResult<(usize, String)> {
    let mut depth = 0_usize;
    let mut index = start;
    let mut expr_end: Option<usize> = None;
    let mut quote: Option<char> = None;
    while index < chars.len() {
        let c = chars[index];
        if let Some(open) = quote {
            if c == open {
                quote = None;
            }
            index += 1;
            continue;
        }
        match c {
            '\'' | '"' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '}' if depth > 0 => depth -= 1,
            '}' => {
                let end = expr_end.unwrap_or(index);
                return Ok((index + 1, chars[start..end].iter().collect()));
            }
            '!' if depth == 0 && chars.get(index + 1) != Some(&'=') && expr_end.is_none() => {
                expr_end = Some(index);
            }
            ':' if depth == 0 && expr_end.is_none() => expr_end = Some(index),
            _ => {}
        }
        index += 1;
    }
    Err("an f-string brace is never closed".to_owned())
}

/// The text of a literal piece of an f-string with its escapes decoded.
fn decode_literal(text: &str, is_raw: bool) -> String {
    if is_raw {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('"') => out.push('"'),
            Some('\n') | None => {}
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}
