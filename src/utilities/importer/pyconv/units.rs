//! Finding the tests in a Python file: classes with test methods, test
//! functions, `with SB(...)` blocks and scripts.

use super::ast::{BinOp, ClassDef, Expr, FuncDef, Stmt, StmtKind};
use super::stmt::is_chrome_constructor;
use super::tr::{py_repr, Ty};

/// The rows of a parameterized test.
pub(super) struct Cases {
    /// The parameter names.
    pub(super) names: Vec<String>,
    /// The types of the parameters.
    pub(super) types: Vec<Ty>,
    /// The Rust type of each parameter.
    pub(super) rust_types: Vec<&'static str>,
    /// The Rust literals of each row.
    pub(super) rows: Vec<Vec<String>>,
}

/// One test to write.
pub(super) struct Unit {
    /// The Python name (a test method or function), empty for a script.
    pub(super) name: String,
    /// The line of its `def` (or first statement).
    pub(super) line: usize,
    /// Its statements.
    pub(super) body: Vec<Stmt>,
    /// What runs first (`setUp`, module-level assignments).
    pub(super) prologue: Vec<Stmt>,
    /// What runs last (`tearDown`).
    pub(super) epilogue: Vec<Stmt>,
    /// Its parameter rows.
    pub(super) cases: Option<Cases>,
    /// `SB(...)` as written, for a comment.
    pub(super) sb_args: Option<String>,
    /// Names, besides `self` and `sb`, that mean the browser.
    pub(super) receivers: Vec<String>,
    /// It drives the pure CDP engine.
    pub(super) cdp: bool,
}

/// What the module holds.
pub(super) struct Found {
    /// The module docstring.
    pub(super) doc: Option<String>,
    /// The tests.
    pub(super) units: Vec<Unit>,
    /// What was not converted at module level: line and reason.
    pub(super) todos: Vec<(usize, String)>,
}

/// Finds the tests in the statements of a module.
pub(super) fn discover(stmts: &[Stmt]) -> Found {
    let mut found = Found {
        doc: None,
        units: Vec::new(),
        todos: Vec::new(),
    };
    let mut rest = stmts;
    if let Some(first) = stmts.first() {
        if let StmtKind::Expr(Expr::Str(text)) = &first.kind {
            found.doc = Some(text.clone());
            rest = &stmts[1..];
        }
    }
    let mut loose: Vec<Stmt> = Vec::new();
    for stmt in rest {
        match &stmt.kind {
            StmtKind::Import(_) | StmtKind::Pass => {}
            StmtKind::Expr(Expr::Call(func, _))
                if func
                    .dotted()
                    .is_some_and(|path| path == ["BaseCase", "main"]) => {}
            StmtKind::If { test, .. } if is_main_check(test) => {}
            StmtKind::ClassDef(class) => class_units(class, stmt.line, &mut found),
            StmtKind::FuncDef(def) => function_unit(def, stmt.line, &mut found),
            StmtKind::With { items, body } if sb_context(items).is_some() => {
                let (args, receiver) = sb_context(items).unwrap_or_default();
                found.units.push(script_unit(
                    String::new(),
                    stmt.line,
                    body.clone(),
                    Some(args),
                    receiver,
                ));
            }
            _ => loose.push(stmt.clone()),
        }
    }
    attach_loose(loose, &mut found);
    found
}

/// Statements outside any test: a script of their own when nothing else is a
/// test, else constants for every test.
fn attach_loose(loose: Vec<Stmt>, found: &mut Found) {
    if loose.is_empty() {
        return;
    }
    if found.units.is_empty() {
        let line = loose.first().map_or(1, |stmt| stmt.line);
        let (body, args, receiver) = unwrap_sb_with(loose);
        found
            .units
            .push(script_unit(String::new(), line, body, args, receiver));
        return;
    }
    let mut constants = Vec::new();
    for stmt in loose {
        match stmt.kind {
            StmtKind::Assign { .. } | StmtKind::AnnAssign { .. } => constants.push(stmt),
            _ => found.todos.push((
                stmt.line,
                "this statement outside a test is not converted".to_owned(),
            )),
        }
    }
    for unit in &mut found.units {
        let mut prologue = constants.clone();
        prologue.append(&mut unit.prologue);
        unit.prologue = prologue;
    }
}

/// `if __name__ == "__main__"`.
fn is_main_check(test: &Expr) -> bool {
    match test {
        Expr::Binary(left, BinOp::Eq, right) => {
            let is_name = |expr: &Expr| matches!(expr, Expr::Name(name) if name == "__name__");
            let is_main = |expr: &Expr| matches!(expr, Expr::Str(text) if text == "__main__");
            (is_name(left) && is_main(right)) || (is_name(right) && is_main(left))
        }
        _ => false,
    }
}

/// `SB(...) as sb`: the call as written and the name it binds.
fn sb_context(items: &[(Expr, Option<Expr>)]) -> Option<(String, Vec<String>)> {
    let [(context @ Expr::Call(func, _), target)] = items else {
        return None;
    };
    if !matches!(&**func, Expr::Name(name) if name == "SB") {
        return None;
    }
    let receivers = match target {
        Some(Expr::Name(name)) => vec![name.clone()],
        _ => Vec::new(),
    };
    Some((py_repr(context), receivers))
}

/// A body that is one `with SB(...)` block gives that block's statements.
fn unwrap_sb_with(stmts: Vec<Stmt>) -> (Vec<Stmt>, Option<String>, Vec<String>) {
    let significant: Vec<&Stmt> = stmts
        .iter()
        .filter(|stmt| !matches!(stmt.kind, StmtKind::Pass | StmtKind::Import(_)))
        .collect();
    if let [only] = significant.as_slice() {
        if let StmtKind::With { items, body } = &only.kind {
            if let Some((args, receivers)) = sb_context(items) {
                return (body.clone(), Some(args), receivers);
            }
        }
    }
    (stmts, None, Vec::new())
}

fn script_unit(
    name: String,
    line: usize,
    body: Vec<Stmt>,
    sb_args: Option<String>,
    receivers: Vec<String>,
) -> Unit {
    let cdp = uses_cdp(&body);
    Unit {
        name,
        line,
        body,
        prologue: Vec::new(),
        epilogue: Vec::new(),
        cases: None,
        sb_args,
        receivers,
        cdp,
    }
}

/// The test methods of a class, with its `setUp` and `tearDown`.
fn class_units(class: &ClassDef, line: usize, found: &mut Found) {
    let mut prologue = Vec::new();
    let mut epilogue = Vec::new();
    for stmt in &class.body {
        if let StmtKind::FuncDef(def) = &stmt.kind {
            match def.name.as_str() {
                "setUp" | "setup" | "setup_method" => prologue.extend(def.body.clone()),
                "tearDown" | "teardown" | "teardown_method" => epilogue.extend(def.body.clone()),
                _ => {}
            }
        }
    }
    let mut tests = 0;
    for stmt in &class.body {
        match &stmt.kind {
            StmtKind::FuncDef(def) if def.name.starts_with("test") => {
                tests += 1;
                let mut unit = method_unit(def, stmt.line, found);
                if let Some(unit) = unit.as_mut() {
                    unit.prologue = prologue.clone();
                    unit.epilogue = epilogue.clone();
                }
                if let Some(unit) = unit {
                    found.units.push(unit);
                }
            }
            StmtKind::FuncDef(def)
                if !matches!(
                    def.name.as_str(),
                    "setUp"
                        | "setup"
                        | "setup_method"
                        | "tearDown"
                        | "teardown"
                        | "teardown_method"
                ) =>
            {
                found.todos.push((
                    stmt.line,
                    format!("the helper method '{}' needs manual migration", def.name),
                ));
            }
            StmtKind::Assign { .. } | StmtKind::AnnAssign { .. } => found.todos.push((
                stmt.line,
                format!("the class attribute in '{}' is not converted", class.name),
            )),
            _ => {}
        }
    }
    let _ = (line, tests);
}

/// A test method as a unit.
fn method_unit(def: &FuncDef, line: usize, found: &mut Found) -> Option<Unit> {
    let params: Vec<String> = def
        .params
        .iter()
        .filter(|param| !param.starred)
        .map(|param| param.name.clone())
        .filter(|name| name != "self" && name != "sb")
        .collect();
    let cases = match cases_of(&def.decorators, &params) {
        Ok(cases) => cases,
        Err(message) => {
            found
                .todos
                .push((line, format!("test '{}': {message}", def.name)));
            return None;
        }
    };
    for decorator in &def.decorators {
        if !is_parameterization(decorator) {
            found.todos.push((
                line,
                format!("the decorator @{} is not converted", py_repr(decorator)),
            ));
        }
    }
    let mut body = def.body.clone();
    strip_docstring(&mut body);
    let cdp = uses_cdp(&body);
    Some(Unit {
        name: def.name.clone(),
        line,
        body,
        prologue: Vec::new(),
        epilogue: Vec::new(),
        cases,
        sb_args: None,
        receivers: Vec::new(),
        cdp,
    })
}

/// A function outside a class: a test, a script, or a helper.
fn function_unit(def: &FuncDef, line: usize, found: &mut Found) {
    let is_test = def.name.starts_with("test");
    let (body, sb_args, receivers) = unwrap_sb_with(def.body.clone());
    let makes_browser = sb_args.is_some() || uses_cdp(&body);
    if !is_test && !makes_browser {
        found.todos.push((
            line,
            format!("the function '{}' needs manual migration", def.name),
        ));
        return;
    }
    let mut unit = if is_test {
        match method_unit(def, line, found) {
            Some(unit) => unit,
            None => return,
        }
    } else {
        let mut body = body.clone();
        strip_docstring(&mut body);
        script_unit(
            def.name.clone(),
            line,
            body,
            sb_args.clone(),
            receivers.clone(),
        )
    };
    if is_test && sb_args.is_some() {
        unit.body = body;
        unit.sb_args = sb_args;
        unit.cdp = uses_cdp(&unit.body);
    }
    unit.receivers.extend(receivers);
    found.units.push(unit);
}

/// Removes a leading docstring.
fn strip_docstring(body: &mut Vec<Stmt>) {
    if matches!(
        body.first().map(|stmt| &stmt.kind),
        Some(StmtKind::Expr(Expr::Str(_)))
    ) {
        body.remove(0);
    }
}

/// Whether a decorator makes rows of cases.
fn is_parameterization(decorator: &Expr) -> bool {
    let Expr::Call(func, _) = decorator else {
        return false;
    };
    func.dotted().is_some_and(|path| {
        path == ["parameterized", "expand"] || path == ["pytest", "mark", "parametrize"]
    })
}

/// The rows of `@parameterized.expand([...])` or
/// `@pytest.mark.parametrize("names", [...])`.
fn cases_of(decorators: &[Expr], params: &[String]) -> Result<Option<Cases>, String> {
    for decorator in decorators {
        if !is_parameterization(decorator) {
            continue;
        }
        let Expr::Call(func, args) = decorator else {
            continue;
        };
        let pytest = func
            .dotted()
            .is_some_and(|path| path == ["pytest", "mark", "parametrize"]);
        let positional: Vec<&Expr> = args
            .iter()
            .filter_map(|arg| match arg {
                super::ast::Arg::Pos(value) => Some(value),
                _ => None,
            })
            .collect();
        let (names, table) = if pytest {
            let [names, table, ..] = positional.as_slice() else {
                return Err("the parametrize call is not read".to_owned());
            };
            let names: Vec<String> = match names {
                Expr::Str(text) => text.split(',').map(|name| name.trim().to_owned()).collect(),
                Expr::List(items) | Expr::Tuple(items) => items
                    .iter()
                    .filter_map(|item| match item {
                        Expr::Str(text) => Some(text.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => return Err("the parametrize names are not read".to_owned()),
            };
            (names, *table)
        } else {
            let [table, ..] = positional.as_slice() else {
                return Err("the parameterized.expand call is not read".to_owned());
            };
            (params.to_vec(), *table)
        };
        let (Expr::List(rows) | Expr::Tuple(rows)) = table else {
            return Err("the cases are not a list written out".to_owned());
        };
        return build_cases(&names, rows).map(Some);
    }
    Ok(None)
}

/// The Rust literal and type of a case value.
fn case_literal(expr: &Expr) -> Option<(String, Ty, &'static str)> {
    match expr {
        Expr::Str(text) => Some((format!("{text:?}"), Ty::Str, "&str")),
        Expr::Num { text, float: false } => Some((text.clone(), Ty::Int, "i64")),
        Expr::Num { text, float: true } => Some((text.clone(), Ty::Float, "f64")),
        Expr::Bool(value) => Some((value.to_string(), Ty::Bool, "bool")),
        Expr::Unary(super::ast::UnOp::Neg, inner) => {
            let (code, ty, rust) = case_literal(inner)?;
            matches!(ty, Ty::Int | Ty::Float).then(|| (format!("-{code}"), ty, rust))
        }
        _ => None,
    }
}

fn build_cases(names: &[String], rows: &[Expr]) -> Result<Cases, String> {
    let mut table: Vec<Vec<String>> = Vec::new();
    let mut types: Vec<Ty> = Vec::new();
    let mut rust_types: Vec<&'static str> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let cells: Vec<&Expr> = match row {
            Expr::List(items) | Expr::Tuple(items) if names.len() != 1 || items.len() == 1 => {
                items.iter().collect()
            }
            single => vec![single],
        };
        if cells.len() != names.len() {
            return Err(format!(
                "case {} has {} values for {} parameters",
                index + 1,
                cells.len(),
                names.len()
            ));
        }
        let mut literals = Vec::new();
        for (column, cell) in cells.into_iter().enumerate() {
            let Some((code, ty, rust)) = case_literal(cell) else {
                return Err("a case value is not a literal".to_owned());
            };
            if index == 0 {
                types.push(ty);
                rust_types.push(rust);
            } else if types.get(column) != Some(&ty) {
                return Err("the cases mix types in one column".to_owned());
            }
            literals.push(code);
        }
        table.push(literals);
    }
    if table.is_empty() {
        return Err("there are no cases".to_owned());
    }
    Ok(Cases {
        names: names.to_vec(),
        types,
        rust_types,
        rows: table,
    })
}

/// Whether the statements make a pure CDP browser or call CDP Mode.
pub(super) fn uses_cdp(stmts: &[Stmt]) -> bool {
    let mut found = false;
    visit_stmts(stmts, &mut |expr| match expr {
        Expr::Call(func, _) => {
            if is_chrome_constructor(expr) {
                found = true;
            }
            if matches!(&**func, Expr::Attr(_, method) if method == "activate_cdp_mode") {
                found = true;
            }
        }
        Expr::Attr(base, name) if name == "cdp" && matches!(&**base, Expr::Name(_)) => {
            found = true;
        }
        _ => {}
    });
    found
}

/// Calls `visit` on every expression of the statements, outermost first.
fn visit_stmts(stmts: &[Stmt], visit: &mut dyn FnMut(&Expr)) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Expr(expr) => visit_expr(expr, visit),
            StmtKind::Assign { targets, value } => {
                for target in targets {
                    visit_expr(target, visit);
                }
                visit_expr(value, visit);
            }
            StmtKind::AugAssign { target, value, .. } => {
                visit_expr(target, visit);
                visit_expr(value, visit);
            }
            StmtKind::AnnAssign { target, value } => {
                visit_expr(target, visit);
                if let Some(value) = value {
                    visit_expr(value, visit);
                }
            }
            StmtKind::Return(Some(value)) | StmtKind::Raise(Some(value)) => {
                visit_expr(value, visit);
            }
            StmtKind::Assert { test, message } => {
                visit_expr(test, visit);
                if let Some(message) = message {
                    visit_expr(message, visit);
                }
            }
            StmtKind::If { test, body, orelse } | StmtKind::While { test, body, orelse } => {
                visit_expr(test, visit);
                visit_stmts(body, visit);
                visit_stmts(orelse, visit);
            }
            StmtKind::For {
                target,
                iter,
                body,
                orelse,
            } => {
                visit_expr(target, visit);
                visit_expr(iter, visit);
                visit_stmts(body, visit);
                visit_stmts(orelse, visit);
            }
            StmtKind::Try {
                body,
                handlers,
                orelse,
                finalbody,
            } => {
                visit_stmts(body, visit);
                for handler in handlers {
                    visit_stmts(&handler.body, visit);
                }
                visit_stmts(orelse, visit);
                visit_stmts(finalbody, visit);
            }
            StmtKind::With { items, body } => {
                for (context, target) in items {
                    visit_expr(context, visit);
                    if let Some(target) = target {
                        visit_expr(target, visit);
                    }
                }
                visit_stmts(body, visit);
            }
            StmtKind::FuncDef(def) => visit_stmts(&def.body, visit),
            StmtKind::ClassDef(class) => visit_stmts(&class.body, visit),
            _ => {}
        }
    }
}

fn visit_expr(expr: &Expr, visit: &mut dyn FnMut(&Expr)) {
    visit(expr);
    match expr {
        Expr::Attr(base, _) => visit_expr(base, visit),
        Expr::Call(func, args) => {
            visit_expr(func, visit);
            for arg in args {
                match arg {
                    super::ast::Arg::Pos(value) | super::ast::Arg::Kw(_, value) => {
                        visit_expr(value, visit);
                    }
                    super::ast::Arg::Spread(_) => {}
                }
            }
        }
        Expr::Index(a, b) | Expr::Binary(a, _, b) => {
            visit_expr(a, visit);
            visit_expr(b, visit);
        }
        Expr::Unary(_, operand) => visit_expr(operand, visit),
        Expr::IfExp(a, b, c) => {
            visit_expr(a, visit);
            visit_expr(b, visit);
            visit_expr(c, visit);
        }
        Expr::List(items) | Expr::Tuple(items) => {
            for item in items {
                visit_expr(item, visit);
            }
        }
        Expr::Dict(pairs) => {
            for (key, value) in pairs {
                visit_expr(key, visit);
                visit_expr(value, visit);
            }
        }
        Expr::FStr(parts) => {
            for part in parts {
                if let super::ast::FPart::Expr(value) = part {
                    visit_expr(value, visit);
                }
            }
        }
        _ => {}
    }
}
