//! The Python syntax the converter reads: a small tree of the statements and
//! expressions a SeleniumBase test is made of. Anything else is kept as its
//! source text ([`Expr::Other`], [`StmtKind::Unsupported`]) so the converter
//! can say what it did not read.

/// A Python expression.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Expr {
    /// A name: `sb`, `self`, `checkbox`.
    Name(String),
    /// A string literal, decoded (adjacent literals are joined).
    Str(String),
    /// An f-string: its literal pieces and the expressions in its braces.
    FStr(Vec<FPart>),
    /// A number, as written, and whether it is a float.
    Num { text: String, float: bool },
    /// `True` or `False`.
    Bool(bool),
    /// `None`.
    None,
    /// `[a, b]`.
    List(Vec<Expr>),
    /// `(a, b)` or `a, b`.
    Tuple(Vec<Expr>),
    /// `{k: v}`.
    Dict(Vec<(Expr, Expr)>),
    /// `value.name`.
    Attr(Box<Expr>, String),
    /// `func(args)`.
    Call(Box<Expr>, Vec<Arg>),
    /// `value[index]` (a slice is [`Expr::Other`]).
    Index(Box<Expr>, Box<Expr>),
    /// `not x`, `-x`.
    Unary(UnOp, Box<Expr>),
    /// `a + b`, `a and b`, `a == b`, `a in b`.
    Binary(Box<Expr>, BinOp, Box<Expr>),
    /// `a if test else b`, held as `(test, a, b)`.
    IfExp(Box<Expr>, Box<Expr>, Box<Expr>),
    /// Source the converter does not read (a lambda, a comprehension, a slice).
    Other(String),
}

/// A piece of an f-string.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum FPart {
    /// Literal text, decoded.
    Lit(String),
    /// A `{expression}`; a conversion or format spec is dropped.
    Expr(Expr),
}

/// An argument of a call.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Arg {
    /// `value`.
    Pos(Expr),
    /// `name=value`.
    Kw(String, Expr),
    /// `*value` or `**value`, as source text.
    Spread(String),
}

/// A unary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnOp {
    /// `not`.
    Not,
    /// `-`.
    Neg,
    /// `+`.
    Pos,
}

/// A binary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BinOp {
    /// `+`.
    Add,
    /// `-`.
    Sub,
    /// `*`.
    Mul,
    /// `/`.
    Div,
    /// `//`.
    FloorDiv,
    /// `%`.
    Mod,
    /// `and`.
    And,
    /// `or`.
    Or,
    /// `==`.
    Eq,
    /// `!=`.
    Ne,
    /// `<`.
    Lt,
    /// `<=`.
    Le,
    /// `>`.
    Gt,
    /// `>=`.
    Ge,
    /// `in`.
    In,
    /// `not in`.
    NotIn,
    /// `is`.
    Is,
    /// `is not`.
    IsNot,
    /// Any other operator (`**`, `|`, `&`, `<<`, `@`): kept as source.
    Other,
}

/// A statement and where it is in the source.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Stmt {
    /// 1-based line of its first token.
    pub(super) line: usize,
    /// 1-based line of its last token.
    pub(super) end_line: usize,
    /// Byte range of its source.
    pub(super) span: (usize, usize),
    /// What it is.
    pub(super) kind: StmtKind,
}

/// What a statement is.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum StmtKind {
    /// An expression statement, a call most of the time.
    Expr(Expr),
    /// `a = b = value`: the targets and the value.
    Assign { targets: Vec<Expr>, value: Expr },
    /// `a += value`.
    AugAssign {
        target: Expr,
        op: String,
        value: Expr,
    },
    /// `a: int = value` (the annotation is dropped).
    AnnAssign { target: Expr, value: Option<Expr> },
    /// `pass`.
    Pass,
    /// `break`.
    Break,
    /// `continue`.
    Continue,
    /// `return value`.
    Return(Option<Expr>),
    /// `assert test, message`.
    Assert { test: Expr, message: Option<Expr> },
    /// `raise value`.
    Raise(Option<Expr>),
    /// `if` with its `elif` chain folded into `orelse`.
    If {
        test: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
    },
    /// `while`.
    While {
        test: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
    },
    /// `for`.
    For {
        target: Expr,
        iter: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
    },
    /// `try` with its handlers.
    Try {
        body: Vec<Stmt>,
        handlers: Vec<Handler>,
        orelse: Vec<Stmt>,
        finalbody: Vec<Stmt>,
    },
    /// `with item as target, ...:`.
    With {
        items: Vec<(Expr, Option<Expr>)>,
        body: Vec<Stmt>,
    },
    /// `def`.
    FuncDef(FuncDef),
    /// `class`.
    ClassDef(ClassDef),
    /// `import x` or `from x import y`: the text after the keyword.
    Import(String),
    /// `global`, `nonlocal`, `del`: kept as source.
    Other(String),
    /// A statement that could not be read: its source.
    Unsupported(String),
}

/// An `except` clause.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Handler {
    /// The exception type, `None` for a bare `except:`.
    pub(super) ty: Option<Expr>,
    /// The `as name`.
    pub(super) name: Option<String>,
    /// The handler body.
    pub(super) body: Vec<Stmt>,
}

/// A parameter of a `def`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Param {
    /// Its name.
    pub(super) name: String,
    /// Its default, if it has one.
    pub(super) default: Option<Expr>,
    /// `*args` or `**kwargs`.
    pub(super) starred: bool,
}

/// A `def`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct FuncDef {
    /// Its name.
    pub(super) name: String,
    /// Its parameters.
    pub(super) params: Vec<Param>,
    /// Its decorators, without the `@`.
    pub(super) decorators: Vec<Expr>,
    /// Its body.
    pub(super) body: Vec<Stmt>,
}

/// A `class`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ClassDef {
    /// Its name.
    pub(super) name: String,
    /// Its base classes.
    pub(super) bases: Vec<Expr>,
    /// Its body.
    pub(super) body: Vec<Stmt>,
}

impl Expr {
    /// The dotted name an expression spells (`self.driver.get` is
    /// `["self", "driver", "get"]`), if it is only names and attributes.
    pub(super) fn dotted(&self) -> Option<Vec<&str>> {
        match self {
            Self::Name(name) => Some(vec![name.as_str()]),
            Self::Attr(base, attr) => {
                let mut path = base.dotted()?;
                path.push(attr.as_str());
                Some(path)
            }
            _ => None,
        }
    }
}
