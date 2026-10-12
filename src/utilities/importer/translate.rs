//! Statement translation, shared by the Playwright and Puppeteer front ends.
//!
//! A small interpreter over the parsed statements: each expression is
//! evaluated to what it stands for in the generated program (the page, a
//! locator, a value read from the page, an action already written as Rust, or
//! an `expect()` waiting for its matcher), and each statement becomes Rust
//! lines or a `TODO`. Text from the input reaches the Rust only through
//! [`rust_str`] and comments; identifiers only through [`var_ident`].

use std::collections::HashMap;

use super::emit::{float, one_line, rust_str, todo_source, var_ident, whole, Helper, Line, Needs};
use super::parser::{Arg, Expr, FnBody, Pattern, Stmt, StmtKind};
use super::selector::{self, Step};
use super::{ImportDiagnostic, ImportSeverity};
use crate::utils::selectors::{css_quoted, Selector};

/// Which tool's API the statements are written against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Flavor {
    Playwright,
    Puppeteer,
}

/// The Rust type of a value read from the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ty {
    /// `String`.
    Str,
    /// `Option<String>` (an attribute).
    OptStr,
    /// `bool`.
    Bool,
    /// `usize` (a count).
    Count,
    /// `serde_json::Value` (what `evaluate` returns).
    Json,
}

/// A literal value from the source.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Lit {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    Regex { pattern: String, flags: String },
}

/// Objects that are not the page: the launcher module, the browser, ...
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Obj {
    /// `puppeteer`, `chromium`, `playwright` and `sync_playwright()`.
    Launcher,
    Browser,
    Context,
    /// A `puppeteer-extra` plugin.
    Plugin,
    /// Any other module.
    Module,
    Console,
    Promise,
    /// Node's `assert`.
    Assert,
    /// Playwright's `test`.
    Test,
    /// `expect` used as an object (`expect.soft`).
    Expect,
    /// Python's `re`.
    Re,
    /// Python's `time`.
    Time,
    /// Python's `asyncio`.
    Asyncio,
}

/// What a name stands for.
#[derive(Clone, Debug)]
pub(super) enum Var {
    Page,
    Object(Obj),
    Locator {
        rust: String,
        list: bool,
    },
    Value {
        rust: String,
        ty: Ty,
    },
    Const(Lit),
    /// Declared without a value yet (`let page;`).
    Declared,
}

/// What an expression evaluates to.
#[derive(Clone, Debug)]
enum Ev {
    Page,
    Object(Obj),
    /// A locator, as Rust code.
    Locator(String),
    /// The elements `$$` found, as a locator.
    List(String),
    Keyboard,
    Mouse,
    /// A value read from the page: Rust code that yields it, and its type.
    Value(String, Ty),
    Lit(Lit),
    /// Something done: the lines that do it.
    Lines(Vec<Line>),
    /// A wait, then the locator waited for.
    Waited(Vec<Line>, String),
    Expect(Expectation),
    /// Something dropped on purpose (closing the browser, timeouts).
    Nothing,
}

#[derive(Clone, Debug)]
struct Expectation {
    subject: Subject,
    negate: bool,
}

#[derive(Clone, Debug)]
enum Subject {
    Page,
    Locator(String),
    List(String),
    Value(String, Ty),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Matcher {
    Eq,
    Contains,
    Match,
    Truthy,
    Null,
    Gt,
    Lt,
    Ge,
    Le,
    Len,
}

type TResult<T> = Result<T, String>;

/// Converts the call arguments into positional and keyword arguments.
#[derive(Clone)]
struct Args<'e> {
    pos: Vec<&'e Expr>,
    kw: Vec<(String, &'e Expr)>,
}

impl<'e> Args<'e> {
    fn new(args: &'e [Arg]) -> TResult<Self> {
        let mut out = Self {
            pos: Vec::new(),
            kw: Vec::new(),
        };
        for arg in args {
            match arg {
                Arg::Pos(expr) => out.pos.push(expr),
                Arg::Kw(name, expr) => out.kw.push((camel(name), expr)),
                Arg::Spread => return Err("a spread argument is not converted".to_owned()),
            }
        }
        Ok(out)
    }

    fn get(&self, index: usize) -> Option<&'e Expr> {
        self.pos.get(index).copied()
    }

    fn need(&self, index: usize, what: &str) -> TResult<&'e Expr> {
        self.get(index)
            .ok_or_else(|| format!("{what} is missing, which is not converted"))
    }

    /// The arguments after the first `n` positional ones.
    fn skip(&self, n: usize) -> Self {
        Self {
            pos: self.pos.iter().skip(n).copied().collect(),
            kw: self.kw.clone(),
        }
    }

    /// The options: the keyword arguments and the object literal at
    /// position `at`, if there is one. More positional arguments are refused.
    fn options(&self, at: usize) -> TResult<Vec<(String, &'e Expr)>> {
        let mut options = self.kw.clone();
        if let Some(expr) = self.pos.get(at) {
            match expr {
                Expr::Object(entries) => {
                    for (key, value) in entries {
                        if key == "..." {
                            return Err("computed or spread options are not converted".to_owned());
                        }
                        options.push((key.clone(), value));
                    }
                }
                Expr::Undefined | Expr::Null => {}
                _ => {
                    return Err(
                        "options that are not an object literal are not converted".to_owned()
                    )
                }
            }
        }
        if self.pos.len() > at + 1 {
            return Err("more arguments than the importer reads".to_owned());
        }
        Ok(options)
    }
}

fn option<'e>(options: &[(String, &'e Expr)], key: &str) -> Option<&'e Expr> {
    options.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
}

fn check_options(method: &str, options: &[(String, &Expr)], allowed: &[&str]) -> TResult<()> {
    match options
        .iter()
        .find(|(key, _)| !allowed.contains(&key.as_str()))
    {
        Some((key, _)) => Err(format!(
            "the `{key}` option of `{method}()` is not converted"
        )),
        None => Ok(()),
    }
}

/// A Python name in camel case: `get_by_role` is `getByRole`, and keyword
/// arguments such as `has_text` become `hasText`.
pub(super) fn camel(name: &str) -> String {
    if !name.contains('_') || name.starts_with('_') {
        return name.to_owned();
    }
    let mut out = String::with_capacity(name.len());
    let mut upper = false;
    for c in name.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn strip_await(expr: &Expr) -> &Expr {
    match expr {
        Expr::Await(inner) => strip_await(inner),
        other => other,
    }
}

/// The module a `require('...')` or `import('...')` loads.
pub(super) fn required_module(expr: &Expr) -> Option<&str> {
    match strip_await(expr) {
        Expr::Call { callee, args } => match (callee.as_ref(), args.as_slice()) {
            (Expr::Ident(name), [Arg::Pos(Expr::Str(module))])
                if name == "require" || name == "import" =>
            {
                Some(module.as_str())
            }
            _ => None,
        },
        _ => None,
    }
}

fn module_object(module: &str) -> Obj {
    match module {
        "puppeteer"
        | "puppeteer-core"
        | "puppeteer-extra"
        | "playwright"
        | "playwright-core"
        | "playwright-chromium"
        | "@playwright/test" => Obj::Launcher,
        "assert" | "node:assert" | "assert/strict" | "node:assert/strict" => Obj::Assert,
        m if m.starts_with("puppeteer-extra-plugin") => Obj::Plugin,
        _ => Obj::Module,
    }
}

/// The method name of a call on `page` (`page.waitForNavigation()`).
fn page_call_name(expr: &Expr) -> Option<String> {
    match strip_await(expr) {
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Member { property, .. } => Some(camel(property)),
            _ => None,
        },
        _ => None,
    }
}

/// A key name as a press of a named key, or as text to type.
enum KeyPress {
    Named(&'static str),
    Text(String),
}

fn key_press(key: &str) -> TResult<KeyPress> {
    let named = match key {
        "Enter" => "Enter",
        "Tab" => "Tab",
        "Escape" => "Escape",
        "Backspace" => "Backspace",
        "Delete" => "Delete",
        "ArrowUp" => "ArrowUp",
        "ArrowDown" => "ArrowDown",
        "ArrowLeft" => "ArrowLeft",
        "ArrowRight" => "ArrowRight",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "PageUp",
        "PageDown" => "PageDown",
        "Space" => return Ok(KeyPress::Text(" ".to_owned())),
        k if k.chars().count() == 1 => return Ok(KeyPress::Text(k.to_owned())),
        k if k.contains('+') => return Err(format!("the key combination {k:?} is not converted")),
        k => return Err(format!("the key {k:?} is not converted")),
    };
    Ok(KeyPress::Named(named))
}

/// A regular expression that is plain text, as that text and whether it is
/// anchored at both ends (`^...$`, an exact match) or not (a contains check).
pub(super) fn regex_text(pattern: &str, flags: &str) -> TResult<(String, bool)> {
    if flags.contains('i') {
        return Err("a case-insensitive regular expression is not converted".to_owned());
    }
    let mut body = pattern;
    let mut start = false;
    let mut end = false;
    if let Some(rest) = body.strip_prefix('^') {
        start = true;
        body = rest;
    }
    if body.ends_with('$') && !body.ends_with("\\$") {
        end = true;
        body = &body[..body.len() - 1];
    }
    if let Some(rest) = body.strip_prefix(".*") {
        start = false;
        body = rest;
    }
    if let Some(rest) = body.strip_suffix(".*") {
        if !rest.ends_with('\\') {
            end = false;
            body = rest;
        }
    }
    let mut text = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(next) if !next.is_ascii_alphanumeric() => text.push(next),
                _ => {
                    return Err(format!(
                        "the regular expression /{pattern}/ is not plain text"
                    ))
                }
            },
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' => {
                return Err(format!(
                    "the regular expression /{pattern}/ is not plain text"
                ))
            }
            c => text.push(c),
        }
    }
    if text.is_empty() {
        return Err(format!(
            "the regular expression /{pattern}/ matches anything"
        ));
    }
    Ok((text, start && end))
}

/// The translator of one file. It is reset for every test.
pub(super) struct Translator<'s> {
    flavor: Flavor,
    python: bool,
    source: &'s str,
    pub(super) needs: Needs,
    pub(super) diagnostics: Vec<ImportDiagnostic>,
    scope: HashMap<String, Var>,
    lines: Vec<Line>,
    page_bound: bool,
    /// The statement being translated: line, start and end.
    current: (usize, usize, usize),
}

impl<'s> Translator<'s> {
    pub(super) fn new(flavor: Flavor, python: bool, source: &'s str) -> Self {
        Self {
            flavor,
            python,
            source,
            needs: Needs::default(),
            diagnostics: Vec::new(),
            scope: HashMap::new(),
            lines: Vec::new(),
            page_bound: false,
            current: (1, 0, 0),
        }
    }

    /// Starts a test: nothing bound, no lines.
    pub(super) fn begin(&mut self) {
        self.scope.clear();
        self.lines.clear();
        self.page_bound = false;
    }

    /// Binds `name` to the test's page, as a fixture or a global does.
    pub(super) fn bind_page(&mut self, name: &str) {
        self.scope.insert(name.to_owned(), Var::Page);
        self.page_bound = true;
    }

    pub(super) fn bind(&mut self, name: &str, var: Var) {
        self.scope.insert(name.to_owned(), var);
    }

    /// Translates statements into the current test.
    pub(super) fn run(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.statement(stmt);
        }
    }

    /// The lines of the test, with variables that are never used renamed
    /// with a leading `_`.
    pub(super) fn finish(&mut self) -> Vec<Line> {
        let mut lines = std::mem::take(&mut self.lines);
        mark_unused(&mut lines);
        lines
    }

    /// Adds a line of Rust code to the current test.
    pub(super) fn push_code(&mut self, code: String) {
        self.lines.push(Line::Code(code));
    }

    /// A `TODO` for a statement outside any test, and its diagnostic.
    pub(super) fn file_todo(&mut self, stmt: &Stmt, reason: &str) -> Line {
        self.todo_line(stmt.line, stmt.start, stmt.end, reason)
    }

    /// A `TODO` for the span `start..end` at `line`, and its diagnostic.
    pub(super) fn todo_line(
        &mut self,
        line: usize,
        start: usize,
        end: usize,
        reason: &str,
    ) -> Line {
        let text = self.source.get(start..end.max(start)).unwrap_or_default();
        let severity = if reason.starts_with("could not read") {
            ImportSeverity::Error
        } else {
            ImportSeverity::Warning
        };
        let message = format!("{reason}: {}", one_line(text, 120));
        let diagnostic = ImportDiagnostic {
            line,
            severity,
            message,
        };
        if !self.diagnostics.contains(&diagnostic) {
            self.diagnostics.push(diagnostic);
        }
        Line::Todo {
            line,
            source: todo_source(text),
            reason: reason.to_owned(),
        }
    }

    /// Adds a `TODO` for the current statement to the test.
    pub(super) fn todo(&mut self, reason: &str) {
        let (line, start, end) = self.current;
        let todo = self.todo_line(line, start, end, reason);
        self.lines.push(todo);
    }

    /// Sets the statement that `TODO`s refer to.
    pub(super) fn at(&mut self, line: usize, start: usize, end: usize) {
        self.current = (line, start, end);
    }

    /// A locator on the page for `steps`, as Rust.
    pub(super) fn locator(&mut self, steps: &[Step]) -> TResult<String> {
        self.chain("page", steps)
    }

    /// A key press on the page's keyboard, as a line of Rust.
    pub(super) fn key(&mut self, key: &str) -> TResult<String> {
        match key_press(key)? {
            KeyPress::Named(key) => {
                self.needs.sb_cdp.insert("Key");
                Ok(format!("page.keyboard().press(Key::{key}).await?;"))
            }
            KeyPress::Text(text) => Ok(format!(
                "page.keyboard().type_text({}).await?;",
                rust_str(&text)
            )),
        }
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    fn statement(&mut self, stmt: &Stmt) {
        self.current = (stmt.line, stmt.start, stmt.end);
        let result = match &stmt.kind {
            StmtKind::Comment(text) => {
                self.lines.push(Line::Comment(text.clone()));
                Ok(())
            }
            StmtKind::Ignored => Ok(()),
            StmtKind::Expr(expr) => self.expression_statement(expr),
            StmtKind::Decl(declarators) => declarators.iter().try_for_each(|declarator| {
                match (&declarator.pattern, &declarator.init) {
                    (Pattern::Name(name), Some(init)) => self.declare(name, init),
                    (Pattern::Name(name), None) => {
                        self.scope.insert(name.clone(), Var::Declared);
                        Ok(())
                    }
                    (Pattern::Destructure(names), Some(init)) => self.destructure(names, init),
                    (Pattern::Destructure(_), None) => Ok(()),
                }
            }),
            StmtKind::Assign { target, value } => match target {
                Expr::Ident(name) => self.declare(name, value),
                _ => Err("assigning to a property or an element is not converted".to_owned()),
            },
            StmtKind::FuncDef { .. } => {
                Err("a function defined inside a test is not converted".to_owned())
            }
            StmtKind::With { items, body } => self.with_block(items, body),
            StmtKind::Assert(test) => self.python_assert(test),
            StmtKind::Return(_) => Err("a return statement is not converted".to_owned()),
            StmtKind::IfMain(body) => {
                self.run(body);
                Ok(())
            }
            StmtKind::Unsupported(what) => Err(if what.starts_with("could not read") {
                what.clone()
            } else {
                format!("{what} is not converted")
            }),
        };
        if let Err(reason) = result {
            self.current = (stmt.line, stmt.start, stmt.end);
            self.todo(&reason);
        }
    }

    fn expression_statement(&mut self, expr: &Expr) -> TResult<()> {
        match self.eval(expr)? {
            Ev::Lines(lines) | Ev::Waited(lines, _) => self.lines.extend(lines),
            Ev::Value(code, _) => self.push_code(format!("{code};")),
            Ev::Expect(_) => {
                return Err("an expect() without a matcher is not converted".to_owned())
            }
            Ev::Page
            | Ev::Object(_)
            | Ev::Locator(_)
            | Ev::List(_)
            | Ev::Keyboard
            | Ev::Mouse
            | Ev::Lit(_)
            | Ev::Nothing => {}
        }
        Ok(())
    }

    fn declare(&mut self, name: &str, init: &Expr) -> TResult<()> {
        if let Some(module) = required_module(init) {
            self.scope
                .insert(name.to_owned(), Var::Object(module_object(module)));
            return Ok(());
        }
        let rust = var_ident(name);
        let var = match self.eval(init)? {
            Ev::Page => Var::Page,
            Ev::Object(obj) => Var::Object(obj),
            Ev::Locator(code) => {
                self.push_code(format!("let {rust} = {code};"));
                Var::Locator { rust, list: false }
            }
            Ev::List(code) => {
                self.push_code(format!("let {rust} = {code};"));
                Var::Locator { rust, list: true }
            }
            Ev::Waited(lines, code) => {
                self.lines.extend(lines);
                self.push_code(format!("let {rust} = {code};"));
                Var::Locator { rust, list: false }
            }
            Ev::Value(code, ty) => {
                self.push_code(format!("let {rust} = {code};"));
                Var::Value { rust, ty }
            }
            Ev::Lit(lit) => Var::Const(lit),
            Ev::Lines(lines) => {
                self.lines.extend(lines);
                Var::Declared
            }
            Ev::Nothing => Var::Object(Obj::Module),
            Ev::Keyboard | Ev::Mouse | Ev::Expect(_) => {
                return Err("keeping this in a variable is not converted".to_owned())
            }
        };
        self.scope.insert(name.to_owned(), var);
        Ok(())
    }

    fn destructure(&mut self, names: &[String], init: &Expr) -> TResult<()> {
        let Some(module) = required_module(init) else {
            return Err("destructuring a value is not converted".to_owned());
        };
        for name in names {
            match (module_object(module), name.as_str()) {
                // `test`, `expect`, `describe` and friends are known by name.
                (
                    _,
                    "test" | "expect" | "describe" | "it" | "beforeEach" | "afterEach"
                    | "beforeAll" | "afterAll" | "devices",
                ) => {}
                (Obj::Launcher, _) => {
                    self.scope.insert(name.clone(), Var::Object(Obj::Launcher));
                }
                (Obj::Assert, _) | (_, "assert") => {
                    self.scope.insert(name.clone(), Var::Object(Obj::Assert));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn with_block(&mut self, items: &[(Expr, Option<String>)], body: &[Stmt]) -> TResult<()> {
        let mut wait_after = false;
        for (item, name) in items {
            match page_call_name(item).as_deref() {
                Some("expectNavigation") => {
                    wait_after = true;
                    continue;
                }
                Some(
                    method @ ("expectPopup"
                    | "expectDownload"
                    | "expectResponse"
                    | "expectRequest"
                    | "expectFileChooser"
                    | "expectConsoleMessage"
                    | "expectEvent"
                    | "expectWebsocket"
                    | "expectWorker"),
                ) => {
                    return Err(format!(
                        "`with page.{method}()` (waiting for an event) is not converted"
                    ))
                }
                _ => {}
            }
            let var = match self.eval(item)? {
                Ev::Object(obj) => Var::Object(obj),
                Ev::Page => Var::Page,
                Ev::Nothing => Var::Object(Obj::Module),
                _ => return Err("this `with` block is not converted".to_owned()),
            };
            if let Some(name) = name {
                self.scope.insert(name.clone(), var);
            }
        }
        self.run(body);
        if wait_after {
            self.push_code("page.wait_for_load().await?;".to_owned());
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn eval(&mut self, expr: &Expr) -> TResult<Ev> {
        match expr {
            Expr::Await(inner) => self.eval(inner),
            Expr::Ident(name) => self.ident(name),
            Expr::Str(text) => Ok(Ev::Lit(Lit::Str(text.clone()))),
            Expr::Num(n) => Ok(Ev::Lit(Lit::Num(*n))),
            Expr::Bool(b) => Ok(Ev::Lit(Lit::Bool(*b))),
            Expr::Null | Expr::Undefined => Ok(Ev::Lit(Lit::Null)),
            Expr::Regex { pattern, flags } => Ok(Ev::Lit(Lit::Regex {
                pattern: pattern.clone(),
                flags: flags.clone(),
            })),
            Expr::Interpolated => {
                Err("a template literal or f-string with placeholders is not converted".to_owned())
            }
            Expr::Member { object, property } => {
                let base = self.eval(object)?;
                self.prop(base, property)
            }
            Expr::Call { callee, args } => match callee.as_ref() {
                Expr::Member { object, property } => {
                    let base = self.eval(object)?;
                    self.method(base, &camel(property), args)
                }
                Expr::Ident(name) => self.function(name, args),
                _ => Err("calling the result of an expression is not converted".to_owned()),
            },
            Expr::New { callee, args } => self.new_object(callee, args),
            Expr::Unary {
                op: "!" | "not",
                operand,
            } => match self.eval(operand)? {
                Ev::Value(code, Ty::Bool) => Ok(Ev::Value(format!("!({code})"), Ty::Bool)),
                Ev::Lit(Lit::Bool(b)) => Ok(Ev::Lit(Lit::Bool(!b))),
                _ => Err("negating this value is not converted".to_owned()),
            },
            Expr::Unary { op: "-", operand } => match operand.as_ref() {
                Expr::Num(n) => Ok(Ev::Lit(Lit::Num(-n))),
                _ => Err("this expression is not converted".to_owned()),
            },
            Expr::Binary { op: "+", .. } => self.concat(expr),
            Expr::Index { object, index } => {
                let base = self.eval(object)?;
                let n = self.number(index)?;
                match (base, whole(n)) {
                    (Ev::List(code), Some(n)) => Ok(Ev::Locator(format!("{code}.nth({n})"))),
                    _ => Err("indexing this value is not converted".to_owned()),
                }
            }
            Expr::Function(_) => Err("a function value here is not converted".to_owned()),
            Expr::Array(_) | Expr::Object(_) => {
                Err("an array or object value here is not converted".to_owned())
            }
            Expr::Unary { .. } | Expr::Binary { .. } | Expr::Opaque => {
                Err("this expression is not converted".to_owned())
            }
        }
    }

    fn ident(&mut self, name: &str) -> TResult<Ev> {
        if let Some(var) = self.scope.get(name) {
            return match var.clone() {
                Var::Page => Ok(Ev::Page),
                Var::Object(obj) => Ok(Ev::Object(obj)),
                Var::Locator { rust, list } => Ok(if list {
                    Ev::List(rust)
                } else {
                    Ev::Locator(rust)
                }),
                Var::Value { rust, ty } => Ok(Ev::Value(rust, ty)),
                Var::Const(lit) => Ok(Ev::Lit(lit)),
                Var::Declared => Err(format!(
                    "`{name}` is used before the importer knows its value"
                )),
            };
        }
        let obj = match name {
            "puppeteer" | "chromium" | "firefox" | "webkit" | "playwright" => Obj::Launcher,
            "console" => Obj::Console,
            "Promise" => Obj::Promise,
            "assert" => Obj::Assert,
            "test" => Obj::Test,
            "expect" => Obj::Expect,
            "re" if self.python => Obj::Re,
            "time" if self.python => Obj::Time,
            "asyncio" if self.python => Obj::Asyncio,
            "page" => return Err("`page` is used before a page was opened".to_owned()),
            _ => {
                return Err(format!(
                    "`{name}` is not something the importer knows (a helper, a fixture or a variable it could not read)"
                ))
            }
        };
        Ok(Ev::Object(obj))
    }

    /// `a + b + ...` of text: a literal when every part is one, else `format!`.
    fn concat(&mut self, expr: &Expr) -> TResult<Ev> {
        let mut parts = Vec::new();
        let mut stack = vec![expr];
        // Flatten left to right without recursion.
        let mut flat = Vec::new();
        while let Some(e) = stack.pop() {
            match e {
                Expr::Binary {
                    op: "+",
                    left,
                    right,
                } => {
                    stack.push(right);
                    stack.push(left);
                }
                other => flat.push(other),
            }
        }
        let mut all_literal = true;
        let mut text = String::new();
        for part in flat {
            match self.eval(part)? {
                Ev::Lit(Lit::Str(s)) => {
                    text.push_str(&s);
                    parts.push(rust_str(&s));
                }
                Ev::Lit(Lit::Num(n)) => {
                    let s = super::parser::format_number(n);
                    text.push_str(&s);
                    parts.push(rust_str(&s));
                }
                Ev::Value(code, Ty::Str | Ty::Count | Ty::Bool | Ty::Json) => {
                    all_literal = false;
                    parts.push(code);
                }
                _ => return Err("this `+` is not converted".to_owned()),
            }
        }
        if all_literal {
            return Ok(Ev::Lit(Lit::Str(text)));
        }
        let format = "{}".repeat(parts.len());
        Ok(Ev::Value(
            format!("format!(\"{format}\", {})", parts.join(", ")),
            Ty::Str,
        ))
    }

    /// Text where a `&str` is expected, as Rust.
    fn text(&mut self, expr: &Expr) -> TResult<String> {
        match self.eval(expr)? {
            Ev::Lit(Lit::Str(s)) => Ok(rust_str(&s)),
            Ev::Lit(Lit::Num(n)) => Ok(rust_str(&super::parser::format_number(n))),
            Ev::Value(code, Ty::Str) => Ok(format!("&{code}")),
            _ => {
                Err("a value that is not text, where text is expected, is not converted".to_owned())
            }
        }
    }

    /// Text that must be known now (a selector, a role, a key).
    fn literal_text(&mut self, expr: &Expr, what: &str) -> TResult<String> {
        match self.eval(expr)? {
            Ev::Lit(Lit::Str(s)) => Ok(s),
            Ev::Lit(Lit::Regex { .. }) => Err(format!(
                "a regular expression as the {what} is not converted"
            )),
            _ => Err(format!(
                "a {what} that is not a literal string is not converted"
            )),
        }
    }

    fn number(&mut self, expr: &Expr) -> TResult<f64> {
        match self.eval(expr)? {
            Ev::Lit(Lit::Num(n)) if n.is_finite() => Ok(n),
            _ => Err("a value that is not a literal number is not converted".to_owned()),
        }
    }

    fn whole_number(&mut self, expr: &Expr, what: &str) -> TResult<u64> {
        let n = self.number(expr)?;
        whole(n).ok_or_else(|| format!("{what} {n} is not a whole number the importer converts"))
    }

    fn boolean(&mut self, expr: &Expr) -> TResult<bool> {
        match self.eval(expr)? {
            Ev::Lit(Lit::Bool(b)) => Ok(b),
            _ => Err("a value that is not a literal true or false is not converted".to_owned()),
        }
    }

    fn prop(&mut self, base: Ev, name: &str) -> TResult<Ev> {
        match base {
            Ev::Page => match name {
                "keyboard" => Ok(Ev::Keyboard),
                "mouse" => Ok(Ev::Mouse),
                "url" => Ok(Ev::Value("page.url().await?".to_owned(), Ty::Str)),
                _ => Err(format!("`page.{name}` is not converted")),
            },
            // Python's `locator.first`, `.last` and `.content_frame` are properties.
            Ev::Locator(code) => match name {
                "first" => Ok(Ev::Locator(format!("{code}.first()"))),
                "last" => Ok(Ev::Locator(format!("{code}.last()"))),
                "content_frame" | "contentFrame" => Ok(Ev::Locator(code)),
                _ => Err(format!("`.{name}` on a locator is not converted")),
            },
            Ev::List(code) if name == "length" => {
                Ok(Ev::Value(format!("{code}.count().await?"), Ty::Count))
            }
            Ev::Value(code, Ty::Str) if name == "length" => {
                Ok(Ev::Value(format!("{code}.chars().count()"), Ty::Count))
            }
            Ev::Expect(mut expectation) => match name {
                "not" => {
                    expectation.negate = !expectation.negate;
                    Ok(Ev::Expect(expectation))
                }
                "to" | "be" | "been" | "is" | "that" | "which" | "and" | "has" | "have"
                | "with" | "at" | "of" | "same" | "but" | "does" | "still" | "deep" | "own" => {
                    Ok(Ev::Expect(expectation))
                }
                "true" | "false" | "ok" | "null" | "undefined" | "exist" | "empty" => {
                    self.chai_terminal(expectation, name)
                }
                _ => Err(format!("`expect(...).{name}` is not converted")),
            },
            Ev::Object(Obj::Launcher) if matches!(name, "chromium" | "firefox" | "webkit") => {
                Ok(Ev::Object(Obj::Launcher))
            }
            _ => Err(format!("`.{name}` here is not converted")),
        }
    }

    fn method(&mut self, base: Ev, name: &str, args: &[Arg]) -> TResult<Ev> {
        match base {
            Ev::Page => self.page_method(name, args),
            Ev::Locator(code) => self.locator_method(&code, name, args),
            Ev::List(code) => match name {
                "count" => Ok(Ev::Value(format!("{code}.count().await?"), Ty::Count)),
                _ => Err(format!(
                    "`.{name}()` on a list of elements is not converted"
                )),
            },
            Ev::Keyboard => self.keyboard_method(name, args),
            Ev::Mouse => self.mouse_method(name, args),
            Ev::Object(obj) => self.object_method(obj, name, args),
            Ev::Expect(expectation) => self.expect_method(expectation, name, args),
            Ev::Value(code, ty) => self.value_method(&code, ty, name, args),
            _ => Err(format!("calling `.{name}()` here is not converted")),
        }
    }

    fn function(&mut self, name: &str, args: &[Arg]) -> TResult<Ev> {
        match name {
            "expect" => {
                let args = Args::new(args)?;
                let subject = self.subject(args.need(0, "the value to check")?)?;
                Ok(Ev::Expect(Expectation {
                    subject,
                    negate: false,
                }))
            }
            "assert" => {
                let args = Args::new(args)?;
                let subject = self.value_subject(args.need(0, "the value to check")?)?;
                self.assert_value(subject, Matcher::Truthy, None, false)
            }
            "print" if self.python => self.log(args),
            "require" => Ok(Ev::Object(Obj::Module)),
            "sync_playwright" | "async_playwright" => Ok(Ev::Object(Obj::Launcher)),
            "setTimeout" | "setInterval" => Err("timers are not converted".to_owned()),
            "describe" | "it" | "test" | "beforeEach" | "beforeAll" | "afterEach" | "afterAll"
            | "before" | "after" => {
                Err("a test or a hook inside a test is not converted".to_owned())
            }
            _ => Err(format!(
                "calling `{name}()` (a helper function) is not converted"
            )),
        }
    }

    fn new_object(&mut self, callee: &Expr, args: &[Arg]) -> TResult<Ev> {
        // `new Promise(r => setTimeout(r, 500))`: a sleep.
        if matches!(callee, Expr::Ident(name) if name == "Promise") {
            if let [Arg::Pos(Expr::Function(func))] = args {
                let resolve = func.params.first().and_then(|p| p.name.as_deref());
                let body = match &func.body {
                    FnBody::Expr(expr) => Some(expr.as_ref()),
                    FnBody::Block(stmts) => match stmts.as_slice() {
                        [Stmt {
                            kind: StmtKind::Expr(expr),
                            ..
                        }] => Some(expr),
                        _ => None,
                    },
                };
                if let (Some(resolve), Some(Expr::Call { callee, args })) = (resolve, body) {
                    if let (Expr::Ident(timer), [Arg::Pos(Expr::Ident(r)), Arg::Pos(delay)]) =
                        (callee.as_ref(), args.as_slice())
                    {
                        if timer == "setTimeout" && r == resolve {
                            let ms = self.number(delay)?;
                            return self.sleep_ms(ms);
                        }
                    }
                }
            }
        }
        Err("`new ...` is not converted".to_owned())
    }

    fn sleep_ms(&mut self, ms: f64) -> TResult<Ev> {
        if !(0.0..=1.0e9).contains(&ms) {
            return Err(format!("a wait of {ms} ms is not converted"));
        }
        let line = match whole(ms) {
            Some(n) => {
                format!("tokio::time::sleep(std::time::Duration::from_millis({n})).await;")
            }
            None => format!(
                "tokio::time::sleep(std::time::Duration::from_secs_f64({})).await;",
                float(ms / 1000.0).unwrap_or_else(|| "0.0".to_owned())
            ),
        };
        Ok(code(line))
    }

    fn value_method(&mut self, code: &str, ty: Ty, name: &str, args: &[Arg]) -> TResult<Ev> {
        let args = Args::new(args)?;
        match (ty, name) {
            (Ty::Str, "trim" | "strip") => {
                Ok(Ev::Value(format!("{code}.trim().to_owned()"), Ty::Str))
            }
            (Ty::Str, "toLowerCase" | "lower") => {
                Ok(Ev::Value(format!("{code}.to_lowercase()"), Ty::Str))
            }
            (Ty::Str, "toUpperCase" | "upper") => {
                Ok(Ev::Value(format!("{code}.to_uppercase()"), Ty::Str))
            }
            (Ty::Str, "includes" | "startsWith" | "endsWith" | "startswith" | "endswith") => {
                let needle = self.text(args.need(0, "the text")?)?;
                let method = match name {
                    "includes" => "contains",
                    "startsWith" | "startswith" => "starts_with",
                    _ => "ends_with",
                };
                Ok(Ev::Value(format!("{code}.{method}({needle})"), Ty::Bool))
            }
            _ => Err(format!("`.{name}()` on a value is not converted")),
        }
    }

    // ------------------------------------------------------------------
    // Objects other than the page
    // ------------------------------------------------------------------

    fn object_method(&mut self, obj: Obj, name: &str, args: &[Arg]) -> TResult<Ev> {
        match (obj, name) {
            (Obj::Launcher, "launch") => Ok(Ev::Object(Obj::Browser)),
            (Obj::Launcher, "launchPersistentContext") => Ok(Ev::Object(Obj::Context)),
            (Obj::Launcher, "use") => Err(
                "puppeteer-extra plugins (`puppeteer.use()`) are not converted; the pure CDP \
                 engine has its own stealth options (`LaunchOptions`, `Identity`)"
                    .to_owned(),
            ),
            (Obj::Launcher, "connect" | "connectOverCDP" | "connectOverCdp") => {
                Err("connecting to a running browser is not converted".to_owned())
            }
            (Obj::Browser | Obj::Context, "newPage") => self.new_page(),
            (
                Obj::Browser,
                "newContext" | "createBrowserContext" | "createIncognitoBrowserContext",
            ) => {
                let options = Args::new(args)?.options(0)?;
                if !options.is_empty() {
                    let reason = format!(
                        "the options of `{name}()` are not carried over; set them on the page instead"
                    );
                    self.todo(&reason);
                }
                Ok(Ev::Object(Obj::Context))
            }
            (Obj::Browser | Obj::Context, "close") => Ok(Ev::Nothing),
            (Obj::Context, "addCookies") => {
                let args = Args::new(args)?;
                match args.need(0, "the cookies")? {
                    Expr::Array(items) => self.cookies(items),
                    _ => Err("cookies that are not an array literal are not converted".to_owned()),
                }
            }
            (Obj::Context, "clearCookies") if self.page_bound => {
                Ok(code("page.cookies().clear().await?;".to_owned()))
            }
            (Obj::Browser, "pages") => {
                Err("`browser.pages()` (several tabs) is not converted".to_owned())
            }
            (Obj::Browser, _) => Err(format!("`browser.{name}()` is not converted")),
            (Obj::Context, _) => Err(format!("`context.{name}()` is not converted")),
            (Obj::Console, "log" | "info" | "warn" | "error" | "debug") => self.log(args),
            (Obj::Promise, "all" | "allSettled") => self.promise_all(args),
            (Obj::Assert, _) => self.node_assert(name, args),
            (Obj::Expect, "soft") => self.function("expect", args),
            (Obj::Test, "step") => self.test_step(args),
            (Obj::Test, "slow" | "setTimeout") => Ok(Ev::Nothing),
            (Obj::Re, "compile") => {
                let args = Args::new(args)?;
                let pattern = self.literal_text(args.need(0, "the pattern")?, "pattern")?;
                let flags = match args.get(1) {
                    None => String::new(),
                    Some(Expr::Member { object, property })
                        if matches!(object.as_ref(), Expr::Ident(re) if re == "re")
                            && matches!(property.as_str(), "I" | "IGNORECASE") =>
                    {
                        "i".to_owned()
                    }
                    Some(_) => {
                        return Err("these regular expression flags are not converted".to_owned())
                    }
                };
                Ok(Ev::Lit(Lit::Regex { pattern, flags }))
            }
            (Obj::Time | Obj::Asyncio, "sleep") => {
                let args = Args::new(args)?;
                let seconds = self.number(args.need(0, "the seconds")?)?;
                self.sleep_ms(seconds * 1000.0)
            }
            (Obj::Plugin | Obj::Module, _) => {
                Err(format!("calling `{name}()` on a module is not converted"))
            }
            _ => Err(format!("`.{name}()` here is not converted")),
        }
    }

    fn new_page(&mut self) -> TResult<Ev> {
        if self.page_bound {
            return Err("a second page (popups, several tabs) is not converted".to_owned());
        }
        self.page_bound = true;
        Ok(Ev::Page)
    }

    fn cookies(&mut self, items: &[Expr]) -> TResult<Ev> {
        if !self.page_bound {
            return Err("setting cookies before a page is open is not converted".to_owned());
        }
        let mut built = Vec::new();
        for item in items {
            let Expr::Object(entries) = item else {
                return Err("a cookie that is not an object literal is not converted".to_owned());
            };
            let field = |key: &str| entries.iter().find(|(k, _)| k == key).map(|(_, v)| v);
            let text = |this: &mut Self, key: &str| -> TResult<Option<String>> {
                field(key)
                    .map(|value| this.literal_text(value, "cookie field"))
                    .transpose()
            };
            for (key, _) in entries {
                if !matches!(
                    key.as_str(),
                    "name"
                        | "value"
                        | "domain"
                        | "path"
                        | "url"
                        | "secure"
                        | "httpOnly"
                        | "expires"
                ) {
                    return Err(format!("the cookie field `{key}` is not converted"));
                }
            }
            let name = text(self, "name")?.ok_or("a cookie without a name is not converted")?;
            let value = text(self, "value")?.ok_or("a cookie without a value is not converted")?;
            let domain = match (text(self, "domain")?, text(self, "url")?) {
                (Some(domain), _) => domain,
                (None, Some(url)) => url
                    .split("://")
                    .nth(1)
                    .unwrap_or(&url)
                    .split(['/', ':', '?', '#'])
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
                (None, None) => {
                    return Err("a cookie without a domain or url is not converted".to_owned())
                }
            };
            let mut cookie = format!(
                "Cookie::new({}, {}).domain({})",
                rust_str(&name),
                rust_str(&value),
                rust_str(&domain)
            );
            if let Some(path) = text(self, "path")? {
                cookie.push_str(&format!(".path({})", rust_str(&path)));
            }
            if let Some(secure) = field("secure") {
                let secure = self.boolean(secure)?;
                cookie.push_str(&format!(".secure({secure})"));
            }
            if let Some(http_only) = field("httpOnly") {
                let http_only = self.boolean(http_only)?;
                cookie.push_str(&format!(".http_only({http_only})"));
            }
            if let Some(expires) = field("expires") {
                let expires = self.number(expires)?;
                let expires = float(expires).ok_or("an expiry that is not a number")?;
                cookie.push_str(&format!(".expires({expires})"));
            }
            built.push(cookie);
        }
        self.needs.sb_cdp.insert("Cookie");
        Ok(code(format!(
            "page.cookies().set(&[{}]).await?;",
            built.join(", ")
        )))
    }

    fn log(&mut self, args: &[Arg]) -> TResult<Ev> {
        let args = Args::new(args)?;
        if !args.kw.is_empty() {
            return Err("print() options are not converted".to_owned());
        }
        let mut format = Vec::new();
        let mut values = Vec::new();
        for expr in &args.pos {
            match self.eval(expr)? {
                Ev::Lit(Lit::Str(s)) => {
                    format.push("{}");
                    values.push(rust_str(&s));
                }
                Ev::Lit(Lit::Num(n)) => {
                    format.push("{}");
                    values.push(float(n).ok_or("a number that is not finite")?);
                }
                Ev::Lit(Lit::Bool(b)) => {
                    format.push("{}");
                    values.push(b.to_string());
                }
                Ev::Value(code, Ty::OptStr) => {
                    format.push("{:?}");
                    values.push(code);
                }
                Ev::Value(code, _) => {
                    format.push("{}");
                    values.push(code);
                }
                _ => return Err("logging this value is not converted".to_owned()),
            }
        }
        if values.is_empty() {
            return Ok(code("println!();".to_owned()));
        }
        Ok(code(format!(
            "println!(\"{}\", {});",
            format.join(" "),
            values.join(", ")
        )))
    }

    fn promise_all(&mut self, args: &[Arg]) -> TResult<Ev> {
        let args = Args::new(args)?;
        let Some(Expr::Array(items)) = args.get(0) else {
            return Err(
                "Promise.all() of something other than an array literal is not converted"
                    .to_owned(),
            );
        };
        let mut now = Vec::new();
        let mut after = Vec::new();
        for item in items {
            let waits = matches!(
                page_call_name(item).as_deref(),
                Some(
                    "waitForNavigation"
                        | "waitForLoadState"
                        | "waitForURL"
                        | "waitForUrl"
                        | "waitForNetworkIdle"
                )
            );
            let lines = match self.eval(item)? {
                Ev::Lines(lines) | Ev::Waited(lines, _) => lines,
                Ev::Value(code, _) => vec![Line::Code(format!("{code};"))],
                Ev::Nothing => Vec::new(),
                _ => return Err("this Promise.all() is not converted".to_owned()),
            };
            // A wait for the navigation an action causes goes after the action.
            if waits {
                after.extend(lines);
            } else {
                now.extend(lines);
            }
        }
        now.extend(after);
        Ok(Ev::Lines(now))
    }

    fn test_step(&mut self, args: &[Arg]) -> TResult<Ev> {
        let args = Args::new(args)?;
        let title = self.literal_text(args.need(0, "the step title")?, "step title")?;
        let Some(Expr::Function(func)) = args.get(1) else {
            return Err("test.step() without a function is not converted".to_owned());
        };
        let saved = std::mem::take(&mut self.lines);
        let saved_current = self.current;
        match &func.body {
            FnBody::Block(body) => self.run(body),
            FnBody::Expr(expr) => {
                if let Err(reason) = self.expression_statement(expr) {
                    self.todo(&reason);
                }
            }
        }
        self.current = saved_current;
        let inner = std::mem::replace(&mut self.lines, saved);
        let mut lines = vec![Line::Comment(format!("step: {title}"))];
        lines.extend(inner);
        Ok(Ev::Lines(lines))
    }

    // ------------------------------------------------------------------
    // The page
    // ------------------------------------------------------------------

    fn page_method(&mut self, name: &str, raw: &[Arg]) -> TResult<Ev> {
        let args = Args::new(raw)?;
        if let Some((steps, list)) = self.locator_steps(name, &args)? {
            let code = self.chain("page", &steps)?;
            return Ok(if list {
                Ev::List(code)
            } else {
                Ev::Locator(code)
            });
        }
        const SELECTOR_ACTIONS: &[&str] = &[
            "click",
            "dblclick",
            "fill",
            "type",
            "press",
            "check",
            "uncheck",
            "setChecked",
            "selectOption",
            "select",
            "hover",
            "focus",
            "tap",
            "textContent",
            "innerText",
            "inputValue",
            "getAttribute",
            "isVisible",
            "isHidden",
            "isChecked",
            "isEnabled",
            "isDisabled",
            "setInputFiles",
            "$eval",
            "$$eval",
        ];
        if SELECTOR_ACTIONS.contains(&name) {
            if name == "$eval" || name == "$$eval" {
                return self.locator_method("page", name, raw);
            }
            let steps = self.selector(args.need(0, "the selector")?)?;
            let locator = self.chain("page", &steps)?;
            return self.locator_action(&locator, name, &args.skip(1));
        }
        match name {
            "goto" => {
                let url = self.text(args.need(0, "the URL")?)?;
                let options = args.options(1)?;
                check_options(name, &options, &["waitUntil", "timeout", "referer"])?;
                Ok(code(format!("page.goto({url}).await?;")))
            }
            "reload" => self.page_plain(name, &args, "page.reload().await?;"),
            "goBack" => self.page_plain(name, &args, "page.back().await?;"),
            "goForward" => self.page_plain(name, &args, "page.forward().await?;"),
            "waitForURL" | "waitForUrl" => {
                let options = args.options(1)?;
                check_options(name, &options, &["timeout", "waitUntil"])?;
                self.wait_for_url(args.need(0, "the URL")?)
            }
            "waitForLoadState" => {
                if let Some(state) = args.get(0) {
                    let state = self.literal_text(state, "load state")?;
                    if !matches!(state.as_str(), "load" | "domcontentloaded" | "networkidle") {
                        return Err(format!("the load state {state:?} is not converted"));
                    }
                }
                Ok(code("page.wait_for_load().await?;".to_owned()))
            }
            "waitForNavigation" | "waitForNetworkIdle" => {
                Ok(code("page.wait_for_load().await?;".to_owned()))
            }
            "waitForTimeout" => {
                let ms = self.number(args.need(0, "the time")?)?;
                self.sleep_ms(ms)
            }
            "waitForSelector" | "waitForXPath" => self.wait_for_selector(name, &args),
            "waitForFunction" => {
                let script = self.script(args.need(0, "the function")?)?;
                if args.pos.len() > 1 || !args.kw.is_empty() {
                    return Err(
                        "waitForFunction() with arguments or options is not converted".to_owned(),
                    );
                }
                Ok(code(format!("page.wait_for_function({script}).await?;")))
            }
            "setViewportSize" | "setViewport" => {
                let options = if args.pos.is_empty() {
                    args.kw.clone()
                } else {
                    args.options(0)?
                };
                check_options(
                    name,
                    &options,
                    &[
                        "width",
                        "height",
                        "deviceScaleFactor",
                        "isMobile",
                        "hasTouch",
                        "isLandscape",
                    ],
                )?;
                for (key, expected) in [
                    ("deviceScaleFactor", Lit::Num(1.0)),
                    ("isMobile", Lit::Bool(false)),
                    ("hasTouch", Lit::Bool(false)),
                    ("isLandscape", Lit::Bool(false)),
                ] {
                    if let Some(value) = option(&options, key) {
                        match self.eval(value)? {
                            Ev::Lit(lit) if lit == expected => {}
                            _ => {
                                return Err(format!(
                                    "the `{key}` option of `{name}()` is not converted"
                                ))
                            }
                        }
                    }
                }
                let width = option(&options, "width").ok_or("a viewport without a width")?;
                let height = option(&options, "height").ok_or("a viewport without a height")?;
                let width = self.whole_number(width, "the width")?;
                let height = self.whole_number(height, "the height")?;
                Ok(code(format!(
                    "page.window().set_bounds(0, 0, {width}, {height}).await?;"
                )))
            }
            "setUserAgent" => {
                let agent = self.text(args.need(0, "the user agent")?)?;
                if args.pos.len() > 1 {
                    return Err("setUserAgent() with client hints is not converted".to_owned());
                }
                Ok(code(format!(
                    "page.emulation().user_agent({agent}, None).await?;"
                )))
            }
            "setDefaultTimeout" | "setDefaultNavigationTimeout" => {
                let ms = self.whole_number(args.need(0, "the timeout")?, "the timeout")?;
                Ok(code(format!(
                    "let page = page.with_timeout(std::time::Duration::from_millis({ms}));"
                )))
            }
            "screenshot" => self.screenshot("page", &args),
            "pdf" => {
                let options = args.options(0)?;
                check_options(name, &options, &["path"])?;
                match option(&options, "path") {
                    Some(path) => {
                        let path = self.text(path)?;
                        Ok(code(format!("std::fs::write({path}, page.pdf().await?)?;")))
                    }
                    None => Ok(code("page.pdf().await?;".to_owned())),
                }
            }
            "title" => Ok(Ev::Value("page.title().await?".to_owned(), Ty::Str)),
            "url" => Ok(Ev::Value("page.url().await?".to_owned(), Ty::Str)),
            "content" => Ok(Ev::Value("page.content().await?".to_owned(), Ty::Str)),
            "evaluate" => {
                if args.pos.len() > 1 || !args.kw.is_empty() {
                    return Err("evaluate() with arguments is not converted".to_owned());
                }
                let script = self.script(args.need(0, "the function")?)?;
                Ok(Ev::Value(
                    format!("page.evaluate({script}).await?"),
                    Ty::Json,
                ))
            }
            "bringToFront" => Ok(code("page.bring_to_front().await?;".to_owned())),
            "close" => Ok(Ev::Nothing),
            "setCookie" => {
                let items: Vec<Expr> = args.pos.iter().map(|e| (*e).clone()).collect();
                self.cookies(&items)
            }
            "dragAndDrop" => {
                let from = self.selector(args.need(0, "the source")?)?;
                let to = self.selector(args.need(1, "the target")?)?;
                let from = self.chain("page", &from)?;
                let to = self.chain("page", &to)?;
                Ok(code(format!("{from}.drag_to(&{to}).await?;")))
            }
            "route" | "unroute" | "routeFromHAR" | "routeFromHar" | "setRequestInterception" => {
                Err(format!(
                    "request interception (`{name}()`) is not converted"
                ))
            }
            "on"
            | "once"
            | "off"
            | "addListener"
            | "removeListener"
            | "waitForEvent"
            | "waitForRequest"
            | "waitForResponse"
            | "waitForFileChooser"
            | "waitForDevicePrompt" => Err(format!(
                "event handlers and waits for events (`{name}()`) are not converted"
            )),
            "frames" | "mainFrame" | "frame" | "childFrames" => {
                Err("frames are not converted beyond frameLocator(css).locator(css)".to_owned())
            }
            "createCDPSession" | "createCdpSession" | "target" => {
                Err("CDP sessions are not converted".to_owned())
            }
            "cookies" | "deleteCookie" => {
                Err(format!("`page.{name}()` (cookies) is not converted"))
            }
            "pause" => Err(
                "page.pause() opens the Playwright inspector; there is nothing to convert it to"
                    .to_owned(),
            ),
            _ => Err(format!("`page.{name}()` is not converted")),
        }
    }

    fn page_plain(&mut self, name: &str, args: &Args<'_>, line: &str) -> TResult<Ev> {
        let options = args.options(0)?;
        check_options(name, &options, &["waitUntil", "timeout"])?;
        Ok(code(line.to_owned()))
    }

    /// `waitForURL`: a string is a glob (`**` at the ends becomes a contains
    /// check), a regular expression must be plain text.
    fn wait_for_url(&mut self, expected: &Expr) -> TResult<Ev> {
        match self.eval(expected)? {
            Ev::Lit(Lit::Str(glob)) => {
                let mut text = glob.as_str();
                let mut partial = false;
                if let Some(rest) = text.strip_prefix("**") {
                    text = rest;
                    partial = true;
                }
                if let Some(rest) = text.strip_suffix("**").or_else(|| text.strip_suffix('*')) {
                    text = rest;
                    partial = true;
                }
                if text.contains('*') || text.is_empty() {
                    return Err(format!("the URL pattern {glob:?} is not converted"));
                }
                let method = if partial {
                    "to_contain_url"
                } else {
                    "to_have_url"
                };
                Ok(code(format!(
                    "page.expect().{method}({}).await?;",
                    rust_str(text)
                )))
            }
            Ev::Lit(Lit::Regex { pattern, flags }) => {
                let (text, exact) = regex_text(&pattern, &flags)?;
                let method = if exact {
                    "to_have_url"
                } else {
                    "to_contain_url"
                };
                Ok(code(format!(
                    "page.expect().{method}({}).await?;",
                    rust_str(&text)
                )))
            }
            Ev::Value(code_text, Ty::Str) => Ok(code(format!(
                "page.expect().to_have_url(&{code_text}).await?;"
            ))),
            _ => Err(
                "a URL check other than a string or a plain regular expression is not converted"
                    .to_owned(),
            ),
        }
    }

    fn wait_for_selector(&mut self, name: &str, args: &Args<'_>) -> TResult<Ev> {
        let steps = if name == "waitForXPath" {
            let xpath = self.literal_text(args.need(0, "the XPath")?, "XPath")?;
            vec![Step::XPath(xpath)]
        } else {
            self.selector(args.need(0, "the selector")?)?
        };
        let options = args.options(1)?;
        let state = match self.flavor {
            Flavor::Playwright => {
                check_options(name, &options, &["state", "timeout", "strict"])?;
                match option(&options, "state") {
                    None => "Visible",
                    Some(state) => match self.literal_text(state, "state")?.as_str() {
                        "attached" => "Present",
                        "detached" => "Absent",
                        "visible" => "Visible",
                        "hidden" => "Hidden",
                        other => return Err(format!("the state {other:?} is not converted")),
                    },
                }
            }
            Flavor::Puppeteer => {
                check_options(name, &options, &["visible", "hidden", "timeout"])?;
                let visible = option(&options, "visible")
                    .map(|v| self.boolean(v))
                    .transpose()?
                    .unwrap_or(false);
                let hidden = option(&options, "hidden")
                    .map(|v| self.boolean(v))
                    .transpose()?
                    .unwrap_or(false);
                match (visible, hidden) {
                    (true, false) => "Visible",
                    (false, true) => "Hidden",
                    (false, false) => "Present",
                    (true, true) => return Err("both `visible` and `hidden`".to_owned()),
                }
            }
        };
        let locator = self.chain("page", &steps)?;
        let waiting = self.with_timeout(&locator, &options)?;
        self.needs.sb_cdp.insert("State");
        Ok(Ev::Waited(
            vec![Line::Code(format!(
                "{waiting}.wait_for(State::{state}).await?;"
            ))],
            locator,
        ))
    }

    fn screenshot(&mut self, base: &str, args: &Args<'_>) -> TResult<Ev> {
        let options = args.options(0)?;
        check_options(
            "screenshot",
            &options,
            &["path", "fullPage", "timeout", "animations"],
        )?;
        if let Some(full) = option(&options, "fullPage") {
            if self.boolean(full)? {
                return Err("a full-page screenshot is not converted".to_owned());
            }
        }
        match option(&options, "path") {
            Some(path) => {
                let path = self.text(path)?;
                Ok(code(format!(
                    "std::fs::write({path}, {base}.screenshot().await?)?;"
                )))
            }
            None => Ok(code(format!("{base}.screenshot().await?;"))),
        }
    }

    /// The JavaScript for `evaluate()`, as a Rust string literal: a string is
    /// used as it is (a Python string that is a function is called), and a
    /// function that uses nothing from the test is called as written.
    fn script(&mut self, expr: &Expr) -> TResult<String> {
        match expr {
            Expr::Str(text) => {
                let trimmed = text.trim_start();
                let function = trimmed.starts_with("function")
                    || trimmed.starts_with("async")
                    || (trimmed.starts_with('(') && text.contains("=>"))
                    || trimmed.split_once("=>").is_some_and(|(param, _)| {
                        param
                            .trim()
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_')
                            && !param.trim().is_empty()
                    });
                if self.python && function {
                    Ok(rust_str(&format!("({text})()")))
                } else {
                    Ok(rust_str(text))
                }
            }
            Expr::Function(func) => {
                if !func.params.is_empty() {
                    return Err(
                        "an evaluate() function that takes arguments is not converted".to_owned(),
                    );
                }
                if func.typescript {
                    return Err(
                        "TypeScript syntax inside an evaluate() function is not converted"
                            .to_owned(),
                    );
                }
                if let Some(name) = func
                    .names
                    .iter()
                    .find(|name| self.scope.contains_key(*name))
                {
                    return Err(format!(
                        "an evaluate() function that uses `{name}` from the test is not converted"
                    ));
                }
                let text = self.source.get(func.start..func.end).unwrap_or_default();
                Ok(rust_str(&format!("({text})()")))
            }
            other => match self.eval(other)? {
                Ev::Lit(Lit::Str(text)) => Ok(rust_str(&text)),
                _ => Err(
                    "evaluate() of something other than a function or a string is not converted"
                        .to_owned(),
                ),
            },
        }
    }

    // ------------------------------------------------------------------
    // Locators
    // ------------------------------------------------------------------

    /// The steps of a selector string, read with the flavor's rules.
    fn selector(&mut self, expr: &Expr) -> TResult<Vec<Step>> {
        let text = match self.eval(expr)? {
            Ev::Lit(Lit::Str(text)) => text,
            Ev::Value(..) => return Err("a selector built at run time is not converted".to_owned()),
            _ => return Err("a selector that is not a string is not converted".to_owned()),
        };
        match self.flavor {
            Flavor::Playwright => selector::playwright(&text),
            Flavor::Puppeteer => selector::puppeteer(&text),
        }
        .map_err(|what| format!("{what} is not converted"))
    }

    /// The steps of a method that makes a locator, or `None` for any other
    /// method. The flag says whether it finds a list (`$$`).
    fn locator_steps(&mut self, name: &str, args: &Args<'_>) -> TResult<Option<(Vec<Step>, bool)>> {
        let exact = |this: &mut Self, options: &[(String, &Expr)]| -> TResult<bool> {
            option(options, "exact")
                .map(|value| this.boolean(value))
                .transpose()
                .map(|exact| exact.unwrap_or(false))
        };
        let steps = match name {
            "locator" => {
                let mut steps = self.selector(args.need(0, "the selector")?)?;
                let options = args.options(1)?;
                check_options(name, &options, &["hasText"])?;
                if let Some(text) = option(&options, "hasText") {
                    steps.push(Step::HasText(self.literal_text(text, "hasText")?));
                }
                (steps, false)
            }
            "frameLocator" | "$" => {
                if args.pos.len() > 1 {
                    return Err(format!("`{name}()` with options is not converted"));
                }
                (self.selector(args.need(0, "the selector")?)?, false)
            }
            "$$" => (self.selector(args.need(0, "the selector")?)?, true),
            "$x" => {
                let xpath = self.literal_text(args.need(0, "the XPath")?, "XPath")?;
                (vec![Step::XPath(xpath)], true)
            }
            "getByRole" => {
                let role = self.literal_text(args.need(0, "the role")?, "role")?;
                let options = args.options(1)?;
                check_options(name, &options, &["name", "exact"])?;
                let role_name = option(&options, "name")
                    .map(|value| self.literal_text(value, "name"))
                    .transpose()?;
                let exact = exact(self, &options)?;
                (
                    vec![Step::Role {
                        role,
                        name: role_name,
                        exact,
                    }],
                    false,
                )
            }
            "getByText" | "getByLabel" | "getByPlaceholder" | "getByAltText" | "getByTitle" => {
                let text = self.literal_text(args.need(0, "the text")?, "text")?;
                let options = args.options(1)?;
                check_options(name, &options, &["exact"])?;
                let exact = exact(self, &options)?;
                let step = match name {
                    "getByText" => Step::Text { text, exact },
                    "getByLabel" => Step::Label { text, exact },
                    "getByPlaceholder" => {
                        Step::Css(selector::attribute_css("placeholder", &text, exact))
                    }
                    "getByAltText" => Step::Css(selector::attribute_css("alt", &text, exact)),
                    _ => Step::Css(selector::attribute_css("title", &text, exact)),
                };
                (vec![step], false)
            }
            "getByTestId" => {
                let id = self.literal_text(args.need(0, "the test id")?, "test id")?;
                (
                    vec![Step::Css(format!("[data-testid={}]", css_quoted(&id, '"')))],
                    false,
                )
            }
            _ => return Ok(None),
        };
        Ok(Some(steps))
    }

    /// `base` with the steps applied, as Rust.
    fn chain(&mut self, base: &str, steps: &[Step]) -> TResult<String> {
        let mut out = base.to_owned();
        for (index, step) in steps.iter().enumerate() {
            let modifier = matches!(
                step,
                Step::Nth(_) | Step::Last | Step::Visible | Step::HasText(_)
            );
            if index == 0 && base == "page" && modifier {
                return Err(
                    "a selector that starts with a position or a filter is not converted"
                        .to_owned(),
                );
            }
            match step {
                Step::Css(css) => {
                    let arg = self.css_arg(css);
                    out.push_str(&format!(".locator({arg})"));
                }
                Step::XPath(xpath) => {
                    let arg = self.xpath_arg(xpath);
                    out.push_str(&format!(".locator({arg})"));
                }
                Step::Role { role, name, exact } => {
                    self.needs.selector_buf = true;
                    let name = name
                        .as_ref()
                        .map_or_else(|| "None".to_owned(), |n| format!("Some({})", rust_str(n)));
                    out.push_str(&format!(
                        ".locator(SelectorBuf::role({}, {name}, {exact}))",
                        rust_str(role)
                    ));
                }
                Step::Text { text, exact } => {
                    self.needs.selector_buf = true;
                    out.push_str(&format!(
                        ".locator(SelectorBuf::text({}, {exact}))",
                        rust_str(text)
                    ));
                }
                Step::Label { text, exact } => {
                    self.needs.selector_buf = true;
                    out.push_str(&format!(
                        ".locator(SelectorBuf::label({}, {exact}))",
                        rust_str(text)
                    ));
                }
                Step::HasText(text) => {
                    self.needs.selector_buf = true;
                    out.push_str(&format!(
                        ".locator(SelectorBuf::has_text({}))",
                        rust_str(text)
                    ));
                }
                Step::Nth(n) => out.push_str(&format!(".nth({n})")),
                Step::Last => out.push_str(".last()"),
                Step::Visible => out.push_str(".visible()"),
            }
        }
        Ok(out)
    }

    fn css_arg(&mut self, css: &str) -> String {
        if matches!(Selector::auto(css), Selector::Css(_)) && !css.contains(":contains(") {
            rust_str(css)
        } else {
            self.needs.selector_buf = true;
            format!("SelectorBuf::css({})", rust_str(css))
        }
    }

    fn xpath_arg(&mut self, xpath: &str) -> String {
        if matches!(Selector::auto(xpath), Selector::XPath(_)) {
            rust_str(xpath)
        } else {
            self.needs.selector_buf = true;
            format!("SelectorBuf::xpath({})", rust_str(xpath))
        }
    }

    fn with_timeout(&mut self, locator: &str, options: &[(String, &Expr)]) -> TResult<String> {
        match option(options, "timeout") {
            Some(timeout) => {
                let ms = self.whole_number(timeout, "the timeout")?;
                Ok(format!(
                    "{locator}.with_timeout(std::time::Duration::from_millis({ms}))"
                ))
            }
            None => Ok(locator.to_owned()),
        }
    }

    fn locator_method(&mut self, locator: &str, name: &str, args: &[Arg]) -> TResult<Ev> {
        let args = Args::new(args)?;
        if let Some((steps, list)) = self.locator_steps(name, &args)? {
            let code = self.chain(locator, &steps)?;
            return Ok(if list {
                Ev::List(code)
            } else {
                Ev::Locator(code)
            });
        }
        match name {
            "first" => Ok(Ev::Locator(format!("{locator}.first()"))),
            "last" => Ok(Ev::Locator(format!("{locator}.last()"))),
            "nth" => {
                let n = self.number(args.need(0, "the index")?)?;
                match whole(n) {
                    Some(n) => Ok(Ev::Locator(format!("{locator}.nth({n})"))),
                    None if n == -1.0 => Ok(Ev::Locator(format!("{locator}.last()"))),
                    None => Err(format!("`nth({n})` is not converted")),
                }
            }
            "filter" => {
                let options = args.options(0)?;
                check_options(name, &options, &["hasText"])?;
                let text = option(&options, "hasText").ok_or("a filter without hasText")?;
                let text = self.literal_text(text, "hasText")?;
                let code = self.chain(locator, &[Step::HasText(text)])?;
                Ok(Ev::Locator(code))
            }
            "contentFrame" => Ok(Ev::Locator(locator.to_owned())),
            "setTimeout" => {
                let ms = self.whole_number(args.need(0, "the timeout")?, "the timeout")?;
                Ok(Ev::Locator(format!(
                    "{locator}.with_timeout(std::time::Duration::from_millis({ms}))"
                )))
            }
            "$eval" => {
                let steps = self.selector(args.need(0, "the selector")?)?;
                let found = self.chain(locator, &steps)?;
                if args.pos.len() > 2 {
                    return Err("$eval() with arguments is not converted".to_owned());
                }
                self.element_read(&found, args.need(1, "the function")?, false)
            }
            "$$eval" => {
                let steps = self.selector(args.need(0, "the selector")?)?;
                let found = self.chain(locator, &steps)?;
                if args.pos.len() > 2 {
                    return Err("$$eval() with arguments is not converted".to_owned());
                }
                self.element_read(&found, args.need(1, "the function")?, true)
            }
            _ => self.locator_action(locator, name, &args),
        }
    }

    fn locator_action(&mut self, locator: &str, name: &str, args: &Args<'_>) -> TResult<Ev> {
        match name {
            "click" | "tap" => {
                let options = args.options(0)?;
                check_options(name, &options, &["button", "clickCount", "timeout", "delay", "noWaitAfter"])?;
                let target = self.with_timeout(locator, &options)?;
                let button = option(&options, "button")
                    .map(|b| self.literal_text(b, "button"))
                    .transpose()?;
                let count = option(&options, "clickCount")
                    .map(|c| self.whole_number(c, "the click count"))
                    .transpose()?
                    .unwrap_or(1);
                let method = match (button.as_deref(), count) {
                    (None | Some("left"), 1) => "click",
                    (None | Some("left"), 2) => "double_click",
                    (Some("right"), 1) => "right_click",
                    _ => return Err("this kind of click is not converted".to_owned()),
                };
                Ok(code(format!("{target}.{method}().await?;")))
            }
            "dblclick" => {
                let options = args.options(0)?;
                check_options(name, &options, &["timeout", "delay", "noWaitAfter"])?;
                let target = self.with_timeout(locator, &options)?;
                Ok(code(format!("{target}.double_click().await?;")))
            }
            "fill" | "type" | "pressSequentially" => {
                let text = self.text(args.need(0, "the text")?)?;
                let options = args.options(1)?;
                check_options(name, &options, &["timeout", "delay", "noWaitAfter"])?;
                let target = self.with_timeout(locator, &options)?;
                let method = if name == "fill" { "fill" } else { "type_text" };
                Ok(code(format!("{target}.{method}({text}).await?;")))
            }
            "press" => {
                let key = self.literal_text(args.need(0, "the key")?, "key")?;
                let options = args.options(1)?;
                check_options(name, &options, &["timeout", "delay", "noWaitAfter"])?;
                let target = self.with_timeout(locator, &options)?;
                match key_press(&key)? {
                    KeyPress::Named(key) => {
                        self.needs.sb_cdp.insert("Key");
                        Ok(code(format!("{target}.press(Key::{key}).await?;")))
                    }
                    KeyPress::Text(text) => Ok(code(format!(
                        "{target}.type_text({}).await?;",
                        rust_str(&text)
                    ))),
                }
            }
            "check" | "uncheck" | "hover" | "focus" | "clear" => {
                let options = args.options(0)?;
                check_options(name, &options, &["timeout", "force", "noWaitAfter"])?;
                if option(&options, "force").is_some() {
                    return Err(format!("`{name}({{ force }})` is not converted"));
                }
                let target = self.with_timeout(locator, &options)?;
                Ok(code(format!("{target}.{name}().await?;")))
            }
            "setChecked" => {
                let checked = self.boolean(args.need(0, "the checked state")?)?;
                Ok(code(format!("{locator}.set_checked({checked}).await?;")))
            }
            "selectOption" => self.select_option(locator, args),
            "select" => {
                if args.pos.len() != 1 {
                    return Err("selecting several options is not converted".to_owned());
                }
                let value = self.text(args.need(0, "the value")?)?;
                self.needs.sb_cdp.insert("SelectBy");
                Ok(code(format!(
                    "{locator}.select_option(SelectBy::Value({value})).await?;"
                )))
            }
            "blur" => Ok(code(format!(
                "{locator}.page().evaluate(\"document.activeElement && document.activeElement.blur()\").await?;"
            ))),
            "scrollIntoViewIfNeeded" | "scrollIntoView" => {
                Ok(code(format!("{locator}.scroll_into_view().await?;")))
            }
            "setInputFiles" | "uploadFile" => {
                let path = match args.need(0, "the file")? {
                    Expr::Array(items) if items.len() == 1 => &items[0],
                    Expr::Array(_) => {
                        return Err("several files at once are not converted".to_owned())
                    }
                    other => other,
                };
                if args.pos.len() > 1 && name == "uploadFile" {
                    return Err("several files at once are not converted".to_owned());
                }
                let path = self.literal_text(path, "file path")?;
                self.needs.helpers.insert(Helper::SetInputFiles);
                Ok(code(format!(
                    "support::set_input_files(&{locator}, {}).await?;",
                    rust_str(&path)
                )))
            }
            "waitFor" => {
                let options = args.options(0)?;
                check_options(name, &options, &["state", "timeout"])?;
                let state = match option(&options, "state") {
                    None => "Visible".to_owned(),
                    Some(state) => match self.literal_text(state, "state")?.as_str() {
                        "attached" => "Present".to_owned(),
                        "detached" => "Absent".to_owned(),
                        "visible" => "Visible".to_owned(),
                        "hidden" => "Hidden".to_owned(),
                        other => return Err(format!("the state {other:?} is not converted")),
                    },
                };
                let target = self.with_timeout(locator, &options)?;
                self.needs.sb_cdp.insert("State");
                Ok(code(format!("{target}.wait_for(State::{state}).await?;")))
            }
            "wait" => {
                self.needs.sb_cdp.insert("State");
                Ok(code(format!("{locator}.wait_for(State::Visible).await?;")))
            }
            "screenshot" => self.screenshot(locator, args),
            "textContent" | "innerText" => {
                Ok(Ev::Value(format!("{locator}.text().await?"), Ty::Str))
            }
            "inputValue" => {
                self.needs.helpers.insert(Helper::FieldValue);
                Ok(Ev::Value(
                    format!("support::field_value(&{locator}).await?"),
                    Ty::Str,
                ))
            }
            "getAttribute" => {
                let attribute = self.text(args.need(0, "the attribute name")?)?;
                Ok(Ev::Value(
                    format!("{locator}.attribute({attribute}).await?"),
                    Ty::OptStr,
                ))
            }
            "isVisible" => Ok(Ev::Value(format!("{locator}.is_visible().await?"), Ty::Bool)),
            "isHidden" => Ok(Ev::Value(format!("!{locator}.is_visible().await?"), Ty::Bool)),
            "isChecked" => Ok(Ev::Value(format!("{locator}.is_checked().await?"), Ty::Bool)),
            "isEnabled" => Ok(Ev::Value(format!("{locator}.is_enabled().await?"), Ty::Bool)),
            "isDisabled" => Ok(Ev::Value(format!("!{locator}.is_enabled().await?"), Ty::Bool)),
            "count" => Ok(Ev::Value(format!("{locator}.count().await?"), Ty::Count)),
            "evaluate" => {
                if args.pos.len() > 1 {
                    return Err("evaluate() with arguments is not converted".to_owned());
                }
                self.element_read(locator, args.need(0, "the function")?, false)
            }
            "dragTo" => match self.eval(args.need(0, "the target")?)? {
                Ev::Locator(target) => Ok(code(format!("{locator}.drag_to(&{target}).await?;"))),
                _ => Err("dragTo() of something other than a locator is not converted".to_owned()),
            },
            _ => Err(format!("`.{name}()` on a locator is not converted")),
        }
    }

    fn select_option(&mut self, locator: &str, args: &Args<'_>) -> TResult<Ev> {
        // Python passes `value=`, `label=` or `index=` as keywords.
        let keyword = args
            .kw
            .iter()
            .find(|(key, _)| matches!(key.as_str(), "value" | "label" | "index"));
        let by = match (args.get(0), keyword) {
            (None, Some((key, value))) => self.select_by(key, value)?,
            (Some(arg), None) => match arg {
                Expr::Array(items) if items.len() == 1 => {
                    return self.select_option(
                        locator,
                        &Args {
                            pos: vec![&items[0]],
                            kw: Vec::new(),
                        },
                    )
                }
                Expr::Array(_) => {
                    return Err("selecting several options is not converted".to_owned())
                }
                Expr::Object(entries) => match entries.as_slice() {
                    [(key, value)] if matches!(key.as_str(), "value" | "label" | "index") => {
                        self.select_by(key, value)?
                    }
                    _ => return Err("these selectOption() options are not converted".to_owned()),
                },
                other => {
                    let wanted = self.text(other)?;
                    self.needs.helpers.insert(Helper::SelectOption);
                    return Ok(code(format!(
                        "support::select_option(&{locator}, {wanted}).await?;"
                    )));
                }
            },
            _ => return Err("this selectOption() is not converted".to_owned()),
        };
        self.needs.sb_cdp.insert("SelectBy");
        Ok(code(format!("{locator}.select_option({by}).await?;")))
    }

    fn select_by(&mut self, key: &str, value: &Expr) -> TResult<String> {
        Ok(match key {
            "value" => format!("SelectBy::Value({})", self.text(value)?),
            "label" => format!("SelectBy::Text({})", self.text(value)?),
            _ => format!(
                "SelectBy::Index({})",
                self.whole_number(value, "the index")?
            ),
        })
    }

    /// A callback that reads one thing from an element (`el => el.textContent`),
    /// as the locator call that reads it.
    fn element_read(&mut self, locator: &str, callback: &Expr, list: bool) -> TResult<Ev> {
        let unsupported = || {
            "an element callback other than one that reads textContent, innerText, value, \
             checked, getAttribute() or length is not converted"
                .to_owned()
        };
        let Expr::Function(func) = callback else {
            return Err(unsupported());
        };
        let [param] = func.params.as_slice() else {
            return Err(unsupported());
        };
        let Some(param) = param.name.as_deref() else {
            return Err(unsupported());
        };
        let body = match &func.body {
            FnBody::Expr(expr) => expr.as_ref(),
            FnBody::Block(stmts) => match stmts.as_slice() {
                [Stmt {
                    kind: StmtKind::Return(Some(expr)),
                    ..
                }] => expr,
                _ => return Err(unsupported()),
            },
        };
        let on_param = |expr: &Expr| matches!(expr, Expr::Ident(name) if name == param);
        if list {
            return match body {
                Expr::Member { object, property } if on_param(object) && property == "length" => {
                    Ok(Ev::Value(format!("{locator}.count().await?"), Ty::Count))
                }
                _ => Err(unsupported()),
            };
        }
        match body {
            Expr::Member { object, property } if on_param(object) => match property.as_str() {
                "textContent" | "innerText" => {
                    Ok(Ev::Value(format!("{locator}.text().await?"), Ty::Str))
                }
                "value" => {
                    self.needs.helpers.insert(Helper::FieldValue);
                    Ok(Ev::Value(
                        format!("support::field_value(&{locator}).await?"),
                        Ty::Str,
                    ))
                }
                "checked" => Ok(Ev::Value(
                    format!("{locator}.is_checked().await?"),
                    Ty::Bool,
                )),
                _ => Err(unsupported()),
            },
            Expr::Call { callee, args } => match (callee.as_ref(), args.as_slice()) {
                (Expr::Member { object, property }, [Arg::Pos(Expr::Str(attribute))])
                    if on_param(object) && property == "getAttribute" =>
                {
                    Ok(Ev::Value(
                        format!("{locator}.attribute({}).await?", rust_str(attribute)),
                        Ty::OptStr,
                    ))
                }
                (Expr::Member { object, property }, [])
                    if property == "trim"
                        && matches!(object.as_ref(), Expr::Member { object, property }
                            if on_param(object) && (property == "textContent" || property == "innerText")) =>
                {
                    Ok(Ev::Value(
                        format!("{locator}.text().await?.trim().to_owned()"),
                        Ty::Str,
                    ))
                }
                _ => Err(unsupported()),
            },
            _ => Err(unsupported()),
        }
    }

    // ------------------------------------------------------------------
    // Keyboard and mouse
    // ------------------------------------------------------------------

    fn keyboard_method(&mut self, name: &str, args: &[Arg]) -> TResult<Ev> {
        let args = Args::new(args)?;
        match name {
            "press" => {
                let key = self.literal_text(args.need(0, "the key")?, "key")?;
                let options = args.options(1)?;
                check_options(name, &options, &["delay"])?;
                match key_press(&key)? {
                    KeyPress::Named(key) => {
                        self.needs.sb_cdp.insert("Key");
                        Ok(code(format!("page.keyboard().press(Key::{key}).await?;")))
                    }
                    KeyPress::Text(text) => Ok(code(format!(
                        "page.keyboard().type_text({}).await?;",
                        rust_str(&text)
                    ))),
                }
            }
            "type" => {
                let text = self.text(args.need(0, "the text")?)?;
                let options = args.options(1)?;
                check_options(name, &options, &["delay"])?;
                Ok(code(format!("page.keyboard().type_text({text}).await?;")))
            }
            "insertText" | "sendCharacter" => {
                let text = self.text(args.need(0, "the text")?)?;
                Ok(code(format!("page.keyboard().insert_text({text}).await?;")))
            }
            _ => Err(format!("`keyboard.{name}()` is not converted")),
        }
    }

    fn mouse_method(&mut self, name: &str, args: &[Arg]) -> TResult<Ev> {
        let args = Args::new(args)?;
        let x = self.number(args.need(0, "the x coordinate")?)?;
        let y = self.number(args.need(1, "the y coordinate")?)?;
        let point = format!(
            "Point::new({}, {})",
            float(x).ok_or("a coordinate that is not finite")?,
            float(y).ok_or("a coordinate that is not finite")?
        );
        let options = args.options(2)?;
        let method = match name {
            "click" => {
                check_options(name, &options, &["button", "clickCount", "delay"])?;
                let button = option(&options, "button")
                    .map(|b| self.literal_text(b, "button"))
                    .transpose()?;
                let count = option(&options, "clickCount")
                    .map(|c| self.whole_number(c, "the click count"))
                    .transpose()?
                    .unwrap_or(1);
                match (button.as_deref(), count) {
                    (None | Some("left"), 1) => "click",
                    (None | Some("left"), 2) => "double_click",
                    (Some("right"), 1) => "right_click",
                    _ => return Err("this kind of click is not converted".to_owned()),
                }
            }
            "dblclick" => {
                check_options(name, &options, &["delay"])?;
                "double_click"
            }
            "move" => {
                check_options(name, &options, &["steps"])?;
                "move_to"
            }
            _ => return Err(format!("`mouse.{name}()` is not converted")),
        };
        self.needs.sb_cdp.insert("Point");
        Ok(code(format!("page.mouse().{method}({point}).await?;")))
    }

    // ------------------------------------------------------------------
    // Assertions
    // ------------------------------------------------------------------

    fn subject(&mut self, expr: &Expr) -> TResult<Subject> {
        match self.eval(expr)? {
            Ev::Page => Ok(Subject::Page),
            Ev::Locator(code) => Ok(Subject::Locator(code)),
            Ev::List(code) => Ok(Subject::List(code)),
            Ev::Value(code, ty) => Ok(Subject::Value(code, ty)),
            Ev::Lit(_) => Err("an expect() on a literal is not converted".to_owned()),
            _ => Err("an expect() on this value is not converted".to_owned()),
        }
    }

    fn value_subject(&mut self, expr: &Expr) -> TResult<(String, Ty)> {
        match self.eval(expr)? {
            Ev::Value(code, ty) => Ok((code, ty)),
            Ev::List(code) => Ok((format!("{code}.count().await?"), Ty::Count)),
            _ => Err("a check of this value is not converted".to_owned()),
        }
    }

    fn expect_method(
        &mut self,
        mut expectation: Expectation,
        name: &str,
        args: &[Arg],
    ) -> TResult<Ev> {
        let mut name = name.to_owned();
        if let Some(rest) = name.strip_prefix("notTo") {
            expectation.negate = !expectation.negate;
            name = format!("to{rest}");
        }
        let args = Args::new(args)?;
        match expectation.subject.clone() {
            Subject::Page => self.page_matcher(expectation.negate, &name, &args),
            Subject::Locator(code) => self.locator_matcher(&code, expectation.negate, &name, &args),
            Subject::List(code) => match name.as_str() {
                "toHaveLength" | "lengthOf" | "length" | "toHaveCount" => {
                    let n = self.whole_number(args.need(0, "the count")?, "the count")?;
                    Ok(Ev::Lines(vec![Line::Code(eq_line(
                        &format!("{code}.count().await?"),
                        &n.to_string(),
                        expectation.negate,
                    ))]))
                }
                _ => Err(format!("`{name}()` on a list of elements is not converted")),
            },
            Subject::Value(code, ty) => {
                let matcher = match name.as_str() {
                    "toBe" | "toEqual" | "toStrictEqual" | "equal" | "equals" | "eq" | "eql"
                    | "eqls" => Matcher::Eq,
                    "toContain" | "include" | "includes" | "contain" | "contains" => {
                        Matcher::Contains
                    }
                    "toMatch" | "match" | "matches" => Matcher::Match,
                    "toBeTruthy" => Matcher::Truthy,
                    "toBeFalsy" => {
                        expectation.negate = !expectation.negate;
                        Matcher::Truthy
                    }
                    "toBeNull" | "toBeUndefined" => Matcher::Null,
                    "toBeDefined" => {
                        expectation.negate = !expectation.negate;
                        Matcher::Null
                    }
                    "toBeGreaterThan" | "above" | "gt" | "greaterThan" => Matcher::Gt,
                    "toBeLessThan" | "below" | "lt" | "lessThan" => Matcher::Lt,
                    "toBeGreaterThanOrEqual" | "least" | "gte" => Matcher::Ge,
                    "toBeLessThanOrEqual" | "most" | "lte" => Matcher::Le,
                    "toHaveLength" | "lengthOf" => Matcher::Len,
                    _ => return Err(format!("the matcher `{name}()` is not converted")),
                };
                self.assert_value((code, ty), matcher, args.get(0), expectation.negate)
            }
        }
    }

    fn chai_terminal(&mut self, expectation: Expectation, name: &str) -> TResult<Ev> {
        let Subject::Value(code, ty) = expectation.subject else {
            return Err(format!("`.{name}` on this value is not converted"));
        };
        let negate = expectation.negate;
        match name {
            "true" | "false" => {
                if ty != Ty::Bool {
                    return Err(format!(
                        "`.{name}` on a value that is not a boolean is not converted"
                    ));
                }
                let expect_true = (name == "true") != negate;
                Ok(code_line(assert_line(&code, !expect_true)))
            }
            "ok" => self.assert_value((code, ty), Matcher::Truthy, None, negate),
            "null" | "undefined" => self.assert_value((code, ty), Matcher::Null, None, negate),
            "exist" => self.assert_value((code, ty), Matcher::Null, None, !negate),
            _ => {
                if ty != Ty::Str {
                    return Err("`.empty` on a value that is not text is not converted".to_owned());
                }
                Ok(code_line(assert_line(
                    &format!("{code}.is_empty()"),
                    negate,
                )))
            }
        }
    }

    fn node_assert(&mut self, name: &str, args: &[Arg]) -> TResult<Ev> {
        let args = Args::new(args)?;
        let (matcher, negate) = match name {
            "ok" => (Matcher::Truthy, false),
            "equal" | "strictEqual" | "deepEqual" | "deepStrictEqual" => (Matcher::Eq, false),
            "notEqual" | "notStrictEqual" | "notDeepEqual" | "notDeepStrictEqual" => {
                (Matcher::Eq, true)
            }
            "match" => (Matcher::Match, false),
            "doesNotMatch" => (Matcher::Match, true),
            _ => return Err(format!("`assert.{name}()` is not converted")),
        };
        let subject = self.value_subject(args.need(0, "the value")?)?;
        self.assert_value(subject, matcher, args.get(1), negate)
    }

    fn python_assert(&mut self, test: &Expr) -> TResult<()> {
        let ev = match test {
            Expr::Unary { op: "not", operand } => {
                let subject = self.value_subject(operand)?;
                self.assert_value(subject, Matcher::Truthy, None, true)?
            }
            Expr::Binary { op, left, right } => {
                let (matcher, negate, subject, expected) = match *op {
                    "==" | "!=" => {
                        // `assert "Shop" == page.title()` reads either way round.
                        let literal = |e: &Expr| {
                            matches!(e, Expr::Str(_) | Expr::Num(_) | Expr::Bool(_) | Expr::Null)
                        };
                        let (subject, expected) = if literal(left) && !literal(right) {
                            (right.as_ref(), left.as_ref())
                        } else {
                            (left.as_ref(), right.as_ref())
                        };
                        (Matcher::Eq, *op == "!=", subject, Some(expected))
                    }
                    "in" | "not in" => (
                        Matcher::Contains,
                        *op == "not in",
                        right.as_ref(),
                        Some(left.as_ref()),
                    ),
                    "is" | "is not" if matches!(right.as_ref(), Expr::Null) => {
                        (Matcher::Null, *op == "is not", left.as_ref(), None)
                    }
                    ">" => (Matcher::Gt, false, left.as_ref(), Some(right.as_ref())),
                    "<" => (Matcher::Lt, false, left.as_ref(), Some(right.as_ref())),
                    ">=" => (Matcher::Ge, false, left.as_ref(), Some(right.as_ref())),
                    "<=" => (Matcher::Le, false, left.as_ref(), Some(right.as_ref())),
                    _ => return Err(format!("an assert with `{op}` is not converted")),
                };
                let subject = self.value_subject(subject)?;
                self.assert_value(subject, matcher, expected, negate)?
            }
            other => {
                let subject = self.value_subject(other)?;
                self.assert_value(subject, Matcher::Truthy, None, false)?
            }
        };
        if let Ev::Lines(lines) = ev {
            self.lines.extend(lines);
        }
        Ok(())
    }

    /// An assertion about a value read from the page, as `assert!` lines.
    fn assert_value(
        &mut self,
        (subject, ty): (String, Ty),
        matcher: Matcher,
        expected: Option<&Expr>,
        negate: bool,
    ) -> TResult<Ev> {
        let not_converted = || "comparing these values is not converted".to_owned();
        let mut expected_value = || -> TResult<Ev> {
            let expr = expected.ok_or("the expected value is missing")?;
            self.eval(expr)
        };
        let line = match matcher {
            Matcher::Eq => match (ty, expected_value()?) {
                (Ty::Bool, Ev::Lit(Lit::Bool(b))) => assert_line(&subject, b == negate),
                (Ty::Str | Ty::Json, Ev::Lit(Lit::Str(text))) => {
                    eq_line(&subject, &rust_str(&text), negate)
                }
                (Ty::Str, Ev::Value(other, Ty::Str)) => eq_line(&subject, &other, negate),
                (Ty::OptStr, Ev::Lit(Lit::Str(text))) => eq_line(
                    &format!("{subject}.as_deref()"),
                    &format!("Some({})", rust_str(&text)),
                    negate,
                ),
                (Ty::OptStr, Ev::Lit(Lit::Null)) => {
                    assert_line(&format!("{subject}.is_none()"), negate)
                }
                (Ty::Count, Ev::Lit(Lit::Num(n))) => eq_line(
                    &subject,
                    &whole(n).ok_or_else(not_converted)?.to_string(),
                    negate,
                ),
                (Ty::Count, Ev::Value(other, Ty::Count)) => eq_line(&subject, &other, negate),
                (Ty::Json, Ev::Lit(Lit::Num(n))) => {
                    let number = match whole(n) {
                        Some(n) => n.to_string(),
                        None => float(n).ok_or_else(not_converted)?,
                    };
                    eq_line(&subject, &number, negate)
                }
                (Ty::Json, Ev::Lit(Lit::Bool(b))) => eq_line(&subject, &b.to_string(), negate),
                (Ty::Json, Ev::Lit(Lit::Null)) => {
                    assert_line(&format!("{subject}.is_null()"), negate)
                }
                _ => return Err(not_converted()),
            },
            Matcher::Contains | Matcher::Match => {
                let (needle, exact) = match (matcher, expected_value()?) {
                    (_, Ev::Lit(Lit::Str(text))) => (rust_str(&text), false),
                    (Matcher::Contains, Ev::Value(other, Ty::Str)) => (format!("&{other}"), false),
                    (Matcher::Match, Ev::Lit(Lit::Regex { pattern, flags })) => {
                        let (text, exact) = regex_text(&pattern, &flags)?;
                        (rust_str(&text), exact)
                    }
                    _ => return Err(not_converted()),
                };
                let hay = match ty {
                    Ty::Str => subject.clone(),
                    Ty::OptStr => format!("{subject}.as_deref().unwrap_or_default()"),
                    Ty::Json => format!("{subject}.as_str().unwrap_or_default()"),
                    _ => return Err(not_converted()),
                };
                if exact {
                    eq_line(&hay, &needle, negate)
                } else {
                    assert_line(&format!("{hay}.contains({needle})"), negate)
                }
            }
            Matcher::Truthy => {
                let condition = match ty {
                    Ty::Bool => subject.clone(),
                    Ty::Str => format!("!{subject}.is_empty()"),
                    Ty::OptStr => format!("{subject}.is_some()"),
                    Ty::Count => format!("{subject} > 0"),
                    Ty::Json => return Err(not_converted()),
                };
                assert_line(&condition, negate)
            }
            Matcher::Null => match ty {
                Ty::OptStr => assert_line(&format!("{subject}.is_none()"), negate),
                Ty::Json => assert_line(&format!("{subject}.is_null()"), negate),
                _ => return Err(not_converted()),
            },
            Matcher::Gt | Matcher::Lt | Matcher::Ge | Matcher::Le => {
                let Ev::Lit(Lit::Num(n)) = expected_value()? else {
                    return Err(not_converted());
                };
                if ty != Ty::Count {
                    return Err(not_converted());
                }
                let n = whole(n).ok_or_else(not_converted)?;
                let op = match matcher {
                    Matcher::Gt => ">",
                    Matcher::Lt => "<",
                    Matcher::Ge => ">=",
                    _ => "<=",
                };
                assert_line(&format!("{subject} {op} {n}"), negate)
            }
            Matcher::Len => {
                let Ev::Lit(Lit::Num(n)) = expected_value()? else {
                    return Err(not_converted());
                };
                let n = whole(n).ok_or_else(not_converted)?;
                match ty {
                    Ty::Str => eq_line(
                        &format!("{subject}.chars().count()"),
                        &n.to_string(),
                        negate,
                    ),
                    _ => return Err(not_converted()),
                }
            }
        };
        Ok(code_line(line))
    }

    fn page_matcher(&mut self, negate: bool, name: &str, args: &Args<'_>) -> TResult<Ev> {
        let not = if negate { ".not()" } else { "" };
        match name {
            "toHaveTitle" | "toHaveURL" | "toHaveUrl" => {
                let options = args.options(1)?;
                check_options(name, &options, &["timeout"])?;
                let timeout = self.expect_timeout(&options)?;
                let (exact_method, contains_method) = if name == "toHaveTitle" {
                    ("to_have_title", "to_contain_title")
                } else {
                    ("to_have_url", "to_contain_url")
                };
                let (text, exact) = match self.eval(args.need(0, "the expected value")?)? {
                    Ev::Lit(Lit::Str(text)) => (rust_str(&text), true),
                    Ev::Value(code_text, Ty::Str) => (format!("&{code_text}"), true),
                    Ev::Lit(Lit::Regex { pattern, flags }) => {
                        let (text, exact) = regex_text(&pattern, &flags)?;
                        (rust_str(&text), exact)
                    }
                    _ => return Err(format!("this `{name}()` value is not converted")),
                };
                let method = if exact { exact_method } else { contains_method };
                Ok(code(format!(
                    "page.expect(){timeout}{not}.{method}({text}).await?;"
                )))
            }
            // jest-puppeteer's matchers on the page.
            "toMatchElement" | "toClick" | "toFill" | "toSelect" | "toUploadFile" => {
                let mut steps = self.selector(args.need(0, "the selector")?)?;
                let options_at = if name == "toFill" || name == "toSelect" || name == "toUploadFile"
                {
                    2
                } else {
                    1
                };
                let options = args.options(options_at)?;
                check_options(name, &options, &["text", "visible", "timeout"])?;
                if let Some(text) = option(&options, "text") {
                    steps.push(Step::HasText(self.literal_text(text, "text")?));
                }
                let locator = self.chain("page", &steps)?;
                let locator = self.with_timeout(&locator, &options)?;
                if negate && name != "toMatchElement" {
                    return Err(format!("`not.{name}()` is not converted"));
                }
                match name {
                    "toMatchElement" => {
                        let visible = option(&options, "visible")
                            .map(|v| self.boolean(v))
                            .transpose()?
                            .unwrap_or(false);
                        let method = if visible { "to_be_visible" } else { "to_exist" };
                        Ok(code(format!("{locator}.expect(){not}.{method}().await?;")))
                    }
                    "toClick" => Ok(code(format!("{locator}.click().await?;"))),
                    "toFill" => {
                        let text = self.text(args.need(1, "the text")?)?;
                        Ok(code(format!("{locator}.fill({text}).await?;")))
                    }
                    "toSelect" => {
                        let wanted = self.text(args.need(1, "the option")?)?;
                        self.needs.helpers.insert(Helper::SelectOption);
                        Ok(code(format!(
                            "support::select_option(&{locator}, {wanted}).await?;"
                        )))
                    }
                    _ => {
                        let path = self.literal_text(args.need(1, "the file")?, "file path")?;
                        self.needs.helpers.insert(Helper::SetInputFiles);
                        Ok(code(format!(
                            "support::set_input_files(&{locator}, {}).await?;",
                            rust_str(&path)
                        )))
                    }
                }
            }
            "toMatchTextContent" | "toMatch" => {
                let options = args.options(1)?;
                check_options(name, &options, &["timeout"])?;
                let text = match self.eval(args.need(0, "the text")?)? {
                    Ev::Lit(Lit::Str(text)) => text,
                    Ev::Lit(Lit::Regex { pattern, flags }) => regex_text(&pattern, &flags)?.0,
                    _ => return Err(format!("this `{name}()` value is not converted")),
                };
                let locator = self.with_timeout("page.locator(\"body\")", &options)?;
                Ok(code(format!(
                    "{locator}.expect(){not}.to_contain_text({}).await?;",
                    rust_str(&text)
                )))
            }
            _ => Err(format!("`expect(page).{name}()` is not converted")),
        }
    }

    fn expect_timeout(&mut self, options: &[(String, &Expr)]) -> TResult<String> {
        match option(options, "timeout") {
            Some(timeout) => {
                let ms = self.whole_number(timeout, "the timeout")?;
                Ok(format!(".timeout(std::time::Duration::from_millis({ms}))"))
            }
            None => Ok(String::new()),
        }
    }

    fn locator_matcher(
        &mut self,
        locator: &str,
        negate: bool,
        name: &str,
        args: &Args<'_>,
    ) -> TResult<Ev> {
        let mut negate = negate;
        // A state matcher's own option, `{ visible: false }` and the like,
        // turns it round.
        let flip = |this: &mut Self, options: &[(String, &Expr)], key: &str| -> TResult<bool> {
            Ok(match option(options, key) {
                Some(value) => !this.boolean(value)?,
                None => false,
            })
        };
        let state = |this: &mut Self, negate: bool, key: &str, method: &str| -> TResult<Ev> {
            let options = args.options(0)?;
            check_options(name, &options, &[key, "timeout"])?;
            let negate = negate != flip(this, &options, key)?;
            let timeout = this.expect_timeout(&options)?;
            let not = if negate { ".not()" } else { "" };
            Ok(code(format!(
                "{locator}.expect(){timeout}{not}.{method}().await?;"
            )))
        };
        match name {
            "toBeVisible" => state(self, negate, "visible", "to_be_visible"),
            "toBeHidden" => state(self, negate, "hidden", "to_be_hidden"),
            "toBeAttached" => state(self, negate, "attached", "to_exist"),
            "toBeChecked" => state(self, negate, "checked", "to_be_checked"),
            "toHaveText" | "toContainText" => {
                let options = args.options(1)?;
                check_options(name, &options, &["timeout", "useInnerText"])?;
                let timeout = self.expect_timeout(&options)?;
                let (text, exact) = match self.eval(args.need(0, "the expected text")?)? {
                    Ev::Lit(Lit::Str(text)) => (rust_str(&text), name == "toHaveText"),
                    Ev::Value(code_text, Ty::Str) => {
                        (format!("&{code_text}"), name == "toHaveText")
                    }
                    Ev::Lit(Lit::Regex { pattern, flags }) => {
                        let (text, exact) = regex_text(&pattern, &flags)?;
                        (rust_str(&text), exact)
                    }
                    _ => return Err(format!("this `{name}()` value is not converted")),
                };
                let method = if exact {
                    "to_have_text"
                } else {
                    "to_contain_text"
                };
                let not = if negate { ".not()" } else { "" };
                Ok(code(format!(
                    "{locator}.expect(){timeout}{not}.{method}({text}).await?;"
                )))
            }
            "toHaveValue" => {
                let options = args.options(1)?;
                check_options(name, &options, &["timeout"])?;
                let target = self.with_timeout(locator, &options)?;
                let value = match self.eval(args.need(0, "the expected value")?)? {
                    Ev::Lit(Lit::Str(text)) => rust_str(&text),
                    Ev::Value(code_text, Ty::Str) => format!("&{code_text}"),
                    Ev::Lit(Lit::Regex { pattern, flags }) => {
                        match regex_text(&pattern, &flags)? {
                            (text, true) => rust_str(&text),
                            (_, false) => return Err(
                                "toHaveValue() with a partial regular expression is not converted"
                                    .to_owned(),
                            ),
                        }
                    }
                    _ => return Err("this toHaveValue() value is not converted".to_owned()),
                };
                self.needs.helpers.insert(Helper::ExpectFieldValue);
                Ok(code(format!(
                    "support::expect_field_value(&{target}, {value}, {negate}).await?;"
                )))
            }
            "toHaveAttribute" | "toHaveId" => {
                let (attribute, value_at) = if name == "toHaveId" {
                    (rust_str("id"), 0)
                } else {
                    (self.text(args.need(0, "the attribute name")?)?, 1)
                };
                let value = match self.eval(args.need(value_at, "the expected value")?)? {
                    Ev::Lit(Lit::Str(text)) => rust_str(&text),
                    Ev::Value(code_text, Ty::Str) => format!("&{code_text}"),
                    Ev::Lit(Lit::Regex { pattern, flags }) => match regex_text(&pattern, &flags)? {
                        (text, true) => rust_str(&text),
                        (_, false) => {
                            return Err(format!(
                                "`{name}()` with a partial regular expression is not converted"
                            ))
                        }
                    },
                    _ => return Err(format!("this `{name}()` value is not converted")),
                };
                let options = args.options(value_at + 1)?;
                check_options(name, &options, &["timeout"])?;
                let timeout = self.expect_timeout(&options)?;
                let not = if negate { ".not()" } else { "" };
                Ok(code(format!(
                    "{locator}.expect(){timeout}{not}.to_have_attribute({attribute}, {value}).await?;"
                )))
            }
            "toBeEnabled" | "toBeDisabled" => {
                let options = args.options(0)?;
                check_options(name, &options, &["timeout", "enabled"])?;
                if flip(self, &options, "enabled")? {
                    negate = !negate;
                }
                let target = self.with_timeout(locator, &options)?;
                let enabled = (name == "toBeEnabled") != negate;
                self.needs.helpers.insert(Helper::ExpectEnabled);
                Ok(code(format!(
                    "support::expect_enabled(&{target}, {enabled}).await?;"
                )))
            }
            "toHaveCount" => {
                let n = self.whole_number(args.need(0, "the count")?, "the count")?;
                let options = args.options(1)?;
                check_options(name, &options, &["timeout"])?;
                let timeout = self.expect_timeout(&options)?;
                let not = if negate { ".not()" } else { "" };
                Ok(code(format!(
                    "{locator}.expect(){timeout}{not}.to_have_count({n}).await?;"
                )))
            }
            _ => Err(format!("`expect(locator).{name}()` is not converted")),
        }
    }
}

fn code(line: String) -> Ev {
    Ev::Lines(vec![Line::Code(line)])
}

fn code_line(line: String) -> Ev {
    code(line)
}

/// `assert!(condition)`, or `assert!(!(condition))` when `negate`.
fn assert_line(condition: &str, negate: bool) -> String {
    if negate {
        format!("assert!(!({condition}));")
    } else {
        format!("assert!({condition});")
    }
}

/// `assert_eq!` or, when `negate`, `assert_ne!`.
fn eq_line(actual: &str, expected: &str, negate: bool) -> String {
    let check = if negate { "assert_ne" } else { "assert_eq" };
    format!("{check}!({actual}, {expected});")
}

/// Renames `let x = ...;` to `let _x = ...;` when nothing after it uses `x`.
fn mark_unused(lines: &mut [Line]) {
    for index in 0..lines.len() {
        let Line::Code(code) = &lines[index] else {
            continue;
        };
        let Some(rest) = code.strip_prefix("let ") else {
            continue;
        };
        let Some((name, _)) = rest.split_once(" = ") else {
            continue;
        };
        if name == "page" || name.starts_with('_') {
            continue;
        }
        let name = name.to_owned();
        let used = lines[index + 1..].iter().any(|line| match line {
            Line::Code(later) => super::emit::mentions(later, &name),
            _ => false,
        });
        if !used {
            if let Line::Code(code) = &mut lines[index] {
                *code = format!("let _{}", &code["let ".len()..]);
            }
        }
    }
}
