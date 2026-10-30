//! Expressions: Python values and calls as Rust code.
//!
//! A call whose Rust equal awaits and is used as a value is written to a
//! `let` first (`let selected = sb.is_selected("#a").await?;`), so no await
//! sits inside the arguments of another call.

use super::ast::{Arg, BinOp, Expr, FPart, UnOp};
use super::table::{lookup, Call, Fill, Flow, Ret, K, P};
use super::tr::{one_line, py_repr, quote, Skip, TResult, Tr, Ty, Val};

/// A call written in Rust.
pub(super) struct Rendered {
    /// The Rust expression, without a closing `;`.
    pub(super) code: String,
    /// What it gives back.
    pub(super) ty: Ty,
    /// It awaits, so it cannot sit inside the arguments of another call.
    pub(super) awaits: bool,
}

impl Rendered {
    /// A call that is not async.
    pub(super) fn plain(code: impl Into<String>, ty: Ty) -> Self {
        Self {
            code: code.into(),
            ty,
            awaits: false,
        }
    }

    /// A call that awaits.
    pub(super) fn awaiting(code: impl Into<String>, ty: Ty) -> Self {
        Self {
            code: code.into(),
            ty,
            awaits: true,
        }
    }
}

/// The arguments of a call matched with the Python parameters.
pub(super) struct Bound<'e> {
    /// One slot per parameter.
    pub(super) slots: Vec<Option<&'e Expr>>,
    /// The positional arguments taken by a `*args` parameter.
    pub(super) rest: Vec<&'e Expr>,
}

/// Matches the arguments of a call with Python's parameters: positional ones
/// in order, keyword ones by name.
pub(super) fn bind<'e>(params: &[P], args: &'e [Arg]) -> TResult<Bound<'e>> {
    let mut slots: Vec<Option<&Expr>> = vec![None; params.len()];
    let mut rest = Vec::new();
    let mut next = 0;
    let rest_index = params.iter().position(|param| param.fill == Fill::Rest);
    for arg in args {
        match arg {
            Arg::Spread(text) => {
                return Err(Skip::error(format!(
                    "`{}` is not supported: write the arguments out",
                    one_line(text)
                )));
            }
            Arg::Pos(value) => {
                if rest_index.is_some_and(|index| next >= index) {
                    rest.push(value);
                    continue;
                }
                if next >= params.len() {
                    return Err(Skip::error("too many arguments"));
                }
                slots[next] = Some(value);
                next += 1;
            }
            Arg::Kw(name, value) => {
                let index = params
                    .iter()
                    .position(|param| param.name == name.as_str())
                    .ok_or_else(|| {
                        Skip::error(format!("the argument '{name}' is not supported"))
                    })?;
                if slots[index].is_some() || Some(index) == rest_index {
                    return Err(Skip::error(format!("'{name}' is given twice")));
                }
                slots[index] = Some(value);
            }
        }
    }
    Ok(Bound { slots, rest })
}

/// A piece of a string that is built from parts.
pub(super) enum Piece {
    /// Literal text.
    Lit(String),
    /// A value written with `{}`.
    Arg(Val),
}

/// The `format!` template and arguments of `pieces`, or the literal text when
/// there are no arguments.
fn template_of(pieces: Vec<Piece>) -> (String, Vec<String>, String) {
    let mut template = String::new();
    let mut plain = String::new();
    let mut args = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Lit(text) => {
                plain.push_str(&text);
                template.push_str(&text.replace('{', "{{").replace('}', "}}"));
            }
            Piece::Arg(value) => {
                template.push_str(if matches!(value.ty, Ty::List(_)) {
                    "{:?}"
                } else {
                    "{}"
                });
                args.push(value.code);
            }
        }
    }
    (template, args, plain)
}

/// A string built from `pieces`: a literal, or a `format!`.
pub(super) fn format_pieces(pieces: Vec<Piece>) -> Val {
    let (template, args, plain) = template_of(pieces);
    if args.is_empty() {
        return Val::new(quote(&plain), Ty::Str);
    }
    Val::new(
        format!("format!({}, {})", quote(&template), args.join(", ")),
        Ty::String,
    )
}

/// The name a hoisted value gets: the method without `is_`, `get_` or `has_`.
fn hint_for(method: &str) -> &str {
    for prefix in ["is_", "get_", "has_"] {
        if let Some(rest) = method.strip_prefix(prefix) {
            if !rest.is_empty() {
                return rest;
            }
        }
    }
    method
}

/// The operand of a comparison that `assert_equal` takes.
fn compare_operand(value: &Val) -> TResult<String> {
    match value.ty {
        Ty::Str => Ok(value.code.clone()),
        Ty::String => Ok(format!("{}.as_str()", value.operand())),
        Ty::Int | Ty::Float | Ty::Bool => Ok(value.code.clone()),
        Ty::List(_) => Ok(format!("&{}", value.operand())),
        _ => Err(Skip::warn(
            "these values cannot be compared by assert_equal",
        )),
    }
}

/// A float literal for a whole number written in Python.
fn float_literal(code: &str) -> String {
    if code.chars().all(|c| c.is_ascii_digit()) {
        format!("{code}.0")
    } else {
        code.to_owned()
    }
}

/// The Rust code of a number written in Python.
fn number(text: &str, float: bool) -> Val {
    let mut code = text.to_owned();
    if code.starts_with('.') {
        code.insert(0, '0');
    }
    Val::new(code, if float { Ty::Float } else { Ty::Int })
}

impl Tr<'_> {
    /// The expression as Rust code; an awaiting call is first written to a
    /// `let`.
    pub(super) fn expr(&mut self, expr: &Expr) -> TResult<Val> {
        self.value(expr, true)
    }

    /// The expression as Rust code. With `hoist` false an awaiting call stays
    /// in the code (for the right side of a `let`).
    pub(super) fn value(&mut self, expr: &Expr, hoist: bool) -> TResult<Val> {
        match expr {
            Expr::Str(text) => Ok(Val::new(quote(text), Ty::Str)),
            Expr::FStr(parts) => self.fstring(parts),
            Expr::Num { text, float } => Ok(number(text, *float)),
            Expr::Bool(value) => Ok(Val::new(value.to_string(), Ty::Bool)),
            Expr::None => Err(Skip::error("None has no Rust equal here")),
            Expr::Name(name) => self.name(name),
            Expr::List(items) | Expr::Tuple(items) => self.list(items),
            Expr::Dict(_) => Err(Skip::warn("a dict is not translated")),
            Expr::Attr(base, attr) => self.attribute(base, attr),
            Expr::Call(func, args) => {
                let rendered = self.render_any(func, args)?;
                let hint = match &**func {
                    Expr::Attr(_, method) => hint_for(method).to_owned(),
                    _ => "value".to_owned(),
                };
                self.rendered_value(rendered, &hint, hoist)
            }
            Expr::Index(base, index) => self.index(base, index),
            Expr::Unary(op, operand) => self.unary(*op, operand),
            Expr::Binary(..) => self.binary(expr),
            Expr::IfExp(test, then, orelse) => self.if_exp(test, then, orelse),
            Expr::Other(text) => Err(Skip::warn(format!(
                "`{}` is not translated",
                one_line(text)
            ))),
        }
    }

    /// A rendered call as a value.
    fn rendered_value(&mut self, rendered: Rendered, hint: &str, hoist: bool) -> TResult<Val> {
        if rendered.ty == Ty::Unit {
            return Err(Skip::warn("this call gives back nothing"));
        }
        if rendered.awaits && hoist {
            let name = self.temp(hint);
            self.emit(format!("let {name} = {};", rendered.code));
            return Ok(Val::new(name, rendered.ty));
        }
        Ok(Val::new(rendered.code, rendered.ty))
    }

    /// Whether translating `expr` may write a `let` first.
    pub(super) fn may_await(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Call(func, args) => {
                self.receiver_method(func).is_some()
                    || matches!(&**func, Expr::Attr(base, _) if self.may_await(base))
                    || args.iter().any(|arg| match arg {
                        Arg::Pos(value) | Arg::Kw(_, value) => self.may_await(value),
                        Arg::Spread(_) => false,
                    })
            }
            Expr::Attr(base, _) => self.may_await(base),
            Expr::Unary(_, operand) => self.may_await(operand),
            Expr::Binary(left, _, right) => self.may_await(left) || self.may_await(right),
            Expr::IfExp(a, b, c) => self.may_await(a) || self.may_await(b) || self.may_await(c),
            Expr::Index(a, b) => self.may_await(a) || self.may_await(b),
            Expr::List(items) | Expr::Tuple(items) => items.iter().any(|item| self.may_await(item)),
            Expr::FStr(parts) => parts.iter().any(|part| match part {
                FPart::Expr(value) => self.may_await(value),
                FPart::Lit(_) => false,
            }),
            _ => false,
        }
    }

    fn name(&mut self, name: &str) -> TResult<Val> {
        if self.receivers.contains(name) {
            return Err(Skip::warn(format!(
                "`{name}` (the browser) cannot be used as a value"
            )));
        }
        if let Some(var) = self.var(name) {
            return Ok(Val::new(var.rust.clone(), var.ty.clone()));
        }
        Err(Skip::error(format!("the name '{name}' is not defined")))
    }

    fn fstring(&mut self, parts: &[FPart]) -> TResult<Val> {
        let mut pieces = Vec::new();
        for part in parts {
            match part {
                FPart::Lit(text) => pieces.push(Piece::Lit(text.clone())),
                FPart::Expr(value) => {
                    let value = self.value(value, true)?;
                    if !matches!(
                        value.ty,
                        Ty::Str | Ty::String | Ty::Int | Ty::Float | Ty::Bool | Ty::List(_)
                    ) {
                        return Err(Skip::warn("this value cannot be put in a string"));
                    }
                    pieces.push(Piece::Arg(value));
                }
            }
        }
        Ok(format_pieces(pieces))
    }

    fn list(&mut self, items: &[Expr]) -> TResult<Val> {
        if items.is_empty() {
            return Ok(Val::new("Vec::new()", Ty::List(Box::new(Ty::Unknown))));
        }
        let mut vals = Vec::new();
        for item in items {
            vals.push(self.value(item, true)?);
        }
        if vals.iter().all(|value| value.ty.is_text()) {
            let mut codes = Vec::new();
            for (item, value) in items.iter().zip(&vals) {
                codes.push(match value.ty {
                    Ty::String => format!("&{}", value.operand()),
                    _ => value.code.clone(),
                });
                let _ = item;
            }
            return Ok(Val::new(
                format!("[{}].map(String::from).to_vec()", codes.join(", ")),
                Ty::List(Box::new(Ty::Str)),
            ));
        }
        for number in [Ty::Int, Ty::Float, Ty::Bool] {
            if vals.iter().all(|value| value.ty == number) {
                let codes: Vec<String> = vals.iter().map(|value| value.code.clone()).collect();
                return Ok(Val::new(
                    format!("vec![{}]", codes.join(", ")),
                    Ty::List(Box::new(number)),
                ));
            }
        }
        Err(Skip::warn("a list of mixed values is not translated"))
    }

    fn attribute(&mut self, base: &Expr, attr: &str) -> TResult<Val> {
        if let Expr::Name(name) = base {
            if self.receivers.contains(name) {
                return Err(Skip::warn(format!("`{name}.{attr}` has no Rust equal")));
            }
        }
        if matches!(base, Expr::Name(name) if self.var(name).is_none()) {
            return Err(Skip::warn(format!(
                "`{}.{attr}` is not translated",
                py_repr(base)
            )));
        }

        let value = self.value(base, true)?;
        match (&value.ty, attr) {
            (Ty::Alert | Ty::Locator, "text") => {
                let name = self.temp("text");
                self.emit(format!("let {name} = {}.text().await?;", value.operand()));
                Ok(Val::new(name, Ty::String))
            }
            _ => Err(Skip::warn(format!(
                "`{}.{attr}` is not translated",
                py_repr(base)
            ))),
        }
    }

    fn index(&mut self, base: &Expr, index: &Expr) -> TResult<Val> {
        let list = self.value(base, true)?;
        let position = self.value(index, true)?;
        match (&list.ty, &position.ty) {
            (Ty::List(element), Ty::Int) => {
                let ty = if **element == Ty::Str {
                    Ty::String
                } else {
                    (**element).clone()
                };
                Ok(Val::new(
                    format!("{}[{}]", list.operand(), position.code),
                    ty,
                ))
            }
            _ => Err(Skip::warn(
                "only a list can be indexed, with a whole number",
            )),
        }
    }

    /// A value as a `bool` expression.
    pub(super) fn truthy(&self, value: Val) -> TResult<Val> {
        match value.ty {
            Ty::Bool => Ok(value),
            Ty::Str | Ty::String | Ty::List(_) => Ok(Val::compound(
                format!("!{}.is_empty()", value.operand()),
                Ty::Bool,
            )),
            Ty::Int => Ok(Val::compound(format!("{} != 0", value.operand()), Ty::Bool)),
            _ => Err(Skip::warn("this value cannot be used as a condition")),
        }
    }

    /// A condition: the expression as a `bool` expression.
    pub(super) fn condition(&mut self, expr: &Expr) -> TResult<Val> {
        let value = self.expr(expr)?;
        self.truthy(value)
    }

    fn unary(&mut self, op: UnOp, operand: &Expr) -> TResult<Val> {
        let value = self.expr(operand)?;
        match op {
            UnOp::Not => {
                let truth = self.truthy(value)?;
                Ok(Val::new(format!("!{}", truth.operand()), Ty::Bool))
            }
            UnOp::Neg if value.ty.is_number() => {
                Ok(Val::new(format!("-{}", value.operand()), value.ty.clone()))
            }
            UnOp::Pos if value.ty.is_number() => Ok(value),
            _ => Err(Skip::warn("this unary operator is not translated")),
        }
    }

    fn if_exp(&mut self, test: &Expr, then: &Expr, orelse: &Expr) -> TResult<Val> {
        let condition = self.condition(test)?;
        let a = self.expr(then)?;
        let b = self.expr(orelse)?;
        if a.ty.is_text() && b.ty.is_text() {
            let owned = |value: &Val| match value.ty {
                Ty::String => value.code.clone(),
                _ => format!("{}.to_owned()", value.operand()),
            };
            let both_str = a.ty == Ty::Str && b.ty == Ty::Str;
            let (x, y) = if both_str {
                (a.code.clone(), b.code.clone())
            } else {
                (owned(&a), owned(&b))
            };
            let ty = if both_str { Ty::Str } else { Ty::String };
            return Ok(Val::compound(
                format!("if {} {{ {x} }} else {{ {y} }}", condition.code),
                ty,
            ));
        }
        if a.ty == b.ty && a.ty != Ty::Unknown {
            let ty = a.ty.clone();
            return Ok(Val::compound(
                format!(
                    "if {} {{ {} }} else {{ {} }}",
                    condition.code, a.code, b.code
                ),
                ty,
            ));
        }
        Err(Skip::warn(
            "a conditional expression of mixed values is not translated",
        ))
    }

    fn binary(&mut self, expr: &Expr) -> TResult<Val> {
        let Expr::Binary(left, op, right) = expr else {
            return Err(Skip::warn("not a binary expression"));
        };
        match op {
            BinOp::Add => self.add(expr),
            BinOp::Mod => self.modulo(left, right),
            BinOp::And | BinOp::Or => {
                let a = self.condition(left)?;
                let b = self.condition(right)?;
                let symbol = if *op == BinOp::And { "&&" } else { "||" };
                Ok(Val::compound(
                    format!("{} {symbol} {}", a.operand(), b.operand()),
                    Ty::Bool,
                ))
            }
            BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::FloorDiv => {
                let a = self.expr(left)?;
                let b = self.expr(right)?;
                if !(a.ty.is_number() && b.ty.is_number()) {
                    return Err(Skip::warn("arithmetic on values that are not numbers"));
                }
                let ty = if a.ty == Ty::Float || b.ty == Ty::Float {
                    Ty::Float
                } else {
                    Ty::Int
                };
                let code = match op {
                    BinOp::Sub => format!("{} - {}", a.operand(), b.operand()),
                    BinOp::Mul => format!("{} * {}", a.operand(), b.operand()),
                    BinOp::Div => {
                        return Ok(Val::compound(
                            format!("{} as f64 / {} as f64", a.operand(), b.operand()),
                            Ty::Float,
                        ));
                    }
                    _ => format!("{} / {}", a.operand(), b.operand()),
                };
                Ok(Val::compound(code, ty))
            }
            BinOp::In | BinOp::NotIn => self.membership(left, right, *op == BinOp::NotIn),
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.comparison(left, *op, right)
            }
            BinOp::Is | BinOp::IsNot | BinOp::Other => {
                Err(Skip::warn("this operator is not translated"))
            }
        }
    }

    fn add(&mut self, expr: &Expr) -> TResult<Val> {
        let mut operands = Vec::new();
        collect_add(expr, &mut operands);
        let mut vals = Vec::new();
        for operand in &operands {
            vals.push(self.value(operand, true)?);
        }
        if vals.iter().all(|value| value.ty.is_text()) {
            let mut pieces = Vec::new();
            for (operand, value) in operands.iter().zip(vals) {
                match operand {
                    Expr::Str(text) => pieces.push(Piece::Lit(text.clone())),
                    _ => pieces.push(Piece::Arg(value)),
                }
            }
            return Ok(format_pieces(pieces));
        }
        if vals.iter().all(|value| value.ty.is_number()) {
            let ty = if vals.iter().any(|value| value.ty == Ty::Float) {
                Ty::Float
            } else {
                Ty::Int
            };
            let code: Vec<String> = vals.iter().map(Val::operand).collect();
            return Ok(Val::compound(code.join(" + "), ty));
        }
        Err(Skip::warn(
            "`+` on values that are not all strings or all numbers",
        ))
    }

    /// `"format" % value` (the only use of `%` on a string).
    fn modulo(&mut self, left: &Expr, right: &Expr) -> TResult<Val> {
        if let Expr::Str(format) = left {
            let args: Vec<&Expr> = match right {
                Expr::Tuple(items) => items.iter().collect(),
                other => vec![other],
            };
            return self.percent_format(format, &args);
        }
        let a = self.expr(left)?;
        let b = self.expr(right)?;
        if a.ty.is_number() && b.ty.is_number() {
            return Ok(Val::compound(
                format!("{} % {}", a.operand(), b.operand()),
                a.ty.clone(),
            ));
        }
        Err(Skip::warn(
            "`%` is translated only on a string literal or numbers",
        ))
    }

    fn percent_format(&mut self, format: &str, args: &[&Expr]) -> TResult<Val> {
        let mut pieces = Vec::new();
        let mut literal = String::new();
        let mut used = 0;
        let mut chars = format.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                literal.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => literal.push('%'),
                Some('s' | 'd' | 'i' | 'f' | 'r') => {
                    if !literal.is_empty() {
                        pieces.push(Piece::Lit(std::mem::take(&mut literal)));
                    }
                    let Some(arg) = args.get(used) else {
                        return Err(Skip::error("the % format has more fields than values"));
                    };
                    used += 1;
                    let value = self.value(arg, true)?;
                    pieces.push(Piece::Arg(value));
                }
                _ => return Err(Skip::warn("this % format field is not translated")),
            }
        }
        if used != args.len() {
            return Err(Skip::error("the % format has fewer fields than values"));
        }
        if !literal.is_empty() {
            pieces.push(Piece::Lit(literal));
        }
        Ok(format_pieces(pieces))
    }

    fn membership(&mut self, left: &Expr, right: &Expr, negate: bool) -> TResult<Val> {
        let item = self.expr(left)?;
        let container = self.expr(right)?;
        let needle = Self::str_code(&item, left)?;
        let code = match &container.ty {
            Ty::Str | Ty::String => format!("{}.contains({needle})", container.operand()),
            Ty::List(element) if element.is_text() => format!(
                "{}.iter().any(|item| item == {needle})",
                container.operand()
            ),
            _ => {
                return Err(Skip::warn(
                    "`in` is translated on a string or a list of strings",
                ))
            }
        };
        if negate {
            Ok(Val::new(format!("!{code}"), Ty::Bool))
        } else {
            Ok(Val::new(code, Ty::Bool))
        }
    }

    fn comparison(&mut self, left: &Expr, op: BinOp, right: &Expr) -> TResult<Val> {
        let a = self.expr(left)?;
        let b = self.expr(right)?;
        let comparable = (a.ty.is_text() && b.ty.is_text())
            || (a.ty.is_number() && b.ty.is_number())
            || (a.ty == Ty::Bool && b.ty == Ty::Bool);
        let ordered = op != BinOp::Eq && op != BinOp::Ne;
        if !comparable || (ordered && !a.ty.is_number() && !a.ty.is_text()) {
            return Err(Skip::warn("these values are not compared"));
        }
        let symbol = match op {
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            _ => ">=",
        };
        Ok(Val::compound(
            format!("{} {symbol} {}", a.operand(), b.operand()),
            Ty::Bool,
        ))
    }

    /// The code that passes `value` where a `&str` is wanted.
    fn str_code(value: &Val, source: &Expr) -> TResult<String> {
        match value.ty {
            Ty::Str => Ok(value.code.clone()),
            Ty::String | Ty::Unknown => Ok(format!("&{}", value.operand())),
            _ => Err(Skip::error(format!(
                "expected a string but found {}",
                py_repr(source)
            ))),
        }
    }

    /// An expression as a `&str` argument.
    pub(super) fn str_arg(&mut self, expr: &Expr) -> TResult<String> {
        let value = self.expr(expr)?;
        Self::str_code(&value, expr)
    }

    /// An expression as a float argument.
    fn float_arg(&mut self, expr: &Expr) -> TResult<String> {
        let value = self.expr(expr)?;
        let literal_negative = matches!(expr, Expr::Unary(UnOp::Neg, _));
        if literal_negative {
            return Err(Skip::error("a negative number of seconds cannot be waited"));
        }
        match value.ty {
            Ty::Float => Ok(value.code),
            Ty::Int if expr_is_literal(expr) => Ok(float_literal(&value.code)),
            Ty::Int => Ok(format!("{} as f64", value.operand())),
            _ => Err(Skip::error(format!(
                "expected a number but found {}",
                py_repr(expr)
            ))),
        }
    }

    /// An expression as an argument of the given kind.
    pub(super) fn argument(&mut self, kind: K, expr: &Expr) -> TResult<String> {
        match kind {
            K::Str => self.str_arg(expr),
            K::Int => {
                let value = self.expr(expr)?;
                if value.ty == Ty::Int {
                    Ok(value.code)
                } else {
                    Err(Skip::error(format!(
                        "expected a whole number but found {}",
                        py_repr(expr)
                    )))
                }
            }
            K::Float => self.float_arg(expr),
            K::Bool => {
                let value = self.expr(expr)?;
                Ok(self.truthy(value)?.code)
            }
            K::Frame | K::Window => {
                let value = self.expr(expr)?;
                match value.ty {
                    Ty::Int => Ok(value.code),
                    _ => Self::str_code(&value, expr),
                }
            }
            K::Strs | K::By => Err(Skip::warn("not an argument")),
        }
    }

    /// The arguments for the slice parameter of `assert_elements`.
    fn rest_argument(&mut self, items: &[&Expr]) -> TResult<String> {
        if items.is_empty() {
            return Err(Skip::error("no selectors are given"));
        }
        if let [Expr::List(list) | Expr::Tuple(list)] = items {
            let mut codes = Vec::new();
            for item in list {
                codes.push(self.str_arg(item)?);
            }
            return Ok(format!("&[{}]", codes.join(", ")));
        }
        if let [single] = items {
            let value = self.expr(single)?;
            if let Ty::List(element) = &value.ty {
                if element.is_text() {
                    return Ok(format!(
                        "&{}.iter().map(String::as_str).collect::<Vec<_>>()",
                        value.operand()
                    ));
                }
            }
            return Ok(format!("&[{}]", Self::str_code(&value, single)?));
        }
        let mut codes = Vec::new();
        for item in items {
            codes.push(self.str_arg(item)?);
        }
        Ok(format!("&[{}]", codes.join(", ")))
    }

    /// Python's `by=`: only the CSS strategy has an equal.
    fn check_by(by: &Expr) -> TResult<()> {
        let css = match by {
            Expr::Str(text) => text == "css selector",
            other => other
                .dotted()
                .is_some_and(|path| path == ["By", "CSS_SELECTOR"]),
        };
        if css {
            Ok(())
        } else {
            Err(Skip::error(format!(
                "by={} needs manual migration: BaseCase reads a selector that starts with '/' or '(' as XPath",
                py_repr(by)
            )))
        }
    }

    /// The Rust arguments of a call, in the order of its Python parameters.
    pub(super) fn rust_arguments(
        &mut self,
        params: &[P],
        bound: &Bound<'_>,
    ) -> TResult<Vec<String>> {
        let mut out = Vec::new();
        for (param, given) in params.iter().zip(&bound.slots) {
            match param.fill {
                Fill::Omit => {
                    if given.is_some() {
                        return Err(Skip::error(if param.name == "timeout" {
                            "a timeout needs manual migration: the assertion waits for the \
                             default timeout (set_timeout)"
                                .to_owned()
                        } else {
                            format!("the '{}' argument needs manual migration", param.name)
                        }));
                    }
                }
                Fill::Drop => {
                    if let Some(value) = given {
                        if param.kind == K::By {
                            Self::check_by(value)?;
                        } else {
                            self.notes.push(format!(
                                "the '{}' argument is dropped: the Rust call waits for the \
                                 default timeout",
                                param.name
                            ));
                        }
                    }
                }
                Fill::Required => {
                    let Some(value) = given else {
                        return Err(Skip::error(format!(
                            "the '{}' argument is missing",
                            param.name
                        )));
                    };
                    out.push(self.argument(param.kind, value)?);
                }
                Fill::Default(default) => match given {
                    Some(value) => out.push(self.argument(param.kind, value)?),
                    None => out.push(default.to_owned()),
                },
                Fill::Rest => out.push(self.rest_argument(&bound.rest)?),
            }
        }
        Ok(out)
    }

    /// `self.method` or `self.cdp.method`: the method name.
    pub(super) fn receiver_method(&self, func: &Expr) -> Option<String> {
        let Expr::Attr(base, method) = func else {
            return None;
        };
        match &**base {
            Expr::Name(name) if self.receivers.contains(name) => Some(method.clone()),
            Expr::Attr(inner, cdp) if cdp == "cdp" => {
                matches!(&**inner, Expr::Name(name) if self.receivers.contains(name))
                    .then(|| method.clone())
            }
            _ => None,
        }
    }

    /// Any call: a browser method, a builtin or a method of a value.
    pub(super) fn render_any(&mut self, func: &Expr, args: &[Arg]) -> TResult<Rendered> {
        if let Some(method) = self.receiver_method(func) {
            return self.render_call(&method, args);
        }
        match func {
            Expr::Name(name) => self.builtin(name, args),
            Expr::Attr(base, method) => self.value_method(base, method, args),
            _ => Err(Skip::warn("this call is not translated")),
        }
    }

    /// A call of a browser method.
    pub(super) fn render_call(&mut self, method: &str, args: &[Arg]) -> TResult<Rendered> {
        if self.cdp {
            return self.cdp_call(method, args);
        }
        match method {
            "assert_equal" | "assert_not_equal" => return self.assert_equal(method, args),
            "assert_in" | "assert_not_in" => return self.assert_in(method, args),
            "highlight_elements" => return self.highlight_elements(args),
            _ => {}
        }
        let Some(call) = lookup(method) else {
            return Err(Skip::warn(format!(
                "SeleniumBase method '{method}' needs manual migration"
            )));
        };
        self.table_call(call, args)
    }

    /// A call from the table.
    fn table_call(&mut self, call: &Call, args: &[Arg]) -> TResult<Rendered> {
        let bound = bind(call.params, args)?;
        let rust_args = self.rust_arguments(call.params, &bound)?.join(", ");
        let name = call.rust;
        let (mut code, awaits) = match call.flow {
            Flow::Await => (format!("sb.{name}({rust_args}).await?"), true),
            Flow::AwaitInfallible => (format!("sb.{name}({rust_args}).await"), true),
            Flow::Try => (format!("sb.{name}({rust_args})?"), false),
        };
        let ty = match call.ret {
            Ret::Unit => Ty::Unit,
            Ret::Str => Ty::String,
            Ret::OptStr => {
                code.push_str(".unwrap_or_default()");
                Ty::String
            }
            Ret::Bool => Ty::Bool,
            Ret::ListStr => Ty::List(Box::new(Ty::Str)),
            Ret::Alert => Ty::Alert,
            Ret::Other => Ty::Unknown,
        };
        Ok(Rendered { code, ty, awaits })
    }

    /// The two positional arguments of `assert_equal(first, second)`.
    fn two_values<'e>(args: &'e [Arg], name: &str) -> TResult<(&'e Expr, &'e Expr)> {
        let mut positional = Vec::new();
        for arg in args {
            match arg {
                Arg::Pos(value) => positional.push(value),
                _ => {
                    return Err(Skip::error(format!(
                        "{name} takes two values, written positionally"
                    )));
                }
            }
        }
        match positional.as_slice() {
            [first, second] => Ok((first, second)),
            _ => Err(Skip::error(format!("{name} takes two values"))),
        }
    }

    fn assert_equal(&mut self, method: &str, args: &[Arg]) -> TResult<Rendered> {
        let (first, second) = Self::two_values(args, method)?;
        let a = self.expr(first)?;
        let b = self.expr(second)?;
        if a.ty.is_text() != b.ty.is_text() {
            return Err(Skip::error(format!(
                "{method} compares a string with a value that is not one"
            )));
        }
        let (a, b) = (compare_operand(&a)?, compare_operand(&b)?);
        if method == "assert_not_equal" {
            return Ok(Rendered::plain(
                format!("sb.assert_true({a} != {b})?"),
                Ty::Unit,
            ));
        }
        Ok(Rendered::plain(
            format!("sb.assert_equal({a}, {b})?"),
            Ty::Unit,
        ))
    }

    fn assert_in(&mut self, method: &str, args: &[Arg]) -> TResult<Rendered> {
        let (first, second) = Self::two_values(args, method)?;
        let item = self.expr(first)?;
        let container = self.expr(second)?;
        let needle = Self::str_code(&item, first)?;
        let found = match &container.ty {
            Ty::Str | Ty::String => format!("{}.contains({needle})", container.operand()),
            Ty::List(element) if element.is_text() => format!(
                "{}.iter().any(|item| item == {needle})",
                container.operand()
            ),
            _ => {
                return Err(Skip::warn(format!(
                    "{method} is translated on a string or a list of strings"
                )));
            }
        };
        let found = if method == "assert_not_in" {
            format!("!{found}")
        } else {
            found
        };
        Ok(Rendered::plain(
            format!("sb.assert_true({found})?"),
            Ty::Unit,
        ))
    }

    fn highlight_elements(&mut self, args: &[Arg]) -> TResult<Rendered> {
        let mut selector = None;
        let mut chain = String::new();
        for arg in args {
            match arg {
                Arg::Pos(value) if selector.is_none() => selector = Some(self.str_arg(value)?),
                Arg::Kw(name, value) if name == "loops" || name == "limit" => {
                    let count = self.argument(K::Int, value)?;
                    chain.push_str(&format!(".{name}({count})"));
                }
                Arg::Kw(name, value) if name == "by" => Self::check_by(value)?,
                _ => {
                    return Err(Skip::error(
                        "highlight_elements is translated with a selector, loops and limit",
                    ));
                }
            }
        }
        let Some(selector) = selector else {
            return Err(Skip::error("the 'selector' argument is missing"));
        };
        Ok(Rendered::awaiting(
            format!("sb.highlight_elements({selector}){chain}.await?"),
            Ty::Unit,
        ))
    }

    /// A call of a name: `print`, `len`, `str`.
    fn builtin(&mut self, name: &str, args: &[Arg]) -> TResult<Rendered> {
        let mut values = Vec::new();
        for arg in args {
            match arg {
                Arg::Pos(value) => values.push(value),
                Arg::Kw(..) | Arg::Spread(_) => {
                    return Err(Skip::warn(format!(
                        "`{name}` with keyword arguments is not translated"
                    )));
                }
            }
        }
        match (name, values.as_slice()) {
            ("print", _) => {
                let mut pieces = Vec::new();
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        pieces.push(Piece::Lit(" ".to_owned()));
                    }
                    match value {
                        Expr::Str(text) => pieces.push(Piece::Lit(text.clone())),
                        other => {
                            let value = self.expr(other)?;
                            pieces.push(Piece::Arg(value));
                        }
                    }
                }
                let (template, args, _) = template_of(pieces);
                let code = if args.is_empty() && template.is_empty() {
                    "println!()".to_owned()
                } else if args.is_empty() {
                    format!("println!({})", quote(&template))
                } else {
                    format!("println!({}, {})", quote(&template), args.join(", "))
                };
                Ok(Rendered::plain(code, Ty::Unit))
            }
            ("len", [value]) => {
                let value = self.expr(value)?;
                match value.ty {
                    Ty::Str | Ty::String | Ty::List(_) => Ok(Rendered::plain(
                        format!("{}.len()", value.operand()),
                        Ty::Int,
                    )),
                    _ => Err(Skip::warn("len() is translated on a string or a list")),
                }
            }
            ("str", [value]) => {
                let value = self.expr(value)?;
                Ok(Rendered::plain(
                    format!("{}.to_string()", value.operand()),
                    Ty::String,
                ))
            }
            _ => Err(Skip::warn(format!(
                "the call of `{name}` is not translated"
            ))),
        }
    }

    /// `time.sleep(n)` and the methods of strings and locators.
    fn value_method(&mut self, base: &Expr, method: &str, args: &[Arg]) -> TResult<Rendered> {
        if matches!(base, Expr::Name(name) if name == "time") && method == "sleep" {
            let call = lookup("sleep").ok_or_else(|| Skip::warn("sleep is not translated"))?;
            return self.table_call(call, args);
        }
        if matches!(base, Expr::Name(name) if self.var(name).is_none()) {
            return Err(Skip::warn(format!(
                "`{}.{method}()` is not translated",
                py_repr(base)
            )));
        }

        let value = self.expr(base)?;
        let mut positional = Vec::new();
        for arg in args {
            match arg {
                Arg::Pos(item) => positional.push(item),
                _ => {
                    return Err(Skip::warn(format!(
                        "`.{method}()` with keyword arguments is not translated"
                    )));
                }
            }
        }
        if value.ty == Ty::Locator {
            return self.locator_method(&value, method, &positional);
        }
        if value.ty.is_text() {
            let text = value.operand();
            return match (method, positional.as_slice()) {
                ("lower", []) => Ok(Rendered::plain(
                    format!("{text}.to_lowercase()"),
                    Ty::String,
                )),
                ("upper", []) => Ok(Rendered::plain(
                    format!("{text}.to_uppercase()"),
                    Ty::String,
                )),
                ("strip", []) => Ok(Rendered::plain(format!("{text}.trim()"), Ty::Str)),
                ("lstrip", []) => Ok(Rendered::plain(format!("{text}.trim_start()"), Ty::Str)),
                ("rstrip", []) => Ok(Rendered::plain(format!("{text}.trim_end()"), Ty::Str)),
                ("replace", [from, to]) => {
                    let (from, to) = (self.str_arg(from)?, self.str_arg(to)?);
                    Ok(Rendered::plain(
                        format!("{text}.replace({from}, {to})"),
                        Ty::String,
                    ))
                }
                ("startswith", [prefix]) => {
                    let prefix = self.str_arg(prefix)?;
                    Ok(Rendered::plain(
                        format!("{text}.starts_with({prefix})"),
                        Ty::Bool,
                    ))
                }
                ("endswith", [suffix]) => {
                    let suffix = self.str_arg(suffix)?;
                    Ok(Rendered::plain(
                        format!("{text}.ends_with({suffix})"),
                        Ty::Bool,
                    ))
                }
                _ => Err(Skip::warn(format!(
                    "the string method `.{method}()` is not translated"
                ))),
            };
        }
        Err(Skip::warn(format!(
            "`{}.{method}()` is not translated",
            py_repr(base)
        )))
    }
}

/// The operands of `a + b + c`, left to right.
fn collect_add<'e>(expr: &'e Expr, out: &mut Vec<&'e Expr>) {
    if let Expr::Binary(left, BinOp::Add, right) = expr {
        collect_add(left, out);
        out.push(right);
    } else {
        out.push(expr);
    }
}

/// A number written out in the source (not a name).
fn expr_is_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Num { .. })
}
