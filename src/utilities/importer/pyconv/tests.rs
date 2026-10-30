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
    /// The parameter names.
    names: Vec<String>,
    /// It is `pub`.
    public: bool,
    /// It is an `async fn`.
    asyncness: bool,
    /// The name of its return type (`Result`, `bool`, `()`).
    ret: String,
    /// The name of the first type argument of the return type.
    inner: String,
    /// The return type written out (`Result<Option<String>>`).
    ret_text: String,
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
                let mut params = Vec::new();
                let mut names = Vec::new();
                for argument in &function.sig.inputs {
                    if let syn::FnArg::Typed(typed) = argument {
                        params.push(type_name(&typed.ty));
                        names.push(match &*typed.pat {
                            syn::Pat::Ident(ident) => ident.ident.to_string(),
                            _ => "_".to_owned(),
                        });
                    }
                }
                let (ret, inner, ret_text) = match &function.sig.output {
                    syn::ReturnType::Default => ("()".to_owned(), String::new(), "()".to_owned()),
                    syn::ReturnType::Type(_, ty) => {
                        (type_name(ty), first_argument(ty), type_text(ty))
                    }
                };
                methods.insert(
                    function.sig.ident.to_string(),
                    Sig {
                        params,
                        names,
                        public: matches!(function.vis, syn::Visibility::Public(_)),
                        asyncness: function.sig.asyncness.is_some(),
                        ret,
                        inner,
                        ret_text,
                    },
                );
            }
        }
    }
    methods
}

/// A type written out: `Result<Option<String>>`, `&str`, `()`.
fn type_text(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Reference(reference) => format!("&{}", type_text(&reference.elem)),
        syn::Type::Tuple(tuple) if tuple.elems.is_empty() => "()".to_owned(),
        syn::Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return String::new();
            };
            let name = segment.ident.to_string();
            let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return name;
            };
            let inner: Vec<String> = arguments
                .args
                .iter()
                .filter_map(|argument| match argument {
                    syn::GenericArgument::Type(ty) => Some(type_text(ty)),
                    _ => None,
                })
                .collect();
            if inner.is_empty() {
                name
            } else {
                format!("{name}<{}>", inner.join(", "))
            }
        }
        _ => "other".to_owned(),
    }
}

/// The methods of every other type with an inherent `impl` in `src/api`: the
/// call builders (`GetSelectOptions::attribute`).
fn builder_methods() -> &'static HashMap<String, std::collections::HashSet<String>> {
    static METHODS: std::sync::OnceLock<HashMap<String, std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    METHODS.get_or_init(|| {
        let mut files = Vec::new();
        rust_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api"),
            &mut files,
        );
        let mut types: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
        for file in files {
            let source = std::fs::read_to_string(&file).unwrap();
            let Ok(syntax) = syn::parse_file(&source) else {
                continue;
            };
            for item in syntax.items {
                let syn::Item::Impl(implementation) = item else {
                    continue;
                };
                if implementation.trait_.is_some() {
                    continue;
                }
                let name = type_name(&implementation.self_ty);
                for member in implementation.items {
                    if let syn::ImplItem::Fn(function) = member {
                        types
                            .entry(name.clone())
                            .or_default()
                            .insert(function.sig.ident.to_string());
                    }
                }
            }
        }
        types
    })
}

// ---------------------------------------------------------------------------
// The generated table
// ---------------------------------------------------------------------------

/// The table constructor of a parameter of this Rust type, for the Python
/// parameter `py`.
fn param_source(py: &str, rust_type: &str) -> Option<String> {
    Some(match rust_type {
        "&str" => format!("s({py:?})"),
        "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" => {
            format!("n({py:?})")
        }
        "f64" => format!("f({py:?})"),
        "bool" => format!("b({py:?})"),
        "impl Into<FrameRef>" => format!("typed({py:?}, K::Frame)"),
        "impl Into<WindowRef>" => format!("typed({py:?}, K::Window)"),
        _ => return None,
    })
}

/// The first type argument of `Result<T, E>` as written: `T`.
fn first_type_argument(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("Result<") else {
        return "";
    };
    let mut depth = 0_usize;
    for (index, c) in rest.char_indices() {
        match c {
            '<' => depth += 1,
            '>' if depth == 0 => return &rest[..index],
            '>' => depth -= 1,
            ',' if depth == 0 => return &rest[..index],
            _ => {}
        }
    }
    rest
}

/// The `Ret` of a call whose `Result` holds `inner`.
fn ret_source(inner: &str) -> &'static str {
    match inner {
        "()" => "Ret::Unit",
        "String" => "Ret::Str",
        "bool" => "Ret::Bool",
        "Option<String>" => "Ret::OptStr",
        "Vec<String>" => "Ret::ListStr",
        _ => "Ret::Other",
    }
}

/// The calls of `generated.rs`: every `implemented` row of the parity
/// manifest that the hand-written table leaves out and whose Rust call is
/// mechanical (public, `Result`, and parameters of the kinds the table knows),
/// with the Python parameters of `parity/upstream-api.json` in their order.
/// The Python parameters Rust has no argument for are `omit`ted (an error when
/// given), `by` and `timeout` are dropped, a Rust timeout takes the Python one.
fn generated_source() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let methods = basecase_signatures();
    let manifest = std::fs::read_to_string(root.join("parity/api.toml"))
        .unwrap()
        .parse::<toml::Table>()
        .unwrap();
    let rows = manifest["basecase"].as_table().unwrap();
    let upstream: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("parity/upstream-api.json")).unwrap(),
    )
    .unwrap();
    let mut python: HashMap<String, Vec<String>> = HashMap::new();
    for method in upstream["basecase"].as_array().unwrap() {
        let name = method["name"].as_str().unwrap().to_owned();
        let params = method["params"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|param| param.as_str().map(str::to_owned))
            .collect();
        python.insert(name, params);
    }
    let handwritten: std::collections::HashSet<&str> = TABLE
        .iter()
        .flat_map(|entry| entry.py.iter().copied())
        .collect();

    let mut entries = std::collections::BTreeMap::new();
    'rows: for (py, row) in rows {
        let Some(row) = row.as_table() else {
            continue;
        };
        if row.get("status").and_then(|status| status.as_str()) != Some("implemented")
            || row.contains_key("divergence")
            || handwritten.contains(py.as_str())
        {
            continue;
        }
        let Some(rust) = row
            .get("rust")
            .and_then(|rust| rust.as_str())
            .and_then(|rust| rust.strip_prefix("BaseCase::"))
        else {
            continue;
        };
        let (Some(sig), Some(py_params)) = (methods.get(rust), python.get(py)) else {
            continue;
        };
        if !sig.public
            || sig.ret != "Result"
            || py_params.iter().any(|param| param.starts_with('*'))
        {
            continue;
        }
        let flow = if sig.asyncness {
            "Flow::Await"
        } else {
            "Flow::Try"
        };
        let ret = ret_source(first_type_argument(&sig.ret_text));

        let is_ancillary =
            |param: &str| param == "by" || param.ends_with("_by") || param == "timeout";
        let main: Vec<&str> = py_params
            .iter()
            .map(String::as_str)
            .filter(|param| !is_ancillary(param))
            .collect();
        let mut next_main = 0;
        let mut last_index: Option<usize> = None;
        let mut roles: HashMap<&str, String> = HashMap::new();
        for (name, rust_type) in sig.names.iter().zip(&sig.params) {
            let (python_name, source) = if name.contains("timeout") && kind_fits(K::Int, rust_type)
            {
                if !py_params.iter().any(|param| param == "timeout") {
                    continue 'rows;
                }
                ("timeout", "WAIT_TIMEOUT".to_owned())
            } else {
                let Some(&python_name) = main.get(next_main) else {
                    continue 'rows;
                };
                next_main += 1;
                let Some(source) = param_source(python_name, rust_type) else {
                    continue 'rows;
                };
                (python_name, source)
            };
            let index = py_params
                .iter()
                .position(|param| param == python_name)
                .unwrap();
            if last_index.is_some_and(|last| last >= index) {
                continue 'rows;
            }
            last_index = Some(index);
            roles.insert(python_name, source);
        }
        let params: Vec<String> = py_params
            .iter()
            .map(|param| {
                if let Some(source) = roles.get(param.as_str()) {
                    source.clone()
                } else if param == "by" {
                    "BY".to_owned()
                } else if param.ends_with("_by") {
                    format!("by_named({param:?})")
                } else if param == "timeout" {
                    "TIMEOUT".to_owned()
                } else {
                    format!("omit({param:?})")
                }
            })
            .collect();
        entries.insert(
            py.clone(),
            format!(
                "    call(&[{py:?}], {rust:?}, &[{}], {flow}, {ret}),\n",
                params.join(", ")
            ),
        );
    }

    // Import what the entries use.
    let body: String = entries.values().map(String::as_str).collect();
    let used: std::collections::HashSet<&str> = body
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .collect();
    let imports: Vec<&str> = [
        "b",
        "by_named",
        "call",
        "f",
        "n",
        "omit",
        "s",
        "typed",
        "Call",
        "Flow",
        "Ret",
        "BY",
        "K",
        "TIMEOUT",
        "WAIT_TIMEOUT",
    ]
    .into_iter()
    .filter(|name| matches!(*name, "call" | "Call") || used.contains(name))
    .collect();
    let mut text = format!(
        "//! Calls of the table that `pyconv::tests` writes out from the signatures of\n\
             //! `BaseCase`, the `implemented` rows of `parity/api.toml` and the Python\n\
             //! parameters of `parity/upstream-api.json`: the ones the hand-written table\n\
             //! leaves out and whose Rust call is mechanical. Write it again with\n\
             //! `SB_UPDATE_GENERATED=1 cargo test -p seleniumbase-rs --lib the_generated_table`.\n\
             \n\
             use super::table::{{{}}};\n\
             \n\
             /// The generated calls.\n\
             pub(super) static GENERATED: &[Call] = &[\n",
        imports.join(", ")
    );
    for line in entries.values() {
        text.push_str(line);
    }
    text.push_str("];\n");
    text
}

#[test]
fn the_generated_table_is_current() {
    let text = generated_source();
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/utilities/importer/pyconv/generated.rs");
    if std::env::var_os("SB_UPDATE_GENERATED").is_some() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    assert!(
        std::fs::read_to_string(&path).unwrap() == text,
        "generated.rs is stale: write it again with SB_UPDATE_GENERATED=1"
    );
    assert!(
        text.matches("    call(").count() > 20,
        "the generator found almost nothing; is it broken?"
    );
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
    for entry in TABLE.iter().chain(super::generated::GENERATED.iter()) {
        let py = entry.py[0];
        let Some(sig) = methods.get(entry.rust) else {
            problems.push(format!("{py}: BaseCase has no method `{}`", entry.rust));
            continue;
        };
        let given: Vec<K> = entry
            .params
            .iter()
            .filter(|param| !matches!(param.fill, Fill::Drop | Fill::Omit | Fill::Chain(_)))
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
        for param in entry.params {
            if let Fill::Chain(template) = param.fill {
                let method = template
                    .trim_start_matches('.')
                    .split('(')
                    .next()
                    .unwrap_or_default();
                let has = builder_methods()
                    .get(&sig.ret)
                    .is_some_and(|methods| methods.contains(method));
                if !has {
                    problems.push(format!(
                        "{py}: `{}` returns {}, which has no method `{method}`",
                        entry.rust, sig.ret
                    ));
                }
            }
        }

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
        if !inner_ok && !builder {
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
    for entry in TABLE.iter().chain(super::generated::GENERATED.iter()) {
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
    for entry in TABLE.iter().chain(super::generated::GENERATED.iter()) {
        for name in entry.py {
            assert!(seen.insert(*name), "{name} is in the table twice");
        }
    }
}

// ---------------------------------------------------------------------------
// The parser
// ---------------------------------------------------------------------------

/// The `sb.method(args)` calls of generated Rust, checked against `BaseCase`
/// the way the recorder checks the calls it writes: the method exists, takes
/// that many arguments, and a literal argument is of a kind the parameter
/// takes.
struct SbCalls<'a> {
    methods: &'a HashMap<String, Sig>,
    problems: Vec<String>,
    calls: usize,
}

impl<'ast> syn::visit::Visit<'ast> for SbCalls<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let on_sb = matches!(&*call.receiver, syn::Expr::Path(path) if path.path.is_ident("sb"));
        // `sb.run(..)` is the fixture's, the one call that is not BaseCase's.
        if on_sb && call.method != "run" {
            self.calls += 1;
            let name = call.method.to_string();
            match self.methods.get(&name) {
                None => self
                    .problems
                    .push(format!("BaseCase has no method `{name}`")),
                Some(sig) if sig.params.len() != call.args.len() => self.problems.push(format!(
                    "`{name}` takes {} argument(s) {:?}, the call passes {}",
                    sig.params.len(),
                    sig.params,
                    call.args.len()
                )),
                Some(sig) => {
                    for (param, argument) in sig.params.iter().zip(&call.args) {
                        let kind = literal_kind(argument);
                        let fits = match param.as_str() {
                            "&str" => kind != "integer" && kind != "float" && kind != "bool",
                            "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32"
                            | "i64" | "isize" => {
                                kind != "string" && kind != "float" && kind != "bool"
                            }
                            "f64" => kind != "string" && kind != "bool",
                            "bool" => kind != "string" && kind != "integer" && kind != "float",
                            "impl Into<FrameRef>" | "impl Into<WindowRef>" => {
                                kind != "float" && kind != "bool"
                            }
                            _ => true,
                        };
                        if !fits {
                            self.problems.push(format!(
                                "`{name}` wants a {param}, the call passes a {kind}"
                            ));
                        }
                    }
                }
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}

/// The kind of a literal argument; `other` for anything else.
fn literal_kind(expr: &syn::Expr) -> &'static str {
    match expr {
        syn::Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Str(_) => "string",
            syn::Lit::Int(_) => "integer",
            syn::Lit::Float(_) => "float",
            syn::Lit::Bool(_) => "bool",
            _ => "other",
        },
        syn::Expr::Unary(unary) => literal_kind(&unary.expr),
        _ => "other",
    }
}

#[test]
fn every_call_the_goldens_make_on_sb_is_a_basecase_method_taking_those_arguments() {
    use syn::visit::Visit;
    let methods = basecase_signatures();
    let mut visitor = SbCalls {
        methods: &methods,
        problems: Vec::new(),
        calls: 0,
    };
    for entry in std::fs::read_dir(golden_dir()).unwrap() {
        let path = entry.unwrap().path();
        if !path.to_string_lossy().ends_with(".rs.golden") {
            continue;
        }
        let file = syn::parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
        visitor.visit_file(&file);
    }
    assert!(
        visitor.calls > 300,
        "read only {} calls; is the oracle broken?",
        visitor.calls
    );
    assert!(
        visitor.problems.is_empty(),
        "{}",
        visitor.problems.join("\n")
    );
}

/// `tests/golden/import_python`: the Python examples and what the importer
/// writes for each.
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/import_python")
}

/// The converter's output for every Python file of the golden folder is the
/// `.rs.golden` file beside it, and none has an error. The goldens are
/// compiled against the API by the `cfg(doctest)` item of `python_importer`.
/// `SB_UPDATE_GOLDENS=1` writes them again.
#[test]
fn the_goldens_are_what_the_converter_writes() {
    use crate::utilities::python_importer::{
        import_python, ImportOptions, ImportSeverity, PythonSource,
    };
    let mut sources: Vec<PathBuf> = std::fs::read_dir(golden_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "py"))
        .collect();
    sources.sort();
    assert!(
        sources.len() >= 28,
        "the golden folder holds {} Python files",
        sources.len()
    );
    let update = std::env::var_os("SB_UPDATE_GOLDENS").is_some();
    let mut problems = Vec::new();
    for path in sources {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let source = std::fs::read_to_string(&path).unwrap();
        let result = import_python(
            &source,
            &ImportOptions {
                source: PythonSource::Auto,
                test_name: stem.clone(),
            },
        );
        if result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ImportSeverity::Error)
        {
            problems.push(format!("{stem}: {:?}", result.diagnostics));
        }
        syn::parse_file(&result.rust).unwrap_or_else(|error| panic!("{stem}: {error}"));
        let golden = path.with_extension("rs.golden");
        if update {
            std::fs::write(&golden, &result.rust).unwrap();
        } else if std::fs::read_to_string(&golden).ok().as_deref() != Some(result.rust.as_str()) {
            problems.push(format!(
                "{stem}: {} is not what the converter writes",
                golden.display()
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// `parity/examples.toml` parses, and each of its `[[converter]]` findings says
/// what was fixed; its counts are those of the goldens.
#[test]
fn the_converter_findings_of_the_examples_manifest_say_what_was_fixed() {
    let text =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("parity/examples.toml"))
            .unwrap();
    let manifest = text.parse::<toml::Table>().unwrap();
    let findings = manifest["converter"].as_array().unwrap();
    assert_eq!(findings.len(), 6);
    for finding in findings {
        for key in ["gap", "seen_in", "fixed", "next"] {
            assert!(
                finding.get(key).is_some_and(toml::Value::is_str),
                "{finding:?} has no `{key}`"
            );
        }
    }
    // Every golden example has a line in the per-file counts.
    let counts = findings[0]["before_after"].as_array().unwrap();
    for entry in std::fs::read_dir(golden_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "py") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            assert!(
                counts
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .any(|line| line.starts_with(&format!("{name}:"))
                        || line.starts_with(&format!("cdp_mode/{name}:"))),
                "{name} is missing from before_after"
            );
        }
    }
}

/// The upstream examples `parity/examples.toml` is about, relative to its
/// `examples/` folder.
const UPSTREAM_EXAMPLES: &[&str] = &[
    "my_first_test.py",
    "test_demo_site.py",
    "iframe_tests.py",
    "test_window_switching.py",
    "test_coffee_cart.py",
    "test_assert_elements.py",
    "test_checkboxes.py",
    "test_select_options.py",
    "test_shadow_dom.py",
    "shadow_root_test.py",
    "test_double_click.py",
    "test_drag_and_drop.py",
    "test_url_asserts.py",
    "test_deferred_asserts.py",
    "handle_alert_test.py",
    "test_tinymce.py",
    "test_todomvc.py",
    "xpath_test.py",
    "test_simple_login.py",
    "test_contains_selector.py",
    "test_cycle_elements.py",
    "test_highlight_elements.py",
    "cdp_mode/raw_demo_site.py",
    "cdp_mode/raw_cdp_tabs.py",
    "cdp_mode/raw_cdp_xpath.py",
    "cdp_mode/raw_basic_cdp.py",
    "cdp_mode/raw_drag_and_drop.py",
    "cdp_mode/raw_cdp_swag.py",
];

/// Converts the upstream examples and prints the warnings and errors of each;
/// with `SBRS_CONVERT_OUT` set it also writes the Rust files there. It does
/// nothing unless `SBRS_UPSTREAM_EXAMPLES` names the upstream `examples/`
/// folder (`cargo test ... upstream_examples -- --nocapture`).
#[test]
fn the_upstream_examples_convert_when_they_are_at_hand() {
    use crate::utilities::python_importer::{
        import_python, ImportOptions, ImportSeverity, PythonSource,
    };
    let Some(root) = std::env::var_os("SBRS_UPSTREAM_EXAMPLES") else {
        return;
    };
    let out = std::env::var_os("SBRS_CONVERT_OUT").map(PathBuf::from);
    for name in UPSTREAM_EXAMPLES {
        // The upstream folder has `cdp_mode/` below it; the golden folder is flat.
        let flat = Path::new(&root).join(Path::new(name).file_name().unwrap());
        let nested = Path::new(&root).join(name);
        let path = if nested.exists() { nested } else { flat };
        let source = std::fs::read_to_string(&path).unwrap();
        let stem = Path::new(name).file_stem().unwrap().to_string_lossy();
        let result = import_python(
            &source,
            &ImportOptions {
                source: PythonSource::Auto,
                test_name: stem.to_string(),
            },
        );
        let count = |severity| {
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == severity)
                .count()
        };
        println!(
            "{name}: {} warning(s), {} error(s)",
            count(ImportSeverity::Warning),
            count(ImportSeverity::Error)
        );
        for diagnostic in &result.diagnostics {
            println!("    line {}: {}", diagnostic.line, diagnostic.message);
        }
        if let Some(out) = &out {
            std::fs::create_dir_all(out).unwrap();
            std::fs::write(out.join(format!("{stem}.rs")), &result.rust).unwrap();
        }
    }
}

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
