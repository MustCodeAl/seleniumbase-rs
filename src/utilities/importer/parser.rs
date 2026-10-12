//! The parser the front ends share: statements and expressions for the subset
//! of JavaScript, TypeScript and Python that browser tests are written in.
//!
//! It reads call chains (`await page.getByRole('button', { name: 'Go' }).click()`),
//! declarations and assignments, arrow and named functions, object and array
//! literals, Python keyword arguments, `with` blocks and decorators. TypeScript
//! type annotations, `as` casts, non-null assertions and generic arguments on
//! calls are read past. Anything else (loops, conditionals, classes, `try`) is
//! kept as one [`StmtKind::Unsupported`] statement with its source span, so the
//! translator can leave it in the output as a `TODO`.
//!
//! Recursion is bounded by [`MAX_DEPTH`]: a statement nested deeper than that
//! becomes `Unsupported` instead of exhausting the stack. Skipping a statement
//! is a loop, never recursion.

use super::lexer::{Comment, Dialect, Lexed, Tok, Token};

/// How deeply statements and expressions may nest before a statement is given
/// up as too deep.
pub(super) const MAX_DEPTH: usize = 100;

/// A statement and where it is.
#[derive(Clone, Debug)]
pub(super) struct Stmt {
    /// 1-based line of its first token.
    pub(super) line: usize,
    /// Byte span in the source.
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) kind: StmtKind,
}

/// What a statement is.
#[derive(Clone, Debug)]
pub(super) enum StmtKind {
    /// A comment on its own, between statements.
    Comment(String),
    /// An import, a type declaration or `pass`: nothing to convert.
    Ignored,
    /// An expression statement.
    Expr(Expr),
    /// `const`, `let` or `var`.
    Decl(Vec<Declarator>),
    /// `target = value` (and every Python binding).
    Assign { target: Expr, value: Expr },
    /// A named function: `function f() {}` or Python's `def f():`.
    FuncDef {
        name: String,
        func: Function,
        decorators: Vec<Expr>,
    },
    /// Python's `with a as b:`.
    With {
        items: Vec<(Expr, Option<String>)>,
        body: Vec<Stmt>,
    },
    /// Python's `assert`.
    Assert(Expr),
    /// `return`.
    Return(Option<Expr>),
    /// Python's `if __name__ == "__main__":`.
    IfMain(Vec<Stmt>),
    /// A statement the importer does not read, and what it is.
    Unsupported(String),
}

/// One name (or pattern) of a declaration and its value.
#[derive(Clone, Debug)]
pub(super) struct Declarator {
    pub(super) pattern: Pattern,
    pub(super) init: Option<Expr>,
}

/// What a declaration binds.
#[derive(Clone, Debug)]
pub(super) enum Pattern {
    /// One name.
    Name(String),
    /// `{ a, b }` or `[a, b]`: the names bound.
    Destructure(Vec<String>),
}

/// An arrow function, a function expression or a named function.
#[derive(Clone, Debug)]
pub(super) struct Function {
    pub(super) params: Vec<Param>,
    pub(super) body: FnBody,
    pub(super) is_async: bool,
    /// Byte span of the whole function in the source.
    pub(super) start: usize,
    pub(super) end: usize,
    /// Every name the function's code mentions, except property names after
    /// a `.`, for telling whether it uses variables from outside.
    pub(super) names: Vec<String>,
    /// TypeScript-only syntax appears in it (a type annotation or `as`).
    pub(super) typescript: bool,
}

/// A function body.
#[derive(Clone, Debug)]
pub(super) enum FnBody {
    /// `=> expression`.
    Expr(Box<Expr>),
    /// `{ statements }` or an indented Python block.
    Block(Vec<Stmt>),
}

/// A parameter: a name, or the fields of a destructured object.
#[derive(Clone, Debug)]
pub(super) struct Param {
    pub(super) name: Option<String>,
    pub(super) fields: Vec<String>,
}

/// An expression.
#[derive(Clone, Debug)]
pub(super) enum Expr {
    Ident(String),
    Str(String),
    /// A template literal with `${}` or an f-string with placeholders: its
    /// text is never read.
    Interpolated,
    Num(f64),
    Bool(bool),
    /// `null` or `None`.
    Null,
    Undefined,
    Regex {
        pattern: String,
        flags: String,
    },
    Array(Vec<Expr>),
    /// Keys decoded; computed keys and spreads have the key `"..."`.
    Object(Vec<(String, Expr)>),
    Member {
        object: Box<Expr>,
        property: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    New {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    Await(Box<Expr>),
    Unary {
        op: &'static str,
        operand: Box<Expr>,
    },
    Binary {
        op: &'static str,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Function(Box<Function>),
    /// Something read but not represented (a conditional, a comprehension, ...).
    Opaque,
}

/// A call argument.
#[derive(Clone, Debug)]
pub(super) enum Arg {
    Pos(Expr),
    /// A Python keyword argument.
    Kw(String, Expr),
    /// `...x`, `*x` or `**x`: not converted, so only its place is kept.
    Spread,
}

type PResult<T> = Result<T, String>;

/// Parses `lexed` into statements. Never fails: what cannot be read becomes
/// [`StmtKind::Unsupported`].
pub(super) fn parse(lexed: &Lexed, dialect: Dialect) -> Vec<Stmt> {
    let mut parser = Parser {
        toks: &lexed.tokens,
        pos: 0,
        comments: &lexed.comments,
        cpos: 0,
        dialect,
        depth: 0,
    };
    let mut out = Vec::new();
    match dialect {
        Dialect::Js => parser.js_block(&mut out, false),
        Dialect::Python => {
            let indent = parser.peek().map_or(0, |t| t.col);
            parser.py_block(&mut out, indent);
        }
    }
    parser.drain_comments(usize::MAX, &mut out);
    out
}

/// Parses the whole input as one JavaScript expression: a JSON document.
pub(super) fn parse_value(lexed: &Lexed) -> Result<Expr, String> {
    let mut parser = Parser {
        toks: &lexed.tokens,
        pos: 0,
        comments: &lexed.comments,
        cpos: 0,
        dialect: Dialect::Js,
        depth: 0,
    };
    let value = parser.expr()?;
    if !parser.at_end() {
        return Err(format!("unexpected {} after the value", parser.describe()));
    }
    Ok(value)
}

/// JavaScript binary operators by precedence, loosest first.
const JS_LEVELS: &[&[&str]] = &[
    &["??", "||"],
    &["&&"],
    &["|"],
    &["^"],
    &["&"],
    &["==", "!=", "===", "!=="],
    &["<", ">", "<=", ">=", "instanceof", "in"],
    &["<<", ">>", ">>>"],
    &["+", "-"],
    &["*", "/", "%"],
    &["**"],
];

/// Python binary operators by precedence, loosest first. Level 2 is `not`.
const PY_LEVELS: &[&[&str]] = &[
    &["or"],
    &["and"],
    &[],
    &[
        "==", "!=", "<", ">", "<=", ">=", "in", "not in", "is", "is not",
    ],
    &["|"],
    &["^"],
    &["&"],
    &["<<", ">>"],
    &["+", "-"],
    &["*", "/", "//", "%", "@"],
    &["**"],
];

/// Assignment operators other than `=`.
const COMPOUND_ASSIGN: &[&str] = &[
    "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=",
    "??=", "//=", "@=",
];

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
    comments: &'a [Comment],
    cpos: usize,
    dialect: Dialect,
    depth: usize,
}

impl Parser<'_> {
    // ------------------------------------------------------------------
    // Tokens
    // ------------------------------------------------------------------

    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }

    fn peek_at(&self, offset: usize) -> Option<&Token> {
        self.toks.get(self.pos + offset)
    }

    fn tok(&self) -> Option<&Tok> {
        self.peek().map(|t| &t.tok)
    }

    fn is_punct(&self, p: &str) -> bool {
        matches!(self.tok(), Some(Tok::Punct(q)) if *q == p)
    }

    fn is_punct_at(&self, offset: usize, p: &str) -> bool {
        matches!(self.peek_at(offset).map(|t| &t.tok), Some(Tok::Punct(q)) if *q == p)
    }

    fn is_ident(&self, name: &str) -> bool {
        matches!(self.tok(), Some(Tok::Ident(n)) if n == name)
    }

    fn is_ident_at(&self, offset: usize, name: &str) -> bool {
        matches!(self.peek_at(offset).map(|t| &t.tok), Some(Tok::Ident(n)) if n == name)
    }

    fn bump(&mut self) {
        if self.pos < self.toks.len() {
            self.pos += 1;
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    /// Python: the logical line ends here.
    fn at_line_end(&self) -> bool {
        self.peek().is_none_or(|t| t.line_start)
    }

    fn python(&self) -> bool {
        self.dialect == Dialect::Python
    }

    fn prev_end(&self) -> usize {
        self.pos
            .checked_sub(1)
            .and_then(|i| self.toks.get(i))
            .map_or(0, |t| t.end)
    }

    fn expect_punct(&mut self, p: &str) -> PResult<()> {
        if self.is_punct(p) {
            self.bump();
            Ok(())
        } else {
            Err(format!("expected `{p}` but found {}", self.describe()))
        }
    }

    fn expect_ident(&mut self) -> PResult<String> {
        match self.tok() {
            Some(Tok::Ident(name)) => {
                let name = name.clone();
                self.bump();
                Ok(name)
            }
            _ => Err(format!("expected a name but found {}", self.describe())),
        }
    }

    /// The current token, for an error message.
    fn describe(&self) -> String {
        match self.tok() {
            None => "the end of the file".to_owned(),
            Some(Tok::Ident(name)) => format!("`{}`", short(name)),
            Some(Tok::Punct(p)) => format!("`{p}`"),
            Some(Tok::Str(_) | Tok::Interpolated(_)) => "a string".to_owned(),
            Some(Tok::Num(_)) => "a number".to_owned(),
            Some(Tok::Regex { .. }) => "a regular expression".to_owned(),
        }
    }

    /// Skips the bracketed group starting at the current opener. A loop with
    /// a counter, so any nesting is fine.
    fn skip_balanced(&mut self) {
        let mut depth = 0_usize;
        while let Some(tok) = self.tok() {
            match tok {
                Tok::Punct("(" | "[" | "{") => depth += 1,
                Tok::Punct(")" | "]" | "}") => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        self.bump();
                        return;
                    }
                }
                _ => {}
            }
            self.bump();
        }
    }

    /// The index of the token that closes the group opened at `open`.
    fn matching_close(&self, open: usize) -> Option<usize> {
        let mut depth = 0_usize;
        for (index, token) in self.toks.iter().enumerate().skip(open) {
            match token.tok {
                Tok::Punct("(" | "[" | "{") => depth += 1,
                Tok::Punct(")" | "]" | "}") => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Comments
    // ------------------------------------------------------------------

    /// Turns the comments before byte `before` into statements.
    fn drain_comments(&mut self, before: usize, out: &mut Vec<Stmt>) {
        while let Some(comment) = self.comments.get(self.cpos) {
            if comment.start >= before {
                break;
            }
            out.push(Stmt {
                line: comment.line,
                start: comment.start,
                end: comment.start,
                kind: StmtKind::Comment(comment.text.clone()),
            });
            self.cpos += 1;
        }
    }

    /// Drops the comments before byte `before`: they were inside a statement.
    fn skip_comments(&mut self, before: usize) {
        while self
            .comments
            .get(self.cpos)
            .is_some_and(|comment| comment.start < before)
        {
            self.cpos += 1;
        }
    }

    // ------------------------------------------------------------------
    // Depth
    // ------------------------------------------------------------------

    fn guarded<T>(&mut self, parse: impl FnOnce(&mut Self) -> PResult<T>) -> PResult<T> {
        self.depth += 1;
        let result = if self.depth > MAX_DEPTH {
            Err(format!("it is nested more than {MAX_DEPTH} levels deep"))
        } else {
            parse(self)
        };
        self.depth -= 1;
        result
    }

    // ------------------------------------------------------------------
    // JavaScript statements
    // ------------------------------------------------------------------

    fn js_block(&mut self, out: &mut Vec<Stmt>, in_braces: bool) {
        loop {
            let next_start = self.peek().map_or(usize::MAX, |t| t.start);
            self.drain_comments(next_start, out);
            let Some(token) = self.peek() else { break };
            if in_braces && token.tok == Tok::Punct("}") {
                break;
            }
            let (line, start) = (token.line, token.start);
            let start_pos = self.pos;
            let kept = out.len();
            if let Err(message) = self.guarded(|p| p.js_statement(out)) {
                out.truncate(kept);
                self.pos = start_pos;
                self.skip_js_statement();
                out.push(Stmt {
                    line,
                    start,
                    end: self.prev_end().max(start),
                    kind: StmtKind::Unsupported(format!(
                        "could not read this statement: {message}"
                    )),
                });
            }
            if self.pos == start_pos {
                self.bump();
            }
            let end = self.prev_end();
            self.skip_comments(end);
        }
    }

    fn js_statement(&mut self, out: &mut Vec<Stmt>) -> PResult<()> {
        let Some(token) = self.peek() else {
            return Ok(());
        };
        let (line, start) = (token.line, token.start);
        // `export` (and `export default`) in front of a statement changes
        // nothing here. A loop, so a run of them cannot recurse.
        while self.is_ident("export") {
            self.bump();
            if self.is_ident("default") {
                self.bump();
            }
            if self.is_punct("{") || self.is_punct("*") {
                self.skip_js_statement();
                out.push(Stmt {
                    line,
                    start,
                    end: self.prev_end(),
                    kind: StmtKind::Ignored,
                });
                return Ok(());
            }
        }
        let Some(token) = self.peek() else {
            return Ok(());
        };
        let kind = match token.tok.clone() {
            Tok::Punct(";") => {
                self.bump();
                return Ok(());
            }
            Tok::Punct("{") => {
                self.bump();
                self.js_block(out, true);
                return self.expect_punct("}");
            }
            Tok::Ident(word) => match word.as_str() {
                "import" if !self.is_punct_at(1, "(") && !self.is_punct_at(1, ".") => {
                    self.skip_js_statement();
                    StmtKind::Ignored
                }
                "const" | "let" | "var"
                    if matches!(
                        self.peek_at(1).map(|t| &t.tok),
                        Some(Tok::Ident(_) | Tok::Punct("{" | "["))
                    ) =>
                {
                    self.js_declaration()?
                }
                "function" => {
                    self.bump();
                    self.js_function_decl(false, start)?
                }
                "async"
                    if self.is_ident_at(1, "function")
                        && !self.peek_at(1).is_some_and(|t| t.nl_before) =>
                {
                    self.bump();
                    self.bump();
                    self.js_function_decl(true, start)?
                }
                "return" => {
                    self.bump();
                    let value = if self.js_at_statement_end() {
                        None
                    } else {
                        Some(self.expr()?)
                    };
                    self.js_end_statement()?;
                    StmtKind::Return(value)
                }
                "if" | "for" | "while" | "do" | "switch" | "try" | "class" | "with" => {
                    self.skip_js_compound();
                    StmtKind::Unsupported(describe_keyword(&word).to_owned())
                }
                "throw" | "break" | "continue" | "debugger" => {
                    self.skip_js_statement();
                    StmtKind::Unsupported(describe_keyword(&word).to_owned())
                }
                "interface" | "enum" | "declare" | "namespace" | "abstract"
                    if matches!(
                        self.peek_at(1).map(|t| &t.tok),
                        Some(Tok::Ident(_) | Tok::Punct("{"))
                    ) =>
                {
                    self.skip_js_compound();
                    StmtKind::Ignored
                }
                "type"
                    if matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Ident(_)))
                        && (self.is_punct_at(2, "=") || self.is_punct_at(2, "<")) =>
                {
                    self.skip_js_statement();
                    StmtKind::Ignored
                }
                _ => self.js_expression_statement()?,
            },
            _ => self.js_expression_statement()?,
        };
        let end = self.prev_end();
        out.push(Stmt {
            line,
            start,
            end,
            kind,
        });
        Ok(())
    }

    fn js_at_statement_end(&self) -> bool {
        match self.peek() {
            None => true,
            Some(t) => t.nl_before || matches!(t.tok, Tok::Punct(";" | "}")),
        }
    }

    fn js_end_statement(&mut self) -> PResult<()> {
        if self.is_punct(";") {
            self.bump();
            Ok(())
        } else if self.js_at_statement_end() {
            Ok(())
        } else {
            Err(format!(
                "expected the end of the statement but found {}",
                self.describe()
            ))
        }
    }

    fn js_expression_statement(&mut self) -> PResult<StmtKind> {
        let expr = self.expr()?;
        if self.is_punct("=") {
            self.bump();
            let value = self.expr()?;
            if self.is_punct("=") {
                return Err("chained assignments are not read".to_owned());
            }
            self.js_end_statement()?;
            return Ok(StmtKind::Assign {
                target: expr,
                value,
            });
        }
        if let Some(Tok::Punct(op)) = self.tok() {
            let op = *op;
            if COMPOUND_ASSIGN.contains(&op) {
                self.skip_js_statement();
                return Ok(StmtKind::Unsupported(format!("a `{op}` assignment")));
            }
        }
        self.js_end_statement()?;
        Ok(StmtKind::Expr(expr))
    }

    fn js_declaration(&mut self) -> PResult<StmtKind> {
        self.bump();
        let mut declarators = Vec::new();
        loop {
            let pattern = self.binding_pattern()?;
            if self.is_punct("!") {
                self.bump();
            }
            if self.is_punct(":") {
                self.bump();
                self.skip_type(false);
            }
            let init = if self.is_punct("=") {
                self.bump();
                Some(self.expr()?)
            } else {
                None
            };
            declarators.push(Declarator { pattern, init });
            if self.is_punct(",") {
                self.bump();
                continue;
            }
            break;
        }
        self.js_end_statement()?;
        Ok(StmtKind::Decl(declarators))
    }

    /// A name, or the names an object or array pattern binds.
    fn binding_pattern(&mut self) -> PResult<Pattern> {
        if self.is_punct("{") || self.is_punct("[") {
            let mut names = Vec::new();
            self.collect_pattern_names(&mut names)?;
            Ok(Pattern::Destructure(names))
        } else {
            Ok(Pattern::Name(self.expect_ident()?))
        }
    }

    fn collect_pattern_names(&mut self, names: &mut Vec<String>) -> PResult<()> {
        self.guarded(|p| {
            let object = p.is_punct("{");
            let close = if object { "}" } else { "]" };
            p.bump();
            loop {
                if p.is_punct(close) {
                    p.bump();
                    return Ok(());
                }
                if p.is_punct(",") {
                    p.bump();
                    continue;
                }
                if p.is_punct("...") {
                    p.bump();
                }
                if p.is_punct("{") || p.is_punct("[") {
                    p.collect_pattern_names(names)?;
                } else {
                    let key = match p.tok() {
                        Some(Tok::Ident(name) | Tok::Str(name)) => name.clone(),
                        _ => return Err(format!("unexpected {} in a pattern", p.describe())),
                    };
                    p.bump();
                    if object && p.is_punct(":") {
                        p.bump();
                        if p.is_punct("{") || p.is_punct("[") {
                            p.collect_pattern_names(names)?;
                        } else {
                            names.push(p.expect_ident()?);
                        }
                    } else {
                        names.push(key);
                    }
                }
                if p.is_punct("=") {
                    p.bump();
                    p.expr()?;
                }
                if !p.is_punct(",") && !p.is_punct(close) {
                    return Err(format!("unexpected {} in a pattern", p.describe()));
                }
            }
        })
    }

    fn js_function_decl(&mut self, is_async: bool, start: usize) -> PResult<StmtKind> {
        let first = self.pos;
        if self.is_punct("*") {
            return Err("generator functions are not read".to_owned());
        }
        let name = self.expect_ident()?;
        let func = self.js_function_rest(is_async, start, first)?;
        Ok(StmtKind::FuncDef {
            name,
            func,
            decorators: Vec::new(),
        })
    }

    /// Parameters, an optional return type and a `{}` body.
    fn js_function_rest(
        &mut self,
        is_async: bool,
        start: usize,
        first: usize,
    ) -> PResult<Function> {
        if self.is_punct("<") {
            self.skip_angles();
        }
        let (params, mut typescript) = self.js_params()?;
        if self.is_punct(":") {
            self.bump();
            self.skip_type(false);
            typescript = true;
        }
        if !self.is_punct("{") {
            return Err(format!(
                "expected a function body but found {}",
                self.describe()
            ));
        }
        self.bump();
        let mut body = Vec::new();
        self.js_block(&mut body, true);
        self.expect_punct("}")?;
        Ok(self.function(
            params,
            FnBody::Block(body),
            is_async,
            start,
            first,
            typescript,
        ))
    }

    fn function(
        &self,
        params: Vec<Param>,
        body: FnBody,
        is_async: bool,
        start: usize,
        first: usize,
        typescript: bool,
    ) -> Function {
        let mut names = Vec::new();
        let mut typescript = typescript;
        let tokens = &self.toks[first.min(self.pos)..self.pos];
        for (index, token) in tokens.iter().enumerate() {
            if let Tok::Ident(name) = &token.tok {
                let after_dot =
                    index > 0 && matches!(tokens[index - 1].tok, Tok::Punct("." | "?."));
                if !after_dot {
                    if matches!(name.as_str(), "as" | "satisfies") && self.dialect == Dialect::Js {
                        typescript = true;
                    }
                    names.push(name.clone());
                }
            }
        }
        Function {
            params,
            body,
            is_async,
            start,
            end: self.prev_end(),
            names,
            typescript,
        }
    }

    /// `( params )`; also says whether a parameter has a type annotation.
    fn js_params(&mut self) -> PResult<(Vec<Param>, bool)> {
        self.expect_punct("(")?;
        let mut params = Vec::new();
        let mut typescript = false;
        loop {
            if self.is_punct(")") {
                self.bump();
                return Ok((params, typescript));
            }
            if self.is_punct("...") {
                self.bump();
            }
            let param = match self.binding_pattern()? {
                Pattern::Name(name) => Param {
                    name: Some(name),
                    fields: Vec::new(),
                },
                Pattern::Destructure(fields) => Param { name: None, fields },
            };
            params.push(param);
            if self.is_punct("?") {
                self.bump();
                typescript = true;
            }
            if self.is_punct(":") {
                self.bump();
                self.skip_type(false);
                typescript = true;
            }
            if self.is_punct("=") {
                self.bump();
                self.expr()?;
            }
            if self.is_punct(",") {
                self.bump();
            } else if !self.is_punct(")") {
                return Err(format!("unexpected {} in parameters", self.describe()));
            }
        }
    }

    /// Skips one statement: to a `;`, or a line break that is not followed
    /// by a continuation, at the outer bracket level. Always moves forward.
    fn skip_js_statement(&mut self) {
        let first = self.pos;
        let mut open: Vec<&'static str> = Vec::new();
        let mut closed = true;
        while let Some(token) = self.peek() {
            if self.pos > first && open.is_empty() && token.nl_before && !continues_line(&token.tok)
            {
                break;
            }
            match token.tok {
                Tok::Punct("(") => open.push(")"),
                Tok::Punct("[") => open.push("]"),
                Tok::Punct("{") => open.push("}"),
                Tok::Punct(closer @ (")" | "]" | "}")) => match open.last() {
                    // The end of the block around the statement.
                    None => break,
                    Some(expected) if *expected == closer => {
                        open.pop();
                    }
                    // A bracket that does not match: the statement is broken
                    // here, so do not let it swallow the enclosing block.
                    Some(_) => {
                        closed = false;
                        break;
                    }
                },
                Tok::Punct(";") if open.is_empty() => {
                    self.bump();
                    break;
                }
                _ => {}
            }
            self.bump();
            closed = open.is_empty();
        }
        if !closed {
            // Brackets that never close: skip the first line only, rather
            // than the rest of the file.
            self.pos = first;
            self.bump();
            while self.peek().is_some_and(|t| !t.nl_before) {
                self.bump();
            }
        }
        if self.pos == first {
            self.bump();
        }
    }

    /// Skips a compound statement: its header, a `{}` body (or a body that
    /// ends at `;`), and any `else`, `catch` or `finally` parts.
    fn skip_js_compound(&mut self) {
        let is_do = self.is_ident("do");
        loop {
            let first = self.pos;
            let mut depth = 0_usize;
            while let Some(token) = self.peek() {
                match token.tok {
                    Tok::Punct("{") if depth == 0 => {
                        self.skip_balanced();
                        break;
                    }
                    Tok::Punct("(" | "[" | "{") => depth += 1,
                    Tok::Punct(")" | "]" | "}") => {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    Tok::Punct(";") if depth == 0 => {
                        self.bump();
                        break;
                    }
                    _ => {}
                }
                self.bump();
            }
            if self.pos == first {
                self.bump();
            }
            if self.is_ident("else") || self.is_ident("catch") || self.is_ident("finally") {
                continue;
            }
            if is_do && self.is_ident("while") {
                self.skip_js_statement();
            }
            break;
        }
    }

    /// Skips a TypeScript type. With `stop_at_arrow`, a `=>` ends it (the
    /// return type of an arrow function).
    fn skip_type(&mut self, stop_at_arrow: bool) {
        let mut expect_operand = true;
        while let Some(tok) = self.tok() {
            match tok {
                Tok::Punct("(" | "[" | "{") if expect_operand => {
                    self.skip_balanced();
                    expect_operand = false;
                }
                Tok::Punct("[") => self.skip_balanced(),
                Tok::Punct("<") => {
                    self.skip_angles();
                    expect_operand = false;
                }
                Tok::Punct("=>") if stop_at_arrow => break,
                Tok::Punct("|" | "&" | "." | "=>" | "...") => {
                    self.bump();
                    expect_operand = true;
                }
                Tok::Punct("?") if !expect_operand => self.bump(),
                Tok::Punct("-") if expect_operand => self.bump(),
                Tok::Ident(word)
                    if matches!(
                        word.as_str(),
                        "is" | "extends" | "keyof" | "typeof" | "infer" | "readonly" | "unique"
                    ) =>
                {
                    self.bump();
                    expect_operand = true;
                }
                Tok::Ident(_) | Tok::Str(_) | Tok::Num(_) if expect_operand => {
                    self.bump();
                    expect_operand = false;
                }
                _ => break,
            }
        }
    }

    /// Skips `<...>` generic arguments, counting `>>` as two closers.
    fn skip_angles(&mut self) {
        let mut depth = 0_i64;
        while let Some(tok) = self.tok() {
            match tok {
                Tok::Punct("<") => depth += 1,
                Tok::Punct(">") => depth -= 1,
                Tok::Punct(">>") => depth -= 2,
                Tok::Punct(">>>") => depth -= 3,
                Tok::Punct("(" | "[" | "{") => {
                    self.skip_balanced();
                    continue;
                }
                Tok::Punct(";" | ")" | "]" | "}") => return,
                _ => {}
            }
            self.bump();
            if depth <= 0 {
                return;
            }
        }
    }

    /// When `<` here opens generic arguments of a call (`f<T>(x)`), the index
    /// of the `(` after them.
    fn generic_call_end(&self) -> Option<usize> {
        let mut depth = 0_i64;
        for (offset, token) in self.toks[self.pos..].iter().enumerate().take(64) {
            match &token.tok {
                Tok::Punct("<") => depth += 1,
                Tok::Punct(">") => depth -= 1,
                Tok::Punct(">>") => depth -= 2,
                Tok::Punct(
                    "," | "." | "[" | "]" | "|" | "&" | "{" | "}" | ":" | ";" | "?" | "=>" | "("
                    | ")",
                )
                | Tok::Ident(_)
                | Tok::Str(_)
                | Tok::Num(_) => {}
                _ => return None,
            }
            if depth == 0 {
                let next = self.pos + offset + 1;
                return self
                    .toks
                    .get(next)
                    .is_some_and(|t| t.tok == Tok::Punct("("))
                    .then_some(next);
            }
            if depth < 0 {
                return None;
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Python statements
    // ------------------------------------------------------------------

    fn py_block(&mut self, out: &mut Vec<Stmt>, indent: usize) {
        loop {
            let Some(token) = self.peek() else { break };
            if token.col < indent && token.line_start {
                break;
            }
            let (line, start) = (token.line, token.start);
            self.drain_comments(start, out);
            let start_pos = self.pos;
            let kept = out.len();
            if let Err(message) = self.guarded(|p| p.py_statement(out, indent)) {
                out.truncate(kept);
                self.pos = start_pos;
                self.skip_py_compound(indent);
                out.push(Stmt {
                    line,
                    start,
                    end: self.prev_end().max(start),
                    kind: StmtKind::Unsupported(format!(
                        "could not read this statement: {message}"
                    )),
                });
            }
            if self.pos == start_pos {
                self.bump();
            }
            let end = self.prev_end();
            self.skip_comments(end);
        }
    }

    fn py_statement(&mut self, out: &mut Vec<Stmt>, indent: usize) -> PResult<()> {
        let Some(token) = self.peek() else {
            return Ok(());
        };
        let (line, start) = (token.line, token.start);
        let word = match &token.tok {
            Tok::Ident(word) => word.clone(),
            Tok::Punct("@") => String::from("@"),
            _ => String::new(),
        };
        let kind = match word.as_str() {
            "import" | "from" => {
                self.skip_py_line();
                StmtKind::Ignored
            }
            "@" => {
                let mut decorators = Vec::new();
                while self.is_punct("@") {
                    self.bump();
                    decorators.push(self.expr()?);
                    if !self.at_line_end() {
                        return Err("expected the end of a decorator line".to_owned());
                    }
                }
                if self.is_ident("def") {
                    self.py_def(decorators, false, indent, start)?
                } else if self.is_ident("async") && self.is_ident_at(1, "def") {
                    self.bump();
                    self.py_def(decorators, true, indent, start)?
                } else {
                    self.skip_py_compound(indent);
                    StmtKind::Unsupported("a decorated class".to_owned())
                }
            }
            "def" => self.py_def(Vec::new(), false, indent, start)?,
            "async" if self.is_ident_at(1, "def") => {
                self.bump();
                self.py_def(Vec::new(), true, indent, start)?
            }
            "async" if self.is_ident_at(1, "with") => {
                self.bump();
                self.py_with(indent)?
            }
            "with" => self.py_with(indent)?,
            "if" => {
                let save = self.pos;
                self.bump();
                let condition = self.expr();
                if condition.as_ref().is_ok_and(is_main_check) && self.is_punct(":") {
                    self.bump();
                    StmtKind::IfMain(self.py_suite(indent)?)
                } else {
                    self.pos = save;
                    self.skip_py_compound(indent);
                    StmtKind::Unsupported("an if statement".to_owned())
                }
            }
            "class" | "for" | "while" | "try" | "match" | "async"
                if word != "match" || !self.is_punct_at(1, "=") =>
            {
                self.skip_py_compound(indent);
                StmtKind::Unsupported(describe_keyword(&word).to_owned())
            }
            "assert" => {
                self.bump();
                let test = self.expr()?;
                if self.is_punct(",") {
                    self.bump();
                    self.expr()?;
                }
                StmtKind::Assert(test)
            }
            "return" => {
                self.bump();
                let value = if self.at_line_end() || self.is_punct(";") {
                    None
                } else {
                    Some(self.expr()?)
                };
                StmtKind::Return(value)
            }
            "pass" => {
                self.bump();
                StmtKind::Ignored
            }
            "raise" | "del" | "global" | "nonlocal" | "break" | "continue" | "yield" => {
                self.skip_py_line();
                StmtKind::Unsupported(describe_keyword(&word).to_owned())
            }
            _ => self.py_simple_statement()?,
        };
        let end = self.prev_end();
        out.push(Stmt {
            line,
            start,
            end,
            kind,
        });
        if self.is_punct(";") {
            self.bump();
            if !self.at_line_end() {
                // `a; b` on one line: the rest of the line is read by the
                // block loop as the next statement.
                return Ok(());
            }
        }
        if !self.at_line_end() {
            return Err(format!(
                "expected the end of the line but found {}",
                self.describe()
            ));
        }
        Ok(())
    }

    fn py_simple_statement(&mut self) -> PResult<StmtKind> {
        let expr = self.expr()?;
        if self.is_punct("=") {
            self.bump();
            let value = self.expr()?;
            if self.is_punct("=") {
                return Err("chained assignments are not read".to_owned());
            }
            return Ok(StmtKind::Assign {
                target: expr,
                value,
            });
        }
        if self.is_punct(":") {
            // An annotated assignment: `x: int = 1`.
            self.bump();
            let mut depth = 0_usize;
            while let Some(tok) = self.tok() {
                if depth == 0 && (self.at_line_end() || matches!(tok, Tok::Punct("="))) {
                    break;
                }
                match tok {
                    Tok::Punct("(" | "[" | "{") => depth += 1,
                    Tok::Punct(")" | "]" | "}") => depth = depth.saturating_sub(1),
                    _ => {}
                }
                self.bump();
            }
            if self.is_punct("=") {
                self.bump();
                let value = self.expr()?;
                return Ok(StmtKind::Assign {
                    target: expr,
                    value,
                });
            }
            return Ok(StmtKind::Ignored);
        }
        if let Some(Tok::Punct(op)) = self.tok() {
            if COMPOUND_ASSIGN.contains(op) {
                let op = *op;
                self.skip_py_line();
                return Ok(StmtKind::Unsupported(format!("a `{op}` assignment")));
            }
        }
        Ok(StmtKind::Expr(expr))
    }

    fn py_def(
        &mut self,
        decorators: Vec<Expr>,
        is_async: bool,
        indent: usize,
        start: usize,
    ) -> PResult<StmtKind> {
        let first = self.pos;
        self.bump();
        let name = self.expect_ident()?;
        let params = self.py_params()?;
        if self.is_punct("->") {
            self.bump();
            while !self.at_end() && !self.is_punct(":") && !self.at_line_end() {
                if self.is_punct("[") || self.is_punct("(") {
                    self.skip_balanced();
                } else {
                    self.bump();
                }
            }
        }
        self.expect_punct(":")?;
        let body = self.py_suite(indent)?;
        let func = self.function(params, FnBody::Block(body), is_async, start, first, false);
        Ok(StmtKind::FuncDef {
            name,
            func,
            decorators,
        })
    }

    fn py_params(&mut self) -> PResult<Vec<Param>> {
        self.expect_punct("(")?;
        let mut params = Vec::new();
        loop {
            if self.is_punct(")") {
                self.bump();
                return Ok(params);
            }
            if self.is_punct("*") || self.is_punct("**") || self.is_punct("/") {
                self.bump();
                if self.is_punct(",") || self.is_punct(")") {
                    if self.is_punct(",") {
                        self.bump();
                    }
                    continue;
                }
            }
            let name = self.expect_ident()?;
            params.push(Param {
                name: Some(name),
                fields: Vec::new(),
            });
            if self.is_punct(":") {
                self.bump();
                let mut depth = 0_usize;
                while let Some(tok) = self.tok() {
                    match tok {
                        Tok::Punct("," | ")" | "=") if depth == 0 => break,
                        Tok::Punct("(" | "[" | "{") => depth += 1,
                        Tok::Punct(")" | "]" | "}") => depth = depth.saturating_sub(1),
                        _ => {}
                    }
                    self.bump();
                }
            }
            if self.is_punct("=") {
                self.bump();
                self.expr()?;
            }
            if self.is_punct(",") {
                self.bump();
            } else if !self.is_punct(")") {
                return Err(format!("unexpected {} in parameters", self.describe()));
            }
        }
    }

    fn py_with(&mut self, indent: usize) -> PResult<StmtKind> {
        self.bump();
        let mut items = Vec::new();
        loop {
            let item = self.expr()?;
            let name = if self.is_ident("as") {
                self.bump();
                Some(self.expect_ident()?)
            } else {
                None
            };
            items.push((item, name));
            if self.is_punct(",") {
                self.bump();
                continue;
            }
            break;
        }
        self.expect_punct(":")?;
        let body = self.py_suite(indent)?;
        Ok(StmtKind::With { items, body })
    }

    /// The body after a `:`: the rest of the line, or an indented block.
    fn py_suite(&mut self, header_indent: usize) -> PResult<Vec<Stmt>> {
        let mut body = Vec::new();
        if !self.at_line_end() {
            self.py_statement(&mut body, header_indent)?;
            return Ok(body);
        }
        let Some(token) = self.peek() else {
            return Ok(body);
        };
        if token.col <= header_indent {
            return Ok(body);
        }
        let indent = token.col;
        self.py_block(&mut body, indent);
        Ok(body)
    }

    /// Skips to the start of the next logical line, moving at least one token.
    fn skip_py_line(&mut self) {
        self.bump();
        while !self.at_line_end() {
            self.bump();
        }
    }

    /// Skips a statement with its indented block and any `elif`, `else`,
    /// `except` or `finally` parts.
    fn skip_py_compound(&mut self, indent: usize) {
        loop {
            self.skip_py_line();
            while self.peek().is_some_and(|t| t.col > indent) {
                self.skip_py_line();
            }
            let continues = self.peek().is_some_and(|t| t.col == indent)
                && ["elif", "else", "except", "finally", "case"]
                    .iter()
                    .any(|word| self.is_ident(word));
            if !continues {
                break;
            }
        }
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn expr(&mut self) -> PResult<Expr> {
        self.guarded(Self::conditional)
    }

    fn conditional(&mut self) -> PResult<Expr> {
        if self.python() && self.is_ident("lambda") {
            while !self.at_end() && !self.is_punct(":") {
                self.bump();
            }
            self.expect_punct(":")?;
            self.expr()?;
            return Ok(Expr::Opaque);
        }
        let test = self.binary(0)?;
        if !self.python() && self.is_punct("?") {
            self.bump();
            self.expr()?;
            self.expect_punct(":")?;
            self.expr()?;
            return Ok(Expr::Opaque);
        }
        if self.python() && !self.at_line_end() && self.is_ident("if") {
            self.bump();
            self.binary(0)?;
            if !self.is_ident("else") {
                return Err("expected `else` in a conditional expression".to_owned());
            }
            self.bump();
            self.expr()?;
            return Ok(Expr::Opaque);
        }
        Ok(test)
    }

    fn binary(&mut self, level: usize) -> PResult<Expr> {
        let levels = if self.python() { PY_LEVELS } else { JS_LEVELS };
        if level >= levels.len() {
            return self.unary();
        }
        if self.python() && level == 2 {
            if self.is_ident("not") && !self.at_line_end() {
                self.bump();
                let operand = self.guarded(|p| p.binary(2))?;
                return Ok(Expr::Unary {
                    op: "not",
                    operand: Box::new(operand),
                });
            }
            return self.binary(3);
        }
        let mut left = self.binary(level + 1)?;
        loop {
            if self.python() && self.at_line_end() {
                break;
            }
            let Some(op) = self.binary_op(levels[level]) else {
                break;
            };
            let right = self.binary(level + 1)?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Consumes a binary operator of this level, if one is next.
    fn binary_op(&mut self, ops: &[&'static str]) -> Option<&'static str> {
        let word = match self.tok()? {
            Tok::Punct(p) => *p,
            Tok::Ident(name) => match name.as_str() {
                "in" => "in",
                "instanceof" => "instanceof",
                "or" => "or",
                "and" => "and",
                "is" => "is",
                "not" if self.python() && self.is_ident_at(1, "in") => "not in",
                _ => return None,
            },
            _ => return None,
        };
        let op = ops.iter().copied().find(|op| {
            *op == word || (*op == "is not" && word == "is" && self.is_ident_at(1, "not"))
        })?;
        match op {
            "not in" | "is not" => {
                self.bump();
                self.bump();
            }
            _ => self.bump(),
        }
        Some(op)
    }

    fn unary(&mut self) -> PResult<Expr> {
        self.guarded(|p| {
            let op = match p.tok() {
                Some(Tok::Punct(op @ ("!" | "-" | "+" | "~"))) if !(p.python() && *op == "!") => {
                    Some(*op)
                }
                Some(Tok::Ident(word)) if !p.python() => match word.as_str() {
                    "typeof" => Some("typeof"),
                    "void" => Some("void"),
                    "delete" => Some("delete"),
                    _ => None,
                },
                _ => None,
            };
            if let Some(op) = op {
                p.bump();
                let operand = p.unary()?;
                return Ok(Expr::Unary {
                    op,
                    operand: Box::new(operand),
                });
            }
            if p.is_ident("await") {
                p.bump();
                return Ok(Expr::Await(Box::new(p.unary()?)));
            }
            if !p.python() && (p.is_punct("++") || p.is_punct("--")) {
                p.bump();
                p.unary()?;
                return Ok(Expr::Opaque);
            }
            if !p.python() && p.is_ident("new") {
                p.bump();
                let callee = p.primary()?;
                let mut callee = callee;
                while p.is_punct(".") {
                    p.bump();
                    let property = p.expect_ident()?;
                    callee = Expr::Member {
                        object: Box::new(callee),
                        property,
                    };
                }
                let args = if p.is_punct("(") {
                    p.call_args()?
                } else {
                    Vec::new()
                };
                let made = Expr::New {
                    callee: Box::new(callee),
                    args,
                };
                return p.postfix_from(made);
            }
            let primary = p.primary()?;
            p.postfix_from(primary)
        })
    }

    fn postfix_from(&mut self, mut expr: Expr) -> PResult<Expr> {
        loop {
            if self.python() && self.at_line_end() {
                break;
            }
            let Some(token) = self.peek() else { break };
            match &token.tok {
                Tok::Punct("." | "?.") => {
                    self.bump();
                    if self.is_punct("(") {
                        let args = self.call_args()?;
                        expr = Expr::Call {
                            callee: Box::new(expr),
                            args,
                        };
                    } else if self.is_punct("[") {
                        expr = self.index(expr)?;
                    } else {
                        let property = match self.tok() {
                            Some(Tok::Ident(name)) => name.clone(),
                            _ => {
                                return Err(format!(
                                    "expected a property name but found {}",
                                    self.describe()
                                ))
                            }
                        };
                        self.bump();
                        expr = Expr::Member {
                            object: Box::new(expr),
                            property,
                        };
                    }
                }
                Tok::Punct("(") => {
                    let args = self.call_args()?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                    };
                }
                Tok::Punct("[") if !token.nl_before || self.python() => {
                    expr = self.index(expr)?;
                }
                Tok::Punct("!") if !self.python() && !token.nl_before => {
                    // A TypeScript non-null assertion.
                    self.bump();
                }
                Tok::Punct("++" | "--") if !self.python() && !token.nl_before => {
                    self.bump();
                    expr = Expr::Opaque;
                }
                Tok::Ident(word)
                    if !self.python()
                        && !token.nl_before
                        && matches!(word.as_str(), "as" | "satisfies") =>
                {
                    self.bump();
                    self.skip_type(false);
                }
                Tok::Punct("<") if !self.python() => match self.generic_call_end() {
                    Some(open) => self.pos = open,
                    None => break,
                },
                _ => break,
            }
        }
        Ok(expr)
    }

    fn index(&mut self, object: Expr) -> PResult<Expr> {
        self.expect_punct("[")?;
        let index = self.expr()?;
        if !self.is_punct("]") {
            return Err(format!("expected `]` but found {}", self.describe()));
        }
        self.bump();
        Ok(Expr::Index {
            object: Box::new(object),
            index: Box::new(index),
        })
    }

    fn call_args(&mut self) -> PResult<Vec<Arg>> {
        self.expect_punct("(")?;
        let mut args = Vec::new();
        loop {
            if self.is_punct(")") {
                self.bump();
                return Ok(args);
            }
            let spread = if self.python() {
                self.is_punct("*") || self.is_punct("**")
            } else {
                self.is_punct("...")
            };
            if spread {
                self.bump();
                self.expr()?;
                args.push(Arg::Spread);
            } else if self.python()
                && matches!(self.tok(), Some(Tok::Ident(_)))
                && self.is_punct_at(1, "=")
            {
                let name = self.expect_ident()?;
                self.bump();
                args.push(Arg::Kw(name, self.expr()?));
            } else {
                args.push(Arg::Pos(self.expr()?));
            }
            if self.python() && self.is_ident("for") {
                return Err("generator expressions are not read".to_owned());
            }
            if self.is_punct(",") {
                self.bump();
            } else if !self.is_punct(")") {
                return Err(format!("expected `,` or `)` but found {}", self.describe()));
            }
        }
    }

    fn primary(&mut self) -> PResult<Expr> {
        let Some(token) = self.peek() else {
            return Err("unexpected end of the file".to_owned());
        };
        let start = token.start;
        match token.tok.clone() {
            Tok::Ident(name) => self.ident_primary(&name, start),
            Tok::Str(value) => {
                self.bump();
                Ok(Expr::Str(value))
            }
            Tok::Interpolated(_) => {
                self.bump();
                Ok(Expr::Interpolated)
            }
            Tok::Num(value) => {
                self.bump();
                Ok(Expr::Num(value))
            }
            Tok::Regex { pattern, flags } => {
                self.bump();
                Ok(Expr::Regex { pattern, flags })
            }
            Tok::Punct("(") => {
                if !self.python() && self.is_arrow_parens(self.pos) {
                    return self.arrow(false, start);
                }
                self.bump();
                if self.is_punct(")") {
                    self.bump();
                    return Ok(Expr::Opaque);
                }
                let inner = self.expr()?;
                if self.is_punct(",") {
                    // A tuple or a comma expression.
                    while !self.at_end() && !self.is_punct(")") {
                        if self.is_punct("(") || self.is_punct("[") || self.is_punct("{") {
                            self.skip_balanced();
                        } else {
                            self.bump();
                        }
                    }
                    self.expect_punct(")")?;
                    return Ok(Expr::Opaque);
                }
                if self.python() && self.is_ident("for") {
                    return Err("generator expressions are not read".to_owned());
                }
                self.expect_punct(")")?;
                Ok(inner)
            }
            Tok::Punct("[") => self.array(),
            Tok::Punct("{") => {
                if self.python() {
                    self.dict()
                } else {
                    self.object()
                }
            }
            _ => Err(format!("unexpected {}", self.describe())),
        }
    }

    fn ident_primary(&mut self, name: &str, start: usize) -> PResult<Expr> {
        let js = !self.python();
        let literal = match name {
            "true" if js => Some(Expr::Bool(true)),
            "false" if js => Some(Expr::Bool(false)),
            "null" if js => Some(Expr::Null),
            "undefined" if js => Some(Expr::Undefined),
            "True" if !js => Some(Expr::Bool(true)),
            "False" if !js => Some(Expr::Bool(false)),
            "None" if !js => Some(Expr::Null),
            _ => None,
        };
        if let Some(literal) = literal {
            self.bump();
            return Ok(literal);
        }
        if js {
            if name == "function" {
                let first = self.pos;
                self.bump();
                if matches!(self.tok(), Some(Tok::Ident(_))) {
                    self.bump();
                }
                let func = self.js_function_rest(false, start, first)?;
                return Ok(Expr::Function(Box::new(func)));
            }
            if name == "async" {
                let next_same_line = self.peek_at(1).is_some_and(|t| !t.nl_before);
                if next_same_line && self.is_ident_at(1, "function") {
                    let first = self.pos;
                    self.bump();
                    self.bump();
                    if matches!(self.tok(), Some(Tok::Ident(_))) {
                        self.bump();
                    }
                    let func = self.js_function_rest(true, start, first)?;
                    return Ok(Expr::Function(Box::new(func)));
                }
                if next_same_line
                    && (self.is_arrow_parens(self.pos + 1)
                        || (matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Ident(_)))
                            && self.is_punct_at(2, "=>")))
                {
                    return self.arrow(true, start);
                }
            }
            if self.is_punct_at(1, "=>") {
                return self.arrow(false, start);
            }
            if name == "class" {
                return Err("class expressions are not read".to_owned());
            }
        }
        self.bump();
        Ok(Expr::Ident(name.to_owned()))
    }

    /// Whether the `(` at `open` starts the parameters of an arrow function.
    fn is_arrow_parens(&self, open: usize) -> bool {
        if !matches!(self.toks.get(open).map(|t| &t.tok), Some(Tok::Punct("("))) {
            return false;
        }
        let Some(close) = self.matching_close(open) else {
            return false;
        };
        match self.toks.get(close + 1).map(|t| &t.tok) {
            Some(Tok::Punct("=>")) => true,
            Some(Tok::Punct(":")) => {
                // A return type, then `=>`.
                let mut depth = 0_usize;
                for token in self.toks.iter().skip(close + 2).take(64) {
                    match token.tok {
                        Tok::Punct("=>") if depth == 0 => return true,
                        Tok::Punct("(" | "[" | "{" | "<") => depth += 1,
                        Tok::Punct(")" | "]" | "}" | ">") => match depth.checked_sub(1) {
                            Some(d) => depth = d,
                            None => return false,
                        },
                        Tok::Punct(";" | ",") if depth == 0 => return false,
                        _ => {}
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn arrow(&mut self, is_async: bool, start: usize) -> PResult<Expr> {
        let first = self.pos;
        if is_async {
            self.bump();
        }
        let (params, mut typescript) = if self.is_punct("(") {
            self.js_params()?
        } else {
            let name = self.expect_ident()?;
            (
                vec![Param {
                    name: Some(name),
                    fields: Vec::new(),
                }],
                false,
            )
        };
        if self.is_punct(":") {
            self.bump();
            self.skip_type(true);
            typescript = true;
        }
        self.expect_punct("=>")?;
        let body = if self.is_punct("{") {
            self.bump();
            let mut body = Vec::new();
            self.js_block(&mut body, true);
            self.expect_punct("}")?;
            FnBody::Block(body)
        } else {
            FnBody::Expr(Box::new(self.expr()?))
        };
        let func = self.function(params, body, is_async, start, first, typescript);
        Ok(Expr::Function(Box::new(func)))
    }

    fn array(&mut self) -> PResult<Expr> {
        self.expect_punct("[")?;
        let mut items = Vec::new();
        loop {
            if self.is_punct("]") {
                self.bump();
                return Ok(Expr::Array(items));
            }
            if self.is_punct(",") {
                self.bump();
                items.push(Expr::Undefined);
                continue;
            }
            if self.is_punct("...") || (self.python() && self.is_punct("*")) {
                self.bump();
                self.expr()?;
                items.push(Expr::Opaque);
            } else {
                items.push(self.expr()?);
            }
            if self.python() && self.is_ident("for") {
                return Err("list comprehensions are not read".to_owned());
            }
            if self.is_punct(",") {
                self.bump();
            } else if !self.is_punct("]") {
                return Err(format!("expected `,` or `]` but found {}", self.describe()));
            }
        }
    }

    fn object(&mut self) -> PResult<Expr> {
        self.expect_punct("{")?;
        let mut entries = Vec::new();
        loop {
            if self.is_punct("}") {
                self.bump();
                return Ok(Expr::Object(entries));
            }
            if self.is_punct("...") {
                self.bump();
                self.expr()?;
                entries.push(("...".to_owned(), Expr::Opaque));
            } else {
                // `get x() {}`, `async x() {}`: a method with a modifier.
                let modifier = matches!(self.tok(), Some(Tok::Ident(w)) if matches!(w.as_str(), "get" | "set" | "async"))
                    && matches!(
                        self.peek_at(1).map(|t| &t.tok),
                        Some(Tok::Ident(_) | Tok::Str(_))
                    );
                if modifier {
                    self.bump();
                }
                let key = match self.tok() {
                    Some(Tok::Ident(name) | Tok::Str(name)) => {
                        let key = name.clone();
                        self.bump();
                        key
                    }
                    Some(Tok::Num(n)) => {
                        let key = format_number(*n);
                        self.bump();
                        key
                    }
                    Some(Tok::Punct("[")) => {
                        self.skip_balanced();
                        "...".to_owned()
                    }
                    _ => return Err(format!("unexpected {} in an object", self.describe())),
                };
                if self.is_punct(":") {
                    self.bump();
                    let value = self.expr()?;
                    entries.push((key, value));
                } else if self.is_punct("(") {
                    let start = self.peek().map_or(0, |t| t.start);
                    let first = self.pos;
                    self.js_function_rest(false, start, first)?;
                    entries.push((key, Expr::Opaque));
                } else if self.is_punct(",") || self.is_punct("}") {
                    entries.push((key.clone(), Expr::Ident(key)));
                } else {
                    return Err(format!("unexpected {} in an object", self.describe()));
                }
            }
            if self.is_punct(",") {
                self.bump();
            } else if !self.is_punct("}") {
                return Err(format!(
                    "expected `,` or `}}` but found {}",
                    self.describe()
                ));
            }
        }
    }

    fn dict(&mut self) -> PResult<Expr> {
        self.expect_punct("{")?;
        let mut entries = Vec::new();
        loop {
            if self.is_punct("}") {
                self.bump();
                return Ok(Expr::Object(entries));
            }
            if self.is_punct("**") {
                self.bump();
                self.expr()?;
                entries.push(("...".to_owned(), Expr::Opaque));
            } else {
                let key = self.expr()?;
                if self.is_ident("for") {
                    return Err("dict comprehensions are not read".to_owned());
                }
                if !self.is_punct(":") {
                    // A set literal.
                    while !self.at_end() && !self.is_punct("}") {
                        self.bump();
                    }
                    self.expect_punct("}")?;
                    return Ok(Expr::Opaque);
                }
                self.bump();
                let value = self.expr()?;
                let key = match key {
                    Expr::Str(key) => key,
                    _ => "...".to_owned(),
                };
                entries.push((key, value));
            }
            if self.is_punct(",") {
                self.bump();
            } else if !self.is_punct("}") {
                return Err(format!(
                    "expected `,` or `}}` but found {}",
                    self.describe()
                ));
            }
        }
    }
}

/// Whether a token at the start of a line continues the statement above.
fn continues_line(tok: &Tok) -> bool {
    matches!(
        tok,
        Tok::Punct(
            "." | "?."
                | "&&"
                | "||"
                | "??"
                | "=>"
                | ","
                | "+"
                | "*"
                | "/"
                | "="
                | "?"
                | ":"
                | ")"
                | "]"
        )
    )
}

/// `__name__ == "__main__"`, either way round.
fn is_main_check(expr: &Expr) -> bool {
    match expr {
        Expr::Binary {
            op: "==",
            left,
            right,
        } => matches!(
            (left.as_ref(), right.as_ref()),
            (Expr::Ident(name), Expr::Str(value)) | (Expr::Str(value), Expr::Ident(name))
                if name == "__name__" && value == "__main__"
        ),
        _ => false,
    }
}

/// What a statement keyword introduces, for a diagnostic.
fn describe_keyword(word: &str) -> &'static str {
    match word {
        "if" => "an if statement",
        "for" => "a for loop",
        "while" => "a while loop",
        "do" => "a do-while loop",
        "switch" => "a switch statement",
        "match" => "a match statement",
        "try" => "a try statement",
        "class" => "a class",
        "with" => "a with statement",
        "throw" | "raise" => "throwing an exception",
        "break" => "a break statement",
        "continue" => "a continue statement",
        "async" => "an async loop or block",
        "del" => "a del statement",
        "global" | "nonlocal" => "a global declaration",
        "yield" => "a yield statement",
        _ => "a statement",
    }
}

/// A number as a key or a name: whole numbers without a decimal point.
pub(super) fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{n:.0}")
    } else {
        format!("{n}")
    }
}

/// At most 40 characters of `text`, for an error message.
fn short(text: &str) -> String {
    let mut out: String = text.chars().take(40).collect();
    if text.chars().count() > 40 {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::lexer::tokenize;
    use super::*;

    fn js(source: &str) -> Vec<Stmt> {
        let lexed = tokenize(source, Dialect::Js).unwrap();
        parse(&lexed, Dialect::Js)
    }

    fn py(source: &str) -> Vec<Stmt> {
        let lexed = tokenize(source, Dialect::Python).unwrap();
        parse(&lexed, Dialect::Python)
    }

    fn kinds(stmts: &[Stmt]) -> Vec<String> {
        stmts
            .iter()
            .map(|s| match &s.kind {
                StmtKind::Comment(_) => "comment".to_owned(),
                StmtKind::Ignored => "ignored".to_owned(),
                StmtKind::Expr(_) => "expr".to_owned(),
                StmtKind::Decl(_) => "decl".to_owned(),
                StmtKind::Assign { .. } => "assign".to_owned(),
                StmtKind::FuncDef { name, .. } => format!("def {name}"),
                StmtKind::With { .. } => "with".to_owned(),
                StmtKind::Assert(_) => "assert".to_owned(),
                StmtKind::Return(_) => "return".to_owned(),
                StmtKind::IfMain(_) => "main".to_owned(),
                StmtKind::Unsupported(what) => format!("unsupported: {what}"),
            })
            .collect()
    }

    #[test]
    fn a_playwright_test_file_reads_as_calls_with_arrow_bodies() {
        let stmts = js("import { test, expect } from '@playwright/test';\n\
             // a comment\n\
             test('signs in', async ({ page }) => {\n  await page.goto('/');\n});\n");
        assert_eq!(kinds(&stmts), ["ignored", "comment", "expr"]);
        let StmtKind::Expr(Expr::Call { args, .. }) = &stmts[2].kind else {
            panic!("{:?}", stmts[2])
        };
        let Arg::Pos(Expr::Function(func)) = &args[1] else {
            panic!("{args:?}")
        };
        assert!(func.is_async);
        assert_eq!(func.params[0].fields, ["page"]);
        let FnBody::Block(body) = &func.body else {
            panic!()
        };
        assert_eq!(kinds(body), ["expr"]);
        assert_eq!(stmts[2].line, 3);
    }

    #[test]
    fn typescript_annotations_casts_and_generics_are_read_past() {
        let stmts = js("const x: Locator = page.locator('a') as Locator;\n\
             const y = await page.evaluate<string>(() => (document.body as HTMLElement).id);\n\
             test('t', async ({ page }: { page: Page }): Promise<void> => { page!.reload(); });\n\
             interface Foo { a: string }\n\
             type Bar = { b: number };\n");
        assert_eq!(
            kinds(&stmts),
            ["decl", "decl", "expr", "ignored", "ignored"]
        );
    }

    #[test]
    fn statements_without_semicolons_end_at_the_line_unless_it_continues() {
        let stmts = js("await page\n  .locator('a')\n  .click()\nawait page.reload()\n");
        assert_eq!(kinds(&stmts), ["expr", "expr"]);
    }

    #[test]
    fn loops_and_conditionals_are_one_unsupported_statement() {
        let stmts = js(
            "if (a) { b(); } else { c(); }\nfor (const x of y) z(x);\nok();\n\
             try { a() } catch (e) { b() } finally { c() }\ndone();",
        );
        assert_eq!(
            kinds(&stmts),
            [
                "unsupported: an if statement",
                "unsupported: a for loop",
                "expr",
                "unsupported: a try statement",
                "expr"
            ]
        );
    }

    #[test]
    fn a_statement_that_cannot_be_read_is_kept_and_parsing_goes_on() {
        let stmts = js("a(;\nb();\n");
        assert_eq!(kinds(&stmts)[1], "expr");
        assert!(kinds(&stmts)[0].starts_with("unsupported: could not read"));
    }

    #[test]
    fn a_broken_statement_stops_at_the_end_of_its_block() {
        let stmts = js("test('t', async () => {\n  await page.goto(;\n  ok();\n});\nafter();");
        assert_eq!(kinds(&stmts), ["expr", "expr"]);
        let StmtKind::Expr(Expr::Call { args, .. }) = &stmts[0].kind else {
            panic!()
        };
        let Arg::Pos(Expr::Function(func)) = &args[1] else {
            panic!()
        };
        let FnBody::Block(body) = &func.body else {
            panic!()
        };
        assert!(kinds(body)[0].starts_with("unsupported: could not read"));
        assert_eq!(body[0].line, 2);
        assert_eq!(kinds(body)[1], "expr");
    }

    #[test]
    fn deep_nesting_is_refused_without_overflowing_the_stack() {
        let deep = format!("f({}1{});\nafter();", "(".repeat(5000), ")".repeat(5000));
        let stmts = js(&deep);
        assert!(
            kinds(&stmts)[0].contains("nested more than"),
            "{:?}",
            kinds(&stmts)
        );
        assert_eq!(kinds(&stmts)[1], "expr");
        let python = format!("f({}1{})\nafter()", "[".repeat(5000), "]".repeat(5000));
        let stmts = py(&python);
        assert!(kinds(&stmts)[0].contains("nested more than"));
        assert_eq!(kinds(&stmts)[1], "expr");
    }

    #[test]
    fn python_blocks_follow_indentation() {
        let stmts = py("import re\n\
             from playwright.sync_api import Page, expect\n\
             \n\
             @pytest.mark.skip(reason='x')\n\
             def test_one(page: Page) -> None:\n\
             \x20   # comment\n\
             \x20   page.goto(\"https://x.test/\")\n\
             \x20   assert page.title() == 'X'\n\
             \x20   for x in y:\n\
             \x20       z(x)\n\
             \x20   page.get_by_role(\"button\", name=\"Go\").click()\n\
             \n\
             with sync_playwright() as p:\n\
             \x20   run(p)\n\
             if __name__ == '__main__':\n\
             \x20   main()\n");
        assert_eq!(
            kinds(&stmts),
            ["ignored", "ignored", "def test_one", "with", "main"]
        );
        let StmtKind::FuncDef {
            func, decorators, ..
        } = &stmts[2].kind
        else {
            panic!()
        };
        assert_eq!(decorators.len(), 1);
        let FnBody::Block(body) = &func.body else {
            panic!()
        };
        assert_eq!(
            kinds(body),
            [
                "comment",
                "expr",
                "assert",
                "unsupported: a for loop",
                "expr"
            ]
        );
        let StmtKind::Expr(Expr::Call { callee, .. }) = &body[4].kind else {
            panic!()
        };
        let Expr::Member { object, .. } = callee.as_ref() else {
            panic!()
        };
        let Expr::Call { args, .. } = object.as_ref() else {
            panic!()
        };
        assert!(matches!(&args[1], Arg::Kw(name, Expr::Str(v)) if name == "name" && v == "Go"));
    }

    #[test]
    fn python_async_functions_and_with_blocks() {
        let stmts = py("async def main() -> None:\n\
             \x20   async with async_playwright() as playwright:\n\
             \x20       await run(playwright)\n\
             asyncio.run(main())\n");
        assert_eq!(kinds(&stmts), ["def main", "expr"]);
        let StmtKind::FuncDef { func, .. } = &stmts[0].kind else {
            panic!()
        };
        assert!(func.is_async);
        let FnBody::Block(body) = &func.body else {
            panic!()
        };
        assert_eq!(kinds(body), ["with"]);
    }

    #[test]
    fn function_names_are_collected_without_property_names() {
        let stmts = js("f(() => document.querySelector(sel).value + x.y);");
        let StmtKind::Expr(Expr::Call { args, .. }) = &stmts[0].kind else {
            panic!()
        };
        let Arg::Pos(Expr::Function(func)) = &args[0] else {
            panic!()
        };
        assert_eq!(func.names, ["document", "sel", "x"]);
    }

    #[test]
    fn object_literals_keep_keys_and_shorthand() {
        let stmts =
            js("f({ name: 'Go', exact: true, page, 'q-a': 1, ...rest, m() { return 1 } });");
        let StmtKind::Expr(Expr::Call { args, .. }) = &stmts[0].kind else {
            panic!()
        };
        let Arg::Pos(Expr::Object(entries)) = &args[0] else {
            panic!()
        };
        let keys: Vec<_> = entries.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["name", "exact", "page", "q-a", "...", "m"]);
    }
}
