//! Tests of the Python converter: the parser, and the call tables against
//! the real signatures of `BaseCase` and the rows of `parity/api.toml`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::ast::{BinOp, Expr, StmtKind};
use super::parse::parse_module;
use super::table::{Fill, Flow, Ret, K, TABLE};

// ---------------------------------------------------------------------------
// The call oracle
// ---------------------------------------------------------------------------

/// A method as `src/api` declares it.
struct Sig {
    /// The parameter types, `self` left out.
    params: Vec<String>,
    /// It is an `async fn`.
    asyncness: bool,
    /// The name of its return type (`Result`, `bool`, `()`).
    ret: String,
    /// The name of the first type argument of the return type.
    inner: String,
}

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

/// `impl Into<X>` as `impl Into<X>` for a frame or a window, `impl Into`
/// otherwise.
fn impl_name(implementation: &syn::TypeImplTrait) -> String {
    let argument = implementation.bounds.iter().find_map(|bound| {
        let syn::TypeParamBound::Trait(bound) = bound else {
            return None;
        };
        let segment = bound.path.segments.last()?;
        let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return None;
        };
        match arguments.args.first()? {
            syn::GenericArgument::Type(ty) => Some(type_name(ty)),
            _ => None,
        }
    });
    match argument.as_deref() {
        Some(name @ ("FrameRef" | "WindowRef")) => format!("impl Into<{name}>"),
        _ => "impl Into".to_owned(),
    }
}

fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Reference(reference) => format!("&{}", type_name(&reference.elem)),
        syn::Type::Slice(slice) => format!("[{}]", type_name(&slice.elem)),
        syn::Type::Tuple(tuple) if tuple.elems.is_empty() => "()".to_owned(),
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default(),
        syn::Type::ImplTrait(implementation) => impl_name(implementation),
        _ => "other".to_owned(),
    }
}

/// The name of the first generic argument of a path type: `T` of `Result<T>`.
fn first_argument(ty: &syn::Type) -> String {
    let syn::Type::Path(path) = ty else {
        return String::new();
    };
    let Some(segment) = path.path.segments.last() else {
        return String::new();
    };
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return String::new();
    };
    match arguments.args.first() {
        Some(syn::GenericArgument::Type(ty)) => type_name(ty),
        _ => String::new(),
    }
}

/// Every method of every `impl BaseCase` in `src/api`.
fn basecase_signatures() -> HashMap<String, Sig> {
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api"),
        &mut files,
    );
    let mut methods = HashMap::new();
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        let Ok(syntax) = syn::parse_file(&source) else {
            continue;
        };
        for item in syntax.items {
            let syn::Item::Impl(implementation) = item else {
                continue;
            };
            let is_base_case = implementation.trait_.is_none()
                && matches!(&*implementation.self_ty,
                    syn::Type::Path(path) if path.path.is_ident("BaseCase"));
            if !is_base_case {
                continue;
            }
            for member in implementation.items {
                let syn::ImplItem::Fn(function) = member else {
                    continue;
                };
                let params = function
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|argument| match argument {
                        syn::FnArg::Typed(typed) => Some(type_name(&typed.ty)),
                        syn::FnArg::Receiver(_) => None,
                    })
                    .collect();
                let (ret, inner) = match &function.sig.output {
                    syn::ReturnType::Default => ("()".to_owned(), String::new()),
                    syn::ReturnType::Type(_, ty) => (type_name(ty), first_argument(ty)),
                };
                methods.insert(
                    function.sig.ident.to_string(),
                    Sig {
                        params,
                        asyncness: function.sig.asyncness.is_some(),
                        ret,
                        inner,
                    },
                );
            }
        }
    }
    methods
}

/// A method that is not `async` and gives back a call builder, which is
/// awaited (`sb.switch_to_frame("x").await?`).
fn is_builder(sig: &Sig) -> bool {
    !sig.asyncness
        && !matches!(
            sig.ret.as_str(),
            "Result"
                | "bool"
                | "()"
                | "String"
                | "Option"
                | "Vec"
                | "f64"
                | "u64"
                | "usize"
                | "other"
        )
}

/// Whether the Rust parameter type takes an argument of this kind.
fn kind_fits(kind: K, rust: &str) -> bool {
    match kind {
        K::Str => rust == "&str",
        K::Int => matches!(
            rust,
            "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize"
        ),
        K::Float => rust == "f64" || rust == "impl Into",
        K::Bool => rust == "bool",
        K::Strs => rust == "&[&str]",
        K::Frame => rust == "impl Into<FrameRef>",
        K::Window => rust == "impl Into<WindowRef>",
        K::By => false,
    }
}

#[test]
fn every_table_entry_is_a_basecase_method_taking_those_arguments() {
    let methods = basecase_signatures();
    assert!(
        methods.len() > 100,
        "read only {} BaseCase methods; is the oracle broken?",
        methods.len()
    );
    let mut problems = Vec::new();
    for entry in TABLE {
        let py = entry.py[0];
        let Some(sig) = methods.get(entry.rust) else {
            problems.push(format!("{py}: BaseCase has no method `{}`", entry.rust));
            continue;
        };
        let given: Vec<K> = entry
            .params
            .iter()
            .filter(|param| !matches!(param.fill, Fill::Drop | Fill::Omit))
            .map(|param| param.kind)
            .collect();
        if given.len() != sig.params.len() {
            problems.push(format!(
                "{py}: `{}` takes {:?}, the table passes {} argument(s)",
                entry.rust,
                sig.params,
                given.len()
            ));
            continue;
        }
        for (kind, rust) in given.iter().zip(&sig.params) {
            if !kind_fits(*kind, rust) {
                problems.push(format!(
                    "{py}: `{}` wants a {rust}, the table passes {kind:?}",
                    entry.rust
                ));
            }
        }
        let builder = is_builder(sig);

        let is_async = entry.flow != Flow::Try;
        if sig.asyncness != is_async && !builder {
            problems.push(format!(
                "{py}: `{}` is{} async, the table says it is{}",
                entry.rust,
                if sig.asyncness { "" } else { " not" },
                if is_async { "" } else { " not" }
            ));
        }
        let fallible = entry.flow != Flow::AwaitInfallible;
        if (sig.ret == "Result") != fallible && !builder {
            problems.push(format!(
                "{py}: `{}` returns {}, the table says it {} fail",
                entry.rust,
                sig.ret,
                if fallible { "can" } else { "cannot" }
            ));
        }
        let inner_ok = match entry.ret {
            Ret::Str => sig.inner == "String",
            Ret::OptStr => sig.inner == "Option",
            Ret::Bool => sig.inner == "bool",
            Ret::ListStr => sig.inner == "Vec",
            Ret::Unit | Ret::Alert | Ret::Other => true,
        };
        if !inner_ok {
            problems.push(format!(
                "{py}: `{}` gives back {}, the table says {:?}",
                entry.rust, sig.inner, entry.ret
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_python_name_is_an_implemented_row_of_the_parity_manifest() {
    let text =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("parity/api.toml"))
            .unwrap();
    let manifest = text.parse::<toml::Table>().unwrap();
    let rows = manifest["basecase"].as_table().unwrap();
    let methods = basecase_signatures();
    let mut problems = Vec::new();
    for entry in TABLE {
        for name in entry.py {
            let Some(row) = rows.get(*name).and_then(|row| row.as_table()) else {
                problems.push(format!(
                    "{name}: no row [basecase.\"{name}\"] in parity/api.toml"
                ));
                continue;
            };
            let status = row.get("status").and_then(|status| status.as_str());
            if !matches!(status, Some("implemented" | "composed")) {
                problems.push(format!("{name}: its status is {status:?}"));
            }
            // The row names the Rust method of the Python name. The table
            // writes the canonical one for an alias, which must then have
            // the same shape.
            let rust = row
                .get("rust")
                .and_then(|rust| rust.as_str())
                .and_then(|rust| rust.strip_prefix("BaseCase::"))
                .unwrap_or_default();
            if rust == entry.rust {
                continue;
            }
            let same_shape = match (methods.get(rust), methods.get(entry.rust)) {
                (Some(alias), Some(canonical)) => {
                    alias.params == canonical.params
                        && alias.asyncness == canonical.asyncness
                        && alias.ret == canonical.ret
                }
                _ => false,
            };
            if !same_shape {
                problems.push(format!(
                    "{name}: the row says `{rust}`, the table `{}`, and their signatures differ",
                    entry.rust
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_table_has_no_python_name_twice() {
    let mut seen = std::collections::HashSet::new();
    for entry in TABLE {
        for name in entry.py {
            assert!(seen.insert(*name), "{name} is in the table twice");
        }
    }
}

// ---------------------------------------------------------------------------
// The parser
// ---------------------------------------------------------------------------

fn parse(source: &str) -> Vec<super::ast::Stmt> {
    parse_module(source).unwrap().stmts
}

#[test]
fn blocks_follow_the_indentation() {
    let stmts = parse("if a:\n    b()\n    c()\nelif d:\n    e()\nelse:\n    f()\ng()\n");
    assert_eq!(stmts.len(), 2);
    let StmtKind::If { body, orelse, .. } = &stmts[0].kind else {
        panic!("{:?}", stmts[0]);
    };
    assert_eq!(body.len(), 2);
    let [elif] = orelse.as_slice() else {
        panic!("{orelse:?}");
    };
    let StmtKind::If { orelse, .. } = &elif.kind else {
        panic!("{elif:?}");
    };
    assert_eq!(orelse.len(), 1);
}

#[test]
fn a_statement_that_cannot_be_read_is_kept_and_the_next_one_still_parses() {
    let stmts = parse("x = = 3\nself.open('a')\n");
    assert_eq!(stmts.len(), 2);
    assert!(matches!(stmts[0].kind, StmtKind::Unsupported(_)));
    assert!(matches!(stmts[1].kind, StmtKind::Expr(_)));
}

#[test]
fn f_strings_and_adjacent_literals_are_read() {
    let stmts = parse("x = f\"a {b} c\" 'd'\n");
    let StmtKind::Assign { value, .. } = &stmts[0].kind else {
        panic!("{:?}", stmts[0]);
    };
    let Expr::FStr(parts) = value else {
        panic!("{value:?}");
    };
    assert_eq!(parts.len(), 3);
}

#[test]
fn operators_nest_by_precedence() {
    let stmts = parse("x = a + b * c == d and not e\n");
    let StmtKind::Assign { value, .. } = &stmts[0].kind else {
        panic!("{:?}", stmts[0]);
    };
    let Expr::Binary(_, BinOp::And, _) = value else {
        panic!("{value:?}");
    };
}

#[test]
fn comprehensions_slices_and_lambdas_are_kept_as_source() {
    let stmts = parse("a = [x for x in y]\nb = c[1:2]\nd = lambda q: q + 1\n");
    for stmt in &stmts {
        let StmtKind::Assign { value, .. } = &stmt.kind else {
            panic!("{stmt:?}");
        };
        assert!(matches!(value, Expr::Other(_)), "{value:?}");
    }
}

#[test]
fn deep_nesting_is_refused_not_overflowed() {
    let source = format!("x = {}1{}\n", "(".repeat(500), ")".repeat(500));
    let stmts = parse(&source);
    assert!(matches!(stmts[0].kind, StmtKind::Unsupported(_)));
}

#[test]
fn decorators_defs_and_classes_are_read() {
    let stmts = parse(
        "class T(Base):\n    @parameterized.expand([[1]])\n    def test_x(self, a):\n        pass\n",
    );
    let StmtKind::ClassDef(class) = &stmts[0].kind else {
        panic!("{:?}", stmts[0]);
    };
    let StmtKind::FuncDef(def) = &class.body[0].kind else {
        panic!("{:?}", class.body[0]);
    };
    assert_eq!(def.decorators.len(), 1);
    assert_eq!(def.params.len(), 2);
}
