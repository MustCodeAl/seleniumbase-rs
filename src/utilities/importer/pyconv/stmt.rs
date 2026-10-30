//! Statements: assignments, loops, conditions, `try` and `with` as Rust.

use super::ast::{Arg, Expr, Handler, Stmt, StmtKind};
use super::expr::bind;
use super::table::{typed, K, TIMEOUT};
use super::tr::{one_line, py_repr, rust_ident, Skip, Tr, Ty};

/// The first line of a statement's source, for a `TODO`.
fn first_line(text: &str) -> String {
    one_line(text.lines().next().unwrap_or_default())
}

/// A statement that ends on its own line (no block of its own).
fn is_simple(kind: &StmtKind) -> bool {
    !matches!(
        kind,
        StmtKind::If { .. }
            | StmtKind::While { .. }
            | StmtKind::For { .. }
            | StmtKind::Try { .. }
            | StmtKind::With { .. }
            | StmtKind::FuncDef(_)
            | StmtKind::ClassDef(_)
    )
}

impl Tr<'_> {
    /// The statements of a block, with the comments between them.
    pub(super) fn stmts(&mut self, list: &[Stmt]) {
        for stmt in list {
            self.flush_comments(stmt.line);
            self.stmt(stmt);
            if is_simple(&stmt.kind) {
                self.trailing_comment(stmt.end_line);
            }
        }
    }

    /// The source of a statement, cut to its first line.
    fn source_of(&self, stmt: &Stmt) -> String {
        self.source
            .get(stmt.span.0..stmt.span.1)
            .map(first_line)
            .unwrap_or_default()
    }

    /// A statement that is not translated: a `TODO` and a diagnostic.
    fn skip_stmt(&mut self, stmt: &Stmt, skip: &Skip) {
        let python = self.source_of(stmt);
        self.notes.clear();
        self.todo(stmt.line, skip, &python);
    }

    /// Reports the arguments the last call dropped.
    fn flush_notes(&mut self, line: usize) {
        for note in std::mem::take(&mut self.notes) {
            self.diag(line, false, note);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.expr_stmt(stmt, expr),
            StmtKind::Assign { targets, value } => self.assign(stmt, targets, value),
            StmtKind::AnnAssign { target, value } => {
                if let Some(value) = value {
                    self.assign(stmt, std::slice::from_ref(target), value);
                }
            }
            StmtKind::AugAssign { target, op, value } => self.aug_assign(stmt, target, op, value),
            StmtKind::Pass | StmtKind::Import(_) => {}
            StmtKind::Break => self.emit("break;"),
            StmtKind::Continue => self.emit("continue;"),
            StmtKind::Return(_) => self.emit("return Ok(());"),
            StmtKind::Assert { test, message } => self.assert_stmt(stmt, test, message.as_ref()),
            StmtKind::Raise(_) => {
                let skip = Skip::warn("`raise` is not translated");
                self.skip_stmt(stmt, &skip);
            }
            StmtKind::If { test, body, orelse } => self.if_chain(stmt.line, test, body, orelse),
            StmtKind::While { test, body, orelse } => {
                self.while_stmt(stmt, test, body, !orelse.is_empty());
            }
            StmtKind::For {
                target,
                iter,
                body,
                orelse,
            } => self.for_stmt(stmt, target, iter, body, !orelse.is_empty()),
            StmtKind::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => self.try_stmt(stmt, body, handlers, orelse, finalbody),
            StmtKind::With { items, body } => self.with_stmt(stmt, items, body),
            StmtKind::FuncDef(def) => {
                let skip = Skip::warn(format!(
                    "the nested function '{}' needs manual migration",
                    def.name
                ));
                self.skip_stmt(stmt, &skip);
            }
            StmtKind::ClassDef(def) => {
                let skip = Skip::warn(format!(
                    "the nested class '{}' needs manual migration",
                    def.name
                ));
                self.skip_stmt(stmt, &skip);
            }
            StmtKind::Other(text) => {
                let skip = Skip::warn(format!("`{}` is not translated", first_line(text)));
                self.skip_stmt(stmt, &skip);
            }
            StmtKind::Unsupported(text) => {
                self.notes.clear();
                let skip = Skip::warn(format!("could not read this statement: {text}"));
                self.todo(stmt.line, &skip, "");
            }
        }
    }

    fn expr_stmt(&mut self, stmt: &Stmt, expr: &Expr) {
        let Expr::Call(func, args) = expr else {
            // A bare string is a docstring; anything else does nothing.
            if !matches!(expr, Expr::Str(_) | Expr::Name(_)) {
                let skip = Skip::warn("this expression statement is not translated");
                self.skip_stmt(stmt, &skip);
            }
            return;
        };
        // `super().setUp()` and `BaseCase.main(..)` have no Rust equal and
        // need none.
        let is_super = matches!(&**func, Expr::Attr(base, _)
            if matches!(&**base, Expr::Call(inner, _) if matches!(&**inner, Expr::Name(n) if n == "super")));
        if is_super
            || func
                .dotted()
                .is_some_and(|path| path == ["BaseCase", "main"])
        {
            return;
        }
        match self.render_any(func, args) {
            Ok(rendered) => {
                if !rendered.code.is_empty() {
                    self.emit(format!("{};", rendered.code));
                }
                self.flush_notes(stmt.line);
            }
            Err(skip) => self.skip_stmt(stmt, &skip),
        }
    }

    fn assign(&mut self, stmt: &Stmt, targets: &[Expr], value: &Expr) {
        match targets {
            [Expr::Name(name)] => {
                // `sb = sb_cdp.Chrome()` makes the browser; the fixture did.
                if is_chrome_constructor(value) {
                    self.receivers.insert(name.clone());
                    return;
                }
                let val = match self.value(value, false) {
                    Ok(val) if val.ty != Ty::Unit => val,
                    Ok(_) => {
                        let skip = Skip::warn("this call gives back nothing to assign");
                        self.skip_stmt(stmt, &skip);
                        return;
                    }
                    Err(skip) => {
                        self.skip_stmt(stmt, &skip);
                        return;
                    }
                };
                self.flush_notes(stmt.line);
                self.bind_name(name, val.code, val.ty);
            }
            [Expr::Tuple(names)] => self.assign_tuple(stmt, names, value),
            _ => {
                let skip = Skip::warn("this assignment is not translated");
                self.skip_stmt(stmt, &skip);
            }
        }
    }

    /// `a, b = x, y`.
    fn assign_tuple(&mut self, stmt: &Stmt, names: &[Expr], value: &Expr) {
        let Expr::Tuple(values) = value else {
            let skip = Skip::warn("a tuple is assigned only from a tuple written out");
            self.skip_stmt(stmt, &skip);
            return;
        };
        if names.len() != values.len() {
            let skip = Skip::error("the names and the values of this assignment differ in number");
            self.skip_stmt(stmt, &skip);
            return;
        }
        let mut bound = Vec::new();
        for (name, value) in names.iter().zip(values) {
            let Expr::Name(name) = name else {
                let skip = Skip::warn("this assignment target is not translated");
                self.skip_stmt(stmt, &skip);
                return;
            };
            match self.value(value, true) {
                Ok(val) => bound.push((name.clone(), val)),
                Err(skip) => {
                    self.skip_stmt(stmt, &skip);
                    return;
                }
            }
        }
        for (name, val) in bound {
            self.bind_name(&name, val.code, val.ty);
        }
    }

    /// `name = code`: a new `let`, or an assignment to a variable of an outer
    /// block (which makes that variable `mut`).
    fn bind_name(&mut self, name: &str, code: String, ty: Ty) {
        let in_current = self
            .scopes
            .last()
            .is_some_and(|scope| scope.contains_key(name));
        let outer = if in_current {
            None
        } else {
            let depth = self.scopes.len();
            self.scopes[..depth.saturating_sub(1)]
                .iter()
                .rev()
                .find_map(|scope| scope.get(name))
                .cloned()
        };
        if let Some(var) = outer {
            if var.ty == ty {
                self.make_mutable(var.line_index);
                self.emit(format!("{} = {code};", var.rust));
                return;
            }
        }
        let rust = self.declare(name, ty);
        self.emit(format!("let {rust} = {code};"));
    }

    fn aug_assign(&mut self, stmt: &Stmt, target: &Expr, op: &str, value: &Expr) {
        let Expr::Name(name) = target else {
            let skip = Skip::warn("this assignment target is not translated");
            self.skip_stmt(stmt, &skip);
            return;
        };
        let Some(var) = self.var(name).cloned() else {
            let skip = Skip::error(format!("the name '{name}' is not defined"));
            self.skip_stmt(stmt, &skip);
            return;
        };
        let val = match self.expr(value) {
            Ok(val) => val,
            Err(skip) => {
                self.skip_stmt(stmt, &skip);
                return;
            }
        };
        let line = match (&var.ty, op) {
            (Ty::String, "+=") if val.ty.is_text() => {
                let text = match val.ty {
                    Ty::Str => val.code.clone(),
                    _ => format!("&{}", val.operand()),
                };
                format!("{}.push_str({text});", var.rust)
            }
            (Ty::Int | Ty::Float, "+=" | "-=" | "*=") if val.ty.is_number() => {
                format!("{} {op} {};", var.rust, val.code)
            }
            _ => {
                let skip = Skip::warn(format!("`{op}` on this value is not translated"));
                self.skip_stmt(stmt, &skip);
                return;
            }
        };
        self.make_mutable(var.line_index);
        self.emit(line);
    }

    fn assert_stmt(&mut self, stmt: &Stmt, test: &Expr, message: Option<&Expr>) {
        let condition = match self.condition(test) {
            Ok(condition) => condition,
            Err(skip) => {
                self.skip_stmt(stmt, &skip);
                return;
            }
        };
        match message {
            None => self.emit(format!("assert!({});", condition.code)),
            Some(message) => match self.expr(message) {
                Ok(message) => {
                    self.emit(format!(
                        "assert!({}, \"{{}}\", {});",
                        condition.code, message.code
                    ));
                }
                Err(skip) => self.skip_stmt(stmt, &skip),
            },
        }
    }

    /// `if`, `elif` and `else`.
    fn if_chain(&mut self, line: usize, test: &Expr, body: &[Stmt], orelse: &[Stmt]) {
        let mut test = test;
        let mut body = body;
        let mut orelse = orelse;
        let mut first = true;
        loop {
            let condition = match self.condition(test) {
                Ok(condition) => condition.code,
                Err(skip) => {
                    // The body is kept but does not run: the condition is not
                    // known.
                    self.todo(line, &skip, &py_repr(test));
                    "false".to_owned()
                }
            };
            if first {
                self.emit(format!("if {condition} {{"));
            } else {
                self.emit(format!("}} else if {condition} {{"));
            }
            first = false;
            self.indented(|tr| tr.stmts(body));
            match orelse {
                [] => {
                    self.emit("}");
                    break;
                }
                [Stmt {
                    kind:
                        StmtKind::If {
                            test: next_test,
                            body: next_body,
                            orelse: next_orelse,
                        },
                    ..
                }] if !self.may_await(next_test) => {
                    test = next_test;
                    body = next_body;
                    orelse = next_orelse;
                }
                other => {
                    self.emit("} else {");
                    self.indented(|tr| tr.stmts(other));
                    self.emit("}");
                    break;
                }
            }
        }
    }

    /// Runs `body` one level deeper, in a scope of its own.
    fn indented(&mut self, body: impl FnOnce(&mut Self)) {
        self.depth += 1;
        self.scopes.push(std::collections::HashMap::new());
        body(self);
        self.scopes.pop();
        self.depth -= 1;
    }

    fn while_stmt(&mut self, stmt: &Stmt, test: &Expr, body: &[Stmt], has_else: bool) {
        if has_else {
            let skip = Skip::warn("the else part of a loop is not translated");
            self.todo(stmt.line, &skip, "");
        }
        if self.may_await(test) {
            self.emit("loop {");
            self.indented(|tr| {
                match tr.condition(test) {
                    Ok(condition) => tr.emit(format!("if !({}) {{ break; }}", condition.code)),
                    Err(skip) => {
                        tr.todo(stmt.line, &skip, &py_repr(test));
                        tr.emit("break;");
                    }
                }
                tr.stmts(body);
            });
            self.emit("}");
            return;
        }
        let condition = match self.condition(test) {
            Ok(condition) => condition.code,
            Err(skip) => {
                self.todo(stmt.line, &skip, &py_repr(test));
                "false".to_owned()
            }
        };
        self.emit(format!("while {condition} {{"));
        self.indented(|tr| tr.stmts(body));
        self.emit("}");
    }

    fn for_stmt(&mut self, stmt: &Stmt, target: &Expr, iter: &Expr, body: &[Stmt], has_else: bool) {
        if has_else {
            let skip = Skip::warn("the else part of a loop is not translated");
            self.todo(stmt.line, &skip, "");
        }
        let header = match self.for_header(target, iter) {
            Ok(header) => header,
            Err(skip) => {
                self.skip_stmt(stmt, &skip);
                return;
            }
        };
        self.emit(format!("for {} in {} {{", header.pattern, header.iterable));
        self.indented(|tr| {
            for (name, ty) in header.names {
                tr.declare(&name, ty);
            }
            tr.stmts(body);
        });
        self.emit("}");
    }

    /// The pattern, the iterable and the names a `for` binds.
    fn for_header(&mut self, target: &Expr, iter: &Expr) -> Result<ForHeader, Skip> {
        // enumerate(items)
        if let Expr::Call(func, args) = iter {
            if let (Expr::Name(function), [Arg::Pos(inner)]) = (&**func, args.as_slice()) {
                if function == "enumerate" {
                    let Expr::Tuple(names) = target else {
                        return Err(Skip::warn("enumerate needs an (index, item) target"));
                    };
                    let [Expr::Name(index), Expr::Name(item)] = names.as_slice() else {
                        return Err(Skip::warn("enumerate needs an (index, item) target"));
                    };
                    let list = self.expr(inner)?;
                    let Ty::List(element) = &list.ty else {
                        return Err(Skip::warn("enumerate is translated on a list"));
                    };
                    let (index_rust, item_rust) = (rust_ident(index), rust_ident(item));
                    return Ok(ForHeader {
                        pattern: format!("({index_rust}, {item_rust})"),
                        iterable: format!("{}.iter().enumerate()", list.operand()),
                        names: vec![
                            (index.clone(), Ty::Int),
                            (item.clone(), (**element).clone()),
                        ],
                    });
                }
                if function == "range" {
                    return self.range_header(target, std::slice::from_ref(inner));
                }
            }
            if let (Expr::Name(function), true) = (&**func, args.len() == 2) {
                if function == "range" {
                    let items: Vec<&Expr> = args
                        .iter()
                        .filter_map(|arg| match arg {
                            Arg::Pos(value) => Some(value),
                            _ => None,
                        })
                        .collect();
                    if items.len() == 2 {
                        return self.range_header_of(target, &items);
                    }
                }
            }
        }
        let Expr::Name(name) = target else {
            return Err(Skip::warn("this loop target is not translated"));
        };
        // A list written out: `for x in ["a", "b"]`.
        if let Expr::List(items) | Expr::Tuple(items) = iter {
            let mut codes = Vec::new();
            for item in items {
                codes.push(self.str_arg(item)?);
            }
            return Ok(ForHeader {
                pattern: rust_ident(name),
                iterable: format!("[{}]", codes.join(", ")),
                names: vec![(name.clone(), Ty::Str)],
            });
        }
        let list = self.expr(iter)?;
        match &list.ty {
            Ty::List(element) => Ok(ForHeader {
                pattern: rust_ident(name),
                iterable: format!("&{}", list.operand()),
                names: vec![(name.clone(), (**element).clone())],
            }),
            _ => Err(Skip::warn("a loop is translated over a list or a range")),
        }
    }

    fn range_header(&mut self, target: &Expr, args: &[Expr]) -> Result<ForHeader, Skip> {
        let items: Vec<&Expr> = args.iter().collect();
        self.range_header_of(target, &items)
    }

    fn range_header_of(&mut self, target: &Expr, args: &[&Expr]) -> Result<ForHeader, Skip> {
        let Expr::Name(name) = target else {
            return Err(Skip::warn("this loop target is not translated"));
        };
        let bounds: Vec<String> = args
            .iter()
            .map(|arg| self.argument(K::Int, arg))
            .collect::<Result<_, _>>()?;
        let iterable = match bounds.as_slice() {
            [end] => format!("0..{end}"),
            [start, end] => format!("{start}..{end}"),
            _ => return Err(Skip::warn("range is translated with one or two bounds")),
        };
        Ok(ForHeader {
            pattern: rust_ident(name),
            iterable,
            names: vec![(name.clone(), Ty::Int)],
        })
    }

    /// `try`/`except` as an async block whose result the handler looks at.
    fn try_stmt(
        &mut self,
        stmt: &Stmt,
        body: &[Stmt],
        handlers: &[Handler],
        orelse: &[Stmt],
        finalbody: &[Stmt],
    ) {
        let attempt = self.temp("attempt");
        self.emit(format!("let {attempt}: Result<()> = async {{"));
        self.indented(|tr| {
            tr.stmts(body);
            tr.emit("Ok(())");
        });
        self.emit("}");
        self.emit(".await;");
        // `}` and `.await;` are one statement: join them for readability.
        let last = self.out.len();
        if last >= 2 {
            if let Some((_, text)) = self.out.pop() {
                if let Some((_, brace)) = self.out.last_mut() {
                    brace.push_str(&text);
                }
            }
        }
        if handlers.len() > 1 {
            let skip =
                Skip::warn("only the first except clause is translated: the others are dropped");
            self.todo(stmt.line, &skip, "");
        }
        match handlers.first() {
            None => {
                self.stmts(finalbody);
                self.emit(format!("{attempt}?;"));
                return;
            }
            Some(handler) => {
                let only_pass = handler
                    .body
                    .iter()
                    .all(|inner| matches!(inner.kind, StmtKind::Pass));
                if !only_pass {
                    let head = match &handler.name {
                        Some(name) => format!("if let Err({}) = &{attempt} {{", rust_ident(name)),
                        None => format!("if {attempt}.is_err() {{"),
                    };
                    self.emit(head);
                    self.indented(|tr| {
                        if let Some(name) = &handler.name {
                            tr.declare(name, Ty::Unknown);
                        }
                        tr.stmts(&handler.body);
                    });
                    self.emit("}");
                }
            }
        }
        if !orelse.is_empty() {
            self.emit(format!("if {attempt}.is_ok() {{"));
            self.indented(|tr| tr.stmts(orelse));
            self.emit("}");
        }
        self.stmts(finalbody);
    }

    /// `with self.frame_switch(x):` as the call builder; other `with` blocks
    /// are kept without the context manager.
    fn with_stmt(&mut self, stmt: &Stmt, items: &[(Expr, Option<Expr>)], body: &[Stmt]) {
        if let [(Expr::Call(func, args), None)] = items {
            if self.receiver_method(func).as_deref() == Some("frame_switch") {
                let params = [typed("frame", K::Frame), TIMEOUT];
                let frame = bind(&params, args)
                    .and_then(|bound| self.rust_arguments(&params, &bound))
                    .map(|rust_args| rust_args.join(", "));
                match frame {
                    Ok(frame) => {
                        self.flush_notes(stmt.line);
                        self.emit(format!("sb.frame_switch({frame})"));
                        self.depth += 1;
                        self.emit(".run(async |sb| {");
                        self.indented(|tr| {
                            tr.stmts(body);
                            tr.emit("Ok(())");
                        });
                        self.emit("})");
                        self.emit(".await?;");
                        self.depth -= 1;
                    }
                    Err(skip) => self.skip_stmt(stmt, &skip),
                }
                return;
            }
        }
        let skip = Skip::warn(
            "this `with` block is not translated: its body follows without the context manager",
        );
        self.skip_stmt(stmt, &skip);
        self.stmts(body);
    }
}

/// What a `for` loop binds.
struct ForHeader {
    /// The pattern after `for`.
    pattern: String,
    /// The iterable after `in`.
    iterable: String,
    /// The Python names the loop binds and their types.
    names: Vec<(String, Ty)>,
}

/// `sb_cdp.Chrome(..)`: the call that makes a pure CDP browser.
pub(super) fn is_chrome_constructor(expr: &Expr) -> bool {
    matches!(expr, Expr::Call(func, _) if func.dotted().is_some_and(|path| path == ["sb_cdp", "Chrome"]))
}
