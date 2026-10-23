//! The usage spec of `sbrs`, read from its KDL.
//!
//! `completions/sbrs.usage.kdl` is written by the `usage` crate from the
//! declarations of the command line and checked by a test of the CLI crate, so
//! it is the one description of the commands that cannot drift. The library may
//! not depend on a command-line crate (`tests/library_manifest.rs`), so this file
//! reads the part of KDL the spec uses itself: nodes with arguments and
//! `key=value` properties, child blocks, quoted, raw and multi-line strings, the
//! keywords `#true`, `#false` and `#null`, and comments.
//!
//! [`Spec::parse`] gives the commands, flags and arguments with their help,
//! defaults, choices and aliases; `help.rs` renders them.

use std::fmt;

/// A value of a node: text, a flag (`#true`, `#false`) or nothing (`#null`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Value {
    /// A string, however it was written.
    Text(String),
    /// `#true` or `#false`.
    Flag(bool),
    /// `#null`.
    Null,
}

impl Value {
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Flag(_) | Self::Null => None,
        }
    }
}

/// One node of a KDL document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Node {
    /// The name of the node: the first word of its line.
    pub name: String,
    /// The values after the name, in order.
    pub args: Vec<Value>,
    /// The key=value pairs, in order.
    pub props: Vec<(String, Value)>,
    /// The nodes of the block in braces, if the node has one.
    pub children: Vec<Node>,
}

impl Node {
    fn arg_text(&self, index: usize) -> Option<&str> {
        self.args.get(index).and_then(Value::text)
    }

    fn prop(&self, key: &str) -> Option<&Value> {
        self.props
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    fn prop_text(&self, key: &str) -> Option<&str> {
        self.prop(key).and_then(Value::text)
    }

    fn prop_true(&self, key: &str) -> bool {
        matches!(self.prop(key), Some(Value::Flag(true)))
    }

    fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|child| child.name == name)
    }

    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Self> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }
}

/// Why a document could not be read, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SpecError {
    /// The line the reader was on, counted from 1.
    pub line: usize,
    /// What was wrong.
    pub reason: String,
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

impl std::error::Error for SpecError {}

type Parsed<T> = Result<T, SpecError>;

/// The text of a document and where the reader is in it.
struct Reader<'a> {
    text: &'a str,
    at: usize,
    line: usize,
}

/// Characters that end a bare word.
const BARE_END: [char; 9] = [';', '{', '}', '=', '"', '(', ')', '[', ']'];

impl<'a> Reader<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            at: 0,
            line: 1,
        }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    /// Moves `count` bytes on, counting the lines they end.
    fn skip_bytes(&mut self, count: usize) {
        let end = self.at + count;
        self.line += self.text[self.at..end].matches('\n').count();
        self.at = end;
    }

    /// Moves one character on.
    fn next_char(&mut self) {
        if let Some(c) = self.peek() {
            self.skip_bytes(c.len_utf8());
        }
    }

    fn error<T>(&self, reason: impl Into<String>) -> Parsed<T> {
        Err(SpecError {
            line: self.line,
            reason: reason.into(),
        })
    }

    /// Blanks, comments inside a line, and a `\` that continues the line.
    fn skip_inline(&mut self) -> Parsed<()> {
        loop {
            match self.peek() {
                Some(' ' | '\t' | '\u{feff}') => self.next_char(),
                Some('\\') => {
                    self.next_char();
                    while matches!(self.peek(), Some(' ' | '\t')) {
                        self.next_char();
                    }
                    if self.starts_with("//") {
                        self.skip_line_comment();
                    }
                    if self.starts_with("\r\n") {
                        self.skip_bytes(2);
                    } else if self.starts_with("\n") {
                        self.skip_bytes(1);
                    } else if self.peek().is_some() {
                        return self.error("a `\\` must end its line");
                    }
                }
                Some('/') if self.starts_with("/*") => self.skip_block_comment()?,
                _ => return Ok(()),
            }
        }
    }

    /// The rest of the line, up to its break.
    fn skip_line_comment(&mut self) {
        let length = self.rest().find('\n').unwrap_or(self.rest().len());
        self.skip_bytes(length);
    }

    /// A `/* ... */` comment, which may hold others.
    fn skip_block_comment(&mut self) -> Parsed<()> {
        let mut depth = 0_usize;
        loop {
            if self.starts_with("/*") {
                depth += 1;
                self.skip_bytes(2);
            } else if self.starts_with("*/") {
                depth -= 1;
                self.skip_bytes(2);
                if depth == 0 {
                    return Ok(());
                }
            } else if self.peek().is_some() {
                self.next_char();
            } else {
                return self.error("a `/*` comment is never closed");
            }
        }
    }

    /// What may lie between nodes: blanks, line breaks, `;` and comments.
    fn skip_between_nodes(&mut self) -> Parsed<()> {
        loop {
            self.skip_inline()?;
            match self.peek() {
                Some('\n' | '\r' | ';') => self.next_char(),
                Some('/') if self.starts_with("//") => self.skip_line_comment(),
                _ => return Ok(()),
            }
        }
    }

    fn nodes(&mut self, nested: bool) -> Parsed<Vec<Node>> {
        let mut nodes = Vec::new();
        loop {
            self.skip_between_nodes()?;
            match self.peek() {
                None if nested => return self.error("a `{` is never closed"),
                None => return Ok(nodes),
                Some('}') if nested => {
                    self.next_char();
                    return Ok(nodes);
                }
                Some('}') => return self.error("a `}` closes nothing"),
                Some(_) => {}
            }
            if self.starts_with("/-") {
                // A node commented out with `/-`: read it, keep nothing.
                self.skip_bytes(2);
                self.skip_inline()?;
                self.node()?;
                continue;
            }
            nodes.push(self.node()?);
        }
    }

    fn node(&mut self) -> Parsed<Node> {
        let name = match self.token()? {
            Token::Text(name) => name,
            Token::Keyword(_) => return self.error("a node name cannot be a keyword"),
        };
        let mut node = Node {
            name,
            ..Node::default()
        };
        loop {
            let before = self.at;
            self.skip_inline()?;
            let separated = self.at > before;
            match self.peek() {
                None | Some('\n' | '\r' | ';' | '}') => return Ok(node),
                Some('/') if self.starts_with("//") => return Ok(node),
                Some('{') => {
                    self.next_char();
                    node.children = self.nodes(true)?;
                    self.skip_inline()?;
                    return Ok(node);
                }
                Some(c) if !separated => {
                    return self.error(format!(
                        "unexpected {c:?}: arguments and properties are separated by blanks"
                    ));
                }
                Some(_) => {}
            }
            if self.starts_with("/-") {
                // An argument, property or block commented out with `/-`.
                self.skip_bytes(2);
                self.skip_inline()?;
                if self.peek() == Some('{') {
                    self.next_char();
                    self.nodes(true)?;
                } else {
                    self.entry(&mut Node::default())?;
                }
                continue;
            }
            self.entry(&mut node)?;
        }
    }

    /// An argument, or a `key=value` property, added to `node`.
    fn entry(&mut self, node: &mut Node) -> Parsed<()> {
        let first = self.token()?;
        if self.peek() == Some('=') {
            let Token::Text(key) = first else {
                return self.error("a property name must be a string");
            };
            self.next_char();
            let value = self.token()?.into_value();
            node.props.push((key, value));
        } else {
            node.args.push(first.into_value());
        }
        Ok(())
    }

    /// A string (bare, quoted, raw or multi-line) or a `#keyword`.
    fn token(&mut self) -> Parsed<Token> {
        match self.peek() {
            Some('"') => Ok(Token::Text(self.quoted(0)?)),
            Some('#') => self.hash(),
            Some('(') => {
                // A type annotation, `(u8)5`: it names a type this reader has no use for.
                let close = self.rest().find(')');
                match close {
                    Some(at) => self.skip_bytes(at + 1),
                    None => return self.error("a `(` is never closed"),
                }
                self.token()
            }
            Some(_) => Ok(Token::Text(self.bare()?)),
            None => self.error("a value is missing"),
        }
    }

    fn bare(&mut self) -> Parsed<String> {
        let start = self.at;
        while let Some(c) = self.peek() {
            let comment = c == '/' && (self.starts_with("//") || self.starts_with("/*"));
            if c.is_whitespace() || c == '\\' || BARE_END.contains(&c) || comment {
                break;
            }
            self.next_char();
        }
        if self.at == start {
            return match self.peek() {
                Some(c) => self.error(format!("unexpected {c:?}")),
                None => self.error("a value is missing"),
            };
        }
        Ok(self.text[start..self.at].to_owned())
    }

    /// `#true`, `#false`, `#null`, or a raw string `#"..."#`.
    fn hash(&mut self) -> Parsed<Token> {
        let mut hashes = 0;
        while self.peek() == Some('#') {
            self.next_char();
            hashes += 1;
        }
        if self.peek() == Some('"') {
            return Ok(Token::Text(self.quoted(hashes)?));
        }
        let word = self.bare()?;
        match (hashes, word.as_str()) {
            (1, "true") => Ok(Token::Keyword(Value::Flag(true))),
            (1, "false") => Ok(Token::Keyword(Value::Flag(false))),
            (1, "null") => Ok(Token::Keyword(Value::Null)),
            (1, "inf" | "-inf" | "nan") => Ok(Token::Keyword(Value::Text(format!("#{word}")))),
            _ => self.error(format!("`{}{word}` is not a keyword", "#".repeat(hashes))),
        }
    }

    /// A string in quotes, with `hashes` `#` after the closing quote (and before the
    /// opening one) when it is raw. The reader is at the opening quote.
    fn quoted(&mut self, hashes: usize) -> Parsed<String> {
        self.skip_bytes(1);
        if self.starts_with("\"\"") {
            self.skip_bytes(2);
            return self.multiline(hashes);
        }
        let closing = format!("\"{}", "#".repeat(hashes));
        let start = self.at;
        while !self.starts_with(&closing) {
            match self.peek() {
                None => return self.error("a string is never closed"),
                Some('\n') => return self.error("a string runs past the end of its line"),
                Some('\\') if hashes == 0 => {
                    // The escaped character cannot close the string.
                    self.next_char();
                    if self.peek().is_none() {
                        return self.error("a string is never closed");
                    }
                    self.next_char();
                }
                Some(_) => self.next_char(),
            }
        }
        let content = &self.text[start..self.at];
        self.skip_bytes(closing.len());
        if hashes == 0 {
            self.unescape(content)
        } else {
            Ok(content.to_owned())
        }
    }

    /// The lines of a `"""` string, after its opening quotes, without the
    /// indentation of the line that closes it.
    fn multiline(&mut self, hashes: usize) -> Parsed<String> {
        let Some(break_at) = self.rest().find('\n') else {
            return self.error("a multi-line string is never closed");
        };
        if !self.rest()[..break_at].trim().is_empty() {
            return self.error("a multi-line string starts on a new line");
        }
        self.skip_bytes(break_at + 1);
        let closing = format!("\"\"\"{}", "#".repeat(hashes));
        let mut lines: Vec<&str> = Vec::new();
        let indent = loop {
            let end = self.rest().find('\n').unwrap_or(self.rest().len());
            let line = self.rest()[..end].trim_end_matches('\r');
            let content = line.trim_start_matches([' ', '\t']);
            if content.starts_with(&closing) {
                let indent = &line[..line.len() - content.len()];
                self.skip_bytes(indent.len() + closing.len());
                break indent;
            }
            if end == self.rest().len() {
                return self.error("a multi-line string is never closed");
            }
            lines.push(line);
            self.skip_bytes(end + 1);
        };
        let text = lines
            .iter()
            .map(|line| {
                line.strip_prefix(indent)
                    .unwrap_or_else(|| line.trim_start())
            })
            .collect::<Vec<_>>()
            .join("\n");
        if hashes == 0 {
            self.unescape(&text)
        } else {
            Ok(text)
        }
    }

    /// `content` of a string that is not raw, with its escapes resolved.
    fn unescape(&self, content: &str) -> Parsed<String> {
        let mut out = String::with_capacity(content.len());
        let mut chars = content.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('b') => out.push('\u{8}'),
                Some('f') => out.push('\u{c}'),
                Some('s') => out.push(' '),
                Some('u') => {
                    if chars.next() != Some('{') {
                        return self.error("`\\u` is followed by `{`");
                    }
                    let digits: String = chars.by_ref().take_while(|c| *c != '}').collect();
                    let Some(ch) = u32::from_str_radix(&digits, 16)
                        .ok()
                        .and_then(char::from_u32)
                    else {
                        return self.error(format!("`\\u{{{digits}}}` is not a character"));
                    };
                    out.push(ch);
                }
                Some(c) if c.is_whitespace() => {
                    // A `\` before blanks and line breaks joins the lines.
                    while chars.peek().is_some_and(|next| next.is_whitespace()) {
                        chars.next();
                    }
                }
                Some(c) => return self.error(format!("`\\{c}` is not an escape")),
                None => return self.error("a string ends with a `\\`"),
            }
        }
        Ok(out)
    }
}

/// What the reader found where a value may stand.
enum Token {
    /// A string of any of the four kinds.
    Text(String),
    /// `#true`, `#false`, `#null` and the like.
    Keyword(Value),
}

impl Token {
    fn into_value(self) -> Value {
        match self {
            Self::Text(text) => Value::Text(text),
            Self::Keyword(value) => value,
        }
    }
}

/// The nodes of a KDL document.
pub(super) fn parse_document(source: &str) -> Parsed<Vec<Node>> {
    Reader::new(source).nodes(false)
}

// ----------------------------------------------------------------------
// The spec
// ----------------------------------------------------------------------

/// One value a flag or an argument may take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Choice {
    /// The value, as it is typed.
    pub name: String,
    /// What the value means, when the spec says.
    pub help: Option<String>,
}

/// A flag, with the spellings, value and attributes the spec gives it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Flag {
    /// Every spelling, as the spec lists them: `-n`, `--threads`.
    pub names: Vec<String>,
    /// The name of the value it takes, such as `<URL>`; `None` for a switch.
    pub value: Option<String>,
    /// The one-line help.
    pub help: String,
    /// The long help, which starts with the one-line help.
    pub long_help: Option<String>,
    /// The value used when the flag is not given.
    pub default: Option<String>,
    /// Whether the flag must be given.
    pub required: bool,
    /// Whether it may be given more than once.
    pub repeatable: bool,
    /// The values it accepts, when the spec lists them.
    pub choices: Vec<Choice>,
    /// The flags it cannot be combined with.
    pub conflicts: Vec<String>,
    /// `--help` and `--version`.
    pub builtin: bool,
}

impl Flag {
    /// The spellings with one dash and one letter.
    pub(super) fn shorts(&self) -> Vec<&str> {
        self.names
            .iter()
            .map(String::as_str)
            .filter(|name| name.len() == 2 && name.starts_with('-') && !name.starts_with("--"))
            .collect()
    }

    /// The spellings that start with two dashes: the first is the name, the others
    /// are aliases.
    pub(super) fn longs(&self) -> Vec<&str> {
        self.names
            .iter()
            .map(String::as_str)
            .filter(|name| name.starts_with("--"))
            .collect()
    }
}

/// A positional argument.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Arg {
    /// As the spec writes it: `<URL>`, `[FILTER]`, `[ARGS]...`.
    pub name: String,
    /// The one-line help.
    pub help: String,
    /// The long help, which starts with the one-line help.
    pub long_help: Option<String>,
    /// The value used when the argument is not given.
    pub default: Option<String>,
    /// The values it accepts, when the spec lists them.
    pub choices: Vec<Choice>,
}

/// A command, or the program itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Command {
    /// The names from the program down, `plugin list`; empty for the program.
    pub path: String,
    /// The one-line help.
    pub help: String,
    /// The long help, which starts with the one-line help.
    pub long_help: Option<String>,
    /// The other names that run the command.
    pub aliases: Vec<String>,
    /// The flags it takes, in the order of the spec.
    pub flags: Vec<Flag>,
    /// The positional arguments it takes, in order.
    pub args: Vec<Arg>,
    /// The commands below it.
    pub subcommands: Vec<Command>,
}

impl Command {
    /// This command and every command below it, depth first.
    fn collect<'a>(&'a self, into: &mut Vec<&'a Self>) {
        into.push(self);
        for sub in &self.subcommands {
            sub.collect(into);
        }
    }

    /// The flags, arguments, aliases and subcommands in the children of a node;
    /// `use NAME` takes in the top-level `flagset NAME` of `top`.
    fn fill(&mut self, nodes: &[Node], top: &[Node]) {
        for node in nodes {
            match node.name.as_str() {
                "flag" => self.flags.push(flag_of(node)),
                "use" => {
                    let named =
                        |set: &&Node| set.name == "flagset" && set.arg_text(0) == node.arg_text(0);
                    for set in top.iter().filter(named) {
                        self.fill(&set.children, top);
                    }
                }
                "arg" => self.args.push(arg_of(node)),
                "alias" => self
                    .aliases
                    .extend(node.args.iter().filter_map(Value::text).map(str::to_owned)),
                "long_help" => self.long_help = node.arg_text(0).map(str::to_owned),
                "cmd" => {
                    let name = node.arg_text(0).unwrap_or_default();
                    let path = if self.path.is_empty() {
                        name.to_owned()
                    } else {
                        format!("{} {name}", self.path)
                    };
                    let mut sub = Self {
                        path,
                        help: node.prop_text("help").unwrap_or_default().to_owned(),
                        ..Self::default()
                    };
                    sub.fill(&node.children, top);
                    self.subcommands.push(sub);
                }
                _ => {}
            }
        }
    }
}

/// The values listed under a `choices` node: `choices { choice a; choice b help=".." }`
/// or `choices "a" "b"`.
fn choices_of(parent: &Node) -> Vec<Choice> {
    let mut choices = Vec::new();
    for list in parent.children_named("choices") {
        choices.extend(list.args.iter().filter_map(Value::text).map(|name| Choice {
            name: name.to_owned(),
            help: None,
        }));
        choices.extend(list.children_named("choice").map(|choice| Choice {
            name: choice.arg_text(0).unwrap_or_default().to_owned(),
            help: choice.prop_text("help").map(str::to_owned),
        }));
    }
    choices
}

fn flag_of(node: &Node) -> Flag {
    let value_node = node.child("arg");
    let mut conflicts: Vec<String> = node
        .prop_text("conflicts")
        .map(|list| list.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default();
    for child in node.children_named("conflicts") {
        conflicts.extend(child.args.iter().filter_map(Value::text).map(str::to_owned));
    }
    let mut choices = choices_of(node);
    if let Some(value) = value_node {
        choices.extend(choices_of(value));
    }
    Flag {
        names: node
            .arg_text(0)
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        value: value_node
            .and_then(|value| value.arg_text(0))
            .map(str::to_owned),
        help: node.prop_text("help").unwrap_or_default().to_owned(),
        long_help: node
            .child("long_help")
            .and_then(|child| child.arg_text(0))
            .map(str::to_owned),
        default: node.prop_text("default").map(str::to_owned),
        required: node.prop_true("required"),
        repeatable: node.prop_true("var"),
        choices,
        conflicts,
        builtin: node.prop_true("builtin"),
    }
}

fn arg_of(node: &Node) -> Arg {
    Arg {
        name: node.arg_text(0).unwrap_or_default().to_owned(),
        help: node.prop_text("help").unwrap_or_default().to_owned(),
        long_help: node
            .child("long_help")
            .and_then(|child| child.arg_text(0))
            .map(str::to_owned),
        default: node.prop_text("default").map(str::to_owned),
        choices: choices_of(node),
    }
}

/// The usage spec of a program: its name, version and commands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Spec {
    /// The name of the program: `sbrs`.
    pub bin: String,
    /// The version line of the program.
    pub version: Option<String>,
    /// What the program is, in one line.
    pub about: String,
    /// What the program is, at length.
    pub long_about: Option<String>,
    /// The program itself: its global flags and its commands.
    pub root: Command,
}

impl Spec {
    /// Reads a usage spec.
    ///
    /// # Errors
    ///
    /// A [`SpecError`] with the line of the first thing that is not KDL, or a `{`
    /// that is never closed.
    pub(super) fn parse(source: &str) -> Parsed<Self> {
        let nodes = parse_document(source)?;
        let text = |name: &str| {
            nodes
                .iter()
                .find(|node| node.name == name)
                .and_then(|node| node.arg_text(0))
        };
        let mut spec = Self {
            bin: text("bin")
                .or_else(|| text("name"))
                .unwrap_or_default()
                .to_owned(),
            version: text("version").map(str::to_owned),
            about: text("about").unwrap_or_default().to_owned(),
            long_about: text("long_about").map(str::to_owned),
            root: Command::default(),
        };
        spec.root.help.clone_from(&spec.about);
        spec.root.fill(&nodes, &nodes);
        Ok(spec)
    }

    /// Every command below the program, depth first, in the order of the spec.
    pub(super) fn commands(&self) -> Vec<&Command> {
        let mut all = Vec::new();
        for command in &self.root.subcommands {
            command.collect(&mut all);
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(source: &str) -> Node {
        let mut nodes = parse_document(source).unwrap();
        assert_eq!(nodes.len(), 1, "{nodes:?}");
        nodes.remove(0)
    }

    fn text(value: &str) -> Value {
        Value::Text(value.to_owned())
    }

    #[test]
    fn a_node_has_arguments_properties_and_children() {
        let node = one("flag \"-n --threads\" help=\"How many\" default=4 {\n    arg <N>\n}\n");

        assert_eq!(node.name, "flag");
        assert_eq!(node.args, [text("-n --threads")]);
        assert_eq!(
            node.props,
            [
                ("help".to_owned(), text("How many")),
                ("default".to_owned(), text("4"))
            ]
        );
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].name, "arg");
        assert_eq!(node.children[0].args, [text("<N>")]);
    }

    #[test]
    fn keywords_are_flags_and_nothing() {
        let node = one("x a=#true b=#false c=#null #true");

        assert_eq!(node.prop("a"), Some(&Value::Flag(true)));
        assert_eq!(node.prop("b"), Some(&Value::Flag(false)));
        assert_eq!(node.prop("c"), Some(&Value::Null));
        assert_eq!(node.args, [Value::Flag(true)]);
        assert!(node.prop_true("a") && !node.prop_true("b") && !node.prop_true("c"));
    }

    #[test]
    fn bare_words_may_hold_dashes_dots_and_brackets_of_angle_kind() {
        let node =
            one("arg <URL> conflicts=--example default=page.pdf delimiter=, help=JavaScript.");

        assert_eq!(node.args, [text("<URL>")]);
        assert_eq!(node.prop_text("conflicts"), Some("--example"));
        assert_eq!(node.prop_text("default"), Some("page.pdf"));
        assert_eq!(node.prop_text("delimiter"), Some(","));
        assert_eq!(node.prop_text("help"), Some("JavaScript."));
    }

    #[test]
    fn quoted_strings_resolve_their_escapes_and_keep_punctuation() {
        let node = one(r#"x "a \"b\" \\ \n \u{e9} {c}; [d]" k="\t.""#);

        assert_eq!(node.args, [text("a \"b\" \\ \n é {c}; [d]")]);
        assert_eq!(node.prop_text("k"), Some("\t."));
        // A `\` before a line break joins the lines.
        let joined = one("x \"one \\\n   two\"");
        assert_eq!(joined.args, [text("one two")]);
    }

    #[test]
    fn raw_strings_keep_every_character() {
        let node = one("x #\"a \"quoted\" \\n\"# ##\"b \"# c\"##");

        assert_eq!(node.args, [text(r#"a "quoted" \n"#), text(r##"b "# c"##)]);
    }

    #[test]
    fn a_multi_line_string_loses_the_indentation_of_its_closing_line() {
        let source = "x #\"\"\"\n  first\n    second\n\n  last\n  \"\"\"# after=1\n";

        let node = one(source);

        assert_eq!(node.args, [text("first\n  second\n\nlast")]);
        assert_eq!(node.prop_text("after"), Some("1"));
        let flush = one("long_help #\"\"\"\nA\n\nB \\n\n\"\"\"#\n");
        assert_eq!(flush.args, [text("A\n\nB \\n")]);
    }

    #[test]
    fn comments_and_slashdash_hide_what_they_cover() {
        let source = "// a comment\nkeep a /* inline */ b // trailing\n/- gone x\n/- also gone {\n}\nnext /- 1 2 /- k=v /- {\n  inner\n}\n/* block\n   comment */\nlast\n";

        let nodes = parse_document(source).unwrap();

        let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(names, ["keep", "next", "last"]);
        assert_eq!(nodes[0].args, [text("a"), text("b")]);
        assert_eq!(nodes[1].args, [text("2")]);
        assert!(nodes[1].props.is_empty() && nodes[1].children.is_empty());
    }

    #[test]
    fn nodes_end_at_a_semicolon_and_children_may_share_a_line() {
        let nodes = parse_document("a 1; b 2 { c; d 3 }\ne\n").unwrap();

        let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "e"]);
        let inner: Vec<&str> = nodes[1].children.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(inner, ["c", "d"]);
    }

    #[test]
    fn a_continued_line_is_one_node() {
        let node = one("x 1 \\\n  2 \\\n  k=3\n");

        assert_eq!(node.args, [text("1"), text("2")]);
        assert_eq!(node.prop_text("k"), Some("3"));
    }

    #[test]
    fn what_is_not_kdl_is_an_error_with_the_line() {
        for (source, line, reason) in [
            ("a {\n  b\n", 3, "never closed"),
            ("a }\n", 1, "closes nothing"),
            ("a \"open\nb\n", 1, "past the end of its line"),
            ("\n\na \"open", 3, "never closed"),
            ("a #nope\n", 1, "not a keyword"),
            ("a x\"y\"\n", 1, "unexpected"),
            ("a #\"\"\"\ntext\n", 3, "never closed"),
            ("a \"\\q\"\n", 1, "not an escape"),
            ("a /* open\n", 2, "never closed"),
            ("a (u8\n", 1, "never closed"),
        ] {
            let error = parse_document(source).unwrap_err();
            assert!(
                error.reason.contains(reason),
                "{source:?}: expected {reason:?}, got {error}"
            );
            assert_eq!(error.line, line, "{source:?}: {error}");
        }
    }

    #[test]
    fn block_comments_nest_and_a_type_annotation_is_skipped() {
        let nodes = parse_document("a /* outer /* inner */ still outer */ 1 (u8)2\nb\n").unwrap();

        assert_eq!(nodes[0].args, [text("1"), text("2")]);
        assert_eq!(nodes[1].name, "b");
    }

    #[test]
    fn every_escape_of_a_quoted_string_is_resolved() {
        let node = one(r#"x "a\sb\tc\u{1F600}\b\f\r\\" "#);

        assert_eq!(node.args, [text("a b\tc\u{1F600}\u{8}\u{c}\r\\")]);
    }

    #[test]
    fn the_non_finite_numbers_are_kept_as_text() {
        let node = one("x #inf #-inf #nan");

        assert_eq!(node.args, [text("#inf"), text("#-inf"), text("#nan")]);
    }

    #[test]
    fn more_text_that_is_not_kdl_is_an_error_with_the_line() {
        for (source, line, reason) in [
            ("a \\ b\n", 1, "must end its line"),
            ("a \"\\u12\"\n", 1, "followed by"),
            ("a \"\\u{110000}\"\n", 1, "not a character"),
            ("a #\"\"\"x\n", 1, "starts on a new line"),
            ("#true 1\n", 1, "cannot be a keyword"),
            ("a #true=1\n", 1, "must be a string"),
        ] {
            let error = parse_document(source).unwrap_err();
            assert!(
                error.reason.contains(reason),
                "{source:?}: expected {reason:?}, got {error}"
            );
            assert_eq!(error.line, line, "{source:?}: {error}");
        }
    }

    #[test]
    fn choices_may_be_listed_as_arguments_and_conflicts_as_a_child() {
        let spec = Spec::parse(
            "bin demo\nflag --mode help=\"m\" {\n    arg <MODE>\n    choices a b\n}\n\
             flag --other help=\"o\" {\n    conflicts --mode --third\n}\n",
        )
        .unwrap();
        let flags = &spec.root.flags;

        let names: Vec<&str> = flags[0].choices.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(flags[1].conflicts, ["--mode", "--third"]);
    }

    const SMALL: &str = r##"name demo
bin demo
version "1.2 (demo)"
about "A demo"
long_about "A demo program."
flag --webdriver help="Server URL" {
    arg <URL>
}
flag "-n --threads" help="Threads" default="4" {
    arg <N>
}
flag --features help="Cargo features" var=#true delimiter=, {
    arg <LIST>
}
flag --browser help="Which" required=#true {
    arg <BROWSER> {
        choices {
            choice chrome
            choice brave help="Brave."
        }
    }
}
flag "--rcs --reuse-class-session --rs" help="Share" bool_value=#true conflicts=--other
flag "-h --help" help="Print help" action=help builtin=#true
cmd open help="Open a URL." {
    flag "-h --help" help="Print help" action=help builtin=#true
    arg <URL> help="The URL."
}
cmd encrypt help="Encrypt." {
    alias obfuscate
    long_help #"""
Encrypt.

The key is in the environment.
"""#
    arg "[TEXT]" help="The text." default=stdin
}
cmd plugin help="Plugins." subcommand_required=#true {
    cmd list help="List them." {
        arg "[NAMES]..." help="Some names."
    }
    cmd info help="Describe one."
}
"##;

    #[test]
    fn a_spec_has_its_program_flags_and_commands() {
        let spec = Spec::parse(SMALL).unwrap();

        assert_eq!(spec.bin, "demo");
        assert_eq!(spec.version.as_deref(), Some("1.2 (demo)"));
        assert_eq!(spec.about, "A demo");
        assert_eq!(spec.long_about.as_deref(), Some("A demo program."));
        assert_eq!(spec.root.flags.len(), 6);
        let paths: Vec<&str> = spec.commands().iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            paths,
            ["open", "encrypt", "plugin", "plugin list", "plugin info"]
        );
    }

    #[test]
    fn a_flag_has_its_spellings_value_default_and_attributes() {
        let spec = Spec::parse(SMALL).unwrap();
        let flags = &spec.root.flags;

        assert_eq!(flags[0].names, ["--webdriver"]);
        assert_eq!(flags[0].value.as_deref(), Some("<URL>"));
        assert_eq!(flags[0].help, "Server URL");
        assert_eq!(flags[1].shorts(), ["-n"]);
        assert_eq!(flags[1].longs(), ["--threads"]);
        assert_eq!(flags[1].default.as_deref(), Some("4"));
        assert!(flags[2].repeatable && !flags[2].required);
        assert!(flags[3].required);
        assert_eq!(
            flags[3].choices,
            [
                Choice {
                    name: "chrome".into(),
                    help: None
                },
                Choice {
                    name: "brave".into(),
                    help: Some("Brave.".into())
                },
            ]
        );
        assert_eq!(flags[4].longs(), ["--rcs", "--reuse-class-session", "--rs"]);
        assert_eq!(flags[4].value, None);
        assert_eq!(flags[4].conflicts, ["--other"]);
        assert!(flags[5].builtin && !flags[0].builtin);
    }

    #[test]
    fn a_command_has_its_aliases_long_help_arguments_and_subcommands() {
        let spec = Spec::parse(SMALL).unwrap();
        let commands = spec.commands();

        assert_eq!(commands[0].help, "Open a URL.");
        assert_eq!(commands[0].args[0].name, "<URL>");
        assert_eq!(commands[0].args[0].help, "The URL.");
        assert_eq!(commands[0].flags.len(), 1);
        assert_eq!(commands[1].aliases, ["obfuscate"]);
        assert_eq!(
            commands[1].long_help.as_deref(),
            Some("Encrypt.\n\nThe key is in the environment.")
        );
        assert_eq!(commands[1].args[0].default.as_deref(), Some("stdin"));
        assert_eq!(commands[2].subcommands.len(), 2);
        assert_eq!(commands[3].args[0].name, "[NAMES]...");
    }

    #[test]
    fn the_embedded_spec_of_sbrs_reads_and_agrees_with_its_text() {
        let source = include_str!("../../../completions/sbrs.usage.kdl");

        let spec = Spec::parse(source).unwrap();

        assert_eq!(spec.bin, "sbrs");
        // Every `cmd` and every `flag` line of the text became one.
        let lines_starting = |word: &str| {
            source
                .lines()
                .filter(|line| line.trim_start().starts_with(&format!("{word} ")))
                .count()
        };
        assert_eq!(spec.commands().len(), lines_starting("cmd"));
        let flags: usize = spec.root.flags.len()
            + spec
                .commands()
                .iter()
                .map(|command| command.flags.len())
                .sum::<usize>();
        assert_eq!(flags, lines_starting("flag"));
        assert!(spec.commands().len() > 100, "{}", spec.commands().len());
        let wait = spec
            .commands()
            .into_iter()
            .find(|c| c.path == "wait-for-text")
            .unwrap();
        let timeout = wait
            .flags
            .iter()
            .find(|f| f.names == ["--timeout"])
            .unwrap();
        assert_eq!(timeout.default.as_deref(), Some("10"));
        assert_eq!(timeout.value.as_deref(), Some("<TIMEOUT>"));
        let encrypt = spec
            .commands()
            .into_iter()
            .find(|c| c.path == "encrypt")
            .unwrap();
        assert_eq!(encrypt.aliases, ["obfuscate"]);
    }
}
