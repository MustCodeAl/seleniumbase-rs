//! Finding the tests in a file: `test()` and `it()` calls inside `describe()`
//! groups with their hooks, pytest functions, and scripts (an async IIFE, a
//! `main()` the file calls, codegen's `run(playwright)`, or top-level code).
//!
//! The result is a list of cases, each with the statements to translate in
//! order (setup: the declarations and `before` hooks of its groups; body;
//! teardown: the `after` hooks), plus the statements outside any test that
//! become file-level `TODO`s.

use std::collections::{HashMap, HashSet};

use super::parser::{Arg, Expr, FnBody, Function, Pattern, Stmt, StmtKind};
use super::translate::Obj;

/// How a test gets its page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PageBinding {
    /// A fixture parameter with this name (`async ({ page }) =>`, `def test_x(page)`).
    Name(String),
    /// A global `page` (jest-puppeteer, jest-playwright).
    Global,
    /// The code opens it (`browser.newPage()`).
    Opened,
}

/// One test to generate.
#[derive(Clone, Debug)]
pub(super) struct Case {
    /// The group titles and the test title; empty for a script.
    pub(super) title: Vec<String>,
    /// Why it is ignored, if the source skipped it.
    pub(super) skip: Option<String>,
    pub(super) page: PageBinding,
    /// Other names to bind first: fixtures such as `context`, or a script
    /// function's `playwright` parameter.
    pub(super) objects: Vec<(String, Obj)>,
    pub(super) setup: Vec<Stmt>,
    pub(super) body: Vec<Stmt>,
    pub(super) teardown: Vec<Stmt>,
}

/// The cases of a file and its statements outside any test.
#[derive(Debug, Default)]
pub(super) struct Structure {
    pub(super) cases: Vec<Case>,
    pub(super) todos: Vec<(Stmt, String)>,
}

/// What a top-level or group-level statement is.
enum Role<'a> {
    Test {
        title: Option<String>,
        func: Option<&'a Function>,
        skip: Option<String>,
    },
    Describe {
        title: Option<String>,
        func: Option<&'a Function>,
        skip: Option<String>,
    },
    Before(Option<&'a Function>),
    After(Option<&'a Function>),
    /// Configuration that means nothing for the generated test.
    Dropped,
    /// `test.use(...)`.
    Use,
    Other,
}

/// `a.b.c` as `("a", ["b", "c"])`.
fn member_path(expr: &Expr) -> Option<(&str, Vec<&str>)> {
    let mut props = Vec::new();
    let mut current = expr;
    loop {
        match current {
            Expr::Member { object, property } => {
                props.push(property.as_str());
                current = object;
            }
            Expr::Ident(name) => {
                props.reverse();
                return Some((name.as_str(), props));
            }
            _ => return None,
        }
    }
}

fn last_function(args: &[Arg]) -> Option<&Function> {
    args.iter().rev().find_map(|arg| match arg {
        Arg::Pos(Expr::Function(func)) => Some(func.as_ref()),
        _ => None,
    })
}

fn title_of(args: &[Arg]) -> Option<String> {
    match args.first() {
        Some(Arg::Pos(Expr::Str(title))) => Some(title.clone()),
        _ => None,
    }
}

fn role(stmt: &Stmt) -> Role<'_> {
    let StmtKind::Expr(expr) = &stmt.kind else {
        return Role::Other;
    };
    let expr = match expr {
        Expr::Await(inner) => inner.as_ref(),
        other => other,
    };
    let Expr::Call { callee, args } = expr else {
        return Role::Other;
    };
    let Some((root, props)) = member_path(callee) else {
        return Role::Other;
    };
    let first = props.first().copied();
    let skipped = props.iter().any(|p| matches!(*p, "skip" | "fixme"));
    let skip_reason = |what: &str| skipped.then(|| format!("skipped with {what} in the source"));
    let func = last_function(args);
    let title = title_of(args);
    match (root, first) {
        ("test", Some("describe")) => {
            if props.contains(&"configure") {
                Role::Dropped
            } else {
                Role::Describe {
                    title,
                    func,
                    skip: skip_reason("test.describe.skip"),
                }
            }
        }
        ("test", Some("beforeEach" | "beforeAll")) => Role::Before(func),
        ("test", Some("afterEach" | "afterAll")) => Role::After(func),
        ("test", Some("use")) => Role::Use,
        ("test", Some("setTimeout" | "slow")) if func.is_none() => Role::Dropped,
        (
            "test" | "it" | "specify",
            None | Some("only" | "skip" | "fixme" | "fail" | "slow" | "concurrent"),
        ) => {
            if root == "test" && first == Some("fail") {
                return Role::Test {
                    title,
                    func,
                    skip: Some("test.fail in the source: it is expected to fail".to_owned()),
                };
            }
            Role::Test {
                title,
                func,
                skip: skip_reason(&format!("{root}.skip")),
            }
        }
        ("xit" | "xtest" | "xspecify", _) => Role::Test {
            title,
            func,
            skip: Some(format!("skipped with {root} in the source")),
        },
        ("fit", _) => Role::Test {
            title,
            func,
            skip: None,
        },
        ("describe" | "context" | "suite" | "fdescribe", _) => Role::Describe {
            title,
            func,
            skip: skip_reason("describe.skip"),
        },
        ("xdescribe" | "xcontext", _) => Role::Describe {
            title,
            func,
            skip: Some(format!("skipped with {root} in the source")),
        },
        ("beforeEach" | "beforeAll" | "before", None) => Role::Before(func),
        ("afterEach" | "afterAll" | "after", None) => Role::After(func),
        ("jest", Some("setTimeout" | "retryTimes"))
        | ("this", Some("timeout" | "retries" | "slow")) => Role::Dropped,
        _ => Role::Other,
    }
}

/// The statements of a function body; an expression body becomes one statement.
fn body_of(func: &Function, line: usize) -> Vec<Stmt> {
    match &func.body {
        FnBody::Block(stmts) => stmts.clone(),
        FnBody::Expr(expr) => vec![Stmt {
            line,
            start: func.start,
            end: func.end,
            kind: StmtKind::Expr(expr.as_ref().clone()),
        }],
    }
}

/// Whether the file declares a variable named `page` (mocha's `let page;`),
/// in which case tests without a fixture use that one, not a global.
fn declares_page(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|stmt| match &stmt.kind {
        StmtKind::Decl(declarators) => declarators.iter().any(|d| match &d.pattern {
            Pattern::Name(name) => name == "page",
            Pattern::Destructure(names) => names.iter().any(|n| n == "page"),
        }),
        _ => match role(stmt) {
            Role::Describe {
                func: Some(func), ..
            } => match &func.body {
                FnBody::Block(inner) => declares_page(inner),
                FnBody::Expr(_) => false,
            },
            _ => false,
        },
    })
}

/// The page binding and the other fixtures of a test function.
fn fixtures(func: &Function, page_declared: bool) -> (PageBinding, Vec<(String, Obj)>) {
    let mut objects = Vec::new();
    let mut page = None;
    for param in &func.params {
        let names: Vec<&str> = match &param.name {
            Some(name) => vec![name.as_str()],
            None => param.fields.iter().map(String::as_str).collect(),
        };
        for name in names {
            match name {
                "page" => page = Some(PageBinding::Name(name.to_owned())),
                "context" => objects.push((name.to_owned(), Obj::Context)),
                "browser" => objects.push((name.to_owned(), Obj::Browser)),
                "playwright" => objects.push((name.to_owned(), Obj::Launcher)),
                _ => {}
            }
        }
    }
    let page = page.unwrap_or(if func.params.is_empty() && !page_declared {
        PageBinding::Global
    } else {
        PageBinding::Opened
    });
    (page, objects)
}

#[derive(Clone, Default)]
struct Group {
    titles: Vec<String>,
    setup: Vec<Stmt>,
    teardown: Vec<Stmt>,
    skip: Option<String>,
}

/// The structure of a JavaScript or TypeScript file.
pub(super) fn js(stmts: &[Stmt]) -> Structure {
    let mut out = Structure::default();
    let has_tests = stmts
        .iter()
        .any(|stmt| matches!(role(stmt), Role::Test { .. } | Role::Describe { .. }));
    if has_tests {
        let page_declared = declares_page(stmts);
        walk_group(stmts, &Group::default(), page_declared, &mut out);
    } else {
        js_script(stmts, &mut out);
    }
    out
}

fn walk_group(stmts: &[Stmt], outer: &Group, page_declared: bool, out: &mut Structure) {
    let mut group = outer.clone();
    // Hooks apply to every test of their group, wherever they are written,
    // so they are gathered first. Inner teardown runs before outer teardown.
    let mut teardown = Vec::new();
    for stmt in stmts {
        match role(stmt) {
            Role::Before(Some(func)) => group.setup.extend(body_of(func, stmt.line)),
            Role::After(Some(func)) => teardown.extend(body_of(func, stmt.line)),
            Role::Other => match &stmt.kind {
                StmtKind::Decl(_) | StmtKind::Assign { .. } | StmtKind::Ignored => {
                    group.setup.push(stmt.clone());
                }
                _ => {}
            },
            _ => {}
        }
    }
    teardown.extend(group.teardown.clone());
    group.teardown = teardown;
    for stmt in stmts {
        match role(stmt) {
            Role::Test { title, func, skip } => {
                let (Some(title), Some(func)) = (title, func) else {
                    out.todos.push((
                        stmt.clone(),
                        "a test without a literal title and a function is not converted".to_owned(),
                    ));
                    continue;
                };
                let (page, objects) = fixtures(func, page_declared);
                let mut titles = group.titles.clone();
                titles.push(title);
                out.cases.push(Case {
                    title: titles,
                    skip: skip.or_else(|| group.skip.clone()),
                    page,
                    objects,
                    setup: group.setup.clone(),
                    body: body_of(func, stmt.line),
                    teardown: group.teardown.clone(),
                });
            }
            Role::Describe { title, func, skip } => {
                let Some(func) = func else {
                    out.todos.push((
                        stmt.clone(),
                        "a describe() without a function is not converted".to_owned(),
                    ));
                    continue;
                };
                let mut inner = group.clone();
                inner.titles.push(title.unwrap_or_default());
                inner.skip = skip.or_else(|| group.skip.clone());
                if let FnBody::Block(body) = &func.body {
                    walk_group(body, &inner, page_declared, out);
                }
            }
            Role::Before(None) | Role::After(None) => out.todos.push((
                stmt.clone(),
                "a hook whose function is not written in place is not converted".to_owned(),
            )),
            Role::Use => out.todos.push((
                stmt.clone(),
                "test.use() options are not carried over".to_owned(),
            )),
            Role::Before(Some(_)) | Role::After(Some(_)) | Role::Dropped => {}
            Role::Other => match &stmt.kind {
                StmtKind::Decl(_)
                | StmtKind::Assign { .. }
                | StmtKind::Ignored
                | StmtKind::Comment(_) => {}
                StmtKind::FuncDef { name, .. } => out.todos.push((
                    stmt.clone(),
                    format!("the helper function `{name}` is not converted"),
                )),
                StmtKind::Unsupported(what) => {
                    out.todos
                        .push((stmt.clone(), format!("{what} is not converted")));
                }
                _ => out.todos.push((
                    stmt.clone(),
                    "a statement outside any test is not converted".to_owned(),
                )),
            },
        }
    }
}

/// An immediately called function: `(async () => {...})()`, also with
/// `.catch(...)`, `.then(...)` or `.finally(...)` after it.
fn iife(expr: &Expr) -> Option<&Function> {
    let expr = match expr {
        Expr::Await(inner) | Expr::Unary { operand: inner, .. } => inner.as_ref(),
        other => other,
    };
    match expr {
        Expr::Call { callee, args } if args.is_empty() => match callee.as_ref() {
            Expr::Function(func) => Some(func),
            _ => None,
        },
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Member { object, property }
                if matches!(property.as_str(), "catch" | "then" | "finally") =>
            {
                iife(object)
            }
            _ => None,
        },
        _ => None,
    }
}

/// The name of a top-level function a statement calls: `main()`,
/// `main().catch(...)`, `await run(playwright)`, `asyncio.run(main())`.
fn called_function(expr: &Expr) -> Option<&str> {
    let expr = match expr {
        Expr::Await(inner) => inner.as_ref(),
        other => other,
    };
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    match callee.as_ref() {
        Expr::Ident(name) => Some(name.as_str()),
        Expr::Member { object, property } => match (object.as_ref(), property.as_str()) {
            (Expr::Ident(module), "run") if module == "asyncio" => match args.as_slice() {
                [Arg::Pos(inner)] => called_function(inner),
                _ => None,
            },
            (inner, "catch" | "then" | "finally") => called_function(inner),
            _ => None,
        },
        _ => None,
    }
}

fn js_script(stmts: &[Stmt], out: &mut Structure) {
    let functions: HashMap<&str, &Function> = stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            StmtKind::FuncDef { name, func, .. } => Some((name.as_str(), func)),
            _ => None,
        })
        .collect();
    let mut used = HashSet::new();
    let mut body = Vec::new();
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::FuncDef { .. } | StmtKind::Comment(_) => {}
            StmtKind::Expr(expr) => {
                if let Some(func) = iife(expr) {
                    body.extend(body_of(func, stmt.line));
                } else if let Some((name, func)) = called_function(expr)
                    .and_then(|name| functions.get(name).map(|func| (name, *func)))
                {
                    if used.insert(name) {
                        body.extend(body_of(func, stmt.line));
                    }
                } else {
                    body.push(stmt.clone());
                }
            }
            _ => body.push(stmt.clone()),
        }
    }
    for stmt in stmts {
        if let StmtKind::FuncDef { name, .. } = &stmt.kind {
            if !used.contains(name.as_str()) {
                out.todos.push((
                    stmt.clone(),
                    format!("the helper function `{name}` is not converted"),
                ));
            }
        }
    }
    let meaningful = body
        .iter()
        .any(|stmt| !matches!(stmt.kind, StmtKind::Ignored | StmtKind::Comment(_)));
    if meaningful {
        out.cases.push(Case {
            title: Vec::new(),
            skip: None,
            page: PageBinding::Opened,
            objects: Vec::new(),
            setup: Vec::new(),
            body,
            teardown: Vec::new(),
        });
    }
}

/// Whether the statements open a page (`new_page()` / `newPage()`), looking
/// inside `with` blocks.
fn opens_page(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|stmt| match &stmt.kind {
        StmtKind::Assign { value, .. } => is_new_page(value),
        StmtKind::Decl(declarators) => declarators
            .iter()
            .any(|d| d.init.as_ref().is_some_and(is_new_page)),
        StmtKind::With { items, body } => {
            items.iter().any(|(item, _)| is_new_page(item)) || opens_page(body)
        }
        StmtKind::IfMain(body) => opens_page(body),
        _ => false,
    })
}

fn is_new_page(expr: &Expr) -> bool {
    let expr = match expr {
        Expr::Await(inner) => inner.as_ref(),
        other => other,
    };
    matches!(expr, Expr::Call { callee, .. }
        if matches!(callee.as_ref(), Expr::Member { property, .. } if property == "new_page" || property == "newPage"))
}

/// What a pytest decorator does to a test: skip it, or nothing; `Err` for a
/// decorator that changes the test in a way that is not converted.
fn pytest_decorator(decorator: &Expr) -> Result<Option<String>, String> {
    let (target, args) = match decorator {
        Expr::Call { callee, args } => (callee.as_ref(), args.as_slice()),
        other => (other, &[][..]),
    };
    let Some((root, props)) = member_path(target) else {
        return Err("a decorator that is not converted".to_owned());
    };
    match (root, props.as_slice()) {
        ("pytest", ["mark", "skip"]) => {
            let reason = args.iter().find_map(|arg| match arg {
                Arg::Kw(key, Expr::Str(reason)) if key == "reason" => Some(reason.clone()),
                Arg::Pos(Expr::Str(reason)) => Some(reason.clone()),
                _ => None,
            });
            Ok(Some(reason.unwrap_or_else(|| {
                "skipped with pytest.mark.skip in the source".to_owned()
            })))
        }
        (
            "pytest",
            ["mark", mark @ ("skipif" | "xfail" | "parametrize" | "usefixtures" | "flaky")],
        ) => Err(format!("the pytest.mark.{mark} decorator is not converted")),
        ("pytest", ["mark", ..]) => Ok(None),
        _ => Err("a decorator that is not converted".to_owned()),
    }
}

/// The structure of a Python file.
pub(super) fn python(stmts: &[Stmt]) -> Structure {
    let mut out = Structure::default();
    let functions: HashMap<&str, &Function> = stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            StmtKind::FuncDef { name, func, .. } => Some((name.as_str(), func)),
            _ => None,
        })
        .collect();
    let mut setup = Vec::new();
    let mut loose = Vec::new();
    let mut tests = Vec::new();
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::FuncDef {
                name,
                func,
                decorators,
            } if name.starts_with("test") => tests.push((stmt, name, func, decorators)),
            StmtKind::FuncDef { .. } | StmtKind::Comment(_) | StmtKind::Ignored => {}
            StmtKind::Assign {
                target: Expr::Ident(_),
                value: Expr::Str(_) | Expr::Num(_) | Expr::Bool(_),
            } => setup.push(stmt.clone()),
            _ => loose.push(stmt.clone()),
        }
    }
    let mut used: HashSet<&str> = HashSet::new();
    for (stmt, name, func, decorators) in &tests {
        let mut skip = None;
        for decorator in decorators.iter() {
            match pytest_decorator(decorator) {
                Ok(Some(reason)) => skip = Some(reason),
                Ok(None) => {}
                Err(reason) => out.todos.push(((*stmt).clone(), reason)),
            }
        }
        let (page, objects) = fixtures(func, true);
        out.cases.push(Case {
            title: vec![(*name).clone()],
            skip,
            page,
            objects,
            setup: setup.clone(),
            body: body_of(func, stmt.line),
            teardown: Vec::new(),
        });
        used.insert(name.as_str());
    }
    if tests.is_empty() {
        let mut objects = Vec::new();
        let body = resolve_entries(&loose, &functions, &mut used, &mut objects, 0);
        let meaningful = body
            .iter()
            .any(|stmt| !matches!(stmt.kind, StmtKind::Ignored | StmtKind::Comment(_)));
        if meaningful {
            out.cases.push(Case {
                title: Vec::new(),
                skip: None,
                page: PageBinding::Opened,
                objects,
                setup,
                body,
                teardown: Vec::new(),
            });
        }
    } else {
        for stmt in loose {
            let reason = match &stmt.kind {
                StmtKind::Unsupported(what) => format!("{what} is not converted"),
                _ => "a statement outside any test is not converted".to_owned(),
            };
            out.todos.push((stmt, reason));
        }
    }
    for stmt in stmts {
        if let StmtKind::FuncDef {
            name, decorators, ..
        } = &stmt.kind
        {
            if used.contains(name.as_str()) {
                continue;
            }
            let fixture = decorators.iter().any(|d| {
                member_path(match d {
                    Expr::Call { callee, .. } => callee,
                    other => other,
                })
                .is_some_and(|(root, props)| root == "pytest" && props.first() == Some(&"fixture"))
            });
            let reason = if fixture {
                format!("the pytest fixture `{name}` is not converted")
            } else {
                format!("the helper function `{name}` is not converted")
            };
            out.todos.push((stmt.clone(), reason));
        }
    }
    out
}

/// The statements a script runs: calls to the file's functions are replaced
/// by the body of the function that opens the page (codegen's `run`), and
/// functions that only call others (`main`) are followed through.
fn resolve_entries<'a>(
    stmts: &[Stmt],
    functions: &HashMap<&'a str, &'a Function>,
    used: &mut HashSet<&'a str>,
    objects: &mut Vec<(String, Obj)>,
    depth: usize,
) -> Vec<Stmt> {
    let mut out = Vec::new();
    if depth > 16 {
        return stmts.to_vec();
    }
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::IfMain(body) => {
                out.extend(resolve_entries(body, functions, used, objects, depth + 1));
            }
            StmtKind::With { body, .. } if !opens_page(body) => {
                out.extend(resolve_entries(body, functions, used, objects, depth + 1));
            }
            StmtKind::Expr(expr) => {
                let target = called_function(expr)
                    .and_then(|name| functions.get_key_value(name).map(|(k, f)| (*k, *f)));
                match target {
                    Some((name, func)) if !used.contains(name) => {
                        used.insert(name);
                        let body = body_of(func, stmt.line);
                        if opens_page(&body) {
                            for param in &func.params {
                                if let Some(param) = &param.name {
                                    objects.push((param.clone(), Obj::Launcher));
                                }
                            }
                            out.extend(body);
                        } else {
                            out.extend(resolve_entries(&body, functions, used, objects, depth + 1));
                        }
                    }
                    Some(_) => {}
                    None => out.push(stmt.clone()),
                }
            }
            _ => out.push(stmt.clone()),
        }
    }
    out
}
